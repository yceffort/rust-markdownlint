// 원본 markdownlint 로 tests/fixtures/markdownlint/*.md 를 기본 설정으로 lint 하고
// 결과를 규칙별로 tests/fixtures/expected/<MD0XX>.json 에 저장한다.
// (원본 test/markdownlint-test-scenarios.mjs 와 같은 조건. 규칙별 실행은 파일 안의
// configure-file 주석이 다른 규칙을 켤 수 있어 결과가 섞인다.)
//
// 사용법: node scripts/dump-expected.mjs <markdownlint@0.40.0 패키지 디렉토리> [markdownlint-cli2 패키지 디렉토리]
//   예: node scripts/dump-expected.mjs bench/node_modules/markdownlint bench/node_modules/markdownlint-cli2
//   cli2 디렉토리를 주면 inline config 를 cli2 와 같은 jsonc/toml/yaml 파서로 해석한다
//   (라이브러리 기본은 JSON.parse 라 따옴표 없는 키 같은 입력을 무시한다).
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const pkgDir = process.argv[2];
if (!pkgDir) {
  console.error("usage: dump-expected.mjs <markdownlint package dir>");
  process.exit(1);
}
const { lint } = await import(pathToFileURL(path.join(pkgDir, "lib/exports-promise.mjs")));
const cli2Dir = process.argv[3];
const configParsers = cli2Dir
  ? (await import(pathToFileURL(path.join(cli2Dir, "parsers/parsers.mjs")))).default
  : undefined;

const root = path.join(import.meta.dirname, "..", "crates", "core", "tests", "fixtures");
const fixtureDir = path.join(root, "markdownlint");
const outDir = path.join(root, "expected");
fs.mkdirSync(outDir, { recursive: true });

const ruleNames = [...new Set(
  fs.readdirSync(path.join(pkgDir, "lib")).flatMap((f) =>
    [...f.matchAll(/md(\d{3})/g)].map((m) => `MD${m[1]}`))
)].sort();
const files = fs.readdirSync(fixtureDir).filter((f) => f.endsWith(".md")).sort();
process.chdir(fixtureDir);
const results = await lint({ files, configParsers });

for (const rule of ruleNames) {
  const expected = {};
  for (const file of files) {
    const errors = results[file]
      .filter((e) => e.ruleNames[0] === rule)
      .map((e) => ({
        lineNumber: e.lineNumber,
        ruleNames: e.ruleNames,
        errorDetail: e.errorDetail,
        errorContext: e.errorContext,
        errorRange: e.errorRange,
        fixInfo: e.fixInfo,
      }));
    if (errors.length > 0) expected[file] = errors;
  }
  fs.writeFileSync(path.join(outDir, `${rule}.json`), `${JSON.stringify(expected, null, 2)}\n`);
  console.log(`${rule}: ${Object.keys(expected).length} files`);
}
