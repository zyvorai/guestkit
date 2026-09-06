// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline Windows SAM password hash write (RC4 + AES / SYSKEY).
//!
//! Reconstructs the boot key from SYSTEM LSA class names, derives the hashed
//! boot key from `SAM\Domains\Account\F`, then encrypts an MD4 NTLM hash with
//! RID-keyed DES and AES-128-CBC (Win10 AU+) or RC4 (legacy), writing the
//! result into the user's `V` value — matching Impacket `secretsdump` edit.

use crate::core::{Error, Result};
use aes::Aes128;
use cbc::{Decryptor as AesCbcDec, Encryptor as AesCbcEnc};
use cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use des::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use des::Des;
use md4::Md4;
use md5::{Digest, Md5};
use rand::RngCore;
use std::fs;
use std::path::Path;

const BOOTKEY_TRANSFORM: [usize; 16] = [8, 5, 4, 2, 11, 9, 13, 3, 0, 6, 1, 12, 14, 10, 15, 7];
const QWERTY: &[u8] = b"!@#$%^&*()qwertyUIOPAzxcvbnmQQQQQQQQQQQQ)(*@&%\0";
const DIGITS: &[u8] = b"0123456789012345678901234567890123456789\0";
const NTPASSWORD: &[u8] = b"NTPASSWORD\0";

const V_DATA_BASE: usize = 0xCC;
#[allow(dead_code)] // used by patch_v_with_nt_hash (registry-write / tests)
const V_LM_LENGTH: usize = 0xA0;
#[allow(dead_code)]
const V_NT_OFFSET: usize = 0xA8;
#[allow(dead_code)]
const V_NT_LENGTH: usize = 0xAC;

/// Write an AES/RC4-encrypted NT hash for `username` into an offline SAM hive.
///
/// Requires the companion SYSTEM hive to reconstruct the boot key.
/// Needs the `registry-write` feature (libhivex) for the hive mutation path.
#[cfg(feature = "registry-write")]
pub fn set_password_aes(
    sam_hive: &Path,
    system_hive: &Path,
    username: &str,
    password: &str,
) -> Result<()> {
    let bootkey = extract_bootkey(system_hive)?;
    let nt_hash = ntlm_hash(password);

    crate::guestfs::sam_password::set_user_nt_hash_encrypted(sam_hive, username, &bootkey, &nt_hash)
}

