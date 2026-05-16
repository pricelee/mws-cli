//! `mws-cli describe` — machine-readable command/scope catalog for agents.
//!
//! Outputs JSON to stdout. The shape is intentionally simple so AI agents
//! and shell scripts can introspect mws-cli without parsing `--help` text.

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
        "binary": "mws-cli",
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
            {"name": "dry-run", "type": "bool", "description": "Print the prepared HTTP request as JSON instead of sending it"},
            {"name": "yes", "short": "y", "type": "bool", "description": "Skip the destructive-op confirmation prompt (required for non-TTY/agent use of DELETE etc.)"},
        ],
        "safety": {
            "destructive_methods": ["DELETE"],
            "destructive_post_suffixes": ["/delete", "/permanentDelete", "/revokeGrants", "/archive"],
            "behavior": "TTY users get a y/N prompt before any destructive operation. Non-TTY callers (agents, scripts) must pass --yes explicitly; otherwise mws-cli exits 4 with a hint.",
            "exit_code_safety_refused": 4,
        },
        "commands": [
            {"name": "auth login", "description": "Sign in (device-code or auth-code+PKCE)"},
            {"name": "auth list", "description": "List cached accounts"},
            {"name": "auth logout", "description": "Sign out (remove cached credentials)"},
            {"name": "whoami", "description": "Show the signed-in user via Graph /me"},
            {"name": "raw", "description": "Make a raw HTTP request to Microsoft Graph (GET/POST/PATCH/PUT/DELETE)"},
            {"name": "mail send", "description": "Send an email"},
            {"name": "drive cp", "description": "Copy a file local→remote (M1) on OneDrive"},
            {"name": "teams list", "description": "List joined Microsoft Teams"},
            {"name": "teams channels", "description": "List channels in a team"},
            {"name": "teams post", "description": "Post a message to a Teams channel"},
            {"name": "teams chats", "description": "List your Teams chats"},
            {"name": "teams chat post", "description": "Post a message to a Teams chat"},
            {"name": "teams presence", "description": "Show your Teams presence"},
            {"name": "calendar events", "description": "List calendar events in a time window"},
            {"name": "calendar create", "description": "Create a calendar event"},
            {"name": "calendar find-times", "description": "Find meeting times for a set of attendees"},
            {"name": "calendar rsvp", "description": "RSVP (accept|decline|tentative) to an event"},
            {"name": "calendar cancel", "description": "Cancel a meeting"},
            {"name": "describe", "description": "This command — print a machine-readable command/scope catalog"},
        ],
        "subcommands_for_detail": "Run `mws-cli describe <command>` for per-command schema; `mws-cli describe scopes` for the scope catalog.",
    })
}

