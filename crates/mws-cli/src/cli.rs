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
    /// Force device-code flow even on a graphical desktop.
    #[arg(long)]
    pub device: bool,
    /// Force browser auth-code+PKCE flow (this is the default on graphical desktops).
    #[arg(long, conflicts_with = "device")]
    pub code: bool,
}
