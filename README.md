# REST Client for Zed

Send HTTP requests directly from `.http` / `.rest` files in the [Zed editor](https://zed.dev).

## Features

- Syntax highlighting for `.http` / `.rest` files
- Send Request code lenses (click to execute)
- Variables: file (`@host = example.com`), system (`{{$guid}}`), environment
- Request chaining via named requests (`# @name login`)
- Environment switching via Zed settings
- Slash commands: `/http-send`, `/http-to-curl`
- Autocomplete for `{{variables}}`

## Local Development Setup

### Prerequisites

- [Zed editor](https://zed.dev)
- Rust toolchain with `wasm32-wasip1` target

```sh
rustup target add wasm32-wasip1
```

### Build the LSP

```sh
cargo build -p rest-client-lsp --release
```

Add the binary to your PATH:

```sh
# Option 1: symlink
ln -s $(pwd)/target/release/rest-client-lsp ~/.local/bin/rest-client-lsp

# Option 2: copy
cp target/release/rest-client-lsp ~/.local/bin/
```

### Install the Extension in Zed

1. Open Zed
2. Open the command palette (`Cmd+Shift+P`)
3. Run `zed: install dev extension`
4. Select this project's root directory

Zed will build the WASM extension and load it. Any `.http` or `.rest` file will now get syntax highlighting and LSP features.

### Verify

Create a file `test.http`:

```http
@host = httpbin.org

# @name getTest
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

You should see:
- Syntax highlighting (methods, URLs, headers, variables in different colors)
- "Send Request" code lens above each request
- Variable autocomplete when typing `{{`

## File Format

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

# Use response from named request
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
| `{{$processEnv VAR}}` | OS environment variable |
| `{{$dotenv VAR}}` | Value from `.env` file |
| `{{req.response.body.$.path}}` | JSONPath from named request |
| `{{req.response.headers.Name}}` | Header from named request |

### Annotations

| Annotation | Description |
|------------|-------------|
| `# @name identifier` | Name the request for chaining |
| `# @no-redirect` | Don't follow redirects |
| `# @no-cookie-jar` | Don't use cookie jar |
| `# @note message` | Show confirmation before sending |
| `# @prompt varName description` | Prompt for input |

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
    }
  }
}
```

`$shared` variables are available in all environments. Active environment variables override `$shared`.

## Slash Commands

In Zed's assistant panel:

- `/http-send` — paste an HTTP request to execute it
- `/http-to-curl` — paste an HTTP request to convert it to cURL

## License

MIT
