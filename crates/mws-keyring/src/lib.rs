//! AES-256-GCM at-rest credential storage over the OS keyring.

mod crypto;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use crypto::{CryptoError, Envelope};

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Stores password-encrypted opaque blobs, keyed by name.
/// The encryption password is stored in the OS keyring.
pub trait Vault: Send + Sync {
    fn put(&self, name: &str, plaintext: &[u8]) -> Result<(), VaultError>;
    fn get(&self, name: &str) -> Result<Vec<u8>, VaultError>;
    fn delete(&self, name: &str) -> Result<(), VaultError>;
}

/// In-memory implementation for tests.
#[derive(Default, Clone)]
pub struct MemoryVault {
    inner: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl Vault for MemoryVault {
    fn put(&self, name: &str, plaintext: &[u8]) -> Result<(), VaultError> {
        // Round-trip through crypto so tests exercise the same path.
        let env = crypto::encrypt(b"mem-test-password", plaintext)?;
        let bytes = serde_json::to_vec(&env)?;
        self.inner.lock().unwrap().insert(name.to_string(), bytes);
        Ok(())
    }
    fn get(&self, name: &str) -> Result<Vec<u8>, VaultError> {
        let map = self.inner.lock().unwrap();
        let bytes = map.get(name).ok_or_else(|| VaultError::NotFound(name.to_string()))?;
        let env: Envelope = serde_json::from_slice(bytes)?;
        Ok(crypto::decrypt(b"mem-test-password", &env)?)
    }
    fn delete(&self, name: &str) -> Result<(), VaultError> {
        self.inner.lock().unwrap().remove(name);
        Ok(())
    }
}

/// Disk-backed vault: encrypted blobs in `dir`, encryption key in the OS keyring.
pub struct DiskVault {
    dir: std::path::PathBuf,
    service: String,
}

impl DiskVault {
    pub fn new(dir: impl Into<std::path::PathBuf>, service: impl Into<String>) -> Self {
        Self { dir: dir.into(), service: service.into() }
    }

    fn key_entry(&self) -> Result<keyring::Entry, VaultError> {
        keyring::Entry::new(&self.service, "vault-key").map_err(|e| VaultError::Keyring(e.to_string()))
    }

    fn password(&self) -> Result<Vec<u8>, VaultError> {
        let entry = self.key_entry()?;
        match entry.get_password() {
            Ok(s) => Ok(s.into_bytes()),
            Err(keyring::Error::NoEntry) => {
                let mut bytes = [0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
                let pw = hex::encode(bytes);
                entry.set_password(&pw).map_err(|e| VaultError::Keyring(e.to_string()))?;
                Ok(pw.into_bytes())
            }
            Err(e) => Err(VaultError::Keyring(e.to_string())),
        }
    }

    fn path_for(&self, name: &str) -> std::path::PathBuf {
        self.dir.join(format!("{name}.bin"))
    }
}

impl Vault for DiskVault {
    fn put(&self, name: &str, plaintext: &[u8]) -> Result<(), VaultError> {
        std::fs::create_dir_all(&self.dir).map_err(|e| VaultError::Keyring(e.to_string()))?;
        let pw = self.password()?;
        let env = crypto::encrypt(&pw, plaintext)?;
        let bytes = serde_json::to_vec(&env)?;
        std::fs::write(self.path_for(name), bytes).map_err(|e| VaultError::Keyring(e.to_string()))?;
        Ok(())
    }
    fn get(&self, name: &str) -> Result<Vec<u8>, VaultError> {
        let bytes = std::fs::read(self.path_for(name)).map_err(|_| VaultError::NotFound(name.to_string()))?;
        let env: Envelope = serde_json::from_slice(&bytes)?;
        let pw = self.password()?;
        Ok(crypto::decrypt(&pw, &env)?)
    }
    fn delete(&self, name: &str) -> Result<(), VaultError> {
        let p = self.path_for(name);
        if p.exists() {
            std::fs::remove_file(p).map_err(|e| VaultError::Keyring(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_vault_round_trip() {
        let v = MemoryVault::default();
        v.put("alice", b"hello").unwrap();
        assert_eq!(v.get("alice").unwrap(), b"hello");
        v.delete("alice").unwrap();
        assert!(matches!(v.get("alice"), Err(VaultError::NotFound(_))));
    }
}