/// MD4(UTF-16LE(password)) — NTLM hash.
pub fn ntlm_hash(password: &str) -> [u8; 16] {
    let utf16: Vec<u8> = password
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    let digest = Md4::digest(&utf16);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

/// Reconstruct the 16-byte boot key from SYSTEM hive LSA class names.
pub fn extract_bootkey(system_hive: &Path) -> Result<[u8; 16]> {
    let data = fs::read(system_hive).map_err(|e| {
        Error::CommandFailed(format!("read SYSTEM hive {}: {e}", system_hive.display()))
    })?;
    if data.len() < 0x1000 || &data[0..4] != b"regf" {
        return Err(Error::InvalidFormat(
            "SYSTEM path is not a registry hive (missing regf header)".into(),
        ));
    }

    let current = select_current(&data)?;
    let cs = format!("ControlSet{current:03}");
    let mut scrambled = Vec::with_capacity(16);
    for name in ["JD", "Skew1", "GBG", "Data"] {
        let class = read_key_classname(&data, &[&cs, "Control", "Lsa", name])?;
        let bytes = decode_hex_ascii(&class).map_err(|e| {
            Error::InvalidFormat(format!("Lsa\\{name} class name not hex ({class}): {e}"))
        })?;
        if bytes.len() != 4 {
            return Err(Error::InvalidFormat(format!(
                "Lsa\\{name} class name decoded to {} bytes (want 4)",
                bytes.len()
            )));
        }
        scrambled.extend_from_slice(&bytes);
    }
    if scrambled.len() != 16 {
        return Err(Error::InvalidFormat("bootkey scramble length != 16".into()));
    }
    let mut bootkey = [0u8; 16];
    for (i, &t) in BOOTKEY_TRANSFORM.iter().enumerate() {
        bootkey[i] = scrambled[t];
    }
    Ok(bootkey)
}

/// Derive the hashed boot key from `SAM\Domains\Account\F` + bootkey.
pub fn hashed_bootkey(domain_f: &[u8], bootkey: &[u8; 16]) -> Result<Vec<u8>> {
    if domain_f.len() < 0x68 + 4 {
        return Err(Error::InvalidFormat(
            "SAM Domains\\Account\\F too short for Key0".into(),
        ));
    }
    let key0 = &domain_f[0x68..];
    match key0.first().copied() {
        Some(0x01) => hashed_bootkey_rc4(key0, bootkey),
        Some(0x02) => hashed_bootkey_aes(key0, bootkey),
        other => Err(Error::InvalidFormat(format!(
            "unsupported SAM Key0 revision {other:?}"
        ))),
    }
}

fn hashed_bootkey_rc4(key0: &[u8], bootkey: &[u8; 16]) -> Result<Vec<u8>> {
    // SAM_KEY_DATA: Revision(4) Length(4) Salt(16) Key(16) CheckSum(16)
    if key0.len() < 56 {
        return Err(Error::InvalidFormat("SAM_KEY_DATA too short".into()));
    }
    let salt = &key0[8..24];
    let key_cs = &key0[24..56];
    let mut md5 = Md5::new();
    md5.update(salt);
    md5.update(QWERTY);
    md5.update(bootkey);
    md5.update(DIGITS);
    let rc4_key = md5.finalize();
    let hashed = rc4_crypt(&rc4_key, key_cs);
    let check = {
        let mut m = Md5::new();
        m.update(&hashed[..16]);
        m.update(DIGITS);
        m.update(&hashed[..16]);
        m.update(QWERTY);
        m.finalize()
    };
    if check.as_slice() != &hashed[16..] {
        return Err(Error::InvalidOperation(
            "hashedBootKey checksum failed (Syskey startup password in use?)".into(),
        ));
    }
    Ok(hashed)
}

fn hashed_bootkey_aes(key0: &[u8], bootkey: &[u8; 16]) -> Result<Vec<u8>> {
    // SAM_KEY_DATA_AES: Rev(4) Len(4) CheckSumLen(4) DataLen(4) Salt(16) Data
    if key0.len() < 32 {
        return Err(Error::InvalidFormat("SAM_KEY_DATA_AES too short".into()));
    }
    let data_len = u32::from_le_bytes(key0[12..16].try_into().unwrap()) as usize;
    let salt = &key0[16..32];
    let data = key0
        .get(32..32 + data_len)
        .ok_or_else(|| Error::InvalidFormat("SAM_KEY_DATA_AES Data truncated".into()))?;
    aes_cbc_decrypt(bootkey, salt, data)
}

/// Encrypt plaintext NT hash for storage in SAM `V` (returns SAM_HASH or SAM_HASH_AES bytes).
pub fn encrypt_nt_hash(
    rid: u32,
    nt_hash: &[u8; 16],
    hbootkey: &[u8],
    salt: &[u8; 16],
    aes_style: bool,
) -> Result<Vec<u8>> {
    let des_enc = des_encrypt_hash(rid, nt_hash);
    if aes_style {
        let enc = aes_cbc_encrypt(&hbootkey[..16], salt, &des_enc)?;
        // SAM_HASH_AES: PekID=0, Revision=2, DataOffset=16, Salt, Hash
        let mut out = Vec::with_capacity(56);
        out.extend_from_slice(&0u16.to_le_bytes()); // PekID
        out.extend_from_slice(&2u16.to_le_bytes()); // Revision (AES)
        out.extend_from_slice(&16u32.to_le_bytes()); // DataOffset
        out.extend_from_slice(salt);
        out.extend_from_slice(&enc);
        Ok(out)
    } else {
        let mut md5 = Md5::new();
        md5.update(&hbootkey[..16]);
        md5.update(rid.to_le_bytes());
        md5.update(NTPASSWORD);
        let rc4_key = md5.finalize();
        let enc = rc4_crypt(&rc4_key, &des_enc);
        let mut out = Vec::with_capacity(20);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&enc);
        Ok(out)
    }
}

