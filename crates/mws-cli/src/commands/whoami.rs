use mws_auth::Endpoints;
use mws_graph::GraphClient;
use mws_output::write;

use crate::context::CliContext;

pub async fn run(ctx: &CliContext) -> anyhow::Result<()> {
    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);
    let value = client.get_json(&mut account, "/me").await?;
    ctx.store.save(&account)?; // persist refreshed tokens
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &slim(&value), &mut stdout)?;
    Ok(())
}

fn slim(v: &serde_json::Value) -> serde_json::Value {
    let pick = |k: &str| v.get(k).cloned().unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "id": pick("id"),
        "displayName": pick("displayName"),
        "userPrincipalName": pick("userPrincipalName"),
        "mail": pick("mail"),
    })
}
