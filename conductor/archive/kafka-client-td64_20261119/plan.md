# Plan — adopt kafka-client's TD-64 changes

Spec: [spec.md](./spec.md). gluon-rs side: its track `kafka-client-td64_20261119`.

## Phase 1: Adoption

- [x] Task 1.1: `UserEventsError` for the route; tests compare variants and text `364c67a`
- [x] Task 1.2: `GroupId`/`Topic` for the consumer settings; a test that the constants parse `364c67a`
- [x] Task: Conductor - User Manual Verification 'Phase 1: Adoption' (Protocol in workflow.md)
      — `scripts/ci.sh` and gluon `bin/verify-port notification-service`

## Archive (2026-11-19)

Done (gluon TD-64 adoption): `UserEventsError` (payload, send) for the route, the
consumer's group and topic parsed into `GroupId`/`Topic` (`consumers::settings`). No
behaviour change. Regression: `scripts/ci.sh` 96.76%; gluon `bin/verify-port
notification-service` 38/38 (rest 24, user_events 6, audited 8). Commit `364c67a`.
