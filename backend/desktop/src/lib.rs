#![forbid(unsafe_code)]
pub mod persistence;
use akanetease_backend::{
    account::{
        AccountService, LogoutReport, QrChallenge, QrProgress, QrStatus, SessionSnapshot,
        UserPlaylist, UserPlaylists,
    },
    api::NeteaseClient,
    audio::AudioEngine,
    error::Result,
    lyrics::LyricLine,
    model::{Page, PlaylistPage, Track},
    player::{NeteaseResolver, PlayerCommand, PlayerHandle, PlayerQueue, PlayerSnapshot},
    queue::Repeat,
};
use akanetease_backend::{
    cache::CacheStats,
    diagnostics::{BackendStatus, Event},
    storage::{self, AppConfig, AppPaths},
};
use persistence::Persistence;
use std::sync::Arc;
use tauri::{Emitter, Manager, Runtime, State};

pub struct Backend {
    pub account: Arc<AccountService>,
    pub music: NeteaseClient,
    pub player: PlayerHandle,
    pub persistence: Arc<Persistence>,
}
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    pub sequence: String,
    pub session: SessionSnapshot,
}

#[tauri::command]
async fn music_search(
    state: State<'_, Backend>,
    query: String,
    offset: u32,
    limit: u32,
) -> Result<Page<Track>> {
    state.music.search(&query, offset, limit).await
}
#[tauri::command]
async fn music_tracks(state: State<'_, Backend>, ids: Vec<String>) -> Result<Vec<Track>> {
    state.music.tracks(&ids).await
}
#[tauri::command]
async fn music_artist_detail(
    state: State<'_, Backend>,
    id: String,
) -> Result<akanetease_backend::model::ArtistDetail> {
    state.music.artist_detail(&id).await
}
#[tauri::command]
async fn music_artist_songs(state: State<'_, Backend>, id: String) -> Result<Vec<Track>> {
    state.music.artist_songs(&id).await
}
#[tauri::command]
async fn music_artist_albums(
    state: State<'_, Backend>,
    id: String,
    offset: u32,
    limit: u32,
) -> Result<Page<akanetease_backend::model::AlbumSummary>> {
    state.music.artist_albums(&id, offset, limit).await
}
#[tauri::command]
async fn music_album_detail(
    state: State<'_, Backend>,
    id: String,
) -> Result<akanetease_backend::model::AlbumDetail> {
    state.music.album_detail(&id).await
}
#[tauri::command]
async fn music_playlist(
    state: State<'_, Backend>,
    id: String,
    offset: u32,
    limit: u32,
) -> Result<PlaylistPage> {
    if state.account.snapshot().await.profile.is_some() {
        state.account.playlist(&id, offset, limit).await
    } else {
        state.music.playlist(&id, offset, limit).await
    }
}
#[tauri::command]
async fn music_lyrics(state: State<'_, Backend>, id: String) -> Result<Vec<LyricLine>> {
    let cache = state.persistence.cache.clone();
    if let Some(cache) = cache.clone() {
        let key = id.clone();
        match tokio::task::spawn_blocking(move || cache.get(&key)).await {
            Ok(Ok(Some(lines))) => return Ok(lines),
            Ok(Err(_)) | Err(_) => state.persistence.record(Event::CacheFailed),
            _ => {}
        }
    }
    let lines = state.music.lyrics(&id).await?;
    if let Some(cache) = cache {
        let saved = lines.clone();
        if !matches!(
            tokio::task::spawn_blocking(move || cache.put(&id, saved)).await,
            Ok(Ok(()))
        ) {
            state.persistence.record(Event::CacheFailed);
        }
    }
    Ok(lines)
}
#[tauri::command]
async fn login_begin(state: State<'_, Backend>) -> Result<QrChallenge> {
    state.player.command(PlayerCommand::Suspend).await?;
    let result = state.account.begin_login().await;
    state.player.command(PlayerCommand::Suspend).await?;
    result
}
#[tauri::command]
async fn login_poll(state: State<'_, Backend>, attempt_id: String) -> Result<QrProgress> {
    let result = state.account.poll_login(&attempt_id).await?;
    if result.status == QrStatus::Authenticated {
        state.player.command(PlayerCommand::Suspend).await?;
    }
    Ok(result)
}
#[tauri::command]
async fn login_cancel(state: State<'_, Backend>, attempt_id: String) -> Result<()> {
    state.account.cancel_login(&attempt_id).await
}
#[tauri::command]
async fn session_restore(state: State<'_, Backend>) -> Result<SessionSnapshot> {
    state.player.command(PlayerCommand::Suspend).await?;
    let result = state.account.restore().await;
    state.player.command(PlayerCommand::Suspend).await?;
    result
}
#[tauri::command]
async fn session_snapshot(state: State<'_, Backend>) -> Result<SessionSnapshot> {
    Ok(state.account.snapshot().await)
}
#[tauri::command]
async fn session_refresh(state: State<'_, Backend>) -> Result<SessionSnapshot> {
    state.account.refresh().await
}
#[tauri::command]
async fn logout(state: State<'_, Backend>) -> Result<LogoutReport> {
    state.player.command(PlayerCommand::Suspend).await?;
    let result = state.account.logout().await;
    state.player.command(PlayerCommand::Suspend).await?;
    Ok(result)
}
#[tauri::command]
async fn user_playlists(
    state: State<'_, Backend>,
    offset: u32,
    limit: u32,
) -> Result<UserPlaylists> {
    state.account.playlists(offset, limit).await
}
#[tauri::command]
async fn liked_tracks(state: State<'_, Backend>) -> Result<Vec<String>> {
    state.account.liked_tracks().await
}
#[tauri::command]
async fn daily_tracks(state: State<'_, Backend>) -> Result<Vec<Track>> {
    state.account.daily_tracks().await
}
#[tauri::command]
async fn track_like(state: State<'_, Backend>, track_id: String, like: bool) -> Result<bool> {
    state.account.track_like(&track_id, like).await
}
#[tauri::command]
async fn playlist_create(
    state: State<'_, Backend>,
    name: String,
    privacy: Option<u32>,
) -> Result<UserPlaylist> {
    state.account.playlist_create(&name, privacy).await
}
#[tauri::command]
async fn playlist_delete(state: State<'_, Backend>, playlist_id: String) -> Result<()> {
    state.account.playlist_delete(&playlist_id).await
}
#[tauri::command]
async fn playlist_tracks_op(
    state: State<'_, Backend>,
    playlist_id: String,
    track_ids: Vec<String>,
    op: String,
) -> Result<usize> {
    state
        .account
        .playlist_tracks_op(&playlist_id, &track_ids, &op)
        .await
}
#[tauri::command]
async fn playlist_subscribe(
    state: State<'_, Backend>,
    playlist_id: String,
    subscribe: bool,
) -> Result<()> {
    state
        .account
        .playlist_subscribe(&playlist_id, subscribe)
        .await
}
#[tauri::command]
fn player_snapshot(state: State<'_, Backend>) -> PlayerSnapshot {
    state.player.snapshot()
}
#[tauri::command]
async fn player_queue(state: State<'_, Backend>) -> Result<PlayerQueue> {
    state.player.queue().await
}
#[tauri::command]
async fn player_replace(
    state: State<'_, Backend>,
    tracks: Vec<Track>,
    selected: usize,
    autoplay: bool,
) -> Result<PlayerSnapshot> {
    state
        .player
        .command(PlayerCommand::Replace {
            tracks,
            selected,
            autoplay,
        })
        .await
}
#[tauri::command]
async fn player_expand(
    state: State<'_, Backend>,
    tracks: Vec<Track>,
    revision: String,
) -> Result<PlayerSnapshot> {
    state
        .player
        .command(PlayerCommand::Expand { tracks, revision })
        .await
}
#[tauri::command]
async fn player_select(
    state: State<'_, Backend>,
    index: usize,
    revision: String,
) -> Result<PlayerSnapshot> {
    state
        .player
        .command(PlayerCommand::Select { index, revision })
        .await
}
#[tauri::command]
async fn player_remove(
    state: State<'_, Backend>,
    index: usize,
    revision: String,
) -> Result<PlayerSnapshot> {
    state
        .player
        .command(PlayerCommand::Remove { index, revision })
        .await
}
#[tauri::command]
async fn player_play(state: State<'_, Backend>) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Play).await
}
#[tauri::command]
async fn player_pause(state: State<'_, Backend>) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Pause).await
}
#[tauri::command]
async fn player_toggle(state: State<'_, Backend>) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Toggle).await
}
#[tauri::command]
async fn player_stop(state: State<'_, Backend>) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Stop).await
}
#[tauri::command]
async fn player_next(state: State<'_, Backend>) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Next).await
}
#[tauri::command]
async fn player_previous(state: State<'_, Backend>) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Previous).await
}
#[tauri::command]
async fn player_seek(
    state: State<'_, Backend>,
    position_ms: u64,
    selection_id: String,
) -> Result<PlayerSnapshot> {
    state
        .player
        .command(PlayerCommand::Seek {
            position_ms,
            selection_id,
        })
        .await
}
#[tauri::command]
async fn player_volume(state: State<'_, Backend>, volume: f64) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Volume(volume)).await
}
#[tauri::command]
async fn player_set_quality(
    state: State<'_, Backend>,
    quality: akanetease_backend::model::SoundQuality,
) -> Result<PlayerSnapshot> {
    state
        .player
        .command(PlayerCommand::SetQuality(quality))
        .await
}
#[tauri::command]
async fn player_repeat(state: State<'_, Backend>, repeat: Repeat) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Repeat(repeat)).await
}
#[tauri::command]
async fn player_shuffle(state: State<'_, Backend>, shuffle: bool) -> Result<PlayerSnapshot> {
    state.player.command(PlayerCommand::Shuffle(shuffle)).await
}

