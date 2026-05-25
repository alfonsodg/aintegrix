use std::path::Path;

use crate::error::AppError;

use super::types::AppConfig;

/// Load and parse configuration from a YAML file.
/// Supports `${ENV_VAR}` interpolation in string values.
pub fn load(path: &str) -> Result<AppConfig, AppError> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(AppError::Config(format!("config file not found: {}", path.display())));
    }

    let raw = std::fs::read_to_string(path).map_err(|e| {
        AppError::Config(format!("failed to read config file: {e}"))
    })?;

    let interpolated = interpolate_env_vars(&raw);

    serde_yaml::from_str(&interpolated)
        .map_err(|e| AppError::Config(format!("invalid YAML: {e}")))
}

/// Replace `${VAR_NAME}` patterns with environment variable values.
fn interpolate_env_vars(input: &str) -> String {
    let mut result = input.to_owned();
    let re_pattern = "${";

    while let Some(start) = result.find(re_pattern) {
        let after_start = start + 2;
        if let Some(end) = result[after_start..].find('}') {
            let var_name = &result[after_start..after_start + end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], value, &result[after_start + end + 1..]);
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_config() {
        let yaml = r#"
server:
  host: "127.0.0.1"
  port: 9090
agents:
  kiro:
    command: kiro-cli
    args: [acp]
    mode: native
    max_sessions: 3
    auto_restart: true
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(yaml.as_bytes()).unwrap();

        let config = load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents["kiro"].command, "kiro-cli");
        assert_eq!(config.agents["kiro"].args, vec!["acp"]);
        assert!(config.agents["kiro"].auto_restart);
    }

    #[test]
    fn test_load_missing_file() {
        let result = load("/nonexistent/path.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_env_var_interpolation() {
        // SAFETY: test runs single-threaded
        unsafe { std::env::set_var("TEST_ACP_KEY", "secret123") };
        let input = "token: ${TEST_ACP_KEY}";
        let result = interpolate_env_vars(input);
        assert_eq!(result, "token: secret123");
        unsafe { std::env::remove_var("TEST_ACP_KEY") };
    }

    #[test]
    fn test_env_var_missing_uses_empty() {
        let input = "token: ${NONEXISTENT_VAR_XYZ}";
        let result = interpolate_env_vars(input);
        assert_eq!(result, "token: ");
    }

    #[test]
    fn test_defaults_applied() {
        let yaml = r#"
server: {}
agents:
  test:
    command: test-agent
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(yaml.as_bytes()).unwrap();

        let config = load(f.path().to_str().unwrap()).unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8050);
        assert_eq!(config.agents["test"].mode, "native");
        assert_eq!(config.agents["test"].max_sessions, 3);
    }
}
