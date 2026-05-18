use std::time::Duration;

use crate::auth::auth_code;
use crate::auth::device_code;
use crate::auth::Endpoints;
use crate::auth::{Account, DEFAULT_SCOPES};

use crate::cli::{AdminConsentArgs, AuthAction, AuthArgs, LoginArgs};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: AuthArgs) -> anyhow::Result<()> {
    match args.action {
        AuthAction::Login(a) => login(ctx, a).await,
        AuthAction::Logout(a) => logout(ctx, a).await,
        AuthAction::List => list(ctx).await,
        AuthAction::AdminConsent(a) => admin_consent(ctx, a).await,
    }
}

/// Resolve the final scope set:
///   - Start with DEFAULT_SCOPES (unless `no_default_scopes`).
///   - Drop anything listed in `exclude`.
///   - Append `extra` (the `--scope` adds), de-duplicated.
///   - Bail if the result is empty — Graph rejects empty-scope flows and the
///     user almost certainly meant to pass something.
fn compute_scopes(
    no_default_scopes: bool,
    exclude: &[String],
    extra: &[String],
) -> anyhow::Result<Vec<String>> {
    let mut scopes: Vec<String> = if no_default_scopes {
        Vec::new()
    } else {
        DEFAULT_SCOPES
            .iter()
            .filter(|s| !exclude.iter().any(|x| x == *s))
            .map(|s| s.to_string())
            .collect()
    };
    for s in extra {
        if !scopes.iter().any(|existing| existing == s) {
            scopes.push(s.clone());
        }
    }
    if scopes.is_empty() {
        anyhow::bail!(
            "no scopes requested — pass --scope to add scopes, or drop --no-default-scopes"
        );
    }
    Ok(scopes)
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
    let scopes = compute_scopes(args.no_default_scopes, &args.exclude_scopes, &args.scopes)?;
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

/// Microsoft's hosted "your consent was recorded" page. Public clients
/// like the Microsoft Graph CLI app are registered with this redirect, so
/// the admin's browser lands somewhere meaningful after granting consent.
const DEFAULT_ADMIN_REDIRECT: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";

/// Build the admin-consent URL for a given tenant + client + scope set.
/// Pure function — easy to unit-test.
pub(crate) fn build_admin_consent_url(
    tenant: &str,
    client_id: &str,
    scopes: &[String],
    redirect_uri: &str,
) -> String {
    use url::Url;
    // v2 endpoint (`/v2.0/adminconsent`) honors the `scope` query param so the
    // admin consents only to what we list. The v1 endpoint (`/adminconsent`
    // without the version) ignores `scope` and falls back to the app's static
    // permissions, which for Microsoft Graph CLI is a much broader set and
    // would surface admin-only scopes the user never asked for.
    let mut url = Url::parse(&format!(
        "https://login.microsoftonline.com/{tenant}/v2.0/adminconsent"
    ))
    .expect("valid base url");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &scopes.join(" "));
    url.to_string()
}

/// `common` / `organizations` / `consumers` are Microsoft's multi-tenant
/// placeholders — they tell the IdP "let the user pick a tenant at sign-in",
/// but they're useless as a target for tenant-wide admin consent (the admin
/// would have to manually pick the right tenant in their browser).
fn is_placeholder_tenant(t: &str) -> bool {
    matches!(t, "common" | "organizations" | "consumers")
}

/// Resolve the tenant to target for admin-consent:
/// 1. Honor `--tenant` if it's a concrete tenant (not a placeholder).
/// 2. Otherwise read the signed-in account's stored tenant — that's the real
///    GUID Microsoft authenticated against, captured from the id_token at
///    login time.
/// 3. Fall back to whatever ctx has (and warn the user later).
fn resolve_admin_tenant(ctx: &CliContext) -> String {
    if !is_placeholder_tenant(&ctx.tenant) {
        return ctx.tenant.clone();
    }
    if let Ok(account) = ctx.store.load(&ctx.account_name) {
        if !is_placeholder_tenant(&account.tenant) {
            return account.tenant;
        }
    }
    ctx.tenant.clone()
}

