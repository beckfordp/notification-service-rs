// SPDX-License-Identifier: MIT

//! The fulfilment-events consumer's route (Scala `FulfilmentEventConsumer`,
//! US-27.4, US-18.4): `DispatchConfirmed` and `DispatchFailed` email whoever
//! the order belongs to, found through order-service; every other event type
//! is skipped. The rest of the per-record decision is gluon-rs
//! `kafka-event-consumer`'s.

use kafka_event_consumer::envelope::{Envelope, Routes};
use schemars::JsonSchema;
use serde::Deserialize;
use telemetry::logging::Logger;

use crate::domain::{EmailMessage, EventId, OrderId, RecipientKey};
use crate::email::{EmailClient, SendError, log_outcome};

/// The topic.
pub const TOPIC: &str = "fulfilment-events";

/// The consumer group (capture `kafka/consumer-groups.txt`).
pub const GROUP: &str = "notification-service-fulfilment-events";

/// The order was dispatched.
pub const DISPATCH_CONFIRMED: &str = "DispatchConfirmed";
/// It couldn't be.
pub const DISPATCH_FAILED: &str = "DispatchFailed";

/// One dispatched line (Scala `DispatchItem`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(inline)]
pub struct DispatchItem {
    /// The product's SKU.
    pub sku: String,
    /// Its name, as ordered.
    pub product_name: String,
    /// How many.
    pub quantity: i32,
    /// The unit price, in cents.
    pub unit_price_cents: i32,
}

/// `DispatchConfirmed`'s payload (Scala `DispatchConfirmedEvent`): `items`
/// required to decode (circe's), not in the schema (tapir lists a `List` as
/// optional).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "DispatchConfirmedEvent",
    description = "Dispatch confirmed: fulfilment-service shipped the order."
)]
pub struct DispatchConfirmed {
    /// The order shipped.
    pub order_id: OrderId,
    /// The carrier's tracking number.
    pub tracking_no: String,
    /// What shipped.
    #[schemars(default)]
    pub items: Vec<DispatchItem>,
}

/// `DispatchFailed`'s payload (Scala `DispatchFailedEvent`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "DispatchFailedEvent",
    description = "Dispatch failed: fulfilment-service couldn't ship the order."
)]
pub struct DispatchFailed {
    /// The order.
    pub order_id: OrderId,
    /// Why it couldn't ship.
    pub reason: String,
}

/// The events this route handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FulfilmentEvent {
    /// `DispatchConfirmed`.
    Confirmed(DispatchConfirmed),
    /// `DispatchFailed`.
    Failed(DispatchFailed),
}

impl FulfilmentEvent {
    /// Its type, as the envelope names it.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Confirmed(_) => DISPATCH_CONFIRMED,
            Self::Failed(_) => DISPATCH_FAILED,
        }
    }

    fn order(&self) -> &OrderId {
        match self {
            Self::Confirmed(e) => &e.order_id,
            Self::Failed(e) => &e.order_id,
        }
    }

    /// Its email (Scala `sendEmail`): to the order, its customer looked up.
    #[must_use]
    pub fn message(&self, event_id: EventId) -> EmailMessage {
        let order = self.order();
        let (reason, subject, body) = match self {
            Self::Confirmed(e) => (
                None,
                "Your order has shipped",
                format!(
                    "Your order {order} has shipped. Tracking number: {}.",
                    e.tracking_no
                ),
            ),
            Self::Failed(e) => (
                Some(e.reason.clone()),
                "We couldn't ship your order",
                format!("Order {order} could not be dispatched: {}.", e.reason),
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

/// Why a fulfilment event wasn't handled: logged, retried and dead-lettered
/// by the DLQ guard.
#[derive(Debug, thiserror::Error)]
pub enum FulfilmentEventsError {
    /// The payload isn't the event its type names (poison).
    #[error(transparent)]
    Payload(#[from] serde_json::Error),
    /// The email's send failed.
    #[error(transparent)]
    Send(#[from] SendError),
}

/// The route: each fulfilment event → its email, through `client`.
#[derive(Debug, Clone)]
pub struct FulfilmentEvents<C> {
    client: C,
    logger: Logger,
}

impl<C> FulfilmentEvents<C> {
    /// Sends through `client`, logging each outcome to `logger`.
    #[must_use]
    pub fn new(client: C, logger: Logger) -> Self {
        Self { client, logger }
    }
}

fn payload<T: for<'de> Deserialize<'de>>(
    envelope: &Envelope,
    event: fn(T) -> FulfilmentEvent,
) -> Result<FulfilmentEvent, FulfilmentEventsError> {
    Ok(event(serde_json::from_value(envelope.payload.clone())?))
}

impl<C: EmailClient> Routes for FulfilmentEvents<C> {
    type Event = FulfilmentEvent;
    type Error = FulfilmentEventsError;

    fn decode(
        &self,
        envelope: &Envelope,
    ) -> Option<Result<FulfilmentEvent, FulfilmentEventsError>> {
        match envelope.event_type.as_str() {
            DISPATCH_CONFIRMED => Some(payload(envelope, FulfilmentEvent::Confirmed)),
            DISPATCH_FAILED => Some(payload(envelope, FulfilmentEvent::Failed)),
            _ => None,
        }
    }

    /// The event's email; a failed send is raised for the DLQ guard.
    async fn handle(
        &self,
        event_id: &str,
        event: &FulfilmentEvent,
    ) -> Result<(), FulfilmentEventsError> {
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
