#!/usr/bin/env bash
# 원본 markdownlint v0.40.0 의 test/markdownlint-test-repos-small.mjs 가 lint 하는 실전 저장소 9개를 depth 1 로 받아
# rust-markdownlint 와 markdownlint-cli2@0.22.1 을 같은 cwd, 같은 인자로 돌리고 stderr 를 diff 한다 (#168).
#
# 사용법: scripts/compare-repos.sh [저장소명 ...]   (인자 없으면 9개 전부. 이름은 아래 repo_def 의 case 라벨)
#   REPOS_DIR=<dir>  클론과 출력 위치 (기본 /tmp/rust-markdownlint-test-repos). 이미 클론돼 있으면 재사용하고 HEAD 를 갱신하지 않는다
#   CLI2=<path>      오라클 markdownlint-cli2 실행 파일 (기본 bench/node_modules/.bin/markdownlint-cli2)
#
# 원본 테스트는 globby 로 파일을 모으고 저장소 루트의 설정 파일 하나를 readConfig 로 읽어(키의 header 를 heading 으로 치환)
# 라이브러리 lint 를 부른다. 여기서는 두 CLI 에 같은 glob 을 주고, 그 설정 파일을 .markdownlint-cli2.jsonc 로 변환해 --config 로 넘긴다.
# 저장소 자체의 루트 설정 파일은 실행 동안 .compare168/ 로 비켜 둬 cli2 의 디렉터리 설정 탐색이 끼어들지 않게 한다.
# customRules, markdownItPlugins, outputFormatters 는 rust 에 없으므로 제거한다. extends 가 npm 모듈이면 그 파일을 복사해 상대 경로로 바꾼다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPOS_DIR="${REPOS_DIR:-/tmp/rust-markdownlint-test-repos}"
CLI2="${CLI2:-$ROOT/bench/node_modules/.bin/markdownlint-cli2}"
MODULES_DIR="$REPOS_DIR/_modules"
RS="$ROOT/target/release/rust-markdownlint"

[ -x "$CLI2" ] || { echo "markdownlint-cli2 를 찾을 수 없음: $CLI2 (CLI2=<path> 로 지정)" >&2; exit 1; }
# cli2 가 설치된 node_modules. 변환 스크립트의 jsonc-parser, js-yaml 과 extends 모듈(markdownlint/style/*) 을 여기서 찾는다
CLI2_NM="$(dirname "$(dirname "$(realpath "$CLI2")")")"
cargo build --release -q -p rust-markdownlint-cli --manifest-path "$ROOT/Cargo.toml"
mkdir -p "$REPOS_DIR" "$MODULES_DIR"

# 원본 테스트와 같은 저장소, glob, 설정 파일 (excludeGlobs 는 ! 접두어)
repo_def() {
  case "$1" in
    apache-airflow) spec=apache/airflow; config=.markdownlint.yml; globs=('**/*.{md,mdown,markdown}') ;;
    electron-electron) spec=electron/electron; config=.markdownlint-cli2.jsonc; globs=('*.md' 'docs/**/*.md') ;;
    eslint-eslint) spec=eslint/eslint; config=.markdownlint.yml; globs=('docs/**/*.md') ;;
    mkdocs-mkdocs) spec=mkdocs/mkdocs; config=.markdownlint.yaml
      globs=(README.md CONTRIBUTING.md docs '!docs/CNAME' '!docs/**/*.css' '!docs/**/*.png' '!docs/**/*.py' '!docs/**/*.svg') ;;
    mochajs-mocha) spec=mochajs/mocha; config=.markdownlint.json
      globs=('*.md' 'docs/**/*.md' '.github/*.md' 'lib/**/*.md' 'test/**/*.md' 'example/**/*.md' '!CHANGELOG.md') ;;
    pi-hole-docs) spec=pi-hole/docs; config=.markdownlint.json; globs=('**/*.md') ;;
    v8-v8-dev) spec=v8/v8.dev; config=.markdownlint.json; globs=('src/**/*.md') ;;
    webhintio-hint) spec=webhintio/hint; config=.markdownlintrc; globs=('**/*.md' '!**/CHANGELOG.md') ;;
    webpack-webpack-js-org) spec=webpack/webpack.js.org; config=.markdownlint.json; globs=('**/*.md') ;;
    *) echo "알 수 없는 저장소: $1" >&2; return 1 ;;
  esac
}
ALL_REPOS=(apache-airflow electron-electron eslint-eslint mkdocs-mkdocs mochajs-mocha pi-hole-docs v8-v8-dev webhintio-hint webpack-webpack-js-org)

# npm pack 으로 패키지 하나를 $MODULES_DIR/<name> 에 푼다 (extends 설정 파일만 필요하므로 의존성은 받지 않는다)
fetch_module() {
  local name=$1 range=$2 dest tgz
  dest="$MODULES_DIR/$name"
  [ -f "$dest/package.json" ] && return
  mkdir -p "$dest"
  tgz="$(npm pack "$name@$range" --pack-destination "$MODULES_DIR" 2>/dev/null | tail -1)" || return 1
  tar -xzf "$MODULES_DIR/$tgz" -C "$dest" --strip-components=1
}

