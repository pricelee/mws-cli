use std::time::Duration;

use crate::auth::auth_code;
use crate::auth::device_code;
use crate::auth::Endpoints;
use crate::auth::{Account, DEFAULT_SCOPES};

use crate::cli::{AuthAction, AuthArgs, LoginArgs};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: AuthArgs) -> anyhow::Result<()> {
    match args.action {
        AuthAction::Login(a) => login(ctx, a).await,
        AuthAction::Logout(a) => logout(ctx, a).await,
        AuthAction::List => list(ctx).await,
    }
}

fn endpoints_for(ctx: &CliContext, args: &LoginArgs) -> Endpoints {
    let default = Endpoints::for_tenant(&ctx.tenant);
    Endpoints {
        device_authorization: args
            .device_endpoint
            .as_ref()
            .map(|s| s.parse().expect("valid url"))
            .unwrap_or(default.device_authorization),
        token: args
            .token_endpoint
            .as_ref()
            .map(|s| s.parse().expect("valid url"))
            .unwrap_or(default.token),
    }
}

async fn login(ctx: &CliContext, args: LoginArgs) -> anyhow::Result<()> {
    let http = reqwest::Client::new();
    let endpoints = endpoints_for(ctx, &args);
    let mut scopes: Vec<String> = DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect();
    for s in &args.scopes {
        if !scopes.iter().any(|existing| existing == s) {
            scopes.push(s.clone());
        }
    }
    let mut account = Account::new(&ctx.account_name, &ctx.tenant, &ctx.client_id, scopes.clone());

    if args.device || !is_graphical_desktop() {
        let auth = device_code::start(&http, &endpoints, &ctx.client_id, &scopes).await?;
        if let Some(msg) = auth.message.as_deref() {
            println!("{msg}");
        } else {
            println!("Go to {} and enter code {}", auth.verification_uri, auth.user_code);
        }
        let grant = device_code::poll(&http, &endpoints, &ctx.client_id, &auth).await?;
        device_code::apply_grant(&mut account, grant);
    } else {
        let (server, redirect_uri) = auth_code::loopback()?;
        let req = auth_code::build_authorize_request(&endpoints, &ctx.tenant, &ctx.client_id, &scopes, &redirect_uri);
        let url_for_browser = args.authorize_url.clone().unwrap_or_else(|| req.authorize_url.to_string());
        println!("Opening {url_for_browser}");
        let _ = open_browser(&url_for_browser);
        let (code, state) = auth_code::await_callback(server, Duration::from_secs(300))?;
        if state != req.state {
            anyhow::bail!("OAuth state mismatch");
        }
        let grant = auth_code::exchange_code(&http, &endpoints, &ctx.client_id, &redirect_uri, &code, &req.pkce.verifier).await?;
        auth_code::apply_grant(&mut account, grant);
    }

    ctx.store.save(&account)?;
    println!("Saved account '{}' for tenant '{}'.", account.name, account.tenant);
    Ok(())
}

async fn logout(ctx: &CliContext, args: crate::cli::LogoutArgs) -> anyhow::Result<()> {
    if args.all {
        let accounts_dir = ctx.config_dir.join("accounts");
        if accounts_dir.exists() {
            for entry in std::fs::read_dir(&accounts_dir)? {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("bin") {
                    if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                        let _ = ctx.store.delete(stem); // best-effort; ignore not-found
                        println!("Removed account '{stem}'.");
                    }
                }
            }
        } else {
            println!("No accounts to remove.");
        }
    } else {
        ctx.store.delete(&ctx.account_name)?;
        println!("Removed account '{}'.", ctx.account_name);
    }
    Ok(())
}

#[cfg(not(test))]
fn is_graphical_desktop() -> bool {
    // crude heuristic — present on win/mac; on linux check DISPLAY/WAYLAND_DISPLAY
    if cfg!(windows) || cfg!(target_os = "macos") {
        return true;
    }
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(test)]
fn is_graphical_desktop() -> bool {
    false
}

async fn list(ctx: &CliContext) -> anyhow::Result<()> {
    let accounts_dir = ctx.config_dir.join("accounts");
    if !accounts_dir.exists() {
        println!("No accounts cached.");
        return Ok(());
    }
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for entry in std::fs::read_dir(&accounts_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("bin") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        match ctx.store.load(name) {
            Ok(account) => {
                let expires_in = account
                    .access_token_expires_at
                    .map(|exp| {
                        let now = crate::auth::account::now_secs() as i64;
                        let delta = exp as i64 - now;
                        if delta < 0 { format!("expired {}s ago", -delta) } else { format!("valid {delta}s") }
                    })
                    .unwrap_or_else(|| "no token".to_string());
                rows.push(serde_json::json!({
                    "name": account.name,
                    "tenant": account.tenant,
                    "username": account.username.clone().unwrap_or_default(),
                    "expires": expires_in,
                }));
            }
            Err(e) => {
                rows.push(serde_json::json!({
                    "name": name,
                    "tenant": "",
                    "username": "",
                    "expires": format!("error: {e}"),
                }));
            }
        }
    }
    if rows.is_empty() {
        println!("No accounts cached.");
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    crate::output::write(ctx.format, &rows, &mut stdout)?;
    Ok(())
}

fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // cmd treats `&` as a command separator, which would truncate the URL at the
        // first query-param boundary. raw_arg with the URL explicitly quoted prevents
        // that. Embedded double-quotes are doubled (cmd's escape).
        let arg = build_windows_cmd_arg(url);
        std::process::Command::new("cmd").raw_arg(&arg).spawn().map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
    }
}

/// Build the raw argument string passed to `cmd /C` to open `url` in the default browser
/// without letting cmd's `&` command-separator parse the URL's query string.
/// Format: `/C start "" "URL"` with any embedded `"` doubled.
#[cfg(windows)]
fn build_windows_cmd_arg(url: &str) -> String {
    let escaped = url.replace('"', "\"\"");
    format!(r#"/C start "" "{escaped}""#)
}

#[cfg(test)]
#[cfg(windows)]
mod browser_tests {
    use super::build_windows_cmd_arg;

    #[test]
    fn quotes_url_so_ampersand_is_preserved() {
        let url = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id=X&response_type=code&scope=User.Read";
        let arg = build_windows_cmd_arg(url);
        assert_eq!(
            arg,
            r#"/C start "" "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id=X&response_type=code&scope=User.Read""#
        );
        // The whole URL — every query pair — must sit inside the outer quoted region
        // so cmd does NOT interpret the `&` characters as command separators.
        let inner = &arg["/C start \"\" \"".len()..arg.len() - 1];
        assert_eq!(inner, url);
    }

    #[test]
    fn doubles_embedded_double_quotes() {
        let url = r#"https://example.com/?q="hi""#;
        let arg = build_windows_cmd_arg(url);
        assert_eq!(
            arg,
            r#"/C start "" "https://example.com/?q=""hi""""#
        );
    }
}
