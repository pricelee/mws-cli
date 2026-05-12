use std::sync::Arc;

use mws_keyring::Vault;

use crate::account::Account;
use crate::error::AuthError;

/// Persists Accounts via a `Vault`. Each account is stored under its `name`.
pub struct AccountStore {
    vault: Arc<dyn Vault>,
}

impl AccountStore {
    pub fn new(vault: Arc<dyn Vault>) -> Self {
        Self { vault }
    }

    pub fn save(&self, account: &Account) -> Result<(), AuthError> {
        let bytes = serde_json::to_vec(account)?;
        self.vault.put(&account.name, &bytes)?;
        Ok(())
    }

    pub fn load(&self, name: &str) -> Result<Account, AuthError> {
        let bytes = self.vault.get(name)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn delete(&self, name: &str) -> Result<(), AuthError> {
        self.vault.delete(name)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{Account, DEFAULT_CLIENT_ID, DEFAULT_SCOPES, DEFAULT_TENANT};
    use mws_keyring::MemoryVault;

    fn store() -> AccountStore {
        AccountStore::new(Arc::new(MemoryVault::default()))
    }

    #[test]
    fn save_load_round_trip() {
        let s = store();
        let mut a = Account::new("default", DEFAULT_TENANT, DEFAULT_CLIENT_ID, DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect());
        a.access_token = Some("AT".into());
        a.refresh_token = Some("RT".into());
        s.save(&a).unwrap();
        let b = s.load("default").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn delete_removes() {
        let s = store();
        let a = Account::new("x", DEFAULT_TENANT, DEFAULT_CLIENT_ID, vec![]);
        s.save(&a).unwrap();
        s.delete("x").unwrap();
        assert!(s.load("x").is_err());
    }
}
