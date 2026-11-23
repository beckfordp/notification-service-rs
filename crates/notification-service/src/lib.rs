// SPDX-License-Identifier: MIT

//! The notification-service binary: the wire-compatible port of the Scala
//! notification-service (gluon ADR 0029).

pub mod clients;
pub mod config;
pub mod consumers;
pub mod db;
pub mod docs;
pub mod domain;
pub mod email;
pub mod emails;
pub mod fulfilment_events;
pub mod ids;
pub mod lifecycle;
pub mod order_events;
pub mod payment_events;
pub mod recipients;
pub mod server;
pub mod store;
pub mod user_events;

/// The package version, as built.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
