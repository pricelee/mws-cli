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
    /// Override the config directory (hidden; for tests).
    #[arg(long, global = true, hide = true)]
    pub config_dir: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Authentication.
    Auth(AuthArgs),
    /// Show the signed-in user.
    Whoami,
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
