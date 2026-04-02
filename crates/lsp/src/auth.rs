/// Process authentication headers before sending a request.
/// - Basic auth: auto-encode `user:password` to base64 if not already encoded
/// - Bearer: passthrough as-is
pub fn process_auth_headers(headers: &mut [(String, String)]) {
    for (name, value) in headers.iter_mut() {
        if !name.eq_ignore_ascii_case("authorization") {
            continue;
        }

        let trimmed = value.trim();

        // Basic auth: "Basic user:password" → encode to base64
        if let Some(rest) = trimmed.strip_prefix("Basic ") {
            let rest = rest.trim();
            // Check if already base64 encoded (no colon = likely encoded)
            if rest.contains(':') {
                let encoded = base64_encode(rest.as_bytes());
                *value = format!("Basic {encoded}");
            }
        }
        // Bearer: passthrough, no transformation needed
    }
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
    fn test_basic_auth_plain() {
        let mut headers = vec![("Authorization".to_string(), "Basic user:passwd".to_string())];
        process_auth_headers(&mut headers);
        assert_eq!(headers[0].1, "Basic dXNlcjpwYXNzd2Q=");
    }

    #[test]
    fn test_basic_auth_already_encoded() {
        let mut headers = vec![(
            "Authorization".to_string(),
            "Basic dXNlcjpwYXNzd2Q=".to_string(),
        )];
        process_auth_headers(&mut headers);
        // Should not double-encode
        assert_eq!(headers[0].1, "Basic dXNlcjpwYXNzd2Q=");
    }

    #[test]
    fn test_bearer_passthrough() {
        let mut headers = vec![(
            "Authorization".to_string(),
            "Bearer my-token-123".to_string(),
        )];
        process_auth_headers(&mut headers);
        assert_eq!(headers[0].1, "Bearer my-token-123");
    }

    #[test]
    fn test_non_auth_header_untouched() {
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        process_auth_headers(&mut headers);
        assert_eq!(headers[0].1, "application/json");
    }

    #[test]
    fn test_basic_auth_with_special_chars() {
        let mut headers = vec![(
            "Authorization".to_string(),
            "Basic admin:p@ss:w0rd!".to_string(),
        )];
        process_auth_headers(&mut headers);
        assert_eq!(
            headers[0].1,
            format!("Basic {}", base64_encode(b"admin:p@ss:w0rd!"))
        );
    }
}