/// Decrypt stored SAM NT hash blob (for round-trip tests).
pub fn decrypt_nt_hash(
    rid: u32,
    blob: &[u8],
    hbootkey: &[u8],
    aes_style: bool,
) -> Result<[u8; 16]> {
    let des_enc = if aes_style {
        if blob.len() < 24 {
            return Err(Error::InvalidFormat("SAM_HASH_AES too short".into()));
        }
        let salt = &blob[8..24];
        let hash = &blob[24..];
        let plain = aes_cbc_decrypt(&hbootkey[..16], salt, hash)?;
        plain[..16].to_vec()
    } else {
        if blob.len() < 20 {
            return Err(Error::InvalidFormat("SAM_HASH too short".into()));
        }
        let enc = &blob[4..20];
        let mut md5 = Md5::new();
        md5.update(&hbootkey[..16]);
        md5.update(rid.to_le_bytes());
        md5.update(NTPASSWORD);
        let rc4_key = md5.finalize();
        rc4_crypt(&rc4_key, enc)
    };
    Ok(des_decrypt_hash(rid, &des_enc))
}

#[cfg_attr(not(feature = "registry-write"), allow(dead_code))]
pub(crate) fn domain_f_is_aes(domain_f: &[u8]) -> bool {
    domain_f.get(0x68).copied() == Some(0x02)
}

#[cfg_attr(not(feature = "registry-write"), allow(dead_code))]
pub(crate) fn patch_v_with_nt_hash(v: &mut Vec<u8>, nt_blob: &[u8]) -> Result<()> {
    if v.len() < V_DATA_BASE {
        return Err(Error::InvalidOperation(format!(
            "SAM V too short ({} bytes)",
            v.len()
        )));
    }
    // Prefer LM empty (length 4).
    v[V_LM_LENGTH..V_LM_LENGTH + 4].copy_from_slice(&4u32.to_le_bytes());

    let cur_len = u32::from_le_bytes(v[V_NT_LENGTH..V_NT_LENGTH + 4].try_into().unwrap()) as usize;
    let cur_off = u32::from_le_bytes(v[V_NT_OFFSET..V_NT_OFFSET + 4].try_into().unwrap()) as usize;
    let abs = V_DATA_BASE + cur_off;

    if cur_len == nt_blob.len() && abs + cur_len <= v.len() && cur_len >= 20 {
        // In-place replace (preserve surrounding layout).
        // For AES, regenerate salt already embedded in nt_blob.
        v[abs..abs + cur_len].copy_from_slice(nt_blob);
        return Ok(());
    }

    // Append at end and retarget offsets.
    let new_off = v.len() - V_DATA_BASE;
    v.extend_from_slice(nt_blob);
    v[V_NT_OFFSET..V_NT_OFFSET + 4].copy_from_slice(&(new_off as u32).to_le_bytes());
    v[V_NT_LENGTH..V_NT_LENGTH + 4].copy_from_slice(&(nt_blob.len() as u32).to_le_bytes());
    Ok(())
}

fn des_encrypt_hash(rid: u32, nt_hash: &[u8; 16]) -> [u8; 16] {
    let (k1, k2) = derive_des_keys(rid);
    let mut out = [0u8; 16];
    des_ecb_encrypt(&k1, &nt_hash[..8], &mut out[..8]);
    des_ecb_encrypt(&k2, &nt_hash[8..], &mut out[8..]);
    out
}

fn des_decrypt_hash(rid: u32, enc: &[u8]) -> [u8; 16] {
    let (k1, k2) = derive_des_keys(rid);
    let mut out = [0u8; 16];
    des_ecb_decrypt(&k1, &enc[..8], &mut out[..8]);
    des_ecb_decrypt(&k2, &enc[8..16], &mut out[8..]);
    out
}

fn derive_des_keys(rid: u32) -> ([u8; 8], [u8; 8]) {
    let key = rid.to_le_bytes();
    let s1 = [key[0], key[1], key[2], key[3], key[0], key[1], key[2]];
    let s2 = [key[3], key[0], key[1], key[2], key[3], key[0], key[1]];
    (transform_des_key(&s1), transform_des_key(&s2))
}

