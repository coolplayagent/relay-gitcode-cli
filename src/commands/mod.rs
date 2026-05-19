use std::{net::TcpListener, path::Path, time::Duration};

use anyhow::{Context, bail};
use clap_complete::{Generator, Shell as CompleteShell, generate};
use serde_json::{Map, Value, json};
use tokio::io::AsyncReadExt;

use crate::{
    auth::CredentialStore,
    cli::{
        AuthCommand, Cli, Command, CompletionArgs, IssueCommand, LabelCommand, PipelineAuthCommand,
        PipelineCommand, PipelineSetMode, PipelineSetupArgs, PrCommand, ReleaseCommand,
        RepoCommand, SearchCommand, Shell, SshKeyCommand, VersionCommand,
    },
    client::{ApiRequest, GitcodeClient, form_body, merge_pages, query},
    config::Config,
    output::{print_json, print_value},
    pipeline::{
        CodecheckWorkflowRequest, OpenlibingClient, OpenlibingCredential,
        OpenlibingPipelineListRequest, PipelineClient, PullRequestListRequest,
        RepositoryQueryRequest, WorkflowDispatchRequest, WorkflowFileRequest, WorkflowListRequest,
        WorkflowRunListRequest, actions_api_base_from_hostname, codecheck_workflow_content,
        credential_from_callback, data_or_value, extract_log_text, oauth_state,
        openlibing_base_from_value, openlibing_credential_key, parse_key_value_inputs,
        validate_file_content_source, validate_workflow_path, wait_for_oauth_callback,
        workflow_file_body,
    },
    repo,
    update::{UpdateConfig, check_for_updates, render_version_check_text},
};

pub async fn run(
    cli: Cli,
    mut config: Config,
    credentials: &dyn CredentialStore,
) -> anyhow::Result<()> {
    let json_output = cli.global.json_output();
    let openlibing_base = cli.global.openlibing_base.clone();
    match cli.command {
        Command::Completion(args) => completion(args),
        Command::Version(command) => version(command, json_output).await,
        Command::Auth(command) => auth(command, &mut config, credentials, json_output).await,
        Command::Pipeline(command) => {
            let http = crate::http::gitcode_http_client()?;
            pipeline_command(
                command,
                &config,
                &openlibing_base,
                credentials,
                http,
                json_output,
            )
            .await
        }
        other => {
            let token = credential_get_token(credentials, &config.hostname)?;
            let http = crate::http::gitcode_http_client()?;
            let client = GitcodeClient::with_http_client(
                http.clone(),
                config.api_base_url()?,
                token.clone(),
            );
            match other {
                Command::Api(args) => {
                    let responses = client
                        .api(ApiRequest {
                            method: args.method,
                            endpoint: args.endpoint,
                            headers: args.headers,
                            raw_fields: args.raw_fields,
                            fields: args.fields,
                            input: args.input,
                            paginate: args.paginate,
                        })
                        .await?;
                    if !args.silent {
                        if args.include {
                            for response in &responses {
                                println!("HTTP {}", response.status);
                                for (name, value) in &response.headers {
                                    println!("{}: {}", name, value.to_str().unwrap_or("<binary>"));
                                }
                                println!();
                            }
                        }
                        print_value(json_output, &merge_pages(responses))?;
                    }
                    Ok(())
                }
                Command::Repo(command) => {
                    repo_command(command, &config, &client, json_output).await
                }
                Command::Issue(command) => {
                    issue_command(command, &config, &client, json_output).await
                }
                Command::Pr(command) => pr_command(command, &config, &client, json_output).await,
                Command::Search(command) => search_command(command, &client, json_output).await,
                Command::SshKey(command) => ssh_key_command(command, &client, json_output).await,
                Command::Label(command) => {
                    label_command(command, &config, &client, json_output).await
                }
                Command::Release(command) => {
                    release_command(command, &config, &client, json_output).await
                }
                Command::Completion(_)
                | Command::Version(_)
                | Command::Auth(_)
                | Command::Pipeline(_) => unreachable!(),
            }
        }
    }
}

async fn version(command: VersionCommand, json_output: bool) -> anyhow::Result<()> {
    match command {
        VersionCommand::Check(_) => {
            let config = UpdateConfig::from_environment()?;
            let response = check_for_updates(&config).await;
            if json_output {
                print_json(&response)
            } else {
                print!("{}", render_version_check_text(&response));
                Ok(())
            }
        }
    }
}

async fn auth(
    command: AuthCommand,
    config: &mut Config,
    credentials: &dyn CredentialStore,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        AuthCommand::Login(args) => {
            if !args.with_token {
                bail!("web login is not supported yet; use gd auth login --with-token");
            }
            let mut token = String::new();
            tokio::io::stdin()
                .read_to_string(&mut token)
                .await
                .context("failed to read token from stdin")?;
            let token = token.trim();
            if token.is_empty() {
                bail!("token from stdin is empty");
            }
            credential_set_token(credentials, &config.hostname, token)?;
            config.save().await?;
            let value = json!({
                "hostname": config.hostname,
                "status": "logged_in",
                "credential_store": "system_keyring"
            });
            print_value(json_output, &value)
        }
        AuthCommand::Logout(args) => {
            let hostname = args.hostname.unwrap_or_else(|| config.hostname.clone());
            credential_delete_token(credentials, &hostname)?;
            let value = json!({"hostname": hostname, "status": "logged_out"});
            print_value(json_output, &value)
        }
        AuthCommand::Status(args) => {
            let hostname = args.hostname.unwrap_or_else(|| config.hostname.clone());
            let token = credential_get_token(credentials, &hostname)?;
            let value = json!({
                "hostname": hostname,
                "logged_in": token.is_some(),
                "token": if args.show_token { token } else { None },
                "source": if std::env::var("GITCODE_TOKEN").ok().filter(|v| !v.trim().is_empty()).is_some() {
                    "env"
                } else {
                    "system_keyring"
                }
            });
            print_value(json_output, &value)
        }
        AuthCommand::Token(args) => {
            let hostname = args.hostname.unwrap_or_else(|| config.hostname.clone());
            let Some(token) = credential_get_token(credentials, &hostname)? else {
                bail!("not logged in to {hostname}");
            };
            println!("{token}");
            Ok(())
        }
    }
}

