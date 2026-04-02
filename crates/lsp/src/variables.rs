use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

/// Resolve all `{{...}}` variables in the given text.
pub fn resolve(text: &str, ctx: &VariableContext) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        let after_open = &rest[start + 2..];

        if let Some(end) = after_open.find("}}") {
            let expr = after_open[..end].trim();
            let resolved = resolve_expression(expr, ctx);
            result.push_str(&resolved);
            rest = &after_open[end + 2..];
        } else {
            result.push_str("{{");
            rest = after_open;
        }
    }
    result.push_str(rest);
    result
}

fn resolve_expression(expr: &str, ctx: &VariableContext) -> String {
    if let Some(sys) = expr.strip_prefix('$') {
        return resolve_system_variable(sys);
    }

    // Request variable: requestName.response.body.$.jsonpath
    if expr.contains(".response.") || expr.contains(".request.") {
        if let Some(val) = resolve_request_variable(expr, ctx) {
            return val;
        }
    }

    // File/environment variable
    if let Some(val) = ctx.variables.get(expr) {
        return resolve(val, ctx);
    }

    // Undefined variables resolve to empty string
    String::new()
}

fn resolve_system_variable(expr: &str) -> String {
    let (name, args) = match expr.find([' ', '\t']) {
        Some(pos) => (expr[..pos].trim(), expr[pos..].trim()),
        None => (expr.trim(), ""),
    };

    match name {
        "guid" => uuid_v4(),
        "randomInt" => random_int(args),
        "timestamp" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            now.as_secs().to_string()
        }
        "datetime" => format_datetime(args, true),
        "localDatetime" => format_datetime(args, false),
        "dotenv" => resolve_dotenv(args),
        "processEnv" => std::env::var(args).unwrap_or_default(),
        _ => format!("{{{{${expr}}}}}"),
    }
}

fn uuid_v4() -> String {
    // Simple UUID v4 using random bytes
    let mut bytes = [0u8; 16];
    fill_random(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 1

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

fn fill_random(buf: &mut [u8]) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let mut state = hasher.finish();

    for byte in buf.iter_mut() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *byte = (state >> 33) as u8;
    }
}

fn random_int(args: &str) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let (min, max) = match parts.len() {
        2 => (
            parts[0].parse::<i64>().unwrap_or(0),
            parts[1].parse::<i64>().unwrap_or(100),
        ),
        _ => (0, 100),
    };

    let mut buf = [0u8; 8];
    fill_random(&mut buf);
    let raw = u64::from_le_bytes(buf);
    let range = (max - min + 1) as u64;
    let val = min + (raw % range) as i64;
    val.to_string()
}

fn format_datetime(format: &str, _utc: bool) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    match format {
        "iso8601" | "" => {
            // Simplified ISO 8601 from epoch
            let days = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;
            let seconds = time_of_day % 60;

            let (year, month, day) = epoch_days_to_date(days as i64);
            format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
        }
        "rfc1123" => {
            let days = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;
            let seconds = time_of_day % 60;
            let dow = ((days + 4) % 7) as usize; // 1970-01-01 was Thursday

            let (year, month, day) = epoch_days_to_date(days as i64);
            let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            let month_names = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            format!(
                "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
                day_names[dow],
                day,
                month_names[(month - 1) as usize],
                year,
                hours,
                minutes,
                seconds
            )
        }
        _ => secs.to_string(),
    }
}

fn epoch_days_to_date(days: i64) -> (i64, i64, i64) {
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn resolve_dotenv(var_name: &str) -> String {
    let dotenv_path = Path::new(".env");
    if let Ok(content) = std::fs::read_to_string(dotenv_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                if key == var_name {
                    let value = trimmed[eq_pos + 1..].trim();
                    // Strip surrounding quotes
                    let value = value
                        .strip_prefix('"')
                        .and_then(|v| v.strip_suffix('"'))
                        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                        .unwrap_or(value);
                    return value.to_string();
                }
            }
        }
    }
    String::new()
}

fn resolve_request_variable(expr: &str, ctx: &VariableContext) -> Option<String> {
    // Format: requestName.response.body.$.jsonpath
    // or: requestName.response.headers.HeaderName
    let parts: Vec<&str> = expr.splitn(4, '.').collect();
    if parts.len() < 3 {
        return None;
    }

    let request_name = parts[0];
    let _direction = parts[1]; // "response" or "request"
    let section = parts[2]; // "body" or "headers"

    let response = ctx.named_responses.get(request_name)?;

    match section {
        "body" => {
            if parts.len() >= 4 {
                let path = parts[3];
                extract_json_path(&response.body, path)
            } else {
                Some(response.body.clone())
            }
        }
        "headers" => {
            if parts.len() >= 4 {
                let header_name = parts[3];
                response
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(header_name))
                    .map(|(_, v)| v.clone())
            } else {
                Some(
                    response
                        .headers
                        .iter()
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            }
        }
        _ => None,
    }
}

