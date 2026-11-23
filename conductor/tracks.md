# Project Tracks

This file tracks all major tracks for the project.

---

## Tracks

- [ ] **Track: The remaining port: recipient lookups, order/payment/fulfilment consumers, docs, image (gluon all-Rust Stage A notification)**
  *Link: [./tracks/port_20261123/](./tracks/port_20261123/)*

---

## Backlog

Title-only placeholders for future tracks — not yet detailed (no spec/plan, no linked
folder), so `/conductor:implement` cannot pick these up by accident. Source of truth for
scope: gluon `PLAN.md` "Rust Port — notification-service" (ADR 0029). One track per
session, outside-in (each harness group in `tests/contract` first), each ending with a
review. When ready, run `/conductor:newTrack <title>`.


- CI checks out gluon-rs: crates depend on ../../lib/gluon-rs by path (gluon TD-43.1); .github/workflows/ci.yml must check out gluon-rs alongside (the repo itself at services/notification-service-rs in the workspace). Needs gluon-rs's GitHub remote first
