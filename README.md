# notification-service-rs

A wire-compatible Rust port of [notification-service](../notification-service),
the second Rust port per gluon
[ADR 0029](../../docs/adr/0029-after-the-rust-spike.md) (option 3, edges
inward). It is a drop-in replacement: same REST, docs, Kafka consumers, DLQs,
schema, config and observability contracts. The Scala service stays the
default.

**Status (2026-11-18):** the walking skeleton (RPN-2) migrates `sent_email`
and answers `/health` and `/health/ready`; RPN-5 adds ADM-2's read-only
`GET /emails?customerId=&orderId=` and the Scala service's CORS policy, so the
harness's REST group (tests/contract `checks/test_notification_rest_wire.py`) is
green, 24/24. RPN-11 adds the user-events consumer: `UserRegistered` sends
the welcome email through the audited send every consumer will share
(`sent_email` claimed, resolved, sent through the logging stub, marked), the
rest skipped or dead-lettered to `user-events-dlq` as the Scala service does,
so the harness's user-events group is green too, 6/6 (30/30 with REST).
`scripts/ci.sh` is CI (`tests/capture.rs` compares the
`sent_email` DDL with the capture, vendored in
`crates/notification-service/tests/fixtures`).
Work is planned in [`conductor/tracks.md`](./conductor/tracks.md) and
sequenced in gluon [`PLAN.md`](../../PLAN.md) "Rust Port —
notification-service". The reference behaviour is the capture
`tests/contract/contracts/notification-service/20261111`.
