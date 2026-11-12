# Product Guidelines — notification-service-rs

## Parity first
- The specified contracts are the reference and the `tests/contract` harness
  is the judge (ADR 0029; the first port: ADR 0028 decisions 2 and 3): the documented contracts, the
  Scala-generated OpenAPI/AsyncAPI artifacts, the Scala repo's test suites and
  `verify-*.sh` scripts, and behaviour captured on the laptop on request. A
  track is not done until the harness groups it targets are green against
  this service in isolated mode.
- Match the reference exactly, including behaviour nobody wrote down: error
  bodies and `error` strings, status codes for malformed input, CORS, metric
  names and labels, log fields. If the harness doesn't cover something yet,
  add a harness check with its source first; if no source pins it down, ask
  for a capture from the Scala service on the laptop.
- Documenting existing behaviour is allowed; changing it is not. A gap found
  here is recorded in the track and raised in gluon (PLAN.md "Rust Port — notification-service").

## Code & engineering conventions
Canonical source: [`code_styleguides/rust.md`](./code_styleguides/rust.md)
(functional core, errors as enums, parse don't validate, no panics on the
request path, explicit wiring). It is the Rust counterpart of the Scala
services' `scala-fp.md`.

## API design conventions
- Plain resource JSON responses, standard HTTP status codes, no response
  envelope; errors are `{"error": "<message>"}` with the Scala service's exact
  messages.
- Each domain error maps to a status explicitly at the edge, one mapping per
  case; no catch-all translator.
- OpenAPI and AsyncAPI are generated from the same Rust definitions as the
  real routes and events, so they stay in sync by construction.
- One dedicated Postgres database (`notification`); never another service's database.

## Testing
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` before
  every commit (CI enforces the same).
- Unit tests for the pure core; Postgres-backed tests against a throwaway
  PostgreSQL 16 cluster started from the local binaries (`initdb`/`pg_ctl`).
  No Docker or Testcontainers: the sandbox, where this repo is developed and
  tested, has no Docker.
- The harness (`tests/contract`, isolated mode) against a build of this
  service at each phase checkpoint.
- Only Rust (and, if needed, Python or TypeScript tooling) changes in the
  sandbox; the Dockerfile is written and built on the laptop (an `RPN-L…`
  session).

## Security conventions
- Email bodies and recipient addresses are customer data: never logged
  beyond what the Scala service logs.
- No secret is committed; configuration comes from env vars only.

