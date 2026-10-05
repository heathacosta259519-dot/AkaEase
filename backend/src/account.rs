pub use crate::personal::{UserPlaylist, UserPlaylists, UserProfile};
use crate::{
    api::{NeteaseClient, protocol},
    credentials::{CredentialStore, EphemeralStore, SecretServiceStore},
    diagnostics::{AuthOutcome, AuthPhase, Diagnostics},
    error::{BackendError, Result},
    model::{StreamSource, Track},
    session::StoredSession,
};
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, Notify},
    time::Instant,
};

const QR_LIFETIME: Duration = Duration::from_secs(180);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QrStatus {
    WaitingScan,
    WaitingConfirmation,
    Authenticated,
    Expired,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Persistence {
    None,
    Secure,
    MemoryOnly,
    CleanupRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub profile: Option<UserProfile>,
    pub persistence: Persistence,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrChallenge {
    pub attempt_id: String,
    pub qr_url: String,
    pub expires_in_ms: u64,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrProgress {
    pub status: QrStatus,
    pub session: SessionSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutReport {
    pub local_cleared: bool,
    pub credentials_cleared: bool,
    pub server_revoked: bool,
}

struct Pending {
    id: String,
    key: String,
    client: NeteaseClient,
    expires: Instant,
    next_poll: Instant,
    poll_gate: Arc<Mutex<()>>,
    authorized: bool,
}
struct Active {
    client: NeteaseClient,
    profile: UserProfile,
}
struct State {
    generation: u64,
    pending: Option<Pending>,
    active: Option<Active>,
    persistence: Persistence,
    login_beginning: bool,
    restoring: bool,
    credential_revision: u64,
    credential_pending: bool,
}
impl State {
    fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            profile: self.active.as_ref().map(|s| s.profile.clone()),
            persistence: self.persistence,
        }
    }
    fn advance(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        self.login_beginning = false;
        self.restoring = false;
    }
    fn credential_change(&mut self) -> u64 {
        self.credential_revision = self.credential_revision.wrapping_add(1);
        self.credential_pending = true;
        self.persistence = Persistence::CleanupRequired;
        self.credential_revision
    }
}

/// One service per application. State and credential IO have separate gates;
/// both network commits and credential writes reject superseded operations.
type ClientFactory = Arc<dyn Fn(Option<&StoredSession>) -> Result<NeteaseClient> + Send + Sync>;

pub struct AccountService {
    state: Arc<Mutex<State>>,
    store: Arc<dyn CredentialStore>,
    store_gate: Arc<Mutex<()>>,
    credential_changed: Arc<Notify>,
    factory: ClientFactory,
    diagnostics: Option<Arc<Diagnostics>>,
}

impl Default for AccountService {
    fn default() -> Self {
        Self::new()
    }
}
impl AccountService {
    pub fn new() -> Self {
        Self::with_store(
            Arc::new(SecretServiceStore),
            Arc::new(|secret| match secret {
                Some(secret) => NeteaseClient::restore(secret),
                None => NeteaseClient::new(),
            }),
        )
    }
    /// Never reads or modifies the system keyring. Useful for headless sessions and probes.
    pub fn ephemeral() -> Self {
        Self::with_store(Arc::new(EphemeralStore), Arc::new(|_| NeteaseClient::new()))
    }
    pub fn configured(proxy: crate::storage::ProxyConfig, memory_only: bool) -> Result<Self> {
        proxy.validate()?;
        let store: Arc<dyn CredentialStore> = if memory_only {
            Arc::new(EphemeralStore)
        } else {
            Arc::new(SecretServiceStore)
        };
        Ok(Self::with_store(
            store,
            Arc::new(move |secret| match secret {
                Some(secret) => NeteaseClient::restore_configured(secret, &proxy),
                None => NeteaseClient::configured(&proxy),
            }),
        ))
    }
    fn with_store(store: Arc<dyn CredentialStore>, factory: ClientFactory) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                generation: 0,
                pending: None,
                active: None,
                persistence: Persistence::None,
                login_beginning: false,
                restoring: false,
                credential_revision: 0,
                credential_pending: false,
            })),
            store,
            store_gate: Arc::new(Mutex::new(())),
            credential_changed: Arc::new(Notify::new()),
            factory,
            diagnostics: None,
        }
    }
    pub async fn snapshot(&self) -> SessionSnapshot {
        self.state.lock().await.snapshot()
    }

    pub fn with_diagnostics(mut self, diagnostics: Arc<Diagnostics>) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    fn record_auth<T>(
        &self,
        phase: AuthPhase,
        started: Instant,
        result: &Result<T>,
        code: Option<i64>,
    ) {
        if let Some(log) = &self.diagnostics {
            let service_code = match result {
                Err(BackendError::Service(code)) => Some(*code),
                _ => None,
            };
            let _ = log.record_auth(
                phase,
                AuthOutcome::from_result(result),
                started.elapsed().as_millis() as u64,
                code,
                service_code,
            );
        }
    }

    pub async fn begin_login(&self) -> Result<QrChallenge> {
        let generation = {
            let mut state = self.state.lock().await;
            if state.login_beginning {
                return Err(BackendError::Busy);
            }
            state.advance();
            state.active = None;
            state.login_beginning = true;
            let revision = state.credential_change();
            self.change_credentials(revision, None);
            state.generation
        };
        let started = Instant::now();
        let result = self.create_challenge(generation).await;
        self.record_auth(AuthPhase::Create, started, &result, None);
        if result.is_err() {
            let mut state = self.state.lock().await;
            if state.generation == generation {
                state.login_beginning = false;
            }
        }
        result
    }

    async fn create_challenge(&self, generation: u64) -> Result<QrChallenge> {
        let client = (self.factory)(None)?;
        let value = client
            .request_weapi_codes(
                "weapi/login/qrcode/unikey",
                serde_json::json!({"type":"1", "csrf_token":""}),
                &[200],
            )
            .await?;
        let key = value["unikey"]
            .as_str()
            .filter(|key| !key.is_empty() && key.len() <= 256)
            .ok_or_else(|| protocol("missing QR key"))?
            .to_owned();
        let attempt_id = format!("{:032x}", rand::random::<u128>());
        let mut qr_url = reqwest::Url::parse("https://music.163.com/login").expect("static URL");
        qr_url.query_pairs_mut().append_pair("codekey", &key);
        let mut state = self.state.lock().await;
        if state.generation != generation || !state.login_beginning {
            return Err(BackendError::StaleOperation);
        }
        state.login_beginning = false;
        state.pending = Some(Pending {
            id: attempt_id.clone(),
            key,
            client,
            expires: Instant::now() + QR_LIFETIME,
            next_poll: Instant::now(),
            poll_gate: Arc::new(Mutex::new(())),
            authorized: false,
        });
        Ok(QrChallenge {
            attempt_id,
            qr_url: qr_url.into(),
            expires_in_ms: QR_LIFETIME.as_millis() as u64,
            poll_interval_ms: POLL_INTERVAL.as_millis() as u64,
        })
    }

    pub async fn cancel_login(&self, attempt_id: &str) -> Result<()> {
        let mut state = self.state.lock().await;
        if state.pending.as_ref().is_none_or(|p| p.id != attempt_id) {
            return Err(BackendError::StaleOperation);
        }
        state.advance();
        Ok(())
    }

    pub async fn poll_login(&self, attempt_id: &str) -> Result<QrProgress> {
        let (generation, client, key, expires, authorized, _lease) = {
            let mut state = self.state.lock().await;
            let generation = state.generation;
            let pending = state
                .pending
                .as_mut()
                .filter(|p| p.id == attempt_id)
                .ok_or(BackendError::StaleOperation)?;
            if Instant::now() >= pending.expires {
                state.pending = None;
                return Ok(QrProgress {
                    status: QrStatus::Expired,
                    session: state.snapshot(),
                });
            }
            let lease = pending
                .poll_gate
                .clone()
                .try_lock_owned()
                .map_err(|_| BackendError::Busy)?;
            if Instant::now() < pending.next_poll {
                return Err(BackendError::Busy);
            }
            pending.next_poll = Instant::now() + POLL_INTERVAL;
            (
                generation,
                pending.client.clone(),
                pending.key.clone(),
                pending.expires,
                pending.authorized,
                lease,
            )
        };
        let code = if authorized {
            803
        } else {
            let started = Instant::now();
            let result = client
                .request_weapi_codes(
                    "weapi/login/qrcode/client/login",
                    serde_json::json!({"key":key, "type":"1", "csrf_token":""}),
                    &[200, 800, 801, 802, 803],
                )
                .await;
            self.record_auth(
                AuthPhase::Poll,
                started,
                &result,
                result.as_ref().ok().and_then(|v| v["code"].as_i64()),
            );
            let value = match result {
                Ok(value) => value,
                Err(error) => {
                    if matches!(
                        error,
                        BackendError::Unauthorized | BackendError::Protocol(_)
                    ) {
                        self.invalidate_pending(generation, attempt_id).await;
                    }
                    return Err(error);
                }
            };
            value["code"].as_i64().expect("validated code")
        };
        // Never adopt a cookie until the server confirms its account identity.
        let profile = if code == 803 || code == 200 {
            let started = Instant::now();
            let cookie = if client.cookies.has_login_cookie(&client.origin) {
                Ok(())
            } else {
                Err(protocol("successful QR check did not set login cookie"))
            };
            self.record_auth(AuthPhase::Cookie, started, &cookie, None);
            if let Err(error) = cookie {
                self.invalidate_pending(generation, attempt_id).await;
                return Err(error);
            }
            {
                let mut state = self.state.lock().await;
                if state.generation != generation {
                    return Err(BackendError::StaleOperation);
                }
                let pending = state
                    .pending
                    .as_mut()
                    .filter(|p| p.id == attempt_id)
                    .ok_or(BackendError::StaleOperation)?;
                pending.authorized = true;
            }
            let started = Instant::now();
            let result = client
                .profile()
                .await
                .and_then(|p| p.ok_or(BackendError::Unauthorized));
            self.record_auth(AuthPhase::Profile, started, &result, None);
            match result {
                Ok(profile) => Some(profile),
                Err(error) => {
                    if matches!(
                        error,
                        BackendError::Unauthorized | BackendError::Protocol(_)
                    ) {
                        self.invalidate_pending(generation, attempt_id).await;
                    }
                    return Err(error);
                }
            }
        } else {
            None
        };
        let mut state = self.state.lock().await;
        if state.generation != generation
            || state.pending.as_ref().is_none_or(|p| p.id != attempt_id)
        {
            return Err(BackendError::StaleOperation);
        }
        if Instant::now() >= expires {
            state.pending = None;
            return Ok(QrProgress {
                status: QrStatus::Expired,
                session: state.snapshot(),
            });
        }
        let status = match code {
            800 => QrStatus::Expired,
            801 => QrStatus::WaitingScan,
            802 => QrStatus::WaitingConfirmation,
            200 | 803 => QrStatus::Authenticated,
            _ => unreachable!(),
        };
        if let Some(profile) = profile {
            let secret = client.cookies.export().ok();
            let cleared = state.persistence == Persistence::None;
            state.active = Some(Active { client, profile });
            state.advance();
            let revision = state.credential_change();
            state.persistence = if cleared {
                Persistence::MemoryOnly
            } else {
                Persistence::CleanupRequired
            };
            self.change_credentials(revision, secret);
        } else if status == QrStatus::Expired {
            state.pending = None;
        }
        Ok(QrProgress {
            status,
            session: state.snapshot(),
        })
    }

    /// Call on startup. Temporary network failures preserve the saved secret for retry.
    pub async fn restore(&self) -> Result<SessionSnapshot> {
        let generation = {
            let mut state = self.state.lock().await;
            if state.active.is_some()
                || state.pending.is_some()
                || state.login_beginning
                || state.restoring
                || state.credential_pending
            {
                return Err(BackendError::Busy);
            }
            state.advance();
            state.restoring = true;
            state.generation
        };
        let started = Instant::now();
        let result = self.restore_generation(generation).await;
        self.record_auth(AuthPhase::Restore, started, &result, None);
        let mut state = self.state.lock().await;
        if state.generation == generation {
            state.restoring = false;
        }
        result
    }

    async fn restore_generation(&self, generation: u64) -> Result<SessionSnapshot> {
        let secret = match self.load_credentials().await {
            Ok(secret) => secret,
            Err(_) => {
                let mut state = self.state.lock().await;
                if state.generation == generation {
                    state.persistence = Persistence::MemoryOnly;
                    return Ok(state.snapshot());
                }
                return Err(BackendError::StaleOperation);
            }
        };
        if self.state.lock().await.generation != generation {
            return Err(BackendError::StaleOperation);
        }
        let Some(secret) = secret else {
            let state = self.state.lock().await;
            if state.generation != generation {
                return Err(BackendError::StaleOperation);
            }
            return Ok(state.snapshot());
        };
        let client = match (self.factory)(Some(&secret)) {
            Ok(client) => client,
            Err(error) => {
                self.clear_if_current(generation).await;
                return Err(error);
            }
        };
        self.validate_restored(generation, client).await
    }

    async fn validate_restored(
        &self,
        generation: u64,
        client: NeteaseClient,
    ) -> Result<SessionSnapshot> {
        let result = client.profile().await;
        let mut state = self.state.lock().await;
        if state.generation != generation {
            return Err(BackendError::StaleOperation);
        }
        match result {
            Ok(Some(profile)) => {
                state.active = Some(Active { client, profile });
                state.persistence = Persistence::Secure;
            }
            Ok(None) | Err(BackendError::Unauthorized) => {
                drop(state);
                self.clear_if_current(generation).await;
                state = self.state.lock().await;
                if state.generation != generation {
                    return Err(BackendError::StaleOperation);
                }
            }
            Err(error) => return Err(error),
        }
        Ok(state.snapshot())
    }

    pub async fn logout(&self) -> LogoutReport {
        let (old, cleanup) = {
            let mut state = self.state.lock().await;
            state.advance();
            let old = state.active.take();
            let revision = state.credential_change();
            (old, self.change_credentials(revision, None))
        };
        let cleared = cleanup.await.unwrap_or(false);
        let revoked = match old {
            None => false,
            Some(active) => active
                .client
                .request(
                    active
                        .client
                        .http
                        .post(active.client.endpoint("api/logout")),
                )
                .await
                .is_ok(),
        };
        LogoutReport {
            local_cleared: true,
            credentials_cleared: cleared,
            server_revoked: revoked,
        }
    }

    async fn current(&self) -> Result<(u64, NeteaseClient, String)> {
        let state = self.state.lock().await;
        let active = state.active.as_ref().ok_or(BackendError::Unauthorized)?;
        Ok((
            state.generation,
            active.client.clone(),
            active.profile.id.clone(),
        ))
    }
    async fn finish<T>(&self, generation: u64, result: Result<T>) -> Result<T> {
        let mut state = self.state.lock().await;
        if state.generation != generation {
            return Err(BackendError::StaleOperation);
        }
        if matches!(result, Err(BackendError::Unauthorized)) {
            state.advance();
            state.active = None;
            let revision = state.credential_change();
            let cleanup = self.change_credentials(revision, None);
            drop(state);
            let _ = cleanup.await;
        }
        result
    }

    async fn load_credentials(&self) -> Result<Option<StoredSession>> {
        let _gate = self.store_gate.lock().await;
        self.store.load().await
    }

    async fn clear_if_current(&self, generation: u64) {
        let cleanup = {
            let mut state = self.state.lock().await;
            if state.generation != generation {
                return;
            }
            let revision = state.credential_change();
            self.change_credentials(revision, None)
        };
        let _ = cleanup.await;
    }

    // Only this worker writes credentials. The state lock is never held during IO.
    // A superseded save is cleared before releasing the storage gate.
    fn change_credentials(
        &self,
        revision: u64,
        secret: Option<StoredSession>,
    ) -> tokio::task::JoinHandle<bool> {
        let store = self.store.clone();
        let store_gate = self.store_gate.clone();
        let state = self.state.clone();
        let changed = self.credential_changed.clone();
        let diagnostics = self.diagnostics.clone();
        tokio::spawn(async move {
            let _gate = store_gate.lock().await;
            if state.lock().await.credential_revision != revision {
                return false;
            }
            let started = Instant::now();
            let clear = store.clear().await;
            if let Some(log) = &diagnostics {
                let _ = log.record_auth(
                    AuthPhase::CredentialClear,
                    AuthOutcome::from_result(&clear),
                    started.elapsed().as_millis() as u64,
                    None,
                    None,
                );
            }
            let cleared = clear.is_ok();
            // A newer intent may have arrived during cleanup; do not save stale credentials.
            let current = state.lock().await.credential_revision == revision;
            let saved = if current {
                match secret.as_ref() {
                    Some(secret) => {
                        let started = Instant::now();
                        let save = store.save(secret).await;
                        if let Some(log) = &diagnostics {
                            let _ = log.record_auth(
                                AuthPhase::CredentialSave,
                                AuthOutcome::from_result(&save),
                                started.elapsed().as_millis() as u64,
                                None,
                                None,
                            );
                        }
                        save.is_ok()
                    }
                    None => false,
                }
            } else {
                false
            };
            let mut current = state.lock().await;
            if current.credential_revision == revision {
                current.persistence = if saved {
                    Persistence::Secure
                } else if !cleared {
                    Persistence::CleanupRequired
                } else if current.active.is_some() {
                    Persistence::MemoryOnly
                } else {
                    Persistence::None
                };
                current.credential_pending = false;
                changed.notify_waiters();
                return cleared;
            }
            drop(current);
            if secret.is_some() {
                let _ = store.clear().await;
            }
            false
        })
    }

    /// CLI/shutdown barrier; GUI login replies never wait for keyring persistence.
    pub async fn flush_credentials(&self) -> SessionSnapshot {
        loop {
            let notified = self.credential_changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let state = self.state.lock().await;
            if !state.credential_pending {
                return state.snapshot();
            }
            drop(state);
            notified.await;
        }
    }

    async fn invalidate_pending(&self, generation: u64, attempt_id: &str) {
        let mut state = self.state.lock().await;
        if state.generation == generation
            && state.pending.as_ref().is_some_and(|p| p.id == attempt_id)
        {
            state.pending = None;
            state.generation = state.generation.wrapping_add(1);
        }
    }
    pub async fn refresh(&self) -> Result<SessionSnapshot> {
        let (generation, client, id) = self.current().await?;
        let profile = client.profile().await.and_then(|profile| {
            profile
                .filter(|p| p.id == id)
                .ok_or(BackendError::Unauthorized)
        });
        let profile = self.finish(generation, profile).await?;
        let mut state = self.state.lock().await;
        if state.generation != generation {
            return Err(BackendError::StaleOperation);
        }
        if let Some(active) = &mut state.active {
            active.profile = profile;
        }
        Ok(state.snapshot())
    }
    pub async fn playlists(&self, offset: u32, limit: u32) -> Result<UserPlaylists> {
        let (generation, client, id) = self.current().await?;
        self.finish(generation, client.user_playlists(&id, offset, limit).await)
            .await
    }
    pub async fn liked_tracks(&self) -> Result<Vec<String>> {
        let (generation, client, id) = self.current().await?;
        self.finish(generation, client.liked_tracks(&id).await)
            .await
    }
    pub async fn daily_tracks(&self) -> Result<Vec<Track>> {
        let (generation, client, _) = self.current().await?;
        self.finish(generation, client.daily_tracks().await).await
    }
    pub async fn stream(&self, id: &str) -> Result<StreamSource> {
        let (generation, client, _) = self.current().await?;
        self.finish(generation, client.stream(id).await).await
    }
    pub async fn playlist(
        &self,
        id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<crate::model::PlaylistPage> {
        let (generation, client, _) = self.current().await?;
        self.finish(generation, client.playlist(id, offset, limit).await)
            .await
    }
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