/// [MS-SAMR] / Impacket `transformKey`: expand 7 bytes → 8-byte DES key with odd parity.
fn transform_des_key(input: &[u8; 7]) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[0] = input[0] >> 1;
    out[1] = ((input[0] & 0x01) << 6) | (input[1] >> 2);
    out[2] = ((input[1] & 0x03) << 5) | (input[2] >> 3);
    out[3] = ((input[2] & 0x07) << 4) | (input[3] >> 4);
    out[4] = ((input[3] & 0x0F) << 3) | (input[4] >> 5);
    out[5] = ((input[4] & 0x1F) << 2) | (input[5] >> 6);
    out[6] = ((input[5] & 0x3F) << 1) | (input[6] >> 7);
    out[7] = input[6] & 0x7F;
    for b in &mut out {
        *b = (*b << 1) & 0xfe;
    }
    out
}

fn des_ecb_encrypt(key: &[u8; 8], block: &[u8], out: &mut [u8]) {
    let cipher = Des::new_from_slice(key).expect("8-byte DES key");
    let mut b = [0u8; 8];
    b.copy_from_slice(block);
    cipher.encrypt_block((&mut b).into());
    out.copy_from_slice(&b);
}

fn des_ecb_decrypt(key: &[u8; 8], block: &[u8], out: &mut [u8]) {
    let cipher = Des::new_from_slice(key).expect("8-byte DES key");
    let mut b = [0u8; 8];
    b.copy_from_slice(block);
    cipher.decrypt_block((&mut b).into());
    out.copy_from_slice(&b);
}

fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let key: [u8; 16] = key
        .try_into()
        .map_err(|_| Error::InvalidFormat("AES key must be 16 bytes".into()))?;
    let iv: [u8; 16] = iv
        .try_into()
        .map_err(|_| Error::InvalidFormat("AES IV must be 16 bytes".into()))?;
    let enc = AesCbcEnc::<Aes128>::new((&key).into(), (&iv).into());
    Ok(enc.encrypt_padded_vec_mut::<Pkcs7>(plaintext))
}

fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let key: [u8; 16] = key
        .try_into()
        .map_err(|_| Error::InvalidFormat("AES key must be 16 bytes".into()))?;
    let iv: [u8; 16] = iv
        .try_into()
        .map_err(|_| Error::InvalidFormat("AES IV must be 16 bytes".into()))?;
    if !ciphertext.len().is_multiple_of(16) {
        return Err(Error::InvalidFormat(
            "AES ciphertext length not multiple of 16".into(),
        ));
    }
    // Impacket decryptAES returns raw CBC output (no PKCS7 strip).
    let dec = AesCbcDec::<Aes128>::new((&key).into(), (&iv).into());
    let mut buf = ciphertext.to_vec();
    let pt = dec
        .decrypt_padded_mut::<cipher::block_padding::NoPadding>(&mut buf)
        .map_err(|e| Error::InvalidFormat(format!("AES decrypt: {e:?}")))?;
    Ok(pt.to_vec())
}

/// RC4 (ARC4) stream cipher used by legacy SAM.
fn rc4_crypt(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut j: u32 = 0;
    for i in 0..256 {
        j = (j + s[i] as u32 + key[i % key.len()] as u32) & 0xff;
        s.swap(i, j as usize);
    }
    let mut i = 0u32;
    j = 0;
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        i = (i + 1) & 0xff;
        j = (j + s[i as usize] as u32) & 0xff;
        s.swap(i as usize, j as usize);
        let k = s[((s[i as usize] as u32 + s[j as usize] as u32) & 0xff) as usize];
        out.push(b ^ k);
    }
    out
}

fn select_current(hive: &[u8]) -> Result<u32> {
    // Prefer ControlSet001 when Select is missing; otherwise read Select\Current.
    match read_dword_value(hive, &["Select"], "Current") {
        Ok(v) if (1..=3).contains(&v) => Ok(v),
        _ => {
            // Fall back: try ControlSet001 path exists
            if key_exists(hive, &["ControlSet001", "Control", "Lsa", "JD"]) {
                Ok(1)
            } else if key_exists(hive, &["ControlSet002", "Control", "Lsa", "JD"]) {
                Ok(2)
            } else {
                Err(Error::InvalidFormat(
                    "could not resolve SYSTEM CurrentControlSet".into(),
                ))
            }
        }
    }
}

