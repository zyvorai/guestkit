// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use sqlx::PgPool;

pub async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS vm_images (
            id UUID PRIMARY KEY,
            tenant TEXT NOT NULL DEFAULT 'default',
            name TEXT NOT NULL,
            object_key TEXT NOT NULL,
            format TEXT NOT NULL,
            size_bytes BIGINT NOT NULL DEFAULT 0,
            checksum TEXT,
            status TEXT NOT NULL DEFAULT 'imported',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS jobs (
            id UUID PRIMARY KEY,
            vm_id UUID NOT NULL REFERENCES vm_images(id),
            operation TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            worker_id TEXT,
            submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            completed_at TIMESTAMPTZ
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS job_results (
            job_id UUID PRIMARY KEY REFERENCES jobs(id),
            result JSONB,
            artifacts JSONB,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS auth_settings (
            id INT PRIMARY KEY,
            settings JSONB NOT NULL DEFAULT '{}',
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
