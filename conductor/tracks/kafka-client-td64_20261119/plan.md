# Plan — adopt kafka-client's TD-64 changes

Spec: [spec.md](./spec.md). gluon-rs side: its track `kafka-client-td64_20261119`.

## Phase 1: Adoption

- [ ] Task 1.1: `UserEventsError` for the route; tests compare variants and text
- [ ] Task 1.2: `GroupId`/`Topic` for the consumer settings; a test that the constants parse
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Adoption' (Protocol in workflow.md)
      — `scripts/ci.sh` and gluon `bin/verify-port notification-service`
