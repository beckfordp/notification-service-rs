# Tech Stack — notification-service-rs

Status (2026-11-12): proposed at conductor setup (RPN-L0). It follows
user-service-rs's [`tech-stack.md`](../../user-service-rs/conductor/tech-stack.md)
unless noted. Items marked **Open** are decided, and recorded here with a
dated note, in the session that first needs them, before any code depends on
them.

## Environment
Developed and tested in the sandbox: Rust via rustup, PostgreSQL 16 binaries,
Kafka (sandbox broker, RPN-9 fixtures), crates.io reachable; no Docker, no
GitHub, no Scala. gluon `plans/private/sandbox.md` (laptop only) records the sandbox; a tool added
here is added to gluon `bin/sandbox-setup` in the same change.

## Language / runtime
- Rust stable, edition 2024, toolchain pinned in `rust-toolchain.toml` to the
  same version as user-service-rs
- The same workspace lints as user-service-rs (`unsafe_code = "forbid"`,
  clippy `all` + `pedantic`, `unwrap_used`/`expect_used`/`panic` denied,
  `missing_docs` warns)
- tokio (multi-threaded runtime)

## Workspace layout
- `notification-service` — the service binary
- Shared crates come from `lib/gluon-rs` by path: `service-basics`
  (environment, shutdown, config, Postgres pool and ping, plain-text errors,
  health) and, for tests, `service-testing` *(2026-11-14, gluon TD-43.1)*;
  later `telemetry` (RPN-7) and `gluon-kafka` (RPN-8).
  *(2026-11-18, RPN-11: `telemetry`, `kafka-event-consumer` and
  `kafka-client` from gluon-rs, by path; "gluon-kafka" became those two
  crates.)*

## HTTP
- axum + tower / tower-http (CORS with the Scala service's exact policy)
  *(2026-11-15, RPN-5: CORS is the crate's own `cors` middleware, not
  tower-http's: http4s's `CORSPolicy` answers a refused preflight 200 with only
  `Vary` and leaves the no-route 404 alone, which tower-http's `CorsLayer`
  doesn't do; tower only in tests.)*
- serde / serde_json
- time and uuid for `sent_email`'s columns (sqlx `time`, `uuid`); instants
  printed by gluon-rs `service-basics::jvm::java_instant` *(2026-11-15, RPN-5)*
- Outbound: reqwest for user-service and order-service, with the Scala
  service's retry and circuit-breaker settings (`user-client`,
  `order-client`). **Open:** where the resilience wrapper lives (probably
  `gluon-rs`).

## Persistence
- sqlx (Postgres, tokio runtime)
- Migrations: the Scala service's V1 and V2 copied verbatim as sqlx
  migrations; the contract is `sent_email`, not Flyway's history table
  (RPN-2).
- One Postgres database (`notification`), one table `sent_email`.
- *(2026-11-18, RPN-11-cleanup)* `sent_at` and `failed_at` are set by SQL
  `now()`, the database's clock, as the Scala service's queries set them:
  deliberate parity, not an injected clock (`rust.md` §5; gluon
  TECHNICAL-DEBT.md L5). Row ids come from an injected `IdGen` (`ids.rs`).

## Messaging
- *(2026-11-18, RPN-11)* gluon-rs `kafka-client` (rdkafka: the consume loop,
  at least once, commit after handling; the DLQ producer) and
  `kafka-event-consumer` (envelope routing through `Routes`, the DLQ guard,
  `supervise`, age, lag, the consume span), assembled per consumer in
  `consumers.rs` as the events-probe does (a shared helper is gluon
  TECHNICAL-DEBT.md S-1). The Scala `kafka.*` settings are constants there
  (not environment-overridable in Scala either). First consumer: user-events
  (`user_events.rs`).
- Emails go through `email::Audited` (Scala `EmailClient.audited`: claim in
  `sent_email`, resolve, record, send through the logging stub, mark), shared
  by every consumer. Resolution uses an address the event carries
  (`CarriedAddress`); otherwise its fallback, `NoLookup` until the
  user-service and order-service lookups arrive, fails, so the record is
  retried and dead-lettered, not lost.
  *(2026-11-18, RPN-11-cleanup)* Its failures are typed (`SendError`:
  store, lookup, transport), made text only at gluon-rs's `Routes` boundary
  and for the row's `error`; a resolver returns the `Recipient` it found;
  `main` wires the client (transport, resolver, ids) and `consumers.rs` only
  the Kafka side.
  *(2026-11-18, RPN-11-could-fix)* Ids and addresses are newtypes
  (`EventId`, `CustomerId`, `OrderId`, `EmailAddress`: any text, as Scala's
  `String`s) and a message names its recipient with one `RecipientKey`; the
  read model (`SentEmail`, `/emails`) keeps the columns as text. Rows are read
  through a `sqlx::FromRow` row in `store.rs` (the enums by `FromStr`), so
  `domain` stays free of sqlx. The run-until-stopped lifecycle is
  `lifecycle::run_until_stopped`, tested with stand-in tasks.
- rdkafka (librdkafka), via `gluon-kafka` from RPN-8: the envelope (ADR 0009),
  bounded retry then `<topic>-dlq`, stream supervision with backoff, the
  Scala service's settings (`dlq-retry`, `dlq-publish-resilience`,
  `stream-supervisor`).

## Observability
- *(2026-11-18, RPN-11)* So far only what the consumer needs: `Tracing`
  (OTLP to `OTEL_EXPORTER_OTLP_ENDPOINT`, built outside the runtime),
  `Logger::stdout`, in-process `Metrics`. No `/metrics` endpoint, HTTP spans
  or HTTP log format yet: the observability session. The OTEL reader in
  `config.rs` is user-service-rs's, copied (TECHNICAL-DEBT S-2).
- The `telemetry` crate from user-service-rs (tracing, OTLP, Prometheus
  metrics on 9090, logstash-compatible JSON logs), `service.name =
  notification-service`.

## Config
- The Scala service's env vars: `NOTIFICATION_SERVICE_PORT`,
  `NOTIFICATION_SERVICE_METRICS_PORT`, `NOTIFICATION_SERVICE_NAME`,
  `POSTGRES_HOST/PORT/DB/USER/PASSWORD`, `KAFKA_BOOTSTRAP_SERVERS`,
  `USER_SERVICE_URL`, `ORDER_SERVICE_URL`, `ASYNCAPI_HTML_DIR`, plus the OTel
  vars. Same defaults.

## Testing
- `cargo test`; Postgres-backed tests against a throwaway PostgreSQL 16
  cluster (`initdb`/`pg_ctl`). No Testcontainers.
- Coverage with cargo-llvm-cov, gate `--fail-under-lines 80`, in
  `scripts/ci.sh` (RPN-2).
- The black-box harness in gluon `tests/contract`.

## Lint / format / audit
- rustfmt, clippy `-D warnings`, cargo-audit in CI (SKIP in the sandbox), as
  user-service-rs.

## Packaging / local deploy
- *(Laptop, `RPN-L…`)* Multi-stage Dockerfile →
  `notification-service-rs:latest`; deployed by gluon's generic Helm chart
  under the release `notification-service` with an implementation switch in
  `bin/k8s-local-up`.
- CI: `scripts/ci.sh`, run directly in the sandbox, on the laptop and by
  GitHub Actions: gluon-rs's shared steps (`scripts/lib/ci.sh`, Linux and
  macOS; `GLUON_RS` overrides `../../lib/gluon-rs`).
