use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, bail};
use reqwest::{
    Method, StatusCode,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use tokio::io::AsyncReadExt;
use url::Url;

#[derive(Debug, Clone)]
pub struct GitcodeClient {
    http: reqwest::Client,
    api_base: Url,
    token: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ApiRequest {
    pub method: String,
    pub endpoint: String,
    pub headers: Vec<String>,
    pub raw_fields: Vec<String>,
    pub fields: Vec<String>,
    pub input: Option<std::path::PathBuf>,
    pub paginate: bool,
}

#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}

impl GitcodeClient {
    pub fn with_http_client(http: reqwest::Client, api_base: Url, token: Option<String>) -> Self {
        Self {
            http,
            api_base,
            token,
        }
    }

    pub async fn get(&self, endpoint: &str, query: &[(&str, String)]) -> anyhow::Result<Value> {
        self.request_json("GET", endpoint, query, None).await
    }

    pub async fn post<T: Serialize + ?Sized>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> anyhow::Result<Value> {
        self.request_json("POST", endpoint, &[], Some(json!(body)))
            .await
    }

    pub async fn patch<T: Serialize + ?Sized>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> anyhow::Result<Value> {
        self.request_json("PATCH", endpoint, &[], Some(json!(body)))
            .await
    }

    pub async fn put<T: Serialize + ?Sized>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> anyhow::Result<Value> {
        self.request_json("PUT", endpoint, &[], Some(json!(body)))
            .await
    }

    pub async fn delete(&self, endpoint: &str) -> anyhow::Result<Value> {
        self.request_json("DELETE", endpoint, &[], None).await
    }

    pub async fn api(&self, request: ApiRequest) -> anyhow::Result<Vec<ApiResponse>> {
        let mut responses = Vec::new();
        let method = request.method.parse::<Method>()?;
        let headers = parse_headers(&request.headers)?;
        let mut url = self.endpoint_url(&request.endpoint)?;
        let body = api_body(
            &request.raw_fields,
            &request.fields,
            request.input.as_deref(),
        )
        .await?;
        loop {
            let mut builder = self.http.request(method.clone(), url.clone());
            builder = self.apply_auth(builder);
            builder = builder.headers(headers.clone());
            if let Some(body) = &body {
                builder = builder.json(body);
            }

            let response = builder.send().await.context("GitCode API request failed")?;
            let status = response.status();
            let response_headers = response.headers().clone();
            let text = response
                .text()
                .await
                .context("failed to read API response")?;
            let body = parse_response_body(&text)?;
            if !status.is_success() {
                return Err(api_status_error(status, &body));
            }
            let next = if request.paginate {
                next_link(&response_headers)
            } else {
                None
            };
            responses.push(ApiResponse {
                status,
                headers: response_headers,
                body,
            });
            let Some(next_url) = next else {
                break;
            };
            url = next_url;
        }
        Ok(responses)
    }

    async fn request_json(
        &self,
        method: &str,
        endpoint: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let method = method.parse::<Method>()?;
        let mut url = self.endpoint_url(endpoint)?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                if !value.is_empty() {
                    pairs.append_pair(key, value);
                }
            }
        }

        let mut builder = self.http.request(method, url);
        builder = self.apply_auth(builder);
        if let Some(body) = body {
            builder = builder.json(&body);
        }
        let response = builder.send().await.context("GitCode API request failed")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read API response")?;
        let body = parse_response_body(&text)?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(api_status_error(status, &body))
        }
    }

    fn endpoint_url(&self, endpoint: &str) -> anyhow::Result<Url> {
        if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            return Url::parse(endpoint)
                .with_context(|| format!("invalid endpoint URL: {endpoint}"));
        }

        let endpoint = endpoint.trim_start_matches('/');
        self.api_base
            .join(endpoint)
            .with_context(|| format!("invalid endpoint path: {endpoint}"))
    }

    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.token {
            builder.bearer_auth(token)
        } else {
            builder
        }
    }
}

