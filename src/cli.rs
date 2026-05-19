use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "gd",
    version,
    about = "Work with GitCode from the command line",
    long_about = "A GitCode CLI with gh-style command naming for GitCode API v5."
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn command_for_completion() -> clap::Command {
        Self::command()
    }
}

#[derive(Debug, Args, Clone)]
pub struct GlobalArgs {
    #[arg(long, global = true, default_value = "gitcode.com")]
    pub hostname: String,
    #[arg(
        long,
        global = true,
        env = "GITCODE_API_BASE",
        default_value = "https://api.gitcode.com/api/v5"
    )]
    pub api_base: String,
    #[arg(long, global = true, help = "Render command output as JSON")]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Authenticate gd with GitCode")]
    #[command(subcommand)]
    Auth(AuthCommand),
    #[command(about = "Make an authenticated GitCode API request")]
    Api(ApiArgs),
    #[command(about = "Manage GitCode repositories")]
    #[command(subcommand)]
    Repo(RepoCommand),
    #[command(about = "Manage pull requests")]
    #[command(subcommand, visible_alias = "mr")]
    Pr(PrCommand),
    #[command(about = "Manage issues")]
    #[command(subcommand)]
    Issue(IssueCommand),
    #[command(about = "Search GitCode")]
    #[command(subcommand)]
    Search(SearchCommand),
    #[command(about = "Manage SSH keys")]
    #[command(name = "ssh-key", subcommand)]
    SshKey(SshKeyCommand),
    #[command(about = "Manage repository labels")]
    #[command(subcommand)]
    Label(LabelCommand),
    #[command(about = "Manage repository releases")]
    #[command(subcommand)]
    Release(ReleaseCommand),
    #[command(about = "Manage GitCode pipelines", visible_alias = "actions")]
    #[command(subcommand)]
    Pipeline(PipelineCommand),
    #[command(about = "Generate shell completion scripts")]
    Completion(CompletionArgs),
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    #[command(about = "Log in to GitCode")]
    Login(AuthLoginArgs),
    #[command(about = "Log out of GitCode")]
    Logout(AuthHostArgs),
    #[command(about = "View authentication status")]
    Status(AuthStatusArgs),
    #[command(about = "Print the stored authentication token")]
    Token(AuthHostArgs),
}

#[derive(Debug, Args)]
pub struct AuthLoginArgs {
    #[arg(long, help = "Read a GitCode personal access token from stdin")]
    pub with_token: bool,
}

#[derive(Debug, Args)]
pub struct AuthHostArgs {
    #[arg(long)]
    pub hostname: Option<String>,
}

#[derive(Debug, Args)]
pub struct AuthStatusArgs {
    #[arg(long)]
    pub hostname: Option<String>,
    #[arg(short = 't', long)]
    pub show_token: bool,
}

#[derive(Debug, Args)]
pub struct ApiArgs {
    pub endpoint: String,
    #[arg(short = 'X', long, default_value = "GET")]
    pub method: String,
    #[arg(short = 'H', long = "header")]
    pub headers: Vec<String>,
    #[arg(short = 'f', long = "raw-field")]
    pub raw_fields: Vec<String>,
    #[arg(short = 'F', long = "field")]
    pub fields: Vec<String>,
    #[arg(long)]
    pub input: Option<PathBuf>,
    #[arg(short = 'i', long)]
    pub include: bool,
    #[arg(long)]
    pub silent: bool,
    #[arg(long)]
    pub paginate: bool,
}

#[derive(Debug, Subcommand)]
pub enum RepoCommand {
    #[command(about = "View a repository")]
    View(RepoViewArgs),
    #[command(about = "List repositories")]
    List(RepoListArgs),
    #[command(about = "Clone a repository")]
    Clone(RepoCloneArgs),
    #[command(about = "Fork a repository")]
    Fork(RepoRefArgs),
    #[command(about = "Create a repository")]
    Create(RepoCreateArgs),
}

#[derive(Debug, Args)]
pub struct RepoViewArgs {
    pub repository: Option<String>,
}

