use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{
    Method, StatusCode,
    header::{self, COOKIE, HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use url::{Url, form_urlencoded};

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

#[derive(Debug, Clone)]
pub struct CodecheckWorkflowRequest {
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub languages: Vec<String>,
    pub access_token_secret: String,
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
        builder = builder.header(header::REFERER, self.api_base.as_str());
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

pub fn codecheck_workflow_content(request: CodecheckWorkflowRequest) -> anyhow::Result<String> {
    let languages = normalize_codecheck_languages(request.languages)?;
    validate_codecheck_secret_name(&request.access_token_secret)?;
    let rule_sets = languages
        .into_iter()
        .map(|language| json!({ "language": language }))
        .collect::<Vec<_>>();
    let rule_sets = serde_json::to_string(&rule_sets)?;
    let access_token = format!("${{{{ secrets.{} }}}}", request.access_token_secret);
    let name = yaml_quote(&request.name);
    let trigger_branch = yaml_quote(&request.branch);
    let fallback_repo_url = github_expression_string_literal(&request.repo_url);
    let rule_sets = yaml_quote(&rule_sets);
    let access_token = yaml_quote(&access_token);
    let codecheck_repo_url =
        format!("${{{{ github.event.pull_request.head.repo.clone_url || {fallback_repo_url} }}}}");
    let codecheck_branch = "${{ github.head_ref || github.ref_name }}";

    Ok(format!(
        "\
name: {name}

on:
  push:
    branches: [ {trigger_branch} ]
  pull_request:
    branches: [ {trigger_branch} ]

jobs:
  build:
    runs-on: euleros-2.10.1
    steps:
      - name: codecheck-action-task
        uses: codecheck-action@0.0.3
        with:
          repo_url: {codecheck_repo_url}
          branch: {codecheck_branch}
          rule_sets: {rule_sets}
          access_token: {access_token}
"
    ))
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

fn normalize_codecheck_languages(values: Vec<String>) -> anyhow::Result<Vec<String>> {
    let values = if values.is_empty() {
        vec!["SHELL".to_string()]
    } else {
        values
    };
    values
        .into_iter()
        .map(|value| {
            let language = value.trim().to_ascii_uppercase();
            if CODECHECK_LANGUAGES.contains(&language.as_str()) {
                Ok(language)
            } else {
                bail!(
                    "unsupported CodeCheck language: {value}; supported languages: {}",
                    CODECHECK_LANGUAGES.join(", ")
                );
            }
        })
        .collect()
}

const CODECHECK_LANGUAGES: &[&str] = &[
    "JAVA",
    "C++",
    "C",
    "TYPESCRIPT",
    "CANGJIE",
    "RUST",
    "ARKTS",
    "CSS",
    "GO",
    "HTML",
    "JAVASCRIPT",
    "KOTLIN",
    "LUA",
    "PHP",
    "PYTHON",
    "SCALA",
    "SHELL",
    "SQL",
];

fn validate_codecheck_secret_name(value: &str) -> anyhow::Result<()> {
    if value.trim().is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        bail!("CodeCheck access token secret name must contain only letters, digits, and '_'");
    }
    Ok(())
}

fn yaml_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn github_expression_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
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

pub const OPENLIBING_TOKEN_ENV: &str = "GD_OPENLIBING_TOKEN";
pub const OPENLIBING_COOKIE_ENV: &str = "GD_OPENLIBING_COOKIE";
pub const OPENLIBING_CSRF_ENV: &str = "GD_OPENLIBING_CSRF_TOKEN";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenlibingCredential {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OAuthCallback {
    pub query: BTreeMap<String, String>,
    pub cookie: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenlibingClient {
    http: reqwest::Client,
    gateway_base: Url,
    credential: Option<OpenlibingCredential>,
}

#[derive(Debug, Clone)]
pub struct OpenlibingPipelineListRequest {
    pub project_id: String,
    pub pipeline_name: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone)]
pub struct PullRequestListRequest {
    pub project_id: String,
    pub repository: Option<String>,
    pub state: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone)]
pub struct RepositoryQueryRequest {
    pub project_id: String,
    pub repository: Option<String>,
    pub repo_id: Option<u64>,
    pub page: u64,
    pub per_page: u64,
}

impl OpenlibingCredential {
    pub fn from_environment() -> Option<Self> {
        let credential = Self {
            token: non_empty_env(OPENLIBING_TOKEN_ENV),
            cookie: non_empty_env(OPENLIBING_COOKIE_ENV),
            csrf_token: non_empty_env(OPENLIBING_CSRF_ENV),
        };
        credential.is_present().then_some(credential)
    }

    pub fn from_stored_json(value: &str) -> anyhow::Result<Self> {
        if value.trim_start().starts_with('{') {
            return serde_json::from_str(value)
                .context("failed to parse stored OpenLibing credential");
        }
        Ok(Self {
            token: Some(value.to_string()),
            cookie: None,
            csrf_token: None,
        })
    }

    pub fn is_present(&self) -> bool {
        self.token
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || self
                .cookie
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
    }
}

impl OpenlibingClient {
    pub fn with_http_client(
        http: reqwest::Client,
        gateway_base: Url,
        credential: Option<OpenlibingCredential>,
    ) -> Self {
        Self {
            http,
            gateway_base,
            credential,
        }
    }

    pub fn oauth_authorize_url(&self, callback_url: &str, state: &str) -> anyhow::Result<Url> {
        let mut url = self.site_base()?;
        url.set_path("/gateway/oauth2/authorization/gitcode");
        url.query_pairs_mut()
            .append_pair("autoLogin", "complete")
            .append_pair("redirect_uri", callback_url)
            .append_pair("state", state);
        Ok(url)
    }

    pub async fn gitcode_auth_status(&self) -> anyhow::Result<Value> {
        self.request_json(
            Method::GET,
            "authorization/callback/checkGitcodeAccessToken",
            &[],
            None,
        )
        .await
    }

    pub async fn pipeline_summary(&self, project_id: &str) -> anyhow::Result<Value> {
        self.request_read_json(
            "openlibing-cicd/project/pipeline/pipeline-run/summary",
            &project_query(project_id),
        )
        .await
    }

    pub async fn list_pipelines(
        &self,
        request: OpenlibingPipelineListRequest,
    ) -> anyhow::Result<Value> {
        let mut query = project_query(&request.project_id);
        query.push(("pageNum".to_string(), request.page.to_string()));
        query.push(("pageSize".to_string(), request.per_page.to_string()));
        if let Some(name) = request
            .pipeline_name
            .filter(|value| !value.trim().is_empty())
        {
            query.push(("pipelineName".to_string(), name));
        }
        self.request_read_json("openlibing-platform-release/config/pipeline/list", &query)
            .await
    }

    pub async fn codecheck_rule_sets(&self, project_id: &str) -> anyhow::Result<Value> {
        self.request_read_json(
            "openlibing-coderepo/project-config/get-project-codecheck-rule-set",
            &project_query(project_id),
        )
        .await
    }

    pub async fn anti_rule_sets(&self, project_id: &str) -> anyhow::Result<Value> {
        self.request_read_json(
            "openlibing-coderepo/project-config/get-project-anti-rule-set",
            &project_query(project_id),
        )
        .await
    }

    pub async fn query_repositories(
        &self,
        request: RepositoryQueryRequest,
    ) -> anyhow::Result<Value> {
        let mut body = Map::new();
        body.insert(
            "projectId".to_string(),
            Value::String(request.project_id.clone()),
        );
        body.insert("pageNum".to_string(), Value::from(request.page));
        body.insert("pageSize".to_string(), Value::from(request.per_page));
        body.insert("branchCount".to_string(), Value::Bool(true));
        body.insert("platform".to_string(), Value::String("gitcode".to_string()));
        if let Some(repo_id) = request.repo_id {
            body.insert("repoId".to_string(), Value::from(repo_id));
        }
        if let Some(repository) = request.repository.filter(|value| !value.trim().is_empty()) {
            body.insert(
                "repoName".to_string(),
                Value::String(repo_name(&repository)),
            );
        }
        self.request_json(
            Method::POST,
            "openlibing-coderepo/project-repo/query-repo",
            &[],
            Some(Value::Object(body)),
        )
        .await
    }

    pub async fn repo_rule_sets(&self, repo_id: u64) -> anyhow::Result<Value> {
        self.request_read_json(
            "openlibing-coderepo/project-repo/get-repo-rule-set",
            &[("repoId".to_string(), repo_id.to_string())],
        )
        .await
    }

    pub async fn add_repository(&self, body: Value) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "openlibing-coderepo/project-repo/add-repo",
            &[],
            Some(body),
        )
        .await
    }

    pub async fn update_repository(&self, body: Value) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "openlibing-coderepo/project-repo/update-repo",
            &[],
            Some(body),
        )
        .await
    }

    pub async fn auto_set_webhook(&self, repo_id: u64) -> anyhow::Result<Value> {
        self.request_read_json(
            "openlibing-coderepo/project-repo/auto-set-webhook",
            &[("repoId".to_string(), repo_id.to_string())],
        )
        .await
    }

    pub async fn list_pull_requests(
        &self,
        request: PullRequestListRequest,
    ) -> anyhow::Result<Value> {
        let mut query = repo_query(request.repository.as_deref())?;
        query.push(("pageNumber".to_string(), request.page.to_string()));
        query.push(("pageSize".to_string(), request.per_page.to_string()));
        if let Some(state) = request.state.filter(|value| !value.trim().is_empty()) {
            query.push(("state".to_string(), state));
        }
        self.request_read_json(
            &project_path(&request.project_id, "pr/gitcode/pulls"),
            &query,
        )
        .await
    }

    pub async fn pull_request(
        &self,
        project_id: &str,
        number: u64,
        repository: Option<&str>,
    ) -> anyhow::Result<Value> {
        self.request_read_json(
            &project_path(project_id, &format!("pr/gitcode/pulls/{number}")),
            &repo_query(repository)?,
        )
        .await
    }

    pub async fn build_checks(
        &self,
        project_id: &str,
        number: u64,
        repository: Option<&str>,
    ) -> anyhow::Result<Value> {
        let mut query = repo_query(repository)?;
        query.push(("number".to_string(), number.to_string()));
        match self
            .request_read_json(&project_path(project_id, "pr/gitcode/build-check"), &query)
            .await
        {
            Ok(value) => Ok(value),
            Err(error) if should_fallback_to_codecheck_summary(&error) && repository.is_some() => {
                self.codecheck_task_summary(project_id, number, repository)
                    .await
            }
            Err(error) => Err(error),
        }
    }

    pub async fn codecheck_task_summary(
        &self,
        project_id: &str,
        number: u64,
        repository: Option<&str>,
    ) -> anyhow::Result<Value> {
        let repo_name = repository.map(repo_name).unwrap_or_default();
        let body = json_object([
            ("pageNum", Value::from(1)),
            ("pageSize", Value::from(20)),
            ("startTime", Value::String(String::new())),
            ("endTime", Value::String(String::new())),
            ("projectId", Value::String(project_id.to_string())),
            ("projectName", Value::String(String::new())),
            ("sourceBranch", Value::String(String::new())),
            ("sigName", Value::String(String::new())),
            ("repoName", Value::String(repo_name)),
            ("mrId", Value::String(number.to_string())),
            ("result", Value::String(String::new())),
        ]);
        self.request_json(
            Method::POST,
            "openlibing-codecheck/ci-portal/v1/codecheck/inc/v1/task/result/summary",
            &[],
            Some(Value::Object(body)),
        )
        .await
    }

    async fn request_read_json(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> anyhow::Result<Value> {
        match self.request_json(Method::GET, path, query, None).await {
            Ok(value) => Ok(value),
            Err(error) if should_retry_read_with_post(&error) => {
                let body = query
                    .iter()
                    .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                    .collect::<Map<_, _>>();
                self.request_json(Method::POST, path, &[], Some(Value::Object(body)))
                    .await
            }
            Err(error) => Err(error),
        }
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let mut url = self.endpoint_url(path)?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                if !value.trim().is_empty() {
                    pairs.append_pair(key, value);
                }
            }
        }

        let mut builder = self.http.request(method, url);
        if let Some(credential) = &self.credential {
            builder = apply_openlibing_auth(builder, credential)?;
        }
        builder = builder
            .header("Accept", "application/json, text/plain, */*")
            .header("X-Requested-With", "XMLHttpRequest");
        if let Ok(site) = self.site_base() {
            builder = builder
                .header("Origin", site.as_str().trim_end_matches('/'))
                .header("Referer", site.as_str());
        }
        if let Some(body) = body {
            builder = builder.json(&body);
        }
        let response = builder
            .send()
            .await
            .context("OpenLibing API request failed")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read OpenLibing API response")?;
        let body = parse_openlibing_response_body(&text)?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(openlibing_status_error(status, &body))
        }
    }

    fn endpoint_url(&self, path: &str) -> anyhow::Result<Url> {
        let path = path.trim_start_matches('/');
        self.gateway_base
            .join(path)
            .with_context(|| format!("invalid OpenLibing endpoint path: {path}"))
    }

    fn site_base(&self) -> anyhow::Result<Url> {
        let mut site = self.gateway_base.clone();
        site.set_path("/");
        site.set_query(None);
        site.set_fragment(None);
        Ok(site)
    }
}

