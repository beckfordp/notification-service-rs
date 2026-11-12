# Plan — RPN-2: walking skeleton — migrations and health

## Phase 1: Workspace, config, liveness
- [ ] Task: Cargo workspace and `scripts/ci.sh`
  - [ ] Workspace as user-service-rs; `.github/workflows/ci.yml` calls `ci.sh`
- [ ] Task: Config contract
  - [ ] Red: `tests/config.rs` (defaults, overrides, every bad port reported)
  - [ ] Green: `config.rs`
- [ ] Task: Serve `/health`, exit 0 on SIGTERM
  - [ ] Red: `tests/health.rs` against the started binary
  - [ ] Green: `server.rs`, `main.rs`
- [ ] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)

## Phase 2: Migrations and readiness
- [ ] Task: V1/V2 as sqlx migrations
  - [ ] Red: `tests/schema.rs` (`sent_email` columns and constraints); unreachable database → exit 1
  - [ ] Green: `db.rs`, migrations
- [ ] Task: `/health/ready` bounded at 2 s; HEAD 405
  - [ ] Red: `tests/readiness.rs` (stopped, paused, back)
  - [ ] Green: ping in `server.rs`
- [ ] Task: Conductor - User Manual Verification 'Phase 2' (Protocol in workflow.md)

## Phase 3: Checks against the capture
- [ ] Task: `scripts/verify-skeleton.sh` reading the capture; failure path seen once
- [ ] Task: `tests/contract` implicit-behaviours N2 (Connection header)
- [ ] Task: gluon `bin/verify-complete-RPN-2.sh` PASS
- [ ] Task: Conductor - User Manual Verification 'Phase 3' (Protocol in workflow.md)
