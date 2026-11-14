#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Verifies track retire-scripts_20261114 (gluon TD-46): verify-skeleton.sh's
# capture comparisons are Rust tests against vendored fixtures, byte-equal to
# capture 20261111; the script is gone and nothing live names it; the shared
# helper is kept; workflow.md states the retirement rule. Retired itself when
# the track is archived. Each result is printed as found.
#   scripts/verify-retire-scripts.sh [--skip-ci] [capture dir]
set -uo pipefail
SELF="$(realpath "${BASH_SOURCE[0]}")"
cd "$(dirname "$SELF")/.." || exit 1
. scripts/lib/verify.sh
verify_as_owner "$SELF" "$@"
SKIP_CI=""
if [ "${1:-}" = "--skip-ci" ]; then
  SKIP_CI=1
  shift
fi
CAPTURE="${1:-../../tests/contract/contracts/notification-service/20261111}"
FIXTURES=crates/notification-service/tests/fixtures
SCRATCH="$(mktemp -d /tmp/verify-retire-scripts.XXXXXX)"
trap 'rm -rf "$SCRATCH"' EXIT

echo "== 1. CI (scripts/ci.sh) =="
if [ -n "$SKIP_CI" ]; then
  echo "skip --skip-ci given"
elif scripts/ci.sh > "$SCRATCH/ci.log" 2>&1; then
  ok "scripts/ci.sh passes ($(grep -E '^TOTAL' "$SCRATCH/ci.log" | awk '{print $(NF-3)}') of lines covered)"
else
  bad "scripts/ci.sh failed:"
  tail -8 "$SCRATCH/ci.log" | sed 's/^/     /'
fi

echo "== 2. Fixtures are the capture's files, unchanged =="
for file in http/health.txt http/health-ready.txt http/health-head.txt db/schema.sql; do
  if cmp -s "$CAPTURE/$file" "$FIXTURES/$file"; then
    ok "$FIXTURES/$file = capture $file"
  else
    bad "$FIXTURES/$file missing or differs from capture $file"
  fi
done

echo "== 3. verify-skeleton.sh is retired =="
if [ -e scripts/verify-skeleton.sh ]; then bad "scripts/verify-skeleton.sh still exists"; else ok "verify-skeleton.sh gone"; fi
hits="$(git grep -l 'verify-skeleton' -- . ':!conductor/archive' ':!conductor/tracks/retire-scripts_*' ':!conductor/tracks.md' ':!scripts/verify-retire-scripts.sh')"
if [ -z "$hits" ]; then ok "no live references"; else bad "named in: $(echo $hits)"; fi

echo "== 4. The shared helper is kept =="
check "scripts/lib/verify.sh present, parses" bash -n scripts/lib/verify.sh

echo "== 5. workflow.md states the rule =="
rule="$(sed -n '/^### Phase Completion/,/^### /p' conductor/workflow.md | grep -m1 'Retiring the script')"
if [ -n "$rule" ]; then ok "Phase Completion: ${rule:0:110}…"; else bad "Phase Completion has no 'Retiring the script' rule"; fi

verify_summary "retire-scripts (gluon TD-46)"