async fn admin_consent(ctx: &CliContext, args: AdminConsentArgs) -> anyhow::Result<()> {
    let scopes = compute_scopes(args.no_default_scopes, &args.exclude_scopes, &args.scopes)?;
    let redirect_uri = args.redirect_uri.as_deref().unwrap_or(DEFAULT_ADMIN_REDIRECT);
    let tenant = resolve_admin_tenant(ctx);
    let url = build_admin_consent_url(&tenant, &ctx.client_id, &scopes, redirect_uri);

    println!("Admin-consent URL for tenant '{tenant}':");
    if tenant != ctx.tenant {
        println!("  (auto-detected from your signed-in account; pass --tenant to override)");
    }
    println!();
    println!("  {url}");
    println!();
    println!("Send this URL to your tenant administrator. When THEY open it and");
    println!("click 'Accept', consent is granted for the entire tenant and any");
    println!("user can then run `mws-cli auth login` without per-user prompts.");
    println!();
    println!("Heads-up: if YOU open this URL (not an admin) you'll see a");
    println!("'needs admin approval' screen — that's the normal screen for");
    println!("non-admin users. The URL only shows the admin-consent screen");
    println!("when opened by someone with tenant-admin role.");
    if is_placeholder_tenant(&tenant) {
        println!();
        println!("Note: tenant is '{tenant}' — a multi-tenant placeholder. The admin");
        println!("will land in whichever tenant their browser is signed in to. To");
        println!("target a specific tenant either sign in first (mws-cli auth login)");
        println!("so the tenant id is captured, or pass --tenant <ID> explicitly.");
    }

    if !args.print_only && is_graphical_desktop() {
        println!();
        println!("Opening URL in your default browser...");
        let _ = open_browser(&url);
    }
    Ok(())
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
mod scope_tests {
    use super::compute_scopes;
    use crate::auth::DEFAULT_SCOPES;

    #[test]
    fn default_when_no_flags() {
        let s = compute_scopes(false, &[], &[]).unwrap();
        assert_eq!(s.len(), DEFAULT_SCOPES.len());
        assert!(s.contains(&"User.Read".to_string()));
        assert!(s.contains(&"Mail.Send".to_string()));
    }

    #[test]
    fn extra_appended_and_deduped() {
        let s = compute_scopes(
            false,
            &[],
            &["Sites.Read.All".into(), "User.Read".into()],
        )
        .unwrap();
        // User.Read already in defaults → only one copy in the final list.
        let user_read_count = s.iter().filter(|x| x.as_str() == "User.Read").count();
        assert_eq!(user_read_count, 1);
        assert!(s.contains(&"Sites.Read.All".to_string()));
    }

    #[test]
    fn exclude_drops_from_defaults() {
        let s = compute_scopes(false, &["Tasks.ReadWrite".into()], &[]).unwrap();
        assert!(!s.contains(&"Tasks.ReadWrite".to_string()));
        // Other defaults survive.
        assert!(s.contains(&"User.Read".to_string()));
    }

    #[test]
    fn no_default_starts_empty_then_adds_extra() {
        let s = compute_scopes(
            true,
            &[],
            &["openid".into(), "User.Read".into()],
        )
        .unwrap();
        assert_eq!(s, vec!["openid".to_string(), "User.Read".to_string()]);
    }

    #[test]
    fn no_default_without_extra_errors() {
        let err = compute_scopes(true, &[], &[]).unwrap_err();
        assert!(err.to_string().contains("no scopes requested"));
    }

    #[test]
    fn admin_consent_url_has_required_params() {
        let url = super::build_admin_consent_url(
            "contoso.onmicrosoft.com",
            "14d82eec-204b-4c2f-b7e8-296a70dab67e",
            &["User.Read".into(), "Sites.Read.All".into()],
            super::DEFAULT_ADMIN_REDIRECT,
        );
        assert!(url.starts_with("https://login.microsoftonline.com/contoso.onmicrosoft.com/v2.0/adminconsent?"));
        assert!(url.contains("client_id=14d82eec-204b-4c2f-b7e8-296a70dab67e"));
        // scope is space-separated, then URL-encoded to %20:
        assert!(url.contains("scope=User.Read+Sites.Read.All") || url.contains("scope=User.Read%20Sites.Read.All"));
        assert!(url.contains("redirect_uri="));
    }

    #[test]
    fn admin_consent_url_uses_common_when_tenant_unspecified() {
        let url = super::build_admin_consent_url(
            "common",
            "X",
            &["openid".into()],
            super::DEFAULT_ADMIN_REDIRECT,
        );
        assert!(url.starts_with("https://login.microsoftonline.com/common/v2.0/adminconsent?"));
    }

    #[test]
    fn explicit_scope_wins_over_exclude() {
        // Edge case: user excludes User.Read but also passes --scope User.Read.
        // Explicit add wins — we don't second-guess.
        let s = compute_scopes(
            false,
            &["User.Read".into()],
            &["User.Read".into()],
        )
        .unwrap();
        assert!(s.contains(&"User.Read".to_string()));
    }
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
