//! cli2 `markdownlint-cli2.mjs` 의 `readOptionsOrConfig`, `getBaseOptions`, `createDirInfos` 포팅.
//! `.cjs`/`.mjs` 설정 파일은 JS 모듈 로딩이 필요하므로 발견 시 오류로 처리한다.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use rust_markdownlint::config::{
    ConfigError, ConfigValue, Format, OPTIONS_KEYS, Options, extend_config, merge_options,
    options_from_value, parse_config_as, read_config_file, truthy,
};
use serde_json::json;

use crate::argv::Argv;
use crate::globs::normalize_glob;

#[derive(Debug)]
pub struct DirInfo {
    pub dir: PathBuf,
    /// `ignores` 적용 전 파일. 원본의 `Linting: N file(s)` 는 이 수를 센다.
    pub files: Vec<PathBuf>,
    pub options: Options,
    /// 같은 디렉토리(또는 상속된) `.markdownlint.*` 우선, 없으면 `options.config`.
    /// None 이면 markdownlint 기본 설정.
    pub effective_config: Option<ConfigValue>,
}

impl DirInfo {
    /// 원본 `lintFiles` 의 `ignores` 적용 결과.
    pub fn files_after_ignores(&self) -> Vec<PathBuf> {
        match &self.options.ignores {
            Some(ignores) if !ignores.is_empty() => {
                remove_ignored_files(&self.dir, self.files.clone(), ignores)
            }
            _ => self.files.clone(),
        }
    }
}

const DOT_ONLY_SUBSTITUTE: &str = "*.{md,markdown}";

const UNSUPPORTED_NAME: &str = "Configuration file should be one of the supported names \
(e.g., '.markdownlint-cli2.jsonc') or a prefix with a supported name \
(e.g., 'example.markdownlint-cli2.jsonc') or have a supported extension \
(e.g., jsonc, json, yaml, yml, cjs, mjs).";

const MODULE_UNSUPPORTED: &str = "JavaScript configuration files (.cjs/.mjs) are not supported";

#[derive(Clone, Copy)]
enum Reader {
    Jsonc,
    Yaml,
    /// markdownlint `readConfig`: jsonc, toml, yaml 순서로 시도
    Config,
    Module,
}

/// 원본 `optionsFiles`. 순서가 우선순위.
const OPTIONS_FILES: &[(&str, Reader)] = &[
    (".markdownlint-cli2.jsonc", Reader::Jsonc),
    (".markdownlint-cli2.yaml", Reader::Yaml),
    (".markdownlint-cli2.cjs", Reader::Module),
    (".markdownlint-cli2.mjs", Reader::Module),
];

/// 원본 `configurationFiles`.
const CONFIG_FILES: &[(&str, Reader)] = &[
    (".markdownlint.jsonc", Reader::Config),
    (".markdownlint.json", Reader::Config),
    (".markdownlint.yaml", Reader::Config),
    (".markdownlint.yml", Reader::Config),
    (".markdownlint.cjs", Reader::Module),
    (".markdownlint.mjs", Reader::Module),
];

/// 원본 `throwForConfigurationFile`.
fn unusable(file: &Path, message: impl Display) -> anyhow::Error {
    anyhow!(
        "Unable to use configuration file '{}'; {message}",
        file.display()
    )
}

fn read_as(file: &Path, format: Format) -> Result<ConfigValue, ConfigError> {
    let content = std::fs::read_to_string(file)?;
    parse_config_as(format, &content)
}

/// 원본 `processFirstMatchingConfigurationFile`: 존재하는 첫 파일.
fn first_existing(dir: &Path, candidates: &[(&str, Reader)]) -> Option<(PathBuf, Reader)> {
    candidates
        .iter()
        .map(|(name, reader)| (dir.join(name), *reader))
        .find(|(file, _)| file.exists())
}

/// 디렉토리의 `.markdownlint-cli2.*`. 파싱 결과가 null 이면(빈 yaml) 옵션 없음.
fn read_dir_options(dir: &Path, warn: &mut dyn FnMut(&str)) -> Result<Option<Options>> {
    let Some((file, reader)) = first_existing(dir, OPTIONS_FILES) else {
        return Ok(None);
    };
    let format = match reader {
        Reader::Jsonc => Format::Jsonc,
        Reader::Yaml => Format::Yaml,
        Reader::Config | Reader::Module => return Err(unusable(&file, MODULE_UNSUPPORTED)),
    };
    let value = read_as(&file, format).map_err(|e| unusable(&file, e))?;
    if value.is_null() {
        return Ok(None);
    }
    let mut options = options_from_value(value, warn);
    if let Some(config) = options.config.take() {
        options.config = Some(extend_config(config, &file).map_err(|e| unusable(&file, e))?);
    }
    Ok(Some(options))
}

/// 디렉토리의 `.markdownlint.*`.
pub fn read_dir_config(dir: &Path) -> Result<Option<ConfigValue>> {
    let Some((file, reader)) = first_existing(dir, CONFIG_FILES) else {
        return Ok(None);
    };
    match reader {
        Reader::Module => Err(unusable(&file, MODULE_UNSUPPORTED)),
        _ => Ok(Some(read_config_file(&file)?)),
    }
}