fn key_exists(hive: &[u8], path: &[&str]) -> bool {
    find_key_cell(hive, path).is_ok()
}

fn read_key_classname(hive: &[u8], path: &[&str]) -> Result<String> {
    let cell = find_key_cell(hive, path)?;
    let nk = parse_nk(hive, cell)?;
    if nk.class_len == 0 || nk.class_off == 0 || nk.class_off == 0xffff_ffff {
        return Err(Error::InvalidFormat(format!(
            "key {} has empty class name",
            path.join("\\")
        )));
    }
    let class_cell = cell_data_offset(nk.class_off)?;
    let bytes = hive
        .get(class_cell..class_cell + nk.class_len as usize)
        .ok_or_else(|| Error::InvalidFormat("class name truncated".into()))?;
    // Class names are ASCII hex stored as UTF-16LE (or compressed ASCII).
    let s = if nk.flags & 0x20 != 0 {
        // KEY_COMP_NAME — name is ASCII; class is typically still UTF-16
        decode_utf16le_lossy(bytes)
    } else {
        decode_utf16le_lossy(bytes)
    };
    Ok(s.trim_end_matches('\0').to_string())
}

fn read_dword_value(hive: &[u8], path: &[&str], value: &str) -> Result<u32> {
    let cell = find_key_cell(hive, path)?;
    let nk = parse_nk(hive, cell)?;
    let vals = read_values(hive, &nk)?;
    for (name, data) in vals {
        if name.eq_ignore_ascii_case(value) && data.len() >= 4 {
            return Ok(u32::from_le_bytes(data[..4].try_into().unwrap()));
        }
    }
    Err(Error::NotFound(format!("{}\\{}", path.join("\\"), value)))
}

fn find_key_cell(hive: &[u8], path: &[&str]) -> Result<usize> {
    let root_off = u32::from_le_bytes(hive[0x24..0x28].try_into().unwrap());
    let mut cell = cell_data_offset(root_off)?;
    for component in path {
        let nk = parse_nk(hive, cell)?;
        let children = list_subkeys(hive, &nk)?;
        let want = component.to_ascii_lowercase();
        let next = children
            .into_iter()
            .find(|(n, _)| n.to_ascii_lowercase() == want)
            .map(|(_, c)| c)
            .ok_or_else(|| Error::NotFound(format!("SYSTEM path missing '{}'", path.join("\\"))))?;
        cell = next;
    }
    Ok(cell)
}

struct NkInfo {
    flags: u16,
    subkey_count: u32,
    subkeys_off: u32,
    value_count: u32,
    values_off: u32,
    class_off: u32,
    class_len: u16,
    name: String,
}

fn parse_nk(hive: &[u8], data_off: usize) -> Result<NkInfo> {
    let d = hive
        .get(data_off..)
        .ok_or_else(|| Error::InvalidFormat("nk truncated".into()))?;
    if d.len() < 0x4C || &d[0..2] != b"nk" {
        return Err(Error::InvalidFormat("not an nk cell".into()));
    }
    let flags = u16::from_le_bytes(d[0x02..0x04].try_into().unwrap());
    let subkey_count = u32::from_le_bytes(d[0x14..0x18].try_into().unwrap());
    let subkeys_off = u32::from_le_bytes(d[0x1C..0x20].try_into().unwrap());
    let value_count = u32::from_le_bytes(d[0x24..0x28].try_into().unwrap());
    let values_off = u32::from_le_bytes(d[0x28..0x2C].try_into().unwrap());
    let class_off = u32::from_le_bytes(d[0x30..0x34].try_into().unwrap());
    let name_len = u16::from_le_bytes(d[0x48..0x4A].try_into().unwrap());
    let class_len = u16::from_le_bytes(d[0x4A..0x4C].try_into().unwrap());
    let name_bytes = d
        .get(0x4C..0x4C + name_len as usize)
        .ok_or_else(|| Error::InvalidFormat("nk name truncated".into()))?;
    let name = if flags & 0x20 != 0 {
        String::from_utf8_lossy(name_bytes).into_owned()
    } else {
        decode_utf16le_lossy(name_bytes)
    };
    Ok(NkInfo {
        flags,
        subkey_count,
        subkeys_off,
        value_count,
        values_off,
        class_off,
        class_len,
        name,
    })
}

