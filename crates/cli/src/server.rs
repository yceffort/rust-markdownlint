//! `rust-markdownlint server`: stdio 로 도는 LSP 서버. 진단(`publishDiagnostics`) 과
//! quick fix(`textDocument/codeAction`) 만 제공한다. 로그는 항상 stderr 로만 쓴다.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Result;
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CodeDescription, Diagnostic, DiagnosticSeverity,
    DidChangeTextDocumentParams, DidChangeWatchedFilesRegistrationOptions,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    FileSystemWatcher, GlobPattern, InitializeParams, InitializeResult, NumberOrString, Position,
    PositionEncodingKind, PublishDiagnosticsParams, Range, Registration, RegistrationParams,
    SaveOptions, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncOptions, TextDocumentSyncSaveOptions, TextEdit, Uri, WorkspaceEdit,
};
use rust_markdownlint::error::{LintError, Severity};
use rust_markdownlint::fix::apply_fixes;
use rust_markdownlint::lint::{LintOptions, lint_content};

use crate::argv::Argv;
use crate::dirs::{create_dir_infos, read_base_options};
use crate::output::{error_message, relative_posix};

/// 설정 파일 변경을 지켜볼 glob. 클라이언트가 동적 등록을 지원할 때만 쓴다.
const CONFIG_GLOBS: [&str; 2] = ["**/.markdownlint*", "**/.markdownlint-cli2.*"];

pub fn run() -> Result<i32> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;
    let params: InitializeParams = serde_json::from_value(params)?;
    let base = workspace_root(&params).unwrap_or(std::env::current_dir()?);
    let result = InitializeResult {
        capabilities: capabilities(),
        server_info: Some(ServerInfo {
            name: "rust-markdownlint".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };
    connection.initialize_finish(id, serde_json::to_value(result)?)?;
    if watched_files_supported(&params) {
        register_watched_files(&connection)?;
    }

    let mut server = Server {
        base,
        docs: HashMap::new(),
    };
    server.main_loop(&connection)?;
    // writer 스레드는 sender 가 모두 닫혀야 끝나므로 join 전에 연결을 버린다
    drop(connection);
    io_threads.join()?;
    Ok(0)
}

fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(PositionEncodingKind::UTF16),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(true),
                })),
                ..Default::default()
            },
        )),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![
                CodeActionKind::QUICKFIX,
                CodeActionKind::SOURCE_FIX_ALL,
            ]),
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn workspace_root(params: &InitializeParams) -> Option<PathBuf> {
    let folder = params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .map(|folder| &folder.uri);
    #[allow(deprecated)] // rootUri 는 폐기됐지만 아직 이것만 보내는 클라이언트가 있다
    let uri = folder.or(params.root_uri.as_ref())?;
    uri_to_path(uri)
}

fn watched_files_supported(params: &InitializeParams) -> bool {
    params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|w| w.did_change_watched_files.as_ref())
        .and_then(|w| w.dynamic_registration)
        .unwrap_or(false)
}

fn register_watched_files(connection: &Connection) -> Result<()> {
    let options = DidChangeWatchedFilesRegistrationOptions {
        watchers: CONFIG_GLOBS
            .iter()
            .map(|glob| FileSystemWatcher {
                glob_pattern: GlobPattern::String(glob.to_string()),
                kind: None,
            })
            .collect(),
    };
    let params = RegistrationParams {
        registrations: vec![Registration {
            id: "markdownlint-config".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(serde_json::to_value(options)?),
        }],
    };
    // 클라이언트의 요청 id 와 겹치지 않게 문자열 id 를 쓴다
    let request = Request::new(
        RequestId::from("markdownlint-register".to_string()),
        "client/registerCapability".to_string(),
        params,
    );
    connection.sender.send(Message::Request(request))?;
    Ok(())
}

struct Server {
    /// 설정 계층의 기준 디렉토리 (workspace 루트).
    base: PathBuf,
    /// 열려 있는 문서의 최신 내용.
    docs: HashMap<Uri, String>,
}

impl Server {
    fn main_loop(&mut self, connection: &Connection) -> Result<()> {
        for message in &connection.receiver {
            match message {
                Message::Request(request) => {
                    if connection.handle_shutdown(&request)? {
                        return Ok(());
                    }
                    let response = self.request(request);
                    connection.sender.send(Message::Response(response))?;
                }
                Message::Notification(notification) => {
                    self.notification(connection, notification)?;
                }
                // client/registerCapability 응답 외에는 올 것이 없다
                Message::Response(_) => {}
            }
        }
        Ok(())
    }

