# rust-markdownlint 설계 문서

작성일: 2026-08-24

## 1. 목표

[markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2) v0.22.1
(markdownlint v0.40.0)과 **동일하게 동작하는** 단일 바이너리 CLI를 Rust로 구현한다.

"동일"의 기준:

- 기존 `.markdownlint-cli2.{jsonc,yaml}`, `.markdownlint.{jsonc,json,yaml,yml}` 을
  수정 없이 그대로 사용할 수 있다.
- 같은 입력에 대해 같은 위치, 같은 규칙, 같은 메시지, 같은 exit code를 낸다.
- `--fix` 결과 파일이 원본과 동일하다.
- 원본 저장소의 `test/*.md` 와 snapshot 을 회귀 테스트로 통과한다.

## 2. 범위

### 포함

- 규칙 54개 전부 (MD001~MD060, 결번 제외. 아래 §6 표 참고)
- CLI 인자: `--config`, `--configPointer`, `--fix`, `--format`, `--help`, `--no-globs`, `-`, `--`
- glob 열거 (globby 의미론: `dot: true`, 디렉토리 자동 확장, `!` 부정, `#` 접두어, `:` 리터럴 경로)
- 설정 파일 탐색과 디렉토리 cascade, `extends`, `--configPointer`(RFC 6901)
- 옵션 키: `config`, `fix`, `frontMatter`, `gitignore`, `globs`, `ignores`, `noBanner`,
  `noInlineConfig`, `noProgress`, `showFound`
- 인라인 주석: `disable`, `enable`, `capture`, `restore`, `disable-file`, `enable-file`,
  `disable-line`, `disable-next-line`, `configure-file`
- 기본 출력 포매터, 배너, 진행 메시지, exit code 0/1/2
- `--fix` (원본 `applyFixes` 알고리즘 그대로)
- front matter (YAML/TOML/JSON) 제거와 줄 번호 보정

### 제외 (명시적 비지원)

- `customRules`, `markdownItPlugins`, `outputFormatters`, `modulePaths` (JS 플러그인)
- `.cjs`/`.mjs` 설정 파일. 발견하면 오류 메시지 출력 후 exit 2
- `package.json` 자동 탐색 (원본도 미지원)
- `.markdownlintignore` (원본도 미지원)
- LSP, 에디터 통합

이 키들이 설정에 존재하면 무시하지 않고 경고를 stderr 에 한 줄 출력한다.
(조용히 무시하면 사용자가 플러그인이 적용된 줄로 오해한다.)

## 3. 핵심 결정: 파서

markdownlint 규칙은 전부 micromark 의 **concrete token tree**
(`atxHeadingSequence`, `whitespace`, `lineEnding`, `listItemPrefix` 등, 모든 바이트가
토큰으로 설명됨)를 순회한다. 이 토큰을 공개 API로 제공하는 Rust 크레이트는 없다.

**결정**: `markdown` 크레이트 1.0.0 (markdown-rs, micromark 의 1:1 포트, MIT)을
`crates/markdown-rs/` 에 vendoring 하고 내부 `event::Event`, `event::Name`,
`parser::parse` 를 공개하는 최소 패치만 가한다. 그 위에 원본 `MicromarkToken` 과
동형인 트리를 만드는 어댑터를 둔다.

기각한 대안: mdast(공개 API) + 원문 라인 스캔. 규칙 절반 이상이 재구현이 되어
호환성 검증이 불가능하다. comrak(바이트 offset 없음, 인라인 sourcepos 불안정),
pulldown-cmark(이벤트가 너무 거침, autolink literal 없음)도 부적합.

위험: 상류는 2025-04 이후 저활동. 패닉/성능 이슈(GFM 테이블 셀 수 제곱 등)는 직접
고친다. 패치는 `crates/markdown-rs/PATCHES.md` 에 기록한다.

파서 옵션은 원본과 동일: `gfm` (autolink literal, footnote, table, strikethrough,
tasklist), `frontmatter`, `math`, `directive`. `.mdx` 확장자는 v1 범위에서 제외한다.

## 4. 아키텍처

