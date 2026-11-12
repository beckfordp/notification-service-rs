// SPDX-License-Identifier: MIT

//! The HTTP server: routes, binding and graceful shutdown.

use std::future::Future;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::db;

/// How long readiness waits for Postgres to answer.
const READY_PING: Duration = Duration::from_secs(2);

/// Exactly what the Scala service sends for text errors (not axum's `utf-8`).
const TEXT_PLAIN: &str = "text/plain; charset=UTF-8";

/// The service's routes.
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(health).head(method_not_allowed))
        .route("/health/ready", get(ready).head(method_not_allowed))
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(pool)
}

/// A framework error as tapir writes it: plain text.
fn text_error(status: StatusCode, message: &'static str) -> Response {
    (status, [(header::CONTENT_TYPE, TEXT_PLAIN)], message).into_response()
}

/// 405, as tapir answers a known path with another method. HEAD included:
/// tapir doesn't answer HEAD for a GET endpoint (capture
/// `http/health-head.txt`).
async fn method_not_allowed() -> Response {
    text_error(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed")
}

/// 404 for any unknown path (capture `http/unknown-path.txt`).
async fn not_found() -> Response {
    text_error(StatusCode::NOT_FOUND, "Not found")
}

/// `GET /health`: 200 with an empty body while the process is up; checks no
/// dependencies (capture `http/health.txt`: no Content-Type).
async fn health() -> StatusCode {
    StatusCode::OK
}

/// `GET /health/ready`: 200 when Postgres answers a ping within 2 s, 503 when
/// it doesn't (capture `http/health-ready*.txt`); empty body either way.
async fn ready(State(pool): State<PgPool>) -> StatusCode {
    if db::ping(&pool, READY_PING).await {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
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
