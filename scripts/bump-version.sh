#!/usr/bin/env bash
# 버전을 한 번에 올린다.
#
#   scripts/bump-version.sh 0.1.2
#   scripts/bump-version.sh v0.1.2   (v 접두어도 받는다)
#
# 현재 버전은 crates/cli/Cargo.toml 에서 읽고, 아래 목록에서 그 문자열을 새 버전으로 바꾼다.
# `v0.1.1` 같은 형태도 `0.1.1` 을 포함하므로 같이 바뀐다.
# .github/ 는 건드리지 않는다. release.yml 의 `예: v0.1.1` 은 설명용 예시다.
set -euo pipefail
cd "$(dirname "$0")/.."

new="${1:-}"
new="${new#v}"
if ! [[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "사용법: scripts/bump-version.sh <버전>   (예: 0.1.2)" >&2
  exit 2
fi

old=$(sed -nE 's/^version = "(.*)"/\1/p' crates/cli/Cargo.toml | head -1)
if [ "$old" = "$new" ]; then
  echo "이미 $new 다."
  exit 0
fi

files=(
  crates/cli/Cargo.toml
  crates/core/Cargo.toml
  npm/rust-markdownlint/package.json
  npm/platforms/darwin-arm64/package.json
  npm/platforms/darwin-x64/package.json
  npm/platforms/linux-arm64/package.json
  npm/platforms/linux-x64/package.json
  npm/platforms/win32-x64/package.json
  README.md
  npm/rust-markdownlint/README.md
  action.yml
  .pre-commit-hooks.yaml
)

for f in "${files[@]}"; do
  before=$(grep -c -- "$old" "$f" || true)
  if [ "$before" = 0 ]; then
    echo "경고: $f 에 $old 가 없다. 확인할 것." >&2
    continue
  fi
  # BSD sed 와 GNU sed 모두에서 도는 형태
  perl -pi -e "s/\Q$old\E/$new/g" "$f"
  echo "  $f ($before 곳)"
done

# Cargo.lock 의 워크스페이스 멤버 버전을 갱신한다
cargo update --workspace --quiet
echo "  Cargo.lock"

echo
echo "$old -> $new"
scripts/check-version.sh "$new"
