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
type FakeState = (PlaybackSnapshot, Vec<AudioEvent>, Vec<String>, bool);
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
            false,
        ))))
    }
    fn event(&self, event: AudioEvent) {
        self.0.lock().unwrap().1.push(event);
    }
}
impl AudioDriver for FakeAudio {
    fn load(&mut self, uri: &str) -> Result<()> {
        let mut s = self.0.lock().unwrap();
        s.2.push(uri.into());
        s.0.position_ms = 0;
        s.0.state = PlaybackState::Stopped;
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
        if self.0.lock().unwrap().3 {
            return Err(BackendError::Audio("synthetic seek failure".into()));
        }
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
                quality: Some(SoundQuality::Standard),
                format: Some("mp3".into()),
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

struct QualityResolver(Mutex<Vec<SoundQuality>>);
impl SourceResolver for QualityResolver {
    fn resolve<'a>(&'a self, track: &'a Track) -> ResolveFuture<'a> {
        self.resolve_quality(track, SoundQuality::default())
    }
    fn resolve_quality<'a>(&'a self, track: &'a Track, quality: SoundQuality) -> ResolveFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(quality);
            tokio::time::sleep(Duration::from_millis(150)).await;
            if quality == SoundQuality::Higher {
                return Err(BackendError::Network);
            }
            Ok(StreamSource {
                track_id: track.id.clone(),
                url: format!("https://audio.invalid/{}/{}", track.id, quality.level()),
                bitrate: 128000,
                quality: Some(SoundQuality::Standard),
                format: Some("mp3".into()),
                expires_in_seconds: None,
                is_preview: false,
            })
        })
    }
}

#[tokio::test]
async fn quality_reload_seek_failure_stops_without_advancing_to_another_track() {
    let audio = FakeAudio::new();
    let player = PlayerHandle::spawn(audio.clone(), Arc::new(QualityResolver(Mutex::new(vec![]))));
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1"), track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    let ready = wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    player
        .command(PlayerCommand::Seek {
            position_ms: 900,
            selection_id: ready.selection_id,
        })
        .await
        .unwrap();
    audio.0.lock().unwrap().3 = true;
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Lossless))
        .await
        .unwrap();
    let stopped = wait(&player, |s| {
        s.last_error.is_some() && s.playback.state == PlaybackState::Stopped
    })
    .await;
    assert_eq!(stopped.current_index, Some(0));
    assert!(!stopped.play_when_ready);
    assert_eq!(stopped.actual_quality, None);
    assert_eq!(audio.0.lock().unwrap().2.len(), 2);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}

#[tokio::test]
async fn quality_switch_preserves_pause_seek_queue_and_reports_downgrade() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(QualityResolver(Mutex::new(vec![])));
    let player =
        PlayerHandle::spawn_with_quality(audio.clone(), resolver.clone(), SoundQuality::Hires);
    assert_eq!(player.snapshot().target_quality, SoundQuality::Hires);
    assert_eq!(player.snapshot().actual_quality, None);
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Standard))
        .await
        .unwrap();
    assert!(resolver.0.lock().unwrap().is_empty());
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1"), track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    let ready = wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    player.command(PlayerCommand::Pause).await.unwrap();
    player
        .command(PlayerCommand::Seek {
            position_ms: 1200,
            selection_id: ready.selection_id.clone(),
        })
        .await
        .unwrap();
    let pending = player
        .command(PlayerCommand::SetQuality(SoundQuality::Lossless))
        .await
        .unwrap();
    assert!(pending.resolving);
    assert_eq!(pending.playback.state, PlaybackState::Paused);
    assert_eq!(pending.playback.position_ms, 1200);
    assert_eq!(pending.queue_revision, ready.queue_revision);
    assert_ne!(pending.selection_id, ready.selection_id);
    assert_eq!(audio.0.lock().unwrap().2.len(), 1);
    let switched = wait(&player, |s| {
        !s.resolving && s.seek_serial == "2" && s.playback.state == PlaybackState::Paused
    })
    .await;
    assert_eq!(switched.actual_quality, Some(SoundQuality::Standard));
    assert_eq!(switched.actual_bitrate, Some(128000));
    assert_eq!(switched.format.as_deref(), Some("mp3"));
    assert_eq!(switched.playback.position_ms, 1200);
    assert!(!switched.play_when_ready);
    assert_eq!(switched.current_index, Some(0));
    assert_eq!(
        *resolver.0.lock().unwrap(),
        vec![SoundQuality::Standard, SoundQuality::Lossless]
    );
    player.command(PlayerCommand::Next).await.unwrap();
    wait(&player, |s| {
        s.current_index == Some(1) && s.playback.state == PlaybackState::Playing
    })
    .await;
    assert_eq!(
        resolver.0.lock().unwrap().last(),
        Some(&SoundQuality::Lossless)
    );
    player.command(PlayerCommand::Stop).await.unwrap();
    let stopped = player
        .command(PlayerCommand::SetQuality(SoundQuality::Hires))
        .await
        .unwrap();
    assert_eq!(stopped.actual_quality, None);
    assert!(!stopped.resolving);
    assert_eq!(audio.0.lock().unwrap().2.len(), 3);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}

