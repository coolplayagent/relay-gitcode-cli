use std::collections::BTreeMap;

use anyhow::{Context, bail};
use hmac::{Hmac, Mac};
use reqwest::{
    Method, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, HOST},
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use url::Url;

type HmacSha256 = Hmac<Sha256>;

const SIGNING_ALGORITHM: &str = "SDK-HMAC-SHA256";

struct SigningRequest<'a> {
    method: &'a Method,
    url: &'a Url,
    body: &'a str,
    access_key: &'a str,
    secret_key: &'a str,
    host: &'a str,
    sdk_date: &'a str,
    security_token: &'a Option<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineClient {
    http: reqwest::Client,
    api_base: Url,
    domain_id: String,
    auth: PipelineAuth,
}

#[derive(Debug, Clone)]
pub enum PipelineAuth {
    AkSk {
        access_key: String,
        secret_key: String,
        security_token: Option<String>,
    },
    Bearer(String),
}

#[derive(Debug, Clone)]
pub struct PipelineRegisterRequest {
    pub kind: String,
    pub https_url: String,
    pub repo_id: Option<String>,
    pub old_file_path: Option<String>,
    pub new_file_path: String,
    pub file_content: Option<String>,
    pub encoding: Option<String>,
    pub default_branch: Option<String>,
    pub file_commit_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineRunRequest {
    pub https_url: String,
    pub file_path: String,
    pub file_content: Option<String>,
    pub branch: Option<String>,
    pub encoding: Option<String>,
    pub tag: Option<String>,
    pub commit_id: Option<String>,
    pub access_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineRunsRequest {
    pub https_url: String,
    pub pipeline_name: Option<String>,
    pub file_path: Option<String>,
    pub pipeline_run_name: Option<String>,
    pub event: Option<String>,
    pub actor: Option<String>,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub offset: u64,
    pub limit: u64,
}

impl PipelineClient {
    pub fn new(api_base: Url, domain_id: String, auth: PipelineAuth) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_base,
            domain_id,
            auth,
        }
    }

    pub async fn register(&self, request: PipelineRegisterRequest) -> anyhow::Result<Value> {
        self.post("yml-register", register_body(request)).await
    }

    pub async fn run(&self, request: PipelineRunRequest) -> anyhow::Result<Value> {
        self.post("run", run_body(request)).await
    }

    pub async fn runs(&self, request: PipelineRunsRequest) -> anyhow::Result<Value> {
        self.post("", runs_body(request)).await
    }

    pub async fn view(&self, pipeline_id: &str, pipeline_run_id: &str) -> anyhow::Result<Value> {
        self.get(&format!("{pipeline_id}/{pipeline_run_id}")).await
    }

    pub async fn log(
        &self,
        pipeline_id: &str,
        pipeline_run_id: &str,
        job_run_id: &str,
        offset: u64,
        limit: u64,
    ) -> anyhow::Result<Value> {
        self.post(
            &format!("{pipeline_id}/{pipeline_run_id}/{job_run_id}/logs"),
            json!({
                "offset": offset,
                "limit": limit,
            }),
        )
        .await
    }

    pub async fn stop(&self, pipeline_id: &str, pipeline_run_id: &str) -> anyhow::Result<Value> {
        self.request_json(
            "POST",
            &format!("{pipeline_id}/{pipeline_run_id}/stop"),
            None,
        )
        .await
    }

    pub async fn retry(
        &self,
        pipeline_id: String,
        pipeline_run_id: String,
        access_token: Option<String>,
    ) -> anyhow::Result<Value> {
        let mut body = Map::new();
        body.insert("pipeline_id".to_string(), Value::String(pipeline_id));
        body.insert(
            "pipeline_run_id".to_string(),
            Value::String(pipeline_run_id),
        );
        insert_opt(&mut body, "access_token", access_token);
        self.post("rerun", Value::Object(body)).await
    }

    fn action_path(&self, suffix: &str) -> String {
        action_path(&self.domain_id, suffix)
    }

    fn endpoint_url(&self, suffix: &str) -> anyhow::Result<Url> {
        let path = self.action_path(suffix);
        self.api_base
            .join(&path)
            .with_context(|| format!("invalid pipeline endpoint path: {path}"))
    }

    async fn get(&self, suffix: &str) -> anyhow::Result<Value> {
        self.request_json("GET", suffix, None).await
    }

    async fn post(&self, suffix: &str, body: Value) -> anyhow::Result<Value> {
        self.request_json("POST", suffix, Some(body)).await
    }

