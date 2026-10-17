use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::rollup_store::db_error;
use crate::application::ports::feature_flags::{FeatureFlagRepository, StoredFlag};
use crate::application::ports::{PortError, PortResult};
use crate::domain::feature_flag::{FeatureFlag, FlagKey, RolloutPercent};

type FlagRow = (String, String, bool, i16, DateTime<Utc>, String);

#[derive(Clone)]
pub struct PgFeatureFlags {
    pool: PgPool,
}

impl PgFeatureFlags {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn to_stored(row: FlagRow) -> PortResult<StoredFlag> {
    let (key, description, enabled, rollout, updated_at, updated_by) = row;
    let corrupt = |e: crate::domain::feature_flag::FlagError| {
        PortError::Unavailable(format!("stored flag {key} is invalid: {e}"))
    };
    let flag = FeatureFlag::new(
        FlagKey::parse(&key).map_err(corrupt)?,
        &description,
        enabled,
        RolloutPercent::parse(i64::from(rollout)).map_err(corrupt)?,
    )
    .map_err(corrupt)?;
    Ok(StoredFlag {
        flag,
        updated_at,
        updated_by,
    })
}

#[async_trait]
impl FeatureFlagRepository for PgFeatureFlags {
    async fn list(&self) -> PortResult<Vec<StoredFlag>> {
        let rows: Vec<FlagRow> = sqlx::query_as(
            "SELECT key, description, enabled, rollout_percent, updated_at, updated_by FROM feature_flags ORDER BY key",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        rows.into_iter().map(to_stored).collect()
    }

    async fn upsert(&self, flag: &FeatureFlag, updated_by: &str) -> PortResult<StoredFlag> {
        let row: FlagRow = sqlx::query_as(
            "INSERT INTO feature_flags (key, description, enabled, rollout_percent, updated_at, updated_by) \
             VALUES ($1, $2, $3, $4, now(), $5) \
             ON CONFLICT (key) DO UPDATE SET description = EXCLUDED.description, enabled = EXCLUDED.enabled, \
               rollout_percent = EXCLUDED.rollout_percent, updated_at = now(), updated_by = EXCLUDED.updated_by \
             RETURNING key, description, enabled, rollout_percent, updated_at, updated_by",
        )
        .bind(flag.key.as_str())
        .bind(&flag.description)
        .bind(flag.enabled)
        .bind(i16::from(flag.rollout_percent.value()))
        .bind(updated_by)
        .fetch_one(&self.pool)
        .await
        .map_err(db_error)?;
        to_stored(row)
    }

    async fn delete(&self, key: &FlagKey) -> PortResult<()> {
        let result = sqlx::query("DELETE FROM feature_flags WHERE key = $1")
            .bind(key.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_error)?;
        if result.rows_affected() == 0 {
            Err(PortError::NotFound)
        } else {
            Ok(())
        }
    }
}
