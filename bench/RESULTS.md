# 벤치마크 결과

후속 실행: [musl 출력 버퍼링 계획·결과](optimization-plan-2026-09-07.md). 동일 세션의 기준 대비 개선을 별도로 기록했으며, 아래 세 도구 표는 출력 변경 전 측정이다.

## GitHub Codespaces 비교 (2026-09-07)

후속 조사: [Thin LTO와 jemalloc A/B, 단계별 계측](remaining-gap-2026-09-07.md). 그 결과로 release 프로필에 Thin LTO 를, Linux CLI 에 jemalloc 을 기본 적용했고, 같은 Codespace 에서 같은 절차로 다시 측정했다. README 의 표는 아래 "기본 설정 반영 후" 의 musl 표다.

### 기본 설정 반영 후 (같은 날 재측정)

소스는 `8bdd653` 위에 이 브랜치의 변경(`lto = "thin"`, `tikv-jemallocator` 0.6.1, Cargo.lock 의 cc 1.4.5)을 얹은 상태다. 도구, 코퍼스, 환경은 아래 원래 측정과 같고, 라운드 수만 6개 실행 순서에 균등하도록 24회로 늘렸다. 배포되는 Linux 바이너리(Releases, npm `linux-x64`)와 같은 `x86_64-unknown-linux-musl` 정적 빌드와 `cargo install` 이 만드는 glibc 빌드를 각각 별도 세션으로 쟀다. 두 빌드 모두 `--locked` 로 빌드했고, `nm` 으로 jemalloc 심볼 517개가 링크된 것을 확인했다. 바이너리 SHA-256 은 각 원시 JSON 의 `rust_binary` 에 있다.

musl 정적 빌드 ([원시 JSON](results/codespaces-2026-09-07-musl.json)):

| Corpus | rust-markdownlint | rumdl 0.2.67 | markdownlint-cli2 0.23.2 |
|---|---:|---:|---:|
| Blog posts, 445 files (7.50 MB) | 434.4 ± 16.1 | 380.1 ± 5.3 | 4,792.9 ± 104.4 |
| markdownlint fixtures, 388 files (0.25 MB) | 83.8 ± 2.4 | 89.8 ± 1.2 | 1,192.4 ± 23.6 |
| Fixtures copied 10 times, 3,880 files (2.45 MB) | 762.9 ± 25.9 | 748.3 ± 14.6 | 6,501.8 ± 157.1 |

glibc 빌드 ([원시 JSON](results/codespaces-2026-09-07-gnu.json)):

| Corpus | rust-markdownlint | rumdl 0.2.67 | markdownlint-cli2 0.23.2 |
|---|---:|---:|---:|
| Blog posts, 445 files (7.50 MB) | 416.7 ± 17.7 | 389.4 ± 25.9 | 4,874.0 ± 175.2 |
| markdownlint fixtures, 388 files (0.25 MB) | 83.8 ± 6.4 | 93.6 ± 6.5 | 1,214.7 ± 40.7 |
| Fixtures copied 10 times, 3,880 files (2.45 MB) | 726.0 ± 23.1 | 758.6 ± 26.8 | 6,549.7 ± 92.5 |

두 세션 모두 세 코퍼스에서 Rust 와 cli2 0.22.1 의 종료 코드, stdout, stderr 가 같았고 진단 수도 원래 측정과 같다(Rust 3,218 / 16,764 / 32,180, rumdl 2,602 / 17,527 / 26,020). 원래 측정(20회) 대비 glibc 빌드는 블로그 451.9 → 416.7ms, 10배 fixture 804.7 → 726.0ms 로, A/B 실험에서 본 개선 폭과 비슷하다. 별도 세션의 musl 정적 빌드 측정값은 같은 소스의 glibc 빌드보다 블로그와 10배 fixture 에서 4~5% 높았다. rumdl 과 cli2 는 musl 세션에서 조금 빨랐으므로 모든 도구가 함께 느려진 결과는 아니다. 다만 GNU·musl 을 같은 세션에서 교대로 측정하지 않았으므로 환경 변동을 배제하거나 차이 전체를 libc 에 귀속할 수는 없다. musl 내부의 어느 연산이 차이를 만드는지는 아직 프로파일링하지 않았다.

앞선 A/B 의 Thin LTO + jemalloc GNU 빌드는 블로그 416.6ms, 10배 fixture 736.1ms 였다. 이번 GNU 빌드의 416.7ms, 726.0ms 는 비슷한 수준이다. 반면 rumdl 의 10배 fixture 값은 A/B 세션의 833.2ms 에서 이번 musl 세션의 748.3ms 로 달라졌다. 따라서 이전 A/B 와 현재 README 의 도구 간 순위 변화에는 Rust 빌드 타깃 변경과 비교 도구의 세션별 변동이 함께 들어 있다. 아래 재현 절차는 현재 코드의 musl 과 GNU 빌드를 각각 지정하며 24회씩 측정한다.

