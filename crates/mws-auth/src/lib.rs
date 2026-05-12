//! OAuth flows and account storage for mws.

pub mod account;
pub mod auth_code;
pub mod device_code;
pub mod error;
pub mod refresh;
pub mod store;
pub mod token;

pub use account::{Account, DEFAULT_CLIENT_ID, DEFAULT_SCOPES, DEFAULT_TENANT};
pub use error::AuthError;
pub use store::AccountStore;
pub use token::RedactedString;
