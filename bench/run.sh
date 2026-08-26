#!/usr/bin/env bash
# rust-markdownlint 와 markdownlint-cli2@0.22.1 을 같은 코퍼스에 실행해 결과 diff 와 속도를 비교한다.
#
# 사용법: bench/run.sh <MD0XX|all>   (all = 원본 기본 설정, 규칙 전체 + inline config)
#   SCALE=N 으로 코퍼스를 N 배 복제 (기본 1)
set -euo pipefail
cd "$(dirname "$0")"
RULE="${1:-all}"
SCALE="${SCALE:-1}"

# 코퍼스: 원본 markdownlint test/*.md 사본 (crates/core/tests/fixtures/markdownlint)
rm -rf corpus && mkdir -p corpus
for ((i = 1; i <= SCALE; i++)); do
  mkdir -p "corpus/$i"
  cp ../crates/core/tests/fixtures/markdownlint/*.md "corpus/$i/"
done

if [ "$RULE" = all ]; then
  echo '{ "noBanner": true }' > corpus/.markdownlint-cli2.jsonc
else
  # noInlineConfig: fixture 의 configure-file 주석이 다른 규칙을 켜는 것을 막는다
  echo "{ \"noBanner\": true, \"noInlineConfig\": true, \"config\": { \"default\": false, \"$RULE\": true } }" > corpus/.markdownlint-cli2.jsonc
fi

[ -d node_modules ] || npm install --no-audit --no-fund
cargo build --release -q --manifest-path ../Cargo.toml
RS="$(cd .. && pwd)/target/release/rust-markdownlint"
JS="$(pwd)/node_modules/.bin/markdownlint-cli2"

(cd corpus && "$RS" '**/*.md' 2> ../out.rs.txt || true)
(cd corpus && "$JS" '**/*.md' 2> ../out.js.txt || true)
if diff -q out.rs.txt out.js.txt > /dev/null; then
  echo "결과 diff: 없음 (오류 $(wc -l < out.rs.txt | tr -d ' ') 줄)"
else
  echo "결과 diff: 있음"
  # pipefail 상태라 diff 의 exit 1 이 스크립트를 죽이지 않게 한다
  diff out.rs.txt out.js.txt | head -20 || true
fi

hyperfine --warmup 3 -i --export-json bench.json \
  -n rust "cd corpus && '$RS' '**/*.md'" \
  -n cli2 "cd corpus && '$JS' '**/*.md'"

python3 - "$RULE" <<'EOF'
import json, sys, datetime
rs, js = json.load(open("bench.json"))["results"]
f = lambda r: f"{r['mean']*1000:.1f} ± {r['stddev']*1000:.1f}"
print("\nRESULTS.md 행:")
print(f"| {sys.argv[1]} | {f(js)} | {f(rs)} | {js['mean']/rs['mean']:.1f}x | {datetime.date.today()} |")
EOF
