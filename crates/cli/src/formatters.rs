//! cli2 `outputFormatters` 의 내장 구현. 이름, 옵션, 출력 바이트는 원본 패키지
//! (markdownlint-cli2 v0.22.1 저장소의 `formatter-*/`)와 같다. 원본은 모듈 id 를 import 하지만
//! 여기서는 id 의 마지막 경로 세그먼트가 패키지 이름(`markdownlint-cli2-formatter-json`)이거나
//! 원본 저장소의 디렉토리 이름(`formatter-json`)이면 내장 구현을 쓰고, 아니면 원본처럼
//! `Unable to import module` 오류다.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Result, bail};
use regex::{Captures, Regex};
use rust_markdownlint::config::ConfigValue;
use rust_markdownlint::error::Severity;
use serde_json::{Map, json};
use sha2::{Digest, Sha256};

use crate::output::{LintResult, format_result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formatter {
    Default,
    CodeQuality,
    Json,
    Junit,
    Pretty,
    Sarif,
    Summarize,
    Template,
}

const BUILTIN: &[(&str, Formatter)] = &[
    ("default", Formatter::Default),
    ("codequality", Formatter::CodeQuality),
    ("json", Formatter::Json),
    ("junit", Formatter::Junit),
    ("pretty", Formatter::Pretty),
    ("sarif", Formatter::Sarif),
    ("summarize", Formatter::Summarize),
    ("template", Formatter::Template),
];

pub fn resolve(id: &str) -> Option<Formatter> {
    let name = id
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(id);
    let suffix = name
        .strip_prefix("markdownlint-cli2-formatter-")
        .or_else(|| name.strip_prefix("formatter-"))?;
    BUILTIN.iter().find(|(s, _)| *s == suffix).map(|(_, f)| *f)
}

/// 원본 `importModuleIdsAndParams`: 항목은 `[id, params]`. 실행 전에 전부 import 하므로 모르는
/// id 는 아무 포맷터도 실행하기 전에 오류다.
pub fn resolve_all(entries: &[ConfigValue]) -> Result<Vec<(Formatter, ConfigValue)>> {
    entries
        .iter()
        .map(|entry| {
            let id = entry.as_array().and_then(|a| a.first());
            match id.and_then(ConfigValue::as_str).and_then(resolve) {
                Some(formatter) => {
                    let params = entry.get(1).cloned().unwrap_or(ConfigValue::Null);
                    Ok((formatter, params))
                }
                None => {
                    let shown = match id {
                        Some(ConfigValue::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                        None => entry.to_string(),
                    };
                    bail!("Unable to import module '{shown}'.")
                }
            }
        })
        .collect()
}

/// 원본 `outputResults`: 배열 순서대로 실행. `Promise.all` 이지만 동기 출력은 순서대로이고, pretty 만
/// `await import` 뒤에 출력하므로 나머지가 끝난 뒤에 온다.
pub fn run(
    formatters: &[(Formatter, ConfigValue)],
    base: &Path,
    results: &[LintResult],
) -> Result<()> {
    let (pretty, rest): (Vec<_>, Vec<_>) = formatters
        .iter()
        .partition(|(f, _)| *f == Formatter::Pretty);
    for (formatter, params) in rest.into_iter().chain(pretty) {
        match formatter {
            Formatter::Default => default(results),
            Formatter::CodeQuality => codequality(base, results, params)?,
            Formatter::Json => json_file(base, results, params)?,
            Formatter::Junit => junit(base, results, params)?,
            Formatter::Pretty => pretty_lines(results, params),
            Formatter::Sarif => sarif(base, results, params)?,
            Formatter::Summarize => summarize(results, params),
            Formatter::Template => template(results, params),
        }
    }
    Ok(())
}

fn param_str<'a>(params: &'a ConfigValue, key: &str) -> Option<&'a str> {
    params.get(key).and_then(ConfigValue::as_str)
}

fn param_truthy(params: &ConfigValue, key: &str) -> bool {
    params
        .get(key)
        .is_some_and(rust_markdownlint::config::truthy)
}

