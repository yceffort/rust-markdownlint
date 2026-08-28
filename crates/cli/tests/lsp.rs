#![cfg(feature = "server")]
//! `rust-markdownlint server` 프로토콜 테스트: 빌드된 바이너리를 띄워 stdio 로 JSON-RPC 를 주고받는다.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use regex::Regex;
use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(60);

struct Lsp {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
}

impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

impl Lsp {
    /// 서버를 띄우고 initialize 까지 끝낸다.
    fn start(root: &Path) -> Lsp {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rust-markdownlint"))
            .arg("server")
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        // 읽기를 스레드에 맡겨야 응답을 기다리다 멈추지 않는다
        let (sender, messages) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        let mut lsp = Lsp {
            child,
            stdin,
            messages,
        };
        lsp.send(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": path_uri(root),
                "capabilities": { "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } } },
            }
        }));
        let response = lsp.wait(|message| message["id"] == 1 && message["method"].is_null());
        let capabilities = &response["result"]["capabilities"];
        assert_eq!(capabilities["positionEncoding"], "utf-16");
        assert_eq!(capabilities["textDocumentSync"]["change"], 1);
        assert_eq!(
            capabilities["codeActionProvider"]["codeActionKinds"],
            json!(["quickfix", "source.fixAll"])
        );
        lsp.send(json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }));
        lsp
    }

    fn send(&mut self, message: Value) {
        let body = serde_json::to_vec(&message).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn wait(&self, matches: impl Fn(&Value) -> bool) -> Value {
        loop {
            let message = self
                .messages
                .recv_timeout(TIMEOUT)
                .expect("server stopped sending messages");
            if matches(&message) {
                return message;
            }
        }
    }

    fn diagnostics(&self, uri: &str) -> Vec<Value> {
        let message = self.wait(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == uri
        });
        message["params"]["diagnostics"].as_array().unwrap().clone()
    }

    fn did_open(&mut self, uri: &str, text: &str) -> Vec<Value> {
        self.send(json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "languageId": "markdown", "version": 1, "text": text } }
        }));
        self.diagnostics(uri)
    }

    fn did_change(&mut self, uri: &str, text: &str) -> Vec<Value> {
        self.send(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": text }],
            }
        }));
        self.diagnostics(uri)
    }

    fn code_actions(&mut self, uri: &str, diagnostics: &[Value]) -> Vec<Value> {
        self.send(json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction",
            "params": {
                "textDocument": { "uri": uri },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                "context": { "diagnostics": diagnostics },
            }
        }));
        let response = self.wait(|message| message["id"] == 2 && message["method"].is_null());
        response["result"].as_array().unwrap().clone()
    }
}

fn read_message(reader: &mut BufReader<ChildStdout>) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = value.parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

/// 경로를 `file:` URI 로. 임시 디렉토리와 fixture 경로에 필요한 만큼만 인코딩한다.
fn path_uri(path: &Path) -> String {
    let path = path
        .to_str()
        .unwrap()
        .replace('\\', "/")
        .replace(' ', "%20");
    match path.starts_with('/') {
        true => format!("file://{path}"),
        false => format!("file:///{path}"),
    }
}

/// LSP 위치(줄, UTF-16 열)를 바이트 오프셋으로.
fn offset(text: &str, position: &Value) -> usize {
    let line = position["line"].as_u64().unwrap() as usize;
    let character = position["character"].as_u64().unwrap() as usize;
    let start: usize = text.split_inclusive('\n').take(line).map(str::len).sum();
    let mut units = 0;
    let mut bytes = 0;
    for c in text[start..].chars() {
        if units >= character {
            break;
        }
        units += c.len_utf16();
        bytes += c.len_utf8();
    }
    start + bytes
}

fn apply_edit(text: &str, edit: &Value) -> String {
    let start = offset(text, &edit["range"]["start"]);
    let end = offset(text, &edit["range"]["end"]);
    format!(
        "{}{}{}",
        &text[..start],
        edit["newText"].as_str().unwrap(),
        &text[end..]
    )
}

