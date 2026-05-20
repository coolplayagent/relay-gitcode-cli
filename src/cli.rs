use std::{collections::BTreeSet, ffi::OsStr, path::PathBuf};

use clap::{
    Arg, ArgAction, Args, CommandFactory, Parser, Subcommand, ValueEnum,
    error::{ContextKind, ContextValue},
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    error: String,
    usage: Option<String>,
    suggestion: Option<String>,
    matched_path: Vec<String>,
    unexpected_token: Option<String>,
    expected: Vec<String>,
    json: bool,
}

impl ParseDiagnostic {
    pub fn from_error<I, S>(args: I, error: &clap::Error) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args = args
            .into_iter()
            .map(|arg| arg.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let root = Cli::command();
        let tokens = args.get(1..).unwrap_or(&[]);
        let matched_path = matched_path(&root, tokens);
        let command = command_for_path(&root, &matched_path);
        let usage = usage_from_error(error).or_else(|| command.map(command_usage));
        let unexpected_token =
            invalid_value_from_message(error).or_else(|| unexpected_token(error));
        let suggestion = suggestion_for(error, command, &matched_path, usage.as_deref());
        let expected = expected_terms(&root, command.unwrap_or(&root));

        Self {
            error: clean_error_message(error),
            usage,
            suggestion,
            matched_path,
            unexpected_token,
            expected,
            json: wants_json_diagnostic(&args),
        }
    }

    pub fn render_stderr(&self) -> String {
        if self.json {
            return serde_json::json!({
                "error": self.error,
                "matched_path": self.matched_path,
                "unexpected_token": self.unexpected_token,
                "expected": self.expected,
                "suggestion": self.suggestion,
                "usage": self.usage,
            })
            .to_string();
        }

        let mut output = self.error.clone();
        if let Some(suggestion) = &self.suggestion {
            output.push_str("\nTry: ");
            output.push_str(suggestion);
        }
        if let Some(usage) = &self.usage {
            output.push_str("\nUsage: ");
            output.push_str(usage);
        }
        output
    }
}

fn wants_json_diagnostic(args: &[String]) -> bool {
    args.iter().enumerate().any(|(index, arg)| {
        arg == "--json"
            || arg == "--format=json"
            || (arg == "--format" && args.get(index + 1).is_some_and(|value| value == "json"))
    })
}

fn matched_path(root: &clap::Command, tokens: &[String]) -> Vec<String> {
    let mut path = Vec::new();
    let mut command = root;
    let mut index = 0;

    while index < tokens.len() {
        let token = tokens[index].as_str();
        if token == "--" {
            break;
        }
        if token.starts_with('-') {
            index += option_width(
                root,
                command,
                token,
                tokens.get(index + 1).map(String::as_str),
            );
            continue;
        }
        if let Some(subcommand) = find_subcommand(command, token) {
            path.push(subcommand.get_name().to_string());
            command = subcommand;
            index += 1;
            continue;
        }
        if command.get_subcommands().next().is_some() {
            break;
        }
        index += 1;
    }

    path
}

fn option_width(
    root: &clap::Command,
    command: &clap::Command,
    option: &str,
    next: Option<&str>,
) -> usize {
    if next.is_none() || option == "--" {
        return 1;
    }
    if let Some(name) = option.strip_prefix("--") {
        if name.contains('=') {
            return 1;
        }
        return if long_option_takes_value(command, name) || long_option_takes_value(root, name) {
            2
        } else {
            1
        };
    }
    if let Some(shorts) = option.strip_prefix('-') {
        let mut chars = shorts.chars();
        if let Some(short) = chars.next() {
            return if chars.next().is_none()
                && (short_option_takes_value(command, short)
                    || short_option_takes_value(root, short))
            {
                2
            } else {
                1
            };
        }
    }
    1
}

fn long_option_takes_value(command: &clap::Command, name: &str) -> bool {
    command
        .get_arguments()
        .any(|arg| arg.get_long() == Some(name) && arg_takes_value(arg))
}

fn short_option_takes_value(command: &clap::Command, short: char) -> bool {
    command
        .get_arguments()
        .any(|arg| arg.get_short() == Some(short) && arg_takes_value(arg))
}

fn arg_takes_value(arg: &Arg) -> bool {
    arg.get_action().takes_values()
}

fn find_subcommand<'a>(command: &'a clap::Command, token: &str) -> Option<&'a clap::Command> {
    command.get_subcommands().find(|subcommand| {
        subcommand.get_name() == token || subcommand.get_all_aliases().any(|alias| alias == token)
    })
}

fn command_for_path<'a>(root: &'a clap::Command, path: &[String]) -> Option<&'a clap::Command> {
    let mut command = root;
    for segment in path {
        command = find_subcommand(command, segment)?;
    }
    Some(command)
}

fn usage_from_error(error: &clap::Error) -> Option<String> {
    error
        .get(ContextKind::Usage)
        .map(ToString::to_string)
        .map(|usage| strip_usage_prefix(&usage))
}