fn rule_name(r: &LintResult) -> String {
    r.error.rule_names.join("/")
}

fn column(r: &LintResult) -> usize {
    r.error.error_range.map_or(0, |(start, _)| start)
}

fn severity_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// `path.resolve(directory, name)` 에 쓴다 (절대 경로 name 은 그대로).
fn write_file(base: &Path, name: &str, content: &str) -> Result<()> {
    std::fs::write(base.join(name), content)?;
    Ok(())
}

/// `JSON.stringify(value, null, indent)`.
fn stringify(value: &ConfigValue, indent: &[u8]) -> String {
    let mut out = Vec::new();
    if indent.is_empty() {
        serde_json::to_writer(&mut out, value).unwrap();
    } else {
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent);
        let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);
        serde::Serialize::serialize(value, &mut ser).unwrap();
    }
    String::from_utf8(out).unwrap()
}

fn default(results: &[LintResult]) {
    for result in results {
        eprintln!("{}", format_result(result));
    }
}

/// 원본 `createResults` 항목을 markdownlint `LintError` 의 키 순서 그대로.
fn result_value(r: &LintResult) -> ConfigValue {
    let e = &r.error;
    let fix_info = e.fix_info.as_ref().map(|fix| {
        let mut map = Map::new();
        if let Some(line) = fix.line_number {
            map.insert("lineNumber".into(), json!(line));
        }
        if let Some(column) = fix.edit_column {
            map.insert("editColumn".into(), json!(column));
        }
        if let Some(count) = fix.delete_count {
            map.insert("deleteCount".into(), json!(count));
        }
        if let Some(text) = &fix.insert_text {
            map.insert("insertText".into(), json!(text));
        }
        ConfigValue::Object(map)
    });
    json!({
        "fileName": r.file_name,
        "lineNumber": e.line_number,
        "ruleNames": e.rule_names,
        "ruleDescription": e.rule_description,
        "ruleInformation": (!e.rule_information.is_empty()).then_some(&e.rule_information),
        "errorDetail": e.error_detail,
        "errorContext": e.error_context,
        "errorRange": e.error_range.map(|(start, len)| [start, len]),
        "fixInfo": fix_info,
        "severity": severity_str(e.severity),
    })
}

/// `markdownlint-cli2-formatter-json`: `name`, `spaces` (`spaces || 2`, JSON.stringify 는 10 까지).
fn json_file(base: &Path, results: &[LintResult], params: &ConfigValue) -> Result<()> {
    let indent: Vec<u8> = match params.get("spaces") {
        Some(ConfigValue::Number(n)) => {
            let n = n.as_f64().unwrap_or(0.0);
            if n == 0.0 {
                vec![b' '; 2]
            } else if n < 0.0 {
                Vec::new()
            } else {
                vec![b' '; (n as usize).min(10)]
            }
        }
        Some(ConfigValue::String(s)) if !s.is_empty() => {
            s.chars().take(10).collect::<String>().into_bytes()
        }
        _ => vec![b' '; 2],
    };
    let values: Vec<ConfigValue> = results.iter().map(result_value).collect();
    let content = stringify(&ConfigValue::Array(values), &indent);
    let name = param_str(params, "name").unwrap_or("markdownlint-cli2-results.json");
    write_file(base, name, &content)
}

/// xmlbuilder 의 속성 이스케이프: `&`, `<`, `"` 만.
fn xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

fn xml_cdata(s: &str) -> String {
    format!("<![CDATA[{}]]>", s.replace("]]>", "]]]]><![CDATA[>"))
}

