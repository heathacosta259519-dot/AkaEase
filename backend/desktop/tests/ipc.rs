use akanetease_backend::{
    account::AccountService,
    api::NeteaseClient,
    audio::AudioEngine,
    model::Track,
    player::{PlayerCommand, PlayerHandle, ResolveFuture, SourceResolver},
};
use akanetease_desktop::{Backend, commands, forward_player_events};
use serde_json::{Value, json};
use std::sync::Arc;
use tauri::{Listener, Manager};

struct Unavailable;
impl SourceResolver for Unavailable {
    fn resolve<'a>(&'a self, _: &'a Track) -> ResolveFuture<'a> {
        Box::pin(async { Err(akanetease_backend::error::BackendError::Unavailable) })
    }
}
fn invoke(
    window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    body: Value,
) -> std::result::Result<Value, Value> {
    invoke_from(window, cmd, body, "tauri://localhost")
}
fn invoke_from(
    window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    body: Value,
    origin: &str,
) -> std::result::Result<Value, Value> {
    tauri::test::get_ipc_response(
        window,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: origin.parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
}
#[test]
fn registered_ipc_changes_actor_and_publishes_versioned_events() {
    let player = tauri::async_runtime::block_on(async {
        PlayerHandle::spawn(AudioEngine::silent().unwrap(), Arc::new(Unavailable))
    });
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let app = commands(tauri::test::mock_builder())
        .build(context)
        .unwrap();
    let root = std::env::temp_dir().join(format!(
        "aka-ipc-{:032x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let paths = akanetease_backend::storage::AppPaths {
        config_dir: root.join("config"),
        state_dir: root.join("state"),
        cache_dir: root.join("cache"),
    };
    let persistence = Arc::new(
        akanetease_desktop::persistence::Persistence::new(paths, Default::default()).unwrap(),
    );
    app.manage(Backend {
        account: Arc::new(AccountService::ephemeral()),
        music: NeteaseClient::new().unwrap(),
        player: player.clone(),
        persistence: persistence.clone(),
    });
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    app.listen("player-state", move |event| {
        tx.send(event.payload().to_string()).unwrap();
    });
    let events = forward_player_events(app.handle().clone(), player.subscribe());
    let empty = invoke(&window, "player_snapshot", json!({})).unwrap();
    assert_eq!(empty["queueLength"], 0);
    let other = tauri::WebviewWindowBuilder::new(&app, "other", Default::default())
        .build()
        .unwrap();
    assert!(invoke(&other, "player_snapshot", json!({})).is_err());
    assert!(invoke_from(&window, "player_snapshot", json!({}), "https://example.com").is_err());
    let changed = invoke(&window, "player_volume", json!({"volume":0.4})).unwrap();
    assert!((changed["playback"]["volume"].as_f64().unwrap() - 0.4).abs() < 0.000001);
    let event: Value =
        serde_json::from_str(&rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap()).unwrap();
    assert_eq!(event["sequence"], changed["sequence"]);
    assert_eq!(
        invoke(&window, "player_volume", json!({"volume":2})).unwrap_err()["code"],
        "invalid_input"
    );
    let track = json!({"id":"1","title":"Synthetic","artists":[],"album":{"id":"1","name":"Test","coverUrl":null},"durationMs":2000});
    let replaced = invoke(
        &window,
        "player_replace",
        json!({"tracks":[track],"selected":0,"autoplay":false}),
    )
    .unwrap();
    assert_eq!(
        invoke(&window, "player_queue", json!({})).unwrap()["revision"],
        replaced["queueRevision"]
    );
    assert_eq!(
        invoke(&window, "player_select", json!({"index":0,"revision":"0"})).unwrap_err()["code"],
        "stale_operation"
    );
    assert_eq!(
        invoke(
            &window,
            "player_seek",
            json!({"positionMs":1,"selectionId":"0"})
        )
        .unwrap_err()["code"],
        "stale_operation"
    );
    assert_eq!(
        invoke(&window, "session_snapshot", json!({})).unwrap()["profile"],
        Value::Null
    );
    assert_eq!(
        invoke(&window, "liked_tracks", json!({})).unwrap_err()["code"],
        "unauthorized"
    );
    assert_eq!(
        invoke(&window, "login_cancel", json!({"attemptId":"old"})).unwrap_err()["code"],
        "stale_operation"
    );
    assert_eq!(
        invoke(
            &window,
            "music_search",
            json!({"query":"","offset":0,"limit":20})
        )
        .unwrap_err()["code"],
        "invalid_input"
    );
    assert_eq!(
        invoke(&window, "backend_status", json!({})).unwrap()["persistence"],
        "ready"
    );
    let mut config = invoke(&window, "config_get", json!({})).unwrap();
    config["proxy"] = json!({"mode":"direct"});
    invoke(&window, "config_set", json!({"config":config})).unwrap();
    assert_eq!(
        invoke(&window, "config_get", json!({})).unwrap()["proxy"]["mode"],
        "direct"
    );
    assert_eq!(
        invoke(&window, "cache_stats", json!({})).unwrap()["entries"],
        0
    );
    invoke(&window, "cache_clear", json!({})).unwrap();
    tauri::async_runtime::block_on(persistence.save(&player)).unwrap();
    assert_eq!(
        akanetease_backend::storage::load_player(&persistence.paths)
            .unwrap()
            .unwrap()
            .tracks
            .len(),
        1
    );
    tauri::async_runtime::block_on(player.command(PlayerCommand::Shutdown)).unwrap();
    events.abort();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn persistence_restores_stopped_and_preserves_corrupt_file() {
    let root = std::env::temp_dir().join(format!(
        "aka-persistence-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let paths = akanetease_backend::storage::AppPaths {
        config_dir: root.join("config"),
        state_dir: root.join("state"),
        cache_dir: root.join("cache"),
    };
    let saved = akanetease_backend::storage::SavedPlayer {
        version: 1,
        tracks: vec![],
        current_index: None,
        repeat: akanetease_backend::queue::Repeat::All,
        shuffle: true,
        volume: 0.25,
        position_ms: 0,
    };
    akanetease_backend::storage::save_player(&paths, &saved).unwrap();
    tauri::async_runtime::block_on(async {
        let player = PlayerHandle::spawn(AudioEngine::silent().unwrap(), Arc::new(Unavailable));
        let persistence =
            akanetease_desktop::persistence::Persistence::new(paths.clone(), Default::default())
                .unwrap();
        persistence.restore(&player).await;
        assert!(persistence.status.lock().unwrap().queue_restored);
        assert_eq!(player.checkpoint().await.unwrap(), saved);
        player.command(PlayerCommand::Shutdown).await.unwrap();
        std::fs::write(paths.player_file(), b"corrupt").unwrap();
        let player = PlayerHandle::spawn(AudioEngine::silent().unwrap(), Arc::new(Unavailable));
        let persistence =
            akanetease_desktop::persistence::Persistence::new(paths.clone(), Default::default())
                .unwrap();
        persistence.restore(&player).await;
        assert_eq!(persistence.status.lock().unwrap().persistence, "degraded");
        assert!(persistence.save(&player).await.is_err());
        assert_eq!(std::fs::read(paths.player_file()).unwrap(), b"corrupt");
        player.command(PlayerCommand::Shutdown).await.unwrap();
    });
    std::fs::remove_dir_all(root).unwrap();
}
