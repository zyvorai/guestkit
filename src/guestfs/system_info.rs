// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! System information gathering (hostnamectl-style)

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Get boot ID
    pub fn get_boot_id(&mut self) -> Result<String> {
        self.ensure_ready()?;

        match self.cat("/proc/sys/kernel/random/boot_id") {
            Ok(content) => Ok(content.trim().to_string()),
            Err(_) => Err(Error::NotFound("boot_id not found".to_string())),
        }
    }

    /// Get systemd version
    pub fn get_systemd_version(&mut self) -> Result<String> {
        self.ensure_ready()?;

        // Try to check if systemd exists
        if self.exists("/lib/systemd/systemd").is_ok() {
            return Ok("systemd".to_string());
        }
        if self.exists("/usr/lib/systemd/systemd").is_ok() {
            return Ok("systemd".to_string());
        }

        Err(Error::NotFound("systemd not found".to_string()))
    }

    /// Get hardware vendor from DMI
    pub fn get_hardware_vendor(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if let Ok(vendor) = self.cat("/sys/class/dmi/id/sys_vendor") {
            return Ok(vendor.trim().to_string());
        }

        if let Ok(vendor) = self.cat("/sys/class/dmi/id/board_vendor") {
            return Ok(vendor.trim().to_string());
        }

        Err(Error::NotFound("hardware vendor not found".to_string()))
    }

    /// Get hardware model from DMI
    pub fn get_hardware_model(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if let Ok(model) = self.cat("/sys/class/dmi/id/product_name") {
            return Ok(model.trim().to_string());
        }

        if let Ok(model) = self.cat("/sys/class/dmi/id/board_name") {
            return Ok(model.trim().to_string());
        }

        Err(Error::NotFound("hardware model not found".to_string()))
    }

    /// Get firmware version from DMI
    pub fn get_firmware_version(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if let Ok(version) = self.cat("/sys/class/dmi/id/bios_version") {
            return Ok(version.trim().to_string());
        }

        Err(Error::NotFound("firmware version not found".to_string()))
    }

    /// Get firmware date from DMI
    pub fn get_firmware_date(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if let Ok(date) = self.cat("/sys/class/dmi/id/bios_date") {
            return Ok(date.trim().to_string());
        }

        Err(Error::NotFound("firmware date not found".to_string()))
    }

    /// Get chassis type from DMI
    pub fn get_chassis_type(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if let Ok(chassis) = self.cat("/sys/class/dmi/id/chassis_type") {
            let chassis_num: u8 = chassis.trim().parse().unwrap_or(0);
            let chassis_name = match chassis_num {
                1 => "other",
                2 => "unknown",
                3 => "desktop",
                4 => "low-profile-desktop",
                5 => "pizza-box",
                6 => "mini-tower",
                7 => "tower",
                8 => "portable",
                9 => "laptop",
                10 => "notebook",
                11 => "hand-held",
                12 => "docking-station",
                13 => "all-in-one",
                14 => "sub-notebook",
                15 => "space-saving",
                16 => "lunch-box",
                17 => "main-server",
                18 => "expansion-chassis",
                19 => "sub-chassis",
                20 => "bus-expansion-chassis",
                21 => "peripheral-chassis",
                22 => "raid-chassis",
                23 => "rack-mount-chassis",
                24 => "sealed-case-pc",
                25 => "multi-system-chassis",
                30 => "tablet",
                31 => "convertible",
                32 => "detachable",
                _ => "unknown",
            };
            return Ok(chassis_name.to_string());
        }

        Ok("unknown".to_string())
    }

    /// Get chassis asset tag
    pub fn get_chassis_asset_tag(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if let Ok(tag) = self.cat("/sys/class/dmi/id/chassis_asset_tag") {
            let tag = tag.trim().to_string();
            if tag.is_empty() || tag == "No Asset Information" || tag == "None" {
                return Ok("No Asset Information".to_string());
            }
            return Ok(tag);
        }

        Ok("No Asset Information".to_string())
    }

    /// Get icon name based on chassis type
    pub fn get_icon_name(&mut self) -> Result<String> {
        let chassis = self
            .get_chassis_type()
            .unwrap_or_else(|_| "unknown".to_string());

        let icon = match chassis.as_str() {
            "laptop" | "notebook" | "sub-notebook" => "computer-laptop",
            "desktop" | "low-profile-desktop" | "tower" | "mini-tower" => "computer-desktop",
            "tablet" => "computer-tablet",
            "server" | "main-server" | "rack-mount-chassis" => "computer-server",
            "convertible" | "detachable" => "computer-convertible",
            "vm" | "virtual-machine" => "computer-vm",
            _ => "computer",
        };

        Ok(icon.to_string())
    }
}