fn list_subkeys(hive: &[u8], nk: &NkInfo) -> Result<Vec<(String, usize)>> {
    if nk.subkey_count == 0 || nk.subkeys_off == 0xffff_ffff {
        return Ok(Vec::new());
    }
    let list_off = cell_data_offset(nk.subkeys_off)?;
    list_subkeys_at(hive, list_off)
}

fn list_subkeys_at(hive: &[u8], list_off: usize) -> Result<Vec<(String, usize)>> {
    let d = hive
        .get(list_off..)
        .ok_or_else(|| Error::InvalidFormat("subkey list truncated".into()))?;
    if d.len() < 4 {
        return Err(Error::InvalidFormat("subkey list too short".into()));
    }
    let sig = &d[0..2];
    let count = u16::from_le_bytes(d[0x02..0x04].try_into().unwrap()) as usize;
    let mut out = Vec::new();
    if sig == b"lf" || sig == b"lh" {
        // elements: 4-byte offset + 4-byte hash
        for i in 0..count {
            let base = 4 + i * 8;
            let off = u32::from_le_bytes(d[base..base + 4].try_into().unwrap());
            let child = cell_data_offset(off)?;
            let nk = parse_nk(hive, child)?;
            out.push((nk.name, child));
        }
    } else if sig == b"li" {
        for i in 0..count {
            let base = 4 + i * 4;
            let off = u32::from_le_bytes(d[base..base + 4].try_into().unwrap());
            let child = cell_data_offset(off)?;
            let nk = parse_nk(hive, child)?;
            out.push((nk.name, child));
        }
    } else if sig == b"ri" {
        for i in 0..count {
            let base = 4 + i * 4;
            let off = u32::from_le_bytes(d[base..base + 4].try_into().unwrap());
            let nested = cell_data_offset(off)?;
            out.extend(list_subkeys_at(hive, nested)?);
        }
    } else {
        return Err(Error::InvalidFormat(format!(
            "unknown subkey list sig {}",
            String::from_utf8_lossy(sig)
        )));
    }
    Ok(out)
}

fn read_values(hive: &[u8], nk: &NkInfo) -> Result<Vec<(String, Vec<u8>)>> {
    if nk.value_count == 0 || nk.values_off == 0xffff_ffff {
        return Ok(Vec::new());
    }
    let list_off = cell_data_offset(nk.values_off)?;
    let mut out = Vec::new();
    for i in 0..nk.value_count as usize {
        let eoff = list_off + i * 4;
        let vk_off = u32::from_le_bytes(
            hive.get(eoff..eoff + 4)
                .ok_or_else(|| Error::InvalidFormat("value list truncated".into()))?
                .try_into()
                .unwrap(),
        );
        let vk_data = cell_data_offset(vk_off)?;
        let vk = hive
            .get(vk_data..)
            .ok_or_else(|| Error::InvalidFormat("vk truncated".into()))?;
        if vk.len() < 20 || &vk[0..2] != b"vk" {
            continue;
        }
        let name_len = u16::from_le_bytes(vk[0x02..0x04].try_into().unwrap()) as usize;
        let data_len_raw = u32::from_le_bytes(vk[0x04..0x08].try_into().unwrap());
        let data_off = u32::from_le_bytes(vk[0x08..0x0C].try_into().unwrap());
        let flags = u16::from_le_bytes(vk[0x10..0x12].try_into().unwrap());
        let name = if name_len == 0 {
            String::new()
        } else if flags & 1 != 0 {
            String::from_utf8_lossy(&vk[0x14..0x14 + name_len]).into_owned()
        } else {
            decode_utf16le_lossy(&vk[0x14..0x14 + name_len])
        };
        let data_size = (data_len_raw & 0x7fff_ffff) as usize;
        let data = if data_len_raw & 0x8000_0000 != 0 {
            // resident
            data_off.to_le_bytes()[..data_size.min(4)].to_vec()
        } else {
            let doff = cell_data_offset(data_off)?;
            hive.get(doff..doff + data_size)
                .ok_or_else(|| Error::InvalidFormat("value data truncated".into()))?
                .to_vec()
        };
        out.push((name, data));
    }
    Ok(out)
}