/// `markdownlint-cli2-formatter-junit` (junit-report-builder 출력 그대로): `name`.
fn junit(base: &Path, results: &[LintResult], params: &ConfigValue) -> Result<()> {
    const SUITE: &str = "markdownlint-cli2-formatter-junit";
    let failures: Vec<&LintResult> = results
        .iter()
        .filter(|r| r.error.severity == Severity::Error)
        .collect();
    let (tests, failed) = if results.is_empty() {
        (1, 0)
    } else {
        (failures.len(), failures.len())
    };
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites tests=\"{tests}\" failures=\"{failed}\" errors=\"0\" skipped=\"0\">\n"
    );
    let suite_attrs = format!(
        "name=\"{SUITE}\" time=\"0\" tests=\"{tests}\" failures=\"{failed}\" errors=\"0\" skipped=\"0\""
    );
    if tests == 0 {
        xml.push_str(&format!("  <testsuite {suite_attrs}/>\n"));
    } else {
        xml.push_str(&format!("  <testsuite {suite_attrs}>\n"));
        for r in failures {
            let e = &r.error;
            let column = match column(r) {
                0 => String::new(),
                c => format!(", Column {c}"),
            };
            let detail = e
                .error_detail
                .as_deref()
                .map(|d| format!(", {d}"))
                .unwrap_or_default();
            let context = e
                .error_context
                .as_deref()
                .map(|c| format!(", Context: \"{c}\""))
                .unwrap_or_default();
            let text = format!("Line {}{column}{detail}{context}", e.line_number);
            xml.push_str(&format!(
                "    <testcase classname=\"{}\" name=\"{}\" time=\"0\">\n      <failure message=\"{}\">{}</failure>\n    </testcase>\n",
                xml_attr(&r.file_name),
                xml_attr(&rule_name(r)),
                xml_attr(e.rule_description),
                xml_cdata(&text)
            ));
        }
        if results.is_empty() {
            xml.push_str(&format!("    <testcase name=\"{SUITE}\" time=\"0\"/>\n"));
        }
        xml.push_str("  </testsuite>\n");
    }
    xml.push_str("</testsuites>");
    let name = param_str(params, "name").unwrap_or("markdownlint-cli2-junit.xml");
    write_file(base, name, &xml)
}

/// SARIF2012 규칙 이름: `ruleNames.join(" ")` 를 소문자로, 단어 첫 글자만 대문자로, 영숫자만 남긴다.
/// (`\w` 와 `\b` 는 ASCII 기준.)
fn sarif_rule_name(names: &[&str]) -> String {
    let joined = names.join(" ");
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = String::with_capacity(joined.len());
    let mut prev_word = false;
    for c in joined.chars() {
        let word = is_word(c);
        if c.is_ascii_alphanumeric() {
            if word && !prev_word {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c.to_ascii_lowercase());
            }
        }
        prev_word = word;
    }
    out
}

/// `markdownlint-cli2-formatter-sarif` 0.0.4: `name`.
fn sarif(base: &Path, results: &[LintResult], params: &ConfigValue) -> Result<()> {
    let mut rules: Vec<ConfigValue> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut sarif_results: Vec<ConfigValue> = Vec::new();
    for r in results {
        let e = &r.error;
        let rule_id = e.rule_names[0];
        let detail = e
            .error_detail
            .as_deref()
            .map(|d| format!(", {d}"))
            .unwrap_or_default();
        let context = e
            .error_context
            .as_deref()
            .map(|c| format!(", Context: \"{c}\""))
            .unwrap_or_default();
        if !seen.contains(&rule_id) {
            seen.push(rule_id);
            let mut rule = json!({
                "id": rule_id,
                "name": sarif_rule_name(e.rule_names),
                "shortDescription": { "text": e.rule_description },
                "fullDescription": { "text": e.rule_description },
            });
            if !e.rule_information.is_empty() {
                rule["helpUri"] = json!(e.rule_information);
            }
            rules.push(rule);
        }
        let mut region = json!({ "startLine": e.line_number, "endLine": e.line_number });
        if let Some((start, len)) = e.error_range {
            region["startColumn"] = json!(start);
            region["endColumn"] = json!(start + len);
        }
        sarif_results.push(json!({
            "ruleId": rule_id,
            "message": { "text": format!("{}{detail}{context}", e.rule_description) },
            "locations": [{
                "physicalLocation": {
                    "artifactLocation": { "uri": r.file_name },
                    "region": region,
                }
            }],
            "level": severity_str(e.severity),
        }));
    }
    let sarif = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "markdownlint-cli2",
                    "version": "0.0.4",
                    "informationUri": "https://github.com/DavidAnson/markdownlint-cli2",
                    "rules": rules,
                }
            },
            "results": sarif_results,
        }],
    });
    let name = param_str(params, "name").unwrap_or("markdownlint-cli2-sarif.sarif");
    write_file(base, name, &stringify(&sarif, b"  "))
}

