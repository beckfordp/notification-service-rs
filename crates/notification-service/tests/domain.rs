// SPDX-License-Identifier: MIT

//! The pure core: `/emails`'s filters read from a query string as tapir reads
//! them through http4s (capture `http/emails-*.txt`; http4s `QueryParser`,
//! `Uri.decode`; tapir `Codec.listHeadOption`), the stored states, the welcome
//! email and the address a message carries.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use notification_service::domain::{
    CustomerId, EmailAddress, EmailFilter, EmailMessage, EmailStatus, EventId, OrderId, QueryError,
    RecipientKey, RecipientType, UnknownValue, carried_address, decode, email_filter, query_pairs,
    welcome_message,
};

fn filter(customer_id: Option<&str>, order_id: Option<&str>) -> EmailFilter {
    EmailFilter {
        customer_id: customer_id.map(str::to_owned),
        order_id: order_id.map(str::to_owned),
    }
}

#[test]
fn no_query_and_an_empty_one_filter_nothing() {
    assert_eq!(email_filter(None), Ok(EmailFilter::default()));
    assert_eq!(email_filter(Some("")), Ok(EmailFilter::default()));
}

#[test]
fn each_filter_is_read_and_unknown_keys_are_ignored() {
    assert_eq!(
        email_filter(Some("customerId=c&orderId=o&foo=bar")),
        Ok(filter(Some("c"), Some("o")))
    );
    assert_eq!(email_filter(Some("foo=bar")), Ok(EmailFilter::default()));
    assert_eq!(email_filter(Some("orderId=o")), Ok(filter(None, Some("o"))));
}

#[test]
fn an_empty_value_is_a_filter_on_the_empty_string_but_a_bare_key_is_none() {
    // N3: `customerId=` filters on "" (capture http/emails-empty-param.txt);
    // http4s gives a bare key no value, so tapir sees no parameter.
    assert_eq!(
        email_filter(Some("customerId=")),
        Ok(filter(Some(""), None))
    );
    assert_eq!(email_filter(Some("customerId")), Ok(EmailFilter::default()));
    assert_eq!(
        email_filter(Some("customerId&customerId=c")),
        Ok(filter(Some("c"), None))
    );
}

#[test]
fn a_filter_is_any_text() {
    // N4 (capture http/emails-not-uuid.txt).
    assert_eq!(
        email_filter(Some("customerId=not-a-uuid")),
        Ok(filter(Some("not-a-uuid"), None))
    );
}

#[test]
fn a_repeated_filter_is_an_error_naming_it_customer_id_first() {
    // N5 (capture http/emails-repeated-param.txt).
    let repeated = email_filter(Some("customerId=a&customerId=b"));
    assert_eq!(repeated, Err(QueryError::Multiple("customerId")));
    assert_eq!(
        repeated.unwrap_err().to_string(),
        "Invalid value for: query parameter customerId (multiple values)"
    );
    assert_eq!(
        email_filter(Some("orderId=a&orderId=a")),
        Err(QueryError::Multiple("orderId"))
    );
    assert_eq!(
        email_filter(Some("orderId=a&orderId=b&customerId=x&customerId=y")),
        Err(QueryError::Multiple("customerId"))
    );
    assert_eq!(
        email_filter(Some("customerId=&customerId=")),
        Err(QueryError::Multiple("customerId"))
    );
}

#[test]
fn pairs_split_on_ampersand_and_semicolon_at_the_first_equals() {
    assert_eq!(
        query_pairs("a=1;b=2&c&d=x=y&"),
        vec![
            ("a".to_owned(), Some("1".to_owned())),
            ("b".to_owned(), Some("2".to_owned())),
            ("c".to_owned(), None),
            ("d".to_owned(), Some("x=y".to_owned())),
            (String::new(), None),
        ]
    );
    assert_eq!(query_pairs(""), Vec::new());
    assert_eq!(
        email_filter(Some("customerId=a;customerId=b")),
        Err(QueryError::Multiple("customerId"))
    );
}

#[test]
fn components_decode_as_http4s_does() {
    assert_eq!(decode("a+b"), "a b");
    assert_eq!(decode("a%20b+c"), "a b c");
    assert_eq!(decode("%7E%7e"), "~~");
    assert_eq!(decode("caf%C3%A9"), "café");
    assert_eq!(decode("caf\u{e9}%21"), "café!");
    // A bad escape stays literal; so does a short one at the end, with the rest.
    assert_eq!(decode("%zz%41"), "%zzA");
    assert_eq!(decode("a%4"), "a%4");
    assert_eq!(decode("a%+"), "a%+");
    // Bytes that aren't UTF-8 are replaced.
    assert_eq!(decode("%FF"), "\u{fffd}");
    assert_eq!(
        email_filter(Some("customerId=11111111-1111-4111-8111-11111111111%31")),
        Ok(filter(Some("11111111-1111-4111-8111-111111111111"), None))
    );
}

