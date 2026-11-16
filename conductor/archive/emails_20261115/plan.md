# Plan — `GET /emails` and CORS (gluon RPN-5)

Spec: [`spec.md`](./spec.md). Baseline (2026-11-15): tests/contract
`checks/test_notification_rest.py --service notification-service` against the
walking skeleton, 5 passed, 19 failed (every `/emails` and CORS check, each a 404).

Each step below is reviewed and approved before it lands on `main`.

## Step 1 — Scope and acceptance
- [x] Task: spec with the decisions D1–D7; this plan; gluon
      `bin/verify-complete-RPN-5.sh`.

## Step 2 — Shared `java_instant` (D1)
- [x] Task: gluon-rs `service-basics::jvm::java_instant`, known answers in
      `tests/jvm.rs`. gluon-rs `c4ee9be`; its lock only adds `time` (TD-53);
      `ci.sh` green on a scratch copy with the sandbox-resolved lock.
- [x] Task: user-service-rs `users::java_instant` re-exports it; its
      `tests/instant.rs` unchanged and green. user-service-rs `d99236e`; clippy
      and `tests/instant.rs`, `tests/startup.rs` green (the identical draft
      commit: `ci.sh` green, contract 128/128).

## Step 3 — Filters and the store (D2, D6)
- [x] Task: `domain`: `SentEmail`, `RecipientType`, `EmailStatus`,
      `EmailFilter`; `email_filter`, `query_pairs`, `decode`;
      `QueryError::Multiple`. Tests: `tests/domain.rs`. `3c36a68`.
- [x] Task: `store`: `SentEmails` and `PgSentEmails` (Scala `listEmails`).
      Tests: `tests/store.rs` against Postgres (the store itself; the HTTP
      versions come with step 4). `3c36a68`; `ci.sh` green, lines 96%.

## Step 4 — `GET /emails` (D4, D5, D6)
- [x] Task: `emails`: the handler, `SentEmailResponse`, `EmailsError`
      (400 / 500); `/emails` and `/emails/`; other methods 405 without `Allow`. `699ed80`.
- [x] Task: `server`: the state generic over the store; `app` wraps the
      routes. Tests: `tests/routes.rs` against an in-memory store.
      `tests/emails.rs` through the binary. `699ed80`; `ci.sh` green, lines 97%;
      harness 21/24 (the 3 CORS checks are step 5).

## Step 5 — CORS (D3)
- [x] Task: `cors`: http4s `CORSPolicy` with `Cors.scala`'s settings; the
      no-route 404 marked `Unrouted`. Tests: `tests/routes.rs`. `3090b52`;
      `ci.sh` green, lines 97%.
- [x] Task: the harness group against `cargo build --locked`: 24/24. `3090b52`, 2026-11-15.

## Step 6 — Docs, debt and close
- [x] Task: gluon TECHNICAL_DEBT.md TD-52 (the one contract gap judged large
      enough; the rest are the spec's "Not captured" list). Approved 2026-11-15.
- [x] Task: gluon TECHNICAL_DEBT.md TD-53 (gluon-rs's lock ahead of the sandbox's
      crates.io mirror; found in step 2). gluon `4e9ddf3`.
- [x] Task: README status, `tech-stack.md` notes (`459cc86`); tests/contract README
      (`7887fdd`); gluon TECHNICAL_DEBT.md TD-42 note (gluon `637f9e2`).
- [x] Task: gluon `docs/ports/notification-service-design-differences.md`:
      the report of the differences below (asked for in review, 2026-11-15).
      gluon `637f9e2`.
- [x] Task: `bin/verify-complete-RPN-5.sh`: gluon-rs CI on a scratch copy with
      a regenerated lock (TD-53); no save in the script (gluon `637f9e2`).
- [~] Task: archive this track; `bin/verify-complete-RPN-5.sh` all PASS; the
      deliverables reviewed; then clean the build output, delete the
      `draft/rpn-5` branches and run `bin/sandbox-save`.

## Deferred
- The `Received request` log line, the server span and metrics (RPN-7).
- `/docs` and the AsyncAPI routes.

## Differences from user-service-rs (noted as each step lands)
Design kept as is (review, 2026-11-15); reported at the end in gluon `docs/ports/`.
- Step 2: none; `java_instant` is now the same function in both.
- Step 3, store: a `SentEmails` trait with `PgSentEmails`; user-service-rs has a
  concrete, instrumented `Store` and no trait.
- Step 3, rows: read by column name (`try_get`) into one struct, text columns parsed
  into enums (an unknown value is an error, as Scala's codecs); user-service-rs reads
  tuples (`query_as`) and has no enum columns.
- Step 3, query parameters: an http4s-faithful parser in `domain`; user-service-rs has
  none (its only input parsing is the path id, `telemetry::jvm::parse_uuid`).
- Step 4, errors: the handler returns `Result<Json<_>, EmailsError>` with one
  `IntoResponse` arm per variant; user-service-rs handlers build a `Response` inline.
- Step 4, 500 body: `Internal server error` (tapir's default handler); user-service-rs
  `Internal Server Error` (its Scala code sets it). Each follows its Scala service.
- Step 4, `Allow`: removed from every 405; user-service-rs keeps axum's (TD-52).
- Step 4, composition: the router wrapped whole (`Router::new().fallback_service`) so
  middleware sees its 404s and 405s; user-service-rs layers the router (`.layer`).
- Step 4, state: `AppState<S>` generic over the store; user-service-rs's is concrete.
- Step 4, tests: routes against an in-memory store in-process (`tower::oneshot`);
  user-service-rs tests its routes through the started binary only.
- Step 4, not yet: logging and store metrics (RPN-7; a store error goes to stderr),
  and `schemars` on the response (the docs session); user-service-rs has both.
- Step 5, CORS: http4s `CORSPolicy` in full: a refused preflight gets only `Vary`,
  the no-route 404 no CORS header, a routed OPTIONS `Vary`; user-service-rs answers
  every preflight with the Allow-* headers and puts its headers on the 404 too (TD-52).
- Step 5, placement: CORS wraps the whole router, outermost; user-service-rs layers it
  on the router.
- Step 5, exposed headers: none (notification-service `Cors.scala`); user-service-rs
  `X-Total-Count` (its `Cors.scala`). Each follows its Scala service.
- Step 5, the no-route marker: `cors::Unrouted`; user-service-rs has
  `telemetry::tracing::Unmatched` for the same fact. One marker once telemetry moves
  to gluon-rs (RPN-7).
- Step 5, same in both: an `Origin` http4s can't parse is treated as an origin.

