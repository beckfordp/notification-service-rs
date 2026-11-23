// SPDX-License-Identifier: MIT

//! The order-events consumer's route (Scala `OrderEventsConsumer`, US-7.1,
//! US-19.4, US-9.9, US-20.5): `OrderConfirmed`, `OrderPaymentFailed` and
//! `OrderCancelled` email the order's customer, through the audited send;
//! every other event type is skipped. The rest of the per-record decision is
//! gluon-rs `kafka-event-consumer`'s, as it is `EnvelopeHandler`'s in Scala.

use kafka_event_consumer::envelope::{Envelope, Routes};
use schemars::JsonSchema;
use serde::Deserialize;
use telemetry::logging::Logger;
use time::OffsetDateTime;

use crate::domain::{CustomerId, EmailMessage, EventId, OrderId, RecipientKey};
use crate::email::{EmailClient, SendError, log_outcome};

/// The topic.
pub const TOPIC: &str = "order-events";

/// The consumer group (capture `kafka/consumer-groups.txt`).
pub const GROUP: &str = "notification-service-order-events";

/// The order is confirmed.
pub const ORDER_CONFIRMED: &str = "OrderConfirmed";
/// Its payment couldn't be captured.
pub const ORDER_PAYMENT_FAILED: &str = "OrderPaymentFailed";
/// It was cancelled.
pub const ORDER_CANCELLED: &str = "OrderCancelled";

/// `OrderConfirmed`'s payload (Scala `OrderConfirmedEvent`): every field
/// required, as circe's derived decoder does.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "OrderConfirmedEvent",
    description = "Order confirmed: order-service consumed PaymentCaptured and moved the order to confirmed."
)]
pub struct OrderConfirmed {
    /// Order UUID (order-service's order id).
    pub order_id: OrderId,
    /// Customer who placed the order.
    pub customer_id: CustomerId,
    /// When the order was confirmed (ISO-8601 instant).
    #[serde(with = "time::serde::rfc3339")]
    #[schemars(with = "String")]
    pub timestamp: OffsetDateTime,
}

/// `OrderPaymentFailed`'s payload (Scala `OrderPaymentFailedEvent`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "OrderPaymentFailedEvent",
    description = "Payment capture failed: order-service consumed PaymentFailed and moved the order to payment_failed."
)]
pub struct OrderPaymentFailed {
    /// Order UUID (order-service's order id).
    pub order_id: OrderId,
    /// Customer who placed the order.
    pub customer_id: CustomerId,
    /// payment-service's capture-failure reason.
    pub reason: String,
    /// When the order moved to `payment_failed` (ISO-8601 instant).
    #[serde(with = "time::serde::rfc3339")]
    #[schemars(with = "String")]
    pub timestamp: OffsetDateTime,
}

/// Who cancelled (Scala `OrderActor`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[schemars(
    inline,
    description = "Cancellation actor: customer, operator or system."
)]
pub struct OrderActor {
    /// Actor type: customer | operator | system.
    #[serde(rename = "type")]
    pub kind: String,
    /// Actor id: the customerId, operator id, or system component.
    pub id: String,
}

/// `OrderCancelled`'s payload (Scala `OrderCancelledEvent`): `reason` and
/// `actor` optional (absent on older messages).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "OrderCancelledEvent",
    description = "Order cancelled: a customer/operator cancel or a consumed PaymentAuthorizationExpired moved the order to cancelled."
)]
pub struct OrderCancelled {
    /// Order UUID (order-service's order id).
    pub order_id: OrderId,
    /// Customer who placed the order.
    pub customer_id: CustomerId,
    /// When the order was cancelled (ISO-8601 instant).
    #[serde(with = "time::serde::rfc3339")]
    #[schemars(with = "String")]
    pub timestamp: OffsetDateTime,
    /// Cancellation trigger: customer | `auth_expired` | stale | operator.
    #[serde(default)]
    pub reason: Option<String>,
    /// Who cancelled; absent on pre-v3 messages.
    #[serde(default)]
    pub actor: Option<OrderActor>,
}

/// The events this route handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderEvent {
    /// `OrderConfirmed`.
    Confirmed(OrderConfirmed),
    /// `OrderPaymentFailed`.
    PaymentFailed(OrderPaymentFailed),
    /// `OrderCancelled`.
    Cancelled(OrderCancelled),
}

