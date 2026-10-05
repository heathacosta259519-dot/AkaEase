//! Real WebKit/IPC acceptance using the delivered dist and isolated, synthetic data.
//! This binary is feature gated and never included in the application package.
use akanetease_backend::{
    account::AccountService,
    api::NeteaseClient,
    audio::AudioEngine,
    lyrics::LyricLine,
    model::{StreamSource, Track},
    player::{PlayerCommand, PlayerHandle, ResolveFuture, SourceResolver},
    storage::{AppConfig, AppPaths},
};
use akanetease_desktop::{Backend, commands, forward_player_events, persistence::Persistence};
use std::{path::PathBuf, sync::Arc};
use tauri::{Listener, Manager};
struct LocalAudio(String);
impl SourceResolver for LocalAudio {
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
fn main() {
    let root = PathBuf::from(
        std::env::var_os("AKA_WEBVIEW_CHECK_DIR")
            .expect("runner must set a temporary check directory"),
    );
    let samples = 8000u32 * 90;
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
    let audio = root.join("synthetic.wav");
    std::fs::write(&audio, wav).unwrap();
    let audio_url = format!("file://{}", audio.display());
    let report = root.join("result.json");
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let app = commands(tauri::Builder::default())
        .on_page_load(|webview, event| {
            if event.event() == tauri::webview::PageLoadEvent::Finished {
                webview
                    .eval(include_str!("../../checks/webview.js"))
                    .expect("run WebView checks");
            }
        })
        .setup(move |app| {
            let persistence = Arc::new(Persistence::new(
                AppPaths {
                    config_dir: root.join("config"),
                    state_dir: root.join("state"),
                    cache_dir: root.join("cache"),
                },
                AppConfig::default(),
            )?);
            let cache = persistence.cache.as_ref().unwrap();
            for id in ["1", "2"] {
                cache.put(
                    id,
                    vec![LyricLine {
                        time_ms: 0,
                        text: "Synthetic lyric for WebView check".into(),
                        translation: None,
                    }],
                )?;
            }
            let player = tauri::async_runtime::block_on(async {
                PlayerHandle::spawn(
                    AudioEngine::silent().unwrap(),
                    Arc::new(LocalAudio(audio_url)),
                )
            });
            app.manage(Backend {
                account: Arc::new(AccountService::ephemeral()),
                music: NeteaseClient::new()?,
                player: player.clone(),
                persistence,
            });
            forward_player_events(app.handle().clone(), player.subscribe());
            let handle = app.handle().clone();
            app.listen("webview-check-result", move |event| {
                let value: serde_json::Value =
                    serde_json::from_str(event.payload()).expect("check report JSON");
                std::fs::write(&report, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
                handle.exit(if value["ok"] == true { 0 } else { 1 });
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(50)).await;
                eprintln!("WebView acceptance timed out (UI/IPC may not have loaded)");
                handle.exit(2);
            });
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                .title("AkaNetease — isolated WebView check")
                .inner_size(1120.0, 760.0)
                .build()?;
            Ok(())
        })
        .build(context)
        .expect("create actual WebKit host");
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let state = handle.state::<Backend>();
            let _ = tauri::async_runtime::block_on(state.player.command(PlayerCommand::Shutdown));
        }
    });
}
