// SPDX-License-Identifier: MIT

//! The user-events route (Scala `UserRegisteredConsumer.handle`, less the
//! stream): which envelopes it acts on, the welcome email it asks for, and
//! the log line each send's outcome gets (`EmailSendLog`). The rest of the
//! per-record decision is gluon-rs `kafka-event-consumer`'s; the whole
//! consumer against a broker is tests/contract's user-events group.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::future::Future;
use std::sync::{Arc, Mutex};

use kafka_event_consumer::envelope::{Envelope, Routes};
use notification_service::domain::{CustomerId, EmailAddress, EmailMessage, EventId, RecipientKey};
use notification_service::email::{EmailClient, SendError, SendOutcome, TransportError};
use notification_service::user_events::{UserEvents, UserRegistered};
use serde_json::Value;
use support::{LogBuffer, captured_logger, log_lines};

const USER: &str = "11111111-1111-4111-8111-111111111111";
const ADA: &str = "ada@example.com";
const EVENT: &str = "e0000000-0000-4000-8000-000000000001";

/// An envelope as the capture publishes it (`CAPTURE.sh`'s `env_`).
fn envelope(event_type: &str, payload: &str) -> Envelope {
    serde_json::from_str(&format!(
        r#"{{"eventId":"{EVENT}","eventType":"{event_type}","timestamp":"2026-11-10T12:00:00Z","source":"capture","correlationId":"{USER}","payload":{payload}}}"#
    ))
    .unwrap()
}

fn registered() -> Envelope {
    envelope(
        "UserRegistered",
        &format!(r#"{{"userId":"{USER}","email":"{ADA}"}}"#),
    )
}

/// A client that records each message and answers with `answer`.
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

/// The route over `client`, and what it logs.
fn route(client: &Fake) -> (UserEvents<Fake>, LogBuffer) {
    let (logger, buffer) = captured_logger();
    (UserEvents::new(client.clone(), logger), buffer)
}

/// Handles the capture's welcome event through `client`; its log lines.
async fn handle_welcome(client: &Fake) -> (Result<(), String>, Vec<Value>) {
    let (route, buffer) = route(client);
    let event = route.decode(&registered()).unwrap().unwrap();
    let result = route.handle(EVENT, &event).await;
    (result, log_lines(&buffer))
}

#[test]
fn only_user_registered_is_routed() {
    let (route, _) = route(&Fake::answering(Ok(SendOutcome::Sent)));

    let deleted = envelope("UserDeleted", &format!(r#"{{"userId":"{USER}"}}"#));

    assert!(route.decode(&deleted).is_none(), "UserDeleted has no route");
    let decoded = route.decode(&registered()).unwrap().unwrap();
    assert_eq!(
        decoded,
        UserRegistered {
            user_id: CustomerId::new(USER),
            email: EmailAddress::new(ADA)
        }
    );
}

#[test]
fn a_user_registered_payload_without_an_email_is_poison() {
    let (route, _) = route(&Fake::answering(Ok(SendOutcome::Sent)));

    let without_email = envelope("UserRegistered", &format!(r#"{{"userId":"{USER}"}}"#));

    let decoded = route.decode(&without_email).unwrap();
    assert!(decoded.is_err(), "{decoded:?}");
}

#[tokio::test]
async fn a_user_registered_event_sends_the_welcome_email_to_its_address() {
    let client = Fake::answering(Ok(SendOutcome::Sent));

    let (result, _) = handle_welcome(&client).await;

    assert_eq!(result, Ok(()));
    let sent = client.sent.lock().unwrap().clone();
    assert_eq!(
        sent,
        [EmailMessage {
            source_event_id: EventId::new(EVENT),
            event_type: "UserRegistered".to_owned(),
            reason: None,
            recipient: RecipientKey::Email(EmailAddress::new(ADA)),
            customer_id: Some(CustomerId::new(USER)),
            order_id: None,
            subject: "Welcome to gshop".to_owned(),
            body: "Welcome, ada@example.com. Your gshop account is ready.".to_owned(),
            recipient_email: Some(EmailAddress::new(ADA)),
        }]
    );
}

#[tokio::test]
async fn a_sent_welcome_is_logged_as_sent() {
    let (_, lines) = handle_welcome(&Fake::answering(Ok(SendOutcome::Sent))).await;

    let [line] = lines.as_slice() else {
        panic!("one line, got {lines:?}")
    };
    assert_eq!(line["message"], "Sent UserRegistered email");
    assert_eq!(line["level"], "INFO");
    assert_eq!(line["outcome"], "sent");
    assert_eq!(line["user_id"], USER);
    assert_eq!(line["event_type"], "UserRegistered");
}

#[tokio::test]
async fn a_duplicate_welcome_is_logged_as_a_duplicate() {
    let (result, lines) = handle_welcome(&Fake::answering(Ok(SendOutcome::AlreadySent))).await;

    assert_eq!(result, Ok(()));
    let [line] = lines.as_slice() else {
        panic!("one line, got {lines:?}")
    };
    assert_eq!(
        line["message"],
        "UserRegistered email already sent or unresolved - skipped"
    );
    assert_eq!(line["level"], "INFO");
    assert_eq!(line["outcome"], "duplicate");
}

#[tokio::test]
async fn an_unresolved_welcome_is_logged_as_a_warning_with_its_reason() {
    let outcome = SendOutcome::Unresolved("no_address".to_owned());

    let (result, lines) = handle_welcome(&Fake::answering(Ok(outcome))).await;

    assert_eq!(result, Ok(()));
    let [line] = lines.as_slice() else {
        panic!("one line, got {lines:?}")
    };
    assert_eq!(
        line["message"],
        "UserRegistered email not sent - recipient unresolved (no_address)"
    );
    assert_eq!(line["level"], "WARN");
    assert_eq!(line["outcome"], "unresolved");
    assert_eq!(line["reason"], "no_address");
}

#[tokio::test]
async fn a_failed_send_is_raised_for_the_dlq_guard() {
    let (result, lines) = handle_welcome(&Fake::answering(Err("db down".to_owned()))).await;

    assert_eq!(result, Err("db down".to_owned()));
    assert!(lines.is_empty(), "no outcome logged: {lines:?}");
}
