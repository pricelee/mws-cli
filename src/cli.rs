use clap::{Parser, Subcommand};

const ROOT_LONG_ABOUT: &str = "\
Microsoft Workspace CLI — one CLI for Microsoft 365, built on the
Microsoft Graph REST API. The Microsoft-side counterpart to
googleworkspace/cli.

For unsupported endpoints, fall through to `mws-cli raw` to call any Graph
URL directly. Run `mws-cli describe` for a machine-readable schema of every
command (useful for AI agents and scripts).";

const ROOT_AFTER_HELP: &str = "\
COMMON WORKFLOWS:
  # Sign in (broad scopes for personal Graph data — one consent screen)
  mws-cli auth login

  # Who am I
  mws-cli whoami

  # List inbox messages
  mws-cli raw GET \"/me/messages?$top=5\"

  # Send a mail
  mws-cli mail send --to a@b.com --subject hi --body \"test\"

  # Upload a file to OneDrive
  mws-cli drive cp .\\file.txt mws:/Documents/file.txt

  # Send a Teams channel message via raw
  mws-cli raw POST \"/teams/<TEAM>/channels/<CHANNEL>/messages\" --body @msg.json --header \"Content-Type:application/json\"

SHELL QUOTING:
  Paths with $ (OData $top, $select, $filter) need correct quoting:
    cmd.exe:    \"/me/messages?$top=5\"   (double quotes)
    PowerShell: '/me/messages?$top=5'   (single quotes)";

#[derive(Debug, Parser)]
#[command(
    name = "mws-cli",
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
    /// Print the prepared HTTP request as JSON instead of sending it.
    /// Useful for inspecting what `mws-cli raw` (or future commands) would do.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Skip the destructive-operation confirmation prompt. Required when
    /// running non-interactively (no TTY) for any DELETE or other destructive
    /// Graph call via `mws-cli raw`.
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,
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
    /// Microsoft 365 Calendar operations (events, create, find-times, rsvp, cancel).
    Calendar(CalendarArgs),
    /// OneDrive / SharePoint operations.
    Drive(DriveArgs),
    /// Mail operations.
    Mail(MailArgs),
    /// Make a raw HTTP request to Microsoft Graph.
    Raw(RawArgs),
    /// Microsoft Teams operations (list teams/channels/chats, post messages, presence).
    Teams(TeamsArgs),
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
  mws-cli drive cp .\\notes.txt mws:/Documents/notes.txt

  # Upload a large file (auto upload-session)
  mws-cli drive cp .\\backup.zip mws:/Backups/backup-2026-05-12.zip

  # Upload preserving folder structure
  mws-cli drive cp .\\report.pdf mws:/Reports/2026/Q2/report.pdf";

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
  mws-cli mail send --to alice@example.com --subject hi --body \"hello\"

  # HTML body (auto-detected when it starts with <, or force --html)
  mws-cli mail send --to alice@example.com --subject report --html --body @./report.html

  # Multiple recipients + CC
  mws-cli mail send --to a@x.com --to b@x.com --cc team@x.com --subject \"FYI\" --body @./note.txt

  # Body from stdin (handy in pipes)
  echo \"meeting at 3\" | mws-cli mail send --to a@x.com --subject reminder --body -

  # With attachments
  mws-cli mail send --to a@x.com --subject \"the file\" --body \"see attached\" \\
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
  mws-cli raw GET /me

  # Top 5 inbox messages (uses Mail.ReadWrite)
  mws-cli raw GET \"/me/messages?$top=5\"

  # Single message by id (--select narrows fields)
  mws-cli raw GET \"/me/messages/<ID>?$select=subject,from\"

  # Calendar events in a window
  mws-cli raw GET \"/me/calendarView?startDateTime=2026-05-12T00:00:00Z&endDateTime=2026-05-13T00:00:00Z\"

  # OneDrive root listing
  mws-cli raw GET /me/drive/root/children

  # Send a mail (use `mws-cli mail send` for ergonomics)
  mws-cli raw POST /me/sendMail --body @msg.json --header \"Content-Type:application/json\"

  # Teams the user is a member of
  mws-cli raw GET /me/joinedTeams

  # List channels in a team
  mws-cli raw GET \"/teams/<TEAM-ID>/channels\"

  # Post to a Teams channel
  mws-cli raw POST \"/teams/<TEAM-ID>/channels/<CHANNEL-ID>/messages\" --body @msg.json --header \"Content-Type:application/json\"

  # Read your chats and send a 1:1 message
  mws-cli raw GET /me/chats
  mws-cli raw POST \"/chats/<CHAT-ID>/messages\" --body @msg.json --header \"Content-Type:application/json\"

  # Paginate through a large collection (global --all flag)
  mws-cli --all raw GET /me/messages

  # Beta endpoints (global --beta flag)
  mws-cli --beta raw GET /me/insights/recent

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

