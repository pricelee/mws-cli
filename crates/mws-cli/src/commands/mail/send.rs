use std::path::Path;

use base64::Engine;
use mws_auth::Endpoints;
use mws_graph::GraphClient;

use crate::cli::SendArgs;
use crate::commands::util::read_body_arg;
use crate::context::CliContext;

/// Combined attachment payload size at which we switch from inline (single sendMail)
/// to draft+upload-session+send. 3 MiB leaves headroom under Graph's 4 MiB request limit
/// after base64 expansion (~33%).
const INLINE_THRESHOLD: u64 = 3 * 1024 * 1024;

pub async fn run(ctx: &CliContext, args: SendArgs) -> anyhow::Result<()> {
    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);

    let body_text = read_body_arg(&args.body)?;
    let body_is_html = args.html || body_text.trim_start().starts_with('<');

    let total_attachment_size: u64 = args
        .attachments
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();

    if total_attachment_size <= INLINE_THRESHOLD {
        send_inline(&client, &mut account, &args, &body_text, body_is_html).await?;
    } else {
        send_via_draft(&client, &mut account, &args, &body_text, body_is_html).await?;
    }
    ctx.store.save(&account)?;
    println!("Sent.");
    Ok(())
}

async fn send_inline(
    client: &GraphClient,
    account: &mut mws_auth::Account,
    args: &SendArgs,
    body_text: &str,
    body_is_html: bool,
) -> anyhow::Result<()> {
    let mut attachments_json = Vec::new();
    for path in &args.attachments {
        let bytes = std::fs::read(path)?;
        let name = file_name(path);
        let ct = mime_guess(path);
        attachments_json.push(serde_json::json!({
            "@odata.type": "#microsoft.graph.fileAttachment",
            "name": name,
            "contentType": ct,
            "contentBytes": base64::engine::general_purpose::STANDARD.encode(&bytes),
        }));
    }
    let message = serde_json::json!({
        "message": {
            "subject": args.subject,
            "body": {
                "contentType": if body_is_html { "HTML" } else { "Text" },
                "content": body_text,
            },
            "toRecipients": args.to.iter().map(addr).collect::<Vec<_>>(),
            "ccRecipients": args.cc.iter().map(addr).collect::<Vec<_>>(),
            "bccRecipients": args.bcc.iter().map(addr).collect::<Vec<_>>(),
            "attachments": attachments_json,
        },
        "saveToSentItems": true,
    });
    let body = serde_json::to_vec(&message)?;
    let (status, resp_body, _ct) = client
        .send_request(
            account,
            reqwest::Method::POST,
            "/me/sendMail",
            Some(body),
            &[("Content-Type".into(), "application/json".into())],
        )
        .await?;
    if !status.is_success() {
        anyhow::bail!("sendMail returned {}: {}", status, String::from_utf8_lossy(&resp_body));
    }
    Ok(())
}

async fn send_via_draft(
    client: &GraphClient,
    account: &mut mws_auth::Account,
    args: &SendArgs,
    body_text: &str,
    body_is_html: bool,
) -> anyhow::Result<()> {
    // 1. Create the draft message.
    let draft = serde_json::json!({
        "subject": args.subject,
        "body": {
            "contentType": if body_is_html { "HTML" } else { "Text" },
            "content": body_text,
        },
        "toRecipients": args.to.iter().map(addr).collect::<Vec<_>>(),
        "ccRecipients": args.cc.iter().map(addr).collect::<Vec<_>>(),
        "bccRecipients": args.bcc.iter().map(addr).collect::<Vec<_>>(),
    });
    let body = serde_json::to_vec(&draft)?;
    let (status, resp_body, _) = client
        .send_request(
            account,
            reqwest::Method::POST,
            "/me/messages",
            Some(body),
            &[("Content-Type".into(), "application/json".into())],
        )
        .await?;
    if !status.is_success() {
        anyhow::bail!("create draft returned {}: {}", status, String::from_utf8_lossy(&resp_body));
    }
    let created: serde_json::Value = serde_json::from_slice(&resp_body)?;
    let id = created
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("draft response missing id"))?
        .to_string();

    // 2. Attach each file via upload session.
    for path in &args.attachments {
        let size = std::fs::metadata(path)?.len();
        let name = file_name(path);
        let ct = mime_guess(path);
        let session_path = format!("/me/messages/{id}/attachments/createUploadSession");
        let body = serde_json::json!({
            "AttachmentItem": {
                "attachmentType": "file",
                "name": name,
                "size": size,
                "contentType": ct,
            }
        });
        client
            .upload_via_session(account, &session_path, body, path)
            .await?;
    }

    // 3. Send the draft.
    let send_path = format!("/me/messages/{id}/send");
    let (status, resp_body, _) = client
        .send_request(account, reqwest::Method::POST, &send_path, None, &[])
        .await?;
    if !status.is_success() {
        anyhow::bail!("send draft returned {}: {}", status, String::from_utf8_lossy(&resp_body));
    }
    Ok(())
}

fn addr(s: &String) -> serde_json::Value {
    serde_json::json!({ "emailAddress": { "address": s } })
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("attachment")
        .to_string()
}

fn mime_guess(p: &Path) -> &'static str {
    match p.extension().and_then(|s| s.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("txt") => "text/plain",
        Some("html" | "htm") => "text/html",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