fn only_edit(action: &Value, uri: &str) -> Value {
    let edits = action["edit"]["changes"][uri].as_array().unwrap();
    assert_eq!(edits.len(), 1);
    edits[0].clone()
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// CLI 기본 출력에서 (줄, 열, 규칙) 을 뽑는다. 열이 없는 줄은 0.
fn cli_findings(root: &Path, file: &str) -> BTreeSet<(usize, usize, String)> {
    let output = Command::new(env!("CARGO_BIN_EXE_rust-markdownlint"))
        .arg(file)
        .current_dir(root)
        .output()
        .unwrap();
    // 기본 포매터는 원본처럼 결과를 stderr 로 쓴다
    let stderr = String::from_utf8(output.stderr).unwrap();
    let line = Regex::new(r"(?m)^\S+?:(\d+)(?::(\d+))? (?:error|warning) (\S+) ").unwrap();
    line.captures_iter(&stderr)
        .map(|c| {
            (
                c[1].parse().unwrap(),
                c.get(2).map_or(0, |m| m.as_str().parse().unwrap()),
                c[3].to_string(),
            )
        })
        .collect()
}

/// LSP 진단에서 같은 값을 뽑는다. 줄 전체를 가리키는 진단은 CLI 가 열을 찍지 않으므로 0.
fn lsp_findings(diagnostics: &[Value], text: &str) -> BTreeSet<(usize, usize, String)> {
    let lines: Vec<&str> = text.split('\n').collect();
    diagnostics
        .iter()
        .map(|diagnostic| {
            let range = &diagnostic["range"];
            let line = range["start"]["line"].as_u64().unwrap() as usize;
            let start = range["start"]["character"].as_u64().unwrap() as usize;
            let end = range["end"]["character"].as_u64().unwrap() as usize;
            let whole_line = start == 0 && end == lines[line].encode_utf16().count();
            (
                line + 1,
                if whole_line { 0 } else { start + 1 },
                diagnostic["code"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn tree(entries: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in entries {
        std::fs::write(dir.path().join(path), content).unwrap();
    }
    dir
}

#[test]
fn publishes_diagnostics_for_the_open_document() {
    // 설정을 읽는지 보려고 MD047 을 끈다
    let dir = tree(&[
        (
            ".markdownlint-cli2.jsonc",
            "{ \"config\": { \"MD047\": false } }",
        ),
        ("doc.md", ""),
    ]);
    let mut lsp = Lsp::start(dir.path());
    let uri = path_uri(&dir.path().join("doc.md"));

    let diagnostics = lsp.did_open(&uri, "# Title\n\ntrailing \n\nlast");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], "MD009/no-trailing-spaces");
    assert_eq!(diagnostic["severity"], 1);
    assert_eq!(diagnostic["source"], "markdownlint");
    assert_eq!(
        diagnostic["message"],
        "Trailing spaces [Expected: 0 or 2; Actual: 1]"
    );
    assert_eq!(
        diagnostic["codeDescription"]["href"],
        "https://github.com/DavidAnson/markdownlint/blob/v0.40.0/doc/md009.md"
    );
    assert_eq!(
        diagnostic["range"],
        json!({ "start": { "line": 2, "character": 8 }, "end": { "line": 2, "character": 9 } })
    );

    // 고친 내용을 보내면 진단이 사라지고, 되돌리면 다시 나온다
    assert!(
        lsp.did_change(&uri, "# Title\n\ntrailing\n\nlast")
            .is_empty()
    );
    assert_eq!(
        lsp.did_change(&uri, "# Title\n\ntrailing \n\nlast").len(),
        1
    );

    // 설정 파일이 바뀌면 열린 문서를 다시 lint 한다
    let config = dir.path().join(".markdownlint-cli2.jsonc");
    std::fs::write(
        &config,
        "{ \"config\": { \"MD047\": false, \"MD009\": false } }",
    )
    .unwrap();
    lsp.send(json!({
        "jsonrpc": "2.0", "method": "workspace/didChangeWatchedFiles",
        "params": { "changes": [{ "uri": path_uri(&config), "type": 2 }] }
    }));
    assert!(lsp.diagnostics(&uri).is_empty());
}

#[test]
fn code_actions_fix_the_same_way_as_the_cli() {
    let text = "#  Title\n\ntrailing  \n\n#  Another";
    let dir = tree(&[
        (
            ".markdownlint-cli2.jsonc",
            "{ \"config\": { \"MD025\": false } }",
        ),
        ("doc.md", text),
    ]);
    let mut lsp = Lsp::start(dir.path());
    let uri = path_uri(&dir.path().join("doc.md"));
    let diagnostics = lsp.did_open(&uri, text);
    assert!(diagnostics.len() >= 3, "{diagnostics:#?}");

    let actions = lsp.code_actions(&uri, &diagnostics);
    let quickfixes: Vec<&Value> = actions.iter().filter(|a| a["kind"] == "quickfix").collect();
    assert_eq!(quickfixes.len(), diagnostics.len());
    for quickfix in &quickfixes {
        let code = quickfix["diagnostics"][0]["code"].as_str().unwrap();
        assert_eq!(quickfix["title"], format!("Fix: {code}"));
        assert_ne!(apply_edit(text, &only_edit(quickfix, &uri)), text);
    }

    let fix_all: Vec<&Value> = actions
        .iter()
        .filter(|a| a["kind"] == "source.fixAll")
        .collect();
    assert_eq!(fix_all.len(), 1);
    assert_eq!(fix_all[0]["title"], "Fix all markdownlint issues");
    let fixed = apply_edit(text, &only_edit(fix_all[0], &uri));

    // `--fix` 와 같은 결과여야 한다
    Command::new(env!("CARGO_BIN_EXE_rust-markdownlint"))
        .args(["--fix", "doc.md"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(
        fixed,
        std::fs::read_to_string(dir.path().join("doc.md")).unwrap()
    );
}

#[test]
fn diagnostics_match_the_cli_output() {
    let files = [
        ("markdownlint-json", "viewme.md"),
        ("markdownlint-cli2-jsonc", "viewme.md"),
        ("config-files", "viewme.md"),
        ("config-files", "dir2/viewme.md"),
    ];
    for (root, file) in files {
        let root = fixture(root);
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        let mut lsp = Lsp::start(&root);
        let uri = path_uri(&root.join(file));
        let diagnostics = lsp.did_open(&uri, &text);
        assert_eq!(
            lsp_findings(&diagnostics, &text),
            cli_findings(&root, file),
            "{}/{file}",
            root.display()
        );
    }
}

#[test]
fn shutdown_and_exit() {
    let dir = tree(&[("doc.md", "# Title\n")]);
    let mut lsp = Lsp::start(dir.path());
    lsp.send(json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null }));
    let response = lsp.wait(|message| message["id"] == 3 && message["method"].is_null());
    assert!(response["error"].is_null());
    lsp.send(json!({ "jsonrpc": "2.0", "method": "exit", "params": null }));
    assert!(lsp.child.wait().unwrap().success());
}

#[test]
fn unknown_request_is_method_not_found() {
    let dir = tree(&[("doc.md", "# Title\n")]);
    let mut lsp = Lsp::start(dir.path());
    lsp.send(json!({ "jsonrpc": "2.0", "id": 4, "method": "textDocument/hover", "params": {} }));
    let response = lsp.wait(|message| message["id"] == 4 && message["method"].is_null());
    assert_eq!(response["error"]["code"], -32601);
}