fn command_usage(command: &clap::Command) -> String {
    let mut command = command.clone();
    strip_usage_prefix(&command.render_usage().to_string())
}

fn strip_usage_prefix(usage: &str) -> String {
    usage
        .trim()
        .strip_prefix("Usage: ")
        .unwrap_or_else(|| usage.trim())
        .to_string()
}

fn unexpected_token(error: &clap::Error) -> Option<String> {
    for kind in [
        ContextKind::InvalidSubcommand,
        ContextKind::InvalidArg,
        ContextKind::InvalidValue,
    ] {
        if let Some(ContextValue::String(value)) = error.get(kind) {
            return Some(value.clone());
        }
    }
    None
}

fn invalid_value_from_message(error: &clap::Error) -> Option<String> {
    let message = clean_error_message(error);
    let rest = message.strip_prefix("invalid value '")?;
    let (value, _) = rest.split_once("'")?;
    Some(value.to_string())
}

fn suggestion_for(
    error: &clap::Error,
    command: Option<&clap::Command>,
    matched_path: &[String],
    usage: Option<&str>,
) -> Option<String> {
    if let Some(value) = command
        .and_then(|command| nearest_subcommand(command, error, matched_path))
        .or_else(|| first_context_string(error, ContextKind::SuggestedSubcommand))
    {
        return Some(command_try(
            matched_path
                .iter()
                .map(String::as_str)
                .chain([value.as_str()]),
        ));
    }
    if let Some(value) = first_context_string(error, ContextKind::SuggestedArg) {
        return Some(command_try(
            matched_path
                .iter()
                .map(String::as_str)
                .chain([value.as_str()]),
        ));
    }
    if let Some(value) = first_context_string(error, ContextKind::SuggestedValue) {
        return Some(value);
    }
    usage.map(str::to_string)
}

fn first_context_string(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind)? {
        ContextValue::String(value) => Some(value.clone()),
        ContextValue::Strings(values) => values.first().cloned(),
        ContextValue::StyledStr(value) => Some(value.to_string()),
        ContextValue::StyledStrs(values) => values.first().map(ToString::to_string),
        _ => None,
    }
}

fn nearest_subcommand(
    command: &clap::Command,
    error: &clap::Error,
    _matched_path: &[String],
) -> Option<String> {
    let token = match error.get(ContextKind::InvalidSubcommand)? {
        ContextValue::String(value) => value,
        _ => return None,
    };
    command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name())
        .min_by_key(|name| {
            (
                edit_distance(name, token),
                name.chars().count().abs_diff(token.chars().count()),
            )
        })
        .filter(|name| edit_distance(name, token) <= 2)
        .map(str::to_string)
}

fn command_try<'a>(segments: impl IntoIterator<Item = &'a str>) -> String {
    std::iter::once("gd")
        .chain(segments)
        .collect::<Vec<_>>()
        .join(" ")
}

fn expected_terms(root: &clap::Command, command: &clap::Command) -> Vec<String> {
    if command.get_subcommands().next().is_some() {
        return command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name().to_string())
            .collect();
    }

    let mut terms = BTreeSet::new();
    insert_expected_args(command, &mut terms);
    if command.get_name() != root.get_name() {
        insert_expected_args(root, &mut terms);
    }
    terms.into_iter().collect()
}

fn insert_expected_args(command: &clap::Command, terms: &mut BTreeSet<String>) {
    for arg in command.get_positionals() {
        if let Some(name) = value_name(arg) {
            terms.insert(format!("<{name}>"));
        }
    }
    for arg in command.get_arguments().filter(|arg| !arg.is_positional()) {
        if let Some(long) = arg.get_long().filter(|long| !is_builtin_arg(long)) {
            terms.insert(format!("--{long}"));
        } else if let Some(short) = arg.get_short() {
            terms.insert(format!("-{short}"));
        }
    }
}

fn value_name(arg: &Arg) -> Option<String> {
    arg.get_value_names()
        .and_then(|names| names.first())
        .map(ToString::to_string)
        .or_else(|| Some(arg.get_id().to_string()))
}

fn is_builtin_arg(name: &str) -> bool {
    matches!(name, "help" | "version")
}

