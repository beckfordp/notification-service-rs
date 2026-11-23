// SPDX-License-Identifier: MIT

//! The config contract: the Scala service's environment variables and defaults.

use std::collections::HashMap;

use notification_service::config::{Config, PostgresConfig, TracingConfig, from_env};

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
            service_name: "notification-service".to_owned(),
            postgres: PostgresConfig {
                host: "localhost".to_owned(),
                port: 5432,
                database: "notification".to_owned(),
                user: "notification".to_owned(),
                password: "notification".to_owned(),
            },
            kafka_bootstrap: "localhost:9092".to_owned(),
            user_url: "http://localhost:8084".to_owned(),
            order_url: "http://localhost:8083".to_owned(),
            tracing: TracingConfig {
                otlp_endpoint: None,
                sampling_ratio: 1.0,
            },
        }
    );
}

#[test]
fn every_variable_overrides_its_default() {
    let config = from_env(&env(&[
        ("NOTIFICATION_SERVICE_PORT", "18080"),
        ("NOTIFICATION_SERVICE_NAME", "notification-service-rs"),
        ("POSTGRES_HOST", "127.0.0.1"),
        ("POSTGRES_PORT", "15432"),
        ("POSTGRES_DB", "db"),
        ("POSTGRES_USER", "u"),
        ("POSTGRES_PASSWORD", "p"),
        ("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:19092"),
        ("USER_SERVICE_URL", "http://user-service:8080"),
        ("ORDER_SERVICE_URL", "http://order-service:8080"),
        ("OTEL_EXPORTER_OTLP_ENDPOINT", " http://127.0.0.1:4318 "),
        ("OTEL_TRACES_SAMPLER_ARG", "0.25"),
    ]))
    .unwrap();
    assert_eq!(
        config,
        Config {
            port: 18080,
            service_name: "notification-service-rs".to_owned(),
            postgres: PostgresConfig {
                host: "127.0.0.1".to_owned(),
                port: 15432,
                database: "db".to_owned(),
                user: "u".to_owned(),
                password: "p".to_owned(),
            },
            kafka_bootstrap: "127.0.0.1:19092".to_owned(),
            user_url: "http://user-service:8080".to_owned(),
            order_url: "http://order-service:8080".to_owned(),
            tracing: TracingConfig {
                otlp_endpoint: Some("http://127.0.0.1:4318".to_owned()),
                sampling_ratio: 0.25,
            },
        }
    );
}

#[test]
fn every_bad_variable_is_reported_at_once() {
    let error = from_env(&env(&[
        ("NOTIFICATION_SERVICE_PORT", "http"),
        ("POSTGRES_PORT", "70000"),
        ("OTEL_TRACES_SAMPLER_ARG", "2"),
    ]))
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid configuration:\n\
         - NOTIFICATION_SERVICE_PORT must be a port number (0-65535), got \"http\"\n\
         - POSTGRES_PORT must be a port number (0-65535), got \"70000\"\n\
         - OTEL_TRACES_SAMPLER_ARG must be a number between 0 and 1, got \"2\""
    );
}

#[test]
fn blank_otel_variables_are_unset() {
    let config = from_env(&env(&[
        ("OTEL_EXPORTER_OTLP_ENDPOINT", "  "),
        ("OTEL_TRACES_SAMPLER_ARG", ""),
    ]))
    .unwrap();
    assert_eq!(
        config.tracing,
        TracingConfig {
            otlp_endpoint: None,
            sampling_ratio: 1.0,
        }
    );
}

#[test]
fn the_version_is_the_package_version() {
    assert_eq!(notification_service::version(), "0.1.0");
}
