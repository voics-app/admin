use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::rollup_store::db_error;
use crate::application::ports::PortResult;
use crate::application::ports::audit_log::AuditLog;
use crate::domain::audit::{AuditEntry, NewAuditEntry};

#[derive(Clone)]
pub struct PgAuditLog {
    pool: PgPool,
}

impl PgAuditLog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLog for PgAuditLog {
    async fn record(&self, entry: &NewAuditEntry) -> PortResult<()> {
        sqlx::query("INSERT INTO admin_audit (actor_email, action, target, details) VALUES ($1, $2, $3, $4::jsonb)")
            .bind(&entry.actor_email)
            .bind(entry.action.as_str())
            .bind(&entry.target)
            .bind(&entry.details)
            .execute(&self.pool)
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn list(&self, limit: u32, before_id: Option<i64>) -> PortResult<Vec<AuditEntry>> {
        let rows: Vec<(i64, DateTime<Utc>, String, String, String, String)> = sqlx::query_as(
            "SELECT id, at, actor_email, action, target, details::text FROM admin_audit \
             WHERE ($1::bigint IS NULL OR id < $1) ORDER BY id DESC LIMIT $2",
        )
        .bind(before_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        Ok(rows
            .into_iter()
            .map(|(id, at, actor_email, action, target, details)| AuditEntry { id, at, actor_email, action, target, details })
            .collect())
    }
}
