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

## Manual smoke test (real tenant)

1. `cargo build --release`
2. `target/release/mws auth login --tenant common`
3. Complete the flow in your browser (or follow the device-code prompt).
4. `target/release/mws whoami` should print your `displayName`, `userPrincipalName`, and `mail`.

## Layout

See `docs/superpowers/specs/2026-05-12-mws-cli-design.md` for the full design.