#[tokio::test]
async fn rapid_quality_changes_follow_latest_intent_and_failure_keeps_old_stream() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(QualityResolver(Mutex::new(vec![])));
    let player = PlayerHandle::spawn(audio.clone(), resolver);
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1"), track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    let ready = wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    player
        .command(PlayerCommand::Seek {
            position_ms: 1100,
            selection_id: ready.selection_id,
        })
        .await
        .unwrap();
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Lossless))
        .await
        .unwrap();
    player.command(PlayerCommand::Pause).await.unwrap();
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Hires))
        .await
        .unwrap();
    player.command(PlayerCommand::Play).await.unwrap();
    let switched = wait(&player, |s| {
        !s.resolving && s.seek_serial == "2" && s.playback.state == PlaybackState::Playing
    })
    .await;
    assert_eq!(switched.playback.position_ms, 1100);
    assert_eq!(
        audio.0.lock().unwrap().2,
        vec![
            "https://audio.invalid/1/exhigh",
            "https://audio.invalid/1/hires"
        ]
    );
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Higher))
        .await
        .unwrap();
    let failed = wait(&player, |s| !s.resolving && s.last_error.is_some()).await;
    assert_eq!(failed.current_index, Some(0));
    assert_eq!(failed.playback.state, PlaybackState::Playing);
    assert_eq!(failed.playback.position_ms, 1100);
    assert_eq!(failed.actual_bitrate, Some(128000));
    assert_eq!(audio.0.lock().unwrap().2.len(), 2);
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Lossless))
        .await
        .unwrap();
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    wait(&player, |s| {
        s.current.as_ref().is_some_and(|t| t.id == "2")
            && s.playback.state == PlaybackState::Playing
    })
    .await;
    assert_eq!(
        audio.0.lock().unwrap().2.last().unwrap(),
        "https://audio.invalid/2/lossless"
    );
    player.command(PlayerCommand::Shutdown).await.unwrap();
}

#[tokio::test]
async fn quality_change_during_restore_resolution_keeps_resume_and_pause() {
    let player = PlayerHandle::spawn(
        FakeAudio::new(),
        Arc::new(QualityResolver(Mutex::new(vec![]))),
    );
    player
        .command(PlayerCommand::Restore(crate::storage::SavedPlayer {
            version: 1,
            tracks: vec![track("1")],
            current_index: Some(0),
            repeat: Repeat::Off,
            shuffle: false,
            volume: 0.3,
            position_ms: 1000,
        }))
        .await
        .unwrap();
    player.command(PlayerCommand::Play).await.unwrap();
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Hires))
        .await
        .unwrap();
    player.command(PlayerCommand::Pause).await.unwrap();
    let state = wait(&player, |s| {
        !s.resolving && s.seek_serial == "1" && s.playback.state == PlaybackState::Paused
    })
    .await;
    assert_eq!(state.playback.position_ms, 1000);
    assert_eq!(state.playback.volume, 0.3);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn expansion_preserves_audio_seek_pause_selection_and_shuffle() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(Resolver {
        calls: AtomicUsize::new(0),
        fail: false,
        slow_first: false,
    });
    let player = PlayerHandle::spawn(audio.clone(), resolver.clone());
    player
        .command(PlayerCommand::Replace {
            tracks: vec![track("57"), track("58")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    let ready = wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    player
        .command(PlayerCommand::Seek {
            position_ms: 1200,
            selection_id: ready.selection_id.clone(),
        })
        .await
        .unwrap();
    player.command(PlayerCommand::Pause).await.unwrap();
    player.command(PlayerCommand::Shuffle(true)).await.unwrap();
    let expanded = player
        .command(PlayerCommand::Expand {
            tracks: (1..=2005).map(|id| track(&id.to_string())).collect(),
            revision: ready.queue_revision.clone(),
        })
        .await
        .unwrap();
    assert_eq!(expanded.current_index, Some(56));
    assert_eq!(expanded.queue_length, 2005);
    assert_eq!(expanded.selection_id, ready.selection_id);
    assert_ne!(expanded.queue_revision, ready.queue_revision);
    assert_eq!(expanded.playback.state, PlaybackState::Paused);
    assert_eq!(expanded.playback.position_ms, 1200);
    assert!(expanded.shuffle);
    assert!(!expanded.play_when_ready);
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
    assert_eq!(audio.0.lock().unwrap().2.len(), 1);
    assert!(matches!(
        player
            .command(PlayerCommand::Expand {
                tracks: vec![track("57")],
                revision: ready.queue_revision,
            })
            .await,
        Err(BackendError::StaleOperation)
    ));
    player.command(PlayerCommand::Shutdown).await.unwrap();
}

#[tokio::test]
async fn expansion_during_resolution_preserves_duplicate_occurrence_and_rejects_invalid_updates() {
    let audio = FakeAudio::new();
    let resolver = Arc::new(Resolver {
        calls: AtomicUsize::new(0),
        fail: false,
        slow_first: true,
    });
    let player = PlayerHandle::spawn(audio.clone(), resolver.clone());
    let initial = player
        .command(PlayerCommand::Replace {
            tracks: vec![track("1"), track("1")],
            selected: 1,
            autoplay: true,
        })
        .await
        .unwrap();
    for tracks in [vec![track("1")], vec![track("0")], vec![track("1"); 10001]] {
        assert!(matches!(
            player
                .command(PlayerCommand::Expand {
                    tracks,
                    revision: initial.queue_revision.clone(),
                })
                .await,
            Err(BackendError::InvalidInput(_))
        ));
        assert_eq!(player.snapshot().queue_revision, initial.queue_revision);
    }
    let expanded = player
        .command(PlayerCommand::Expand {
            tracks: vec![track("2"), track("1"), track("3"), track("1"), track("4")],
            revision: initial.queue_revision,
        })
        .await
        .unwrap();
    assert!(expanded.resolving);
    assert_eq!(expanded.current_index, Some(3));
    assert_eq!(expanded.selection_id, initial.selection_id);
    wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
    assert_eq!(audio.0.lock().unwrap().2.len(), 1);
    audio.event(AudioEvent::Ended);
    wait(&player, |s| {
        s.current.as_ref().is_some_and(|t| t.id == "4")
            && s.playback.state == PlaybackState::Playing
    })
    .await;
    player.command(PlayerCommand::Shutdown).await.unwrap();
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
