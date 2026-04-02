mod environments;
mod executor;
mod formatter;
mod handler;
mod parser;
mod variables;

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use handler::{SharedState, State};

const SWITCH_ENV_COMMAND: &str = "rest-client.switchEnvironment";

struct RestClientLsp {
    client: Client,
    state: SharedState,
}

#[tower_lsp::async_trait]
impl LanguageServer for RestClientLsp {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
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
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["{".to_string()]),
                    ..Default::default()
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        handler::SEND_REQUEST_COMMAND.to_string(),
                        SWITCH_ENV_COMMAND.to_string(),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "rest-client-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "rest-client-lsp initialized")
            .await;

        self.refresh_settings().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text.clone();

        {
            let mut state = self.state.write().await;
            state.documents.insert(uri.clone(), text.clone());
            self.sync_file_variables(&mut state, &text);
        }

        self.publish_diagnostics(&uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            {
                let mut state = self.state.write().await;
                state.documents.insert(uri.clone(), text.clone());
                self.sync_file_variables(&mut state, &text);
            }
            self.publish_diagnostics(&uri, &text).await;
        }
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.refresh_settings().await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut state = self.state.write().await;
        state.documents.remove(&params.text_document.uri);
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = &params.text_document.uri;
        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t,
            None => return Ok(None),
        };

        let lenses = handler::code_lenses(uri, text);
        Ok(Some(lenses))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let state = self.state.read().await;
        let text = match state.documents.get(uri) {
            Some(t) => t,
            None => return Ok(None),
        };

        let items = handler::completions_at(text, position, &state.variable_ctx);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        match params.command.as_str() {
            cmd if cmd == handler::SEND_REQUEST_COMMAND => {
                if let (Some(uri_val), Some(line_val)) =
                    (params.arguments.first(), params.arguments.get(1))
                {
                    let uri_str = uri_val.as_str().unwrap_or_default();
                    let line = line_val.as_u64().unwrap_or(0) as usize;

                    if let Ok(uri) = Url::parse(uri_str) {
                        match handler::execute_request(&uri, line, &self.state).await {
                            Ok(response) => {
                                self.show_response(&response).await;
                                return Ok(Some(Value::String(response)));
                            }
                            Err(e) => {
                                self.client
                                    .show_message(
                                        MessageType::ERROR,
                                        format!("Request failed: {e}"),
                                    )
                                    .await;
                                return Ok(Some(Value::String(e)));
                            }
                        }
                    }
                }
            }
            cmd if cmd == SWITCH_ENV_COMMAND => {
                if let Some(env_name) = params.arguments.first().and_then(|v| v.as_str()) {
                    let mut state = self.state.write().await;
                    state.settings.active_environment = Some(env_name.to_string());
                    self.apply_environment_variables(&mut state);

                    self.client
                        .log_message(
                            MessageType::INFO,
                            format!("Switched to environment: {env_name}"),
                        )
                        .await;
                    return Ok(Some(Value::String(env_name.to_string())));
                }
            }
            _ => {}
        }
        Ok(None)
    }
}

impl RestClientLsp {
    async fn show_response(&self, response: &str) {
        let dir = std::env::temp_dir().join("rest-client-zed");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("response.http");
        if std::fs::write(&path, response).is_ok() {
            if let Ok(uri) = Url::from_file_path(&path) {
                let _ = self
                    .client
                    .show_document(ShowDocumentParams {
                        uri,
                        external: Some(false),
                        take_focus: Some(true),
                        selection: None,
                    })
                    .await;
            }
        }
    }

    async fn refresh_settings(&self) {
        let config_item = ConfigurationItem {
            scope_uri: None,
            section: Some("rest-client".to_string()),
        };

        match self.client.configuration(vec![config_item]).await {
            Ok(configs) => {
                let json = Value::Array(configs);
                let settings = environments::parse_settings(&json);

                let env_name = {
                    let mut state = self.state.write().await;
                    state.settings = settings;
                    self.apply_environment_variables(&mut state);
                    state
                        .settings
                        .active_environment
                        .clone()
                        .unwrap_or_else(|| "none".to_string())
                };

                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("Loaded settings, active environment: {env_name}"),
                    )
                    .await;
            }
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("Failed to read settings: {e}"),
                    )
                    .await;
            }
        }
    }

    fn apply_environment_variables(&self, state: &mut State) {
        let env_vars = state.settings.resolved_variables();
        for (k, v) in env_vars {
            state.variable_ctx.variables.insert(k, v);
        }
    }

    fn sync_file_variables(&self, state: &mut State, text: &str) {
        let file = parser::parse(text);
        for (k, v) in file.variables {
            state.variable_ctx.variables.insert(k, v);
        }
    }

    async fn publish_diagnostics(&self, uri: &Url, text: &str) {
        let diags = handler::diagnostics(text);
        self.client
            .publish_diagnostics(uri.clone(), diags, None)
            .await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let state = Arc::new(RwLock::new(State::new()));

    let (service, socket) = LspService::new(|client| RestClientLsp {
        client,
        state: state.clone(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