fn credential_get_token(
    credentials: &dyn CredentialStore,
    hostname: &str,
) -> anyhow::Result<Option<String>> {
    tokio::task::block_in_place(|| credentials.get_token(hostname))
}

fn credential_set_token(
    credentials: &dyn CredentialStore,
    hostname: &str,
    token: &str,
) -> anyhow::Result<()> {
    tokio::task::block_in_place(|| credentials.set_token(hostname, token))
}

fn credential_delete_token(
    credentials: &dyn CredentialStore,
    hostname: &str,
) -> anyhow::Result<()> {
    tokio::task::block_in_place(|| credentials.delete_token(hostname))
}

fn credential_get_stored_token(
    credentials: &dyn CredentialStore,
    hostname: &str,
) -> anyhow::Result<Option<String>> {
    tokio::task::block_in_place(|| credentials.get_stored_token(hostname))
}

async fn repo_command(
    command: RepoCommand,
    config: &Config,
    client: &GitcodeClient,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        RepoCommand::View(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client.get(&format!("repos/{repository}"), &[]).await?;
            print_value(json_output, &value)
        }
        RepoCommand::List(args) => {
            let endpoint = if let Some(owner) = args.owner {
                format!("users/{owner}/repos")
            } else {
                "user/repos".to_string()
            };
            let value = client
                .get(
                    &endpoint,
                    &[
                        ("page", "1".to_string()),
                        ("per_page", args.limit.to_string()),
                    ],
                )
                .await?;
            print_value(json_output, &value)
        }
        RepoCommand::Clone(args) => {
            repo::run_git_clone(
                &config.hostname,
                &args.repository,
                args.directory,
                &args.git_flags,
            )
            .await
        }
        RepoCommand::Fork(args) => {
            let value = client
                .post(&format!("repos/{}/forks", args.repository), &json!({}))
                .await?;
            print_value(json_output, &value)
        }
        RepoCommand::Create(args) => {
            let body = form_body([
                ("name", Some(args.name)),
                ("description", args.description),
                ("private", Some(args.private.to_string())),
            ]);
            let value = client.post("user/repos", &body).await?;
            print_value(json_output, &value)
        }
    }
}

async fn issue_command(
    command: IssueCommand,
    config: &Config,
    client: &GitcodeClient,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        IssueCommand::List(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(
                    &format!("repos/{repository}/issues"),
                    &[
                        ("state", args.state),
                        ("page", "1".to_string()),
                        ("per_page", args.limit.to_string()),
                    ],
                )
                .await?;
            print_value(json_output, &value)
        }
        IssueCommand::View(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(&format!("repos/{repository}/issues/{}", args.number), &[])
                .await?;
            print_value(json_output, &value)
        }
        IssueCommand::Create(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let (owner, repo_name) = repo::split_repo(&repository)?;
            let body = form_body([
                ("repo", Some(repo_name.to_string())),
                ("title", Some(args.title)),
                ("body", args.body),
                ("labels", join(args.label)),
                ("assignee", args.assignee),
            ]);
            let value = client.post(&format!("repos/{owner}/issues"), &body).await?;
            print_value(json_output, &value)
        }
        IssueCommand::Comment(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let body = json!({ "body": args.body });
            let value = client
                .post(
                    &format!("repos/{repository}/issues/{}/comments", args.number),
                    &body,
                )
                .await?;
            print_value(json_output, &value)
        }
    }
}

async fn pr_command(
    command: PrCommand,
    config: &Config,
    client: &GitcodeClient,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        PrCommand::List(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(
                    &format!("repos/{repository}/pulls"),
                    &query([
                        ("state", Some(args.state)),
                        ("base", args.base),
                        ("page", Some("1".to_string())),
                        ("per_page", Some(args.limit.to_string())),
                    ]),
                )
                .await?;
            print_value(json_output, &value)
        }
        PrCommand::View(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(&format!("repos/{repository}/pulls/{}", args.number), &[])
                .await?;
            print_value(json_output, &value)
        }
        PrCommand::Create(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let body = form_body([
                ("title", Some(args.title)),
                ("body", args.body),
                ("base", Some(args.base)),
                ("head", Some(args.head)),
                ("labels", join(args.label)),
                ("assignees", join(args.assignee)),
            ]);
            let value = client
                .post(&format!("repos/{repository}/pulls"), &body)
                .await?;
            print_value(json_output, &value)
        }
        PrCommand::Comments(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(
                    &format!("repos/{repository}/pulls/{}/comments", args.number),
                    &[
                        ("page", args.page.to_string()),
                        ("per_page", args.limit.to_string()),
                    ],
                )
                .await?;
            print_value(json_output, &value)
        }
        PrCommand::Comment(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let mut body = serde_json::Map::new();
            body.insert("body".to_string(), Value::String(args.body));
            if let Some(path) = args.path {
                body.insert("path".to_string(), Value::String(path));
            }
            if let Some(position) = args.position {
                body.insert("position".to_string(), Value::from(position));
            }
            body.insert(
                "need_to_resolve".to_string(),
                Value::Bool(args.need_to_resolve),
            );
            let value = client
                .post(
                    &format!("repos/{repository}/pulls/{}/comments", args.number),
                    &Value::Object(body),
                )
                .await?;
            print_value(json_output, &value)
        }
        PrCommand::Reply(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let body = json!({ "body": args.body });
            let value = client
                .post(
                    &format!(
                        "repos/{repository}/pulls/{}/discussions/{}/comments",
                        args.number, args.discussion_id
                    ),
                    &body,
                )
                .await?;
            print_value(json_output, &value)
        }
    }
}

