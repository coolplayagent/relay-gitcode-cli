use anyhow::Context;

const GD_SSL_VERIFY_ENV: &str = "GD_SSL_VERIFY";
const GITCODE_SSL_VERIFY_ENV: &str = "GITCODE_SSL_VERIFY";
const SSL_VERIFY_ENV: &str = "SSL_VERIFY";
const GIT_SSL_NO_VERIFY_ENV: &str = "GIT_SSL_NO_VERIFY";

pub fn gitcode_http_client() -> anyhow::Result<reqwest::Client> {
    let ssl_verify = ssl_verify_from_env()?;
    reqwest::Client::builder()
        .danger_accept_invalid_certs(!ssl_verify)
        .build()
        .context("failed to build GitCode HTTP client")
}

fn ssl_verify_from_env() -> anyhow::Result<bool> {
    ssl_verify_from_lookup(|name| std::env::var(name).ok())
}

fn ssl_verify_from_lookup(
    mut lookup: impl FnMut(&'static str) -> Option<String>,
) -> anyhow::Result<bool> {
    for name in [GD_SSL_VERIFY_ENV, GITCODE_SSL_VERIFY_ENV, SSL_VERIFY_ENV] {
        if let Some(value) = lookup(name).filter(|value| !value.trim().is_empty()) {
            return crate::env::parse_env_bool(name, &value);
        }
    }
    if lookup(GIT_SSL_NO_VERIFY_ENV)
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        return Ok(false);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ssl_verify_from_pairs(pairs: &[(&'static str, &str)]) -> anyhow::Result<bool> {
        ssl_verify_from_lookup(|name| {
            pairs
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .map(|(_, value)| (*value).to_string())
        })
    }

    #[test]
    fn ssl_verification_defaults_to_disabled() {
        assert!(!ssl_verify_from_pairs(&[]).unwrap());
    }

    #[test]
    fn positive_ssl_verify_variables_enable_verification() {
        for name in [GD_SSL_VERIFY_ENV, GITCODE_SSL_VERIFY_ENV, SSL_VERIFY_ENV] {
            assert!(ssl_verify_from_pairs(&[(name, "true")]).unwrap());
            assert!(!ssl_verify_from_pairs(&[(name, "false")]).unwrap());
        }
    }

    #[test]
    fn git_ssl_no_verify_disables_verification_for_any_non_empty_value() {
        for value in ["true", "false", "1", "0", "anything"] {
            assert!(!ssl_verify_from_pairs(&[(GIT_SSL_NO_VERIFY_ENV, value)]).unwrap());
        }
    }

    #[test]
    fn ssl_verify_priority_prefers_gd_prefix() {
        assert!(
            ssl_verify_from_pairs(&[
                (GD_SSL_VERIFY_ENV, "true"),
                (GITCODE_SSL_VERIFY_ENV, "false"),
                (SSL_VERIFY_ENV, "false"),
                (GIT_SSL_NO_VERIFY_ENV, "true"),
            ])
            .unwrap()
        );
    }

    #[test]
    fn invalid_ssl_verify_value_is_error() {
        let error = ssl_verify_from_pairs(&[(GD_SSL_VERIFY_ENV, "maybe")]).unwrap_err();
        assert!(
            error.to_string().contains("invalid GD_SSL_VERIFY value"),
            "{error}"
        );
    }
}
