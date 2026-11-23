// SPDX-License-Identifier: MIT

//! Sending an email, audited (Scala `EmailClient`, `RecipientResolver`): every
//! send is claimed in `sent_email` first (ADM-2), its recipient resolved to an
//! address (ADM-3, ADR 0025), then sent through a [`Transport`] and marked.
//! Every consumer sends through [`Audited`]; the route decides the message.
//! Failures are typed ([`SendError`]); they become text only where gluon-rs's
//! DLQ guard and the row's `error` column need it.

use std::future::Future;

use telemetry::logging::Logger;
use uuid::Uuid;

use crate::domain::{CustomerId, EmailAddress, EmailMessage, carried_address};
use crate::ids::{IdGen, RandomIds};
use crate::store::{Claim, EmailAudit, StoreError};

/// What one send did, so the consumer's log says what happened (Scala
/// `EmailSendOutcome`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendOutcome {
    /// Sent, and marked `sent`.
    Sent,
    /// The event was already `sent` or `unresolved`: nothing done.
    AlreadySent,
    /// No address exists (terminal): marked `unresolved`, nothing sent.
    Unresolved(String),
}

/// Who a message goes to, as a resolver found (Scala `RecipientResolution`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    /// An address, and the customer a lookup found it for (`None`: the
    /// message's own attribution stands).
    Found {
        /// Where the email goes.
        address: EmailAddress,
        /// The customer it is attributed to, if a lookup found one.
        customer_id: Option<CustomerId>,
    },
    /// No address exists, and why, e.g. `unknown_customer:<id>` (terminal).
    Unknown(String),
}

/// A lookup that failed for now: retried by the DLQ guard, then dead-lettered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LookupError {
    /// The lookup failed, e.g. `Circuit breaker is open`: its text is the
    /// row's `error`, as the Scala exception's message is.
    #[error("{0}")]
    Failed(String),
}

/// A send the transport couldn't make; its text is the row's `error`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct TransportError(pub String);