    async fn request_json(
        &self,
        method: &str,
        suffix: &str,
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let method = method.parse::<Method>()?;
        let url = self.endpoint_url(suffix)?;
        let body_text = body
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .context("failed to serialize pipeline request body")?;
        let mut builder = self.http.request(method.clone(), url.clone());
        builder = apply_auth(builder, &method, &url, body_text.as_deref(), &self.auth)?;
        if let Some(body_text) = body_text {
            builder = builder.body(body_text);
        }

        let response = builder
            .send()
            .await
            .context("CodeArts Pipeline API request failed")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read pipeline API response")?;
        let body = parse_response_body(&text);
        if status.is_success() {
            Ok(body)
        } else {
            Err(api_status_error(status, &body))
        }
    }
}

impl PipelineAuth {
    pub fn from_env_or_token(token: Option<String>) -> anyhow::Result<Self> {
        let access_key = first_env(["HUAWEICLOUD_SDK_AK", "CLOUD_SDK_AK"]);
        let secret_key = first_env(["HUAWEICLOUD_SDK_SK", "CLOUD_SDK_SK"]);
        match (access_key, secret_key) {
            (Some(access_key), Some(secret_key)) => Ok(Self::AkSk {
                access_key,
                secret_key,
                security_token: first_env([
                    "HUAWEICLOUD_SDK_SECURITY_TOKEN",
                    "CLOUD_SDK_SECURITY_TOKEN",
                ]),
            }),
            (None, None) => token
                .filter(|value| !value.trim().is_empty())
                .map(Self::Bearer)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "pipeline API auth requires HUAWEICLOUD_SDK_AK/HUAWEICLOUD_SDK_SK or a GitCode token"
                    )
                }),
            _ => bail!(
                "pipeline API AK/SK auth requires both HUAWEICLOUD_SDK_AK and HUAWEICLOUD_SDK_SK"
            ),
        }
    }
}

pub fn parse_pipeline_api_base(value: &str) -> anyhow::Result<Url> {
    let mut url =
        Url::parse(value).with_context(|| format!("invalid pipeline API base URL: {value}"))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        bail!("pipeline API base URL must use http or https");
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url)
}

pub fn action_path(domain_id: &str, suffix: &str) -> String {
    let suffix = suffix.trim_matches('/');
    if suffix.is_empty() {
        format!("v6/{domain_id}/api/pac/pipelines/actions")
    } else {
        format!("v6/{domain_id}/api/pac/pipelines/actions/{suffix}")
    }
}

pub fn validate_file_content_source(
    file_content: Option<&str>,
    file: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    if file_content.is_some() && file.is_some() {
        bail!("use either --file-content or --file, not both");
    }
    Ok(())
}

fn apply_auth(
    builder: reqwest::RequestBuilder,
    method: &Method,
    url: &Url,
    body: Option<&str>,
    auth: &PipelineAuth,
) -> anyhow::Result<reqwest::RequestBuilder> {
    match auth {
        PipelineAuth::Bearer(token) => {
            let mut builder = builder.bearer_auth(token);
            if body.is_some() {
                builder = builder.header(CONTENT_TYPE, "application/json");
            }
            Ok(builder)
        }
        PipelineAuth::AkSk {
            access_key,
            secret_key,
            security_token,
        } => {
            let host = host_header(url)?;
            let sdk_date = sdk_date();
            let body = body.unwrap_or("");
            let (authorization, signed_headers) = sign_request(SigningRequest {
                method,
                url,
                body,
                access_key,
                secret_key,
                host: &host,
                sdk_date: &sdk_date,
                security_token,
            })?;
            let mut builder = builder
                .header(CONTENT_TYPE, "application/json")
                .header(HOST, host)
                .header("X-Sdk-Date", sdk_date)
                .header(AUTHORIZATION, authorization);
            if let Some(security_token) = security_token {
                builder = builder.header("X-Security-Token", security_token);
            }
            debug_assert!(signed_headers.contains("host"));
            Ok(builder)
        }
    }
}

