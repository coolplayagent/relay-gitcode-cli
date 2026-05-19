use std::sync::OnceLock;

use anyhow::Context;
use keyring_core::{Entry, Error as KeyringError};

const SERVICE_PREFIX: &str = "gd.gitcode";

pub trait CredentialStore {
    fn get_token(&self, hostname: &str) -> anyhow::Result<Option<String>>;
    fn set_token(&self, hostname: &str, token: &str) -> anyhow::Result<()>;
    fn delete_token(&self, hostname: &str) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(hostname: &str) -> anyhow::Result<Entry> {
        static INIT: OnceLock<anyhow::Result<()>> = OnceLock::new();
        match INIT.get_or_init(|| {
            keyring::use_native_store(false)
                .or_else(|_| keyring::use_native_store(true))
                .context("failed to initialize system keyring")
        }) {
            Ok(()) => {}
            Err(error) => anyhow::bail!("{error:#}"),
        }

        Entry::new(&format!("{SERVICE_PREFIX}.{hostname}"), "token")
            .with_context(|| format!("failed to open system keyring entry for {hostname}"))
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn get_token(&self, hostname: &str) -> anyhow::Result<Option<String>> {
        if let Ok(token) = std::env::var("GITCODE_TOKEN")
            && !token.trim().is_empty()
        {
            return Ok(Some(token));
        }

        let entry = match Self::entry(hostname) {
            Ok(entry) => entry,
            Err(_) => return Ok(None),
        };
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    fn set_token(&self, hostname: &str, token: &str) -> anyhow::Result<()> {
        Self::entry(hostname)?
            .set_password(token)
            .context("failed to store token in system keyring")
    }

    fn delete_token(&self, hostname: &str) -> anyhow::Result<()> {
        let entry = Self::entry(hostname)?;
        match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(error).context("failed to delete token from system keyring"),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    #[derive(Debug, Default)]
    pub struct MemoryCredentialStore {
        tokens: Mutex<HashMap<String, String>>,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn get_token(&self, hostname: &str) -> anyhow::Result<Option<String>> {
            Ok(self.tokens.lock().unwrap().get(hostname).cloned())
        }

        fn set_token(&self, hostname: &str, token: &str) -> anyhow::Result<()> {
            self.tokens
                .lock()
                .unwrap()
                .insert(hostname.to_string(), token.to_string());
            Ok(())
        }

        fn delete_token(&self, hostname: &str) -> anyhow::Result<()> {
            self.tokens.lock().unwrap().remove(hostname);
            Ok(())
        }
    }

    #[test]
    fn memory_store_round_trips_token() {
        let store = MemoryCredentialStore::default();
        assert!(store.get_token("gitcode.com").unwrap().is_none());
        store.set_token("gitcode.com", "token").unwrap();
        assert_eq!(
            store.get_token("gitcode.com").unwrap().as_deref(),
            Some("token")
        );
        store.delete_token("gitcode.com").unwrap();
        assert!(store.get_token("gitcode.com").unwrap().is_none());
    }
}
