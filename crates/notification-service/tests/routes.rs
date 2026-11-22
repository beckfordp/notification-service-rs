// SPDX-License-Identifier: MIT

//! The HTTP surface against an in-memory store: `/emails`, its errors, the
//! other methods, and CORS (captures `http/emails-*.txt`, `cors-*.txt`;
//! http4s `CORSPolicy` for what they don't show). Postgres-backed behaviour is
//! `tests/store.rs` and `tests/emails.rs`; wire parity is the harness's (`test_notification_rest_wire.py`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::future::{Future, ready};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use notification_service::domain::{EmailFilter, EmailStatus, RecipientType, SentEmail};
use notification_service::server::app;
use notification_service::store::{SentEmails, StoreError};
use serde_json::{Value, json};
use time::macros::datetime;
use tower::ServiceExt;
use uuid::Uuid;

/// An in-memory `sent_email`: answers with its rows (already in order), or
/// fails; records each filter it was asked for.
#[derive(Clone, Default)]
struct Memory {
    rows: Vec<SentEmail>,
    fail: bool,
    down: bool,
    asked: Arc<Mutex<Vec<EmailFilter>>>,
}

impl SentEmails for Memory {
    fn list(
        &self,
        filter: &EmailFilter,
    ) -> impl Future<Output = Result<Vec<SentEmail>, StoreError>> + Send {
        self.asked.lock().unwrap().push(filter.clone());
        ready(if self.fail {
            Err(StoreError::Database(sqlx::Error::PoolTimedOut))
        } else {
            Ok(self.rows.clone())
        })
    }

    fn ping(&self, _within: Duration) -> impl Future<Output = bool> + Send {
        ready(!self.down)
    }
}

fn failed_row() -> SentEmail {
    // The row CAPTURE.sh step 6 made (capture http/emails-after-flaky.txt).
    SentEmail {
        id: Uuid::parse_str("f4d69436-721a-40aa-833b-edb631026c35").unwrap(),
        source_event_id: "e0000000-0000-4000-8000-000000000050".to_owned(),
        event_type: "OrderConfirmed".to_owned(),
        reason: None,
        recipient_type: RecipientType::Customer,
        recipient_key: "33333333-3333-4333-8333-333333333333".to_owned(),
        recipient_email: None,
        customer_id: Some("33333333-3333-4333-8333-333333333333".to_owned()),
        order_id: Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned()),
        subject: "Your order is confirmed".to_owned(),
        body: "Good news - order aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa is confirmed.".to_owned(),
        status: EmailStatus::Failed,
        created_at: datetime!(2026-11-11 11:21:33.857235 UTC),
        sent_at: None,
        failed_at: Some(datetime!(2026-11-11 11:21:36.144438 UTC)),
        error: Some("Circuit breaker is open".to_owned()),
    }
}

fn sent_row() -> SentEmail {
    SentEmail {
        id: Uuid::parse_str("7dfbf75f-aec7-4a4a-b79a-fdb7b76765d0").unwrap(),
        source_event_id: "e0000000-0000-4000-8000-000000000041".to_owned(),
        event_type: "DispatchFailed".to_owned(),
        reason: Some("carrier_unavailable".to_owned()),
        recipient_type: RecipientType::Order,
        recipient_key: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
        recipient_email: Some("ada@example.com".to_owned()),
        customer_id: Some("11111111-1111-4111-8111-111111111111".to_owned()),
        order_id: Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned()),
        subject: "We couldn't ship your order".to_owned(),
        body: "Order aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa could not be dispatched: carrier_unavailable."
            .to_owned(),
        status: EmailStatus::Sent,
        created_at: datetime!(2026-11-11 11:21:12.952901 UTC),
        sent_at: Some(datetime!(2026-11-11 11:21:12.972031 UTC)),
        failed_at: None,
        error: None,
    }
}

