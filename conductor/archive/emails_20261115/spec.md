# Spec — `GET /emails` and CORS (gluon RPN-5)

## Why
The harness's REST group for notification-service (tests/contract
`checks/test_notification_rest.py`, RPN-4) is red against the walking skeleton
in its 19 `/emails` and CORS checks (gluon TD-42: "the 19 `/emails` checks still
RPN-5's"). This track turns them green: ADM-2's read-only audit API, as the
Scala service answers it.

## Reference
- Capture `tests/contract/contracts/notification-service/20261111`:
  `http/emails-*.txt`, `cors-*.txt`, `db/sent-email.jsonl`, `openapi.yaml`.
- Scala `EmailRoutes.scala` (the endpoint and `SentEmailResponse`),
  `SentEmailStore.scala` (`listEmails`: the filters and `order by created_at
  desc, id`), `Cors.scala` (the policy), `AppRoutes.scala` (CORS outermost).
- Where no capture pins a case down, the libraries the Scala service runs,
  read at source: http4s 0.23 `CORSPolicy` and `QueryParser`/`Uri.decode`,
  tapir `DecodeBasicInputs`, `DefaultExceptionHandler`. Each such case is
  listed under "Not captured" below, to confirm with a capture.

## Functional requirements
- FR1 `GET /emails?customerId=&orderId=`: 200 `application/json`, a JSON array
  of every `sent_email` row matching both filters (each optional, matched as
  exact text), newest first (`created_at desc, id`). Each element has the 16
  fields of `SentEmailResponse` in its order, absent values as `null`,
  instants as `java.time.Instant.toString` prints them.
- FR2 The query as tapir reads it through http4s: `&` and `;` separate pairs,
  `+` is a space, `%XX` decodes (a bad escape stays literal), a key without
  `=` has no value (`?customerId` is no filter; `?customerId=` filters on
  `""`), unknown keys are ignored, a repeated filter is 400 text/plain
  `Invalid value for: query parameter <name> (multiple values)`
  (`customerId` reported first).
- FR3 `/emails/` answers as `/emails`.
- FR4 Any other method on `/emails` (HEAD included) is 405 text/plain
  `Method Not Allowed`, with no `Allow` header.
- FR5 CORS as http4s `CORS.policy` with the Scala settings (any origin; GET,
  POST, PATCH, PUT, DELETE; `Content-Type`; no credentials; nothing exposed):
  - a preflight (OPTIONS with `Origin` and `Access-Control-Request-Method`),
    on any path, is 200 and empty; when the method is allowed and every
    requested header is, it carries `Access-Control-Allow-Origin: *`,
    `-Allow-Methods` and `-Allow-Headers: Content-Type`; otherwise none of them;
  - every OPTIONS answer from a route carries
    `Vary: Access-Control-Request-Method, Access-Control-Request-Headers`;
  - a request with `Origin` answered by a route gets
    `Access-Control-Allow-Origin: *` and nothing else; the 404 for an unknown
    path (no route) gets no CORS header.
- FR6 A store failure is 500 text/plain `Internal server error` (tapir's
  default exception handler: `EmailRoutes` uses `serverLogicSuccess`).
- FR7 An inbound `traceparent` doesn't change the answer (tracing itself is
  RPN-7).

## Out of scope
- The `Received request` log line and the server span and metrics: the
  telemetry crate arrives in RPN-7.
- `/docs`, `/docs/docs.yaml` and the AsyncAPI routes (their own session).
- Writing rows: the consumers (RPN-10 on).

## Not captured (source-derived; to confirm with a capture)
- FR2's `;` separator, key without `=`, `+` and bad `%` escapes.
- FR5's preflight on an unknown path, a preflight for a disallowed method,
  and the `Vary` on a non-preflight OPTIONS.
- FR6's 500 body.
- tapir's path handling beyond `/emails/` (leading and repeated slashes,
  percent-encoded segments, `/health/`): not built here.

## Acceptance
- tests/contract `checks/test_notification_rest.py --service
  notification-service` against `cargo build --locked`: every check green
  (24/24, the readiness check included).
- `scripts/ci.sh` green (fmt, clippy `-D warnings`, tests, coverage ≥ 80%).
- Unit tests for the pure parts (query, row mapping, instants), handler and
  CORS tests against an in-memory store, and Postgres-backed tests for the
  query's filters and order.

## Decisions (2026-11-15, approved in review step 1)
- D1 Java's `Instant.toString` lives once, in gluon-rs
  `service-basics::jvm::java_instant`; user-service-rs re-exports it (TD-43).
- D2 Query cases no capture shows follow http4s/tapir at source (FR2), each
  listed under "Not captured" for a laptop capture.
- D3 CORS is this crate's own middleware following http4s `CORSPolicy`
  (tower-http's `CorsLayer` can't answer a refused preflight as N8 requires);
  sharing it with user-service-rs is TD-52, not this track. An `Origin` http4s
  can't parse is treated as an origin: a documented gap, not built.
- D4 No `Allow` on any 405 (tapir sends none), `/health` included.
- D5 Only `/emails/` is routed; tapir's wider path handling is not built.
- D6 The store behind a `SentEmails` trait (handlers tested with an in-memory
  one); an unknown stored value is a 500; rows fetched at once, not streamed;
  a store error goes to stderr until RPN-7. CORS and the `Allow` removal wrap
  the whole router (`Router::fallback_service`), so they see its 404s and 405s.
- D7 D1 and TD-52's write-up are part of RPN-5.
