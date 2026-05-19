use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Method, StatusCode};
use serde_json::{Map, Value, json};
use url::Url;

#[derive(Debug, Clone)]
pub struct PipelineClient {
    http: reqwest::Client,
    api_base: Url,
    token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkflowFileRequest {
    pub content: String,
    pub message: String,
    pub branch: Option<String>,
    pub sha: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkflowListRequest {
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone)]
pub struct WorkflowRunListRequest {
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub event: Option<String>,
    pub status: Option<String>,
    pub branch: Option<String>,
    pub executor_id: Option<String>,
    pub mr_id: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone)]
pub struct WorkflowDispatchRequest {
    pub file_path: String,
    pub repo_https_url: String,
    pub branch: Option<String>,
    pub branch_commit_id: Option<String>,
    pub repo_id: Option<String>,
    pub inputs: Map<String, Value>,
}

impl PipelineClient {
    pub fn with_http_client(http: reqwest::Client, api_base: Url, token: Option<String>) -> Self {
        Self {
            http,
            api_base,
            token,
        }
    }

    pub async fn list_workflows(
        &self,
        project_id: &str,
        request: WorkflowListRequest,
    ) -> anyhow::Result<Value> {
        self.request_json(
            "POST",
            &action_path(project_id, "workflows/list"),
            &[],
            Some(workflow_list_body(request)),
        )
        .await
    }

    pub async fn list_runs(
        &self,
        project_id: &str,
        request: WorkflowRunListRequest,
    ) -> anyhow::Result<Value> {
        let path = if let Some(workflow_id) = request.workflow_id.as_deref() {
            action_path(project_id, &format!("workflow-runs/{workflow_id}/list"))
        } else {
            action_path(project_id, "workflow-runs/list")
        };
        self.request_json("POST", &path, &[], Some(workflow_run_list_body(request)))
            .await
    }

    pub async fn view_run(&self, project_id: &str, workflow_run_id: &str) -> anyhow::Result<Value> {
        self.request_json(
            "GET",
            &action_path(project_id, &format!("workflow-runs/{workflow_run_id}")),
            &[],
            None,
        )
        .await
    }

    pub async fn job_log(
        &self,
        project_id: &str,
        workflow_run_id: &str,
        job_identifier: &str,
        step_run_id: Option<String>,
        offset: u64,
        limit: u64,
    ) -> anyhow::Result<Value> {
        let query = step_run_id
            .map(|value| vec![("step_run_id", value)])
            .unwrap_or_default();
        self.request_json(
            "POST",
            &action_path(
                project_id,
                &format!("workflow-runs/{workflow_run_id}/jobs/{job_identifier}/logs"),
            ),
            &query,
            Some(json!({
                "offset": offset,
                "limit": limit,
            })),
        )
        .await
    }

    pub async fn stop_run(&self, project_id: &str, workflow_run_id: &str) -> anyhow::Result<Value> {
        self.request_json(
            "POST",
            &action_path(project_id, &format!("workflow-runs/{workflow_run_id}/stop")),
            &[],
            None,
        )
        .await
    }

    pub async fn retry_run(
        &self,
        project_id: &str,
        workflow_run_id: &str,
        repo_https_url: Option<String>,
        job_run_ids: Vec<String>,
    ) -> anyhow::Result<Value> {
        self.request_json(
            "POST",
            &action_path(
                project_id,
                &format!("workflow-runs/{workflow_run_id}/retry"),
            ),
            &[],
            Some(retry_body(repo_https_url, job_run_ids)),
        )
        .await
    }

    pub async fn rerun(&self, project_id: &str, workflow_run_id: &str) -> anyhow::Result<Value> {
        self.request_json(
            "POST",
            &action_path(
                project_id,
                &format!("workflow-runs/{workflow_run_id}/rerun"),
            ),
            &[],
            None,
        )
        .await
    }

    pub async fn dispatch(
        &self,
        project_id: &str,
        workflow_id: &str,
        request: WorkflowDispatchRequest,
    ) -> anyhow::Result<Value> {
        self.request_json(
            "POST",
            &action_path(project_id, &format!("workflows/{workflow_id}/dispatch")),
            &[],
            Some(dispatch_body(request)),
        )
        .await
    }

    async fn request_json(
        &self,
        method: &str,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let method = method.parse::<Method>()?;
        let mut url = self.endpoint_url(path)?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                if !value.is_empty() {
                    pairs.append_pair(key, value);
                }
            }
        }

        let mut builder = self.http.request(method, url);
        if let Some(token) = &self.token {
            builder = builder.bearer_auth(token);
        }
        if let Some(body) = body {
            builder = builder.json(&body);
        }
        let response = builder
            .send()
            .await
            .context("GitCode Actions API request failed")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read GitCode Actions API response")?;
        let body = parse_response_body(&text)?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(api_status_error(status, &body))
        }
    }

    fn endpoint_url(&self, path: &str) -> anyhow::Result<Url> {
        let path = path.trim_start_matches('/');
        self.api_base
            .join(path)
            .with_context(|| format!("invalid GitCode Actions endpoint path: {path}"))
    }
}

