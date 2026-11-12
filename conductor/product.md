# Initial Concept

A wire-compatible Rust port of notification-service, the second Rust port per
gluon ADR 0029 (option 3, edges inward) and PLAN.md "Rust Port —
notification-service". It is a drop-in replacement: same contracts, chosen in
gluon-local by an implementation switch; the Scala service stays the default.

# Product Guide — notification-service-rs

## Context
Full cross-repo vision lives in the `gluon` platform repo, not here:
[`../../../docs/adr/0029-after-the-rust-spike.md`](../../../docs/adr/0029-after-the-rust-spike.md)
(the decision this repo exists for),
[`../../../docs/rust-spike-findings.md`](../../../docs/rust-spike-findings.md)
(what the first port taught, and the exit criteria),
[`../../../docs/system-design.md`](../../../docs/system-design.md),
[`../../../PLAN.md`](../../../PLAN.md) ("Rust Port — notification-service").

The reference is the specified contracts: the documented contracts, the
Scala-generated OpenAPI/AsyncAPI artifacts, the Scala service's test suites
and `verify-*.sh` scripts in
[`../../notification-service`](../../notification-service), and the laptop
capture `tests/contract/contracts/notification-service/20261111`. Where this
service differs, this repo changes, never the contract.

## Purpose
Port a service that consumes events and calls other services, which
user-service didn't: four supervised consumers with DLQs, resilient HTTP
clients, an audited send. Shared Rust crates move to `lib/gluon-rs` as both
ports need them (RPN-7, RPN-8). The result is checked by the black-box
harness in `tests/contract` and judged against the exit criteria in
`docs/ports/notification-service.criteria.json`.

## Users
- The consultant (solo): builds it in the sandbox, runs it in gluon-local in
  place of the Scala service.
- gshop's Admin console (`GET /emails`), unchanged.
- The producers of `user-events`, `order-events`, `fulfilment-events` and
  `payment-events`, unchanged.

## What this service does
Same as the Scala notification-service (US-7.1, US-16.4, ADM-2, ADM-3):
- Consumes `user-events` (`UserRegistered` → welcome), `order-events`
  (`OrderConfirmed`, `OrderPaymentFailed`, `OrderCancelled`),
  `fulfilment-events` (`DispatchConfirmed`, `DispatchFailed`) and
  `payment-events` (`PaymentCaptured`, `PaymentFailed`), each in group
  `notification-service-<topic>`, supervised (restart with backoff), with
  bounded retry then `<topic>-dlq`.
- Resolves the recipient at send time (ADR 0025): a customerId through
  user-service `GET /users/{id}`, an orderId-only event through order-service
  `GET /orders/{id}` first; an unknown customer is terminal `unresolved`.
- Records each handled event in `sent_email`, keyed by the envelope
  `eventId`: claim, then sent / failed / unresolved; a redelivery never sends
  twice. Emails go to a logging transport (no real provider).
- `GET /emails?customerId=&orderId=` (read-only, Admin console).
- `/health`, `/health/ready` (503 when Postgres doesn't answer).
- The same docs: `/docs`, `/docs/docs.yaml`, `/docs/asyncapi.json`,
  `/docs/asyncapi/`.

## Domain model
- **Sent email** — one `sent_email` row per handled event: `source_event_id`
  (unique), event type, reason, recipient type/key/email, customer and order
  ids, subject, body, status (`pending`, `sent`, `failed`, `unresolved`) and
  its timestamps. Schema equal to the Scala service's V1 + V2.

## Sequencing (gluon `PLAN.md` "Rust Port — notification-service")
Sessions `RPN-N` in the sandbox, `RPN-L…` on the laptop, one at a time,
outside-in: each harness group is written in `tests/contract` and seen failing
before the session here that turns it green. Each session ends with a review,
a conductor track archived, and `bin/sandbox-save`.

## Out of scope
- Any change to a contract, or to another service.
- A real email provider.
- Migrating data between implementations: switching starts from a clean
  schema and reseeds.
