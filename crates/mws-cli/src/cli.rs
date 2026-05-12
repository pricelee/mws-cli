use clap::{Parser, Subcommand};

const ROOT_LONG_ABOUT: &str = "\
Microsoft Workspace CLI — one CLI for Microsoft 365, built on the
Microsoft Graph REST API. The Microsoft-side counterpart to
googleworkspace/cli.

For unsupported endpoints, fall through to `mws raw` to call any Graph
URL directly. Run `mws describe` for a machine-readable schema of every
command (useful for AI agents and scripts).";

const ROOT_AFTER_HELP: &str = "\
COMMON WORKFLOWS:
  # Sign in (broad scopes for personal Graph data — one consent screen)
  mws auth login

  # Who am I
  mws whoami

  # List inbox messages
  mws raw GET \"/me/messages?$top=5\"

  # Send a mail
  mws mail send --to a@b.com --subject hi --body \"test\"

  # Upload a file to OneDrive
  mws drive cp .\\file.txt mws:/Documents/file.txt

  # Send a Teams channel message via raw
  mws raw POST \"/teams/<TEAM>/channels/<CHANNEL>/messages\" --body @msg.json --header \"Content-Type:application/json\"

SHELL QUOTING:
  Paths with $ (OData $top, $select, $filter) need correct quoting:
    cmd.exe:    \"/me/messages?$top=5\"   (double quotes)
    PowerShell: '/me/messages?$top=5'   (single quotes)";

#[derive(Debug, Parser)]
#[command(
    name = "mws",
    version,
    about = "Microsoft Workspace CLI — one CLI for Microsoft 365",
    long_about = ROOT_LONG_ABOUT,
    after_help = ROOT_AFTER_HELP,
)]
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
    /// Follow @odata.nextLink and return the full collection. Only meaningful for GET requests
    /// against collection endpoints.
    #[arg(long, global = true)]
    pub all: bool,
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
    /// OneDrive / SharePoint operations.
    Drive(DriveArgs),
    /// Mail operations.
    Mail(MailArgs),
    /// Make a raw HTTP request to Microsoft Graph.
    Raw(RawArgs),
    /// Show the signed-in user.
    Whoami,
    /// Print a machine-readable description of a command (for agents/scripts).
    Describe(DescribeArgs),
}

#[derive(Debug, clap::Args)]
pub struct DescribeArgs {
    /// Command to describe (e.g. `whoami`, `mail send`, `raw`). If omitted,
    /// lists every top-level command. Pass `scopes` to print the scope catalog.
    pub path: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct DriveArgs {
    #[command(subcommand)]
    pub action: DriveAction,
}

#[derive(Debug, clap::Subcommand)]
pub enum DriveAction {
    /// Copy a file to or from OneDrive.
    Cp(CpArgs),
}

const DRIVE_CP_LONG_ABOUT: &str = "\
Copy a file between your local filesystem and OneDrive.

Paths prefixed with `mws:` are remote (e.g. `mws:/Documents/file.txt`);
anything else is treated as a local path. M1 supports local-to-remote
only; download (mws:/... -> local) and remote-to-remote ship in M2.

Files under 4 MiB go through a single PUT; larger files automatically
switch to a Graph upload session and stream in 5 MiB chunks.

Required scope: Files.ReadWrite (already in DEFAULT_SCOPES).";

const DRIVE_CP_AFTER_HELP: &str = "\
EXAMPLES:
  # Upload a small file
  mws drive cp .\\notes.txt mws:/Documents/notes.txt

  # Upload a large file (auto upload-session)
  mws drive cp .\\backup.zip mws:/Backups/backup-2026-05-12.zip

  # Upload preserving folder structure
  mws drive cp .\\report.pdf mws:/Reports/2026/Q2/report.pdf";

#[derive(Debug, clap::Args)]
#[command(long_about = DRIVE_CP_LONG_ABOUT, after_help = DRIVE_CP_AFTER_HELP)]
pub struct CpArgs {
    /// Source path. Use `mws:/path` for remote, otherwise local. (M1: local→remote only.)
    pub src: String,
    /// Destination path. Use `mws:/path` for remote, otherwise local. (M1: local→remote only.)
    pub dst: String,
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

const MAIL_SEND_LONG_ABOUT: &str = "\
Send an email as the signed-in user via Microsoft Graph.

Attachments under ~3 MiB combined are inlined into a single
`/me/sendMail` request (base64 in the body). Larger attachments switch
to an upload-session-per-file flow against `/me/messages/{id}` to
avoid Graph's 4 MiB single-request limit.

Required scope: Mail.Send (already in DEFAULT_SCOPES).";

const MAIL_SEND_AFTER_HELP: &str = "\
EXAMPLES:
  # Plain text
  mws mail send --to alice@example.com --subject hi --body \"hello\"