fn clean_error_message(error: &clap::Error) -> String {
    let message = error.to_string();
    let line = message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("failed to parse command");
    line.strip_prefix("error: ").unwrap_or(line).to_string()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right_len = right.chars().count();
    let mut previous = (0..=right_len).collect::<Vec<_>>();
    let mut current = vec![0; right_len + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right.chars().enumerate() {
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            let substitution = previous[right_index] + usize::from(left_char != right_char);
            current[right_index + 1] = insertion.min(deletion).min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_len]
}

fn parse_gitcode_repository_arg(value: &str) -> Result<String, String> {
    crate::repo::split_repo(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

fn parse_github_repository_arg(value: &str) -> Result<String, String> {
    crate::repo::parse_github_repo(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

fn parse_api_base_arg(value: &str) -> Result<String, String> {
    crate::config::parse_api_base(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

fn parse_openlibing_base_arg(value: &str) -> Result<String, String> {
    crate::pipeline::openlibing_base_from_value(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

fn parse_api_field_arg(value: &str) -> Result<String, String> {
    crate::client::split_field(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

fn parse_workflow_path_arg(value: &str) -> Result<String, String> {
    crate::pipeline::validate_workflow_path(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

fn parse_workflow_input_arg(value: &str) -> Result<String, String> {
    let Some((key, _)) = value.split_once('=') else {
        return Err(format!("workflow input must be in key=value form: {value}"));
    };
    if key.trim().is_empty() {
        return Err("workflow input key cannot be empty".to_string());
    }
    Ok(value.to_string())
}

fn parse_codecheck_secret_arg(value: &str) -> Result<String, String> {
    crate::pipeline::validate_codecheck_secret_name(value)
        .map(|_| value.to_string())
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliOutputFormat {
    Text,
    Json,
}

#[derive(Debug, Args, Clone)]
pub struct GlobalArgs {
    #[arg(long, global = true, default_value = "gitcode.com")]
    pub hostname: String,
    #[arg(
        long,
        global = true,
        value_name = "URL",
        value_parser = parse_api_base_arg,
        help = "GitCode API base URL"
    )]
    pub api_base: Option<String>,
    #[arg(
        long,
        global = true,
        env = "GD_OPENLIBING_BASE",
        default_value = "https://www.openlibing.com/gateway",
        value_parser = parse_openlibing_base_arg,
        help = "OpenLibing gateway base URL for pipeline gate checks"
    )]
    pub openlibing_base: String,
    #[arg(long, global = true, help = "Render command output as JSON")]
    pub json: bool,
    #[arg(
        long,
        global = true,
        value_enum,
        value_name = "FORMAT",
        help = "Render command output as text or JSON"
    )]
    pub format: Option<CliOutputFormat>,
}

impl GlobalArgs {
    pub fn json_output(&self) -> bool {
        self.json || self.format == Some(CliOutputFormat::Json)
    }
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
    #[command(about = "Inspect gd version and updates")]
    #[command(subcommand)]
    Version(VersionCommand),
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

#[derive(Debug, Subcommand)]
pub enum VersionCommand {
    #[command(about = "Check whether a newer gd release is available")]
    Check(VersionCheckArgs),
}

#[derive(Debug, Args)]
pub struct VersionCheckArgs {}

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
    #[arg(short = 'f', long = "raw-field", value_parser = parse_api_field_arg)]
    pub raw_fields: Vec<String>,
    #[arg(short = 'F', long = "field", value_parser = parse_api_field_arg)]
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
    #[command(about = "Move or rename a repository")]
    Move(RepoMoveArgs),
    #[command(about = "Sync a GitHub repository into GitCode")]
    SyncGithub(RepoSyncGithubArgs),
}

#[derive(Debug, Args)]
pub struct RepoViewArgs {
    #[arg(value_parser = parse_gitcode_repository_arg)]
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
    #[arg(value_parser = parse_gitcode_repository_arg)]
    pub repository: String,
    pub directory: Option<PathBuf>,
    #[arg(last = true)]
    pub git_flags: Vec<String>,
}

#[derive(Debug, Args)]
pub struct RepoRefArgs {
    #[arg(value_parser = parse_gitcode_repository_arg)]
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

#[derive(Debug, Args)]
pub struct RepoMoveArgs {
    #[arg(value_parser = parse_gitcode_repository_arg)]
    pub source: String,
    pub target: String,
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RepoSyncIfExists {
    Fail,
    Skip,
    Update,
    Recreate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RepoSyncMethod {
    Import,
    GitPush,
}

#[derive(Debug, Args)]
pub struct RepoSyncGithubArgs {
    #[arg(value_parser = parse_github_repository_arg)]
    pub github_repo: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(long)]
    pub org: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub private: bool,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long, value_enum, default_value_t = RepoSyncIfExists::Skip)]
    pub if_exists: RepoSyncIfExists,
    #[arg(long, value_enum, default_value_t = RepoSyncMethod::Import)]
    pub method: RepoSyncMethod,
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
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 's', long, default_value = "open")]
    pub state: String,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct IssueViewArgs {
    pub number: u64,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueCreateArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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
    #[command(about = "List pull request review comments")]
    Comments(PrCommentsArgs),
    #[command(about = "Add a pull request review comment")]
    Comment(PrCommentArgs),
    #[command(about = "Reply to a pull request review discussion")]
    Reply(PrReplyArgs),
}

#[derive(Debug, Args)]
pub struct PrListArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
}

#[derive(Debug, Args)]
pub struct PrCreateArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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

#[derive(Debug, Args)]
pub struct PrCommentsArgs {
    pub number: u64,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: u64,
    #[arg(short = 'L', long = "limit", default_value_t = 30)]
    pub limit: u64,
}

#[derive(Debug, Args)]
pub struct PrCommentArgs {
    pub number: u64,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 'b', long)]
    pub body: String,
    #[arg(long)]
    pub path: Option<String>,
    #[arg(long)]
    pub position: Option<u64>,
    #[arg(long)]
    pub need_to_resolve: bool,
}

#[derive(Debug, Args)]
pub struct PrReplyArgs {
    pub number: u64,
    pub discussion_id: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 'b', long)]
    pub body: String,
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
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 'L', long, default_value_t = 100)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct LabelCreateArgs {
    pub name: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 'c', long)]
    pub color: Option<String>,
    #[arg(short = 'd', long)]
    pub description: Option<String>,
}

