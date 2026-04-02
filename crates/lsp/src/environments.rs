use std::collections::HashMap;

use serde::Deserialize;

/// Settings structure read from `.zed/settings.json` under `rest-client`.
///
/// Example:
/// ```json
/// {
///   "rest-client": {
///     "activeEnvironment": "dev",
///     "environments": {
///       "$shared": { "apiVersion": "v1" },
///       "dev": { "host": "localhost:3000", "token": "dev-token" },
///       "prod": { "host": "api.example.com", "token": "prod-token" }
///     }
///   }
/// }
/// ```
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestClientSettings {
    #[serde(default)]
    pub active_environment: Option<String>,
    #[serde(default)]
    pub environments: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub allowed_process_env_vars: Vec<String>,
}

impl RestClientSettings {
    /// Resolve environment variables for the active environment.
    /// Returns $shared variables merged with active environment variables.
    /// Active environment values override $shared values.
    pub fn resolved_variables(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        // Start with $shared
        if let Some(shared) = self.environments.get("$shared") {
            vars.extend(shared.clone());
        }

        // Override with active environment
        if let Some(env_name) = &self.active_environment {
            if let Some(env_vars) = self.environments.get(env_name) {
                vars.extend(env_vars.clone());
            }
        }

        vars
    }

    /// List available environment names (excluding $shared).
    #[allow(dead_code)]
    pub fn environment_names(&self) -> Vec<String> {
        self.environments
            .keys()
            .filter(|k| k.as_str() != "$shared")
            .cloned()
            .collect()
    }
}

/// Parse settings from the workspace/configuration response.
/// The response is a JSON array where each element corresponds to a
/// requested configuration section.
pub fn parse_settings(config: &serde_json::Value) -> RestClientSettings {
    // workspace/configuration returns an array; we request one section
    let section = if config.is_array() {
        config.get(0).unwrap_or(config)
    } else {
        config
    };

    serde_json::from_value(section.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolved_variables_shared_only() {
        let settings = RestClientSettings {
            active_environment: None,
            allowed_process_env_vars: vec![],
            environments: HashMap::from([(
                "$shared".to_string(),
                HashMap::from([("apiVersion".to_string(), "v1".to_string())]),
            )]),
        };
        let vars = settings.resolved_variables();
        assert_eq!(vars.get("apiVersion").unwrap(), "v1");
    }

    #[test]
    fn test_resolved_variables_with_active_env() {
        let settings = RestClientSettings {
            active_environment: Some("dev".to_string()),
            allowed_process_env_vars: vec![],
            environments: HashMap::from([
                (
                    "$shared".to_string(),
                    HashMap::from([
                        ("apiVersion".to_string(), "v1".to_string()),
                        ("host".to_string(), "default.com".to_string()),
                    ]),
                ),
                (
                    "dev".to_string(),
                    HashMap::from([("host".to_string(), "localhost:3000".to_string())]),
                ),
            ]),
        };
        let vars = settings.resolved_variables();
        assert_eq!(vars.get("apiVersion").unwrap(), "v1");
        assert_eq!(vars.get("host").unwrap(), "localhost:3000");
    }

    #[test]
    fn test_resolved_variables_no_environments() {
        let settings = RestClientSettings::default();
        let vars = settings.resolved_variables();
        assert!(vars.is_empty());
    }

    #[test]
    fn test_environment_names() {
        let settings = RestClientSettings {
            active_environment: None,
            allowed_process_env_vars: vec![],
            environments: HashMap::from([
                ("$shared".to_string(), HashMap::new()),
                ("dev".to_string(), HashMap::new()),
                ("prod".to_string(), HashMap::new()),
            ]),
        };
        let mut names = settings.environment_names();
        names.sort();
        assert_eq!(names, vec!["dev", "prod"]);
    }

    #[test]
    fn test_parse_settings_from_array() {
        let json = serde_json::json!([{
            "activeEnvironment": "dev",
            "environments": {
                "$shared": { "apiVersion": "v1" },
                "dev": { "host": "localhost" }
            }
        }]);
        let settings = parse_settings(&json);
        assert_eq!(settings.active_environment, Some("dev".to_string()));
        assert_eq!(settings.environments.len(), 2);
    }

    #[test]
    fn test_parse_settings_empty() {
        let json = serde_json::json!([{}]);
        let settings = parse_settings(&json);
        assert!(settings.active_environment.is_none());
        assert!(settings.environments.is_empty());
    }
}
