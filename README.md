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
- [Status & roadmap](#status--roadmap)
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

> **Not yet on crates.io?** Until the first publish lands, `cargo install --git https://github.com/pricelee/mws-cli` builds it directly from this repo.

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

Power users / admins widen with `--scope`:

```sh
mws-cli auth login --scope Sites.Read.All --scope Directory.Read.All
```

Re-running `mws-cli auth login` with new scopes triggers Microsoft's incremental-consent prompt; already-granted scopes are not re-prompted.

Full machine-readable catalog: `mws-cli describe scopes`.

## Agent / scripting surface

`mws-cli` is built to be driven by AI agents and scripts as well as humans.

- **`mws-cli describe`** prints a JSON catalog of every command, flag, required scope, and example.
- **`mws-cli describe <command>`** returns the schema for one leaf (`mws-cli describe calendar create`).
- **`--dry-run`** on every write surfaces the exact prepared HTTP request — agents inspect before sending, retry deterministically, or hand-edit the body and replay through `mws-cli raw`.
- **Destructive-op guard** — `DELETE` and `POST .../delete|/permanentDelete|/revokeGrants|/archive` prompt on TTY; non-TTY callers must pass `--yes` or exit 4. No silent damage.
- **Stable exit codes** — 0 ok, 1 generic, 2 usage, 3 auth, 4 permission/safety, 5 network, 6 server, 7 throttled, 8 not-found, 9 conflict.

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
docs/superpowers/              # design specs and implementation plans per milestone
```

Design and per-milestone plans live in [`docs/superpowers/`](docs/superpowers/).

## Status & roadmap

Pre-1.0. Milestones shipped:

- **M0** — skeleton, auth login, whoami.
- **M1** — raw escape hatch, mail send (with attachments), drive cp (with upload sessions), throttling, paging, agent-surface polish.
- **M2a** — Teams sugar (list, channels, post, chats, chat post, presence).
- **M2b** — Calendar sugar (events, create, find-times, rsvp, cancel).

Next:

- **M2c** — Users / Groups admin sugar.
- **M3** — `mws-cli api` dynamic-from-OpenAPI layer; richer `describe`.
- **M4** — release packaging (Homebrew, Scoop, Winget, npm wrapper).

## License

Dual-licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
