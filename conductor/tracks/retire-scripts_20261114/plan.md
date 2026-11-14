# Plan — retire-scripts (gluon TD-46)

## Phase 1: Acceptance and coverage map
- [ ] Task: `scripts/verify-retire-scripts.sh`, red
  - [ ] Checks: verify-skeleton.sh gone; no `git grep` hits outside `conductor/archive`; fixtures present and `cmp`-equal to tests/contract capture 20261111; `lib/verify.sh` present and `bash -n` clean; rule in workflow.md; `scripts/ci.sh` green
  - [ ] Run: red for the stated reasons
- [ ] Task: coverage map in this plan: each verify-skeleton.sh section → test or ci.sh step; gaps (§3, §4) named as Phase 2 tests
- [ ] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)

## Phase 2: Into CI (TDD)
- [ ] Task: fixtures: copy the four capture files into `crates/notification-service/tests/fixtures/` (+ a README line citing capture 20261111)
- [ ] Task: `tests/capture.rs`: health exchanges equal to the capture
  - [ ] Red: the test against a mutated fixture (a header value) fails, naming the header
  - [ ] Green: canonical form (status, sorted lower-case headers without date and connection, body) as a helper in `tests/support`
- [ ] Task: `tests/capture.rs`: pg_dump sent_email DDL equal to `db/schema.sql`
  - [ ] Red: a mutated DDL line fails
  - [ ] Green: `Postgres::pg_dump` in support (`pg_bin()`), `canon_ddl` in Rust
- [ ] Task: `scripts/ci.sh` green, coverage ≥ 80%
- [ ] Task: Conductor - User Manual Verification 'Phase 2' (Protocol in workflow.md)

## Phase 3: Retire and rule
- [ ] Task: delete `scripts/verify-skeleton.sh`; live references updated (README, conductor docs)
- [ ] Task: `conductor/workflow.md` Phase Completion step 4: retirement rule
- [ ] Task: `verify-retire-scripts.sh` PASS; failure path proven on a copy
- [ ] Task: Conductor - User Manual Verification 'Phase 3' (Protocol in workflow.md)

At archive: `verify-retire-scripts.sh` deleted (per the rule). gluon follow-up: TD-46.1 `[x]`; TD-39 note (canon_http here gone); tests/contract `docs/implicit-behaviours.md` N2 and health row; TD-42 text.

Risks: pg_dump output can differ by Postgres version (capture taken on PG 16; the local binaries may be newer). canon_ddl drops SET/comment lines; if the statements still differ, compare via `information_schema` instead and record it.
