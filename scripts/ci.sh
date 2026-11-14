#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# CI for notification-service-rs, defined once: the sandbox runs this script
# directly and GitHub Actions calls it (.github/workflows/ci.yml). Stops at the
# first failing step; each step announces itself with a "== " line.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

step() { echo "== $*"; }

step "cargo fmt --check"
cargo fmt --all --check

step "cargo clippy (warnings are errors)"
cargo clippy --workspace --all-targets --locked -- -D warnings

# Runs the tests once, instrumented; fails below 80% of lines (workflow.md).
# cargo-llvm-cov 0.9.1 exits 1 below the gate without a word, so say why here.
step "tests with coverage (lines >= 80%)"
coverage_log="$(mktemp)"
trap 'rm -f "$coverage_log"' EXIT
if ! cargo llvm-cov --workspace --locked --fail-under-lines 80 --summary-only 2>&1 \
  | tee "$coverage_log"; then
  if ! grep -q "test result: FAILED" "$coverage_log"; then
    lines="$(awk '/^TOTAL/ {print $(NF-3)}' "$coverage_log")"
    echo "FAIL coverage: ${lines:-an unknown share} of lines covered, below the 80% gate"
  fi
  exit 1
fi

# The RustSec advisory database lives on GitHub, which the sandbox can't
# reach (gluon plans/private/sandbox.md): say so rather than pass silently.
step "cargo audit"
advisory_db="https://github.com/rustsec/advisory-db"
if ! timeout 20 git ls-remote --exit-code "$advisory_db" HEAD > /dev/null 2>&1; then
  echo "SKIP cargo-audit: advisory database unreachable ($advisory_db)"
elif ! command -v cargo-audit > /dev/null 2>&1; then
  echo "SKIP cargo-audit: not installed (GitHub Actions installs it)"
else
  # .cargo/audit.toml's ignores hold only while these are lockfile-only
  for crate in rsa libsqlite3-sys; do
    if [ -n "$(cargo tree -i "$crate" -e all --target all --locked -q)" ]; then
      echo "FAIL $crate is built: drop its .cargo/audit.toml ignore"
      exit 1
    fi
  done
  cargo audit --deny warnings
fi
