// SPDX-License-Identifier: MIT

//! The `sent_email` schema equals the Scala service's (capture 20261111,
//! vendored unchanged in `tests/fixtures`; moved from the walking skeleton's
//! track script, gluon TD-46). The health exchanges are the harness's now
//! (gluon TD-42: tests/contract `test_notification_rest_wire.py`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use support::{Postgres, Service, canon_ddl};

const SCHEMA: &str = include_str!("fixtures/db/schema.sql");

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