/// Why a send failed: raised so the caller's DLQ guard retries it.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// `sent_email` couldn't be read or written.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The recipient couldn't be looked up (the row is `failed`).
    #[error(transparent)]
    Lookup(#[from] LookupError),
    /// The transport failed (the row is `failed`).
    #[error(transparent)]
    Transport(#[from] TransportError),
}

/// Finds a message's address (Scala `RecipientResolver`).
pub trait Resolver: Send + Sync {
    /// Who `message` goes to.
    ///
    /// # Errors
    /// A transient failure: retried by the caller.
    fn resolve(
        &self,
        message: &EmailMessage,
    ) -> impl Future<Output = Result<Recipient, LookupError>> + Send;
}

/// Uses the address a message already carries ([`carried_address`]) with no
/// lookup; any other message goes to `fallback` (Scala `RecipientResolver`'s
/// `resolvedAddress`, ahead of its lookups).
#[derive(Debug, Clone)]
pub struct CarriedAddress<F> {
    fallback: F,
}

impl<F> CarriedAddress<F> {
    /// Falls back to `fallback` for a message without an address.
    #[must_use]
    pub fn new(fallback: F) -> Self {
        Self { fallback }
    }
}

impl<F: Resolver> Resolver for CarriedAddress<F> {
    async fn resolve(&self, message: &EmailMessage) -> Result<Recipient, LookupError> {
        match carried_address(message) {
            Some(address) => Ok(Recipient::Found {
                address: address.clone(),
                customer_id: None,
            }),
            None => self.fallback.resolve(message).await,
        }
    }
}

/// Delivers a resolved message (Scala `EmailClient`'s transport).
pub trait Transport: Send + Sync {
    /// Sends `message`.
    ///
    /// # Errors
    /// When the send failed: the row is marked `failed` and the error raised.
    fn send(
        &self,
        message: &EmailMessage,
    ) -> impl Future<Output = Result<(), TransportError>> + Send;
}

/// The transport stub (Scala `EmailClient.logging`; no provider is decided):
/// one log line per email, never failing.
#[derive(Debug, Clone)]
pub struct LoggingTransport {
    logger: Logger,
}

impl LoggingTransport {
    /// Logs to `logger`.
    #[must_use]
    pub fn new(logger: Logger) -> Self {
        Self { logger }
    }
}

impl Transport for LoggingTransport {
    fn send(
        &self,
        message: &EmailMessage,
    ) -> impl Future<Output = Result<(), TransportError>> + Send {
        let to = message
            .recipient_email
            .as_ref()
            .map_or(message.recipient.as_str(), EmailAddress::as_str);
        self.logger.info(
            &[
                ("to", to),
                ("subject", &message.subject),
                ("body", &message.body),
                ("event_type", &message.event_type),
                ("source_event_id", message.source_event_id.as_str()),
            ],
            &format!(
                "Sending email (stub - logged only) to={to} subject={}",
                message.subject
            ),
        );
        std::future::ready(Ok(()))
    }
}

/// Sends a consumer's email (Scala `EmailClient`): [`Audited`] in the
/// service, a fake in the routes' tests.
pub trait EmailClient: Send + Sync {
    /// Sends `message`, saying what happened.
    ///
    /// # Errors
    /// A failure the caller's DLQ guard should retry.
    fn send(
        &self,
        message: EmailMessage,
    ) -> impl Future<Output = Result<SendOutcome, SendError>> + Send;
}

/// A consumer's log line for a send's outcome, so a duplicate or unresolved
/// event never logs as sent (Scala `EmailSendLog`): `context` plus
/// `outcome`, and `what` names the email, e.g. `UserRegistered`.
pub fn log_outcome(logger: &Logger, context: &[(&str, &str)], what: &str, outcome: &SendOutcome) {
    let mut fields = context.to_vec();
    match outcome {
        SendOutcome::Sent => {
            fields.push(("outcome", "sent"));
            logger.info(&fields, &format!("Sent {what} email"));
        }
        SendOutcome::AlreadySent => {
            fields.push(("outcome", "duplicate"));
            logger.info(
                &fields,
                &format!("{what} email already sent or unresolved - skipped"),
            );
        }
        SendOutcome::Unresolved(reason) => {
            fields.extend([("outcome", "unresolved"), ("reason", reason)]);
            logger.warn(
                &fields,
                &format!("{what} email not sent - recipient unresolved ({reason})"),
            );
        }
    }
}

/// The audited send (Scala `EmailClient.audited`): claim, resolve, record,
/// send, mark. A duplicate of a `sent` or `unresolved` event does nothing; an
/// unresolvable recipient is marked `unresolved` (terminal, not raised); a
/// failed lookup or send marks the row `failed` and is raised.
#[derive(Debug, Clone)]
pub struct Audited<S, T, R, I = RandomIds> {
    store: S,
    transport: T,
    resolver: R,
    ids: I,
    logger: Logger,
}

impl<S: EmailAudit, T: Transport, R: Resolver, I: IdGen> Audited<S, T, R, I> {
    /// Claims in `store` under ids from `ids`, resolves with `resolver`, sends
    /// through `transport`.
    #[must_use]
    pub fn new(store: S, transport: T, resolver: R, ids: I, logger: Logger) -> Self {
        Self {
            store,
            transport,
            resolver,
            ids,
            logger,
        }
    }

    /// Records the resolved identity (before the send, so it survives a
    /// failure), sends, marks `sent`.
    async fn deliver(&self, row: Uuid, message: EmailMessage) -> Result<SendOutcome, SendError> {
        self.store
            .mark_resolved(
                row,
                message.customer_id.as_ref(),
                message.recipient_email.as_ref(),
            )
            .await?;
        if let Err(error) = self.transport.send(&message).await {
            return Err(self.failed(row, error.into()).await);
        }
        self.store.mark_sent(row).await?;
        let email_id = row.to_string();
        let recipient = message
            .recipient_email
            .as_ref()
            .map_or(message.recipient.as_str(), EmailAddress::as_str);
        self.logger.info(
            &[
                ("email_id", &email_id),
                ("source_event_id", message.source_event_id.as_str()),
                ("recipient", recipient),
            ],
            &format!("Recorded sent email (id={email_id})"),
        );
        Ok(SendOutcome::Sent)
    }

    /// Marks the row `failed` with `error`'s text; `error`, or the store's
    /// failure if marking fails.
    async fn failed(&self, row: Uuid, error: SendError) -> SendError {
        match self.store.mark_failed(row, &error.to_string()).await {
            Ok(()) => error,
            Err(failure) => failure.into(),
        }
    }

    /// Marks the row `unresolved` with `reason`, and says so.
    async fn unresolved(
        &self,
        row: Uuid,
        message: &EmailMessage,
        reason: String,
    ) -> Result<SendOutcome, SendError> {
        self.store.mark_unresolved(row, &reason).await?;
        self.logger.warn(
            &[
                ("source_event_id", message.source_event_id.as_str()),
                ("event_type", &message.event_type),
                ("reason", &reason),
            ],
            &format!("Recipient unresolved ({reason}) - no email sent"),
        );
        Ok(SendOutcome::Unresolved(reason))
    }
}

impl<S: EmailAudit, T: Transport, R: Resolver, I: IdGen> EmailClient for Audited<S, T, R, I> {
    async fn send(&self, message: EmailMessage) -> Result<SendOutcome, SendError> {
        let row = match self.store.claim(self.ids.next_id(), &message).await? {
            Claim::Send(row) => row,
            Claim::Duplicate => {
                self.logger.info(
                    &[
                        ("source_event_id", message.source_event_id.as_str()),
                        ("event_type", &message.event_type),
                    ],
                    "Duplicate event - email already sent or unresolved; skipping",
                );
                return Ok(SendOutcome::AlreadySent);
            }
        };
        match self.resolver.resolve(&message).await {
            Err(error) => Err(self.failed(row, error.into()).await),
            Ok(Recipient::Unknown(reason)) => self.unresolved(row, &message, reason).await,
            Ok(Recipient::Found {
                address,
                customer_id,
            }) => {
                self.deliver(row, message.addressed_to(address, customer_id))
                    .await
            }
        }
    }
}
