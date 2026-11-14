# Spec: CI on gluon-rs's shared steps, Linux and macOS (gluon TD-43.2)

## Overview
`scripts/ci.sh` copies the steps gluon-rs's `scripts/lib/ci.sh` now holds, and
its audit step uses coreutils `timeout`: on a stock Mac (no Homebrew
coreutils) it says the advisory database is unreachable when it isn't, so
cargo audit never runs. `scripts/lib/verify.sh` is an unused copy of
gluon-rs's.

## Functional Requirements
- FR-1 `scripts/ci.sh` sources `${GLUON_RS:-../../lib/gluon-rs}/scripts/lib/ci.sh`
  and runs `ci_fmt`, `ci_clippy`, `ci_coverage`, `ci_audit rsa libsqlite3-sys`.
- FR-2 `scripts/lib/verify.sh` removed; workflow.md's TD-46 rule points at
  gluon-rs's.

## Acceptance Criteria
- `scripts/ci.sh` green, with the same steps, on the normal PATH and on a
  stock-macOS PATH (no coreutils), where cargo audit now runs.
- shellcheck clean.

## Out of Scope
- The GitHub workflow's gluon-rs checkout (backlog; needs gluon-rs's remote).
