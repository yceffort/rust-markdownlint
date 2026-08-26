//! 디렉토리 설정 cascade. `tests/fixtures/{config-files,markdownlint-json,markdownlint-cli2-jsonc}`
//! 는 markdownlint-cli2 v0.22.1 `test/` 의 사본(MIT)이며, 기대값은 원본 snapshot 의 stderr 에서
//! 각 디렉토리에 적용된 설정을 역산한 것이다.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rust_markdownlint::config::{GitIgnore, Options};
use rust_markdownlint_cli::argv::{Argv, parse_argv};
use rust_markdownlint_cli::dirs::{DirInfo, create_dir_infos, read_base_options, resolve_globs};
use rust_markdownlint_cli::globs::enumerate_files;
use serde_json::json;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .canonicalize()
        .unwrap()
}

fn argv(args: &[&str]) -> Argv {
    parse_argv(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
}

fn run(base: &Path, args: &[&str]) -> Result<Vec<DirInfo>> {
    let argv = argv(args);
    let base_options = read_base_options(base, &argv, &mut |_| {})?;
    let patterns = resolve_globs(&argv, &base_options);
    let files = enumerate_files(base, &patterns, &GitIgnore::Enabled(false));
    create_dir_infos(base, &files, &base_options, &mut |_| {})
}

fn rel(base: &Path, p: &Path) -> String {
    let s = p
        .strip_prefix(base)
        .unwrap()
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if s.is_empty() { ".".into() } else { s }
}

/// (디렉토리, 파일들) 을 base 기준 posix 상대 경로로.
fn summary(base: &Path, infos: &[DirInfo]) -> Vec<(String, Vec<String>)> {
    infos
        .iter()
        .map(|i| {
            (
                rel(base, &i.dir),
                i.files.iter().map(|f| rel(base, f)).collect(),
            )
        })
        .collect()
}

fn pairs(v: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    v.iter()
        .map(|(d, fs)| (d.to_string(), fs.iter().map(|f| f.to_string()).collect()))
        .collect()
}

fn three_rules() -> serde_json::Value {
    json!({
        "no-trailing-spaces": false,
        "no-multiple-space-atx": false,
        "single-trailing-newline": false
    })
}

fn four_rules() -> serde_json::Value {
    json!({
        "no-multiple-blanks": false,
        "no-trailing-spaces": false,
        "no-multiple-space-atx": false,
        "single-trailing-newline": false
    })
}

#[test]
fn markdownlint_file_replaces_without_merge() {
    let base = fixture("markdownlint-json");
    let infos = run(&base, &["**/*.md"]).unwrap();
    assert_eq!(
        summary(&base, &infos),
        pairs(&[
            ("dir/subdir", &["dir/subdir/info.md"]),
            (".", &["viewme.md", "dir/about.md"]),
        ])
    );
    // 원본 snapshot: dir/subdir/info.md 에 MD012 가 보고된다 (부모의 no-multiple-blanks 미상속)
    assert_eq!(
        infos[0].effective_config,
        Some(json!({"first-line-heading": false}))
    );
    assert_eq!(
        infos[1].effective_config,
        Some(json!({"MD032": false, "no-multiple-blanks": false}))
    );
}

#[test]
fn cli2_options_merge_config_by_key() {
    let base = fixture("markdownlint-cli2-jsonc");
    let infos = run(&base, &["**/*.md"]).unwrap();
    assert_eq!(
        summary(&base, &infos),
        pairs(&[
            ("dir/subdir", &["dir/subdir/info.md"]),
            (".", &["viewme.md", "dir/about.md"]),
        ])
    );
    // 원본 snapshot: dir/subdir/info.md 에 MD012 가 없다 (부모 config 상속)
    assert_eq!(
        infos[0].effective_config,
        Some(json!({
            "MD032": false,
            "no-multiple-blanks": false,
            "first-line-heading": false
        }))
    );
    assert_eq!(
        infos[1].effective_config,
        Some(json!({"MD032": false, "no-multiple-blanks": false}))
    );
}

#[test]
fn config_arg_is_base_and_cascades() {
    let base = fixture("config-files");
    let infos = run(
        &base,
        &["--config", "cfg/.markdownlint-cli2.jsonc", "**/*.md"],
    )
    .unwrap();
    assert_eq!(
        summary(&base, &infos),
        pairs(&[
            ("dir2", &["dir2/viewme.md"]),
            (".", &["viewme.md", "dir1/viewme.md"]),
        ])
    );
    assert_eq!(infos[1].effective_config, Some(three_rules()));
    assert_eq!(infos[1].options.no_banner, Some(true));
    assert_eq!(infos[1].options.fix, Some(false));
    assert_eq!(infos[0].effective_config, Some(four_rules()));
    assert_eq!(infos[0].options.no_banner, Some(true));
}

#[test]
fn config_arg_formats() {
    let base = fixture("config-files");
    let base_of = |args: &[&str]| {
        let infos = run(&base, args).unwrap();
        infos.into_iter().find(|i| i.dir == base).unwrap()
    };

    // .markdownlint.* 는 { config } 로 감싸져 base 옵션이 된다
    let b = base_of(&["--config", "cfg/.markdownlint.json", "**/*.md"]);
    assert_eq!(b.effective_config, Some(three_rules()));
    assert_eq!(b.options.no_banner, None);

    // 지원 이름 접두어
    let b = base_of(&[
        "--config",
        "cfg/alternate.markdownlint-cli2.yaml",
        "**/*.md",
    ]);
    assert_eq!(b.effective_config, Some(three_rules()));
    assert_eq!(b.options.no_banner, Some(true));

    // 확장자만 지원: cli2 키 유무로 옵션/설정 판별
    let b = base_of(&["--config", "cfg/.markdownlint-cli2.toml", "**/*.md"]);
    assert_eq!(b.effective_config, Some(three_rules()));
    assert_eq!(b.options.no_banner, Some(true));
    let b = base_of(&["--config", "cfg/.markdownlint.toml", "**/*.md"]);
    assert_eq!(b.effective_config, Some(three_rules()));
    assert_eq!(b.options.no_banner, None);

    // extends 는 설정 파일 기준 상대 경로
    let b = base_of(&["--config", "cfg/options.yaml", "**/*.md"]);
    assert_eq!(b.effective_config, Some(four_rules()));
    assert_eq!(b.options.no_banner, Some(true));
    let b = base_of(&["--config", "cfg/config.jsonc", "**/*.md"]);
    assert_eq!(b.effective_config, Some(four_rules()));
    assert_eq!(b.options.no_banner, None);

    // 절대 경로
    let abs = base.join("cfg/.markdownlint-cli2.jsonc");
    let b = base_of(&["--config", abs.to_str().unwrap(), "**/*.md"]);
    assert_eq!(b.options.no_banner, Some(true));
}

#[test]
fn config_pointer() {
    let base = fixture("config-files");
    let base_of = |args: &[&str]| {
        let infos = run(&base, args).unwrap();
        infos.into_iter().find(|i| i.dir == base).unwrap()
    };

    let b = base_of(&[
        "--config",
        "cfg/config-nested.json",
        "--configPointer",
        "/nested",
        "**/*.md",
    ]);
    assert_eq!(b.effective_config, Some(four_rules()));

    let b = base_of(&[
        "--config",
        "cfg/options-nested-nested.yaml",
        "--configPointer",
        "/outer/inner",
        "**/*.md",
    ]);
    assert_eq!(b.effective_config, Some(four_rules()));
    assert_eq!(b.options.no_banner, Some(true));

    // 빈 포인터는 전체
    let b = base_of(&[
        "--config",
        "cfg/config.json",
        "--configPointer",
        "",
        "**/*.md",
    ]);
    assert_eq!(b.effective_config, Some(four_rules()));

    // falsy 값과 없는 경로는 {}
    for pointer in ["/null", "/missing"] {
        let b = base_of(&[
            "--config",
            "cfg/config-nested.json",
            "--configPointer",
            pointer,
            "**/*.md",
        ]);
        assert_eq!(b.effective_config, Some(json!({})));
    }

    let err = run(
        &base,
        &[
            "--config",
            "cfg/config.json",
            "--configPointer",
            "invalid",
            "**/*.md",
        ],
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Invalid JSON pointer.");
}

#[test]
fn config_arg_errors() {
    let base = fixture("config-files");

    let err = run(&base, &["--config", "cfg/unrecognized.txt", "**/*.md"]).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "Unable to use configuration file '{}'; Configuration file should be one of the supported names (e.g., '.markdownlint-cli2.jsonc') or a prefix with a supported name (e.g., 'example.markdownlint-cli2.jsonc') or have a supported extension (e.g., jsonc, json, yaml, yml, cjs, mjs).",
            base.join("cfg/unrecognized.txt").display()
        )
    );

    for file in [
        "invalid.markdownlint-cli2.jsonc",
        "invalid.markdownlint.json",
    ] {
        let err = run(&base, &["--config", &format!("cfg/{file}"), "**/*.md"]).unwrap_err();
        let prefix = format!(
            "Unable to use configuration file '{}'; ",
            base.join("cfg").join(file).display()
        );
        assert!(err.to_string().starts_with(&prefix), "{err}");
    }

    for file in [".markdownlint-cli2.cjs", ".markdownlint.mjs", "config.mjs"] {
        let err = run(&base, &["--config", &format!("cfg/{file}"), "**/*.md"]).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("Unable to use configuration file '"),
            "{msg}"
        );
        assert!(msg.contains(file), "{msg}");
    }

    let err = run(&base, &["--config", "cfg/does-not-exist.jsonc", "**/*.md"]).unwrap_err();
    assert!(
        err.to_string()
            .starts_with("Unable to use configuration file '"),
        "{err}"
    );
}

