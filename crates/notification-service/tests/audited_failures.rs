// SPDX-License-Identifier: MIT

//! The audited send when `sent_email` fails, against an in-memory store with a
//! failure switch (user-service-rs's `tests/routes.rs` pattern; gluon
//! TECHNICAL-DEBT.md M5): every store failure is raised as
//! `SendError::Store`, for the DLQ guard to retry, and nothing is sent that
//! wasn't recorded first. The happy path's order of calls is pinned too. The
//! behaviours against Postgres are `tests/audited_send.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::future::Future;
use std::sync::{Arc, Mutex};

use notification_service::domain::{
    CustomerId, EmailAddress, EmailMessage, EventId, RecipientKey, welcome_message,
};
use notification_service::email::{
    Audited, CarriedAddress, EmailClient, LookupError, NoLookup, Recipient, Resolver, SendError,
    SendOutcome, Transport, TransportError,
};
use notification_service::ids::RandomIds;
use notification_service::store::{Claim, EmailAudit, StoreError};
use support::captured_logger;
use uuid::Uuid;

/// What was called, in order: the store's methods and the transport's `send`.
type Calls = Arc<Mutex<Vec<&'static str>>>;

/// `sent_email` in memory: every claim is new; the call named `fail` fails.
#[derive(Clone)]
struct Memory {
    calls: Calls,
    fail: Option<&'static str>,
}

impl Memory {
    fn failing(fail: Option<&'static str>, calls: &Calls) -> Self {
        Self {
            calls: calls.clone(),
            fail,
        }
    }

    fn call(&self, name: &'static str) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push(name);
        if self.fail == Some(name) {
            Err(StoreError::Database(sqlx::Error::PoolTimedOut))
        } else {
            Ok(())
        }
    }
}

impl EmailAudit for Memory {
    fn claim(
        &self,
        id: Uuid,
        _message: &EmailMessage,
    ) -> impl Future<Output = Result<Claim, StoreError>> + Send {
        std::future::ready(self.call("claim").map(|()| Claim::Send(id)))
    }

    fn mark_resolved(
        &self,
        _id: Uuid,
        _customer_id: Option<&CustomerId>,
        _recipient_email: Option<&EmailAddress>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        std::future::ready(self.call("mark_resolved"))
    }

    fn mark_sent(&self, _id: Uuid) -> impl Future<Output = Result<(), StoreError>> + Send {
        std::future::ready(self.call("mark_sent"))
    }

    fn mark_failed(
        &self,
        _id: Uuid,
        _error: &str,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        std::future::ready(self.call("mark_failed"))
    }

    fn mark_unresolved(
        &self,
        _id: Uuid,
        _reason: &str,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        std::future::ready(self.call("mark_unresolved"))
    }
}

/// A transport that records its sends in the same call list; fails if told to.
#[derive(Clone)]
struct Sends {
    calls: Calls,
    fail: bool,
}

impl Transport for Sends {
    fn send(
        &self,
        _message: &EmailMessage,
    ) -> impl Future<Output = Result<(), TransportError>> + Send {
        self.calls.lock().unwrap().push("send");
        std::future::ready(if self.fail {
            Err(TransportError("smtp down".to_owned()))
        } else {
            Ok(())
        })
    }
}

/// Finds no address for anyone.
#[derive(Clone)]
struct NoOne;

impl Resolver for NoOne {
    fn resolve(
        &self,
        _message: &EmailMessage,
    ) -> impl Future<Output = Result<Recipient, LookupError>> + Send {
        std::future::ready(Ok(Recipient::Unknown("unknown_customer:x".to_owned())))
    }
}

fn welcome() -> EmailMessage {
    welcome_message(
        EventId::new("e0000000-0000-4000-8000-000000000001"),
        CustomerId::new("11111111-1111-4111-8111-111111111111"),
        EmailAddress::new("ada@example.com"),
    )
}

fn by_customer() -> EmailMessage {
    EmailMessage {
        recipient: RecipientKey::Customer(CustomerId::new("11111111-1111-4111-8111-111111111111")),
        recipient_email: None,
        ..welcome()
    }
}

