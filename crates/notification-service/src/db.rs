// SPDX-License-Identifier: MIT

//! The service's database: the pool and the migrations. The connection and
//! the readiness ping are gluon-rs `service-basics::postgres`.

use sqlx::PgPool;

use crate::config::PostgresConfig;

/// Connections in the pool, as user-service-rs.
const POOL_SIZE: u32 = 10;

/// Connects the pool (one connection is opened now, so a wrong address fails at startup).
///
/// # Errors
/// When the database can't be reached or refuses the login.
pub async fn connect(config: &PostgresConfig) -> Result<PgPool, sqlx::Error> {
    service_basics::postgres::connect(config, POOL_SIZE).await
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
