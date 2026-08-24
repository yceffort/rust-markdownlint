//! 수동 대조용: DUMP_IN=<md 파일> DUMP_OUT=<json> cargo test --test dump_tokens
#[test]
fn dump() {
    let (Ok(inp), Ok(out)) = (std::env::var("DUMP_IN"), std::env::var("DUMP_OUT")) else { return };
    let tree = rust_markdownlint::parser::parse(&std::fs::read_to_string(inp).unwrap());
    std::fs::write(out, tree.to_json().to_string()).unwrap();
}