async fn search_command(
    command: SearchCommand,
    client: &GitcodeClient,
    json_output: bool,
) -> anyhow::Result<()> {
    let (endpoint, query_text, limit) = match command {
        SearchCommand::Repos(args) => ("search/repositories", args.query, args.limit),
        SearchCommand::Issues(args) => ("search/issues", args.query, args.limit),
        SearchCommand::Users(args) => ("search/users", args.query, args.limit),
    };
    let value = client
        .get(
            endpoint,
            &[
                ("q", query_text),
                ("page", "1".to_string()),
                ("per_page", limit.to_string()),
            ],
        )
        .await?;
    print_value(json_output, &value)
}

async fn ssh_key_command(
    command: SshKeyCommand,
    client: &GitcodeClient,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        SshKeyCommand::List => {
            let value = client.get("user/keys", &[]).await?;
            print_value(json_output, &value)
        }
        SshKeyCommand::Add(args) => {
            let key = tokio::fs::read_to_string(&args.key_file)
                .await
                .with_context(|| format!("failed to read {}", args.key_file.display()))?;
            let body = form_body([("key", Some(key)), ("title", args.title)]);
            let value = client.post("user/keys", &body).await?;
            print_value(json_output, &value)
        }
        SshKeyCommand::Delete(args) => {
            let value = client.delete(&format!("user/keys/{}", args.id)).await?;
            print_value(json_output, &value)
        }
    }
}

async fn label_command(
    command: LabelCommand,
    config: &Config,
    client: &GitcodeClient,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        LabelCommand::List(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(
                    &format!("repos/{repository}/labels"),
                    &[
                        ("page", "1".to_string()),
                        ("per_page", args.limit.to_string()),
                    ],
                )
                .await?;
            print_value(json_output, &value)
        }
        LabelCommand::Create(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let body = form_body([
                ("name", Some(args.name)),
                ("color", args.color),
                ("description", args.description),
            ]);
            let value = client
                .post(&format!("repos/{repository}/labels"), &body)
                .await?;
            print_value(json_output, &value)
        }
        LabelCommand::Edit(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let body = form_body([
                ("name", args.new_name),
                ("color", args.color),
                ("description", args.description),
            ]);
            let value = client
                .patch(&format!("repos/{repository}/labels/{}", args.name), &body)
                .await?;
            print_value(json_output, &value)
        }
        LabelCommand::Delete(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .delete(&format!("repos/{repository}/labels/{}", args.name))
                .await?;
            print_value(json_output, &value)
        }
    }
}

async fn release_command(
    command: ReleaseCommand,
    config: &Config,
    client: &GitcodeClient,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        ReleaseCommand::List(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(
                    &format!("repos/{repository}/releases"),
                    &[
                        ("page", "1".to_string()),
                        ("per_page", args.limit.to_string()),
                    ],
                )
                .await?;
            print_value(json_output, &value)
        }
        ReleaseCommand::View(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let value = client
                .get(
                    &format!("repos/{repository}/releases/tags/{}", args.tag),
                    &[],
                )
                .await?;
            print_value(json_output, &value)
        }
        ReleaseCommand::Create(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let body = form_body([
                ("tag_name", Some(args.tag)),
                ("name", args.title),
                ("body", args.notes),
                ("target_commitish", args.target),
            ]);
            let value = client
                .post(&format!("repos/{repository}/releases"), &body)
                .await?;
            print_value(json_output, &value)
        }
    }
}

async fn pipeline_command(
    command: PipelineCommand,
    config: &Config,
    openlibing_base: &str,
    credentials: &dyn CredentialStore,
    http: reqwest::Client,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        PipelineCommand::Set(_)
        | PipelineCommand::Codecheck(_)
        | PipelineCommand::List(_)
        | PipelineCommand::Run(_)
        | PipelineCommand::Runs(_)
        | PipelineCommand::View(_)
        | PipelineCommand::Log(_)
        | PipelineCommand::Stop(_)
        | PipelineCommand::Retry(_)
        | PipelineCommand::Rerun(_) => {
            let token = credential_get_token(credentials, &config.hostname)?;
            let api_client = GitcodeClient::with_http_client(
                http.clone(),
                config.api_base_url()?,
                token.clone(),
            );
            gitcode_pipeline_command(command, config, &api_client, http, token, json_output).await
        }
        PipelineCommand::Auth(_)
        | PipelineCommand::Config(_)
        | PipelineCommand::Setup(_)
        | PipelineCommand::Prs(_)
        | PipelineCommand::Checks(_)
        | PipelineCommand::GateView(_)
        | PipelineCommand::GateRuns(_) => {
            openlibing_pipeline_command(command, openlibing_base, credentials, http, json_output)
                .await
        }
    }
}