/// `markdownlint-cli2-formatter-codequality` (GitLab Code Quality): `name`, `severity`,
/// `severityError`, `severityWarning`.
fn codequality(base: &Path, results: &[LintResult], params: &ConfigValue) -> Result<()> {
    let issues: Vec<ConfigValue> = results
        .iter()
        .map(|r| {
            let e = &r.error;
            let rule = rule_name(r);
            let detail = e
                .error_detail
                .as_deref()
                .map(|d| format!(" [{d}]"))
                .unwrap_or_default();
            let context = e
                .error_context
                .as_deref()
                .map(|c| format!(" [Context: \"{c}\"]"))
                .unwrap_or_default();
            let column = match column(r) {
                0 => String::new(),
                c => format!(":{c}"),
            };
            let error_text = format!(
                "{}:{}{column} {rule} {}{detail}{context}",
                r.file_name, e.line_number, e.rule_description
            );
            let by_severity = match e.severity {
                Severity::Warning => param_str(params, "severityWarning"),
                Severity::Error => param_str(params, "severityError"),
            };
            let severity = by_severity
                .or_else(|| param_str(params, "severity"))
                .unwrap_or("minor");
            let fingerprint = format!("{:x}", Sha256::digest(error_text.as_bytes()));
            json!({
                "type": "issue",
                "check_name": rule,
                "description": format!("{rule}: {}{detail}", e.rule_description),
                "severity": severity,
                "fingerprint": fingerprint,
                "location": { "path": r.file_name, "lines": { "begin": e.line_number } },
            })
        })
        .collect();
    let name = param_str(params, "name").unwrap_or("markdownlint-cli2-codequality.json");
    write_file(base, name, &stringify(&ConfigValue::Array(issues), b"  "))
}

/// JS `Array.prototype.sort` 기본 순서 (UTF-16 코드 유닛).
fn js_sorted<'a>(keys: impl Iterator<Item = &'a String>) -> Vec<&'a String> {
    let mut keys: Vec<&String> = keys.collect();
    keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    keys
}

fn log_columns(count: impl ToString, name: &str, indent: usize) {
    println!("{:indent$}{:>5} {name}", "", count.to_string());
}

/// `markdownlint-cli2-formatter-summarize`: `byFile`, `byRule`, `byFileByRule`, `byRuleByFile`.
fn summarize(results: &[LintResult], params: &ConfigValue) {
    let mut by_file: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_rule: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_file_by_rule: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut by_rule_by_file: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for r in results {
        let rule = rule_name(r);
        *by_file.entry(r.file_name.clone()).or_default() += 1;
        *by_rule.entry(rule.clone()).or_default() += 1;
        *by_file_by_rule
            .entry(r.file_name.clone())
            .or_default()
            .entry(rule.clone())
            .or_default() += 1;
        *by_rule_by_file
            .entry(rule)
            .or_default()
            .entry(r.file_name.clone())
            .or_default() += 1;
    }
    if param_truthy(params, "byFile") {
        log_columns("Count", "File", 0);
        for file in js_sorted(by_file.keys()) {
            log_columns(by_file[file], file, 0);
        }
        log_columns(results.len(), "[Total]", 0);
    }
    if param_truthy(params, "byRule") {
        log_columns("Count", "Rule", 0);
        for rule in js_sorted(by_rule.keys()) {
            log_columns(by_rule[rule], rule, 0);
        }
        log_columns(results.len(), "[Total]", 0);
    }
    if param_truthy(params, "byFileByRule") {
        for file in js_sorted(by_file_by_rule.keys()) {
            println!("{file}");
            log_columns("Count", "Rule", 2);
            let rules = &by_file_by_rule[file];
            for rule in js_sorted(rules.keys()) {
                log_columns(rules[rule], rule, 2);
            }
            log_columns(by_file[file], "[Total]", 2);
        }
    }
    if param_truthy(params, "byRuleByFile") {
        for rule in js_sorted(by_rule_by_file.keys()) {
            println!("{rule}");
            log_columns("Count", "File", 2);
            let files = &by_rule_by_file[rule];
            for file in js_sorted(files.keys()) {
                log_columns(files[file], file, 2);
            }
            log_columns(by_rule[rule], "[Total]", 2);
        }
    }
}

