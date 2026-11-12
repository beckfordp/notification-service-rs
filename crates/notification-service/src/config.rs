// SPDX-License-Identifier: MIT

//! The config contract: the Scala service's environment variables and
//! defaults (notification-service `application.conf`). Pure: an environment
//! map in, a [`Config`] or every problem out. The walking skeleton (RPN-2)
//! reads only the HTTP port and Postgres; the rest arrives with its feature.

use std::collections::HashMap;
use std::fmt;
use std::hash::BuildHasher;

/// Everything the service reads from its environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `NOTIFICATION_SERVICE_PORT`: the HTTP API.
    pub port: u16,
    /// `POSTGRES_*`.
    pub postgres: PostgresConfig,
}

/// Where the service's database is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresConfig {
    /// `POSTGRES_HOST`.
    pub host: String,
    /// `POSTGRES_PORT`.
    pub port: u16,
    /// `POSTGRES_DB`.
    pub database: String,
    /// `POSTGRES_USER`.
    pub user: String,
    /// `POSTGRES_PASSWORD`.
    pub password: String,
}

/// Every problem with the environment, so one fix-up round is enough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// One line per invalid variable.
    pub problems: Vec<String>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid configuration:")?;
        for problem in &self.problems {
            write!(f, "\n- {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

/// Reads the config from an environment map.
///
/// # Errors
/// [`ConfigError`] listing every invalid variable.
pub fn from_env<S: BuildHasher>(env: &HashMap<String, String, S>) -> Result<Config, ConfigError> {
    let mut problems = Vec::new();
    let text = |name: &str, default: &str| env.get(name).map_or(default, String::as_str).to_owned();
    let mut port = |name: &str, default: u16| match env.get(name) {
        None => default,
        Some(raw) => raw.trim().parse::<u16>().unwrap_or_else(|_| {
            problems.push(format!(
                "{name} must be a port number (0-65535), got {raw:?}"
            ));
            default
        }),
    };
    let http_port = port("NOTIFICATION_SERVICE_PORT", 8080);
    let postgres_port = port("POSTGRES_PORT", 5432);

    if !problems.is_empty() {
        return Err(ConfigError { problems });
    }
    Ok(Config {
        port: http_port,
        postgres: PostgresConfig {
            host: text("POSTGRES_HOST", "localhost"),
            port: postgres_port,
            database: text("POSTGRES_DB", "notification"),
            user: text("POSTGRES_USER", "notification"),
            password: text("POSTGRES_PASSWORD", "notification"),
        },
    })
}
