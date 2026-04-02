use std::net::IpAddr;
use std::path::Path;

/// Validate that a file path is safe to read.
/// Rejects absolute paths and paths containing `..` to prevent path traversal.
pub fn validate_file_path(path: &str, workspace_root: &str) -> Result<String, String> {
    let path = path.trim();

    // Reject absolute paths
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(format!(
            "absolute file path not allowed: {path}. Use a relative path within the workspace."
        ));
    }

    // Reject path traversal
    if path.contains("..") {
        return Err(format!(
            "path traversal not allowed: {path}. File references must stay within the workspace."
        ));
    }

    // Resolve relative to workspace root
    let full_path = Path::new(workspace_root).join(path);
    let canonical = full_path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path '{path}': {e}"))?;

    let workspace_canonical = Path::new(workspace_root)
        .canonicalize()
        .map_err(|e| format!("cannot resolve workspace root: {e}"))?;

    // Verify the resolved path is within the workspace
    if !canonical.starts_with(&workspace_canonical) {
        return Err(format!(
            "file path resolves outside workspace: {path}. Must be within {}",
            workspace_root
        ));
    }

    Ok(canonical.to_string_lossy().to_string())
}

/// Check if a URL targets an internal/private network address.
/// Returns a warning message if the URL is potentially dangerous, None if safe.
pub fn check_url_safety(url: &str) -> Option<String> {
    let host = extract_host(url)?;

    // Check localhost
    if host == "localhost" || host == "[::1]" {
        return Some(format!(
            "request targets localhost ({host}). Ensure this is intentional."
        ));
    }

    // Parse as IP and check private ranges
    let ip: IpAddr = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()?;
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback() {
                return Some(format!(
                    "request targets loopback address ({v4}). Ensure this is intentional."
                ));
            }
            let octets = v4.octets();
            // 10.0.0.0/8
            if octets[0] == 10 {
                return Some(format!(
                    "request targets private network ({v4}). Ensure this is intentional."
                ));
            }
            // 172.16.0.0/12
            if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                return Some(format!(
                    "request targets private network ({v4}). Ensure this is intentional."
                ));
            }
            // 192.168.0.0/16
            if octets[0] == 192 && octets[1] == 168 {
                return Some(format!(
                    "request targets private network ({v4}). Ensure this is intentional."
                ));
            }
            // 169.254.0.0/16 (link-local, cloud metadata)
            if octets[0] == 169 && octets[1] == 254 {
                return Some(format!(
                    "request targets link-local/cloud metadata address ({v4}). \
                     This is commonly used to steal cloud credentials. Ensure this is intentional."
                ));
            }
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() {
                return Some(format!(
                    "request targets loopback address ({v6}). Ensure this is intentional."
                ));
            }
        }
    }

    None
}

fn extract_host(url: &str) -> Option<String> {
    // Strip scheme
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    // Take host:port part (before first /)
    let host_port = after_scheme.split('/').next()?;

    // Strip port
    if host_port.starts_with('[') {
        // IPv6: [::1]:8080
        Some(host_port.split(']').next()?.to_string() + "]")
    } else {
        Some(host_port.split(':').next()?.to_string())
    }
}

/// Validate environment variable access using an allowlist.
/// If the allowlist is empty, all access is blocked by default.
/// Configure allowed variables in .zed/settings.json:
/// ```json
/// { "rest-client": { "allowedProcessEnvVars": ["HOME", "PATH", "NODE_ENV"] } }
/// ```
pub fn validate_env_var(name: &str, allowed: &[String]) -> Result<(), String> {
    if allowed.is_empty() {
        return Err(format!(
            "access to environment variable '{name}' is blocked. \
             Add it to rest-client.allowedProcessEnvVars in .zed/settings.json to allow access."
        ));
    }

    if allowed.iter().any(|a| a == name) {
        Ok(())
    } else {
        Err(format!(
            "access to environment variable '{name}' is not in the allowlist. \
             Add it to rest-client.allowedProcessEnvVars in .zed/settings.json."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_absolute_path() {
        let result = validate_file_path("/etc/passwd", "/workspace");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("absolute"));
    }

    #[test]
    fn test_reject_path_traversal() {
        let result = validate_file_path("../../etc/passwd", "/workspace");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }

    #[test]
    fn test_localhost_warning() {
        assert!(check_url_safety("http://localhost:8080/api").is_some());
        assert!(check_url_safety("http://127.0.0.1/api").is_some());
    }

    #[test]
    fn test_private_network_warning() {
        assert!(check_url_safety("http://10.0.0.1/api").is_some());
        assert!(check_url_safety("http://192.168.1.1/api").is_some());
        assert!(check_url_safety("http://172.16.0.1/api").is_some());
    }

    #[test]
    fn test_cloud_metadata_warning() {
        let warning = check_url_safety("http://169.254.169.254/latest/meta-data/");
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("cloud metadata"));
    }

    #[test]
    fn test_public_url_no_warning() {
        assert!(check_url_safety("https://httpbin.org/get").is_none());
        assert!(check_url_safety("https://api.example.com/v1").is_none());
    }

    #[test]
    fn test_env_var_empty_allowlist_blocks_all() {
        assert!(validate_env_var("HOME", &[]).is_err());
        assert!(validate_env_var("PATH", &[]).is_err());
    }

    #[test]
    fn test_env_var_allowlist() {
        let allowed = vec!["HOME".to_string(), "NODE_ENV".to_string()];
        assert!(validate_env_var("HOME", &allowed).is_ok());
        assert!(validate_env_var("NODE_ENV", &allowed).is_ok());
        assert!(validate_env_var("AWS_SECRET_KEY", &allowed).is_err());
        assert!(validate_env_var("PATH", &allowed).is_err());
    }

    #[test]
    fn test_nested_path_traversal() {
        let result = validate_file_path("data/some/../../../../../etc/passwd", "/workspace");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }
}