fn extract_json_path(body: &str, path: &str) -> Option<String> {
    let json: Value = serde_json::from_str(body).ok()?;

    // Simple JSONPath: $.key.nested or $[0].key
    let path = path.strip_prefix("$").unwrap_or(path);
    let mut current = &json;

    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        // Handle array index: [0]
        if let Some(idx_str) = segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if let Ok(idx) = idx_str.parse::<usize>() {
                current = current.get(idx)?;
                continue;
            }
        }
        current = current.get(segment)?;
    }

    match current {
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

pub struct NamedResponse {
    pub headers: Vec<(String, String)>,
    pub body: String,
}

pub struct VariableContext {
    pub variables: HashMap<String, String>,
    pub named_responses: HashMap<String, NamedResponse>,
}

impl VariableContext {
    pub fn new(variables: HashMap<String, String>) -> Self {
        Self {
            variables,
            named_responses: HashMap::new(),
        }
    }

    pub fn store_response(&mut self, name: &str, response: NamedResponse) {
        self.named_responses.insert(name.to_string(), response);
    }
}

/// List all variable names available for completion.
pub fn available_variables(ctx: &VariableContext) -> Vec<String> {
    let mut vars: Vec<String> = ctx.variables.keys().cloned().collect();

    // System variables
    for name in &[
        "$guid",
        "$randomInt",
        "$timestamp",
        "$datetime",
        "$localDatetime",
        "$dotenv",
        "$processEnv",
    ] {
        vars.push(name.to_string());
    }

    // Named response variables
    for name in ctx.named_responses.keys() {
        vars.push(format!("{name}.response.body"));
        vars.push(format!("{name}.response.headers"));
    }

    vars.sort();
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ctx() -> VariableContext {
        VariableContext::new(HashMap::new())
    }

    #[test]
    fn test_no_variables() {
        let ctx = empty_ctx();
        assert_eq!(resolve("hello world", &ctx), "hello world");
    }

    #[test]
    fn test_file_variable() {
        let mut vars = HashMap::new();
        vars.insert("host".to_string(), "example.com".to_string());
        let ctx = VariableContext::new(vars);
        assert_eq!(
            resolve("https://{{host}}/api", &ctx),
            "https://example.com/api"
        );
    }

    #[test]
    fn test_multiple_variables() {
        let mut vars = HashMap::new();
        vars.insert("host".to_string(), "example.com".to_string());
        vars.insert("port".to_string(), "8080".to_string());
        let ctx = VariableContext::new(vars);
        assert_eq!(
            resolve("https://{{host}}:{{port}}/api", &ctx),
            "https://example.com:8080/api"
        );
    }

    #[test]
    fn test_unresolved_variable() {
        let ctx = empty_ctx();
        assert_eq!(resolve("{{unknown}}", &ctx), "");
    }

    #[test]
    fn test_system_guid() {
        let ctx = empty_ctx();
        let result = resolve("{{$guid}}", &ctx);
        assert_eq!(result.len(), 36);
        assert_eq!(result.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn test_system_timestamp() {
        let ctx = empty_ctx();
        let result = resolve("{{$timestamp}}", &ctx);
        let ts: u64 = result.parse().unwrap();
        assert!(ts > 1_000_000_000);
    }

    #[test]
    fn test_system_random_int() {
        let ctx = empty_ctx();
        let result = resolve("{{$randomInt 1 10}}", &ctx);
        let val: i64 = result.parse().unwrap();
        assert!((1..=10).contains(&val));
    }

    #[test]
    fn test_process_env() {
        std::env::set_var("TEST_REST_CLIENT_VAR", "test_value");
        let ctx = empty_ctx();
        let result = resolve("{{$processEnv TEST_REST_CLIENT_VAR}}", &ctx);
        assert_eq!(result, "test_value");
        std::env::remove_var("TEST_REST_CLIENT_VAR");
    }

    #[test]
    fn test_nested_variable() {
        let mut vars = HashMap::new();
        vars.insert("scheme".to_string(), "https".to_string());
        vars.insert("base".to_string(), "{{scheme}}://example.com".to_string());
        let ctx = VariableContext::new(vars);
        assert_eq!(resolve("{{base}}/api", &ctx), "https://example.com/api");
    }

    #[test]
    fn test_request_variable_body() {
        let mut ctx = empty_ctx();
        ctx.store_response(
            "login",
            NamedResponse {
                headers: vec![],
                body: r#"{"token":"abc123","user":"admin"}"#.to_string(),
            },
        );
        assert_eq!(resolve("{{login.response.body.$.token}}", &ctx), "abc123");
    }

    #[test]
    fn test_request_variable_header() {
        let mut ctx = empty_ctx();
        ctx.store_response(
            "login",
            NamedResponse {
                headers: vec![("X-Auth-Token".to_string(), "secret".to_string())],
                body: String::new(),
            },
        );
        assert_eq!(
            resolve("{{login.response.headers.X-Auth-Token}}", &ctx),
            "secret"
        );
    }

    #[test]
    fn test_datetime_iso8601() {
        let ctx = empty_ctx();
        let result = resolve("{{$datetime iso8601}}", &ctx);
        assert!(result.contains('T'));
        assert!(result.ends_with('Z'));
    }

    #[test]
    fn test_available_variables() {
        let mut vars = HashMap::new();
        vars.insert("host".to_string(), "example.com".to_string());
        let mut ctx = VariableContext::new(vars);
        ctx.store_response(
            "login",
            NamedResponse {
                headers: vec![],
                body: String::new(),
            },
        );
        let available = available_variables(&ctx);
        assert!(available.contains(&"host".to_string()));
        assert!(available.contains(&"$guid".to_string()));
        assert!(available.contains(&"login.response.body".to_string()));
    }
}
