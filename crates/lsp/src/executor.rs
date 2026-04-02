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
        req = req.body(body);
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

    let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;

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