/// Node `util.styleText` 의 색상 사용 판정 (기본 stream 은 stdout): `FORCE_COLOR`, `NO_COLOR`,
/// `NODE_DISABLE_COLORS`, TTY 여부.
fn colors_enabled() -> bool {
    let env = |k: &str| std::env::var(k).ok();
    if let Some(force) = env("FORCE_COLOR") {
        return !(force == "0" || force.eq_ignore_ascii_case("false"));
    }
    env("NO_COLOR").is_none()
        && env("NODE_DISABLE_COLORS").is_none()
        && std::io::stdout().is_terminal()
        && env("TERM").as_deref() != Some("dumb")
}

/// supports-hyperlinks (stderr): `FORCE_HYPERLINK`, 아니면 TTY 이면서 하이퍼링크를 지원하는 터미널.
fn hyperlinks_enabled() -> bool {
    let env = |k: &str| std::env::var(k).ok();
    if let Some(force) = env("FORCE_HYPERLINK") {
        return force.is_empty() || force.trim().parse::<i64>() != Ok(0);
    }
    if !std::io::stderr().is_terminal() || env("CI").is_some() {
        return false;
    }
    matches!(
        env("TERM_PROGRAM").as_deref(),
        Some("Hyper" | "iTerm.app" | "WezTerm" | "vscode" | "ghostty")
    ) || env("VTE_VERSION").is_some()
        || env("TERM").as_deref() == Some("alacritty")
}

/// `markdownlint-cli2-formatter-pretty`: `appendLink`. 색은 `util.styleText`, 링크는 terminal-link.
fn pretty_lines(results: &[LintResult], params: &ConfigValue) {
    let colors = colors_enabled();
    let links = hyperlinks_enabled();
    let style = |code: u8, text: &str| -> String {
        if colors {
            format!("\x1b[{code}m{text}\x1b[39m")
        } else {
            text.to_string()
        }
    };
    let append_link = param_truthy(params, "appendLink");
    for r in results {
        let e = &r.error;
        let rule = rule_name(r);
        let rule_text = if links && !e.rule_information.is_empty() {
            format!("\x1b]8;;{}\x07{rule}\x1b]8;;\x07", e.rule_information)
        } else {
            rule.clone()
        };
        let details = format!(
            "{}{}",
            e.error_detail
                .as_deref()
                .map(|d| format!(" [{d}]"))
                .unwrap_or_default(),
            e.error_context
                .as_deref()
                .map(|c| format!(" [Context: \"{c}\"]"))
                .unwrap_or_default()
        );
        let append = if append_link && !e.rule_information.is_empty() {
            style(94, &format!(" {}", e.rule_information))
        } else {
            String::new()
        };
        let column = match column(r) {
            0 => String::new(),
            c => format!("{}{}", style(36, ":"), style(32, &c.to_string())),
        };
        eprintln!(
            "{}{}{}{column} {} {} {}{}{append}",
            style(35, &r.file_name),
            style(36, ":"),
            style(32, &e.line_number.to_string()),
            style(90, severity_str(e.severity)),
            style(33, &rule_text),
            e.rule_description,
            style(33, &details)
        );
    }
}

const DEFAULT_TEMPLATE: &str = "fileName=\"${fileName}\" lineNumber=${lineNumber} ${columnNumber:columnNumber=${columnNumber} }ruleName=${ruleName} ruleDescription=\"${ruleDescription}\" ruleInformation=${ruleInformation} errorContext=\"${errorContext}\" errorDetail=\"${errorDetail}\" errorSeverity=\"${errorSeverity}\"";

