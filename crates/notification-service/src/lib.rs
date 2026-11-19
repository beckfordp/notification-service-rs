// SPDX-License-Identifier: MIT

//! The notification-service binary: the wire-compatible port of the Scala
//! notification-service (gluon ADR 0029).

pub mod config;
pub mod consumers;
pub mod cors;
pub mod db;
pub mod domain;
pub mod email;
pub mod emails;
pub mod ids;
pub mod lifecycle;
pub mod server;
pub mod store;
pub mod user_events;

/// The package version, as built.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
