# notification-service-rs

A wire-compatible Rust port of [notification-service](../notification-service),
the second Rust port per gluon
[ADR 0029](../../docs/adr/0029-after-the-rust-spike.md) (option 3, edges
inward). It is a drop-in replacement: same REST, docs, Kafka consumers, DLQs,
schema, config and observability contracts. The Scala service stays the
default.

**Status (2026-11-12):** walking skeleton (RPN-2): migrates `sent_email`,
answers `/health` and `/health/ready`; `scripts/ci.sh` (its `tests/capture.rs`
compares the health exchanges and schema with the capture, vendored in
`crates/notification-service/tests/fixtures`).
Work is planned in [`conductor/tracks.md`](./conductor/tracks.md) and
sequenced in gluon [`PLAN.md`](../../PLAN.md) "Rust Port —
notification-service". The reference behaviour is the capture
`tests/contract/contracts/notification-service/20261111`.
