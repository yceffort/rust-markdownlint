# 남은 성능 차이: LTO와 할당기 A/B, 단계별 계측 (2026-09-07)

같은 4 vCPU GitHub Codespace에서 `8bdd653`의 코드를 네 가지 빌드 설정으로 비교했다. **jemalloc과 Thin LTO는 실제 개선 효과가 있었고, 현재 가장 큰 코어 비용은 markdown-rs 파싱이었다.** 아래 수치는 제품 반영 전 A/B 실험의 결과다. 후속 코드 변경으로 release 빌드에 Thin LTO를, Linux CLI에 jemalloc을 기본 적용했다.

## 측정 조건

- Ubuntu 24.04, AMD EPYC 7763 4 vCPU / 16GB, Rust 1.98.1. 모든 빌드가 끝난 뒤 측정을 시작했다.
- 소스는 `8bdd65342cd02a11f5e09d02186be51e0e4cc3c6`을 `git archive`로 별도 디렉터리에 풀었다. README 작업과 원래 소스는 유지했다.
- 기본 설정, `CARGO_PROFILE_RELEASE_LTO=thin`, `tikv-jemallocator` 전역 할당기, 두 설정의 조합을 비교했다. 기본 Cargo release의 크레이트 간 LTO는 꺼져 있다. `tikv-jemallocator`는 rumdl과 같은 0.6 계열을 사용했으며 실제 해결 버전은 0.6.1이다.
- 각 도구와 코퍼스 조합마다 3회 준비 실행 후 20회. 도구 5개를 회전하고 5회마다 순서를 뒤집었다. 프로세스 시작부터 종료까지 blocking wait로 측정하고 기본 출력은 `/dev/null`로 보냈다.
- README 실험과 파일 내용과 상대 경로의 SHA-256이 같은 코퍼스 3종을 사용했다. 이번 복사본 위치는 `/workspaces/markdownlint-performance/ab-2026-09-07/corpora`이다. 저장소 설정은 복사하지 않고 `noBanner: true`만 설정했다. rumdl은 `check --no-cache --no-config .`로 실행했다.
- 같은 세션의 기존 빌드와 rumdl을 다시 측정했다. 이전 README 세션의 절대시간과 빼서 개선량을 계산하지 않는다. 도구 간 규칙과 진단 집합은 다르다.

## 계측 없는 CLI 전체 시간

평균 ± 표본 표준편차(ms), 20회씩 총 300개 측정값이다.

| 빌드 | fixture 388개 | 블로그 445개 | fixture 3,880개 |
|---|---:|---:|---:|
| 기존 설정 | 92.5 ± 7.2 | 447.3 ± 29.7 | 810.6 ± 36.5 |
| Thin LTO | 88.0 ± 4.0 | 431.2 ± 15.1 | 772.7 ± 15.3 |
| jemalloc | 86.0 ± 3.2 | 414.7 ± 7.7 | 761.7 ± 10.4 |
| Thin LTO + jemalloc | 83.0 ± 2.2 | 416.6 ± 15.0 | 736.1 ± 16.7 |
| rumdl 0.2.67 | 98.7 ± 2.1 | 403.9 ± 23.3 | 833.2 ± 28.7 |

블로그에서 기존 447.3ms 대비 jemalloc은 414.7ms(-7.3%), Thin LTO는 431.2ms(-3.6%)였다. 같은 실행 묶음에서 rumdl은 403.9ms였으므로 jemalloc으로 평균 격차가 43.3ms에서 10.8ms로 줄었다. 이 세션에서 관찰한 격차의 약 75%가 줄었지만, 이전 세션의 68ms 차이를 그대로 분해한 것은 아니다.

블로그에서 두 설정을 합친 값은 416.6ms로 jemalloc 단독보다 더 빠르지 않았다. 차이는 약 1.9ms이고 측정 편차 안에 있다. fixture에서는 조합이 가장 빨랐다. 두 설정의 이득을 더해서 예측하면 안 된다.

