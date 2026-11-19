// SPDX-License-Identifier: MIT

//! The audited send's nine behaviours (Scala `EmailClient.audited` and
//! `SentEmailStore.claim`; the track spec's table, gluon RPN-11), one test
//! each, in the table's order, against a throwaway Postgres migrated as the
//! service migrates it. gluon `bin/verify-complete-RPN-11.sh` holds this file
//! to exactly these nine names. The transport records what it was given (and
//! the row as the send found it); the resolvers are fakes, except
//! `CarriedAddress`, the one this service has so far.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use notification_service::domain::{
    CustomerId, EmailAddress, EmailMessage, EmailStatus, EventId, OrderId, RecipientKey, SentEmail,
};
use notification_service::email::{
    Audited, CarriedAddress, EmailClient, LookupError, Recipient, Resolver, SendError, SendOutcome,
    Transport, TransportError,
};
use notification_service::ids::{IdGen, RandomIds};
use notification_service::store::PgSentEmails;
use support::{Postgres, captured_logger, rows, store};
use uuid::Uuid;

const USER: &str = "11111111-1111-4111-8111-111111111111";
const ADA: &str = "ada@example.com";
const EVENT: &str = "e0000000-0000-4000-8000-000000000001";

/// The welcome email as the user-events route builds it: the address carried.
fn welcome() -> EmailMessage {
    EmailMessage {
        source_event_id: EventId::new(EVENT),
        event_type: "UserRegistered".to_owned(),
        reason: None,
        recipient: RecipientKey::Email(EmailAddress::new(ADA)),
        customer_id: Some(CustomerId::new(USER)),
        order_id: None,
        subject: "Welcome to gshop".to_owned(),
        body: format!("Welcome, {ADA}. Your gshop account is ready."),
        recipient_email: Some(EmailAddress::new(ADA)),
    }
}

/// An order email naming its recipient by customer only: it needs resolving.
fn by_customer() -> EmailMessage {
    EmailMessage {
        source_event_id: EventId::new("e0000000-0000-4000-8000-000000000010"),
        event_type: "OrderConfirmed".to_owned(),
        reason: None,
        recipient: RecipientKey::Customer(CustomerId::new(USER)),
        customer_id: None,
        order_id: Some(OrderId::new("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")),
        subject: "Your order is confirmed".to_owned(),
        body: "Good news".to_owned(),
        recipient_email: None,
    }
}

/// Each message sent, with the rows as the send found them.
type Sends = Arc<Mutex<Vec<(EmailMessage, Vec<SentEmail>)>>>;

/// A transport that records each message and the rows as the send found
/// them; it fails with `fail` when given one.
#[derive(Clone)]
struct Recording {
    store: PgSentEmails,
    fail: Option<String>,
    sent: Sends,
}

impl Recording {
    fn new(store: &PgSentEmails) -> Self {
        Self {
            store: store.clone(),
            fail: None,
            sent: Arc::default(),
        }
    }

    fn failing(store: &PgSentEmails, error: &str) -> Self {
        Self {
            fail: Some(error.to_owned()),
            ..Self::new(store)
        }
    }

    fn sends(&self) -> Vec<(EmailMessage, Vec<SentEmail>)> {
        self.sent.lock().unwrap().clone()
    }
}

impl Transport for Recording {
    async fn send(&self, message: &EmailMessage) -> Result<(), TransportError> {
        let found = rows(&self.store).await;
        self.sent.lock().unwrap().push((message.clone(), found));
        self.fail
            .clone()
            .map_or(Ok(()), |error| Err(TransportError(error)))
    }
}

/// How a fake resolver answers a message.
type Answer = Arc<dyn Fn(&EmailMessage) -> Result<Recipient, LookupError> + Send + Sync>;

/// A resolver that answers every message the same way, counting the calls.
#[derive(Clone)]
struct Fake {
    answer: Answer,
    calls: Arc<AtomicUsize>,
}

