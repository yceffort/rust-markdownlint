use std::path::{Path, PathBuf};
use std::sync::Arc;

use rust_markdownlint::config::GitIgnore;

use crate::output::relative_posix;

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

/// picomatch 는 `p/**` 로 `p` 자체도 매치하므로 둘 다 넣는다. suppressErrors: 잘못된 패턴은 무시.
fn add_glob(set: &mut globset::GlobSetBuilder, pattern: &str) {
    if let Some(glob) = build_glob(pattern) {
        set.add(glob);
    }
    if let Some(dir) = pattern.strip_suffix("/**")
        && let Some(glob) = build_glob(dir)
    {
        set.add(glob);
    }
}

/// fast-glob `removeLeadingDotSegment`.
fn remove_leading_dot_segment(pattern: &str) -> &str {
    pattern.strip_prefix("./").unwrap_or(pattern)
}

/// globby `directoryToGlob`: `**/name` (정적이고 확장자 없는 마지막 세그먼트) 과 실제 디렉토리는
/// `{p}/**` 로 확장한다.
fn expand_directory(base: &Path, pattern: &str) -> String {
    if let Some((_, name)) = pattern.rsplit_once("**/")
        && !name.is_empty()
        && !name.contains('/')
        && !name.contains(['*', '?', '[', ']', '{', '}'])
        && (name.starts_with('.') || name.rfind('.').is_none_or(|i| i == 0))
    {
        return format!("{pattern}/**");
    }
    if base.join(pattern).is_dir() {
        return format!("{}/**", pattern.trim_end_matches('/'));
    }
    pattern.to_string()
}

/// globby `getParentDirectoryPrefix`: 앞쪽의 `../` 반복.
fn parent_prefix(pattern: &str) -> &str {
    let mut end = 0;
    while pattern[end..].starts_with("../") {
        end += 3;
    }
    &pattern[..end]
}

/// globby 의 fast-glob 작업 하나: 양의 패턴들과 그 뒤에 온 부정 패턴들.
struct Task {
    positive: globset::GlobSet,
    ignore: globset::GlobSet,
    /// fast-glob DeepFilter: `/**` 로 끝나거나 마지막 세그먼트가 정적인 부정 패턴은 그 디렉토리 자체를
    /// 순회하지 않는다 (`!**/node_modules` 는 아래 파일까지 제외, `!a/*` 는 아니다).
    prune: globset::GlobSet,
    prefixes: Vec<Vec<String>>,
}

impl Task {
    fn new(base: &Path, positive: &[String], ignore: &[String]) -> Option<Task> {
        let positive: Vec<String> = positive
            .iter()
            .map(|p| expand_directory(base, remove_leading_dot_segment(p)))
            .collect();
        let ignore: Vec<String> = ignore
            .iter()
            .map(|p| expand_directory(base, remove_leading_dot_segment(p)))
            .collect();
        // globby adjustIgnorePatternsForParentDirectories: 양의 패턴이 모두 같은 `../` 접두어를
        // 가지면 `**/` 로 시작하는 부정 패턴도 같은 기준으로 옮긴다
        let parent = parent_prefix(&positive[0]);
        let same_parent = !parent.is_empty() && positive.iter().all(|p| parent_prefix(p) == parent);
        let ignore: Vec<String> = ignore
            .into_iter()
            .map(|p| {
                if same_parent && p.starts_with("**/") {
                    format!("{parent}{p}")
                } else {
                    p
                }
            })
            .collect();

        let mut positive_set = globset::GlobSetBuilder::new();
        let mut prefixes = Vec::new();
        for pattern in &positive {
            if let Some(glob) = build_glob(pattern) {
                positive_set.add(glob);
                prefixes.push(static_prefix(pattern));
            }
        }
        let mut ignore_set = globset::GlobSetBuilder::new();
        let mut prune = globset::GlobSetBuilder::new();
        for pattern in &ignore {
            add_glob(&mut ignore_set, pattern);
            if pattern.ends_with("/**") || !pattern.rsplit('/').next().is_some_and(has_glob_meta) {
                add_glob(&mut prune, pattern);
            }
        }
        Some(Task {
            positive: positive_set.build().ok()?,
            ignore: ignore_set.build().ok()?,
            prune: prune.build().ok()?,
            prefixes,
        })
    }