#[derive(Debug, Args)]
pub struct RepoListArgs {
    pub owner: Option<String>,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct RepoCloneArgs {
    pub repository: String,
    pub directory: Option<PathBuf>,
    #[arg(last = true)]
    pub git_flags: Vec<String>,
}

#[derive(Debug, Args)]
pub struct RepoRefArgs {
    pub repository: String,
}

#[derive(Debug, Args)]
pub struct RepoCreateArgs {
    pub name: String,
    #[arg(long)]
    pub private: bool,
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    #[command(about = "List issues")]
    List(IssueListArgs),
    #[command(about = "View an issue")]
    View(IssueViewArgs),
    #[command(about = "Create an issue")]
    Create(IssueCreateArgs),
    #[command(about = "Add a comment to an issue")]
    Comment(IssueCommentArgs),
}

#[derive(Debug, Args)]
pub struct IssueListArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 's', long, default_value = "open")]
    pub state: String,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct IssueViewArgs {
    pub number: u64,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueCreateArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 't', long)]
    pub title: String,
    #[arg(short = 'b', long)]
    pub body: Option<String>,
    #[arg(short = 'l', long)]
    pub label: Vec<String>,
    #[arg(short = 'a', long)]
    pub assignee: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueCommentArgs {
    pub number: u64,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 'b', long)]
    pub body: String,
}

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    #[command(about = "List pull requests")]
    List(PrListArgs),
    #[command(about = "View a pull request")]
    View(PrViewArgs),
    #[command(about = "Create a pull request")]
    Create(PrCreateArgs),
}

#[derive(Debug, Args)]
pub struct PrListArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 's', long, default_value = "open")]
    pub state: String,
    #[arg(short = 'B', long)]
    pub base: Option<String>,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct PrViewArgs {
    pub number: u64,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
}

#[derive(Debug, Args)]
pub struct PrCreateArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 't', long)]
    pub title: String,
    #[arg(short = 'b', long)]
    pub body: Option<String>,
    #[arg(short = 'B', long)]
    pub base: String,
    #[arg(short = 'H', long)]
    pub head: String,
    #[arg(short = 'l', long)]
    pub label: Vec<String>,
    #[arg(short = 'a', long)]
    pub assignee: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum SearchCommand {
    #[command(about = "Search repositories")]
    Repos(SearchArgs),
    #[command(about = "Search issues")]
    Issues(SearchArgs),
    #[command(about = "Search users")]
    Users(SearchArgs),
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: String,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub limit: u32,
}

#[derive(Debug, Subcommand)]
pub enum SshKeyCommand {
    #[command(about = "List SSH keys")]
    List,
    #[command(about = "Add an SSH key")]
    Add(SshKeyAddArgs),
    #[command(about = "Delete an SSH key")]
    Delete(SshKeyDeleteArgs),
}

#[derive(Debug, Args)]
pub struct SshKeyAddArgs {
    pub key_file: PathBuf,
    #[arg(short = 't', long)]
    pub title: Option<String>,
}

#[derive(Debug, Args)]
pub struct SshKeyDeleteArgs {
    pub id: String,
}

#[derive(Debug, Subcommand)]
pub enum LabelCommand {
    #[command(about = "List repository labels")]
    List(LabelListArgs),
    #[command(about = "Create a repository label")]
    Create(LabelCreateArgs),
    #[command(about = "Edit a repository label")]
    Edit(LabelEditArgs),
    #[command(about = "Delete a repository label")]
    Delete(LabelDeleteArgs),
}

#[derive(Debug, Args)]
pub struct LabelListArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 'L', long, default_value_t = 100)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct LabelCreateArgs {
    pub name: String,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 'c', long)]
    pub color: Option<String>,
    #[arg(short = 'd', long)]
    pub description: Option<String>,
}

#[derive(Debug, Args)]
pub struct LabelEditArgs {
    pub name: String,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(long)]
    pub new_name: Option<String>,
    #[arg(short = 'c', long)]
    pub color: Option<String>,
    #[arg(short = 'd', long)]
    pub description: Option<String>,
}

