#!/usr/bin/env bash
# Every test STATUS.md cites must exist; every "Not done" row must cite a test
# that is #[ignore]d, and every "Done" row a test that is not.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f STATUS.md ] || { echo "FAIL: STATUS.md missing"; exit 1; }
fail=0
while IFS='|' read -r _ cap status proof _; do
  status=$(echo "$status" | xargs | sed -E 's/^(Done|Partial|Not done).*/\1/'); test=$(echo "$proof" | grep -oE '`[a-z_]+`' | head -1 | tr -d '`')
  case "$status" in Done|Partial|"Not done") ;; *) continue ;; esac
  [ -n "$test" ] || { echo "FAIL: no test cited for:$cap"; fail=1; continue; }
  loc=$(grep -rn --include=*.rs -E "fn $test\(" crates | head -1)
  [ -n "$loc" ] || { echo "FAIL: test not found: $test"; fail=1; continue; }
  file=${loc%%:*}; line=$(echo "$loc" | cut -d: -f2)
  ignored=$(sed -n "$((line-3)),$((line-1))p" "$file" | grep -c '#\[ignore' || true)
  if [ "$status" = "Not done" ] && [ "$ignored" = 0 ]; then echo "FAIL: '$test' backs a Not done row but is not ignored"; fail=1; fi
  if [ "$status" = "Done" ] && [ "$ignored" != 0 ]; then echo "FAIL: '$test' backs a Done row but is ignored"; fail=1; fi
done < <(grep '^|' STATUS.md | tail -n +3)
[ "$fail" = 0 ] && echo PASS || exit 1
