use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HttpFile {
    pub variables: HashMap<String, String>,
    pub requests: Vec<ParsedRequest>,
}

#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub name: Option<String>,
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
        parse_section(&section.text, section.start_line, &mut variables, &mut requests);
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
    let mut line_num = 0;

    for line in text.lines() {
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
        line_num += 1;
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
                break; // This is an annotation, not a regular comment
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
    let mut no_redirect = false;
    let mut no_cookie_jar = false;

    let annotation_start = i;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if let Some(ann) = try_parse_annotation(trimmed) {
            match ann.0.as_str() {
                "name" => name = ann.1,
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
    let (method, url) = match parse_request_line(request_line) {
        Some(r) => r,
        None => {
            // Reset annotation scan - these weren't annotations before a request
            // They might be regular comments
            let _ = annotation_start;
            return;
        }
    };
    i += 1;

    // Parse headers
    let mut headers = Vec::new();
    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() {
            break;
        }
        if let Some(pos) = line.find(':') {
            let header_name = line[..pos].trim().to_string();
            let header_value = line[pos + 1..].trim().to_string();
            headers.push((header_name, header_value));
        }
        i += 1;
    }

    // Skip blank line before body
    if i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }

    // Parse body (rest of section)
    let body = if i < lines.len() {
        let body_text: String = lines[i..]
            .iter()
            .map(|l| *l)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string();
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
    let annotations = [
        "name",
        "prompt",
        "note",
        "no-redirect",
        "no-cookie-jar",
    ];

    for ann in &annotations {
        if rest.starts_with(ann) {
            let after = &rest[ann.len()..];
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
    file.requests
        .iter()
        .rev()
        .find(|r| r.line <= line)
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
        assert_eq!(file.requests[0].body.as_deref(), Some("{\"key\": \"value\"}"));
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
}
