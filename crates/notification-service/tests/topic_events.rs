// SPDX-License-Identifier: MIT

//! The order-, payment- and fulfilment-events routes (Scala
//! `OrderEventsConsumer`, `PaymentEventConsumer`, `FulfilmentEventConsumer`,
//! less the stream): which envelopes each acts on, the email each asks for
//! (the capture's copy, `db/sent-email.jsonl`), and the log line of each
//! send's outcome. The whole consumers against a broker are tests/contract's
//! topic groups.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::future::Future;
use std::sync::{Arc, Mutex};

use kafka_event_consumer::envelope::{Envelope, Routes};
use notification_service::domain::{CustomerId, EmailMessage, EventId, OrderId, RecipientKey};
use notification_service::email::{EmailClient, SendError, SendOutcome, TransportError};
use notification_service::fulfilment_events::{FulfilmentEvents, FulfilmentEventsError};
use notification_service::order_events::{OrderEvents, OrderEventsError, cancelled_body};
use notification_service::payment_events::{PaymentEvents, PaymentEventsError, format_amount};
use serde_json::Value;
use support::{captured_logger, log_lines};

const ORDER: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER: &str = "11111111-1111-4111-8111-111111111111";
const EVENT: &str = "e0000000-0000-4000-8000-000000000010";
const TS: &str = r#""timestamp":"2026-11-10T12:00:00Z""#;

fn envelope(event_type: &str, payload: &str) -> Envelope {
    serde_json::from_str(&format!(
        r#"{{"eventId":"{EVENT}","eventType":"{event_type}","timestamp":"2026-11-10T12:00:00Z","source":"capture","correlationId":"{ORDER}","payload":{payload}}}"#
    ))
    .unwrap()
}

