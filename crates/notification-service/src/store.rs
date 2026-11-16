// SPDX-License-Identifier: MIT

//! The `sent_email` table, as the Scala service reads it (`SentEmailStore`).
//! Handlers reach it through the [`SentEmails`] trait, so they are testable
//! with an in-memory store; [`PgSentEmails`] is the Postgres one.

use std::future::Future;
use std::time::Duration;

use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

use crate::domain::{EmailFilter, EmailStatus, RecipientType, SentEmail};

/// Why a store call failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database refused or failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// A stored value the service doesn't know (the Scala codecs'
    /// `RecipientType.fromString` / `EmailStatus.fromString` failing).
    #[error("sent_email.{column} holds an unknown value {value:?}")]
    UnknownValue {
        /// The column.
        column: &'static str,
        /// What it held.
        value: String,
    },
}

/// What the handlers need from `sent_email`.
pub trait SentEmails: Clone + Send + Sync + 'static {
    /// The rows matching `filter`, newest first (`created_at desc, id`).
    fn list(
        &self,
        filter: &EmailFilter,
    ) -> impl Future<Output = Result<Vec<SentEmail>, StoreError>> + Send;

    /// True when the database answers within `within` (readiness).
    fn ping(&self, within: Duration) -> impl Future<Output = bool> + Send;
}

/// `sent_email` in Postgres.
#[derive(Clone)]
pub struct PgSentEmails {
    pool: PgPool,
}

impl PgSentEmails {
    /// The store over the service's pool.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Scala `SentEmailStore.listEmails`: a null filter matches every row; the
/// tie on `created_at` broken by `id`.
const LIST: &str = "\
    select id, source_event_id, event_type, reason, recipient_type, \
           recipient_key, recipient_email, customer_id, order_id, subject, \
           body, status, created_at, sent_at, failed_at, error \
    from sent_email \
    where ($1::text is null or customer_id = $1) \
      and ($2::text is null or order_id = $2) \
    order by created_at desc, id";

impl SentEmails for PgSentEmails {
    async fn list(&self, filter: &EmailFilter) -> Result<Vec<SentEmail>, StoreError> {
        let rows = sqlx::query(LIST)
            .bind(filter.customer_id.as_deref())
            .bind(filter.order_id.as_deref())
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(sent_email).collect()
    }

    async fn ping(&self, within: Duration) -> bool {
        service_basics::postgres::ping(&self.pool, within)
            .await
            .is_ok()
    }
}

/// One selected row as a [`SentEmail`].
fn sent_email(row: &PgRow) -> Result<SentEmail, StoreError> {
    let recipient_type: String = row.try_get("recipient_type")?;
    let status: String = row.try_get("status")?;
    Ok(SentEmail {
        id: row.try_get("id")?,
        source_event_id: row.try_get("source_event_id")?,
        event_type: row.try_get("event_type")?,
        reason: row.try_get("reason")?,
        recipient_type: known("recipient_type", recipient_type, RecipientType::parse)?,
        recipient_key: row.try_get("recipient_key")?,
        recipient_email: row.try_get("recipient_email")?,
        customer_id: row.try_get("customer_id")?,
        order_id: row.try_get("order_id")?,
        subject: row.try_get("subject")?,
        body: row.try_get("body")?,
        status: known("status", status, EmailStatus::parse)?,
        created_at: row.try_get("created_at")?,
        sent_at: row.try_get("sent_at")?,
        failed_at: row.try_get("failed_at")?,
        error: row.try_get("error")?,
    })
}

/// A text column parsed into the service's own type, or why not.
fn known<T>(
    column: &'static str,
    value: String,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, StoreError> {
    parse(&value).ok_or(StoreError::UnknownValue { column, value })
}
