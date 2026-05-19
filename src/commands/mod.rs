use std::io::Read;

use anyhow::{Context, bail};
use clap_complete::{Generator, Shell as CompleteShell, generate};
use serde_json::json;

use crate::{
    auth::CredentialStore,
    cli::{
        AuthCommand, Cli, Command, CompletionArgs, IssueCommand, LabelCommand, PrCommand,
        ReleaseCommand, RepoCommand, SearchCommand, Shell, SshKeyCommand,
    },
    client::{ApiRequest, GitcodeClient, form_body, merge_pages, query},
    config::Config,
    output::print_value,
    repo,
};

pub async fn run(
    cli: Cli,
    mut config: Config,
    credentials: &dyn CredentialStore,
) -> anyhow::Result<()> {
    match cli.command {
        Command::Completion(args) => completion(args),
        Command::Auth(command) => auth(command, &mut config, credentials, cli.global.json).await,
        other => {
            let token = credentials.get_token(&config.hostname)?;
            let client = GitcodeClient::new(config.api_base_url()?, token);
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
                        print_value(cli.global.json, &merge_pages(responses))?;
                    }
                    Ok(())
                }
                Command::Repo(command) => {
                    repo_command(command, &config, &client, cli.global.json).await
                }
                Command::Issue(command) => {
                    issue_command(command, &config, &client, cli.global.json).await
                }
                Command::Pr(command) => {
                    pr_command(command, &config, &client, cli.global.json).await
                }
                Command::Search(command) => search_command(command, &client, cli.global.json).await,
                Command::SshKey(command) => {
                    ssh_key_command(command, &client, cli.global.json).await
                }
                Command::Label(command) => {
                    label_command(command, &config, &client, cli.global.json).await
                }
                Command::Release(command) => {
                    release_command(command, &config, &client, cli.global.json).await
                }
                Command::Completion(_) | Command::Auth(_) => unreachable!(),
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
            std::io::stdin()
                .read_to_string(&mut token)
                .context("failed to read token from stdin")?;
            let token = token.trim();
            if token.is_empty() {
                bail!("token from stdin is empty");
            }
            credentials.set_token(&config.hostname, token)?;
            config.save()?;
            let value = json!({
                "hostname": config.hostname,
                "status": "logged_in",
                "credential_store": "system_keyring"
            });
            print_value(json_output, &value)
        }
        AuthCommand::Logout(args) => {
            let hostname = args.hostname.unwrap_or_else(|| config.hostname.clone());
            credentials.delete_token(&hostname)?;
            let value = json!({"hostname": hostname, "status": "logged_out"});
            print_value(json_output, &value)
        }
        AuthCommand::Status(args) => {
            let hostname = args.hostname.unwrap_or_else(|| config.hostname.clone());
            let token = credentials.get_token(&hostname)?;
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
            let Some(token) = credentials.get_token(&hostname)? else {
                bail!("not logged in to {hostname}");
            };
            println!("{token}");
            Ok(())
        }
    }
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
        RepoCommand::Clone(args) => repo::run_git_clone(
            &config.hostname,
            &args.repository,
            args.directory,
            &args.git_flags,
        ),
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
            let value = client
                .get(&format!("repos/{repository}/issues/{}", args.number), &[])
                .await?;
            print_value(json_output, &value)
        }
        IssueCommand::Create(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
            let value = client
                .get(&format!("repos/{repository}/pulls/{}", args.number), &[])
                .await?;
            print_value(json_output, &value)
        }
        PrCommand::Create(args) => {
            let repository =
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
            let key = std::fs::read_to_string(&args.key_file)
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
                repo::resolve_repo(args.repository.as_deref(), config.default_repo.as_deref())?;
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
