#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("vault: {0}")]
    Vault(#[from] mws_keyring::VaultError),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("oauth error: {error}: {description}")]
    OAuth { error: String, description: String },
    #[error("token expired and no refresh token is available")]
    NoRefreshToken,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("flow timeout")]
    Timeout,
    #[error("flow cancelled")]
    Cancelled,
    #[error("invalid state: {0}")]
    State(String),
}