#[derive(Debug, Args)]
pub struct LabelDeleteArgs {
    pub name: String,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ReleaseCommand {
    #[command(about = "List repository releases")]
    List(ReleaseListArgs),
    #[command(about = "View a repository release")]
    View(ReleaseViewArgs),
    #[command(about = "Create a repository release")]
    Create(ReleaseCreateArgs),
}

#[derive(Debug, Args)]
pub struct ReleaseListArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct ReleaseViewArgs {
    pub tag: String,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
}

#[derive(Debug, Args)]
pub struct ReleaseCreateArgs {
    pub tag: String,
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(short = 't', long)]
    pub title: Option<String>,
    #[arg(short = 'n', long)]
    pub notes: Option<String>,
    #[arg(long)]
    pub target: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum PipelineCommand {
    #[command(
        about = "Create or update a GitCode workflow file",
        visible_alias = "register"
    )]
    Set(PipelineSetArgs),
    #[command(about = "List GitCode workflow files")]
    List(PipelineListArgs),
    #[command(about = "Manually run a GitCode workflow")]
    Run(PipelineRunArgs),
    #[command(about = "List GitCode workflow run records")]
    Runs(PipelineRunsArgs),
    #[command(about = "View a GitCode workflow run")]
    View(PipelineViewArgs),
    #[command(about = "Read a GitCode workflow job log")]
    Log(PipelineLogArgs),
    #[command(about = "Stop a GitCode workflow run")]
    Stop(PipelineStopArgs),
    #[command(about = "Retry failed GitCode workflow jobs")]
    Retry(PipelineRetryArgs),
    #[command(about = "Rerun all jobs in a GitCode workflow run")]
    Rerun(PipelineRerunArgs),
}

#[derive(Debug, Args)]
pub struct PipelineSetArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    pub path: String,
    #[arg(long, value_enum, default_value_t = PipelineSetMode::Create)]
    pub mode: PipelineSetMode,
    #[arg(long)]
    pub content: Option<String>,
    #[arg(long)]
    pub file: Option<PathBuf>,
    #[arg(short = 'm', long, default_value = "Configure GitCode workflow")]
    pub message: String,
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long)]
    pub sha: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PipelineSetMode {
    Create,
    Update,
}

#[derive(Debug, Args)]
pub struct PipelineListArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: u64,
    #[arg(short = 'L', long = "limit", default_value_t = 50)]
    pub limit: u64,
}

#[derive(Debug, Args)]
pub struct PipelineRunArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    pub workflow_id: String,
    #[arg(long)]
    pub file_path: String,
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long)]
    pub branch_commit_id: Option<String>,
    #[arg(long = "input")]
    pub inputs: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PipelineRunsArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    #[arg(long)]
    pub workflow_id: Option<String>,
    #[arg(long)]
    pub workflow_name: Option<String>,
    #[arg(long)]
    pub event: Option<String>,
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub executor_id: Option<String>,
    #[arg(long)]
    pub mr_id: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: u64,
    #[arg(short = 'L', long = "limit", default_value_t = 20)]
    pub limit: u64,
}

#[derive(Debug, Args)]
pub struct PipelineViewArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    pub workflow_run_id: String,
}

#[derive(Debug, Args)]
pub struct PipelineLogArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    pub workflow_run_id: String,
    pub job_identifier: String,
    #[arg(long)]
    pub step_run_id: Option<String>,
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
    #[arg(long, default_value_t = 100)]
    pub limit: u64,
}

#[derive(Debug, Args)]
pub struct PipelineStopArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    pub workflow_run_id: String,
}

#[derive(Debug, Args)]
pub struct PipelineRetryArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    pub workflow_run_id: String,
    #[arg(long = "job-run-id")]
    pub job_run_ids: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PipelineRerunArgs {
    #[arg(short = 'R', long = "repo")]
    pub repository: Option<String>,
    pub workflow_run_id: String,
}

