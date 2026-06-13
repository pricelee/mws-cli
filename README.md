# mws-cli

> One CLI for Microsoft 365 / Microsoft Graph — the Microsoft-side counterpart to [`googleworkspace/cli`](https://github.com/googleworkspace/cli).

[![crates.io](https://img.shields.io/crates/v/mws-cli.svg)](https://crates.io/crates/mws-cli)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.86%2B-orange.svg)](rust-toolchain.toml)

Send mail, post to Teams, manage your calendar, upload to OneDrive, and reach any Graph endpoint — all from a single binary, with OAuth and token storage handled for you.

```sh
cargo install mws-cli
mws-cli auth login
mws-cli whoami
mws-cli teams post --team <TEAM-ID> --channel <CHANNEL-ID> --message "hello from mws-cli"
```

---

## Table of contents

- [Why](#why)
- [Install](#install)
- [Quickstart](#quickstart)
- [Commands](#commands)
- [Output formats](#output-formats)
- [Authentication](#authentication)
- [Scopes](#scopes)
- [Agent / scripting surface](#agent--scripting-surface)
- [Shell quoting on Windows](#shell-quoting-on-windows)
- [Building from source](#building-from-source)
- [Project layout](#project-layout)
- [License](#license)

## Why

The Microsoft Graph REST API is huge, the OAuth dance is fiddly, and small ergonomic things — pagination, throttling, upload sessions, refresh-on-401, dry-run, output formatting — have to be re-implemented every time. `mws-cli` makes them defaults.

- **Sugar layer** — typed commands for the workloads you use daily (mail, drive, calendar, teams).
- **Raw escape hatch** — `mws-cli raw <METHOD> <path>` reaches any Graph v1.0 (or `--beta`) endpoint, with auth and retries already wired.
- **Agent-ready** — JSON output for non-TTY, `--dry-run` on every mutating command, machine-readable `mws-cli describe` schemas, exit codes, destructive-op guard.

## Install

```sh
cargo install mws-cli
```

Requires Rust 1.86+. The crate is a single binary named `mws-cli`.

### Windows users — use the MSVC toolchain

`mws-cli` (and most of the Rust ecosystem) targets `x86_64-pc-windows-msvc`. If you installed Rust through `winget install Rustlang.Rust.GNU` or any other GNU-target package, `cargo install mws-cli` will fail with errors like:

```
error: linker `dlltool.exe` not found
error: failed to compile `windows-sys`
```

Fix it once and for all by switching to the `rustup` installer, which manages MSVC properly:

```sh
# 1. Remove a GNU-only Rust if you have one
winget uninstall Rustlang.Rust.GNU

# 2. Install the official rustup
winget install --id Rustlang.Rustup

# 3. Open a NEW shell, then:
rustup default stable-x86_64-pc-windows-msvc

# 4. Now this works:
cargo install mws-cli
```

The first build also needs **Visual Studio Build Tools 2022** with the "Desktop development with C++" workload. `rustup-init` will prompt you to install it automatically if it's missing. Or install it manually:

```sh
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
```

### macOS / Linux

Standard `rustup` install from <https://rustup.rs> is all you need — no extra toolchain setup.

## Quickstart

```sh
mws-cli auth login                      # opens browser (or use --device for SSH/headless)
mws-cli whoami                          # confirms the signed-in user
mws-cli mail send --to a@x.com --subject hi --body "hello"
mws-cli calendar events                 # next 7 days
mws-cli drive cp ./notes.txt mws:/Documents/notes.txt
mws-cli teams list
```

## Commands

| Workload | Commands |
|---|---|
| **auth** | `login` (device-code / auth-code+PKCE), `list`, `logout [--all]` |
| **whoami** | profile via Graph `/me` |
| **raw** | `raw <METHOD> <path> [--body @file\|-] [--header k:v]...` — any Graph endpoint, with `--all` paging |
| **mail** | `send --to ... --subject ... --body ... [--attachment ...]` — small attachments inline, large ones via upload session |
| **drive** | `cp <local> mws:/<remote>` — single PUT under 4 MiB, chunked upload session above |
| **teams** | `list`, `channels --team <id>`, `post --team <id> --channel <id> --message <s> [--html]`, `chats`, `chat post --chat <id> --message <s>`, `presence` |
| **calendar** | `events [--start --end]`, `create --subject --start --end --attendee... [--online --body --location]`, `find-times --attendee... --duration PT30M`, `rsvp --event <id> --response accept\|decline\|tentative`, `cancel --event <id>` |
| **describe** | machine-readable command/scope catalog for AI agents |

Every mutating command supports **`--dry-run`** — prints the prepared HTTP request as JSON and exits 0 without sending. Useful for inspection and agent self-correction.

```sh
mws-cli mail send --to a@x.com --subject hi --body "test" --dry-run
mws-cli calendar create --subject Sync --start 2026-05-20T14:00:00Z --end 2026-05-20T15:00:00Z --attendee a@x.com --online --dry-run
mws-cli raw DELETE /me/messages/<ID> --dry-run
```

## Output formats

- **TTY stdout** → human table by default.
- **Pipe / redirect / agent** → JSON by default.
- Override anywhere with `--output {json|table|yaml|tsv}` (or `-o`).
- `--all` follows `@odata.nextLink` to materialize the entire collection.

```sh
mws-cli teams list                                # table
mws-cli teams list -o json | jq '.value[].displayName'
mws-cli --all raw GET /me/messages -o json        # full inbox as a JSON array
```

## Authentication

Two flows ship in M1:

- **Auth code + PKCE** (default on desktops) — opens a browser, listens on `http://localhost:<random>`, exchanges the code, persists tokens.
- **Device code** (`--device`) — for SSH, CI, or anywhere without a browser. Print a code, you visit `microsoft.com/devicelogin`.

Tokens are stored **AES-256-GCM-encrypted on disk** with the encryption key held in the OS keyring (Windows Credential Manager, macOS Keychain, Linux Secret Service / kwallet). The same shape as [`gws`](https://github.com/googleworkspace/cli).

Multiple named accounts:

```sh
mws-cli --account work auth login
mws-cli --account personal auth login
mws-cli --account work mail send ...
```

## Scopes

`mws-cli auth login` requests a single broad consent screen covering the personal-productivity surface — mail, calendar, contacts, OneDrive, OneNote, To Do, Teams chat/presence. No admin-consent (`*.All`) scopes by default.

| Workload | Scopes |
|---|---|
| Identity | `openid`, `profile`, `email`, `offline_access`, `User.Read` |
| Mail | `Mail.ReadWrite`, `Mail.Send`, `MailboxSettings.ReadWrite` |
| Calendar | `Calendars.ReadWrite` |
| Contacts | `Contacts.ReadWrite` |
| Files | `Files.ReadWrite` |
| OneNote | `Notes.ReadWrite` |
| Tasks | `Tasks.ReadWrite` |
| People | `People.Read` |
| Teams | `Presence.Read`, `Chat.ReadWrite`, `Chat.Create`, `Team.ReadBasic.All`, `Channel.ReadBasic.All`, `ChannelMessage.Send` |

### Adjusting the requested scopes

`mws-cli auth login` has three flags that compose:

| Flag | Effect |
|---|---|
| `--scope <SCOPE>` (repeatable) | **Add** to the default set. Most common use: opt into admin / `*.All` scopes. |
| `--exclude-scope <SCOPE>` (repeatable) | **Drop** a scope from the default set. Use when your tenant blocks specific delegated scopes. |
| `--no-default-scopes` | **Skip** DEFAULT_SCOPES entirely. Only the scopes you list with `--scope` are requested. |

Resolution order: defaults → minus excludes → plus explicit adds. An explicit `--scope` always wins over `--exclude-scope` for the same scope name. If the final set is empty, sign-in errors out (Graph rejects empty-scope flows).

```sh
# 1. Add admin-consent scopes (opens an admin-approval prompt if needed)
mws-cli auth login --scope Sites.Read.All --scope Directory.Read.All

# 2. Tenant blocks Tasks and Notes — drop them, keep the rest
mws-cli auth login \
  --exclude-scope Tasks.ReadWrite \
  --exclude-scope Notes.ReadWrite

# 3. Minimum-privilege sign-in — just identity, nothing else
mws-cli auth login --no-default-scopes \
  --scope openid \
  --scope offline_access \
  --scope User.Read

# 4. Custom set tailored to one workload (mail only)
mws-cli auth login --no-default-scopes \
  --scope openid --scope offline_access --scope User.Read \
  --scope Mail.ReadWrite --scope Mail.Send
```

Re-running `mws-cli auth login` with different scopes triggers Microsoft's incremental-consent prompt; already-granted scopes are not re-prompted.

### Common admin-consent (`*.All`) scopes

These typically need admin approval — opt in only when you know your tenant allows them:

| Scope | Use |
|---|---|
| `Sites.Read.All` / `Sites.ReadWrite.All` | SharePoint sites |
| `Files.Read.All` / `Files.ReadWrite.All` | All files including shared |
| `Directory.Read.All` | Read directory (users, groups, devices) |
| `User.Read.All` | All users in the org |
| `Group.Read.All` / `Group.ReadWrite.All` | All groups |
| `OnlineMeetings.ReadWrite` | Create/manage Teams meetings |
| `ChannelMessage.Read.All` | Read Teams channel messages (also gated by Microsoft "Protected APIs for Teams") |
| `Chat.Read.All` | Read all chats in the tenant, not just your own |
| `Mail.Send.Shared` | Send from a shared mailbox |
| `Calendars.ReadWrite.Shared` | Read/write shared calendars |

### Requesting admin approval

If `mws-cli auth login` fails with `AADSTS65001`, `AADSTS90094`, or "needs admin approval", your tenant requires an administrator to pre-consent on behalf of all users. Generate the URL admin needs to click:

```sh
# Default: URL covers DEFAULT_SCOPES — admin clicks once, sign-in works for everyone
mws-cli auth admin-consent

# Add scopes that need admin consent on top of defaults
mws-cli auth admin-consent --scope Sites.Read.All --scope Directory.Read.All

# Only specific scopes — minimum-privilege admin grant
mws-cli auth admin-consent --no-default-scopes --scope Sites.Read.All

# Print-only mode (no browser launch) — handy for sending via Slack/email
mws-cli auth admin-consent --print-only

# Target a specific tenant (recommended over the default 'common')
mws-cli --tenant contoso.onmicrosoft.com auth admin-consent
```

The URL points to Microsoft's `/{tenant}/adminconsent` endpoint. When the admin opens it and clicks **Accept**, consent is recorded tenant-wide. After that any user in the tenant can run `mws-cli auth login` without per-user consent prompts.

**Tenant auto-detection:** if you've already signed in once, `mws-cli` captures your real tenant id from the id_token and uses it automatically — you don't need `--tenant`. Pass it only if you want to target a different tenant than you signed in to.

### Automatic remediation (you don't have to know any of this up front)

You rarely need to reach for `admin-consent` manually. When a command — or `auth login` itself — fails because a scope is missing or ungranted, `mws-cli` diagnoses it and prints the next step for you:

- **You can grant it yourself** → it tells you the exact `mws-cli auth login --scope <SCOPE>` to run. No admin involved.
- **It needs admin consent** → it prints a ready-to-send **admin-consent URL** (scoped to the minimum needed, targeting your tenant) plus a paste-ready message for your admin, and the steps to finish once they accept.

When a runtime call lists several acceptable scopes, `mws-cli` prefers one you can self-consent to, so you only escalate to an admin when there's genuinely no self-service path.

After your admin accepts, your cached token still doesn't have the new scope — re-run `mws-cli auth login --scope <SCOPE>` once (it succeeds silently now), then re-run your original command. `mws-cli` spells these steps out in the failure message.

**Exit codes:** a sign-in that needs admin consent exits `3` (auth); a runtime Graph 403 insufficient-scope exits `4` (permission). In JSON output mode (the non-TTY default) the failure carries a structured `remediation` object on stderr (`type`, `scopes`, `consent_url`, `next_steps`) so agents can act on it — see `mws-cli describe`.

Full machine-readable catalog: `mws-cli describe scopes`.

## Agent / scripting surface

`mws-cli` is built to be driven by AI agents and scripts as well as humans.

- **`mws-cli describe`** prints a JSON catalog of every command, flag, required scope, and example.
- **`mws-cli describe <command>`** returns the schema for one leaf (`mws-cli describe calendar create`).
- **`--dry-run`** on every write surfaces the exact prepared HTTP request — agents inspect before sending, retry deterministically, or hand-edit the body and replay through `mws-cli raw`.
- **Destructive-op guard** — `DELETE` and `POST .../delete|/permanentDelete|/revokeGrants|/archive` prompt on TTY; non-TTY callers must pass `--yes` or exit 4. No silent damage.
- **Stable exit codes** — 0 ok, 1 generic, 2 usage, 3 auth, 4 permission/safety, 5 network, 6 server, 7 throttled, 8 not-found, 9 conflict.
- **Actionable consent errors** — permission/consent failures (exit 3/4) carry a structured `remediation` object on stderr (`type`, `scopes`, `consent_url`, `next_steps`) so an agent can forward the admin-consent URL or run the self-consent command without scraping text. See [Automatic remediation](#automatic-remediation-you-dont-have-to-know-any-of-this-up-front).

## Shell quoting on Windows

URLs containing `$` (OData `$top`, `$select`, `$filter`) need different quoting per shell:

| Shell | Style | Example |
|---|---|---|
| `cmd.exe` | double-quotes | `mws-cli raw GET "/me/messages?$top=3"` |
| PowerShell | single-quotes | `mws-cli raw GET '/me/messages?$top=3'` |

`cmd` treats `'` as a literal character, so `mws-cli raw GET '/path?...'` would include the quotes in the URL and the request would fail. On macOS / Linux, single-quotes are always fine.

## Building from source

```sh
git clone https://github.com/pricelee/mws-cli
cd mws-cli
cargo build --release
./target/release/mws-cli --help
```

Run the test suite (uses `wiremock`, no real tenant required):

```sh
cargo test --features test-helpers
```

## Project layout

```
src/
├── main.rs                    # binary entry
├── cli.rs                     # clap derive (root + subcommand args)
├── context.rs errors.rs safety.rs
├── auth/                      # OAuth flows + account storage
├── graph/                     # Graph HTTP client (auth, retry, paging, upload)
├── keyring/                   # AES-256-GCM vault over OS keyring
├── output.rs                  # JSON/table/YAML/TSV formatters
└── commands/                  # one module per workload
    ├── auth.rs whoami.rs raw.rs describe.rs util.rs
    ├── mail/   drive/   teams/   calendar/
tests/                         # integration tests (assert_cmd + wiremock)
```

## License

Dual-licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
