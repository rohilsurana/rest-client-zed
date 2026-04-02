use zed_extension_api::{self as zed, Result};

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
        // Try cached path first
        if let Some(path) = &self.cached_binary_path {
            if std::fs::metadata(path).is_ok() {
                return Ok(zed::Command {
                    command: path.clone(),
                    args: vec![],
                    env: worktree.shell_env(),
                });
            }
        }

        // Try PATH
        if let Some(path) = worktree.which(LSP_BINARY) {
            self.cached_binary_path = Some(path.clone());
            return Ok(zed::Command {
                command: path,
                args: vec![],
                env: worktree.shell_env(),
            });
        }

        // Download from GitHub releases
        let path = self.download_lsp_binary()?;
        self.cached_binary_path = Some(path.clone());

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

impl RestClientExtension {
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

        let version = release.version.strip_prefix("lsp-").unwrap_or(&release.version);
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

zed::register_extension!(RestClientExtension);
