# rust-markdownlint

[![CI](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml/badge.svg)](https://github.com/yceffort/rust-markdownlint/actions/workflows/ci.yml)

A Rust implementation of [markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2) v0.22.1 (markdownlint v0.40.0). It is meant to be a drop-in replacement: the same command line, the same `.markdownlint-cli2.{jsonc,yaml}` and `.markdownlint.{jsonc,json,yaml,yml}` configuration files, the same inline comments (`<!-- markdownlint-disable -->` and friends), and byte-identical lint results, subject to the [differences below](#differences-from-markdownlint-cli2).

- All 53 rules of markdownlint v0.40.0 are implemented. Linting the original `test/*.md` corpus (388 files) with the default configuration produces 3218 errors that match the original byte for byte. A real-world repository with 20966 markdown files (including `node_modules`) produces 264114 identical errors.
- Files are linted in parallel. See [Performance](#performance) for measured comparisons with markdownlint-cli2 and rumdl.
- A single static binary. No Node.js required.

## Installation

With npm (the package is a thin wrapper that runs a prebuilt binary from a platform package installed as an optional dependency: `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`, `win32-x64`; no postinstall script, no download at install time):

```bash
npm i -D @yceffort/rust-markdownlint
npx rust-markdownlint "**/*.md" "#node_modules"
```

Or download a binary for your platform from [Releases](https://github.com/yceffort/rust-markdownlint/releases): macOS (arm64, x86_64), Linux (x86_64, arm64, statically linked with musl), Windows (x86_64). Each archive comes with a `.sha256` file.

```bash
curl -LO https://github.com/yceffort/rust-markdownlint/releases/latest/download/rust-markdownlint-v0.1.3-aarch64-apple-darwin.tar.gz
tar xzf rust-markdownlint-v0.1.3-aarch64-apple-darwin.tar.gz
./rust-markdownlint --help
rust-markdownlint completions zsh > ~/.zsh/completions/_rust-markdownlint   # shell completion
```

To build from source you need Rust 1.88 or later. On Linux you also need a C compiler and `make`, because the CLI links jemalloc there:

```bash
cargo install --git https://github.com/yceffort/rust-markdownlint rust-markdownlint-cli
```

Either way you get a `rust-markdownlint` binary.

### pre-commit

```yaml
repos:
  - repo: https://github.com/yceffort/rust-markdownlint
    rev: v0.1.3
    hooks:
      - id: rust-markdownlint        # or rust-markdownlint-fix to apply fixes
```

`rust-markdownlint` and `rust-markdownlint-fix` download the release binary for `rev` the first time they run (verified against the `.sha256` file) and need neither cargo nor Node.js, only a POSIX shell (macOS, Linux, Git Bash on Windows). `rust-markdownlint-node` installs the npm package instead and works wherever pre-commit's `node` language works; pin the package version with `additional_dependencies: ["@yceffort/rust-markdownlint@<version>"]` if it should differ from `rev`. All three lint the staged Markdown files that pre-commit passes in, with the configuration files of your repository.

### GitHub Action

```yaml
steps:
  - uses: actions/checkout@v4
  - uses: yceffort/rust-markdownlint@v0
    with:
      globs: |            # default: **/*.md
        docs/**/*.md
        "#node_modules"
      # config: .markdownlint-cli2.jsonc   # passed as --config
      # fix: true                          # leaves fixed files modified, does not commit
      # version: v0.1.3                    # default: the release matching the action ref
```

The action downloads the release binary (with `.sha256` verification), registers a problem matcher so every result becomes an annotation on the pull request, and fails when errors remain (warnings do not fail it, same as the exit code). `@v0` and `@v0.1` follow the newest release with that prefix; `@v0.1.3` pins one.

## How to use

The examples below use the installed `rust-markdownlint` binary. If you installed the npm package locally, use `npx rust-markdownlint` instead. If you extracted a release archive, use the binary's path, such as `./rust-markdownlint`.

### Check Markdown files

```bash
rust-markdownlint README.md                   # one file
rust-markdownlint "docs/**/*.md"              # a directory, including subdirectories
rust-markdownlint "**/*.md" "#node_modules"    # the project, excluding dependencies
```

Quote glob patterns so the CLI expands them consistently across shells. Diagnostics include the file, line, and rule ID. An exit code of 0 means there are no errors (warnings are allowed); 1 means lint errors remain; 2 means help was requested or an operational error occurred.

### Preview and apply fixes

```bash
rust-markdownlint --diff "docs/**/*.md"    # preview fixes as a patch
rust-markdownlint --fix "docs/**/*.md"     # write fixes to files
```

`--fix` applies the fixes supported by each rule and reports any remaining errors. Some issues need manual editing. `--diff` leaves files unchanged and exits with code 1 when changes are available.

### Configure a project

Create `.markdownlint-cli2.jsonc` in the project root:

```jsonc
{
  "globs": ["**/*.md"],
  "ignores": ["node_modules/**", "dist/**"],
  "config": {
    "MD013": false
  }
}
```

This checks Markdown files throughout the project, excludes dependencies and build output, and disables the line-length rule. Run from the project root to use the configured globs:

```bash
rust-markdownlint
rust-markdownlint --fix
```

Existing `.markdownlint-cli2.*` and `.markdownlint.*` files are discovered automatically. To choose a configuration explicitly, use `--config .markdownlint-cli2.jsonc`. See [supported options](#supported-options) and [compatibility differences](#differences-from-markdownlint-cli2) for supported file formats and behavior.

### Add npm scripts

After `npm i -D @yceffort/rust-markdownlint`, add these entries to your `package.json` scripts. They use the project configuration above:

```json
{
  "scripts": {
    "lint:md": "rust-markdownlint",
    "lint:md:fix": "rust-markdownlint --fix"
  }
}
```

Run `npm run lint:md` to check files or `npm run lint:md:fix` to apply fixes. For automation, see the [GitHub Action](#github-action) and [pre-commit hook](#pre-commit) examples.

### Command reference

The command line follows markdownlint-cli2. Replace the executable name in existing commands and scripts; additional Rust CLI options are marked below.

```bash
rust-markdownlint --config .markdownlint-cli2.jsonc "*.md"
rust-markdownlint --config .markdownlint.yaml --configPointer /config "*.md"
rust-markdownlint --no-globs "README.md"
cat README.md | rust-markdownlint -          # lint stdin
cat README.md | rust-markdownlint --format   # fix stdin and print the result to stdout
cat docs/x.md | rust-markdownlint --stdin-filename docs/x.md -   # lint stdin with docs/ configuration
rust-markdownlint --help
rust-markdownlint completions zsh > ~/.zsh/completions/_rust-markdownlint   # shell completion
```

| Argument | Description |
|----------|-------------|
| `glob0 [glob1] ...` | globby-style globs. A leading `!` or `#` excludes, a leading `:` is a literal path, everything after `--` is a glob |
| `-` | Lint stdin as a file named `stdin` |
| `--config <file>` | Top-level configuration file. The name must be a supported one (`.markdownlint-cli2.jsonc` etc.) or end with `.jsonc`, `.json`, `.toml`, `.yaml`, `.yml` |
| `--configPointer <pointer>` | JSON Pointer into the `--config` file |
| `--fix` | Write fixable errors back to the files |
| `--diff` | Not in markdownlint-cli2. Print what `--fix` would write as a unified diff on stdout (`git apply` takes it as is) and leave the files alone. Wins over `--fix` and over `fix` in the configuration, but a configured `fix: false` still turns the diff off. Exit code 1 when there is something to change |
| `--format` | Fix stdin and print it to stdout (no banner, progress, or results) |
| `--no-globs` | Ignore `globs` from configuration files and use only the command line globs |
| `--stdin-filename <path>` | Not in markdownlint-cli2. Report stdin as `<path>` and apply the configuration files of that directory (`.markdownlint-cli2.*`, `.markdownlint.*`, `ignores`), so editors can lint an unsaved buffer with the right settings. The file itself is neither read nor written, and if a glob also matches it only stdin is linted |
| `--help` | Show help |
| `server` | Not in markdownlint-cli2. Run a Language Server Protocol server on stdio (see [docs/lsp.md](docs/lsp.md)) |
| `completions <shell>` | Not in markdownlint-cli2. Write the `bash`, `zsh`, or `fish` completion script to stdout. The scripts also ship in the release archives under `completions/` |

- Configuration cascades per directory exactly like the original: `.markdownlint-cli2.{jsonc,yaml}` merges with the parent options, `.markdownlint.{jsonc,json,yaml,yml}` replaces the parent rule configuration.
- Normal lint output is byte-identical to markdownlint-cli2 except for the banner line. Results go to stderr, progress (`Finding:`, `Linting:`, `Summary:`) goes to stdout. Fatal configuration and file-system errors are described under [Differences](#differences-from-markdownlint-cli2).
- Exit codes: 0 (no errors, or warnings only), 1 (errors), 2 (help, invalid configuration, exception).

### Editor integration (LSP)

`rust-markdownlint server` runs a Language Server Protocol server on stdio, so Neovim, Helix, and other LSP clients get diagnostics and quick fixes without Node. Diagnostic positions and the quick fix results are the same as the CLI output and `--fix`.

```bash
rust-markdownlint server   # JSON-RPC over stdin/stdout, started by the editor
```

[docs/lsp.md](docs/lsp.md) has Neovim (`vim.lsp.config` and nvim-lspconfig), Helix, and Zed configuration, the supported requests, and a manual verification checklist.

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
| `outputFormatters` | Yes | Built-in ports of the original formatter packages, selected by package name: `markdownlint-cli2-formatter-default`, `-json` (`name`, `spaces`), `-junit` (`name`), `-sarif` (`name`), `-codequality` (`name`, `severity`, `severityError`, `severityWarning`), `-summarize` (`byFile`, `byRule`, `byFileByRule`, `byRuleByFile`), `-pretty` (`appendLink`), `-template` (`template`). Output files and text are byte-identical to the originals. Any other module id (a custom `.cjs`/`.mjs` formatter, an unknown package) is `Unable to import module '<id>'.` with exit 2, like the original when the module cannot be loaded |
| `modulePaths` | No | A one-line warning on stderr, then ignored |

Rule configuration supports all 53 rules of markdownlint v0.40.0 (MD001 through MD060, excluding the deprecated ones) with their parameters, aliases, and tags.

## Differences from markdownlint-cli2

- The banner reads `rust-markdownlint v0.1.3 (markdownlint-cli2 v0.22.1 / markdownlint v0.40.0 compatible)`. Turn on `noBanner` if something parses it.
- Anything that requires loading JavaScript modules is not supported. `.markdownlint-cli2.{cjs,mjs}` and `.markdownlint.{cjs,mjs}` configuration files are an error (exit 2), and `customRules`, `markdownItPlugins`, `modulePaths` are ignored as listed above. `outputFormatters` works with the built-in formatters listed above (the original npm packages are not loaded, so `-pretty` decides on colors and hyperlinks from `FORCE_COLOR`, `NO_COLOR`, `FORCE_HYPERLINK`, and the terminal like the original does, but with a shorter list of recognized terminals). Use the original if you need custom rules, markdown-it plugins, or a custom formatter module.
- Configuration files are parsed with Rust parsers (jsonc-parser, toml, serde-saphyr). Error messages for invalid files keep the original wording where the original tests rely on it (`Unable to parse JSONC content`, `Invalid TOML document`, `duplicated mapping key`) but the details differ. YAML flow collections are additionally checked with the js-yaml rules (an implicit key must have its `:` on the line where the key starts, and a multi-line plain scalar must stay indented past the enclosing block), so a JSONC document saved under a `.yaml` name fails with `missed comma between flow collection entries` like the original.
- Fatal configuration-loading and file-system errors have the same exit code (2), but use a concise Rust error message instead of Node.js `Error` object formatting and stack traces.
- A circular `extends` chain (`a.jsonc` extends `b.jsonc` extends `a.jsonc`) is reported as an unusable configuration file (exit 2). The original never finishes.
- File names in the results are sorted with an approximation of ICU `localeCompare` that is exact for ASCII. Non-ASCII file names sort by code point.
- MD060 measures character width with `unicode-width` instead of `string-width`. A handful of characters (for example half-width katakana voiced marks) may differ.
- The markdown parser is a modified [markdown-rs](https://github.com/wooorm/markdown-rs) rather than micromark. 12 of the 388 original fixtures have slightly different token structure (lazy continuation lines after fenced code inside lists, for example); rule results are unaffected. Text directives (`:name[label]`, from micromark-extension-directive) are not recognized; in practice this only showed up when linting binary files, where `_` inside such a label paired with one outside and produced extra MD049 errors.

## Performance

Release builds enable Thin LTO, and the Linux CLI uses jemalloc. The table below measures a build with these defaults; the [Codespaces A/B comparison](bench/remaining-gap-2026-09-07.md) that motivated them is recorded separately. Performance on macOS and Windows has not been measured.

Measured on **GitHub Codespaces**, 2026-09-07: 4 vCPUs (AMD EPYC 7763), 16 GB RAM, Ubuntu 24.04 x86_64. Rust 1.98.1, Node.js 24.14.0. The Rust binary is the static `x86_64-unknown-linux-musl` build that Releases and the npm `linux-x64` package ship, built with `cargo build --release --locked` from [`8bdd653`](https://github.com/yceffort/rust-markdownlint/commit/8bdd65342cd02a11f5e09d02186be51e0e4cc3c6) plus the Thin LTO and jemalloc change. rumdl uses its official Linux GNU release binary.

**Mean ± sample standard deviation, in milliseconds; lower is better.** Each tool ran 24 times per corpus after 3 warm-ups, cycling through all six tool orders four times. Timings include process startup, file discovery, linting, diagnostic sorting, and default output formatting; stdout/stderr were redirected to `/dev/null`.

| Corpus | rust-markdownlint | rumdl 0.2.67 | markdownlint-cli2 0.23.2 |
|---|---:|---:|---:|
| Blog posts, 445 files (7.50 MB) | 434.4 ± 16.1 | 380.1 ± 5.3 | 4,792.9 ± 104.4 |
| markdownlint fixtures, 388 files (0.25 MB) | 83.8 ± 2.4 | 89.8 ± 1.2 | 1,192.4 ± 23.6 |
| Fixtures copied 10 times, 3,880 files (2.45 MB) | 762.9 ± 25.9 | 748.3 ± 14.6 | 6,501.8 ± 157.1 |

A glibc build of the same source (what `cargo install` produces on Linux) measured 416.7 ± 17.7 ms on the blog corpus and 726.0 ± 23.1 ms on the 10x corpus in a separate session; the static musl binary is about 4% to 5% slower on the larger corpora.

The corpora contain only Markdown files copied into isolated directories, with `noBanner: true` and each tool's default rules. Project rule configurations are excluded; inline directives remain in the source. rumdl runs with `--no-cache --no-config`, and all tools use a warm filesystem cache. The [blog corpus is pinned to a commit](https://github.com/yceffort/blog/tree/4c7cade067a10eb565a8e608081532fa055218c3/apps/blog/posts).

Rust and markdownlint-cli2 0.23.2 produced byte-identical diagnostics on these corpora; cli2's progress output differs. A separate check against the compatibility target, cli2 0.22.1, matched exit codes, stdout, and stderr. rumdl has different rules and diagnostics: on the blog corpus it reported 17,527 diagnostics versus 16,764 for Rust and cli2. Fixture inputs also exercise inline configuration that rumdl interprets differently.

[Methodology, reproduction commands, diagnostic counts, and earlier measurements](bench/RESULTS.md#github-codespaces-비교-2026-09-07) are recorded alongside the raw samples and environment for the [musl](bench/results/codespaces-2026-09-07-musl.json) and [glibc](bench/results/codespaces-2026-09-07-gnu.json) builds.

## Development

Linux CLI builds compile jemalloc and require a C compiler and `make`. For Linux musl release targets, install `musl-tools` as well (Debian/Ubuntu). jemalloc fixes the page size at build time, so the aarch64 release workflow sets `JEMALLOC_SYS_WITH_LG_PAGE=16` to keep the binary working on 16K and 64K page kernels. The allocator is configured in the CLI binary; library consumers retain control of their allocator.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

To compare one rule against the original markdownlint expectations, filter the snapshot test by rule name: `cargo test -p rust-markdownlint --test rules_snapshot -- MD047`. Regenerate the expectations with `node scripts/dump-expected.mjs bench/node_modules/markdownlint bench/node_modules/markdownlint-cli2`.

The command line behavior is checked against the markdownlint-cli2 test scenarios and their snapshots: `cargo test -p rust-markdownlint-cli --test cli2_scenarios` (one scenario: `CLI2_SCENARIO=<name>`). `scripts/compare-fix.sh` runs `--fix` with both tools on the 388 fixtures and diffs the results. Scenario list, exclusions, and results are in [docs/cli2-scenarios.md](docs/cli2-scenarios.md).

### Benchmarks

For the three-tool Codespaces comparison, [bench/compare-tools.py](bench/compare-tools.py) prepares isolated corpora, checks outputs, alternates tool order, and saves individual timings as JSON. See [setup and methodology](bench/RESULTS.md#github-codespaces-비교-2026-09-07).

`bench/run.sh` runs both tools on the same corpus, diffs the results, and times them with `hyperfine` (needs `node` and `hyperfine`).

```bash
bench/run.sh MD047          # one rule
bench/run.sh all            # default configuration (all rules, inline config honored)
SCALE=10 bench/run.sh all   # corpus copied 10 times
```

Results are recorded in `bench/RESULTS.md`. On pull requests, CI benchmarks the changed rules and posts the numbers as a comment.

### Releases

Bump `version` in `crates/cli/Cargo.toml` and push a matching `v*` tag. The [release workflow](.github/workflows/release.yml) builds the five platform binaries and uploads them to a GitHub Release. It fails if the tag and the crate version differ.

The same tag also publishes six npm packages: `@yceffort/rust-markdownlint-{darwin-arm64,darwin-x64,linux-x64,linux-arm64,win32-x64}` (one binary each, built from the same artifacts as the GitHub Release) and then `@yceffort/rust-markdownlint` (the wrapper, with the platform packages as `optionalDependencies`). Bump `version` in `npm/rust-markdownlint/package.json` (including its `optionalDependencies`) and in the five `npm/platforms/*/package.json` too; the workflow fails before building if any of them differs from the tag. The `publish-npm` job runs after the GitHub Release is created, so a failed npm publish leaves the release in place. It authenticates with the `NPM_TOKEN` repository secret (an npm granular access token with publish permission for the `@yceffort` scope) and publishes with `--provenance`. To switch to npm trusted publishing instead, add a trusted publisher on npmjs.com for each of the six packages (organization or user `yceffort`, repository `rust-markdownlint`, workflow `release.yml`), remove the `NODE_AUTH_TOKEN` line from the workflow, and make sure the job runs npm 11.5.1 or later (`npm install -g npm@latest` after `setup-node`), which is what trusted publishing requires; the `id-token: write` permission is already there.

## License

MIT, see [LICENSE](LICENSE). The binary contains a modified copy of [markdown-rs](https://github.com/wooorm/markdown-rs) (Titus Wormer, MIT), and the rules and command line are ported from [markdownlint](https://github.com/DavidAnson/markdownlint) and [markdownlint-cli2](https://github.com/DavidAnson/markdownlint-cli2) (David Anson, MIT). Linux binaries also statically link [jemalloc](https://github.com/jemalloc/jemalloc) (BSD-2-Clause) through tikv-jemallocator (MIT OR Apache-2.0). Their notices are in [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md), which ships with every release archive and npm package.