pub fn openlibing_base_from_value(value: &str) -> anyhow::Result<Url> {
    let mut url =
        Url::parse(value).with_context(|| format!("invalid OpenLibing base URL: {value}"))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        bail!("OpenLibing base URL must use http or https");
    }
    if url.host_str().is_none() {
        bail!("OpenLibing base URL must include a host");
    }
    if !url.path().trim_end_matches('/').ends_with("gateway") {
        let path = format!("{}/gateway", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url)
}

pub fn openlibing_credential_key(base: &Url) -> String {
    let host = base.host_str().unwrap_or("openlibing.com");
    let port = base
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("openlibing.{host}{port}")
}

pub fn oauth_state() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("gd-{millis}-{}", std::process::id())
}

pub async fn wait_for_oauth_callback(
    listener: TcpListener,
    expected_state: String,
    timeout: Duration,
) -> anyhow::Result<OAuthCallback> {
    tokio::task::spawn_blocking(move || read_oauth_callback(listener, expected_state, timeout))
        .await
        .context("OpenLibing OAuth callback task failed")?
}

pub fn credential_from_callback(callback: OAuthCallback) -> anyhow::Result<OpenlibingCredential> {
    let token = first_query_value(
        &callback.query,
        &[
            "access_token",
            "token",
            "openlibing_token",
            "authorization",
            "Authorization",
        ],
    )
    .map(strip_bearer);
    let csrf_token = first_query_value(
        &callback.query,
        &["csrf-token-open-li-bing", "csrf_token_open_li_bing", "csrf"],
    );
    let credential = OpenlibingCredential {
        token,
        cookie: callback.cookie,
        csrf_token,
    };
    if credential.is_present() {
        return Ok(credential);
    }

    if callback.query.contains_key("code") {
        bail!(
            "OpenLibing OAuth callback returned an authorization code but no token or cookie; CLI token exchange is not exposed by the current OpenLibing callback"
        );
    }
    bail!("OpenLibing OAuth callback did not include a token or cookie")
}