fn temp_tree(entries: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in entries {
        let full = dir.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }
    dir
}

#[test]
fn module_config_in_directory_is_error() {
    let dir = temp_tree(&[("sub/.markdownlint-cli2.mjs", ""), ("sub/a.md", "# a\n")]);
    let base = dir.path().canonicalize().unwrap();
    let err = run(&base, &["**/*.md"]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.starts_with("Unable to use configuration file '"),
        "{msg}"
    );
    assert!(msg.contains(".markdownlint-cli2.mjs"), "{msg}");

    let dir = temp_tree(&[("sub/.markdownlint.cjs", ""), ("sub/a.md", "# a\n")]);
    let base = dir.path().canonicalize().unwrap();
    assert!(run(&base, &["**/*.md"]).is_err());
}

#[test]
fn options_config_blocks_markdownlint_file_inheritance() {
    let dir = temp_tree(&[
        (".markdownlint.json", r#"{"MD047": false}"#),
        ("a.md", "# a\n"),
        (
            "with-config/.markdownlint-cli2.jsonc",
            r#"{"config": {"MD018": false}}"#,
        ),
        ("with-config/a.md", "# a\n"),
        ("no-config/.markdownlint-cli2.jsonc", r#"{"fix": true}"#),
        ("no-config/a.md", "# a\n"),
        ("own-file/.markdownlint.yaml", "MD018: false\n"),
        ("own-file/a.md", "# a\n"),
    ]);
    let base = dir.path().canonicalize().unwrap();
    let infos = run(&base, &["**/*.md"]).unwrap();
    let find = |d: &str| infos.iter().find(|i| i.dir == base.join(d)).unwrap();

    assert_eq!(
        find("with-config").effective_config,
        Some(json!({"MD018": false}))
    );
    assert_eq!(
        find("no-config").effective_config,
        Some(json!({"MD047": false}))
    );
    assert_eq!(find("no-config").options.fix, Some(true));
    assert_eq!(
        find("own-file").effective_config,
        Some(json!({"MD018": false}))
    );
    assert_eq!(find("").effective_config, Some(json!({"MD047": false})));
}

#[test]
fn base_globs_and_ignores() {
    let dir = temp_tree(&[
        (
            ".markdownlint-cli2.jsonc",
            r#"{"globs": ["*.md"], "ignores": ["skip"]}"#,
        ),
        ("a.md", "# a\n"),
        ("skip/s.md", "# s\n"),
        ("sub/.markdownlint-cli2.jsonc", r#"{"ignores": ["b.md"]}"#),
        ("sub/a.md", "# a\n"),
        ("sub/b.md", "# b\n"),
    ]);
    let base = dir.path().canonicalize().unwrap();

    let a = argv(&["**/*.md"]);
    let options = read_base_options(&base, &a, &mut |_| {}).unwrap();
    assert_eq!(resolve_globs(&a, &options), ["**/*.md", "*.md", "!skip"]);
    let no_globs = argv(&["**/*.md", "--no-globs"]);
    assert_eq!(resolve_globs(&no_globs, &options), ["**/*.md", "!skip"]);

    let infos = run(&base, &["**/*.md"]).unwrap();
    assert_eq!(
        summary(&base, &infos),
        pairs(&[("sub", &["sub/a.md", "sub/b.md"]), (".", &["a.md"])])
    );
    assert_eq!(infos[0].options.ignores, Some(vec!["b.md".into()]));
    // 하위 디렉토리 ignores 는 lint 시점에 적용
    assert_eq!(infos[0].files_after_ignores(), [base.join("sub/a.md")]);
}

#[test]
fn dot_only_glob_is_substituted() {
    assert_eq!(
        resolve_globs(&argv(&["."]), &Options::default()),
        ["*.{md,markdown}"]
    );
    assert_eq!(
        resolve_globs(&argv(&[".", "x"]), &Options::default()),
        [".", "x"]
    );
}

#[test]
fn files_outside_base_attach_to_base() {
    let dir = temp_tree(&[
        (
            "base/.markdownlint-cli2.jsonc",
            r#"{"config": {"MD047": false}}"#,
        ),
        ("base/a.md", "# a\n"),
        ("other/b.md", "# b\n"),
    ]);
    let root = dir.path().canonicalize().unwrap();
    let base = root.join("base");
    let files = vec![base.join("a.md"), root.join("other/b.md")];
    let options = read_base_options(&base, &argv(&["**/*.md"]), &mut |_| {}).unwrap();
    let infos = create_dir_infos(&base, &files, &options, &mut |_| {}).unwrap();
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].dir, base);
    assert_eq!(infos[0].files, files);
}
