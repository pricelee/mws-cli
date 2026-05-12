//! `mws describe` — machine-readable command/scope catalog for agents.
//!
//! Outputs JSON to stdout. The shape is intentionally simple so AI agents
//! and shell scripts can introspect mws without parsing `--help` text.

use serde_json::{json, Value};

use crate::cli::DescribeArgs;

pub fn run(args: DescribeArgs) -> anyhow::Result<()> {
    let value = match args.path.as_slice() {
        [] => describe_root(),
        [first] if first == "scopes" => describe_scopes(),
        path => describe_command(path)?,
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn describe_root() -> Value {
    json!({
        "binary": "mws",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Microsoft Workspace CLI — one CLI for Microsoft 365",
        "global_flags": [
            {"name": "account", "type": "string", "env": "MWS_ACCOUNT", "default": "default", "description": "Named account to use"},
            {"name": "tenant", "type": "string", "env": "MWS_TENANT", "default": "common", "description": "Tenant id, domain, or common/organizations/consumers"},
            {"name": "client-id", "type": "string", "env": "MWS_CLIENT_ID", "description": "Override OAuth client id"},
            {"name": "output", "type": "enum", "values": ["json", "table", "yaml", "tsv"], "default": "table (TTY) / json (pipe)", "description": "Output format"},
            {"name": "all", "type": "bool", "description": "Follow @odata.nextLink for collection GETs"},
            {"name": "beta", "type": "bool", "description": "Use https://graph.microsoft.com/beta"},
            {"name": "verbose", "type": "bool", "description": "Verbose tracing"},
        ],
        "commands": [
            {"name": "auth login", "description": "Sign in (device-code or auth-code+PKCE)"},
            {"name": "auth list", "description": "List cached accounts"},
            {"name": "auth logout", "description": "Sign out (remove cached credentials)"},
            {"name": "whoami", "description": "Show the signed-in user via Graph /me"},
            {"name": "raw", "description": "Make a raw HTTP request to Microsoft Graph (GET/POST/PATCH/PUT/DELETE)"},
            {"name": "mail send", "description": "Send an email"},
            {"name": "drive cp", "description": "Copy a file local→remote (M1) on OneDrive"},
            {"name": "describe", "description": "This command — print a machine-readable command/scope catalog"},
        ],
        "subcommands_for_detail": "Run `mws describe <command>` for per-command schema; `mws describe scopes` for the scope catalog.",
    })
}

fn describe_scopes() -> Value {
    use mws_auth::DEFAULT_SCOPES;
    json!({
        "default": DEFAULT_SCOPES,
        "default_includes": {
            "identity": ["openid", "profile", "email", "offline_access", "User.Read"],
            "mail": ["Mail.ReadWrite", "Mail.Send", "MailboxSettings.ReadWrite"],
            "calendar": ["Calendars.ReadWrite"],
            "contacts": ["Contacts.ReadWrite"],
            "files": ["Files.ReadWrite"],
            "notes": ["Notes.ReadWrite"],
            "tasks": ["Tasks.ReadWrite"],
            "people": ["People.Read"],
            "teams": ["Presence.Read", "Chat.ReadWrite", "Chat.Create", "Team.ReadBasic.All", "Channel.ReadBasic.All", "ChannelMessage.Send"],
        },
        "common_opt_in": {
            "Mail.Read": "Read mailbox (Mail.ReadWrite already covers this; rarely needed standalone)",
            "Mail.Send.Shared": "Send from a shared mailbox",
            "Calendars.ReadWrite.Shared": "Read/write shared calendars",
            "Sites.Read.All": "SharePoint sites (typically requires admin consent)",
            "Sites.ReadWrite.All": "SharePoint sites write (admin)",
            "Files.Read.All": "All files including shared (admin)",
            "Files.ReadWrite.All": "All files write (admin)",
            "Directory.Read.All": "Read directory (admin)",
            "User.Read.All": "Read all users in the org (admin)",
            "Group.Read.All": "Read all groups (admin)",
            "OnlineMeetings.ReadWrite": "Create/manage Teams meetings",
        },
        "command_requires": {
            "whoami": ["User.Read"],
            "mail send": ["Mail.Send"],
            "drive cp (upload)": ["Files.ReadWrite"],
            "raw GET /me/joinedTeams": ["Team.ReadBasic.All"],
            "raw GET /me/chats": ["Chat.ReadWrite"],
            "raw POST /chats/.../messages": ["Chat.ReadWrite"],
            "raw POST /teams/.../channels/.../messages": ["ChannelMessage.Send"],
        },
        "how_to_widen": "Re-run `mws auth login --scope <SCOPE>` (repeatable) to request additional scopes. Microsoft prompts for incremental consent.",
    })
}

fn describe_command(path: &[String]) -> anyhow::Result<Value> {
    let joined = path.join(" ");
    let entry = match joined.as_str() {
        "whoami" => json!({
            "name": "whoami",
            "description": "Print the signed-in user via Graph GET /me.",
            "args": [],
            "scopes_required": ["User.Read"],
            "examples": [
                {"description": "Default table output", "command": "mws whoami"},
                {"description": "JSON output", "command": "mws --output json whoami"},
            ],
        }),
        "auth login" => json!({
            "name": "auth login",
            "description": "Sign in and cache credentials encrypted with the OS keyring.",
            "args": [
                {"name": "device", "type": "bool", "description": "Force device-code flow (headless)"},
                {"name": "code", "type": "bool", "description": "Force auth-code+PKCE flow (graphical)"},
                {"name": "scope", "type": "list<string>", "repeatable": true, "description": "Additional OAuth scope on top of DEFAULT_SCOPES"},
            ],
            "scopes_required": "see `mws describe scopes`",
            "examples": [
                {"description": "Standard sign-in", "command": "mws auth login"},
                {"description": "Headless / SSH", "command": "mws auth login --device"},
                {"description": "Admin-consent scopes", "command": "mws auth login --scope Sites.Read.All --scope Directory.Read.All"},
                {"description": "Named account", "command": "mws --account work auth login"},
            ],
        }),
        "auth list" => json!({
            "name": "auth list",
            "description": "List cached accounts and token expiry.",
            "args": [],
            "examples": [
                {"description": "Show all accounts", "command": "mws auth list"},
                {"description": "JSON for scripting", "command": "mws --output json auth list"},
            ],
        }),
        "auth logout" => json!({
            "name": "auth logout",
            "description": "Remove cached credentials.",
            "args": [
                {"name": "all", "type": "bool", "description": "Remove every cached account, not just the named one"},
            ],
            "examples": [
                {"description": "Current account", "command": "mws auth logout"},
                {"description": "Everything", "command": "mws auth logout --all"},
            ],
        }),
        "raw" => json!({
            "name": "raw",
            "description": "Make a raw Microsoft Graph request. Handles 401-refresh and 429/503 retry automatically.",
            "positional_args": [
                {"name": "method", "type": "enum", "values": ["GET", "POST", "PATCH", "PUT", "DELETE"], "required": true},
                {"name": "path", "type": "string", "required": true, "description": "Path appended to https://graph.microsoft.com/v1.0 (or /beta with --beta)"},
            ],
            "flags": [
                {"name": "body", "type": "string", "description": "Literal JSON, @file, or - for stdin"},
                {"name": "header", "short": "H", "type": "list<string>", "repeatable": true, "description": "key:value, repeatable"},
            ],
            "examples": [
                {"description": "Get profile", "command": "mws raw GET /me"},
                {"description": "Top 5 messages", "command": "mws raw GET \"/me/messages?$top=5\""},
                {"description": "Calendar events in window", "command": "mws raw GET \"/me/calendarView?startDateTime=2026-05-12T00:00:00Z&endDateTime=2026-05-13T00:00:00Z\""},
                {"description": "OneDrive root", "command": "mws raw GET /me/drive/root/children"},
                {"description": "List teams", "command": "mws raw GET /me/joinedTeams"},
                {"description": "Post to channel", "command": "mws raw POST \"/teams/<TEAM>/channels/<CHANNEL>/messages\" --body @msg.json --header \"Content-Type:application/json\""},
                {"description": "Paginate fully", "command": "mws --all raw GET /me/messages"},
            ],
            "shell_quoting": {
                "cmd": "Use double-quotes around paths with $: \"/me/messages?$top=5\"",
                "powershell": "Use single-quotes: '/me/messages?$top=5'",
            },
        }),
        "mail send" => json!({
            "name": "mail send",
            "description": "Send an email. Attachments <3MiB inline, larger ones via upload session.",
            "flags": [
                {"name": "to", "type": "list<string>", "required": true, "repeatable": true},
                {"name": "cc", "type": "list<string>", "repeatable": true},
                {"name": "bcc", "type": "list<string>", "repeatable": true},
                {"name": "subject", "type": "string", "required": true},
                {"name": "body", "type": "string", "required": true, "description": "Literal, @file, or - for stdin"},
                {"name": "html", "type": "bool", "description": "Treat body as HTML (auto-detected if starts with <)"},
                {"name": "attachment", "type": "list<path>", "repeatable": true},
            ],
            "scopes_required": ["Mail.Send"],
            "examples": [
                {"description": "Plain text", "command": "mws mail send --to a@x.com --subject hi --body \"hello\""},
                {"description": "HTML from file", "command": "mws mail send --to a@x.com --subject report --html --body @./report.html"},
                {"description": "With attachments", "command": "mws mail send --to a@x.com --subject \"the file\" --body see --attachment ./a.pdf --attachment ./b.png"},
            ],
        }),
        "drive cp" => json!({
            "name": "drive cp",
            "description": "Copy a file to OneDrive. M1: local→remote only. <4MiB single PUT, ≥4MiB upload session.",
            "positional_args": [
                {"name": "src", "type": "string", "required": true, "description": "Local path or mws:/... for remote"},
                {"name": "dst", "type": "string", "required": true, "description": "Local path or mws:/... for remote"},
            ],
            "scopes_required": ["Files.ReadWrite"],
            "examples": [
                {"description": "Small file", "command": "mws drive cp .\\notes.txt mws:/Documents/notes.txt"},
                {"description": "Large file (auto upload-session)", "command": "mws drive cp .\\backup.zip mws:/Backups/backup.zip"},
            ],
        }),
        "describe" => json!({
            "name": "describe",
            "description": "Machine-readable catalog of mws commands and scopes.",
            "positional_args": [
                {"name": "path", "type": "list<string>", "description": "Command name (space-separated). Omit for top-level. Use `scopes` for scope catalog."},
            ],
            "examples": [
                {"description": "Top-level command list", "command": "mws describe"},
                {"description": "raw schema", "command": "mws describe raw"},
                {"description": "mail send schema", "command": "mws describe mail send"},
                {"description": "Scope catalog", "command": "mws describe scopes"},
            ],
        }),
        other => anyhow::bail!(
            "unknown command '{other}'. Run `mws describe` for the command list."
        ),
    };
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> DescribeArgs {
        DescribeArgs {
            path: parts.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn root_describes_all_top_level_commands() {
        let v = describe_root();
        let cmds = v["commands"].as_array().unwrap();
        let names: Vec<&str> = cmds.iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"whoami"));
        assert!(names.contains(&"auth login"));
        assert!(names.contains(&"raw"));
        assert!(names.contains(&"mail send"));
        assert!(names.contains(&"drive cp"));
        assert!(names.contains(&"describe"));
    }

    #[test]
    fn scopes_describes_default_set() {
        let v = describe_scopes();
        let default = v["default"].as_array().unwrap();
        let names: Vec<&str> = default.iter().filter_map(|s| s.as_str()).collect();
        assert!(names.contains(&"User.Read"));
        assert!(names.contains(&"Mail.Send"));
        assert!(names.contains(&"Files.ReadWrite"));
        assert!(names.contains(&"ChannelMessage.Send"));
    }

    #[test]
    fn describe_known_command_returns_schema() {
        let v = describe_command(&["raw".into()]).unwrap();
        assert_eq!(v["name"], "raw");
        assert!(v["examples"].as_array().unwrap().len() > 0);
    }

    #[test]
    fn describe_multipart_command() {
        let v = describe_command(&["mail".into(), "send".into()]).unwrap();
        assert_eq!(v["name"], "mail send");
        let flags = v["flags"].as_array().unwrap();
        assert!(flags.iter().any(|f| f["name"] == "subject"));
    }

    #[test]
    fn describe_unknown_command_errors() {
        let err = describe_command(&["bogus".into()]).unwrap_err();
        assert!(err.to_string().contains("unknown command"));
    }

    #[test]
    fn run_dispatches_correctly() {
        // Just verify dispatch doesn't panic for valid inputs.
        run(args(&[])).unwrap();
        run(args(&["scopes"])).unwrap();
        run(args(&["raw"])).unwrap();
        run(args(&["mail", "send"])).unwrap();
    }
}
