// SPDX-License-Identifier: MIT

//! `GET /emails` end to end: the started binary over a throwaway Postgres.
//! The query's filters and order are `tests/store.rs`'s; the JSON and the
//! errors in detail are `tests/routes.rs`'s.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use serde_json::{Value, json};
use support::{Postgres, Service};

const A: &str = "00000000-0000-4000-8000-00000000000a";

#[test]
fn a_stored_row_is_served_as_the_scala_service_writes_it() {
    let postgres = Postgres::start();
    let service = Service::start(&postgres.env());
    postgres.query(&format!(
        "insert into sent_email (id, source_event_id, event_type, reason, recipient_type, \
         recipient_key, customer_id, order_id, subject, body, status, created_at, sent_at, \
         failed_at, error, recipient_email) values ('{A}', 'event-a', 'DispatchFailed', 'lost', \
         'order', 'o1', null, 'o1', 'subject', 'body', 'unresolved', \
         '2026-11-11 11:21:06.84858+00', null, null, 'unknown_customer:x', null)"
    ));
    let (status, content_type, body) = service.get("/emails?orderId=o1").unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!([{
            "id": A, "sourceEventId": "event-a", "eventType": "DispatchFailed",
            "reason": "lost", "recipientType": "order", "recipientKey": "o1",
            "recipientEmail": null, "customerId": null, "orderId": "o1", "subject": "subject",
            "body": "body", "status": "unresolved", "createdAt": "2026-11-11T11:21:06.848580Z",
            "sentAt": null, "failedAt": null, "error": "unknown_customer:x"
        }])
    );
}

#[test]
fn a_stopped_database_is_a_plain_text_500() {
    let postgres = Postgres::start();
    let mut service = Service::start(&postgres.env());
    postgres.stop();
    let answer = service.get("/emails").unwrap();
    postgres.start_again();
    assert_eq!(
        answer,
        (
            500,
            Some("text/plain; charset=UTF-8".to_owned()),
            "Internal server error".to_owned()
        )
    );
    assert!(service.stderr().contains("GET /emails failed"));
}
