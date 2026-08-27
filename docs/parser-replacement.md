# 파서 교체 검토 결정 기록 (#162)

작성일 2026-08-27. 기준 커밋 `d10ca0a` (PR #159 머지 직후). 결론은 **지금은 교체하지 않는다** 이다. 근거와 재검토 조건을 아래에 남긴다.

## 결론

1. 파서만 바꿔서는 이슈가 노리는 코어당 20배에 못 간다. 파서와 어댑터를 0ms 로 만들어도 규칙과 나머지 비용 119ms 가 남아 코어당 상한은 12.4배다 (cli2 1475ms 기준). 20배는 74ms 이하여야 하므로 규칙 쪽을 손대지 않는 한 어떤 파서로도 불가능하다.
2. 착수 조건(#160, #161 결과 확인)이 아직 충족되지 않았다. 둘 다 OPEN 이다.
3. 세 방향 중 현실적인 것은 1번(pulldown-cmark 위에 micromark 토큰 재구성)뿐이고, 기대치는 코어당 7~9배다. #160, #161 을 끝낸 뒤 예상되는 3.6배의 약 2배이지만, 재구성 층은 어댑터(`adapt.rs` 806줄)의 quirk 재현을 그대로 유지한 채 구성요소 25종 안팎의 내부 토큰을 원문에서 다시 잘라내는 코드를 추가하는 일이며, micromark 와 pulldown-cmark 의 의미 차이가 그대로 규칙 결과 diff 가 된다.

## 측정

### 파서별 파싱 시간

blog 포스트 441개 (7,185,132 바이트), Apple Silicon 단일 스레드, 파일을 메모리에 올린 뒤 in-process 로 10회 반복해 최선값. 각 파서에 이 저장소와 같은 확장(GFM 테이블, 각주, 수식, autolink literal)을 켰다.

| 대상 | 시간 | 산출물 |
|---|---|---|
| markdown-rs `parser::parse` (이벤트만) | 365.6 ms | 이벤트 1,143,302개 |
| markdown-rs + 어댑터 (`rust_markdownlint::parser::parse`) | 524.7 ms | 토큰 582,321개 |
| pulldown-cmark 0.13.4 `into_offset_iter` (TABLES, FOOTNOTES, MATH, GFM) | 9.8 ms | 이벤트 217,388개 |
| comrak 0.42.0 `parse_document` (table, footnotes, autolink, math_dollars, sourcepos) | 51.9 ms | 노드 159,599개 |
| `lint_content` (규칙 53개, 기본 설정) | 643.9 ms | 오류 16,278건 |

여기서 어댑터 약 159ms, 규칙과 나머지(인라인 설정, HTML 주석 치환, 줄 분할) 약 119ms 다. 같은 포스트를 CLI 로 돌린 값은 단일 스레드 581ms, 10코어 123.7ms, cli2 1475ms 다 (`bench/RESULTS.md` 2026-08-27).

pulldown-cmark 가 markdown-rs 보다 37배 빠른 것은 사실이지만, 이 수치는 이벤트를 소비만 한 값이다. 규칙이 쓰는 토큰 58만 개를 만들어 트리로 잇는 비용은 포함되지 않는다.

### 코어당 배율 추정 (cli2 1475ms 기준)

| 시나리오 | 단일 스레드 | 코어당 배율 |
|---|---|---|
| 현재 (in-process) | 644 ms | 2.3x |
| #160 (어댑터 159 → 30ms) + #161 (토크나이저 30% 감소) | 약 405 ms | 3.6x |
| 파서와 어댑터 0ms, 규칙 그대로 (이론 상한) | 119 ms | 12.4x |
| 방향 1: pulldown 10ms + 재구성 층 30~80ms + 규칙 119ms | 160~210 ms | 7~9x |
| 20배 목표 | 74 ms 이하 | 규칙 비용(119ms)보다 작음 |

방향 1 의 재구성 층 비용은 추정이다. 근거는 토큰 58만 개를 정적 종류와 원문 범위로 만들면 #160 목표(어댑터 5% 이하, 약 30ms)와 같은 급이 되고, 접두어 스캔과 구성요소별 재렉싱이 붙으면 그 2~3배까지 볼 수 있다는 것이다.

### 규칙의 토큰 의존 범위

토큰 오라클(fixture 388개 JS 덤프)에 나타나는 micromark 토큰 종류는 142종이고, 규칙 53개와 `parser/helpers.rs` 가 문자열로 참조하는 종류는 89종이다. 그중 pulldown-cmark 이벤트가 범위째로 주는 것은 블록 컨테이너, 헤딩, 코드 블록, 링크/이미지/강조/코드 스팬, HTML, 테이블 정도로 20종 안팎이다. 나머지는 원문에서 다시 잘라내야 한다.

- 접두어와 공백류 (규칙 20개와 `helpers.rs` 가 직접 참조): `linePrefix`, `listItemIndent`, `blockQuotePrefix`, `blockQuotePrefixWhitespace`, `listItemPrefix`, `listItemPrefixWhitespace`, `listItemMarker`, `listItemValue`, `whitespace`, `lineSuffix`, `lineEnding`, `lineEndingBlank`
- 구성요소 내부: `atxHeadingSequence`, `setextHeadingLine`, `codeFencedFence`, `codeFencedFenceSequence`, `codeFencedFenceInfo`, `codeFencedFenceMeta`, `codeFlowValue`, `codeTextData`, `codeTextPadding`, `emphasisSequence`, `strongSequence`, `label`, `labelText`, `resource`, `resourceDestination*`, `resourceTitle*`, `reference`, `referenceString`, `tableRow`, `tableCellDivider`, `tableDelimiterRow`, `tableDelimiter`, `tableContent`, `characterEscapeValue`, `autolinkProtocol`, `autolinkEmail`, `htmlFlowData`, `htmlTextData`
- pulldown-cmark 에 이벤트 자체가 없는 것: 링크 정의 (`definition*`, `Parser::reference_definitions()` 로 라벨과 범위만 얻고 목적지/제목 위치는 없다), 실패한 참조 (`undefinedReference*`, `broken_link_callback` 으로 `[..]` 범위만), container directive, `content` 청크
- 구조 quirk: lineEnding 을 다음 줄 접두어 끝까지 늘리는 defineSkip, 리스트 exit 를 앞으로 옮기는 postprocess, lazy 문단 content 병합, htmlFlow 재파싱, `codeTextPadding`. 지금 `adapt.rs` 806줄이 하는 일이며 파서를 바꿔도 그대로 필요하다.

### 블록 구조 일치율 (pulldown-cmark 대 markdown-rs)

블록 시작 (종류, 줄) 시퀀스만 비교했다. 인라인은 비교하지 않았다.

| 코퍼스 | 일치 | 불일치 원인 |
|---|---|---|
| fixture 388개 | 385 | 수식 `$` 2건 (`mathjax-scenarios.md`, `texmath-content.md`), 리스트 1건 (`list-item-prefix-alignment.md` 43행에서 pulldown 만 순서 목록 시작) |
| blog 포스트 441개 | 436 | 5건 전부 pulldown 이 본문 `$` 를 display math 로 잡음 |

블록 수준은 99% 일치하지만 불일치는 모두 확장(수식) 의미 차이다. 인라인 확장(autolink literal 의 `previousUnbalanced` 같은 규칙, 각주, 테이블 셀의 escaped pipe)은 micromark 와 cmark-gfm 계열이 서로 다르게 구현돼 있어 같은 종류의 차이가 더 나올 것으로 본다. 이런 차이를 맞추려면 pulldown-cmark 도 벤더링해 고쳐야 하며, 그러면 지금 markdown-rs 에서 하는 일(`PATCHES.md` 의 패치와 이후 directive, heading resolve, autolink 수정)을 micromark 설계와 더 먼 코드베이스에서 반복하게 된다.

## 세 방향의 비용과 효과

| 방향 | 효과 (코어당) | 비용 | 위험 |
|---|---|---|---|
| 1. pulldown-cmark 이벤트 + 원문 오프셋에서 micromark 토큰 재구성 | 7~9x | 접두어 스캐너와 구성요소별 재렉서 신규 (추정 2,000~3,000줄), `adapt.rs` quirk 유지, directive 전처리 별도. 검증은 토큰 오라클 376/388, fixture, blog 코퍼스 그대로 사용 가능 | micromark 와 pulldown 의 의미 차이(수식에서 이미 확인)가 규칙 diff 로 드러남. 차이를 맞추려면 두 번째 파서 벤더링 |
| 2. micromark 와 같은 토큰을 내는 파서 직접 작성 | 상한 12.4x, 실제는 1번과 비슷하거나 그 이하 | markdown-rs 는 43,546줄. 토큰 스트림이 micromark 알고리즘(상태 기계, resolver, postprocess)의 산물이라 같은 결정을 다시 구현해야 하며 속도 이득은 바이트별 상태 호출과 EditMap 제거 같은 엔지니어링에서만 나옴 | 사실상 #161 의 최대 버전. 검증 하네스는 있지만 공수가 월 단위 |
| 3. 규칙을 토큰 의존이 덜한 형태로 재작성 | 규칙 비용도 같이 줄어 유일하게 20x 에 접근 가능 | 규칙 8,914줄 재작성 (원본 포팅 전체와 같은 규모). 토큰 오라클을 버리고 출력 바이트 대조(fixture 388, blog 264,114건)만 남음 | MD027/MD007/MD028/MD030/MD032 (접두어 의미), MD037 (bare 마커), MD038 (`codeTextPadding`), MD051/052/053 (`undefinedReference*`) 는 micromark quirk 자체가 규칙 정의다. 상류 markdownlint 는 계속 micromark 토큰 기준으로 규칙을 쓰므로 이후 동기화마다 번역이 필요 |

## 재검토 조건

- #160, #161 을 끝낸 뒤 같은 포스트 441개에서 코어당 배율을 다시 잰다. 예상은 3.6x 안팎이다.
- 그 시점에 규칙 비용(현재 119ms)을 규칙별로 프로파일한다. 파서를 어떻게 바꾸든 그 다음 병목은 규칙이다.
- 그래도 파서를 바꾸기로 하면 방향 1 만 프로토타입한다. 범위는 이슈에 적힌 대로 MD001, MD032, MD052 가 참조하는 토큰 종류(헤딩, 리스트/인용 접두어, `linePrefix`, `undefinedReference*`, `definition*`)만 재구성하고, go/no-go 기준은 (a) 그 종류에 한정한 토큰 오라클 통과율이 지금(376/388)과 같고 (b) 포스트 441개 파싱+재구성이 100ms 이하인 것으로 둔다.

## 측정 방법

- 파서 비교: pulldown-cmark, comrak, 이 저장소의 `markdown`, `rust-markdownlint` 를 path 의존으로 묶은 별도 cargo 프로젝트에서 `std::time::Instant` 로 측정. markdown-rs 옵션은 `crates/core/src/parser/build.rs` 의 `parse_options(true)` 와 동일. 매 반복 전 warmup 1회.
- 토큰 의존 행렬: 오라클 `js/*.md.json` 에서 종류 집합을 만들고 규칙 파일의 `"..."` 문자열과 교차.
- 블록 일치율: 양쪽 파서에서 (블록 종류, 시작 줄) 시퀀스를 뽑아 파일 단위로 비교. 종류는 heading, code, ol, ul, item, bq, table, html, hr, footnote, math 11종.
