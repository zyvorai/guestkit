// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Hivex (Windows Registry) operations for disk image manipulation
//!
//! This implementation provides Windows registry hive manipulation functionality
//! using the nt_hive2 crate for read operations. Write operations return
//! Error::Unsupported as nt_hive2 is read-only.
//!
//! Node and value handles are opaque i64 IDs. Node 0 is always the hive root.
//! Non-root nodes and all values are tracked in [`Guestfs::hive_nodes`] /
//! [`Guestfs::hive_values`] as paths from the root, so multi-level navigation
//! (e.g. Microsoft → Windows NT → CurrentVersion) works across calls.

use std::cell::RefCell;
use std::fs::File;
use std::rc::Rc;

use nt_hive2::{CleanHive, Hive, HiveParseMode, KeyNode, KeyValue, RegistryValue, SubPath};

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

/// Registry value type constants (matching Windows REG_* types)
#[allow(dead_code)]
const REG_NONE: i64 = 0;
#[allow(dead_code)]
const REG_SZ: i64 = 1;
#[allow(dead_code)]
const REG_EXPAND_SZ: i64 = 2;
#[allow(dead_code)]
const REG_BINARY: i64 = 3;
#[allow(dead_code)]
const REG_DWORD: i64 = 4;
#[allow(dead_code)]
const REG_DWORD_BIG_ENDIAN: i64 = 5;
#[allow(dead_code)]
const REG_MULTI_SZ: i64 = 7;
#[allow(dead_code)]
const REG_QWORD: i64 = 11;

/// Open a hive file from the stored path and return a mutable Hive handle.
fn open_hive_file(host_path: &std::path::Path) -> Result<Hive<File, CleanHive>> {
    let file = File::open(host_path).map_err(|e| {
        Error::CommandFailed(format!(
            "Failed to open hive {}: {}",
            host_path.display(),
            e
        ))
    })?;
    Hive::new(file, HiveParseMode::NormalWithBaseBlock)
        .map_err(|e| Error::CommandFailed(format!("Failed to parse hive: {:?}", e)))
}

fn reg_type_of(value: &RegistryValue) -> i64 {
    match value {
        RegistryValue::RegNone => REG_NONE,
        RegistryValue::RegSZ(_) => REG_SZ,
        RegistryValue::RegExpandSZ(_) => REG_EXPAND_SZ,
        RegistryValue::RegBinary(_) => REG_BINARY,
        RegistryValue::RegDWord(_) => REG_DWORD,
        RegistryValue::RegDWordBigEndian(_) => REG_DWORD_BIG_ENDIAN,
        RegistryValue::RegMultiSZ(_) => REG_MULTI_SZ,
        RegistryValue::RegQWord(_) => REG_QWORD,
        _ => REG_NONE,
    }
}

fn value_as_bytes(value: &RegistryValue) -> Vec<u8> {
    match value {
        RegistryValue::RegBinary(data) => data.clone(),
        RegistryValue::RegSZ(s) | RegistryValue::RegExpandSZ(s) | RegistryValue::RegLink(s) => {
            s.as_bytes().to_vec()
        }
        RegistryValue::RegMultiSZ(parts) => {
            let mut out = Vec::new();
            for p in parts {
                out.extend_from_slice(p.as_bytes());
                out.push(0);
            }
            out.push(0);
            out
        }
        RegistryValue::RegDWord(d) | RegistryValue::RegDWordBigEndian(d) => {
            d.to_le_bytes().to_vec()
        }
        RegistryValue::RegQWord(q) => q.to_le_bytes().to_vec(),
        _ => Vec::new(),
    }
}

fn value_as_string(value: &RegistryValue) -> Option<String> {
    match value {
        RegistryValue::RegSZ(data) | RegistryValue::RegExpandSZ(data) => Some(data.clone()),
        _ => None,
    }
}

fn value_as_dword(value: &RegistryValue) -> Option<u32> {
    match value {
        RegistryValue::RegDWord(data) | RegistryValue::RegDWordBigEndian(data) => Some(*data),
        _ => None,
    }
}

fn find_value_by_name<'a>(values: &'a [KeyValue], key: &str) -> Option<&'a KeyValue> {
    let needle = key.to_lowercase();
    values.iter().find(|v| v.name().to_lowercase() == needle)
}

impl Guestfs {
    fn hive_host_path(&self, handle: i64) -> Result<&std::path::PathBuf> {
        self.open_hives
            .get(&handle)
            .ok_or_else(|| Error::InvalidState(format!("No hive open with handle {}", handle)))
    }