#[tauri::command]
fn backend_status(state: State<'_, Backend>) -> BackendStatus {
    state.persistence.status.lock().unwrap().clone()
}
#[tauri::command]
async fn config_get(state: State<'_, Backend>) -> Result<AppConfig> {
    let paths = state.persistence.paths.clone();
    tokio::task::spawn_blocking(move || storage::load_config(&paths))
        .await
        .unwrap_or(Err(akanetease_backend::error::BackendError::Storage))
}
#[tauri::command]
async fn config_set(state: State<'_, Backend>, config: AppConfig) -> Result<serde_json::Value> {
    let paths = state.persistence.paths.clone();
    tokio::task::spawn_blocking(move || storage::save_config(&paths, &config))
        .await
        .unwrap_or(Err(akanetease_backend::error::BackendError::Storage))?;
    Ok(serde_json::json!({"restartRequired":true}))
}
#[tauri::command]
async fn cache_stats(state: State<'_, Backend>) -> Result<CacheStats> {
    let cache = state
        .persistence
        .cache
        .clone()
        .ok_or(akanetease_backend::error::BackendError::Storage)?;
    tokio::task::spawn_blocking(move || cache.stats())
        .await
        .unwrap_or(Err(akanetease_backend::error::BackendError::Storage))
}
#[tauri::command]
async fn cache_clear(state: State<'_, Backend>) -> Result<()> {
    let cache = state
        .persistence
        .cache
        .clone()
        .ok_or(akanetease_backend::error::BackendError::Storage)?;
    tokio::task::spawn_blocking(move || cache.clear())
        .await
        .unwrap_or(Err(akanetease_backend::error::BackendError::Storage))
}

