# Plan — adopt service-basics' HTTP edge (gluon TD-52.3)

Spec: [spec.md](./spec.md). gluon-rs side: its track `http-edge-td52_20261119`.

## Phase 1: Adoption

- [x] Task 1.1: `server::app` through `service_basics::edge`; `cors.rs` and `without_allow` gone
      (the route tests, unchanged, are the check)
- [x] Task: Conductor - User Manual Verification 'Phase 1: Adoption' (Protocol in workflow.md)
      — `scripts/ci.sh` and gluon `bin/verify-port notification-service`

## Archive (2026-11-19)

Done (gluon TD-52.3): `server::app` is `service_basics::edge::edge(routes, CorsPolicy::default())`; `cors.rs` and `without_allow` gone; route tests unchanged. `scripts/ci.sh` 96.77%; gluon `bin/verify-port notification-service` 38/38. Commit `5674540`.
