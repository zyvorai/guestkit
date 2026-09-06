// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline Windows local-account password clear / set via SAM.
//!
//! Prefer AES/RC4 SYSKEY hash write when a SYSTEM hive is available
//! ([`set_windows_password`]). Fall back to chntpw-style blank + first-boot
//! RunOnce `net user` when AES write is unavailable.
//!
//! Requires the `registry-write` feature (libhivex).

use crate::core::{Error, Result};
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::Path;

const HIVEX_OPEN_WRITE: c_int = 4;
const REG_BINARY: c_int = 3;

type HiveH = *mut std::ffi::c_void;
type HiveNodeH = usize;
type HiveValueH = usize;

#[repr(C)]
struct HiveSetValue {
    key: *const c_char,
    t: c_int,
    len: usize,
    value: *const c_char,
}

#[link(name = "hivex")]
extern "C" {
    fn hivex_open(filename: *const c_char, flags: c_int) -> HiveH;
    fn hivex_close(h: HiveH) -> c_int;
    fn hivex_root(h: HiveH) -> HiveNodeH;
    fn hivex_node_get_child(h: HiveH, node: HiveNodeH, name: *const c_char) -> HiveNodeH;
    fn hivex_node_get_value(h: HiveH, node: HiveNodeH, key: *const c_char) -> HiveValueH;
    fn hivex_value_value(h: HiveH, val: HiveValueH, t: *mut c_int, len: *mut usize) -> *mut c_char;
    fn hivex_value_type(h: HiveH, val: HiveValueH, t: *mut c_int, len: *mut usize) -> c_int;
    fn hivex_node_set_value(
        h: HiveH,
        node: HiveNodeH,
        val: *const HiveSetValue,
        flags: c_int,
    ) -> c_int;
    fn hivex_commit(h: HiveH, filename: *const c_char, flags: c_int) -> c_int;
}

extern "C" {
    fn free(ptr: *mut std::ffi::c_void);
}

