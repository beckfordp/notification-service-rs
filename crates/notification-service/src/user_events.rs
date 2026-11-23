// SPDX-License-Identifier: MIT

//! The user-events consumer's route (Scala `UserRegisteredConsumer`,
//! US-16.4): `UserRegistered` sends the welcome email to the address in the
//! event, through the audited send; every other event type is skipped. The
//! rest of the per-record decision (null values, undecodable envelopes and
//! payloads to `user-events-dlq`, the DLQ guard) is gluon-rs
//! `kafka-event-consumer`'s `handle_record`, as it is `EnvelopeHandler`'s in
//! Scala.

use kafka_event_consumer::envelope::{Envelope, Routes};
use schemars::JsonSchema;
use serde::Deserialize;
use telemetry::logging::Logger;

use crate::domain::{CustomerId, EmailAddress, EventId, USER_REGISTERED, welcome_message};
use crate::email::{EmailClient, SendError, log_outcome};

/// The topic.
pub const TOPIC: &str = "user-events";

/// The consumer group (capture `kafka/consumer-groups.txt`).
pub const GROUP: &str = "notification-service-user-events";

/// `UserRegistered`'s payload (Scala `UserRegisteredEvent`; gluon
/// docs/system-design.md "Payload contracts"): both fields required, others
/// ignored, as circe's derived decoder does.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "UserRegisteredEvent",
    description = "A user registered: user-service created the account."
)]
pub struct UserRegistered {
    /// The new user's id: the email's customer.
    pub user_id: CustomerId,
    /// Their address: the recipient, no lookup needed.
    pub email: EmailAddress,
}

/// Why a `UserRegistered` wasn't handled: logged, retried and dead-lettered by
/// the DLQ guard. Each reads as the message it replaced (gluon TD-64.1).
#[derive(Debug, thiserror::Error)]
pub enum UserEventsError {
    /// The payload isn't a `UserRegistered` (poison).
    #[error(transparent)]
    Payload(#[from] serde_json::Error),
    /// The welcome email's send failed.
    #[error(transparent)]
    Send(#[from] SendError),
}

/// The route: `UserRegistered` → the welcome email, through `client`.
#[derive(Debug, Clone)]
pub struct UserEvents<C> {
    client: C,
    logger: Logger,
}

impl<C> UserEvents<C> {
    /// Sends through `client`, logging each outcome to `logger`.
    #[must_use]
    pub fn new(client: C, logger: Logger) -> Self {
        Self { client, logger }
    }
}

impl<C: EmailClient> Routes for UserEvents<C> {
    type Event = UserRegistered;
    type Error = UserEventsError;

    fn decode(&self, envelope: &Envelope) -> Option<Result<UserRegistered, UserEventsError>> {
        (envelope.event_type == USER_REGISTERED).then(|| {
            serde_json::from_value(envelope.payload.clone()).map_err(UserEventsError::from)
        })
    }

    /// The welcome email ([`welcome_message`]); a failed send is raised for
    /// the DLQ guard.
    async fn handle(&self, event_id: &str, event: &UserRegistered) -> Result<(), UserEventsError> {
        let outcome = self
            .client
            .send(welcome_message(
                EventId::new(event_id),
                event.user_id.clone(),
                event.email.clone(),
            ))
            .await?;
        log_outcome(
            &self.logger,
            &[
                ("user_id", event.user_id.as_str()),
                ("event_type", USER_REGISTERED),
            ],
            USER_REGISTERED,
            &outcome,
        );
        Ok(())
    }
}
