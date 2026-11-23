// SPDX-License-Identifier: MIT

//! The `notification-service` binary: reads the config contract, migrates the
//! database, then serves the HTTP API and runs the Kafka consumers until
//! SIGTERM or SIGINT. A consumer or server that stops by itself stops the
//! service, non-zero (Scala `KafkaConsumers.watched`): a pod answering
//! `/health` with no consumer is what that prevents.

use std::process::ExitCode;

use kafka_client::supervised::ConsumerTelemetry;
use notification_service::clients::{self, Orders, Users};
use notification_service::config::{self, Config};
use notification_service::consumers;
use notification_service::email::{Audited, CarriedAddress, LoggingTransport};
use notification_service::ids::RandomIds;
use notification_service::recipients::Lookups;
use notification_service::store::PgSentEmails;
use notification_service::{db, lifecycle, server};
use service_basics::{environment, runtime, shutdown};
use telemetry::logging::Logger;
use telemetry::metrics::Metrics;
use telemetry::tracing::{SdkTracer, Tracing};
use tokio::sync::watch;

fn main() -> ExitCode {
    let config = match config::from_env(&environment::variables()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{SERVICE}: {error}");
            return ExitCode::FAILURE;
        }
    };
    // Built outside the async runtime: the OTLP exporter's blocking HTTP
    // client must not be created (or dropped) on a runtime thread.
    let tracing = match Tracing::new(
        &config.service_name,
        config.tracing.otlp_endpoint.as_deref(),
        config.tracing.sampling_ratio,
    ) {
        Ok(tracing) => tracing,
        Err(error) => {
            eprintln!("{SERVICE}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match runtime::multi_thread() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{SERVICE}: can't start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    let code = runtime.block_on(serve(&config, tracing.tracer()));
    drop(runtime);
    tracing.shutdown();
    code
}

/// The service's name in its stderr lines and SIGTERM warning.
const SERVICE: &str = "notification-service";

async fn serve(config: &Config, tracer: SdkTracer) -> ExitCode {
    let pool = match db::connect(&config.postgres).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!(
                "{SERVICE}: database {}:{}/{} unreachable: {error}",
                config.postgres.host, config.postgres.port, config.postgres.database
            );
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = db::migrate(&pool).await {
        eprintln!("{SERVICE}: database migration failed: {error}");
        return ExitCode::FAILURE;
    }
    let logger = Logger::stdout(&config.service_name);
    logger.info(
        &[("port", &config.port.to_string())],
        "notification-service starting",
    );
    let metrics = Metrics::new(&config.service_name);
    let lookups = match (
        clients::resilient(tracer.clone(), &metrics),
        clients::resilient(tracer.clone(), &metrics),
    ) {
        (Ok(users), Ok(orders)) => Lookups::new(
            Users::new(&config.user_url, users),
            Orders::new(&config.order_url, orders),
        ),
        (Err(error), _) | (_, Err(error)) => {
            eprintln!("{SERVICE}: can't build the HTTP client: {error}");
            return ExitCode::FAILURE;
        }
    };
    let store = PgSentEmails::new(pool);
    let client = Audited::new(
        store.clone(),
        LoggingTransport::new(logger.clone()),
        CarriedAddress::new(lookups),
        RandomIds,
        logger.clone(),
    );
    let telemetry = ConsumerTelemetry {
        tracer,
        logger: logger.clone(),
        metrics,
    };
    let (stop, stopped) = watch::channel(false);
    let server = {
        let (port, mut stopped) = (config.port, stopped.clone());
        async move {
            let until_stopped = async move {
                let _ = stopped.wait_for(|stop| *stop).await;
            };
            server::run(port, store, until_stopped)
                .await
                .map_err(|error| error.to_string())
        }
    };
    let consumers = {
        let bootstrap = config.kafka_bootstrap.clone();
        async move {
            // Four supervised consumers: boxed, not held inline in `serve`.
            Box::pin(consumers::run(&bootstrap, client, telemetry, stopped))
                .await
                .map_err(|error| error.to_string())
        }
    };
    let signal = shutdown::signal(SERVICE);
    if lifecycle::run_until_stopped(server, consumers, signal, stop, &logger).await {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
