from pathlib import Path
import sys
root=Path(sys.argv[1])
def edit(path,old,new):
 p=root/path;s=p.read_text()
 if s.count(old)!=1:raise RuntimeError(f'{path}: expected exactly one match ({s.count(old)}): {old[:90]}')
 p.write_text(s.replace(old,new))
(root/'crates/core/src/perf_probe.rs').write_text(r'''
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
const COUNT: usize = 96;
static GLOBAL: [AtomicU64; COUNT] = [const { AtomicU64::new(0) }; COUNT];
thread_local! { static LOCAL: RefCell<[u64; COUNT]> = const { RefCell::new([0; COUNT]) }; }
pub fn add(index: usize, start: Instant) {
    let elapsed = start.elapsed().as_nanos() as u64;
    LOCAL.with(|v| v.borrow_mut()[index] += elapsed);
}
pub fn add_global(index: usize, start: Instant) {
    GLOBAL[index].fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
}
pub struct Flush;
impl Drop for Flush {
    fn drop(&mut self) {
        let values = LOCAL.with(|v| std::mem::replace(&mut *v.borrow_mut(), [0; COUNT]));
        for (index, value) in values.into_iter().enumerate() {
            if value != 0 { GLOBAL[index].fetch_add(value, Ordering::Relaxed); }
        }
    }
}
pub struct Scope { index: usize, start: Instant }
impl Scope {
    pub fn new(index: usize) -> Self { Self { index, start: Instant::now() } }
}
impl Drop for Scope { fn drop(&mut self) { add(self.index, self.start); } }
pub fn snapshot() -> serde_json::Value {
    let labels = ["core_total", "preprocessing", "parse_tree", "parser_engine", "clear_split",
        "rules_inclusive", "error_copy", "core_sort", "file_read", "cli_lint_jobs", "cli_collect",
        "cli_sort", "cli_output", "cli_setup", "cli_total", "rule_setup", "rule_checks", "unused17", "unused18", "unused19"];
    let mut map = serde_json::Map::new();
    for (i, counter) in GLOBAL.iter().enumerate() {
        let ns = counter.load(Ordering::Relaxed);
        if ns == 0 { continue; }
        let label = if i < labels.len() { labels[i].to_string() } else { format!("rule_MD{:03}", i - 20) };
        map.insert(label, serde_json::json!(ns as f64 / 1_000_000.0));
    }
    serde_json::Value::Object(map)
}
''')
edit('crates/core/src/lib.rs','pub mod config;','pub mod perf_probe;\npub mod config;')
edit('crates/core/src/lint.rs','    let content = content.strip_prefix(\'\\u{FEFF}\').unwrap_or(content);','''    let _perf_flush = crate::perf_probe::Flush;
    let _perf_total = crate::perf_probe::Scope::new(0);
    let perf_prep = std::time::Instant::now();
    let content = content.strip_prefix('\\u{FEFF}').unwrap_or(content);''')
edit('crates/core/src/lint.rs','    let need_tokens = enabled_rules.iter().any(|rule| rule.meta().needs_tokens);','''    crate::perf_probe::add(1, perf_prep);
    let perf_parse = std::time::Instant::now();
    let need_tokens = enabled_rules.iter().any(|rule| rule.meta().needs_tokens);''')
edit('crates/core/src/lint.rs','    let cleared = clear_html_comment_text(content);','''    crate::perf_probe::add(2, perf_parse);
    let perf_clear = std::time::Instant::now();
    let cleared = clear_html_comment_text(content);''')
edit('crates/core/src/lint.rs','    let mut results = Vec::new();\n    for rule in enabled_rules {','''    crate::perf_probe::add(4, perf_clear);
    let perf_rules = std::time::Instant::now();
    let mut results = Vec::new();
    for rule in enabled_rules {
        let perf_setup = std::time::Instant::now();''')
edit('crates/core/src/lint.rs','        rule.check(&ctx, &mut sink);','''        crate::perf_probe::add(15, perf_setup);
        let perf_check = std::time::Instant::now();
        rule.check(&ctx, &mut sink);
        crate::perf_probe::add(16, perf_check);
        crate::perf_probe::add(20 + rule_name[2..].parse::<usize>().unwrap(), perf_check);
        let perf_copy = std::time::Instant::now();''')
edit('crates/core/src/lint.rs','''                .cloned(),
        );
    }

    results.sort_by''','''                .cloned(),
        );
        crate::perf_probe::add(6, perf_copy);
    }
    crate::perf_probe::add(5, perf_rules);
    let perf_sort = std::time::Instant::now();
    results.sort_by''')
edit('crates/core/src/lint.rs','    Ok(results)\n}', '    crate::perf_probe::add(7, perf_sort);\n    Ok(results)\n}')
edit('crates/core/src/parser/build.rs','    let (mut events, _) = markdown::parser::parse(parse_text, &opts).expect("markdown-rs parse");','''    let perf_engine = std::time::Instant::now();
    let (mut events, _) = markdown::parser::parse(parse_text, &opts).expect("markdown-rs parse");
    crate::perf_probe::add(3, perf_engine);''')
edit('crates/cli/src/main.rs','    let args: Vec<String> = std::env::args().skip(1).collect();','    let perf_total = std::time::Instant::now();\n    let args: Vec<String> = std::env::args().skip(1).collect();')
edit('crates/cli/src/main.rs','    std::process::exit(code);','''    rust_markdownlint::perf_probe::add_global(14, perf_total);
    if let Ok(path) = std::env::var("MARKDOWNLINT_PERF_OUT") {
        std::fs::write(path, rust_markdownlint::perf_probe::snapshot().to_string()).unwrap();
    } else {
        eprintln!("PERF_JSON {}", rust_markdownlint::perf_probe::snapshot());
    }
    std::process::exit(code);''')
edit('crates/cli/src/main.rs','        let content = lossy_utf8(std::fs::read(file)?);','''        let perf_read = std::time::Instant::now();
        let content = lossy_utf8(std::fs::read(file)?);
        rust_markdownlint::perf_probe::add_global(8, perf_read);''')
edit('crates/cli/src/main.rs','    let base = std::env::current_dir()?;','    let perf_setup = std::time::Instant::now();\n    let base = std::env::current_dir()?;')
edit('crates/cli/src/main.rs','    let outcomes: Vec<Result<FileOutcome>> = jobs','''    rust_markdownlint::perf_probe::add_global(13, perf_setup);
    let perf_jobs = std::time::Instant::now();
    let outcomes: Vec<Result<FileOutcome>> = jobs''')
edit('crates/cli/src/main.rs','    let mut results = Vec::new();\n    let mut errors_present','''    rust_markdownlint::perf_probe::add_global(9, perf_jobs);
    let perf_collect = std::time::Instant::now();
    let mut results = Vec::new();
    let mut errors_present''')
edit('crates/cli/src/main.rs','    sort_results(&mut results);','''    rust_markdownlint::perf_probe::add_global(10, perf_collect);
    let perf_sort = std::time::Instant::now();
    sort_results(&mut results);
    rust_markdownlint::perf_probe::add_global(11, perf_sort);''')
edit('crates/cli/src/main.rs','        formatters::run(&formatters, &base, &results)?;','''        let perf_output = std::time::Instant::now();
        formatters::run(&formatters, &base, &results)?;
        rust_markdownlint::perf_probe::add_global(12, perf_output);''')
print('instrumented',root)
