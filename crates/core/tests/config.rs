use std::path::Path;

use rust_markdownlint::config::{
    effective_config, merge_options, options_from_value, read_config_file,
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

#[test]
fn plugin_keys_warn() {
    let mut w = vec![];
    options_from_value(json!({"customRules": ["x"]}), &mut |s| {
        w.push(s.to_string())
    });
    assert_eq!(w.len(), 1);
}
