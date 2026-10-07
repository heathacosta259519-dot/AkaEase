#![cfg(feature = "audio")]
mod support;
use akanetease_backend::{audio::PlaybackState, player::PlayerCommand};

#[tokio::test]
async fn gstreamer_quality_reload_seeks_before_resume_and_preserves_paused_state() {
    use akanetease_backend::model::SoundQuality;
    let fixture = support::Fixture::new(5);
    let player = fixture.player();
    player
        .command(PlayerCommand::Replace {
            tracks: vec![support::track("1")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    let ready = support::wait(&player, |s| {
        s.playback.state == PlaybackState::Playing && s.can_seek
    })
    .await;
    player.command(PlayerCommand::Pause).await.unwrap();
    player
        .command(PlayerCommand::Seek {
            position_ms: 1500,
            selection_id: ready.selection_id,
        })
        .await
        .unwrap();
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Lossless))
        .await
        .unwrap();
    let paused = support::wait(&player, |s| {
        !s.resolving && s.seek_serial == "2" && s.playback.state == PlaybackState::Paused
    })
    .await;
    assert_eq!(paused.playback.position_ms, 1500);
    assert!(!paused.play_when_ready);
    assert_eq!(paused.actual_quality, Some(SoundQuality::Standard));
    player.command(PlayerCommand::Play).await.unwrap();
    support::wait(&player, |s| s.playback.state == PlaybackState::Playing).await;
    player
        .command(PlayerCommand::SetQuality(SoundQuality::Hires))
        .await
        .unwrap();
    let playing = support::wait(&player, |s| {
        !s.resolving && s.seek_serial == "3" && s.playback.state == PlaybackState::Playing
    })
    .await;
    assert!(playing.playback.position_ms >= 1500);
    assert!(playing.playback.position_ms < 3000);
    assert_eq!(playing.queue_revision, paused.queue_revision);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn gstreamer_plays_two_tracks_and_stops_at_queue_end() {
    let fixture = support::Fixture::new(1);
    let player = fixture.player();
    player
        .command(PlayerCommand::Replace {
            tracks: vec![support::track("1"), support::track("2")],
            selected: 0,
            autoplay: true,
        })
        .await
        .unwrap();
    support::wait(&player, |s| {
        s.current_index == Some(0) && s.playback.state == PlaybackState::Playing
    })
    .await;
    support::wait(&player, |s| {
        s.current_index == Some(1) && s.playback.state == PlaybackState::Playing
    })
    .await;
    let ended = support::wait(&player, |s| {
        s.current_index == Some(1) && s.playback.state == PlaybackState::Stopped
    })
    .await;
    assert!(!ended.play_when_ready);
    assert!(ended.last_error.is_none());
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
