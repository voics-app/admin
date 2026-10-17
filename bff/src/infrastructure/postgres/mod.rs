//! Postgres adapters for the `discordy_admin` database.

pub mod audit_log;
pub mod feature_flags;
pub mod rollup_store;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Connects and applies the embedded migrations.
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new().max_connections(10).connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
