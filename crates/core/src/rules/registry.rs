use std::collections::HashMap;
use std::sync::LazyLock;

use super::Rule;
use super::md001::Md001;
use super::md003::Md003;
use super::md004::Md004;
use super::md005::Md005;
use super::md007::Md007;
use super::md009::Md009;
use super::md010::Md010;
use super::md012::Md012;
use super::md013::Md013;
use super::md014::Md014;
use super::md018::Md018;
use super::md019::Md019;
use super::md020::Md020;
use super::md021::Md021;
use super::md022::Md022;
use super::md023::Md023;
use super::md024::Md024;
use super::md025::Md025;
use super::md026::Md026;
use super::md027::Md027;
use super::md028::Md028;
use super::md029::Md029;
use super::md030::Md030;
use super::md031::Md031;
use super::md033::Md033;
use super::md034::Md034;
use super::md035::Md035;
use super::md036::Md036;
use super::md040::Md040;
use super::md041::Md041;
use super::md043::Md043;
use super::md046::Md046;
use super::md047::Md047;

static RULES: [&dyn Rule; 33] = [
    &Md001, &Md003, &Md004, &Md005, &Md007, &Md009, &Md010, &Md012, &Md013, &Md014, &Md018, &Md019,
    &Md020, &Md021, &Md022, &Md023, &Md024, &Md025, &Md026, &Md027, &Md028, &Md029, &Md030, &Md031,
    &Md033, &Md034, &Md035, &Md036, &Md040, &Md041, &Md043, &Md046, &Md047,
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