#[derive(Debug, Args)]
pub struct LabelEditArgs {
    pub name: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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
    #[command(about = "Migrate GitHub Releases and assets into GitCode")]
    MigrateGithub(ReleaseMigrateGithubArgs),
}

#[derive(Debug, Args)]
pub struct ReleaseListArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub limit: u32,
}

#[derive(Debug, Args)]
pub struct ReleaseViewArgs {
    pub tag: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
}

#[derive(Debug, Args)]
pub struct ReleaseCreateArgs {
    pub tag: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 't', long)]
    pub title: Option<String>,
    #[arg(short = 'n', long)]
    pub notes: Option<String>,
    #[arg(long)]
    pub target: Option<String>,
}

#[derive(Debug, Args)]
pub struct ReleaseMigrateGithubArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(long, value_parser = parse_github_repository_arg)]
    pub github_repo: String,
    #[arg(long, conflicts_with = "all")]
    pub tag: Option<String>,
    #[arg(long)]
    pub all: bool,
    #[arg(
        long,
        action = ArgAction::Set,
        default_value_t = true,
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true
    )]
    pub skip_existing_assets: bool,
    #[arg(
        long,
        action = ArgAction::Set,
        default_value_t = true,
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = true
    )]
    pub update_release: bool,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum PipelineCommand {
    #[command(
        about = "Create or update a GitCode workflow file",
        visible_alias = "register"
    )]
    Set(PipelineSetArgs),
    #[command(about = "Create or update a GitCode CodeCheck workflow file")]
    Codecheck(PipelineCodecheckArgs),
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
    #[command(about = "Authenticate OpenLibing GitCode OAuth for pipeline checks")]
    #[command(subcommand)]
    Auth(PipelineAuthCommand),
    #[command(about = "Show OpenLibing pipeline and CodeCheck configuration")]
    Config(PipelineConfigArgs),
    #[command(about = "Configure an OpenLibing GitCode repository for PR gate checks")]
    Setup(Box<PipelineSetupArgs>),
    #[command(about = "List GitCode pull requests known to OpenLibing")]
    Prs(PipelinePrsArgs),
    #[command(about = "Show OpenLibing gate checks for a pull request")]
    Checks(PipelinePrArgs),
    #[command(
        name = "gate-view",
        about = "View a GitCode pull request with OpenLibing checks"
    )]
    GateView(PipelinePrArgs),
    #[command(name = "gate-runs", about = "List OpenLibing pipeline run summaries")]
    GateRuns(PipelineGateRunsArgs),
}

#[derive(Debug, Args)]
pub struct PipelineSetArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(value_parser = parse_workflow_path_arg)]
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

#[derive(Debug, Args)]
pub struct PipelineCodecheckArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(
        long,
        default_value = ".gitcode/workflows/codecheck.yml",
        value_parser = parse_workflow_path_arg
    )]
    pub path: String,
    #[arg(long, value_enum, default_value_t = PipelineSetMode::Create)]
    pub mode: PipelineSetMode,
    #[arg(
        short = 'm',
        long,
        default_value = "Configure GitCode CodeCheck workflow"
    )]
    pub message: String,
    #[arg(long = "commit-branch")]
    pub commit_branch: Option<String>,
    #[arg(long)]
    pub sha: Option<String>,
    #[arg(long, default_value = "codecheck-pipeline")]
    pub name: String,
    #[arg(long = "check-branch")]
    pub check_branch: Option<String>,
    #[arg(long = "repo-url")]
    pub repo_url: Option<String>,
    #[arg(long = "language")]
    pub languages: Vec<String>,
    #[arg(
        long = "access-token-secret",
        default_value = "CODECHECK_ACCESS_TOKEN",
        value_parser = parse_codecheck_secret_arg
    )]
    pub access_token_secret: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PipelineSetMode {
    Create,
    Update,
}

#[derive(Debug, Args)]
pub struct PipelineListArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: u64,
    #[arg(short = 'L', long = "limit", default_value_t = 50)]
    pub limit: u64,
}

#[derive(Debug, Args)]
pub struct PipelineRunArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    pub workflow_id: String,
    #[arg(long, value_parser = parse_workflow_path_arg)]
    pub file_path: String,
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long)]
    pub branch_commit_id: Option<String>,
    #[arg(long = "input", value_parser = parse_workflow_input_arg)]
    pub inputs: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PipelineRunsArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    pub workflow_run_id: String,
}

#[derive(Debug, Args)]
pub struct PipelineLogArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
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
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    pub workflow_run_id: String,
}