/// 원본 `readOptionsOrConfig`: `--config` 파일을 이름으로 판별해 옵션 객체로 만든다.
fn read_options_or_config(
    file: &Path,
    pointer: Option<&str>,
    warn: &mut dyn FnMut(&str),
) -> Result<Options> {
    enum Kind {
        Options,
        Config,
        Unknown,
    }
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ends = |suffixes: &[&str]| suffixes.iter().any(|s| name.ends_with(s));
    let (kind, format) = if ends(&[".markdownlint-cli2.jsonc"]) {
        (Kind::Options, Format::Jsonc)
    } else if ends(&[".markdownlint-cli2.yaml"]) {
        (Kind::Options, Format::Yaml)
    } else if ends(&[".markdownlint-cli2.cjs", ".markdownlint-cli2.mjs"]) {
        return Err(unusable(file, MODULE_UNSUPPORTED));
    } else if ends(&[".markdownlint.jsonc", ".markdownlint.json"]) {
        (Kind::Config, Format::Jsonc)
    } else if ends(&[".markdownlint.yaml", ".markdownlint.yml"]) {
        (Kind::Config, Format::Yaml)
    } else if ends(&[".markdownlint.cjs", ".markdownlint.mjs"]) {
        return Err(unusable(file, MODULE_UNSUPPORTED));
    } else if ends(&[".jsonc", ".json"]) {
        (Kind::Unknown, Format::Jsonc)
    } else if ends(&[".toml"]) {
        (Kind::Unknown, Format::Toml)
    } else if ends(&[".yaml", ".yml"]) {
        (Kind::Unknown, Format::Yaml)
    } else if ends(&[".cjs", ".mjs"]) {
        return Err(unusable(file, MODULE_UNSUPPORTED));
    } else {
        return Err(unusable(file, UNSUPPORTED_NAME));
    };

    let mut value = read_as(file, format).map_err(|e| unusable(file, e))?;
    if let Some(pointer) = pointer {
        // jsonpointer: 빈 문자열은 전체, 그 외는 '/' 로 시작해야 한다
        if !pointer.is_empty() && !pointer.starts_with('/') {
            bail!("Invalid JSON pointer.");
        }
        // 원본 `obj && (jsonpointer.get(obj, pointer) || {})`
        if truthy(&value) {
            value = value
                .pointer(pointer)
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or_else(|| json!({}));
        }
    }
    if !truthy(&value) {
        return Err(unusable(file, "empty configuration"));
    }

    let kind = match kind {
        Kind::Unknown => {
            let has_options_key = value
                .as_object()
                .is_some_and(|o| o.keys().any(|k| OPTIONS_KEYS.contains(&k.as_str())));
            if has_options_key {
                Kind::Options
            } else {
                Kind::Config
            }
        }
        kind => kind,
    };
    match kind {
        Kind::Options | Kind::Unknown => {
            let mut options = options_from_value(value, warn);
            if let Some(config) = options.config.take() {
                options.config = Some(extend_config(config, file)?);
            }
            Ok(options)
        }
        Kind::Config => {
            let config = if value.is_object() { value } else { json!({}) };
            Ok(Options {
                config: Some(extend_config(config, file)?),
                ..Options::default()
            })
        }
    }
}

/// 원본 `readOptionsOrConfig` + `getBaseOptions`:
/// `{fix: --fix}` ← `--config` 파일 ← cwd 의 `.markdownlint-cli2.*`.
pub fn read_base_options(cwd: &Path, argv: &Argv, warn: &mut dyn FnMut(&str)) -> Result<Options> {
    let options_argv = match &argv.config_path {
        Some(Some(path)) => {
            read_options_or_config(&cwd.join(path), argv.config_pointer.as_deref(), warn)?
        }
        _ => Options::default(),
    };
    let fix = Options {
        fix: Some(argv.fix),
        ..Options::default()
    };
    let cwd_options = read_dir_options(cwd, warn)?.unwrap_or_default();
    Ok(merge_options(
        &merge_options(&fix, &options_argv),
        &cwd_options,
    ))
}

/// 원본 `processArgv` + `getBaseOptions` 의 glob 목록: 정규화, `.` 치환,
/// base `globs` 추가(`--no-globs` 제외), base `ignores` 를 `!` 패턴으로 추가.
pub fn resolve_globs(argv: &Argv, base: &Options) -> Vec<String> {
    let mut globs: Vec<String> = argv.globs.iter().map(|g| normalize_glob(g)).collect();
    if globs.len() == 1 && globs[0] == "." {
        globs[0] = DOT_ONLY_SUBSTITUTE.to_string();
    }
    if !argv.no_globs {
        globs.extend(base.globs.iter().flatten().cloned());
    }
    globs.extend(base.ignores.iter().flatten().map(|g| format!("!{g}")));
    globs
}

