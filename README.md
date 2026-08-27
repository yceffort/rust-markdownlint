# rust-markdownlint

[![CI](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml/badge.svg)](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml)

A Rust implementation of [markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2) v0.22.1 (markdownlint v0.40.0). It is meant to be a drop-in replacement: the same command line, the same `.markdownlint-cli2.{jsonc,yaml}` and `.markdownlint.{jsonc,json,yaml,yml}` configuration files, the same inline comments (`<!-- markdownlint-disable -->` and friends), and byte-identical output.

- All 53 rules of markdownlint v0.40.0 are implemented. Linting the original `test/*.md` corpus (388 files) with the default configuration produces 3218 errors that match the original byte for byte. A real-world repository with 20966 markdown files (including `node_modules`) produces 264114 identical errors.
- Files are linted in parallel. 3x to 12x faster than markdownlint-cli2 depending on the corpus and the machine (see [Performance](#performance)).
- A single static binary. No Node.js required.

## Installation

Download a binary for your platform from [Releases](https://github.com/yceffort/rust-markdownlint/releases): macOS (arm64, x86_64), Linux (x86_64, arm64, statically linked with musl), Windows (x86_64). Each archive comes with a `.sha256` file.

```bash
curl -LO https://github.com/yceffort/rust-markdownlint/releases/latest/download/rust-markdownlint-v0.1.0-aarch64-apple-darwin.tar.gz
tar xzf rust-markdownlint-v0.1.0-aarch64-apple-darwin.tar.gz
./rust-markdownlint --help
```

To build from source you need Rust 1.88 or later:

```bash
cargo install --git https://github.com/yceffort/rust-markdownlint rust-markdownlint-cli
```

Either way you get a `rust-markdownlint` binary.

## Usage

The command line is the same as markdownlint-cli2. Replace the executable name in your existing commands and scripts.

```bash
rust-markdownlint "**/*.md" "#node_modules"
rust-markdownlint --fix "docs/**/*.md"
rust-markdownlint --config .markdownlint-cli2.jsonc "*.md"
rust-markdownlint --config .markdownlint.yaml --configPointer /config "*.md"
rust-markdownlint --no-globs "README.md"
cat README.md | rust-markdownlint -          # lint stdin
cat README.md | rust-markdownlint --format   # fix stdin and print the result to stdout
rust-markdownlint --help
```

| Argument | Description |
|----------|-------------|
| `glob0 [glob1] ...` | globby-style globs. A leading `!` or `#` excludes, a leading `:` is a literal path, everything after `--` is a glob |
| `-` | Lint stdin as a file named `stdin` |
| `--config <file>` | Top-level configuration file. The name must be a supported one (`.markdownlint-cli2.jsonc` etc.) or end with `.jsonc`, `.json`, `.toml`, `.yaml`, `.yml` |
| `--configPointer <pointer>` | JSON Pointer into the `--config` file |
| `--fix` | Write fixable errors back to the files |
| `--format` | Fix stdin and print it to stdout (no banner, progress, or results) |
| `--no-globs` | Ignore `globs` from configuration files and use only the command line globs |
| `--help` | Show help |

- Configuration cascades per directory exactly like the original: `.markdownlint-cli2.{jsonc,yaml}` merges with the parent options, `.markdownlint.{jsonc,json,yaml,yml}` replaces the parent rule configuration.
- Output is byte-identical to markdownlint-cli2 except for the banner line. Results go to stderr, progress (`Finding:`, `Linting:`, `Summary:`) goes to stdout.
- Exit codes: 0 (no errors, or warnings only), 1 (errors), 2 (help, invalid configuration, exception).

### Supported options

Options in `.markdownlint-cli2.{jsonc,yaml}`:

| Option | Supported | Notes |
|--------|-----------|-------|
| `config` | Yes | Rule configuration, including `extends` |
| `fix` | Yes | Same as `--fix`. `false` in a configuration file overrides the flag |
| `frontMatter` | Yes | Front matter regular expression (JavaScript syntax) |
| `gitignore` | Yes | `true` or a gitignore-style string |
| `globs` | Yes | |
| `ignores` | Yes | |
| `noBanner` | Yes | |
| `noInlineConfig` | Yes | |
| `noProgress` | Yes | |
| `showFound` | Yes | |
| `customRules` | No | A one-line warning on stderr, then ignored |
| `markdownItPlugins` | No | A one-line warning on stderr, then ignored |
| `outputFormatters` | No | A one-line warning on stderr, then ignored. Only the default formatter is available |
| `modulePaths` | No | A one-line warning on stderr, then ignored |

Rule configuration supports all 53 rules of markdownlint v0.40.0 (MD001 through MD060, excluding the deprecated ones) with their parameters, aliases, and tags.

## Differences from markdownlint-cli2

- The banner reads `rust-markdownlint v0.1.0 (markdownlint-cli2 v0.22.1 / markdownlint v0.40.0 compatible)`. Turn on `noBanner` if something parses it.
- Anything that requires loading JavaScript modules is not supported. `.markdownlint-cli2.{cjs,mjs}` and `.markdownlint.{cjs,mjs}` configuration files are an error (exit 2), and `customRules`, `markdownItPlugins`, `outputFormatters`, `modulePaths` are ignored as listed above. Use the original if you need custom rules or markdown-it plugins.
- File names in the results are sorted with an approximation of ICU `localeCompare` that is exact for ASCII. Non-ASCII file names sort by code point.
- MD060 measures character width with `unicode-width` instead of `string-width`. A handful of characters (for example half-width katakana voiced marks) may differ.
- The markdown parser is a modified [markdown-rs](https://github.com/wooorm/markdown-rs) rather than micromark. 12 of the 388 original fixtures have slightly different token structure (lazy continuation lines after fenced code inside lists, for example); rule results are unaffected.

## Performance

`hyperfine --warmup 3`, mean ± σ in milliseconds, ratio is markdownlint-cli2 / rust-markdownlint. The results of both tools are identical (no diff) in every row.

| Corpus | Machine | markdownlint-cli2 | rust-markdownlint | Ratio |
|--------|---------|-------------------|-------------------|-------|
| markdownlint `test/*.md`, 388 files, all rules | Apple M-series, 10 cores | 366.5 ± 7.0 | 57.5 ± 1.6 | 6.4x |
| markdownlint `test/*.md`, 388 files, all rules | GitHub Actions ubuntu-latest | 1267.3 ± 90.4 | 178.4 ± 1.3 | 7.1x |
| Same corpus copied 10 times, 3880 files | Apple M-series, 10 cores | 2956.7 ± 199.6 | 682.6 ± 10.0 | 4.3x |
| A blog repository, `apps/blog/posts/**/*.md`, 441 posts (7.2 MB), project config | Apple M-series, 10 cores | 1475.0 ± 48.0 | 123.7 ± 6.3 | 11.9x |
| The same repository, `**/*.md` including `node_modules`, 20966 files (single run) | Apple M-series, 10 cores | 48306 | 14419 | 3.4x |

The 388-file corpus is small enough that process startup dominates both tools. Parallel linting alone made the Rust binary 2.8x faster than its own sequential version on that corpus (159.8 ms to 57.5 ms) and 2.9x on the 10x corpus (1516 ms to 524 ms). Per-rule results and the parallelization comparison are in [bench/RESULTS.md](bench/RESULTS.md).

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

To compare one rule against the original markdownlint expectations, filter the snapshot test by rule name: `cargo test -p rust-markdownlint --test rules_snapshot -- MD047`. Regenerate the expectations with `node scripts/dump-expected.mjs bench/node_modules/markdownlint bench/node_modules/markdownlint-cli2`.

### Benchmarks

`bench/run.sh` runs both tools on the same corpus, diffs the results, and times them with `hyperfine` (needs `node` and `hyperfine`).

```bash
bench/run.sh MD047          # one rule
bench/run.sh all            # default configuration (all rules, inline config honored)
SCALE=10 bench/run.sh all   # corpus copied 10 times
```

Results are recorded in `bench/RESULTS.md`. On pull requests, CI benchmarks the changed rules and posts the numbers as a comment.

### Releases

Bump `version` in `crates/cli/Cargo.toml` and push a matching `v*` tag. The [release workflow](.github/workflows/release.yml) builds the five platform binaries and uploads them to a GitHub Release. It fails if the tag and the crate version differ.
