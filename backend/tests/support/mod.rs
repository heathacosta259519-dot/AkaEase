use akanetease_backend::{
    audio::AudioEngine,
    model::{Album, StreamSource, Track},
    player::{PlayerHandle, PlayerSnapshot, ResolveFuture, SourceResolver},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
pub struct Fixture {
    path: PathBuf,
}
impl Fixture {
    pub fn new(seconds: u32) -> Self {
        let samples = 8000 * seconds;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + samples * 2).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(8000u32.to_le_bytes());
        wav.extend(16000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((samples * 2).to_le_bytes());
        wav.resize(44 + samples as usize * 2, 0);
        let path = std::env::temp_dir().join(format!(
            "aka-m3-{}-{}.wav",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::write(&path, wav).unwrap();
        Self { path }
    }
    pub fn player(&self) -> PlayerHandle {
        PlayerHandle::spawn(
            AudioEngine::silent().unwrap(),
            Arc::new(LocalSource(
                reqwest::Url::from_file_path(&self.path)
                    .unwrap()
                    .to_string(),
            )),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
struct LocalSource(String);
impl SourceResolver for LocalSource {
    fn resolve<'a>(&'a self, track: &'a Track) -> ResolveFuture<'a> {
        Box::pin(async move {
            Ok(StreamSource {
                track_id: track.id.clone(),
                url: self.0.clone(),
                bitrate: 128000,
                expires_in_seconds: None,
                is_preview: false,
            })
        })
    }
}
pub fn track(id: &str) -> Track {
    Track {
        id: id.into(),
        title: format!("Synthetic {id}"),
        artists: vec![],
        album: Album {
            id: "1".into(),
            name: "Generated WAV".into(),
            cover_url: None,
        },
        duration_ms: 3000,
    }
}
pub async fn wait(
    player: &PlayerHandle,
    predicate: impl Fn(&PlayerSnapshot) -> bool,
) -> PlayerSnapshot {
    let mut state = player.subscribe();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let s = state.borrow().clone();
            if predicate(&s) {
                return s;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .expect("player transition deadline")
}
