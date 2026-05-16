//! `mws-cli calendar events` — GET /me/calendarView with a time window.

use crate::auth::Endpoints;
use crate::cli::EventsArgs;
use crate::context::CliContext;
use crate::graph::GraphClient;
use crate::output::write;
use serde_json::Value;

use super::datetime::{default_window, graph_dt, parse_rfc3339_utc};

pub async fn run(ctx: &CliContext, args: EventsArgs) -> anyhow::Result<()> {
    let (start, end) = resolve_window(args.start.as_deref(), args.end.as_deref())?;
    let mut path = format!("/me/calendarView?startDateTime={start}&endDateTime={end}");
    if let Some(top) = args.top {
        path.push_str(&format!("&$top={top}"));
    }

    if ctx.dry_run {
        return dry_run_print(ctx, &path);
    }

    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);

    let value: Value = if ctx.all {
        let items = client.get_all_json(&mut account, &path).await?;
        ctx.store.save(&account)?;
        Value::Array(items)
    } else {
        let v = client.get_json(&mut account, &path).await?;
        ctx.store.save(&account)?;
        v
    };

    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &value, &mut stdout)?;
    Ok(())
}

fn resolve_window(
    start: Option<&str>,
    end: Option<&str>,
) -> anyhow::Result<(String, String)> {
    match (start, end) {
        (Some(s), Some(e)) => {
            let s_utc = parse_rfc3339_utc(s)?;
            let e_utc = parse_rfc3339_utc(e)?;
            Ok((graph_dt(s_utc), graph_dt(e_utc)))
        }
        (Some(s), None) => {
            let s_utc = parse_rfc3339_utc(s)?;
            let e_utc = s_utc + chrono::Duration::days(7);
            Ok((graph_dt(s_utc), graph_dt(e_utc)))
        }
        (None, Some(_)) => {
            anyhow::bail!("--end requires --start (otherwise defaults are paired)");
        }
        (None, None) => {
            let (s, e) = default_window();
            Ok((graph_dt(s), graph_dt(e)))
        }
    }
}

fn dry_run_print(ctx: &CliContext, path: &str) -> anyhow::Result<()> {
    let value = serde_json::json!({
        "dry_run": true,
        "method": "GET",
        "url": format!("{}{}", ctx.graph_base, path),
    });
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &value, &mut stdout)?;
    Ok(())
}