pub fn data_or_value(value: &Value) -> Value {
    value.get("data").cloned().unwrap_or_else(|| value.clone())
}

fn read_oauth_callback(
    listener: TcpListener,
    expected_state: String,
    timeout: Duration,
) -> anyhow::Result<OAuthCallback> {
    listener
        .set_nonblocking(false)
        .context("failed to configure OAuth callback listener")?;
    listener
        .set_ttl(64)
        .context("failed to configure OAuth callback listener ttl")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure OAuth callback listener nonblocking mode")?;

    let started = std::time::Instant::now();
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() >= timeout {
                    bail!("timed out waiting for OpenLibing OAuth callback");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(error).context("failed to accept OpenLibing OAuth callback"),
        }
    };

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("failed to configure OAuth callback read timeout")?;
    let request = read_http_request(&mut stream)?;
    let callback = parse_callback_request(&request, &expected_state)?;
    let body = b"OpenLibing authorization received. You can close this window.";
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        String::from_utf8_lossy(body)
    );
    stream
        .write_all(response.as_bytes())
        .context("failed to write OpenLibing OAuth callback response")?;
    Ok(callback)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> anyhow::Result<String> {
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0; 1024];
        let read = stream
            .read(&mut chunk)
            .context("failed to read OpenLibing OAuth callback request")?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 32 * 1024 {
            bail!("OpenLibing OAuth callback request was too large");
        }
    }
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

