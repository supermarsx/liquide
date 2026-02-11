use crate::credential::{CredentialStore, StorageMode, StoredCredential};

fn make_credential(server: &str) -> StoredCredential {
    StoredCredential {
        server_address: server.to_string(),
        username: "alice".to_string(),
        encrypted_password: vec![1, 2, 3, 4],
        stored_at: 1700000000,
    }
}

#[test]
fn test_store_and_retrieve() {
    let mut store = CredentialStore::new(StorageMode::OsKeychain);
    store.store(make_credential("srv1:3389")).unwrap();
    let cred = store.retrieve("srv1:3389").unwrap();
    assert_eq!(cred.username, "alice");
}

#[test]
fn test_remove_credential() {
    let mut store = CredentialStore::new(StorageMode::OsKeychain);
    store.store(make_credential("srv1:3389")).unwrap();
    assert!(store.remove("srv1:3389"));
    assert!(store.retrieve("srv1:3389").is_none());
}

#[test]
fn test_store_fails_when_locked() {
    let mut store = CredentialStore::new(StorageMode::MasterPassword);
    store.lock();
    let result = store.store(make_credential("srv1:3389"));
    assert!(result.is_err());
}

#[test]
fn test_retrieve_returns_none_when_locked() {
    let mut store = CredentialStore::new(StorageMode::MasterPassword);
    store.store(make_credential("srv1:3389")).unwrap();
    store.lock();
    assert!(store.retrieve("srv1:3389").is_none());
}

#[test]
fn test_unlock_with_passphrase() {
    let mut store = CredentialStore::new(StorageMode::MasterPassword);
    store.store(make_credential("srv1:3389")).unwrap();
    store.lock();
    assert!(store.is_locked());

    store.unlock("my-secret").unwrap();
    assert!(!store.is_locked());
    assert!(store.retrieve("srv1:3389").is_some());
}

#[test]
fn test_unlock_empty_passphrase_fails() {
    let mut store = CredentialStore::new(StorageMode::MasterPassword);
    store.lock();
    let result = store.unlock("");
    assert!(result.is_err());
}

#[test]
fn test_clear_all() {
    let mut store = CredentialStore::new(StorageMode::OsKeychain);
    store.store(make_credential("a:3389")).unwrap();
    store.store(make_credential("b:3389")).unwrap();
    assert_eq!(store.credential_count(), 2);
    store.clear_all();
    assert_eq!(store.credential_count(), 0);
}

#[test]
fn test_credential_count() {
    let mut store = CredentialStore::new(StorageMode::OsKeychain);
    assert_eq!(store.credential_count(), 0);
    store.store(make_credential("x:3389")).unwrap();
    assert_eq!(store.credential_count(), 1);
}
