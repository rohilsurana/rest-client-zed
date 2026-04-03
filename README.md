# REST Client for Zed

Send HTTP requests directly from `.http` / `.rest` files in the [Zed editor](https://zed.dev).

Inspired by the popular [VS Code REST Client](https://github.com/Huachao/vscode-restclient).

## Features

- Syntax highlighting for `.http` / `.rest` files (custom tree-sitter grammar)
- Send requests via gutter play button with full HTTP response
- Variables: file (`@host = example.com`), system (`{{$guid}}`), environment
- Request chaining via named requests (`# @name login`)
- Environment switching via Zed settings
- Multiline URL and header support
- Multipart form-data with file references
- cURL import/export and code generation (Python, JavaScript, Go)
- Basic auth auto-encoding
- Go to definition (`Cmd+click`) and hover for variables
- Request history
- Autocomplete for `{{variables}}`

## Installation

### From Zed Extensions (once published)

`Cmd+Shift+P` → `zed: extensions` → search "REST Client" → Install

### Local Development

Prerequisites: Rust toolchain with `wasm32-wasip1` target

```sh
rustup target add wasm32-wasip1
```

Build the LSP and add to PATH:

```sh
cd crates/lsp && cargo build --release
ln -sf $(pwd)/target/release/rest-client-lsp ~/.cargo/bin/rest-client-lsp
```

Install the extension in Zed:

1. `Cmd+Shift+P` → `zed: install dev extension`
2. Select this project's root directory

## Quick Start

Create a file `test.http`:

```http
@host = httpbin.org

GET https://{{host}}/get HTTP/1.1
Accept: application/json

###

POST https://{{host}}/post HTTP/1.1
Content-Type: application/json

{
  "message": "hello",
  "id": "{{$guid}}"
}
```

Click the **play button** (▶) in the gutter next to a request to execute it. The response appears in the terminal pane with status code, headers, timing, and pretty-printed body.

## File Format

Based on the [JetBrains HTTP Request in Editor spec](https://github.com/JetBrains/http-request-in-editor-spec).

```http
# File variables
@host = api.example.com
@token = my-secret-token

# Comments start with # or //

# Annotations
# @name login
# @no-redirect
POST https://{{host}}/auth/login HTTP/1.1
Content-Type: application/json
Authorization: Bearer {{token}}

{
  "username": "admin",
  "password": "{{$guid}}"
}

### Separator with optional label

# Use response from named request (run login first)
GET https://{{host}}/users
Authorization: Bearer {{login.response.body.$.token}}
```

### Variables

| Syntax | Description |
|--------|-------------|
| `@name = value` | File variable |
| `{{name}}` | Variable reference |
| `{{$guid}}` | UUID v4 |
| `{{$randomInt min max}}` | Random integer |
| `{{$timestamp}}` | Unix epoch seconds |
| `{{$datetime iso8601}}` | ISO 8601 datetime |
| `{{$datetime rfc1123}}` | RFC 1123 datetime |
| `{{$processEnv VAR}}` | OS environment variable (allowlisted) |
| `{{$dotenv VAR}}` | Value from `.env` file |
| `{{req.response.body.$.path}}` | JSONPath from named request |
| `{{req.response.headers.Name}}` | Header from named request |

### Request Chaining

Named requests store their response for use by subsequent requests:

1. Add `# @name login` before a request
2. Execute it (click play button)
3. Reference the response in other requests: `{{login.response.body.$.token}}`

Named responses are **session-scoped** — they clear when the editor restarts.

### Multiline URLs

```http
GET https://example.com
    /api
    /users
    ?page=1
    &limit=10
Accept: application/json
```

### Annotations

| Annotation | Description |
|------------|-------------|
| `# @name identifier` | Name the request for chaining |
| `# @no-redirect` | Don't follow redirects |
| `# @no-cookie-jar` | Don't use cookie jar |
| `# @note message` | Show confirmation before sending |

## Environments

Add to `.zed/settings.json`:

```json
{
  "rest-client": {
    "activeEnvironment": "dev",
    "environments": {
      "$shared": {
        "apiVersion": "v1"
      },
      "dev": {
        "host": "localhost:3000",
        "token": "dev-token"
      },
      "prod": {
        "host": "api.example.com",
        "token": "prod-token"
      }
    },
    "allowedProcessEnvVars": ["HOME", "NODE_ENV"]
  }
}
```

- `$shared` variables are available in all environments
- Active environment variables override `$shared`
- `allowedProcessEnvVars` whitelists which OS environment variables `{{$processEnv}}` can access (empty = all blocked)

## Tasks (Command Palette)

With a `.http` file open, `Cmd+Shift+P` → `task: spawn`:

| Task | Description |
|------|-------------|
| **Send Request** | Execute request at cursor (also via play button) |
| **REST: Copy as cURL** | Convert request to cURL (copies to clipboard) |
| **REST: Generate Python** | Generate Python requests code (clipboard) |
| **REST: Generate JavaScript** | Generate JS fetch code (clipboard) |
| **REST: Generate Go** | Generate Go net/http code (clipboard) |
| **REST: Request History** | Show last 50 requests for current file |

## CLI

The LSP binary also works as a standalone CLI:

```sh
rest-client-lsp --exec file.http 5          # Execute request at line 5
rest-client-lsp --to-curl file.http 5       # Convert to cURL
rest-client-lsp --from-curl "curl -X ..."   # Import cURL to .http format
rest-client-lsp --generate python file.http 5  # Generate Python code
rest-client-lsp --history file.http         # Show request history
```

## Security

- **Path traversal protection**: file references (`< ./file`) restricted to workspace directory
- **Environment variable allowlist**: `{{$processEnv}}` blocked by default, must be allowlisted in settings
- **SSRF warnings**: alerts when targeting localhost, private networks, or cloud metadata endpoints
- **Response size limit**: 10MB max to prevent OOM
- **Response sanitization**: cached response values can't inject `{{variables}}`

## License

MIT
