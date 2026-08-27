# markdownlint-cli2 scenario snapshots

markdownlint-cli2 v0.22.1 ships 216 command line scenarios in `test/markdownlint-cli2-test-cases.mjs`; 210 of them have an expected `stdout`, `stderr`, and exit code in `test/snapshots/markdownlint-cli2-test-exec.mjs.md`. `crates/cli/tests/cli2_scenarios.rs` runs every scenario that does not need JavaScript with the same working directory and the same arguments and compares the result with the snapshot.

| Category | Scenarios |
|----------|-----------|
| Pass (identical to the snapshot) | 159 |
| Excluded by design (JavaScript module loading) | 51 |
| Excluded because the original does not snapshot them (`*-no-require`) | 6 |
| Known differences | 0 |
| Total | 216 |

## Running

```bash
cargo test -p rust-markdownlint-cli --test cli2_scenarios                  # all scenarios, about 1.5 s
CLI2_SCENARIO=nested-directories cargo test -p rust-markdownlint-cli --test cli2_scenarios
```

The fixtures live in `crates/cli/tests/fixtures/cli2/` (MIT, see the README there). To regenerate them from a newer checkout of the original:

```bash
git clone --depth 1 --branch v0.22.1 --filter=blob:none --sparse https://github.com/DavidAnson/markdownlint-cli2.git
(cd markdownlint-cli2 && git sparse-checkout set test)
node scripts/dump-cli2-scenarios.mjs markdownlint-cli2/test
git add -f crates/cli/tests/fixtures/cli2   # the gitignore scenarios' own .gitignore files hide 4 fixture files from a plain git add
```

The test copies the fixture tree to a temporary directory (scenarios reference sibling directories such as `../config-files`, and `isolate` scenarios create and remove `<name>-copy-exec`), runs the binary, and normalizes the output exactly like the original `sanitize` function: `\r` removed, `vX.Y.Z` for version strings, `:[PATH]` for the absolute `sentinel` path. The only extra step is replacing the banner line with the original `markdownlint-cli2 vX.Y.Z (markdownlint vX.Y.Z)` banner, which the README lists as a difference. Scenarios with a `stderrRe` in the original are checked with the same regular expression instead of a `stderr` snapshot.

The fixture files are marked `-text` in `.gitattributes` so that a Windows checkout with `core.autocrlf` keeps their LF line endings (line numbers and MD047 depend on them).

## Excluded scenarios

Everything that needs JavaScript module loading is excluded, matching the README section "Differences from markdownlint-cli2": `.markdownlint-cli2.{cjs,mjs}` and `.markdownlint.{cjs,mjs}` configuration files are an error (exit 2, `Unable to use configuration file '...'; JavaScript configuration files (.cjs/.mjs) are not supported`), and `customRules`, `markdownItPlugins`, `modulePaths` print `Ignoring unsupported option: <name>` and are otherwise ignored. `outputFormatters` is implemented with built-in formatters, so the scenarios that only name the original formatter packages (`outputFormatters`, `outputFormatters-npm`, `outputFormatters-params`, `outputFormatters-severity`, `outputFormatters-clean`, `outputFormatters-missing`, `formatter-summarize`, `formatter-pretty`, `formatter-template`) run and pass even though the original marks them `usesRequire` or sets `FORCE_COLOR`; the harness lists them in `BUILTIN_FORMATTER_SCENARIOS`. The "observed" column is what `rust-markdownlint` does today (checked by running every excluded scenario), the "original" column is the exit code of markdownlint-cli2.

### JavaScript configuration file (exit 2 with the error above)

