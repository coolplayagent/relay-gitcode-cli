use anyhow::bail;

pub const GD_TOKEN_ENV: &str = "GD_TOKEN";
pub const GITCODE_TOKEN_ENV: &str = "GITCODE_TOKEN";
pub const GD_API_BASE_ENV: &str = "GD_API_BASE";
pub const GITCODE_API_BASE_ENV: &str = "GITCODE_API_BASE";

pub fn gitcode_token_env() -> Option<(&'static str, String)> {
    first_non_empty_env(&[GD_TOKEN_ENV, GITCODE_TOKEN_ENV])
}

pub fn gitcode_api_base_env() -> Option<(&'static str, String)> {
    first_non_empty_env(&[GD_API_BASE_ENV, GITCODE_API_BASE_ENV])
}

pub fn first_non_empty_env(names: &[&'static str]) -> Option<(&'static str, String)> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| (*name, value))
    })
}

pub fn parse_env_bool(name: &str, value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("invalid {name} value '{value}', expected true or false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_true_boolean_values() {
        for value in ["true", "1", "yes", "on", " TRUE "] {
            assert!(parse_env_bool("TEST_BOOL", value).unwrap());
        }
    }

    #[test]
    fn parses_false_boolean_values() {
        for value in ["false", "0", "no", "off", " FALSE "] {
            assert!(!parse_env_bool("TEST_BOOL", value).unwrap());
        }
    }

    #[test]
    fn rejects_invalid_boolean_values() {
        let error = parse_env_bool("TEST_BOOL", "maybe").unwrap_err();
        assert!(
            error.to_string().contains("invalid TEST_BOOL value"),
            "{error}"
        );
    }
}
