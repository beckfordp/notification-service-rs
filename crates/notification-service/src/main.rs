// SPDX-License-Identifier: MIT

//! The `notification-service` binary: reads the config contract, migrates the
//! database, then serves until SIGTERM or SIGINT.

use std::collections::HashMap;
use std::process::ExitCode;

use notification_service::config::{self, Config};
use notification_service::{db, server};

fn main() -> ExitCode {
    let env: HashMap<String, String> = std::env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .collect();
    let config = match config::from_env(&env) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("notification-service: {error}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("notification-service: can't start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(serve(&config))
}

async fn serve(config: &Config) -> ExitCode {
    let pool = match db::connect(&config.postgres).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!(
                "notification-service: database {}:{}/{} unreachable: {error}",
                config.postgres.host, config.postgres.port, config.postgres.database
            );
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = db::migrate(&pool).await {
        eprintln!("notification-service: database migration failed: {error}");
        return ExitCode::FAILURE;
    }
    println!("notification-service starting on port {}", config.port);
    match server::run(config.port, pool, shutdown_signal()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("notification-service: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Completes on SIGTERM (how the harness and Kubernetes stop a process) or SIGINT.
async fn shutdown_signal() {
    let interrupt = tokio::signal::ctrl_c();
    let mut terminate =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(error) => {
                eprintln!("notification-service: can't listen for SIGTERM: {error}");
                let _ = interrupt.await;
                return;
            }
        };
    tokio::select! {
        _ = interrupt => {}
        _ = terminate.recv() => {}
    }
}