/// Clear the NT password for `username` in an offline SAM hive (chntpw blank).
pub fn clear_windows_password(sam_hive: &Path, username: &str) -> Result<()> {
    let path = path_cstring(sam_hive)?;
    let h = unsafe { hivex_open(path.as_ptr(), HIVEX_OPEN_WRITE) };
    if h.is_null() {
        return Err(Error::CommandFailed(format!(
            "hivex_open({}) failed: {}",
            sam_hive.display(),
            std::io::Error::last_os_error()
        )));
    }
    struct Guard(HiveH);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                hivex_close(self.0);
            }
        }
    }
    let guard = Guard(h);

    let rid = lookup_rid(guard.0, username)?;
    let rid_hex = format!("{rid:08X}");
    let user_node = navigate(guard.0, &["SAM", "Domains", "Account", "Users", &rid_hex])?;

    let mut v = get_binary_value(guard.0, user_node, "V")?;
    if v.len() < 0xB0 {
        return Err(Error::InvalidOperation(format!(
            "SAM V value for '{username}' is too short ({} bytes)",
            v.len()
        )));
    }
    v[0xA0..0xA4].copy_from_slice(&4u32.to_le_bytes());
    v[0xAC..0xB0].copy_from_slice(&4u32.to_le_bytes());
    set_binary_value(guard.0, user_node, "V", &v)?;
    clear_account_disabled(guard.0, user_node)?;

    let rc = unsafe { hivex_commit(guard.0, std::ptr::null(), 0) };
    if rc != 0 {
        return Err(Error::CommandFailed(format!(
            "hivex_commit failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Result of offline Windows password set.
#[derive(Debug, Clone)]
pub struct WindowsPasswordSetResult {
    pub username: String,
    pub aes_written: bool,
    pub sam_blanked: bool,
    pub runonce_staged: bool,
}

/// Set a Windows local-account password offline.
///
/// Tries SYSKEY AES/RC4 NT-hash write when `system_hive` is provided; otherwise
/// (or on failure) blanks SAM and stages SOFTWARE RunOnce `net user`.
pub fn set_windows_password(
    sam_hive: &Path,
    software_hive: &Path,
    system_hive: Option<&Path>,
    username: &str,
    password: &str,
) -> Result<WindowsPasswordSetResult> {
    validate_windows_password(password)?;
    validate_windows_username(username)?;

    if let Some(sys) = system_hive {
        match crate::guestfs::sam_aes::set_password_aes(sam_hive, sys, username, password) {
            Ok(()) => {
                return Ok(WindowsPasswordSetResult {
                    username: username.to_string(),
                    aes_written: true,
                    sam_blanked: false,
                    runonce_staged: false,
                });
            }
            Err(e) => {
                eprintln!(
                    "Warning: AES/RC4 SAM hash write failed ({e}); falling back to blank + RunOnce"
                );
            }
        }
    }

    clear_windows_password(sam_hive, username)?;

    let cmd = runonce_net_user_command(username, password);
    crate::guestfs::hivex_ffi::set_registry_value(
        software_hive,
        &[
            "Microsoft".into(),
            "Windows".into(),
            "CurrentVersion".into(),
            "RunOnce".into(),
        ],
        "GuestKitSetPassword",
        "REG_SZ",
        &serde_json::Value::String(cmd),
    )?;

    Ok(WindowsPasswordSetResult {
        username: username.to_string(),
        aes_written: false,
        sam_blanked: true,
        runonce_staged: true,
    })
}

/// Encrypt `nt_hash` with the domain hashed bootkey and write into the user's `V`.
pub fn set_user_nt_hash_encrypted(
    sam_hive: &Path,
    username: &str,
    bootkey: &[u8; 16],
    nt_hash: &[u8; 16],
) -> Result<()> {
    let path = path_cstring(sam_hive)?;
    let h = unsafe { hivex_open(path.as_ptr(), HIVEX_OPEN_WRITE) };
    if h.is_null() {
        return Err(Error::CommandFailed(format!(
            "hivex_open({}) failed: {}",
            sam_hive.display(),
            std::io::Error::last_os_error()
        )));
    }
    struct Guard(HiveH);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                hivex_close(self.0);
            }
        }
    }
    let guard = Guard(h);

    let account = navigate(guard.0, &["SAM", "Domains", "Account"])?;
    let domain_f = get_binary_value(guard.0, account, "F")?;
    let hboot = crate::guestfs::sam_aes::hashed_bootkey(&domain_f, bootkey)?;
    let aes_style = crate::guestfs::sam_aes::domain_f_is_aes(&domain_f);

    let rid = lookup_rid(guard.0, username)?;
    let rid_hex = format!("{rid:08X}");
    let user_node = navigate(guard.0, &["SAM", "Domains", "Account", "Users", &rid_hex])?;

    let salt = crate::guestfs::sam_aes::random_salt();
    let blob = crate::guestfs::sam_aes::encrypt_nt_hash(rid, nt_hash, &hboot, &salt, aes_style)?;

    let mut v = get_binary_value(guard.0, user_node, "V")?;
    crate::guestfs::sam_aes::patch_v_with_nt_hash(&mut v, &blob)?;
    set_binary_value(guard.0, user_node, "V", &v)?;
    clear_account_disabled(guard.0, user_node)?;

    let rc = unsafe { hivex_commit(guard.0, std::ptr::null(), 0) };
    if rc != 0 {
        return Err(Error::CommandFailed(format!(
            "hivex_commit failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Build the RunOnce command string (also used by unit tests).
pub fn runonce_net_user_command(username: &str, password: &str) -> String {
    let u = quote_cmd_arg(username);
    let p = quote_cmd_arg(password);
    format!(
        "cmd.exe /c net user {u} {p} && reg delete \"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce\" /v GuestKitSetPassword /f"
    )
}

fn clear_account_disabled(h: HiveH, user_node: HiveNodeH) -> Result<()> {
    if let Ok(mut f) = get_binary_value(h, user_node, "F") {
        if f.len() >= 0x3C {
            let flags = u32::from_le_bytes(f[0x38..0x3C].try_into().unwrap());
            let cleared = flags & !0x0002;
            if cleared != flags {
                f[0x38..0x3C].copy_from_slice(&cleared.to_le_bytes());
                set_binary_value(h, user_node, "F", &f)?;
            }
        }
    }
    Ok(())
}

fn quote_cmd_arg(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn validate_windows_username(username: &str) -> Result<()> {
    let u = username.trim();
    if u.is_empty() || u.len() > 20 {
        return Err(Error::InvalidOperation(
            "Windows username must be 1–20 characters".into(),
        ));
    }
    if u.contains([
        '\\', '/', '[', ']', ':', ';', '|', '=', ',', '+', '*', '?', '<', '>', '@',
    ]) {
        return Err(Error::InvalidOperation(format!(
            "Windows username contains illegal characters: {u}"
        )));
    }
    Ok(())
}

fn validate_windows_password(password: &str) -> Result<()> {
    if password.is_empty() {
        return Err(Error::InvalidOperation(
            "password must be non-empty (omit --password to only blank the SAM)".into(),
        ));
    }
    if password.len() > 127 {
        return Err(Error::InvalidOperation(
            "Windows password must be at most 127 characters".into(),
        ));
    }
    Ok(())
}

fn lookup_rid(h: HiveH, username: &str) -> Result<u32> {
    let names = navigate(h, &["SAM", "Domains", "Account", "Users", "Names"])?;
    let user_c = CString::new(username)
        .map_err(|_| Error::InvalidOperation(format!("username has NUL: {username}")))?;
    let node = unsafe { hivex_node_get_child(h, names, user_c.as_ptr()) };
    if node == 0 {
        return Err(Error::InvalidOperation(format!(
            "Windows user '{username}' not found in SAM"
        )));
    }
    let empty = CString::new("").unwrap();
    let val = unsafe { hivex_node_get_value(h, node, empty.as_ptr()) };
    if val == 0 {
        return Err(Error::InvalidOperation(format!(
            "SAM Names\\{username} has no default value (RID)"
        )));
    }
    let mut rid_type: c_int = 0;
    let mut rid_len: usize = 0;
    let rc = unsafe { hivex_value_type(h, val, &mut rid_type, &mut rid_len) };
    if rc != 0 || rid_type <= 0 {
        return Err(Error::InvalidOperation(format!(
            "SAM Names\\{username} RID lookup failed"
        )));
    }
    // chntpw-style quirk: the RID is encoded in the value's *type* field, not its data.
    Ok(rid_type as u32)
}

fn navigate(h: HiveH, path: &[&str]) -> Result<HiveNodeH> {
    let mut node = unsafe { hivex_root(h) };
    if node == 0 {
        return Err(Error::CommandFailed("hivex_root failed".into()));
    }
    for component in path {
        let name = CString::new(*component)
            .map_err(|_| Error::InvalidOperation(format!("registry path has NUL: {component}")))?;
        let child = unsafe { hivex_node_get_child(h, node, name.as_ptr()) };
        if child == 0 {
            return Err(Error::InvalidOperation(format!(
                "SAM path missing component '{component}'"
            )));
        }
        node = child;
    }
    Ok(node)
}

fn get_binary_value(h: HiveH, node: HiveNodeH, key: &str) -> Result<Vec<u8>> {
    let key_c = CString::new(key)
        .map_err(|_| Error::InvalidOperation(format!("value name has NUL: {key}")))?;
    let val = unsafe { hivex_node_get_value(h, node, key_c.as_ptr()) };
    if val == 0 {
        return Err(Error::InvalidOperation(format!(
            "SAM value '{key}' not found"
        )));
    }
    let mut t: c_int = 0;
    let mut len: usize = 0;
    let ptr = unsafe { hivex_value_value(h, val, &mut t, &mut len) };
    if ptr.is_null() {
        return Err(Error::CommandFailed(format!(
            "hivex_value_value({key}) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }.to_vec();
    unsafe { free(ptr as *mut _) };
    let _ = t;
    Ok(bytes)
}

fn set_binary_value(h: HiveH, node: HiveNodeH, key: &str, data: &[u8]) -> Result<()> {
    let key_c = CString::new(key)
        .map_err(|_| Error::InvalidOperation(format!("value name has NUL: {key}")))?;
    let set = HiveSetValue {
        key: key_c.as_ptr(),
        t: REG_BINARY,
        len: data.len(),
        value: data.as_ptr() as *const c_char,
    };
    let rc = unsafe { hivex_node_set_value(h, node, &set, 0) };
    if rc != 0 {
        return Err(Error::CommandFailed(format!(
            "hivex_node_set_value({key}) failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

fn path_cstring(path: &Path) -> Result<CString> {
    let s = path
        .to_str()
        .ok_or_else(|| Error::InvalidOperation(format!("path not UTF-8: {}", path.display())))?;
    CString::new(s).map_err(|_| Error::InvalidOperation(format!("path has NUL: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rid_hex_format() {
        assert_eq!(format!("{:08X}", 500u32), "000001F4");
        assert_eq!(format!("{:08X}", 1001u32), "000003E9");
    }

    #[test]
    fn runonce_command_quotes_and_self_deletes() {
        let cmd = runonce_net_user_command("Admin", r#"p@ss"word"#);
        assert!(cmd.contains("net user \"Admin\""));
        assert!(cmd.contains("\"p@ss\"\"word\""));
        assert!(cmd.contains("GuestKitSetPassword"));
        assert!(cmd.contains("RunOnce"));
    }

    #[test]
    fn password_validation() {
        assert!(validate_windows_password("").is_err());
        assert!(validate_windows_password("ok").is_ok());
        assert!(validate_windows_password(&"x".repeat(128)).is_err());
    }

    #[test]
    fn username_validation() {
        assert!(validate_windows_username("Administrator").is_ok());
        assert!(validate_windows_username("bad:name").is_err());
        assert!(validate_windows_username("").is_err());
    }
}
