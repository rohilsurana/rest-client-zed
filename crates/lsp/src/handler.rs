use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;

use crate::executor;
use crate::formatter;
use crate::parser;

pub const SEND_REQUEST_COMMAND: &str = "rest-client.sendRequest";

pub struct State {
    pub documents: HashMap<Url, String>,
}

impl State {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }
}

pub type SharedState = Arc<RwLock<State>>;

pub fn code_lenses(uri: &Url, text: &str) -> Vec<CodeLens> {
    let file = parser::parse(text);
    file.requests
        .iter()
        .map(|req| {
            let title = format!("Send Request - {} {}", req.method, req.url);
            CodeLens {
                range: Range {
                    start: Position {
                        line: req.line as u32,
                        character: 0,
                    },
                    end: Position {
                        line: req.line as u32,
                        character: 0,
                    },
                },
                command: Some(Command {
                    title,
                    command: SEND_REQUEST_COMMAND.to_string(),
                    arguments: Some(vec![
                        Value::String(uri.to_string()),
                        Value::Number(req.line.into()),
                    ]),
                }),
                data: None,
            }
        })
        .collect()
}

pub fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let file = parser::parse(text);

    // Check for requests with empty URLs
    for req in &file.requests {
        if req.url.is_empty() {
            diags.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: req.line as u32,
                        character: 0,
                    },
                    end: Position {
                        line: req.line as u32,
                        character: lines.get(req.line).map(|l| l.len() as u32).unwrap_or(0),
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("rest-client".to_string()),
                message: "Request URL is empty".to_string(),
                ..Default::default()
            });
        }
    }

    // Check for sections with no valid request
    let _ = (lines, file);

    diags
}

pub async fn execute_request(
    uri: &Url,
    line: usize,
    state: &SharedState,
) -> Result<String, String> {
    let text = {
        let state = state.read().await;
        state
            .documents
            .get(uri)
            .cloned()
            .ok_or_else(|| "document not found".to_string())?
    };

    let file = parser::parse(&text);
    let request = parser::find_request_at_line(&file, line)
        .ok_or_else(|| format!("no request found at line {line}"))?;

    let response = executor::execute(request, &file.variables).await?;
    Ok(formatter::format_response(&response))
}
