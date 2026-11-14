// SPDX-License-Identifier: MIT

//! The walking skeleton's health exchanges and `sent_email` schema equal the
//! Scala service's (capture 20261111, vendored unchanged in `tests/fixtures`;
//! was `scripts/verify-skeleton.sh`, gluon TD-46).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use support::{Postgres, Service, canon_ddl, canon_exchange};

const HEALTH: &str = include_str!("fixtures/http/health.txt");
const HEALTH_READY: &str = include_str!("fixtures/http/health-ready.txt");
const HEALTH_HEAD: &str = include_str!("fixtures/http/health-head.txt");
const SCHEMA: &str = include_str!("fixtures/db/schema.sql");

#[test]
fn the_health_exchanges_are_the_captured_ones() {
    let postgres = Postgres::start();
    let service = Service::start(&postgres.env());
    for (method, path, captured) in [
        ("GET", "/health", HEALTH),
        ("GET", "/health/ready", HEALTH_READY),
        ("HEAD", "/health", HEALTH_HEAD),
    ] {
        assert_eq!(
            canon_exchange(&service.exchange(method, path)),
            canon_exchange(captured),
            "{method} {path} (Rust left, capture right)"
        );
    }
}

#[test]
fn the_sent_email_ddl_is_the_captured_one() {
    let postgres = Postgres::start();
    let _service = Service::start(&postgres.env());
    assert_eq!(
        canon_ddl(&postgres.pg_dump("sent_email")),
        canon_ddl(SCHEMA),
        "pg_dump -t sent_email (Rust left, capture right)"
    );
}