## 내부 계측

A/B용 바이너리에는 계측 코드를 넣지 않았다. 별도 바이너리에서 파일별 파싱, 전처리, 규칙, 진단 복제 시간을 기록하고, CLI의 파일 처리, 정렬, 출력 시간도 기록했다. 계측 결과는 별도 JSON 파일로 써서 진단 stderr와 분리했다. 기본 할당기와 jemalloc 프로파일을 번갈아 실행했다.

아래는 블로그 445개, `RAYON_NUM_THREADS=1`, 준비 실행 2회 후 각 5회의 평균이다. 단위는 ms이다.

| 단일 스레드, 코어 구간 | 기본 할당기 | jemalloc |
|---|---:|---:|
| markdown-rs 파서 본체 | 810.3 | 772.2 |
| 호환성 변환과 토큰 트리 구성 | 106.2 | 96.4 |
| 규칙 처리 전체(진단 생성과 복제 포함) | 128.9 | 115.8 |
| 코어 전체 | 1091.1 | 1025.9 |

기본 할당기에서 파서 본체는 코어 전체의 약 74%, 호환성 변환과 트리 구성은 약 10%, 규칙 처리는 약 12%였다. 파싱과 변환을 합치면 약 84%다. 4스레드의 파일별 누적 시간으로도 파싱과 변환 비중이 약 82%였다. 따라서 큰 절대 비용은 어댑터보다 **markdown-rs 파서 본체**에 있다.

이 단일 스레드 계측에서 jemalloc의 코어 시간 감소는 약 65ms였고, 그중 약 48ms가 파싱과 변환에서 줄었다. 규칙 처리도 약 13ms 줄었다. 메모리 할당기 변경이 어느 한 규칙만이 아니라 여러 단계에 영향을 준다는 증거다. 이 수치를 병렬 CLI의 32.6ms 개선과 합산하거나 직접 대응시키지는 않는다.

규칙별로는 MD013 약 27.3ms, MD051 약 12.5ms가 상위였다. MD013은 규칙 중 가장 크지만 코어 전체의 약 2.5%다. 별도로 잰 진단 필터링과 복제(`results.extend(...cloned())`)는 약 3.3ms여서, 이번 결과만으로 최우선 최적화 대상으로 보기 어렵다.

기본 할당기의 병렬 CLI 계측 평균은 다음과 같다. 파일 처리 내부의 누적 시간은 여러 스레드에서 동시에 잰 값이므로 이 표와 합산할 수 없다.

| CLI 구간 | 시간(ms) |
|---|---:|
| 설정과 파일 검색 등 준비 | 7.3 |
| 파일 읽기와 린트 병렬 작업 | 366.0 |
| 진단 수집 | 3.4 |
| 최종 정렬 | 19.0 |
| 기본 포매터와 출력 | 34.2 |
| 외부에서 잰 프로세스 전체 | 435.7 |

이 표는 계측 바이너리의 별도 실행이다. 위 계측 없는 A/B 평균과 동일한 값으로 취급하지 않는다. 남은 수집, 정렬, 출력은 개선 여지가 있지만 파일 처리 구간이 가장 크다.

## 검증과 한계

- 네 빌드 모두 세 코퍼스에서 종료 코드, stdout, stderr가 기존 설정과 바이트 단위로 같았다. fixture 3,218건, 블로그 16,764건, 10배 fixture 32,180건이다.
- 계측 바이너리도 기본 출력이 같았고, 계측 수치는 별도 파일에 기록했다.
- raw JSON의 코퍼스 해시가 이전 README 측정과 같으며, 모든 300개 개별 실행과 집계 통계를 검증했다.
- rumdl 공식 바이너리의 `--profile`은 `Profiling is disabled.`를 출력했다. 따라서 rumdl 내부 단계별 시간은 이번에 측정하지 못했다. 우리의 파싱 비중이 크다는 사실만으로 남은 도구 간 격차를 전부 파서 탓으로 귀속할 수는 없다.
- 이 성능 실험은 Linux GNU 환경이다. macOS, Windows, musl의 성능은 측정하지 않았다. 파서 교체나 규칙 의미 변경은 수행하지 않았다.

