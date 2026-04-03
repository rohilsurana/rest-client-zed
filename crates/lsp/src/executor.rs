use std::time::Instant;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy;

use crate::parser::ParsedRequest;
use crate::variables::{self, VariableContext};

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub elapsed_ms: u128,
}

pub async fn execute(request: &ParsedRequest, ctx: &VariableContext) -> Result<Response, String> {
    let url = variables::resolve(&request.url, ctx);

    // Check for requests targeting internal/private networks
    if let Some(warning) = crate::security::check_url_safety(&url) {
        eprintln!("\x1b[33m⚠ Security: {warning}\x1b[0m");
    }

    let client = build_client(request)?;

    let method: reqwest::Method = request
        .method
        .parse()
        .map_err(|e| format!("invalid method: {e}"))?;

    let mut req = client.request(method, &url);

    let mut headers = HeaderMap::new();
    for (name, value) in &request.headers {
        let value = variables::resolve(value, ctx);
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| format!("invalid header name '{name}': {e}"))?;
        let val =
            HeaderValue::from_str(&value).map_err(|e| format!("invalid header value: {e}"))?;
        headers.insert(name, val);
    }
    req = req.headers(headers);

    if let Some(body) = &request.body {
        let body = variables::resolve(body, ctx);

        // Check if this is a multipart request
        let content_type = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");

        if content_type.contains("multipart/form-data") {
            // Extract boundary from Content-Type header
            let boundary = content_type
                .split("boundary=")
                .nth(1)
                .unwrap_or("boundary")
                .trim();
            let parts = crate::parser::parse_multipart_body(&body, boundary);

            let mut form = reqwest::multipart::Form::new();
            for part in parts {
                let field = match part.data {
                    crate::parser::MultipartData::Text(text) => {
                        let f = reqwest::multipart::Part::text(text);
                        match part.filename {
                            Some(fname) => f.file_name(fname),
                            None => f,
                        }
                    }
                    crate::parser::MultipartData::File(path) => {
                        // Validate file path to prevent path traversal
                        let workspace = std::env::current_dir()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let safe_path = crate::security::validate_file_path(&path, &workspace)?;
                        let file_bytes = std::fs::read(&safe_path)
                            .map_err(|e| format!("read file '{path}': {e}"))?;
                        let fname = part.filename.unwrap_or_else(|| {
                            path.rsplit('/').next().unwrap_or("file").to_string()
                        });
                        reqwest::multipart::Part::bytes(file_bytes).file_name(fname)
                    }
                };
                let field = if let Some(ct) = &part.content_type {
                    match field.mime_str(ct) {
                        Ok(f) => f,
                        Err(_) => reqwest::multipart::Part::text(""),
                    }
                } else {
                    field
                };
                form = form.part(part.field_name, field);
            }
            req = req.multipart(form);
        } else {
            req = req.body(body);
        }
    }

    let start = Instant::now();
    let resp = req
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let elapsed_ms = start.elapsed().as_millis();

    let status = resp.status().as_u16();
    let status_text = resp.status().canonical_reason().unwrap_or("").to_string();

    let resp_headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                v.to_str().unwrap_or("<binary>").to_string(),
            )
        })
        .collect();

    // Limit response body to 10MB to prevent OOM from malicious servers
    const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;
    let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
    if body.len() > MAX_BODY_SIZE {
        return Err(format!(
            "response body too large ({} bytes, max {})",
            body.len(),
            MAX_BODY_SIZE
        ));
    }

    Ok(Response {
        status,
        status_text,
        headers: resp_headers,
        body,
        elapsed_ms,
    })
}

fn build_client(request: &ParsedRequest) -> Result<reqwest::Client, String> {
    let redirect_policy = if request.no_redirect {
        Policy::none()
    } else {
        Policy::limited(10)
    };

    reqwest::Client::builder()
        .redirect(redirect_policy)
        .build()
        .map_err(|e| format!("client build: {e}"))
}