pub fn commands<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        backend_status,
        config_get,
        config_set,
        cache_stats,
        cache_clear,
        music_search,
        music_tracks,
        music_artist_detail,
        music_artist_songs,
        music_artist_albums,
        music_album_detail,
        music_playlist,
        music_lyrics,
        login_begin,
        login_poll,
        login_cancel,
        session_restore,
        session_snapshot,
        session_refresh,
        logout,
        user_playlists,
        liked_tracks,
        daily_tracks,
        track_like,
        playlist_create,
        playlist_delete,
        playlist_tracks_op,
        playlist_subscribe,
        player_snapshot,
        player_queue,
        player_replace,
        player_expand,
        player_select,
        player_remove,
        player_play,
        player_pause,
        player_toggle,
        player_stop,
        player_next,
        player_previous,
        player_seek,
        player_volume,
        player_set_quality,
        player_repeat,
        player_shuffle
    ])
}

pub fn forward_player_events<R: Runtime>(
    handle: tauri::AppHandle<R>,
    mut events: tokio::sync::watch::Receiver<PlayerSnapshot>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        while events.changed().await.is_ok() {
            let state = events.borrow_and_update().clone();
            let _ = handle.emit("player-state", state);
        }
    })
}

struct SaveTask(tauri::async_runtime::JoinHandle<()>);