impl Fake {
    fn new(
        answer: impl Fn(&EmailMessage) -> Result<Recipient, LookupError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            answer: Arc::new(answer),
            calls: Arc::default(),
        }
    }

    /// Resolves to `customer` at `email`, as a user lookup would.
    fn found(customer: &str, email: &str) -> Self {
        let (customer, email) = (customer.to_owned(), email.to_owned());
        Self::new(move |_| {
            Ok(Recipient::Found {
                address: EmailAddress::new(email.clone()),
                customer_id: Some(CustomerId::new(customer.clone())),
            })
        })
    }

    fn unresolved(reason: &str) -> Self {
        let reason = reason.to_owned();
        Self::new(move |_| Ok(Recipient::Unknown(reason.clone())))
    }

    fn failing(error: &str) -> Self {
        let error = error.to_owned();
        Self::new(move |_| Err(LookupError::Failed(error.clone())))
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Resolver for Fake {
    fn resolve(
        &self,
        message: &EmailMessage,
    ) -> impl Future<Output = Result<Recipient, LookupError>> + Send {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::future::ready((self.answer)(message))
    }
}

/// Always the same id.
struct Fixed(Uuid);

impl IdGen for Fixed {
    fn next_id(&self) -> Uuid {
        self.0
    }
}

fn audited<T: Transport + Clone, R: Resolver + Clone>(
    store: &PgSentEmails,
    transport: &T,
    resolver: &R,
) -> Audited<PgSentEmails, T, R> {
    Audited::new(
        store.clone(),
        transport.clone(),
        resolver.clone(),
        RandomIds,
        captured_logger().0,
    )
}

// 1
#[tokio::test]
async fn a_new_event_is_claimed_pending_before_the_send() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::new(&store);
    let message = welcome();
    let id = Uuid::parse_str("00000000-0000-4000-8000-000000000c1a").unwrap();

    Audited::new(
        store.clone(),
        transport.clone(),
        Fake::found(USER, ADA),
        Fixed(id),
        captured_logger().0,
    )
    .send(message.clone())
    .await
    .unwrap();

    let sends = transport.sends();
    assert_eq!(sends.len(), 1);
    let [claimed] = sends[0].1.as_slice() else {
        panic!("one row when the send ran, got {:?}", sends[0].1)
    };
    assert_eq!(claimed.id, id, "the injected id");
    assert_eq!(claimed.status, EmailStatus::Pending);
    assert_eq!(claimed.sent_at, None);
    assert_eq!(claimed.source_event_id, message.source_event_id.as_str());
    assert_eq!(claimed.event_type, message.event_type);
    assert_eq!(claimed.reason, message.reason);
    assert_eq!(claimed.recipient_type, message.recipient.recipient_type());
    assert_eq!(claimed.recipient_key, message.recipient.as_str());
    assert_eq!(claimed.order_id, None);
    assert_eq!(claimed.subject, message.subject);
    assert_eq!(claimed.body, message.body);
}

// 2
#[tokio::test]
async fn a_carried_address_is_used_without_a_lookup() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::new(&store);
    let lookup = Fake::failing("no lookup expected");
    let resolver = CarriedAddress::new(lookup.clone());
    // The event is an address (UserRegistered), not yet copied to recipient_email;
    // and a customer email whose address is already known.
    let address_only = EmailMessage {
        recipient_email: None,
        ..welcome()
    };
    let known = EmailMessage {
        recipient_email: Some(EmailAddress::new("known@example.com")),
        customer_id: Some(CustomerId::new(USER)),
        ..by_customer()
    };

    for message in [address_only, known] {
        let outcome = audited(&store, &transport, &resolver).send(message).await;
        assert_eq!(outcome.unwrap(), SendOutcome::Sent);
    }

    assert_eq!(
        lookup.calls(),
        0,
        "no lookup for an address the event carries"
    );
    let to: Vec<_> = transport
        .sends()
        .iter()
        .map(|(message, _)| message.recipient_email.as_ref().map(ToString::to_string))
        .collect();
    assert_eq!(
        to,
        [Some(ADA.to_owned()), Some("known@example.com".to_owned())]
    );
    let mut stored: Vec<_> = rows(&store)
        .await
        .into_iter()
        .map(|row| row.recipient_email)
        .collect();
    stored.sort();
    assert_eq!(
        stored,
        [Some(ADA.to_owned()), Some("known@example.com".to_owned())]
    );
}

