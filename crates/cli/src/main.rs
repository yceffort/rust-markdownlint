use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use rayon::prelude::*;
use rust_markdownlint::config::GitIgnore;
use rust_markdownlint::error::{LintError, Severity};
use rust_markdownlint::fix::apply_fixes;
use rust_markdownlint::lint::{LintOptions, lint_content};
use rust_markdownlint_cli::argv::parse_argv;
use rust_markdownlint_cli::dirs::{
    DirInfo, create_dir_infos, read_base_options, read_dir_config, remove_ignored_files,
    resolve_globs,
};
use rust_markdownlint_cli::formatters::{self, Formatter};
use rust_markdownlint_cli::globs::enumerate_files;
use rust_markdownlint_cli::output::{BANNER, HELP, LintResult, relative_posix, sort_results};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e}");
            2
        }
    };
    // process::exit 는 stdout 버퍼를 비우지 않는다
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

/// `path.resolve(base, p)`: 절대 경로화 후 `.` 과 `..` 정규화.
fn resolve(base: &Path, p: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for component in base.join(p).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            component => out.push(component),
        }
    }
    out
}

fn warn(message: &str) {
    eprintln!("{message}");
}

struct FileOutcome {
    name: String,
    errors: Vec<LintError>,
    /// `--format` 일 때 stdin 을 고친 결과
    formatted: Option<String>,
}

