//! Chunked uploads via Graph upload sessions.
//!
//! See: <https://learn.microsoft.com/en-us/graph/api/driveitem-createuploadsession>

use std::path::Path;

use crate::auth::account::Account;
use serde_json::Value;
use tokio::io::AsyncReadExt;

use super::{GraphClient, GraphError};

/// 5 MiB. Must be a multiple of 320 KiB per Graph's requirements.
const CHUNK_SIZE: usize = 5 * 1024 * 1024;

impl GraphClient {
    /// POST to `create_session_path` to obtain an `uploadUrl`, then PUT the contents of
    /// `content` in 5 MiB chunks. Returns the final response body (typically the created
    /// item's metadata).
    ///
    /// `create_session_body` is sent as the JSON body of the create-session POST. The Graph
    /// endpoint specifies what fields it accepts (e.g., `item.conflictBehavior`).
    pub async fn upload_via_session(
        &self,
        account: &mut Account,
        create_session_path: &str,
        create_session_body: Value,
        content: &Path,
    ) -> Result<Value, GraphError> {
        // 1. Create the session.
        let session_url = format!("{}{}", self.base, create_session_path);
        let token = account
            .access_token
            .as_deref()
            .ok_or_else(|| GraphError::Api {
                status: 401,
                code: "no_token".into(),
                message: "no access token cached".into(),
            })?;
        let resp = self
            .http
            .post(&session_url)
            .bearer_auth(token)
            .json(&create_session_body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            let code = body.pointer("/error/code").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
            let message = body.pointer("/error/message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Err(GraphError::Api { status, code, message });
        }
        let session: Value = resp.json().await?;
        let upload_url = session
            .get("uploadUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GraphError::Api {
                status: 500,
                code: "no_upload_url".into(),
                message: "createUploadSession response missing uploadUrl".into(),
            })?
            .to_string();

        // 2. Stream the file in 5 MiB chunks.
        let total: u64 = tokio::fs::metadata(content).await?.len();
        let mut file = tokio::fs::File::open(content).await?;
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut offset: u64 = 0;
        let mut last_response: Option<Value> = None;
        while offset < total {
            let want = (total - offset).min(CHUNK_SIZE as u64) as usize;
            let mut read = 0usize;
            while read < want {
                let n = file.read(&mut buf[read..want]).await?;
                if n == 0 { break; }
                read += n;
            }
            let end = offset + read as u64 - 1;
            let range = format!("bytes {offset}-{end}/{total}");
            let chunk = buf[..read].to_vec();
            let resp = self
                .http
                .put(&upload_url)
                .header(reqwest::header::CONTENT_LENGTH, read)
                .header(reqwest::header::CONTENT_RANGE, &range)
                .body(chunk)
                .send()
                .await?;
            let status = resp.status();
            if !status.is_success() && status != reqwest::StatusCode::ACCEPTED {
                let body: Value = resp.json().await.unwrap_or(Value::Null);
                let code = body.pointer("/error/code").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                let message = body.pointer("/error/message").and_then(|v| v.as_str()).unwrap_or("").to_string();
                return Err(GraphError::Api { status: status.as_u16(), code, message });
            }
            last_response = resp.json().await.ok();
            offset += read as u64;
        }
        Ok(last_response.unwrap_or(Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::account::Account;
    use wiremock::matchers::{method, path as wpath};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn account_with(token: &str) -> Account {
        let mut a = Account::new("x", "common", "CID", vec!["User.Read".into()]);
        a.access_token = Some(token.into());
        a.access_token_expires_at = Some(u64::MAX);
        a.refresh_token = Some("RT".into());
        a
    }

    #[tokio::test]
    async fn small_file_uploads_in_one_chunk() {
        let graph = MockServer::start().await;
        let base = graph.uri();

        Mock::given(method("POST"))
            .and(wpath("/drive/items/root:/foo.bin:/createUploadSession"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "uploadUrl": format!("{base}/upload-session/xyz")
            })))
            .mount(&graph).await;

        Mock::given(method("PUT"))
            .and(wpath("/upload-session/xyz"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "ITEM-1",
                "name": "foo.bin",
                "size": 10
            })))
            .mount(&graph).await;

        let token_srv = MockServer::start().await;
        let endpoints = crate::auth::Endpoints {
            device_authorization: format!("{}/devicecode", token_srv.uri()).parse().unwrap(),
            token: format!("{}/token", token_srv.uri()).parse().unwrap(),
        };
        let client = GraphClient::new(graph.uri(), endpoints);
        let mut a = account_with("AT");

        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("foo.bin");
        std::fs::write(&p, b"0123456789").unwrap();

        let body = serde_json::json!({"item": {"@microsoft.graph.conflictBehavior": "rename"}});
        let resp = client
            .upload_via_session(
                &mut a,
                "/drive/items/root:/foo.bin:/createUploadSession",
                body,
                &p,
            )
            .await
            .unwrap();
        assert_eq!(resp["id"], "ITEM-1");
    }
}
