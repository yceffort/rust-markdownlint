//! 기대값은 원본 markdownlint-cli2 v0.22.1 을 같은 입력으로 실행한 결과에서 M0 에 없는 규칙
//! (MD041 등)의 줄만 제거한 것이다. 배너 한 줄만 원본과 다르다.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

const BANNER: &str = concat!(
    "rust-markdownlint v",
    env!("CARGO_PKG_VERSION"),
    " (markdownlint-cli2 v0.22.1 / markdownlint v0.40.0 compatible)\n"
);

const MD018: &str =
    "MD018/no-missing-space-atx No space after hash on atx style heading [Context: \"#x\"]";
const MD041: &str = "MD041/first-line-heading/first-line-h1 First line in a file should be a top-level heading [Context: \"#x\"]";
const MD047: &str =
    "MD047/single-trailing-newline Files should end with a single newline character";

fn cmd(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("rust-markdownlint").unwrap();
    c.current_dir(dir);
    c
}

fn tree(entries: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in entries {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }
    dir
}

fn help_stdout() -> impl Predicate<str> {
    predicate::str::starts_with(format!(
        "{BANNER}https://github.com/DavidAnson/markdownlint-cli2\n\nSyntax: markdownlint-cli2 glob0"
    ))
    .and(predicate::str::ends_with(
        "The most compatible syntax for cross-platform support:\n$ markdownlint-cli2 \"**/*.md\" \"#node_modules\"\n",
    ))
}

#[test]
fn no_args_prints_help_exit_2() {
    let t = tree(&[]);
    cmd(t.path())
        .assert()
        .code(2)
        .stdout(help_stdout())
        .stderr("");
    cmd(t.path())
        .arg("--help")
        .assert()
        .code(2)
        .stdout(help_stdout());
}

#[test]
fn config_without_value_exit_2() {
    let t = tree(&[("a.md", "#x")]);
    cmd(t.path())
        .args(["a.md", "--config"])
        .assert()
        .code(2)
        .stdout(help_stdout());
}

#[test]
fn error_output_format_and_exit_1() {
    let t = tree(&[("a.md", "#x")]);
    cmd(t.path())
        .arg("a.md")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: a.md\nLinting: 1 file(s)\nSummary: 3 error(s)\n"
        ))
        .stderr(format!(
            "a.md:1:1 error {MD018}\na.md:1 error {MD041}\na.md:1:2 error {MD047}\n"
        ));
}

#[test]
fn fix_rewrites_and_exit_0() {
    let t = tree(&[("a.md", "#x")]);
    cmd(t.path())
        .args(["--fix", "a.md"])
        .assert()
        .code(0)
        .stdout(format!(
            "{BANNER}Finding: a.md\nLinting: 1 file(s)\nSummary: 0 error(s)\n"
        ))
        .stderr("");
    assert_eq!(fs::read_to_string(t.path().join("a.md")).unwrap(), "# x\n");
}

#[test]
fn fix_false_in_config_overrides_flag() {
    let t = tree(&[
        ("a.md", "#x"),
        (".markdownlint-cli2.jsonc", r#"{"fix": false}"#),
    ]);
    cmd(t.path()).args(["--fix", "a.md"]).assert().code(1);
    assert_eq!(fs::read_to_string(t.path().join("a.md")).unwrap(), "#x");
}

#[test]
fn stdin_dash() {
    let t = tree(&[]);
    cmd(t.path())
        .arg("-")
        .write_stdin("#x\n")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: \nLinting: 1 file(s)\nSummary: 2 error(s)\n"
        ))
        .stderr(format!("stdin:1:1 error {MD018}\nstdin:1 error {MD041}\n"));
}

#[test]
fn stdin_and_file_sorted_together() {
    let t = tree(&[("a.md", "#x")]);
    cmd(t.path())
        .args(["-", "a.md"])
        .write_stdin("#x\n")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: a.md\nLinting: 2 file(s)\nSummary: 5 error(s)\n"
        ))
        .stderr(format!(
            "a.md:1:1 error {MD018}\na.md:1 error {MD041}\na.md:1:2 error {MD047}\nstdin:1:1 error {MD018}\nstdin:1 error {MD041}\n"
        ));
}

