// SPDX-License-Identifier: MIT

//! The Kafka consumers (Scala `KafkaConsumers.supervised`): user-, order-,
//! payment- and fulfilment-events, each assembled and supervised by gluon-rs
//! `kafka-client`'s `run_supervised` — its own DLQ guard (purekafka
//! `DeadLetter`), restarted with backoff when its stream fails
//! (`StreamSupervisor`) — with `application.conf`'s `kafka.*` settings
//! (`Resilience::default`), sending through the email client `main` wires.
//! Consuming needs a broker, so tests/contract's consumer groups test it, not
//! this crate's tests.

use std::time::Duration;

use kafka_client::KafkaError;
use kafka_client::settings::{ConsumerSettings, SettingsError};
use kafka_client::supervised::{ConsumerTelemetry, Resilience, run_supervised};
use tokio::sync::watch;

use crate::email::EmailClient;
use crate::fulfilment_events::{self, FulfilmentEvents};
use crate::order_events::{self, OrderEvents};
use crate::payment_events::{self, PaymentEvents};
use crate::user_events::{self, UserEvents};

/// How often librdkafka reports the statistics the lag gauge reads (the
/// events-probe's default; the Scala client's metrics have no setting).
pub const LAG_INTERVAL: Duration = Duration::from_secs(5);

/// One consumer's settings on `bootstrap`.
///
/// # Errors
/// When its group or topic isn't a name Kafka allows.
pub fn settings(
    bootstrap: &str,
    topic: &str,
    group: &str,
) -> Result<ConsumerSettings, SettingsError> {
    Ok(ConsumerSettings {
        bootstrap: bootstrap.to_owned(),
        group_id: group.parse()?,
        topic: topic.parse()?,
        lag_interval: LAG_INTERVAL,
    })
}

/// Runs the four consumers on `bootstrap` until `stop` is set, each sending
/// through `client`.
///
/// # Errors
/// When a consumer can't be built (its settings, the DLQ producer's
/// configuration); a failed stream is restarted, not returned.
pub async fn run<C: EmailClient + Clone>(
    bootstrap: &str,
    client: C,
    telemetry: ConsumerTelemetry,
    stop: watch::Receiver<bool>,
) -> Result<(), KafkaError> {
    let consumer = |topic: &str, group: &str| {
        settings(bootstrap, topic, group)
            .map_err(|error| KafkaError::ClientCreation(error.to_string()))
    };
    let users = consumer(user_events::TOPIC, user_events::GROUP)?;
    let orders = consumer(order_events::TOPIC, order_events::GROUP)?;
    let payments = consumer(payment_events::TOPIC, payment_events::GROUP)?;
    let dispatches = consumer(fulfilment_events::TOPIC, fulfilment_events::GROUP)?;
    let resilience = Resilience::default();
    let logger = telemetry.logger.clone();
    tokio::try_join!(
        run_supervised(
            users,
            &resilience,
            UserEvents::new(client.clone(), logger.clone()),
            telemetry.clone(),
            stop.clone(),
        ),
        run_supervised(
            orders,
            &resilience,
            OrderEvents::new(client.clone(), logger.clone()),
            telemetry.clone(),
            stop.clone(),
        ),
        run_supervised(
            payments,
            &resilience,
            PaymentEvents::new(client.clone(), logger.clone()),
            telemetry.clone(),
            stop.clone(),
        ),
        run_supervised(
            dispatches,
            &resilience,
            FulfilmentEvents::new(client, logger),
            telemetry,
            stop,
        ),
    )?;
    Ok(())
}
