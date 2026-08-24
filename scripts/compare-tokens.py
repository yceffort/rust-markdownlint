#!/usr/bin/env python3
"""Rust 토큰 트리를 markdownlint(micromark JS) 덤프와 대조한다.

사용법: scripts/compare-tokens.py <oracle_dir> [--show N] [--file NAME]
  oracle_dir 에는 js/<name>.md.json 과 markdownlint/test/<name>.md 가 있어야 한다.
  JS 덤프는 oracle_dir/dump-tokens.mjs 로 생성한다.
"""
import collections, glob, json, os, re, subprocess, sys

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
oracle = sys.argv[1]
show = int(sys.argv[sys.argv.index("--show") + 1]) if "--show" in sys.argv else 20
only = sys.argv[sys.argv.index("--file") + 1] if "--file" in sys.argv else None

# 덤프 바이너리 빌드
out = subprocess.run(["cargo", "test", "-q", "-p", "rust-markdownlint", "--test", "dump_tokens", "--no-run", "--message-format=json"],
                     cwd=root, capture_output=True, text=True, check=True).stdout
binary = None
for line in out.splitlines():
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and "dump_tokens" in m["executable"]:
        binary = m["executable"]
assert binary, "dump_tokens binary not found"

rs_dir = os.path.join(oracle, "rs"); os.makedirs(rs_dir, exist_ok=True)

def norm_js(nodes):
    """directive 확장은 markdown-rs 에 없으므로 directiveText 를 data 로 취급하고 인접 data 를 병합."""
    out = []
    for n in nodes:
        t = n["t"]
        if t.startswith("directive"):
            n = {"t": "data", "s": n["s"], "e": n["e"], "c": []}
            t = "data"
        c = norm_js(n["c"])
        if t == "data" and out and out[-1]["t"] == "data" and out[-1]["e"] == n["s"]:
            out[-1] = {"t": "data", "s": out[-1]["s"], "e": n["e"], "c": []}
            continue
        out.append({"t": t, "s": n["s"], "e": n["e"], "c": c})
    return out

def first_diff(a, b, path=""):
    for i, (x, y) in enumerate(zip(a, b)):
        if (x["t"], x["s"], x["e"]) != (y["t"], y["s"], y["e"]):
            return f"{path}/{i}: rs={x['t']} {x['s']}-{x['e']} js={y['t']} {y['s']}-{y['e']}"
        d = first_diff(x["c"], y["c"], f"{path}/{x['t']}")
        if d:
            return d
    if len(a) != len(b):
        nxt = a[len(b)] if len(a) > len(b) else b[len(a)]
        who = "rs extra" if len(a) > len(b) else "js extra"
        return f"{path}: {who} {nxt['t']} {nxt['s']}-{nxt['e']}"
    return None

ok = 0; total = 0; bad = collections.Counter(); ex = {}; directive_files = 0
for f in sorted(glob.glob(os.path.join(oracle, "js", "*.md.json"))):
    name = os.path.basename(f)[:-5]
    if only and name != only:
        continue
    src = os.path.join(oracle, "markdownlint", "test", name)
    if not os.path.exists(src):
        continue
    js_raw = json.load(open(f))
    if "directive" in json.dumps(js_raw):
        directive_files += 1
    rs_out = os.path.join(rs_dir, name + ".json")
    r = subprocess.run([binary, "-q"], env={**os.environ, "DUMP_IN": src, "DUMP_OUT": rs_out}, capture_output=True, text=True)
    total += 1
    if r.returncode != 0:
        bad["RUST PANIC"] += 1; ex.setdefault("RUST PANIC", name); continue
    d = first_diff(json.load(open(rs_out)), norm_js(js_raw))
    if d is None:
        ok += 1
    else:
        key = re.sub(r"\[\d+, \d+\]", "[]", d.split(": ", 1)[1])
        key = re.sub(r"^.*?: ", "", key)
        bad[key] += 1; ex.setdefault(key, f"{name} :: {d}")
print(f"match {ok}/{total}  (directive files: {directive_files})")
for k, v in bad.most_common(show):
    print(f"{v:4d} {k}\n       e.g. {ex[k]}")