```
Cargo workspace (edition 2024, rust-version 1.88)
├── crates/markdown-rs/        vendoring + 최소 패치
├── crates/core/               라이브러리 `rust_markdownlint`
│   ├── parser/
│   │   ├── token.rs           Token { kind: Name, start/end (line, col), text: &str, children, parent }
│   │   ├── build.rs           Event 스트림 → Token 트리 (arena Vec<Token>, 인덱스 참조)
│   │   └── helpers.rs         filter_by_types, get_descendants_by_type, get_parent_of_type,
│   │                          in_html_flow, get_block_quote_prefix_text 등 원본 helpers 포팅
│   ├── rules/
│   │   ├── mod.rs             trait Rule, RuleMeta { names, description, tags, parser, fixable }
│   │   ├── registry.rs        전체 규칙 목록, alias/tag → 규칙 매핑 (대소문자 무시)
│   │   └── md0XX.rs           규칙 하나당 파일 하나
│   ├── config/
│   │   ├── value.rs           설정 표현 = serde_json::Value (jsonc/yaml/toml 공통)
│   │   ├── load.rs            jsonc-parser, serde-saphyr, toml 로 파일 읽기, extends 재귀
│   │   ├── effective.rs       default/alias/tag 해석 → 규칙별 (enabled, severity, params)
│   │   └── options.rs         cli2 옵션 객체 스키마, merge_options
│   ├── inline.rs              인라인 주석 처리 → 줄별 활성 규칙 집합
│   ├── front_matter.rs        기본 정규식, 사용자 정규식, 줄 수 계산
│   ├── fix.rs                 apply_fix, apply_fixes (정렬, 중복 제거, 충돌 스킵)
│   ├── error.rs               LintError { line, rule_names, description, detail, context, range, fix_info, severity }
│   └── lint.rs                lint_content(): BOM → front matter → 인라인 주석 → 파싱 → 규칙 실행 → 정렬
└── crates/cli/                바이너리 `rust-markdownlint`
    ├── argv.rs                원본과 동일한 단일 패스 파서 (clap 미사용. 원본은 `--flag=value` 미지원,
    │                          알 수 없는 `--xyz` 를 glob 으로 취급하므로 clap 과 의미가 어긋남)
    ├── globs.rs               패턴 정규화, globset + ignore(WalkBuilder) 로 열거
    ├── dirs.rs                디렉토리별 설정 탐색과 cascade (createDirInfos 포팅)
    ├── output.rs              배너, Finding/Found/Linting/Summary, 기본 포매터
    └── main.rs                흐름 제어, --fix 루프, exit code
```

### 규칙 인터페이스

```rust
pub struct LintContext<'a> {
    pub name: &'a str,
    pub lines: Vec<&'a str>,            // HTML 주석 본문이 '.' 로 치환된 줄
    pub tokens: &'a TokenTree,          // 원문 기준 파싱
    pub front_matter_lines: usize,
    pub config: &'a RuleParams,         // 규칙별 파라미터 (serde_json::Value 맵)
}

pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check(&self, ctx: &LintContext, out: &mut ErrorSink);
}
```

`ErrorSink` 는 원본 `onError` 검증(줄 범위, range, fixInfo 범위)을 수행하고
`add_error`, `add_error_detail_if`, `add_error_context`(ellipsify 30자 규칙) 를 제공한다.
규칙은 원본 `lib/md0XX.mjs` 를 함수 단위로 1:1 포팅한다.

### 데이터 흐름 (파일 하나)

1. 파일 읽기 → BOM 제거
2. front matter 정규식 매치 (index 0 만) → 제거, 줄 수 기록
3. 인라인 주석 스캔 → `configure-file` 반영 → 유효 설정 계산 → 줄별 활성 규칙 맵
4. 활성 규칙 중 `parser: micromark` 가 하나라도 있으면 토큰 파싱
5. HTML 주석 본문 치환 → `lines`
6. 규칙 실행, 비활성 줄의 에러 드롭, 줄 번호에 front matter 오프셋 가산
7. `ruleNames[0]` → `lineNumber` 순 정렬

### 설정 cascade (원본 `createDirInfos` 그대로)

- base 옵션 = `{fix: --fix}` ← `--config` 파일 ← cwd 의 `.markdownlint-cli2.*`
  (`merge_options`: 얕은 병합, `config` 만 key 단위 병합)
- 매치된 파일의 디렉토리와 base 까지의 모든 조상에 dirInfo. 설정 파일 없는 디렉토리는
  부모로 흡수.
- 자식 옵션 = merge_options(부모 옵션, 자식 옵션). `.markdownlint.*` 는 병합 없이 대체하며,
  체인 어딘가의 옵션에 `config` 가 있으면 조상의 `.markdownlint.*` 는 상속되지 않는다.
- 유효 config = 같은 디렉토리 `.markdownlint.*` 우선, 없으면 옵션의 `config`.
- base 전용 키: `globs`, `gitignore`, `noBanner`, `noProgress`, `showFound`.
  base 의 `ignores` 는 열거 전 `!` 패턴으로 변환, 하위 디렉토리의 `ignores` 는 열거 후 필터.

### 출력 (모두 원본 문자열과 바이트 단위 동일)

