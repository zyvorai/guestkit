// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows registry **write** support via hand-rolled FFI to libhivex.
//!
//! The read path uses the pure-Rust `nt_hive2` crate (see [`super::hivex_ops`] and
//! [`super::windows_registry`]), which is read-only. Offline hive mutation requires
//! libhivex, which we bind directly here rather than pulling in the EUPL-licensed
//! `hivex` crate — libhivex itself is LGPL-2.1 and dynamically linked, so it does not
//! impose copyleft on GuestKit.
//!
//! Gated behind the `registry-write` feature; the build then links `libhivex`
//! (Debian/Ubuntu `libhivex-dev`, Fedora/EL `hivex-devel`).

use crate::core::{Error, Result};
use serde_json::Value;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::Path;

/// `HIVEX_OPEN_WRITE` — open the hive for writing.
const HIVEX_OPEN_WRITE: c_int = 4;

// libhivex uses `size_t` for node/value handles. `usize` matches on LP64/LLP64.
type HiveH = *mut std::ffi::c_void;
type HiveNodeH = usize;

/// Mirrors `struct hive_set_value` from `<hivex.h>`.
#[repr(C)]
struct HiveSetValue {
    key: *const c_char,
    t: c_int,
    len: usize,
    value: *const c_char,
}

// hive_type constants (subset we emit).
const REG_SZ: c_int = 1;
const REG_EXPAND_SZ: c_int = 2;
const REG_BINARY: c_int = 3;
const REG_DWORD: c_int = 4;
const REG_MULTI_SZ: c_int = 7;
const REG_QWORD: c_int = 11;

// libhivex also uses `size_t` for value handles.
type HiveValueH = usize;

#[link(name = "hivex")]
extern "C" {
    fn hivex_open(filename: *const c_char, flags: c_int) -> HiveH;
    fn hivex_close(h: HiveH) -> c_int;
    fn hivex_root(h: HiveH) -> HiveNodeH;
    fn hivex_node_get_child(h: HiveH, node: HiveNodeH, name: *const c_char) -> HiveNodeH;
    fn hivex_node_add_child(h: HiveH, parent: HiveNodeH, name: *const c_char) -> HiveNodeH;
    fn hivex_node_set_value(
        h: HiveH,
        node: HiveNodeH,
        val: *const HiveSetValue,
        flags: c_int,
    ) -> c_int;
    fn hivex_commit(h: HiveH, filename: *const c_char, flags: c_int) -> c_int;
    // Read/enumerate: `hivex_node_children` returns a malloc'd 0-terminated
    // array of child handles; `hivex_node_name` returns a malloc'd C string.
    fn hivex_node_children(h: HiveH, node: HiveNodeH) -> *mut HiveNodeH;
    fn hivex_node_name(h: HiveH, node: HiveNodeH) -> *mut c_char;
    fn hivex_node_delete_child(h: HiveH, node: HiveNodeH) -> c_int;
    // Value read: `hivex_node_get_value` returns 0 (not found) or a value
    // handle; `hivex_value_value` returns the malloc'd raw bytes and sets
    // `*t_rtn`/`*len_rtn` to the REG_* type and byte length.
    fn hivex_node_get_value(h: HiveH, node: HiveNodeH, key: *const c_char) -> HiveValueH;
    fn hivex_value_value(
        h: HiveH,
        value: HiveValueH,
        t_rtn: *mut c_int,
        len_rtn: *mut usize,
    ) -> *mut c_char;
    // Value enumeration: `hivex_node_values` returns a malloc'd 0-terminated
    // array of value handles under a node; `hivex_value_key` returns the
    // malloc'd C string name of a single value.
    fn hivex_node_values(h: HiveH, node: HiveNodeH) -> *mut HiveValueH;
    fn hivex_value_key(h: HiveH, value: HiveValueH) -> *mut c_char;
}

extern "C" {
    // libhivex returns malloc'd buffers the caller must free (libc, not libhivex).
    fn free(ptr: *mut std::ffi::c_void);
}

/// RAII wrapper so the hive is always closed, even on early return.
struct Hive(HiveH);

