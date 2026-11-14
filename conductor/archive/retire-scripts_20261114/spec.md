# Spec — retire-scripts: verify-skeleton.sh into CI (gluon TD-46)

## Overview
The one track script, `scripts/verify-skeleton.sh` (RPN-2), compares the started service against capture 20261111. Its §3 compares the health exchanges and §4 the `sent_email` DDL. The Rust tests check the same behaviour from hardcoded expected values, but not equality with the capture. Move that equality into CI, delete the script, and record the rule. Precedents: user-service-rs and tests/contract `retire-scripts_20261113`.

## Functional Requirements
- **FR1:** vendor the capture's `http/health.txt`, `health-ready.txt`, `health-head.txt` and `db/schema.sql` into `crates/notification-service/tests/fixtures/`, unchanged, citing capture 20261111. Load them with `include_str!`.
- **FR2:** a Rust test: each exchange from the started binary equals its fixture (status line, headers in any case and order except Date and Connection, body). Connection is excluded as tests/contract N2.
- **FR3:** a Rust test: `pg_dump -s --no-owner -t sent_email` after startup equals the fixture's sent_email statements, normalised as the script's `canon_ddl` did. pg_dump comes from the support module's `pg_bin()`.
- **FR4:** coverage map in plan.md: §1 CI → ci.sh; §2 serves → tests/health.rs; §3 → FR2; §4 → FR3; §5 SIGTERM → `health.rs` `sigterm_exits_0`.
- **FR5:** delete `scripts/verify-skeleton.sh`. Keep `scripts/lib/verify.sh` (the helper for future track scripts).
- **FR6:** `conductor/workflow.md` Phase Completion step 4 gets the retirement rule (wording as in the other two repos).
- **FR7:** update live references (README, conductor docs); archives untouched.

## Non-Functional Requirements
- `scripts/ci.sh` green, coverage ≥ 80%.
- Each new test proven red on a mutation (a fixture header and a DDL line changed).

## Acceptance Criteria
`scripts/verify-retire-scripts.sh` PASS: verify-skeleton.sh gone and unreferenced outside archives; fixtures present and byte-equal to the capture (checked against tests/contract here); `lib/verify.sh` present and parses; the rule in workflow.md; ci.sh green; failure path proven on a copy. The script itself is retired at archive.

## Follow-ups (gluon root, after archive)
TD-46.1 → `[x]`. TD-46/TD-39 notes: canon_http no longer in notification-service-rs. tests/contract `docs/implicit-behaviours.md` N2 and the health row point at the Rust tests. TD-42 text.

## Out of Scope
- TD-42 (health checks in the harness), TD-43 shared crates
- notification-service-rs features (RPN-5+)