/// 원본 `fs.readFile(file, "utf8")`: 잘못된 시퀀스는 U+FFFD 로 치환하고 계속한다 (BOM 은 남긴다).
/// Node 와 Rust 모두 WHATWG 방식(maximal subpart 하나당 U+FFFD 하나)이라 컬럼이 같다.
fn lossy_utf8(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

/// 파일 하나의 lint(+fix). `--format` 은 stdin 만 고치고 결과는 버린다.
fn lint_file(
    base: &Path,
    info: &DirInfo,
    file: &Path,
    non_file: &HashMap<PathBuf, String>,
    formatting: bool,
) -> Result<FileOutcome> {
    let opts = LintOptions {
        config: info.effective_config.as_ref(),
        front_matter: info.options.front_matter.as_deref(),
        no_inline_config: info.options.no_inline_config == Some(true),
    };
    let name = relative_posix(base, file);
    let mut formatted = None;
    let errors = if let Some(content) = non_file.get(file) {
        let errors = lint_content(&name, content, &opts)?;
        if formatting {
            formatted = Some(apply_fixes(content, &errors));
            Vec::new()
        } else {
            errors
        }
    } else {
        let content = lossy_utf8(std::fs::read(file)?);
        let mut errors = lint_content(&name, &content, &opts)?;
        if formatting {
            errors = Vec::new();
        } else if info.options.fix == Some(true) && errors.iter().any(|e| e.fix_info.is_some()) {
            let fixed = apply_fixes(&content, &errors);
            std::fs::write(file, &fixed)?;
            errors = lint_content(&name, &fixed, &opts)?;
        }
        errors
    };
    Ok(FileOutcome {
        name,
        errors,
        formatted,
    })
}

/// 원본 `main` 순서: 배너 → base 옵션 → glob → dirInfos → lint(+fix) → 정렬 → Summary → 포매터 → exit.
fn run(args: &[String]) -> Result<i32> {
    // cli2 에 없는 서브커맨드라 argv 파서 앞에서 가로챈다
    #[cfg(feature = "server")]
    if args.first().is_some_and(|arg| arg == "server") {
        return rust_markdownlint_cli::server::run();
    }
    let argv = parse_argv(args);
    if argv.help {
        println!("{BANNER}");
        println!("{HELP}");
        return Ok(2);
    }
    let base = std::env::current_dir()?;
    let formatting = argv.format;

    let base_options = read_base_options(&base, &argv, &mut warn);
    // 원본 finally 블록: 옵션 읽기에 실패해도 배너는 출력
    if !formatting
        && !base_options
            .as_ref()
            .is_ok_and(|o| o.no_banner == Some(true))
    {
        println!("{BANNER}");
    }
    let base_options = base_options?;
    // 원본 getBaseOptions 는 base 의 `.markdownlint.*` 도 이 시점에 읽으므로 파싱 오류가 Finding 전에 난다
    read_dir_config(&base)?;
    let patterns = resolve_globs(&argv, &base_options);
    if (patterns.is_empty() && !argv.use_stdin) || argv.config_path == Some(None) {
        println!("{HELP}");
        return Ok(2);
    }

    let mut non_file: HashMap<PathBuf, String> = HashMap::new();
    if argv.use_stdin {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes)?;
        // --stdin-filename: 그 경로에 파일이 있는 것처럼 이름과 디렉토리 설정 계층을 정한다
        let path = match &argv.stdin_filename {
            Some(p) => resolve(&base, p),
            None => base.join("stdin"),
        };
        non_file.insert(path, lossy_utf8(bytes));
    }

    let show_progress = base_options.no_progress != Some(true) && !formatting;
    if show_progress {
        println!("Finding: {}", patterns.join(" "));
    }

    // enumerateFiles: ':' 리터럴 분리, '\:' 언이스케이프, 리터럴에는 base globs 의 '!' 패턴만 적용
    let (literal, glob_patterns): (Vec<&String>, Vec<&String>) =
        patterns.iter().partition(|p| p.starts_with(':'));
    let glob_patterns: Vec<String> = glob_patterns
        .into_iter()
        .map(|p| match p.strip_prefix("\\:") {
            Some(rest) => format!(":{rest}"),
            None => p.clone(),
        })
        .collect();
    let literal: Vec<PathBuf> = literal.iter().map(|p| resolve(&base, &p[1..])).collect();
    let globs_for_ignore: Vec<String> = base_options
        .globs
        .iter()
        .flatten()
        .filter_map(|g| g.strip_prefix('!').map(str::to_string))
        .collect();
    let literal = if !literal.is_empty() && !globs_for_ignore.is_empty() {
        remove_ignored_files(&base, literal, &globs_for_ignore)
    } else {
        literal
    };
    let gitignore = base_options
        .gitignore
        .clone()
        .unwrap_or(GitIgnore::Enabled(false));
    let mut files = enumerate_files(&base, &glob_patterns, &gitignore);
    files.extend(literal);
    // glob 이 같은 경로를 찾았어도 stdin 내용을 한 번만 lint 한다
    let stdin_paths: Vec<PathBuf> = non_file
        .keys()
        .filter(|k| !files.contains(k))
        .cloned()
        .collect();
    files.extend(stdin_paths);
    let dir_infos = create_dir_infos(&base, &files, &base_options, &mut warn)?;

    if show_progress {
        let mut names: Vec<String> = dir_infos
            .iter()
            .flat_map(|d| d.files.iter().map(|f| relative_posix(&base, f)))
            .collect();
        let count = names.len();
        if base_options.show_found == Some(true) {
            names.push(String::new());
            names.sort();
            println!("Found:{}", names.join("\n "));
        }
        println!("Linting: {count} file(s)");
    }

    // lintFiles: 파일 단위로 병렬 실행하되 결과는 파일 순서대로 모은 뒤 정렬하므로 출력은 순차 실행과 같다.
    let jobs: Vec<(&DirInfo, PathBuf)> = dir_infos
        .iter()
        .flat_map(|info| {
            info.files_after_ignores()
                .into_iter()
                .map(move |f| (info, f))
        })
        .collect();
    let outcomes: Vec<Result<FileOutcome>> = jobs
        .par_iter()
        .map(|(info, file)| lint_file(&base, info, file, &non_file, formatting))
        .collect();

    let mut results = Vec::new();
    let mut errors_present = false;
    let mut formatted = None;
    for outcome in outcomes {
        let outcome = outcome?;
        formatted = outcome.formatted.or(formatted);
        errors_present |= outcome
            .errors
            .iter()
            .any(|e| e.severity != Severity::Warning);
        results.extend(outcome.errors.into_iter().map(|error| LintResult {
            file_name: outcome.name.clone(),
            error,
        }));
    }

    sort_results(&mut results);
    if show_progress {
        println!("Summary: {} error(s)", results.len());
    }
    if formatting {
        print!("{}", formatted.unwrap_or_default());
    } else {
        // outputResults: base 옵션의 outputFormatters (빈 배열이면 아무것도 안 함), 없으면 기본 포맷터
        let formatters = match &base_options.output_formatters {
            Some(entries) => formatters::resolve_all(entries)?,
            None => vec![(Formatter::Default, serde_json::Value::Null)],
        };
        formatters::run(&formatters, &base, &results)?;
    }
    Ok(if errors_present { 1 } else { 0 })
}
