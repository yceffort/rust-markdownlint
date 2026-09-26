# LSP 서버 (#183)

`rust-markdownlint server` 는 stdio 로 도는 Language Server Protocol 서버다. Node 없이 바이너리 하나로 편집기에서 저장 전 진단과 quick fix 를 받을 수 있다. 진단 위치와 fix 결과는 CLI 출력, `--fix` 결과와 같다.

## 지원 범위

| 기능 | 메서드 | 비고 |
| --- | --- | --- |
| 진단 | `textDocument/didOpen`, `didChange`, `didSave` | 버퍼 내용을 lint 해 `textDocument/publishDiagnostics` 로 보낸다. 동기화는 전체(FULL) |
| 진단 지우기 | `textDocument/didClose` | 빈 진단 목록을 보내고 문서를 버린다 |
| 설정 변경 감지 | `workspace/didChangeWatchedFiles` | `**/.markdownlint*`, `**/.markdownlint-cli2.*`. 클라이언트가 동적 등록을 지원하면 `client/registerCapability` 로 등록한다. 알림을 받으면 열린 문서를 모두 다시 lint 한다 |
| quick fix | `textDocument/codeAction` | `fixInfo` 가 있는 진단마다 `quickfix` 하나, 파일 전체를 고치는 `source.fixAll` 하나 |

진단 필드는 이렇게 채운다.

- `code`: 규칙 이름 전체 (`MD025/single-title/single-h1`). CLI 가 찍는 문자열과 같다.
- `codeDescription.href`: 규칙 문서 URL.
- `severity`: 설정의 `severity` (error 는 1, warning 은 2).
- `source`: `markdownlint`.
- `message`: 규칙 설명과 detail, context (CLI 가 규칙 이름 뒤에 찍는 부분).
- `range`: `errorRange` 가 있으면 그 열과 길이, 없으면 줄 전체. 열은 UTF-16 단위(`positionEncoding` 은 `utf-16`)라 JS 구현과 같다.

범위 밖: `textDocument/formatting`, 인라인 설정 자동완성, 규칙 hover.

## 설정 해석

설정은 파일 경로 기준 디렉토리 계층을 그대로 따른다 (`.markdownlint-cli2.{jsonc,yaml}`, `.markdownlint.{jsonc,json,yaml,yml}`, `ignores`, `frontMatter`, `noInlineConfig`). 기준 디렉토리는 `initialize` 의 `workspaceFolders` 첫 항목, 없으면 `rootUri`, 그것도 없으면 서버 프로세스의 현재 디렉토리다. `ignores` 로 걸러지는 파일은 진단을 보내지 않는다.

설정은 lint 할 때마다 다시 읽는다. 설정 파일은 작아서 캐시 이득이 없고, 캐시가 없으면 파일이 바뀌어도 낡은 값을 쓸 일이 없다.

## Neovim

Neovim 0.11 이상은 `vim.lsp.config` 로 바로 등록할 수 있다.

```lua
vim.lsp.config("rust_markdownlint", {
  cmd = { "rust-markdownlint", "server" },
  filetypes = { "markdown" },
  root_markers = {
    ".markdownlint-cli2.jsonc",
    ".markdownlint-cli2.yaml",
    ".markdownlint.jsonc",
    ".markdownlint.json",
    ".markdownlint.yaml",
    ".markdownlint.yml",
    ".git",
  },
})
vim.lsp.enable("rust_markdownlint")
```

nvim-lspconfig 를 쓰는 구버전 설정은 `lspconfig.configs` 에 직접 넣는다.

```lua
local configs = require("lspconfig.configs")
local util = require("lspconfig.util")

if not configs.rust_markdownlint then
  configs.rust_markdownlint = {
    default_config = {
      cmd = { "rust-markdownlint", "server" },
      filetypes = { "markdown" },
      root_dir = util.root_pattern(
        ".markdownlint-cli2.jsonc",
        ".markdownlint-cli2.yaml",
        ".markdownlint.jsonc",
        ".markdownlint.json",
        ".markdownlint.yaml",
        ".markdownlint.yml",
        ".git"
      ),
      single_file_support = true,
    },
  }
end

require("lspconfig").rust_markdownlint.setup({})
```

quick fix 는 `vim.lsp.buf.code_action()` 으로 고른다. 저장할 때 파일 전체를 고치려면 이렇게 붙인다.

```lua
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = "*.md",
  callback = function()
    vim.lsp.buf.code_action({
      context = { only = { "source.fixAll" }, diagnostics = {} },
      apply = true,
    })
  end,
})
```

## Helix

`~/.config/helix/languages.toml` 에 서버를 정의하고 markdown 에 붙인다.

```toml
[language-server.rust-markdownlint]
command = "rust-markdownlint"
args = ["server"]

[[language]]
name = "markdown"
language-servers = ["rust-markdownlint"]
```

`hx --health markdown` 으로 서버가 잡혔는지 보고, 진단 위에서 `space` `a` 로 code action 을 연다.

