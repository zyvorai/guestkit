// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Progressive inspection loading stages for the TUI.

/// Stages executed one per tick so the UI can render between steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingStage {
    Bootstrap,
    NetworkSecurity,
    PackagesServices,
    Profiles,
    StorageKernel,
    CompareImage,
    Done,
}

impl LoadingStage {
    pub fn label(self) -> &'static str {
        match self {
            LoadingStage::Bootstrap => "Mounting image",
            LoadingStage::NetworkSecurity => "Network & security",
            LoadingStage::PackagesServices => "Packages & services",
            LoadingStage::Profiles => "Security profiles",
            LoadingStage::StorageKernel => "Storage & kernel",
            LoadingStage::CompareImage => "Compare image",
            LoadingStage::Done => "Ready",
        }
    }

    pub fn index(self) -> u8 {
        match self {
            LoadingStage::Bootstrap => 0,
            LoadingStage::NetworkSecurity => 1,
            LoadingStage::PackagesServices => 2,
            LoadingStage::Profiles => 3,
            LoadingStage::StorageKernel => 4,
            LoadingStage::CompareImage => 5,
            LoadingStage::Done => 6,
        }
    }

    pub const TOTAL: u8 = 7;

    pub fn next(self) -> Self {
        match self {
            LoadingStage::Bootstrap => LoadingStage::NetworkSecurity,
            LoadingStage::NetworkSecurity => LoadingStage::PackagesServices,
            LoadingStage::PackagesServices => LoadingStage::Profiles,
            LoadingStage::Profiles => LoadingStage::StorageKernel,
            LoadingStage::StorageKernel => LoadingStage::CompareImage,
            LoadingStage::CompareImage | LoadingStage::Done => LoadingStage::Done,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadingState {
    pub stage: LoadingStage,
    pub message: String,
}

impl Default for LoadingState {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingState {
    pub fn new() -> Self {
        Self {
            stage: LoadingStage::Bootstrap,
            message: LoadingStage::Bootstrap.label().to_string(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.stage == LoadingStage::Done
    }

    pub fn progress_label(&self) -> String {
        format!(
            "Loading {} ({}/{})",
            self.message,
            self.stage.index() + 1,
            LoadingStage::TOTAL
        )
    }
}
