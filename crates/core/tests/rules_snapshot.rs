//! 원본 markdownlint@0.40.0 의 `test/*.md` 를 규칙 하나씩 켜서 lint 하고
//! `fixtures/expected/<MD0XX>.json` (scripts/dump-expected.mjs 로 생성) 과 대조한다.
//!
//! 규칙 하나만 확인: `cargo test -p rust-markdownlint --test rules_snapshot -- MD047`

use std::collections::BTreeMap;
use std::fs;

use rust_markdownlint::error::LintError;
use rust_markdownlint::lint::{LintOptions, lint_content};
use rust_markdownlint::rules::registry;
use serde_json::{Value, json};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn project(e: &LintError) -> Value {
    let fix_info = e.fix_info.as_ref().map(|f| {
        let mut obj = serde_json::Map::new();
        if let Some(n) = f.line_number {
            obj.insert("lineNumber".into(), n.into());
        }
        if let Some(c) = f.edit_column {
            obj.insert("editColumn".into(), c.into());
        }
        if let Some(d) = f.delete_count {
            obj.insert("deleteCount".into(), d.into());
        }
        if let Some(t) = &f.insert_text {
            obj.insert("insertText".into(), t.as_str().into());
        }
        Value::Object(obj)
    });
    json!({
        "lineNumber": e.line_number,
        "ruleNames": e.rule_names,
        "errorDetail": e.error_detail,
        "errorContext": e.error_context,
        "errorRange": e.error_range.map(|(c, l)| [c, l]),
        "fixInfo": fix_info,
    })
}

fn check(id: &str) {
    let expected: BTreeMap<String, Value> = serde_json::from_str(
        &fs::read_to_string(format!("{FIXTURES}/expected/{id}.json")).unwrap(),
    )
    .unwrap();
    let config = json!({ "default": false, id: true });
    let opts = LintOptions {
        config: Some(&config),
        ..Default::default()
    };

    let mut names: Vec<String> = fs::read_dir(format!("{FIXTURES}/markdownlint"))
        .unwrap()
        .map(|d| d.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".md"))
        .collect();
    names.sort();
    for name in expected.keys() {
        assert!(
            names.contains(name),
            "{id}: expected file {name} is missing"
        );
    }

    let mut failures = Vec::new();
    for name in &names {
        let content = fs::read_to_string(format!("{FIXTURES}/markdownlint/{name}")).unwrap();
        let got: Vec<Value> = lint_content(name, &content, &opts)
            .unwrap()
            .iter()
            .filter(|e| e.rule_names[0] == id)
            .map(project)
            .collect();
        let exp = expected.get(name).cloned().unwrap_or_else(|| json!([]));
        if Value::Array(got.clone()) != exp {
            failures.push(format!(
                "--- {name}\nexpected: {}\ngot:      {}",
                serde_json::to_string(&exp).unwrap(),
                serde_json::to_string(&got).unwrap()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{id}: {} of {} files differ\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}

macro_rules! rule_tests {
    ($($id:ident),* $(,)?) => {
        const RULES: &[&str] = &[$(stringify!($id)),*];
        $(
            #[test]
            #[allow(non_snake_case)]
            fn $id() {
                check(stringify!($id));
            }
        )*
    };
}

rule_tests!(
    MD004, MD005, MD007, MD009, MD010, MD012, MD018, MD020, MD023, MD047
);

#[test]
fn every_registered_rule_has_a_test() {
    let mut ids: Vec<&str> = registry::all_rules()
        .iter()
        .map(|r| r.meta().names[0])
        .collect();
    ids.sort_unstable();
    let mut rules = RULES.to_vec();
    rules.sort_unstable();
    assert_eq!(
        ids, rules,
        "add the rule to rule_tests! in rules_snapshot.rs"
    );
}
