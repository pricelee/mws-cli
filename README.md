# mws — Microsoft Workspace CLI

One CLI for Microsoft 365 and Entra ID, built on Microsoft Graph. The Microsoft-side counterpart to [googleworkspace/cli](https://github.com/googleworkspace/cli).

## Status

Early development. M1 ships:

- `mws auth login` / `auth list` / `auth logout` — device-code (default for headless) or auth-code+PKCE (default for desktops)
- `mws whoami` — print the signed-in user via Graph `/me`
- `mws raw <METHOD> <path>` — escape hatch for any Graph endpoint; `--all` follows `@odata.nextLink`
- `mws mail send --to ... --subject ... --body ... [--attachment ...]` — small attachments go inline; large ones use upload sessions
- `mws drive cp <local> mws:/<remote>` — local→remote, single PUT for <4 MiB, upload session for larger
- AES-256-GCM at-rest token storage with the OS keyring
- 429/503 retry honoring `Retry-After`
- Platform-conditional keyring backends (Windows / macOS / Linux)

## Quickstart

```sh
cargo install --path crates/mws-cli
mws auth login            # opens a browser (or use --device for headless)
mws whoami
mws --output json whoami | jq .userPrincipalName
```

## Scopes

`mws auth login` requests the union of scopes needed by every shipped sugar command, so the typical case just works without thinking about consent:

- `User.Read` — `mws whoami`
- `Mail.Send` — `mws mail send`
- `Files.ReadWrite` — `mws drive cp`
- `offline_access`, `openid`, `profile` — OIDC baseline

For ad-hoc Graph reads via `mws raw` you may need extra scopes (e.g. `Mail.Read` for `/me/messages`, `Calendars.Read` for `/me/events`). Add them with `--scope`:

```sh
mws auth login --scope Mail.Read --scope Calendars.Read
```

Re-running `mws auth login` with additional scopes triggers Microsoft's incremental-consent flow.

## Manual smoke test (real tenant)

1. `cargo build --release`
2. `target/release/mws auth login`
3. Complete the flow in your browser (or follow the device-code prompt). Microsoft asks you to consent to Mail.Send and Files.ReadWrite the first time.
4. `target/release/mws whoami` should print your `displayName`, `userPrincipalName`, and `mail`.

### Shell quoting note (Windows cmd vs PowerShell)

URLs with `$` (OData query like `$top`, `$select`) need different quoting:

- **cmd.exe**: use double-quotes — `mws raw GET "/me/messages?$top=3"`
- **PowerShell**: use single-quotes — `mws raw GET '/me/messages?$top=3'`

cmd treats `'` as a literal character, so `mws raw GET '/path?...'` includes the quotes in the URL and the request fails.

## Layout

See `docs/superpowers/specs/2026-05-12-mws-cli-design.md` for the full design.
