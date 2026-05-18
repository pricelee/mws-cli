//! OAuth token types and supporting wrappers shared across auth flows.

use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};
use url::Url;

/// A `String` that masks itself in `Debug` output, transparent to serde.
///
/// Use this for OAuth tokens so they don't accidentally leak through
/// `tracing::debug!("{:?}", account)` or similar.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedString(String);

impl fmt::Debug for RedactedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("REDACTED")
    }
}

impl Deref for RedactedString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<String> for RedactedString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for RedactedString {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl Serialize for RedactedString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for RedactedString {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self)
    }
}

/// Endpoints the device-code flow talks to. Public so tests can stub them.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub device_authorization: Url,
    pub token: Url,
}

impl Endpoints {
    pub fn for_tenant(tenant: &str) -> Self {
        let base = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0");
        Self {
            device_authorization: format!("{base}/devicecode").parse().expect("valid url"),
            token: format!("{base}/token").parse().expect("valid url"),
        }
    }
}

#[derive(Debug)]
pub struct TokenGrant {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in: u64,
}

/// Parse a JWT id_token's claims payload (the middle segment), without
/// verifying the signature. We only read values Microsoft already
/// authenticated for us at token-exchange time.
pub fn extract_claims(id_token: &str) -> Option<serde_json::Value> {
    use base64::Engine;
    let mid = id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(mid)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Extract the `tid` (tenant id) claim from a JWT id_token.
pub fn extract_tid(id_token: &str) -> Option<String> {
    extract_claims(id_token)?
        .get("tid")?
        .as_str()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::{extract_claims, extract_tid};
    use base64::Engine;

    fn jwt_with(claims: serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let payload_b64 =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        format!("{header}.{payload_b64}.signature")
    }

    #[test]
    fn extracts_tid_from_well_formed_jwt() {
        let token = jwt_with(serde_json::json!({
            "tid": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "name": "Alice"
        }));
        assert_eq!(
            extract_tid(&token).as_deref(),
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890")
        );
    }

    #[test]
    fn extracts_arbitrary_claims() {
        let token = jwt_with(serde_json::json!({
            "name": "Lee Junho",
            "preferred_username": "pricelee@contoso.com",
            "tid": "T"
        }));
        let claims = extract_claims(&token).unwrap();
        assert_eq!(claims["name"], "Lee Junho");
        assert_eq!(claims["preferred_username"], "pricelee@contoso.com");
    }

    #[test]
    fn returns_none_for_garbage() {
        assert!(extract_tid("not-a-jwt").is_none());
        assert!(extract_tid("only.one").is_none());
        assert!(extract_tid("a.b.c").is_none()); // 'b' is invalid base64 / not JSON
    }
}
