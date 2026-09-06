// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! License detection and mapping

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Common license mappings for well-known packages
static LICENSE_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // Common packages and their licenses
    m.insert("nginx", "BSD-2-Clause");
    m.insert("apache2", "Apache-2.0");
    m.insert("httpd", "Apache-2.0");
    m.insert("openssl", "Apache-2.0");
    m.insert("libssl", "Apache-2.0");
    m.insert("python3", "PSF-2.0");
    m.insert("python2", "PSF-2.0");
    m.insert("perl", "Artistic-2.0");
    m.insert("bash", "GPL-3.0-or-later");
    m.insert("coreutils", "GPL-3.0-or-later");
    m.insert("gcc", "GPL-3.0-or-later");
    m.insert("glibc", "LGPL-2.1-or-later");
    m.insert("libc6", "LGPL-2.1-or-later");
    m.insert("zlib", "Zlib");
    m.insert("curl", "MIT");
    m.insert("git", "GPL-2.0-only");
    m.insert("nodejs", "MIT");
    m.insert("npm", "Artistic-2.0");
    m.insert("redis", "BSD-3-Clause");
    m.insert("postgresql", "PostgreSQL");
    m.insert("mysql", "GPL-2.0-only");
    m.insert("mariadb", "GPL-2.0-only");
    m.insert("vim", "Vim");
    m.insert("emacs", "GPL-3.0-or-later");
    m.insert("systemd", "LGPL-2.1-or-later");
    m.insert("openssh", "BSD-2-Clause");
    m.insert("sqlite3", "Public-Domain");

    m
});

/// Detect license for a package
pub fn detect_license(package_name: &str, _package_type: &str) -> Option<String> {
    // Try exact match first
    if let Some(license) = LICENSE_MAP.get(package_name) {
        return Some(license.to_string());
    }

    // Try prefix match for libraries
    for (key, license) in LICENSE_MAP.iter() {
        if package_name.starts_with(key) {
            return Some(license.to_string());
        }
    }

    None
}

/// Check if a license is GPL-family
#[allow(dead_code)]
pub fn is_gpl_license(license: &str) -> bool {
    license.starts_with("GPL") || license.starts_with("AGPL") || license.starts_with("LGPL")
}

/// Check if a license is permissive
#[allow(dead_code)]
pub fn is_permissive_license(license: &str) -> bool {
    matches!(
        license,
        "MIT" | "BSD-2-Clause" | "BSD-3-Clause" | "Apache-2.0" | "ISC" | "Zlib"
    )
}

