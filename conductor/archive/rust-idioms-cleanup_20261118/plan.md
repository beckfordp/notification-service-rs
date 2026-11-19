# Plan — RPN-11 cleanup (notification-service-rs)

Spec: [spec.md](./spec.md). Acceptance: gluon `bin/verify-complete-RPN-11.sh`.
Refactors: the existing tests green before and after; new behaviour (typed errors,
injected ids, the JSON startup line, store failures) red first.

## Phase 1: The audited send `[checkpoint: 2348562]`

- [x] Task 1.1: Test support (M4) `feae690`
- [x] Task 1.2: Exhaustive match on `Claim` (H3) `7664c5a`
- [x] Task 1.3: Pure welcome message and carried address; injected ids (H1, H4) `5938ee8`
- [x] Task 1.4: Typed errors, `Recipient`, one `send` (H2, L1, L2) `4d86a70`
- [x] Task 1.5: Store-failure tests with an in-memory store (M5) `2348562`

## Phase 2: Wiring and docs `[checkpoint: 02eb002]`

- [x] Task 2.1: `main` wires the client; JSON startup line; shutdown results logged (H5, S-8, M3) `0b4ba15`
- [x] Task 2.2: Docs (L5) `02eb002`

## Archive (2026-11-18)

Done: TECHNICAL-DEBT items 1–7 (H1–H5, S-8, M3–M5, L1, L2, L5) in this repo,
behaviour unchanged (contract 30/30, RPN-11's nine audited-send tests, CI). The
supervised-consumer helper and the OTEL reader (S-1, S-2) arrive from gluon-rs's
`shared-consumer-otel_20261118` (RPN-11-cleanup), adopted here in their own
commits. No track script: the acceptance is gluon `bin/verify-complete-RPN-11.sh`.