- 배너 (stdout): `rust-markdownlint v{ver} (markdownlint-cli2 v0.22.1 / markdownlint v0.40.0 compatible)`
  원본 배너와 유일하게 다른 줄이다. `noBanner` 로 끌 수 있다.
- `Finding: {패턴들}` / `Found:\n file...` / `Linting: N file(s)` / `Summary: N error(s)`
- 결과 (stderr): `{file}:{line}[:{col}] {MDxxx/alias} {desc}[ [{detail}]][ [Context: "{ctx}"]]`
  파일명은 base 기준 posix 상대 경로, `localeCompare` → line → rule 순 정렬
- exit: 0 (경고만), 1 (에러 존재), 2 (help, glob 없음, `--config` 값 없음, 예외)

### `--fix`

파일별로 fixInfo 가 있는 에러가 있으면 `apply_fixes` 후 파일에 쓰고 다시 lint 한 결과를
보고한다. `apply_fixes` 정렬: line 내림차순 → 줄 삭제(-1) 뒤로 → editColumn 내림차순 →
insertText 길이 내림차순, 완전 중복 제거, 같은 줄 겹침 스킵, 줄 끝 문자는 입력의 다수결.

### 병렬성

파일 단위로 rayon `par_iter`. 결과는 수집 후 정렬하므로 출력 순서는 결정적이다.
Phase 5 에서 도입하며 뼈대는 순차 실행.

## 5. 테스트 전략

- **규칙 회귀**: 원본 저장소 `test/*.md` 와 `test/snapshots/` 를 `fixtures/markdownlint/`
  에 복사(MIT, 출처 명시). 각 규칙 이슈는 해당 fixture 에서 자기 규칙의 기대 결과와
  일치해야 닫힌다. 비교기는 `insta` 로 규칙 이름, 줄, range, detail, context, fixInfo 를 대조.
- **CLI 회귀**: 원본 `test/` 의 시나리오 디렉토리(설정 cascade, globs, ignores, fix) 를
  `assert_cmd` + `insta` 로 stdout/stderr/exit code 대조.
- **파서 정합성**: markdown-rs 자체 테스트 유지 + 토큰 트리를 원본 micromark 출력과 비교하는
  fixture 몇 개 (Phase 0 에서 JSON 덤프로 확보).
- 각 규칙 단위 테스트는 원본 `test/rules/` 의 케이스 중 해당 규칙 것을 인라인 스냅샷으로.

## 6. 규칙 표

