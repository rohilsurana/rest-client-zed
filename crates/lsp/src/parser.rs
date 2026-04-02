use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HttpFile {
    pub variables: HashMap<String, String>,
    pub requests: Vec<ParsedRequest>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParsedRequest {
    pub name: Option<String>,
    pub note: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub line: usize,
    pub no_redirect: bool,
    pub no_cookie_jar: bool,
}

pub fn parse(text: &str) -> HttpFile {
    let mut variables = HashMap::new();
    let mut requests = Vec::new();

    let sections = split_sections(text);
    for section in sections {
        parse_section(
            &section.text,
            section.start_line,
            &mut variables,
            &mut requests,
        );
    }

    HttpFile {
        variables,
        requests,
    }
}

struct Section {
    text: String,
    start_line: usize,
}

fn split_sections(text: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut start_line = 0;
    for (line_num, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("###") {
            if !current.is_empty() {
                sections.push(Section {
                    text: current,
                    start_line,
                });
                current = String::new();
            }
            start_line = line_num + 1;
        } else {
            if current.is_empty() {
                start_line = line_num;
            }
            current.push_str(line);
            current.push('\n');
        }
    }

    if !current.is_empty() {
        sections.push(Section {
            text: current,
            start_line,
        });
    }

    sections
}

fn parse_section(
    text: &str,
    start_line: usize,
    variables: &mut HashMap<String, String>,
    requests: &mut Vec<ParsedRequest>,
) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    // Skip leading blank lines and comments, collect file variables
    // Stop at annotations (# @name, etc.) — they belong to the next request
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            if try_parse_annotation(trimmed).is_some() {
                break;
            }
            i += 1;
            continue;
        }
        if trimmed.starts_with('@') {
            if let Some((name, value)) = try_parse_file_variable(trimmed) {
                variables.insert(name, value);
                i += 1;
                continue;
            }
        }
        break;
    }

    if i >= lines.len() {
        return;
    }

    // Collect annotations
    let mut name = None;
    let mut note = None;
    let mut no_redirect = false;
    let mut no_cookie_jar = false;

    let annotation_start = i;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if let Some(ann) = try_parse_annotation(trimmed) {
            match ann.0.as_str() {
                "name" => name = ann.1,
                "note" => note = ann.1,
                "no-redirect" => no_redirect = true,
                "no-cookie-jar" => no_cookie_jar = true,
                _ => {}
            }
            i += 1;
        } else {
            break;
        }
    }

    if i >= lines.len() {
        return;
    }

    // Parse request line
    let request_line_num = start_line + i;
    let request_line = lines[i].trim();
    let (method, mut url) = match parse_request_line(request_line) {
        Some(r) => r,
        None => {
            let _ = annotation_start;
            return;
        }
    };
    i += 1;

    // Multiline URL: continuation lines starting with whitespace that look like
    // path segments (/...), query params (?... or &...), or fragments (#...)
    while i < lines.len() {
        let line = lines[i];
        if !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        // URL continuation: starts with /, ?, &, or #
        if trimmed.starts_with('/')
            || trimmed.starts_with('?')
            || trimmed.starts_with('&')
            || trimmed.starts_with('#')
        {
            url.push_str(trimmed);
            i += 1;
        } else {
            break;
        }
    }

    // Parse headers (with multiline value support)
    let mut headers = Vec::new();
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(pos) = trimmed.find(':') {
            let header_name = trimmed[..pos].trim().to_string();
            let header_value = trimmed[pos + 1..].trim().to_string();
            headers.push((header_name, header_value));
            i += 1;

            // Multiline header value: next lines starting with whitespace
            // that don't look like a new header (no colon in non-indented position)
            while i < lines.len() {
                let next = lines[i];
                if (next.starts_with(' ') || next.starts_with('\t')) && !next.trim().is_empty() {
                    // Check it's not a new header (indented lines without ':' at start)
                    let next_trimmed = next.trim();
                    if next_trimmed.contains(':') && !next_trimmed.starts_with(':') {
                        // Could be a new header with odd indentation, be conservative
                        break;
                    }
                    if let Some(last) = headers.last_mut() {
                        last.1.push(' ');
                        last.1.push_str(next_trimmed);
                    }
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            i += 1;
        }
    }

    // Skip blank line before body
    if i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }

    // Parse body (rest of section)
    let body = if i < lines.len() {
        let body_text: String = lines[i..].to_vec().join("\n").trim_end().to_string();
        if body_text.is_empty() {
            None
        } else {
            Some(body_text)
        }
    } else {
        None
    };

    requests.push(ParsedRequest {
        name,
        note,
        method,
        url,
        headers,
        body,
        line: request_line_num,
        no_redirect,
        no_cookie_jar,
    });
}

