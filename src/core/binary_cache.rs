// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Binary cache implementation using bincode for fast serialization

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cached inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedInspection {
    /// Timestamp when cached (Unix epoch)
    pub timestamp: u64,
    /// SHA256 hash of disk image
    pub disk_hash: String,
    /// Operating system information
    pub os_info: OsInfoCache,
    /// Filesystem information
    pub filesystems: Vec<FilesystemCache>,
    /// User accounts
    pub users: Vec<UserCache>,
    /// Installed packages (limited to first 1000)
    pub packages: Vec<PackageCache>,
    /// Network configuration
    pub network: Option<NetworkCache>,
    /// System configuration
    pub system_config: Option<SystemConfigCache>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OsInfoCache {
    pub os_type: String,
    pub distribution: String,
    pub product_name: String,
    pub version_major: i32,
    pub version_minor: i32,
    pub architecture: String,
    pub hostname: String,
    pub package_format: String,
    pub init_system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemCache {
    pub device: String,
    pub fs_type: String,
    pub size: i64,
    pub uuid: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCache {
    pub username: String,
    pub uid: String,
    pub gid: String,
    pub home_dir: String,
    pub shell: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageCache {
    pub name: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCache {
    pub interfaces: Vec<NetworkInterfaceCache>,
    pub dns_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceCache {
    pub name: String,
    pub ip_address: String,
    pub mac_address: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfigCache {
    pub timezone: String,
    pub locale: String,
    pub selinux: String,
    pub cloud_init: bool,
}

/// Binary cache manager using bincode for fast serialization
pub struct BinaryCache {
    cache_dir: PathBuf,
}

impl BinaryCache {
    /// Create new binary cache manager
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .context("Could not find cache directory")?
            .join("guestkit")
            .join("binary");

        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        Ok(Self { cache_dir })
    }

    /// Create cache with custom directory (for testing and benchmarks)
    pub fn with_dir(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;
        Ok(Self { cache_dir })
    }

    /// Validate that a cache key contains only safe characters
    fn validate_key(key: &str) -> Result<()> {
        if key.is_empty() {
            anyhow::bail!("Cache key must not be empty");
        }
        if key.contains('/') || key.contains('\\') || key.contains("..") {
            anyhow::bail!(
                "Cache key contains unsafe characters (/, \\, or ..): {:?}",
                key
            );
        }
        if !key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            anyhow::bail!(
                "Cache key contains disallowed characters (only alphanumeric, -, _, . allowed): {:?}",
                key
            );
        }
        Ok(())
    }

    /// Get cache file path for a given key
    fn cache_path(&self, key: &str) -> Result<PathBuf> {
        Self::validate_key(key)?;
        Ok(self.cache_dir.join(format!("{}.bin", key)))
    }

    /// Save inspection result to binary cache
    ///
    /// Uses bincode for efficient binary serialization - 5-10x faster than JSON
    /// and produces 50-70% smaller files.
    pub fn save(&self, key: &str, data: &CachedInspection) -> Result<()> {
        let path = self.cache_path(key)?;

        // Serialize to binary format
        let encoded = bincode::serialize(data).context("Failed to serialize cache data")?;

        // Write atomically using temp file + rename
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, encoded).context("Failed to write cache file")?;

        fs::rename(&temp_path, &path).context("Failed to rename cache file")?;

        log::debug!("Saved cache to {:?} ({} bytes)", path, data.size());

        Ok(())
    }

    /// Load inspection result from binary cache
    pub fn load(&self, key: &str) -> Result<CachedInspection> {
        let path = self.cache_path(key)?;

        let bytes = fs::read(&path).context("Failed to read cache file")?;

        // Limit deserialization to 100MB to prevent memory exhaustion from corrupt/malicious data
        use bincode::Options;
        let data: CachedInspection = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .allow_trailing_bytes()
            .with_limit(100 * 1024 * 1024)
            .deserialize(&bytes)
            .context("Failed to deserialize cache data")?;

        log::debug!("Loaded cache from {:?} ({} bytes)", path, bytes.len());

        Ok(data)
    }

    /// Check if cache exists for given key
    pub fn exists(&self, key: &str) -> bool {
        self.cache_path(key).map(|p| p.exists()).unwrap_or(false)
    }

    /// Check if cache is valid (not older than max_age_seconds)
    pub fn is_valid(&self, key: &str, max_age_seconds: u64) -> Result<bool> {
        if !self.exists(key) {
            return Ok(false);
        }

        let data = self.load(key)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let age = now.saturating_sub(data.timestamp);
        Ok(age < max_age_seconds)
    }

    /// Delete cache entry
    pub fn delete(&self, key: &str) -> Result<()> {
        let path = self.cache_path(key)?;
        if path.exists() {
            fs::remove_file(&path).context("Failed to delete cache file")?;
        }
        Ok(())
    }

    /// Clear all cache entries
    pub fn clear_all(&self) -> Result<usize> {
        let mut count = 0;

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                fs::remove_file(&path)?;
                count += 1;
            }
        }

        log::info!("Cleared {} cache entries", count);
        Ok(count)
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats> {
        let mut total_entries = 0;
        let mut total_size = 0u64;
        let mut oldest_timestamp = u64::MAX;
        let mut newest_timestamp = 0u64;

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                total_entries += 1;

                // Get file size
                if let Ok(metadata) = fs::metadata(&path) {
                    total_size += metadata.len();
                }

                // Get timestamp from cached data
                if let Ok(bytes) = fs::read(&path) {
                    use bincode::Options;
                    if let Ok(data) = bincode::DefaultOptions::new()
                        .with_fixint_encoding()
                        .allow_trailing_bytes()
                        .with_limit(100 * 1024 * 1024)
                        .deserialize::<CachedInspection>(&bytes)
                    {
                        oldest_timestamp = oldest_timestamp.min(data.timestamp);
                        newest_timestamp = newest_timestamp.max(data.timestamp);
                    }
                }
            }
        }

        Ok(CacheStats {
            total_entries,
            total_size_bytes: total_size,
            oldest_entry: if oldest_timestamp != u64::MAX {
                Some(oldest_timestamp)
            } else {
                None
            },
            newest_entry: if newest_timestamp != 0 {
                Some(newest_timestamp)
            } else {
                None
            },
        })
    }

    /// Clear cache entries older than specified seconds
    pub fn clear_older_than(&self, max_age_seconds: u64) -> Result<usize> {
        let mut count = 0;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                if let Ok(bytes) = fs::read(&path) {
                    use bincode::Options;
                    if let Ok(data) = bincode::DefaultOptions::new()
                        .with_fixint_encoding()
                        .allow_trailing_bytes()
                        .with_limit(100 * 1024 * 1024)
                        .deserialize::<CachedInspection>(&bytes)
                    {
                        let age = now.saturating_sub(data.timestamp);
                        if age > max_age_seconds {
                            fs::remove_file(&path)?;
                            count += 1;
                        }
                    }
                }
            }
        }

        log::info!(
            "Cleared {} cache entries older than {} seconds",
            count,
            max_age_seconds
        );
        Ok(count)
    }
}

