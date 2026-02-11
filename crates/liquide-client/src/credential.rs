//! Credential storage backed by the OS keychain or a master password.

use std::collections::HashMap;
use std::fmt;

use crate::{ClientError, Result};

/// How credentials are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    OsKeychain,
    MasterPassword,
    Combined,
}

impl fmt::Display for StorageMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::OsKeychain => "OS Keychain",
            Self::MasterPassword => "Master Password",
            Self::Combined => "Combined",
        };
        f.write_str(label)
    }
}

/// A single stored credential entry.
#[derive(Debug, Clone)]
pub struct StoredCredential {
    pub server_address: String,
    pub username: String,
    pub encrypted_password: Vec<u8>,
    pub stored_at: u64,
}

/// In-memory credential store with locking support.
pub struct CredentialStore {
    mode: StorageMode,
    credentials: HashMap<String, StoredCredential>,
    locked: bool,
    auto_lock_timeout_min: u32,
}

impl CredentialStore {
    /// Create a new credential store.
    #[must_use]
    pub fn new(mode: StorageMode) -> Self {
        Self {
            mode,
            credentials: HashMap::new(),
            locked: false,
            auto_lock_timeout_min: 15,
        }
    }

    /// Store a credential. Fails if the store is locked.
    pub fn store(&mut self, credential: StoredCredential) -> Result<()> {
        if self.locked {
            return Err(ClientError::CredentialStorageError {
                detail: "credential store is locked".to_string(),
            });
        }
        self.credentials
            .insert(credential.server_address.clone(), credential);
        Ok(())
    }

    /// Retrieve a credential by server address.
    #[must_use]
    pub fn retrieve(&self, server_address: &str) -> Option<&StoredCredential> {
        if self.locked {
            return None;
        }
        self.credentials.get(server_address)
    }

    /// Remove the credential for a server address. Returns `true` if found.
    pub fn remove(&mut self, server_address: &str) -> bool {
        self.credentials.remove(server_address).is_some()
    }

    /// Whether the store is currently locked.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Lock the store, preventing reads and writes.
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Unlock the store. In a real implementation this would validate the
    /// master password. Here we accept any non-empty passphrase.
    pub fn unlock(&mut self, passphrase: &str) -> Result<()> {
        if passphrase.is_empty() {
            return Err(ClientError::CredentialStorageError {
                detail: "passphrase must not be empty".to_string(),
            });
        }
        self.locked = false;
        Ok(())
    }

    /// Remove all stored credentials.
    pub fn clear_all(&mut self) {
        self.credentials.clear();
    }

    /// Number of stored credentials.
    #[must_use]
    pub fn credential_count(&self) -> usize {
        self.credentials.len()
    }
}
