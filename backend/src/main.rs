use akanetease_backend::{
    account::{AccountService, QrStatus},
    api::NeteaseClient,
    cache::LyricCache,
    error::{BackendError, Result},
    storage::{self, AppPaths},
};
use serde::Serialize;

fn print(value: impl Serialize) {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("serializable backend result")
    );
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" {
        println!(
            "aka-backend login [--memory-only] | session | logout | playlists [offset] [limit] | liked | daily | qr-probe\naka-backend search <query> | track <id> | playlist <id> [offset] | lyrics <id> | stream <id> | play <file-path-or-http-url>\naka-backend config | config-set <json-file> | paths | cache-stats | cache-clear | account-playlist <id>\nAll times are milliseconds; play requires the audio feature. Stop playback with Ctrl+C."
        );
        return Ok(());
    }
    let argument = args.get(1);
    let paths = AppPaths::discover()?;
    let config = if matches!(args[0].as_str(), "config-set" | "paths") {
        Default::default()
    } else {
        storage::load_config(&paths)?
    };
    let client = NeteaseClient::configured(&config.proxy)?;
    match args[0].as_str() {
        "paths" => print(&paths),
        "config" => print(&config),
        "config-set" => {
            let file =
                argument.ok_or_else(|| BackendError::InvalidInput("missing config file".into()))?;
            let mut input = std::fs::File::open(file).map_err(|_| BackendError::Storage)?;
            use std::io::Read;
            let mut bytes = Vec::new();
            (&mut input)
                .take(16385)
                .read_to_end(&mut bytes)
                .map_err(|_| BackendError::Storage)?;
            if bytes.len() > 16384 {
                return Err(BackendError::Storage);
            }
            let config = serde_json::from_slice(&bytes)
                .map_err(|_| BackendError::InvalidInput("invalid configuration".into()))?;
            storage::save_config(&paths, &config)?;
            print(serde_json::json!({"saved":true,"restartRequired":true}));
        }
        "cache-stats" => {
            let _lock = paths.lock()?;
            print(LyricCache::new(&paths, config.cache_limit_bytes)?.stats()?);
        }
        "cache-clear" => {
            let _lock = paths.lock()?;
            LyricCache::new(&paths, config.cache_limit_bytes)?.clear()?;
            print(serde_json::json!({"cleared":true}));
        }
        "account-playlist" => {
            let service = AccountService::configured(config.proxy.clone(), false)?;
            service.restore().await?;
            print(
                service
                    .playlist(
                        argument.ok_or_else(|| {
                            BackendError::InvalidInput("missing playlist ID".into())
                        })?,
                        0,
                        20,
                    )
                    .await?,
            );
        }
        "login" => {
            let service = if argument.is_some_and(|s| s == "--memory-only") {
                AccountService::configured(config.proxy.clone(), true)?
            } else {
                AccountService::configured(config.proxy.clone(), false)?
            };
            interactive_login(&service).await?;
        }
        "qr-probe" => {
            let service = AccountService::configured(config.proxy.clone(), true)?;
            let challenge = service.begin_login().await?;
            let progress = service.poll_login(&challenge.attempt_id).await?;
            print(serde_json::json!({"qrCreated":true,"status":progress.status}));
            service.cancel_login(&challenge.attempt_id).await?;
        }
        "session" => print(
            AccountService::configured(config.proxy.clone(), false)?
                .restore()
                .await?,
        ),
        "logout" => {
            let service = AccountService::configured(config.proxy.clone(), false)?;
            if let Err(error) = service.restore().await {
                eprintln!("Session validation failed; clearing local credentials: {error}");
            }
            let report = service.logout().await;
            let cleared = report.credentials_cleared;
            print(report);
            if !cleared {
                return Err(BackendError::CredentialStorage);
            }
        }
        "playlists" => {
            let service = AccountService::configured(config.proxy.clone(), false)?;
            service.restore().await?;
            let offset = argument
                .map(|v| v.parse::<u32>())
                .transpose()
                .map_err(|_| BackendError::InvalidInput("invalid offset".into()))?
                .unwrap_or(0);
            let limit = args
                .get(2)
                .map(|v| v.parse::<u32>())
                .transpose()
                .map_err(|_| BackendError::InvalidInput("invalid limit".into()))?
                .unwrap_or(20);
            print(service.playlists(offset, limit).await?);
        }
        "liked" => {
            let service = AccountService::configured(config.proxy.clone(), false)?;
            service.restore().await?;
            print(service.liked_tracks().await?);
        }
        "daily" => {
            let service = AccountService::configured(config.proxy.clone(), false)?;
            service.restore().await?;
            print(service.daily_tracks().await?);
        }
        "search" => print(
            client
                .search(
                    argument.ok_or_else(|| BackendError::InvalidInput("missing query".into()))?,
                    0,
                    20,
                )
                .await?,
        ),
        "track" => print(
            client
                .tracks(std::slice::from_ref(argument.ok_or_else(|| {
                    BackendError::InvalidInput("missing track ID".into())
                })?))
                .await?,
        ),
        "playlist" => {
            let argument =
                argument.ok_or_else(|| BackendError::InvalidInput("missing playlist ID".into()))?;
            let offset = args
                .get(2)
                .map(|value| value.parse::<u32>())
                .transpose()
                .map_err(|_| BackendError::InvalidInput("invalid offset".into()))?
                .unwrap_or(0);
            print(client.playlist(argument, offset, 20).await?);
        }
        "lyrics" => print(
            client
                .lyrics(
                    argument
                        .ok_or_else(|| BackendError::InvalidInput("missing track ID".into()))?,
                )
                .await?,
        ),
        "stream" => print(
            client
                .stream(
                    argument
                        .ok_or_else(|| BackendError::InvalidInput("missing track ID".into()))?,
                )
                .await?,
        ),
        #[cfg(feature = "audio")]
        "play" => {
            use akanetease_backend::audio::{AudioEngine, AudioEvent};
            let argument =
                argument.ok_or_else(|| BackendError::InvalidInput("missing audio path".into()))?;
            let uri = if argument.starts_with("https://") || argument.starts_with("http://") {
                argument.to_owned()
            } else {
                let file = std::fs::canonicalize(argument)
                    .map_err(|_| BackendError::InvalidInput("audio file does not exist".into()))?;
                reqwest::Url::from_file_path(file)
                    .map_err(|_| BackendError::InvalidInput("invalid file path".into()))?
                    .to_string()
            };
            let mut player = AudioEngine::configured(&config.proxy)?;
            player.load(&uri)?;
            player.play()?;
            loop {
                for event in player.drain_events()? {
                    print(&event);
                    if event == AudioEvent::Ended {
                        return Ok(());
                    }
                    if event == AudioEvent::Error {
                        return Err(BackendError::Audio("stream failed".into()));
                    }
                }
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {},
                    _ = tokio::signal::ctrl_c() => { player.stop()?; return Ok(()); }
                }
            }
        }
        _ => {
            return Err(BackendError::InvalidInput(
                "unknown command or disabled feature; see --help".into(),
            ));
        }
    }
    Ok(())
}

