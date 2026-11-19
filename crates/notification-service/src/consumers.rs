// SPDX-License-Identifier: MIT

//! The Kafka consumers (Scala `KafkaConsumers.supervised`): each assembled and
//! supervised by gluon-rs `kafka-client`'s `run_supervised` — its own DLQ guard
//! (purekafka `DeadLetter`), restarted with backoff when its stream fails
//! (`StreamSupervisor`) — with `application.conf`'s `kafka.*` settings
//! (`Resilience::default`), sending through the email client `main` wires.
//! So far the user-events consumer (RPN-11). Consuming needs a broker, so
//! tests/contract's consumer groups test it, not this crate's tests.

use std::time::Duration;

use kafka_client::KafkaError;
use kafka_client::settings::ConsumerSettings;
use kafka_client::supervised::{ConsumerTelemetry, Resilience, run_supervised};
use tokio::sync::watch;

use crate::email::EmailClient;
use crate::user_events::{GROUP, TOPIC, UserEvents};

/// How often librdkafka reports the statistics the lag gauge reads (the
/// events-probe's default; the Scala client's metrics have no setting).
pub const LAG_INTERVAL: Duration = Duration::from_secs(5);

/// Runs the consumers on `bootstrap` until `stop` is set, sending through
/// `client`.
///
/// # Errors
/// When a consumer can't be built (the DLQ producer's configuration); a
/// failed stream is restarted, not returned.
pub async fn run<C: EmailClient>(
    bootstrap: &str,
    client: C,
    telemetry: ConsumerTelemetry,
    stop: watch::Receiver<bool>,
) -> Result<(), KafkaError> {
    let settings = ConsumerSettings {
        bootstrap: bootstrap.to_owned(),
        group_id: GROUP.to_owned(),
        topic: TOPIC.to_owned(),
        lag_interval: LAG_INTERVAL,
    };
    let routes = UserEvents::new(client, telemetry.logger.clone());
    run_supervised(settings, &Resilience::default(), routes, telemetry, stop).await
}
