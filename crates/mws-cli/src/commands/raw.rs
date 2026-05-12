use std::io::Read;

use mws_auth::Endpoints;
use mws_graph::GraphClient;
use mws_output::write;

use crate::cli::RawArgs;
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: RawArgs) -> anyhow::Result<()> {
    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);

    let method = reqwest::Method::from_bytes(args.method.as_bytes())?;
    let body = read_body(args.body.as_deref())?;
    let headers = parse_headers(&args.headers)?;

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
