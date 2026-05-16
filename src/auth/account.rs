use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::token::RedactedString;

pub const DEFAULT_CLIENT_ID: &str = "14d82eec-204b-4c2f-b7e8-296a70dab67e";
pub const DEFAULT_TENANT: &str = "common";

/// Scopes requested by `mws-cli auth login` by default. Covers the personal-
/// productivity surface of Microsoft Graph that a user owns themselves:
/// mail, calendar, contacts, OneDrive, OneNote, To Do, Teams chat/presence.
///
/// **No `*.All` admin-consent scopes here.** Including admin-only scopes
/// would either silently fail or block all non-admin users at sign-in
/// ("needs admin approval"). Power users / admins who need them widen with
/// `mws-cli auth login --scope Sites.Read.All --scope Directory.Read.All ...`.
///
/// Adding new sugar commands? Add their delegated user-data scopes here so
/// the typical user gets a single consent prompt instead of per-command
/// re-auth.
pub const DEFAULT_SCOPES: &[&str] = &[
    // --- OIDC / identity ---
    "openid",
    "profile",
    "email",
    "offline_access",
    "User.Read",

    // --- Mail (Outlook) ---
    "Mail.ReadWrite",
    "Mail.Send",
    "MailboxSettings.ReadWrite",

    // --- Calendar ---
    "Calendars.ReadWrite",

    // --- Contacts ---
    "Contacts.ReadWrite",

    // --- Files (OneDrive personal) ---
    "Files.ReadWrite",

    // --- OneNote ---
    "Notes.ReadWrite",

    // --- Tasks (To Do) ---
    "Tasks.ReadWrite",

    // --- People ---
    "People.Read",

    // --- Teams (user-level only) ---
    "Presence.Read",
    "Chat.ReadWrite",
    "Chat.Create",
    "Team.ReadBasic.All",
    "Channel.ReadBasic.All",
    "ChannelMessage.Send",
];

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

}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token::RedactedString;

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
