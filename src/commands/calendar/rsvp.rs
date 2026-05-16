//! `mws-cli calendar rsvp` — POST /me/events/{id}/{accept|decline|tentativelyAccept}.

use crate::auth::Endpoints;
use crate::cli::RsvpArgs;
use crate::commands::util::validate_id;
use crate::context::CliContext;
use crate::graph::GraphClient;
use crate::output::write;
use serde_json::{json, Value};

pub async fn run(ctx: &CliContext, args: RsvpArgs) -> anyhow::Result<()> {
    validate_id("event", &args.event)?;
    let action = match args.response.as_str() {
        "accept" => "accept",
        "decline" => "decline",
        "tentative" => "tentativelyAccept",
        other => anyhow::bail!("invalid --response {other:?} (expected accept|decline|tentative)"),
    };
    let path = format!("/me/events/{}/{action}", args.event);
    let body = build_rsvp_body(args.comment.as_deref(), args.no_reply);

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

pub fn build_rsvp_body(comment: Option<&str>, no_reply: bool) -> Value {
    let mut body = json!({});
    if let Some(c) = comment {
        body["comment"] = json!(c);
    }
    if no_reply {
        body["sendResponse"] = json!(false);
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
    fn body_with_comment_only() {
        let v = build_rsvp_body(Some("running late"), false);
        assert_eq!(v["comment"], "running late");
        assert!(v.get("sendResponse").is_none());
    }

    #[test]
    fn no_reply_suppresses_response() {
        let v = build_rsvp_body(None, true);
        assert_eq!(v["sendResponse"], false);
    }

    #[test]
    fn empty_body_when_no_args() {
        let v = build_rsvp_body(None, false);
        assert!(v.as_object().unwrap().is_empty());
    }
}