async fn gitcode_pipeline_command(
    command: PipelineCommand,
    config: &Config,
    api_client: &GitcodeClient,
    http: reqwest::Client,
    token: Option<String>,
    json_output: bool,
) -> anyhow::Result<()> {
    let client = PipelineClient::with_http_client(
        http,
        actions_api_base_from_hostname(&config.hostname)?,
        token,
    );
    match command {
        PipelineCommand::Set(args) => {
            validate_file_content_source(args.content.as_deref(), args.file.as_deref())?;
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let path = args.path.trim_start_matches('/').to_string();
            validate_workflow_path(&path)?;
            if args.mode == PipelineSetMode::Update
                && args
                    .sha
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .is_none()
            {
                bail!("--sha is required when --mode update is used");
            }
            let content = read_required_content(args.content, args.file.as_deref()).await?;
            let body = workflow_file_body(WorkflowFileRequest {
                content,
                message: args.message,
                branch: args.branch,
                sha: args.sha,
            });
            let endpoint = format!("repos/{repository}/contents/{path}");
            let value = match args.mode {
                PipelineSetMode::Create => api_client.post(&endpoint, &body).await?,
                PipelineSetMode::Update => api_client.put(&endpoint, &body).await?,
            };
            print_value(json_output, &value)
        }
        PipelineCommand::Codecheck(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())
                    .await?;
            let path = args.path.trim_start_matches('/').to_string();
            validate_workflow_path(&path)?;
            if args.mode == PipelineSetMode::Update
                && args
                    .sha
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .is_none()
            {
                bail!("--sha is required when --mode update is used");
            }
            let repo_value = api_client.get(&format!("repos/{repository}"), &[]).await?;
            let default_branch = string_field(&repo_value, &["default_branch"])
                .unwrap_or_else(|| "main".to_string());
            let repo_url = args
                .repo_url
                .or_else(|| string_field(&repo_value, &["http_url_to_repo", "clone_url"]))
                .unwrap_or_else(|| repo::clone_url(&config.hostname, &repository));
            let content = codecheck_workflow_content(CodecheckWorkflowRequest {
                name: args.name,
                repo_url,
                branch: args.check_branch.unwrap_or(default_branch),
                languages: args.languages,
                access_token_secret: args.access_token_secret,
            })?;
            let body = workflow_file_body(WorkflowFileRequest {
                content,
                message: args.message,
                branch: args.commit_branch,
                sha: args.sha,
            });
            let endpoint = format!("repos/{repository}/contents/{path}");
            let value = match args.mode {
                PipelineSetMode::Create => api_client.post(&endpoint, &body).await?,
                PipelineSetMode::Update => api_client.put(&endpoint, &body).await?,
            };
            print_value(json_output, &value)
        }
        PipelineCommand::List(args) => {
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let value = client
                .list_workflows(
                    &context.project_id,
                    WorkflowListRequest {
                        page: args.page,
                        per_page: args.limit,
                    },
                )
                .await?;
            print_value(json_output, &value)
        }
        PipelineCommand::Run(args) => {
            validate_workflow_path(&args.file_path)?;
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let inputs = parse_key_value_inputs(&args.inputs)?;
            let value = client
                .dispatch(
                    &context.project_id,
                    &args.workflow_id,
                    WorkflowDispatchRequest {
                        repo_id: Some(context.project_id.clone()),
                        repo_https_url: context.repo_https_url,
                        file_path: args.file_path,
                        branch: args.branch,
                        branch_commit_id: args.branch_commit_id,
                        inputs,
                    },
                )
                .await?;
            print_value(json_output, &value)
        }
        PipelineCommand::Runs(args) => {
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let value = client
                .list_runs(
                    &context.project_id,
                    WorkflowRunListRequest {
                        workflow_id: args.workflow_id,
                        workflow_name: args.workflow_name,
                        event: args.event,
                        status: args.status,
                        branch: args.branch,
                        executor_id: args.executor_id,
                        mr_id: args.mr_id,
                        page: args.page,
                        per_page: args.limit,
                    },
                )
                .await?;
            print_value(json_output, &value)
        }
        PipelineCommand::View(args) => {
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let value = client
                .view_run(&context.project_id, &args.workflow_run_id)
                .await?;
            print_value(json_output, &value)
        }
        PipelineCommand::Log(args) => {
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let value = client
                .job_log(
                    &context.project_id,
                    &args.workflow_run_id,
                    &args.job_identifier,
                    args.step_run_id,
                    args.offset,
                    args.limit,
                )
                .await?;
            print_pipeline_log(json_output, &value)
        }
        PipelineCommand::Stop(args) => {
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let value = client
                .stop_run(&context.project_id, &args.workflow_run_id)
                .await?;
            print_value(json_output, &value)
        }
        PipelineCommand::Retry(args) => {
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let repo_https_url = (!args.job_run_ids.is_empty()).then_some(context.repo_https_url);
            let value = client
                .retry_run(
                    &context.project_id,
                    &args.workflow_run_id,
                    repo_https_url,
                    args.job_run_ids,
                )
                .await?;
            print_value(json_output, &value)
        }
        PipelineCommand::Rerun(args) => {
            let context =
                pipeline_repo_context(api_client, config, args.repository.as_deref()).await?;
            let value = client
                .rerun(&context.project_id, &args.workflow_run_id)
                .await?;
            print_value(json_output, &value)
        }
        PipelineCommand::Auth(_)
        | PipelineCommand::Config(_)
        | PipelineCommand::Setup(_)
        | PipelineCommand::Prs(_)
        | PipelineCommand::Checks(_)
        | PipelineCommand::GateView(_)
        | PipelineCommand::GateRuns(_) => {
            unreachable!("OpenLibing pipeline command routed to GitCode workflow handler")
        }
    }
}

