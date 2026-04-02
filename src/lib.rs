use zed_extension_api::{
    self as zed,
    http_client::{HttpMethod, HttpRequest},
    Result, SlashCommand, SlashCommandOutput, SlashCommandOutputSection,
};

const LSP_REPO: &str = "rohilsurana/rest-client-zed";
const LSP_BINARY: &str = "rest-client-lsp";

struct RestClientExtension {
    cached_binary_path: Option<String>,
}

impl zed::Extension for RestClientExtension {
    fn new() -> Self {
        RestClientExtension {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        if let Some(path) = &self.cached_binary_path {
            if std::fs::metadata(path).is_ok() {
                return Ok(zed::Command {
                    command: path.clone(),
                    args: vec![],
                    env: worktree.shell_env(),
                });
            }
        }

        if let Some(path) = worktree.which(LSP_BINARY) {
            self.cached_binary_path = Some(path.clone());
            return Ok(zed::Command {
                command: path,
                args: vec![],
                env: worktree.shell_env(),
            });
        }

        let path = self.download_lsp_binary()?;
        self.cached_binary_path = Some(path.clone());

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }

    fn run_slash_command(
        &self,
        command: SlashCommand,
        args: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> Result<SlashCommandOutput> {
        match command.name.as_str() {
            "http-send" => self.run_http_send(&args),
            "http-to-curl" => self.run_http_to_curl(&args),
            _ => Err(format!("unknown command: {}", command.name)),
        }
    }
}

impl RestClientExtension {
    fn run_http_send(&self, args: &[String]) -> Result<SlashCommandOutput> {
        let input = args.join(" ");
        let parsed = parse_request(&input)?;

        let method = match parsed.method.to_uppercase().as_str() {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            "PATCH" => HttpMethod::Patch,
            "HEAD" => HttpMethod::Head,
            "OPTIONS" => HttpMethod::Options,
            _ => return Err(format!("unsupported method: {}", parsed.method)),
        };

        let mut builder = HttpRequest::builder().method(method).url(&parsed.url);
        for (name, value) in &parsed.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &parsed.body {
            builder = builder.body(body.as_bytes().to_vec());
        }
        let request = builder.build()?;

        let response = request
            .fetch()
            .map_err(|e| format!("request failed: {e}"))?;

        let mut text = String::new();
        text.push_str("Response Headers:\n");
        for (name, value) in &response.headers {
            text.push_str(&format!("{name}: {value}\n"));
        }
        text.push('\n');

        let body_str = String::from_utf8_lossy(&response.body);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body_str) {
            if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                text.push_str(&pretty);
            } else {
                text.push_str(&body_str);
            }
        } else {
            text.push_str(&body_str);
        }

        let len = text.len();
        Ok(SlashCommandOutput {
            text,
            sections: vec![SlashCommandOutputSection {
                range: zed::Range {
                    start: 0,
                    end: len as u32,
                },
                label: format!("{} {}", parsed.method, parsed.url),
            }],
        })
    }

    fn run_http_to_curl(&self, args: &[String]) -> Result<SlashCommandOutput> {
        let input = args.join(" ");
        let parsed = parse_request(&input)?;

        let mut curl = format!("curl -X {} '{}'", parsed.method, parsed.url);
        for (name, value) in &parsed.headers {
            curl.push_str(&format!(" \\\n  -H '{name}: {value}'"));
        }
        if let Some(body) = &parsed.body {
            let escaped = body.replace('\'', "'\\''");
            curl.push_str(&format!(" \\\n  -d '{escaped}'"));
        }

        let len = curl.len();
        Ok(SlashCommandOutput {
            text: curl,
            sections: vec![SlashCommandOutputSection {
                range: zed::Range {
                    start: 0,
                    end: len as u32,
                },
                label: format!("cURL: {} {}", parsed.method, parsed.url),
            }],
        })
    }

    fn download_lsp_binary(&self) -> Result<String> {
        let release = zed::latest_github_release(
            LSP_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();
        let target = match (platform, arch) {
            (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
            (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin",
            (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu",
            (zed::Os::Windows, zed::Architecture::X8664) => "x86_64-pc-windows-msvc",
            _ => return Err("unsupported platform".into()),
        };

        let ext = if platform == zed::Os::Windows {
            "zip"
        } else {
            "tar.gz"
        };

        let asset_name = format!("{LSP_BINARY}-{target}.{ext}");
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("no asset found: {asset_name}"))?;

        let version = release
            .version
            .strip_prefix("lsp-")
            .unwrap_or(&release.version);
        let version_dir = format!("rest-client-lsp-{version}");
        let binary_name = if platform == zed::Os::Windows {
            format!("{LSP_BINARY}.exe")
        } else {
            LSP_BINARY.to_string()
        };
        let binary_path = format!("{version_dir}/{binary_name}");

        if std::fs::metadata(&binary_path).is_err() {
            std::fs::create_dir_all(&version_dir).map_err(|e| format!("create dir: {e}"))?;

            zed::download_file(
                &asset.download_url,
                &version_dir,
                if platform == zed::Os::Windows {
                    zed::DownloadedFileType::Zip
                } else {
                    zed::DownloadedFileType::GzipTar
                },
            )
            .map_err(|e| format!("download: {e}"))?;

            if platform != zed::Os::Windows {
                zed::make_file_executable(&binary_path).map_err(|e| format!("chmod: {e}"))?;
            }
        }

        Ok(binary_path)
    }
}

struct ParsedRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

fn parse_request(input: &str) -> Result<ParsedRequest> {
    let lines: Vec<&str> = input.lines().collect();
    if lines.is_empty() {
        return Err("empty request".into());
    }

    let mut i = 0;

    // Skip blank lines and comments
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//") {
            break;
        }
        i += 1;
    }

    if i >= lines.len() {
        return Err("no request line found".into());
    }

    // Parse request line
    let parts: Vec<&str> = lines[i].trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err("invalid request line, expected: METHOD URL".into());
    }
    let method = parts[0].to_string();
    let url = parts[1].to_string();
    i += 1;

    // Parse headers
    let mut headers = Vec::new();
    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() {
            i += 1;
            break;
        }
        if let Some(pos) = line.find(':') {
            headers.push((
                line[..pos].trim().to_string(),
                line[pos + 1..].trim().to_string(),
            ));
        }
        i += 1;
    }

    // Parse body
    let body = if i < lines.len() {
        let body_text = lines[i..].join("\n").trim_end().to_string();
        if body_text.is_empty() {
            None
        } else {
            Some(body_text)
        }
    } else {
        None
    };

    Ok(ParsedRequest {
        method,
        url,
        headers,
        body,
    })
}

zed::register_extension!(RestClientExtension);
