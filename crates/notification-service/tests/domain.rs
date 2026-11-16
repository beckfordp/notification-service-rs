// SPDX-License-Identifier: MIT

//! The pure core: `/emails`'s filters read from a query string as tapir reads
//! them through http4s (capture `http/emails-*.txt`; http4s `QueryParser`,
//! `Uri.decode`; tapir `Codec.listHeadOption`), and the stored states.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use notification_service::domain::{
    EmailFilter, EmailStatus, QueryError, RecipientType, decode, email_filter, query_pairs,
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
        assert_eq!(RecipientType::parse(kind.as_str()), Some(kind));
    }
    for status in [
        EmailStatus::Pending,
        EmailStatus::Sent,
        EmailStatus::Failed,
        EmailStatus::Unresolved,
    ] {
        assert_eq!(EmailStatus::parse(status.as_str()), Some(status));
    }
    assert_eq!(RecipientType::parse("Customer"), None);
    assert_eq!(EmailStatus::parse("bogus"), None);
}
