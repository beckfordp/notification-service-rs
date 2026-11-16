// SPDX-License-Identifier: MIT

//! `PgSentEmails` against a throwaway Postgres, migrated as the service
//! migrates it: the Scala `listEmails` query's filters and order
//! (`created_at desc, id`), the stored values as read back, and failures.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::time::Duration;

use notification_service::config::PostgresConfig;
use notification_service::db;
use notification_service::domain::{EmailFilter, EmailStatus, RecipientType, SentEmail};
use notification_service::store::{PgSentEmails, SentEmails, StoreError};
use support::Postgres;
use time::macros::datetime;
use uuid::Uuid;

/// The store over `postgres`, migrated.
async fn store(postgres: &Postgres) -> PgSentEmails {
    let env: std::collections::HashMap<_, _> = postgres.env().into_iter().collect();
    let config = PostgresConfig {
        host: env["POSTGRES_HOST"].clone(),
        port: env["POSTGRES_PORT"].parse().unwrap(),
        database: env["POSTGRES_DB"].clone(),
        user: env["POSTGRES_USER"].clone(),
        password: env["POSTGRES_PASSWORD"].clone(),
    };
    let pool = db::connect(&config).await.unwrap();
    db::migrate(&pool).await.unwrap();
    PgSentEmails::new(pool)
}

/// Inserts a sent row; `None` columns stay null.
fn insert(
    postgres: &Postgres,
    id: &str,
    customer: Option<&str>,
    order: Option<&str>,
    created: &str,
) {
    let text = |value: Option<&str>| value.map_or("null".to_owned(), |value| format!("'{value}'"));
    postgres.query(&format!(
        "insert into sent_email (id, source_event_id, event_type, reason, recipient_type, \
         recipient_key, customer_id, order_id, subject, body, status, created_at, sent_at, \
         failed_at, error, recipient_email) values ('{id}', 'event-{id}', 'OrderConfirmed', null, \
         'customer', 'key', {}, {}, 'subject', 'body', 'sent', '{created}', '{created}', null, \
         null, 'ada@example.com')",
        text(customer),
        text(order)
    ));
}

fn filter(customer_id: Option<&str>, order_id: Option<&str>) -> EmailFilter {
    EmailFilter {
        customer_id: customer_id.map(str::to_owned),
        order_id: order_id.map(str::to_owned),
    }
}

async fn ids(store: &PgSentEmails, filter: EmailFilter) -> Vec<String> {
    store
        .list(&filter)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.id.to_string())
        .collect()
}

const A: &str = "00000000-0000-4000-8000-00000000000a";
const B: &str = "00000000-0000-4000-8000-00000000000b";
const C: &str = "00000000-0000-4000-8000-00000000000c";
const D: &str = "00000000-0000-4000-8000-00000000000d";

#[tokio::test]
async fn filters_match_exact_text_combined_with_and_newest_first_ties_by_id() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    assert_eq!(ids(&store, filter(None, None)).await, Vec::<String>::new());
    // B and C tie on created_at: id breaks it, ascending.
    insert(
        &postgres,
        C,
        Some("c1"),
        Some("o1"),
        "2026-11-11 11:21:02.5+00",
    );
    insert(
        &postgres,
        A,
        Some("c1"),
        Some("o2"),
        "2026-11-11 11:21:01+00",
    );
    insert(
        &postgres,
        B,
        Some("c2"),
        Some("o1"),
        "2026-11-11 11:21:02.5+00",
    );
    insert(&postgres, D, None, Some(""), "2026-11-11 11:21:03+00");
    assert_eq!(ids(&store, filter(None, None)).await, [D, B, C, A]);
    assert_eq!(ids(&store, filter(Some("c1"), None)).await, [C, A]);
    assert_eq!(ids(&store, filter(None, Some("o1"))).await, [B, C]);
    assert_eq!(ids(&store, filter(Some("c1"), Some("o1"))).await, [C]);
    assert_eq!(
        ids(&store, filter(Some("C1"), None)).await,
        Vec::<String>::new()
    );
    // An empty filter matches the empty string, never a null (N3).
    assert_eq!(ids(&store, filter(None, Some(""))).await, [D]);
    assert_eq!(
        ids(&store, filter(Some(""), None)).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn stored_values_come_back_intact() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    insert(
        &postgres,
        A,
        Some("c1"),
        None,
        "2026-11-11 11:21:06.84858+00",
    );
    postgres.query(&format!(
        "update sent_email set status = 'unresolved', sent_at = null, \
         failed_at = '2026-11-11 11:21:07+00', error = 'unknown_customer:x', reason = 'lost', \
         recipient_type = 'order', recipient_email = null where id = '{A}'"
    ));
    assert_eq!(
        store.list(&EmailFilter::default()).await.unwrap(),
        vec![SentEmail {
            id: Uuid::parse_str(A).unwrap(),
            source_event_id: format!("event-{A}"),
            event_type: "OrderConfirmed".to_owned(),
            reason: Some("lost".to_owned()),
            recipient_type: RecipientType::Order,
            recipient_key: "key".to_owned(),
            recipient_email: None,
            customer_id: Some("c1".to_owned()),
            order_id: None,
            subject: "subject".to_owned(),
            body: "body".to_owned(),
            status: EmailStatus::Unresolved,
            created_at: datetime!(2026-11-11 11:21:06.84858 UTC),
            sent_at: None,
            failed_at: Some(datetime!(2026-11-11 11:21:07 UTC)),
            error: Some("unknown_customer:x".to_owned()),
        }]
    );
}

#[tokio::test]
async fn a_value_the_service_does_not_know_is_an_error_naming_it() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    insert(&postgres, A, Some("c1"), None, "2026-11-11 11:21:06+00");
    postgres.query("alter table sent_email drop constraint sent_email_status_check");
    postgres.query("update sent_email set status = 'bounced'");
    let error = store.list(&EmailFilter::default()).await.unwrap_err();
    assert!(
        matches!(&error, StoreError::UnknownValue { column: "status", value } if value == "bounced"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "sent_email.status holds an unknown value \"bounced\""
    );
}

#[tokio::test]
async fn a_stopped_database_is_an_error_and_a_failed_ping() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    assert!(store.ping(Duration::from_secs(2)).await);
    postgres.stop();
    let listed = store.list(&EmailFilter::default()).await;
    let pinged = store.ping(Duration::from_secs(2)).await;
    postgres.start_again();
    assert!(matches!(listed, Err(StoreError::Database(_))), "{listed:?}");
    assert!(!pinged);
}
