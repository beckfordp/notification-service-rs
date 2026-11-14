# Plan — CI on gluon-rs's shared steps, Linux and macOS (gluon TD-43.2)

## Phase 1: shared CI steps
- [x] Task: `scripts/ci.sh` on gluon-rs's library; drop `scripts/lib/verify.sh` 14435c3
    - [x] Red: on a stock-macOS PATH, ci.sh says "SKIP cargo-audit: advisory
          database unreachable" with the database reachable
    - [x] Green: ci.sh on the library; audit runs on that PATH
- [x] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)