fn sign_request(request: SigningRequest<'_>) -> anyhow::Result<(String, String)> {
    let mut headers = BTreeMap::new();
    headers.insert("content-type", "application/json".to_string());
    headers.insert("host", request.host.to_string());
    headers.insert("x-sdk-date", request.sdk_date.to_string());
    if let Some(security_token) = request.security_token {
        headers.insert("x-security-token", security_token.to_string());
    }
    let canonical_headers = headers
        .iter()
        .map(|(name, value)| format!("{name}:{}\n", value.trim()))
        .collect::<String>();
    let signed_headers = headers.keys().copied().collect::<Vec<_>>().join(";");
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        request.method.as_str(),
        canonical_uri(request.url),
        canonical_query_string(request.url),
        canonical_headers,
        signed_headers,
        sha256_hex(request.body.as_bytes())
    );
    let string_to_sign = format!(
        "{SIGNING_ALGORITHM}\n{}\n{}",
        request.sdk_date,
        sha256_hex(canonical_request.as_bytes())
    );
    let signature = hmac_sha256_hex(request.secret_key.as_bytes(), string_to_sign.as_bytes())?;
    Ok((
        format!(
            "{SIGNING_ALGORITHM} Access={}, SignedHeaders={signed_headers}, Signature={signature}",
            request.access_key
        ),
        signed_headers,
    ))
}

fn register_body(request: PipelineRegisterRequest) -> Value {
    let mut item = Map::new();
    item.insert("type".to_string(), Value::String(request.kind));
    item.insert("https_url".to_string(), Value::String(request.https_url));
    insert_opt(&mut item, "repo_id", request.repo_id);
    insert_opt(&mut item, "old_file_path", request.old_file_path);
    item.insert(
        "new_file_path".to_string(),
        Value::String(request.new_file_path),
    );
    insert_opt(&mut item, "file_content", request.file_content);
    insert_opt(&mut item, "encoding", request.encoding);
    insert_opt(&mut item, "default_branch", request.default_branch);
    insert_opt(&mut item, "file_commit_id", request.file_commit_id);
    json!({ "yaml_file_list": [Value::Object(item)] })
}

fn run_body(request: PipelineRunRequest) -> Value {
    let mut body = Map::new();
    body.insert("https_url".to_string(), Value::String(request.https_url));
    body.insert("file_path".to_string(), Value::String(request.file_path));
    insert_opt(&mut body, "file_content", request.file_content);
    insert_opt(&mut body, "branch", request.branch);
    insert_opt(&mut body, "encoding", request.encoding);
    insert_opt(&mut body, "tag", request.tag);
    insert_opt(&mut body, "commit_id", request.commit_id);
    insert_opt(&mut body, "access_token", request.access_token);
    Value::Object(body)
}

fn runs_body(request: PipelineRunsRequest) -> Value {
    let mut body = Map::new();
    body.insert("offset".to_string(), Value::from(request.offset));
    body.insert("limit".to_string(), Value::from(request.limit));
    body.insert("https_url".to_string(), Value::String(request.https_url));
    insert_opt(&mut body, "pipeline_name", request.pipeline_name);
    insert_opt(&mut body, "file_path", request.file_path);
    insert_opt(&mut body, "pipeline_run_name", request.pipeline_run_name);
    insert_opt(&mut body, "event", request.event);
    insert_opt(&mut body, "actor", request.actor);
    insert_opt(&mut body, "branch", request.branch);
    insert_opt(&mut body, "status", request.status);
    Value::Object(body)
}

fn insert_opt(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        object.insert(key.to_string(), Value::String(value));
    }
}

fn parse_response_body(text: &str) -> Value {
    if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
    }
}

fn api_status_error(status: StatusCode, body: &Value) -> anyhow::Error {
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| body.get("error_msg").and_then(Value::as_str))
        .or_else(|| body.get("error").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string());
    anyhow::anyhow!("CodeArts Pipeline API returned {status}: {message}")
}

fn first_env<const N: usize>(names: [&str; N]) -> Option<String> {
    names.into_iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    })
}

fn sdk_date() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn host_header(url: &Url) -> anyhow::Result<String> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("pipeline API base URL must include a host"))?;
    Ok(match url.port() {
        Some(port) if !is_default_port(url, port) => format!("{host}:{port}"),
        _ => host.to_string(),
    })
}

fn is_default_port(url: &Url, port: u16) -> bool {
    matches!((url.scheme(), port), ("http", 80) | ("https", 443))
}

fn canonical_uri(url: &Url) -> String {
    let mut path = url.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    path
}

