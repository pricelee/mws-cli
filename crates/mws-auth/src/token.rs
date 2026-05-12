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

impl RedactedString {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

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
