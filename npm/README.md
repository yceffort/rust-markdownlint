# npm packages

- `rust-markdownlint/`: the main package `@yceffort/rust-markdownlint`. `bin/rust-markdownlint.js` picks the platform package for `process.platform` and `process.arch`, runs its binary with `spawnSync`, and forwards stdio and the exit code. The platform packages are `optionalDependencies`, so npm installs only the one that matches the machine.
- `platforms/<name>/`: `@yceffort/rust-markdownlint-<name>` for `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`, `win32-x64`. Each holds a `package.json` with `os` and `cpu` and, at release time, the binary copied from the matching build (see `.github/workflows/release.yml`). The binaries are not committed.

All six `package.json` files must carry the same `version` as `crates/cli/Cargo.toml`. The release workflow checks this against the tag before building.

To try the packages locally: build the binary, copy it into `npm/platforms/<name>/`, `npm pack` the main package and that platform package, and install both tarballs into an empty project.