#[derive(Debug, Args)]
pub struct CompletionArgs {
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_view_with_json_flag() {
        let cli = Cli::try_parse_from(["gd", "--json", "repo", "view", "owner/repo"]).unwrap();
        assert!(cli.global.json);
        match cli.command {
            Command::Repo(RepoCommand::View(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pr_create() {
        let cli = Cli::try_parse_from([
            "gd",
            "pr",
            "create",
            "-R",
            "owner/repo",
            "--title",
            "change",
            "--body",
            "body",
            "--base",
            "main",
            "--head",
            "feature",
        ])
        .unwrap();

        match cli.command {
            Command::Pr(PrCommand::Create(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.title, "change");
                assert_eq!(args.base, "main");
                assert_eq!(args.head, "feature");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_api_typed_and_raw_fields() {
        let cli =
            Cli::try_parse_from(["gd", "api", "/user", "-X", "POST", "-f", "a=b", "-F", "n=2"])
                .unwrap();

        match cli.command {
            Command::Api(args) => {
                assert_eq!(args.method, "POST");
                assert_eq!(args.endpoint, "/user");
                assert_eq!(args.raw_fields, ["a=b"]);
                assert_eq!(args.fields, ["n=2"]);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_set() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "set",
            "--repo",
            "owner/repo",
            ".gitcode/workflows/ci.yml",
            "--content",
            "name: ci",
            "--mode",
            "update",
            "--sha",
            "abc",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Set(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.path, ".gitcode/workflows/ci.yml");
                assert_eq!(args.content.as_deref(), Some("name: ci"));
                assert_eq!(args.mode, PipelineSetMode::Update);
                assert_eq!(args.sha.as_deref(), Some("abc"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_register_alias() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "register",
            "--repo",
            "owner/repo",
            ".gitcode/workflows/ci.yml",
            "--content",
            "name: ci",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Command::Pipeline(PipelineCommand::Set(_))
        ));
    }

    #[test]
    fn parses_actions_alias() {
        let cli =
            Cli::try_parse_from(["gd", "actions", "list", "--repo", "owner/repo", "-L", "10"])
                .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::List(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.limit, 10);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_run() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "run",
            "--repo",
            "owner/repo",
            "workflow-1",
            "--file-path",
            ".gitcode/workflows/ci.yml",
            "--branch",
            "main",
            "--branch-commit-id",
            "abc",
            "--input",
            "dry_run=true",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Run(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.workflow_id, "workflow-1");
                assert_eq!(args.file_path, ".gitcode/workflows/ci.yml");
                assert_eq!(args.branch.as_deref(), Some("main"));
                assert_eq!(args.branch_commit_id.as_deref(), Some("abc"));
                assert_eq!(args.inputs, ["dry_run=true"]);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_runs() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "runs",
            "--repo",
            "owner/repo",
            "--workflow-name",
            "ci",
            "--event",
            "push",
            "--page",
            "2",
            "-L",
            "20",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Runs(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.workflow_name.as_deref(), Some("ci"));
                assert_eq!(args.event.as_deref(), Some("push"));
                assert_eq!(args.page, 2);
                assert_eq!(args.limit, 20);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_view_log_stop_retry_and_rerun() {
        let view =
            Cli::try_parse_from(["gd", "pipeline", "view", "--repo", "owner/repo", "run"]).unwrap();
        match view.command {
            Command::Pipeline(PipelineCommand::View(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.workflow_run_id, "run");
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let log = Cli::try_parse_from([
            "gd",
            "pipeline",
            "log",
            "--repo",
            "owner/repo",
            "run",
            "job",
            "--step-run-id",
            "step",
            "--limit",
            "7",
        ])
        .unwrap();
        match log.command {
            Command::Pipeline(PipelineCommand::Log(args)) => {
                assert_eq!(args.workflow_run_id, "run");
                assert_eq!(args.job_identifier, "job");
                assert_eq!(args.step_run_id.as_deref(), Some("step"));
                assert_eq!(args.limit, 7);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let stop =
            Cli::try_parse_from(["gd", "pipeline", "stop", "-R", "owner/repo", "run"]).unwrap();
        assert!(matches!(
            stop.command,
            Command::Pipeline(PipelineCommand::Stop(_))
        ));

        let retry = Cli::try_parse_from([
            "gd",
            "pipeline",
            "retry",
            "-R",
            "owner/repo",
            "run",
            "--job-run-id",
            "job",
        ])
        .unwrap();
        match retry.command {
            Command::Pipeline(PipelineCommand::Retry(args)) => {
                assert_eq!(args.job_run_ids, ["job"]);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let rerun =
            Cli::try_parse_from(["gd", "pipeline", "rerun", "-R", "owner/repo", "run"]).unwrap();
        assert!(matches!(
            rerun.command,
            Command::Pipeline(PipelineCommand::Rerun(_))
        ));
    }
}
