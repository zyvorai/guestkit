// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use super::types::{AuthUserClaims, IdentitySettings, ROLE_ADMIN, ROLE_OPERATOR, ROLE_VIEWER};

pub fn is_admin(user: &AuthUserClaims) -> bool {
    user.role.eq_ignore_ascii_case(ROLE_ADMIN)
}

pub fn can_approve_guest_actions(user: &AuthUserClaims) -> bool {
    is_admin(user) || user.role.eq_ignore_ascii_case(ROLE_OPERATOR)
}

pub fn is_viewer(user: &AuthUserClaims) -> bool {
    user.role.eq_ignore_ascii_case(ROLE_VIEWER)
}

pub fn can_request_guest_remediation(user: &AuthUserClaims) -> bool {
    can_approve_guest_actions(user)
}

pub fn resolve_role(
    identity: &IdentitySettings,
    email: Option<&str>,
    _name: Option<&str>,
    groups: &[String],
) -> String {
    if let Some(email) = email {
        if identity
            .admin_emails
            .iter()
            .any(|e| e.eq_ignore_ascii_case(email))
        {
            return ROLE_ADMIN.into();
        }
    }
    for group in groups {
        if identity
            .admin_roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case(group))
        {
            return ROLE_ADMIN.into();
        }
    }
    if identity.default_role.trim().is_empty() {
        "operator".into()
    } else {
        identity.default_role.clone()
    }
}

pub fn groups_from_json(value: &serde_json::Value, claim: &str) -> Vec<String> {
    let claim = if claim.trim().is_empty() {
        "groups"
    } else {
        claim
    };
    value
        .get(claim)
        .and_then(|v| {
            if let Some(arr) = v.as_array() {
                Some(
                    arr.iter()
                        .filter_map(|item| item.as_str().map(String::from))
                        .collect(),
                )
            } else {
                v.as_str().map(|s| vec![s.to_string()])
            }
        })
        .unwrap_or_default()
}

pub fn email_from_claims(email: Option<&str>, name: Option<&str>, sub: &str) -> Option<String> {
    email
        .map(String::from)
        .or_else(|| name.filter(|n| n.contains('@')).map(String::from))
        .or_else(|| sub.contains('@').then(|| sub.to_string()))
}
