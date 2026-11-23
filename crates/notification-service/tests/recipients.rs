// SPDX-License-Identifier: MIT

//! The recipient lookups (Scala `RecipientResolverSuite`'s cases, ADR 0025):
//! a customer's own address, the order's guest address as fallback, and the
//! unresolved reasons; a failed lookup raised.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use notification_service::domain::{
    CustomerId, EmailAddress, EmailMessage, EventId, OrderContact, OrderId, RecipientKey,
};
use notification_service::email::{LookupError, Recipient, Resolver};
use notification_service::recipients::{CustomerEmails, Lookups, OrderContacts};

const ADA: &str = "ada@example.com";
const GUEST: &str = "guest@example.com";

/// Users by id; `None` for a failing user-service. Records each id asked.
#[derive(Clone, Default)]
struct Users {
    known: Option<HashMap<String, String>>,
    asked: Arc<Mutex<Vec<String>>>,
}

impl CustomerEmails for Users {
    fn email(
        &self,
        customer: &CustomerId,
    ) -> impl Future<Output = Result<Option<EmailAddress>, LookupError>> + Send {
        self.asked.lock().unwrap().push(customer.to_string());
        std::future::ready(match &self.known {
            None => Err(LookupError::Failed("users down".to_owned())),
            Some(known) => Ok(known.get(customer.as_str()).map(EmailAddress::new)),
        })
    }
}

/// Orders by id: (customer, guest email); `None` for a failing order-service.
#[derive(Clone, Default)]
struct Orders {
    known: Option<HashMap<String, (String, Option<String>)>>,
    asked: Arc<Mutex<Vec<String>>>,
}

impl OrderContacts for Orders {
    fn contact(
        &self,
        order: &OrderId,
    ) -> impl Future<Output = Result<Option<OrderContact>, LookupError>> + Send {
        self.asked.lock().unwrap().push(order.to_string());
        std::future::ready(match &self.known {
            None => Err(LookupError::Failed("orders down".to_owned())),
            Some(known) => Ok(known
                .get(order.as_str())
                .map(|(customer, guest)| OrderContact {
                    customer_id: CustomerId::new(customer),
                    guest_email: guest.as_deref().map(EmailAddress::new),
                })),
        })
    }
}

fn users(known: &[(&str, &str)]) -> Users {
    Users {
        known: Some(
            known
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        ),
        ..Users::default()
    }
}

fn orders(known: &[(&str, &str, Option<&str>)]) -> Orders {
    Orders {
        known: Some(
            known
                .iter()
                .map(|(o, c, g)| ((*o).to_owned(), ((*c).to_owned(), g.map(str::to_owned))))
                .collect(),
        ),
        ..Orders::default()
    }
}

/// A message naming `customer` (order-events) or only `order` (payment and
/// fulfilment events).
fn message(customer: Option<&str>, order: Option<&str>) -> EmailMessage {
    EmailMessage {
        source_event_id: EventId::new("e1"),
        event_type: "OrderConfirmed".to_owned(),
        reason: None,
        recipient: match (customer, order) {
            (Some(c), _) => RecipientKey::Customer(CustomerId::new(c)),
            (None, o) => RecipientKey::Order(OrderId::new(o.unwrap_or("none"))),
        },
        customer_id: customer.map(CustomerId::new),
        order_id: order.map(OrderId::new),
        subject: "s".to_owned(),
        body: "b".to_owned(),
        recipient_email: None,
    }
}

fn found(address: &str, customer: &str) -> Recipient {
    Recipient::Found {
        address: EmailAddress::new(address),
        customer_id: Some(CustomerId::new(customer)),
    }
}

async fn resolve(
    users: &Users,
    orders: &Orders,
    message: &EmailMessage,
) -> Result<Recipient, LookupError> {
    Lookups::new(users.clone(), orders.clone())
        .resolve(message)
        .await
}

#[tokio::test]
async fn a_known_customer_gets_their_own_address_without_the_order() {
    let (u, o) = (users(&[("c1", ADA)]), orders(&[]));
    let got = resolve(&u, &o, &message(Some("c1"), Some("o1")))
        .await
        .unwrap();
    assert_eq!(got, found(ADA, "c1"));
    assert!(o.asked.lock().unwrap().is_empty(), "no order lookup");
}

#[tokio::test]
async fn an_unknown_customer_falls_back_to_the_orders_guest_address() {
    let (u, o) = (users(&[]), orders(&[("o1", "c1", Some(GUEST))]));
    let got = resolve(&u, &o, &message(Some("c1"), Some("o1")))
        .await
        .unwrap();
    assert_eq!(got, found(GUEST, "c1"));
}

#[tokio::test]
async fn an_unknown_customer_with_no_guest_address_is_unresolved() {
    let (u, o) = (users(&[]), orders(&[("o1", "c1", None)]));
    let got = resolve(&u, &o, &message(Some("c1"), Some("o1")))
        .await
        .unwrap();
    assert_eq!(got, Recipient::Unknown("unknown_customer:c1".to_owned()));
    let no_order = resolve(&u, &o, &message(Some("c1"), None)).await.unwrap();
    assert_eq!(
        no_order,
        Recipient::Unknown("unknown_customer:c1".to_owned())
    );
}

#[tokio::test]
async fn an_order_is_looked_up_for_its_customer_once() {
    let (u, o) = (users(&[("c1", ADA)]), orders(&[("o1", "c1", None)]));
    let got = resolve(&u, &o, &message(None, Some("o1"))).await.unwrap();
    assert_eq!(got, found(ADA, "c1"));
    assert_eq!(*u.asked.lock().unwrap(), ["c1"]);

    let (u, o) = (users(&[]), orders(&[("o2", "c2", Some(GUEST))]));
    let got = resolve(&u, &o, &message(None, Some("o2"))).await.unwrap();
    assert_eq!(got, found(GUEST, "c2"));
    assert_eq!(
        *o.asked.lock().unwrap(),
        ["o2"],
        "the contact found is reused"
    );
}

#[tokio::test]
async fn an_unknown_order_is_unresolved() {
    let (u, o) = (users(&[]), orders(&[]));
    let got = resolve(&u, &o, &message(None, Some("o9"))).await.unwrap();
    assert_eq!(got, Recipient::Unknown("unknown_order:o9".to_owned()));
    assert!(u.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn neither_customer_nor_order_is_no_customer_identity() {
    let got = resolve(&users(&[]), &orders(&[]), &message(None, None))
        .await
        .unwrap();
    assert_eq!(got, Recipient::Unknown("no_customer_identity".to_owned()));
}

#[tokio::test]
async fn a_failed_lookup_is_raised() {
    let down = Users::default();
    let error = resolve(&down, &orders(&[]), &message(Some("c1"), None))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "users down");
    let error = resolve(&users(&[]), &Orders::default(), &message(None, Some("o1")))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "orders down");
    let error = resolve(
        &users(&[]),
        &Orders::default(),
        &message(Some("c1"), Some("o1")),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "orders down",
        "the guest lookup's failure too"
    );
}
