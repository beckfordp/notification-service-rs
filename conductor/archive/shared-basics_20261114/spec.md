# Spec: On lib/gluon-rs (gluon TD-43.1, TD-47)

## Overview
notification-service-rs drops its copies of the shared pieces and uses
`lib/gluon-rs` by path. No behaviour change.

## Functional Requirements
- FR-1 `main.rs`: `environment::variables`, `runtime::multi_thread`,
  `shutdown::signal("notification-service")`.
- FR-2 `config.rs` keeps `Config` and `NOTIFICATION_SERVICE_PORT`; uses
  `config::port`, `config::postgres(env, "notification")` and `ConfigError`
  (re-exported, so `config::ConfigError` still resolves).
- FR-3 `db.rs` keeps `migrate`; `connect(config, POOL_SIZE)`, with
  `POOL_SIZE = 10` citing Scala `SentEmailStore` (`.pooled(max = 10)`, TD-47);
  its ping goes.
- FR-4 `server.rs`: `health::routes` and text_error's 404/405; the router
  state implements `Readiness` through `postgres::ping` (2 s).
- FR-5 `tests/support/mod.rs`: a `Service` newtype over
  `service_testing::Service` (`Deref`), with `start`/`spawn(extra)` setting
  `NOTIFICATION_SERVICE_PORT`; `Postgres` and `free_port` re-exported;
  `canon_ddl` stays. The test files are unchanged.

## Acceptance Criteria
- `scripts/ci.sh` green.
- `git grep 'as user-service-rs'` finds nothing; no copy of a moved piece left.
- The harness REST group against this build: still 5 green, 19 red.

## Out of Scope
- `/emails` (RPN-5).
- The CI workflow's checkout of gluon-rs (with user-service's track; GitHub
  later).
