use akanetease_backend::{
    cache::LyricCache,
    diagnostics::{BackendStatus, Diagnostics, Event},
    error::Result,
    player::{PlayerCommand, PlayerHandle},
    storage::{self, AppConfig, AppPaths},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub struct Persistence {
    pub paths: AppPaths,
    pub active_config: AppConfig,
    pub cache: Option<Arc<LyricCache>>,
    pub status: Mutex<BackendStatus>,
    pub log: Arc<Diagnostics>,
    save_gate: Arc<tokio::sync::Mutex<()>>,
    save_enabled: AtomicBool,
}
impl Persistence {
    pub fn new(paths: AppPaths, config: AppConfig) -> Result<Self> {
        let log = Arc::new(Diagnostics::new(&paths)?);
        let cache = LyricCache::new(&paths, config.cache_limit_bytes)
            .ok()
            .map(Arc::new);
        let status = BackendStatus {
            cache_available: cache.is_some(),
            ..BackendStatus::default()
        };
        Ok(Self {
            paths,
            active_config: config,
            cache,
            status: Mutex::new(status),
            log,
            save_gate: Arc::new(tokio::sync::Mutex::new(())),
            save_enabled: AtomicBool::new(true),
        })
    }
    pub fn record(&self, event: Event) {
        if self.log.record(event).is_err() {
            self.status.lock().unwrap().persistence = "degraded".into();
        }
    }
    pub async fn restore(&self, player: &PlayerHandle) {
        if !self.active_config.restore_queue {
            return;
        }
        let paths = self.paths.clone();
        let result = tokio::task::spawn_blocking(move || storage::load_player(&paths)).await;
        let result = match result {
            Ok(Ok(Some(saved))) => player
                .command(PlayerCommand::Restore(saved))
                .await
                .map(|_| true),
            Ok(Ok(None)) => Ok(false),
            _ => Err(akanetease_backend::error::BackendError::Storage),
        };
        match result {
            Ok(restored) => self.status.lock().unwrap().queue_restored = restored,
            Err(_) => {
                // Preserve corrupt or newer-version state for manual recovery.
                self.save_enabled.store(false, Ordering::SeqCst);
                self.status.lock().unwrap().persistence = "degraded".into();
                self.record(Event::RestoreFailed);
            }
        }
    }
    pub async fn save(&self, player: &PlayerHandle) -> Result<()> {
        if !self.active_config.restore_queue {
            return Ok(());
        }
        if !self.save_enabled.load(Ordering::SeqCst) {
            return Err(akanetease_backend::error::BackendError::Storage);
        }
        let gate = self.save_gate.clone().lock_owned().await;
        let saved = player.checkpoint().await?;
        let paths = self.paths.clone();
        // The owned guard stays with the write even if the caller is cancelled.
        let result = tokio::task::spawn_blocking(move || {
            let _gate = gate;
            storage::save_player(&paths, &saved)
        })
        .await
        .unwrap_or(Err(akanetease_backend::error::BackendError::Storage));
        if result.is_err() {
            self.status.lock().unwrap().persistence = "degraded".into();
            self.record(Event::StorageFailed);
        }
        result
    }
}
