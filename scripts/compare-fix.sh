#!/usr/bin/env bash
# 원본 markdownlint test/*.md 사본(crates/core/tests/fixtures/markdownlint) 388개에 rust-markdownlint --fix 와
# markdownlint-cli2 --fix 를 각각 적용해 결과 파일 전체와 stderr 를 diff 한다 (bench/run.sh all 과 같은
# `{ "noBanner": true }` 설정).
#
# 사용법: scripts/compare-fix.sh [markdownlint-cli2 실행 파일]   (기본 bench/node_modules/.bin/markdownlint-cli2)
#   OUT=<dir> 로 작업 디렉토리 지정 (기본 mktemp). 종료 코드: diff 없으면 0.
set -euo pipefail
cd "$(dirname "$0")/.."
JS="${1:-$(pwd)/bench/node_modules/.bin/markdownlint-cli2}"
OUT="${OUT:-$(mktemp -d)}"
FIXTURES="$(pwd)/crates/core/tests/fixtures/markdownlint"

cargo build --release -q -p rust-markdownlint-cli
RS="$(pwd)/target/release/rust-markdownlint"

for side in rs js; do
  rm -rf "$OUT/$side" && mkdir -p "$OUT/$side"
  cp "$FIXTURES"/*.md "$OUT/$side/"
  echo '{ "noBanner": true }' > "$OUT/$side/.markdownlint-cli2.jsonc"
done
(cd "$OUT/rs" && "$RS" --fix '*.md' > ../rs.stdout 2> ../rs.stderr || true)
(cd "$OUT/js" && "$JS" --fix '*.md' > ../js.stdout 2> ../js.stderr || true)

total=$(ls "$OUT/rs"/*.md | wc -l | tr -d ' ')
changed=$(cd "$OUT/rs" && for f in *.md; do cmp -s "$f" "$FIXTURES/$f" || echo "$f"; done | wc -l | tr -d ' ')
status=0
if diff -r "$OUT/rs" "$OUT/js" > "$OUT/files.diff"; then
  echo "fixed files: $total files, $changed changed by --fix, diff against cli2: none"
else
  echo "fixed files: $total files, $changed changed by --fix, diff against cli2: $(grep -c '^diff ' "$OUT/files.diff") file(s) differ (see $OUT/files.diff)"
  status=1
fi
if diff "$OUT/rs.stderr" "$OUT/js.stderr" > "$OUT/stderr.diff"; then
  echo "stderr after --fix: $(wc -l < "$OUT/rs.stderr" | tr -d ' ') lines, diff against cli2: none"
else
  echo "stderr after --fix: diff against cli2 (see $OUT/stderr.diff)"
  head -20 "$OUT/stderr.diff"
  status=1
fi
exit $status