fn parse_request_line(line: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return None;
    }

    let method = parts[0].to_uppercase();
    let valid_methods = [
        "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "CONNECT", "TRACE",
    ];
    if !valid_methods.contains(&method.as_str()) {
        return None;
    }

    let url = parts[1].to_string();
    Some((method, url))
}

fn try_parse_file_variable(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('@') {
        return None;
    }

    let rest = &trimmed[1..];
    let eq_pos = rest.find('=')?;
    let name = rest[..eq_pos].trim().to_string();
    let value = rest[eq_pos + 1..].trim().to_string();

    if name.is_empty() {
        return None;
    }

    Some((name, value))
}

fn try_parse_annotation(line: &str) -> Option<(String, Option<String>)> {
    let rest = if let Some(r) = line.strip_prefix('#') {
        r.trim()
    } else if let Some(r) = line.strip_prefix("//") {
        r.trim()
    } else {
        return None;
    };

    let rest = rest.strip_prefix('@')?;
    let annotations = ["name", "prompt", "note", "no-redirect", "no-cookie-jar"];

    for ann in &annotations {
        if let Some(after) = rest.strip_prefix(ann) {
            if after.is_empty() || after.starts_with(' ') || after.starts_with('\t') {
                let value = after.trim();
                let value = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
                return Some((ann.to_string(), value));
            }
        }
    }

    None
}

pub fn find_request_at_line(file: &HttpFile, line: usize) -> Option<&ParsedRequest> {
    file.requests.iter().rev().find(|r| r.line <= line)
}

/// Parse a multipart form-data body into individual parts for sending via reqwest.
/// Returns a list of (field_name, filename, content_type, data) tuples.
pub fn parse_multipart_body(body: &str, boundary: &str) -> Vec<MultipartPart> {
    let mut parts = Vec::new();
    let delimiter = format!("--{boundary}");
    let _end_delimiter = format!("--{boundary}--");

    let sections: Vec<&str> = body.split(&delimiter).collect();
    for section in sections {
        let section = section.trim();
        if section.is_empty() || section == "--" || section.starts_with("--") {
            continue;
        }
        // Remove trailing -- if this is the end delimiter
        let section = section.strip_suffix("--").unwrap_or(section).trim();
        if section.is_empty() {
            continue;
        }

        // Split headers from body at blank line
        let (header_part, body_part) = if let Some(pos) = section.find("\n\n") {
            (&section[..pos], section[pos + 2..].trim())
        } else if let Some(pos) = section.find("\r\n\r\n") {
            (&section[..pos], section[pos + 4..].trim())
        } else {
            ("", section)
        };

        let mut field_name = None;
        let mut filename = None;
        let mut content_type = None;

        for header_line in header_part.lines() {
            let header_line = header_line.trim();
            if let Some(rest) = header_line.strip_prefix("Content-Disposition:") {
                let rest = rest.trim();
                // Parse name="value" and filename="value"
                for param in rest.split(';') {
                    let param = param.trim();
                    if let Some(val) = param.strip_prefix("name=") {
                        field_name = Some(unquote(val));
                    } else if let Some(val) = param.strip_prefix("filename=") {
                        filename = Some(unquote(val));
                    }
                }
            } else if let Some(rest) = header_line.strip_prefix("Content-Type:") {
                content_type = Some(rest.trim().to_string());
            }
        }

        let data = if let Some(path) = body_part.strip_prefix("< ") {
            MultipartData::File(path.trim().to_string())
        } else {
            MultipartData::Text(body_part.to_string())
        };

        parts.push(MultipartPart {
            field_name: field_name.unwrap_or_default(),
            filename,
            content_type,
            data,
        });
    }

    parts
}

fn unquote(s: &str) -> String {
    s.trim()
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s.trim())
        .to_string()
}

