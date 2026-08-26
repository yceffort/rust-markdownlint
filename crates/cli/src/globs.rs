use std::path::{Path, PathBuf};

use rust_markdownlint::config::GitIgnore;

/// cli2 `processArgv` 의 glob 정규화. `["."]` → `"*.{md,markdown}"` 치환은 호출부 책임.
pub fn normalize_glob(g: &str) -> String {
    if g.starts_with(':') {
        return g.to_string();
    }
    if let Some(rest) = g.strip_prefix("\\:") {
        return format!("\\:{}", replace_special_backslashes(rest));
    }
    match g.strip_prefix('#') {
        Some(rest) => replace_special_backslashes(&format!("!{rest}")),
        None => replace_special_backslashes(g),
    }
}

/// 원본 정규식 `/\\(?![$()*+?[\]^])/gu` 와 동일: fast-glob 특수문자 이스케이프가 아닌
/// 백슬래시만 `/` 로 바꾼다.
fn replace_special_backslashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && !chars.peek().is_some_and(|n| "$()*+?[]^".contains(*n)) {
            out.push('/');
        } else {
            out.push(c);
        }
    }
    out
}

fn has_glob_meta(segment: &str) -> bool {
    segment.contains(['*', '?', '[', ']', '{', '}', '\\'])
}

/// 패턴 앞쪽의 glob 메타문자 없는 세그먼트들 (fast-glob 의 static prefix). 이 밖의
/// 디렉토리는 어떤 파일도 매치할 수 없어 순회하지 않는다.
fn static_prefix(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .take_while(|s| !has_glob_meta(s))
        .map(str::to_string)
        .collect()
}

fn build_glob(pattern: &str) -> Option<globset::Glob> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
        .ok()
}