    /// 이 작업의 어떤 양의 패턴이 이 디렉토리 아래를 매치할 수 있는가.
    fn may_descend(&self, dir: &[String]) -> bool {
        if self.prune.is_match(dir.join("/")) {
            return false;
        }
        self.prefixes.iter().any(|p| {
            let n = p.len().min(dir.len());
            leading_dots(p) == leading_dots(dir) && p[..n] == dir[..n]
        })
    }

    /// fast-glob 은 패턴의 static prefix 디렉토리에서 순회를 시작하므로 `../` 개수가 다른 패턴은
    /// 서로의 파일을 보지 못한다 (`**/*.md` 가 `../x/a.md` 를 매치하면 안 된다).
    fn matches(&self, rel: &str, parts: &[String]) -> bool {
        !self.ignore.is_match(rel)
            && self
                .positive
                .matches(rel)
                .iter()
                .any(|&i| leading_dots(&self.prefixes[i]) == leading_dots(parts))
    }
}

fn leading_dots(parts: &[String]) -> usize {
    parts.iter().take_while(|c| *c == "..").count()
}

/// globby `convertNegativePatterns`: 부정 패턴은 앞선 양의 패턴에만 적용된다.
fn split_tasks(base: &Path, patterns: &[String]) -> Vec<Task> {
    let mut tasks: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for pattern in patterns {
        match pattern.strip_prefix('!') {
            Some(negated) => {
                for (_, ignore) in &mut tasks {
                    ignore.push(negated.to_string());
                }
                if !pending.is_empty() {
                    tasks.push((std::mem::take(&mut pending), vec![negated.to_string()]));
                }
            }
            None => pending.push(pattern.clone()),
        }
    }
    if !pending.is_empty() {
        tasks.push((pending, Vec::new()));
    }
    // expandNegationOnlyPatterns:false — 양의 패턴이 없으면 빈 결과
    tasks
        .iter()
        .filter_map(|(positive, ignore)| Task::new(base, positive, ignore))
        .collect()
}

