#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("auth: {0}")]
    Auth(#[from] mws_auth::AuthError),
    #[error("graph {status}: {code}: {message}")]
    Api { status: u16, code: String, message: String },
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