#[derive(Debug, Clone)]
pub struct MultipartPart {
    pub field_name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: MultipartData,
}

#[derive(Debug, Clone)]
pub enum MultipartData {
    Text(String),
    File(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_get() {
        let input = "GET https://example.com/api\n";
        let file = parse(input);
        assert_eq!(file.requests.len(), 1);
        assert_eq!(file.requests[0].method, "GET");
        assert_eq!(file.requests[0].url, "https://example.com/api");
        assert!(file.requests[0].body.is_none());
    }

    #[test]
    fn test_post_with_body() {
        let input = "\
POST https://example.com/api
Content-Type: application/json

{\"key\": \"value\"}
";
        let file = parse(input);
        assert_eq!(file.requests.len(), 1);
        assert_eq!(file.requests[0].method, "POST");
        assert_eq!(file.requests[0].headers.len(), 1);
        assert_eq!(file.requests[0].headers[0].0, "Content-Type");
        assert_eq!(
            file.requests[0].body.as_deref(),
            Some("{\"key\": \"value\"}")
        );
    }

    #[test]
    fn test_multiple_requests() {
        let input = "\
GET https://example.com/one

###

POST https://example.com/two
";
        let file = parse(input);
        assert_eq!(file.requests.len(), 2);
        assert_eq!(file.requests[0].url, "https://example.com/one");
        assert_eq!(file.requests[1].url, "https://example.com/two");
    }

    #[test]
    fn test_file_variables() {
        let input = "\
@host = example.com

GET https://{{host}}/api
";
        let file = parse(input);
        assert_eq!(file.variables.get("host").unwrap(), "example.com");
        assert_eq!(file.requests.len(), 1);
    }

    #[test]
    fn test_annotations() {
        let input = "\
# @name login
# @no-redirect
POST https://example.com/auth
";
        let file = parse(input);
        assert_eq!(file.requests.len(), 1);
        assert_eq!(file.requests[0].name.as_deref(), Some("login"));
        assert!(file.requests[0].no_redirect);
    }

    #[test]
    fn test_request_line_numbers() {
        let input = "\
@host = example.com

# comment
GET https://example.com/one

###

POST https://example.com/two
";
        let file = parse(input);
        assert_eq!(file.requests[0].line, 3);
        assert_eq!(file.requests[1].line, 7);
    }

    #[test]
    fn test_multiline_url() {
        let input = "\
GET https://example.com
    /api
    /users
    ?page=1
    &limit=10
Accept: application/json
";
        let file = parse(input);
        assert_eq!(file.requests.len(), 1);
        assert_eq!(
            file.requests[0].url,
            "https://example.com/api/users?page=1&limit=10"
        );
        assert_eq!(file.requests[0].headers.len(), 1);
    }

    #[test]
    fn test_multiline_url_with_fragment() {
        let input = "\
GET https://example.com
    /api
    /docs
    #section-2
";
        let file = parse(input);
        assert_eq!(
            file.requests[0].url,
            "https://example.com/api/docs#section-2"
        );
    }

    #[test]
    fn test_multiline_header_value() {
        let input = "\
GET https://example.com
Accept: text/html,
    application/xhtml+xml,
    application/xml;q=0.9
";
        let file = parse(input);
        assert_eq!(file.requests[0].headers.len(), 1);
        assert_eq!(
            file.requests[0].headers[0].1,
            "text/html, application/xhtml+xml, application/xml;q=0.9"
        );
    }

    #[test]
    fn test_multipart_body() {
        let body = "\
--boundary
Content-Disposition: form-data; name=\"text\"

Hello World
--boundary
Content-Disposition: form-data; name=\"file\"; filename=\"data.json\"
Content-Type: application/json

< ./data.json
--boundary--";
        let parts = parse_multipart_body(body, "boundary");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].field_name, "text");
        assert!(parts[0].filename.is_none());
        assert!(matches!(parts[0].data, MultipartData::Text(ref t) if t == "Hello World"));
        assert_eq!(parts[1].field_name, "file");
        assert_eq!(parts[1].filename.as_deref(), Some("data.json"));
        assert!(matches!(parts[1].data, MultipartData::File(ref f) if f == "./data.json"));
    }
}