async fn interactive_login(service: &AccountService) -> Result<()> {
    let challenge = service.begin_login().await?;
    let qr = qrcode::QrCode::new(challenge.qr_url.as_bytes())
        .map_err(|_| BackendError::Protocol("cannot render QR code".into()))?;
    println!(
        "请使用网易云音乐 App 扫码，Ctrl+C 取消。\n{}",
        qr.render::<char>()
            .quiet_zone(true)
            .module_dimensions(2, 1)
            .build()
    );
    let polling = async {
        let mut last = None;
        loop {
            match service.poll_login(&challenge.attempt_id).await {
                Ok(progress) => {
                    if last != Some(progress.status) {
                        print(&progress);
                        last = Some(progress.status);
                    }
                    match progress.status {
                        QrStatus::Authenticated => return Ok(()),
                        QrStatus::Expired => return Err(BackendError::Unauthorized),
                        _ => {}
                    }
                }
                Err(BackendError::Busy) => {}
                Err(BackendError::Network | BackendError::Timeout) => {
                    eprintln!("连接暂时失败，将重试二维码状态。")
                }
                Err(error) => return Err(error),
            }
            tokio::time::sleep(std::time::Duration::from_millis(challenge.poll_interval_ms)).await;
        }
    };
    let result = tokio::select! {
        result=polling=>result,
        _=tokio::signal::ctrl_c()=>{
            let _=service.cancel_login(&challenge.attempt_id).await;
            return Ok(());
        }
    };
    result?;
    print(service.flush_credentials().await);
    // Keep the process alive so a memory-only login is useful without a keyring.
    println!(
        "已登录。输入 status / playlists / playlist <id> / liked / daily / stream <id> / logout / quit。quit 不删除已保存的会话。"
    );
    use tokio::io::AsyncBufReadExt;
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    loop {
        let line = tokio::select! {
            line=lines.next_line()=>line.map_err(|_|BackendError::InvalidInput("cannot read terminal input".into()))?,
            _=tokio::signal::ctrl_c()=>return Ok(()),
        };
        let Some(line) = line else {
            return Ok(());
        };
        let words: Vec<_> = line.split_whitespace().collect();
        let result: Result<()> = async {
            match words.as_slice() {
                ["status"] => print(service.refresh().await?),
                ["playlists"] => print(service.playlists(0, 20).await?),
                ["liked"] => print(service.liked_tracks().await?),
                ["daily"] => print(service.daily_tracks().await?),
                ["stream", id] => print(service.stream(id).await?),
                ["playlist", id] => print(service.playlist(id, 0, 20).await?),
                ["logout"] => {
                    print(service.logout().await);
                }
                ["quit"] | [] => {}
                _ => {
                    return Err(BackendError::InvalidInput(
                        "unknown interactive command".into(),
                    ));
                }
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("{}", serde_json::to_string(&error).unwrap());
        }
        if words.as_slice() == ["quit"] {
            return Ok(());
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }
}