    fn request(&self, request: Request) -> Response {
        if request.method != "textDocument/codeAction" {
            return Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unhandled request: {}", request.method),
            );
        }
        match serde_json::from_value::<CodeActionParams>(request.params) {
            Ok(params) => Response::new_ok(request.id, self.code_actions(&params)),
            Err(e) => Response::new_err(request.id, ErrorCode::InvalidParams as i32, e.to_string()),
        }
    }

    fn notification(&mut self, connection: &Connection, notification: Notification) -> Result<()> {
        let params = notification.params;
        match notification.method.as_str() {
            "textDocument/didOpen" => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(params)?;
                let uri = params.text_document.uri;
                self.docs.insert(uri.clone(), params.text_document.text);
                self.publish(connection, &uri)?;
            }
            "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams = serde_json::from_value(params)?;
                let uri = params.text_document.uri;
                // 전체 동기화라 마지막 변경의 text 가 문서 전체다
                if let Some(change) = params.content_changes.into_iter().next_back() {
                    self.docs.insert(uri.clone(), change.text);
                    self.publish(connection, &uri)?;
                }
            }
            "textDocument/didSave" => {
                let params: DidSaveTextDocumentParams = serde_json::from_value(params)?;
                let uri = params.text_document.uri;
                if let Some(text) = params.text {
                    self.docs.insert(uri.clone(), text);
                }
                self.publish(connection, &uri)?;
            }
            "textDocument/didClose" => {
                let params: DidCloseTextDocumentParams = serde_json::from_value(params)?;
                let uri = params.text_document.uri;
                self.docs.remove(&uri);
                self.publish(connection, &uri)?;
            }
            "workspace/didChangeWatchedFiles" => {
                for uri in self.docs.keys().cloned().collect::<Vec<_>>() {
                    self.publish(connection, &uri)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn publish(&self, connection: &Connection, uri: &Uri) -> Result<()> {
        let diagnostics = match self.docs.get(uri) {
            Some(text) => diagnostics(&self.lint(uri, text), text),
            // 닫힌 문서의 진단은 지운다
            None => Vec::new(),
        };
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: None,
        };
        let notification = Notification::new("textDocument/publishDiagnostics".to_string(), params);
        connection
            .sender
            .send(Message::Notification(notification))?;
        Ok(())
    }

    fn lint(&self, uri: &Uri, text: &str) -> Vec<LintError> {
        match self.try_lint(uri, text) {
            Ok(errors) => errors,
            Err(e) => {
                eprintln!("rust-markdownlint: {uri:?}: {e}");
                Vec::new()
            }
        }
    }

    /// 설정은 lint 마다 다시 읽는다. 설정 파일은 작고, 캐시가 없으면 파일이 바뀌어도 낡은 값을 쓸 일이 없다.
    fn try_lint(&self, uri: &Uri, text: &str) -> Result<Vec<LintError>> {
        let Some(path) = uri_to_path(uri) else {
            return Ok(Vec::new());
        };
        let mut warn = |message: &str| eprintln!("{message}");
        let base_options = read_base_options(&self.base, &Argv::default(), &mut warn)?;
        let infos = create_dir_infos(
            &self.base,
            std::slice::from_ref(&path),
            &base_options,
            &mut warn,
        )?;
        // ignores 로 걸러진 파일이면 진단하지 않는다
        let Some(info) = infos
            .iter()
            .find(|info| info.files_after_ignores().contains(&path))
        else {
            return Ok(Vec::new());
        };
        let options = LintOptions {
            config: info.effective_config.as_ref(),
            front_matter: info.options.front_matter.as_deref(),
            no_inline_config: info.options.no_inline_config == Some(true),
        };
        Ok(lint_content(
            &relative_posix(&self.base, &path),
            text,
            &options,
        )?)
    }

    fn code_actions(&self, params: &CodeActionParams) -> Vec<CodeActionOrCommand> {
        let uri = &params.text_document.uri;
        let Some(text) = self.docs.get(uri) else {
            return Vec::new();
        };
        let errors = self.lint(uri, text);
        let mut actions = Vec::new();
        for diagnostic in &params.context.diagnostics {
            // data 는 진단을 만든 lint 결과에서의 색인. 문서가 바뀌었을 수 있으니 줄까지 확인한다.
            let Some(error) = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.as_u64())
                .and_then(|index| errors.get(index as usize))
                .filter(|error| error.fix_info.is_some())
                .filter(|error| error.line_number == diagnostic.range.start.line as usize + 1)
            else {
                continue;
            };
            let fixed = apply_fixes(text, std::slice::from_ref(error));
            if let Some(edit) = text_edit(text, &fixed) {
                actions.push(code_action(
                    format!("Fix: {}", error.rule_names.join("/")),
                    CodeActionKind::QUICKFIX,
                    Some(vec![diagnostic.clone()]),
                    uri,
                    edit,
                ));
            }
        }
        if errors.iter().any(|error| error.fix_info.is_some()) {
            let fixed = apply_fixes(text, &errors);
            if let Some(edit) = text_edit(text, &fixed) {
                actions.push(code_action(
                    "Fix all markdownlint issues".to_string(),
                    CodeActionKind::SOURCE_FIX_ALL,
                    None,
                    uri,
                    edit,
                ));
            }
        }
        actions
    }
}

