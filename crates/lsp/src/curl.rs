use crate::parser::ParsedRequest;

/// Convert a ParsedRequest to a cURL command string.
pub fn to_curl(request: &ParsedRequest) -> String {
    let mut parts = vec![format!("curl -X {}", request.method)];

    for (name, value) in &request.headers {
        parts.push(format!("  -H '{name}: {value}'"));
    }

    if let Some(body) = &request.body {
        let escaped = body.replace('\'', "'\\''");
        parts.push(format!("  -d '{escaped}'"));
    }

    parts.push(format!("  '{}'", request.url));
    parts.join(" \\\n")
}

/// Parse a cURL command string into .http file format.
pub fn from_curl(curl_cmd: &str) -> Result<String, String> {
    let tokens = tokenize_curl(curl_cmd)?;

    let mut method = None;
    let mut url = None;
    let mut headers = Vec::new();
    let mut body = None;

    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        match token.as_str() {
            "curl" => {}
            "-X" | "--request" => {
                i += 1;
                if i < tokens.len() {
                    method = Some(tokens[i].to_uppercase());
                }
            }
            "-H" | "--header" => {
                i += 1;
                if i < tokens.len() {
                    let header = &tokens[i];
                    headers.push(header.clone());
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" => {
                i += 1;
                if i < tokens.len() {
                    body = Some(tokens[i].clone());
                }
            }
            "--data-urlencode" => {
                i += 1;
                if i < tokens.len() {
                    body = Some(tokens[i].clone());
                    if !headers
                        .iter()
                        .any(|h: &String| h.to_lowercase().starts_with("content-type:"))
                    {
                        headers.push("Content-Type: application/x-www-form-urlencoded".to_string());
                    }
                }
            }
            "-u" | "--user" => {
                i += 1;
                if i < tokens.len() {
                    let encoded = base64_encode(tokens[i].as_bytes());
                    headers.push(format!("Authorization: Basic {encoded}"));
                }
            }
            "-A" | "--user-agent" => {
                i += 1;
                if i < tokens.len() {
                    headers.push(format!("User-Agent: {}", tokens[i]));
                }
            }
            "-L" | "--location" => {
                // Follow redirects — default behavior in .http, nothing to add
            }
            "-k" | "--insecure" | "-s" | "--silent" | "-S" | "--show-error" | "-v"
            | "--verbose" | "-i" | "--include" | "--compressed" => {
                // Flags without values — skip
            }
            "-o" | "--output" | "-w" | "--write-out" | "--connect-timeout" | "-m"
            | "--max-time" => {
                // Flags with values — skip both
                i += 1;
            }
            _ => {
                if !token.starts_with('-') && url.is_none() {
                    url = Some(token.clone());
                }
            }
        }
        i += 1;
    }

    let url = url.ok_or_else(|| "no URL found in cURL command".to_string())?;
    let method = method.unwrap_or_else(|| {
        if body.is_some() {
            "POST".to_string()
        } else {
            "GET".to_string()
        }
    });

    let mut output = format!("{method} {url} HTTP/1.1\n");
    for header in &headers {
        output.push_str(header);
        output.push('\n');
    }

    if let Some(body) = &body {
        output.push('\n');
        // Try to pretty-print JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                output.push_str(&pretty);
            } else {
                output.push_str(body);
            }
        } else {
            output.push_str(body);
        }
        output.push('\n');
    }

    Ok(output)
}

/// Tokenize a cURL command, handling quoted strings and backslash continuations.
fn tokenize_curl(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    // Normalize line continuations
    let normalized = input.replace("\\\n", " ").replace("\\\r\n", " ");
    let chars: Vec<char> = normalized.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let mut token = String::new();

        if chars[i] == '\'' {
            // Single-quoted string
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                token.push(chars[i]);
                i += 1;
            }
            i += 1; // skip closing quote
        } else if chars[i] == '"' {
            // Double-quoted string
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                token.push(chars[i]);
                i += 1;
            }
            i += 1; // skip closing quote
        } else {
            // Unquoted token
            while i < chars.len() && !chars[i].is_whitespace() {
                token.push(chars[i]);
                i += 1;
            }
        }

        if !token.is_empty() {
            tokens.push(token);
        }
    }

    Ok(tokens)
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = if i + 1 < input.len() {
            input[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < input.len() {
            input[i + 2] as u32
        } else {
            0
        };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < input.len() {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if i + 2 < input.len() {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_curl_get() {
        let req = ParsedRequest {
            name: None,
            note: None,
            method: "GET".to_string(),
            url: "https://example.com/api".to_string(),
            headers: vec![("Accept".to_string(), "application/json".to_string())],
            body: None,
            line: 0,
            no_redirect: false,
            no_cookie_jar: false,
        };
        let curl = to_curl(&req);
        assert!(curl.contains("curl -X GET"));
        assert!(curl.contains("-H 'Accept: application/json'"));
        assert!(curl.contains("'https://example.com/api'"));
    }

    #[test]
    fn test_to_curl_post() {
        let req = ParsedRequest {
            name: None,
            note: None,
            method: "POST".to_string(),
            url: "https://example.com/api".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: Some(r#"{"key": "value"}"#.to_string()),
            line: 0,
            no_redirect: false,
            no_cookie_jar: false,
        };
        let curl = to_curl(&req);
        assert!(curl.contains("-d '{\"key\": \"value\"}'"));
    }

    #[test]
    fn test_from_curl_simple() {
        let result = from_curl("curl https://example.com/api").unwrap();
        assert!(result.contains("GET https://example.com/api"));
    }

    #[test]
    fn test_from_curl_post_with_headers() {
        let result = from_curl(
            "curl -X POST https://example.com/api \
             -H 'Content-Type: application/json' \
             -d '{\"key\": \"value\"}'",
        )
        .unwrap();
        assert!(result.contains("POST https://example.com/api"));
        assert!(result.contains("Content-Type: application/json"));
        assert!(result.contains("\"key\": \"value\""));
    }

    #[test]
    fn test_from_curl_with_auth() {
        let result = from_curl("curl -u user:passwd https://example.com/api").unwrap();
        assert!(result.contains("Authorization: Basic dXNlcjpwYXNzd2Q="));
    }

    #[test]
    fn test_from_curl_multiline() {
        let result = from_curl(
            "curl -X POST \\\n\
             -H 'Content-Type: application/json' \\\n\
             -d '{\"a\": 1}' \\\n\
             https://example.com/api",
        )
        .unwrap();
        assert!(result.contains("POST https://example.com/api"));
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b"user:passwd"), "dXNlcjpwYXNzd2Q=");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }
}
