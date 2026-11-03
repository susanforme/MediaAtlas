//! Native OS credential service adapter with explicit, process-lifetime-only fallback.
//! Secret values never implement Serialize or Debug and are never placed in argv.
use crate::network::SecretString;
use mediaatlas_domain::ProviderFailure;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialMode {
    System,
    SessionOnly,
}
#[derive(Clone, Default)]
pub struct CredentialStore {
    session: Arc<Mutex<BTreeMap<String, SecretString>>>,
    #[cfg(test)]
    system_unavailable: bool,
    #[cfg(test)]
    deletion_unavailable: Arc<std::sync::atomic::AtomicBool>,
}
impl CredentialStore {
    pub fn new() -> Self {
        Self::default()
    }
    fn valid(reference: &str) -> bool {
        !reference.is_empty()
            && reference.len() <= 128
            && reference
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    }
    /// System mode fails closed if the native OS credential service is unavailable.
    /// SessionOnly must be explicitly selected; it never writes configuration/files.
    fn put_sync(
        &self,
        reference: &str,
        secret: SecretString,
        mode: CredentialMode,
    ) -> Result<(), ProviderFailure> {
        if !Self::valid(reference)
            || secret.expose().is_empty()
            || secret.expose().len() > 16384
            || secret.expose().contains(['\n', '\r', '\0'])
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if mode == CredentialMode::SessionOnly {
            let mut session = self
                .session
                .lock()
                .map_err(|_| ProviderFailure::CredentialsUnavailable)?;
            if session.len() >= 128 && !session.contains_key(reference) {
                return Err(ProviderFailure::CredentialsUnavailable);
            }
            session.insert(reference.to_owned(), secret);
            return Ok(());
        }
        #[cfg(test)]
        if self.system_unavailable {
            return Err(ProviderFailure::CredentialsUnavailable);
        }
        keyring::Entry::new("com.mediaatlas.desktop", reference)
            .and_then(|entry| entry.set_password(secret.expose()))
            .map_err(|_| ProviderFailure::CredentialsUnavailable)?;
        Ok(())
    }
    fn get_sync(
        &self,
        reference: &str,
        mode: CredentialMode,
    ) -> Result<SecretString, ProviderFailure> {
        if !Self::valid(reference) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if mode == CredentialMode::SessionOnly {
            return self
                .session
                .lock()
                .map_err(|_| ProviderFailure::CredentialsUnavailable)?
                .get(reference)
                .cloned()
                .ok_or(ProviderFailure::CredentialsUnavailable);
        }
        #[cfg(test)]
        if self.system_unavailable {
            return Err(ProviderFailure::CredentialsUnavailable);
        }
        let secret = keyring::Entry::new("com.mediaatlas.desktop", reference)
            .and_then(|entry| entry.get_password())
            .map_err(|_| ProviderFailure::CredentialsUnavailable)?;
        if secret.is_empty() || secret.len() > 16384 || secret.contains(['\n', '\r', '\0']) {
            return Err(ProviderFailure::CredentialsUnavailable);
        }
        Ok(SecretString::new(secret))
    }
    pub async fn put(
        &self,
        reference: &str,
        secret: SecretString,
        mode: CredentialMode,
    ) -> Result<(), ProviderFailure> {
        if mode == CredentialMode::SessionOnly {
            return self.put_sync(reference, secret, mode);
        }
        let store = self.clone();
        let reference = reference.to_owned();
        tokio::time::timeout(std::time::Duration::from_secs(5), async move {
            let permit = credential_gate()
                .acquire_owned()
                .await
                .map_err(|_| ProviderFailure::CredentialsUnavailable)?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                store.put_sync(&reference, secret, mode)
            })
            .await
            .map_err(|_| ProviderFailure::CredentialsUnavailable)?
        })
        .await
        .map_err(|_| ProviderFailure::CredentialsUnavailable)?
    }
    pub async fn get(
        &self,
        reference: &str,
        mode: CredentialMode,
    ) -> Result<SecretString, ProviderFailure> {
        if mode == CredentialMode::SessionOnly {
            return self.get_sync(reference, mode);
        }
        let store = self.clone();
        let reference = reference.to_owned();
        tokio::time::timeout(std::time::Duration::from_secs(5), async move {
            let permit = credential_gate()
                .acquire_owned()
                .await
                .map_err(|_| ProviderFailure::CredentialsUnavailable)?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                store.get_sync(&reference, mode)
            })
            .await
            .map_err(|_| ProviderFailure::CredentialsUnavailable)?
        })
        .await
        .map_err(|_| ProviderFailure::CredentialsUnavailable)?
    }
    fn delete_sync(&self, reference: &str, mode: CredentialMode) -> Result<(), ProviderFailure> {
        #[cfg(test)]
        if self
            .deletion_unavailable
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(ProviderFailure::CredentialsUnavailable);
        }

        if !Self::valid(reference) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if mode == CredentialMode::SessionOnly {
            self.session
                .lock()
                .map_err(|_| ProviderFailure::CredentialsUnavailable)?
                .remove(reference);
            return Ok(());
        }
        #[cfg(test)]
        if self.system_unavailable {
            return Err(ProviderFailure::CredentialsUnavailable);
        }
        match keyring::Entry::new("com.mediaatlas.desktop", reference)
            .and_then(|entry| entry.delete_credential())
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(ProviderFailure::CredentialsUnavailable),
        }
    }
    pub async fn delete(
        &self,
        reference: &str,
        mode: CredentialMode,
    ) -> Result<(), ProviderFailure> {
        if mode == CredentialMode::SessionOnly {
            return self.delete_sync(reference, mode);
        }
        let store = self.clone();
        let reference = reference.to_owned();
        tokio::time::timeout(std::time::Duration::from_secs(5), async move {
            let permit = credential_gate()
                .acquire_owned()
                .await
                .map_err(|_| ProviderFailure::CredentialsUnavailable)?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                store.delete_sync(&reference, mode)
            })
            .await
            .map_err(|_| ProviderFailure::CredentialsUnavailable)?
        })
        .await
        .map_err(|_| ProviderFailure::CredentialsUnavailable)?
    }
    #[cfg(test)]
    pub(crate) fn set_deletion_unavailable_for_test(&self, unavailable: bool) {
        self.deletion_unavailable
            .store(unavailable, std::sync::atomic::Ordering::SeqCst);
    }
    #[cfg(test)]
    pub(crate) fn unavailable_system_for_test() -> Self {
        Self {
            system_unavailable: true,
            ..Default::default()
        }
    }
    pub fn clear_session(&self) -> Result<(), ProviderFailure> {
        self.session
            .lock()
            .map_err(|_| ProviderFailure::CredentialsUnavailable)?
            .clear();
        Ok(())
    }
}
// A timed-out native call keeps its permit until it actually exits; a locked
// OS service cannot create an unbounded number of detached blocking workers.
fn credential_gate() -> Arc<tokio::sync::Semaphore> {
    static GATE: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    GATE.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_fallback_is_explicit_and_never_persists() {
        let store = CredentialStore::new();
        store
            .put_sync(
                "fixture-key",
                SecretString::new("test-only-value".into()),
                CredentialMode::SessionOnly,
            )
            .unwrap();
        assert_eq!(
            store
                .get_sync("fixture-key", CredentialMode::SessionOnly)
                .unwrap()
                .expose(),
            "test-only-value"
        );
        assert!(CredentialStore::new()
            .get_sync("fixture-key", CredentialMode::SessionOnly)
            .is_err());
        assert!(!format!(
            "{:?}",
            store
                .get_sync("fixture-key", CredentialMode::SessionOnly)
                .unwrap()
        )
        .contains("test-only-value"));
        store.clear_session().unwrap();
        assert!(store
            .get_sync("fixture-key", CredentialMode::SessionOnly)
            .is_err());
    }
    #[test]
    fn rejects_reference_and_secret_injection() {
        let store = CredentialStore::new();
        assert_eq!(
            store.put_sync(
                "--help",
                SecretString::new("x\ny".into()),
                CredentialMode::SessionOnly
            ),
            Err(ProviderFailure::InvalidConfiguration)
        );
        assert_eq!(
            store.get_sync("../secret", CredentialMode::System).err(),
            Some(ProviderFailure::InvalidConfiguration)
        );
    }
}
