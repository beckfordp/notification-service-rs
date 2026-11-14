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
- Shared crates come from `lib/gluon-rs` by path once extracted: `telemetry`
  (RPN-7), `gluon-kafka` (RPN-8). Until then, none (RPN-2 has no shared
  crates).

## HTTP
- axum + tower / tower-http (CORS with the Scala service's exact policy)
- serde / serde_json
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

## Messaging
- rdkafka (librdkafka), via `gluon-kafka` from RPN-8: the envelope (ADR 0009),
  bounded retry then `<topic>-dlq`, stream supervision with backoff, the
  Scala service's settings (`dlq-retry`, `dlq-publish-resilience`,
  `stream-supervisor`).

## Observability
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
- CI: `scripts/ci.sh`, run directly in the sandbox and by GitHub Actions.
