// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Local filesystem disk source.

use super::uri::{DiskSource, DiskSourceMetadata};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct LocalDiskSource {
    file: File,
    path: PathBuf,
    size: u64,
}

impl LocalDiskSource {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open disk image: {}", path.display()))?;
        let size = file.metadata()?.len();
        Ok(Self {
            file,
            path: path.to_path_buf(),
            size,
        })
    }
}

impl Read for LocalDiskSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for LocalDiskSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }
}

impl DiskSource for LocalDiskSource {
    fn metadata(&self) -> DiskSourceMetadata {
        DiskSourceMetadata {
            uri: self.path.display().to_string(),
            size_bytes: Some(self.size),
            backend: "local".to_string(),
        }
    }

    fn local_path(&self) -> Option<&Path> {
        Some(&self.path)
    }
}
