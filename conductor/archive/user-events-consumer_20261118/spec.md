# Spec — the user-events consumer

Track `user-events-consumer_20261118`. Gluon plan item: **RPN-11** (RPN-10 wrote
the checks red). Status: agreed 2026-11-18; done when gluon
`bin/verify-complete-RPN-11.sh` passes.

## Goal

notification-service-rs consumes `user-events` as the Scala service's
`UserRegisteredConsumer` does: a `UserRegistered` event sends the welcome email,
audited in `sent_email`; everything else on the topic is skipped or
dead-lettered to `user-events-dlq` exactly as the capture records.

## Acceptance (written first: RPN-10)

The acceptance test already exists and is red:

```
tests/contract$ scripts/verify-check-group.sh checks/test_notification_user_events.py \
    --service notification-service --expect-red "not yet implemented"
```

passes today (6/6 PENDING: no consumer ever joins
`notification-service-user-events`). The track is done when:

1. `... --expect-green` passes: all six checks green.

   | # | Check | Record (capture `CAPTURE.sh`) | Outcome |
   |---|---|---|---|
   | 0 | `test_a_user_registered_event_sends_the_welcome_email` | welcome email | one `sent_email` row equal to the capture's (recipient type `email`, status `sent`, `sent_at` set); no `GET /users/` |
   | 1 | `test_a_repeated_event_id_sends_no_second_email` | duplicate eventId | still one row (eventId is the idempotency key) |
   | 2 | `test_an_unrelated_event_type_is_skipped` | `UserDeleted` | no row, no dead letter |
   | 3 | `test_an_undecodable_envelope_is_dead_lettered_unchanged` | `{not json` | on `user-events-dlq` once, key and value as sent, no headers |
   | 4 | `test_an_undecodable_payload_is_dead_lettered_unchanged` | `UserRegistered` without `email` | on the DLQ once, whole envelope, no headers; no row |
   | 5 | `test_a_null_value_is_skipped` | null value | no row, no dead letter; the group has committed 6 of 6 |

2. The REST group stays green: `checks/test_notification_rest.py --expect-green` (24/24).
3. `scripts/ci.sh` passes (fmt, clippy `-D warnings`, tests, line coverage ≥ 80 %).
4. The acceptance as gluon runs it: `bin/verify-port notification-service --only
   build contract` prints `ok   contract: 30/30 green` and C1 MET (24/30 at the start).
5. The nine audited-send behaviours below, one Rust test each in
   `tests/audited_send.rs`.
6. tests/contract and gluon-rs unchanged (no harness change; duplication accepted,
   ideas go to gluon `TECHNICAL-DEBT.md`).

All of it is `bin/verify-complete-RPN-11.sh`, written before the work (red: 4 of 9).

## Reference behaviour

Scala `notification-service`: `UserRegisteredConsumer`, `EnvelopeHandler`,
`KafkaConsumers` (supervision, `watched`), `EmailClient.audited` and
`EmailClient.logging`, `SentEmailStore.claim` / `markResolved` / `markSent` /
`markFailed` / `markUnresolved`, `RecipientResolver` (the address-already-known
branch only), `application.conf` `kafka.*`. Capture
`tests/contract/contracts/notification-service/20261111`.

## What already exists (reuse, don't rewrite)

- gluon-rs `kafka-event-consumer`: `Envelope`, `Routes`, `handle_record` (null →
  skip, poison → DLQ, unrouted → skip, routed → inside the DLQ guard),
  `DeadLetter`, `supervise`, `ConsumerMetrics`, `consumer_span`.
- gluon-rs `kafka-client`: `EventConsumer` (consume loop, at-least-once commit
  after handling, earliest offset), `KafkaPublisher` (DLQ producer, no headers),
  `ConsumerSettings`.
- gluon-rs `telemetry`: `Tracing`, `Logger`, `Metrics`, `resilience`.
- gluon-rs `service-basics`: config helpers, shutdown signal.
- This repo: `domain::{SentEmail, RecipientType, EmailStatus}`, `store::SentEmails`
  (list, ping), migrations V1–V2.

The service's own new code should be: the `UserRegistered` route, the audited
send (store claim/mark + logging transport), `KAFKA_BOOTSTRAP_SERVERS` in config,
and running the consumer beside the HTTP server.

## The audited send's nine behaviours

Scala `EmailClient.audited` and `SentEmailStore.claim`. Only 2, 4 and 5 can be
driven through user-events (`UserRegistered` always carries its address), and the
user-events group already checks them; 3, 8 and 9 become contract checks with the
order/payment/fulfilment groups; 1, 6 and 7 aren't in the capture (TECHNICAL-DEBT
S-6). All nine are Rust tests here.

| # | Behaviour | Rust test |
|---|---|---|
| 1 | A new event is claimed: a `pending` row before anything else | `a_new_event_is_claimed_pending_before_the_send` |
| 2 | An address the event carries is used as-is, no lookup | `a_carried_address_is_used_without_a_lookup` |
| 3 | The resolved recipient is recorded before the send (survives a failure) | `the_resolved_recipient_is_recorded_before_the_send` |
| 4 | Sent through the transport, then `sent` with `sent_at` | `a_sent_email_is_marked_sent_with_sent_at` |
| 5 | A duplicate of a `sent` event: no write, no send | `a_duplicate_of_a_sent_event_is_not_sent_again` |
| 6 | A duplicate of an `unresolved` event: no write, no send | `a_duplicate_of_an_unresolved_event_is_not_sent_again` |
| 7 | A `pending`/`failed` row is reset and retried on redelivery | `a_pending_or_failed_row_is_retried_on_redelivery` |
| 8 | No address: `unresolved` with the reason, nothing sent, not raised | `an_unresolvable_recipient_is_marked_unresolved_and_not_raised` |
| 9 | A transient failure: `failed`, `failed_at`, the error, raised for retry/DLQ | `a_transient_failure_is_marked_failed_and_raised_for_retry` |

## Design decisions (agreed 2026-11-18)

- **Checks:** the user-events group stays at its six checks; no separate
  audited-sends group, no harness change.
- **Duplication:** accepted this session to reach done; improvements go to gluon
  `TECHNICAL-DEBT.md` for review at the end. So the consumer wiring is
  assembled in the service as the probe does (S-1), and the OTEL variables are
  read by a copy of user-service-rs's reader (S-2).
- **Telemetry:** the minimum the consumer needs — `Tracing` (OTLP to the
  endpoint given), `Logger::stdout`, in-process `Metrics`; no `/metrics`
  endpoint, HTTP spans or HTTP log format (the observability track).
- **Audited send:** the whole state machine, in the service (only it sends
  email), behind a resolver that uses an address the event carries and
  otherwise defers to a fallback; here the fallback fails (no lookups yet: they
  come with the order-events consumer). The transport is the logging stub.
- **Payload type:** the service defines its own `UserRegistered`.
- **Lifecycle:** as `KafkaConsumers.watched`: the consumer runs beside the HTTP
  server; SIGTERM/SIGINT stops both; the consumer stopping otherwise exits the
  process non-zero.
- **Config:** `KAFKA_BOOTSTRAP_SERVERS` (default `localhost:9092`) and the OTEL
  variables; `dlq-retry`, `dlq-publish-resilience` and `stream-supervisor` are
  constants with `application.conf`'s values (not env-overridable in Scala).

## Noticed, out of scope

In gluon `TECHNICAL-DEBT.md` (S-1 to S-6).

## Out of scope

order-events, payment-events and fulfilment-events consumers; recipient lookups;
`/metrics`, logs and traces contract groups; AsyncAPI docs; deployment (RPN-L…).
