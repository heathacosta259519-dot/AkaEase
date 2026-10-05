//! Fixed-schema diagnostics: no arbitrary user content, URLs or credentials.
use crate::{
    error::{BackendError, Result},
    storage::{AppPaths, private_dir, reject_link, storage_error},
};
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Started,
    Stopped,
    PlayerError,
    StorageFailed,
    RestoreFailed,
    MprisReady,
    MprisUnavailable,
    CacheFailed,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthPhase {
    Create,
    Poll,
    Cookie,
    Profile,
    CredentialClear,
    CredentialSave,
    Restore,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthOutcome {
    Success,
    Network,
    Timeout,
    Http,
    Service,
    Protocol,
    Unauthorized,
    StaleOperation,
    Busy,
    CredentialStorage,
    Failed,
}
impl AuthOutcome {
    pub fn from_result<T>(result: &Result<T>) -> Self {
        match result {
            Ok(_) => Self::Success,
            Err(BackendError::Network) => Self::Network,
            Err(BackendError::Timeout) => Self::Timeout,
            Err(BackendError::Http(_)) => Self::Http,
            Err(BackendError::Service(_)) => Self::Service,
            Err(BackendError::Protocol(_)) => Self::Protocol,
            Err(BackendError::Unauthorized) => Self::Unauthorized,
            Err(BackendError::StaleOperation) => Self::StaleOperation,
            Err(BackendError::Busy) => Self::Busy,
            Err(BackendError::CredentialStorage) => Self::CredentialStorage,
            Err(_) => Self::Failed,
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    pub mpris: String,
    pub persistence: String,
    pub queue_restored: bool,
    pub cache_available: bool,
}
impl Default for BackendStatus {
    fn default() -> Self {
        Self {
            mpris: "starting".into(),
            persistence: "ready".into(),
            queue_restored: false,
            cache_available: true,
        }
    }
}
pub struct Diagnostics {
    path: PathBuf,
    gate: Mutex<()>,
}
impl Diagnostics {
    pub fn new(paths: &AppPaths) -> Result<Self> {
        private_dir(&paths.state_dir)?;
        Ok(Self {
            path: paths.state_dir.join("backend.jsonl"),
            gate: Mutex::new(()),
        })
    }
    pub fn record(&self, event: Event) -> Result<()> {
        self.write(serde_json::json!({"event":event}))
    }
    pub fn record_auth(
        &self,
        phase: AuthPhase,
        outcome: AuthOutcome,
        elapsed_ms: u64,
        qr_code: Option<i64>,
        service_code: Option<i64>,
    ) -> Result<()> {
        let qr_code = qr_code.filter(|c| *c == 200 || (800..=803).contains(c));
        let service_code = service_code.filter(|c| (-999..=9999).contains(c));
        self.write(
            serde_json::json!({"event":"auth","phase":phase,"outcome":outcome,
            "elapsedMs":elapsed_ms,"qrCode":qr_code,"serviceCode":service_code}),
        )
    }
    fn write(&self, mut line: serde_json::Value) -> Result<()> {
        let _gate = self.gate.lock().map_err(|_| storage_error())?;
        reject_link(&self.path)?;
        if fs::metadata(&self.path).is_ok_and(|m| m.len() >= 1024 * 1024) {
            let previous = self.path.with_extension("jsonl.1");
            reject_link(&previous)?;
            fs::rename(&self.path, previous).map_err(|_| storage_error())?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.path)
            .map_err(|_| storage_error())?;
        line["timeUnixMs"] = serde_json::json!(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        writeln!(file, "{line}").map_err(|_| storage_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_schema_and_rotation_bound_log_growth() {
        let root = std::env::temp_dir().join(format!("aka-log-{:032x}", rand::random::<u128>()));
        let paths = AppPaths {
            config_dir: root.join("c"),
            state_dir: root.join("s"),
            cache_dir: root.join("cache"),
        };
        let log = Diagnostics::new(&paths).unwrap();
        log.record(Event::Started).unwrap();
        let line = fs::read_to_string(&log.path).unwrap();
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["event"], "started");
        assert_eq!(value.as_object().unwrap().len(), 2);
        let error: Result<()> = Err(BackendError::Protocol("synthetic-private-payload".into()));
        log.record_auth(
            AuthPhase::Profile,
            AuthOutcome::from_result(&error),
            123,
            Some(42),
            None,
        )
        .unwrap();
        let lines = fs::read_to_string(&log.path).unwrap();
        assert!(!lines.contains("synthetic-private-payload"));
        let auth: serde_json::Value = serde_json::from_str(lines.lines().nth(1).unwrap()).unwrap();
        assert_eq!(auth["phase"], "profile");
        assert_eq!(auth["outcome"], "protocol");
        assert_eq!(auth["elapsedMs"], 123);
        assert!(auth["qrCode"].is_null());
        assert!(auth["serviceCode"].is_null());
        assert_eq!(auth.as_object().unwrap().len(), 7);
        log.record_auth(AuthPhase::Poll, AuthOutcome::Service, 24, None, Some(882))
            .unwrap();
        let lines = fs::read_to_string(&log.path).unwrap();
        let failure: serde_json::Value =
            serde_json::from_str(lines.lines().nth(2).unwrap()).unwrap();
        assert_eq!(failure["serviceCode"], 882);
        fs::write(&log.path, vec![b' '; 1024 * 1024]).unwrap();
        log.record(Event::Stopped).unwrap();
        assert!(fs::metadata(&log.path).unwrap().len() < 200);
        assert_eq!(
            fs::metadata(log.path.with_extension("jsonl.1"))
                .unwrap()
                .len(),
            1024 * 1024
        );
        fs::remove_dir_all(root).unwrap();
    }
}
