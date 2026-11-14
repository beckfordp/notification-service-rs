// SPDX-License-Identifier: MIT

//! The `notification-service` binary: reads the config contract, migrates the
//! database, then serves until SIGTERM or SIGINT.

use std::process::ExitCode;

use notification_service::config::{self, Config};
use notification_service::{db, server};
use service_basics::{environment, runtime, shutdown};

fn main() -> ExitCode {
    let config = match config::from_env(&environment::variables()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("notification-service: {error}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match runtime::multi_thread() {
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
    match server::run(config.port, pool, shutdown::signal("notification-service")).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("notification-service: {error}");
            ExitCode::FAILURE
        }
    }
}
