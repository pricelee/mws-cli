use std::io::Read;

use crate::auth::Endpoints;
use crate::graph::GraphClient;
use crate::output::write;

use crate::cli::RawArgs;
use crate::context::CliContext;
use crate::safety;

pub async fn run(ctx: &CliContext, args: RawArgs) -> anyhow::Result<()> {
    let method_str = args.method.to_ascii_uppercase();
    let method = reqwest::Method::from_bytes(method_str.as_bytes())?;
    let body = read_body(args.body.as_deref())?;
    let headers = parse_headers(&args.headers)?;

    if ctx.dry_run {
        return dry_run_print(ctx, &method_str, &args.path, body.as_deref(), &headers);
    }

    safety::gate(&method_str, &args.path, ctx.yes)?;

    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);

    if ctx.all && method == reqwest::Method::GET {
        let items = client.get_all_json(&mut account, &args.path).await?;
        ctx.store.save(&account)?;
        let mut stdout = std::io::stdout().lock();
        write(ctx.format, &items, &mut stdout)?;
        return Ok(());
    }

    let (status, body_bytes, content_type) = client
        .send_request(&mut account, method, &args.path, body, &headers)
        .await?;
    ctx.store.save(&account)?; // persist refreshed tokens

    if !status.is_success() {
        anyhow::bail!("graph {} returned {}: {}", args.path, status, String::from_utf8_lossy(&body_bytes));
    }

    let is_json = content_type
        .as_deref()
        .map(|ct| ct.contains("application/json"))
        .unwrap_or(false);

    if is_json {
        let value: serde_json::Value = serde_json::from_slice(&body_bytes)?;
        let mut stdout = std::io::stdout().lock();
        write(ctx.format, &value, &mut stdout)?;
    } else {
        use std::io::Write;
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&body_bytes)?;
    }
    Ok(())
}

fn read_body(spec: Option<&str>) -> anyhow::Result<Option<Vec<u8>>> {
    match spec {
        None => Ok(None),
        Some("-") => {
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf)?;
            Ok(Some(buf))
        }
        Some(s) if s.starts_with('@') => {
            let path = &s[1..];
            Ok(Some(std::fs::read(path)?))
        }
        Some(s) => Ok(Some(s.as_bytes().to_vec())),
    }
}

fn parse_headers(raw: &[String]) -> anyhow::Result<Vec<(String, String)>> {
    let mut out = Vec::with_capacity(raw.len());
    for h in raw {
        let (k, v) = h
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invalid header (expected key:value): {h}"))?;
        out.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok(out)
}

/// Print the prepared request as JSON instead of dispatching it. The Authorization
/// header isn't shown — at dry-run time no token has been requested yet, and we
/// wouldn't want to print one even if we had it.
fn dry_run_print(
    ctx: &CliContext,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    headers: &[(String, String)],
) -> anyhow::Result<()> {
    let body_preview = body.map(|b| match std::str::from_utf8(b) {
        Ok(s) => serde_json::Value::String(s.to_string()),
        Err(_) => serde_json::json!({ "binary_bytes": b.len() }),
    });
    let header_map: serde_json::Map<String, serde_json::Value> = headers
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    let value = serde_json::json!({
        "dry_run": true,
        "method": method,
        "url": format!("{}{}", ctx.graph_base, path),
        "headers": header_map,
        "body": body_preview,
        "destructive": safety::is_destructive(method, path),
        "would_paginate": ctx.all && method == "GET",
    });
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &value, &mut stdout)?;
    Ok(())
}
