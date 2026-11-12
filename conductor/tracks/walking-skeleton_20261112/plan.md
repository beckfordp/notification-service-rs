# Plan — RPN-2: walking skeleton — migrations and health

Done on the laptop (2026-11-12): the sandbox session didn't run
`bin/sandbox-save`. Tests were written alongside the code rather than seen
red first; instead two mutations (no HEAD 405, ping bounded at 30 s) were
seen failing `tests/health.rs` and `tests/readiness.rs`.

## Phase 1: Workspace, config, migrations, health [checkpoint: f7bc264]
- [x] Task: Cargo workspace and `scripts/ci.sh` (fmt, clippy, coverage 92% lines, cargo audit ran) `f7bc264`
- [x] Task: Config contract (`tests/config.rs`) `f7bc264`
- [x] Task: `/health`, 404/405 text errors, exit 0 on SIGTERM (`tests/health.rs`) `f7bc264`
- [x] Task: V1/V2 as sqlx migrations; unreachable database → exit 1 (`tests/schema.rs`) `f7bc264`
- [x] Task: `/health/ready` bounded at 2 s: stopped, paused, back (`tests/readiness.rs`) `f7bc264`

## Phase 2: Checks against the capture [checkpoint: 949d53c]
- [x] Task: `scripts/verify-skeleton.sh` reading the capture: PASS 8 checks; failure path seen once (broken copy: both files named, exit 1) `949d53c`
- [x] Task: `tests/contract` implicit-behaviours N2 (Connection header) `4cdcf38` (tests/contract)
- [x] Task: gluon `bin/verify-complete-RPN-2.sh` PASS (run after archive)

## Phase: Review
- [x] Task: Self-review (prompting off): no findings needing changes
