use std::time::Duration;

use mws_auth::auth_code;
use mws_auth::device_code::{self, Endpoints};
use mws_auth::{Account, DEFAULT_SCOPES};

use crate::cli::{AuthAction, AuthArgs, LoginArgs};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: AuthArgs) -> anyhow::Result<()> {
    match args.action {
        AuthAction::Login(a) => login(ctx, a).await,
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
    let scopes: Vec<String> = DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect();
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

fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn().map(|_| ())
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