const TEAMS_LONG_ABOUT: &str = "\
Microsoft Teams operations: list joined teams and their channels, post
to channels and chats, and read your presence.

Required scopes (already in DEFAULT_SCOPES):
  Team.ReadBasic.All, Channel.ReadBasic.All, ChannelMessage.Send,
  Chat.ReadWrite, Chat.Create, Presence.Read.";

const TEAMS_AFTER_HELP: &str = "\
EXAMPLES:
  # List teams you're a member of
  mws-cli teams list

  # List channels in a team
  mws-cli teams channels --team <TEAM-ID>

  # Post to a channel (plain text)
  mws-cli teams post --team <TEAM-ID> --channel <CHANNEL-ID> --message \"hello\"

  # Post HTML (from a file)
  mws-cli teams post --team <T> --channel <C> --html --message @./note.html

  # List your chats
  mws-cli teams chats

  # Post to a chat
  mws-cli teams chat post --chat <CHAT-ID> --message \"ping\"

  # Your presence
  mws-cli teams presence

  # Dry-run any post (prints prepared request, doesn't send)
  mws-cli teams post --team T --channel C --message hi --dry-run";

#[derive(Debug, clap::Args)]
#[command(long_about = TEAMS_LONG_ABOUT, after_help = TEAMS_AFTER_HELP)]
pub struct TeamsArgs {
    #[command(subcommand)]
    pub cmd: TeamsCmd,
}

#[derive(Debug, clap::Subcommand)]
pub enum TeamsCmd {
    /// List teams you're a member of (GET /me/joinedTeams).
    List,
    /// List channels in a team (GET /teams/{id}/channels).
    Channels(ChannelsArgs),
    /// Post a message to a channel.
    Post(ChannelPostArgs),
    /// List your chats (GET /me/chats).
    Chats,
    /// Chat operations (subcommands).
    Chat(ChatArgs),
    /// Show your presence (GET /me/presence).
    Presence,
}

#[derive(Debug, clap::Args)]
pub struct ChannelsArgs {
    /// Team id.
    #[arg(long)]
    pub team: String,
}

#[derive(Debug, clap::Args)]
pub struct ChannelPostArgs {
    /// Team id.
    #[arg(long)]
    pub team: String,
    /// Channel id.
    #[arg(long)]
    pub channel: String,
    /// Message body. Literal, or `@file`, or `-` for stdin.
    #[arg(long)]
    pub message: String,
    /// Treat the message as HTML (default: plain text).
    #[arg(long)]
    pub html: bool,
}

#[derive(Debug, clap::Args)]
pub struct ChatArgs {
    #[command(subcommand)]
    pub action: ChatAction,
}

#[derive(Debug, clap::Subcommand)]
pub enum ChatAction {
    /// Post a message to a chat.
    Post(ChatPostArgs),
}

#[derive(Debug, clap::Args)]
pub struct ChatPostArgs {
    /// Chat id.
    #[arg(long)]
    pub chat: String,
    /// Message body. Literal, or `@file`, or `-` for stdin.
    #[arg(long)]
    pub message: String,
    /// Treat the message as HTML (default: plain text).
    #[arg(long)]
    pub html: bool,
}

const CALENDAR_LONG_ABOUT: &str = "\
Microsoft 365 Calendar operations: list upcoming events, create new
events (with attendees and optional Teams online meeting), find a time
that works for a set of attendees, RSVP to invites, and cancel meetings.

Required scope (already in DEFAULT_SCOPES): Calendars.ReadWrite.";