### 기본 설정 반영 전 (원래 측정)

로컬 머신 대신 전용 GitHub Codespace (`standardLinux32gb`, 4 vCPU, 16GB RAM)에서 세 도구를 실행했다. Ubuntu 24.04.4 LTS / Linux x86_64, AMD EPYC 7763(노출된 논리 CPU 4개), Rust 1.98.1, Node.js 24.14.0이다. CPU 스레드 수는 별도 제한하지 않았다.

- rust-markdownlint: `8bdd65342cd02a11f5e09d02186be51e0e4cc3c6` (#207 반영), `cargo build --release --locked`.
- rumdl: 공식 `v0.2.67` x86_64 Linux GNU 릴리스 바이너리, 배포된 SHA-256 검증.
- markdownlint-cli2: npm `0.23.2` (markdownlint `0.41.1`). README의 호환 대상인 `0.22.1`은 별도 출력 검증에만 사용했다.
- 블로그: `yceffort/blog`의 `4c7cade067a10eb565a8e608081532fa055218c3`, `apps/blog/posts/**/*.md` 445개. 10배 코퍼스는 fixture를 별도 하위 디렉터리 10개에 복제한 것이며, 서로 다른 문서 3,880개라는 뜻은 아니다.

각 코퍼스의 Markdown 파일만 `/tmp`의 새 디렉터리에 복사했다. 저장소의 규칙 설정은 가져오지 않고 `{"noBanner":true}`만 설정했으며, 각 도구의 기본 규칙과 소스에 포함된 인라인 지시문을 사용했다. rumdl에는 `--no-config --no-cache`를 전달했다. 규칙 집합과 인라인 설정 해석이 도구마다 다르므로 동일한 검사를 수행한다고 가정하지 않는다. 특히 fixture는 markdownlint의 경계 사례와 인라인 설정을 포함하며, rumdl은 일부에 대해 설정 경고도 출력한다.

코퍼스별 각 도구 3회 준비 실행 후 20회 측정했다. 매 라운드에 세 도구를 한 번씩 실행하고, 가능한 6개 실행 순서를 순환했다. Python `time.perf_counter`로 프로세스 시작부터 종료까지 재며(종료 대기는 timeout polling 없이 blocking wait 사용), 기본 포매터가 실제 실행된 상태에서 stdout/stderr만 `/dev/null`로 보냈다. 파일 검색, 읽기, 파싱, 규칙, 정렬, 출력이 모두 포함된다. 파일 시스템 캐시는 따뜻한 상태이고 rumdl의 영속 lint 캐시는 껐다. 빌드, 설치, 네트워크 다운로드는 측정 전에 끝냈다. 아래 값은 평균 ± 표본 표준편차(ms)이다.

| Corpus | rust-markdownlint | rumdl 0.2.67 | markdownlint-cli2 0.23.2 |
|---|---:|---:|---:|
| Blog posts, 445 files (7.50 MB) | 451.9 ± 21.9 | 384.0 ± 13.6 | 4,780.8 ± 153.9 |
| markdownlint fixtures, 388 files (0.25 MB) | 97.3 ± 12.7 | 96.2 ± 11.8 | 1,247.4 ± 37.6 |
| Fixtures copied 10 times, 3,880 files (2.45 MB) | 804.7 ± 27.8 | 755.9 ± 24.5 | 6,480.8 ± 107.6 |

진단 수와 출력 검증:

| 코퍼스 | Rust | cli2 0.23.2 | rumdl 0.2.67 |
|---|---:|---:|---:|
| Blog posts, 445 files (7.50 MB) | 16,764 | 16,764 | 17,527 |
| markdownlint fixtures, 388 files (0.25 MB) | 3,218 | 3,218 | 2,602 |
| Fixtures copied 10 times, 3,880 files (2.45 MB) | 32,180 | 32,180 | 26,020 |

세 코퍼스 모두 Rust와 cli2 0.23.2의 진단(stderr)은 바이트 단위로 같았다. 최신 cli2의 진행 상황 출력(stdout)은 달랐다. 별도로 실행한 cli2 0.22.1은 종료 코드, stdout, stderr 모두 Rust와 같았다. rumdl 진단 수는 별도 JSON 출력 실행에서 센 lint 진단이며, fixture의 설정 경고는 포함하지 않는다. 시간 측정에는 기본 출력의 설정 경고도 포함된다.

코퍼스별 exit code, 출력 SHA-256, 환경, 커밋, 명령, 20개 개별 측정값은 [원시 JSON](results/codespaces-2026-09-07.json)에 기록했다. 현재 머신과 과거 Apple Silicon/Actions 측정의 절대시간은 직접 비교하지 않는다.

### 현재 코드의 musl·GNU 비교 재현

재현 하네스는 [compare-tools.py](compare-tools.py)이며 Python 표준 라이브러리만 사용한다. 다음 명령은 현재 코드를 기준으로 README 의 musl 표와 위 GNU 표를 각각 측정한다. 최적화 반영 전 `8bdd653` 의 원래 측정을 재현하는 명령은 아니다. 새 4코어 x86_64 Codespace 안에서 rust-markdownlint 저장소 루트를 기준으로 실행한다. 빌드 및 도구 설치는 시간 측정 전에 끝낸다.

```bash
# rustup 설치 후, 측정에 사용한 컴파일러로 빌드
rustup toolchain install 1.98.1 --profile minimal
rustup override set 1.98.1
rustup target add x86_64-unknown-linux-musl
sudo apt-get update && sudo apt-get install -y musl-tools
cargo build --release --locked -p rust-markdownlint-cli --target x86_64-unknown-linux-musl
cargo build --release --locked -p rust-markdownlint-cli --target x86_64-unknown-linux-gnu

BENCH_ROOT=/workspaces/markdownlint-performance
mkdir -p "$BENCH_ROOT/bin" "$BENCH_ROOT/tools"
npm install --prefix "$BENCH_ROOT/cli2-latest" --no-audit --no-fund markdownlint-cli2@0.23.2
npm install --prefix "$BENCH_ROOT/cli2-compatible" --no-audit --no-fund markdownlint-cli2@0.22.1

# rumdl 공식 바이너리와 체크섬
curl -fL https://github.com/rvben/rumdl/releases/download/v0.2.67/rumdl-v0.2.67-x86_64-unknown-linux-gnu.tar.gz \
  -o "$BENCH_ROOT/tools/rumdl-v0.2.67-x86_64-unknown-linux-gnu.tar.gz"
curl -fL https://github.com/rvben/rumdl/releases/download/v0.2.67/rumdl-v0.2.67-x86_64-unknown-linux-gnu.tar.gz.sha256 \
  -o "$BENCH_ROOT/tools/rumdl-v0.2.67-x86_64-unknown-linux-gnu.tar.gz.sha256"
(cd "$BENCH_ROOT/tools" && sha256sum -c rumdl-v0.2.67-x86_64-unknown-linux-gnu.tar.gz.sha256)
tar -xzf "$BENCH_ROOT/tools/rumdl-v0.2.67-x86_64-unknown-linux-gnu.tar.gz" -C "$BENCH_ROOT/bin"

git clone --filter=blob:none --sparse https://github.com/yceffort/blog.git "$BENCH_ROOT/blog"
git -C "$BENCH_ROOT/blog" sparse-checkout set apps/blog/posts
git -C "$BENCH_ROOT/blog" checkout --detach 4c7cade067a10eb565a8e608081532fa055218c3

# README 표: 배포용 musl 정적 바이너리
python3 bench/compare-tools.py \
  --rust "$PWD/target/x86_64-unknown-linux-musl/release/rust-markdownlint" \
  --rumdl "$BENCH_ROOT/bin/rumdl" \
  --cli2 "$BENCH_ROOT/cli2-latest/node_modules/.bin/markdownlint-cli2" \
  --compatible-cli2 "$BENCH_ROOT/cli2-compatible/node_modules/.bin/markdownlint-cli2" \
  --blog "$BENCH_ROOT/blog" \
  --output "$BENCH_ROOT/results-musl.json" --runs 24 --warmup 3

# GNU 표: 별도 세션으로 측정하고 결과 파일도 분리
python3 bench/compare-tools.py \
  --rust "$PWD/target/x86_64-unknown-linux-gnu/release/rust-markdownlint" \
  --rumdl "$BENCH_ROOT/bin/rumdl" \
  --cli2 "$BENCH_ROOT/cli2-latest/node_modules/.bin/markdownlint-cli2" \
  --compatible-cli2 "$BENCH_ROOT/cli2-compatible/node_modules/.bin/markdownlint-cli2" \
  --blog "$BENCH_ROOT/blog" \
  --output "$BENCH_ROOT/results-gnu.json" --runs 24 --warmup 3
```

## 이전 README의 환경별 측정 (보관)

아래 표는 이번 Codespaces 비교 이전에 README에 있던 수치다. 머신, 코퍼스, 설정, 코드 버전이 달라 새 비교표와 직접 합산하거나 속도 변화를 계산하는 데 쓰지 않는다. 특히 프로젝트 설정을 사용한 블로그 측정은 다수 규칙이 꺼져 있었으므로 기본 규칙 전체의 성능을 나타내지 않는다. 20,966개 파일 행은 반복 측정이 아닌 단일 실행이다. 당시 표는 hyperfine 준비 실행 3회, mean ± σ(ms), cli2/Rust 배율로 기록됐으며 두 도구의 진단 결과는 동일했다.

| Corpus | Machine | markdownlint-cli2 | rust-markdownlint | Ratio |
|--------|---------|-------------------|-------------------|-------|
| markdownlint `test/*.md`, 388 files, all rules | Apple M-series, 10 cores | 366.2 ± 2.8 | 55.1 ± 1.1 | 6.6x |
| markdownlint `test/*.md`, 388 files, all rules | GitHub Actions ubuntu-latest | 1267.3 ± 90.4 | 178.4 ± 1.3 | 7.1x |
| Same corpus copied 10 times, 3880 files | Apple M-series, 10 cores | 2956.7 ± 199.6 | 682.6 ± 10.0 | 4.3x |
| [yceffort/blog](https://github.com/yceffort/blog), `apps/blog/posts/**/*.md`, 441 posts (7.2 MB), project config | Apple M-series, 10 cores | 1411.2 ± 17.5 | 105.0 ± 2.3 | 13.4x |
| The same repository, `**/*.md` including `node_modules`, 20966 files (single run) | Apple M-series, 10 cores | 48306 | 14419 | 3.4x |

## 규칙별 과거 측정 (2026-08)

`bench/run.sh <MD0XX|all>` 출력의 마지막 행을 누적 기록한다. 코퍼스는 원본 markdownlint `test/*.md` 388개 (SCALE=1),
Apple Silicon macOS, `hyperfine --warmup 3`. 시간은 mean ± σ (ms), 배율은 cli2 / rust.
코퍼스가 작아 두 도구의 프로세스 시작 시간 비중이 크다.

| 규칙 | cli2 (ms) | rust (ms) | 배율 | 날짜 |
|------|-----------|-----------|------|------|
| MD047 | 158.2 ± 10.8 | 29.9 ± 2.7 | 5.3x | 2026-08-26 |
| MD009 | 583.3 ± 49.3 | 119.7 ± 7.6 | 4.9x | 2026-08-26 |
| MD010 | 477.5 ± 48.6 | 98.9 ± 12.0 | 4.8x | 2026-08-26 |
| all (MD018, MD047) | 490.3 ± 21.9 | 91.7 ± 6.6 | 5.3x | 2026-08-26 |
| MD023 | 577.1 ± 24.1 | 144.7 ± 20.5 | 4.0x | 2026-08-26 |
| MD020 | 600.9 ± 32.0 | 108.3 ± 9.6 | 5.6x | 2026-08-26 |
| MD012 | 572.0 ± 49.2 | 108.1 ± 11.0 | 5.3x | 2026-08-26 |
| MD007 | 547.1 ± 66.9 | 113.6 ± 6.7 | 4.8x | 2026-08-26 |
| MD004 | 559.0 ± 55.0 | 117.9 ± 9.4 | 4.7x | 2026-08-26 |
| MD005 | 582.8 ± 23.6 | 114.2 ± 9.0 | 5.1x | 2026-08-26 |
| MD022 | 593.9 ± 30.1 | 107.8 ± 5.8 | 5.5x | 2026-08-26 |
| MD003 | 549.0 ± 52.9 | 130.9 ± 18.6 | 4.2x | 2026-08-26 |
| MD019 | 561.0 ± 38.5 | 112.1 ± 7.4 | 5.0x | 2026-08-26 |
| MD021 | 570.9 ± 16.1 | 116.9 ± 5.1 | 4.9x | 2026-08-26 |
| MD001 | 568.2 ± 20.2 | 122.9 ± 7.5 | 4.6x | 2026-08-26 |
| MD025 | 259.7 ± 4.7 | 78.9 ± 0.6 | 3.3x | 2026-08-26 |
| MD035 | 255.2 ± 2.5 | 69.9 ± 0.6 | 3.7x | 2026-08-26 |
| MD043 | 253.7 ± 3.4 | 69.2 ± 0.7 | 3.7x | 2026-08-26 |
| MD024 | 258.6 ± 3.3 | 69.7 ± 0.8 | 3.7x | 2026-08-26 |
| MD028 | 257.8 ± 3.2 | 69.7 ± 0.6 | 3.7x | 2026-08-26 |
| MD026 | 257.5 ± 2.9 | 70.1 ± 0.6 | 3.7x | 2026-08-26 |
| MD030 | 259.6 ± 3.0 | 69.8 ± 0.6 | 3.7x | 2026-08-26 |
| MD041 | 259.7 ± 5.1 | 80.1 ± 0.5 | 3.2x | 2026-08-26 |
| MD029 | 258.9 ± 3.1 | 69.9 ± 0.7 | 3.7x | 2026-08-26 |
| MD027 | 262.5 ± 3.3 | 70.0 ± 0.9 | 3.8x | 2026-08-26 |
| MD013 | 277.0 ± 2.0 | 73.1 ± 0.9 | 3.8x | 2026-08-26 |
| MD018 | 257.5 ± 2.5 | 70.5 ± 1.3 | 3.7x | 2026-08-26 |
| MD047 | 104.4 ± 1.0 | 24.0 ± 0.5 | 4.4x | 2026-08-26 |
| MD046 | 261.3 ± 5.3 | 71.6 ± 1.0 | 3.6x | 2026-08-26 |
| MD040 | 260.2 ± 1.9 | 71.4 ± 0.8 | 3.6x | 2026-08-26 |
| MD033 | 264.6 ± 4.8 | 72.3 ± 0.9 | 3.7x | 2026-08-26 |
| MD014 | 269.0 ± 6.2 | 71.9 ± 1.2 | 3.7x | 2026-08-26 |
| MD036 | 275.0 ± 4.7 | 71.7 ± 0.8 | 3.8x | 2026-08-26 |
| MD034 | 264.9 ± 5.3 | 72.2 ± 0.6 | 3.7x | 2026-08-26 |
| MD031 | 265.1 ± 5.4 | 71.9 ± 0.8 | 3.7x | 2026-08-26 |
| MD038 | 265.5 ± 6.4 | 72.1 ± 1.1 | 3.7x | 2026-08-26 |
| MD011 | 266.4 ± 3.6 | 78.1 ± 0.8 | 3.4x | 2026-08-26 |
| MD032 | 267.2 ± 3.6 | 72.1 ± 0.9 | 3.7x | 2026-08-26 |
| MD042 | 258.8 ± 3.1 | 73.5 ± 0.5 | 3.5x | 2026-08-26 |
| MD045 | 256.8 ± 2.1 | 72.9 ± 0.7 | 3.5x | 2026-08-26 |
| MD048 | 256.4 ± 3.2 | 72.5 ± 0.8 | 3.5x | 2026-08-26 |
| MD039 | 257.2 ± 2.6 | 73.2 ± 0.7 | 3.5x | 2026-08-26 |
| MD052 | 104.1 ± 1.1 | 26.2 ± 0.5 | 4.0x | 2026-08-26 |
| MD051 | 264.9 ± 4.1 | 78.1 ± 0.9 | 3.4x | 2026-08-26 |
| MD044 | 257.0 ± 4.0 | 72.1 ± 0.8 | 3.6x | 2026-08-26 |
| MD049 | 259.7 ± 2.7 | 73.7 ± 1.0 | 3.5x | 2026-08-26 |
| MD050 | 261.4 ± 3.5 | 73.2 ± 0.7 | 3.6x | 2026-08-26 |
| MD037 | 269.6 ± 5.9 | 74.2 ± 1.4 | 3.6x | 2026-08-26 |
| MD053 | 104.3 ± 1.1 | 27.3 ± 0.5 | 3.8x | 2026-08-26 |
| MD054 | 255.1 ± 2.7 | 73.6 ± 1.1 | 3.5x | 2026-08-26 |
| MD055 | 266.2 ± 4.5 | 75.5 ± 0.7 | 3.5x | 2026-08-26 |
| MD056 | 263.2 ± 4.3 | 74.2 ± 1.3 | 3.5x | 2026-08-26 |
| MD058 | 265.2 ± 5.4 | 74.3 ± 0.8 | 3.6x | 2026-08-26 |
| MD059 | 264.8 ± 3.7 | 74.8 ± 1.2 | 3.5x | 2026-08-26 |
| MD060 | 282.2 ± 2.7 | 75.2 ± 0.7 | 3.8x | 2026-08-26 |
| all (53 규칙, 기본 설정) | 366.5 ± 7.0 | 160.6 ± 1.3 | 2.3x | 2026-08-26 |
| all (53 규칙, 기본 설정, rayon) | 366.5 ± 7.0 | 57.5 ± 1.6 | 6.4x | 2026-08-26 |
| all (SCALE=10, 3880 파일) | 2956.7 ± 199.6 | 682.6 ± 10.0 | 4.3x | 2026-08-26 |
| blog `apps/blog/posts/**/*.md` (441 파일, 프로젝트 설정) | 1853.9 ± 14.7 | 210.2 ± 6.4 | 8.8x | 2026-08-26 |
| blog `apps/blog/posts/**/*.md` (컬럼 인덱스, 종류 인덱스, MD011 regex 후) | 1475.0 ± 48.0 | 123.7 ± 6.3 | 11.9x | 2026-08-27 |
| all (53 규칙, 기본 설정, 어댑터 String 할당 제거 후) | 366.2 ± 2.8 | 55.1 ± 1.1 | 6.6x | 2026-08-27 |
| blog `apps/blog/posts/**/*.md` (어댑터 String 할당 제거 후) | 1411.2 ± 17.5 | 105.0 ± 2.3 | 13.4x | 2026-08-27 |

## 핫패스 프로파일 (2026-08-27, blog 포스트 441개, 단일 스레드, samply)

규칙 없음 42ms, 파싱이 필요한 규칙 하나 ~570ms, 전체 808ms → 파싱 60%. 그중 `column_at`(이벤트마다 줄 시작부터 UTF-16 재계산) 8%, `filter_by_types`(규칙 호출마다 전체 트리 재귀) 9%, MD011 fancy_regex 12.5%, MD051 파일마다 regex 컴파일 2.5% 를 제거해 단일 스레드 808 → 581ms. 남은 비용은 markdown-rs 토크나이저 55%, 이벤트→토큰 어댑터 15%, 규칙 25%.

## 어댑터 토큰별 String 할당 제거 (2026-08-27, #160)

포스트 441개를 메모리에 올려 in-process 단일 스레드로 10회 반복한 최선값. `Token.kind` 를 정적 표(`kinds.rs`)의 `&'static str` 로, `Token.text` 를 원문 범위(`TokenTree::text`)로 바꾸고, 종류 인덱스를 `Kind` 판별값 배열로, 컬럼 계산을 앞으로만 진행하는 커서로 바꿨다.

| 단계 | 전 | 후 |
|---|---|---|
| markdown-rs `parser::parse` (이벤트만) | 365.6 ms | 319.9 ms |
| 어댑터 (이벤트 → 토큰 트리, 위 값과의 차) | 159.1 ms | 45.4 ms |
| `lint_content` 53 규칙 전체 | 643.9 ms | 499.0 ms |
| 어댑터 비중 | 25% | 9% |

markdown-rs 자체의 차이는 측정 편차(같은 코드)다. 남은 어댑터 45ms 는 단계별 계측으로 `nest` 12ms, `adapt` 변환 패스 7개 19ms, `flatten` 7ms, `index_kinds` 2ms 이며, 할당이 아니라 트리를 패스마다 다시 걷는 비용이다. 5% 이하로 가려면 중첩 `Node` 트리 대신 아레나에서 패스를 융합하는 재설계가 필요하다.

## markdown-rs 토크나이저 내부 비용 절감 (2026-08-27, #161)

포스트 441개를 메모리에 올려 in-process 단일 스레드로 10회 반복한 최선값 (`markdown::parser::parse` 만, 이벤트 1,143,302개 동일). 변경은 `crates/markdown-rs/PATCHES.md` 에 정리.

| 단계 | `markdown::parser::parse` |
|---|---|
| 기준 (main, d10ca0a) | 320 ms |
| EditMap 한 번에 재구성, `State::Error` Box, `consume` 의 byte_action 재계산 제거, `exit` 의 VOID_EVENTS 탐색 제거 | 253 ms |
| 테이블 헤드 행 시도 사전 검사, data 마커 비트 집합, `State` 비교 `matches!` | 203 ms (−37%) |

samply 로 본 병목은 순서대로 `EditMap::consume` (inclusive 11%, `split_off`/`append` 로 이벤트 전체를 두 번 복사), `drop_in_place<State>` (self 7%, `Message` 페이로드의 drop glue), `gfm_table::head_row_data` (inclusive 5%, 모든 flow 줄을 테이블 헤드 후보로 끝까지 훑음), `partial_data::inside` (마커 16개 선형 탐색) 였다. 테이블 사전 검사는 건너뛸 때 공용 카운터 `seen` 을 리셋하지 않으면 앞 구성요소가 남긴 값 때문에 다음 테이블이 문단이 되는 회귀가 있어(`tables` 경계 사례로 발견) 실패 경로와 같이 리셋한다.

## 파일 단위 병렬화 (rayon)

같은 코퍼스와 기본 설정에서 병렬화 전후 rust 바이너리를 `hyperfine --warmup 3 -N` 으로 비교 (2026-08-26, Apple Silicon 10코어).
출력(stdout, stderr)은 두 SCALE 모두 순차 실행과 바이트 단위로 같다.

| 코퍼스 | 순차 (ms) | 병렬 (ms) | 배율 |
|--------|-----------|-----------|------|
| SCALE=1 (388 파일) | 159.8 ± 1.0 | 57.5 ± 1.6 | 2.8x |
| SCALE=10 (3880 파일) | 1516.0 ± 5.2 | 523.9 ± 9.1 | 2.9x |

## 규칙별 프로파일과 상위 규칙 최적화 (2026-08-27, #166)

포스트 441개를 메모리에 올려 한 번 파싱해 두고 규칙 53개를 하나씩 `rule.check` 로 호출한 시간 (단일 스레드, 20회 반복 최선값, 줄 분할과 front matter 는 제외). 설정은 기본값이라 blog 설정에서 꺼진 MD013 도 켜져 있다. 전/후는 main 과 이 브랜치의 하네스를 같은 부하에서 번갈아 잰 값이다.

| 규칙 | 전 (ms) | 후 (ms) | 변화 | 오류 수 |
|---|---|---|---|---|
| MD013 | 17.82 | 10.62 | -40% | 14540 |
| MD001 | 12.01 | 0.26 | -98% | 14 |
| MD041 | 11.43 | 0.03 | -100% | 0 |
| MD011 | 10.77 | 0.92 | -91% | 0 |
| MD051 | 6.69 | 3.80 | -43% | 0 |
| MD034 | 6.45 | 1.32 | -80% | 719 |
| MD037 | 5.93 | 2.75 | -54% | 0 |
| MD060 | 5.06 | 2.38 | -53% | 0 |
| MD010 | 4.78 | 1.44 | -70% | 60 |
| MD020 | 4.37 | 0.60 | -86% | 0 |
| MD032 | 4.14 | 2.52 | -39% | 0 |
| MD009 | 4.04 | 2.16 | -47% | 0 |
| MD050 | 3.88 | 0.24 | -94% | 0 |
| MD049 | 3.73 | 0.04 | -99% | 28 |
| MD018 | 3.29 | 0.38 | -88% | 0 |
| MD012 | 3.05 | 0.53 | -83% | 0 |
| MD038 | 2.43 | 1.07 | -56% | 2 |
| MD014 | 2.28 | 1.31 | -43% | 13 |
| MD022 | 1.66 | 0.62 | -63% | 0 |
| MD036 | 1.50 | 1.17 | -22% | 130 |
| MD059 | 1.28 | 1.31 | +2% | 1 |
| MD042 | 1.06 | 1.12 | +6% | 0 |
| MD040 | 0.97 | 0.69 | -29% | 335 |
| MD024 | 0.82 | 0.83 | +1% | 116 |
| MD025 | 0.77 | 0.13 | -83% | 47 |
| MD030 | 0.70 | 0.69 | -1% | 0 |
| MD048 | 0.64 | 0.85 | +33% | 0 |
| MD053 | 0.60 | 0.66 | +10% | 4 |
| MD052 | 0.58 | 0.64 | +10% | 1 |
| MD004 | 0.45 | 0.53 | - | 0 |
| MD007 | 0.35 | 0.43 | - | 0 |
| MD005 | 0.28 | 0.32 | - | 0 |
| MD056 | 0.23 | 0.25 | - | 0 |
| MD029 | 0.21 | 0.23 | - | 32 |
| MD055 | 0.20 | 0.21 | - | 0 |
| MD046 | 0.17 | 0.19 | - | 0 |
| MD026 | 0.13 | 0.13 | - | 101 |
| MD039 | 0.13 | 0.15 | - | 4 |
| MD019 | 0.11 | 0.13 | - | 0 |
| MD003 | 0.08 | 0.11 | - | 0 |
| MD028 | 0.07 | 0.08 | - | 57 |
| MD021 | 0.05 | 0.07 | - | 0 |
| MD045 | 0.05 | 0.06 | - | 37 |
| MD031 | 0.04 | 0.06 | - | 10 |
| MD023 | 0.03 | 0.05 | - | 0 |
| MD058 | 0.02 | 0.04 | - | 0 |
| MD035 | 0.02 | 0.04 | - | 0 |
| MD033 | 0.02 | 0.03 | - | 27 |
| MD054 | 0.01 | 0.02 | - | 0 |
| MD027 | 0.00 | 0.02 | - | 0 |
| MD044 | 0.00 | 0.02 | - | 0 |
| MD043 | 0.00 | 0.02 | - | 0 |
| MD047 | 0.00 | 0.02 | - | 0 |
| 합계 | 125.4 | 44.3 | -65% | |

0.5ms 아래의 차이는 측정 편차다. 같은 하네스에서 규칙 밖 단계도 같이 쟀다 (parse 는 바꾸지 않았고 부하 편차).

| 단계 | 전 (ms) | 후 (ms) |
|---|---|---|
| parse (markdown-rs + 어댑터) | 237.7 | 262.0 |
| clear_html_comment_text | 6.5 | 6.8 |
| split_lines x2 | 5.4 | 5.5 |
| apply_inline_config | 20.4 | 8.2 |
| effective_config | 0.7 | 0.7 |
| `lint_content` 53 규칙 전체 | 427.8 | 335.0 |

원인과 조치 (samply 와 코드 판독):

- MD001, MD041 (각 12ms): `front_matter_has_title` 이 기본 title 패턴을 파일마다 fancy_regex 로 컴파일했다. LazyLock 으로 한 번만.
- MD013 (18ms): 코드 블록/heading/링크 줄 번호를 `HashSet<usize>` 에 줄마다 넣고 찾던 해싱이 규칙 시간의 23%, 오류 14,540건 생성(`add_error` 의 String 5~6개)이 그 다음. 줄 번호 집합을 `LineSet` 비트맵으로, `LintError.rule_names`/`rule_description` 을 정적 참조로, 문서 URL 은 sink 당 한 번. 남은 10ms 는 줄마다 `chars().count()` 와 오류 자체.
- MD011, MD018, MD020, MD014, MD038, MD010, MD009: 줄(또는 토큰)마다 정규식이나 문자 순회를 돌렸다. 정규식이 요구하는 필요조건(`)[`, `#`, `$`, 첫/끝 글자 공백, `\t`, `trim_end`)을 memchr 급 검사로 먼저 본다.
- MD034, MD037, MD049, MD050, MD032, MD051, MD060: `filter_by_predicate` 의 transform 클로저가 방문하는 토큰마다 `children.clone()` 으로 Vec 을 만들었다 (58만 토큰). 출력 버퍼를 넘기는 시그니처로 바꿨다. MD037 과 MD049/MD050 은 전체 트리 순회 대신 종류 인덱스로 같은 집합을 얻는다 (htmlFlow 안의 해당 토큰은 재파싱 토큰뿐이라 `in_html_flow` 제외와 동치).
- MD060 (5ms): 파이프마다 `js_slice` 가 `Vec<u16>` 과 `String` 을 만들었다. 접두 부분 문자열(`&str`)로 대체.
- MD022, MD032: 오류가 없어도 linePrefix 전체를 모으고(MD022 는 heading 마다 prefix 텍스트까지) 계산했다. 오류가 있을 때만.
- 인라인 설정 (20ms): 줄마다 정규식 3회와 `HashSet` 복제. `<!--` 없는 줄은 정규식을 건너뛰고 줄별 활성 집합은 `Rc` 로 공유.
- 남은 후보: `reference_link_image_data` 를 MD042/MD052/MD053/MD054 가 각각 계산 (합쳐 3ms 미만이라 두었다), MD013 의 오류당 detail/context String, MD051 의 `OrderedMap` SipHash.

## CLI 결과 정렬 최적화 (2026-09-06, #197)

Apple M1 8코어, Rust 1.88.0 release 빌드. 변경 전 기준은 `c0c9d5c`다. 블로그 포스트 445개(7,504,540바이트)를 저장소 설정이 없는 디렉토리에 복사하고 `noBanner: true`와 빈 규칙 설정으로 기본 53개 규칙을 실행했다. #197의 443개 코퍼스와는 구성이 다르므로 당시 절대 시간과 직접 비교하지 않는다.

변경 전, 변경 후, rumdl, rumdl, 변경 후, 변경 전 순서를 반복했다. 각 바이너리의 준비 실행 2회를 제외한 20회 평균과 표준편차이며 stdout과 stderr는 `/dev/null`로 보냈다. rust는 `rust-markdownlint '**/*.md'`, rumdl은 공식 0.2.61 macOS arm64 릴리스 바이너리로 `rumdl check --no-cache --no-config .`를 실행했다.

| 대상 | 평균과 표준편차 (ms) |
|---|---|
| rust-markdownlint 변경 전 | 343.1 ± 7.9 |
| rust-markdownlint 변경 후 | 179.0 ± 11.3 |
| rumdl 0.2.61 | 175.9 ± 4.7 |

전체 실행 시간이 47.8% 줄었다. 변경 후 평균은 rumdl의 1.02배로, 이 코퍼스에서 기존의 큰 격차가 사라졌다. 두 도구의 규칙과 진단 결과는 서로 다르며, 이 비교는 출력 호환성을 의미하지 않는다.

`crates/cli/src/output.rs`의 `locale_compare`는 정렬 비교마다 양쪽 파일명의 전체 비교 키를 `Vec`으로 만들고, 1차 키가 같으면 대소문자 키도 새로 만들었다. 동일한 문자열은 바로 반환하고, 나머지는 문자 이터레이터를 직접 비교하도록 바꿨다. 비교 키 정의와 정렬 순서는 유지했다. 별도 내부 측정에서 같은 진단 16,764건의 정렬 중앙값은 128.0ms에서 23.2ms로 줄었다(8회, 입력 벡터 복제 시간 제외). 내부 측정의 입력 순서는 CLI 작업 순서와 다를 수 있어 이 차이를 CLI 시간에서 그대로 빼면 안 된다.

MD013만 켜도 진단이 15,138건 발생한다. 규칙을 켜고 끈 CLI 실행 시간의 차이에는 규칙 본체 외에 결과 수집, 정렬과 출력 비용도 포함되므로 이를 전부 규칙 시간으로 해석하면 안 된다.

검증: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, release 빌드 통과. 테스트 609개 통과, 기존 제외 항목 11개. 블로그 445개(진단 16,764건)와 fixture 388개(진단 3,218건)의 종료 코드, stdout과 stderr가 변경 전후 모두 같았다. 로컬 Xcode 실행 경로 문제를 피하기 위해 빌드와 문서 테스트에 clang 경로를 명시했다.
