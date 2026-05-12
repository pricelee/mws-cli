use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_CLIENT_ID: &str = "14d82eec-204b-4c2f-b7e8-296a70dab67e";
pub const DEFAULT_TENANT: &str = "common";
pub const DEFAULT_SCOPES: &[&str] = &["User.Read", "offline_access", "openid", "profile"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub tenant: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub access_token: Option<String>,
    /// Unix epoch seconds.
    pub access_token_expires_at: Option<u64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
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
