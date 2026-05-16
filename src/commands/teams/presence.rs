//! `mws-cli teams presence` — GET /me/presence.

use crate::auth::Endpoints;
use crate::graph::GraphClient;
use crate::output::write;

use crate::context::CliContext;

pub async fn run(ctx: &CliContext) -> anyhow::Result<()> {
    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);
    let value = client.get_json(&mut account, "/me/presence").await?;
    ctx.store.save(&account)?;
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &slim(&value), &mut stdout)?;
    Ok(())
}

fn slim(v: &serde_json::Value) -> serde_json::Value {
    let pick = |k: &str| v.get(k).cloned().unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "id": pick("id"),
        "availability": pick("availability"),
        "activity": pick("activity"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slim_picks_three_fields() {
        let raw = serde_json::json!({
            "id": "u",
            "availability": "Available",
            "activity": "Available",
            "outOfOfficeSettings": {"isOutOfOffice": false},
            "extra": "noise"
        });
        let s = slim(&raw);
        assert_eq!(s["availability"], "Available");
        assert!(s.get("extra").is_none());
    }
}