## Zed

Zed 는 settings.json 만으로 새 언어 서버를 등록하지 못한다. [Markdownlint 확장](https://zed.dev/extensions/markdownlint)(vitallium/zed-markdownlint) 을 설치한 뒤, 그 확장이 등록한 `markdownlint` 서버의 바이너리를 이 서버로 바꾼다. 저장할 때 파일 전체를 고치려면 Markdown 의 format 단계에 `source.fixAll` code action 을 건다.

```json
{
  "lsp": {
    "markdownlint": {
      "binary": {
        "path": "/Users/me/.cargo/bin/rust-markdownlint",
        "arguments": ["server"]
      }
    }
  },
  "languages": {
    "Markdown": {
      "format_on_save": "on",
      "code_actions_on_format": { "source.fixAll": true },
      "formatter": []
    }
  }
}
```

- `arguments` 를 빼면 확장이 `--stdio` 를 붙여 실행하므로 서버가 뜨지 않는다.
- 위 `formatter: []` 조합으로 확인했다. prettier 같은 다른 formatter 와 함께 쓰는 조합은 확인하지 않았다.
- 확장 README 의 `lsp.markdownlint.settings` (규칙 on/off) 는 이 서버가 읽지 않는다. 규칙은 CLI 와 같은 설정 파일에서 읽는다.
- 서버가 떴는지는 명령 팔레트의 `dev: open language server logs` 에서 `serverInfo.name` 이 `rust-markdownlint` 인지로 본다.

Zed Preview 1.22.0 에서 진단, quick fix, 저장 시 fixAll 을 확인했다.

## VS Code

VS Code 의 [markdownlint 확장](https://marketplace.visualstudio.com/items?itemName=DavidAnson.vscode-markdownlint) 은 LSP 가 아니라 markdownlint 라이브러리를 직접 불러 쓰므로 바이너리를 바꿀 수 없다. 대신 임의의 LSP 서버를 붙이는 [Generic LSP Client (v2)](https://marketplace.visualstudio.com/items?itemName=zsol.vscode-glspc) 확장으로 이 서버를 띄운다. 이 확장은 서버를 하나만 등록할 수 있다.

```json
{
  "glspc.server.command": "/Users/me/.cargo/bin/rust-markdownlint",
  "glspc.server.commandArguments": ["server"],
  "glspc.server.languageId": ["markdown"],
  "[markdown]": {
    "editor.codeActionsOnSave": { "source.fixAll": "explicit" }
  }
}
```

- `"explicit"` 은 직접 저장할 때만 고친다. `files.autoSave` 로 저장될 때도 고치려면 `"always"` 로 둔다.
- markdownlint 확장이 함께 켜져 있으면 같은 진단이 두 번 뜨고 두 확장이 같은 줄을 고친다. 이 서버를 쓰는 워크스페이스에서는 그 확장을 끈다.
- 서버 stderr 는 출력 패널의 `Generic LSP Client` 채널에 찍힌다.

VS Code 설정은 아직 실제 편집기에서 확인하지 않았다.

## 수동 확인 절차

1. `cargo build --release -p rust-markdownlint-cli` 로 바이너리를 만들고 `PATH` 에 올린다.
2. 설정 파일(`.markdownlint-cli2.jsonc`) 이 있는 저장소에서 markdown 파일을 연다.
3. 줄 끝에 공백 하나를 넣어 MD009 진단이 그 줄, 그 열에 뜨는지 본다. 같은 파일을 `rust-markdownlint <파일>` 로 돌린 결과와 줄, 열, 규칙 이름이 같아야 한다.
4. 그 진단 위에서 code action 을 열어 `Fix: MD009/no-trailing-spaces` 와 `Fix all markdownlint issues` 가 보이는지 본다.
5. `Fix all markdownlint issues` 를 적용한 버퍼가 `rust-markdownlint --fix <파일>` 결과와 같은지 본다.
6. 설정 파일에서 그 규칙을 끄고 저장했을 때 열려 있는 버퍼의 진단이 사라지는지 본다 (편집기가 파일 변경을 감시하는 경우).
7. 편집기를 닫았을 때 `rust-markdownlint server` 프로세스가 남지 않는지 본다.

## 자동 테스트

`crates/cli/tests/lsp.rs` 가 빌드된 바이너리를 띄워 stdio 로 JSON-RPC 를 주고받는다. initialize 응답의 capabilities, didOpen/didChange 진단, 설정 파일 변경 뒤 재검사, code action 의 quickfix 와 fixAll, `shutdown`/`exit` 종료 코드, 모르는 요청의 `MethodNotFound` 를 확인한다. fixture 4개(`markdownlint-json`, `markdownlint-cli2-jsonc`, `config-files`, `config-files/dir2`) 는 LSP 진단의 (줄, 열, 규칙 이름) 집합이 CLI 출력에서 파싱한 집합과 같은지 대조한다.