pub fn run() {
    let app = commands(tauri::Builder::default())
        .setup(|app| {
            let paths = AppPaths::discover()?;
            let lock = paths.lock()?;
            let config = storage::load_config(&paths)?;
            let persistence = Arc::new(Persistence::new(paths, config.clone())?);
            persistence.record(Event::Started);
            app.manage(lock);
            let account = Arc::new(
                AccountService::configured(config.proxy.clone(), false)?
                    .with_diagnostics(persistence.log.clone()),
            );
            let music = NeteaseClient::configured(&config.proxy)?;
            let player = tauri::async_runtime::block_on(async {
                Ok::<_, akanetease_backend::error::BackendError>(PlayerHandle::spawn_with_quality(
                    AudioEngine::configured(&config.proxy)?,
                    Arc::new(NeteaseResolver {
                        account: account.clone(),
                        anonymous: music.clone(),
                    }),
                    config.default_quality,
                ))
            })?;
            tauri::async_runtime::block_on(persistence.restore(&player));
            app.manage(Backend {
                account: account.clone(),
                music,
                player: player.clone(),
                persistence: persistence.clone(),
            });
            let persistence_for_save = persistence.clone();
            let player_for_save = player.clone();
            let saver = tauri::async_runtime::spawn(async move {
                let mut previous_error = None;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    let error = player_for_save.snapshot().last_error;
                    if error.is_some() && error != previous_error {
                        persistence_for_save.record(Event::PlayerError);
                    }
                    previous_error = error;
                    let _ = persistence_for_save.save(&player_for_save).await;
                }
            });
            app.manage(SaveTask(saver));
            let signals = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(mut term) =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                {
                    tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
                    signals.exit(0);
                }
            });
            let handle = app.handle().clone();
            let _events = forward_player_events(handle, player.subscribe());
            let handle = app.handle().clone();
            let player_for_session = player.clone();
            tauri::async_runtime::spawn(async move {
                let mut previous = account.snapshot().await;
                let mut sequence = 0u64;
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    let current = account.snapshot().await;
                    if current != previous {
                        if current.profile != previous.profile {
                            let _ = player_for_session.command(PlayerCommand::Suspend).await;
                        }
                        sequence += 1;
                        let _ = handle.emit(
                            "session-state",
                            SessionEvent {
                                sequence: sequence.to_string(),
                                session: current.clone(),
                            },
                        );
                        previous = current;
                    }
                }
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match akanetease_backend::mpris::MprisBridge::start(player).await {
                    Ok(bridge) => {
                        persistence.status.lock().unwrap().mpris = "ready".into();
                        persistence.record(Event::MprisReady);
                        handle.manage(bridge);
                    }
                    Err(_) => {
                        persistence.status.lock().unwrap().mpris = "unavailable".into();
                        persistence.record(Event::MprisUnavailable);
                        let _ = handle.emit("backend-warning", "mpris_unavailable");
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("initialize desktop host");
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit)
            && let Some(state) = handle.try_state::<Backend>()
        {
            if let Some(task) = handle.try_state::<SaveTask>() {
                task.0.abort();
            }
            tauri::async_runtime::block_on(async {
                let _ = state.persistence.save(&state.player).await;
                let _ = state.player.command(PlayerCommand::Shutdown).await;
                let _ = state.account.flush_credentials().await;
            });
            state.persistence.record(Event::Stopped);
        }
    });
}
