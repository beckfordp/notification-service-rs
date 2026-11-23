// SPDX-License-Identifier: MIT

//! The payment-events consumer's route (Scala `PaymentEventConsumer`,
//! US-17.5, US-19.4): `PaymentCaptured` and `PaymentFailed` email whoever the
//! order belongs to, found through order-service; every other event type is
//! skipped. The rest of the per-record decision is gluon-rs
//! `kafka-event-consumer`'s.

use kafka_event_consumer::envelope::{Envelope, Routes};
use schemars::JsonSchema;
use serde::Deserialize;
use telemetry::logging::Logger;
use time::OffsetDateTime;

use crate::domain::{EmailMessage, EventId, OrderId, RecipientKey};
use crate::email::{EmailClient, SendError, log_outcome};

/// The topic.
pub const TOPIC: &str = "payment-events";

/// The consumer group (capture `kafka/consumer-groups.txt`).
pub const GROUP: &str = "notification-service-payment-events";

/// The payment was captured.
pub const PAYMENT_CAPTURED: &str = "PaymentCaptured";
/// It couldn't be.
pub const PAYMENT_FAILED: &str = "PaymentFailed";

/// `PaymentCaptured`'s payload (Scala `PaymentCapturedEvent`): every field
/// required.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "PaymentCapturedEvent",
    description = "Payment captured: payment-service took the authorised amount."
)]
pub struct PaymentCaptured {
    /// The order paid for.
    pub order_id: OrderId,
    /// The amount, in cents.
    pub amount_cents: i32,
    /// When it was captured (ISO-8601 instant).
    #[serde(with = "time::serde::rfc3339")]
    #[schemars(with = "String")]
    pub timestamp: OffsetDateTime,
}

/// `PaymentFailed`'s payload (Scala `PaymentFailedEvent`): every field
/// required.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "PaymentFailedEvent",
    description = "Payment failed: payment-service couldn't capture the amount."
)]
pub struct PaymentFailed {
    /// The order paid for.
    pub order_id: OrderId,
    /// The payment.
    pub payment_id: String,
    /// The amount, in cents.
    pub amount_cents: i32,
    /// Why it failed.
    pub reason: String,
    /// When it failed (ISO-8601 instant).
    #[serde(with = "time::serde::rfc3339")]
    #[schemars(with = "String")]
    pub timestamp: OffsetDateTime,
}

/// The events this route handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentEvent {
    /// `PaymentCaptured`.
    Captured(PaymentCaptured),
    /// `PaymentFailed`.
    Failed(PaymentFailed),
}

/// Cents as dollars, e.g. `$49.99`, `-$0.05` (Scala `formatAmount`).
#[must_use]
pub fn format_amount(cents: i32) -> String {
    let abs = i64::from(cents).abs();
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}${}.{:02}", abs / 100, abs % 100)
}

impl PaymentEvent {
    /// Its type, as the envelope names it.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Captured(_) => PAYMENT_CAPTURED,
            Self::Failed(_) => PAYMENT_FAILED,
        }
    }

    fn order(&self) -> &OrderId {
        match self {
            Self::Captured(e) => &e.order_id,
            Self::Failed(e) => &e.order_id,
        }
    }

    /// Its email (Scala `sendEmail`): to the order, its customer looked up.
    #[must_use]
    pub fn message(&self, event_id: EventId) -> EmailMessage {
        let order = self.order();
        let (reason, subject, body) = match self {
            Self::Captured(e) => (
                None,
                "Your payment was received",
                format!(
                    "We received your payment of {} for order {order}.",
                    format_amount(e.amount_cents)
                ),
            ),
            Self::Failed(e) => (
                Some(e.reason.clone()),
                "We couldn't take your payment",
                format!(
                    "We couldn't take payment of {} for order {order}: {}.",
                    format_amount(e.amount_cents),
                    e.reason
                ),
            ),
        };
        EmailMessage {
            source_event_id: event_id,
            event_type: self.event_type().to_owned(),
            reason,
            recipient: RecipientKey::Order(order.clone()),
            customer_id: None,
            order_id: Some(order.clone()),
            subject: subject.to_owned(),
            body,
            recipient_email: None,
        }
    }
}

/// Why a payment event wasn't handled: logged, retried and dead-lettered by
/// the DLQ guard.
#[derive(Debug, thiserror::Error)]
pub enum PaymentEventsError {
    /// The payload isn't the event its type names (poison).
    #[error(transparent)]
    Payload(#[from] serde_json::Error),
    /// The email's send failed.
    #[error(transparent)]
    Send(#[from] SendError),
}

/// The route: each payment event → its email, through `client`.
#[derive(Debug, Clone)]
pub struct PaymentEvents<C> {
    client: C,
    logger: Logger,
}

impl<C> PaymentEvents<C> {
    /// Sends through `client`, logging each outcome to `logger`.
    #[must_use]
    pub fn new(client: C, logger: Logger) -> Self {
        Self { client, logger }
    }
}

fn payload<T: for<'de> Deserialize<'de>>(
    envelope: &Envelope,
    event: fn(T) -> PaymentEvent,
) -> Result<PaymentEvent, PaymentEventsError> {
    Ok(event(serde_json::from_value(envelope.payload.clone())?))
}

impl<C: EmailClient> Routes for PaymentEvents<C> {
    type Event = PaymentEvent;
    type Error = PaymentEventsError;

    fn decode(&self, envelope: &Envelope) -> Option<Result<PaymentEvent, PaymentEventsError>> {
        match envelope.event_type.as_str() {
            PAYMENT_CAPTURED => Some(payload(envelope, PaymentEvent::Captured)),
            PAYMENT_FAILED => Some(payload(envelope, PaymentEvent::Failed)),
            _ => None,
        }
    }

    /// The event's email; a failed send is raised for the DLQ guard.
    async fn handle(&self, event_id: &str, event: &PaymentEvent) -> Result<(), PaymentEventsError> {
        let outcome = self
            .client
            .send(event.message(EventId::new(event_id)))
            .await?;
        log_outcome(
            &self.logger,
            &[
                ("order_id", event.order().as_str()),
                ("event_type", event.event_type()),
            ],
            event.event_type(),
            &outcome,
        );
        Ok(())
    }
}
