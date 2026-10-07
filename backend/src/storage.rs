//! Versioned, bounded non-secret state under XDG directories.
use crate::{
    api::validate_id,
    error::{BackendError, Result},
    model::Track,
    queue::Repeat,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub(crate) fn storage_error() -> BackendError {
    BackendError::Storage
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "url", rename_all = "snake_case")]
pub enum ProxyConfig {
    #[default]
    System,
    Direct,
    Http(String),
}
impl ProxyConfig {
    pub fn validate(&self) -> Result<()> {
        if let Self::Http(url) = self {
            let parsed = reqwest::Url::parse(url)
                .map_err(|_| BackendError::InvalidInput("invalid proxy URL".into()))?;
            if parsed.scheme() != "http"
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
                || parsed.path() != "/"
            {
                return Err(BackendError::InvalidInput(
                    "proxy must be an HTTP origin without credentials, query or path".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn apply(&self, builder: reqwest::ClientBuilder) -> Result<reqwest::ClientBuilder> {
        self.validate()?;
        Ok(match self {
            Self::System => builder,
            Self::Direct => builder.no_proxy(),
            Self::Http(url) => builder.no_proxy().proxy(
                reqwest::Proxy::all(url)
                    .map_err(|_| BackendError::InvalidInput("invalid proxy".into()))?,
            ),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppConfig {
    pub version: u32,
    pub cache_limit_bytes: u64,
    pub proxy: ProxyConfig,
    pub restore_queue: bool,
    #[serde(default)]
    pub default_quality: crate::model::SoundQuality,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            cache_limit_bytes: 32 * 1024 * 1024,
            proxy: ProxyConfig::System,
            restore_queue: true,
            default_quality: Default::default(),
        }
    }
}
impl AppConfig {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.cache_limit_bytes > 1024 * 1024 * 1024 {
            return Err(BackendError::InvalidInput(
                "config version must be 1; cache limit must be 0..1GiB".into(),
            ));
        }
        self.proxy.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedPlayer {
    pub version: u32,
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub volume: f64,
    pub position_ms: u64,
}
impl SavedPlayer {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.tracks.len() > 10_000
            || !self.volume.is_finite()
            || !(0.0..=1.0).contains(&self.volume)
            || (self.tracks.is_empty() && self.current_index.is_some())
            || (!self.tracks.is_empty()
                && self.current_index.is_none_or(|i| i >= self.tracks.len()))
            || self.position_ms > u64::MAX / 1_000_000
        {
            return Err(BackendError::InvalidInput(
                "invalid saved player state".into(),
            ));
        }
        for track in &self.tracks {
            validate_id(&track.id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
}
impl AppPaths {
    pub fn discover() -> Result<Self> {
        Self::from_env(|key| std::env::var_os(key).map(PathBuf::from))
    }
    fn from_env(get: impl Fn(&str) -> Option<PathBuf>) -> Result<Self> {
        let home = get("HOME").filter(|p| p.is_absolute());
        let base = |key, fallback| -> Result<PathBuf> {
            get(key)
                .filter(|p| p.is_absolute())
                .or_else(|| home.as_ref().map(|p| p.join(fallback)))
                .ok_or(BackendError::Storage)
        };
        Ok(Self {
            config_dir: base("XDG_CONFIG_HOME", ".config")?.join("akanetease"),
            state_dir: base("XDG_STATE_HOME", ".local/state")?.join("akanetease"),
            cache_dir: base("XDG_CACHE_HOME", ".cache")?.join("akanetease"),
        })
    }
    pub fn ensure(&self) -> Result<()> {
        for path in [&self.config_dir, &self.state_dir, &self.cache_dir] {
            private_dir(path)?;
        }
        Ok(())
    }
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.json")
    }
    pub fn player_file(&self) -> PathBuf {
        self.state_dir.join("player.json")
    }
    /// Hold for the desktop process lifetime. CLI read-only diagnostics need no lock.
    pub fn lock(&self) -> Result<File> {
        self.ensure()?;
        let path = self.state_dir.join("desktop.lock");
        reject_link(&path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| storage_error())?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => BackendError::AlreadyRunning,
            fs::TryLockError::Error(_) => storage_error(),
        })?;
        Ok(file)
    }
}

pub(crate) fn private_dir(path: &Path) -> Result<()> {
    reject_link(path)?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|_| storage_error())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| storage_error())
}
pub(crate) fn reject_link(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Err(storage_error()),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(storage_error()),
    }
}
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<Option<T>> {
    reject_link(path)?;
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(storage_error()),
    };
    if !file.metadata().map_err(|_| storage_error())?.is_file() {
        return Err(storage_error());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| storage_error())?;
    if bytes.len() as u64 > limit {
        return Err(storage_error());
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| storage_error())
}
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    reject_link(path)?;
    let parent = path.parent().ok_or_else(storage_error)?;
    let tmp = parent.join(format!(".aka-{:032x}.tmp", rand::random::<u128>()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| storage_error())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| storage_error())?;
        fs::rename(&tmp, path).map_err(|_| storage_error())?;
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| storage_error())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
pub(crate) fn atomic_json<T: Serialize>(path: &Path, value: &T, limit: usize) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| storage_error())?;
    if bytes.len() > limit {
        return Err(storage_error());
    }
    atomic_write(path, &bytes)
}
pub fn load_config(paths: &AppPaths) -> Result<AppConfig> {
    let config: AppConfig = read_json(&paths.config_file(), 16 * 1024)?.unwrap_or_default();
    config.validate()?;
    Ok(config)
}
pub fn save_config(paths: &AppPaths, config: &AppConfig) -> Result<()> {
    config.validate()?;
    paths.ensure()?;
    atomic_json(&paths.config_file(), config, 16 * 1024)
}
pub fn load_player(paths: &AppPaths) -> Result<Option<SavedPlayer>> {
    let player: Option<SavedPlayer> = read_json(&paths.player_file(), 16 * 1024 * 1024)?;
    if let Some(p) = &player {
        p.validate()?;
    }
    Ok(player)
}
pub fn save_player(paths: &AppPaths, player: &SavedPlayer) -> Result<()> {
    player.validate()?;
    paths.ensure()?;
    atomic_json(&paths.player_file(), player, 16 * 1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_config_defaults_quality_and_all_quality_values_roundtrip() {
        let old = serde_json::json!({"version":1,"cacheLimitBytes":33554432,"proxy":{"mode":"system"},"restoreQueue":true});
        let config: AppConfig = serde_json::from_value(old.clone()).unwrap();
        assert_eq!(config.default_quality, crate::model::SoundQuality::Exhigh);
        for level in ["standard", "higher", "exhigh", "lossless", "hires"] {
            let mut value = old.clone();
            value["defaultQuality"] = serde_json::json!(level);
            let config: AppConfig = serde_json::from_value(value.clone()).unwrap();
            config.validate().unwrap();
            assert_eq!(serde_json::to_value(config).unwrap(), value);
        }
        let mut invalid = old;
        invalid["defaultQuality"] = serde_json::json!("unsupported");
        assert!(serde_json::from_value::<AppConfig>(invalid).is_err());
    }
    #[test]
    fn xdg_ignores_relative_paths_and_requires_an_absolute_base() {
        let paths = AppPaths::from_env(|k| match k {
            "HOME" => Some("/tmp/user".into()),
            "XDG_CONFIG_HOME" => Some("relative".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            paths.config_dir,
            PathBuf::from("/tmp/user/.config/akanetease")
        );
        assert!(AppPaths::from_env(|_| None).is_err());
    }
    #[test]
    fn atomic_roundtrip_permissions_lock_and_invalid_state() {
        let root =
            std::env::temp_dir().join(format!("aka-storage-{:032x}", rand::random::<u128>()));
        let paths = AppPaths {
            config_dir: root.join("config"),
            state_dir: root.join("state"),
            cache_dir: root.join("cache"),
        };
        let config = AppConfig {
            proxy: ProxyConfig::Http("http://127.0.0.1:8888".into()),
            ..AppConfig::default()
        };
        save_config(&paths, &config).unwrap();
        assert_eq!(load_config(&paths).unwrap(), config);
        assert_eq!(
            fs::metadata(paths.config_file())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let lock = paths.lock().unwrap();
        assert!(matches!(paths.lock(), Err(BackendError::AlreadyRunning)));
        drop(lock);
        drop(paths.lock().unwrap());
        let player = SavedPlayer {
            version: 1,
            tracks: vec![],
            current_index: None,
            repeat: Repeat::All,
            shuffle: true,
            volume: 0.4,
            position_ms: 0,
        };
        save_player(&paths, &player).unwrap();
        assert_eq!(load_player(&paths).unwrap(), Some(player));
        fs::write(paths.player_file(), "broken").unwrap();
        assert!(load_player(&paths).is_err());
        fs::write(paths.config_file(), vec![b' '; 17000]).unwrap();
        assert!(load_config(&paths).is_err());
        assert!(
            ProxyConfig::Http("http://user:secret@localhost".into())
                .validate()
                .is_err()
        );
        assert!(
            ProxyConfig::Http("socks5://localhost".into())
                .validate()
                .is_err()
        );
        let target = root.join("outside");
        fs::write(&target, b"unchanged").unwrap();
        fs::remove_file(paths.player_file()).unwrap();
        std::os::unix::fs::symlink(&target, paths.player_file()).unwrap();
        assert!(load_player(&paths).is_err());
        assert_eq!(fs::read(target).unwrap(), b"unchanged");
        fs::remove_dir_all(root).unwrap();
    }
}
