# Plan — retire-scripts (gluon TD-46)

## Phase 1: Acceptance and coverage map [checkpoint: 129b8cf]
- [x] Task: `scripts/verify-retire-scripts.sh`, red `740ad40` — 7 of 9 failed: fixtures absent, script present and named by README, no rule; ci.sh green (92.36%)
  - [x] Checks: verify-skeleton.sh gone; no `git grep` hits outside `conductor/archive`; fixtures present and `cmp`-equal to tests/contract capture 20261111; `lib/verify.sh` present and `bash -n` clean; rule in workflow.md; `scripts/ci.sh` green
  - [x] Run: red for the stated reasons
- [x] Task: coverage map in this plan: each verify-skeleton.sh section → test or ci.sh step; gaps (§3, §4) named as Phase 2 tests

  | verify-skeleton.sh § | Covered by |
  |---|---|
  | 1 CI | `scripts/ci.sh` |
  | 2 serves on its port (build, throwaway Postgres) | `tests/health.rs` `health_is_200_with_no_body_and_no_content_type` (support `Service::start` builds and waits) |
  | 3 health exchanges = capture `http/health.txt`, `health-ready.txt`, `health-head.txt` | **gap** → `tests/capture.rs` against vendored fixtures (today's tests assert hardcoded values) |
  | 4 `pg_dump -t sent_email` = capture `db/schema.sql` | **gap** → `tests/capture.rs` (`tests/schema.rs` checks columns and the status check, not the whole DDL) |
  | 5 SIGTERM exits 0 | `tests/health.rs` `sigterm_exits_0` |
- [x] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)

## Phase 2: Into CI (TDD) [checkpoint: 0e28d22]
- [x] Task: fixtures: copy the four capture files into `crates/notification-service/tests/fixtures/` (+ a README line citing capture 20261111) `5b29fc9`, CRLF kept `a90cd8e` (`.gitattributes` `-text`: `core.autocrlf=input` had stored them LF)
- [x] Task: `tests/capture.rs`: health exchanges equal to the capture `f321916`
  - [x] Red: the test against a mutated fixture (a header value) fails, naming the header — red first: helpers missing; HEAD fixture `Content-Length: 18` → `17` fails, both sides shown
  - [x] Green: canonical form (status, sorted lower-case headers without date and connection, body) as a helper in `tests/support` — `canon_exchange`, `Service::exchange` (status line from the code's standard reason phrase)
- [x] Task: `tests/capture.rs`: pg_dump sent_email DDL equal to `db/schema.sql` `f321916`
  - [x] Red: a mutated DDL line fails — `reason text` → `character varying` fails
  - [x] Green: `Postgres::pg_dump` in support (`pg_bin()`), `canon_ddl` in Rust — local PG 16.15 = capture's
- [x] Task: `scripts/ci.sh` green, coverage ≥ 80% — 92.36% lines
- [x] Task: Conductor - User Manual Verification 'Phase 2' (Protocol in workflow.md)

## Phase 3: Retire and rule [checkpoint: 94136ad]
- [x] Task: delete `scripts/verify-skeleton.sh`; live references updated (README, conductor docs) `b5d2b97`
- [x] Task: `conductor/workflow.md` Phase Completion step 4: retirement rule `b5d2b97`
- [x] Task: `verify-retire-scripts.sh` PASS; failure path proven on a copy — PASS 9 checks (ci.sh 92.36%); copy citing a missing fixture: exit 1, file named
- [x] Task: Conductor - User Manual Verification 'Phase 3' (Protocol in workflow.md)

At archive: `verify-retire-scripts.sh` deleted (per the rule). gluon follow-up: TD-46.1 `[x]`; TD-39 note (canon_http here gone); tests/contract `docs/implicit-behaviours.md` N2 and health row; TD-42 text.

Risks: pg_dump output can differ by Postgres version (capture taken on PG 16; the local binaries may be newer). canon_ddl drops SET/comment lines; if the statements still differ, compare via `information_schema` instead and record it.

## Phase: Review Fixes
- [x] Task: Apply review suggestions b052004
