# Plan — CI on gluon-rs's shared steps, Linux and macOS (gluon TD-43.2)

## Phase 1: shared CI steps
- [ ] Task: `scripts/ci.sh` on gluon-rs's library; drop `scripts/lib/verify.sh`
    - [x] Red: on a stock-macOS PATH, ci.sh says "SKIP cargo-audit: advisory
          database unreachable" with the database reachable
    - [ ] Green: ci.sh on the library; audit runs on that PATH
- [ ] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)