# 저장소 설정 파일 -> .markdownlint-cli2.jsonc 변환. 인자: <cli2 node_modules> <원본 설정> <출력 디렉터리> <모듈 디렉터리>
GEN_CONFIG=$(cat <<'EOF'
const fs = require("node:fs");
const path = require("node:path");
const [nm, src, outDir, modulesDir] = process.argv.slice(1);
const jsonc = require(path.join(nm, "jsonc-parser"));
const yaml = require(path.join(nm, "js-yaml"));
let options = {};
let config = {};
if (fs.existsSync(src)) {
  const text = fs.readFileSync(src, "utf8");
  if (/\.ya?ml$/u.test(src)) {
    config = yaml.load(text);
  } else {
    // 원본 lintTestRepo 의 jsoncParse: config.config || config
    const obj = jsonc.parse(text, [], { allowTrailingComma: true });
    if (obj.config) {
      options = obj;
      config = obj.config;
    } else {
      config = obj;
    }
  }
} else {
  console.error(`설정 파일 없음, 기본 설정으로 실행: ${src}`);
}
for (const key of ["customRules", "markdownItPlugins", "outputFormatters"]) {
  if (key in options) {
    console.error(`${key} 제거 (rust 에 없음)`);
    delete options[key];
  }
}
config = Object.fromEntries(Object.entries(config).map(([k, v]) => [k.replace(/header/u, "heading"), v]));
if (typeof config.extends === "string") {
  // 설정 파일 기준 상대 경로 -> npm 모듈 순으로 찾는다 (markdownlint resolveConfigExtends 와 같은 순서)
  const candidates = [path.resolve(path.dirname(src), config.extends)];
  for (const base of [modulesDir, nm]) {
    candidates.push(path.join(base, config.extends), path.join(base, `${config.extends}.json`));
  }
  const found = candidates.find((p) => fs.existsSync(p) && fs.statSync(p).isFile());
  if (!found) {
    throw new Error(`extends 를 찾을 수 없음: ${config.extends}`);
  }
  fs.copyFileSync(found, path.join(outDir, "extends.json"));
  console.error(`extends ${config.extends} -> ${found}`);
  config.extends = "./extends.json";
}
options.config = config;
fs.writeFileSync(path.join(outDir, ".markdownlint-cli2.jsonc"), `${JSON.stringify(options, null, 2)}\n`);
EOF
)

ROWS=()
run_repo() {
  local name=$1 spec config globs dir work sha f n_rs n_cli2 e_rs e_cli2 d
  repo_def "$name"
  dir="$REPOS_DIR/$name"
  work="$dir/.compare168"
  echo "== $name ($spec)"
  # run_repo 는 || 뒤에서 불려 set -e 가 꺼지므로 실패해야 할 단계에 || return 1 을 명시한다
  if [ ! -d "$dir/.git" ]; then
    git clone -q --depth 1 "https://github.com/$spec.git" "$dir" || return 1
  fi
  sha="$(git -C "$dir" rev-parse --short HEAD)"

  # 이전 실행 흔적 정리: 비켜 둔 설정 파일을 되돌리고 작업 디렉터리를 비운다
  rm -rf "$work"
  git -C "$dir" checkout -q -- .
  mkdir -p "$work"
  for f in "$dir"/.markdownlint-cli2.* "$dir"/.markdownlint.* "$dir/.markdownlintrc"; do
    [ -e "$f" ] && mv "$f" "$work/orig-$(basename "$f")"
  done
  if [ "$name" = electron-electron ]; then
    # 원본은 @electron/lint-roller 를 설치해 extends 와 customRules 에 쓴다. extends 설정 파일만 받고 customRules 는 비교에서 뺀다
    fetch_module "@electron/lint-roller" "$(node -p "require('$dir/package.json').devDependencies['@electron/lint-roller']")" || return 1
  fi
  node -e "$GEN_CONFIG" "$CLI2_NM" "$work/orig-$config" "$work" "$MODULES_DIR" || return 1

  echo "   args: --config .compare168/.markdownlint-cli2.jsonc ${globs[*]}"
  (cd "$dir" && "$RS" --config .compare168/.markdownlint-cli2.jsonc "${globs[@]}" > "$work/rs.out" 2> "$work/rs.err") || true
  (cd "$dir" && "$CLI2" --config .compare168/.markdownlint-cli2.jsonc "${globs[@]}" > "$work/cli2.out" 2> "$work/cli2.err") || true

  n_rs="$(sed -n 's/^Linting: \([0-9]*\) file(s)$/\1/p' "$work/rs.out")"
  n_cli2="$(sed -n 's/^Linting: \([0-9]*\) file(s)$/\1/p' "$work/cli2.out")"
  e_rs="$(wc -l < "$work/rs.err" | tr -d ' ')"
  e_cli2="$(wc -l < "$work/cli2.err" | tr -d ' ')"
  # pipefail 상태라 diff 의 exit 1 이 스크립트를 죽이지 않게 한다
  d="$(diff "$work/rs.err" "$work/cli2.err" | grep -c '^[<>]' || true)"
  [ "$n_rs" = "$n_cli2" ] || n_rs="$n_rs(rust)/$n_cli2(cli2)"
  echo "   파일 ${n_rs:-?}, 오류 rust $e_rs / cli2 $e_cli2, diff $d 줄 -> $work/{rs,cli2}.err"
  ROWS+=("| $name | $sha | ${n_rs:-?} | $e_rs | $e_cli2 | $d |")
}

REPOS=("$@")
[ $# -gt 0 ] || REPOS=("${ALL_REPOS[@]}")
for name in "${REPOS[@]}"; do
  run_repo "$name" || ROWS+=("| $name | 실패 | | | | |")
done

echo
echo "| 저장소 | 커밋 | 파일 수 | rust 오류 | cli2 오류 | diff 줄 |"
echo "| --- | --- | --- | --- | --- | --- |"
printf '%s\n' "${ROWS[@]}"
