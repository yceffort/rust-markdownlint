fn main() {
    println!("{}", rust_markdownlint::parse_events("# hi\n").len());
}
