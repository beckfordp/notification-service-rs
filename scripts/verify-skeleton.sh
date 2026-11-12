#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Verifies track walking-skeleton_20261112 (gluon PLAN.md RPN-2) against the
# capture tests/contract/contracts/notification-service/20261111, read
# directly: GET /health, GET /health/ready and HEAD /health match http/*.txt
# (status line, headers bar Date and Connection in any case and order, body),
# and pg_dump -t sent_email matches db/schema.sql. Each result is printed as
# it is found.
#   scripts/verify-skeleton.sh [--skip-ci] [capture dir]
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
SCRATCH="$(mktemp -d /tmp/verify-skeleton.XXXXXX)"
SVC=""
cleanup() {
  [ -n "$SVC" ] && kill -KILL "$SVC" 2> /dev/null
  pg_stop
  rm -rf "$SCRATCH"
}
trap cleanup EXIT

# canon_http FILE: an exchange as status line, headers bar Date and Connection
# (Connection is hop-by-hop: tests/contract implicit behaviour N2; lower-case
# names, sorted), blank line, body
canon_http() {
  python3 -I - "$1" << 'PY'
import sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read().replace("\r\n", "\n")
head, _, body = text.split("\n--\nelapsed=")[0].partition("\n\n")
status, *lines = head.split("\n")
headers = sorted(
    f"{name.strip().lower()}: {value.strip()}"
    for name, _, value in (line.partition(":") for line in lines)
    if name.strip().lower() not in ("date", "connection")
)
print(status.strip(), *headers, "", body.rstrip("\n"), sep="\n")
PY
}

# canon_ddl FILE: a pg_dump's sent_email statements, whitespace collapsed,
# sorted; no comments, settings or ownership
canon_ddl() {
  python3 -I - "$1" << 'PY'
import sys
lines = open(sys.argv[1], encoding="utf-8").read().split("\n")
kept = "\n".join(l for l in lines if not l.startswith(("--", "\\", "SET ", "SELECT pg_catalog.")))
statements = (" ".join(s.split()) for s in kept.split(";"))
print("\n".join(sorted(s for s in statements if "sent_email" in s and " OWNER TO " not in s)))
PY
}

echo "== 1. CI (scripts/ci.sh) =="
if [ -n "$SKIP_CI" ]; then
  echo "skip --skip-ci given"
elif scripts/ci.sh > "$SCRATCH/ci.log" 2>&1; then
  ok "scripts/ci.sh passes ($(grep -E '^TOTAL' "$SCRATCH/ci.log" | awk '{print $(NF-3)}') of lines covered)"
else
  bad "scripts/ci.sh failed:"
  tail -8 "$SCRATCH/ci.log" | sed 's/^/     /'
fi

echo "== 2. The service against a throwaway Postgres =="
check "capture $CAPTURE" test -f "$CAPTURE/http/health.txt"
check "cargo build" cargo build -q --locked
pg_start
PORT="$(free_port)"
BASE="http://127.0.0.1:$PORT"
# shellcheck disable=SC2086 # PG_ENV is a list of assignments
env -i PATH="$PATH" $PG_ENV NOTIFICATION_SERVICE_PORT="$PORT" target/debug/notification-service \
  > "$SCRATCH/service.log" 2>&1 < /dev/null &
SVC=$!
if wait_http "$BASE/health" 30; then
  ok "serves $BASE"
else
  bad "no answer on $BASE/health:"
  tail -5 "$SCRATCH/service.log" | sed 's/^/     /'
fi

echo "== 3. Health exchanges match the capture =="
for exchange in "health -i /health" "health-ready -i /health/ready" "health-head -I /health"; do
  read -r name flag path <<< "$exchange"
  curl -sS -m 5 "$flag" "$BASE$path" > "$SCRATCH/$name.txt" 2>&1
  if diff <(canon_http "$CAPTURE/http/$name.txt") <(canon_http "$SCRATCH/$name.txt") > "$SCRATCH/$name.diff"; then
    ok "http/$name.txt"
  else
    bad "http/$name.txt differs (capture <, Rust >):"
    sed 's/^/     /' "$SCRATCH/$name.diff"
  fi
done

echo "== 4. sent_email DDL matches the capture =="
PG_PORT="${PG_ENV##*POSTGRES_PORT=}"
PG_PORT="${PG_PORT%% *}"
"$PG_BIN/pg_dump" -s --no-owner -t sent_email -h "$PG_DIR" -p "$PG_PORT" -U user user > "$SCRATCH/schema.sql" 2>&1
if diff <(canon_ddl "$CAPTURE/db/schema.sql") <(canon_ddl "$SCRATCH/schema.sql") > "$SCRATCH/ddl.diff"; then
  ok "pg_dump -t sent_email equals db/schema.sql ($(canon_ddl "$SCRATCH/schema.sql" | wc -l | tr -d ' ') statements)"
else
  bad "pg_dump -t sent_email differs (capture <, Rust >):"
  sed 's/^/     /' "$SCRATCH/ddl.diff"
fi

echo "== 5. SIGTERM =="
kill -TERM "$SVC"
wait "$SVC"
expect "exit status" 0 "$?"
SVC=""

verify_summary "RPN-2 walking skeleton"
