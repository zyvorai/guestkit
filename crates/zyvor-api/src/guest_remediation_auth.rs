// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! RBAC helpers for guest remediation enqueue routes.

use crate::auth::rbac::can_request_guest_remediation;
use crate::auth::types::AuthUserClaims;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn require_guest_remediation_requester(
    state: &AppState,
    user: Option<&AuthUserClaims>,
) -> ApiResult<()> {
    if !state.config.auth_enabled {
        return Ok(());
    }
    let user = user.ok_or_else(|| ApiError::unauthorized("authentication required"))?;
    if !can_request_guest_remediation(user) {
        return Err(ApiError::forbidden(
            "operator or admin role required for guest remediation",
        ));
    }
    Ok(())
}
