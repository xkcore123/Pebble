use pebble_core::{PebbleError, Result};
use rand::RngCore;
use tracing::{info, warn};
use zeroize::Zeroizing;

const SERVICE_NAME: &str = "com.pebble.email";
const KEY_ENTRY: &str = "master-dek";

pub struct KeyStore;

trait DekCredential {
    fn get_secret(&self) -> std::result::Result<Vec<u8>, keyring::Error>;
    fn set_secret(&self, secret: &[u8]) -> std::result::Result<(), keyring::Error>;
    fn delete_credential(&self) -> std::result::Result<(), keyring::Error>;
}

impl DekCredential for keyring::Entry {
    fn get_secret(&self) -> std::result::Result<Vec<u8>, keyring::Error> {
        keyring::Entry::get_secret(self)
    }

    fn set_secret(&self, secret: &[u8]) -> std::result::Result<(), keyring::Error> {
        keyring::Entry::set_secret(self, secret)
    }

    fn delete_credential(&self) -> std::result::Result<(), keyring::Error> {
        keyring::Entry::delete_credential(self)
    }
}

impl KeyStore {
    /// Get or create the Data Encryption Key from the OS credential store.
    pub fn get_or_create_dek() -> Result<Zeroizing<[u8; 32]>> {
        let entry = keyring::Entry::new(SERVICE_NAME, KEY_ENTRY)
            .map_err(|e| PebbleError::Auth(format!("Keyring entry error: {e}")))?;

        get_or_create_dek_from_credential(&entry)
    }

    /// Delete the DEK from the OS credential store.
    pub fn delete_dek() -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, KEY_ENTRY)
            .map_err(|e| PebbleError::Auth(format!("Keyring entry error: {e}")))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already gone
            Err(e) => Err(PebbleError::Auth(format!("Failed to delete DEK: {e}"))),
        }
    }
}

fn get_or_create_dek_from_credential(
    credential: &impl DekCredential,
) -> Result<Zeroizing<[u8; 32]>> {
    match credential.get_secret() {
        Ok(secret) => {
            let secret = Zeroizing::new(secret);
            if secret.len() == 32 {
                let mut key = Zeroizing::new([0u8; 32]);
                key.copy_from_slice(&secret);
                return Ok(key);
            }

            warn!(
                "Stored DEK has invalid length: expected 32, got {}; replacing it",
                secret.len()
            );
            replace_dek(credential)
        }
        Err(keyring::Error::NoEntry) => {
            info!("No DEK found, generating new one");
            generate_and_store_dek(credential)
        }
        Err(e) => Err(PebbleError::Auth(format!("Keyring read error: {e}"))),
    }
}

fn replace_dek(credential: &impl DekCredential) -> Result<Zeroizing<[u8; 32]>> {
    match credential.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => generate_and_store_dek(credential),
        Err(e) => Err(PebbleError::Auth(format!(
            "Failed to delete invalid DEK: {e}"
        ))),
    }
}

fn generate_and_store_dek(credential: &impl DekCredential) -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0u8; 32]);
    rand::thread_rng().fill_bytes(&mut *key);
    credential
        .set_secret(&*key)
        .map_err(|e| PebbleError::Auth(format!("Failed to store DEK: {e}")))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct FakeCredential {
        secret: RefCell<Option<Vec<u8>>>,
        deletes: Cell<usize>,
        writes: Cell<usize>,
    }

    impl FakeCredential {
        fn with_secret(secret: Vec<u8>) -> Self {
            Self {
                secret: RefCell::new(Some(secret)),
                deletes: Cell::new(0),
                writes: Cell::new(0),
            }
        }
    }

    impl DekCredential for FakeCredential {
        fn get_secret(&self) -> std::result::Result<Vec<u8>, keyring::Error> {
            self.secret.borrow().clone().ok_or(keyring::Error::NoEntry)
        }

        fn set_secret(&self, secret: &[u8]) -> std::result::Result<(), keyring::Error> {
            self.writes.set(self.writes.get() + 1);
            self.secret.borrow_mut().replace(secret.to_vec());
            Ok(())
        }

        fn delete_credential(&self) -> std::result::Result<(), keyring::Error> {
            self.deletes.set(self.deletes.get() + 1);
            self.secret.borrow_mut().take();
            Ok(())
        }
    }

    #[test]
    fn invalid_stored_dek_is_replaced_with_new_32_byte_dek() {
        let credential = FakeCredential::with_secret(vec![7u8; 50]);

        let dek = get_or_create_dek_from_credential(&credential).unwrap();

        assert_eq!(dek.len(), 32);
        assert_eq!(credential.secret.borrow().as_ref().unwrap().len(), 32);
        assert_eq!(credential.deletes.get(), 1);
        assert_eq!(credential.writes.get(), 1);
    }
}
