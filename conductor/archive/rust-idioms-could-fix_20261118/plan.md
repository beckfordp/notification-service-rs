# Plan — could-fix: idiomatic types, a tested lifecycle (gluon RPN-11-could-fix)

Spec: [spec.md](./spec.md). New behaviour red first; refactors guarded by the
existing tests, the contract 30/30 and RPN-11's nine tests.

## Phase 1: Types `[checkpoint: af64652]`

- [x] Task 1.1: `FromStr` for the stored enums; rows through `sqlx::FromRow` (I4) `e7d026c`
- [x] Task 1.2: Newtypes and one `RecipientKey` (M1) `af64652`

## Phase 2: Lifecycle `[checkpoint: 6153c98]`

- [x] Task 2.1: The lifecycle as a tested library function (M2) `6153c98`

## Archive (2026-11-18)

Done (gluon RPN-11-could-fix, TECHNICAL-DEBT items 13-15): `FromStr`/`Display`
for the stored enums and rows through `sqlx::FromRow`; newtypes and one
`RecipientKey`; the lifecycle as `lifecycle::run_until_stopped`, whose tests
found and fixed a spurious "HTTP server stopped" line when the consumers
stopped by themselves (from the cleanup's `0b4ba15`). Behaviour otherwise
unchanged: contract 30/30, RPN-11's nine audited-send tests, CI. gluon-rs's
`envelope-parity_20261118` (S-3, S-4) reached this repo as a lock refresh
(`ad22ac2`). No track script: the acceptance is gluon
`bin/verify-complete-RPN-11.sh`.