fn parse_callback_request(request: &str, expected_state: &str) -> anyhow::Result<OAuthCallback> {
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("OpenLibing OAuth callback request was malformed"))?;
    let url = Url::parse(&format!("http://localhost{path}"))
        .context("OpenLibing OAuth callback URL was malformed")?;
    let query = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<BTreeMap<_, _>>();
    if let Some(state) = query.get("state")
        && state != expected_state
    {
        bail!("OpenLibing OAuth callback state did not match");
    }
    let cookie = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("cookie")
            .then(|| value.trim().to_string())
    });
    Ok(OAuthCallback { query, cookie })
}

fn apply_openlibing_auth(
    builder: reqwest::RequestBuilder,
    credential: &OpenlibingCredential,
) -> anyhow::Result<reqwest::RequestBuilder> {
    let mut builder = builder;
    if let Some(token) = credential
        .token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        builder = builder.bearer_auth(strip_bearer(token));
    }
    if let Some(cookie) = credential
        .cookie
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        builder = builder.header(COOKIE, HeaderValue::from_str(cookie.trim())?);
    }
    if let Some(csrf) = credential
        .csrf_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        builder = builder.header(
            HeaderName::from_static("csrf-token-open-li-bing"),
            HeaderValue::from_str(csrf.trim())?,
        );
    }
    Ok(builder)
}