// 3
#[tokio::test]
async fn the_resolved_recipient_is_recorded_before_the_send() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::failing(&store, "smtp down");

    let outcome = audited(&store, &transport, &Fake::found(USER, ADA))
        .send(by_customer())
        .await;

    let error = outcome.unwrap_err();
    assert!(matches!(error, SendError::Transport(_)), "{error:?}");
    assert_eq!(error.to_string(), "smtp down");
    let sends = transport.sends();
    let [(given, found)] = sends.as_slice() else {
        panic!("one send, got {sends:?}")
    };
    let [at_send] = found.as_slice() else {
        panic!("one row when the send ran, got {found:?}")
    };
    assert_eq!(given.recipient_email, Some(EmailAddress::new(ADA)));
    assert_eq!(at_send.customer_id.as_deref(), Some(USER));
    assert_eq!(at_send.recipient_email.as_deref(), Some(ADA));
    // ... so the attribution survives the failed send.
    let [row] = rows(&store).await.try_into().unwrap();
    assert_eq!(row.status, EmailStatus::Failed);
    assert_eq!(row.customer_id.as_deref(), Some(USER));
    assert_eq!(row.recipient_email.as_deref(), Some(ADA));
}

// 4
#[tokio::test]
async fn a_sent_email_is_marked_sent_with_sent_at() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::new(&store);

    let outcome = audited(&store, &transport, &Fake::found(USER, ADA))
        .send(welcome())
        .await;

    assert_eq!(outcome.unwrap(), SendOutcome::Sent);
    assert_eq!(transport.sends().len(), 1);
    let [row] = rows(&store).await.try_into().unwrap();
    assert_eq!(row.status, EmailStatus::Sent);
    assert!(row.sent_at.is_some(), "sent_at set");
    assert!(
        row.sent_at >= Some(row.created_at),
        "sent after it was claimed"
    );
    assert_eq!(row.failed_at, None);
    assert_eq!(row.error, None);
}

// 5
#[tokio::test]
async fn a_duplicate_of_a_sent_event_is_not_sent_again() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::new(&store);
    let send = audited(&store, &transport, &Fake::found(USER, ADA));

    assert_eq!(send.send(welcome()).await.unwrap(), SendOutcome::Sent);
    let [first] = rows(&store).await.try_into().unwrap();
    assert_eq!(
        send.send(welcome()).await.unwrap(),
        SendOutcome::AlreadySent
    );

    assert_eq!(transport.sends().len(), 1, "sent once");
    let [row] = rows(&store).await.try_into().unwrap();
    assert_eq!(row, first, "the row untouched");
}

// 6
#[tokio::test]
async fn a_duplicate_of_an_unresolved_event_is_not_sent_again() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::new(&store);
    let resolver = Fake::unresolved("unknown_customer:22222222-2222-4222-8222-222222222222");
    let send = audited(&store, &transport, &resolver);

    let first = send.send(by_customer()).await;
    assert!(matches!(first, Ok(SendOutcome::Unresolved(_))), "{first:?}");
    let [unresolved] = rows(&store).await.try_into().unwrap();
    assert_eq!(
        send.send(by_customer()).await.unwrap(),
        SendOutcome::AlreadySent
    );

    assert_eq!(resolver.calls(), 1, "not resolved again");
    assert!(transport.sends().is_empty(), "never sent");
    let [row] = rows(&store).await.try_into().unwrap();
    assert_eq!(row, unresolved, "the row untouched");
}