async fn openlibing_pipeline_command(
    command: PipelineCommand,
    openlibing_base: &str,
    credentials: &dyn CredentialStore,
    http: reqwest::Client,
    json_output: bool,
) -> anyhow::Result<()> {
    let base = openlibing_base_from_value(openlibing_base)?;
    let credential_key = openlibing_credential_key(&base);
    let credential = openlibing_credential(credentials, &credential_key)?;
    let client = OpenlibingClient::with_http_client(http, base.clone(), credential.clone());
    match command {
        PipelineCommand::Auth(command) => {
            pipeline_auth_command(command, credentials, &credential_key, &client, json_output).await
        }
        PipelineCommand::Config(args) => {
            require_openlibing_credential(credential.as_ref())?;
            let pipelines = client
                .list_pipelines(OpenlibingPipelineListRequest {
                    project_id: args.project_id.clone(),
                    pipeline_name: None,
                    page: 1,
                    per_page: 100,
                })
                .await?;
            let codecheck_rule_sets = client.codecheck_rule_sets(&args.project_id).await?;
            let summary = client.pipeline_summary(&args.project_id).await.ok();
            let value = json!({
                "project_id": args.project_id,
                "pipelines": data_or_value(&pipelines),
                "codecheck_rule_sets": data_or_value(&codecheck_rule_sets),
                "pipeline_summary": summary.as_ref().map(data_or_value),
                "raw": {
                    "pipelines": pipelines,
                    "codecheck_rule_sets": codecheck_rule_sets,
                    "pipeline_summary": summary,
                }
            });
            print_value(json_output, &value)
        }
        PipelineCommand::Setup(args) => {
            require_openlibing_credential(credential.as_ref())?;
            let value = pipeline_setup_command(&client, *args).await?;
            print_value(json_output, &value)
        }
        PipelineCommand::Prs(args) => {
            require_openlibing_credential(credential.as_ref())?;
            let value = client
                .list_pull_requests(PullRequestListRequest {
                    project_id: args.project_id.clone(),
                    repository: args.repository,
                    state: args.state,
                    page: args.page,
                    per_page: args.limit,
                })
                .await?;
            let output = json!({
                "project_id": args.project_id,
                "pull_requests": data_or_value(&value),
                "raw": value,
            });
            print_value(json_output, &output)
        }
        PipelineCommand::Checks(args) => {
            require_openlibing_credential(credential.as_ref())?;
            let checks = client
                .build_checks(&args.project_id, args.number, args.repository.as_deref())
                .await?;
            let value = json!({
                "project_id": args.project_id,
                "number": args.number,
                "repository": args.repository,
                "checks": data_or_value(&checks),
                "raw": checks,
            });
            print_value(json_output, &value)
        }
        PipelineCommand::GateView(args) => {
            require_openlibing_credential(credential.as_ref())?;
            let pull_request = client
                .pull_request(&args.project_id, args.number, args.repository.as_deref())
                .await?;
            let checks = client
                .build_checks(&args.project_id, args.number, args.repository.as_deref())
                .await
                .ok();
            let value = json!({
                "project_id": args.project_id,
                "number": args.number,
                "repository": args.repository,
                "pull_request": data_or_value(&pull_request),
                "checks": checks.as_ref().map(data_or_value),
                "raw": {
                    "pull_request": pull_request,
                    "checks": checks,
                }
            });
            print_value(json_output, &value)
        }
        PipelineCommand::GateRuns(args) => {
            require_openlibing_credential(credential.as_ref())?;
            let runs = client
                .list_pipelines(OpenlibingPipelineListRequest {
                    project_id: args.project_id.clone(),
                    pipeline_name: args.pipeline_name,
                    page: args.page,
                    per_page: args.limit,
                })
                .await?;
            let value = json!({
                "project_id": args.project_id,
                "runs": data_or_value(&runs),
                "raw": runs,
            });
            print_value(json_output, &value)
        }
        PipelineCommand::Set(_)
        | PipelineCommand::Codecheck(_)
        | PipelineCommand::List(_)
        | PipelineCommand::Run(_)
        | PipelineCommand::Runs(_)
        | PipelineCommand::View(_)
        | PipelineCommand::Log(_)
        | PipelineCommand::Stop(_)
        | PipelineCommand::Retry(_)
        | PipelineCommand::Rerun(_) => {
            unreachable!("GitCode workflow pipeline command routed to OpenLibing handler")
        }
    }
}