/// The cancellation copy for `reason` (Scala `cancelledEmail`): an unknown
/// reason or none, the plain sentence.
#[must_use]
pub fn cancelled_body(order: &OrderId, reason: Option<&str>) -> String {
    match reason {
        Some("customer") => format!("Order {order} was cancelled at your request."),
        Some("auth_expired") => {
            format!("Order {order} was cancelled because its payment authorization expired.")
        }
        Some("stale") => {
            format!("Order {order} was cancelled because it could not be completed in time.")
        }
        Some("operator") => format!("Order {order} was cancelled by our support team."),
        Some(_) | None => format!("Order {order} was cancelled."),
    }
}

impl OrderEvent {
    /// Its type, as the envelope names it.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Confirmed(_) => ORDER_CONFIRMED,
            Self::PaymentFailed(_) => ORDER_PAYMENT_FAILED,
            Self::Cancelled(_) => ORDER_CANCELLED,
        }
    }

    fn ids(&self) -> (&OrderId, &CustomerId) {
        match self {
            Self::Confirmed(e) => (&e.order_id, &e.customer_id),
            Self::PaymentFailed(e) => (&e.order_id, &e.customer_id),
            Self::Cancelled(e) => (&e.order_id, &e.customer_id),
        }
    }

    /// Its email (Scala `send`): to the order's customer, looked up.
    #[must_use]
    pub fn message(&self, event_id: EventId) -> EmailMessage {
        let (order, customer) = self.ids();
        let (reason, subject, body) = match self {
            Self::Confirmed(_) => (
                None,
                "Your order is confirmed",
                format!("Good news - order {order} is confirmed."),
            ),
            Self::PaymentFailed(e) => (
                Some(e.reason.clone()),
                "We couldn't process your payment",
                format!("We couldn't process your payment for order {order}."),
            ),
            Self::Cancelled(e) => (
                e.reason.clone(),
                "Your order was cancelled",
                cancelled_body(order, e.reason.as_deref()),
            ),
        };
        EmailMessage {
            source_event_id: event_id,
            event_type: self.event_type().to_owned(),
            reason,
            recipient: RecipientKey::Customer(customer.clone()),
            customer_id: Some(customer.clone()),
            order_id: Some(order.clone()),
            subject: subject.to_owned(),
            body,
            recipient_email: None,
        }
    }
}

/// Why an order event wasn't handled: logged, retried and dead-lettered by
/// the DLQ guard.
#[derive(Debug, thiserror::Error)]
pub enum OrderEventsError {
    /// The payload isn't the event its type names (poison).
    #[error(transparent)]
    Payload(#[from] serde_json::Error),
    /// The email's send failed.
    #[error(transparent)]
    Send(#[from] SendError),
}

/// The route: each order event → its email, through `client`.
#[derive(Debug, Clone)]
pub struct OrderEvents<C> {
    client: C,
    logger: Logger,
}

impl<C> OrderEvents<C> {
    /// Sends through `client`, logging each outcome to `logger`.
    #[must_use]
    pub fn new(client: C, logger: Logger) -> Self {
        Self { client, logger }
    }
}

fn payload<T: for<'de> Deserialize<'de>>(
    envelope: &Envelope,
    event: fn(T) -> OrderEvent,
) -> Result<OrderEvent, OrderEventsError> {
    Ok(event(serde_json::from_value(envelope.payload.clone())?))
}

impl<C: EmailClient> Routes for OrderEvents<C> {
    type Event = OrderEvent;
    type Error = OrderEventsError;

    fn decode(&self, envelope: &Envelope) -> Option<Result<OrderEvent, OrderEventsError>> {
        match envelope.event_type.as_str() {
            ORDER_CONFIRMED => Some(payload(envelope, OrderEvent::Confirmed)),
            ORDER_PAYMENT_FAILED => Some(payload(envelope, OrderEvent::PaymentFailed)),
            ORDER_CANCELLED => Some(payload(envelope, OrderEvent::Cancelled)),
            _ => None,
        }
    }

    /// The event's email; a failed send is raised for the DLQ guard.
    async fn handle(&self, event_id: &str, event: &OrderEvent) -> Result<(), OrderEventsError> {
        let outcome = self
            .client
            .send(event.message(EventId::new(event_id)))
            .await?;
        let (order, _) = event.ids();
        let mut context = vec![
            ("order_id", order.as_str()),
            ("event_type", event.event_type()),
        ];
        if let OrderEvent::Cancelled(OrderCancelled {
            actor: Some(actor), ..
        }) = event
        {
            context.push(("actor_type", &actor.kind));
            context.push(("actor_id", &actor.id));
        }
        log_outcome(&self.logger, &context, event.event_type(), &outcome);
        Ok(())
    }
}