| ID | 별칭 | 태그 | 파라미터 (기본값) | fix |
|---|---|---|---|---|
| MD001 | heading-increment | headings | front_matter_title=`^\s*title\s*[:=]` | |
| MD003 | heading-style | headings | style=consistent | |
| MD004 | ul-style | bullet, ul | style=consistent | O |
| MD005 | list-indent | bullet, ul, indentation | | O |
| MD007 | ul-indent | bullet, ul, indentation | indent=2, start_indented=false, start_indent=2 | O |
| MD009 | no-trailing-spaces | whitespace | br_spaces=2, code_blocks=false, list_item_empty_lines=false, strict=false | O |
| MD010 | no-hard-tabs | whitespace, hard_tab | code_blocks=true, ignore_code_languages=[], spaces_per_tab=1 | O |
| MD011 | no-reversed-links | links | | O |
| MD012 | no-multiple-blanks | whitespace, blank_lines | maximum=1 | O |
| MD013 | line-length | line_length | line_length=80, heading_line_length=80, code_block_line_length=80, code_blocks=true, tables=true, headings=true, strict=false, stern=false | |
| MD014 | commands-show-output | code | | O |
| MD018 | no-missing-space-atx | headings, atx, spaces | | O |
| MD019 | no-multiple-space-atx | headings, atx, spaces | | O |
| MD020 | no-missing-space-closed-atx | headings, atx_closed, spaces | | O |
| MD021 | no-multiple-space-closed-atx | headings, atx_closed, spaces | | O |
| MD022 | blanks-around-headings | headings, blank_lines | lines_above=1, lines_below=1 (int 또는 레벨별 배열) | O |
| MD023 | heading-start-left | headings, spaces | | O |
| MD024 | no-duplicate-heading | headings | siblings_only=false | |
| MD025 | single-title, single-h1 | headings | level=1, front_matter_title=`^\s*title\s*[:=]` | |
| MD026 | no-trailing-punctuation | headings | punctuation=`.,;:!。，；：！` | O |
| MD027 | no-multiple-space-blockquote | blockquote, whitespace, indentation | list_items=true | O |
| MD028 | no-blanks-blockquote | blockquote, whitespace | | |
| MD029 | ol-prefix | ol | style=one_or_ordered | O |
| MD030 | list-marker-space | ol, ul, whitespace | ul_single=1, ol_single=1, ul_multi=1, ol_multi=1 | O |
| MD031 | blanks-around-fences | code, blank_lines | list_items=true | O |
| MD032 | blanks-around-lists | bullet, ul, ol, blank_lines | | O |
| MD033 | no-inline-html | html | allowed_elements=[], table_allowed_elements=[] | |
| MD034 | no-bare-urls | links, url | | O |
| MD035 | hr-style | hr | style=consistent | |
| MD036 | no-emphasis-as-heading | headings, emphasis | punctuation=`.,;:!?。，；：！？` | |
| MD037 | no-space-in-emphasis | whitespace, emphasis | | O |
| MD038 | no-space-in-code | whitespace, code | | O |
| MD039 | no-space-in-links | whitespace, links | | O |
| MD040 | fenced-code-language | code, language | allowed_languages=[], language_only=false | |
| MD041 | first-line-heading, first-line-h1 | headings | allow_preamble=false, level=1, front_matter_title=`^\s*title\s*[:=]` | |
| MD042 | no-empty-links | links | | |
| MD043 | required-headings | headings | headings=[], match_case=false | |
| MD044 | proper-names | spelling | names=[], code_blocks=true, html_elements=true | O |
| MD045 | no-alt-text | accessibility, images | | |
| MD046 | code-block-style | code | style=consistent | |
| MD047 | single-trailing-newline | blank_lines | | O |
| MD048 | code-fence-style | code | style=consistent | |
| MD049 | emphasis-style | emphasis | style=consistent | O |
| MD050 | strong-style | emphasis | style=consistent | O |
| MD051 | link-fragments | links | ignore_case=false, ignored_pattern="" | O |
| MD052 | reference-links-images | images, links | ignored_labels=["x"], shortcut_syntax=false | |
| MD053 | link-image-reference-definitions | images, links | ignored_definitions=["//"] | O |
| MD054 | link-image-style | images, links | autolink, inline, full, collapsed, shortcut, url_inline (모두 true) | O |
| MD055 | table-pipe-style | table | style=consistent | |
| MD056 | table-column-count | table | | |
| MD058 | blanks-around-tables | table | | O |
| MD059 | descriptive-link-text | accessibility, links | prohibited_texts=["click here","here","link","more"] | |
| MD060 | table-column-style | table | style=any, aligned_delimiter=false | O |

`parser: none` 인 규칙: MD047, MD052, MD053 (토큰 대신 줄 또는 참조 정의 캐시 사용).

## 7. 로드맵

| 마일스톤 | 내용 | 닫힘 기준 |
|---|---|---|
| M0 뼈대 | workspace, markdown-rs vendoring, Token 트리, Rule trait + registry, 설정 로딩/cascade, 인라인 주석, front matter, apply_fixes, CLI argv/glob/출력, 테스트 하네스, 샘플 규칙 MD047·MD018 | 원본 CLI 시나리오 테스트 중 규칙 무관 항목 전부 통과 |
| M1 줄 기반 규칙 | MD009 010 012 013 022 023 047 | 해당 규칙 fixture 통과 |
| M2 헤딩/리스트 | MD001 003 004 005 007 018~021 024~030 041 043 | 〃 |
| M3 인라인/링크/코드 | MD011 014 031~040 042 044~046 048~054 059 | 〃 |
| M4 테이블 | MD055 056 058 060 | 〃 |
| M5 마무리 | 전체 snapshot 통과, `--fix` 종단 검증, rayon, 릴리즈 바이너리(GitHub Actions, macOS/Linux/Windows) | 원본 테스트 전체 통과 |

이슈 단위: M0 은 기능 단위 약 8개, M1~M4 는 규칙당 1개(54개), M5 는 4개. 각 규칙 이슈는
이 문서 §6 의 한 줄과 원본 소스(`lib/md0XX.mjs`), 문서(`doc/md0XX.md`), 테스트 fixture
경로를 본문에 담는다.

## 8. 의존성

| 용도 | 크레이트 |
|---|---|
| 파서 | `markdown` 1.0.0 (vendored) |
| JSONC | `jsonc-parser` (serde feature) |
| YAML | `serde-saphyr` |
| TOML | `toml` |
| 설정 표현 | `serde_json::Value` |
| glob | `globset`, `ignore` |
| 정규식 | `regex` (lookaround 필요 시 `fancy-regex`) |
| 병렬 | `rayon` (M5) |
| 에러 | `thiserror` (core), `anyhow` (cli) |
| 테스트 | `insta`, `assert_cmd`, `predicates`, `tempfile` |
