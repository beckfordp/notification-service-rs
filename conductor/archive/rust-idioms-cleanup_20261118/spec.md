# Spec — RPN-11 cleanup: rust.md compliance, idiomatic Rust

Track `rust-idioms-cleanup_20261118`, gluon **RPN-11-cleanup** (with gluon-rs's
`shared-consumer-otel_20261118`). Agreed with the owner 2026-11-18: the "fix now"
items 1–7 of gluon `TECHNICAL-DEBT.md` (the RPN-11 code review against
`conductor/code_styleguides/rust.md` and the comparison with user-service-rs);
where a choice arises, idiomatic Rust over a straight Scala port.

## Acceptance

gluon `bin/verify-complete-RPN-11.sh` (written first, red), criteria 1–8
here; behaviour unchanged: the contract stays 30/30, RPN-11's nine audited-send
tests keep their names, CI green.

## Changes

| Item | Change |
|---|---|
| 1 H3 | `match` on `Claim`, exhaustive (no `let … else`). |
| 2 H1 H4 | `domain::welcome_message(event_id, user_id, email)`, `domain::carried_address`, `EmailMessage::addressed_to` (pure); row ids from an injected `IdGen` (`ids.rs`: `RandomIds`). |
| 3 H2 L1 L2 | `SendError { Store, Lookup, Transport }` (thiserror); `Resolver` returns `Recipient::{Found, Unknown}` (what it found, not a copy of the message); one `send`, in `EmailClient for Audited`; strings only at gluon-rs's `Routes` boundary and in the row's `error`. |
| 4 H5 S-8 M3 | `main` builds the audited client and passes it to `consumers::run`; the startup line is JSON (`notification-service starting`, `port`, `metrics_port`), as the capture's; every task result on shutdown is logged. |
| 5 M5 | `tests/audited_failures.rs`: an in-memory `EmailAudit` with failure switches (user-service-rs's pattern), each store failure of the audited send. |
| 6 M4 | The Postgres store helper and the log-buffer helpers in `tests/support`. |
| 7 L5 | `tech-stack.md`: `now()` in SQL is deliberate parity. |

Out of scope: the later items (TECHNICAL-DEBT 9–16), the other direction (U-1–U-6).