fn project_query(project_id: &str) -> Vec<(String, String)> {
    vec![("projectId".to_string(), project_id.to_string())]
}

fn json_object<const N: usize>(entries: [(&str, Value); N]) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn project_path(project_id: &str, suffix: &str) -> String {
    format!(
        "openlibing-cicd/project/{project_id}/{}",
        suffix.trim_start_matches('/')
    )
}

fn repo_query(repository: Option<&str>) -> anyhow::Result<Vec<(String, String)>> {
    let Some(repository) = repository.filter(|value| !value.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    let Some((owner, repo)) = repository.split_once('/') else {
        bail!("repository must be in owner/repo form: {repository}");
    };
    Ok(vec![
        ("owner".to_string(), owner.to_string()),
        ("repo".to_string(), repo.to_string()),
    ])
}

fn repo_name(repository: &str) -> String {
    repository
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or(repository)
        .to_string()
}

fn parse_openlibing_response_body(text: &str) -> anyhow::Result<Value> {
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string())))
}

fn openlibing_status_error(status: StatusCode, body: &Value) -> anyhow::Error {
    let message = body
        .get("msg")
        .and_then(Value::as_str)
        .or_else(|| body.get("message").and_then(Value::as_str))
        .or_else(|| body.get("error").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string());
    anyhow::anyhow!("OpenLibing API returned {status}: {message}")
}

