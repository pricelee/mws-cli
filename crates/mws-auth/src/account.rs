use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::token::RedactedString;

pub const DEFAULT_CLIENT_ID: &str = "14d82eec-204b-4c2f-b7e8-296a70dab67e";
pub const DEFAULT_TENANT: &str = "common";
pub const DEFAULT_SCOPES: &[&str] = &["User.Read", "offline_access", "openid", "profile"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub tenant: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub access_token: Option<RedactedString>,
    /// Unix epoch seconds.
    pub access_token_expires_at: Option<u64>,
    pub refresh_token: Option<RedactedString>,
    pub id_token: Option<RedactedString>,
    /// Optional cached display info (filled by `whoami`).
    pub username: Option<String>,
}

impl Account {
    pub fn new(name: impl Into<String>, tenant: impl Into<String>, client_id: impl Into<String>, scopes: Vec<String>) -> Self {
        Self {
            name: name.into(),
            tenant: tenant.into(),
            client_id: client_id.into(),
            scopes,
            access_token: None,
            access_token_expires_at: None,
            refresh_token: None,
            id_token: None,
            username: None,
        }
    }

    pub fn access_token_valid(&self, skew_secs: u64) -> bool {
        match (self.access_token.as_ref(), self.access_token_expires_at) {
            (Some(_), Some(exp)) => now_secs() + skew_secs < exp,
            _ => false,
        }
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::RedactedString;

    #[test]
    fn account_debug_does_not_leak_tokens() {
        let mut a = Account::new("x", DEFAULT_TENANT, DEFAULT_CLIENT_ID, vec![]);
        a.access_token = Some(RedactedString::from("SECRET-AT"));
        a.refresh_token = Some(RedactedString::from("SECRET-RT"));
        a.id_token = Some(RedactedString::from("SECRET-ID"));
        let s = format!("{a:?}");
        assert!(!s.contains("SECRET-AT"), "access_token leaked: {s}");
        assert!(!s.contains("SECRET-RT"), "refresh_token leaked: {s}");
        assert!(!s.contains("SECRET-ID"), "id_token leaked: {s}");
        assert!(s.contains("REDACTED"));
    }

    #[test]
    fn redacted_string_is_serde_transparent() {
        let r = RedactedString::from("hello");
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, "\"hello\"");
        let back: RedactedString = serde_json::from_str(&j).unwrap();
        assert_eq!(&*back, "hello");
    }
}
