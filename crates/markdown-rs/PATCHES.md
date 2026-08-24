# markdown-rs 1.0.0 로컬 패치

원본: https://crates.io/crates/markdown/1.0.0 (MIT, Titus Wormer). `license` 파일 참고.

| 파일 | 변경 | 이유 |
|---|---|---|
| `src/lib.rs` | `mod event;` → `pub mod event;`, `mod parser;` → `pub mod parser;` | micromark 토큰 트리를 만들기 위해 `Event`/`Name`/`Point` 와 `parser::parse` 를 외부에서 사용 |
| `Cargo.toml` | 정규화된 파일에서 `Cargo.lock`, `.github` 등 배포 메타 제거 | vendoring 정리 |
