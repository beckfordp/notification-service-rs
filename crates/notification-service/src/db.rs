// SPDX-License-Identifier: MIT

//! The service's database: the pool, the migrations and the readiness ping.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::config::PostgresConfig;

/// Connections in the pool, as user-service-rs.
const POOL_SIZE: u32 = 10;
/// How long a request waits for a connection before failing.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);

/// Connects the pool (one connection is opened now, so a wrong address fails at startup).
///
/// # Errors
/// When the database can't be reached or refuses the login.
pub async fn connect(config: &PostgresConfig) -> Result<PgPool, sqlx::Error> {
    let options = PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .database(&config.database)
        .username(&config.user)
        .password(&config.password);
    PgPoolOptions::new()
        .max_connections(POOL_SIZE)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect_with(options)
        .await
}

/// Applies the Scala service's Flyway migrations V1 and V2, copied verbatim
/// (recorded in `_sqlx_migrations`: the contract is `sent_email`, not
/// Flyway's history table).
///
/// # Errors
/// When a migration fails.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Whether Postgres answers `SELECT 1` within `within`. Bounded, so a paused
/// database is a quick `false`, not a hang (tests/contract implicit
/// behaviour N1).
pub async fn ping(pool: &PgPool, within: Duration) -> bool {
    let work = async {
        let mut conn = pool.acquire().await?;
        sqlx::query("SELECT 1").execute(&mut *conn).await?;
        Ok::<(), sqlx::Error>(())
    };
    matches!(tokio::time::timeout(within, work).await, Ok(Ok(())))
}
