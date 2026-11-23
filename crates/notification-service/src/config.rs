// SPDX-License-Identifier: MIT

//! The config contract: the Scala service's environment variables and
//! defaults (notification-service `application.conf`). Pure: an environment
//! map in, a [`Config`] or every problem out; the pieces every service reads
//! the same way are gluon-rs `service-basics`. The walking skeleton (RPN-2)
//! reads the HTTP port and Postgres; the user-events consumer (RPN-11) adds the
//! service name, Kafka and the OTEL variables (gluon-rs `telemetry`'s reader);
//! the rest arrives with its feature.

use std::collections::HashMap;
use std::hash::BuildHasher;

pub use service_basics::config::{ConfigError, PostgresConfig};
use service_basics::config::{port, postgres};
pub use telemetry::config::TracingConfig;
use telemetry::config::tracing_from_env;

/// Everything the service reads from its environment.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `NOTIFICATION_SERVICE_PORT`: the HTTP API.
    pub port: u16,
    /// `NOTIFICATION_SERVICE_NAME`: `service.name` on spans, the logger name.
    pub service_name: String,
    /// `POSTGRES_*`.
    pub postgres: PostgresConfig,
    /// `KAFKA_BOOTSTRAP_SERVERS`: the consumers' and DLQ producer's brokers.
    pub kafka_bootstrap: String,
    /// `USER_SERVICE_URL`: the recipient lookups' user-service.
    pub user_url: String,
    /// `ORDER_SERVICE_URL`: the recipient lookups' order-service.
    pub order_url: String,
    /// `OTEL_*`.
    pub tracing: TracingConfig,
}

/// Reads the config from an environment map.
///
/// # Errors
/// [`ConfigError`] listing every invalid variable.
pub fn from_env<S: BuildHasher>(env: &HashMap<String, String, S>) -> Result<Config, ConfigError> {
    let mut problems = Vec::new();
    let text = |name: &str, default: &str| env.get(name).map_or(default, String::as_str).to_owned();
    let http_port = port(env, "NOTIFICATION_SERVICE_PORT", 8080, &mut problems);
    let postgres = postgres(env, "notification", &mut problems);
    let tracing = tracing_from_env(env, &mut problems);
    if !problems.is_empty() {
        return Err(ConfigError { problems });
    }
    Ok(Config {
        port: http_port,
        service_name: text("NOTIFICATION_SERVICE_NAME", "notification-service"),
        postgres,
        kafka_bootstrap: text("KAFKA_BOOTSTRAP_SERVERS", "localhost:9092"),
        user_url: text("USER_SERVICE_URL", "http://localhost:8084"),
        order_url: text("ORDER_SERVICE_URL", "http://localhost:8083"),
        tracing,
    })
}
