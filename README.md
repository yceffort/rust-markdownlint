# rust-markdownlint

[![CI](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml/badge.svg)](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml)

[markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2) 와 동일하게 동작하는 것을 목표로 하는 Rust 구현입니다. 기존 `.markdownlint-cli2.{jsonc,yaml}`, `.markdownlint.{jsonc,json,yaml,yml}` 설정을 그대로 사용할 수 있는 drop-in 대체를 지향합니다.

아직 개발 초기 단계입니다. 진행 상황은 [마일스톤](https://github.com/yceffort/rust-markdownlint/milestones) 을 참고하시기 바랍니다.

## 설치

```bash
cargo install --path crates/cli
```

`rust-markdownlint` 바이너리가 설치됩니다.

## 사용법

명령줄 인터페이스는 markdownlint-cli2 v0.22.1 과 같습니다.

```bash
rust-markdownlint "**/*.md" "#node_modules"
rust-markdownlint --fix "docs/**/*.md"
rust-markdownlint --config .markdownlint-cli2.jsonc "*.md"
cat README.md | rust-markdownlint -          # stdin 을 lint
cat README.md | rust-markdownlint --format   # stdin 을 고쳐 stdout 으로
rust-markdownlint --help
```

- glob 은 globby 규칙을 따릅니다. `!` 또는 `#` 으로 시작하면 제외, `:` 로 시작하면 리터럴 경로입니다.
- 설정은 디렉토리별로 cascade 됩니다. 각 디렉토리의 `.markdownlint-cli2.{jsonc,yaml}` 은 부모 옵션과 병합되고, `.markdownlint.{jsonc,json,yaml,yml}` 은 부모 설정을 대체합니다.
- 출력은 배너 한 줄을 제외하고 markdownlint-cli2 와 바이트 단위로 같습니다. 결과는 stderr, 진행 상황은 stdout 입니다.
- exit code: 0 (문제 없음 또는 경고만), 1 (오류 있음), 2 (도움말, 잘못된 설정, 예외).

## 원본과 다른 점

JavaScript 모듈 로딩이 필요한 기능은 지원하지 않습니다.

- `.markdownlint-cli2.{cjs,mjs}`, `.markdownlint.{cjs,mjs}` 설정 파일을 발견하면 오류로 종료합니다 (exit 2).
- 옵션의 `customRules`, `markdownItPlugins`, `outputFormatters`, `modulePaths` 는 stderr 에 경고 한 줄을 출력하고 무시합니다. 기본 포매터만 제공합니다.
- 결과의 파일명 정렬은 ICU `localeCompare` 를 ASCII 범위에서 근사합니다. 비 ASCII 파일명은 코드 포인트 순입니다.
- 현재 구현된 규칙은 M0 뼈대 범위인 MD018, MD047 뿐입니다. 나머지 규칙은 마일스톤에 따라 추가됩니다.

## 개발

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

규칙 하나의 결과를 원본 markdownlint 기대값과 대조하려면 `cargo test -p rust-markdownlint --test rules_snapshot -- MD047` 처럼 규칙 이름으로 필터합니다. 기대값은 `node scripts/dump-expected.mjs <markdownlint@0.40.0 패키지 경로>` 로 다시 생성합니다.

### 벤치마크

원본 markdownlint-cli2 v0.22.1 과 같은 코퍼스에서 결과를 diff 하고 속도를 비교합니다 (`hyperfine`, `node` 필요).

```bash
bench/run.sh MD047   # 규칙 하나
bench/run.sh all     # 포팅된 규칙 전체
SCALE=10 bench/run.sh all   # 코퍼스 10배 복제
```

결과는 `bench/RESULTS.md` 에 기록합니다.
