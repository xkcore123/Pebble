pub mod aes;
pub mod keystore;
pub mod passphrase;

use pebble_core::Result;
use zeroize::Zeroizing;

/// Service that manages encryption/decryption using a DEK from the OS keystore.
pub struct CryptoService {
    dek: Zeroizing<[u8; 32]>,
}

impl CryptoService {
    /// Initialize by loading (or creating) the DEK from the OS credential store.
    pub fn init() -> Result<Self> {
        let dek = keystore::KeyStore::get_or_create_dek()?;
        Ok(Self { dek })
    }

    /// Encrypt plaintext bytes.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        aes::encrypt(&self.dek, plaintext)
    }

    /// Decrypt ciphertext bytes.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        aes::decrypt(&self.dek, ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passphrase_encrypted_blob_round_trips() {
        let plaintext = br#"{"accounts":[{"id":"a1","password":"secret"}]}"#;

        let encrypted =
            passphrase::encrypt_with_passphrase(plaintext, "correct horse battery staple").unwrap();
        let decrypted =
            passphrase::decrypt_with_passphrase(&encrypted, "correct horse battery staple")
                .unwrap();

        assert_eq!(decrypted, plaintext);
        assert_ne!(encrypted.ciphertext_hex, String::from_utf8_lossy(plaintext));
    }

    #[test]
    fn passphrase_encrypted_blob_rejects_wrong_passphrase() {
        let encrypted = passphrase::encrypt_with_passphrase(b"secret", "right passphrase").unwrap();

        let err = passphrase::decrypt_with_passphrase(&encrypted, "wrong passphrase")
            .unwrap_err()
            .to_string();

        assert!(err.contains("Decryption failed"));
    }

    #[test]
    fn passphrase_encrypted_blob_rejects_empty_passphrase() {
        let err = passphrase::encrypt_with_passphrase(b"secret", "")
            .unwrap_err()
            .to_string();

        assert!(err.contains("passphrase is required"));
    }

    #[test]
    fn passphrase_encrypted_blob_rejects_unsupported_iterations_before_decrypting() {
        let mut encrypted =
            passphrase::encrypt_with_passphrase(b"secret", "right passphrase").unwrap();
        encrypted.iterations = 1;

        let err = passphrase::decrypt_with_passphrase(&encrypted, "right passphrase")
            .unwrap_err()
            .to_string();

        assert!(err.contains("Unsupported backup secret KDF iterations"));
    }

    #[test]
    #[ignore] // Requires OS credential store access
    fn test_crypto_service_init() {
        let service = CryptoService::init();
        assert!(service.is_ok());
    }

    #[test]
    #[ignore] // Requires OS credential store access
    fn test_crypto_service_round_trip() {
        let service = CryptoService::init().unwrap();
        let plaintext = b"test credentials json";
        let encrypted = service.encrypt(plaintext).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
