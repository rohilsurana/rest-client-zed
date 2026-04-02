use zed_extension_api::{self as zed, Result};

struct RestClientExtension;

impl zed::Extension for RestClientExtension {
    fn new() -> Self {
        RestClientExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree
            .which("rest-client-lsp")
            .ok_or_else(|| "rest-client-lsp not found in PATH".to_string())?;

        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(RestClientExtension);
