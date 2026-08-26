use std::collections::HashMap;
use std::sync::LazyLock;

use super::Rule;
use super::md004::Md004;
use super::md005::Md005;
use super::md007::Md007;
use super::md009::Md009;
use super::md010::Md010;
use super::md012::Md012;
use super::md018::Md018;
use super::md020::Md020;
use super::md022::Md022;
use super::md023::Md023;
use super::md047::Md047;

static RULES: [&dyn Rule; 11] = [
    &Md004, &Md005, &Md007, &Md009, &Md010, &Md012, &Md018, &Md020, &Md022, &Md023, &Md047,
];

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