fn parse_headers(headers: &[String]) -> anyhow::Result<HeaderMap> {
    let mut output = HeaderMap::new();
    for header in headers {
        let Some((name, value)) = header.split_once(':') else {
            bail!("header must be in 'Name: value' form: {header}");
        };
        output.insert(
            HeaderName::from_bytes(name.trim().as_bytes())?,
            HeaderValue::from_str(value.trim())?,
        );
    }
    Ok(output)
}

async fn api_body(
    raw_fields: &[String],
    typed_fields: &[String],
    input: Option<&Path>,
) -> anyhow::Result<Option<Value>> {
    if let Some(path) = input {
        let content = read_text_input(path).await?;
        if content.trim().is_empty() {
            return Ok(Some(Value::Null));
        }
        return Ok(Some(
            serde_json::from_str(&content).unwrap_or(Value::String(content)),
        ));
    }

    if raw_fields.is_empty() && typed_fields.is_empty() {
        return Ok(None);
    }

    let mut object = Map::new();
    for field in raw_fields {
        let (key, value) = split_field(field)?;
        object.insert(key.to_string(), Value::String(value.to_string()));
    }
    for field in typed_fields {
        let (key, value) = split_field(field)?;
        object.insert(key.to_string(), parse_typed_value(value));
    }
    Ok(Some(Value::Object(object)))
}

async fn read_text_input(path: &Path) -> anyhow::Result<String> {
    if path == Path::new("-") {
        let mut content = String::new();
        tokio::io::stdin()
            .read_to_string(&mut content)
            .await
            .context("failed to read request body from stdin")?;
        return Ok(content);
    }

    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))
}

fn split_field(field: &str) -> anyhow::Result<(&str, &str)> {
    field
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("field must be in key=value form: {field}"))
}

fn parse_typed_value(value: &str) -> Value {
    match value {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => value
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| value.parse::<f64>().map(Value::from))
            .unwrap_or_else(|_| Value::String(value.to_string())),
    }
}

fn parse_response_body(text: &str) -> anyhow::Result<Value> {
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string())))
}

fn api_status_error(status: StatusCode, body: &Value) -> anyhow::Error {
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| body.get("error").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string());
    anyhow::anyhow!("GitCode API returned {status}: {message}")
}

fn next_link(headers: &HeaderMap) -> Option<Url> {
    let link = headers.get(reqwest::header::LINK)?.to_str().ok()?;
    for part in link.split(',') {
        let (url_part, rel_part) = part.split_once(';')?;
        if rel_part.contains("rel=\"next\"") {
            let url = url_part
                .trim()
                .trim_start_matches('<')
                .trim_end_matches('>');
            if let Ok(url) = Url::parse(url) {
                return Some(url);
            }
        }
    }
    None
}

pub fn query(
    entries: impl IntoIterator<Item = (&'static str, Option<String>)>,
) -> Vec<(&'static str, String)> {
    entries
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect()
}

pub fn merge_pages(responses: Vec<ApiResponse>) -> Value {
    if responses.len() == 1 {
        return responses
            .into_iter()
            .next()
            .map(|response| response.body)
            .unwrap_or(Value::Null);
    }

    let mut items = Vec::new();
    for response in responses {
        match response.body {
            Value::Array(mut page) => items.append(&mut page),
            other => items.push(other),
        }
    }
    Value::Array(items)
}

pub fn form_body(entries: impl IntoIterator<Item = (&'static str, Option<String>)>) -> Value {
    let mut body = BTreeMap::new();
    for (key, value) in entries {
        if let Some(value) = value {
            body.insert(key, value);
        }
    }
    json!(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_values_parse_basic_scalars() {
        assert_eq!(parse_typed_value("true"), Value::Bool(true));
        assert_eq!(parse_typed_value("42"), Value::from(42));
        assert_eq!(parse_typed_value("name"), Value::String("name".to_string()));
    }

    #[test]
    fn endpoint_is_joined_under_api_base() {
        let client = GitcodeClient::with_http_client(
            crate::http::gitcode_http_client().unwrap(),
            Url::parse("https://api.gitcode.com/api/v5/").unwrap(),
            None,
        );
        assert_eq!(
            client.endpoint_url("/repos/a/b").unwrap().as_str(),
            "https://api.gitcode.com/api/v5/repos/a/b"
        );
    }
}