/// `--stdin-filename` (cli2 에 없는 옵션): 결과 이름과 디렉토리 설정 계층을 그 경로 기준으로 한다.
#[test]
fn stdin_filename_applies_directory_config_and_sorts_by_name() {
    let t = tree(&[
        ("a.md", "#x\n"),
        ("zz.md", "#x\n"),
        (
            "sub/.markdownlint-cli2.jsonc",
            r#"{"config": {"MD041": false}}"#,
        ),
    ]);
    cmd(t.path())
        .args(["--stdin-filename", "sub/m.md", "-", "a.md", "zz.md"])
        .write_stdin("#x\n")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: a.md zz.md\nLinting: 3 file(s)\nSummary: 5 error(s)\n"
        ))
        .stderr(format!(
            "a.md:1:1 error {MD018}\na.md:1 error {MD041}\nsub/m.md:1:1 error {MD018}\nzz.md:1:1 error {MD018}\nzz.md:1 error {MD041}\n"
        ));
    assert!(!t.path().join("sub/m.md").exists());
}

/// 같은 경로가 glob 에도 매치되면 stdin 내용만 lint 하고 파일은 (`--fix` 여도) 건드리지 않는다.
#[test]
fn stdin_filename_shadows_matching_file() {
    let t = tree(&[("sub/a.md", "# ok\n")]);
    cmd(t.path())
        .args(["--fix", "--stdin-filename", "sub/a.md", "-", "**/*.md"])
        .write_stdin("#x\n")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: **/*.md\nLinting: 1 file(s)\nSummary: 2 error(s)\n"
        ))
        .stderr(format!(
            "sub/a.md:1:1 error {MD018}\nsub/a.md:1 error {MD041}\n"
        ));
    assert_eq!(
        fs::read_to_string(t.path().join("sub/a.md")).unwrap(),
        "# ok\n"
    );
}

#[test]
fn stdin_filename_with_format_uses_directory_config() {
    let t = tree(&[("sub/.markdownlint.jsonc", r#"{"MD018": false}"#)]);
    cmd(t.path())
        .args(["--format", "--stdin-filename", "sub/a.md"])
        .write_stdin("#x")
        .assert()
        .code(0)
        .stdout("#x\n")
        .stderr("");
}

#[test]
fn format_writes_fixed_to_stdout() {
    let t = tree(&[]);
    cmd(t.path())
        .arg("--format")
        .write_stdin("#x")
        .assert()
        .code(0)
        .stdout("# x\n")
        .stderr("");
}

#[test]
fn cjs_config_is_error() {
    let t = tree(&[
        ("a.md", "#x"),
        (".markdownlint-cli2.cjs", "module.exports = {};"),
    ]);
    cmd(t.path())
        .arg("**/*.md")
        .assert()
        .code(2)
        .stdout(BANNER)
        .stderr(
            predicate::str::starts_with("Error: Unable to use configuration file '")
                .and(predicate::str::contains(".markdownlint-cli2.cjs")),
        );
}

#[test]
fn invalid_config_prints_banner_then_error() {
    let t = tree(&[("a.md", "#x"), (".markdownlint-cli2.jsonc", "{")]);
    cmd(t.path())
        .arg("**/*.md")
        .assert()
        .code(2)
        .stdout(BANNER)
        .stderr(predicate::str::starts_with(
            "Error: Unable to use configuration file '",
        ));
}

