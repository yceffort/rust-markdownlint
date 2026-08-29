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

Zed 는 settings.json 만으로 새 언어 서버를 등록하지 못한다. markdownlint 계열 언어 서버를 제공하는 확장을 설치한 뒤, 그 서버가 쓰는 바이너리를 이 서버로 바꾸는 형태가 된다. 확장이 등록한 서버 이름을 키로 쓴다.

```json
{
  "lsp": {
    "markdownlint": {
      "binary": {
        "path": "/usr/local/bin/rust-markdownlint",
        "arguments": ["server"]
      }
    }
  }
}
```

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