impl Default for BinaryCache {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            let fallback = std::env::temp_dir().join("guestkit").join("binary");
            log::warn!(
                "Primary cache directory unavailable ({}), falling back to {}. \
                 Cache data in this location will be lost on reboot.",
                e,
                fallback.display()
            );
            if let Err(create_err) = std::fs::create_dir_all(&fallback) {
                log::error!("Fallback cache directory creation also failed: {}. Caching will be non-functional.", create_err);
            }
            Self { cache_dir: fallback }
        })
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of cache entries
    pub total_entries: usize,
    /// Total size in bytes
    pub total_size_bytes: u64,
    /// Oldest cache entry timestamp
    pub oldest_entry: Option<u64>,
    /// Newest cache entry timestamp
    pub newest_entry: Option<u64>,
}

impl CacheStats {
    /// Get total size in human-readable format
    pub fn total_size_human(&self) -> String {
        let size = self.total_size_bytes as f64;
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.2} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl CachedInspection {
    /// Get approximate size in bytes
    fn size(&self) -> usize {
        // Approximate calculation
        let mut size = 0;
        size += std::mem::size_of::<u64>(); // timestamp
        size += self.disk_hash.len();
        size += std::mem::size_of::<OsInfoCache>();
        size += self.filesystems.len() * std::mem::size_of::<FilesystemCache>();
        size += self.users.len() * std::mem::size_of::<UserCache>();
        size += self.packages.len() * std::mem::size_of::<PackageCache>();
        size
    }

    /// Create new cached inspection with current timestamp
    pub fn new(disk_hash: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            timestamp,
            disk_hash,
            os_info: OsInfoCache::default(),
            filesystems: Vec::new(),
            users: Vec::new(),
            packages: Vec::new(),
            network: None,
            system_config: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_cache_roundtrip() {
        let cache = BinaryCache::new().unwrap();

        let data = CachedInspection::new("abc123".to_string());

        cache.save("test-key", &data).unwrap();
        let loaded = cache.load("test-key").unwrap();

        assert_eq!(data.disk_hash, loaded.disk_hash);
        assert_eq!(data.timestamp, loaded.timestamp);
    }

    #[test]
    fn test_cache_exists() {
        let cache = BinaryCache::new().unwrap();

        assert!(!cache.exists("nonexistent"));

        let data = CachedInspection::new("test".to_string());
        cache.save("exists-test", &data).unwrap();

        assert!(cache.exists("exists-test"));
    }

    #[test]
    fn test_cache_stats() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let data = CachedInspection::new("test".to_string());
        cache.save("stats-test", &data).unwrap();

        let stats = cache.stats().unwrap();
        assert_eq!(stats.total_entries, 1);
        assert!(stats.total_size_bytes > 0);
    }

    #[test]
    fn test_clear_older_than() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let mut old_data = CachedInspection::new("old".to_string());
        old_data.timestamp = 0; // Very old

        let new_data = CachedInspection::new("new".to_string());

        cache.save("old", &old_data).unwrap();
        cache.save("new", &new_data).unwrap();

        // Clear entries older than 1 day
        let cleared = cache.clear_older_than(86400).unwrap();
        assert_eq!(cleared, 1); // Only old entry cleared

        assert!(!cache.exists("old"));
        assert!(cache.exists("new"));
    }

