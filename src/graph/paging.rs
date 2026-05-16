//! Walk `@odata.nextLink` to collect all pages of a Graph collection.

use crate::auth::account::Account;
use serde_json::Value;

use super::{GraphClient, GraphError};

impl GraphClient {
    /// Read every page of a collection endpoint, returning the concatenated `value` arrays.
    ///
    /// Follows `@odata.nextLink` until the server stops emitting one. Each page is fetched
    /// with the full throttling/refresh logic of `get_json`.
    pub async fn get_all_json(&self, account: &mut Account, path: &str) -> Result<Vec<Value>, GraphError> {
        let mut out = Vec::new();
        let mut next_url: Option<String> = Some(format!("{}{}", self.base, path));
        while let Some(url) = next_url.take() {
            // `get_json` takes a path; we have a full URL. Strip the base if it matches.
            let path = url.strip_prefix(&self.base).map(str::to_string).unwrap_or(url.clone());
            let page = self.get_json(account, &path).await?;
            if let Some(arr) = page.get("value").and_then(|v| v.as_array()) {
                out.extend(arr.iter().cloned());
            } else {
                // Endpoint isn't a collection — treat the body itself as one item.
                out.push(page.clone());
                break;
            }
            next_url = page
                .get("@odata.nextLink")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        Ok(out)
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
    async fn follows_next_link_across_pages() {
        let graph = MockServer::start().await;
        let base = graph.uri();
        // page 1
        Mock::given(method("GET"))
            .and(wpath("/me/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{"id": "m1"}, {"id": "m2"}],
                "@odata.nextLink": format!("{base}/me/messages?$skiptoken=ABC")
            })))
            .up_to_n_times(1)
            .mount(&graph).await;
        // page 2 (no nextLink)
        Mock::given(method("GET"))
            .and(wpath("/me/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{"id": "m3"}]
            })))
            .mount(&graph).await;
        let token = MockServer::start().await;
        let endpoints = crate::auth::Endpoints {
            device_authorization: format!("{}/devicecode", token.uri()).parse().unwrap(),
            token: format!("{}/token", token.uri()).parse().unwrap(),
        };
        let client = GraphClient::new(graph.uri(), endpoints);
        let mut a = account_with("AT");
        let all = client.get_all_json(&mut a, "/me/messages").await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0]["id"], "m1");
        assert_eq!(all[2]["id"], "m3");
    }
}
