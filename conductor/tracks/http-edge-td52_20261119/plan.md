# Plan — adopt service-basics' HTTP edge (gluon TD-52.3)

Spec: [spec.md](./spec.md). gluon-rs side: its track `http-edge-td52_20261119`.

## Phase 1: Adoption

- [ ] Task 1.1: `server::app` through `service_basics::edge`; `cors.rs` and `without_allow` gone
      (the route tests, unchanged, are the check)
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Adoption' (Protocol in workflow.md)
      — `scripts/ci.sh` and gluon `bin/verify-port notification-service`
