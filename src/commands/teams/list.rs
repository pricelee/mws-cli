//! `mws-cli teams list|channels|chats` — collection GETs.

use crate::auth::Endpoints;
use crate::graph::GraphClient;
use crate::output::write;
use serde_json::Value;

use crate::context::CliContext;

pub async fn run_list(ctx: &CliContext) -> anyhow::Result<()> {
    fetch_and_print(ctx, "/me/joinedTeams").await
}

pub async fn run_channels(ctx: &CliContext, team: &str) -> anyhow::Result<()> {
    validate_id("team", team)?;
    let path = format!("/teams/{team}/channels");
    fetch_and_print(ctx, &path).await
}

pub async fn run_chats(ctx: &CliContext) -> anyhow::Result<()> {
    fetch_and_print(ctx, "/me/chats").await
}

/// Reject obviously bad ids early — empty, whitespace, or anything containing
/// `/`, `\`, `?`, `#`, or a control character. Graph would also reject these,
/// but we want a usage-style error rather than a 400 from the server.
pub(crate) fn validate_id(kind: &str, id: &str) -> anyhow::Result<()> {
    if id.trim().is_empty() {
        anyhow::bail!("--{kind} must not be empty");
    }
    if id.chars().any(|c| c == '/' || c == '\\' || c == '?' || c == '#' || c.is_control()) {
        anyhow::bail!("--{kind} contains an invalid character: {id:?}");
    }
    Ok(())
}

async fn fetch_and_print(ctx: &CliContext, path: &str) -> anyhow::Result<()> {
    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);

    let value: Value = if ctx.all {
        let items = client.get_all_json(&mut account, path).await?;
        ctx.store.save(&account)?;
        Value::Array(items)
    } else {
        let v = client.get_json(&mut account, path).await?;
        ctx.store.save(&account)?;
        v
    };

    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &value, &mut stdout)?;
    Ok(())
}