| Scenario | Original exit | Observed |
|----------|---------------|----------|
| `markdownlint-cjs`, `markdownlint-mjs`, `markdownlint-cli2-cjs`, `markdownlint-cli2-mjs` | 1 | exit 2, error names the `.cjs`/`.mjs` file in the directory |
| `markdownlint-cjs-invalid`, `markdownlint-mjs-invalid`, `markdownlint-cli2-cjs-invalid`, `markdownlint-cli2-mjs-invalid` | 2 | exit 2 |
| `markdownlint-cli2-extends` | 1 | exit 2 (`cjs/.markdownlint-cli2.cjs` in a subdirectory) |
| `config-files-.markdownlint-cli2.cjs-arg`, `-alternate-arg`, `-absolute-arg`, `config-files-options.cjs-arg` | 1 | exit 2 |
| `config-files-.markdownlint-cli2.mjs-arg`, `-alternate-arg`, `-absolute-arg`, `config-files-options.mjs-arg` | 1 | exit 2 |
| `config-files-.markdownlint.cjs-arg`, `-alternate-arg`, `-absolute-arg`, `config-files-config.cjs-arg` | 1 | exit 2 |
| `config-files-.markdownlint.mjs-arg`, `-alternate-arg`, `-absolute-arg`, `config-files-config.mjs-arg` | 1 | exit 2 |
| `config-files-invalid.markdownlint-cli2.cjs-invalid-arg`, `config-files-invalid.markdownlint-cli2.mjs-invalid-arg`, `config-files-invalid.markdownlint.cjs-invalid-arg`, `config-files-invalid.markdownlint.mjs-invalid-arg` | 2 | exit 2 |
| `config-files-.markdownlint.cjs-redundant-arg` | 1 | exit 2 |
| `customRules-pre-imported`, `outputFormatters-pre-imported`, `outputFormatters-params-absolute` | 1 | exit 2 (`.markdownlint-cli2.cjs`) |
| `tilde-paths-commonjs`, `tilde-paths-module` | 1 | exit 2 |
| `customRules`, `markdownItPlugins`, `modulePaths-non-root` | 1 | warning for the ignored option, then exit 2 because a subdirectory has a `.cjs`/`.mjs` configuration file |

### Ignored option (warning on stderr, then normal linting)

| Scenario | Original exit | Observed |
|----------|---------------|----------|
| `markdownlint-cli2-jsonc-example`, `markdownlint-cli2-yaml-example` | 1 | exit 1, warnings for `customRules`, `markdownItPlugins`, `modulePaths` (`outputFormatters` names the default formatter, which is built in) |
| `config-relative-commonjs-arg`, `config-relative-module-arg` | 1 | exit 1, warnings for `customRules`, `markdownItPlugins`; `outputFormatters` names a custom module, so exit 2 after the summary |
| `customRules-throws` | 1 | exit 1 |
| `customRules-missing`, `customRules-invalid` | 2 | exit 0 (the option is ignored, so a missing or invalid rule module is not an error) |
| `markdownItPlugins-missing` | 2 | exit 0 (same reason) |
| `outputFormatters-file`, `outputFormatters-module` | 1 | exit 2, `Unable to import module '<custom formatter>'.` after the summary (the built-in formatters only match the original package names) |
| `formatter-pretty-appendLink` | 1 | exit 1, warning for `customRules`; the `extended-ascii` custom rule result is missing from the pretty output |
| `nested-options-config`, `modulePaths` | 1 | exit 1 |

### Not part of the exec snapshot

`markdownlint-cjs-no-require`, `markdownlint-mjs-no-require`, `markdownlint-cli2-cjs-no-require`, `markdownlint-cli2-mjs-no-require`, `customRules-no-require`, `markdownItPlugins-no-require` are only run by the original in hosts without `import` support and have no exec snapshot. `rust-markdownlint` exits 2 for all of them (a `.cjs`/`.mjs` configuration file is involved in each).

## Known differences

None. `markdownlint-cli2-yaml-mismatch`, `markdownlint-cli2-yaml-mismatch-config`, `markdownlint-yaml-mismatch-config` (a JSONC document with a `// Comment` line saved under a `.yaml` name) used to differ: js-yaml rejects it with `missed comma between flow collection entries` while serde-saphyr follows the YAML specification and parses it as `{ "// Comment \"config\"": { "default": false } }`. `parse_config_as` now walks the granit-parser scanner tokens before parsing YAML and reproduces the js-yaml rule (inside a flow collection, an implicit key's `:` must be on the line where the key starts, and the continuation lines of a multi-line plain scalar must be indented at least one column past the enclosing block collection), so these scenarios exit 2 with the same message and position.

## `--fix` comparison

`scripts/compare-fix.sh` copies the 388 markdownlint `test/*.md` fixtures twice, runs `rust-markdownlint --fix` and `markdownlint-cli2 --fix` with `{ "noBanner": true }`, and diffs the resulting files and the stderr output.

Result with markdownlint-cli2 v0.22.1 (markdownlint v0.40.0): 388 files, 174 changed by `--fix`, 0 files differ, stderr after the fix (1560 lines) identical.

```bash
scripts/compare-fix.sh                      # uses bench/node_modules/.bin/markdownlint-cli2
scripts/compare-fix.sh path/to/markdownlint-cli2
```
