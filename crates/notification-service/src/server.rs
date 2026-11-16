// SPDX-License-Identifier: MIT

//! The HTTP server: routes, binding and graceful shutdown. The health routes
//! and tapir's plain-text errors are gluon-rs `service-basics`.

use std::future::Future;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::extract::Request;
use axum::http::header::ALLOW;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::any;
use axum::{Extension, Router};
use service_basics::health::{self, Readiness};
use service_basics::text_error::{self, TextError, method_not_allowed};
use tokio::net::TcpListener;

use crate::cors::{self, Unrouted};
use crate::emails::list_emails;
use crate::store::SentEmails;

/// How long readiness waits for Postgres to answer.
const READY_PING: Duration = Duration::from_secs(2);

/// What the handlers share.
#[derive(Clone)]
pub struct AppState<S> {
    /// The `sent_email` table.
    pub store: S,
}

/// Ready when Postgres answers a ping within 2 s (capture
/// `http/health-ready*.txt`).
impl<S: SentEmails> Readiness for AppState<S> {
    async fn ready(&self) -> bool {
        self.store.ping(READY_PING).await
    }
}

/// The service: its routes ([`routes`]) as the Scala service's middleware
/// sees them, CORS outermost (`AppRoutes`). Both layers wrap the whole
/// router, so they see its final answer: axum's 405s and the 404 fallback.
pub fn app<S: SentEmails>(store: S) -> Router {
    Router::new()
        .fallback_service(routes(store))
        .layer(middleware::from_fn(without_allow))
        .layer(middleware::from_fn(cors::cors))
}

/// The routes: health, `/emails` (GET only, `/emails/` too), tapir's 405 for
/// a known path with another method and 404 for any other (capture
/// `http/unknown-path.txt`).
pub fn routes<S: SentEmails>(store: S) -> Router {
    let emails = any(method_not_allowed)
        .get(list_emails::<S>)
        .head(method_not_allowed);
    health::routes()
        .route("/emails", emails.clone())
        .route("/emails/", emails)
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(AppState { store })
}

/// 404 for any unknown path, marked [`Unrouted`] for CORS.
async fn not_found() -> (Extension<Unrouted>, TextError) {
    (Extension(Unrouted), text_error::not_found().await)
}

/// tapir's 405 has no `Allow` header (capture `http/emails-post.txt`,
/// `health-head.txt`); axum's method router adds one.
async fn without_allow(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().remove(ALLOW);
    response
}

/// Serves the API on `0.0.0.0:<port>` until `shutdown` completes.
///
/// # Errors
/// When the port can't be bound or serving fails.
pub async fn run<S: SentEmails>(
    port: u16,
    store: S,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))).await?;
    axum::serve(listener, app(store))
        .with_graceful_shutdown(shutdown)
        .await
}
