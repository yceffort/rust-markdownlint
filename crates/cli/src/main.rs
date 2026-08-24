fn main() {
    println!("{}", rust_markdownlint::parser::parse("# hi\n").tokens.len());
}