    /// Resolve a node handle to its path from the hive root (empty = root).
    fn hive_node_path(&self, handle: i64, node: i64) -> Result<Vec<String>> {
        if node == 0 {
            return Ok(Vec::new());
        }
        self.hive_nodes
            .get(&(handle, node))
            .cloned()
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "Node {} not found for hive {} (navigate with hivex_node_get_child)",
                    node, handle
                ))
            })
    }

    /// Allocate (or reuse) a stable node handle for a path under a hive.
    fn alloc_hive_node(&mut self, handle: i64, path: Vec<String>) -> i64 {
        if path.is_empty() {
            return 0;
        }
        for ((h, id), p) in &self.hive_nodes {
            if *h == handle && path_eq(p, &path) {
                return *id;
            }
        }
        let (next_node, next_value) = self.hive_next_ids.entry(handle).or_insert((1, 1));
        let id = *next_node;
        *next_node += 1;
        let _ = next_value;
        self.hive_nodes.insert((handle, id), path);
        id
    }

    /// Allocate (or reuse) a stable value handle for (node path, value name).
    fn alloc_hive_value(&mut self, handle: i64, path: Vec<String>, name: String) -> i64 {
        for ((h, id), (p, n)) in &self.hive_values {
            if *h == handle && path_eq(p, &path) && n.eq_ignore_ascii_case(&name) {
                return *id;
            }
        }
        let (next_node, next_value) = self.hive_next_ids.entry(handle).or_insert((1, 1));
        let id = *next_value;
        *next_value += 1;
        let _ = next_node;
        self.hive_values.insert((handle, id), (path, name));
        id
    }

    fn clear_hive_state(&mut self, handle: i64) {
        self.open_hives.remove(&handle);
        self.hive_next_ids.remove(&handle);
        self.hive_nodes.retain(|(h, _), _| *h != handle);
        self.hive_values.retain(|(h, _), _| *h != handle);
    }

    /// Navigate to a key node by path components; invoke `f` with the key.
    fn with_key_node<R, F>(&self, handle: i64, path: &[String], f: F) -> Result<R>
    where
        F: FnOnce(&KeyNode) -> Result<R>,
    {
        let host_path = self.hive_host_path(handle)?;
        let mut hive = open_hive_file(host_path)?;
        let root = hive
            .root_key_node()
            .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;

        if path.is_empty() {
            return f(&root);
        }

        let joined = path.join("\\");
        let node = root
            .subpath(joined.as_str(), &mut hive)
            .map_err(|e| {
                Error::CommandFailed(format!("Failed to navigate registry path: {:?}", e))
            })?
            .ok_or_else(|| Error::NotFound(format!("Registry key not found: {}", joined)))?;
        let borrowed = node.borrow();
        f(&borrowed)
    }

    /// Like [`with_key_node`] but also needs a mutable hive (e.g. for subkeys).
    fn with_key_node_mut<R, F>(&self, handle: i64, path: &[String], f: F) -> Result<R>
    where
        F: FnOnce(&KeyNode, &mut Hive<File, CleanHive>) -> Result<R>,
    {
        let host_path = self.hive_host_path(handle)?;
        let mut hive = open_hive_file(host_path)?;
        let root = hive
            .root_key_node()
            .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;

        if path.is_empty() {
            return f(&root, &mut hive);
        }

        let joined = path.join("\\");
        let node = root
            .subpath(joined.as_str(), &mut hive)
            .map_err(|e| {
                Error::CommandFailed(format!("Failed to navigate registry path: {:?}", e))
            })?
            .ok_or_else(|| Error::NotFound(format!("Registry key not found: {}", joined)))?;
        // Clone the Rc so we can borrow the key while still using hive.
        let node_rc: Rc<RefCell<KeyNode>> = Rc::clone(&node);
        let borrowed = node_rc.borrow();
        f(&borrowed, &mut hive)
    }

    fn with_value_ctx<R, F>(&self, handle: i64, value: i64, f: F) -> Result<R>
    where
        F: FnOnce(&KeyValue) -> Result<R>,
    {
        let (path, name) = self
            .hive_values
            .get(&(handle, value))
            .cloned()
            .ok_or_else(|| {
                Error::NotFound(format!("Value {} not found for hive {}", value, handle))
            })?;

        self.with_key_node(handle, &path, |key| {
            let values = key.values();
            let kv = find_value_by_name(values, &name).ok_or_else(|| {
                Error::NotFound(format!(
                    "Value '{}' not found under key '{}'",
                    name,
                    if path.is_empty() {
                        "(root)".to_string()
                    } else {
                        path.join("\\")
                    }
                ))
            })?;
            f(kv)
        })
    }

    /// Open Windows registry hive
    pub fn hivex_open(&mut self, filename: &str, _write: bool) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_open {} {}", filename, _write);
        }

        let host_path = self.resolve_guest_path(filename)?;

        if !host_path.exists() {
            return Err(Error::NotFound(format!(
                "Hive file not found: {}",
                filename
            )));
        }

        // Validate it's a parseable hive by opening it
        {
            let mut hive = open_hive_file(&host_path)?;
            let _root = hive
                .root_key_node()
                .map_err(|e| Error::CommandFailed(format!("Failed to get root key: {:?}", e)))?;
        }

        // Generate handle from inode
        #[cfg(unix)]
        let handle = {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(&host_path).map_err(Error::Io)?;
            metadata.ino() as i64
        };

        #[cfg(not(unix))]
        let handle = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            host_path.hash(&mut hasher);
            hasher.finish() as i64
        };

        // Re-open of same hive: clear stale node/value maps
        self.clear_hive_state(handle);
        self.open_hives.insert(handle, host_path);
        self.hive_next_ids.insert(handle, (1, 1));
        Ok(handle)
    }

    /// Close Windows registry hive
    pub fn hivex_close(&mut self, handle: i64) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_close {}", handle);
        }

        self.clear_hive_state(handle);
        Ok(())
    }

    /// Get root node of registry hive
    pub fn hivex_root(&mut self, handle: i64) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_root {}", handle);
        }

        // Verify hive is open and valid
        self.with_key_node(handle, &[], |_| Ok(()))?;
        // Root node is always represented as 0
        Ok(0)
    }

    /// Get node name
    pub fn hivex_node_name(&mut self, handle: i64, node: i64) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_name {} {}", handle, node);
        }

        let path = self.hive_node_path(handle, node)?;
        if let Some(leaf) = path.last() {
            return Ok(leaf.clone());
        }
        self.with_key_node(handle, &path, |key| Ok(key.name().to_string()))
    }

    /// Get child nodes
    pub fn hivex_node_children(&mut self, handle: i64, node: i64) -> Result<Vec<i64>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_children {} {}", handle, node);
        }

        let path = self.hive_node_path(handle, node)?;
        let child_names: Vec<String> = self.with_key_node_mut(handle, &path, |key, hive| {
            let subkeys = key
                .subkeys(hive)
                .map_err(|e| Error::CommandFailed(format!("Failed to get subkeys: {:?}", e)))?;
            Ok(subkeys
                .iter()
                .map(|sk| sk.borrow().name().to_string())
                .collect())
        })?;

        let mut children = Vec::with_capacity(child_names.len());
        for name in child_names {
            let mut child_path = path.clone();
            child_path.push(name);
            children.push(self.alloc_hive_node(handle, child_path));
        }
        Ok(children)
    }

    /// Get node values
    pub fn hivex_node_values(&mut self, handle: i64, node: i64) -> Result<Vec<i64>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_values {} {}", handle, node);
        }

        let path = self.hive_node_path(handle, node)?;
        let names: Vec<String> = self.with_key_node(handle, &path, |key| {
            Ok(key.values().iter().map(|v| v.name().to_string()).collect())
        })?;

        let mut value_handles = Vec::with_capacity(names.len());
        for name in names {
            value_handles.push(self.alloc_hive_value(handle, path.clone(), name));
        }
        Ok(value_handles)
    }

    /// Get child node by name
    pub fn hivex_node_get_child(&mut self, handle: i64, node: i64, name: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_get_child {} {} {}", handle, node, name);
        }

        let path = self.hive_node_path(handle, node)?;
        let found_name: String =
            self.with_key_node_mut(handle, &path, |key, hive| match key.subkey(name, hive) {
                Ok(Some(child)) => Ok(child.borrow().name().to_string()),
                Ok(None) => Err(Error::NotFound(format!("Child node not found: {}", name))),
                Err(e) => Err(Error::CommandFailed(format!(
                    "Failed to get child: {:?}",
                    e
                ))),
            })?;

        let mut child_path = path;
        child_path.push(found_name);
        Ok(self.alloc_hive_node(handle, child_path))
    }

    /// Get value handle by name (GuestKit `hivex_node_get_value`)
    pub fn hivex_node_get_value(&mut self, handle: i64, node: i64, key: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_get_value {} {} {}", handle, node, key);
        }

        let path = self.hive_node_path(handle, node)?;
        let found_name: String = self.with_key_node(handle, &path, |kn| {
            find_value_by_name(kn.values(), key)
                .map(|v| v.name().to_string())
                .ok_or_else(|| Error::NotFound(format!("Value not found: {}", key)))
        })?;

        Ok(self.alloc_hive_value(handle, path, found_name))
    }

    /// Get value key (name)
    pub fn hivex_value_key(&mut self, handle: i64, value: i64) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_value_key {} {}", handle, value);
        }

        self.with_value_ctx(handle, value, |kv| Ok(kv.name().to_string()))
    }

    /// Get value type
    pub fn hivex_value_type(&mut self, handle: i64, value: i64) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_value_type {} {}", handle, value);
        }

        self.with_value_ctx(handle, value, |kv| Ok(reg_type_of(kv.value())))
    }

    /// Get value as string
    pub fn hivex_value_string(&mut self, handle: i64, value: i64) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_value_string {} {}", handle, value);
        }

        self.with_value_ctx(handle, value, |kv| {
            Ok(value_as_string(kv.value()).unwrap_or_default())
        })
    }

    /// Get value as integer (DWORD)
    pub fn hivex_value_dword(&mut self, handle: i64, value: i64) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_value_dword {} {}", handle, value);
        }

        self.with_value_ctx(handle, value, |kv| {
            Ok(value_as_dword(kv.value()).unwrap_or(0) as i32)
        })
    }

    /// Alias of [`hivex_value_dword`] returning `u32` (for hyper2kvm / binding compatibility).
    pub fn hivex_value_uint32(&mut self, handle: i64, value: i64) -> Result<u32> {
        Ok(self.hivex_value_dword(handle, value)? as u32)
    }

    /// Alias of [`hivex_value_dword`] returning `i64` (for hyper2kvm / binding compatibility).
    pub fn hivex_value_integer(&mut self, handle: i64, value: i64) -> Result<i64> {
        Ok(self.hivex_value_dword(handle, value)? as i64)
    }

    /// Get value as binary data
    pub fn hivex_value_value(&mut self, handle: i64, value: i64) -> Result<Vec<u8>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_value_value {} {}", handle, value);
        }

        self.with_value_ctx(handle, value, |kv| Ok(value_as_bytes(kv.value())))
    }

    /// Commit changes to hive — not supported (nt_hive2 is read-only)
    pub fn hivex_commit(&mut self, _handle: i64, filename: Option<&str>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_commit {:?}", filename);
        }

        Err(Error::Unsupported(
            "Registry write operations are not supported (nt_hive2 is read-only)".to_string(),
        ))
    }

    /// Set node value — not supported (nt_hive2 is read-only)
    pub fn hivex_node_set_value(
        &mut self,
        _handle: i64,
        _node: i64,
        key: &str,
        _t: i64,
        _val: &[u8],
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_set_value {}", key);
        }

        Err(Error::Unsupported(
            "Registry write operations are not supported (nt_hive2 is read-only)".to_string(),
        ))
    }

    /// Add child node — not supported (nt_hive2 is read-only)
    pub fn hivex_node_add_child(&mut self, _handle: i64, _parent: i64, name: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_add_child {}", name);
        }

        Err(Error::Unsupported(
            "Registry write operations are not supported (nt_hive2 is read-only)".to_string(),
        ))
    }

    /// Delete node — not supported (nt_hive2 is read-only)
    pub fn hivex_node_delete_child(&mut self, _handle: i64, _node: i64) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: hivex_node_delete_child");
        }

        Err(Error::Unsupported(
            "Registry write operations are not supported (nt_hive2 is read-only)".to_string(),
        ))
    }
}

fn path_eq(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| x.eq_ignore_ascii_case(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hivex_ops_api_exists() {
        let g = Guestfs::new().unwrap();
        assert!(g.open_hives.is_empty());
        assert!(g.hive_nodes.is_empty());
        assert!(g.hive_values.is_empty());
    }

    #[test]
    fn test_path_eq_case_insensitive() {
        assert!(path_eq(
            &["Microsoft".into(), "Windows NT".into()],
            &["microsoft".into(), "windows nt".into()]
        ));
        assert!(!path_eq(&["a".into()], &["a".into(), "b".into()]));
    }
}
