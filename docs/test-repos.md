# 실전 저장소 9개 대조 결과 (#168)

실행일 2026-08-27. rust-markdownlint 는 `456efaf` (PR #167 머지 직후), 오라클은 markdownlint-cli2 0.22.1 (markdownlint 0.40.0). 원본 markdownlint v0.40.0 의 `test/markdownlint-test-repos-small.mjs` 가 스냅샷 테스트하는 실제 저장소 9개를 `--depth 1` 로 받아, rust 와 cli2 를 같은 cwd, 같은 인자로 돌리고 stderr 를 diff 했다.

## 결과

| 저장소 | 커밋 | 파일 수 | rust 오류 | cli2 오류 | diff 줄 |
| --- | --- | --- | --- | --- | --- |
| apache/airflow | `f727ea8544d1b0b7759e78e5dfcb257dd8292957` | 264 | 1149 | 1149 | 0 |
| electron/electron | `65bba2c24d6b7944fd9592983913e1670623594d` | 303 | 59 | 59 | 0 |
| eslint/eslint | `5634542be580750ffb1a5766470f9e9c72719696` | 409 | 0 | 0 | 0 |
| mkdocs/mkdocs | `2862536793b3c67d9d83c33e0dd6d50a791928f8` | 21 | 0 | 0 | 0 |
| mochajs/mocha | `e6b9ee773481fd739ae24caeb42f32ac0b010f95` | 16 | 298 | 298 | 4 |
| pi-hole/docs | `15cd305f5aaf747f02cba6bc29d7c198b2a27771` | 85 | 0 | 0 | 0 |
| v8/v8.dev | `031a6578036da033a0c3fb728ec58527345336b6` | 261 | 201 | 201 | 0 |
| webhintio/hint | `62bfce68b934aab205bd60beb136112a5bfa1da1` | 168 | 117 | 117 | 2 |
| webpack/webpack.js.org | `e919bc0bddf170b1cb2a79252fe252965c5c7461` | 7 | 1 | 1 | 0 |

파일 수는 두 CLI 의 `Linting: N file(s)` 줄로, 9개 모두 rust 와 cli2 가 같은 수를 보고했다. 오류 수는 stderr 줄 수, diff 줄은 `diff rs.err cli2.err` 의 `<`, `>` 줄 합계다. 총 1534 파일, 1825 오류 중 diff 6줄(3건)이고 원인은 하나다(아래).

## 절차

`scripts/compare-repos.sh [저장소명 ...]` 로 재실행한다. 인자가 없으면 9개 전부, `REPOS_DIR` (기본 `/tmp/rust-markdownlint-test-repos`) 에 클론이 있으면 재사용하고 HEAD 를 갱신하지 않는다. 오라클은 `CLI2` (기본 `bench/node_modules/.bin/markdownlint-cli2`) 로 지정한다. 저장소별 출력은 `<clone>/.compare168/{rs,cli2}.{out,err}` 에 남는다.

원본 테스트는 `globby` 로 파일을 모으고 저장소 루트의 설정 파일 하나를 `readConfig` 로 읽어(키의 `header` 를 `heading` 으로 치환, `.markdownlint-cli2.jsonc` 는 `config` 키만) 라이브러리 `lint` 를 부른다. 여기서는 두 CLI 가 같은 결과를 내는지가 목적이므로 다음처럼 맞췄다.

- glob 은 원본 테스트와 같은 패턴을 cli2 문법으로 넘긴다 (`excludeGlobs` 는 `!` 접두어). cli2 는 `dot: true` 로 globby 를 부르므로 `**/*.md` 가 `.github/` 아래도 잡는 등 원본 테스트와 파일 집합이 다를 수 있다. rust 도 같은 의미로 동작하므로 비교에는 영향이 없다.
- 설정은 저장소 루트 설정 파일을 위 규칙대로 `.markdownlint-cli2.jsonc` 로 변환해 `--config` 로 넘긴다. 저장소 자체의 루트 설정 파일(`.markdownlint.*`, `.markdownlint-cli2.*`, `.markdownlintrc`) 은 실행 동안 `.compare168/` 로 비켜 둬 cli2 의 디렉터리 설정 탐색이 끼어들지 않게 한다. `customRules`, `markdownItPlugins`, `outputFormatters` 는 rust 에 없으므로 제거한다.
- `extends` 가 npm 모듈이면 그 파일을 `.compare168/extends.json` 으로 복사하고 상대 경로로 바꾼다. rust 의 `extends` 는 설정 파일 기준 상대 경로만 해석하고 node 모듈 탐색을 하지 않기 때문이다. eslint 의 `markdownlint/style/prettier` 는 오라클 node_modules 에서, electron 의 `@electron/lint-roller/configs/markdownlint.json` 은 `npm pack @electron/lint-roller@^3.2.0` (3.3.0 으로 해석) 으로 받아 복사했다.
- electron 의 `.markdownlint-cli2.jsonc` 는 `customRules` (`@electron/lint-roller` 의 EMD002~004) 를 쓴다. 이 부분은 비교에서 뺐고, 설정에 남은 커스텀 규칙 이름(`no-angle-brackets` 등) 은 두 CLI 모두 무시했다.
- mochajs/mocha 는 현재 HEAD 에 `.markdownlint.json` 이 없다 (원본 테스트가 가리키는 파일이 사라졌다). 기본 설정 `{ "config": {} }` 으로 실행했다.
- 저장소가 커밋을 고정하지 않으므로 재실행 시 파일 수와 오류 수는 달라질 수 있다.

저장소별 인자 (cwd 는 클론 루트, 앞에 `--config .compare168/.markdownlint-cli2.jsonc` 가 붙는다):

| 저장소 | 원본 설정 파일 | glob |
| --- | --- | --- |
| apache/airflow | `.markdownlint.yml` | `**/*.{md,mdown,markdown}` |
| electron/electron | `.markdownlint-cli2.jsonc` | `*.md` `docs/**/*.md` |
| eslint/eslint | `.markdownlint.yml` (extends `markdownlint/style/prettier`) | `docs/**/*.md` |
| mkdocs/mkdocs | `.markdownlint.yaml` | `README.md` `CONTRIBUTING.md` `docs` `!docs/CNAME` `!docs/**/*.css` `!docs/**/*.png` `!docs/**/*.py` `!docs/**/*.svg` |
| mochajs/mocha | 없음 | `*.md` `docs/**/*.md` `.github/*.md` `lib/**/*.md` `test/**/*.md` `example/**/*.md` `!CHANGELOG.md` |
| pi-hole/docs | `.markdownlint.json` | `**/*.md` |
| v8/v8.dev | `.markdownlint.json` | `src/**/*.md` |
| webhintio/hint | `.markdownlintrc` | `**/*.md` `!**/CHANGELOG.md` |
| webpack/webpack.js.org | `.markdownlint.json` | `**/*.md` |

## diff 원인

### 1. JS 문자열 길이(UTF-16 코드 유닛) 대 Rust 문자 수: MD013 `Actual` 과 `ellipsify` 컨텍스트 (3건 전부)

mocha `.github/CONTRIBUTING.md` 73, 79행 (🎸 포함) 의 MD013 `Actual` 이 rust 163/261, cli2 164/262 로 1 차이. webhint `.github/ISSUE_TEMPLATE/---request-documentation-improvements.md` 10행 (📚 포함) 의 MD025 컨텍스트가 rust `"📚 Request documentation enhanc..."`, cli2 `"📚 Request documentation enhan..."` 로 한 글자 차이. 원본은 `line.length` (md013.mjs) 와 `text.length`, `text.slice(0, 30)` (helpers.cjs `ellipsify`) 를 쓰므로 BMP 밖 문자(이모지 등 서로게이트 쌍) 를 2 로 센다. rust 는 `crates/core/src/rules/md013.rs` 의 `line.chars().count()` 와 `crates/core/src/error.rs` 의 `ellipsify` 가 `chars()` 로 세어 1 로 센다. 열(column) 은 이미 UTF-16 기준으로 맞춰져 있어 두 CLI 모두 81 로 같다. 규칙/헬퍼 차이이고 파서 차이가 아니다.

최소 재현 (기본 설정, `*.md`):

```markdown
# Title

aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 🎸 bbb
```

```markdown
---
title: x
---

# 📚 Request documentation enhancements
```

```text
rust: md013.md:3:81 error MD013/line-length Line length [Expected: 80; Actual: 84]
cli2: md013.md:3:81 error MD013/line-length Line length [Expected: 80; Actual: 85]
rust: md025.md:5 error MD025/single-title/single-h1 Multiple top-level headings in the same document [Context: "📚 Request documentation enhanc..."]
cli2: md025.md:5 error MD025/single-title/single-h1 Multiple top-level headings in the same document [Context: "📚 Request documentation enhan..."]
```

수정은 별도 PR. 같은 계열로 `crates/core/src/rules/` 에 `chars().count()` 가 20개 규칙 36곳 있는데, 그중 원본이 `.length` 로 세는 값을 사용자에게 보이는 자리(오류 detail, range) 에 쓰는 곳은 같은 문제가 있을 수 있다. 이번 코퍼스에서는 MD013 과 `ellipsify` 만 드러났고 나머지는 확인하지 않았다.