// 7
#[tokio::test]
async fn a_pending_or_failed_row_is_retried_on_redelivery() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let resolver = Fake::found(USER, ADA);

    // failed: the send failed, the redelivery sends it
    let failing = Recording::failing(&store, "smtp down");
    assert!(
        audited(&store, &failing, &resolver)
            .send(welcome())
            .await
            .is_err()
    );
    let [failed] = rows(&store).await.try_into().unwrap();
    assert_eq!(failed.status, EmailStatus::Failed);
    let working = Recording::new(&store);
    assert_eq!(
        audited(&store, &working, &resolver)
            .send(welcome())
            .await
            .unwrap(),
        SendOutcome::Sent
    );
    let [retried] = rows(&store).await.try_into().unwrap();
    assert_eq!(retried.id, failed.id, "the same row");
    assert_eq!(retried.status, EmailStatus::Sent);
    assert!(retried.sent_at.is_some());
    assert_eq!((retried.failed_at, retried.error), (None, None));

    // pending: a send interrupted after the claim
    let pending = "00000000-0000-4000-8000-0000000000aa";
    postgres.query(&format!(
        "insert into sent_email (id, source_event_id, event_type, recipient_type, \
         recipient_key, subject, body, status) values ('{pending}', 'interrupted', \
         'UserRegistered', 'email', '{ADA}', 'Welcome to gshop', 'body', 'pending')"
    ));
    let interrupted = EmailMessage {
        source_event_id: EventId::new("interrupted"),
        ..welcome()
    };
    assert_eq!(
        audited(&store, &working, &resolver)
            .send(interrupted)
            .await
            .unwrap(),
        SendOutcome::Sent
    );
    let row = rows(&store)
        .await
        .into_iter()
        .find(|row| row.source_event_id == "interrupted")
        .unwrap();
    assert_eq!(row.id.to_string(), pending, "the same row");
    assert_eq!(row.status, EmailStatus::Sent);
    assert_eq!(working.sends().len(), 2);
}

// 8
#[tokio::test]
async fn an_unresolvable_recipient_is_marked_unresolved_and_not_raised() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::new(&store);
    let reason = "unknown_customer:22222222-2222-4222-8222-222222222222";

    let outcome = audited(&store, &transport, &Fake::unresolved(reason))
        .send(by_customer())
        .await;

    assert_eq!(outcome.unwrap(), SendOutcome::Unresolved(reason.to_owned()));
    assert!(transport.sends().is_empty(), "nothing sent");
    let [row] = rows(&store).await.try_into().unwrap();
    assert_eq!(row.status, EmailStatus::Unresolved);
    assert_eq!(row.error.as_deref(), Some(reason));
    assert_eq!((row.sent_at, row.failed_at), (None, None));
}

// 9
#[tokio::test]
async fn a_transient_failure_is_marked_failed_and_raised_for_retry() {
    let postgres = Postgres::start();
    let store = store(&postgres).await;
    let transport = Recording::new(&store);

    // a lookup that fails (capture: the always-500 user, its breaker open)
    let outcome = audited(
        &store,
        &transport,
        &Fake::failing("Circuit breaker is open"),
    )
    .send(by_customer())
    .await;

    let error = outcome.unwrap_err();
    assert!(matches!(error, SendError::Lookup(_)), "{error:?}");
    assert_eq!(error.to_string(), "Circuit breaker is open");
    assert!(transport.sends().is_empty(), "nothing sent");
    let [row] = rows(&store).await.try_into().unwrap();
    assert_eq!(row.status, EmailStatus::Failed);
    assert!(row.failed_at.is_some(), "failed_at set");
    assert_eq!(row.error.as_deref(), Some("Circuit breaker is open"));
    assert_eq!(row.sent_at, None);

    // a send that fails
    let failing = Recording::failing(&store, "smtp down");
    let outcome = audited(&store, &failing, &Fake::found(USER, ADA))
        .send(welcome())
        .await;
    let error = outcome.unwrap_err();
    assert!(matches!(error, SendError::Transport(_)), "{error:?}");
    assert_eq!(error.to_string(), "smtp down");
    let row = rows(&store)
        .await
        .into_iter()
        .find(|row| row.source_event_id == EVENT)
        .unwrap();
    assert_eq!(row.status, EmailStatus::Failed);
    assert!(row.failed_at.is_some());
    assert_eq!(row.error.as_deref(), Some("smtp down"));
}
