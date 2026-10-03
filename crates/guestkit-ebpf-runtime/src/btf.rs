// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Minimal kernel BTF reader: struct member byte offsets, so programs that
//! walk kernel structs (without CO-RE in Rust eBPF) get the running kernel's
//! layout through a config map instead of compiled-in offsets.

use std::collections::HashMap;

const BTF_MAGIC: u16 = 0xeb9f;

const KIND_INT: u32 = 1;
const KIND_ARRAY: u32 = 3;
const KIND_STRUCT: u32 = 4;
const KIND_UNION: u32 = 5;
const KIND_ENUM: u32 = 6;
const KIND_TYPEDEF: u32 = 8;
const KIND_VOLATILE: u32 = 9;
const KIND_CONST: u32 = 10;
const KIND_RESTRICT: u32 = 11;
const KIND_FUNC_PROTO: u32 = 13;
const KIND_VAR: u32 = 14;
const KIND_DATASEC: u32 = 15;
const KIND_DECL_TAG: u32 = 17;
const KIND_TYPE_TAG: u32 = 18;
const KIND_ENUM64: u32 = 19;

#[derive(Clone, Copy)]
struct Ty {
    name: u32,
    kind: u32,
    vlen: u32,
    kflag: bool,
    size_or_type: u32,
    /// Byte offset of the kind-specific trailer in `types`.
    extra: usize,
}

pub struct Btf {
    types: Vec<u8>,
    strings: Vec<u8>,
    /// Index 0 is the void type.
    tys: Vec<Ty>,
    by_name: HashMap<String, Vec<u32>>,
}

fn u32_at(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(off..off + 4)?.try_into().ok()?))
}

impl Btf {
    pub fn from_sys_fs() -> Result<Self, String> {
        let data = std::fs::read("/sys/kernel/btf/vmlinux").map_err(|e| format!("read vmlinux BTF: {e}"))?;
        Self::parse(&data)
    }

    pub fn parse(data: &[u8]) -> Result<Self, String> {
        let bad = || "malformed BTF".to_string();
        if data.len() < 24 || u16::from_le_bytes([data[0], data[1]]) != BTF_MAGIC {
            return Err("not little-endian BTF".into());
        }
        let hdr_len = u32_at(data, 4).ok_or_else(bad)? as usize;
        let type_off = u32_at(data, 8).ok_or_else(bad)? as usize;
        let type_len = u32_at(data, 12).ok_or_else(bad)? as usize;
        let str_off = u32_at(data, 16).ok_or_else(bad)? as usize;
        let str_len = u32_at(data, 20).ok_or_else(bad)? as usize;
        let types = data
            .get(hdr_len + type_off..hdr_len + type_off + type_len)
            .ok_or_else(bad)?
            .to_vec();
        let strings = data.get(hdr_len + str_off..hdr_len + str_off + str_len).ok_or_else(bad)?.to_vec();
        let mut tys = vec![Ty { name: 0, kind: 0, vlen: 0, kflag: false, size_or_type: 0, extra: 0 }];
        let mut off = 0;
        while off + 12 <= types.len() {
            let name = u32_at(&types, off).ok_or_else(bad)?;
            let info = u32_at(&types, off + 4).ok_or_else(bad)?;
            let size_or_type = u32_at(&types, off + 8).ok_or_else(bad)?;
            let kind = (info >> 24) & 0x1f;
            let vlen = info & 0xffff;
            let extra = off + 12;
            let trailer = match kind {
                KIND_INT | KIND_VAR | KIND_DECL_TAG => 4,
                KIND_ARRAY => 12,
                KIND_STRUCT | KIND_UNION | KIND_DATASEC => 12 * vlen as usize,
                KIND_ENUM | KIND_FUNC_PROTO => 8 * vlen as usize,
                KIND_ENUM64 => 12 * vlen as usize,
                _ => 0,
            };
            tys.push(Ty { name, kind, vlen, kflag: info >> 31 != 0, size_or_type, extra });
            off = extra + trailer;
        }
        let mut btf = Self { types, strings, tys, by_name: HashMap::new() };
        for id in 1..btf.tys.len() as u32 {
            let t = btf.tys[id as usize];
            if matches!(t.kind, KIND_STRUCT | KIND_UNION) && t.name != 0 {
                let n = btf.str(t.name).to_string();
                btf.by_name.entry(n).or_default().push(id);
            }
        }
        Ok(btf)
    }

    fn str(&self, off: u32) -> &str {
        let s = self.strings.get(off as usize..).unwrap_or_default();
        let n = s.iter().position(|c| *c == 0).unwrap_or(s.len());
        std::str::from_utf8(&s[..n]).unwrap_or("")
    }

    /// Strip typedef / qualifiers.
    fn resolve(&self, mut id: u32) -> u32 {
        for _ in 0..32 {
            let Some(t) = self.tys.get(id as usize) else { return id };
            match t.kind {
                KIND_TYPEDEF | KIND_VOLATILE | KIND_CONST | KIND_RESTRICT | KIND_TYPE_TAG => id = t.size_or_type,
                _ => return id,
            }
        }
        id
    }

    /// (bit offset, member type) of `name` inside struct/union `id`,
    /// descending into anonymous struct/union members.
    fn member(&self, id: u32, name: &str, depth: u32) -> Option<(u32, u32)> {
        let t = *self.tys.get(id as usize)?;
        if !matches!(t.kind, KIND_STRUCT | KIND_UNION) || depth > 8 {
            return None;
        }
        for i in 0..t.vlen as usize {
            let m = t.extra + i * 12;
            let mname = u32_at(&self.types, m)?;
            let mty = u32_at(&self.types, m + 4)?;
            let moff = u32_at(&self.types, m + 8)?;
            let bits = if t.kflag { moff & 0xff_ffff } else { moff };
            if mname == 0 {
                if let Some((o, ty)) = self.member(self.resolve(mty), name, depth + 1) {
                    return Some((bits + o, ty));
                }
            } else if self.str(mname) == name {
                return Some((bits, mty));
            }
        }
        None
    }

    /// Byte offset of a dotted member path (`__sk_common.skc_net.net`) in a
    /// named struct.
    pub fn offset(&self, strukt: &str, path: &str) -> Option<u32> {
        let ids = self.by_name.get(strukt)?;
        'outer: for &start in ids {
            let mut id = start;
            let mut bits = 0;
            for part in path.split('.') {
                let Some((o, ty)) = self.member(self.resolve(id), part, 0) else { continue 'outer };
                bits += o;
                id = ty;
            }
            if bits % 8 == 0 {
                return Some(bits / 8);
            }
        }
        None
    }
}

