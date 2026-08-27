//! 원본 markdownlint-cli2 v0.22.1 의 `test/markdownlint-cli2-test-cases.mjs` 시나리오를 같은 cwd, 같은
//! 인자로 실행해 `test/snapshots/markdownlint-cli2-test-exec.mjs.md` 와 대조한다. `fixtures/cli2/` 는
//! `scripts/dump-cli2-scenarios.mjs` 로 생성한 시나리오 정의(+스냅샷)와 fixture 사본(MIT)이다.
//!
//! 정규화는 원본 `sanitize` 와 같고 (`\r` 제거, 버전 문자열, sentinel 절대 경로), 여기에 배너 한 줄만
//! 원본 배너로 치환한다. 제외 시나리오와 사유는 `docs/cli2-scenarios.md` 참고.
//!
//! 하나만 실행: `CLI2_SCENARIO=<name> cargo test -p rust-markdownlint-cli --test cli2_scenarios`

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use regex::Regex;
use serde::Deserialize;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cli2");
const BASE_PLACEHOLDER: &str = "/__BASE__";
const ORIGINAL_BANNER: &str = "markdownlint-cli2 vX.Y.Z (markdownlint vX.Y.Z)";
const OUR_BANNER: &str =
    "rust-markdownlint vX.Y.Z (markdownlint-cli2 vX.Y.Z / markdownlint vX.Y.Z compatible)";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    name: String,
    cwd: String,
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    isolate: bool,
    shadow: Option<String>,
    #[serde(default)]
    no_import: bool,
    #[serde(default)]
    uses_require: bool,
    #[serde(default)]
    uses_env: bool,
    stderr_re: Option<StderrRe>,
    expected: Option<Expected>,
}

