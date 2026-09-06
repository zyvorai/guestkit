// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

const REVOKED_PREFIX: &str = "auth:revoked:";

pub async fn revoke_jti(redis: &mut ConnectionManager, jti: &str, ttl_secs: u64) -> Result<()> {
    if jti.is_empty() || ttl_secs == 0 {
        return Ok(());
    }
    let key = format!("{REVOKED_PREFIX}{jti}");
    redis.set_ex::<_, _, ()>(key, "1", ttl_secs).await?;
    Ok(())
}

pub async fn is_revoked(redis: &mut ConnectionManager, jti: &str) -> Result<bool> {
    if jti.is_empty() {
        return Ok(false);
    }
    let key = format!("{REVOKED_PREFIX}{jti}");
    let exists: bool = redis.exists(key).await?;
    Ok(exists)
}