const CALENDAR_AFTER_HELP: &str = "\
EXAMPLES:
  # What's on my calendar this week
  mws-cli calendar events

  # A specific window (ISO 8601 UTC; trailing Z)
  mws-cli calendar events --start 2026-05-16T00:00:00Z --end 2026-05-23T00:00:00Z

  # Create an event with attendees and a Teams meeting link
  mws-cli calendar create --subject \"Weekly Sync\" \\
    --start 2026-05-17T14:00:00Z --end 2026-05-17T15:00:00Z \\
    --attendee alice@x.com --attendee bob@x.com --online --body \"agenda...\"

  # Find a 30-min slot
  mws-cli calendar find-times --attendee alice@x.com --duration PT30M

  # RSVP
  mws-cli calendar rsvp --event <ID> --response accept
  mws-cli calendar rsvp --event <ID> --response decline --comment \"conflict\"

  # Cancel a meeting (sends cancellation notice to attendees)
  mws-cli calendar cancel --event <ID> --comment \"rescheduling\"

  # Dry-run any write to inspect the prepared request
  mws-cli calendar create ... --dry-run";

#[derive(Debug, clap::Args)]
#[command(long_about = CALENDAR_LONG_ABOUT, after_help = CALENDAR_AFTER_HELP)]
pub struct CalendarArgs {
    #[command(subcommand)]
    pub cmd: CalendarCmd,
}

#[derive(Debug, clap::Subcommand)]
pub enum CalendarCmd {
    /// List events in a time window (default: now → +7 days).
    Events(EventsArgs),
    /// Create an event.
    Create(CreateArgs),
    /// Find meeting times for a set of attendees.
    #[command(name = "find-times")]
    FindTimes(FindTimesArgs),
    /// RSVP (accept | decline | tentative).
    Rsvp(RsvpArgs),
    /// Cancel a meeting (sends cancellation notice).
    Cancel(CancelArgs),
}

#[derive(Debug, clap::Args)]
pub struct EventsArgs {
    /// Window start (RFC 3339 / ISO 8601 with offset or Z). Defaults to now.
    #[arg(long)]
    pub start: Option<String>,
    /// Window end. Defaults to start + 7 days.
    #[arg(long)]
    pub end: Option<String>,
    /// Max events to return per page (Graph default: 10).
    #[arg(long)]
    pub top: Option<u32>,
}

