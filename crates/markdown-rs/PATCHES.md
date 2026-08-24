# markdown-rs 1.0.0 로컬 패치

원본: https://crates.io/crates/markdown/1.0.0 (MIT, Titus Wormer). `license` 파일 참고.

| 파일 | 변경 | 이유 |
|---|---|---|
| `src/lib.rs` | `mod event;` → `pub mod event;`, `mod parser;` → `pub mod parser;` | micromark 토큰 트리를 만들기 위해 `Event`/`Name`/`Point` 와 `parser::parse` 를 외부에서 사용 |
| `src/lib.rs` | `#![deny(clippy::pedantic)]` → `#![allow(clippy::all, clippy::pedantic)]` | 상류 코드가 최신 clippy 에서 실패. vendored 코드는 lint 대상에서 제외 |
| `Cargo.toml` | 정규화된 파일에서 `Cargo.lock`, `.github` 등 배포 메타 제거, `[dev-dependencies.*]` 전부 제거 | vendoring 정리. 상류 테스트/벤치는 포함하지 않으므로 swc_core 등 대형 dev 의존성이 불필요 |

