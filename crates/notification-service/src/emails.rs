// SPDX-License-Identifier: MIT

//! `GET /emails?customerId=&orderId=` (ADM-2): the sent-email audit history,
//! newest first (Scala `EmailRoutes`; capture `http/emails-*.txt`).

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use service_basics::jvm::java_instant;
use service_basics::text_error::TextError;

use crate::domain::{QueryError, SentEmail, email_filter};
use crate::server::AppState;
use crate::store::{SentEmails, StoreError};

/// One recorded email send attempt: Scala `SentEmailResponse`, its 16 fields
/// in its order, absent values as `null` (circe's default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentEmailResponse {
    /// Audit row id (UUID).
    pub id: String,
    /// Envelope eventId that triggered the email; the idempotency key.
    pub source_event_id: String,
    /// Event type, e.g. `OrderConfirmed` or `PaymentCaptured`.
    pub event_type: String,
    /// Cancellation or failure reason carried by the event, if any.
    pub reason: Option<String>,
    /// Recipient discriminator: customer, email or order.
    pub recipient_type: String,
    /// Event identity: customerId, email address or orderId.
    pub recipient_key: String,
    /// Resolved destination email address, when known (ADM-3).
    pub recipient_email: Option<String>,
    /// Customer the email is attributed to, when known.
    pub customer_id: Option<String>,
    /// Order the email concerns, when the event carries one.
    pub order_id: Option<String>,
    /// Email subject.
    pub subject: String,
    /// Email body.
    pub body: String,
    /// Send state: pending, sent, failed or unresolved.
    pub status: String,
    /// When the send was attempted.
    pub created_at: String,
    /// When the send succeeded, if it did.
    pub sent_at: Option<String>,
    /// When the send failed, if it did.
    pub failed_at: Option<String>,
    /// Failure detail, if the send failed.
    pub error: Option<String>,
}

impl From<SentEmail> for SentEmailResponse {
    fn from(row: SentEmail) -> Self {
        Self {
            id: row.id.to_string(),
            source_event_id: row.source_event_id,
            event_type: row.event_type,
            reason: row.reason,
            recipient_type: row.recipient_type.as_str().to_owned(),
            recipient_key: row.recipient_key,
            recipient_email: row.recipient_email,
            customer_id: row.customer_id,
            order_id: row.order_id,
            subject: row.subject,
            body: row.body,
            status: row.status.as_str().to_owned(),
            created_at: java_instant(row.created_at),
            sent_at: row.sent_at.map(java_instant),
            failed_at: row.failed_at.map(java_instant),
            error: row.error,
        }
    }
}

/// Why `GET /emails` didn't answer 200.
#[derive(Debug, thiserror::Error)]
pub enum EmailsError {
    /// A filter tapir can't decode.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// The store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl IntoResponse for EmailsError {
    fn into_response(self) -> Response {
        match self {
            // tapir's decode failure: 400, its message as plain text
            // (capture `http/emails-repeated-param.txt`).
            Self::Query(error) => TextError {
                status: StatusCode::BAD_REQUEST,
                message: error.to_string(),
            }
            .into_response(),
            // `serverLogicSuccess` fails the effect: tapir's
            // `DefaultExceptionHandler` answers 500 "Internal server error".
            Self::Store(error) => {
                eprintln!("notification-service: GET /emails failed: {error}");
                TextError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    message: "Internal server error".to_owned(),
                }
                .into_response()
            }
        }
    }
}

/// `GET /emails` (and `/emails/`, capture `http/emails-trailing-slash.txt`).
///
/// # Errors
/// [`EmailsError`]: a repeated filter (400) or a store failure (500).
pub async fn list_emails<S: SentEmails>(
    State(state): State<AppState<S>>,
    RawQuery(query): RawQuery,
) -> Result<Json<Vec<SentEmailResponse>>, EmailsError> {
    let filter = email_filter(query.as_deref())?;
    let rows = state.store.list(&filter).await?;
    Ok(Json(
        rows.into_iter().map(SentEmailResponse::from).collect(),
    ))
}