const TEMPLATE_TOKENS: [&str; 9] = [
    "fileName",
    "lineNumber",
    "columnNumber",
    "ruleName",
    "ruleDescription",
    "ruleInformation",
    "errorContext",
    "errorDetail",
    "errorSeverity",
];

/// `markdownlint-cli2-formatter-template`: `template`. 토큰마다 별도 정규식을 두 번씩 적용한다
/// (원본과 같음). `columnNumber` 만 undefined 가 될 수 있고 null 값은 빈 문자열로 치환된다.
fn template(results: &[LintResult], params: &ConfigValue) {
    let template = param_str(params, "template")
        .filter(|t| !t.is_empty())
        .unwrap_or(DEFAULT_TEMPLATE);
    let regexes: Vec<Regex> = TEMPLATE_TOKENS
        .iter()
        .map(|token| {
            Regex::new(&format!(
                r"\$\{{({token})(?:([:!])([^{{}}]*\{{[^{{}}]+\}}[^{{}}]*|[^}}]+))?\}}"
            ))
            .unwrap()
        })
        .collect();
    for r in results {
        let e = &r.error;
        // None 은 JS undefined
        let value = |token: &str| -> Option<String> {
            Some(match token {
                "fileName" => r.file_name.clone(),
                "lineNumber" => e.line_number.to_string(),
                "columnNumber" => e.error_range?.0.to_string(),
                "ruleName" => rule_name(r),
                "ruleDescription" => e.rule_description.to_string(),
                "ruleInformation" => e.rule_information.clone(),
                "errorContext" => e.error_context.clone().unwrap_or_default(),
                "errorDetail" => e.error_detail.clone().unwrap_or_default(),
                "errorSeverity" => severity_str(e.severity).to_string(),
                _ => unreachable!(),
            })
        };
        let replacer = |caps: &Captures| -> String {
            let value = value(&caps[1]);
            let text = caps.get(3).map_or("", |m| m.as_str());
            match caps.get(2).map(|m| m.as_str()) {
                Some(":") => {
                    if value.is_none() {
                        String::new()
                    } else {
                        text.to_string()
                    }
                }
                Some("!") => {
                    if value.is_none() {
                        text.to_string()
                    } else {
                        String::new()
                    }
                }
                _ => value.unwrap_or_default(),
            }
        };
        let mut output = template.to_string();
        for re in &regexes {
            for _ in 0..2 {
                output = re.replace_all(&output, &replacer).into_owned();
            }
        }
        eprintln!("{output}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_package_and_directory_names() {
        assert_eq!(
            resolve("markdownlint-cli2-formatter-json"),
            Some(Formatter::Json)
        );
        assert_eq!(resolve("../../formatter-sarif"), Some(Formatter::Sarif));
        assert_eq!(resolve("./formatter-default/"), Some(Formatter::Default));
        assert_eq!(resolve("markdownlint-cli2-formatter-nope"), None);
        assert_eq!(resolve("./custom-output-formatter.cjs"), None);
        assert_eq!(resolve("missing-package"), None);
    }

    /// node 오라클: `["MD001", "x-y_z", "weird.name"]` → `Md001XYzWeirdName`.
    #[test]
    fn sarif_rule_name_like_original() {
        assert_eq!(
            sarif_rule_name(&["MD001", "x-y_z", "weird.name"]),
            "Md001XYzWeirdName"
        );
        assert_eq!(
            sarif_rule_name(&["MD025", "single-title", "single-h1"]),
            "Md025SingleTitleSingleH1"
        );
    }

    #[test]
    fn xml_escapes_like_xmlbuilder() {
        assert_eq!(xml_attr("a<b>&\"c'"), "a&lt;b>&amp;&quot;c'");
        assert_eq!(xml_cdata("x ]]> y"), "<![CDATA[x ]]]]><![CDATA[> y]]>");
    }
}
