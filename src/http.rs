use anyhow::Context;

pub fn gitcode_http_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .context("failed to build GitCode HTTP client")
}
