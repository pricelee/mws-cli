//! `mws teams post` and `mws teams chat post` — message-posting sugar.

use crate::auth::Endpoints;
use crate::graph::GraphClient;
use crate::output::write;
use serde_json::Value;

use crate::commands::teams::list::validate_id;
use crate::commands::util::read_body_arg;
use crate::context::CliContext;

/// JSON body for a Teams message post.
pub fn build_message_body(message: &str, html: bool) -> Value {
    serde_json::json!({
        "body": {
            "content": message,
            "contentType": if html { "html" } else { "text" },
        }
    })
}

pub async fn run_channel_post(
    ctx: &CliContext,
    team: &str,
    channel: &str,
    message: &str,
    html: bool,
) -> anyhow::Result<()> {
    validate_id("team", team)?;
    validate_id("channel", channel)?;
    let content = read_body_arg(message)?;
    let body = build_message_body(&content, html);
    let path = format!("/teams/{team}/channels/{channel}/messages");
    post_message(ctx, &path, body).await
}

pub async fn run_chat_post(
    ctx: &CliContext,
    chat: &str,
    message: &str,
    html: bool,
) -> anyhow::Result<()> {
    validate_id("chat", chat)?;
    let content = read_body_arg(message)?;
    let body = build_message_body(&content, html);
    let path = format!("/chats/{chat}/messages");
    post_message(ctx, &path, body).await
}

async fn post_message(ctx: &CliContext, path: &str, body: Value) -> anyhow::Result<()> {
    if ctx.dry_run {
        let preview = serde_json::json!({
            "dry_run": true,
            "method": "POST",
            "url": format!("{}{}", ctx.graph_base, path),
            "headers": {"Content-Type": "application/json"},
            "body": body,
        });
        let mut stdout = std::io::stdout().lock();
        write(ctx.format, &preview, &mut stdout)?;
        return Ok(());
    }

    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);
    let body_bytes = serde_json::to_vec(&body)?;
    let (status, resp_body, _ct) = client
        .send_request(
            &mut account,
            reqwest::Method::POST,
            path,
            Some(body_bytes),
            &[("Content-Type".into(), "application/json".into())],
        )
        .await?;
    ctx.store.save(&account)?;

    if !status.is_success() {
        anyhow::bail!(
            "post to {} returned {}: {}",
            path,
            status,
            String::from_utf8_lossy(&resp_body)
        );
    }
    let value: Value = serde_json::from_slice(&resp_body)?;
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &value, &mut stdout)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_defaults_to_text() {
        let v = build_message_body("hi", false);
        assert_eq!(v["body"]["contentType"], "text");
        assert_eq!(v["body"]["content"], "hi");
    }

    #[test]
    fn body_html_flag_switches_type() {
        let v = build_message_body("<b>hi</b>", true);
        assert_eq!(v["body"]["contentType"], "html");
    }
}
