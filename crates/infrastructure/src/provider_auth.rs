//! Secret-free auth settings plus explicit OS/session credential storage.
use crate::{
    credentials::{CredentialMode, CredentialStore},
    library::{LibraryRepository, RepositoryError},
    network::SecretString,
};
use mediaatlas_domain::ProviderFailure;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Inbound-only. Deliberately neither Debug nor Serialize.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthSaveInput {
    pub provider_id: String,
    pub method: String,
    #[serde(default = "default_storage")]
    pub storage: String,
    pub secret: String,
}
fn default_storage() -> String {
    "system".into()
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthStatus {
    pub provider_id: String,
    pub supported_methods: Vec<String>,
    pub method: String,
    pub storage: String,
    pub configured: bool,
    pub status: String,
}
#[derive(Clone)]
pub struct ProviderAuthService {
    repo: LibraryRepository,
    credentials: Arc<CredentialStore>,
    gate: Arc<Mutex<()>>,
}
struct AuthConfig {
    method: String,
    storage: String,
    reference: String,
}
pub(crate) struct ResolvedAuth {
    pub method: &'static str,
    pub secret: SecretString,
}
impl ProviderAuthService {
    pub fn new(repo: LibraryRepository, credentials: Arc<CredentialStore>) -> Self {
        Self {
            repo,
            credentials,
            gate: Arc::default(),
        }
    }
    async fn config(&self, provider: &str) -> Result<Option<AuthConfig>, RepositoryError> {
        let row = sqlx::query(
            "SELECT method,storage,credential_ref FROM provider_auth_settings WHERE provider_id=?",
        )
        .bind(provider)
        .fetch_optional(self.repo.pool())
        .await?;
        row.map(|r| {
            Ok(AuthConfig {
                method: r.try_get("method")?,
                storage: r.try_get("storage")?,
                reference: r.try_get("credential_ref")?,
            })
        })
        .transpose()
    }
    pub async fn status(&self, provider: &str) -> Result<AuthStatus, RepositoryError> {
        let _gate = self.gate.lock().await;
        self.status_inner(provider).await
    }
    async fn status_inner(&self, provider: &str) -> Result<AuthStatus, RepositoryError> {
        validate_provider(provider)?;
        if !matches!(provider, "tmdb" | "dmm") {
            return Ok(AuthStatus {
                provider_id: provider.into(),
                supported_methods: vec!["none".into()],
                method: "none".into(),
                storage: "system".into(),
                configured: true,
                status: "not_required".into(),
            });
        }
        let cleanup_pending = self.retry_retirements(provider).await?;
        let config = self.config(provider).await?;
        let (method, storage, configured, status) = if let Some(config) = config {
            let mode = storage_mode(&config.storage)?;
            let available = self.credentials.get(&config.reference, mode).await.is_ok();
            (
                config.method,
                config.storage,
                available,
                if available {
                    "configured"
                } else if mode == CredentialMode::System {
                    "credentials_unavailable"
                } else {
                    "missing_credentials"
                },
            )
        } else {
            (
                if provider == "dmm" {
                    "affiliate"
                } else {
                    "bearer"
                }
                .into(),
                "system".into(),
                false,
                "not_configured",
            )
        };
        Ok(AuthStatus {
            provider_id: provider.into(),
            supported_methods: if provider == "dmm" {
                vec!["affiliate".into()]
            } else {
                vec!["bearer".into(), "api_key".into()]
            },
            method,
            storage,
            configured,
            status: if cleanup_pending {
                "cleanup_pending"
            } else {
                status
            }
            .into(),
        })
    }
    pub async fn save(&self, input: AuthSaveInput) -> Result<AuthStatus, RepositoryError> {
        let _gate = self.gate.lock().await;
        validate_provider(&input.provider_id)?;
        let valid = match (input.provider_id.as_str(), input.method.as_str()) {
            ("tmdb", "bearer" | "api_key") => {
                !input.secret.is_empty()
                    && input.secret.len() <= 16384
                    && !input.secret.chars().any(char::is_whitespace)
                    && !input.secret.chars().any(char::is_control)
            }
            ("dmm", "affiliate") => parse_affiliate_credential(&input.secret).is_ok(),
            _ => false,
        };
        if !valid {
            return Err(RepositoryError::InvalidInput);
        }
        let mode = storage_mode(&input.storage)?;
        let reference = format!(
            "{}.auth.{}.{}.{}",
            input.provider_id,
            input.method,
            if mode == CredentialMode::System {
                "system"
            } else {
                "session"
            },
            Uuid::new_v4().simple()
        );
        self.replace(
            &input.provider_id,
            &input.method,
            &input.storage,
            &reference,
            SecretString::new(input.secret),
        )
        .await?;
        self.status_inner(&input.provider_id).await
    }
    async fn replace(
        &self,
        provider: &str,
        method: &str,
        storage: &str,
        reference: &str,
        secret: SecretString,
    ) -> Result<(), RepositoryError> {
        let mode = storage_mode(storage)?;
        let previous = self.config(provider).await?;
        self.credentials
            .put(reference, secret, mode)
            .await
            .map_err(RepositoryError::Provider)?;
        let result:Result<(),RepositoryError>=async {
            let mut tx=self.repo.pool().begin().await?;
            if let Some(previous)=&previous {
                if previous.reference!=reference {
                    sqlx::query("INSERT OR IGNORE INTO provider_auth_retirements(credential_ref,provider_id,storage) VALUES (?,?,?)").bind(&previous.reference).bind(provider).bind(&previous.storage).execute(&mut *tx).await?;
                }
            }
            sqlx::query("INSERT INTO provider_auth_settings(provider_id,method,storage,credential_ref) VALUES (?,?,?,?) ON CONFLICT(provider_id) DO UPDATE SET method=excluded.method,storage=excluded.storage,credential_ref=excluded.credential_ref").bind(provider).bind(method).bind(storage).bind(reference).execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(())
        }.await;
        if let Err(error) = result {
            if self.credentials.delete(reference, mode).await.is_err() {
                // Retain cleanup intent even for a newly staged credential when
                // a settings write fails and the key service also refuses delete.
                let _=sqlx::query("INSERT OR IGNORE INTO provider_auth_retirements(credential_ref,provider_id,storage) VALUES (?,?,?)").bind(reference).bind(provider).bind(storage).execute(self.repo.pool()).await;
            }
            return Err(error);
        }
        Ok(())
    }
    /// Retry a small bounded batch. Delete/timeout/cleanup-SQL failures retain the
    /// durable intent and surface as cleanup_pending, never as a failed save.
    async fn retry_retirements(&self, provider: &str) -> Result<bool, RepositoryError> {
        let rows=sqlx::query("SELECT credential_ref,storage FROM provider_auth_retirements WHERE provider_id=? ORDER BY credential_ref LIMIT 4").bind(provider).fetch_all(self.repo.pool()).await?;
        for row in rows {
            let reference: String = row.try_get("credential_ref")?;
            let storage: String = row.try_get("storage")?;
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                self.credentials.delete(&reference, storage_mode(&storage)?),
            )
            .await;
            if matches!(result, Ok(Ok(()))) {
                let _ = sqlx::query("DELETE FROM provider_auth_retirements WHERE credential_ref=?")
                    .bind(reference)
                    .execute(self.repo.pool())
                    .await;
            }
        }
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM provider_auth_retirements WHERE provider_id=?)",
        )
        .bind(provider)
        .fetch_one(self.repo.pool())
        .await?)
    }
    /// Explicit trusted bootstrap for an existing cloud/session binding, never OS storage.
    pub async fn configure_session_binding(
        &self,
        provider: &str,
        secret: SecretString,
    ) -> Result<AuthStatus, RepositoryError> {
        let _gate = self.gate.lock().await;
        if provider != "tmdb" {
            return Err(RepositoryError::InvalidInput);
        }
        // Runtime bindings never override a saved choice, including an expired
        // prior session. Historical job references are never rebound on restart.
        if self.config(provider).await?.is_some() {
            return self.status_inner(provider).await;
        }
        let reference = format!("tmdb.auth.bearer.session.{}", Uuid::new_v4().simple());
        self.replace(provider, "bearer", "session_only", &reference, secret)
            .await?;
        self.status_inner(provider).await
    }
    pub async fn clear(&self, provider: &str) -> Result<AuthStatus, RepositoryError> {
        let _gate = self.gate.lock().await;
        validate_provider(provider)?;
        if let Some(previous) = self.config(provider).await? {
            let mut tx = self.repo.pool().begin().await?;
            sqlx::query("INSERT OR IGNORE INTO provider_auth_retirements(credential_ref,provider_id,storage) VALUES (?,?,?)").bind(&previous.reference).bind(provider).bind(&previous.storage).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM provider_auth_settings WHERE provider_id=?")
                .bind(provider)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        self.status_inner(provider).await
    }
    /// Capture this opaque reference in the immutable ProviderJob, never secret material.
    pub async fn snapshot_ref(&self, provider: &str) -> Result<Option<String>, RepositoryError> {
        let _gate = self.gate.lock().await;
        validate_provider(provider)?;
        Ok(self.config(provider).await?.map(|c| c.reference))
    }
    /// Resolve only the captured ref. A rotation may invalidate an old job, but can
    /// never silently give that job a different method, token or storage mode.
    pub(crate) async fn resolve(
        &self,
        reference: Option<&str>,
    ) -> Result<ResolvedAuth, ProviderFailure> {
        let _gate = self.gate.lock().await;
        let reference = reference.ok_or(ProviderFailure::CredentialsUnavailable)?;
        let pieces = reference.split('.').collect::<Vec<_>>();
        if pieces.len() != 5
            || !matches!(pieces[0], "tmdb" | "dmm")
            || pieces[1] != "auth"
            || Uuid::parse_str(pieces[4]).is_err()
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let method = match (pieces[0], pieces[2]) {
            ("tmdb", "bearer") => "bearer",
            ("tmdb", "api_key") => "api_key",
            ("dmm", "affiliate") => "affiliate",
            _ => return Err(ProviderFailure::InvalidConfiguration),
        };
        let mode = match pieces[3] {
            "system" => CredentialMode::System,
            "session" => CredentialMode::SessionOnly,
            _ => return Err(ProviderFailure::InvalidConfiguration),
        };
        let retired = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM provider_auth_retirements WHERE credential_ref=?)",
        )
        .bind(reference)
        .fetch_one(self.repo.pool())
        .await
        .map_err(|_| ProviderFailure::CredentialsUnavailable)?;
        if retired {
            return Err(ProviderFailure::CredentialsUnavailable);
        }
        let secret = self.credentials.get(reference, mode).await?;
        Ok(ResolvedAuth { method, secret })
    }
}
/// Secret-bearing multi-field credential. No Debug/Serialize implementation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AffiliateCredential {
    pub api_id: String,
    pub affiliate_id: String,
}
pub(crate) fn parse_affiliate_credential(
    value: &str,
) -> Result<AffiliateCredential, ProviderFailure> {
    if value.len() > 4096 {
        return Err(ProviderFailure::InvalidConfiguration);
    }
    let parsed: AffiliateCredential =
        serde_json::from_str(value).map_err(|_| ProviderFailure::InvalidConfiguration)?;
    if [&parsed.api_id, &parsed.affiliate_id].iter().any(|v| {
        v.is_empty()
            || v.len() > 256
            || !v
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    }) {
        return Err(ProviderFailure::InvalidConfiguration);
    }
    Ok(parsed)
}
fn validate_provider(provider: &str) -> Result<(), RepositoryError> {
    if matches!(
        provider,
        "tmdb" | "anilist" | "wikipedia.html" | "dmm" | "wikidata"
    ) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidInput)
    }
}
fn storage_mode(storage: &str) -> Result<CredentialMode, RepositoryError> {
    match storage {
        "system" => Ok(CredentialMode::System),
        "session_only" => Ok(CredentialMode::SessionOnly),
        _ => Err(RepositoryError::InvalidInput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn service(store: Arc<CredentialStore>) -> (tempfile::TempDir, ProviderAuthService) {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("auth.sqlite"))
            .await
            .unwrap();
        (dir, ProviderAuthService::new(repo, store))
    }
    fn input(method: &str, storage: &str, secret: &str) -> AuthSaveInput {
        AuthSaveInput {
            provider_id: "tmdb".into(),
            method: method.into(),
            storage: storage.into(),
            secret: secret.into(),
        }
    }
    #[tokio::test]
    async fn session_rotation_clear_and_restart_never_persist_or_echo_credentials() {
        let store = Arc::new(CredentialStore::new());
        let (_dir, service) = service(store.clone()).await;
        let before = service.status("tmdb").await.unwrap();
        assert!(!before.configured);
        assert_eq!(before.storage, "system");
        let status = service
            .save(input("bearer", "session_only", "synthetic-bearer-value"))
            .await
            .unwrap();
        assert!(status.configured);
        assert_eq!(status.method, "bearer");
        let first = service.snapshot_ref("tmdb").await.unwrap().unwrap();
        assert_eq!(
            service.resolve(Some(&first)).await.unwrap().method,
            "bearer"
        );
        let output = serde_json::to_string(&status).unwrap();
        assert!(!output.contains("synthetic-bearer-value"));
        assert!(!output.contains("credential_ref"));
        let persisted: String = sqlx::query_scalar(
            "SELECT method||storage||credential_ref FROM provider_auth_settings",
        )
        .fetch_one(service.repo.pool())
        .await
        .unwrap();
        assert!(!persisted.contains("synthetic-bearer-value"));
        let status = service
            .save(input("api_key", "session_only", "synthetic-api-key"))
            .await
            .unwrap();
        assert_eq!(status.method, "api_key");
        let second = service.snapshot_ref("tmdb").await.unwrap().unwrap();
        assert_ne!(first, second);
        assert_eq!(
            service.resolve(Some(&first)).await.err(),
            Some(ProviderFailure::CredentialsUnavailable)
        );
        assert_eq!(
            store.get(&first, CredentialMode::SessionOnly).await.err(),
            Some(ProviderFailure::CredentialsUnavailable)
        );
        assert_eq!(
            service.resolve(Some(&second)).await.unwrap().method,
            "api_key"
        );
        let restarted =
            ProviderAuthService::new(service.repo.clone(), Arc::new(CredentialStore::new()));
        let status = restarted.status("tmdb").await.unwrap();
        assert!(!status.configured);
        assert_eq!(status.status, "missing_credentials");
        assert!(!service.clear("tmdb").await.unwrap().configured);
        assert_eq!(
            store.get(&second, CredentialMode::SessionOnly).await.err(),
            Some(ProviderFailure::CredentialsUnavailable)
        );
        assert!(service.snapshot_ref("tmdb").await.unwrap().is_none());
    }
    #[tokio::test]
    async fn unavailable_native_storage_is_explicit_without_automatic_session_fallback() {
        let store = Arc::new(CredentialStore::unavailable_system_for_test());
        let (_dir, service) = service(store.clone()).await;
        assert_eq!(
            service
                .save(input("bearer", "system", "synthetic-private-token"))
                .await,
            Err(RepositoryError::Provider(
                ProviderFailure::CredentialsUnavailable
            ))
        );
        assert!(!service.status("tmdb").await.unwrap().configured);
        assert_eq!(
            store
                .get("tmdb.access_token", CredentialMode::SessionOnly)
                .await
                .err(),
            Some(ProviderFailure::CredentialsUnavailable)
        );
        assert!(
            service
                .save(input("bearer", "session_only", "synthetic-session-token"))
                .await
                .unwrap()
                .configured
        );
        let old = service.snapshot_ref("tmdb").await.unwrap();
        assert_eq!(
            service
                .save(input("api_key", "system", "synthetic-key"))
                .await,
            Err(RepositoryError::Provider(
                ProviderFailure::CredentialsUnavailable
            ))
        );
        assert_eq!(service.snapshot_ref("tmdb").await.unwrap(), old);
        assert_eq!(service.status("tmdb").await.unwrap().method, "bearer");
        service.clear("tmdb").await.unwrap();
    }
    #[tokio::test]
    async fn public_providers_and_invalid_methods_are_honest() {
        let (_dir, service) = service(Arc::new(CredentialStore::new())).await;
        let public = service.status("anilist").await.unwrap();
        assert_eq!(public.supported_methods, vec!["none"]);
        assert_eq!(public.status, "not_required");
        assert_eq!(
            service
                .save(input("oauth", "session_only", "synthetic-value"))
                .await,
            Err(RepositoryError::InvalidInput)
        );
        assert_eq!(
            service
                .save(input("bearer", "plaintext", "synthetic-value"))
                .await,
            Err(RepositoryError::InvalidInput)
        );
        assert_eq!(
            service
                .save(input("bearer", "session_only", "bad\nvalue"))
                .await,
            Err(RepositoryError::InvalidInput)
        );
        assert_eq!(
            service.status("unknown").await,
            Err(RepositoryError::InvalidInput)
        );
        let parsed: AuthSaveInput = serde_json::from_str(
            r#"{"provider_id":"tmdb","method":"bearer","secret":"synthetic"}"#,
        )
        .unwrap();
        assert_eq!(parsed.storage, "system");
    }
    #[tokio::test]
    async fn failed_settings_write_keeps_previous_credential_and_bootstrap_never_overrides_user_choice(
    ) {
        let store = Arc::new(CredentialStore::new());
        let (_dir, service) = service(store.clone()).await;
        service
            .save(input("api_key", "session_only", "synthetic-original-key"))
            .await
            .unwrap();
        let previous = service.snapshot_ref("tmdb").await.unwrap();
        let status = service
            .configure_session_binding("tmdb", SecretString::new("synthetic-env-binding".into()))
            .await
            .unwrap();
        assert_eq!(status.method, "api_key");
        assert_eq!(service.snapshot_ref("tmdb").await.unwrap(), previous);
        assert!(store
            .get("tmdb.access_token", CredentialMode::SessionOnly)
            .await
            .is_err());
        sqlx::query("CREATE TRIGGER fail_auth_write BEFORE UPDATE ON provider_auth_settings BEGIN SELECT RAISE(ABORT,'synthetic_rejection'); END").execute(service.repo.pool()).await.unwrap();
        assert_eq!(
            service
                .save(input("bearer", "session_only", "synthetic-rejected-bearer"))
                .await,
            Err(RepositoryError::Database)
        );
        assert_eq!(service.snapshot_ref("tmdb").await.unwrap(), previous);
        let resolved = service.resolve(previous.as_deref()).await.unwrap();
        assert_eq!(resolved.method, "api_key");
        assert_eq!(resolved.secret.expose(), "synthetic-original-key");
    }
    #[tokio::test]
    async fn rotation_delete_failure_is_saved_with_durable_warning_then_status_retries_cleanup() {
        let store = Arc::new(CredentialStore::new());
        let (_dir, service) = service(store.clone()).await;
        service
            .save(input("bearer", "session_only", "synthetic-old-bearer"))
            .await
            .unwrap();
        let old = service.snapshot_ref("tmdb").await.unwrap().unwrap();
        store.set_deletion_unavailable_for_test(true);
        let status = service
            .save(input("api_key", "session_only", "synthetic-active-key"))
            .await
            .unwrap();
        assert!(status.configured);
        assert_eq!(status.method, "api_key");
        assert_eq!(status.status, "cleanup_pending");
        let new = service.snapshot_ref("tmdb").await.unwrap().unwrap();
        assert_ne!(new, old);
        assert_eq!(service.resolve(Some(&new)).await.unwrap().method, "api_key");
        assert_eq!(
            service.resolve(Some(&old)).await.err(),
            Some(ProviderFailure::CredentialsUnavailable)
        );
        assert!(store.get(&old, CredentialMode::SessionOnly).await.is_ok());
        let tracked: String = sqlx::query_scalar(
            "SELECT credential_ref||storage||provider_id FROM provider_auth_retirements",
        )
        .fetch_one(service.repo.pool())
        .await
        .unwrap();
        assert!(tracked.contains(&old));
        assert!(!tracked.contains("synthetic-old-bearer"));
        assert!(!tracked.contains("synthetic-active-key"));
        assert_eq!(
            service.status("tmdb").await.unwrap().status,
            "cleanup_pending"
        );
        store.set_deletion_unavailable_for_test(false);
        assert_eq!(service.status("tmdb").await.unwrap().status, "configured");
        assert!(store.get(&old, CredentialMode::SessionOnly).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_auth_retirements")
            .fetch_one(service.repo.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert!(service.resolve(Some(&new)).await.is_ok());
    }
    #[tokio::test]
    async fn clear_commits_trackable_cleanup_before_secret_delete_and_sql_failure_keeps_auth() {
        let store = Arc::new(CredentialStore::new());
        let (_dir, service) = service(store.clone()).await;
        service
            .save(input("bearer", "session_only", "synthetic-current-token"))
            .await
            .unwrap();
        let reference = service.snapshot_ref("tmdb").await.unwrap().unwrap();
        sqlx::query("CREATE TRIGGER fail_auth_clear BEFORE DELETE ON provider_auth_settings BEGIN SELECT RAISE(ABORT,'synthetic_clear_rejection'); END").execute(service.repo.pool()).await.unwrap();
        assert_eq!(service.clear("tmdb").await, Err(RepositoryError::Database));
        assert_eq!(
            service.snapshot_ref("tmdb").await.unwrap().as_deref(),
            Some(reference.as_str())
        );
        assert!(service.resolve(Some(&reference)).await.is_ok());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_auth_retirements")
            .fetch_one(service.repo.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
        sqlx::query("DROP TRIGGER fail_auth_clear")
            .execute(service.repo.pool())
            .await
            .unwrap();
        store.set_deletion_unavailable_for_test(true);
        let status = service.clear("tmdb").await.unwrap();
        assert!(!status.configured);
        assert_eq!(status.status, "cleanup_pending");
        assert!(service.snapshot_ref("tmdb").await.unwrap().is_none());
        assert_eq!(
            service.resolve(Some(&reference)).await.err(),
            Some(ProviderFailure::CredentialsUnavailable)
        );
        assert!(store
            .get(&reference, CredentialMode::SessionOnly)
            .await
            .is_ok());
        store.set_deletion_unavailable_for_test(false);
        let status = service.clear("tmdb").await.unwrap();
        assert!(!status.configured);
        assert_eq!(status.status, "not_configured");
        assert!(store
            .get(&reference, CredentialMode::SessionOnly)
            .await
            .is_err());
    }
    #[tokio::test]
    async fn failed_outbox_sql_cleanup_remains_pending_and_delete_is_idempotent_on_retry() {
        let store = Arc::new(CredentialStore::new());
        let (_dir, service) = service(store.clone()).await;
        service
            .save(input("bearer", "session_only", "synthetic-old-token"))
            .await
            .unwrap();
        let old = service.snapshot_ref("tmdb").await.unwrap().unwrap();
        sqlx::query("CREATE TRIGGER fail_outbox_cleanup BEFORE DELETE ON provider_auth_retirements BEGIN SELECT RAISE(ABORT,'synthetic_cleanup_rejection'); END").execute(service.repo.pool()).await.unwrap();
        let status = service
            .save(input("api_key", "session_only", "synthetic-new-key"))
            .await
            .unwrap();
        assert!(status.configured);
        assert_eq!(status.status, "cleanup_pending");
        assert!(store.get(&old, CredentialMode::SessionOnly).await.is_err());
        sqlx::query("DROP TRIGGER fail_outbox_cleanup")
            .execute(service.repo.pool())
            .await
            .unwrap();
        assert_eq!(service.status("tmdb").await.unwrap().status, "configured");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_auth_retirements")
            .fetch_one(service.repo.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    #[tokio::test]
    async fn trusted_binding_is_only_session_scoped_and_never_rebinds_historical_refs() {
        let store = Arc::new(CredentialStore::unavailable_system_for_test());
        let (_dir, service) = service(store.clone()).await;
        let status = service
            .configure_session_binding("tmdb", SecretString::new("synthetic-cloud-binding".into()))
            .await
            .unwrap();
        assert_eq!(status.storage, "session_only");
        let reference = service.snapshot_ref("tmdb").await.unwrap().unwrap();
        assert!(reference.starts_with("tmdb.auth.bearer.session."));
        assert!(store
            .get(&reference, CredentialMode::SessionOnly)
            .await
            .is_ok());
        let restarted =
            ProviderAuthService::new(service.repo.clone(), Arc::new(CredentialStore::new()));
        let status = restarted
            .configure_session_binding(
                "tmdb",
                SecretString::new("synthetic-new-env-binding".into()),
            )
            .await
            .unwrap();
        assert!(!status.configured);
        assert_eq!(status.status, "missing_credentials");
        assert_eq!(
            restarted.snapshot_ref("tmdb").await.unwrap().as_deref(),
            Some(reference.as_str())
        );
        service.clear("tmdb").await.unwrap();
        assert!(store
            .get(&reference, CredentialMode::SessionOnly)
            .await
            .is_err());
        let status = restarted
            .configure_session_binding(
                "tmdb",
                SecretString::new("synthetic-new-env-binding".into()),
            )
            .await
            .unwrap();
        assert!(status.configured);
        assert_ne!(
            restarted.snapshot_ref("tmdb").await.unwrap().as_deref(),
            Some(reference.as_str())
        );
    }
    #[tokio::test]
    async fn affiliate_multifield_json_is_secret_only_revision_scoped_and_validated() {
        let store = Arc::new(CredentialStore::new());
        let (_dir, service) = service(store.clone()).await;
        let mut input = AuthSaveInput {
            provider_id: "dmm".into(),
            method: "affiliate".into(),
            storage: "session_only".into(),
            secret: r#"{"api_id":"synthetic-api","affiliate_id":"synthetic-affiliate"}"#.into(),
        };
        let status = service
            .save(AuthSaveInput {
                provider_id: input.provider_id.clone(),
                method: input.method.clone(),
                storage: input.storage.clone(),
                secret: input.secret.clone(),
            })
            .await
            .unwrap();
        assert!(status.configured);
        assert_eq!(status.supported_methods, vec!["affiliate"]);
        assert!(!serde_json::to_string(&status)
            .unwrap()
            .contains("synthetic"));
        let old = service.snapshot_ref("dmm").await.unwrap().unwrap();
        assert!(old.starts_with("dmm.auth.affiliate.session."));
        assert_eq!(
            service.resolve(Some(&old)).await.unwrap().method,
            "affiliate"
        );
        input.secret =
            r#"{"api_id":"synthetic-new-api","affiliate_id":"synthetic-new-affiliate"}"#.into();
        service.save(input).await.unwrap();
        assert_ne!(
            service.snapshot_ref("dmm").await.unwrap().as_deref(),
            Some(old.as_str())
        );
        assert!(service.resolve(Some(&old)).await.is_err());
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT provider_id||method||storage||credential_ref FROM provider_auth_settings",
        )
        .fetch_all(service.repo.pool())
        .await
        .unwrap();
        assert!(!rows.join("").contains("synthetic"));
        for secret in [
            r#"{"api_id":"synthetic"}"#,
            r#"{"api_id":"a","affiliate_id":"b","cookie":"c"}"#,
            r#"{"api_id":"a b","affiliate_id":"b"}"#,
            r#"{"api_id":"a","affiliate_id":""}"#,
        ] {
            assert!(service
                .save(AuthSaveInput {
                    provider_id: "dmm".into(),
                    method: "affiliate".into(),
                    storage: "session_only".into(),
                    secret: secret.into()
                })
                .await
                .is_err());
        }
        assert_eq!(
            service.status("wikidata").await.unwrap().status,
            "not_required"
        );
        service.clear("dmm").await.unwrap();
        assert!(!service.status("dmm").await.unwrap().configured);
    }
}
