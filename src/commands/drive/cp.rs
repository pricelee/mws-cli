use std::path::PathBuf;

use crate::auth::Endpoints;
use crate::graph::GraphClient;

use crate::cli::CpArgs;
use crate::context::CliContext;

const UPLOAD_SESSION_THRESHOLD: u64 = 4 * 1024 * 1024;

enum Endpoint {
    Local(PathBuf),
    Remote(String),
}

fn parse(s: &str) -> Endpoint {
    if let Some(rest) = s.strip_prefix("mws:") {
        let clean = rest.strip_prefix('/').unwrap_or(rest);
        Endpoint::Remote(clean.to_string())
    } else {
        Endpoint::Local(PathBuf::from(s))
    }
}

pub async fn run(ctx: &CliContext, args: CpArgs) -> anyhow::Result<()> {
    let src = parse(&args.src);
    let dst = parse(&args.dst);

    match (src, dst) {
        (Endpoint::Local(local), Endpoint::Remote(remote)) => {
            let mut account = ctx.store.load(&ctx.account_name)?;
            let endpoints = Endpoints::for_tenant(&account.tenant);
            let client = GraphClient::new(ctx.graph_base.clone(), endpoints);
            upload(&client, &mut account, &local, &remote).await?;
            ctx.store.save(&account)?;
            println!("Uploaded {} to mws:/{remote}", local.display());
            Ok(())
        }
        (Endpoint::Remote(_), Endpoint::Local(_)) => {
            anyhow::bail!("download (mws:/... -> local) is M2; M1 supports upload only")
        }
        (Endpoint::Local(_), Endpoint::Local(_)) => {
            anyhow::bail!("both paths are local; use the OS's `cp` for that")
        }
        (Endpoint::Remote(_), Endpoint::Remote(_)) => {
            anyhow::bail!("remote-to-remote copy is M2")
        }
    }
}

async fn upload(
    client: &GraphClient,
    account: &mut crate::auth::Account,
    local: &std::path::Path,
    remote: &str,
) -> anyhow::Result<()> {
    let size = std::fs::metadata(local)?.len();
    let encoded = percent_encode(remote);
    if size < UPLOAD_SESSION_THRESHOLD {
        let body = std::fs::read(local)?;
        let path = format!("/me/drive/root:/{encoded}:/content");
        let (status, resp_body, _) = client
            .send_request(
                account,
                reqwest::Method::PUT,
                &path,
                Some(body),
                &[("Content-Type".into(), "application/octet-stream".into())],
            )
            .await?;
        if !status.is_success() {
            anyhow::bail!("PUT returned {}: {}", status, String::from_utf8_lossy(&resp_body));
        }
    } else {
        let session_path = format!("/me/drive/root:/{encoded}:/createUploadSession");
        let body = serde_json::json!({
            "item": {"@microsoft.graph.conflictBehavior": "rename"}
        });
        client
            .upload_via_session(account, &session_path, body, local)
            .await?;
    }
    Ok(())
}

/// RFC 3986 percent-encode of path segments, preserving `/`.
fn percent_encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for ch in path.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | '/' => out.push(ch),
            _ => {
                let mut buf = [0u8; 4];
                for b in ch.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_keeps_slashes() {
        assert_eq!(percent_encode("a/b/c.txt"), "a/b/c.txt");
        assert_eq!(percent_encode("a b/c.txt"), "a%20b/c.txt");
        assert_eq!(percent_encode("файл"), "%D1%84%D0%B0%D0%B9%D0%BB");
    }
}