fn cell_data_offset(offset: u32) -> Result<usize> {
    // Offsets in hive are relative to start of hive bins (after 4K base block).
    // Cell header is 4 bytes (size); data follows.
    Ok(0x1000 + offset as usize + 4)
}

fn decode_utf16le_lossy(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&u16s)
}

fn decode_hex_ascii(s: &str) -> std::result::Result<Vec<u8>, String> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return Err("odd length".into());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = from_hex(bytes[i]).ok_or_else(|| format!("bad hex {}", bytes[i] as char))?;
        let lo =
            from_hex(bytes[i + 1]).ok_or_else(|| format!("bad hex {}", bytes[i + 1] as char))?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Random 16-byte salt for AES SAM_HASH_AES.
pub fn random_salt() -> [u8; 16] {
    let mut s = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntlm_hash_empty_and_known() {
        // MD4("") = 31d6cfe0d16ae931b73c59d7e0c089c0
        let empty = ntlm_hash("");
        assert_eq!(
            empty,
            [
                0x31, 0xd6, 0xcf, 0xe0, 0xd1, 0x6a, 0xe9, 0x31, 0xb7, 0x3c, 0x59, 0xd7, 0xe0, 0xc0,
                0x89, 0xc0
            ]
        );
    }

    #[test]
    fn bootkey_permute_identity_check() {
        let scrambled: Vec<u8> = (0u8..16).collect();
        let mut bootkey = [0u8; 16];
        for (i, &t) in BOOTKEY_TRANSFORM.iter().enumerate() {
            bootkey[i] = scrambled[t];
        }
        assert_eq!(bootkey[0], 8);
        assert_eq!(bootkey[15], 7);
    }

    #[test]
    fn aes_des_roundtrip_with_fake_hbootkey() {
        let rid = 500u32;
        let nt = ntlm_hash("P@ssw0rd!");
        let hboot = [0x11u8; 32]; // only first 16 used for AES
        let salt = [0x22u8; 16];
        let blob = encrypt_nt_hash(rid, &nt, &hboot, &salt, true).unwrap();
        assert_eq!(blob.len(), 56);
        assert_eq!(blob[2], 2); // revision AES
        let back = decrypt_nt_hash(rid, &blob, &hboot, true).unwrap();
        assert_eq!(back, nt);
    }

    #[test]
    fn rc4_roundtrip_with_fake_hbootkey() {
        let rid = 1001u32;
        let nt = ntlm_hash("secret");
        let hboot = [0x33u8; 32];
        let salt = [0u8; 16];
        let blob = encrypt_nt_hash(rid, &nt, &hboot, &salt, false).unwrap();
        assert_eq!(blob.len(), 20);
        let back = decrypt_nt_hash(rid, &blob, &hboot, false).unwrap();
        assert_eq!(back, nt);
    }

    #[test]
    fn patch_v_appends_when_blank() {
        let mut v = vec![0u8; 0xCC + 4];
        // blank: NT length 4, offset 0
        v[V_NT_OFFSET..V_NT_OFFSET + 4].copy_from_slice(&0u32.to_le_bytes());
        v[V_NT_LENGTH..V_NT_LENGTH + 4].copy_from_slice(&4u32.to_le_bytes());
        let blob = vec![0xABu8; 56];
        patch_v_with_nt_hash(&mut v, &blob).unwrap();
        let off = u32::from_le_bytes(v[V_NT_OFFSET..V_NT_OFFSET + 4].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(v[V_NT_LENGTH..V_NT_LENGTH + 4].try_into().unwrap()) as usize;
        assert_eq!(len, 56);
        assert_eq!(&v[V_DATA_BASE + off..V_DATA_BASE + off + 56], &blob[..]);
    }
}