#[derive(Debug, Args)]
pub struct PipelineRetryArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    pub workflow_run_id: String,
    #[arg(long = "job-run-id")]
    pub job_run_ids: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PipelineRerunArgs {
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    pub workflow_run_id: String,
}

#[derive(Debug, Subcommand)]
pub enum PipelineAuthCommand {
    #[command(about = "Open a browser and complete OpenLibing GitCode OAuth")]
    Login(PipelineAuthLoginArgs),
    #[command(about = "View OpenLibing GitCode OAuth status")]
    Status,
    #[command(about = "Remove stored OpenLibing pipeline credentials")]
    Logout,
}

#[derive(Debug, Args)]
pub struct PipelineAuthLoginArgs {
    #[arg(long, default_value = "127.0.0.1")]
    pub callback_host: String,
    #[arg(long, default_value_t = 0)]
    pub callback_port: u16,
    #[arg(long, default_value_t = 180)]
    pub timeout_seconds: u64,
    #[arg(long, help = "Print the OAuth URL without opening a browser")]
    pub no_browser: bool,
}

#[derive(Debug, Args)]
pub struct PipelineConfigArgs {
    #[arg(long)]
    pub project_id: String,
}

#[derive(Debug, Args)]
pub struct PipelineSetupArgs {
    #[arg(long)]
    pub project_id: String,
    #[arg(
        short = 'R',
        long = "repo",
        required_unless_present = "repo_url",
        value_parser = parse_gitcode_repository_arg
    )]
    pub repository: Option<String>,
    #[arg(long, required_unless_present = "repository")]
    pub repo_url: Option<String>,
    #[arg(long)]
    pub repo_id: Option<u64>,
    #[arg(long)]
    pub repo_name: Option<String>,
    #[arg(long)]
    pub repo_owner: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub language: Vec<String>,
    #[arg(long)]
    pub codecheck_rule_set: Option<String>,
    #[arg(long)]
    pub anti_rule_set: Option<String>,
    #[arg(long, default_value = "自研源码")]
    pub purpose: String,
    #[arg(long, default_value = "lead")]
    pub open_source: String,
    #[arg(long, default_value = "1")]
    pub assume_pr: String,
    #[arg(long, default_value = "1")]
    pub auto_trigger: String,
    #[arg(long, default_value = "0")]
    pub auto_trigger_design_scan: String,
    #[arg(long, default_value_t = 1)]
    pub disallow_self_merge: u8,
    #[arg(long, default_value_t = 0)]
    pub disallow_unresolved_discussions_merge: u8,
    #[arg(long)]
    pub public_token_env: Option<String>,
    #[arg(
        long,
        help = "Skip OpenLibing webhook reconfiguration after repository setup"
    )]
    pub no_configure_webhook: bool,
}

#[derive(Debug, Args)]
pub struct PipelinePrsArgs {
    #[arg(long)]
    pub project_id: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(short = 's', long)]
    pub state: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: u64,
    #[arg(short = 'L', long = "limit", default_value_t = 20)]
    pub limit: u64,
}

#[derive(Debug, Args)]
pub struct PipelinePrArgs {
    #[arg(long)]
    pub project_id: String,
    #[arg(short = 'R', long = "repo", value_parser = parse_gitcode_repository_arg)]
    pub repository: Option<String>,
    #[arg(long = "pr", alias = "mr")]
    pub number: u64,
}

#[derive(Debug, Args)]
pub struct PipelineGateRunsArgs {
    #[arg(long)]
    pub project_id: String,
    #[arg(long)]
    pub pipeline_name: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: u64,
    #[arg(short = 'L', long = "limit", default_value_t = 20)]
    pub limit: u64,
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

    fn diagnostic<const N: usize>(args: [&str; N]) -> ParseDiagnostic {
        let error = Cli::try_parse_from(args).unwrap_err();
        ParseDiagnostic::from_error(args, &error)
    }

