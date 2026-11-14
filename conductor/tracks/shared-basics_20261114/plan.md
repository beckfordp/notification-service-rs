# Plan — On lib/gluon-rs (gluon TD-43.1, TD-47)

A refactor: the existing tests are the guard, run before and after each task.

## Phase 1: onto gluon-rs
- [ ] Task: Path dependencies, `config` and `main` (guard: `tests/config.rs`)
- [ ] Task: `db` and `server` onto `postgres`, `health` and `text_error`
      (guard: `tests/health.rs`, `tests/readiness.rs`)
- [ ] Task: `tests/support` onto service-testing
- [ ] Task: TD-47: the pool size cites the Scala source
- [ ] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)
    - [ ] `scripts/ci.sh` green
    - [ ] `git grep 'as user-service-rs'` empty
    - [ ] `tests/contract` `verify-check-group.sh checks/test_notification_rest.py
          --service notification-service`: 5 green, 19 red