fn code_action(
    title: String,
    kind: CodeActionKind,
    diagnostics: Option<Vec<Diagnostic>>,
    uri: &Uri,
    edit: TextEdit,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(kind),
        diagnostics,
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn diagnostics(errors: &[LintError], text: &str) -> Vec<Diagnostic> {
    let lines: Vec<&str> = text.split('\n').collect();
    errors
        .iter()
        .enumerate()
        .map(|(index, error)| {
            let line = error.line_number.saturating_sub(1);
            let (start, end) = match error.error_range {
                Some((column, length)) => {
                    (column.saturating_sub(1), column.saturating_sub(1) + length)
                }
                // 범위가 없는 오류는 줄 전체를 가리킨다
                None => (
                    0,
                    lines
                        .get(line)
                        .map_or(0, |line| line.encode_utf16().count()),
                ),
            };
            let line = line as u32;
            Diagnostic {
                range: Range::new(
                    Position::new(line, start as u32),
                    Position::new(line, end as u32),
                ),
                severity: Some(match error.severity {
                    Severity::Error => DiagnosticSeverity::ERROR,
                    Severity::Warning => DiagnosticSeverity::WARNING,
                }),
                code: Some(NumberOrString::String(error.rule_names.join("/"))),
                code_description: Uri::from_str(&error.rule_information)
                    .ok()
                    .map(|href| CodeDescription { href }),
                source: Some("markdownlint".to_string()),
                message: error_message(error),
                data: Some(index.into()),
                ..Default::default()
            }
        })
        .collect()
}

/// 두 텍스트가 다른 구간만 바꾸는 편집 하나. 내용이 같으면 None.
fn text_edit(old: &str, new: &str) -> Option<TextEdit> {
    let (a, b) = (old.as_bytes(), new.as_bytes());
    let mut prefix = 0;
    while prefix < a.len() && prefix < b.len() && a[prefix] == b[prefix] {
        prefix += 1;
    }
    while !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = 0;
    let max = (a.len() - prefix).min(b.len() - prefix);
    while suffix < max && a[a.len() - 1 - suffix] == b[b.len() - 1 - suffix] {
        suffix += 1;
    }
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) {
        suffix -= 1;
    }
    if prefix == a.len() && prefix == b.len() {
        return None;
    }
    Some(TextEdit {
        range: Range::new(
            position_at(old, prefix),
            position_at(old, old.len() - suffix),
        ),
        new_text: new[prefix..new.len() - suffix].to_string(),
    })
}

/// 바이트 오프셋을 LSP 위치(줄 번호, UTF-16 단위 열)로.
fn position_at(text: &str, offset: usize) -> Position {
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    Position::new(line as u32, before[start..].encode_utf16().count() as u32)
}

/// `file:` URI 를 경로로. 클라이언트가 보낸 URI 만 다루므로 퍼센트 디코딩과
/// 윈도우 드라이브 문자(`/C:/x`)만 처리한다.
fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    // 호스트가 있으면 건너뛰고 경로만 쓴다
    let path = percent_decode(&rest[rest.find('/')?..]);
    let path = if cfg!(windows) {
        match path.strip_prefix('/') {
            Some(drive) if drive.as_bytes().get(1) == Some(&b':') => drive.to_string(),
            _ => path,
        }
    } else {
        path
    };
    // 구분자를 플랫폼 형태로 맞춘다 (윈도우의 `C:/x` → `C:\x`)
    Some(Path::new(&path).components().collect())
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%')
            .then(|| bytes.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match hex {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(old: &str, new: &str) -> (Range, String) {
        let edit = text_edit(old, new).unwrap();
        (edit.range, edit.new_text)
    }

    #[test]
    fn text_edit_covers_changed_lines_only() {
        assert_eq!(
            edit("a\nb\nc\n", "a\nB\nc\n"),
            (
                Range::new(Position::new(1, 0), Position::new(1, 1)),
                "B".to_string()
            )
        );
        // 끝에 개행 추가 (MD047)
        assert_eq!(
            edit("a\nb", "a\nb\n"),
            (
                Range::new(Position::new(1, 1), Position::new(1, 1)),
                "\n".to_string()
            )
        );
        // 줄 삭제
        assert_eq!(
            edit("a\nb\nc\n", "a\nc\n"),
            (
                Range::new(Position::new(1, 0), Position::new(2, 0)),
                String::new()
            )
        );
        assert_eq!(text_edit("a\n", "a\n"), None);
    }

    #[test]
    fn text_edit_counts_utf16_units() {
        // "𝄞" 는 UTF-16 두 단위
        let edit = text_edit("𝄞x  \n", "𝄞x\n").unwrap();
        assert_eq!(
            edit.range,
            Range::new(Position::new(0, 3), Position::new(0, 5))
        );
        assert_eq!(edit.new_text, "");
    }

    #[test]
    fn uri_to_path_decodes_percent_escapes() {
        let uri = Uri::from_str("file:///tmp/a%20b/c.md").unwrap();
        assert_eq!(uri_to_path(&uri), Some(PathBuf::from("/tmp/a b/c.md")));
    }
}
