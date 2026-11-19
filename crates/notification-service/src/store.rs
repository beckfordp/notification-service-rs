// SPDX-License-Identifier: MIT

//! The `sent_email` table, as the Scala service reads and writes it
//! (`SentEmailStore`). Handlers reach it through the [`SentEmails`] trait, so
//! they are testable with an in-memory store; the audited send through
//! [`EmailAudit`]; [`PgSentEmails`] is the Postgres one.

use std::future::Future;
use std::time::Duration;

use sqlx::PgPool;
use time::OffsetDateTime;

use uuid::Uuid;

use crate::domain::{
    CustomerId, EmailAddress, EmailFilter, EmailMessage, OrderId, SentEmail, UnknownValue,
};

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

/// What claiming an event for sending found (Scala `EmailClaim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    /// Send it, then mark this row.
    Send(Uuid),
    /// Already `sent` or `unresolved`: not again.
    Duplicate,
}

/// What the audited send needs from `sent_email` (Scala `SentEmailStore`'s
/// writes).
pub trait EmailAudit: Clone + Send + Sync + 'static {
    /// Claims `message`'s event atomically: a new row `pending` with `id`; an
    /// existing `pending` or `failed` row reset to `pending` and reused (a
    /// redelivery finishes an interrupted send); an existing `sent` or
    /// `unresolved` row is [`Claim::Duplicate`], and nothing is written.
    fn claim(
        &self,
        id: Uuid,
        message: &EmailMessage,
    ) -> impl Future<Output = Result<Claim, StoreError>> + Send;

    /// Records the resolved identity before the send (ADM-3), so the
    /// attribution survives a failed send; `None` leaves a column as it is.
    fn mark_resolved(
        &self,
        id: Uuid,
        customer_id: Option<&CustomerId>,
        recipient_email: Option<&EmailAddress>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// `sent`, now.
    fn mark_sent(&self, id: Uuid) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// `failed`, now, with why.
    fn mark_failed(
        &self,
        id: Uuid,
        error: &str,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// `unresolved` (terminal: no address exists), with why.
    fn mark_unresolved(
        &self,
        id: Uuid,
        reason: &str,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
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
        let rows: Vec<Row> = sqlx::query_as(LIST)
            .bind(filter.customer_id.as_deref())
            .bind(filter.order_id.as_deref())
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(SentEmail::try_from).collect()
    }

    async fn ping(&self, within: Duration) -> bool {
        service_basics::postgres::ping(&self.pool, within)
            .await
            .is_ok()
    }
}

/// Scala `SentEmailStore.claimEmail`: no row is returned when the existing
/// one is `sent` or `unresolved`.
const CLAIM: &str = "\
    insert into sent_email \
      (id, source_event_id, event_type, reason, recipient_type, \
       recipient_key, recipient_email, customer_id, order_id, subject, \
       body, status) \
    values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'pending') \
    on conflict (source_event_id) do update \
      set status = 'pending', \
          sent_at = null, \
          failed_at = null, \
          error = null \
      where sent_email.status not in ('sent', 'unresolved') \
    returning id";

const MARK_RESOLVED: &str = "\
    update sent_email \
    set customer_id = coalesce($1, customer_id), \
        recipient_email = coalesce($2, recipient_email) \
    where id = $3";

const MARK_SENT: &str = "\
    update sent_email \
    set status = 'sent', sent_at = now(), failed_at = null, error = null \
    where id = $1";

const MARK_FAILED: &str = "\
    update sent_email \
    set status = 'failed', failed_at = now(), error = $1 \
    where id = $2";

const MARK_UNRESOLVED: &str = "\
    update sent_email \
    set status = 'unresolved', error = $1, failed_at = null \
    where id = $2";

impl EmailAudit for PgSentEmails {
    async fn claim(&self, id: Uuid, message: &EmailMessage) -> Result<Claim, StoreError> {
        let claimed: Option<Uuid> = sqlx::query_scalar(CLAIM)
            .bind(id)
            .bind(message.source_event_id.as_str())
            .bind(&message.event_type)
            .bind(message.reason.as_deref())
            .bind(message.recipient.recipient_type().as_str())
            .bind(message.recipient.as_str())
            .bind(message.recipient_email.as_ref().map(EmailAddress::as_str))
            .bind(message.customer_id.as_ref().map(CustomerId::as_str))
            .bind(message.order_id.as_ref().map(OrderId::as_str))
            .bind(&message.subject)
            .bind(&message.body)
            .fetch_optional(&self.pool)
            .await?;
        Ok(claimed.map_or(Claim::Duplicate, Claim::Send))
    }

    async fn mark_resolved(
        &self,
        id: Uuid,
        customer_id: Option<&CustomerId>,
        recipient_email: Option<&EmailAddress>,
    ) -> Result<(), StoreError> {
        sqlx::query(MARK_RESOLVED)
            .bind(customer_id.map(CustomerId::as_str))
            .bind(recipient_email.map(EmailAddress::as_str))
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_sent(&self, id: Uuid) -> Result<(), StoreError> {
        sqlx::query(MARK_SENT).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn mark_failed(&self, id: Uuid, error: &str) -> Result<(), StoreError> {
        sqlx::query(MARK_FAILED)
            .bind(error)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_unresolved(&self, id: Uuid, reason: &str) -> Result<(), StoreError> {
        sqlx::query(MARK_UNRESOLVED)
            .bind(reason)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// A selected `sent_email` row as Postgres gives it: the enum columns as
/// text, turned into the domain's types by `TryFrom`.
#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    source_event_id: String,
    event_type: String,
    reason: Option<String>,
    recipient_type: String,
    recipient_key: String,
    recipient_email: Option<String>,
    customer_id: Option<String>,
    order_id: Option<String>,
    subject: String,
    body: String,
    status: String,
    created_at: OffsetDateTime,
    sent_at: Option<OffsetDateTime>,
    failed_at: Option<OffsetDateTime>,
    error: Option<String>,
}

impl TryFrom<Row> for SentEmail {
    type Error = StoreError;

    /// A stored value the service doesn't know is an error naming its column
    /// (the Scala codecs' `RecipientType.fromString` / `EmailStatus.fromString`).
    fn try_from(row: Row) -> Result<Self, StoreError> {
        let unknown =
            |column| move |UnknownValue(value)| StoreError::UnknownValue { column, value };
        Ok(Self {
            id: row.id,
            source_event_id: row.source_event_id,
            event_type: row.event_type,
            reason: row.reason,
            recipient_type: row
                .recipient_type
                .parse()
                .map_err(unknown("recipient_type"))?,
            recipient_key: row.recipient_key,
            recipient_email: row.recipient_email,
            customer_id: row.customer_id,
            order_id: row.order_id,
            subject: row.subject,
            body: row.body,
            status: row.status.parse().map_err(unknown("status"))?,
            created_at: row.created_at,
            sent_at: row.sent_at,
            failed_at: row.failed_at,
            error: row.error,
        })
    }
}
