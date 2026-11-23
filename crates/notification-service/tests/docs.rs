// SPDX-License-Identifier: MIT

//! The docs (NOTIF-D1, D2): the OpenAPI operations and the AsyncAPI channels
//! as the capture's (`openapi.yaml`, `asyncapi.json`), each message's required
//! payload fields as tapir's, and the pages served.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use notification_service::docs;
use serde_json::Value;
use tower::ServiceExt;

fn required(document: &Value, channel: &str, message: &str) -> Vec<String> {
    let payload = &document["channels"][channel]["messages"][message]["payload"];
    let mut found: Vec<String> = payload["required"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .map(|n| n.as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

#[test]
fn the_openapi_document_lists_emails_and_the_health_checks() {
    let yaml = docs::api().to_yaml();
    for path in [
        "'/health':",
        "'/health/ready':",
        "'/emails':",
        "customerId",
        "SentEmailResponse",
    ] {
        assert!(yaml.contains(path), "{path} in {yaml}");
    }
}

#[test]
fn the_asyncapi_document_receives_what_the_scala_service_does() {
    let document = docs::asyncapi();
    let cases: [(&str, &str, &[&str]); 8] = [
        (
            "order-events",
            "OrderConfirmed",
            &["customerId", "orderId", "timestamp"],
        ),
        (
            "order-events",
            "OrderPaymentFailed",
            &["customerId", "orderId", "reason", "timestamp"],
        ),
        (
            "order-events",
            "OrderCancelled",
            &["customerId", "orderId", "timestamp"],
        ),
        ("user-events", "UserRegistered", &["email", "userId"]),
        (
            "fulfilment-events",
            "DispatchConfirmed",
            &["orderId", "trackingNo"],
        ),
        (
            "fulfilment-events",
            "DispatchFailed",
            &["orderId", "reason"],
        ),
        (
            "payment-events",
            "PaymentCaptured",
            &["amountCents", "orderId", "timestamp"],
        ),
        (
            "payment-events",
            "PaymentFailed",
            &["amountCents", "orderId", "paymentId", "reason", "timestamp"],
        ),
    ];
    for (channel, message, want) in cases {
        assert_eq!(
            required(&document, channel, message),
            want,
            "{channel} {message}"
        );
    }
    let actions: Vec<&str> = document["operations"]
        .as_object()
        .unwrap()
        .values()
        .map(|op| op["action"].as_str().unwrap())
        .collect();
    assert!(actions.iter().all(|a| *a == "receive"), "{actions:?}");
}

#[tokio::test]
async fn the_pages_are_served() {
    for (path, has) in [
        ("/docs/", "swagger-ui"),
        ("/docs/docs.yaml", "openapi"),
        ("/docs/asyncapi.json", "asyncapi"),
        ("/docs/asyncapi/", "order-events"),
    ] {
        let response = docs::router()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_lowercase();
        assert!(text.contains(has), "{path}: {has}");
    }
}
