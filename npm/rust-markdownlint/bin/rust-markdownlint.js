#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");

const PLATFORMS = {
  "darwin-arm64": "@yceffort/rust-markdownlint-darwin-arm64",
  "darwin-x64": "@yceffort/rust-markdownlint-darwin-x64",
  "linux-x64": "@yceffort/rust-markdownlint-linux-x64",
  "linux-arm64": "@yceffort/rust-markdownlint-linux-arm64",
  "win32-x64": "@yceffort/rust-markdownlint-win32-x64",
};

function binaryPath(platform, arch) {
  const key = `${platform}-${arch}`;
  const pkg = PLATFORMS[key];
  if (!pkg) {
    throw new Error(
      `Unsupported platform: ${key}. Supported: ${Object.keys(PLATFORMS).join(", ")}`,
    );
  }
  const file = platform === "win32" ? "rust-markdownlint.exe" : "rust-markdownlint";
  try {
    return require.resolve(`${pkg}/${file}`);
  } catch {
    throw new Error(
      `The platform package ${pkg} is not installed. It is an optional dependency of @yceffort/rust-markdownlint; reinstall without --no-optional, or install it directly.`,
    );
  }
}

function main() {
  let bin;
  try {
    bin = binaryPath(process.platform, process.arch);
  } catch (error) {
    console.error(error.message);
    return 1;
  }
  const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
  if (result.error) {
    console.error(`Failed to run ${bin}: ${result.error.message}`);
    return 1;
  }
  if (result.signal) {
    process.kill(process.pid, result.signal);
    return 1;
  }
  return result.status;
}

if (require.main === module) {
  process.exitCode = main();
}

module.exports = { PLATFORMS, binaryPath };
