# Plan — On lib/gluon-rs (gluon TD-43.1, TD-47)

A refactor: the existing tests are the guard, run before and after each task.

## Phase 1: onto gluon-rs
- [x] Task: Path dependencies, `config` and `main` (guard: `tests/config.rs`) 639ea7c
- [x] Task: `db` and `server` onto `postgres`, `health` and `text_error` c334373
      (guard: `tests/health.rs`, `tests/readiness.rs`)
- [x] Task: `tests/support` onto service-testing 8b73b64
- [x] Task: TD-47: the pool size cites the Scala source b104cb6
- [ ] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)
    - [ ] `scripts/ci.sh` green
    - [ ] `git grep 'as user-service-rs'` empty
    - [ ] `tests/contract` `verify-check-group.sh checks/test_notification_rest.py
          --service notification-service`: 5 green, 19 red
