# Plan — the user-events consumer (gluon RPN-11)

Spec: [spec.md](./spec.md). Acceptance, written first: gluon
`bin/verify-complete-RPN-11.sh` (red at the start: 4 of 9; verify-port
`notification-service --only build contract` 24/30, `user_events` 0/6).
Each task: tests red first, then green; its diff and message confirmed by the
owner before the commit. Each phase ends with the owner's review.

## Phase 1: The audited send `[checkpoint: cd76d16]`

- [x] Task 1.1: The nine audited-send behaviours (spec's table) `cd76d16`
  - Red: `tests/audited_send.rs`, one test per behaviour, against a throwaway
    Postgres (gluon-rs `service-testing`), with a recording transport and fake
    resolvers.
  - Green: `store` — an `EmailAudit` trait beside `SentEmails` (`claim`,
    `mark_resolved`, `mark_sent`, `mark_failed`, `mark_unresolved`; Scala's SQL)
    on `PgSentEmails`; `email` — `EmailMessage`, `SendOutcome`, `Transport` (the
    logging stub, `EmailClient.logging`), `Resolver` (`CarriedAddress` over a
    fallback; `NoLookup` fails until the lookups arrive), and `Audited::send`
    (`EmailClient.audited`).

## Phase 2: The UserRegistered route `[checkpoint: eff889b]`

- [x] Task 2.1: `user_events` — the `UserRegistered` payload, the welcome email `eff889b`
  (`emailFor`), the route (gluon-rs `Routes`: only `UserRegistered`; a payload
  that doesn't decode is poison), and the outcome log lines (`EmailSendLog`).
  Red first: `tests/user_events.rs` with a fake sender.

## Phase 3: Wiring `[checkpoint: 341fa3e]`

- [x] Task 3.1: Config — `KAFKA_BOOTSTRAP_SERVERS` (default `localhost:9092`), `813f223`
  `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_TRACES_SAMPLER_ARG` (copied from
  user-service-rs: TECHNICAL-DEBT S-2). Red first in `tests/config.rs`.
- [x] Task 3.2: The consumer beside the HTTP server — tracing, logger, `341fa3e`
  metrics; gluon-rs `KafkaPublisher` → `DeadLetter` (dlq-retry 3 × 200 ms;
  publish 3 × 100 ms, breaker 5 / 30 s) → `EventConsumer` on
  `notification-service-user-events` → `supervise` (1 s doubling to 30 s);
  SIGTERM/SIGINT stops both; the consumer stopping otherwise exits non-zero.
  Red → green: verify-port `notification-service --only build contract` 30/30.

## Phase 4: Close `[checkpoint: archive commit]`

- [x] Task 4.1: Docs — `tech-stack.md` dated notes (Kafka, telemetry, the
  audited send), README status, gluon `TECHNICAL-DEBT.md` brought up to date. `0f51ecd`
- [x] Task 4.2: Archive the track; `bin/verify-complete-RPN-11.sh` PASS.

## Archive (2026-11-18)

Done: tests/contract's user-events group green against the Rust build (6/6;
`bin/verify-port notification-service --only build contract` 30/30, C1 MET),
the audited send's nine behaviours tested (`tests/audited_send.rs`), CI green.
No track script to retire (workflow.md "Retiring the script"): the acceptance
is the harness's user-events group (C1), `scripts/ci.sh`, and gluon
`bin/verify-complete-RPN-11.sh`. Copies accepted to reach done, and other
improvements noticed, are gluon `TECHNICAL-DEBT.md` S-1 to S-8, for review at
the session's end. The code review against the Rust guidelines is deferred to
the end-of-session review (owner, 2026-11-18).
