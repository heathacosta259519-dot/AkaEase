use super::*;
use crate::model::Album;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

pub(super) fn track(id: &str) -> Track {
    Track {
        id: id.into(),
        title: format!("Track {id}"),
        artists: vec![],
        album: Album {
            id: "1".into(),
            name: "Synthetic".into(),
            cover_url: None,
        },
        duration_ms: 2000,
    }
}
type FakeState = (PlaybackSnapshot, Vec<AudioEvent>, Vec<String>);
#[derive(Clone)]
struct FakeAudio(Arc<Mutex<FakeState>>);
impl FakeAudio {
    fn new() -> Self {
        Self(Arc::new(Mutex::new((
            PlaybackSnapshot {
                state: PlaybackState::Stopped,
                position_ms: 0,
                duration_ms: Some(2000),
                volume: 0.5,
            },
            vec![],
            vec![],
        ))))
    }
    fn event(&self, event: AudioEvent) {
        self.0.lock().unwrap().1.push(event);
    }
}
impl AudioDriver for FakeAudio {
    fn load(&mut self, uri: &str) -> Result<()> {
        self.0.lock().unwrap().2.push(uri.into());
        Ok(())
    }
    fn play(&mut self) -> Result<()> {
        self.0.lock().unwrap().0.state = PlaybackState::Playing;
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.0.lock().unwrap().0.state = PlaybackState::Paused;
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        let mut s = self.0.lock().unwrap();
        s.0.state = PlaybackState::Stopped;
        s.0.position_ms = 0;
        s.1.clear();
        Ok(())
    }
    fn seek(&mut self, ms: u64) -> Result<()> {
        self.0.lock().unwrap().0.position_ms = ms;
        Ok(())
    }
    fn volume(&mut self, v: f64) -> Result<()> {
        self.0.lock().unwrap().0.volume = v;
        Ok(())
    }
    fn snapshot(&self) -> PlaybackSnapshot {
        self.0.lock().unwrap().0.clone()
    }
    fn events(&mut self) -> Result<Vec<AudioEvent>> {
        Ok(std::mem::take(&mut self.0.lock().unwrap().1))
    }
    fn can_seek(&self) -> bool {
        true
    }
}
struct Resolver {
    calls: AtomicUsize,
    fail: bool,
    slow_first: bool,
}
impl SourceResolver for Resolver {
    fn resolve<'a>(&'a self, track: &'a Track) -> ResolveFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.slow_first && track.id == "1" {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            if self.fail {
                return Err(BackendError::Unavailable);
            }
            Ok(StreamSource {
                track_id: track.id.clone(),
                url: format!("https://audio.invalid/{}", track.id),
                bitrate: 128000,
                expires_in_seconds: Some(10),
                is_preview: false,
            })
        })
    }
}
async fn wait(
    player: &PlayerHandle,
    predicate: impl Fn(&PlayerSnapshot) -> bool,
) -> PlayerSnapshot {
    let mut state = player.subscribe();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let current = state.borrow().clone();
            if predicate(&current) {
                return current;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn fast_selection_pause_during_resolution_and_stale_edits() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(Resolver {
        calls: AtomicUsize::new(0),
        fail: false,
        slow_first: true,
    });
    let player = PlayerHandle::spawn(audio.clone(), resolver);
    let s = player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1"), track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    player.command(PlayerCommand::Pause).await.unwrap();
    player
        .command(PlayerCommand::Select {
            index: 1,
            revision: s.queue_revision.clone(),
        })
        .await
        .unwrap();
    let ready = wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    assert_eq!(ready.current.unwrap().id, "2");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(audio.0.lock().unwrap().2, vec!["https://audio.invalid/2"]);
    player
        .command(PlayerCommand::Remove {
            index: 0,
            revision: s.queue_revision.clone(),
        })
        .await
        .unwrap();
    assert!(matches!(
        player
            .command(PlayerCommand::Select {
                index: 0,
                revision: s.queue_revision
            })
            .await,
        Err(BackendError::StaleOperation)
    ));
    assert!(matches!(
        player
            .command(PlayerCommand::Seek {
                position_ms: 1,
                selection_id: s.selection_id
            })
            .await,
        Err(BackendError::StaleOperation)
    ));
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn pause_while_loading_does_not_autoplay_and_eos_advances() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(Resolver {
        calls: AtomicUsize::new(0),
        fail: false,
        slow_first: true,
    });
    let player = PlayerHandle::spawn(audio.clone(), resolver);
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1"), track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    player.command(PlayerCommand::Pause).await.unwrap();
    let paused = wait(&player, |s| s.playback.state == PlaybackState::Paused).await;
    assert!(!paused.play_when_ready);
    player.command(PlayerCommand::Play).await.unwrap();
    audio.event(AudioEvent::Ended);
    wait(&player, |s| {
        s.current.as_ref().is_some_and(|t| t.id == "2")
            && s.playback.state == PlaybackState::Playing
    })
    .await;
    audio.event(AudioEvent::Ended);
    wait(&player, |s| s.playback.state == PlaybackState::Stopped).await;
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn unavailable_queue_stops_even_with_repeat_all() {
    let resolver = Arc::new(Resolver {
        calls: AtomicUsize::new(0),
        fail: true,
        slow_first: false,
    });
    let player = PlayerHandle::spawn(FakeAudio::new(), resolver.clone());
    player
        .command(PlayerCommand::Repeat(Repeat::All))
        .await
        .unwrap();
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1"), track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    wait(&player, |s| s.last_error.is_some() && !s.play_when_ready).await;
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 2);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn engine_failure_refreshes_once_then_stops_and_reports_seek() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(Resolver {
        calls: AtomicUsize::new(0),
        fail: false,
        slow_first: false,
    });
    let player = PlayerHandle::spawn(audio.clone(), resolver.clone());
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    let ready = wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    let sought = player
        .command(PlayerCommand::Seek {
            position_ms: 1200,
            selection_id: ready.selection_id,
        })
        .await
        .unwrap();
    assert_eq!(sought.seek_position_ms, 1200);
    assert_eq!(sought.seek_serial, "1");
    audio.event(AudioEvent::Error);
    wait(&player, |s| {
        s.selection_id != sought.selection_id
            && s.playback.state == PlaybackState::Playing
            && s.seek_serial == "2"
            && s.playback.position_ms == 1200
    })
    .await;
    audio.event(AudioEvent::Error);
    wait(&player, |s| s.last_error.is_some() && !s.play_when_ready).await;
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 2);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}

#[tokio::test]
async fn checkpoint_restore_preserves_queue_without_autoplay_then_resumes() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(Resolver {
        calls: AtomicUsize::new(0),
        fail: false,
        slow_first: false,
    });
    let player = PlayerHandle::spawn(audio, resolver.clone());
    let saved = crate::storage::SavedPlayer {
        version: 1,
        tracks: vec![track("1"), track("1")],
        current_index: Some(1),
        repeat: Repeat::All,
        shuffle: true,
        volume: 0.3,
        position_ms: 1100,
    };
    player
        .command(PlayerCommand::Restore(saved.clone()))
        .await
        .unwrap();
    assert_eq!(player.snapshot().playback.state, PlaybackState::Stopped);
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 0);
    assert_eq!(player.checkpoint().await.unwrap(), saved);
    player.command(PlayerCommand::Suspend).await.unwrap();
    assert_eq!(player.checkpoint().await.unwrap(), saved);
    player.command(PlayerCommand::Play).await.unwrap();
    let state = wait(&player, |s| s.seek_position_ms == 1100).await;
    assert_eq!(state.current_index, Some(1));
    assert_eq!(state.playback.volume, 0.3);
    assert_eq!(player.checkpoint().await.unwrap().position_ms, 1100);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
