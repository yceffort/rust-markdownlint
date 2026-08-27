# markdown-rs 1.0.0 로컬 패치

원본: https://crates.io/crates/markdown/1.0.0 (MIT, Titus Wormer). `license` 파일 참고.

| 파일 | 변경 | 이유 |
|---|---|---|
| `src/lib.rs` | `mod event;` → `pub mod event;`, `mod parser;` → `pub mod parser;` | micromark 토큰 트리를 만들기 위해 `Event`/`Name`/`Point` 와 `parser::parse` 를 외부에서 사용 |
| `src/lib.rs` | `#![deny(clippy::pedantic)]` → `#![allow(clippy::all, clippy::pedantic)]` | 상류 코드가 최신 clippy 에서 실패. vendored 코드는 lint 대상에서 제외 |
| `src/lib.rs` | `#![no_std]` 제거, `pub mod undefined_refs;` 추가 | 실패한 label reference 기록에 `std::thread_local!` 사용 |
| `src/undefined_refs.rs` | 신규 파일 | markdownlint 의 `undefinedReference*` 토큰 재현: micromark JS 가 `labelEnd` nok 을 가로채는 것과 같은 지점에서 (시작, 끝, 내부 data/lineEnding 스팬) 을 스레드 로컬로 기록 |
| `src/construct/label_end.rs` | `nok()` 에서 `undefined_refs::record()` 호출 | 위와 동일 |
| `src/parser.rs` | `parse()` 시작 시 `undefined_refs::clear()` 호출 | 이전 파싱의 잔여 기록 제거 |
| `src/construct/document.rs` | `exit_containers()` 가 `Phase::After` 에서는 `child.interrupt` 를 리셋하지 않음 | 리스트 등 컨테이너가 닫힌 직후 시작한 문단의 interrupt 상태가 지워져, 다음 줄의 `2.` 같은 목록이 문단을 끊었다 (CommonMark 는 `1.` 만 허용). micromark 는 flow 를 닫을 때만 리셋한다 |
| `Cargo.toml` | 정규화된 파일에서 `Cargo.lock`, `.github` 등 배포 메타 제거, `[dev-dependencies.*]` 전부 제거 | vendoring 정리. 상류 테스트/벤치는 포함하지 않으므로 swc_core 등 대형 dev 의존성이 불필요 |
| `src/util/edit_map.rs` | `Vec<(at, remove, add)>` 선형 탐색 → `BTreeMap`, `consume` 이 `split_off`/`append` 대신 새 벡터를 한 번에 구성 | 편집 추가가 O(K²), 소비가 이벤트 전체를 두 번 복사하던 것을 제거 (#161) |
| `src/state.rs` | `State::Error(Message)` → `State::Error(Box<Message>)` | State 를 16바이트로 줄여 상태 전이마다 붙던 drop glue 를 가볍게 함 (프로파일 self 7%). 생성 지점 9곳은 `Box::new` 로 감쌈 |
| `src/tokenizer.rs` | `expect` 가 받은 `ByteAction` 을 `pending` 에 보관해 `consume` 이 재계산하지 않음, `exit` 의 `VOID_EVENTS` 선형 탐색을 debug 로 한정, `move_point_back` 을 `\r\n` 검사로 단순화, `State` 비교를 `matches!` 로, `push_impl` 이 이벤트 벡터를 미리 예약, `TokenizeState::set_markers`/`is_marker` (256비트 집합) 추가 | 바이트당 고정 비용 절감 (#161) |
| `src/construct/partial_data.rs`, `text.rs`, `string.rs` | `markers.contains(&byte)` → `is_marker(byte)` | data 상태가 바이트마다 마커 16개를 선형 탐색하던 것을 비트 검사로 |
| `src/construct/gfm_table.rs` | `start()` 가 다음 줄이 구분자 행 꼴(`|`, `-`, `:`, 공백만, `-` 포함)이 아니면 헤드 행 시도를 건너뜀. 건너뛸 때 실패 경로처럼 `seen`/`size`/`size_b` 를 리셋 | 모든 flow 줄을 헤드 행 후보로 끝까지 훑던 비용 제거. 필요조건만 보므로 결과는 같음. 카운터 리셋을 빠뜨리면 앞 구성요소가 남긴 `seen` 이 다음 테이블의 셀 수를 어긋나게 함 (`crates/core/tests/parser.rs` 회귀 테스트) |
