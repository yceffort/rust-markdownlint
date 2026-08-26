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