fn canonical_query_string(url: &Url) -> String {
    let mut pairs = url.query_pairs().collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn sha256_hex(input: &[u8]) -> String {
    hex_lower(Sha256::digest(input).as_slice())
}

fn hmac_sha256_hex(key: &[u8], input: &[u8]) -> anyhow::Result<String> {
    let mut mac = HmacSha256::new_from_slice(key).context("invalid HMAC key")?;
    mac.update(input);
    Ok(hex_lower(&mac.finalize().into_bytes()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use reqwest::Method;

    use super::*;

    #[test]
    fn normalizes_pipeline_api_base_with_trailing_slash() {
        let url = parse_pipeline_api_base("https://devcloud.example.com").unwrap();
        assert_eq!(url.as_str(), "https://devcloud.example.com/");
    }

    #[test]
    fn rejects_invalid_pipeline_api_base_scheme() {
        assert!(parse_pipeline_api_base("file:///tmp/api").is_err());
    }

    #[test]
    fn builds_pipeline_action_paths() {
        assert_eq!(
            action_path("domain", "run"),
            "v6/domain/api/pac/pipelines/actions/run"
        );
        assert_eq!(
            action_path("domain", ""),
            "v6/domain/api/pac/pipelines/actions"
        );
        assert_eq!(
            action_path("domain", "/pipe/run/logs/"),
            "v6/domain/api/pac/pipelines/actions/pipe/run/logs"
        );
    }

    #[test]
    fn validates_single_file_content_source() {
        assert!(validate_file_content_source(Some("content"), None).is_ok());
        assert!(validate_file_content_source(None, Some(std::path::Path::new("a.yml"))).is_ok());
        assert!(
            validate_file_content_source(Some("content"), Some(std::path::Path::new("a.yml")))
                .is_err()
        );
    }

    #[test]
    fn builds_register_body() {
        let body = register_body(PipelineRegisterRequest {
            kind: "create".to_string(),
            https_url: "https://gitcode.com/o/r.git".to_string(),
            repo_id: Some("repo-id".to_string()),
            old_file_path: None,
            new_file_path: ".gitcode/workflows/ci.yml".to_string(),
            file_content: Some("name: ci".to_string()),
            encoding: Some("UTF-8".to_string()),
            default_branch: Some("main".to_string()),
            file_commit_id: None,
        });
        assert_eq!(
            body["yaml_file_list"][0]["https_url"],
            "https://gitcode.com/o/r.git"
        );
        assert_eq!(
            body["yaml_file_list"][0]["new_file_path"],
            ".gitcode/workflows/ci.yml"
        );
    }

    #[test]
    fn builds_run_body() {
        let body = run_body(PipelineRunRequest {
            https_url: "https://gitcode.com/o/r.git".to_string(),
            file_path: ".gitcode/workflows/ci.yml".to_string(),
            file_content: None,
            branch: Some("main".to_string()),
            encoding: None,
            tag: None,
            commit_id: Some("abc".to_string()),
            access_token: None,
        });
        assert_eq!(body["branch"], "main");
        assert_eq!(body["commit_id"], "abc");
        assert!(body.get("access_token").is_none());
    }

    #[test]
    fn builds_runs_body_with_numbers() {
        let body = runs_body(PipelineRunsRequest {
            https_url: "https://gitcode.com/o/r.git".to_string(),
            pipeline_name: Some("ci".to_string()),
            file_path: None,
            pipeline_run_name: None,
            event: Some("push".to_string()),
            actor: None,
            branch: Some("main".to_string()),
            status: Some("success".to_string()),
            offset: 3,
            limit: 5,
        });
        assert_eq!(body["offset"], 3);
        assert_eq!(body["limit"], 5);
        assert_eq!(body["pipeline_name"], "ci");
    }

    #[test]
    fn signs_ak_sk_requests() {
        let url = Url::parse(
            "https://service.region.example.com/v1/77b6a44cba5143ab91d13ab9a8ff44fd/vpcs?limit=2&marker=13551d6b-755d-4757-b956-536f674975c0",
        )
        .unwrap();
        let (authorization, signed_headers) = sign_request(SigningRequest {
            method: &Method::GET,
            url: &url,
            body: "",
            access_key: "QTWA***KYUC",
            secret_key: "MFyf***VmHc",
            host: "service.region.example.com",
            sdk_date: "20191115T033655Z",
            security_token: &None,
        })
        .unwrap();
        assert_eq!(signed_headers, "content-type;host;x-sdk-date");
        assert!(authorization.starts_with("SDK-HMAC-SHA256 Access=QTWA***KYUC"));
        assert!(authorization.contains("SignedHeaders=content-type;host;x-sdk-date"));
    }
}