#[test]
fn locale_sort_and_nested_ignores() {
    let t = tree(&[
        (".markdownlint-cli2.jsonc", r#"{"ignores": ["skip"]}"#),
        ("sub/.markdownlint-cli2.jsonc", r#"{"ignores": ["b.md"]}"#),
        ("a.md", "#x"),
        ("sub/a.md", "#x"),
        ("sub/b.md", "#x"),
        ("skip/s.md", "#x"),
        ("README.md", "#x"),
        ("a_b.md", "#x"),
        ("a-b.md", "#x"),
        ("Zed.md", "#x"),
        ("B.md", "#x"),
    ]);
    let files = [
        "a_b.md",
        "a-b.md",
        "a.md",
        "B.md",
        "README.md",
        "sub/a.md",
        "Zed.md",
    ];
    let stderr: String = files
        .iter()
        .map(|f| format!("{f}:1:1 error {MD018}\n{f}:1 error {MD041}\n{f}:1:2 error {MD047}\n"))
        .collect();
    // Linting 수는 sub/b.md (하위 ignores) 를 포함하고 skip/ (base ignores) 는 제외
    cmd(t.path())
        .arg("**/*.md")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: **/*.md !skip\nLinting: 8 file(s)\nSummary: 21 error(s)\n"
        ))
        .stderr(stderr);
}

#[test]
fn warning_severity_show_found_exit_0() {
    let t = tree(&[
        ("a.md", "#x"),
        (
            ".markdownlint-cli2.jsonc",
            r#"{"config": {"MD018": {"severity": "warning"}, "MD041": false, "MD047": false}, "showFound": true}"#,
        ),
    ]);
    cmd(t.path())
        .arg("**/*.md")
        .assert()
        .code(0)
        .stdout(format!(
            "{BANNER}Finding: **/*.md\nFound:\n a.md\nLinting: 1 file(s)\nSummary: 1 error(s)\n"
        ))
        .stderr(format!("a.md:1:1 warning {MD018}\n"));
}

#[test]
fn no_progress_no_banner() {
    let t = tree(&[
        ("a.md", "#x"),
        (
            ".markdownlint-cli2.jsonc",
            r#"{"noProgress": true, "noBanner": true}"#,
        ),
    ]);
    cmd(t.path())
        .arg("**/*.md")
        .assert()
        .code(1)
        .stdout("")
        .stderr(format!(
            "a.md:1:1 error {MD018}\na.md:1 error {MD041}\na.md:1:2 error {MD047}\n"
        ));
}

#[test]
fn invalid_front_matter_exit_2_after_progress() {
    let t = tree(&[
        ("a.md", "#x"),
        (".markdownlint-cli2.jsonc", r#"{"frontMatter": "(["}"#),
    ]);
    cmd(t.path())
        .arg("**/*.md")
        .assert()
        .code(2)
        .stdout(format!("{BANNER}Finding: **/*.md\nLinting: 1 file(s)\n"))
        .stderr(predicate::str::starts_with("Error: "));
}

#[test]
fn no_match_exit_0() {
    let t = tree(&[]);
    cmd(t.path())
        .arg("nothing/*.md")
        .assert()
        .code(0)
        .stdout(format!(
            "{BANNER}Finding: nothing/*.md\nLinting: 0 file(s)\nSummary: 0 error(s)\n"
        ))
        .stderr("");
}

#[test]
fn literal_files() {
    let t = tree(&[("a.md", "#x")]);
    cmd(t.path())
        .arg(":./a.md")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: :./a.md\nLinting: 1 file(s)\nSummary: 3 error(s)\n"
        ))
        .stderr(format!(
            "a.md:1:1 error {MD018}\na.md:1 error {MD041}\na.md:1:2 error {MD047}\n"
        ));
    cmd(t.path())
        .arg(":missing.md")
        .assert()
        .code(2)
        .stdout(format!(
            "{BANNER}Finding: :missing.md\nLinting: 1 file(s)\n"
        ))
        .stderr(predicate::str::starts_with("Error: "));
}

#[test]
fn config_globs_and_no_globs() {
    let t = tree(&[
        ("docs/x.md", "#x"),
        (".markdownlint-cli2.jsonc", r#"{"globs": ["docs/*.md"]}"#),
    ]);
    cmd(t.path())
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: docs/*.md\nLinting: 1 file(s)\nSummary: 3 error(s)\n"
        ))
        .stderr(format!(
            "docs/x.md:1:1 error {MD018}\ndocs/x.md:1 error {MD041}\ndocs/x.md:1:2 error {MD047}\n"
        ));
    cmd(t.path())
        .arg("--no-globs")
        .assert()
        .code(2)
        .stdout(help_stdout());
}

#[test]
fn unsupported_option_warns_on_stderr() {
    let t = tree(&[
        ("a.md", "# x\n"),
        (".markdownlint-cli2.jsonc", r#"{"customRules": ["x"]}"#),
    ]);
    cmd(t.path())
        .arg("a.md")
        .assert()
        .code(0)
        .stderr("Ignoring unsupported option: customRules\n");
}

/// 내장 포맷터는 원본 패키지 이름으로 지정한다. 파일 포맷터의 출력 형식 자체는 cli2 시나리오
/// (`outputFormatters*`, `formatter-*`) 가 원본 스냅샷과 대조한다.
#[test]
fn output_formatters_by_package_name() {
    let t = tree(&[
        ("a.md", "#x\n"),
        (
            ".markdownlint-cli2.jsonc",
            r#"{"outputFormatters": [["markdownlint-cli2-formatter-json", {"name": "out.json", "spaces": 1}], ["markdownlint-cli2-formatter-summarize", {"byRule": true}]]}"#,
        ),
    ]);
    cmd(t.path())
        .arg("a.md")
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: a.md\nLinting: 1 file(s)\nSummary: 2 error(s)\nCount Rule\n    1 MD018/no-missing-space-atx\n    1 MD041/first-line-heading/first-line-h1\n    2 [Total]\n"
        ))
        .stderr("");
    let json = fs::read_to_string(t.path().join("out.json")).unwrap();
    assert!(json.starts_with("[\n {\n  \"fileName\": \"a.md\",\n  \"lineNumber\": 1,\n  \"ruleNames\": [\n   \"MD018\","), "{json}");
    assert!(
        json.ends_with("\n  \"severity\": \"error\"\n }\n]"),
        "{json}"
    );
}

#[test]
fn output_formatters_empty_array_prints_nothing() {
    let t = tree(&[
        ("a.md", "#x\n"),
        (".markdownlint-cli2.jsonc", r#"{"outputFormatters": []}"#),
    ]);
    cmd(t.path()).arg("a.md").assert().code(1).stderr("");
}

/// 원본은 모듈 import 에 실패하면 Summary 뒤에 오류로 exit 2 한다.
#[test]
fn unknown_output_formatter_is_error_after_summary() {
    let t = tree(&[
        ("a.md", "# x\n"),
        (
            ".markdownlint-cli2.jsonc",
            r#"{"outputFormatters": [["markdownlint-cli2-formatter-json"], ["./my-formatter.cjs"]]}"#,
        ),
    ]);
    cmd(t.path())
        .arg("a.md")
        .assert()
        .code(2)
        .stdout(format!(
            "{BANNER}Finding: a.md\nLinting: 1 file(s)\nSummary: 0 error(s)\n"
        ))
        .stderr("Error: Unable to import module './my-formatter.cjs'.\n");
    assert!(!t.path().join("markdownlint-cli2-results.json").exists());
}

#[test]
fn front_matter_and_bom() {
    let t = tree(&[("fm.md", "\u{FEFF}---\ntitle: x\n---\n#x")]);
    cmd(t.path()).arg("fm.md").assert().code(1).stderr(format!(
        "fm.md:4:1 error {MD018}\nfm.md:4:2 error {MD047}\n"
    ));
}

// 원본은 fs.readFile(file, "utf8") 이라 잘못된 바이트가 U+FFFD 로 치환되고 lint 는 계속된다.
// 치환 단위(잘못된 시퀀스 하나당 U+FFFD 하나)가 같아야 컬럼이 맞는다: 잘린 3바이트/4바이트 1개,
// overlong C0 AF 와 FF FE 는 각각 2개.
const BAD_UTF8: &[u8] = b"# T\n\nab\xe2\x82cd\xf0\x9f\x98e\xc0\xaff\xff\xfe\tx\n";
const MD010: &str = "MD010/no-hard-tabs Hard tabs [Column: 13]";

#[test]
fn invalid_utf8_file_is_decoded_lossily_like_cli2() {
    let t = tree(&[]);
    fs::write(t.path().join("bad.md"), BAD_UTF8).unwrap();
    fs::write(
        t.path().join("img.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00",
    )
    .unwrap();
    cmd(t.path())
        .args(["bad.md", "img.png"])
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: bad.md img.png\nLinting: 2 file(s)\nSummary: 3 error(s)\n"
        ))
        .stderr(format!(
            "bad.md:3:13 error {MD010}\nimg.png:1 error MD041/first-line-heading/first-line-h1 First line in a file should be a top-level heading [Context: \"\u{FFFD}PNG\"]\nimg.png:4:5 error {MD047}\n"
        ));
}

#[test]
fn invalid_utf8_stdin_is_decoded_lossily_like_cli2() {
    let t = tree(&[]);
    cmd(t.path())
        .arg("-")
        .write_stdin(BAD_UTF8)
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: \nLinting: 1 file(s)\nSummary: 1 error(s)\n"
        ))
        .stderr(format!("stdin:3:13 error {MD010}\n"));
}

/// `$schema` 는 cli2 `constants.mjs` 의 `cli2SchemaKeys` 에 없다. 임의 이름의 설정 파일이
/// 이 키를 가졌다고 해서 옵션 객체로 오분류되면 안 된다 (#195).
#[test]
fn schema_key_does_not_make_config_an_options_object() {
    let t = tree(&[
        ("a.md", "#x\n"),
        (
            "cfg.json",
            r#"{"$schema": "https://json.schemastore.org/markdownlint", "default": false, "MD018": true}"#,
        ),
    ]);
    cmd(t.path())
        .args(["--config", "cfg.json", "a.md"])
        .assert()
        .code(1)
        .stdout(format!(
            "{BANNER}Finding: a.md\nLinting: 1 file(s)\nSummary: 1 error(s)\n"
        ))
        .stderr(format!("a.md:1:1 error {MD018}\n"));
}
