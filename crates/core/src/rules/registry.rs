use std::collections::HashMap;
use std::sync::LazyLock;

use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;

// check 본문은 Task 4(md018.rs, md047.rs 포팅)에서 채운다.
pub(crate) struct Md018;
pub(crate) struct Md047;

static MD018_META: RuleMeta = RuleMeta {
    names: &["MD018", "no-missing-space-atx"],
    description: "No space after hash on atx style heading",
    tags: &["headings", "atx", "spaces"],
    needs_tokens: true,
    fixable: true,
};

static MD047_META: RuleMeta = RuleMeta {
    names: &["MD047", "single-trailing-newline"],
    description: "Files should end with a single newline character",
    tags: &["blank_lines"],
    needs_tokens: false,
    fixable: true,
};

impl Rule for Md018 {
    fn meta(&self) -> &'static RuleMeta {
        &MD018_META
    }

    fn check(&self, _ctx: &LintContext, _out: &mut ErrorSink) {
        todo!("Task 4: port lib/md018.mjs")
    }
}

impl Rule for Md047 {
    fn meta(&self) -> &'static RuleMeta {
        &MD047_META
    }

    fn check(&self, _ctx: &LintContext, _out: &mut ErrorSink) {
        todo!("Task 4: port lib/md047.mjs")
    }
}

static RULES: [&dyn Rule; 2] = [&Md018, &Md047];

pub fn all_rules() -> &'static [&'static dyn Rule] {
    &RULES
}

static ALIASES: LazyLock<HashMap<String, Vec<&'static str>>> = LazyLock::new(|| {
    let mut map: HashMap<String, Vec<&'static str>> = HashMap::new();
    for rule in all_rules() {
        let meta = rule.meta();
        let primary = meta.names[0];
        for name in meta.names {
            map.entry(name.to_uppercase()).or_default().push(primary);
        }
        for tag in meta.tags {
            map.entry(tag.to_uppercase()).or_default().push(primary);
        }
    }
    map
});

/// 규칙 이름, alias, 태그를 대소문자 무시로 기본 이름 목록으로 해석한다.
pub fn resolve_alias(name: &str) -> Vec<&'static str> {
    ALIASES
        .get(&name.to_uppercase())
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_alias_cases() {
        assert_eq!(resolve_alias("md047"), vec!["MD047"]);
        assert_eq!(resolve_alias("single-trailing-newline"), vec!["MD047"]);
        assert!(resolve_alias("HEADINGS").contains(&"MD018"));
        assert!(resolve_alias("nope").is_empty());
    }
}
