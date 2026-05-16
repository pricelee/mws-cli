//! `mws-cli calendar cancel` — POST /me/events/{id}/cancel.

use crate::auth::Endpoints;
use crate::cli::CancelArgs;
use crate::commands::util::validate_id;
use crate::context::CliContext;
use crate::graph::GraphClient;
use crate::output::write;
use serde_json::{json, Value};

pub async fn run(ctx: &CliContext, args: CancelArgs) -> anyhow::Result<()> {
    validate_id("event", &args.event)?;
    let path = format!("/me/events/{}/cancel", args.event);
    let body = build_cancel_body(args.comment.as_deref());

    if ctx.dry_run {
        return dry_run_print(ctx, &path, &body);
    }

    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);
    let bytes = serde_json::to_vec(&body)?;
    let (status, resp_body, _ct) = client
        .send_request(
            &mut account,
            reqwest::Method::POST,
            &path,
            Some(bytes),
            &[("Content-Type".into(), "application/json".into())],
        )
        .await?;
    ctx.store.save(&account)?;
    if !status.is_success() {
        anyhow::bail!(
            "POST {} returned {}: {}",
            path,
            status,
            String::from_utf8_lossy(&resp_body)
        );
    }
    println!("OK");
    Ok(())
}

pub fn build_cancel_body(comment: Option<&str>) -> Value {
    let mut body = json!({});
    if let Some(c) = comment {
        // Graph's /cancel action uses capital-C "Comment". Verified per docs:
        // https://learn.microsoft.com/en-us/graph/api/event-cancel
        body["Comment"] = json!(c);
    }
    body
}

fn dry_run_print(ctx: &CliContext, path: &str, body: &Value) -> anyhow::Result<()> {
    let preview = json!({
        "dry_run": true,
        "method": "POST",
        "url": format!("{}{}", ctx.graph_base, path),
        "headers": {"Content-Type": "application/json"},
        "body": body,
    });
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &preview, &mut stdout)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_with_capital_c_comment() {
        let v = build_cancel_body(Some("rescheduling"));
        assert_eq!(v["Comment"], "rescheduling");
    }

    #[test]
    fn empty_body_when_no_comment() {
        let v = build_cancel_body(None);
        assert!(v.as_object().unwrap().is_empty());
    }
}