    #[test]
    fn parses_repo_view_with_json_flag() {
        let cli = Cli::try_parse_from(["gd", "--json", "repo", "view", "owner/repo"]).unwrap();
        assert!(cli.global.json);
        assert!(cli.global.json_output());
        match cli.command {
            Command::Repo(RepoCommand::View(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_repo_sync_github() {
        let cli = Cli::try_parse_from([
            "gd",
            "repo",
            "sync-github",
            "git@github.com:source/repo.git",
            "--org",
            "target-org",
            "--name",
            "target-repo",
            "--private",
            "--if-exists",
            "skip",
        ])
        .unwrap();

        match cli.command {
            Command::Repo(RepoCommand::SyncGithub(args)) => {
                assert_eq!(args.github_repo, "git@github.com:source/repo.git");
                assert_eq!(args.org.as_deref(), Some("target-org"));
                assert_eq!(args.name.as_deref(), Some("target-repo"));
                assert!(args.private);
                assert_eq!(args.if_exists, RepoSyncIfExists::Skip);
                assert_eq!(args.method, RepoSyncMethod::Import);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_repo_sync_github_git_push_recreate() {
        let cli = Cli::try_parse_from([
            "gd",
            "repo",
            "sync-github",
            "source/repo",
            "--repo",
            "target-org/target-repo",
            "--method",
            "git-push",
            "--if-exists",
            "recreate",
        ])
        .unwrap();

        match cli.command {
            Command::Repo(RepoCommand::SyncGithub(args)) => {
                assert_eq!(args.repository.as_deref(), Some("target-org/target-repo"));
                assert_eq!(args.if_exists, RepoSyncIfExists::Recreate);
                assert_eq!(args.method, RepoSyncMethod::GitPush);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_repo_move_target_path() {
        let cli = Cli::try_parse_from([
            "gd",
            "repo",
            "move",
            "source-owner/source-repo",
            "target-owner/target-repo",
        ])
        .unwrap();

        match cli.command {
            Command::Repo(RepoCommand::Move(args)) => {
                assert_eq!(args.source, "source-owner/source-repo");
                assert_eq!(args.target, "target-owner/target-repo");
                assert_eq!(args.name, None);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_repo_move_with_name_flag() {
        let cli = Cli::try_parse_from([
            "gd",
            "repo",
            "move",
            "source-owner/source-repo",
            "target-owner",
            "--name",
            "target-repo",
        ])
        .unwrap();

        match cli.command {
            Command::Repo(RepoCommand::Move(args)) => {
                assert_eq!(args.source, "source-owner/source-repo");
                assert_eq!(args.target, "target-owner");
                assert_eq!(args.name.as_deref(), Some("target-repo"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_format_json_as_json_output() {
        let cli = Cli::try_parse_from(["gd", "--format", "json", "repo", "list", "owner"]).unwrap();
        assert_eq!(cli.global.format, Some(CliOutputFormat::Json));
        assert!(cli.global.json_output());
    }

    #[test]
    fn parses_version_check_without_auth_context() {
        let cli = Cli::try_parse_from(["gd", "version", "check", "--json"]).unwrap();
        assert!(cli.global.json_output());
        match cli.command {
            Command::Version(VersionCommand::Check(_)) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_diagnostic_suggests_nearest_subcommand() {
        let args = ["gd", "pipeline", "rnus"];
        let error = Cli::try_parse_from(args).unwrap_err();
        let diagnostic = ParseDiagnostic::from_error(args, &error);

        assert_eq!(diagnostic.matched_path, ["pipeline"]);
        assert_eq!(diagnostic.unexpected_token.as_deref(), Some("rnus"));
        assert_eq!(diagnostic.suggestion.as_deref(), Some("gd pipeline runs"));
        assert!(diagnostic.expected.iter().any(|term| term == "runs"));

        let stderr = diagnostic.render_stderr();
        assert!(stderr.contains("Try: gd pipeline runs"));
        assert!(stderr.contains("Usage: gd pipeline"));
    }

    #[test]
    fn parse_diagnostic_reports_deepest_option_context() {
        let args = ["gd", "repo", "list", "owner", "--limt"];
        let error = Cli::try_parse_from(args).unwrap_err();
        let diagnostic = ParseDiagnostic::from_error(args, &error);

        assert_eq!(diagnostic.matched_path, ["repo", "list"]);
        assert_eq!(diagnostic.unexpected_token.as_deref(), Some("--limt"));
        assert!(diagnostic.expected.iter().any(|term| term == "--limit"));
        assert!(
            diagnostic
                .usage
                .as_deref()
                .is_some_and(|usage| { usage.starts_with("gd repo list") })
        );
    }

    #[test]
    fn parse_diagnostic_renders_json_for_json_flag() {
        let args = ["gd", "--json", "repo", "list", "owner", "--limt"];
        let error = Cli::try_parse_from(args).unwrap_err();
        let diagnostic = ParseDiagnostic::from_error(args, &error);
        let stderr = diagnostic.render_stderr();

        assert!(!stderr.contains('\n'));
        let value: serde_json::Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(value["matched_path"], serde_json::json!(["repo", "list"]));
        assert_eq!(value["unexpected_token"], "--limt");
        assert!(
            value["expected"]
                .as_array()
                .unwrap()
                .iter()
                .any(|term| term == "--limit")
        );
    }

    #[test]
    fn parse_diagnostic_renders_json_for_format_json() {
        let args = ["gd", "--format", "json", "repo", "list", "owner", "--limt"];
        let error = Cli::try_parse_from(args).unwrap_err();
        let diagnostic = ParseDiagnostic::from_error(args, &error);
        let stderr = diagnostic.render_stderr();

        assert!(!stderr.contains('\n'));
        let value: serde_json::Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(value["matched_path"], serde_json::json!(["repo", "list"]));
        assert_eq!(value["unexpected_token"], "--limt");
    }

    #[test]
    fn parse_diagnostic_reports_invalid_repository_value() {
        let diagnostic = diagnostic(["gd", "repo", "view", "owner/repo/extra"]);

        assert_eq!(diagnostic.matched_path, ["repo", "view"]);
        assert_eq!(
            diagnostic.unexpected_token.as_deref(),
            Some("owner/repo/extra")
        );
        assert!(
            diagnostic
                .error
                .contains("repository must be in owner/repo form")
        );
    }

    #[test]
    fn parse_diagnostic_reports_invalid_github_repository_value() {
        let diagnostic = diagnostic(["gd", "repo", "sync-github", "https://gitlab.com/o/r"]);

        assert_eq!(diagnostic.matched_path, ["repo", "sync-github"]);
        assert_eq!(
            diagnostic.unexpected_token.as_deref(),
            Some("https://gitlab.com/o/r")
        );
        assert!(
            diagnostic
                .error
                .contains("repository must be in owner/repo form")
        );
    }

    #[test]
    fn parse_diagnostic_reports_invalid_workflow_paths() {
        for diagnostic in [
            diagnostic(["gd", "pipeline", "set", "ci.yml"]),
            diagnostic(["gd", "pipeline", "codecheck", "--path", "ci.yml"]),
            diagnostic(["gd", "pipeline", "run", "workflow", "--file-path", "ci.yml"]),
        ] {
            assert_eq!(diagnostic.unexpected_token.as_deref(), Some("ci.yml"));
            assert!(
                diagnostic
                    .error
                    .contains("workflow path must be under .gitcode/workflows/")
            );
        }
    }

    #[test]
    fn parse_diagnostic_reports_invalid_workflow_input() {
        let diagnostic = diagnostic([
            "gd",
            "pipeline",
            "run",
            "workflow",
            "--file-path",
            ".gitcode/workflows/ci.yml",
            "--input",
            "missing_equals",
        ]);

        assert_eq!(diagnostic.matched_path, ["pipeline", "run"]);
        assert_eq!(
            diagnostic.unexpected_token.as_deref(),
            Some("missing_equals")
        );
        assert!(
            diagnostic
                .error
                .contains("workflow input must be in key=value form")
        );
    }

    #[test]
    fn parse_diagnostic_reports_invalid_codecheck_secret() {
        let diagnostic = diagnostic([
            "gd",
            "pipeline",
            "codecheck",
            "--access-token-secret",
            "bad-name",
        ]);

        assert_eq!(diagnostic.matched_path, ["pipeline", "codecheck"]);
        assert_eq!(diagnostic.unexpected_token.as_deref(), Some("bad-name"));
        assert!(
            diagnostic
                .error
                .contains("must contain only letters, digits, and '_'")
        );
    }

    #[test]
    fn parse_diagnostic_reports_invalid_api_base() {
        let diagnostic = diagnostic(["gd", "--api-base", "file:///tmp", "api", "/user"]);

        assert_eq!(diagnostic.matched_path, ["api"]);
        assert_eq!(diagnostic.unexpected_token.as_deref(), Some("file:///tmp"));
        assert!(diagnostic.error.contains("must use http or https"));
    }

    #[test]
    fn parse_diagnostic_reports_invalid_api_field() {
        let diagnostic = diagnostic(["gd", "api", "/user", "-F", "missing_equals"]);

        assert_eq!(diagnostic.matched_path, ["api"]);
        assert_eq!(
            diagnostic.unexpected_token.as_deref(),
            Some("missing_equals")
        );
        assert!(diagnostic.error.contains("field must be in key=value form"));
    }

    #[test]
    fn parse_diagnostic_renders_json_for_invalid_value() {
        let diagnostic = diagnostic(["gd", "--json", "repo", "view", "owner/repo/extra"]);
        let stderr = diagnostic.render_stderr();

        assert!(!stderr.contains('\n'));
        let value: serde_json::Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(value["matched_path"], serde_json::json!(["repo", "view"]));
        assert_eq!(value["unexpected_token"], "owner/repo/extra");
        assert!(
            value["expected"]
                .as_array()
                .unwrap()
                .iter()
                .any(|term| term == "<REPOSITORY>")
        );
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
    fn parses_pr_comments_comment_and_reply() {
        let comments = Cli::try_parse_from([
            "gd",
            "pr",
            "comments",
            "7",
            "--repo",
            "owner/repo",
            "--page",
            "2",
            "--limit",
            "40",
        ])
        .unwrap();
        match comments.command {
            Command::Pr(PrCommand::Comments(args)) => {
                assert_eq!(args.number, 7);
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.page, 2);
                assert_eq!(args.limit, 40);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let comment = Cli::try_parse_from([
            "gd",
            "pr",
            "comment",
            "7",
            "--repo",
            "owner/repo",
            "--body",
            "please fix",
            "--path",
            "src/main.rs",
            "--position",
            "3",
            "--need-to-resolve",
        ])
        .unwrap();
        match comment.command {
            Command::Pr(PrCommand::Comment(args)) => {
                assert_eq!(args.number, 7);
                assert_eq!(args.body, "please fix");
                assert_eq!(args.path.as_deref(), Some("src/main.rs"));
                assert_eq!(args.position, Some(3));
                assert!(args.need_to_resolve);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let reply = Cli::try_parse_from([
            "gd",
            "pr",
            "reply",
            "7",
            "discussion-1",
            "--repo",
            "owner/repo",
            "--body",
            "done",
        ])
        .unwrap();
        match reply.command {
            Command::Pr(PrCommand::Reply(args)) => {
                assert_eq!(args.number, 7);
                assert_eq!(args.discussion_id, "discussion-1");
                assert_eq!(args.body, "done");
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
    fn parses_release_migrate_github() {
        let cli = Cli::try_parse_from([
            "gd",
            "release",
            "migrate-github",
            "--repo",
            "owner/repo",
            "--github-repo",
            "source/repo",
            "--tag",
            "v1.0.0",
            "--skip-existing-assets",
            "--update-release",
            "--dry-run",
        ])
        .unwrap();

        match cli.command {
            Command::Release(ReleaseCommand::MigrateGithub(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.github_repo, "source/repo");
                assert_eq!(args.tag.as_deref(), Some("v1.0.0"));
                assert!(!args.all);
                assert!(args.skip_existing_assets);
                assert!(args.update_release);
                assert!(args.dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_release_migrate_github_disabled_toggles() {
        let cli = Cli::try_parse_from([
            "gd",
            "release",
            "migrate-github",
            "--repo",
            "owner/repo",
            "--github-repo",
            "source/repo",
            "--tag",
            "v1.0.0",
            "--skip-existing-assets=false",
            "--update-release=false",
        ])
        .unwrap();

        match cli.command {
            Command::Release(ReleaseCommand::MigrateGithub(args)) => {
                assert!(!args.skip_existing_assets);
                assert!(!args.update_release);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_auth_login() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "auth",
            "login",
            "--callback-port",
            "19090",
            "--no-browser",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Auth(PipelineAuthCommand::Login(args))) => {
                assert_eq!(args.callback_port, 19090);
                assert!(args.no_browser);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_config() {
        let cli =
            Cli::try_parse_from(["gd", "pipeline", "config", "--project-id", "project-1"]).unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Config(args)) => {
                assert_eq!(args.project_id, "project-1");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_setup() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "setup",
            "--project-id",
            "project-1",
            "--repo",
            "owner/repo",
            "--language",
            "Rust,Shell",
            "--codecheck-rule-set",
            "default",
            "--public-token-env",
            "GITCODE_TOKEN",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Setup(args)) => {
                assert_eq!(args.project_id, "project-1");
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.language, ["Rust", "Shell"]);
                assert_eq!(args.codecheck_rule_set.as_deref(), Some("default"));
                assert_eq!(args.public_token_env.as_deref(), Some("GITCODE_TOKEN"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_prs_and_checks() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "prs",
            "--project-id",
            "project-1",
            "--repo",
            "owner/repo",
            "--state",
            "open",
            "-L",
            "10",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Prs(args)) => {
                assert_eq!(args.project_id, "project-1");
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.state.as_deref(), Some("open"));
                assert_eq!(args.limit, 10);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let checks = Cli::try_parse_from([
            "gd",
            "pipeline",
            "checks",
            "--project-id",
            "project-1",
            "--repo",
            "owner/repo",
            "--pr",
            "7",
        ])
        .unwrap();

        match checks.command {
            Command::Pipeline(PipelineCommand::Checks(args)) => {
                assert_eq!(args.project_id, "project-1");
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.number, 7);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_gate_runs() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "gate-runs",
            "--project-id",
            "project-1",
            "--pipeline-name",
            "ci",
            "--page",
            "2",
            "-L",
            "20",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::GateRuns(args)) => {
                assert_eq!(args.project_id, "project-1");
                assert_eq!(args.pipeline_name.as_deref(), Some("ci"));
                assert_eq!(args.page, 2);
                assert_eq!(args.limit, 20);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_pipeline_gate_view() {
        let view = Cli::try_parse_from([
            "gd",
            "pipeline",
            "gate-view",
            "--project-id",
            "project-1",
            "--repo",
            "owner/repo",
            "--mr",
            "12",
        ])
        .unwrap();
        match view.command {
            Command::Pipeline(PipelineCommand::GateView(args)) => {
                assert_eq!(args.project_id, "project-1");
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.number, 12);
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
    fn parses_pipeline_codecheck() {
        let cli = Cli::try_parse_from([
            "gd",
            "pipeline",
            "codecheck",
            "--repo",
            "owner/repo",
            "--language",
            "SHELL",
            "--language",
            "RUST",
            "--check-branch",
            "main",
            "--access-token-secret",
            "CODECHECK_TOKEN",
        ])
        .unwrap();

        match cli.command {
            Command::Pipeline(PipelineCommand::Codecheck(args)) => {
                assert_eq!(args.repository.as_deref(), Some("owner/repo"));
                assert_eq!(args.path, ".gitcode/workflows/codecheck.yml");
                assert_eq!(args.languages, ["SHELL", "RUST"]);
                assert_eq!(args.check_branch.as_deref(), Some("main"));
                assert_eq!(args.access_token_secret, "CODECHECK_TOKEN");
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