/// Sends `message` with the store failing at `fail` (the transport at `send`)
/// and `resolver`; the outcome and every call made.
async fn send_with<R: Resolver>(
    fail: Option<&'static str>,
    resolver: R,
    message: EmailMessage,
) -> (Result<SendOutcome, SendError>, Vec<&'static str>) {
    let calls = Calls::default();
    let transport = Sends {
        calls: calls.clone(),
        fail: fail == Some("send"),
    };
    let audited = Audited::new(
        Memory::failing(fail, &calls),
        transport,
        resolver,
        RandomIds,
        captured_logger().0,
    );
    let outcome = audited.send(message).await;
    let made = calls.lock().unwrap().clone();
    (outcome, made)
}

fn assert_store_error(outcome: &Result<SendOutcome, SendError>) {
    assert!(
        matches!(outcome, Err(SendError::Store(_))),
        "a store failure, got {outcome:?}"
    );
}

#[tokio::test]
async fn a_send_claims_records_sends_and_marks_in_that_order() {
    let (outcome, calls) = send_with(None, CarriedAddress::new(NoLookup), welcome()).await;

    assert_eq!(outcome.unwrap(), SendOutcome::Sent);
    assert_eq!(calls, ["claim", "mark_resolved", "send", "mark_sent"]);
}

#[tokio::test]
async fn a_failed_claim_is_raised_and_nothing_else_happens() {
    let (outcome, calls) = send_with(Some("claim"), CarriedAddress::new(NoLookup), welcome()).await;

    assert_store_error(&outcome);
    assert_eq!(calls, ["claim"]);
}

#[tokio::test]
async fn a_recipient_that_cant_be_recorded_is_not_sent_to() {
    let (outcome, calls) = send_with(
        Some("mark_resolved"),
        CarriedAddress::new(NoLookup),
        welcome(),
    )
    .await;

    assert_store_error(&outcome);
    assert_eq!(calls, ["claim", "mark_resolved"], "no send");
}

#[tokio::test]
async fn a_send_that_cant_be_marked_sent_is_raised() {
    // At least once: the redelivery finds the row pending and sends again, as
    // the Scala service does.
    let (outcome, calls) =
        send_with(Some("mark_sent"), CarriedAddress::new(NoLookup), welcome()).await;

    assert_store_error(&outcome);
    assert_eq!(calls, ["claim", "mark_resolved", "send", "mark_sent"]);
}

#[tokio::test]
async fn an_unresolved_mark_that_fails_is_raised() {
    let (outcome, calls) = send_with(Some("mark_unresolved"), NoOne, by_customer()).await;

    assert_store_error(&outcome);
    assert_eq!(calls, ["claim", "mark_unresolved"], "no send");
}

#[tokio::test]
async fn a_failed_mark_after_a_failed_send_raises_the_store_failure() {
    // Both fail: the store's failure is raised (as Scala's markFailed *>
    // raiseError); either way the DLQ guard retries.
    let calls = Calls::default();
    let audited = Audited::new(
        Memory::failing(Some("mark_failed"), &calls),
        Sends {
            calls: calls.clone(),
            fail: true,
        },
        CarriedAddress::new(NoLookup),
        RandomIds,
        captured_logger().0,
    );

    let outcome = audited.send(welcome()).await;

    assert_store_error(&outcome);
    assert_eq!(
        *calls.lock().unwrap(),
        ["claim", "mark_resolved", "send", "mark_failed"]
    );
}

#[tokio::test]
async fn a_message_needing_a_lookup_fails_until_lookups_arrive() {
    let (outcome, calls) = send_with(None, CarriedAddress::new(NoLookup), by_customer()).await;

    assert!(
        matches!(
            outcome,
            Err(SendError::Lookup(LookupError::NotPorted { .. }))
        ),
        "{outcome:?}"
    );
    assert_eq!(calls, ["claim", "mark_failed"], "marked failed, no send");
}
