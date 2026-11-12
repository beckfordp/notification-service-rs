# Spec — RPN-2: walking skeleton — migrations and health

## Overview
First code in notification-service-rs (gluon PLAN.md RPN-2): a binary that
migrates `sent_email` and answers the health probes as the Scala service does
in capture `tests/contract/contracts/notification-service/20261111`. Done on
the laptop: the sandbox session didn't run `bin/sandbox-save`, so its work was
lost (only the plan tick came back).

## Functional Requirements
1. Cargo workspace as user-service-rs (same toolchain pin, lints, clippy.toml,
   `.cargo/audit.toml`), one crate `crates/notification-service`, binary
   `notification-service`; axum, sqlx; no shared crates.
2. `scripts/ci.sh`: fmt, clippy `-D warnings`, tests with coverage (lines
   >= 80%), cargo audit (SKIP when unreachable/not installed).
3. Config: `NOTIFICATION_SERVICE_PORT` (8080),
   `POSTGRES_HOST/PORT/DB/USER/PASSWORD` (localhost, 5432, notification ×3),
   as the Scala `application.conf`; every invalid variable reported, exit 1.
4. Scala `V1__create_sent_email.sql`, `V2__add_recipient_email.sql` copied
   verbatim as sqlx migrations, run at startup.
5. `GET /health` 200, empty body, no Content-Type. `GET /health/ready` 200, or
   503 when a Postgres ping fails or takes over 2 s. `HEAD` on either: 405
   `text/plain; charset=UTF-8`, `Content-Length: 18`, no `Allow`.
6. SIGTERM/SIGINT: graceful shutdown, exit 0. Database unreachable at
   startup: exit 1 naming the database.
7. `scripts/verify-skeleton.sh`: CI, then against a throwaway Postgres, the
   three exchanges compared with the capture's `http/health*.txt` (status,
   headers bar Date/Connection, body) and `pg_dump -t sent_email` with the
   capture's DDL.
8. `tests/contract` `docs/implicit-behaviours.md` N2: Scala's
   `Connection: keep-alive` is hop-by-hop, not contract.

## Acceptance Criteria
- `scripts/ci.sh` PASS on the laptop (cargo audit running).
- `scripts/verify-skeleton.sh` PASS; failure path seen once.
- gluon `bin/verify-complete-RPN-2.sh` PASS.

## Out of Scope
Metrics, tracing, JSON logs, Kafka, `/emails`, CORS, docs (RPN-3 on);
Dockerfile, k8s (RPN-L…); GitHub Actions workflow beyond calling `ci.sh`.
