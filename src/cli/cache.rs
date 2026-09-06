// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Inspection result caching

use crate::cli::formatters::InspectionReport;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cache manager for inspection results
pub struct InspectionCache {
    cache_dir: PathBuf,
}

impl InspectionCache {
    /// Create a new cache manager
    pub fn new() -> Result<Self> {
        let cache_dir = Self::get_cache_directory()?;
        fs::create_dir_all(&cache_dir)?;

        Ok(Self { cache_dir })
    }

    /// Get the cache directory path
    fn get_cache_directory() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Could not determine home directory")?;

        Ok(PathBuf::from(home).join(".cache").join("guestkit"))
    }

    /// Generate cache key for a disk image
    fn cache_key(&self, image_path: &Path) -> Result<String> {
        // Get absolute path and metadata atomically from same path
        let abs_path = fs::canonicalize(image_path)
            .with_context(|| format!("Could not canonicalize path: {}", image_path.display()))?;

        // Get file metadata
        let metadata = fs::metadata(&abs_path)
            .with_context(|| format!("Could not read metadata: {}", abs_path.display()))?;

        let mtime = metadata
            .modified()
            .with_context(|| format!("Could not read modification time: {}", abs_path.display()))?
            .duration_since(SystemTime::UNIX_EPOCH)
            .with_context(|| "System time before UNIX epoch")?
            .as_secs();

        let size = metadata.len();

        // Create hash from path + mtime + size
        let mut hasher = Sha256::new();
        hasher.update(abs_path.to_string_lossy().as_bytes());
        hasher.update(mtime.to_le_bytes());
        hasher.update(size.to_le_bytes());

        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }

    /// Get cached inspection result if available and valid
    pub fn get(&self, image_path: &Path) -> Result<Option<InspectionReport>> {
        let key = self.cache_key(image_path)?;
        let cache_file = self.cache_dir.join(format!("{}.json", key));

        if !cache_file.exists() {
            return Ok(None);
        }

        // Read cached result
        let content = fs::read_to_string(&cache_file).context("Failed to read cache file")?;

        let report: InspectionReport =
            serde_json::from_str(&content).context("Failed to parse cached inspection report")?;

        log::debug!("Cache hit for {}", image_path.display());
        Ok(Some(report))
    }

    /// Store inspection result in cache
    pub fn store(&self, image_path: &Path, report: &InspectionReport) -> Result<()> {
        let key = self.cache_key(image_path)?;
        let cache_file = self.cache_dir.join(format!("{}.json", key));

        let json = serde_json::to_string_pretty(report)
            .context("Failed to serialize inspection report")?;

        fs::write(&cache_file, json)
            .with_context(|| format!("Failed to write cache file: {}", cache_file.display()))?;

        log::debug!("Cached inspection result for {}", image_path.display());
        Ok(())
    }

    /// Clear all cached results
    pub fn clear_all(&self) -> Result<usize> {
        let mut count = 0;

        if self.cache_dir.exists() {
            for entry in fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    fs::remove_file(entry.path())?;
                    count += 1;
                }
            }
        }

        log::info!("Cleared {} cached inspection results", count);
        Ok(count)
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats> {
        let mut total_entries = 0;
        let mut total_size = 0;

        if self.cache_dir.exists() {
            for entry in fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    total_entries += 1;
                    if let Ok(metadata) = fs::metadata(&path) {
                        total_size += metadata.len();
                    }
                }
            }
        }

        Ok(CacheStats {
            entries: total_entries,
            total_bytes: total_size,
        })
    }

    /// Store evidence snapshot in cache
    pub fn store_evidence(
        &self,
        image_path: &Path,
        evidence: &crate::evidence::EvidenceSnapshot,
    ) -> Result<()> {
        let key = self.cache_key(image_path)?;
        let cache_file = self.cache_dir.join(format!("{}-evidence-v1.json", key));
        let json = serde_json::to_string_pretty(evidence)
            .context("Failed to serialize evidence snapshot")?;
        fs::write(&cache_file, json)?;
        Ok(())
    }

    /// Get cached evidence snapshot
    pub fn get_evidence(
        &self,
        image_path: &Path,
    ) -> Result<Option<crate::evidence::EvidenceSnapshot>> {
        let key = self.cache_key(image_path)?;
        let cache_file = self.cache_dir.join(format!("{}-evidence-v1.json", key));
        if !cache_file.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&cache_file)?;
        let evidence: crate::evidence::EvidenceSnapshot = serde_json::from_str(&content)?;
        Ok(Some(evidence))
    }
}

/// Evidence snapshot cache wrapper
pub struct EvidenceCache(InspectionCache);

impl EvidenceCache {
    pub fn new() -> Result<Self> {
        Ok(Self(InspectionCache::new()?))
    }

    pub fn store(
        &self,
        image_path: &Path,
        evidence: &crate::evidence::EvidenceSnapshot,
    ) -> Result<()> {
        self.0.store_evidence(image_path, evidence)
    }

    pub fn get(&self, image_path: &Path) -> Result<Option<crate::evidence::EvidenceSnapshot>> {
        self.0.get_evidence(image_path)
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub total_bytes: u64,
}

impl CacheStats {
    /// Format size in human-readable format
    pub fn size_human(&self) -> String {
        let kb = self.total_bytes as f64 / 1024.0;
        if kb < 1024.0 {
            format!("{:.1} KB", kb)
        } else {
            let mb = kb / 1024.0;
            format!("{:.1} MB", mb)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cache_key_generation() {
        let cache = InspectionCache::new().unwrap();
        let temp_file = NamedTempFile::new().unwrap();

        // Same file should generate same key
        let key1 = cache.cache_key(temp_file.path()).unwrap();
        let key2 = cache.cache_key(temp_file.path()).unwrap();

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_stats_creation() {
        let stats = CacheStats {
            entries: 10,
            total_bytes: 2048,
        };

        assert_eq!(stats.entries, 10);
        assert_eq!(stats.total_bytes, 2048);
    }

    #[test]
    fn test_cache_stats_size_human_kb() {
        let stats = CacheStats {
            entries: 5,
            total_bytes: 10240, // 10 KB
        };

        let size_str = stats.size_human();
        assert!(size_str.contains("KB"));
        assert!(size_str.contains("10.0"));
    }

    #[test]
    fn test_cache_stats_size_human_mb() {
        let stats = CacheStats {
            entries: 5,
            total_bytes: 2097152, // 2 MB
        };

        let size_str = stats.size_human();
        assert!(size_str.contains("MB"));
        assert!(size_str.contains("2.0"));
    }

    #[test]
    fn test_cache_stats_size_human_small() {
        let stats = CacheStats {
            entries: 1,
            total_bytes: 512,
        };

        let size_str = stats.size_human();
        assert!(size_str.contains("KB"));
        assert!(size_str.contains("0.5"));
    }

    #[test]
    fn test_cache_stats_size_human_zero() {
        let stats = CacheStats {
            entries: 0,
            total_bytes: 0,
        };

        let size_str = stats.size_human();
        assert!(size_str.contains("KB"));
        assert!(size_str.contains("0.0"));
    }

    #[test]
    fn test_cache_key_is_hex_string() {
        let cache = InspectionCache::new().unwrap();
        let temp_file = NamedTempFile::new().unwrap();

        let key = cache.cache_key(temp_file.path()).unwrap();

        // SHA256 hash should be 64 hex characters
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