#[derive(Debug, clap::Args)]
pub struct CreateArgs {
    /// Event subject (required).
    #[arg(long)]
    pub subject: String,
    /// Start time (RFC 3339 / ISO 8601).
    #[arg(long)]
    pub start: String,
    /// End time (RFC 3339 / ISO 8601).
    #[arg(long)]
    pub end: String,
    /// Attendee email (repeatable).
    #[arg(long = "attendee")]
    pub attendees: Vec<String>,
    /// Body. Literal, `@file`, or `-` for stdin.
    #[arg(long)]
    pub body: Option<String>,
    /// Treat body as HTML (default: text).
    #[arg(long)]
    pub html: bool,
    /// Location string.
    #[arg(long)]
    pub location: Option<String>,
    /// Add a Microsoft Teams online meeting link.
    #[arg(long)]
    pub online: bool,
    /// Override the `timeZone` field on start/end (default: "UTC").
    #[arg(long)]
    pub timezone: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct FindTimesArgs {
    /// Attendee email (repeatable, required).
    #[arg(long = "attendee", required = true)]
    pub attendees: Vec<String>,
    /// Meeting duration as ISO 8601 (e.g., PT30M, PT1H).
    #[arg(long)]
    pub duration: String,
    /// Window start; default = now.
    #[arg(long)]
    pub start: Option<String>,
    /// Window end; default = start + 7 days.
    #[arg(long)]
    pub end: Option<String>,
    /// Max suggestions to return (Graph default ~3).
    #[arg(long)]
    pub top: Option<u32>,
}

#[derive(Debug, clap::Args)]
pub struct RsvpArgs {
    /// Event id.
    #[arg(long)]
    pub event: String,
    /// Response.
    #[arg(long, value_parser = ["accept", "decline", "tentative"])]
    pub response: String,
    /// Optional comment in the response email.
    #[arg(long)]
    pub comment: Option<String>,
    /// Do not send a response email back to the organizer.
    #[arg(long)]
    pub no_reply: bool,
}

#[derive(Debug, clap::Args)]
pub struct CancelArgs {
    /// Event id (must be a meeting the user organizes).
    #[arg(long)]
    pub event: String,
    /// Cancellation comment included in the email to attendees.
    #[arg(long)]
    pub comment: Option<String>,
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
    /// Print (and optionally open) an admin-consent URL. Send it to your
    /// tenant administrator; once they approve, normal sign-in works.
    #[command(name = "admin-consent")]
    AdminConsent(AdminConsentArgs),
}

const ADMIN_CONSENT_LONG_ABOUT: &str = "\
Generate a tenant-wide admin-consent URL for the scopes mws-cli needs.

When `mws-cli auth login` fails with errors like 'AADSTS65001' or
'needs admin approval', your tenant requires an administrator to
pre-consent on behalf of all users. This command builds the URL that,
when opened by an admin, lets them grant consent for the whole tenant
in one click.

mws-cli auto-detects the tenant from your signed-in account (captured
from the id_token at login time). If you haven't logged in yet — or
want to target a different tenant — pass --tenant <ID>. Send the
printed URL to your admin via Slack/email, or let mws-cli open it
in your browser (handy if you're the admin).";

const ADMIN_CONSENT_AFTER_HELP: &str = "\
EXAMPLES:
  # Generate URL for DEFAULT_SCOPES (most common — admin grants everything)
  mws-cli auth admin-consent

  # Add admin-consent scopes (admin grants these + defaults)
  mws-cli auth admin-consent --scope Sites.Read.All --scope Directory.Read.All

  # Only specific scopes — minimum-privilege admin grant
  mws-cli auth admin-consent --no-default-scopes --scope Sites.Read.All

  # Don't open browser — just print the URL (for Slack/email)
  mws-cli auth admin-consent --print-only

  # Target a specific tenant (useful when current account is on common)
  mws-cli --tenant contoso.onmicrosoft.com auth admin-consent";

#[derive(Debug, clap::Args)]
#[command(long_about = ADMIN_CONSENT_LONG_ABOUT, after_help = ADMIN_CONSENT_AFTER_HELP)]
pub struct AdminConsentArgs {
    /// Additional OAuth scope to include in the consent URL (repeatable).
    #[arg(long = "scope")]
    pub scopes: Vec<String>,
    /// Scope to drop from the default set (repeatable).
    #[arg(long = "exclude-scope")]
    pub exclude_scopes: Vec<String>,
    /// Skip DEFAULT_SCOPES; only --scope adds end up in the URL.
    #[arg(long)]
    pub no_default_scopes: bool,
    /// Print the URL only — don't try to open a browser.
    #[arg(long)]
    pub print_only: bool,
    /// Override the redirect_uri sent to Microsoft. Defaults to the
    /// Microsoft native-client redirect (admin sees a "consent granted"
    /// page from Microsoft, no listener needed on our side).
    #[arg(long, hide = true)]
    pub redirect_uri: Option<String>,
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

By default mws-cli chooses auth-code + PKCE (opens your browser) on a
graphical desktop and device-code (manual code entry) on headless
systems. Force either with --device or --code.

DEFAULT_SCOPES already covers the personal-productivity surface
(mail, calendar, contacts, files, notes, tasks, Teams chat). For
admin/`.All` scopes or extra Graph features, add --scope.

If your tenant blocks specific delegated scopes, use --exclude-scope
to drop them from the default set, or --no-default-scopes to start from
empty and request only what you list with --scope.";

const LOGIN_AFTER_HELP: &str = "\
EXAMPLES:
  # Standard sign-in (broad delegated scopes)
  mws-cli auth login

  # Headless / SSH session
  mws-cli auth login --device

  # Add admin scope (will prompt for admin consent if not pre-granted)
  mws-cli auth login --scope Sites.Read.All --scope Directory.Read.All

  # Drop scopes your tenant doesn't grant (keep the rest of the defaults)
  mws-cli auth login --exclude-scope Tasks.ReadWrite --exclude-scope Notes.ReadWrite

  # Replace the default set entirely with an explicit minimum
  mws-cli auth login --no-default-scopes --scope openid --scope offline_access --scope User.Read

  # Sign in as a named account
  mws-cli --account work auth login

  # Specific tenant
  mws-cli --tenant contoso.onmicrosoft.com auth login

  # See what's cached
  mws-cli auth list

  # Sign out
  mws-cli auth logout            # current account
  mws-cli auth logout --all      # every cached account";

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
    /// Scope to drop from the default set (repeatable). Use when your
    /// tenant blocks specific delegated scopes. Has no effect when
    /// combined with --no-default-scopes (defaults are already empty).
    #[arg(long = "exclude-scope")]
    pub exclude_scopes: Vec<String>,
    /// Skip DEFAULT_SCOPES entirely; only the scopes you list with --scope
    /// are requested. Use for minimum-privilege sign-in.
    #[arg(long)]
    pub no_default_scopes: bool,
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
