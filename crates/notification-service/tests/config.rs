// SPDX-License-Identifier: MIT

//! The config contract: the Scala service's environment variables and defaults.

use std::collections::HashMap;

use notification_service::config::{Config, PostgresConfig, from_env};

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn defaults_match_the_scala_service() {
    assert_eq!(
        from_env(&HashMap::new()).unwrap(),
        Config {
            port: 8080,
            postgres: PostgresConfig {
                host: "localhost".to_owned(),
                port: 5432,
                database: "notification".to_owned(),
                user: "notification".to_owned(),
                password: "notification".to_owned(),
            },
        }
    );
}

#[test]
fn every_variable_overrides_its_default() {
    let config = from_env(&env(&[
        ("NOTIFICATION_SERVICE_PORT", "18080"),
        ("POSTGRES_HOST", "127.0.0.1"),
        ("POSTGRES_PORT", "15432"),
        ("POSTGRES_DB", "db"),
        ("POSTGRES_USER", "u"),
        ("POSTGRES_PASSWORD", "p"),
    ]))
    .unwrap();
    assert_eq!(
        config,
        Config {
            port: 18080,
            postgres: PostgresConfig {
                host: "127.0.0.1".to_owned(),
                port: 15432,
                database: "db".to_owned(),
                user: "u".to_owned(),
                password: "p".to_owned(),
            },
        }
    );
}

#[test]
fn every_bad_port_is_reported_at_once() {
    let error = from_env(&env(&[
        ("NOTIFICATION_SERVICE_PORT", "http"),
        ("POSTGRES_PORT", "70000"),
    ]))
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid configuration:\n\
         - NOTIFICATION_SERVICE_PORT must be a port number (0-65535), got \"http\"\n\
         - POSTGRES_PORT must be a port number (0-65535), got \"70000\""
    );
}

#[test]
fn the_version_is_the_package_version() {
    assert_eq!(notification_service::version(), "0.1.0");
}