async fn pipeline_setup_command(
    client: &OpenlibingClient,
    args: PipelineSetupArgs,
) -> anyhow::Result<Value> {
    let repository = args.repository.as_deref();
    let repo_url = pipeline_repo_url(repository, args.repo_url.as_deref())?;
    let repo_ref = repository
        .map(ToString::to_string)
        .or_else(|| repository_from_url(&repo_url));
    let repo_name = args
        .repo_name
        .clone()
        .unwrap_or_else(|| repo_name_from_url(&repo_url));
    let repo_owner = args
        .repo_owner
        .clone()
        .or_else(|| repo_ref.as_deref().and_then(repository_owner))
        .with_context(|| "repo owner could not be inferred; pass --repo-owner")?;

    let repositories = client
        .query_repositories(RepositoryQueryRequest {
            project_id: args.project_id.clone(),
            repository: repo_ref.clone(),
            repo_id: args.repo_id,
            page: 1,
            per_page: 200,
        })
        .await
        .ok();
    let existing = repositories
        .as_ref()
        .and_then(|value| find_openlibing_repo(value, args.repo_id, &repo_url, &repo_name))
        .cloned();
    let repo_id = args
        .repo_id
        .or_else(|| existing.as_ref().and_then(repo_id_from_value));

    let languages = setup_languages(&args, existing.as_ref());
    let current_rule_sets = if let Some(repo_id) = repo_id {
        client.repo_rule_sets(repo_id).await.ok()
    } else {
        None
    };
    let codecheck_rules = if let Some(requested) = args.codecheck_rule_set.as_deref() {
        load_project_rule_sets(
            client.codecheck_rule_sets(&args.project_id).await,
            requested,
            "codecheck",
        )?
    } else {
        None
    };
    let anti_rules = if let Some(requested) = args.anti_rule_set.as_deref() {
        load_project_rule_sets(
            client.anti_rule_sets(&args.project_id).await,
            requested,
            "anti",
        )?
    } else {
        None
    };
    let codecheck_rule_set = rule_set_config(
        &languages,
        args.codecheck_rule_set.as_deref(),
        codecheck_rules.as_ref(),
        current_rule_sets.as_ref(),
        "codecheck",
    )?;
    let anti_rule_set = rule_set_config(
        &languages,
        args.anti_rule_set.as_deref(),
        anti_rules.as_ref(),
        current_rule_sets.as_ref(),
        "anti",
    )?;
    let public_token = args
        .public_token_env
        .as_deref()
        .map(read_non_empty_env)
        .transpose()?;
    let assume_pr_enabled = args.assume_pr == "1";
    let auto_trigger_enabled = args.auto_trigger == "1";

    let mut body = existing
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);
    body.insert(
        "projectId".to_string(),
        Value::String(args.project_id.clone()),
    );
    if let Some(repo_id) = repo_id {
        body.insert("repoId".to_string(), Value::from(repo_id));
    }
    body.insert("repoUrl".to_string(), Value::String(repo_url.clone()));
    body.insert("repoName".to_string(), Value::String(repo_name.clone()));
    body.insert("repoOwner".to_string(), Value::String(repo_owner));
    body.insert("platform".to_string(), Value::String("gitcode".to_string()));
    body.insert("purpose".to_string(), Value::String(args.purpose));
    body.insert("openSource".to_string(), Value::String(args.open_source));
    body.insert(
        "repoLanguage".to_string(),
        Value::String(languages.join(",")),
    );
    body.insert(
        "assumePr".to_string(),
        Value::String(args.assume_pr.clone()),
    );
    body.insert(
        "autoTrigger".to_string(),
        Value::String(args.auto_trigger.clone()),
    );
    body.insert(
        "autoTriggerDesignScan".to_string(),
        Value::String(args.auto_trigger_design_scan),
    );
    body.insert(
        "disallowSelfMerge".to_string(),
        Value::from(args.disallow_self_merge),
    );
    body.insert(
        "disallowUnresolvedDiscussionsMerge".to_string(),
        Value::from(args.disallow_unresolved_discussions_merge),
    );
    if let Some(value) = codecheck_rule_set.clone() {
        body.insert("codecheckRuleSet".to_string(), value);
    }
    if let Some(value) = anti_rule_set.clone() {
        body.insert("antiRuleSet".to_string(), value);
    }
    body.insert(
        "isEditAccessToken".to_string(),
        Value::Bool(public_token.is_some()),
    );
    if let Some(token) = public_token {
        body.insert("accessToken".to_string(), Value::String(token));
    }

    let is_update = repo_id.is_some();
    let setup_response = if is_update {
        client
            .update_repository(Value::Object(body))
            .await
            .map_err(|error| openlibing_repo_setup_error(error, true))?
    } else {
        client
            .add_repository(Value::Object(body))
            .await
            .map_err(|error| openlibing_repo_setup_error(error, false))?
    };
    let configured_repo_id = repo_id.or_else(|| repo_id_from_value(&setup_response));
    let mut webhook_error = None;
    let webhook_response = if !args.no_configure_webhook {
        if let Some(repo_id) = configured_repo_id {
            match client.auto_set_webhook(repo_id).await {
                Ok(value) => Some(value),
                Err(error) => {
                    webhook_error = Some(openlibing_webhook_setup_message(&error));
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    Ok(json!({
        "project_id": args.project_id,
        "repository": repo_ref,
        "repo_url": repo_url,
        "repo_name": repo_name,
        "repo_id": configured_repo_id,
        "mode": if is_update { "update" } else { "add" },
        "assume_pr": assume_pr_enabled,
        "auto_trigger": auto_trigger_enabled,
        "codecheck_rule_set_configured": codecheck_rule_set.as_ref().is_some_and(rule_set_value_is_non_empty),
        "anti_rule_set_configured": anti_rule_set.as_ref().is_some_and(rule_set_value_is_non_empty),
        "webhook_configured": webhook_response.is_some(),
        "raw": {
            "repository_lookup": repositories.as_ref().map(redact_sensitive_value),
            "current_rule_sets": current_rule_sets.as_ref().map(redact_sensitive_value),
            "setup": redact_sensitive_value(&setup_response),
            "webhook": webhook_response.as_ref().map(redact_sensitive_value),
            "webhook_error": webhook_error,
        }
    }))
}

fn pipeline_repo_url(repository: Option<&str>, repo_url: Option<&str>) -> anyhow::Result<String> {
    if let Some(repo_url) = repo_url.filter(|value| !value.trim().is_empty()) {
        return Ok(repo_url.trim().to_string());
    }
    let repository = repository
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("repository or repo URL is required"))?;
    if repository.split_once('/').is_none() {
        bail!("repository must be in owner/repo form: {repository}");
    }
    Ok(format!("https://gitcode.com/{repository}"))
}

fn repository_from_url(repo_url: &str) -> Option<String> {
    let trimmed = repo_url.trim().trim_end_matches(".git");
    if let Some((_, path)) = trimmed.split_once("gitcode.com:") {
        return owner_repo_from_path(path);
    }
    if let Some((_, path)) = trimmed.split_once("gitcode.com/") {
        return owner_repo_from_path(path);
    }
    owner_repo_from_path(trimmed)
}

fn owner_repo_from_path(path: &str) -> Option<String> {
    let mut parts = path
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.trim().is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?;
    Some(format!("{owner}/{repo}"))
}

fn repo_name_from_url(repo_url: &str) -> String {
    repo_url
        .trim()
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("repo")
        .to_string()
}

fn repository_owner(repository: &str) -> Option<String> {
    repository
        .split_once('/')
        .map(|(owner, _)| owner.to_string())
        .filter(|owner| !owner.trim().is_empty())
}

fn setup_languages(args: &PipelineSetupArgs, existing: Option<&Value>) -> Vec<String> {
    let languages = if args.language.is_empty() {
        existing
            .and_then(|value| value.get("repoLanguage"))
            .and_then(Value::as_str)
            .map(split_languages)
            .unwrap_or_else(|| vec!["Rust".to_string()])
    } else {
        args.language.clone()
    };
    let mut normalized = Vec::new();
    for language in languages {
        let language = language.trim();
        if !language.is_empty() && !normalized.iter().any(|value| value == language) {
            normalized.push(language.to_string());
        }
    }
    if normalized.is_empty() {
        normalized.push("Rust".to_string());
    }
    normalized
}

fn split_languages(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn find_openlibing_repo<'a>(
    value: &'a Value,
    repo_id: Option<u64>,
    repo_url: &str,
    repo_name: &str,
) -> Option<&'a Value> {
    let mut repositories = Vec::new();
    collect_objects(value, &mut repositories);
    repositories.into_iter().find(|candidate| {
        repo_id.is_some_and(|id| repo_id_from_value(candidate) == Some(id))
            || string_field(candidate, &["repoUrl", "url"])
                .is_some_and(|value| same_repo_url(&value, repo_url))
            || string_field(candidate, &["repoName", "name"])
                .is_some_and(|value| value.eq_ignore_ascii_case(repo_name))
    })
}

fn collect_objects<'a>(value: &'a Value, objects: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            objects.push(value);
            for value in map.values() {
                collect_objects(value, objects);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_objects(value, objects);
            }
        }
        _ => {}
    }
}

fn same_repo_url(left: &str, right: &str) -> bool {
    normalize_repo_url(left) == normalize_repo_url(right)
}

