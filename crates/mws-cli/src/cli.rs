use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "mws", version, about = "Microsoft Workspace CLI — one CLI for Microsoft 365")]
pub struct Cli {
    /// Account name to use (default: "default").
    #[arg(long, global = true, env = "MWS_ACCOUNT")]
    pub account: Option<String>,
    /// Tenant id, domain, or one of common|organizations|consumers.
    #[arg(long, global = true, env = "MWS_TENANT")]
    pub tenant: Option<String>,
    /// Override the OAuth client id (default: Microsoft Graph CLI public client).
    #[arg(long, global = true, env = "MWS_CLIENT_ID")]
    pub client_id: Option<String>,
    /// Output format: json|table|yaml|tsv.
    #[arg(long, short = 'o', global = true, env = "MWS_OUTPUT")]
    pub output: Option<String>,
    /// Use Microsoft Graph beta endpoints.
    #[arg(long, global = true)]
    pub beta: bool,
    /// Verbose logging.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,
    /// Override the Graph base URL (hidden; for tests).
    #[arg(long, global = true, hide = true)]
    pub graph_base: Option<String>,
    /// Override the config directory (test helper; only compiled with --features test-helpers).
    #[cfg(feature = "test-helpers")]
    #[arg(long, global = true, hide = true)]
    pub config_dir: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Authentication.
    Auth(AuthArgs),
    /// Mail operations.
    Mail(MailArgs),
    /// Make a raw HTTP request to Microsoft Graph.
    Raw(RawArgs),
    /// Show the signed-in user.
    Whoami,
}

#[derive(Debug, clap::Args)]
pub struct MailArgs {
    #[command(subcommand)]
    pub action: MailAction,
}

#[derive(Debug, clap::Subcommand)]
pub enum MailAction {
    /// Send an email.
    Send(SendArgs),
}

#[derive(Debug, clap::Args)]
pub struct SendArgs {
    /// Recipient (repeatable). At least one required.
    #[arg(long, required = true)]
    pub to: Vec<String>,
    /// CC recipient (repeatable).
    #[arg(long)]
    pub cc: Vec<String>,
    /// BCC recipient (repeatable).
    #[arg(long)]
    pub bcc: Vec<String>,
    /// Subject line.
    #[arg(long)]
    pub subject: String,
    /// Body. Literal text, or `@file` to read from a file, or `-` for stdin.
    #[arg(long)]
    pub body: String,
    /// Treat body as HTML (default: detect or plain text).
    #[arg(long)]
    pub html: bool,
    /// Attachment path (repeatable).
    #[arg(long = "attachment")]
    pub attachments: Vec<std::path::PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct RawArgs {
    /// HTTP method.
    #[arg(value_parser = ["GET", "POST", "PATCH", "PUT", "DELETE"])]
    pub method: String,
    /// Path appended to the Graph base URL (e.g., `/me/messages`).
    pub path: String,
    /// Request body. Use `@file` to read from a file, `-` for stdin, or pass literal JSON.
    #[arg(long)]
    pub body: Option<String>,
    /// Custom header in `key:value` form. Repeatable.
    #[arg(long = "header", short = 'H')]
    pub headers: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub action: AuthAction,
}

#[derive(Debug, Subcommand)]
pub enum AuthAction {
    /// Sign in.
    Login(LoginArgs),
    /// Sign out (remove cached credentials).
    Logout(LogoutArgs),
    /// List cached accounts.
    List,
}

#[derive(Debug, clap::Args)]
pub struct LogoutArgs {
    /// Remove every cached account in the config dir.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, clap::Args)]
pub struct LoginArgs {
    /// Force device-code flow.
    #[arg(long)]
    pub device: bool,
    /// Force browser auth-code+PKCE flow.
    #[arg(long, conflicts_with = "device")]
    pub code: bool,
    /// Override the OAuth token endpoint (hidden; for tests).
    #[arg(long, hide = true)]
    pub token_endpoint: Option<String>,
    /// Override the OAuth device authorization endpoint (hidden; for tests).
    #[arg(long, hide = true)]
    pub device_endpoint: Option<String>,
    /// Override the authorize URL printed to the user (hidden; for tests).
    #[arg(long, hide = true)]
    pub authorize_url: Option<String>,
}
