use std::path::Path;

use rust_markdownlint::config::{
    Format, effective_config, merge_options, options_from_value, parse_config_as, read_config_file,
};
use rust_markdownlint::error::Severity;
use serde_json::json;

#[test]
fn default_false_then_enable_one() {
    let c = json!({"default": false, "MD047": true});
    let e = effective_config(&c);
    assert!(e.enabled("MD047"));
    assert!(!e.enabled("MD018"));
}

#[test]
fn tag_then_rule_order_matters() {
    let c = json!({"headings": false, "md018": true});
    assert!(effective_config(&c).enabled("MD018"));
}

#[test]
fn warning_severity_and_params() {
    // 계획 문서는 MD013 을 쓰지만 M0 에는 미등록 규칙이라 MD018 로 대체
    let c = json!({"MD018": {"line_length": 100, "severity": "warning"}});
    let (en, sev, p) = effective_config(&c).get("MD018");
    assert!(en);
    assert_eq!(sev, Severity::Warning);
    assert_eq!(p["line_length"], 100);
    assert!(p.get("severity").is_none());
}

#[test]
fn default_warning_severity() {
    let c = json!({"default": "warning"});
    let (en, sev, _) = effective_config(&c).get("MD047");
    assert!(en);
    assert_eq!(sev, Severity::Warning);
}

#[test]
fn extends_relative_and_shallow() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/config/child.yaml"
    );
    let c = read_config_file(Path::new(path)).unwrap();
    assert_eq!(c["default"], json!(false));
    assert_eq!(c["MD018"], json!({"b": 3}));
    assert_eq!(c["MD047"], json!(false));
    assert!(c.get("extends").is_none());
}

#[test]
fn merge_options_config_by_key() {
    let a = options_from_value(
        json!({"config": {"MD001": false, "MD002": false}, "fix": true}),
        &mut |_| {},
    );
    let b = options_from_value(json!({"config": {"MD002": true}}), &mut |_| {});
    let m = merge_options(&a, &b);
    assert_eq!(m.config.unwrap(), json!({"MD001": false, "MD002": true}));
    assert_eq!(m.fix, Some(true));
}

/// js-yaml 4.1.1 (`bench/node_modules/js-yaml`) 로 뽑은 기대값. flow collection 안의 여러 줄 plain scalar 는
/// js-yaml 이 받아들이는 경우와 `missed comma between flow collection entries` 로 거부하는 경우가 갈린다 (#176).
#[test]
fn yaml_flow_multiline_plain_scalar_accepted_like_js_yaml() {
    let accepted = [
        ("{ a: foo\n  bar }", json!({"a": "foo bar"})),
        ("[ foo\n  bar ]", json!(["foo bar"])),
        ("{ foo\n  bar }", json!({"foo bar": null})),
        ("{ ? foo\n  bar : 1 }", json!({"foo bar": 1})),
        (
            "key:\n  { a: foo\n    bar }",
            json!({"key": {"a": "foo bar"}}),
        ),
        ("key:\n  { a: foo\n bar }", json!({"key": {"a": "foo bar"}})),
        ("key:\n  [ foo\n, bar ]", json!({"key": ["foo", "bar"]})),
        ("- { a: foo\n bar }", json!([{"a": "foo bar"}])),
        (
            "- key: { a: foo\n   bar }",
            json!([{"key": {"a": "foo bar"}}]),
        ),
        ("{ a: 1,\n  b: 2 }", json!({"a": 1, "b": 2})),
        ("{ a: 1\n, b: 2 }", json!({"a": 1, "b": 2})),
        ("{ a: foo\n  bar, b: 1 }", json!({"a": "foo bar", "b": 1})),
        ("{ a: foo\n\n  bar }", json!({"a": "foo\nbar"})),
        ("{ a: [x,\n  z] }", json!({"a": ["x", "z"]})),
        ("{ a: \"x\n  y\" }", json!({"a": "x y"})),
        ("[ a: 1,\n  b: 2 ]", json!([{"a": 1}, {"b": 2}])),
        ("k: { a:\n  1 }", json!({"k": {"a": 1}})),
        ("{ a:\n  b }", json!({"a": "b"})),
        ("--- { a: foo\nbar }", json!({"a": "foo bar"})),
        ("!!map\n  { a: foo\nbar }", json!({"a": "foo bar"})),
        ("a: foo\n  bar", json!({"a": "foo bar"})),
        (
            "default: true\nMD013:\n  line_length: 120\n",
            json!({"default": true, "MD013": {"line_length": 120}}),
        ),
    ];
    for (src, expected) in accepted {
        let value = parse_config_as(Format::Yaml, src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(value, expected, "{src:?}");
    }
}

#[test]
fn yaml_flow_multiline_plain_scalar_rejected_like_js_yaml() {
    let rejected = [
        ("{\n  // Comment\n  \"config\": 1\n}", "3:11"),
        (
            "{\n  \"config\": {\n    // Comment\n    \"default\": false\n  }\n}\n",
            "4:14",
        ),
        ("{ foo\n  bar: 1 }", "2:6"),
        ("{ foo\n  : 1 }", "2:3"),
        ("{ \"foo\n  bar\": 1 }", "2:7"),
        ("{ !!str foo\n  : 1 }", "2:3"),
        ("{ &x foo\n  : 1 }", "2:3"),
        ("{ foo # x: y\n  : 1 }", "2:3"),
        ("key:\n  { a: foo\nbar }", "3:1"),
        ("key:\n  { foo\n  bar: 1 }", "3:6"),
        ("key:\n  { foo\nbar: 1 }", "3:1"),
        ("- key: { a: foo\n bar }", "2:2"),
        ("  { a: foo\nbar }", "2:1"),
        ("# c\n  { a: foo\nbar }", "3:1"),
    ];
    for (src, position) in rejected {
        let error = parse_config_as(Format::Yaml, src).unwrap_err().to_string();
        assert_eq!(
            error,
            format!("missed comma between flow collection entries ({position})"),
            "{src:?}"
        );
    }
}

#[test]
fn plugin_keys_warn() {
    let mut w = vec![];
    options_from_value(json!({"customRules": ["x"]}), &mut |s| {
        w.push(s.to_string())
    });
    assert_eq!(w.len(), 1);
}