fn normalize_repo_url(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn repo_id_from_value(value: &Value) -> Option<u64> {
    let data = data_or_value(value);
    first_number_field(value, &["repoId", "id"])
        .or_else(|| first_number_field(&data, &["repoId", "id"]))
        .or_else(|| {
            data.as_object().and_then(|object| {
                object
                    .values()
                    .find_map(|value| first_number_field(value, &["repoId", "id"]))
            })
        })
}

fn first_number_field(value: &Value, names: &[&str]) -> Option<u64> {
    let object = value.as_object()?;
    names.iter().find_map(|name| {
        object
            .get(*name)
            .and_then(Value::as_u64)
            .or_else(|| object.get(*name).and_then(Value::as_str)?.parse().ok())
    })
}

fn rule_set_config(
    languages: &[String],
    requested: Option<&str>,
    project_rules: Option<&Value>,
    current_rules: Option<&Value>,
    kind: &str,
) -> anyhow::Result<Option<Value>> {
    if let Some(requested) = requested.filter(|value| !value.trim().is_empty()) {
        let values =
            languages
                .iter()
                .map(|language| {
                    let rule_set_id = if let Some(rules) = project_rules {
                        find_rule_set_id(rules, language, requested).with_context(|| {
                            format!(
                                "could not find {kind} rule set '{requested}' for language {language}"
                            )
                        })?
                    } else if looks_like_rule_set_id(requested) {
                        requested.trim().to_string()
                    } else {
                        bail!(
                            "OpenLibing {kind} rule set list was not loaded; pass a rule set id instead of name"
                        );
                    };
                    Ok(json!({
                        "language": language,
                        "ruleSetId": rule_set_id,
                    }))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
        return Ok(Some(Value::Array(values)));
    }

    let Some(current_rules) = current_rules else {
        return Ok(None);
    };
    let values = current_repo_rule_sets(current_rules, kind);
    if values.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Array(values)))
    }
}

fn load_project_rule_sets(
    result: anyhow::Result<Value>,
    requested: &str,
    kind: &str,
) -> anyhow::Result<Option<Value>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if looks_like_rule_set_id(requested) && should_use_direct_rule_set_id(&error) =>
        {
            Ok(None)
        }
        Err(error) => {
            Err(error).with_context(|| format!("failed to load OpenLibing {kind} rule sets"))
        }
    }
}

fn current_repo_rule_sets(value: &Value, kind: &str) -> Vec<Value> {
    let root = data_or_value(value);
    let Some(values) = root.get(kind).and_then(Value::as_array) else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|value| {
            let language = string_field(value, &["language"])?;
            let rule_set_id = string_field(value, &["ruleSetId", "templateId", "id"])?;
            Some(json!({
                "language": language,
                "ruleSetId": rule_set_id,
            }))
        })
        .collect()
}

fn find_rule_set_id(value: &Value, language: &str, requested: &str) -> Option<String> {
    let mut candidates = Vec::new();
    collect_objects(value, &mut candidates);
    let requested = requested.trim();
    let matches = |candidate: &Value| {
        string_field(candidate, &["templateId", "ruleSetId", "id"])
            .is_some_and(|value| value == requested)
            || string_field(candidate, &["templateName", "name"])
                .is_some_and(|value| value.eq_ignore_ascii_case(requested))
    };
    let language_matches = |candidate: &Value| {
        string_field(candidate, &["language"]).is_none_or(|value| same_language(&value, language))
    };
    candidates
        .iter()
        .copied()
        .find(|candidate| matches(candidate) && language_matches(candidate))
        .or_else(|| {
            candidates
                .iter()
                .copied()
                .find(|candidate| matches(candidate))
        })
        .and_then(|candidate| string_field(candidate, &["templateId", "ruleSetId", "id"]))
}

fn same_language(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn looks_like_rule_set_id(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn should_use_direct_rule_set_id(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("401 Unauthorized")
        || message.contains("403 Forbidden")
        || message.contains("404 Not Found")
}

fn openlibing_repo_setup_error(error: anyhow::Error, is_update: bool) -> anyhow::Error {
    let action = if is_update { "update" } else { "add" };
    if is_openlibing_permission_error(&error) {
        return anyhow::anyhow!(
            "OpenLibing repository {action} failed: {error}. Correct setup requires an OpenLibing project administrator or equivalent project approver with repository information maintenance permission. The repository must be recorded in OpenLibing Code Repository Management, PR takeover must be enabled, CodeCheck language/rule-set configuration must be selected, a GitCode public or robot account must have repository access, and webhook configuration must be allowed."
        );
    }
    anyhow::anyhow!("OpenLibing repository {action} failed: {error}")
}

fn openlibing_webhook_setup_message(error: &anyhow::Error) -> String {
    if is_openlibing_permission_error(error) {
        return format!(
            "{error}; webhook configuration requires OpenLibing repository maintenance permission and GitCode repository permission for the configured public or robot account"
        );
    }
    error.to_string()
}

fn is_openlibing_permission_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("401 Unauthorized") || message.contains("403 Forbidden")
}

fn read_non_empty_env(name: &str) -> anyhow::Result<String> {
    let value = std::env::var(name)
        .with_context(|| format!("{name} is not set"))?
        .trim()
        .to_string();
    if value.is_empty() {
        bail!("{name} is empty");
    }
    Ok(value)
}

fn rule_set_value_is_non_empty(value: &Value) -> bool {
    value.as_array().is_some_and(|values| !values.is_empty())
}

fn redact_sensitive_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_sensitive_key(key) {
                        (key.clone(), Value::String("<redacted>".to_string()))
                    } else {
                        (key.clone(), redact_sensitive_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(redact_sensitive_value).collect()),
        other => other.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "accesstoken" | "access_token" | "token" | "authorization" | "cookie"
    )
}