struct Answer {
    status: StatusCode,
    headers: Vec<(String, String)>,
    body: String,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn names(&self) -> Vec<&str> {
        self.headers.iter().map(|(key, _)| key.as_str()).collect()
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

async fn send(app: Router, method: &str, uri: &str, headers: &[(&str, &str)]) -> Answer {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.to_str().unwrap().to_owned()))
        .collect();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    Answer {
        status,
        headers,
        body: String::from_utf8(body.to_vec()).unwrap(),
    }
}

async fn get(store: &Memory, uri: &str) -> Answer {
    send(app(store.clone()), "GET", uri, &[]).await
}

const TEXT: &str = "text/plain; charset=UTF-8";
const ORIGIN: (&str, &str) = ("origin", "http://localhost:5173");
const VARY: &str = "Access-Control-Request-Method, Access-Control-Request-Headers";

// --- GET /emails -------------------------------------------------------------------------------

#[tokio::test]
async fn emails_are_the_stores_rows_in_its_order_every_field_as_the_scala_service_writes_it() {
    let store = Memory {
        rows: vec![failed_row(), sent_row()],
        ..Memory::default()
    };
    let answer = get(&store, "/emails").await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    // capture http/emails-after-flaky.txt's row, then emails-all.txt's second.
    assert_eq!(
        answer.json(),
        json!([
            {"id":"f4d69436-721a-40aa-833b-edb631026c35","sourceEventId":"e0000000-0000-4000-8000-000000000050","eventType":"OrderConfirmed","reason":null,"recipientType":"customer","recipientKey":"33333333-3333-4333-8333-333333333333","recipientEmail":null,"customerId":"33333333-3333-4333-8333-333333333333","orderId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","subject":"Your order is confirmed","body":"Good news - order aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa is confirmed.","status":"failed","createdAt":"2026-11-11T11:21:33.857235Z","sentAt":null,"failedAt":"2026-11-11T11:21:36.144438Z","error":"Circuit breaker is open"},
            {"id":"7dfbf75f-aec7-4a4a-b79a-fdb7b76765d0","sourceEventId":"e0000000-0000-4000-8000-000000000041","eventType":"DispatchFailed","reason":"carrier_unavailable","recipientType":"order","recipientKey":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","recipientEmail":"ada@example.com","customerId":"11111111-1111-4111-8111-111111111111","orderId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","subject":"We couldn't ship your order","body":"Order aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa could not be dispatched: carrier_unavailable.","status":"sent","createdAt":"2026-11-11T11:21:12.952901Z","sentAt":"2026-11-11T11:21:12.972031Z","failedAt":null,"error":null}
        ])
    );
    // The fields in SentEmailResponse's order, as circe writes them.
    assert!(answer.body.starts_with(
        r#"[{"id":"f4d69436-721a-40aa-833b-edb631026c35","sourceEventId":"e0000000-0000-4000-8000-000000000050","eventType":"OrderConfirmed","reason":null,"recipientType""#
    ));
    assert_eq!(*store.asked.lock().unwrap(), vec![EmailFilter::default()]);
}

#[tokio::test]
async fn no_rows_is_an_empty_array() {
    let answer = get(&Memory::default(), "/emails?customerId=nobody").await;
    assert_eq!(
        (answer.status, answer.body.as_str()),
        (StatusCode::OK, "[]")
    );
}

#[tokio::test]
async fn the_filters_reach_the_store_and_a_trailing_slash_is_the_same_resource() {
    let store = Memory::default();
    for uri in [
        "/emails?customerId=c&orderId=o&foo=bar",
        "/emails/?customerId=c&orderId=o",
    ] {
        assert_eq!(get(&store, uri).await.status, StatusCode::OK, "{uri}");
    }
    let wanted = EmailFilter {
        customer_id: Some("c".to_owned()),
        order_id: Some("o".to_owned()),
    };
    assert_eq!(*store.asked.lock().unwrap(), vec![wanted.clone(), wanted]);
}

#[tokio::test]
async fn a_repeated_filter_is_a_plain_text_400_and_the_store_is_not_asked() {
    let store = Memory::default();
    let answer = get(&store, "/emails?customerId=a&customerId=b").await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.header("content-type"), Some(TEXT));
    assert_eq!(
        answer.body,
        "Invalid value for: query parameter customerId (multiple values)"
    );
    assert!(store.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_store_failure_is_tapirs_plain_text_500() {
    let store = Memory {
        fail: true,
        ..Memory::default()
    };
    let answer = get(&store, "/emails").await;
    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(answer.header("content-type"), Some(TEXT));
    assert_eq!(answer.body, "Internal server error");
}

#[tokio::test]
async fn any_other_method_is_a_plain_text_405_without_allow() {
    for method in ["POST", "PUT", "PATCH", "DELETE", "OPTIONS", "TRACE", "HEAD"] {
        for path in ["/emails", "/emails/", "/health", "/health/ready"] {
            let answer = send(app(Memory::default()), method, path, &[]).await;
            assert_eq!(
                answer.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path}"
            );
            assert_eq!(answer.header("content-type"), Some(TEXT), "{method} {path}");
            assert_eq!(
                answer.header("content-length"),
                Some("18"),
                "{method} {path}"
            );
            assert_eq!(answer.header("allow"), None, "{method} {path}");
            let body = if method == "HEAD" {
                ""
            } else {
                "Method Not Allowed"
            };
            assert_eq!(answer.body, body, "{method} {path}");
        }
    }
}

#[tokio::test]
async fn an_unknown_path_is_a_plain_text_404() {
    let answer = get(&Memory::default(), "/emails/x").await;
    assert_eq!(
        (answer.status, answer.body.as_str()),
        (StatusCode::NOT_FOUND, "Not found")
    );
}

#[tokio::test]
async fn readiness_follows_the_stores_ping() {
    assert_eq!(
        get(&Memory::default(), "/health/ready").await.status,
        StatusCode::OK
    );
    let down = Memory {
        down: true,
        ..Memory::default()
    };
    assert_eq!(
        get(&down, "/health/ready").await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

// --- CORS --------------------------------------------------------------------------------------

async fn preflight(path: &str, method: &str, request_headers: Option<&str>) -> Answer {
    let mut headers = vec![ORIGIN, ("access-control-request-method", method)];
    if let Some(names) = request_headers {
        headers.push(("access-control-request-headers", names));
    }
    send(app(Memory::default()), "OPTIONS", path, &headers).await
}

fn assert_allowed(answer: &Answer, what: &str) {
    assert_eq!(answer.status, StatusCode::OK, "{what}");
    assert_eq!(answer.body, "", "{what}");
    assert_eq!(
        answer.header("access-control-allow-origin"),
        Some("*"),
        "{what}"
    );
    assert_eq!(
        answer.header("access-control-allow-methods"),
        Some("PATCH, PUT, GET, POST, DELETE"),
        "{what}"
    );
    assert_eq!(
        answer.header("access-control-allow-headers"),
        Some("Content-Type"),
        "{what}"
    );
    assert_eq!(answer.header("vary"), Some(VARY), "{what}");
    assert_eq!(
        answer.header("access-control-expose-headers"),
        None,
        "{what}"
    );
    assert_eq!(
        answer.header("access-control-allow-credentials"),
        None,
        "{what}"
    );
    assert_eq!(answer.header("access-control-max-age"), None, "{what}");
}

fn assert_refused(answer: &Answer, what: &str) {
    assert_eq!(answer.status, StatusCode::OK, "{what}");
    assert_eq!(answer.body, "", "{what}");
    assert_eq!(answer.header("vary"), Some(VARY), "{what}");
    assert!(
        !answer
            .names()
            .iter()
            .any(|name| name.starts_with("access-control-")),
        "{what}: {:?}",
        answer.headers
    );
}

#[tokio::test]
async fn a_preflight_the_policy_allows_gets_every_allow_header() {
    // capture http/cors-preflight.txt
    assert_allowed(
        &preflight("/emails", "GET", Some("content-type")).await,
        "as captured",
    );
    for (method, names) in [
        ("DELETE", None),
        ("PATCH", Some("Content-Type")),
        ("POST", Some("CONTENT-TYPE , content-type")),
        ("PUT", Some("content-type,")),
    ] {
        assert_allowed(
            &preflight("/emails", method, names).await,
            &format!("{method} {names:?}"),
        );
    }
}

#[tokio::test]
async fn a_preflight_on_any_path_is_answered_by_the_policy() {
    for path in ["/no-such-path", "/health", "/emails/"] {
        assert_allowed(&preflight(path, "GET", None).await, path);
    }
}

#[tokio::test]
async fn a_preflight_the_policy_refuses_gets_only_vary() {
    // capture http/cors-preflight-bad-header.txt (N8)
    assert_refused(
        &preflight("/emails", "GET", Some("authorization")).await,
        "authorization",
    );
    for (method, names) in [
        ("GET", Some("content-type, authorization")),
        ("GET", Some("")),
        ("HEAD", None),
        ("OPTIONS", Some("content-type")),
        ("get", None),
    ] {
        assert_refused(
            &preflight("/emails", method, names).await,
            &format!("{method} {names:?}"),
        );
    }
}

#[tokio::test]
async fn a_cross_origin_request_gets_only_allow_origin() {
    // capture http/cors-simple.txt
    let store = Memory {
        rows: vec![sent_row()],
        ..Memory::default()
    };
    let answer = send(app(store), "GET", "/emails", &[ORIGIN]).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.header("access-control-allow-origin"), Some("*"));
    assert_eq!(answer.json().as_array().unwrap().len(), 1);
    let cors: Vec<&str> = answer
        .names()
        .into_iter()
        .filter(|name| name.starts_with("access-control-") || *name == "vary")
        .collect();
    assert_eq!(cors, vec!["access-control-allow-origin"]);
    // Errors from a route too.
    let post = send(app(Memory::default()), "POST", "/emails", &[ORIGIN]).await;
    assert_eq!(post.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(post.header("access-control-allow-origin"), Some("*"));
}

#[tokio::test]
async fn no_route_means_no_cors_headers() {
    let answer = send(app(Memory::default()), "GET", "/no-such-path", &[ORIGIN]).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(answer.header("access-control-allow-origin"), None);
    let options = send(app(Memory::default()), "OPTIONS", "/no-such-path", &[]).await;
    assert_eq!(options.status, StatusCode::NOT_FOUND);
    assert_eq!(options.header("vary"), None);
}

#[tokio::test]
async fn an_options_request_that_isnt_a_preflight_is_routed_and_gets_vary() {
    let with_origin = send(app(Memory::default()), "OPTIONS", "/emails", &[ORIGIN]).await;
    assert_eq!(with_origin.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(with_origin.header("vary"), Some(VARY));
    assert_eq!(with_origin.header("access-control-allow-origin"), Some("*"));
    // A method that isn't a token isn't a preflight either.
    let bad_method = send(
        app(Memory::default()),
        "OPTIONS",
        "/emails",
        &[ORIGIN, ("access-control-request-method", "G T")],
    )
    .await;
    assert_eq!(bad_method.status, StatusCode::METHOD_NOT_ALLOWED);
    let without_origin = send(
        app(Memory::default()),
        "OPTIONS",
        "/emails",
        &[("access-control-request-method", "GET")],
    )
    .await;
    assert_eq!(without_origin.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(without_origin.header("vary"), Some(VARY));
    assert_eq!(without_origin.header("access-control-allow-origin"), None);
}

#[tokio::test]
async fn a_get_without_origin_gets_no_cors_headers() {
    let answer = get(&Memory::default(), "/emails").await;
    assert!(
        !answer
            .names()
            .iter()
            .any(|name| name.starts_with("access-control-") || *name == "vary"),
        "{:?}",
        answer.headers
    );
}