pub fn actions_api_base_from_hostname(hostname: &str) -> anyhow::Result<Url> {
    let hostname = hostname.trim().trim_end_matches('/');
    if hostname.is_empty() {
        bail!("hostname cannot be empty");
    }
    let value = if hostname.starts_with("http://") || hostname.starts_with("https://") {
        hostname.to_string()
    } else {
        format!("https://{hostname}")
    };
    parse_actions_api_base(&value)
}

pub fn workflow_file_body(request: WorkflowFileRequest) -> Value {
    let mut body = BTreeMap::new();
    body.insert("content", Value::String(STANDARD.encode(request.content)));
    body.insert("message", Value::String(request.message));
    insert_opt(&mut body, "branch", request.branch);
    insert_opt(&mut body, "sha", request.sha);
    json!(body)
}

pub fn validate_file_content_source(
    file_content: Option<&str>,
    file: Option<&Path>,
) -> anyhow::Result<()> {
    if file_content.is_some() && file.is_some() {
        bail!("use either --content or --file, not both");
    }
    Ok(())
}

pub fn validate_workflow_path(path: &str) -> anyhow::Result<()> {
    let path = path.trim_start_matches('/');
    if path.starts_with(".gitcode/workflows/") && !path.ends_with('/') {
        return Ok(());
    }
    bail!("workflow path must be under .gitcode/workflows/");
}

pub fn parse_key_value_inputs(values: &[String]) -> anyhow::Result<Map<String, Value>> {
    let mut inputs = Map::new();
    for value in values {
        let Some((key, raw_value)) = value.split_once('=') else {
            bail!("workflow input must be in key=value form: {value}");
        };
        let key = key.trim();
        if key.is_empty() {
            bail!("workflow input key cannot be empty");
        }
        inputs.insert(key.to_string(), parse_input_value(raw_value.trim()));
    }
    Ok(inputs)
}

pub fn extract_log_text(value: &Value) -> Option<&str> {
    if let Some(text) = value.as_str() {
        return Some(text);
    }
    const PATHS: &[&[&str]] = &[
        &["log"],
        &["logs"],
        &["content"],
        &["data"],
        &["data", "log"],
        &["data", "logs"],
        &["data", "content"],
        &["data", "data"],
        &["data", "data", "log"],
        &["data", "data", "logs"],
        &["data", "data", "content"],
    ];
    for path in PATHS {
        if let Some(text) = string_at_path(value, path) {
            return Some(text);
        }
    }
    None
}

fn parse_actions_api_base(value: &str) -> anyhow::Result<Url> {
    let mut url = Url::parse(value)
        .with_context(|| format!("invalid GitCode Actions API base URL: {value}"))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        bail!("GitCode Actions API base URL must use http or https");
    }
    if url.host_str().is_none() {
        bail!("GitCode Actions API base URL must include a host");
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url)
}

fn action_path(project_id: &str, suffix: &str) -> String {
    let suffix = suffix.trim_start_matches('/');
    format!("api/v2/projects/{project_id}/actions/{suffix}")
}

fn workflow_list_body(request: WorkflowListRequest) -> Value {
    json!({
        "page": request.page,
        "per_page": request.per_page,
    })
}

fn workflow_run_list_body(request: WorkflowRunListRequest) -> Value {
    let mut body = BTreeMap::new();
    insert_opt(&mut body, "workflow_name", request.workflow_name);
    insert_opt(&mut body, "event", request.event);
    insert_opt(&mut body, "status", request.status);
    insert_opt(&mut body, "branch", request.branch);
    insert_opt(&mut body, "executor_id", request.executor_id);
    insert_opt(&mut body, "mr_id", request.mr_id);
    body.insert("page", Value::from(request.page));
    body.insert("per_page", Value::from(request.per_page));
    json!(body)
}

fn dispatch_body(request: WorkflowDispatchRequest) -> Value {
    let mut body = BTreeMap::new();
    body.insert("file_path", Value::String(request.file_path));
    body.insert("repo_https_url", Value::String(request.repo_https_url));
    insert_opt(&mut body, "branch", request.branch);
    insert_opt(&mut body, "branch_commit_id", request.branch_commit_id);
    insert_opt(&mut body, "repo_id", request.repo_id);
    body.insert("inputs", Value::Object(request.inputs));
    json!(body)
}

