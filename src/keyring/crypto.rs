//! AES-256-GCM with an Argon2id-derived key.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use rand::RngCore;
use serde::{Deserialize, Serialize};

const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;
const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("argon2 key derivation failed: {0}")]
    Kdf(argon2::Error),
    #[error("aead failure")]
    Aead,
    #[error("invalid ciphertext envelope")]
    Envelope,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub fn encrypt(password: &[u8], plaintext: &[u8]) -> Result<Envelope, CryptoError> {
    let mut salt = vec![0u8; SALT_LEN];
    let mut nonce = vec![0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce);
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| CryptoError::Aead)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| CryptoError::Aead)?;
    Ok(Envelope { salt, nonce, ciphertext })
}

pub fn decrypt(password: &[u8], env: &Envelope) -> Result<Vec<u8>, CryptoError> {
    if env.salt.len() != SALT_LEN || env.nonce.len() != NONCE_LEN {
        return Err(CryptoError::Envelope);
    }
    let key = derive_key(password, &env.salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| CryptoError::Aead)?;
    cipher
        .decrypt(Nonce::from_slice(&env.nonce), env.ciphertext.as_slice())
        .map_err(|_| CryptoError::Aead)
}

fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; KEY_LEN], CryptoError> {
    let mut out = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(password, salt, &mut out)
        .map_err(CryptoError::Kdf)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let env = encrypt(b"correct horse", b"the cake is a lie").unwrap();
        let back = decrypt(b"correct horse", &env).unwrap();
        assert_eq!(back, b"the cake is a lie");
    }

    #[test]
    fn wrong_password_fails() {
        let env = encrypt(b"a", b"secret").unwrap();
        assert!(decrypt(b"b", &env).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let mut env = encrypt(b"a", b"secret").unwrap();
        env.ciphertext[0] ^= 0xff;
        assert!(decrypt(b"a", &env).is_err());
    }
}
