// SPDX-License-Identifier: MIT

//! The notification-service binary: the wire-compatible port of the Scala
//! notification-service (gluon ADR 0029).

pub mod config;
pub mod db;
pub mod server;

/// The package version, as built.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
