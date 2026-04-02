use crate::curl;
use crate::parser::ParsedRequest;

pub fn generate(request: &ParsedRequest, language: &str) -> Result<String, String> {
    match language {
        "curl" => Ok(curl::to_curl(request)),
        "python" => Ok(generate_python(request)),
        "javascript" | "js" => Ok(generate_javascript(request)),
        "go" => Ok(generate_go(request)),
        _ => Err(format!(
            "unsupported language: {language}. Options: curl, python, javascript, go"
        )),
    }
}

#[allow(dead_code)]
pub fn supported_languages() -> &'static [&'static str] {
    &["curl", "python", "javascript", "go"]
}

fn generate_python(req: &ParsedRequest) -> String {
    let mut code = String::from("import requests\n\n");

    let method = req.method.to_lowercase();
    code.push_str(&format!("url = \"{}\"\n", req.url));

    if !req.headers.is_empty() {
        code.push_str("headers = {\n");
        for (name, value) in &req.headers {
            code.push_str(&format!("    \"{name}\": \"{value}\",\n"));
        }
        code.push_str("}\n");
    }

    if let Some(body) = &req.body {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            code.push_str(&format!(
                "payload = {}\n",
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| body.clone())
            ));
        } else {
            code.push_str(&format!("payload = \"{}\"\n", body.replace('"', "\\\"")));
        }
    }

    code.push_str(&format!("\nresponse = requests.{method}(\n    url,\n"));
    if !req.headers.is_empty() {
        code.push_str("    headers=headers,\n");
    }
    if req.body.is_some() {
        let content_type = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        if content_type.contains("json") {
            code.push_str("    json=payload,\n");
        } else {
            code.push_str("    data=payload,\n");
        }
    }
    code.push_str(")\n\nprint(response.status_code)\nprint(response.text)\n");

    code
}

fn generate_javascript(req: &ParsedRequest) -> String {
    let mut code = String::new();

    if !req.headers.is_empty() {
        code.push_str("const headers = {\n");
        for (name, value) in &req.headers {
            code.push_str(&format!("  \"{name}\": \"{value}\",\n"));
        }
        code.push_str("};\n\n");
    }

    code.push_str(&format!(
        "const response = await fetch(\"{}\", {{\n",
        req.url
    ));
    code.push_str(&format!("  method: \"{}\",\n", req.method));
    if !req.headers.is_empty() {
        code.push_str("  headers,\n");
    }
    if let Some(body) = &req.body {
        let escaped = body.replace('\\', "\\\\").replace('"', "\\\"");
        let content_type = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        if content_type.contains("json") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
                code.push_str(&format!(
                    "  body: JSON.stringify({}),\n",
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| body.clone())
                ));
            } else {
                code.push_str(&format!("  body: \"{escaped}\",\n"));
            }
        } else {
            code.push_str(&format!("  body: \"{escaped}\",\n"));
        }
    }
    code.push_str("});\n\n");
    code.push_str("console.log(response.status);\n");
    code.push_str("console.log(await response.text());\n");

    code
}

fn generate_go(req: &ParsedRequest) -> String {
    let mut code = String::from(
        "package main\n\n\
         import (\n\
         \t\"fmt\"\n\
         \t\"io\"\n\
         \t\"net/http\"\n",
    );

    if req.body.is_some() {
        code.push_str("\t\"strings\"\n");
    }

    code.push_str(")\n\nfunc main() {\n");

    if let Some(body) = &req.body {
        let escaped = body.replace('\\', "\\\\").replace('"', "\\\"");
        code.push_str(&format!("\tbody := strings.NewReader(\"{escaped}\")\n"));
        code.push_str(&format!(
            "\treq, err := http.NewRequest(\"{}\", \"{}\", body)\n",
            req.method, req.url
        ));
    } else {
        code.push_str(&format!(
            "\treq, err := http.NewRequest(\"{}\", \"{}\", nil)\n",
            req.method, req.url
        ));
    }

    code.push_str("\tif err != nil {\n\t\tpanic(err)\n\t}\n\n");

    for (name, value) in &req.headers {
        code.push_str(&format!("\treq.Header.Set(\"{name}\", \"{value}\")\n"));
    }

    code.push_str(
        "\n\
         \tresp, err := http.DefaultClient.Do(req)\n\
         \tif err != nil {\n\
         \t\tpanic(err)\n\
         \t}\n\
         \tdefer resp.Body.Close()\n\n\
         \tfmt.Println(resp.StatusCode)\n\
         \tdata, _ := io.ReadAll(resp.Body)\n\
         \tfmt.Println(string(data))\n\
         }\n",
    );

    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_post() -> ParsedRequest {
        ParsedRequest {
            name: None,
            note: None,
            method: "POST".to_string(),
            url: "https://example.com/api".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: Some(r#"{"key": "value"}"#.to_string()),
            line: 0,
            no_redirect: false,
            no_cookie_jar: false,
        }
    }

    #[test]
    fn test_generate_python() {
        let code = generate(&sample_post(), "python").unwrap();
        assert!(code.contains("import requests"));
        assert!(code.contains("requests.post("));
        assert!(code.contains("json=payload"));
    }

    #[test]
    fn test_generate_javascript() {
        let code = generate(&sample_post(), "javascript").unwrap();
        assert!(code.contains("await fetch("));
        assert!(code.contains("JSON.stringify("));
    }

    #[test]
    fn test_generate_go() {
        let code = generate(&sample_post(), "go").unwrap();
        assert!(code.contains("http.NewRequest("));
        assert!(code.contains("strings.NewReader("));
    }

    #[test]
    fn test_generate_curl() {
        let code = generate(&sample_post(), "curl").unwrap();
        assert!(code.contains("curl -X POST"));
    }

    #[test]
    fn test_unsupported_language() {
        let result = generate(&sample_post(), "ruby");
        assert!(result.is_err());
    }
}
