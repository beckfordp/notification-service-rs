// SPDX-License-Identifier: MIT

//! The config contract: the Scala service's environment variables and
//! defaults (notification-service `application.conf`). Pure: an environment
//! map in, a [`Config`] or every problem out; the pieces every service reads
//! the same way are gluon-rs `service-basics`. The walking skeleton (RPN-2)
//! reads only the HTTP port and Postgres; the rest arrives with its feature.

use std::collections::HashMap;
use std::hash::BuildHasher;

pub use service_basics::config::{ConfigError, PostgresConfig};
use service_basics::config::{port, postgres};

/// Everything the service reads from its environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `NOTIFICATION_SERVICE_PORT`: the HTTP API.
    pub port: u16,
    /// `POSTGRES_*`.
    pub postgres: PostgresConfig,
}

/// Reads the config from an environment map.
///
/// # Errors
/// [`ConfigError`] listing every invalid variable.
pub fn from_env<S: BuildHasher>(env: &HashMap<String, String, S>) -> Result<Config, ConfigError> {
    let mut problems = Vec::new();
    let http_port = port(env, "NOTIFICATION_SERVICE_PORT", 8080, &mut problems);
    let postgres = postgres(env, "notification", &mut problems);
    if !problems.is_empty() {
        return Err(ConfigError { problems });
    }
    Ok(Config {
        port: http_port,
        postgres,
    })
}
