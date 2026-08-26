// main 흐름(Task 11)에서 연결되기 전까지 미사용
#[allow(dead_code)]
mod argv;
#[allow(dead_code)]
mod globs;

fn main() {
    println!(
        "{}",
        rust_markdownlint::parser::parse("# hi\n").tokens.len()
    );
}
