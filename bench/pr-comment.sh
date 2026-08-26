#!/usr/bin/env bash
# PR 에서 변경된 규칙(crates/core/src/rules/md*.rs)만 bench/run.sh 로 돌리고
# PR 댓글 본문을 bench/comment.md 에 쓴다. 변경된 규칙이 없으면 all 로 실행한다.
#
# 사용법: BASE=origin/main bench/pr-comment.sh
set -euo pipefail
cd "$(dirname "$0")/.."
BASE="${BASE:-origin/main}"

RULES=$(git diff --name-only --diff-filter=d "$BASE...HEAD" -- 'crates/core/src/rules/md*.rs' \
  | sed -E 's|.*/md([0-9]+)\.rs|MD\1|' | sort -u | xargs echo)
RULES="${RULES:-all}"
cd bench

{
  echo "<!-- bench -->"
  echo "## 벤치마크 ($RULES)"
  echo
  echo "cli2 = markdownlint-cli2, 시간은 mean ± σ (ms), 배율은 cli2 / rust."
  echo "CI 러너 기준이라 절대값은 \`bench/RESULTS.md\` (Apple Silicon) 와 다르다."
  echo
  echo "| 규칙 | cli2 (ms) | rust (ms) | 배율 | 결과 diff |"
  echo "|------|-----------|-----------|------|-----------|"
} > comment.md

DETAILS=""
for rule in $RULES; do
  out=$(./run.sh "$rule" 2>&1)
  echo "$out"
  diff=$(sed -n 's/^결과 diff: //p' <<< "$out")
  row=$(grep '^| ' <<< "$out" | tail -1 | cut -d'|' -f2-5)
  echo "|$row| $diff |" >> comment.md
  if [ "$diff" = "있음" ]; then
    DETAILS+=$'\n'"<details><summary>$rule 결과 diff (rust &lt; &gt; cli2)</summary>"$'\n\n```diff\n'
    DETAILS+=$(diff out.rs.txt out.js.txt | head -40 || true)
    DETAILS+=$'\n```\n</details>\n'
  fi
done
echo "$DETAILS" >> comment.md
