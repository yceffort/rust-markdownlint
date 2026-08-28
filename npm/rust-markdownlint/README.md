# @yceffort/rust-markdownlint

A Rust implementation of [markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2) v0.22.1 (markdownlint v0.40.0). It is meant to be a drop-in replacement: the same command line, the same `.markdownlint-cli2.{jsonc,yaml}` and `.markdownlint.{jsonc,json,yaml,yml}` configuration files, the same inline comments (`<!-- markdownlint-disable -->` and friends), and byte-identical output. Source, benchmarks, and issues are at [yceffort/rust-markdownlint](https://github.com/yceffort/rust-markdownlint).

## Installation

```bash
npm i -D @yceffort/rust-markdownlint
npx rust-markdownlint "**/*.md" "#node_modules"
```

The package is a thin wrapper that runs a prebuilt binary from one of the platform packages installed as optional dependencies: `@yceffort/rust-markdownlint-darwin-arm64`, `-darwin-x64`, `-linux-x64`, `-linux-arm64`, `-win32-x64`. The Linux binaries are statically linked with musl and work on glibc and musl (Alpine) systems alike. No Node.js addon, no postinstall script, no download at install time.

You can also download a binary for your platform from [Releases](https://github.com/yceffort/rust-markdownlint/releases): macOS (arm64, x86_64), Linux (x86_64, arm64, statically linked with musl), Windows (x86_64). Each archive comes with a `.sha256` file.

```bash
curl -LO https://github.com/yceffort/rust-markdownlint/releases/latest/download/rust-markdownlint-v0.1.1-aarch64-apple-darwin.tar.gz
tar xzf rust-markdownlint-v0.1.1-aarch64-apple-darwin.tar.gz
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

- The banner reads `rust-markdownlint v0.1.1 (markdownlint-cli2 v0.22.1 / markdownlint v0.40.0 compatible)`. Turn on `noBanner` if something parses it.
- Anything that requires loading JavaScript modules is not supported. `.markdownlint-cli2.{cjs,mjs}` and `.markdownlint.{cjs,mjs}` configuration files are an error (exit 2), and `customRules`, `markdownItPlugins`, `outputFormatters`, `modulePaths` are ignored as listed above. Use the original if you need custom rules or markdown-it plugins.
- File names in the results are sorted with an approximation of ICU `localeCompare` that is exact for ASCII. Non-ASCII file names sort by code point.
- MD060 measures character width with `unicode-width` instead of `string-width`. A handful of characters (for example half-width katakana voiced marks) may differ.
- The markdown parser is a modified [markdown-rs](https://github.com/wooorm/markdown-rs) rather than micromark. 12 of the 388 original fixtures have slightly different token structure (lazy continuation lines after fenced code inside lists, for example); rule results are unaffected.

## License

MIT