impl Hive {
    fn open_write(path: &Path) -> Result<Self> {
        let c = path_to_cstring(path)?;
        // SAFETY: `c` is a valid NUL-terminated C string for the duration of the call.
        let h = unsafe { hivex_open(c.as_ptr(), HIVEX_OPEN_WRITE) };
        if h.is_null() {
            return Err(Error::CommandFailed(format!(
                "hivex_open({}) failed: {}",
                path.display(),
                std::io::Error::last_os_error()
            )));
        }
        Ok(Hive(h))
    }

    fn commit(&mut self) -> Result<()> {
        // NULL filename => write back to the file it was opened from.
        // SAFETY: self.0 is a live hive handle.
        let rc = unsafe { hivex_commit(self.0, std::ptr::null(), 0) };
        if rc != 0 {
            return Err(Error::CommandFailed(format!(
                "hivex_commit failed: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(())
    }
}

impl Drop for Hive {
    fn drop(&mut self) {
        // SAFETY: self.0 is a live hive handle owned by this wrapper.
        unsafe {
            hivex_close(self.0);
        }
    }
}

/// Set (creating intermediate keys as needed) a single registry value in an
/// offline hive file that has already been downloaded to the host.
///
/// * `hive_file` — host path to the hive (e.g. a downloaded SOFTWARE/SYSTEM hive).
/// * `subpath` — key components **below the hive root** (the `HKLM\SOFTWARE`
///   prefix maps to the hive root, so pass only what follows it).
/// * `value_name` — value to set; empty string targets the key's default value.
/// * `data_type` — REG_* type string (`REG_SZ`, `REG_DWORD`, `String`, `DWORD`, …).
/// * `new_data` — the new value as JSON, interpreted per `data_type`.
pub fn set_registry_value(
    hive_file: &Path,
    subpath: &[String],
    value_name: &str,
    data_type: &str,
    new_data: &Value,
) -> Result<()> {
    let (reg_type, encoded) = encode_value(data_type, new_data)?;

    let mut hive = Hive::open_write(hive_file)?;

    // Navigate from root, creating missing intermediate keys.
    // SAFETY: hive.0 is live for all of these calls.
    let mut node: HiveNodeH = unsafe { hivex_root(hive.0) };
    if node == 0 {
        return Err(Error::CommandFailed(format!(
            "hivex_root failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    for component in subpath {
        let name = CString::new(component.as_str()).map_err(|_| {
            Error::InvalidOperation(format!("registry key component has NUL byte: {component}"))
        })?;
        let child = unsafe { hivex_node_get_child(hive.0, node, name.as_ptr()) };
        node = if child != 0 {
            child
        } else {
            let created = unsafe { hivex_node_add_child(hive.0, node, name.as_ptr()) };
            if created == 0 {
                return Err(Error::CommandFailed(format!(
                    "hivex_node_add_child({component}) failed: {}",
                    std::io::Error::last_os_error()
                )));
            }
            created
        };
    }

    let key = CString::new(value_name).map_err(|_| {
        Error::InvalidOperation(format!("registry value name has NUL byte: {value_name}"))
    })?;
    let set = HiveSetValue {
        key: key.as_ptr(),
        t: reg_type,
        len: encoded.len(),
        value: encoded.as_ptr() as *const c_char,
    };
    // SAFETY: `key` and `encoded` outlive this call; `set` points into both.
    let rc = unsafe { hivex_node_set_value(hive.0, node, &set, 0) };
    if rc != 0 {
        return Err(Error::CommandFailed(format!(
            "hivex_node_set_value({value_name}) failed: {}",
            std::io::Error::last_os_error()
        )));
    }

    hive.commit()?;
    Ok(())
}

/// The raw on-disk representation of a registry value: its REG_* type
/// constant and byte payload (REG_SZ/REG_EXPAND_SZ are UTF-16LE with a
/// trailing NUL, REG_DWORD/REG_QWORD are little-endian, REG_MULTI_SZ is
/// NUL-separated UTF-16LE with a double NUL terminator - the caller
/// decodes per `reg_type`, matching how [`encode_value`] encodes them).
pub struct RawRegistryValue {
    pub reg_type: i32,
    pub data: Vec<u8>,
}

/// Read a single registry value from an offline hive file, or `None` if
/// the key path or the value itself doesn't exist. Read-only (opens with
/// `HIVEX_OPEN_WRITE` regardless, matching every other function in this
/// module - libhivex permits reads on a write-mode handle, and this keeps
/// the module to one open mode rather than adding a second).
pub fn get_registry_value(
    hive_file: &Path,
    subpath: &[String],
    value_name: &str,
) -> Result<Option<RawRegistryValue>> {
    let hive = Hive::open_write(hive_file)?;
    let subpath_refs: Vec<&str> = subpath.iter().map(String::as_str).collect();

    // SAFETY: hive.0 stays live for the whole call.
    unsafe {
        let root = hivex_root(hive.0);
        if root == 0 {
            return Err(Error::CommandFailed("hivex_root failed".into()));
        }
        let Some(node) = navigate(hive.0, root, &subpath_refs) else {
            return Ok(None);
        };

        let key = CString::new(value_name).map_err(|_| {
            Error::InvalidOperation(format!("registry value name has NUL byte: {value_name}"))
        })?;
        let value = hivex_node_get_value(hive.0, node, key.as_ptr());
        if value == 0 {
            return Ok(None);
        }

        let mut t: c_int = 0;
        let mut len: usize = 0;
        let buf = hivex_value_value(hive.0, value, &mut t, &mut len);
        if buf.is_null() {
            return Ok(None);
        }
        let data = std::slice::from_raw_parts(buf as *const u8, len).to_vec();
        free(buf as *mut std::ffi::c_void);

        Ok(Some(RawRegistryValue { reg_type: t, data }))
    }
}

/// True if every component of `subpath` resolves to an existing key.
pub fn registry_key_exists(hive_file: &Path, subpath: &[String]) -> Result<bool> {
    let hive = Hive::open_write(hive_file)?;
    let subpath_refs: Vec<&str> = subpath.iter().map(String::as_str).collect();
    // SAFETY: hive.0 stays live for the whole call.
    unsafe {
        let root = hivex_root(hive.0);
        if root == 0 {
            return Err(Error::CommandFailed("hivex_root failed".into()));
        }
        Ok(navigate(hive.0, root, &subpath_refs).is_some())
    }
}

/// Delete the key at `subpath` (and everything under it) if it exists.
/// Returns whether a key was actually deleted. `subpath` must be
/// non-empty - deleting the hive root isn't supported (matches libhivex,
/// which has no such operation either).
pub fn delete_registry_key(hive_file: &Path, subpath: &[String]) -> Result<bool> {
    let Some((leaf, parent_path)) = subpath.split_last() else {
        return Err(Error::InvalidOperation(
            "delete_registry_key: subpath must be non-empty".into(),
        ));
    };

    let mut hive = Hive::open_write(hive_file)?;
    let parent_refs: Vec<&str> = parent_path.iter().map(String::as_str).collect();

    let deleted = {
        // SAFETY: hive.0 stays live for the whole traversal.
        unsafe {
            let root = hivex_root(hive.0);
            if root == 0 {
                return Err(Error::CommandFailed("hivex_root failed".into()));
            }
            let Some(parent) = navigate(hive.0, root, &parent_refs) else {
                return Ok(false);
            };
            let name = CString::new(leaf.as_str()).map_err(|_| {
                Error::InvalidOperation(format!("registry key component has NUL byte: {leaf}"))
            })?;
            let child = hivex_node_get_child(hive.0, parent, name.as_ptr());
            if child == 0 {
                false
            } else {
                hivex_node_delete_child(hive.0, child) == 0
            }
        }
    };

    if deleted {
        hive.commit()?;
    }
    Ok(deleted)
}

/// Encode a JSON value into libhivex's on-disk byte representation for `data_type`.
fn encode_value(data_type: &str, data: &Value) -> Result<(c_int, Vec<u8>)> {
    let norm = data_type.trim().to_ascii_uppercase();
    let norm = norm.strip_prefix("REG_").unwrap_or(&norm);

    match norm {
        "SZ" | "STRING" | "EXPAND_SZ" | "EXPANDSTRING" => {
            let s = data
                .as_str()
                .ok_or_else(|| Error::InvalidOperation("expected string data".into()))?;
            let t = if norm.starts_with("EXPAND") {
                REG_EXPAND_SZ
            } else {
                REG_SZ
            };
            Ok((t, utf16le_nul(s)))
        }
        "DWORD" | "DWORD_LITTLE_ENDIAN" => {
            let n = as_u64(data)?;
            Ok((REG_DWORD, (n as u32).to_le_bytes().to_vec()))
        }
        "QWORD" => {
            let n = as_u64(data)?;
            Ok((REG_QWORD, n.to_le_bytes().to_vec()))
        }
        "MULTI_SZ" | "MULTISTRING" => {
            let arr = data
                .as_array()
                .ok_or_else(|| Error::InvalidOperation("expected array for MULTI_SZ".into()))?;
            let mut bytes = Vec::new();
            for item in arr {
                let s = item.as_str().ok_or_else(|| {
                    Error::InvalidOperation("MULTI_SZ items must be strings".into())
                })?;
                bytes.extend_from_slice(&utf16le_nul(s));
            }
            bytes.extend_from_slice(&[0, 0]); // final terminating NUL
            Ok((REG_MULTI_SZ, bytes))
        }
        "BINARY" => Ok((REG_BINARY, decode_binary(data)?)),
        other => Err(Error::InvalidOperation(format!(
            "unsupported registry data type: {other}"
        ))),
    }
}

/// UTF-16LE encode with a trailing NUL, as Windows stores REG_SZ.
fn utf16le_nul(s: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

/// Accept a JSON number, or a decimal/0x-hex string, as an integer.
fn as_u64(data: &Value) -> Result<u64> {
    if let Some(n) = data.as_u64() {
        return Ok(n);
    }
    if let Some(s) = data.as_str() {
        let s = s.trim();
        let parsed = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            u64::from_str_radix(hex, 16)
        } else {
            s.parse::<u64>()
        };
        return parsed
            .map_err(|_| Error::InvalidOperation(format!("invalid integer registry data: {s}")));
    }
    Err(Error::InvalidOperation("expected integer data".into()))
}

/// Binary data as a JSON array of byte values, or a hex string.
fn decode_binary(data: &Value) -> Result<Vec<u8>> {
    if let Some(arr) = data.as_array() {
        return arr
            .iter()
            .map(|v| {
                v.as_u64()
                    .filter(|n| *n <= 0xFF)
                    .map(|n| n as u8)
                    .ok_or_else(|| Error::InvalidOperation("binary byte out of range".into()))
            })
            .collect();
    }
    if let Some(s) = data.as_str() {
        let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if !clean.len().is_multiple_of(2) {
            return Err(Error::InvalidOperation("odd-length hex binary data".into()));
        }
        return (0..clean.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&clean[i..i + 2], 16)
                    .map_err(|_| Error::InvalidOperation("invalid hex in binary data".into()))
            })
            .collect();
    }
    Err(Error::InvalidOperation(
        "expected byte array or hex string for REG_BINARY".into(),
    ))
}

fn path_to_cstring(path: &Path) -> Result<CString> {
    let s = path
        .to_str()
        .ok_or_else(|| Error::InvalidOperation("hive path is not valid UTF-8".into()))?;
    CString::new(s).map_err(|_| Error::InvalidOperation("hive path contains NUL byte".into()))
}

/// Read a hive node's name, or None. SAFETY: `h`/`node` must be live.
unsafe fn node_name(h: HiveH, node: HiveNodeH) -> Option<String> {
    let p = hivex_node_name(h, node);
    if p.is_null() {
        return None;
    }
    let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
    free(p as *mut std::ffi::c_void);
    Some(s)
}

/// Enumerate a node's child handles. SAFETY: `h`/`node` must be live.
unsafe fn node_children(h: HiveH, node: HiveNodeH) -> Vec<HiveNodeH> {
    let arr = hivex_node_children(h, node);
    let mut out = Vec::new();
    if arr.is_null() {
        return out;
    }
    let mut i = 0isize;
    loop {
        let val = *arr.offset(i);
        if val == 0 {
            break;
        }
        out.push(val);
        i += 1;
    }
    free(arr as *mut std::ffi::c_void);
    out
}

/// Enumerate a node's value handles. SAFETY: `h`/`node` must be live.
unsafe fn node_values(h: HiveH, node: HiveNodeH) -> Vec<HiveValueH> {
    let arr = hivex_node_values(h, node);
    let mut out = Vec::new();
    if arr.is_null() {
        return out;
    }
    let mut i = 0isize;
    loop {
        let val = *arr.offset(i);
        if val == 0 {
            break;
        }
        out.push(val);
        i += 1;
    }
    free(arr as *mut std::ffi::c_void);
    out
}

/// Read a value's name, or None. SAFETY: `h`/`value` must be live.
unsafe fn value_key(h: HiveH, value: HiveValueH) -> Option<String> {
    let p = hivex_value_key(h, value);
    if p.is_null() {
        return None;
    }
    let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
    free(p as *mut std::ffi::c_void);
    Some(s)
}

/// List the names of every value directly under the key at `subpath`
/// (not the empty-string "default" value, which libhivex represents the
/// same way as a named value - callers checking for it should look for
/// `""` in the result). Returns an empty vector if the key doesn't
/// exist. Matches the enumeration that `python-hivex`'s
/// `h.node_values(node)` combined with `h.value_key(v)` performs (e.g.
/// to find "any Run-key value whose name contains a marker string").
pub fn list_registry_value_names(hive_file: &Path, subpath: &[String]) -> Result<Vec<String>> {
    let hive = Hive::open_write(hive_file)?;
    let subpath_refs: Vec<&str> = subpath.iter().map(String::as_str).collect();
    // SAFETY: hive.0 stays live for the whole call.
    unsafe {
        let root = hivex_root(hive.0);
        if root == 0 {
            return Err(Error::CommandFailed("hivex_root failed".into()));
        }
        let Some(node) = navigate(hive.0, root, &subpath_refs) else {
            return Ok(Vec::new());
        };
        Ok(node_values(hive.0, node)
            .into_iter()
            .filter_map(|v| value_key(hive.0, v))
            .collect())
    }
}

/// Navigate from `start` down `subpath`, returning the node handle or None.
/// SAFETY: `h` must be live.
unsafe fn navigate(h: HiveH, start: HiveNodeH, subpath: &[&str]) -> Option<HiveNodeH> {
    let mut node = start;
    for comp in subpath {
        let name = CString::new(*comp).ok()?;
        let child = hivex_node_get_child(h, node, name.as_ptr());
        if child == 0 {
            return None;
        }
        node = child;
    }
    Some(node)
}

/// Set `CONFIGFLAG_REINSTALL` (0x20) on every PnP device instance under
/// `SYSTEM\<control_set>\Enum\PCI` whose device-key name contains any of
/// `hwid_needles` (case-insensitive). This asks Windows to re-run driver
/// installation for the device on next boot — the reliable offline way to make
/// a guest pick up a newly staged driver (e.g. virtio-serial) when the existing
/// cached devnode would otherwise never re-search for one. Returns how many
/// instance nodes were flagged.
pub fn set_configflags_reinstall(
    hive_file: &Path,
    control_set: &str,
    hwid_needles: &[&str],
) -> Result<usize> {
    let needles: Vec<String> = hwid_needles
        .iter()
        .map(|n| n.to_ascii_uppercase())
        .collect();
    let mut hive = Hive::open_write(hive_file)?;
    let mut flagged = 0usize;
    // SAFETY: hive.0 stays live for the whole traversal; all handles derive from it.
    unsafe {
        let root = hivex_root(hive.0);
        if root == 0 {
            return Err(Error::CommandFailed("hivex_root failed".into()));
        }
        let pci = match navigate(hive.0, root, &[control_set, "Enum", "PCI"]) {
            Some(n) => n,
            None => return Ok(0),
        };
        for dev in node_children(hive.0, pci) {
            let dev_name = node_name(hive.0, dev)
                .unwrap_or_default()
                .to_ascii_uppercase();
            if !needles.iter().any(|n| dev_name.contains(n.as_str())) {
                continue;
            }
            for inst in node_children(hive.0, dev) {
                let key = CString::new("ConfigFlags").unwrap();
                let data = 0x20u32.to_le_bytes();
                let set = HiveSetValue {
                    key: key.as_ptr(),
                    t: REG_DWORD,
                    len: data.len(),
                    value: data.as_ptr() as *const c_char,
                };
                if hivex_node_set_value(hive.0, inst, &set, 0) == 0 {
                    flagged += 1;
                }
            }
        }
    }
    hive.commit()?;
    Ok(flagged)
}

/// Delete every device key under `SYSTEM\<control_set>\Enum\PCI` whose name
/// contains any of `hwid_needles` (case-insensitive). Removing a device's cached
/// enumeration forces the PCI bus driver to re-detect it as brand-new on the next
/// boot and run a full PnP driver install — the strongest offline way to make a
/// guest install a driver for a device it previously left in a "no driver"
/// state. Returns how many device keys were deleted.
pub fn delete_device_nodes(
    hive_file: &Path,
    control_set: &str,
    hwid_needles: &[&str],
) -> Result<usize> {
    let needles: Vec<String> = hwid_needles
        .iter()
        .map(|n| n.to_ascii_uppercase())
        .collect();
    let mut hive = Hive::open_write(hive_file)?;
    let mut deleted = 0usize;
    // SAFETY: hive.0 stays live; all handles derive from it. We delete by handle
    // immediately after matching by name, before enumerating further siblings.
    unsafe {
        let root = hivex_root(hive.0);
        if root == 0 {
            return Err(Error::CommandFailed("hivex_root failed".into()));
        }
        let pci = match navigate(hive.0, root, &[control_set, "Enum", "PCI"]) {
            Some(n) => n,
            None => return Ok(0),
        };
        for dev in node_children(hive.0, pci) {
            let dev_name = node_name(hive.0, dev)
                .unwrap_or_default()
                .to_ascii_uppercase();
            if needles.iter().any(|n| dev_name.contains(n.as_str()))
                && hivex_node_delete_child(hive.0, dev) == 0
            {
                deleted += 1;
            }
        }
    }
    hive.commit()?;
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn utf16le_encodes_with_terminator() {
        // "AB" -> 41 00 42 00 00 00
        assert_eq!(utf16le_nul("AB"), vec![0x41, 0x00, 0x42, 0x00, 0x00, 0x00]);
        assert_eq!(utf16le_nul(""), vec![0x00, 0x00]);
    }

    #[test]
    fn dword_encoding() {
        let (t, b) = encode_value("REG_DWORD", &json!(1)).unwrap();
        assert_eq!(t, REG_DWORD);
        assert_eq!(b, vec![1, 0, 0, 0]);
        // hex string form
        let (_, b2) = encode_value("DWORD", &json!("0x100")).unwrap();
        assert_eq!(b2, vec![0, 1, 0, 0]);
    }

    #[test]
    fn qword_encoding() {
        let (t, b) = encode_value("REG_QWORD", &json!(258)).unwrap();
        assert_eq!(t, REG_QWORD);
        assert_eq!(b, vec![2, 1, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn multi_sz_has_double_terminator() {
        let (t, b) = encode_value("REG_MULTI_SZ", &json!(["A", "B"])).unwrap();
        assert_eq!(t, REG_MULTI_SZ);
        // "A\0" "B\0" then final "\0"
        assert_eq!(b, vec![0x41, 0, 0, 0, 0x42, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn binary_from_array_and_hex() {
        let (t, b) = encode_value("REG_BINARY", &json!([1, 2, 255])).unwrap();
        assert_eq!(t, REG_BINARY);
        assert_eq!(b, vec![1, 2, 255]);
        let (_, b2) = encode_value("BINARY", &json!("01 02 ff")).unwrap();
        assert_eq!(b2, vec![1, 2, 255]);
    }

    #[test]
    fn string_type() {
        let (t, _) = encode_value("String", &json!("Enabled")).unwrap();
        assert_eq!(t, REG_SZ);
        let (t2, _) = encode_value("REG_EXPAND_SZ", &json!("%SystemRoot%")).unwrap();
        assert_eq!(t2, REG_EXPAND_SZ);
    }

    #[test]
    fn rejects_bad_type() {
        assert!(encode_value("REG_WEIRD", &json!("x")).is_err());
    }

    /// Locate a real (non-Windows-specific) test hive shipped with the
    /// system's libhivex install, if any - used to exercise
    /// set/get/exists/delete against an actual regf file rather than
    /// mocks. Not every environment has this (e.g. CI images without
    /// hivex's test fixtures installed), so callers must skip gracefully
    /// when `None`.
    fn find_test_hive() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("HIVEX_TEST_HIVE") {
            let p = std::path::PathBuf::from(p);
            if p.is_file() {
                return Some(p);
            }
        }
        let glob_pattern = "/opt/homebrew/Cellar/hivex/*/share/hivex/test/large";
        for entry in glob::glob(glob_pattern).ok()?.flatten() {
            if entry.is_file() {
                return Some(entry);
            }
        }
        let fixed = std::path::PathBuf::from("/usr/share/hivex/test/large");
        if fixed.is_file() {
            return Some(fixed);
        }
        None
    }

    fn with_writable_copy(f: impl FnOnce(&std::path::Path)) {
        let Some(src) = find_test_hive() else {
            eprintln!(
                "skipping: no libhivex test fixture found (set HIVEX_TEST_HIVE to point at one)"
            );
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("test.hive");
        std::fs::copy(&src, &copy).unwrap();
        f(&copy);
    }

    #[test]
    fn set_then_get_registry_value_round_trips() {
        with_writable_copy(|hive| {
            let subpath = vec!["A".to_string()];
            set_registry_value(hive, &subpath, "MyValue", "REG_SZ", &json!("hello")).unwrap();

            let got = get_registry_value(hive, &subpath, "MyValue")
                .unwrap()
                .unwrap();
            assert_eq!(got.reg_type, REG_SZ);
            // REG_SZ is UTF-16LE with a trailing NUL.
            assert_eq!(got.data, utf16le_nul("hello"));
        });
    }

    #[test]
    fn get_registry_value_is_none_for_missing_key_or_value() {
        with_writable_copy(|hive| {
            assert!(get_registry_value(hive, &["DoesNotExist".to_string()], "X")
                .unwrap()
                .is_none());
            assert!(get_registry_value(hive, &["A".to_string()], "NoSuchValue")
                .unwrap()
                .is_none());
        });
    }

    #[test]
    fn registry_key_exists_reflects_real_state() {
        with_writable_copy(|hive| {
            assert!(registry_key_exists(hive, &["A".to_string()]).unwrap());
            assert!(!registry_key_exists(hive, &["NopeNotHere".to_string()]).unwrap());
        });
    }

    #[test]
    fn set_registry_value_creates_intermediate_keys() {
        with_writable_copy(|hive| {
            let subpath = vec!["Brand".to_string(), "New".to_string(), "Path".to_string()];
            assert!(!registry_key_exists(hive, &subpath).unwrap());
            set_registry_value(hive, &subpath, "V", "REG_DWORD", &json!(42)).unwrap();
            assert!(registry_key_exists(hive, &subpath).unwrap());
            let got = get_registry_value(hive, &subpath, "V").unwrap().unwrap();
            assert_eq!(got.reg_type, REG_DWORD);
            assert_eq!(got.data, vec![42, 0, 0, 0]);
        });
    }

    #[test]
    fn delete_registry_key_removes_it_and_is_idempotent() {
        with_writable_copy(|hive| {
            let subpath = vec!["A".to_string(), "ToDelete".to_string()];
            set_registry_value(hive, &subpath, "V", "REG_SZ", &json!("x")).unwrap();
            assert!(registry_key_exists(hive, &subpath).unwrap());

            assert!(delete_registry_key(hive, &subpath).unwrap());
            assert!(!registry_key_exists(hive, &subpath).unwrap());

            // Deleting again is a no-op, not an error.
            assert!(!delete_registry_key(hive, &subpath).unwrap());
        });
    }

    #[test]
    fn list_registry_value_names_reflects_real_state() {
        with_writable_copy(|hive| {
            let subpath = vec!["ListValuesTest".to_string()];
            set_registry_value(hive, &subpath, "vmware-tools", "REG_SZ", &json!("x")).unwrap();
            set_registry_value(hive, &subpath, "OtherApp", "REG_SZ", &json!("y")).unwrap();

            let mut names = list_registry_value_names(hive, &subpath).unwrap();
            names.sort();
            assert_eq!(
                names,
                vec!["OtherApp".to_string(), "vmware-tools".to_string()]
            );

            assert!(names.iter().any(|n| n.to_lowercase().contains("vmware")));
        });
    }

    #[test]
    fn list_registry_value_names_is_empty_for_missing_key() {
        with_writable_copy(|hive| {
            assert!(
                list_registry_value_names(hive, &["DoesNotExist".to_string()])
                    .unwrap()
                    .is_empty()
            );
        });
    }
}
