# Spec — adopt service-basics' HTTP edge (gluon TD-52.3)

## Overview
This service's `cors.rs` moved to gluon-rs `service-basics` (track
`http-edge-td52_20261119`), with tapir's Allow-less 405, behind one `edge(routes, policy)`.
The service uses it and drops its own copy, with no behaviour change.

## Acceptance Criteria
- [ ] No `cors.rs`, no `Allow` code; `server::app` is `edge(routes, CorsPolicy::default())`.
- [ ] `tests/routes.rs`' CORS and 405 cases unchanged and green; `scripts/ci.sh` green;
      gluon `bin/verify-port notification-service` 38/38.
