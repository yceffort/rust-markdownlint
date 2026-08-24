# rust-markdownlint

[![CI](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml/badge.svg)](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml)

[markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2) 와 동일하게 동작하는 것을 목표로 하는 Rust 구현입니다. 기존 `.markdownlint-cli2.{jsonc,yaml}`, `.markdownlint.{jsonc,json,yaml,yml}` 설정을 그대로 사용할 수 있는 drop-in 대체를 지향합니다.

아직 개발 초기 단계입니다. 진행 상황은 [마일스톤](https://github.com/yceffort/rust-markdownlint/milestones) 을 참고하시기 바랍니다.

## 개발

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