#[test]
fn recipient_types_and_statuses_round_trip_and_nothing_else_parses() {
    for kind in [
        RecipientType::Customer,
        RecipientType::Email,
        RecipientType::Order,
    ] {
        assert_eq!(kind.as_str().parse::<RecipientType>(), Ok(kind));
        assert_eq!(kind.to_string(), kind.as_str());
    }
    for status in [
        EmailStatus::Pending,
        EmailStatus::Sent,
        EmailStatus::Failed,
        EmailStatus::Unresolved,
    ] {
        assert_eq!(status.as_str().parse::<EmailStatus>(), Ok(status));
        assert_eq!(status.to_string(), status.as_str());
    }
    assert_eq!(
        "Customer".parse::<RecipientType>(),
        Err(UnknownValue("Customer".to_owned()))
    );
    assert_eq!(
        "bogus".parse::<EmailStatus>(),
        Err(UnknownValue("bogus".to_owned()))
    );
}

const USER: &str = "11111111-1111-4111-8111-111111111111";
const ADA: &str = "ada@example.com";
const EVENT: &str = "e0000000-0000-4000-8000-000000000001";
const ORDER: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

fn welcome() -> EmailMessage {
    welcome_message(
        EventId::new(EVENT),
        CustomerId::new(USER),
        EmailAddress::new(ADA),
    )
}

#[test]
fn the_welcome_message_is_the_captured_one() {
    // capture db/sent-email.jsonl, e…001; Scala UserRegisteredConsumer.emailFor, sendWelcome
    assert_eq!(
        welcome(),
        EmailMessage {
            source_event_id: EventId::new(EVENT),
            event_type: "UserRegistered".to_owned(),
            reason: None,
            recipient: RecipientKey::Email(EmailAddress::new(ADA)),
            customer_id: Some(CustomerId::new(USER)),
            order_id: None,
            subject: "Welcome to gshop".to_owned(),
            body: "Welcome, ada@example.com. Your gshop account is ready.".to_owned(),
            recipient_email: Some(EmailAddress::new(ADA)),
        }
    );
}

#[test]
fn a_recipient_key_is_its_type_and_its_text() {
    // The stored pair (recipient_type, recipient_key), one value in the domain.
    let keys = [
        (
            RecipientKey::Customer(CustomerId::new(USER)),
            RecipientType::Customer,
            USER,
        ),
        (
            RecipientKey::Email(EmailAddress::new(ADA)),
            RecipientType::Email,
            ADA,
        ),
        (
            RecipientKey::Order(OrderId::new(ORDER)),
            RecipientType::Order,
            ORDER,
        ),
    ];
    for (key, recipient_type, text) in keys {
        assert_eq!(key.recipient_type(), recipient_type);
        assert_eq!(key.as_str(), text);
    }
}

#[test]
fn ids_and_addresses_are_any_text_as_the_scala_service_takes_them() {
    // No validation beyond Scala's: these are strings there.
    for text in ["", "not-a-uuid", "no at sign"] {
        assert_eq!(CustomerId::new(text).as_str(), text);
        assert_eq!(EmailAddress::new(text).to_string(), text);
        assert_eq!(OrderId::new(text).as_str(), text);
        assert_eq!(EventId::new(text).as_str(), text);
    }
}

fn addressed(recipient: RecipientKey, email: Option<&str>) -> EmailMessage {
    EmailMessage {
        recipient,
        recipient_email: email.map(EmailAddress::new),
        ..welcome()
    }
}

#[test]
fn a_message_carries_its_resolved_address_or_the_address_it_names() {
    // Scala RecipientResolver.resolvedAddress
    let known = addressed(
        RecipientKey::Customer(CustomerId::new(USER)),
        Some("known@example.com"),
    );
    assert_eq!(
        carried_address(&known),
        Some(&EmailAddress::new("known@example.com"))
    );
    let named = addressed(RecipientKey::Email(EmailAddress::new(ADA)), None);
    assert_eq!(carried_address(&named), Some(&EmailAddress::new(ADA)));
}

#[test]
fn a_customer_or_order_without_an_address_carries_none() {
    let customer = addressed(RecipientKey::Customer(CustomerId::new(USER)), None);
    assert_eq!(carried_address(&customer), None);
    let order = addressed(RecipientKey::Order(OrderId::new(ORDER)), None);
    assert_eq!(carried_address(&order), None);
}

#[test]
fn a_found_address_is_the_recipient_and_a_found_customer_the_attribution() {
    let order = addressed(RecipientKey::Customer(CustomerId::new(USER)), None);

    let found = order.clone().addressed_to(
        EmailAddress::new("guest@example.com"),
        Some(CustomerId::new("22222222-2222-4222-8222-222222222222")),
    );
    assert_eq!(
        found.recipient_email,
        Some(EmailAddress::new("guest@example.com"))
    );
    assert_eq!(
        found.customer_id,
        Some(CustomerId::new("22222222-2222-4222-8222-222222222222"))
    );

    let carried = order.addressed_to(EmailAddress::new(ADA), None);
    assert_eq!(
        carried.customer_id,
        Some(CustomerId::new(USER)),
        "as claimed"
    );
}