fn order_payload(extra: &str) -> String {
    format!(r#"{{"orderId":"{ORDER}","customerId":"{USER}",{TS}{extra}}}"#)
}

#[derive(Clone)]
struct Fake {
    answer: Result<SendOutcome, String>,
    sent: Arc<Mutex<Vec<EmailMessage>>>,
}

impl Fake {
    fn answering(answer: Result<SendOutcome, String>) -> Self {
        Self {
            answer,
            sent: Arc::default(),
        }
    }

    fn sent(&self) -> Vec<EmailMessage> {
        self.sent.lock().unwrap().clone()
    }
}

impl EmailClient for Fake {
    fn send(
        &self,
        message: EmailMessage,
    ) -> impl Future<Output = Result<SendOutcome, SendError>> + Send {
        self.sent.lock().unwrap().push(message);
        std::future::ready(
            self.answer
                .clone()
                .map_err(|error| SendError::Transport(TransportError(error))),
        )
    }
}

fn sent() -> Fake {
    Fake::answering(Ok(SendOutcome::Sent))
}

/// Decodes and handles `envelope` with `route`; the message sent and the log.
async fn through<R: Routes>(route: &R, envelope: &Envelope) -> Result<(), R::Error> {
    let event = route.decode(envelope).unwrap()?;
    route.handle(EVENT, &event).await
}

fn by_customer(event_type: &str, reason: Option<&str>, subject: &str, body: &str) -> EmailMessage {
    EmailMessage {
        source_event_id: EventId::new(EVENT),
        event_type: event_type.to_owned(),
        reason: reason.map(str::to_owned),
        recipient: RecipientKey::Customer(CustomerId::new(USER)),
        customer_id: Some(CustomerId::new(USER)),
        order_id: Some(OrderId::new(ORDER)),
        subject: subject.to_owned(),
        body: body.to_owned(),
        recipient_email: None,
    }
}

fn by_order(event_type: &str, reason: Option<&str>, subject: &str, body: &str) -> EmailMessage {
    EmailMessage {
        recipient: RecipientKey::Order(OrderId::new(ORDER)),
        customer_id: None,
        ..by_customer(event_type, reason, subject, body)
    }
}

#[tokio::test]
async fn order_events_email_the_customer_in_the_captures_copy() {
    let client = sent();
    let (logger, buffer) = captured_logger();
    let route = OrderEvents::new(client.clone(), logger);
    through(&route, &envelope("OrderConfirmed", &order_payload("")))
        .await
        .unwrap();
    through(
        &route,
        &envelope(
            "OrderPaymentFailed",
            &order_payload(r#","reason":"capture_failed""#),
        ),
    )
    .await
    .unwrap();
    through(
        &route,
        &envelope(
            "OrderCancelled",
            &order_payload(
                r#","reason":"operator","actor":{"type":"operator","id":"dev-operator"}"#,
            ),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        client.sent(),
        [
            by_customer(
                "OrderConfirmed",
                None,
                "Your order is confirmed",
                &format!("Good news - order {ORDER} is confirmed.")
            ),
            by_customer(
                "OrderPaymentFailed",
                Some("capture_failed"),
                "We couldn't process your payment",
                &format!("We couldn't process your payment for order {ORDER}.")
            ),
            by_customer(
                "OrderCancelled",
                Some("operator"),
                "Your order was cancelled",
                &format!("Order {ORDER} was cancelled by our support team.")
            ),
        ]
    );
    let lines: Vec<Value> = log_lines(&buffer);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["message"], "Sent OrderConfirmed email");
    assert_eq!(lines[0]["order_id"], ORDER);
    assert_eq!(lines[2]["actor_type"], "operator");
    assert_eq!(lines[2]["actor_id"], "dev-operator");
}

#[test]
fn the_cancellation_copy_says_why() {
    let order = OrderId::new("o1");
    let cases = [
        (Some("customer"), "Order o1 was cancelled at your request."),
        (
            Some("auth_expired"),
            "Order o1 was cancelled because its payment authorization expired.",
        ),
        (
            Some("stale"),
            "Order o1 was cancelled because it could not be completed in time.",
        ),
        (
            Some("operator"),
            "Order o1 was cancelled by our support team.",
        ),
        (Some("meteor"), "Order o1 was cancelled."),
        (None, "Order o1 was cancelled."),
    ];
    for (reason, body) in cases {
        assert_eq!(cancelled_body(&order, reason), body, "{reason:?}");
    }
}

#[tokio::test]
async fn order_events_skip_other_types_and_reject_bad_payloads() {
    let client = Fake::answering(Err("down".to_owned()));
    let route = OrderEvents::new(client, captured_logger().0);
    assert!(
        route
            .decode(&envelope("OrderCreated", &order_payload("")))
            .is_none()
    );
    let poison = envelope("OrderConfirmed", &format!(r#"{{"orderId":"{ORDER}"}}"#));
    assert!(matches!(
        route.decode(&poison),
        Some(Err(OrderEventsError::Payload(_)))
    ));
    let bad_time = envelope(
        "OrderConfirmed",
        &format!(r#"{{"orderId":"{ORDER}","customerId":"{USER}","timestamp":"yesterday"}}"#),
    );
    assert!(matches!(route.decode(&bad_time), Some(Err(_))));
    let result = through(&route, &envelope("OrderCancelled", &order_payload(""))).await;
    assert!(
        matches!(result, Err(OrderEventsError::Send(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn payment_events_email_the_order() {
    let client = sent();
    let route = PaymentEvents::new(client.clone(), captured_logger().0);
    let captured = format!(r#"{{"orderId":"{ORDER}","amountCents":4999,{TS}}}"#);
    let failed = format!(
        r#"{{"orderId":"{ORDER}","paymentId":"p1","amountCents":4999,"reason":"capture_failed",{TS}}}"#
    );
    through(&route, &envelope("PaymentCaptured", &captured))
        .await
        .unwrap();
    through(&route, &envelope("PaymentFailed", &failed))
        .await
        .unwrap();
    assert_eq!(
        client.sent(),
        [
            by_order(
                "PaymentCaptured",
                None,
                "Your payment was received",
                &format!("We received your payment of $49.99 for order {ORDER}.")
            ),
            by_order(
                "PaymentFailed",
                Some("capture_failed"),
                "We couldn't take your payment",
                &format!("We couldn't take payment of $49.99 for order {ORDER}: capture_failed.")
            ),
        ]
    );
    assert!(
        route
            .decode(&envelope("PaymentAuthorized", &captured))
            .is_none()
    );
    assert!(matches!(
        route.decode(&envelope("PaymentFailed", &captured)),
        Some(Err(PaymentEventsError::Payload(_)))
    ));
    let down = PaymentEvents::new(Fake::answering(Err("down".to_owned())), captured_logger().0);
    let result = through(&down, &envelope("PaymentCaptured", &captured)).await;
    assert!(matches!(result, Err(PaymentEventsError::Send(_))));
}

#[test]
fn amounts_are_dollars_and_cents() {
    assert_eq!(format_amount(4999), "$49.99");
    assert_eq!(format_amount(100), "$1.00");
    assert_eq!(format_amount(5), "$0.05");
    assert_eq!(format_amount(-5), "-$0.05");
    assert_eq!(format_amount(i32::MIN), "-$21474836.48");
}

#[tokio::test]
async fn fulfilment_events_email_the_order() {
    let client = sent();
    let route = FulfilmentEvents::new(client.clone(), captured_logger().0);
    let items =
        r#"[{"sku":"SKU-0042","productName":"Espresso cup","quantity":2,"unitPriceCents":1250}]"#;
    let confirmed = format!(r#"{{"orderId":"{ORDER}","trackingNo":"TRK-1","items":{items}}}"#);
    let failed = format!(r#"{{"orderId":"{ORDER}","reason":"lost"}}"#);
    through(&route, &envelope("DispatchConfirmed", &confirmed))
        .await
        .unwrap();
    through(&route, &envelope("DispatchFailed", &failed))
        .await
        .unwrap();
    assert_eq!(
        client.sent(),
        [
            by_order(
                "DispatchConfirmed",
                None,
                "Your order has shipped",
                &format!("Your order {ORDER} has shipped. Tracking number: TRK-1.")
            ),
            by_order(
                "DispatchFailed",
                Some("lost"),
                "We couldn't ship your order",
                &format!("Order {ORDER} could not be dispatched: lost.")
            ),
        ]
    );
    let no_items = format!(r#"{{"orderId":"{ORDER}","trackingNo":"TRK-1"}}"#);
    assert!(matches!(
        route.decode(&envelope("DispatchConfirmed", &no_items)),
        Some(Err(FulfilmentEventsError::Payload(_)))
    ));
    assert!(
        route
            .decode(&envelope("DispatchCreated", &failed))
            .is_none()
    );
    let down = FulfilmentEvents::new(Fake::answering(Err("down".to_owned())), captured_logger().0);
    let result = through(&down, &envelope("DispatchFailed", &failed)).await;
    assert!(matches!(result, Err(FulfilmentEventsError::Send(_))));
}
