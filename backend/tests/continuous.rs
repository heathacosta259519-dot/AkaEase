#![cfg(feature = "audio")]
mod support;
use akanetease_backend::{audio::PlaybackState, player::PlayerCommand};
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