struct Node {
    parent: Option<PathBuf>,
    files: Vec<PathBuf>,
    config: Option<ConfigValue>,
    options: Option<Options>,
}

/// 원본 `getAndProcessDirInfo`: 없으면 설정 파일을 읽어 생성.
fn ensure_dir(
    nodes: &mut BTreeMap<PathBuf, Node>,
    dir: &Path,
    warn: &mut dyn FnMut(&str),
) -> Result<()> {
    if nodes.contains_key(dir) {
        return Ok(());
    }
    let options = read_dir_options(dir, warn)?;
    let config = read_dir_config(dir)?;
    nodes.insert(
        dir.to_path_buf(),
        Node {
            parent: None,
            files: Vec::new(),
            config,
            options,
        },
    );
    Ok(())
}

/// 원본 `removeIgnoredFiles`: `dir` 기준 상대 경로가 `ignores` 에 매치되면 제외 (micromatch, dot:true).
pub fn remove_ignored_files(dir: &Path, files: Vec<PathBuf>, ignores: &[String]) -> Vec<PathBuf> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in ignores {
        if let Ok(glob) = globset::GlobBuilder::new(pattern)
            .literal_separator(true)
            .backslash_escape(true)
            .build()
        {
            builder.add(glob);
        }
    }
    let Ok(set) = builder.build() else {
        return files;
    };
    files
        .into_iter()
        .filter(|file| {
            file.strip_prefix(dir)
                .map(|rel| !set.is_match(rel))
                .unwrap_or(true)
        })
        .collect()
}

/// 원본 `createDirInfos` (파일 열거 이후 부분). `files` 는 절대 경로.
pub fn create_dir_infos(
    base: &Path,
    files: &[PathBuf],
    base_options: &Options,
    warn: &mut dyn FnMut(&str),
) -> Result<Vec<DirInfo>> {
    let mut nodes: BTreeMap<PathBuf, Node> = BTreeMap::new();

    // getBaseOptions: base 의 옵션은 이미 read_base_options 가 읽어 병합했다
    nodes.insert(
        base.to_path_buf(),
        Node {
            parent: None,
            files: Vec::new(),
            config: read_dir_config(base)?,
            options: Some(base_options.clone()),
        },
    );

    // enumerateFiles
    for file in files {
        let dir = file.parent().unwrap_or(base);
        ensure_dir(&mut nodes, dir, warn)?;
        nodes.get_mut(dir).unwrap().files.push(file.clone());
    }

    // enumerateParents: base 조상에 닿을 때까지 부모를 만들고 연결
    let base_parents: HashSet<PathBuf> = base.ancestors().map(Path::to_path_buf).collect();
    for start in nodes.keys().cloned().collect::<Vec<_>>() {
        let mut last = start.clone();
        let mut dir = start;
        while !base_parents.contains(&dir) {
            let Some(parent) = dir.parent().map(Path::to_path_buf) else {
                break;
            };
            ensure_dir(&mut nodes, &parent, warn)?;
            nodes.get_mut(&last).unwrap().parent = Some(parent.clone());
            last = parent.clone();
            dir = parent;
        }
        // base 아래가 아닌 디렉토리는 base 를 부모로 삼는다
        if dir != base {
            nodes.get_mut(&dir).unwrap().parent = Some(base.to_path_buf());
        }
    }

    // 설정 파일 없는 디렉토리는 부모로 흡수 (긴 경로부터)
    let mut dirs: Vec<PathBuf> = nodes.keys().cloned().collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.as_os_str().len()));
    let mut kept = Vec::new();
    for dir in dirs {
        let absorb_into = match &nodes[&dir] {
            Node {
                parent: Some(parent),
                config: None,
                options: None,
                ..
            } => Some(parent.clone()),
            _ => None,
        };
        match absorb_into {
            Some(parent) => {
                let files = std::mem::take(&mut nodes.get_mut(&dir).unwrap().files);
                nodes.get_mut(&parent).unwrap().files.extend(files);
            }
            None => kept.push(dir),
        }
    }

    // 상속: 옵션은 merge_options, `.markdownlint.*` 는 체인에 config 옵션이 없을 때만 대체
    let mut infos = Vec::with_capacity(kept.len());
    for dir in kept {
        let node = &nodes[&dir];
        let mut config = node.config.clone();
        let mut options = node.options.clone();
        let mut parent = node.parent.as_deref();
        while let Some(p) = parent {
            let pn = &nodes[p];
            if let Some(po) = &pn.options {
                options = Some(merge_options(po, &options.take().unwrap_or_default()));
            }
            if config.is_none()
                && pn.config.is_some()
                && options.as_ref().is_none_or(|o| o.config.is_none())
            {
                config = pn.config.clone();
            }
            parent = pn.parent.as_deref();
        }
        let options = options.unwrap_or_default();
        infos.push(DirInfo {
            effective_config: config.or_else(|| options.config.clone()),
            dir,
            files: node.files.clone(),
            options,
        });
    }
    Ok(infos)
}
