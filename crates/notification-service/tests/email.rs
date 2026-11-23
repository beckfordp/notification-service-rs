// SPDX-License-Identifier: MIT

//! The pieces of `email` the audited send's nine behaviours don't pin
//! (`tests/audited_send.rs`): the logging transport's line (Scala
//! `EmailClient.logging`; capture `stdout.log`), `NoLookup`, and
//! `CarriedAddress` deferring to its fallback.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::future::Future;

use notification_service::domain::{CustomerId, EmailAddress, EmailMessage, EventId, RecipientKey};
use notification_service::email::{
    CarriedAddress, LoggingTransport, LookupError, Recipient, Resolver, Transport,
};
use serde_json::Value;
use support::{captured_logger, log_lines};

const USER: &str = "11111111-1111-4111-8111-111111111111";

fn message(recipient: RecipientKey, email: Option<&str>) -> EmailMessage {
    EmailMessage {
        source_event_id: EventId::new("e0000000-0000-4000-8000-000000000001"),
        event_type: "UserRegistered".to_owned(),
        reason: None,
        recipient,
        customer_id: None,
        order_id: None,
        subject: "Welcome to gshop".to_owned(),
        body: "Welcome, ada@example.com. Your gshop account is ready.".to_owned(),
        recipient_email: email.map(EmailAddress::new),
    }
}

/// The lines `send` logs, each as JSON.
async fn logged(message: &EmailMessage) -> Vec<Value> {
    let (logger, buffer) = captured_logger();
    LoggingTransport::new(logger).send(message).await.unwrap();
    log_lines(&buffer)
}

#[tokio::test]
async fn the_logging_transport_logs_the_email_as_the_scala_stub_does() {
    let email = message(
        RecipientKey::Email(EmailAddress::new("ada@example.com")),
        Some("ada@example.com"),
    );

    let [line] = logged(&email).await.try_into().unwrap();

    assert_eq!(
        line["message"],
        "Sending email (stub - logged only) to=ada@example.com subject=Welcome to gshop"
    );
    assert_eq!(line["level"], "INFO");
    assert_eq!(line["to"], "ada@example.com");
    assert_eq!(line["subject"], "Welcome to gshop");
    assert_eq!(line["body"], email.body);
    assert_eq!(line["event_type"], "UserRegistered");
    assert_eq!(line["source_event_id"], email.source_event_id.as_str());
}

#[tokio::test]
async fn the_logging_transport_falls_back_to_the_key_without_an_address() {
    let unresolved = message(RecipientKey::Customer(CustomerId::new(USER)), None);

    let [line] = logged(&unresolved).await.try_into().unwrap();

    assert_eq!(line["to"], "11111111-1111-4111-8111-111111111111");
}

#[tokio::test]
async fn a_message_without_an_address_goes_to_the_fallback() {
    #[derive(Clone)]
    struct Found;
    impl Resolver for Found {
        fn resolve(
            &self,
            message: &EmailMessage,
        ) -> impl Future<Output = Result<Recipient, LookupError>> + Send {
            std::future::ready(Ok(Recipient::Found {
                address: EmailAddress::new(format!(
                    "found-for-{}@example.com",
                    message.recipient.as_str()
                )),
                customer_id: None,
            }))
        }
    }
    let customer = message(RecipientKey::Customer(CustomerId::new(USER)), None);

    let resolution = CarriedAddress::new(Found).resolve(&customer).await.unwrap();

    assert_eq!(
        resolution,
        Recipient::Found {
            address: EmailAddress::new(
                "found-for-11111111-1111-4111-8111-111111111111@example.com"
            ),
            customer_id: None,
        }
    );
}

/// A lookup that always fails: a carried address must not need it.
struct Down;

impl Resolver for Down {
    fn resolve(
        &self,
        _message: &EmailMessage,
    ) -> impl Future<Output = Result<Recipient, LookupError>> + Send {
        std::future::ready(Err(LookupError::Failed("down".to_owned())))
    }
}

#[tokio::test]
async fn a_carried_address_is_found_with_no_customer_of_its_own() {
    let welcome = message(
        RecipientKey::Email(EmailAddress::new("ada@example.com")),
        None,
    );

    let recipient = CarriedAddress::new(Down).resolve(&welcome).await.unwrap();

    // The claimed attribution stands (mark_resolved keeps a column given None).
    assert_eq!(
        recipient,
        Recipient::Found {
            address: EmailAddress::new("ada@example.com"),
            customer_id: None,
        }
    );
}
