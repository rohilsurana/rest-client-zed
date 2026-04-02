# REST Client for Zed — Implementation Plan

A Zed editor extension providing REST client functionality similar to [vscode-restclient](https://github.com/Huachao/vscode-restclient). Supports `.http`/`.rest` files with syntax highlighting, request execution, variables, environments, and response viewing.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Extension ID | `rest-client` | Clean, no "zed"/"extension" per naming rules |
| Tree-sitter grammar | Custom (`tree-sitter-http-rest`) | Full control over nodes for variables, `@name`, `@prompt`, environments |
| Execution model | Both: slash commands + gutter run buttons | Slash commands for assistant panel, runnables for inline execution |
| LSP | Yes, native Rust LSP from the start | Real status codes, completions, hover, diagnostics, code lenses |
| Environment config | Zed settings-based (`.zed/settings.json`) | Familiar pattern, no extra files |
| Publishing | Local testing first, submit when fully ready | Stronger first impression |
| Extensions fork | Personal GitHub account | Zed staff can push review fixes directly |

## Architecture

```
rest-client-zed/
├── extension.toml                    # Extension manifest
├── Cargo.toml                        # Workspace root
├── crates/
│   ├── extension/                    # WASM extension (cdylib → wasm32-wasip1)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                # zed::Extension trait impl
│   └── lsp/                          # Native LSP server binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs               # LSP entry point (tower-lsp + tokio)
│           ├── handler.rs            # LSP request/notification handlers
│           ├── parser.rs             # HTTP file parser (shared logic)
│           ├── executor.rs           # HTTP request execution (reqwest)
│           ├── variables.rs          # Variable resolution
│           ├── environments.rs       # Environment management
│           └── formatter.rs          # Response formatting
├── tree-sitter-http-rest/            # Custom tree-sitter grammar (git submodule)
│   ├── grammar.js                    # Grammar definition
│   ├── src/
│   │   ├── parser.c                  # Generated parser
│   │   └── ...
│   ├── test/                         # Tree-sitter test corpus
│   │   └── corpus/
│   │       ├── requests.txt
│   │       ├── variables.txt
│   │       ├── headers.txt
│   │       └── ...
│   └── package.json
├── languages/
│   └── http/
│       ├── config.toml               # Language config (.http, .rest)
│       ├── highlights.scm            # Syntax highlighting queries
│       ├── brackets.scm              # Bracket matching
│       ├── outline.scm               # Document outline (request names)
│       ├── indents.scm               # Auto-indentation
│       └── runnables.scm             # Run button per request
├── .github/
│   └── workflows/
│       ├── ci.yml                    # Build + lint + test on push/PR
│       └── release.yml               # Publish to Zed extensions index on tag
├── LICENSE                           # MIT
└── README.md
```

**Two-crate workspace:**
- `crates/extension/` — WASM extension (`cdylib`, `zed_extension_api`). Registers language, slash commands, downloads/launches the LSP binary.
- `crates/lsp/` — Native LSP server (`tower-lsp`, `tokio`, `reqwest`). Handles execution, completions, hover, diagnostics, code lenses. Built per-platform, distributed via GitHub releases. The WASM extension downloads the correct binary at runtime via `latest_github_release()`.

**Custom tree-sitter grammar** (`tree-sitter-http-rest/`):
- Separate repo, added as git submodule
- First-class nodes for: `variable` (`{{...}}`), `system_variable` (`{{$...}}`), `request_name` (`# @name`), `prompt_variable` (`# @prompt`), `request_separator` (`###`), `file_variable` (`@var = val`)
- Enables accurate highlighting, outline, and LSP features without regex hacks

## Prior Art

| Extension | Approach | Limitation |
|-----------|----------|------------|
| [tie304/zed-http](https://github.com/tie304/zed-http) | Grammar only, external CLI for execution | No built-in execution |
| [OgDev-01/zed-restclient](https://github.com/OgDev-01/zed-restclient) | Full WASM + native LSP | Heavy, LSP binary not auto-installable via Zed UI |

Our extension: custom grammar for full `.http` syntax coverage, native LSP for real HTTP execution (with status codes), WASM extension for Zed integration + LSP lifecycle.

---

## Implementation Phases

### Phase 1: Project Scaffolding

- [ ] Initialize git repo
- [ ] Create Cargo workspace with `crates/extension/` and `crates/lsp/`
- [ ] Create `extension.toml` with metadata and language server declaration
- [ ] Create `crates/extension/Cargo.toml` with `zed_extension_api = "0.7.0"`
- [ ] Create `crates/lsp/Cargo.toml` with `tower-lsp`, `tokio`, `reqwest`, `serde`, `serde_json`
- [ ] Create minimal `crates/extension/src/lib.rs` implementing `zed::Extension` with `new()` and `language_server_command()` stub
- [ ] Create minimal `crates/lsp/src/main.rs` with tower-lsp skeleton (initialize, shutdown)
- [ ] Create `languages/http/config.toml` (name: `HTTP`, suffixes: `.http`, `.rest`)
- [ ] Add MIT `LICENSE`
- [ ] **Test:** `cargo build -p extension --target wasm32-wasip1` compiles
- [ ] **Test:** `cargo build -p lsp` compiles native binary
- [ ] **Test:** Load via Zed → Extensions → "Install Dev Extension", `.http` files recognized

### Phase 2: Custom Tree-sitter Grammar

- [ ] Create `tree-sitter-http-rest/` as a separate git repo (will be submodule)
- [ ] Write `grammar.js` with nodes for:
  - `document` (root)
  - `request` (method + url + optional http_version + headers + body)
  - `method` (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS, CONNECT, TRACE)
  - `url`
  - `http_version` (HTTP/1.0, HTTP/1.1, HTTP/2)
  - `header` (header_name `:` header_value)
  - `body` (everything after blank line until next separator)
  - `request_separator` (`###` with optional label)
  - `comment` (`#` or `//`)
  - `variable` (`{{variableName}}`)
  - `system_variable` (`{{$name args}}`)
  - `file_variable` (`@name = value`)
  - `request_name` (`# @name identifier`)
  - `prompt_variable` (`# @prompt name description`)
  - `request_note` (`# @note message`)
  - `request_no_redirect` (`# @no-redirect`)
  - `request_no_cookie_jar` (`# @no-cookie-jar`)
  - `multipart_form_data` (boundary-based)
  - `file_reference` (`< filepath` / `<@ filepath`)
  - `graphql_body` (when `X-Request-Type: GraphQL`)
- [ ] Generate parser: `npx tree-sitter generate`
- [ ] Write test corpus in `test/corpus/`:
  - `requests.txt` — basic GET, POST, PUT, DELETE
  - `headers.txt` — various header formats
  - `body.txt` — JSON, XML, form-urlencoded, multipart
  - `variables.txt` — file vars, template vars, system vars
  - `comments.txt` — line comments, request separators with labels
  - `annotations.txt` — `@name`, `@prompt`, `@note`, `@no-redirect`
  - `edge_cases.txt` — empty body, no headers, query params on multiple lines
- [ ] **Test:** `npx tree-sitter test` — all corpus tests pass
- [ ] **Test:** `npx tree-sitter parse examples/sample.http` — parses without errors
- [ ] Reference grammar in `extension.toml` with pinned rev
- [ ] Add grammar repo as submodule

### Phase 3: Syntax Highlighting & Language Features

- [ ] Write `languages/http/highlights.scm`:
  - `method` → `@keyword`
  - `url` → `@string.special`
  - `http_version` → `@constant`
  - `header_name` → `@property`
  - `header_value` → `@string`
  - `body` → embedded language injection (JSON, XML based on Content-Type)
  - `comment` → `@comment`
  - `variable` → `@variable.special`
  - `system_variable` → `@function.builtin`
  - `file_variable` name → `@variable`, value → `@string`
  - `request_separator` → `@punctuation.delimiter`
  - `request_name` → `@label`
  - `prompt_variable` → `@attribute`
- [ ] Write `languages/http/brackets.scm` for `{}`, `[]`, `()`
- [ ] Write `languages/http/indents.scm` for JSON body indentation
- [ ] Write `languages/http/outline.scm` to show requests (method + URL or @name) in symbol outline
- [ ] Write `languages/http/injections.scm` for embedded JSON/XML in body
- [ ] **Test:** Open a `.http` file in Zed, verify all syntax elements are colored correctly
- [ ] **Test:** Verify outline panel shows request list with method + URL
- [ ] **Test:** Verify bracket matching works in JSON bodies
- [ ] **Test:** Verify JSON body gets JSON highlighting via injection

### Phase 4: Runnables (Gutter Run Buttons)

- [ ] Write `languages/http/runnables.scm` to tag each `request` node with run label
- [ ] Configure runnables to invoke the LSP or an external task
- [ ] **Test:** Gutter "Run" button appears next to each request
- [ ] **Test:** Clicking run triggers request execution

### Phase 5: LSP — Core Request Execution

- [ ] Implement `crates/lsp/src/parser.rs`:
  - Parse `.http` file into list of requests
  - Each request: method, URL, headers, body, name, annotations
  - Handle `###` separators
  - Handle multi-line query parameters (`?` and `&` on separate lines)
- [ ] Implement `crates/lsp/src/executor.rs`:
  - Execute HTTP request via `reqwest`
  - Capture: status code, status text, headers, body, timing
  - Follow redirects (unless `@no-redirect`)
  - Cookie jar support (unless `@no-cookie-jar`)
- [ ] Implement `crates/lsp/src/formatter.rs`:
  - Format response as readable text
  - Pretty-print JSON bodies
  - Format XML bodies
  - Show timing breakdown
- [ ] Implement `crates/lsp/src/handler.rs`:
  - Code Lens: "Send Request" above each request block
  - `executeCommand` for running a request and returning response
  - Diagnostics: malformed request lines, missing URL, invalid headers
- [ ] Wire up in `crates/lsp/src/main.rs`
- [ ] **Test:** Start LSP manually, send initialize request, verify capabilities
- [ ] **Test:** Send Code Lens request for a `.http` file, verify lenses returned
- [ ] **Test:** Execute request command, verify response with real status code
- [ ] **Test:** GET to httpbin.org/get returns 200 with JSON
- [ ] **Test:** POST with JSON body to httpbin.org/post returns correct echo
- [ ] **Test:** Malformed request shows diagnostic

### Phase 6: LSP Binary Distribution + WASM Integration

- [ ] Set up cross-compilation for LSP binary (macOS aarch64/x86_64, Linux x86_64, Windows x86_64)
- [ ] GitHub Actions workflow to build and attach binaries to GitHub releases
- [ ] Implement `language_server_command()` in WASM extension:
  - Use `latest_github_release()` to find latest LSP binary
  - Download correct binary for current platform via `download_file()`
  - Make executable via `make_file_executable()`
  - Return command to launch it
- [ ] Implement `language_server_initialization_options()` for extension settings
- [ ] **Test:** "Install Dev Extension" in Zed, LSP auto-downloads and starts
- [ ] **Test:** Code Lens "Send Request" appears in `.http` file
- [ ] **Test:** Clicking Code Lens executes request, response shown
- [ ] **Test:** Diagnostics appear for malformed requests

### Phase 7: Slash Commands (Assistant Panel Integration)

- [ ] Register slash commands in `extension.toml`: `send-request`, `switch-environment`, `copy-as-curl`, `paste-curl`
- [ ] Implement `run_slash_command()` in WASM extension:
  - `/send-request` — delegate to LSP or use WASM HTTP client as fallback
  - `/switch-environment` — list available environments, switch active
  - `/copy-as-curl` — convert current request to cURL
  - `/paste-curl` — parse cURL into `.http` format
- [ ] Implement `complete_slash_command_argument()` for environment name completion
- [ ] **Test:** `/send-request` in assistant panel triggers request
- [ ] **Test:** `/switch-environment` shows environment list
- [ ] **Test:** `/copy-as-curl` produces valid cURL command

### Phase 8: Variables

- [ ] Implement file variable parsing in LSP (`@variableName = value`)
- [ ] Implement variable substitution in requests (`{{variableName}}`)
- [ ] Implement system variables in LSP:
  - `{{$guid}}` — UUID v4
  - `{{$randomInt min max}}` — random integer
  - `{{$timestamp}}` — UTC epoch
  - `{{$datetime format}}` — formatted datetime
  - `{{$localDatetime format}}` — local datetime
  - `{{$dotenv varName}}` — read from `.env` file
  - `{{$processEnv varName}}` — read from OS env
- [ ] Implement request variables (`# @name requestName`) for response chaining:
  - Store last response per named request in memory
  - Resolve `{{requestName.response.body.$.jsonpath}}`
  - Resolve `{{requestName.response.headers.HeaderName}}`
- [ ] LSP completions: suggest variable names in `{{` context
- [ ] LSP hover: show variable value on hover
- [ ] **Test:** File variable `@host = httpbin.org` substituted in `GET https://{{host}}/get`
- [ ] **Test:** `{{$guid}}` generates a valid UUID in request body
- [ ] **Test:** `{{$timestamp}}` inserts current epoch
- [ ] **Test:** Named request chaining: login response token used in subsequent request
- [ ] **Test:** Autocomplete suggests defined variables inside `{{}}`

### Phase 9: Environments (Zed Settings-based)

- [ ] Read environment config from `.zed/settings.json` under `rest-client.environments`:
  ```json
  {
    "rest-client": {
      "environments": {
        "$shared": { "apiVersion": "v1" },
        "dev": { "host": "localhost:3000", "token": "dev-token" },
        "prod": { "host": "api.example.com", "token": "prod-token" }
      },
      "activeEnvironment": "dev"
    }
  }
  ```
- [ ] LSP reads settings via `workspace/configuration` request
- [ ] Environment variables resolve after file variables, before system variables
- [ ] `$shared` variables available in all environments
- [ ] Environment-specific variables override `$shared`
- [ ] `/switch-environment` slash command updates active environment
- [ ] LSP status: show active environment name
- [ ] **Test:** Switch between `dev` and `prod`, verify URL changes
- [ ] **Test:** `$shared` variables accessible in all environments
- [ ] **Test:** Environment variable overrides shared variable of same name
- [ ] **Test:** Changing environment re-evaluates diagnostics

### Phase 10: Quality of Life Features

- [ ] cURL import: parse cURL syntax into `.http` request format
- [ ] cURL export: convert `.http` request to cURL command
- [ ] Request history: LSP stores last 50 request/response pairs
  - Custom LSP command to list/search history
- [ ] Code generation from request:
  - Python (requests library)
  - JavaScript (fetch API)
  - Go (net/http)
  - cURL
- [ ] **Test:** Paste a complex cURL (with -H, -d, -X), verify valid `.http` output
- [ ] **Test:** Export request as cURL, verify it's executable
- [ ] **Test:** History accessible after executing multiple requests

### Phase 11: Authentication Helpers

- [ ] Basic auth: detect `Authorization: Basic user:password`, auto-encode to base64
- [ ] Bearer token: passthrough (no transformation needed)
- [ ] Digest auth: implement challenge-response flow in LSP executor
- [ ] LSP completion: suggest auth schemes after `Authorization: `
- [ ] **Test:** Basic auth with plain `user:pass` gets base64-encoded before sending
- [ ] **Test:** Bearer token header sent as-is
- [ ] **Test:** Digest auth completes handshake with httpbin.org/digest-auth

---

## CI/CD

### CI Pipeline (`.github/workflows/ci.yml`)

- [ ] On push/PR to main:
  - Build WASM extension: `cargo build -p extension --target wasm32-wasip1 --release`
  - Build LSP: `cargo build -p lsp --release`
  - Lint: `cargo clippy --workspace -- -D warnings`
  - Format: `cargo fmt --check`
  - Test: `cargo test --workspace`
  - Tree-sitter tests: `cd tree-sitter-http-rest && npx tree-sitter test`
- [ ] **Test:** CI passes on push to main

### LSP Binary Release (`.github/workflows/lsp-release.yml`)

- [ ] On tag `lsp-v*`:
  - Cross-compile LSP binary for:
    - `x86_64-apple-darwin`
    - `aarch64-apple-darwin`
    - `x86_64-unknown-linux-gnu`
    - `x86_64-pc-windows-msvc`
  - Create GitHub release with binaries attached
  - Name binaries: `rest-client-lsp-{os}-{arch}` (matching Zed's `current_platform()` output)
- [ ] **Test:** Tag `lsp-v0.1.0`, verify binaries published to release

### Extension Release (`.github/workflows/release.yml`)

- [ ] On tag `v*`:
  - Build WASM extension
  - Use `huacnlee/zed-extension-action@v2` to create PR to extensions index
  - Config:
    - `extension-name: rest-client`
    - `push-to: <personal-account>/extensions` (personal fork)
  - Requires `COMMITTER_TOKEN` secret (PAT with `repo` + `workflow` scopes)
- [ ] **Test:** Tag `v0.1.0`, verify action creates PR to `zed-industries/extensions`

### Publishing Checklist (when ready to submit)

- [ ] Extension has MIT license
- [ ] `extension.toml` has all required fields
- [ ] Extension id is `rest-client` (no "zed"/"extension")
- [ ] Fork `zed-industries/extensions` to personal account
- [ ] Add submodule: `git submodule add https://github.com/rohilsurana/rest-client-zed.git extensions/rest-client`
- [ ] Add to `extensions.toml`: `[rest-client]` with submodule and version
- [ ] Run `pnpm sort-extensions`
- [ ] Open PR
- [ ] Extension appears in Zed marketplace after merge

---

## Milestones

| Milestone | Phases | What Users Get |
|-----------|--------|----------------|
| **v0.1.0** | 1-4 | Custom grammar, syntax highlighting, gutter run buttons |
| **v0.2.0** | 5-7 | LSP with real request execution, code lenses, slash commands, diagnostics |
| **v0.3.0** | 8-9 | Variables, environments, request chaining, completions |
| **v0.4.0** | 10-11 | cURL import/export, history, codegen, auth helpers |

## Notes

- The custom tree-sitter grammar is the foundation — invest time getting it right in Phase 2.
- Native LSP gives us real HTTP status codes (Zed WASM HTTP client doesn't expose them).
- LSP binary distributed via GitHub releases, auto-downloaded by WASM extension at runtime.
- Environments in `.zed/settings.json` keeps config familiar and avoids extra file formats.
- Extension will be tested fully locally before any submission to the Zed extensions index.