async fn pipeline_auth_command(
    command: PipelineAuthCommand,
    credentials: &dyn CredentialStore,
    credential_key: &str,
    client: &OpenlibingClient,
    json_output: bool,
) -> anyhow::Result<()> {
    match command {
        PipelineAuthCommand::Login(args) => {
            let listener = TcpListener::bind((args.callback_host.as_str(), args.callback_port))
                .with_context(|| {
                    format!(
                        "failed to bind OAuth callback listener on {}:{}",
                        args.callback_host, args.callback_port
                    )
                })?;
            let callback_addr = listener
                .local_addr()
                .context("failed to read OAuth callback listener address")?;
            let callback_url = format!("http://{callback_addr}/openlibing/oauth/callback");
            let state = oauth_state();
            let authorize_url = client.oauth_authorize_url(&callback_url, &state)?;
            if args.no_browser {
                println!("{authorize_url}");
            } else {
                open_browser(authorize_url.as_str())?;
            }
            let callback =
                wait_for_oauth_callback(listener, state, Duration::from_secs(args.timeout_seconds))
                    .await?;
            let credential = credential_from_callback(callback)?;
            let stored = serde_json::to_string(&credential)?;
            credential_set_token(credentials, credential_key, &stored)?;
            let value = json!({
                "status": "logged_in",
                "credential_store": "system_keyring",
                "openlibing": credential_key,
            });
            print_value(json_output, &value)
        }
        PipelineAuthCommand::Status => {
            let logged_in = OpenlibingCredential::from_environment().is_some()
                || credential_get_stored_token(credentials, credential_key)?.is_some();
            let remote = if logged_in {
                client.gitcode_auth_status().await.ok()
            } else {
                None
            };
            let value = json!({
                "openlibing": credential_key,
                "logged_in": logged_in,
                "source": if OpenlibingCredential::from_environment().is_some() {
                    "env"
                } else {
                    "system_keyring"
                },
                "remote": remote,
            });
            print_value(json_output, &value)
        }
        PipelineAuthCommand::Logout => {
            credential_delete_token(credentials, credential_key)?;
            let value = json!({
                "openlibing": credential_key,
                "status": "logged_out",
            });
            print_value(json_output, &value)
        }
    }
}

fn openlibing_credential(
    credentials: &dyn CredentialStore,
    credential_key: &str,
) -> anyhow::Result<Option<OpenlibingCredential>> {
    if let Some(credential) = OpenlibingCredential::from_environment() {
        return Ok(Some(credential));
    }
    credential_get_stored_token(credentials, credential_key)?
        .map(|value| OpenlibingCredential::from_stored_json(&value))
        .transpose()
}

fn require_openlibing_credential(credential: Option<&OpenlibingCredential>) -> anyhow::Result<()> {
    if credential.is_some_and(OpenlibingCredential::is_present) {
        return Ok(());
    }
    bail!(
        "not logged in to OpenLibing; run gd pipeline auth login or set GD_OPENLIBING_TOKEN/GD_OPENLIBING_COOKIE"
    )
}

fn open_browser(url: &str) -> anyhow::Result<()> {
    if let Ok(browser) = std::env::var("BROWSER")
        && !browser.trim().is_empty()
    {
        let status = std::process::Command::new(browser)
            .arg(url)
            .status()
            .context("failed to run BROWSER for OpenLibing OAuth")?;
        if status.success() {
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    let candidates: &[(&str, &[&str])] = &[("open", &[url])];
    #[cfg(target_os = "windows")]
    let candidates: &[(&str, &[&str])] = &[("cmd", &["/C", "start", "", url])];
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let candidates: &[(&str, &[&str])] = &[("xdg-open", &[url])];

    for (program, args) in candidates {
        if let Ok(status) = std::process::Command::new(program).args(*args).status()
            && status.success()
        {
            return Ok(());
        }
    }
    bail!("failed to open browser; rerun with --no-browser and open the URL manually")
}

fn completion(args: CompletionArgs) -> anyhow::Result<()> {
    let mut command = Cli::command_for_completion();
    let name = command.get_name().to_string();
    match args.shell {
        Shell::Bash => generate_completion(CompleteShell::Bash, &mut command, &name),
        Shell::Zsh => generate_completion(CompleteShell::Zsh, &mut command, &name),
        Shell::Fish => generate_completion(CompleteShell::Fish, &mut command, &name),
        Shell::Powershell => generate_completion(CompleteShell::PowerShell, &mut command, &name),
    }
    Ok(())
}

fn generate_completion<G: Generator>(generator: G, command: &mut clap::Command, name: &str) {
    generate(generator, command, name, &mut std::io::stdout());
}

fn join(values: Vec<String>) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(values.join(","))
    }
}

#[derive(Debug)]
struct PipelineRepoContext {
    project_id: String,
    repo_https_url: String,
}

async fn pipeline_repo_context(
    client: &GitcodeClient,
    config: &Config,
    repository: Option<&str>,
) -> anyhow::Result<PipelineRepoContext> {
    let repository = repo::resolve_repo(repository, config.default_repo.as_deref()).await?;
    let value = client.get(&format!("repos/{repository}"), &[]).await?;
    let project_id = string_field(&value, &["id", "project_id"])
        .ok_or_else(|| anyhow::anyhow!("repo response did not include a project id"))?;
    let repo_https_url = string_field(&value, &["http_url_to_repo", "clone_url"])
        .unwrap_or_else(|| repo::clone_url(&config.hostname, &repository));
    Ok(PipelineRepoContext {
        project_id,
        repo_https_url,
    })
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key).and_then(value_as_string))
        .find(|value| !value.trim().is_empty())
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

async fn read_required_content(
    file_content: Option<String>,
    file: Option<&Path>,
) -> anyhow::Result<String> {
    if let Some(file) = file {
        let content = if file == Path::new("-") {
            let mut content = String::new();
            tokio::io::stdin()
                .read_to_string(&mut content)
                .await
                .context("failed to read workflow content from stdin")?;
            content
        } else {
            tokio::fs::read_to_string(file)
                .await
                .with_context(|| format!("failed to read {}", file.display()))?
        };
        return Ok(content);
    }
    file_content.ok_or_else(|| anyhow::anyhow!("set workflow content with --content or --file"))
}

fn print_pipeline_log(json_output: bool, value: &Value) -> anyhow::Result<()> {
    if json_output {
        return print_value(true, value);
    }
    if let Some(log) = extract_log_text(value) {
        print!("{log}");
        if !log.ends_with('\n') {
            println!();
        }
        Ok(())
    } else {
        print_value(false, value)
    }
}