    // ========== Additional Edge Case Tests ==========

    #[test]
    fn test_delete_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        // Deleting non-existent key should not error
        assert!(cache.delete("nonexistent").is_ok());
    }

    #[test]
    fn test_load_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        // Loading non-existent key should error
        assert!(cache.load("nonexistent").is_err());
    }

    #[test]
    fn test_clear_all_empty_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let count = cache.clear_all().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_clear_all_multiple_entries() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        for i in 0..5 {
            let data = CachedInspection::new(format!("hash-{}", i));
            cache.save(&format!("key-{}", i), &data).unwrap();
        }

        let count = cache.clear_all().unwrap();
        assert_eq!(count, 5);

        // Verify all entries are gone
        for i in 0..5 {
            assert!(!cache.exists(&format!("key-{}", i)));
        }
    }

    #[test]
    fn test_is_valid_fresh_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let data = CachedInspection::new("test".to_string());
        cache.save("fresh", &data).unwrap();

        // Fresh cache should be valid for 1 hour
        assert!(cache.is_valid("fresh", 3600).unwrap());
    }

    #[test]
    fn test_is_valid_expired_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let mut data = CachedInspection::new("test".to_string());
        data.timestamp = 0; // Very old

        cache.save("old", &data).unwrap();

        // Old cache should not be valid for 1 hour
        assert!(!cache.is_valid("old", 3600).unwrap());
    }

    #[test]
    fn test_is_valid_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        // Non-existent key should not be valid
        assert!(!cache.is_valid("nonexistent", 3600).unwrap());
    }

    #[test]
    fn test_stats_empty_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let stats = cache.stats().unwrap();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert_eq!(stats.oldest_entry, None);
        assert_eq!(stats.newest_entry, None);
    }

    #[test]
    fn test_stats_single_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let data = CachedInspection::new("test".to_string());
        cache.save("single", &data).unwrap();

        let stats = cache.stats().unwrap();
        assert_eq!(stats.total_entries, 1);
        assert!(stats.total_size_bytes > 0);
        assert!(stats.oldest_entry.is_some());
        assert!(stats.newest_entry.is_some());
    }

    #[test]
    fn test_stats_total_size_human_bytes() {
        let stats = CacheStats {
            total_entries: 1,
            total_size_bytes: 512,
            oldest_entry: None,
            newest_entry: None,
        };
        assert_eq!(stats.total_size_human(), "512 B");
    }

    #[test]
    fn test_stats_total_size_human_kb() {
        let stats = CacheStats {
            total_entries: 1,
            total_size_bytes: 2048,
            oldest_entry: None,
            newest_entry: None,
        };
        assert_eq!(stats.total_size_human(), "2.00 KB");
    }

    #[test]
    fn test_stats_total_size_human_mb() {
        let stats = CacheStats {
            total_entries: 1,
            total_size_bytes: 2 * 1024 * 1024,
            oldest_entry: None,
            newest_entry: None,
        };
        assert_eq!(stats.total_size_human(), "2.00 MB");
    }

    #[test]
    fn test_stats_total_size_human_gb() {
        let stats = CacheStats {
            total_entries: 1,
            total_size_bytes: 3 * 1024 * 1024 * 1024,
            oldest_entry: None,
            newest_entry: None,
        };
        assert_eq!(stats.total_size_human(), "3.00 GB");
    }

    #[test]
    fn test_cached_inspection_new() {
        let data = CachedInspection::new("abc123".to_string());

        assert_eq!(data.disk_hash, "abc123");
        assert!(data.timestamp > 0);
        assert_eq!(data.filesystems.len(), 0);
        assert_eq!(data.users.len(), 0);
        assert_eq!(data.packages.len(), 0);
        assert!(data.network.is_none());
        assert!(data.system_config.is_none());
    }

    #[test]
    fn test_cached_inspection_with_data() {
        let mut data = CachedInspection::new("test".to_string());
        data.users.push(UserCache {
            username: "root".to_string(),
            uid: "0".to_string(),
            gid: "0".to_string(),
            home_dir: "/root".to_string(),
            shell: "/bin/bash".to_string(),
        });

        assert_eq!(data.users.len(), 1);
        assert_eq!(data.users[0].username, "root");
    }

    #[test]
    fn test_save_load_with_full_data() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let mut data = CachedInspection::new("full-test".to_string());
        data.os_info = OsInfoCache {
            os_type: "linux".to_string(),
            distribution: "ubuntu".to_string(),
            product_name: "Ubuntu 22.04".to_string(),
            version_major: 22,
            version_minor: 4,
            architecture: "x86_64".to_string(),
            hostname: "test-host".to_string(),
            package_format: "deb".to_string(),
            init_system: "systemd".to_string(),
        };

        cache.save("full", &data).unwrap();
        let loaded = cache.load("full").unwrap();

        assert_eq!(loaded.os_info.os_type, "linux");
        assert_eq!(loaded.os_info.distribution, "ubuntu");
        assert_eq!(loaded.os_info.version_major, 22);
    }

    #[test]
    fn test_clear_older_than_zero() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let data = CachedInspection::new("test".to_string());
        cache.save("recent", &data).unwrap();

        // Clear entries older than 0 seconds - should clear nothing
        let cleared = cache.clear_older_than(0).unwrap();
        assert_eq!(cleared, 0);
    }

    #[test]
    fn test_multiple_save_overwrites() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let data1 = CachedInspection::new("first".to_string());
        cache.save("key", &data1).unwrap();

        let data2 = CachedInspection::new("second".to_string());
        cache.save("key", &data2).unwrap();

        let loaded = cache.load("key").unwrap();
        assert_eq!(loaded.disk_hash, "second");
    }

    #[test]
    fn test_special_characters_in_key() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

        let data = CachedInspection::new("test".to_string());

        // Test various special characters in keys
        cache.save("key-with-dashes", &data).unwrap();
        cache.save("key_with_underscores", &data).unwrap();
        cache.save("key123", &data).unwrap();

        assert!(cache.exists("key-with-dashes"));
        assert!(cache.exists("key_with_underscores"));
        assert!(cache.exists("key123"));
    }

    #[test]
    fn test_default_os_info_cache() {
        let os_info = OsInfoCache::default();

        assert_eq!(os_info.os_type, "");
        assert_eq!(os_info.distribution, "");
        assert_eq!(os_info.product_name, "");
        assert_eq!(os_info.version_major, 0);
        assert_eq!(os_info.version_minor, 0);
        assert_eq!(os_info.architecture, "");
        assert_eq!(os_info.hostname, "");
        assert_eq!(os_info.package_format, "");
        assert_eq!(os_info.init_system, "");
    }

    // Property-based tests
    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Property: Any data saved to cache can be loaded back unchanged
            #[test]
            fn prop_cache_roundtrip(disk_hash in "[a-f0-9]{64}") {
                let temp_dir = tempfile::tempdir().unwrap();
                let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

                let data = CachedInspection::new(disk_hash.clone());
                cache.save(&disk_hash, &data).unwrap();
                let loaded = cache.load(&disk_hash).unwrap();

                prop_assert_eq!(data.disk_hash, loaded.disk_hash);
                prop_assert_eq!(data.timestamp, loaded.timestamp);
            }

            /// Property: Cache key can be any valid string
            #[test]
            fn prop_cache_key_validity(key in "[a-zA-Z0-9_-]{1,64}") {
                let temp_dir = tempfile::tempdir().unwrap();
                let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

                let data = CachedInspection::new("test-hash".to_string());
                prop_assert!(cache.save(&key, &data).is_ok());
                prop_assert!(cache.exists(&key));
                prop_assert!(cache.load(&key).is_ok());
            }

            /// Property: Clearing cache with any age threshold works correctly
            #[test]
            fn prop_clear_older_than(age_secs in 0u64..1000000u64) {
                let temp_dir = tempfile::tempdir().unwrap();
                let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

                // Add some data
                let data = CachedInspection::new("test".to_string());
                cache.save("test", &data).unwrap();

                // Clear with arbitrary age
                let result = cache.clear_older_than(age_secs);
                prop_assert!(result.is_ok());
            }

            /// Property: Cache stats always return valid values
            #[test]
            fn prop_cache_stats_valid(num_entries in 0usize..100) {
                let temp_dir = tempfile::tempdir().unwrap();
                let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

                // Add random number of entries
                for i in 0..num_entries {
                    let data = CachedInspection::new(format!("hash-{}", i));
                    cache.save(&format!("key-{}", i), &data).unwrap();
                }

                let stats = cache.stats().unwrap();
                prop_assert_eq!(stats.total_entries, num_entries);
                // Verify total_size_bytes is reasonable (non-zero if entries exist)
                if num_entries > 0 {
                    prop_assert!(stats.total_size_bytes > 0);
                }
            }

            /// Property: Saving then deleting makes exists() return false
            #[test]
            fn prop_save_delete_consistency(key in "[a-z]{10}") {
                let temp_dir = tempfile::tempdir().unwrap();
                let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

                let data = CachedInspection::new("test-hash".to_string());
                cache.save(&key, &data).unwrap();
                prop_assert!(cache.exists(&key));

                cache.delete(&key).unwrap();
                prop_assert!(!cache.exists(&key));
            }

            /// Property: Timestamp is always <= current time
            #[test]
            fn prop_timestamp_validity(disk_hash in "[a-f0-9]{32}") {
                let data = CachedInspection::new(disk_hash);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                prop_assert!(data.timestamp <= now);
            }
        }
    }
}