fn describe_scopes() -> Value {
    use crate::auth::DEFAULT_SCOPES;
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
        "how_to_widen": "Re-run `mws-cli auth login --scope <SCOPE>` (repeatable) to request additional scopes. Microsoft prompts for incremental consent.",
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
                {"description": "Default table output", "command": "mws-cli whoami"},
                {"description": "JSON output", "command": "mws-cli --output json whoami"},
            ],
        }),
        "auth login" => json!({
            "name": "auth login",
            "description": "Sign in and cache credentials encrypted with the OS keyring.",
            "args": [
                {"name": "device", "type": "bool", "description": "Force device-code flow (headless)"},
                {"name": "code", "type": "bool", "description": "Force auth-code+PKCE flow (graphical)"},
                {"name": "scope", "type": "list<string>", "repeatable": true, "description": "Additional OAuth scope on top of DEFAULT_SCOPES"},
                {"name": "exclude-scope", "type": "list<string>", "repeatable": true, "description": "Drop these scopes from the default set (use when your tenant blocks them)"},
                {"name": "no-default-scopes", "type": "bool", "description": "Skip DEFAULT_SCOPES entirely; only --scope adds will be requested"},
            ],
            "scopes_required": "see `mws-cli describe scopes`",
            "examples": [
                {"description": "Standard sign-in", "command": "mws-cli auth login"},
                {"description": "Headless / SSH", "command": "mws-cli auth login --device"},
                {"description": "Admin-consent scopes", "command": "mws-cli auth login --scope Sites.Read.All --scope Directory.Read.All"},
                {"description": "Drop tenant-blocked scopes", "command": "mws-cli auth login --exclude-scope Tasks.ReadWrite --exclude-scope Notes.ReadWrite"},
                {"description": "Minimum-privilege sign-in", "command": "mws-cli auth login --no-default-scopes --scope openid --scope offline_access --scope User.Read"},
                {"description": "Named account", "command": "mws-cli --account work auth login"},
            ],
        }),
        "auth list" => json!({
            "name": "auth list",
            "description": "List cached accounts and token expiry.",
            "args": [],
            "examples": [
                {"description": "Show all accounts", "command": "mws-cli auth list"},
                {"description": "JSON for scripting", "command": "mws-cli --output json auth list"},
            ],
        }),
        "auth logout" => json!({
            "name": "auth logout",
            "description": "Remove cached credentials.",
            "args": [
                {"name": "all", "type": "bool", "description": "Remove every cached account, not just the named one"},
            ],
            "examples": [
                {"description": "Current account", "command": "mws-cli auth logout"},
                {"description": "Everything", "command": "mws-cli auth logout --all"},
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
                {"description": "Get profile", "command": "mws-cli raw GET /me"},
                {"description": "Top 5 messages", "command": "mws-cli raw GET \"/me/messages?$top=5\""},
                {"description": "Calendar events in window", "command": "mws-cli raw GET \"/me/calendarView?startDateTime=2026-05-12T00:00:00Z&endDateTime=2026-05-13T00:00:00Z\""},
                {"description": "OneDrive root", "command": "mws-cli raw GET /me/drive/root/children"},
                {"description": "List teams", "command": "mws-cli raw GET /me/joinedTeams"},
                {"description": "Post to channel", "command": "mws-cli raw POST \"/teams/<TEAM>/channels/<CHANNEL>/messages\" --body @msg.json --header \"Content-Type:application/json\""},
                {"description": "Paginate fully", "command": "mws-cli --all raw GET /me/messages"},
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
                {"description": "Plain text", "command": "mws-cli mail send --to a@x.com --subject hi --body \"hello\""},
                {"description": "HTML from file", "command": "mws-cli mail send --to a@x.com --subject report --html --body @./report.html"},
                {"description": "With attachments", "command": "mws-cli mail send --to a@x.com --subject \"the file\" --body see --attachment ./a.pdf --attachment ./b.png"},
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
                {"description": "Small file", "command": "mws-cli drive cp .\\notes.txt mws:/Documents/notes.txt"},
                {"description": "Large file (auto upload-session)", "command": "mws-cli drive cp .\\backup.zip mws:/Backups/backup.zip"},
            ],
        }),
        "teams list" => json!({
            "name": "teams list",
            "description": "List teams the signed-in user is a member of (GET /me/joinedTeams).",
            "args": [],
            "scopes_required": ["Team.ReadBasic.All"],
            "examples": [
                {"description": "Table output", "command": "mws-cli teams list"},
                {"description": "Paginate all teams", "command": "mws-cli --all teams list"},
            ],
        }),
        "teams channels" => json!({
            "name": "teams channels",
            "description": "List channels in a team (GET /teams/{team}/channels).",
            "flags": [{"name": "team", "type": "string", "required": true}],
            "scopes_required": ["Channel.ReadBasic.All"],
            "examples": [
                {"description": "Channels for a team", "command": "mws-cli teams channels --team <TEAM-ID>"},
            ],
        }),
        "teams post" => json!({
            "name": "teams post",
            "description": "Post a message to a Teams channel.",
            "flags": [
                {"name": "team", "type": "string", "required": true},
                {"name": "channel", "type": "string", "required": true},
                {"name": "message", "type": "string", "required": true,
                 "description": "Literal text, `@file`, or `-` for stdin"},
                {"name": "html", "type": "bool"},
            ],
            "scopes_required": ["ChannelMessage.Send"],
            "examples": [
                {"description": "Plain text", "command": "mws-cli teams post --team T --channel C --message \"hi\""},
                {"description": "HTML from file", "command": "mws-cli teams post --team T --channel C --html --message @./note.html"},
                {"description": "Dry-run", "command": "mws-cli teams post --team T --channel C --message hi --dry-run"},
            ],
        }),
        "teams chats" => json!({
            "name": "teams chats",
            "description": "List your Teams chats (GET /me/chats).",
            "args": [],
            "scopes_required": ["Chat.ReadWrite"],
            "examples": [
                {"description": "Table output", "command": "mws-cli teams chats"},
            ],
        }),
        "teams chat post" => json!({
            "name": "teams chat post",
            "description": "Post a message to a Teams chat.",
            "flags": [
                {"name": "chat", "type": "string", "required": true},
                {"name": "message", "type": "string", "required": true,
                 "description": "Literal text, `@file`, or `-` for stdin"},
                {"name": "html", "type": "bool"},
            ],
            "scopes_required": ["Chat.ReadWrite"],
            "examples": [
                {"description": "Send", "command": "mws-cli teams chat post --chat C --message ping"},
            ],
        }),
        "teams presence" => json!({
            "name": "teams presence",
            "description": "Show your Microsoft Teams presence (GET /me/presence).",
            "args": [],
            "scopes_required": ["Presence.Read"],
            "examples": [
                {"description": "Default table", "command": "mws-cli teams presence"},
            ],
        }),
        "calendar events" => json!({
            "name": "calendar events",
            "description": "List events in a time window. Defaults to now → +7 days.",
            "flags": [
                {"name": "start", "type": "string", "description": "ISO 8601 (default: now)"},
                {"name": "end", "type": "string", "description": "ISO 8601 (default: start + 7 days)"},
                {"name": "top", "type": "u32", "description": "Page size"},
            ],
            "scopes_required": ["Calendars.ReadWrite"],
            "examples": [
                {"description": "This week's events", "command": "mws-cli calendar events"},
                {"description": "Explicit window", "command": "mws-cli calendar events --start 2026-05-16T00:00:00Z --end 2026-05-23T00:00:00Z"},
                {"description": "Paginate all", "command": "mws-cli --all calendar events"},
            ],
        }),
        "calendar create" => json!({
            "name": "calendar create",
            "description": "Create a calendar event.",
            "flags": [
                {"name": "subject", "type": "string", "required": true},
                {"name": "start", "type": "string", "required": true, "description": "ISO 8601"},
                {"name": "end", "type": "string", "required": true, "description": "ISO 8601"},
                {"name": "attendee", "type": "list<string>", "repeatable": true},
                {"name": "body", "type": "string", "description": "Literal, @file, or - for stdin"},
                {"name": "html", "type": "bool"},
                {"name": "location", "type": "string"},
                {"name": "online", "type": "bool", "description": "Add Teams meeting link"},
                {"name": "timezone", "type": "string", "description": "Override timeZone (default: UTC)"},
            ],
            "scopes_required": ["Calendars.ReadWrite"],
            "examples": [
                {"description": "Simple", "command": "mws-cli calendar create --subject Sync --start 2026-05-17T14:00:00Z --end 2026-05-17T15:00:00Z --attendee a@x.com"},
                {"description": "Online + body", "command": "mws-cli calendar create --subject Standup --start 2026-05-17T09:00:00Z --end 2026-05-17T09:30:00Z --attendee a@x.com --attendee b@x.com --online --body @./agenda.md --html"},
            ],
        }),
        "calendar find-times" => json!({
            "name": "calendar find-times",
            "description": "Find meeting times for a set of attendees (POST /me/findMeetingTimes).",
            "flags": [
                {"name": "attendee", "type": "list<string>", "required": true, "repeatable": true},
                {"name": "duration", "type": "string", "required": true, "description": "ISO 8601 duration (e.g., PT30M)"},
                {"name": "start", "type": "string"},
                {"name": "end", "type": "string"},
                {"name": "top", "type": "u32"},
            ],
            "scopes_required": ["Calendars.ReadWrite"],
            "examples": [
                {"description": "30-min slot", "command": "mws-cli calendar find-times --attendee alice@x.com --duration PT30M"},
            ],
        }),
        "calendar rsvp" => json!({
            "name": "calendar rsvp",
            "description": "RSVP to an event (accept | decline | tentative).",
            "flags": [
                {"name": "event", "type": "string", "required": true},
                {"name": "response", "type": "enum", "values": ["accept", "decline", "tentative"], "required": true},
                {"name": "comment", "type": "string"},
                {"name": "no-reply", "type": "bool"},
            ],
            "scopes_required": ["Calendars.ReadWrite"],
            "examples": [
                {"description": "Accept", "command": "mws-cli calendar rsvp --event <ID> --response accept"},
                {"description": "Decline with comment", "command": "mws-cli calendar rsvp --event <ID> --response decline --comment conflict"},
            ],
        }),
        "calendar cancel" => json!({
            "name": "calendar cancel",
            "description": "Cancel a meeting (sends cancellation notice to attendees).",
            "flags": [
                {"name": "event", "type": "string", "required": true},
                {"name": "comment", "type": "string"},
            ],
            "scopes_required": ["Calendars.ReadWrite"],
            "examples": [
                {"description": "Cancel", "command": "mws-cli calendar cancel --event <ID>"},
                {"description": "With comment", "command": "mws-cli calendar cancel --event <ID> --comment rescheduling"},
            ],
        }),
        "describe" => json!({
            "name": "describe",
            "description": "Machine-readable catalog of mws-cli commands and scopes.",
            "positional_args": [
                {"name": "path", "type": "list<string>", "description": "Command name (space-separated). Omit for top-level. Use `scopes` for scope catalog."},
            ],
            "examples": [
                {"description": "Top-level command list", "command": "mws-cli describe"},
                {"description": "raw schema", "command": "mws-cli describe raw"},
                {"description": "mail send schema", "command": "mws-cli describe mail send"},
                {"description": "Scope catalog", "command": "mws-cli describe scopes"},
            ],
        }),
        other => anyhow::bail!(
            "unknown command '{other}'. Run `mws-cli describe` for the command list."
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
        assert!(names.contains(&"teams post"));
        assert!(names.contains(&"teams presence"));
        assert!(names.contains(&"calendar events"));
        assert!(names.contains(&"calendar create"));
        assert!(names.contains(&"describe"));
    }

    #[test]
    fn describe_teams_subcommands() {
        for name in [
            "teams list",
            "teams channels",
            "teams post",
            "teams chats",
            "teams chat post",
            "teams presence",
        ] {
            let parts: Vec<String> = name.split_whitespace().map(str::to_string).collect();
            let v = describe_command(&parts).unwrap();
            assert_eq!(v["name"], name);
        }
    }

    #[test]
    fn describe_calendar_subcommands() {
        for name in [
            "calendar events",
            "calendar create",
            "calendar find-times",
            "calendar rsvp",
            "calendar cancel",
        ] {
            let parts: Vec<String> = name.split_whitespace().map(str::to_string).collect();
            let v = describe_command(&parts).unwrap();
            assert_eq!(v["name"], name);
        }
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
