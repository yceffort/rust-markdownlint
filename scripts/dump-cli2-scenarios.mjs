// 원본 markdownlint-cli2 v0.22.1 의 test/markdownlint-cli2-test-cases.mjs 시나리오 정의와
// test/snapshots/markdownlint-cli2-test-exec.mjs.md 의 기대 출력을 합쳐
// crates/cli/tests/fixtures/cli2/scenarios.json 으로 덤프하고, 시나리오 fixture 디렉토리를
// crates/cli/tests/fixtures/cli2/test/ 아래에 복사한다. crates/cli/tests/cli2_scenarios.rs 가 이를 읽는다.
//
// 사용법: node scripts/dump-cli2-scenarios.mjs <markdownlint-cli2 저장소의 test 디렉토리>
//   예: git clone --depth 1 --branch v0.22.1 --filter=blob:none --sparse https://github.com/DavidAnson/markdownlint-cli2.git
//       (cd markdownlint-cli2 && git sparse-checkout set test)
//       node scripts/dump-cli2-scenarios.mjs markdownlint-cli2/test
//
// 시나리오 정의는 ava 의 test() 를 가로채 원본 testCases() 를 그대로 실행해 얻는다 (invoke/copyDir 훅으로
// args, cwd, env, isolate, shadow 를 기록). usesRequire/env/script 플래그는 include* 옵션을 바꿔 두 번
// 실행한 차집합으로 복원한다. 스냅샷은 ava 의 markdown 리포트를 파싱한다.
import fs from "node:fs";
import path from "node:path";

const testDir = process.argv[2];
if (!testDir) {
  console.error("usage: dump-cli2-scenarios.mjs <markdownlint-cli2/test>");
  process.exit(1);
}
const outDir = path.join(import.meta.dirname, "..", "crates", "cli", "tests", "fixtures", "cli2");
const BASE = "/__BASE__";

// --- 시나리오 정의 -----------------------------------------------------------------------------
async function collect(include) {
  const source = fs
    .readFileSync(path.join(testDir, "markdownlint-cli2-test-cases.mjs"), "utf8")
    // 모듈은 한 번만 평가되므로 test 는 호출 시점의 전역을 보게 한다
    .replace('import test from "ava";', "const test = (...a) => globalThis.__cli2Test(...a);")
    // sameFileSystem 을 항상 참으로 만들어 tilde-paths 시나리오도 목록에 넣는다
    .replace(/import\.meta\.dirname/g, "os.homedir()");
  const registered = [];
  globalThis.__cli2Test = (name, fn) => registered.push([name, fn]);
  const { default: testCases } = await import(
    `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
  );
  const scenarios = [];
  let current = null;
  testCases({
    host: "exec",
    baseDir: BASE,
    invoke: (relative, args, noImport, env) => async () => {
      current.cwd = relative;
      current.args = args;
      current.noImport = Boolean(noImport);
      if (env) current.env = env;
      return { exitCode: 0, stdout: "", stderr: "" };
    },
    copyDir: async (fromDir, toDir) => {
      current.isolate = true;
      current.shadow = fromDir;
      current.isolatedDir = toDir;
    },
    removeDir: async () => {},
    ...include,
  });
  const t = {
    plan() {},
    is(_actual, expected) {
      current.exitCode = expected;
    },
    regex(_actual, re) {
      current.stderrRe = { source: re.source, flags: re.flags };
    },
    true() {},
    snapshot() {},
  };
  for (const [fullName, fn] of registered) {
    current = { name: fullName.replace(/ \(exec\)$/, "") };
    await fn(t);
    scenarios.push(current);
  }
  return scenarios;
}

const all = await collect({ includeNoImport: true, includeEnv: true, includeScript: true, includeRequire: true });
const names = (list) => new Set(list.map((s) => s.name));
const withoutRequire = names(await collect({ includeNoImport: true, includeEnv: true, includeScript: true, includeRequire: false }));
const withoutEnv = names(await collect({ includeNoImport: true, includeEnv: false, includeScript: true, includeRequire: true }));
const withoutScript = names(await collect({ includeNoImport: true, includeEnv: true, includeScript: false, includeRequire: true }));
for (const s of all) {
  s.usesRequire = !withoutRequire.has(s.name);
  s.usesEnv = !withoutEnv.has(s.name);
  s.usesScript = !withoutScript.has(s.name);
}

// --- 스냅샷 파싱 -------------------------------------------------------------------------------
// ava (concordance) 리포트: "## <name> (exec)" 아래 4칸 들여쓴 객체 리터럴. 문자열은 '...' 또는
// 줄마다 ␊ 로 끝나는 `...` 템플릿이며 이어지는 줄은 6칸 들여쓰기다.
function unescapeString(body) {
  return body.replace(/\\(.)/gu, (_, c) => {
    if (!"\\`'$".includes(c)) throw new Error(`unexpected escape \\${c}`);
    return c;
  });
}
function parseSnapshot(markdown) {
  const expected = {};
  const sections = markdown.split(/^## /mu).slice(1);
  for (const section of sections) {
    const name = section.slice(0, section.indexOf(" (exec)\n"));
    const lines = section.split("\n");
    const entry = {};
    for (let i = 0; i < lines.length; i++) {
      const m = /^      (\w+): (.*)$/u.exec(lines[i]);
      if (!m) continue;
      const [, key, rest] = m;
      if (/^-?\d+,$/u.test(rest)) {
        entry[key] = Number(rest.slice(0, -1));
      } else if (rest.startsWith("'")) {
        if (!rest.endsWith("',")) throw new Error(`multi-line quoted string in ${name}.${key}`);
        entry[key] = unescapeString(rest.slice(1, -2));
      } else if (rest.startsWith("`")) {
        let body = rest;
        while (body.endsWith("␊")) {
          i++;
          body += `\n${lines[i].replace(/^      /u, "")}`;
        }
        if (!body.endsWith("`,")) throw new Error(`unterminated template in ${name}.${key}`);
        entry[key] = unescapeString(body.slice(1, -2).replace(/␊\n/gu, "\n"));
      } else {
        throw new Error(`unrecognized snapshot value for ${name}.${key}: ${rest}`);
      }
    }
    if (name in expected) throw new Error(`duplicate snapshot ${name}`);
    expected[name] = entry;
  }
  return expected;
}
const snapshots = parseSnapshot(
  fs.readFileSync(path.join(testDir, "snapshots", "markdownlint-cli2-test-exec.mjs.md"), "utf8")
);

for (const s of all) {
  const snap = snapshots[s.name];
  if (snap) {
    if (snap.exitCode !== s.exitCode) throw new Error(`${s.name}: exit code mismatch`);
    s.expected = snap;
  } else {
    s.expected = null;
  }
}
const unused = Object.keys(snapshots).filter((n) => !all.some((s) => s.name === n));
if (unused.length > 0) throw new Error(`snapshots without scenario: ${unused.join(", ")}`);

// --- fixture 복사와 출력 --------------------------------------------------------------------------
const fixtureOut = path.join(outDir, "test");
fs.rmSync(fixtureOut, { recursive: true, force: true });
for (const entry of fs.readdirSync(testDir, { withFileTypes: true })) {
  if (entry.isDirectory() && entry.name !== "snapshots") {
    fs.cpSync(path.join(testDir, entry.name), path.join(fixtureOut, entry.name), { recursive: true });
  }
}
fs.writeFileSync(path.join(outDir, "scenarios.json"), `${JSON.stringify(all, null, 2)}\n`);
console.log(`${all.length} scenarios, ${all.filter((s) => s.expected).length} with snapshots`);