#[derive(Deserialize)]
struct StderrRe {
    source: String,
    flags: String,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Expected {
    exit_code: i32,
    stdout: String,
    /// `stderrRe` 시나리오는 스냅샷에 stderr 가 없다
    stderr: Option<String>,
    formatter_code_quality: String,
    formatter_json: String,
    formatter_junit: String,
    formatter_sarif: String,
}

/// 원본은 `usesRequire`/`env` 로 표시하지만 내장 포맷터만 쓰므로 실행하는 시나리오.
const BUILTIN_FORMATTER_SCENARIOS: &[&str] = &[
    "outputFormatters",
    "outputFormatters-npm",
    "outputFormatters-params",
    "outputFormatters-severity",
    "outputFormatters-clean",
    "outputFormatters-missing",
    "formatter-summarize",
    "formatter-pretty",
    "formatter-template",
];

/// JavaScript 모듈 로딩이 필요해 설계상 제외하는 시나리오. README "Differences" 절과 맞춘다.
fn skip_reason(s: &Scenario) -> Option<&'static str> {
    if s.no_import {
        Some("*-no-require variants are not part of the exec snapshot")
    } else if BUILTIN_FORMATTER_SCENARIOS.contains(&s.name.as_str()) {
        None
    } else if s.uses_require {
        Some("needs JavaScript module loading")
    } else if s.uses_env {
        Some("outputFormatters (JavaScript) with FORCE_COLOR")
    } else {
        None
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// 원본 `sanitize` 와 execa 의 마지막 개행 제거, 배너 치환, 그리고 우리만 있는 `--stdin-filename` 도움말 줄 제거.
fn sanitize(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text
        .strip_suffix('\n')
        .map(|t| t.strip_suffix('\r').unwrap_or(t))
        .unwrap_or(&text);
    let text = text.replace('\r', "");
    let text = Regex::new(r"\bv\d+\.\d+\.\d+\b")
        .unwrap()
        .replace_all(&text, "vX.Y.Z");
    let text = Regex::new(r" :.+[/\\]sentinel")
        .unwrap()
        .replace_all(&text, " :[PATH]");
    let text = Regex::new(r"(?m)^- --stdin-filename .*\n")
        .unwrap()
        .replace_all(&text, "");
    text.replace(OUR_BANNER, ORIGINAL_BANNER)
}

fn read_formatter_output(dir: &Path, default: &str, custom: &str) -> String {
    let read = |name: &str| fs::read(dir.join(name)).unwrap_or_default();
    let output = read(default);
    let output = if output.is_empty() {
        read(custom)
    } else {
        output
    };
    sanitize(&output)
}

fn run(root: &Path, s: &Scenario) -> Result<(), String> {
    let base = posix(root);
    let directory = root.join(&s.cwd);
    if s.isolate {
        copy_dir(&root.join(s.shadow.as_ref().unwrap()), &directory);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_rust-markdownlint"))
        .args(s.args.iter().map(|a| a.replace(BASE_PLACEHOLDER, &base)))
        .envs(&s.env)
        .current_dir(&directory)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let raw_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let actual = Expected {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: sanitize(&output.stdout),
        stderr: s.stderr_re.is_none().then(|| sanitize(&output.stderr)),
        formatter_code_quality: read_formatter_output(
            &directory,
            "markdownlint-cli2-codequality.json",
            "custom-name-codequality.json",
        ),
        formatter_json: read_formatter_output(
            &directory,
            "markdownlint-cli2-results.json",
            "custom-name-results.json",
        ),
        formatter_junit: read_formatter_output(
            &directory,
            "markdownlint-cli2-junit.xml",
            "custom-name-junit.xml",
        ),
        formatter_sarif: read_formatter_output(
            &directory,
            "markdownlint-cli2-sarif.sarif",
            "custom-name-sarif.sarif",
        ),
    };
    if s.isolate {
        fs::remove_dir_all(&directory).unwrap();
    }

    let expected = s.expected.as_ref().unwrap();
    let mut problems = Vec::new();
    if let Some(re) = &s.stderr_re {
        let dotall = if re.flags.contains('s') { "(?s)" } else { "" };
        let re = Regex::new(&format!("{dotall}{}", re.source)).unwrap();
        if !re.is_match(&raw_stderr) {
            problems.push(format!(
                "stderr does not match /{}/:\n{raw_stderr}",
                re.as_str()
            ));
        }
    }
    if actual != *expected {
        problems.push(format!("expected:\n{expected:#?}\nactual:\n{actual:#?}"));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[test]
fn cli2_scenarios() {
    let scenarios: Vec<Scenario> =
        serde_json::from_str(&fs::read_to_string(format!("{FIXTURES}/scenarios.json")).unwrap())
            .unwrap();
    let only = std::env::var("CLI2_SCENARIO").ok();
    let selected: Vec<&Scenario> = scenarios
        .iter()
        .filter(|s| only.as_ref().is_none_or(|o| &s.name == o))
        .filter(|s| skip_reason(s).is_none())
        .collect();
    assert!(!selected.is_empty(), "no scenario selected");
    for s in &selected {
        assert!(s.expected.is_some(), "{}: no snapshot", s.name);
    }

    // 원본처럼 시나리오 디렉토리를 나란히 두어야 `../sibling` 참조가 동작한다. 격리 시나리오는
    // `<name>-copy-exec` 를 만들고 지우므로 저장소 fixture 대신 임시 사본에서 실행한다.
    let temp = tempfile::tempdir().unwrap();
    // macOS 의 /var → /private/var 심볼릭 링크를 풀어야 절대 경로 인자와 cwd 가 같은 접두어를 가진다
    // (Windows 의 canonicalize 는 \\?\ 접두어를 붙이므로 제외).
    let root: PathBuf = if cfg!(windows) {
        temp.path().to_path_buf()
    } else {
        temp.path().canonicalize().unwrap()
    };
    copy_dir(Path::new(&format!("{FIXTURES}/test")), &root);

    let failures = Mutex::new(Vec::new());
    let queue = Mutex::new(selected.iter());
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().unwrap().next();
                    let Some(s) = next else { break };
                    if let Err(problem) = run(&root, s) {
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("## {}\n{problem}", s.name));
                    }
                }
            });
        }
    });

    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    assert!(
        failures.is_empty(),
        "{} of {} scenarios differ from the markdownlint-cli2 snapshot\n\n{}",
        failures.len(),
        selected.len(),
        failures.join("\n\n")
    );
}
