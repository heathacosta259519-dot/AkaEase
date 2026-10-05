//! Public lyrics only: no account responses, cookies, signed URLs or audio files.
use crate::{
    api::validate_id,
    error::Result,
    lyrics::LyricLine,
    storage::{AppPaths, atomic_json, private_dir, read_json, storage_error},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
const MAX_ENTRY: u64 = 2 * 1024 * 1024;
const TTL: u64 = 7 * 24 * 60 * 60;
#[derive(Serialize, Deserialize)]
struct Entry {
    version: u32,
    saved_at: u64,
    lines: Vec<LyricLine>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub bytes: u64,
    pub entries: usize,
    pub limit_bytes: u64,
}
pub struct LyricCache {
    root: PathBuf,
    limit: u64,
    gate: Mutex<()>,
}
impl LyricCache {
    pub fn new(paths: &AppPaths, limit: u64) -> Result<Self> {
        let root = paths.cache_dir.join("lyrics-v1");
        paths.ensure()?;
        private_dir(&root)?;
        let cache = Self {
            root,
            limit,
            gate: Mutex::new(()),
        };
        cache.prune()?;
        Ok(cache)
    }
    fn file(&self, id: &str) -> Result<PathBuf> {
        Ok(self.root.join(format!("{}.json", validate_id(id)?)))
    }
    fn entries(&self) -> Result<Vec<(PathBuf, u64, SystemTime)>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_| storage_error())? {
            let entry = entry.map_err(|_| storage_error())?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !name
                .strip_suffix(".json")
                .is_some_and(|s| validate_id(s).is_ok())
                && !(name.starts_with(".aka-") && name.ends_with(".tmp"))
            {
                continue;
            }
            let m = fs::symlink_metadata(entry.path()).map_err(|_| storage_error())?;
            if m.is_file() {
                entries.push((entry.path(), m.len(), m.modified().unwrap_or(UNIX_EPOCH)));
            }
        }
        Ok(entries)
    }
    pub fn get(&self, id: &str) -> Result<Option<Vec<LyricLine>>> {
        let path = self.file(id)?;
        let _gate = self.gate.lock().map_err(|_| storage_error())?;
        if self.limit == 0 {
            return Ok(None);
        }
        match read_json::<Entry>(&path, MAX_ENTRY) {
            Ok(Some(entry))
                if entry.version == 1
                    && now()
                        .checked_sub(entry.saved_at)
                        .is_some_and(|age| age < TTL) =>
            {
                Ok(Some(entry.lines))
            }
            Ok(None) => Ok(None),
            _ => {
                // Corruption is a cache miss, never a permanent network failure.
                if fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) {
                    fs::remove_file(path).map_err(|_| storage_error())?;
                }
                Ok(None)
            }
        }
    }
    pub fn put(&self, id: &str, lines: Vec<LyricLine>) -> Result<()> {
        let path = self.file(id)?;
        let _gate = self.gate.lock().map_err(|_| storage_error())?;
        if self.limit == 0 {
            return Ok(());
        }
        let entry = Entry {
            version: 1,
            saved_at: now(),
            lines,
        };
        let bytes = serde_json::to_vec(&entry).map_err(|_| storage_error())?;
        if bytes.len() as u64 > self.limit.min(MAX_ENTRY) {
            return Ok(());
        }
        atomic_json(&path, &entry, MAX_ENTRY as usize)?;
        self.prune_inner()
    }
    fn prune_inner(&self) -> Result<()> {
        let mut entries = self.entries()?;
        entries.sort_by_key(|(_, _, time)| *time);
        let mut total = entries.iter().map(|(_, size, _)| *size).sum::<u64>();
        for (path, size, _) in entries {
            if total <= self.limit {
                break;
            }
            fs::remove_file(path).map_err(|_| storage_error())?;
            total = total.saturating_sub(size);
        }
        Ok(())
    }
    pub fn prune(&self) -> Result<()> {
        let _gate = self.gate.lock().map_err(|_| storage_error())?;
        self.prune_inner()
    }
    pub fn stats(&self) -> Result<CacheStats> {
        let _gate = self.gate.lock().map_err(|_| storage_error())?;
        let entries = self.entries()?;
        Ok(CacheStats {
            bytes: entries.iter().map(|(_, size, _)| *size).sum(),
            entries: entries.len(),
            limit_bytes: self.limit,
        })
    }
    pub fn clear(&self) -> Result<()> {
        let _gate = self.gate.lock().map_err(|_| storage_error())?;
        for (path, _, _) in self.entries()? {
            fs::remove_file(path).map_err(|_| storage_error())?;
        }
        Ok(())
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hit_expiry_corruption_eviction_disabled_and_clear() {
        let root = std::env::temp_dir().join(format!("aka-cache-{:032x}", rand::random::<u128>()));
        let paths = AppPaths {
            config_dir: root.join("c"),
            state_dir: root.join("s"),
            cache_dir: root.join("cache"),
        };
        let cache = LyricCache::new(&paths, 200).unwrap();
        let lines = vec![LyricLine {
            time_ms: 0,
            text: "synthetic lyric".into(),
            translation: None,
        }];
        cache.put("1", lines.clone()).unwrap();
        assert_eq!(cache.get("01").unwrap(), Some(lines.clone()));
        cache.put("2", lines.clone()).unwrap();
        assert!(cache.stats().unwrap().bytes <= 200);
        atomic_json(
            &cache.file("3").unwrap(),
            &Entry {
                version: 1,
                saved_at: 1,
                lines: lines.clone(),
            },
            1024,
        )
        .unwrap();
        assert_eq!(cache.get("3").unwrap(), None);
        fs::write(cache.file("4").unwrap(), b"bad").unwrap();
        assert_eq!(cache.get("4").unwrap(), None);
        assert!(matches!(
            cache.get("../secret"),
            Err(crate::error::BackendError::InvalidInput(_))
        ));
        let unrelated = cache.root.join("keep.txt");
        fs::write(&unrelated, b"keep").unwrap();
        cache.clear().unwrap();
        assert_eq!(cache.stats().unwrap().entries, 0);
        assert!(unrelated.exists());
        let disabled = LyricCache::new(&paths, 0).unwrap();
        disabled.put("5", lines).unwrap();
        assert_eq!(disabled.get("5").unwrap(), None);
        fs::remove_dir_all(root).unwrap();
    }
}