fn retry_body(repo_https_url: Option<String>, job_run_ids: Vec<String>) -> Value {
    let mut body = BTreeMap::new();
    insert_opt(&mut body, "repo_https_url", repo_https_url);
    let job_run_ids = job_run_ids
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .map(Value::String)
        .collect::<Vec<_>>();
    if !job_run_ids.is_empty() {
        body.insert("job_run_ids", Value::Array(job_run_ids));
    }
    json!(body)
}

fn insert_opt(body: &mut BTreeMap<&'static str, Value>, key: &'static str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        body.insert(key, Value::String(value));
    }
}

fn parse_input_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
}

fn string_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
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
        .or_else(|| body.get("msg").and_then(Value::as_str))
        .or_else(|| body.get("error").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string());
    anyhow::anyhow!("GitCode Actions API returned {status}: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_actions_api_base_from_hostname() {
        let url = actions_api_base_from_hostname("gitcode.com").unwrap();
        assert_eq!(url.as_str(), "https://gitcode.com/");
    }

    #[test]
    fn rejects_invalid_actions_base_scheme() {
        assert!(parse_actions_api_base("file:///tmp/api").is_err());
    }

    #[test]
    fn builds_action_endpoint_paths() {
        assert_eq!(
            action_path("42", "workflow-runs/7/jobs/build/logs"),
            "api/v2/projects/42/actions/workflow-runs/7/jobs/build/logs"
        );
    }

    #[test]
    fn workflow_file_body_encodes_content() {
        let body = workflow_file_body(WorkflowFileRequest {
            content: "name: ci".to_string(),
            message: "configure ci".to_string(),
            branch: Some("main".to_string()),
            sha: Some("abc".to_string()),
        });

        assert_eq!(body["content"], "bmFtZTogY2k=");
        assert_eq!(body["message"], "configure ci");
        assert_eq!(body["branch"], "main");
        assert_eq!(body["sha"], "abc");
    }

    #[test]
    fn workflow_run_list_body_omits_empty_filters() {
        let body = workflow_run_list_body(WorkflowRunListRequest {
            workflow_id: Some("wf".to_string()),
            workflow_name: Some("ci".to_string()),
            event: None,
            status: Some("success".to_string()),
            branch: Some(String::new()),
            executor_id: None,
            mr_id: None,
            page: 2,
            per_page: 20,
        });

        assert_eq!(body["workflow_name"], "ci");
        assert_eq!(body["status"], "success");
        assert_eq!(body["page"], 2);
        assert_eq!(body["per_page"], 20);
        assert!(body.get("branch").is_none());
    }

    #[test]
    fn dispatch_body_includes_inputs() {
        let mut inputs = Map::new();
        inputs.insert("deploy".to_string(), Value::Bool(true));
        let body = dispatch_body(WorkflowDispatchRequest {
            file_path: ".gitcode/workflows/ci.yml".to_string(),
            repo_https_url: "https://gitcode.com/owner/repo.git".to_string(),
            branch: Some("main".to_string()),
            branch_commit_id: Some("abc".to_string()),
            repo_id: Some("42".to_string()),
            inputs,
        });

        assert_eq!(body["file_path"], ".gitcode/workflows/ci.yml");
        assert_eq!(body["branch_commit_id"], "abc");
        assert_eq!(body["repo_id"], "42");
        assert_eq!(body["inputs"]["deploy"], true);
    }

    #[test]
    fn retry_body_can_target_jobs() {
        let body = retry_body(
            Some("https://gitcode.com/owner/repo.git".to_string()),
            vec!["job-1".to_string(), String::new()],
        );

        assert_eq!(body["repo_https_url"], "https://gitcode.com/owner/repo.git");
        assert_eq!(body["job_run_ids"], json!(["job-1"]));
    }

    #[test]
    fn parses_workflow_inputs_as_json_scalars() {
        let inputs = parse_key_value_inputs(&[
            "name=release".to_string(),
            "dry_run=true".to_string(),
            "count=2".to_string(),
        ])
        .unwrap();

        assert_eq!(inputs["name"], "release");
        assert_eq!(inputs["dry_run"], true);
        assert_eq!(inputs["count"], 2);
    }

    #[test]
    fn validates_workflow_paths() {
        assert!(validate_workflow_path(".gitcode/workflows/ci.yml").is_ok());
        assert!(validate_workflow_path("ci.yml").is_err());
    }

    #[test]
    fn extracts_nested_log_text() {
        let value = json!({"data": {"data": {"content": "hello\n"}}});
        assert_eq!(extract_log_text(&value), Some("hello\n"));
    }
}
