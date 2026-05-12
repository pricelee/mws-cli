//! OAuth flows and account storage for mws.

pub mod account;
pub mod auth_code;
pub mod device_code;
pub mod error;
pub mod store;

pub use account::{Account, DEFAULT_CLIENT_ID, DEFAULT_SCOPES, DEFAULT_TENANT};
pub use error::AuthError;
pub use store::AccountStore;
