#!/usr/bin/env bash
# 버전 문자열이 저장소 전체에서 일치하는지 본다.
#
#   scripts/check-version.sh            crates/cli/Cargo.toml 을 기준으로 나머지를 검사
#   scripts/check-version.sh v0.1.2     주어진 버전과 전부 같은지 검사 (릴리즈 워크플로용)
#
# 잡으려는 실수는 "Cargo.toml 만 올리고 README 나 npm package.json 을 빠뜨림" 이다.
set -uo pipefail
cd "$(dirname "$0")/.."

want="${1:-}"
want="${want#v}"
cli=$(sed -nE 's/^version = "(.*)"/\1/p' crates/cli/Cargo.toml | head -1)
[ -n "$want" ] || want="$cli"

fail=0
bad() {
  echo "  $1" >&2
  fail=1
}

# 값이 정확히 일치해야 하는 곳
[ "$cli" = "$want" ] || bad "crates/cli/Cargo.toml: $cli (기대 $want)"

core=$(sed -nE 's/^version = "(.*)"/\1/p' crates/core/Cargo.toml | head -1)
[ "$core" = "$want" ] || bad "crates/core/Cargo.toml: $core (기대 $want)"

for pkg in npm/rust-markdownlint npm/platforms/*; do
  v=$(node -p "require('./$pkg/package.json').version")
  [ "$v" = "$want" ] || bad "$pkg/package.json: $v (기대 $want)"
done

node -e '
  const deps = require("./npm/rust-markdownlint/package.json").optionalDependencies;
  const bad = Object.entries(deps).filter(([, v]) => v !== process.argv[1]);
  if (bad.length) {
    console.error("  npm/rust-markdownlint/package.json optionalDependencies: " + JSON.stringify(bad));
    process.exit(1);
  }
' "$want" || fail=1

# 버전이 문장 안에 박혀 있어 최소한 등장은 해야 하는 곳.
# 올리면서 빠뜨리면 새 버전 문자열이 아예 없으므로 여기서 걸린다.
for f in Cargo.lock README.md npm/rust-markdownlint/README.md action.yml .pre-commit-hooks.yaml; do
  grep -q -- "$want" "$f" || bad "$f: $want 가 없다"
done

if [ "$fail" != 0 ]; then
  echo "버전 불일치. scripts/bump-version.sh <버전> 으로 한 번에 맞출 수 있다." >&2
  exit 1
fi
echo "버전 $want, 13곳 일치"
