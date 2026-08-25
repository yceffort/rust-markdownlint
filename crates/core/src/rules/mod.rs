use crate::error::ErrorSink;
use crate::parser::TokenTree;

pub mod registry;

pub struct RuleMeta {
    pub names: &'static [&'static str],
    pub description: &'static str,
    pub tags: &'static [&'static str],
    pub needs_tokens: bool,
    pub fixable: bool,
}

pub type RuleParams = serde_json::Map<String, serde_json::Value>;

pub struct LintContext<'a> {
    pub name: &'a str,
    pub lines: &'a [&'a str],
    pub tokens: &'a TokenTree,
    pub front_matter_lines: usize,
    pub config: &'a RuleParams,
}

pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check(&self, ctx: &LintContext, out: &mut ErrorSink);
}