jemalloc과 Thin LTO를 제품 코드에 반영했다. 그다음 성능 작업은 markdown-rs 파서 본체를 프로파일링해 할당, 상태 전이, 이벤트 처리 중 무엇이 지배하는지 좁히는 것이 타당하다. UTF-16 처리나 진단 복제부터 큰 효과를 기대할 근거는 이번 측정에서 약했다.

## 제품 반영 후 검증

- release 프로필의 `lto = "thin"`과 Linux CLI의 `tikv-jemallocator = "0.6"`을 기본 적용했다. 전역 할당기는 CLI 바이너리에만 선언했다.
- Codespaces에서 Linux x86_64 GNU와 musl release 빌드, 그리고 GNU의 `--no-default-features` 빌드가 성공했다. musl 바이너리의 정적 링크도 확인했다.
- 세 제품 빌드 모두 기존 실험의 세 코퍼스에서 기준 바이너리와 종료 코드, stdout, stderr가 바이트 단위로 같았다. [바이너리 및 출력 해시](results/product-builds-2026-09-07.json)를 보관했다. 이 확인에서는 시간을 다시 측정하지 않았다.
- Linux GNU와 macOS에서 각각 `cargo test --workspace --locked`가 609개 통과, 11개 무시로 끝났고, `cargo clippy --workspace --all-targets --locked -- -D warnings`도 통과했다.
- macOS arm64와 x86_64의 Thin LTO release 빌드가 성공했다. README 사용 예제와 수정한 Markdown 문서도 검증했다.
- 기본 설정을 반영한 소스로 세 도구 비교를 다시 쟀다. 결과는 [RESULTS.md](RESULTS.md#기본-설정-반영-후-같은-날-재측정)에 있고, README 의 표를 그 musl 빌드 수치로 바꿨다.
- Linux arm64와 Windows 빌드는 이번 환경에서 실행하지 않았다. Linux x86_64와 arm64 musl release 빌드와 README 린트를 PR CI에 추가했다. aarch64 빌드는 `JEMALLOC_SYS_WITH_LG_PAGE=16`으로 jemalloc 페이지 크기를 고정해 16K와 64K 페이지 커널에서도 실행되게 했다.

## 원시 데이터와 재현

[모든 측정값과 단계별 샘플](results/remaining-gap-2026-09-07.json)에 환경, 빌드별 바이너리 SHA-256, 할당기 버전, 코퍼스 해시, 출력 해시를 기록했다.

실행한 Codespaces 전용 스크립트는 [build.py](experiments/remaining-gap/build.py), [instrument.py](experiments/remaining-gap/instrument.py), [measure.py](experiments/remaining-gap/measure.py), [profile-allocator.py](experiments/remaining-gap/profile-allocator.py)에 보관했다. 경로는 `/workspaces/rust-markdownlint`와 `/workspaces/markdownlint-performance`로 고정된 일회성 조사 하네스다. [기존 Codespaces 준비 절차](RESULTS.md#github-codespaces-비교-2026-09-07)로 rumdl과 고정한 블로그 체크아웃을 준비한 새 Codespace에서 실행한다.

```bash
source /home/codespace/.cargo/env
cp bench/experiments/remaining-gap/instrument.py /tmp/instrument-markdownlint-ab.py
python3 bench/experiments/remaining-gap/build.py
python3 bench/experiments/remaining-gap/measure.py
python3 bench/experiments/remaining-gap/profile-allocator.py
```

마지막 스크립트는 실험 후 원래 저장소의 일반 release 바이너리를 다시 빌드한다. 실험 소스와 바이너리는 별도 디렉터리에 남으며, 측정 완료 후 Codespace는 정지한다.
