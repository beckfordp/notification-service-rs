// SPDX-License-Identifier: MIT

//! The pure core: the audit row and its states, and the `/emails` filters as
//! the Scala service reads them from a query string. No I/O, no `async`
//! (`conductor/code_styleguides/rust.md` rule 1).

use std::fmt;

use time::OffsetDateTime;
use uuid::Uuid;

/// Who an email is addressed to, as the event identifies them (Scala
/// `RecipientType`: `customer`, `email`, `order`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientType {
    /// A customerId, resolved through user-service.
    Customer,
    /// An email address carried by the event.
    Email,
    /// An orderId, resolved through order-service then user-service.
    Order,
}

impl RecipientType {
    /// The stored and wire form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Customer => "customer",
            Self::Email => "email",
            Self::Order => "order",
        }
    }

    /// The stored form parsed; `None` for any other text.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "customer" => Some(Self::Customer),
            "email" => Some(Self::Email),
            "order" => Some(Self::Order),
            _ => None,
        }
    }
}

/// Where a send stands (Scala `EmailStatus`; `sent_email_status_check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailStatus {
    /// Claimed, not yet sent.
    Pending,
    /// Sent.
    Sent,
    /// The send failed; a redelivery retries it.
    Failed,
    /// No address could be found: terminal.
    Unresolved,
}

impl EmailStatus {
    /// The stored and wire form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Unresolved => "unresolved",
        }
    }

    /// The stored form parsed; `None` for any other text.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "pending" => Some(Self::Pending),
            "sent" => Some(Self::Sent),
            "failed" => Some(Self::Failed),
            "unresolved" => Some(Self::Unresolved),
            _ => None,
        }
    }
}

/// One `sent_email` row (Scala `SentEmailRow`): one handled event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentEmail {
    /// The row id.
    pub id: Uuid,
    /// The envelope `eventId`: the idempotency key.
    pub source_event_id: String,
    /// The event type, e.g. `OrderConfirmed`.
    pub event_type: String,
    /// The event's cancellation or failure reason, if any.
    pub reason: Option<String>,
    /// How the event identifies the recipient.
    pub recipient_type: RecipientType,
    /// The event's identity for the recipient: customerId, address or orderId.
    pub recipient_key: String,
    /// The resolved address, once known.
    pub recipient_email: Option<String>,
    /// The customer the email is attributed to, when known.
    pub customer_id: Option<String>,
    /// The order the email concerns, when the event carries one.
    pub order_id: Option<String>,
    /// The subject.
    pub subject: String,
    /// The body.
    pub body: String,
    /// Where the send stands.
    pub status: EmailStatus,
    /// When the send was claimed.
    pub created_at: OffsetDateTime,
    /// When it was sent, if it was.
    pub sent_at: Option<OffsetDateTime>,
    /// When it last failed, if it did.
    pub failed_at: Option<OffsetDateTime>,
    /// Why it failed or is unresolved.
    pub error: Option<String>,
}

/// `GET /emails`'s filters: each optional, matched as exact text, combined
/// with AND (Scala `SentEmailStore.listEmails`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmailFilter {
    /// `customerId`: only rows attributed to this customer.
    pub customer_id: Option<String>,
    /// `orderId`: only rows for this order.
    pub order_id: Option<String>,
}

/// A query parameter tapir can't decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// The parameter came more than once (tapir's `DecodeResult.Multiple`).
    Multiple(&'static str),
}

impl fmt::Display for QueryError {
    /// tapir's decode-failure message (capture `http/emails-repeated-param.txt`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Multiple(name) => {
                write!(
                    f,
                    "Invalid value for: query parameter {name} (multiple values)"
                )
            }
        }
    }
}

impl std::error::Error for QueryError {}

/// The filter parameters, in the order tapir decodes the endpoint's inputs
/// (`EmailRoutes`), so a failure names the first.
const CUSTOMER_ID: &str = "customerId";
const ORDER_ID: &str = "orderId";

/// Reads the filters from a raw query string (without the `?`), as tapir
/// decodes `query[Option[String]]` through http4s: a parameter present once
/// is its value, absent (or given only as a bare key, `?customerId`) is no
/// filter, present twice or more is [`QueryError::Multiple`]. Other keys are
/// ignored (capture `http/emails-unknown-param.txt`).
///
/// # Errors
/// [`QueryError::Multiple`] for the first repeated filter.
pub fn email_filter(query: Option<&str>) -> Result<EmailFilter, QueryError> {
    let pairs = query.map(query_pairs).unwrap_or_default();
    Ok(EmailFilter {
        customer_id: single(&pairs, CUSTOMER_ID)?,
        order_id: single(&pairs, ORDER_ID)?,
    })
}

/// The values given for `name` (a bare key gives none): one is the filter,
/// none is no filter, more is an error (tapir `Codec.listHeadOption`).
fn single(
    pairs: &[(String, Option<String>)],
    name: &'static str,
) -> Result<Option<String>, QueryError> {
    let mut values = pairs
        .iter()
        .filter(|(key, _)| key == name)
        .filter_map(|(_, value)| value.clone());
    match (values.next(), values.next()) {
        (first, None) => Ok(first),
        (Some(_) | None, Some(_)) => Err(QueryError::Multiple(name)),
    }
}

/// A query string split as http4s `QueryParser` splits it: pairs separated by
/// `&` or `;`, the first `=` between key and value, a key without `=` has no
/// value; keys and values decoded by [`decode`]. An empty query has no pairs.
#[must_use]
pub fn query_pairs(query: &str) -> Vec<(String, Option<String>)> {
    if query.is_empty() {
        return Vec::new();
    }
    query
        .split(['&', ';'])
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (decode(key), Some(decode(value))),
            None => (decode(pair), None),
        })
        .collect()
}

/// A query component decoded as http4s `Uri.decode(_, UTF-8, plusIsSpace =
/// true)`: `+` is a space, `%XX` is a byte; a `%` not followed by two hex
/// digits stays literal, and so does everything after a `%` with fewer than
/// two characters left. Bytes that aren't UTF-8 become U+FFFD. (Byte by byte:
/// everything it acts on is ASCII, so it decides as http4s's char loop does.)
#[must_use]
pub fn decode(text: &str) -> String {
    if !text.contains('%') {
        return text.replace('+', " ");
    }
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        at += 1;
        match byte {
            b'%' => {
                let (Some(&high), Some(&low)) = (bytes.get(at), bytes.get(at + 1)) else {
                    out.push(b'%');
                    out.extend_from_slice(bytes.get(at..).unwrap_or_default());
                    break;
                };
                if let (Some(high), Some(low)) = (hex(high), hex(low)) {
                    out.push((high << 4) | low);
                    at += 2;
                } else {
                    out.push(b'%');
                }
            }
            b'+' => out.push(b' '),
            other => out.push(other),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// An ASCII hex digit's value.
fn hex(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|digit| u8::try_from(digit).ok())
}