/// globby 의미론(absolute, dot:true, 디렉토리 확장, 부정, gitignore)으로 base 아래 파일 열거.
/// 결과는 정렬된 절대 경로.
pub fn enumerate_files(base: &Path, patterns: &[String], gitignore: &GitIgnore) -> Vec<PathBuf> {
    let mut positive = globset::GlobSetBuilder::new();
    let mut negative = globset::GlobSetBuilder::new();
    // fast-glob DeepFilter: `/**` 로 끝나거나 마지막 세그먼트가 정적인 부정 패턴은 그 디렉토리 자체를
    // 순회하지 않는다 (`!**/node_modules` 는 아래 파일까지 제외, `!a/*` 는 아니다).
    let mut prune = globset::GlobSetBuilder::new();
    let mut prefixes: Vec<Vec<String>> = Vec::new();
    let mut has_positive = false;
    for pattern in patterns {
        let (negated, pattern) = match pattern.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, pattern.as_str()),
        };
        // globby expandDirectories: 디렉토리 패턴은 `{p}/**` 로 확장
        let pattern = if base.join(pattern).is_dir() {
            format!("{}/**", pattern.trim_end_matches('/'))
        } else {
            pattern.to_string()
        };
        // suppressErrors: 잘못된 패턴은 무시
        let Some(glob) = build_glob(&pattern) else {
            continue;
        };
        if negated {
            negative.add(glob);
            if let Some(dir) = pattern.strip_suffix("/**") {
                prune.add(build_glob(dir).expect("prefix of a valid glob"));
                prune.add(build_glob(&pattern).expect("valid glob"));
            } else if !pattern.rsplit('/').next().is_some_and(has_glob_meta) {
                prune.add(build_glob(&pattern).expect("valid glob"));
            }
        } else {
            positive.add(glob);
            prefixes.push(static_prefix(&pattern));
            has_positive = true;
        }
    }
    // globby expandNegationOnlyPatterns:false — 양의 패턴이 없으면 빈 결과
    if !has_positive {
        return Vec::new();
    }
    let (Ok(positive), Ok(negative), Ok(prune)) =
        (positive.build(), negative.build(), prune.build())
    else {
        return Vec::new();
    };

    let base_owned = base.to_path_buf();
    let mut walk = ignore::WalkBuilder::new(base);
    // fast-glob 기본값 followSymbolicLinks:true (pnpm node_modules 등)
    walk.standard_filters(false)
        .hidden(false)
        .follow_links(true)
        .filter_entry(move |entry| {
            if !entry.file_type().is_some_and(|t| t.is_dir()) {
                return true;
            }
            let Ok(rel) = entry.path().strip_prefix(&base_owned) else {
                return true;
            };
            if rel.as_os_str().is_empty() {
                return true;
            }
            if prune.is_match(rel) {
                return false;
            }
            let dir: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            // 어떤 양의 패턴의 static prefix 와도 같은 줄기에 있어야 한다
            prefixes.iter().any(|p| {
                let n = p.len().min(dir.len());
                p[..n] == dir[..n]
            })
        });
    match gitignore {
        GitIgnore::Enabled(true) => {
            walk.git_ignore(true).require_git(false);
        }
        GitIgnore::Enabled(false) => {}
        GitIgnore::Pattern(pattern) => {
            // globby ignoreFiles 는 파일 glob 이지만 ignore 크레이트는 파일명 단위라
            // 마지막 경로 요소만 사용한다
            if let Some(name) = Path::new(pattern).file_name() {
                walk.add_custom_ignore_filename(name);
            }
        }
    }

    let mut files = Vec::new();
    for entry in walk.build().flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(base) else {
            continue;
        };
        if positive.is_match(rel) && !negative.is_match(rel) {
            files.push(entry.into_path());
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn normalize() {
        assert_eq!(normalize_glob("#node_modules"), "!node_modules");
        assert_eq!(normalize_glob("a\\b"), "a/b");
        assert_eq!(normalize_glob("a\\*b"), "a\\*b");
        assert_eq!(normalize_glob(":lit\\p"), ":lit\\p");
        assert_eq!(normalize_glob("\\:esc\\d"), "\\:esc/d");
        // node 오라클(원본 specialCharacters 정규식) 대조 값
        assert_eq!(normalize_glob("a\\\\b"), "a//b");
        assert_eq!(normalize_glob("x\\[y"), "x\\[y");
        assert_eq!(normalize_glob("trail\\"), "trail/");
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for d in ["docs/sub", ".hidden", "node_modules"] {
            fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        for f in [
            "docs/a.md",
            "docs/sub/b.txt",
            ".hidden/c.md",
            "node_modules/d.md",
        ] {
            fs::write(dir.path().join(f), "x\n").unwrap();
        }
        dir
    }

    // Windows 에서도 `/` 구분자로 비교
    fn rel(base: &Path, files: &[PathBuf]) -> Vec<String> {
        files
            .iter()
            .map(|f| {
                f.strip_prefix(base)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect()
    }

    #[test]
    fn enumerate_expands_dirs_and_negates() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();

        let files = enumerate_files(
            &base,
            &["**/*.md".into(), "!node_modules".into()],
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), [".hidden/c.md", "docs/a.md"]);

        let files = enumerate_files(&base, &["docs".into()], &GitIgnore::Enabled(false));
        assert_eq!(rel(&base, &files), ["docs/a.md", "docs/sub/b.txt"]);
    }

    #[cfg(unix)]
    #[test]
    fn enumerate_follows_symlinks() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(base.join("docs"), base.join("linked")).unwrap();
        let files = enumerate_files(
            &base,
            &["linked/**/*.md".into()],
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), ["linked/a.md"]);
    }

    /// fast-glob DeepFilter: 마지막 세그먼트가 정적인 부정 패턴은 디렉토리 아래 전체를 제외한다.
    #[test]
    fn enumerate_negative_static_basename_prunes_directory() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        let files = enumerate_files(
            &base,
            &["**/*.md".into(), "!**/node_modules".into()],
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), [".hidden/c.md", "docs/a.md"]);
        // 동적 basename 은 디렉토리를 끊지 않는다
        let files = enumerate_files(
            &base,
            &["**/*".into(), "!docs/s*".into()],
            &GitIgnore::Enabled(false),
        );
        assert!(rel(&base, &files).contains(&"docs/sub/b.txt".to_string()));
    }

    #[test]
    fn enumerate_negation_only_is_empty() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        // globby expandNegationOnlyPatterns:false 와 동일하게 빈 결과
        let files = enumerate_files(&base, &["!node_modules".into()], &GitIgnore::Enabled(false));
        assert!(files.is_empty());
    }

    #[test]
    fn enumerate_respects_gitignore() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        fs::write(base.join(".gitignore"), "docs/sub/\nnode_modules/\n").unwrap();

        let files = enumerate_files(&base, &["**/*".into()], &GitIgnore::Enabled(true));
        assert_eq!(
            rel(&base, &files),
            [".gitignore", ".hidden/c.md", "docs/a.md"]
        );
    }
}
