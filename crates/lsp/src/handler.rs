use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;

use crate::executor;
use crate::formatter;
use crate::parser;
use crate::variables::{self, NamedResponse, VariableContext};

pub const SEND_REQUEST_COMMAND: &str = "rest-client.sendRequest";

pub struct State {
    pub documents: HashMap<Url, String>,
    pub variable_ctx: VariableContext,
}

impl State {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            variable_ctx: VariableContext::new(HashMap::new()),
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

    let _ = (lines, file);

    diags
}

pub fn completions_at(
    text: &str,
    position: Position,
    ctx: &VariableContext,
) -> Vec<CompletionItem> {
    let lines: Vec<&str> = text.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return vec![];
    }

    let line = lines[line_idx];
    let col = position.character as usize;
    let before_cursor = if col <= line.len() {
        &line[..col]
    } else {
        line
    };

    // Check if we're inside {{ }}
    if let Some(open) = before_cursor.rfind("{{") {
        let after_open = &before_cursor[open + 2..];
        if !after_open.contains("}}") {
            // We're inside a variable reference
            let prefix = after_open.trim();
            let vars = variables::available_variables(ctx);
            return vars
                .into_iter()
                .filter(|v| prefix.is_empty() || v.starts_with(prefix))
                .map(|name| CompletionItem {
                    label: name.clone(),
                    kind: Some(if name.starts_with('$') {
                        CompletionItemKind::FUNCTION
                    } else {
                        CompletionItemKind::VARIABLE
                    }),
                    detail: Some("variable".to_string()),
                    ..Default::default()
                })
                .collect();
        }
    }

    vec![]
}

pub async fn execute_request(
    uri: &Url,
    line: usize,
    state: &SharedState,
) -> Result<String, String> {
    let (text, mut ctx) = {
        let state = state.read().await;
        let text = state
            .documents
            .get(uri)
            .cloned()
            .ok_or_else(|| "document not found".to_string())?;
        let ctx = VariableContext::new(state.variable_ctx.variables.clone());
        (text, ctx)
    };

    let file = parser::parse(&text);

    // Merge file variables into context
    for (k, v) in &file.variables {
        ctx.variables.insert(k.clone(), v.clone());
    }

    // Copy named responses from shared state
    {
        let state = state.read().await;
        for (k, v) in &state.variable_ctx.named_responses {
            ctx.named_responses.insert(
                k.clone(),
                NamedResponse {
                    headers: v.headers.clone(),
                    body: v.body.clone(),
                },
            );
        }
    }

    let request = parser::find_request_at_line(&file, line)
        .ok_or_else(|| format!("no request found at line {line}"))?;

    let response = executor::execute(request, &ctx).await?;
    let formatted = formatter::format_response(&response);

    // Store named response if request has a name
    if let Some(name) = &request.name {
        let mut state = state.write().await;
        state.variable_ctx.store_response(
            name,
            NamedResponse {
                headers: response.headers.clone(),
                body: response.body.clone(),
            },
        );
    }

    Ok(formatted)
}
