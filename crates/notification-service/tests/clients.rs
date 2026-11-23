// SPDX-License-Identifier: MIT

//! user-service's and order-service's lookups (Scala `UserClientSuite`,
//! `OrderClientSuite`) against a local server: 200 decoded, 400 and 404 "not
//! found", anything else raised after the retries, an open breaker raised as
//! `Circuit breaker is open`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::IntoResponse;
use notification_service::clients::{self, Orders, Users};
use notification_service::domain::{CustomerId, EmailAddress, OrderContact, OrderId};
use notification_service::email::LookupError;
use notification_service::recipients::{CustomerEmails, OrderContacts};
use telemetry::metrics::Metrics;
use telemetry::tracing::Tracing;

type Shared = (Arc<Mutex<Vec<String>>>, Arc<Mutex<(u16, String)>>);

/// A local server answering every GET with one status and body, recording
/// each path.
struct Callee {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
    answer: Arc<Mutex<(u16, String)>>,
}

async fn answer(State((seen, answer)): State<Shared>, uri: Uri) -> impl IntoResponse {
    seen.lock().unwrap().push(uri.path().to_owned());
    let (status, body) = answer.lock().unwrap().clone();
    (
        StatusCode::from_u16(status).unwrap(),
        [(header::CONTENT_TYPE, "application/json")],
        body,
    )
}

impl Callee {
    async fn start(status: u16, body: &str) -> Self {
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let answer = Arc::new(Mutex::new((status, body.to_owned())));
        let app = axum::Router::new()
            .fallback(self::answer)
            .with_state((seen.clone(), answer.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, seen, answer }
    }

    fn seen(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
}

fn client() -> telemetry::client::ResilientClient {
    let tracing = Tracing::with_exporter(
        "notification-service",
        1.0,
        opentelemetry_sdk::trace::InMemorySpanExporter::default(),
    );
    clients::resilient(tracing.tracer(), &Metrics::new("notification-service")).unwrap()
}

fn customer() -> CustomerId {
    CustomerId::new("c1")
}

#[tokio::test]
async fn a_known_user_is_their_address() {
    let callee = Callee::start(
        200,
        r#"{"userId":"c1","email":"ada@example.com","createdAt":"x"}"#,
    )
    .await;
    let users = Users::new(&callee.url, client());
    assert_eq!(
        users.email(&customer()).await.unwrap(),
        Some(EmailAddress::new("ada@example.com"))
    );
    assert_eq!(callee.seen(), ["/users/c1"], "the base URL's slash trimmed");
}

#[tokio::test]
async fn not_found_and_bad_request_are_nobody() {
    for status in [400, 404] {
        let callee = Callee::start(status, r#"{"error":"x"}"#).await;
        let users = Users::new(&callee.url, client());
        assert_eq!(users.email(&customer()).await.unwrap(), None, "{status}");
        assert_eq!(callee.seen().len(), 1, "not retried");
    }
}

#[tokio::test]
async fn another_status_is_raised_after_the_retries_then_the_breaker_opens() {
    let callee = Callee::start(500, "").await;
    let users = Users::new(&callee.url, client());
    let error = CustomerEmails::email(&users, &customer())
        .await
        .unwrap_err();
    assert_eq!(
        error,
        LookupError::Failed(
            "Unexpected response from user-service for c1: 500 Internal Server Error".to_owned()
        )
    );
    assert_eq!(callee.seen().len(), 1 + clients::RETRY.max_retries as usize);
    let mut last = error;
    for _ in 0..clients::BREAKER.failure_threshold {
        last = CustomerEmails::email(&users, &customer())
            .await
            .unwrap_err();
    }
    assert_eq!(
        last,
        LookupError::Failed("Circuit breaker is open".to_owned())
    );
}

#[tokio::test]
async fn a_body_it_cannot_read_is_raised() {
    let callee = Callee::start(200, r#"{"userId":"c1"}"#).await;
    let users = Users::new(&callee.url, client());
    assert!(matches!(
        users.email(&customer()).await,
        Err(clients::CallError::Body(_))
    ));
}

#[tokio::test]
async fn an_order_is_its_customer_and_guest_address() {
    let body = r#"{"id":"o1","customerId":"c1","guestEmail":"guest@example.com","status":"confirmed","items":[]}"#;
    let callee = Callee::start(200, body).await;
    let orders = Orders::new(&callee.url, client());
    let order = OrderId::new("o1");
    assert_eq!(
        OrderContacts::contact(&orders, &order).await.unwrap(),
        Some(OrderContact {
            customer_id: customer(),
            guest_email: Some(EmailAddress::new("guest@example.com")),
        })
    );
    assert_eq!(callee.seen(), ["/orders/o1"]);
    *callee.answer.lock().unwrap() = (200, r#"{"customerId":"c1","guestEmail":null}"#.to_owned());
    assert_eq!(
        orders.contact(&order).await.unwrap().unwrap().guest_email,
        None
    );
    *callee.answer.lock().unwrap() = (404, String::new());
    assert_eq!(orders.contact(&order).await.unwrap(), None);
    *callee.answer.lock().unwrap() = (503, String::new());
    assert_eq!(
        OrderContacts::contact(&orders, &order)
            .await
            .unwrap_err()
            .to_string(),
        "Unexpected response from order-service for o1: 503 Service Unavailable"
    );
}
