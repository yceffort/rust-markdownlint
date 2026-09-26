# 릴리즈 절차

## 요약

**버전 범프 PR 을 머지하면 릴리즈가 나간다.** 태그를 손으로 밀 필요가 없다.

```text
버전 범프 PR 머지
  → tag.yml 이 Cargo.toml 버전에 해당하는 태그가 없는 걸 보고 태그를 만들어 push
  → repository_dispatch 로 release.yml 을 발화
  → 5개 타깃 빌드, GitHub Release 생성, v0/v0.1 태그 이동, npm 6개 publish
```

## 워크플로 셋

| 파일 | 트리거 | 하는 일 |
| --- | --- | --- |
| `tag.yml` | main 에 push | Cargo.toml 버전의 태그가 없으면 만들어 밀고 `release.yml` 발화. 있으면 아무것도 안 함 |
| `release.yml` | `repository_dispatch` (`release`) | 진짜 릴리즈. `tag.yml` 이 보낸다 |
| `release.yml` | `v*.*.*` 태그 push | 진짜 릴리즈 (수동 탈출구) |
| `release.yml` | 수동 실행 | **항상 리허설.** 빌드와 패키징까지만 하고 publish 는 `--dry-run` |

`workflow_dispatch` 를 리허설로 못박은 것은 의도적이다. `dry_run` 불리언을 두면 실수로 진짜 배포가 나간다.

## 버전 올리기

스크립트 하나로 끝난다. 버전 문자열이 13곳에 흩어져 있어서 손으로 고치면 반드시 빠뜨린다.

```bash
scripts/bump-version.sh 0.1.2
```

`crates/cli/Cargo.toml` 의 현재 버전을 읽어 아래를 전부 바꾸고, 끝에 검사까지 돌린다.

- `crates/cli/Cargo.toml`, `crates/core/Cargo.toml`, `Cargo.lock`
- `npm/rust-markdownlint/package.json` (`version` 과 `optionalDependencies` 5개 전부)
- `npm/platforms/*/package.json` 5개
- `npm/rust-markdownlint/README.md`, `README.md`, `action.yml`, `.pre-commit-hooks.yaml`

`.github/` 는 건드리지 않는다. `release.yml` 의 `예: v0.1.1` 은 설명용 예시다.

검사는 따로도 부를 수 있고, CI 의 `version` 잡과 릴리즈 첫 스텝이 이걸 쓴다.

```bash
scripts/check-version.sh          # crates/cli/Cargo.toml 기준
scripts/check-version.sh v0.1.2   # 주어진 버전 기준
```

## 알려진 함정

### 1. 버전 범프 PR 은 pre-commit CI 가 구조적으로 깨진다

`scripts/pre-commit-hook.sh` 가 `crates/cli/Cargo.toml` 의 버전에 해당하는 GitHub Release 바이너리를 받는다. 범프 직후에는 그 릴리즈가 아직 없으니 404 다. `ci.yml` 이 체크아웃을 빌드해 `dist/v<ver>/` 에 릴리즈 자산 이름으로 묶고 `RUST_MARKDOWNLINT_DOWNLOAD_BASE=file://$PWD/dist` 로 받게 우회한다. 이 우회를 지우면 안 된다.

### 2. GITHUB_TOKEN 으로 push 한 태그는 워크플로를 트리거하지 않는다

GitHub 의 무한 루프 방지 규칙이다. `repository_dispatch` 와 `workflow_dispatch` 는 예외라서, `tag.yml` 은 태그를 민 뒤 `repository_dispatch` (`event_type=release`, `client_payload.version`) 로 `release.yml` 을 발화시킨다. 이 방식은 PAT 이 필요 없다.

`workflow_call` 은 쓰면 안 된다. npm trusted publishing 은 호출한 쪽 워크플로 이름을 검증하는데 npmjs.com 에는 `release.yml` 로 등록돼 있어서, `tag.yml` 이 부르면 publish 가 `E404` 로 실패한다.

`repository_dispatch` 는 main 에서 실행되고 `GITHUB_SHA` 도 main 의 최신 커밋이다. 그래서 체크아웃마다 `RELEASE_REF` (태그) 를 지정하고, 부동 태그 이동은 체크아웃한 `HEAD` 를 쓴다.

### 3. npm 은 토큰이 아니라 trusted publishing (OIDC) 으로 인증한다

npm 6개 패키지 모두 npmjs.com 에 trusted publisher (user `yceffort`, repository `rust-markdownlint`, workflow `release.yml`) 가 등록돼 있어야 한다. npm CLI 11.5.1 이상이 필요해서 `publish-npm` 잡이 `npm install -g npm@latest` 를 먼저 돌린다. `NPM_TOKEN` secret 은 쓰지 않는다. v0.1.4 는 토큰 인증이 `E404` 로 실패해 npm 에 나가지 못했다.

### 4. 리허설은 인증 경로를 검증하지 못한다

`npm publish --dry-run` 은 레지스트리 인증까지 가지 않는다. 빌드, 아카이브, 자산 구성, 패키지 내용은 전부 검증되지만 토큰 문제는 실제 publish 에서만 드러난다. v0.1.1 의 EOTP 가 그 사례다.

## 실패했을 때

`publish-npm` 만 실패한 경우 태그와 릴리즈는 이미 만들어져 있다. 원인을 고친 뒤 실패한 잡만 다시 돌리면 된다. 재실행은 처음 실행한 커밋의 워크플로 파일을 그대로 쓰므로, 워크플로를 고쳐야 하는 원인이면 버전을 올려 새로 릴리즈한다.

```bash
gh run rerun <run-id> --failed
```

## 릴리즈 전 리허설

버전을 올리기 전이나 워크플로를 고친 뒤에는 리허설을 한 번 돌려 본다.

```bash
gh workflow run Release --ref main -f version=v0.1.1
```

버전은 `crates/cli/Cargo.toml` 과 같아야 하며, 다르면 첫 스텝에서 막힌다.
