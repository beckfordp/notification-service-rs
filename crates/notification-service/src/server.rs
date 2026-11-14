// SPDX-License-Identifier: MIT

//! The HTTP server: routes, binding and graceful shutdown. The health routes
//! and tapir's plain-text errors are gluon-rs `service-basics`.

use std::future::Future;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::Router;
use service_basics::health::{self, Readiness};
use service_basics::postgres;
use service_basics::text_error::{method_not_allowed, not_found};
use sqlx::PgPool;
use tokio::net::TcpListener;

/// How long readiness waits for Postgres to answer.
const READY_PING: Duration = Duration::from_secs(2);

/// What the handlers share.
#[derive(Clone)]
pub struct AppState {
    /// The service's database.
    pub pool: PgPool,
}

/// Ready when Postgres answers a ping within 2 s (capture
/// `http/health-ready*.txt`).
impl Readiness for AppState {
    async fn ready(&self) -> bool {
        postgres::ping(&self.pool, READY_PING).await.is_ok()
    }
}

/// The service's routes: the health routes, and tapir's 405 for a known path
/// with another method, 404 for any other (capture `http/unknown-path.txt`).
pub fn router(pool: PgPool) -> Router {
    health::routes()
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(AppState { pool })
}

/// Serves the API on `0.0.0.0:<port>` until `shutdown` completes.
///
/// # Errors
/// When the port can't be bound or serving fails.
pub async fn run(
    port: u16,
    pool: PgPool,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))).await?;
    axum::serve(listener, router(pool))
        .with_graceful_shutdown(shutdown)
        .await
}
