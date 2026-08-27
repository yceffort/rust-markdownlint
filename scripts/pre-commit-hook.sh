#!/usr/bin/env bash
# pre-commit `language: script` 훅. 이 체크아웃(= `rev` 태그)의 릴리즈 바이너리를 처음 한 번 내려받아
# 훅 저장소 안(.bin/)에 두고 실행한다. cargo 도 Node 도 필요 없다.
#
# pre-commit 은 옵션(--fix) 뒤에 파일 목록을 붙여 부른다. 파일은 `:` 리터럴 경로로 넘겨
# 이름에 `[`, `*` 같은 glob 문자가 있어도 그대로 lint 한다.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="v$(sed -nE 's/^version = "(.*)"/\1/p' "$root/crates/cli/Cargo.toml" | head -1)"
dest="$root/.bin/$version"
bash "$root/scripts/install-release.sh" "$version" "$dest"

args=()
for arg in "$@"; do
  case "$arg" in
    --*) args+=("$arg") ;;
    *) args+=(":$arg") ;;
  esac
done
exec "$dest/rust-markdownlint" "${args[@]}"
