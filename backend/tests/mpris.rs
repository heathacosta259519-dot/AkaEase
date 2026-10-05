#![cfg(feature = "mpris")]
mod support;
use akanetease_backend::{audio::PlaybackState, mpris::MprisBridge, player::PlayerCommand};
use futures_util::StreamExt;
use mpris_server::{TrackId, zbus};
#[tokio::test]
#[ignore = "run on an isolated session bus using dbus-run-session"]
async fn private_bus_controls_real_player_and_emits_seeked() {
    let fixture = support::Fixture::new(4);
    let player = fixture.player();
    player
        .command(PlayerCommand::Replace {
            tracks: vec![support::track("1"), support::track("2")],
            selected: 0,
            autoplay: false,
        })
        .await
        .unwrap();
    let name = format!("AkaNeteaseTest.p{}", std::process::id());
    let bridge = MprisBridge::named(player.clone(), &name).await.unwrap();
    let connection = zbus::Connection::session().await.unwrap();
    let bus = format!("org.mpris.MediaPlayer2.{name}");
    let proxy = zbus::Proxy::new(
        &connection,
        bus.as_str(),
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
    )
    .await
    .unwrap();
    proxy.call::<_, _, ()>("Play", &()).await.unwrap();
    support::wait(&player, |s| {
        s.playback.state == PlaybackState::Playing && s.can_seek
    })
    .await;
    proxy.call::<_, _, ()>("Pause", &()).await.unwrap();
    support::wait(&player, |s| s.playback.state == PlaybackState::Paused).await;
    proxy.set_property("Volume", 0.35f64).await.unwrap();
    assert!((player.snapshot().playback.volume - 0.35).abs() < 0.000001);
    proxy.set_property("LoopStatus", "Playlist").await.unwrap();
    assert_eq!(
        player.snapshot().repeat,
        akanetease_backend::queue::Repeat::All
    );
    let mut old: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
        proxy.get_property("Metadata").await.unwrap();
    let old_id: zbus::zvariant::OwnedObjectPath =
        old.remove("mpris:trackid").unwrap().try_into().unwrap();
    proxy.call::<_, _, ()>("Next", &()).await.unwrap();
    support::wait(&player, |s| {
        s.current_index == Some(1) && s.playback.state == PlaybackState::Playing && s.can_seek
    })
    .await;
    proxy.call::<_, _, ()>("Pause", &()).await.unwrap();
    support::wait(&player, |s| s.playback.state == PlaybackState::Paused).await;
    let serial = player.snapshot().seek_serial;
    proxy
        .call::<_, _, ()>("SetPosition", &(old_id, 1_500_000i64))
        .await
        .unwrap();
    assert_eq!(serial, player.snapshot().seek_serial);
    let state = player.snapshot();
    let id = TrackId::try_from(format!("/io/akanetease/selection/{}", state.selection_id)).unwrap();
    let mut signals = proxy.receive_signal("Seeked").await.unwrap();
    proxy
        .call::<_, _, ()>("SetPosition", &(id, 1_500_000i64))
        .await
        .unwrap();
    let signal = tokio::time::timeout(std::time::Duration::from_secs(3), signals.next())
        .await
        .unwrap()
        .unwrap();
    let (micros,): (i64,) = signal.body().deserialize().unwrap();
    assert_eq!(micros, 1_500_000);
    assert_eq!(player.snapshot().seek_position_ms, 1500);
    proxy.call::<_, _, ()>("Stop", &()).await.unwrap();
    assert_eq!(player.snapshot().playback.state, PlaybackState::Stopped);
    drop(bridge);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