/// Get license category
#[allow(dead_code)]
pub fn license_category(license: &str) -> &'static str {
    if is_permissive_license(license) {
        "Permissive"
    } else if is_gpl_license(license) {
        "Copyleft"
    } else if license == "Public-Domain" {
        "Public Domain"
    } else {
        "Other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_license_exact_match() {
        assert_eq!(
            detect_license("nginx", "deb"),
            Some("BSD-2-Clause".to_string())
        );
        assert_eq!(
            detect_license("apache2", "deb"),
            Some("Apache-2.0".to_string())
        );
        assert_eq!(
            detect_license("python3", "deb"),
            Some("PSF-2.0".to_string())
        );
    }

    #[test]
    fn test_detect_license_prefix_match() {
        // libssl should match "openssl" prefix
        assert_eq!(
            detect_license("libssl1.1", "deb"),
            Some("Apache-2.0".to_string())
        );
        assert_eq!(
            detect_license("openssh-server", "deb"),
            Some("BSD-2-Clause".to_string())
        );
    }

    #[test]
    fn test_detect_license_no_match() {
        assert_eq!(detect_license("unknown-package", "deb"), None);
        assert_eq!(detect_license("my-custom-app", "deb"), None);
    }

    #[test]
    fn test_detect_license_common_packages() {
        assert_eq!(
            detect_license("bash", "deb"),
            Some("GPL-3.0-or-later".to_string())
        );
        assert_eq!(detect_license("curl", "deb"), Some("MIT".to_string()));
        assert_eq!(
            detect_license("git", "deb"),
            Some("GPL-2.0-only".to_string())
        );
        assert_eq!(
            detect_license("redis", "deb"),
            Some("BSD-3-Clause".to_string())
        );
        assert_eq!(
            detect_license("postgresql", "deb"),
            Some("PostgreSQL".to_string())
        );
    }

    #[test]
    fn test_is_gpl_license() {
        assert!(is_gpl_license("GPL-3.0-or-later"));
        assert!(is_gpl_license("GPL-2.0-only"));
        assert!(is_gpl_license("LGPL-2.1-or-later"));
        assert!(is_gpl_license("AGPL-3.0"));
        assert!(!is_gpl_license("MIT"));
        assert!(!is_gpl_license("Apache-2.0"));
        assert!(!is_gpl_license("BSD-2-Clause"));
    }

    #[test]
    fn test_is_permissive_license() {
        assert!(is_permissive_license("MIT"));
        assert!(is_permissive_license("BSD-2-Clause"));
        assert!(is_permissive_license("BSD-3-Clause"));
        assert!(is_permissive_license("Apache-2.0"));
        assert!(is_permissive_license("ISC"));
        assert!(is_permissive_license("Zlib"));
        assert!(!is_permissive_license("GPL-3.0-or-later"));
        assert!(!is_permissive_license("LGPL-2.1-or-later"));
        assert!(!is_permissive_license("Public-Domain"));
    }

    #[test]
    fn test_license_category_permissive() {
        assert_eq!(license_category("MIT"), "Permissive");
        assert_eq!(license_category("BSD-2-Clause"), "Permissive");
        assert_eq!(license_category("Apache-2.0"), "Permissive");
    }

    #[test]
    fn test_license_category_copyleft() {
        assert_eq!(license_category("GPL-3.0-or-later"), "Copyleft");
        assert_eq!(license_category("LGPL-2.1-or-later"), "Copyleft");
        assert_eq!(license_category("AGPL-3.0"), "Copyleft");
    }

    #[test]
    fn test_license_category_public_domain() {
        assert_eq!(license_category("Public-Domain"), "Public Domain");
    }

    #[test]
    fn test_license_category_other() {
        assert_eq!(license_category("PostgreSQL"), "Other");
        assert_eq!(license_category("Vim"), "Other");
        assert_eq!(license_category("PSF-2.0"), "Other");
        assert_eq!(license_category("Artistic-2.0"), "Other");
    }

    #[test]
    fn test_license_map_web_servers() {
        assert_eq!(
            detect_license("nginx", "deb"),
            Some("BSD-2-Clause".to_string())
        );
        assert_eq!(
            detect_license("apache2", "deb"),
            Some("Apache-2.0".to_string())
        );
        assert_eq!(
            detect_license("httpd", "deb"),
            Some("Apache-2.0".to_string())
        );
    }

    #[test]
    fn test_license_map_databases() {
        assert_eq!(
            detect_license("postgresql", "deb"),
            Some("PostgreSQL".to_string())
        );
        assert_eq!(
            detect_license("mysql", "deb"),
            Some("GPL-2.0-only".to_string())
        );
        assert_eq!(
            detect_license("mariadb", "deb"),
            Some("GPL-2.0-only".to_string())
        );
        assert_eq!(
            detect_license("redis", "deb"),
            Some("BSD-3-Clause".to_string())
        );
        assert_eq!(
            detect_license("sqlite3", "deb"),
            Some("Public-Domain".to_string())
        );
    }

    #[test]
    fn test_license_map_programming_languages() {
        assert_eq!(
            detect_license("python3", "deb"),
            Some("PSF-2.0".to_string())
        );
        assert_eq!(
            detect_license("python2", "deb"),
            Some("PSF-2.0".to_string())
        );
        assert_eq!(
            detect_license("perl", "deb"),
            Some("Artistic-2.0".to_string())
        );
        assert_eq!(detect_license("nodejs", "deb"), Some("MIT".to_string()));
    }

    #[test]
    fn test_license_map_system_tools() {
        assert_eq!(
            detect_license("bash", "deb"),
            Some("GPL-3.0-or-later".to_string())
        );
        assert_eq!(
            detect_license("coreutils", "deb"),
            Some("GPL-3.0-or-later".to_string())
        );
        assert_eq!(
            detect_license("systemd", "deb"),
            Some("LGPL-2.1-or-later".to_string())
        );
        assert_eq!(detect_license("vim", "deb"), Some("Vim".to_string()));
        assert_eq!(
            detect_license("emacs", "deb"),
            Some("GPL-3.0-or-later".to_string())
        );
    }

    #[test]
    fn test_license_map_libraries() {
        assert_eq!(
            detect_license("openssl", "deb"),
            Some("Apache-2.0".to_string())
        );
        assert_eq!(
            detect_license("glibc", "deb"),
            Some("LGPL-2.1-or-later".to_string())
        );
        assert_eq!(
            detect_license("libc6", "deb"),
            Some("LGPL-2.1-or-later".to_string())
        );
        assert_eq!(detect_license("zlib", "deb"), Some("Zlib".to_string()));
        assert_eq!(detect_license("curl", "deb"), Some("MIT".to_string()));
    }
}