  # HTML body (auto-detected when it starts with <, or force --html)
  mws mail send --to alice@example.com --subject report --html --body @./report.html

  # Multiple recipients + CC
  mws mail send --to a@x.com --to b@x.com --cc team@x.com --subject \"FYI\" --body @./note.txt

  # Body from stdin (handy in pipes)
  echo \"meeting at 3\" | mws mail send --to a@x.com --subject reminder --body -

  # With attachments
  mws mail send --to a@x.com --subject \"the file\" --body \"see attached\" \\
    --attachment ./report.pdf --attachment ./chart.png";

#[derive(Debug, clap::Args)]
#[command(long_about = MAIL_SEND_LONG_ABOUT, after_help = MAIL_SEND_AFTER_HELP)]
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

const RAW_LONG_ABOUT: &str = "\
Make a raw HTTP request to Microsoft Graph.

The path is appended to the configured base URL (default
https://graph.microsoft.com/v1.0). Authorization, refresh-on-401, and
429/503 throttling are handled automatically. JSON responses are
pretty-printed via the global --output formatter; binary responses
stream through as-is.";

const RAW_AFTER_HELP: &str = "\
EXAMPLES:
  # Your profile (uses User.Read; in DEFAULT_SCOPES)
  mws raw GET /me

  # Top 5 inbox messages (uses Mail.ReadWrite)
  mws raw GET \"/me/messages?$top=5\"

  # Single message by id (--select narrows fields)
  mws raw GET \"/me/messages/<ID>?$select=subject,from\"

  # Calendar events in a window
  mws raw GET \"/me/calendarView?startDateTime=2026-05-12T00:00:00Z&endDateTime=2026-05-13T00:00:00Z\"

  # OneDrive root listing
  mws raw GET /me/drive/root/children

  # Send a mail (use `mws mail send` for ergonomics)
  mws raw POST /me/sendMail --body @msg.json --header \"Content-Type:application/json\"

  # Teams the user is a member of
  mws raw GET /me/joinedTeams

  # List channels in a team
  mws raw GET \"/teams/<TEAM-ID>/channels\"

  # Post to a Teams channel
  mws raw POST \"/teams/<TEAM-ID>/channels/<CHANNEL-ID>/messages\" --body @msg.json --header \"Content-Type:application/json\"

  # Read your chats and send a 1:1 message
  mws raw GET /me/chats
  mws raw POST \"/chats/<CHAT-ID>/messages\" --body @msg.json --header \"Content-Type:application/json\"

  # Paginate through a large collection (global --all flag)
  mws --all raw GET /me/messages

  # Beta endpoints (global --beta flag)
  mws --beta raw GET /me/insights/recent

BODY:
  --body \"literal JSON\"   pass directly
  --body @file.json       read from file
  --body -                read from stdin

HEADERS:
  Repeatable: -H \"k:v\" -H \"k2:v2\"
  Authorization is set automatically — don't pass it.";

#[derive(Debug, clap::Args)]
#[command(long_about = RAW_LONG_ABOUT, after_help = RAW_AFTER_HELP)]
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

const LOGIN_LONG_ABOUT: &str = "\
Sign in to Microsoft 365 and cache the resulting tokens locally,
AES-256-GCM-encrypted with a key in the OS keyring.

By default mws chooses auth-code + PKCE (opens your browser) on a
graphical desktop and device-code (manual code entry) on headless
systems. Force either with --device or --code.

DEFAULT_SCOPES already covers the personal-productivity surface
(mail, calendar, contacts, files, notes, tasks, Teams chat). For
admin/`.All` scopes or extra Graph features, add --scope.";

const LOGIN_AFTER_HELP: &str = "\
EXAMPLES:
  # Standard sign-in (broad delegated scopes)
  mws auth login

  # Headless / SSH session
  mws auth login --device

  # Add admin scope (will prompt for admin consent if not pre-granted)
  mws auth login --scope Sites.Read.All --scope Directory.Read.All

  # Sign in as a named account
  mws --account work auth login

  # Specific tenant
  mws --tenant contoso.onmicrosoft.com auth login

  # See what's cached
  mws auth list

  # Sign out
  mws auth logout            # current account
  mws auth logout --all      # every cached account";

#[derive(Debug, clap::Args)]
#[command(long_about = LOGIN_LONG_ABOUT, after_help = LOGIN_AFTER_HELP)]
pub struct LoginArgs {
    /// Force device-code flow.
    #[arg(long)]
    pub device: bool,
    /// Force browser auth-code+PKCE flow.
    #[arg(long, conflicts_with = "device")]
    pub code: bool,
    /// Additional OAuth scope to request, on top of DEFAULT_SCOPES.
    /// Repeatable. Common opt-ins: Sites.Read.All (SharePoint),
    /// Directory.Read.All (admin directory read), User.Read.All.
    #[arg(long = "scope")]
    pub scopes: Vec<String>,
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
