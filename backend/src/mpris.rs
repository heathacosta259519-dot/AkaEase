use crate::{
    audio::PlaybackState,
    player::{PlayerCommand, PlayerHandle, PlayerSnapshot},
    queue::Repeat,
};
use mpris_server::{
    LoopStatus, Metadata, PlaybackStatus, PlayerInterface, Property, RootInterface, Server, Signal,
    Time, TrackId, zbus, zbus::fdo,
};
use std::sync::Arc;
use tokio::task::JoinHandle;

pub const BUS_NAME: &str = "org.mpris.MediaPlayer2.AkaNetease";
pub struct MprisBridge {
    task: JoinHandle<()>,
}
impl Drop for MprisBridge {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub struct Endpoint {
    player: PlayerHandle,
}
fn error(e: crate::error::BackendError) -> fdo::Error {
    fdo::Error::Failed(e.to_string())
}
fn status(s: &PlayerSnapshot) -> PlaybackStatus {
    match s.playback.state {
        PlaybackState::Playing => PlaybackStatus::Playing,
        PlaybackState::Paused => PlaybackStatus::Paused,
        _ => PlaybackStatus::Stopped,
    }
}
fn loop_status(s: &PlayerSnapshot) -> LoopStatus {
    match s.repeat {
        Repeat::Off => LoopStatus::None,
        Repeat::One => LoopStatus::Track,
        Repeat::All => LoopStatus::Playlist,
    }
}
fn track_id(s: &PlayerSnapshot) -> TrackId {
    if s.current.is_some() {
        TrackId::try_from(format!("/io/akanetease/selection/{}", s.selection_id))
            .expect("numeric selection")
    } else {
        TrackId::NO_TRACK
    }
}
fn time(ms: u64) -> Time {
    Time::from_micros(ms.saturating_mul(1000).min(i64::MAX as u64) as i64)
}
fn metadata(s: &PlayerSnapshot) -> Metadata {
    let mut data = Metadata::new();
    data.set_trackid(Some(track_id(s)));
    if let Some(track) = &s.current {
        data.set_title(Some(track.title.clone()));
        data.set_album(Some(track.album.name.clone()));
        data.set_artist(Some(
            track
                .artists
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>(),
        ));
        data.set_art_url(track.album.cover_url.clone());
        data.set_length(Some(time(
            s.playback.duration_ms.unwrap_or(track.duration_ms),
        )));
    }
    data
}
impl Endpoint {
    async fn command(&self, c: PlayerCommand) -> fdo::Result<()> {
        self.player.command(c).await.map(|_| ()).map_err(error)
    }
}

impl MprisBridge {
    pub async fn start(player: PlayerHandle) -> zbus::Result<Self> {
        Self::named(player, "AkaNetease").await
    }
    pub async fn named(player: PlayerHandle, name: &str) -> zbus::Result<Self> {
        let server = Arc::new(
            Server::new(
                name,
                Endpoint {
                    player: player.clone(),
                },
            )
            .await?,
        );
        let mut state = player.subscribe();
        let task = tokio::spawn(async move {
            let mut previous = state.borrow().clone();
            while state.changed().await.is_ok() {
                let now = state.borrow_and_update().clone();
                let mut properties = Vec::new();
                if status(&now) != status(&previous) {
                    properties.push(Property::PlaybackStatus(status(&now)));
                }
                if now.current != previous.current
                    || now.selection_id != previous.selection_id
                    || now.playback.duration_ms != previous.playback.duration_ms
                {
                    properties.push(Property::Metadata(metadata(&now)));
                }
                if now.repeat != previous.repeat {
                    properties.push(Property::LoopStatus(loop_status(&now)));
                }
                if now.shuffle != previous.shuffle {
                    properties.push(Property::Shuffle(now.shuffle));
                }
                if now.playback.volume != previous.playback.volume {
                    properties.push(Property::Volume(now.playback.volume));
                }
                if now.can_next != previous.can_next {
                    properties.push(Property::CanGoNext(now.can_next));
                }
                if now.can_previous != previous.can_previous {
                    properties.push(Property::CanGoPrevious(now.can_previous));
                }
                if now.can_seek != previous.can_seek {
                    properties.push(Property::CanSeek(now.can_seek));
                }
                if now.current.is_some() != previous.current.is_some() {
                    properties.push(Property::CanPlay(now.current.is_some()));
                    properties.push(Property::CanPause(now.current.is_some()));
                }
                if !properties.is_empty() && server.properties_changed(properties).await.is_err() {
                    break;
                }
                if now.seek_serial != previous.seek_serial
                    && server
                        .emit(Signal::Seeked {
                            position: time(now.seek_position_ms),
                        })
                        .await
                        .is_err()
                {
                    break;
                }
                previous = now;
            }
        });
        Ok(Self { task })
    }
}
impl RootInterface for Endpoint {
    async fn raise(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("use desktop window".into()))
    }
    async fn quit(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("use desktop window".into()))
    }
    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn set_fullscreen(&self, _: bool) -> zbus::Result<()> {
        Err(fdo::Error::NotSupported("audio client".into()).into())
    }
    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn identity(&self) -> fdo::Result<String> {
        Ok("AkaNetease-desktop".into())
    }
    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("io.akanetease.desktop".into())
    }
    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
}
impl PlayerInterface for Endpoint {
    async fn next(&self) -> fdo::Result<()> {
        self.command(PlayerCommand::Next).await
    }
    async fn previous(&self) -> fdo::Result<()> {
        self.command(PlayerCommand::Previous).await
    }
    async fn pause(&self) -> fdo::Result<()> {
        self.command(PlayerCommand::Pause).await
    }
    async fn play_pause(&self) -> fdo::Result<()> {
        self.command(PlayerCommand::Toggle).await
    }
    async fn stop(&self) -> fdo::Result<()> {
        self.command(PlayerCommand::Stop).await
    }
    async fn play(&self) -> fdo::Result<()> {
        self.command(PlayerCommand::Play).await
    }
    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        self.command(PlayerCommand::SeekRelative(offset.as_micros() / 1000))
            .await
    }
    async fn set_position(&self, id: TrackId, position: Time) -> fdo::Result<()> {
        let state = self.player.snapshot();
        if id != track_id(&state) || position.as_micros() < 0 {
            return Ok(());
        }
        let ms = position.as_micros() as u64 / 1000;
        if state
            .playback
            .duration_ms
            .is_some_and(|duration| ms > duration)
        {
            return Ok(());
        }
        self.command(PlayerCommand::Seek {
            position_ms: ms,
            selection_id: state.selection_id,
        })
        .await
    }
    async fn open_uri(&self, _: String) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(
            "select a track in the application".into(),
        ))
    }
    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(status(&self.player.snapshot()))
    }
    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(loop_status(&self.player.snapshot()))
    }
    async fn set_loop_status(&self, value: LoopStatus) -> zbus::Result<()> {
        self.command(PlayerCommand::Repeat(match value {
            LoopStatus::None => Repeat::Off,
            LoopStatus::Track => Repeat::One,
            LoopStatus::Playlist => Repeat::All,
        }))
        .await
        .map_err(Into::into)
    }
    async fn rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }
    async fn set_rate(&self, rate: f64) -> zbus::Result<()> {
        if rate == 1.0 {
            Ok(())
        } else {
            Err(fdo::Error::NotSupported("fixed rate".into()).into())
        }
    }
    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.player.snapshot().shuffle)
    }
    async fn set_shuffle(&self, value: bool) -> zbus::Result<()> {
        self.command(PlayerCommand::Shuffle(value))
            .await
            .map_err(Into::into)
    }
    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(metadata(&self.player.snapshot()))
    }
    async fn volume(&self) -> fdo::Result<f64> {
        Ok(self.player.snapshot().playback.volume)
    }
    async fn set_volume(&self, value: f64) -> zbus::Result<()> {
        self.command(PlayerCommand::Volume(value))
            .await
            .map_err(Into::into)
    }
    async fn position(&self) -> fdo::Result<Time> {
        Ok(time(self.player.snapshot().playback.position_ms))
    }
    async fn minimum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }
    async fn maximum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }
    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(self.player.snapshot().can_next)
    }
    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(self.player.snapshot().can_previous)
    }
    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(self.player.snapshot().current.is_some())
    }
    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(self.player.snapshot().current.is_some())
    }
    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(self.player.snapshot().can_seek)
    }
    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
