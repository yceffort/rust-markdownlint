#!/usr/bin/env bash
# GitHub Release 에서 이 플랫폼의 rust-markdownlint 바이너리를 내려받아 검증(.sha256)하고 푼다.
#
# 사용법: install-release.sh <vX.Y.Z> <dest-dir>
# 결과: <dest-dir>/rust-markdownlint (Windows 는 .exe). 이미 있으면 아무것도 하지 않는다.
# pre-commit 훅(scripts/pre-commit-hook.sh)과 GitHub Action(action.yml)이 같이 쓴다.
set -euo pipefail

version="$1"
dest="$2"
# 테스트용: 다른 호스트(로컬 http.server 등)에서 같은 경로 구조로 받는다
base_url="${RUST_MARKDOWNLINT_DOWNLOAD_BASE:-https://github.com/yceffort/rust-markdownlint/releases/download}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Linux-x86_64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
  MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64) target=x86_64-pc-windows-msvc ;;
  *)
    echo "rust-markdownlint: no release binary for $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

exe=rust-markdownlint
ext=tar.gz
if [ "$target" = x86_64-pc-windows-msvc ]; then
  exe=rust-markdownlint.exe
  ext=zip
fi
if [ -x "$dest/$exe" ]; then
  exit 0
fi

asset="rust-markdownlint-$version-$target.$ext"
url="$base_url/$version/$asset"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "rust-markdownlint: downloading $url" >&2
curl -fsSL --retry 3 -o "$tmp/$asset" "$url"
curl -fsSL --retry 3 -o "$tmp/$asset.sha256" "$url.sha256"
(cd "$tmp" && { sha256sum -c "$asset.sha256" 2> /dev/null || shasum -a 256 -c "$asset.sha256"; } > /dev/null)

mkdir -p "$dest"
if [ "$ext" = zip ]; then
  if command -v unzip > /dev/null; then
    unzip -oq "$tmp/$asset" -d "$dest"
  else
    powershell.exe -NoProfile -Command "Expand-Archive -Force -LiteralPath '$(cygpath -w "$tmp/$asset")' -DestinationPath '$(cygpath -w "$dest")'"
  fi
else
  tar xzf "$tmp/$asset" -C "$dest"
fi
chmod +x "$dest/$exe"