fn should_retry_read_with_post(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("405 Method Not Allowed") || message.contains("404 Not Found")
}

fn should_fallback_to_codecheck_summary(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("401 Unauthorized")
        || message.contains("403 Forbidden")
        || message.contains("404 Not Found")
}

fn first_query_value(query: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| query.get(*key))
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

fn strip_bearer(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .trim()
        .strip_prefix("Bearer ")
        .unwrap_or_else(|| value.as_ref().trim())
        .to_string()
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[allow(dead_code)]
fn _encode_query(query: &[(String, String)]) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(
            query
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )
        .finish()
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
    fn codecheck_workflow_uses_secret_reference() {
        let content = codecheck_workflow_content(CodecheckWorkflowRequest {
            name: "codecheck-pipeline".to_string(),
            repo_url: "https://gitcode.com/owner/repo.git".to_string(),
            branch: "main".to_string(),
            languages: vec!["shell".to_string(), "Rust".to_string()],
            access_token_secret: "CODECHECK_TOKEN".to_string(),
        })
        .unwrap();

        assert!(content.contains("uses: codecheck-action@0.0.3"));
        assert!(content.contains(
            "repo_url: ${{ github.event.pull_request.head.repo.clone_url || 'https://gitcode.com/owner/repo.git' }}"
        ));
        assert!(
            content.contains("rule_sets: '[{\"language\":\"SHELL\"},{\"language\":\"RUST\"}]'")
        );
        assert!(content.contains("branches: [ 'main' ]"));
        assert!(content.contains("branch: ${{ github.head_ref || github.ref_name }}"));
        assert!(content.contains("access_token: '${{ secrets.CODECHECK_TOKEN }}'"));
        assert!(!content.contains("integration-token"));
    }

    #[test]
    fn codecheck_workflow_rejects_invalid_language_and_secret() {
        assert!(
            codecheck_workflow_content(CodecheckWorkflowRequest {
                name: "codecheck-pipeline".to_string(),
                repo_url: "https://gitcode.com/owner/repo.git".to_string(),
                branch: "main".to_string(),
                languages: vec!["brainfuck".to_string()],
                access_token_secret: "CODECHECK_TOKEN".to_string(),
            })
            .is_err()
        );
        assert!(
            codecheck_workflow_content(CodecheckWorkflowRequest {
                name: "codecheck-pipeline".to_string(),
                repo_url: "https://gitcode.com/owner/repo.git".to_string(),
                branch: "main".to_string(),
                languages: vec!["SHELL".to_string()],
                access_token_secret: "bad-secret".to_string(),
            })
            .is_err()
        );
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

    #[test]
    fn parses_openlibing_base_with_gateway() {
        let url = openlibing_base_from_value("https://www.openlibing.com").unwrap();
        assert_eq!(url.as_str(), "https://www.openlibing.com/gateway/");
    }

    #[test]
    fn credential_key_uses_openlibing_host() {
        let url = openlibing_base_from_value("https://www.openlibing.com/gateway").unwrap();
        assert_eq!(
            openlibing_credential_key(&url),
            "openlibing.www.openlibing.com"
        );
    }

    #[test]
    fn callback_extracts_token() {
        let request =
            "GET /callback?state=s&access_token=Bearer%20abc HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let callback = parse_callback_request(request, "s").unwrap();
        let credential = credential_from_callback(callback).unwrap();
        assert_eq!(credential.token.as_deref(), Some("abc"));
    }

    #[test]
    fn callback_rejects_state_mismatch() {
        let request =
            "GET /callback?state=other&access_token=abc HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(parse_callback_request(request, "expected").is_err());
    }

    #[test]
    fn repo_query_splits_owner_repo() {
        let query = repo_query(Some("owner/repo")).unwrap();
        assert_eq!(
            query,
            vec![
                ("owner".to_string(), "owner".to_string()),
                ("repo".to_string(), "repo".to_string())
            ]
        );
    }
}