/// `../` 로 시작하는 패턴은 base 밖에서 순회를 시작해야 한다.
fn walk_roots(base: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for pattern in patterns.iter().filter(|p| !p.starts_with('!')) {
        let mut root = base.to_path_buf();
        for _ in 0..parent_prefix(remove_leading_dot_segment(pattern)).len() / 3 {
            root.pop();
        }
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

fn components(rel: &str) -> Vec<String> {
    rel.split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// globby `ignoreFiles`: ignore 파일 하나를 그 디렉토리 기준으로 적용하는 matcher.
fn ignore_file_matcher(file: &Path) -> Option<ignore::gitignore::Gitignore> {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(file.parent()?);
    builder.add(file);
    builder.build().ok()
}

/// globby 의미론(absolute, dot:true, 디렉토리 확장, 부정, gitignore)으로 base 아래 파일 열거.
/// 결과는 정렬된 절대 경로.
pub fn enumerate_files(base: &Path, patterns: &[String], gitignore: &GitIgnore) -> Vec<PathBuf> {
    let tasks = Arc::new(split_tasks(base, patterns));
    if tasks.is_empty() {
        return Vec::new();
    }
    // globby ignoreFiles: base 기준 glob 에 맞는 ignore 파일 (양의 패턴과 무관하게 찾는다)
    let ignore_files_glob = match gitignore {
        GitIgnore::Pattern(pattern) => build_glob(pattern).map(|g| g.compile_matcher()),
        _ => None,
    };
    let mut ignore_matchers = Vec::new();

    let mut files: Vec<PathBuf> = Vec::new();
    for root in walk_roots(base, patterns) {
        let mut walk = ignore::WalkBuilder::new(&root);
        let (walk_tasks, base_owned) = (Arc::clone(&tasks), base.to_path_buf());
        // fast-glob 기본값 followSymbolicLinks:true (pnpm node_modules 등)
        walk.standard_filters(false)
            .hidden(false)
            .follow_links(true)
            .filter_entry(move |entry| {
                if !entry.file_type().is_some_and(|t| t.is_dir()) {
                    return true;
                }
                let dir = components(&relative_posix(&base_owned, entry.path()));
                dir.is_empty() || walk_tasks.iter().any(|t| t.may_descend(&dir))
            });
        if let GitIgnore::Enabled(true) = gitignore {
            walk.git_ignore(true).require_git(false);
        }
        for entry in walk.build().flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let rel = relative_posix(base, entry.path());
            if ignore_files_glob.as_ref().is_some_and(|g| g.is_match(&rel)) {
                ignore_matchers.extend(ignore_file_matcher(entry.path()));
            }
            let parts = components(&rel);
            if tasks.iter().any(|t| t.matches(&rel, &parts)) {
                files.push(entry.into_path());
            }
        }
    }
    files.retain(|file| {
        !ignore_matchers.iter().any(|m| {
            file.starts_with(m.path()) && m.matched_path_or_any_parents(file, false).is_ignore()
        })
    });
    files.sort();
    files.dedup();
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
        files.iter().map(|f| relative_posix(base, f)).collect()
    }

    fn globs(patterns: &[&str]) -> Vec<String> {
        patterns.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn enumerate_expands_dirs_and_negates() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();

        let files = enumerate_files(
            &base,
            &globs(&["**/*.md", "!node_modules"]),
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), [".hidden/c.md", "docs/a.md"]);

        let files = enumerate_files(&base, &globs(&["docs"]), &GitIgnore::Enabled(false));
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
            &globs(&["linked/**/*.md"]),
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
            &globs(&["**/*.md", "!**/node_modules"]),
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), [".hidden/c.md", "docs/a.md"]);
        // 동적 basename 은 디렉토리를 끊지 않는다
        let files = enumerate_files(
            &base,
            &globs(&["**/*", "!docs/s*"]),
            &GitIgnore::Enabled(false),
        );
        assert!(rel(&base, &files).contains(&"docs/sub/b.txt".to_string()));
    }

    #[test]
    fn enumerate_negation_only_is_empty() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        // globby expandNegationOnlyPatterns:false 와 동일하게 빈 결과
        let files = enumerate_files(
            &base,
            &globs(&["!node_modules"]),
            &GitIgnore::Enabled(false),
        );
        assert!(files.is_empty());
    }

    /// globby: 부정 패턴은 앞선 양의 패턴에만 적용된다 (cli2 시나리오 nested-directories).
    #[test]
    fn enumerate_negation_applies_to_preceding_patterns_only() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        let files = enumerate_files(
            &base,
            &globs(&["**", "!docs", "docs/sub", "!node_modules"]),
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), [".hidden/c.md", "docs/sub/b.txt"]);
    }

    /// `./` 접두어와 `../` 형제 디렉토리 패턴 (cli2 시나리오 file-paths-as-args, sibling-directory).
    #[test]
    fn enumerate_dot_prefix_and_parent_patterns() {
        let dir = fixture();
        let root = dir.path().canonicalize().unwrap();
        let base = root.join("docs");
        let files = enumerate_files(
            &base,
            &globs(&["./a.md", "../.hidden/**/*.md"]),
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), ["../.hidden/c.md", "a.md"]);
        // `**/*.md` 는 base 밖(`../node_modules/d.md`)을 보지 않는다
        let files = enumerate_files(
            &base,
            &globs(&["**/*.md", "../.hidden/**/*.md"]),
            &GitIgnore::Enabled(false),
        );
        assert_eq!(rel(&base, &files), ["../.hidden/c.md", "a.md"]);
    }

    #[test]
    fn enumerate_respects_gitignore() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        fs::write(base.join(".gitignore"), "docs/sub/\nnode_modules/\n").unwrap();

        let files = enumerate_files(&base, &globs(&["**/*"]), &GitIgnore::Enabled(true));
        assert_eq!(
            rel(&base, &files),
            [".gitignore", ".hidden/c.md", "docs/a.md"]
        );
    }

    /// globby ignoreFiles: glob 에 맞는 ignore 파일만 적용한다 (cli2 시나리오 gitignore-root-only).
    #[test]
    fn enumerate_ignore_files_pattern() {
        let dir = fixture();
        let base = dir.path().canonicalize().unwrap();
        fs::write(base.join(".gitignore"), "c.md\n").unwrap();
        fs::write(base.join("docs/.gitignore"), "a.md\n").unwrap();

        let files = enumerate_files(
            &base,
            &globs(&["**/*.md"]),
            &GitIgnore::Pattern(".gitignore".into()),
        );
        assert_eq!(rel(&base, &files), ["docs/a.md", "node_modules/d.md"]);
        let files = enumerate_files(
            &base,
            &globs(&["**/*.md"]),
            &GitIgnore::Pattern("**/.gitignore".into()),
        );
        assert_eq!(rel(&base, &files), ["node_modules/d.md"]);
    }
}
