// SPDX-License-Identifier: MIT

//! Shared by the integration tests: gluon-rs `service-testing`'s throwaway
//! PostgreSQL cluster, and this service's binary under test.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ops::{Deref, DerefMut};

// Not every test file uses both.
#[allow(unused_imports)]
pub use service_testing::{Postgres, free_port};

/// The notification-service binary, on a fresh `NOTIFICATION_SERVICE_PORT`.
pub struct Service(service_testing::Service);

impl Service {
    /// Starts it with exactly `extra` (plus its port); does not wait.
    pub fn spawn(extra: &[(&str, String)]) -> Self {
        Self(service_testing::Service::spawn(
            env!("CARGO_BIN_EXE_notification-service"),
            "NOTIFICATION_SERVICE_PORT",
            extra,
        ))
    }

    /// Starts it and waits until `/health` answers.
    pub fn start(extra: &[(&str, String)]) -> Self {
        Self(service_testing::Service::start(
            env!("CARGO_BIN_EXE_notification-service"),
            "NOTIFICATION_SERVICE_PORT",
            extra,
        ))
    }
}

impl Deref for Service {
    type Target = service_testing::Service;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Service {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A `pg_dump`'s statements about `sent_email`, whitespace collapsed and
/// sorted: no comments, `\restrict`, settings or ownership.
pub fn canon_ddl(dump: &str) -> Vec<String> {
    let kept: Vec<&str> = dump
        .lines()
        .filter(|line| {
            !["--", "\\", "SET ", "SELECT pg_catalog."]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect();
    let mut statements: Vec<String> = kept
        .join("\n")
        .split(';')
        .map(|statement| statement.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|statement| statement.contains("sent_email") && !statement.contains(" OWNER TO "))
        .collect();
    statements.sort();
    statements
}
