use super::*;
use crate::session::{SessionCookies, StoredSession};
use reqwest::{Client, Url};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Mutex as StdMutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};
use zeroize::Zeroizing;

#[derive(Default)]
struct MemoryStore {
    secret: StdMutex<Option<Vec<u8>>>,
    fail_save: AtomicBool,
    fail_clear: AtomicBool,
    fail_load: AtomicBool,
    saves: AtomicUsize,
    hold_load: StdMutex<Option<StoreHold>>,
    hold_save: StdMutex<Option<StoreHold>>,
    hold_clear: StdMutex<Option<StoreHold>>,
}
type StoreHold = (Arc<Notify>, Arc<Notify>);
fn store_hold() -> StoreHold {
    (Arc::new(Notify::new()), Arc::new(Notify::new()))
}
async fn responsive<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(1), future)
        .await
        .expect("account operation blocked on held keyring")
}
async fn pause_store(hold: &StdMutex<Option<StoreHold>>) {
    let pause = hold.lock().unwrap().take();
    if let Some((entered, release)) = pause {
        entered.notify_one();
        release.notified().await;
    }
}
impl CredentialStore for MemoryStore {
    fn load(&self) -> crate::credentials::StoreFuture<'_, Option<StoredSession>> {
        Box::pin(async move {
            pause_store(&self.hold_load).await;
            if self.fail_load.load(Ordering::SeqCst) {
                return Err(BackendError::CredentialStorage);
            }
            Ok(self
                .secret
                .lock()
                .unwrap()
                .clone()
                .map(|s| StoredSession(Zeroizing::new(s))))
        })
    }
    fn save<'a>(&'a self, secret: &'a StoredSession) -> crate::credentials::StoreFuture<'a, ()> {
        Box::pin(async move {
            pause_store(&self.hold_save).await;
            if self.fail_save.load(Ordering::SeqCst) {
                return Err(BackendError::CredentialStorage);
            }
            *self.secret.lock().unwrap() = Some(secret.0.to_vec());
            self.saves.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
    fn clear(&self) -> crate::credentials::StoreFuture<'_, ()> {
        Box::pin(async move {
            pause_store(&self.hold_clear).await;
            if self.fail_clear.load(Ordering::SeqCst) {
                return Err(BackendError::CredentialStorage);
            }
            *self.secret.lock().unwrap() = None;
            Ok(())
        })
    }
}

struct Reply {
    path: &'static str,
    body: Value,
    cookie: Option<&'static str>,
    expect_cookie: Option<&'static str>,
    no_login_cookie: bool,
    hold: Option<(Arc<Notify>, Arc<Notify>)>,
    expect_json: Option<Value>,
}
fn reply(path: &'static str, body: Value) -> Reply {
    Reply {
        path,
        body,
        cookie: None,
        expect_cookie: None,
        no_login_cookie: false,
        hold: None,
        expect_json: None,
    }
}
fn key() -> Reply {
    let mut r = reply(
        "/weapi/login/qrcode/unikey",
        json!({"code":200,"unikey":"synthetic-key"}),
    );
    r.no_login_cookie = true;
    r
}
fn success(cookie: &'static str) -> Reply {
    let mut r = reply("/weapi/login/qrcode/client/login", json!({"code":803}));
    r.cookie = Some(cookie);
    r
}
fn profile(id: u64, cookie: &'static str) -> Reply {
    let mut r = reply(
        "/api/nuser/account/get",
        json!({"code":200,"account":{"id":id},"profile":{"userId":id,"nickname":"Synthetic account","avatarUrl":null}}),
    );
    r.expect_cookie = Some(cookie);
    r
}

struct Server {
    factory: ClientFactory,
    pending: Arc<StdMutex<VecDeque<Reply>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    fn done(&self) {
        assert!(
            self.pending.lock().unwrap().is_empty(),
            "not all expected requests were made"
        );
    }
}

async fn server(replies: Vec<Reply>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let factory: ClientFactory = Arc::new(move |secret| {
        let cookies = Arc::new(match secret {
            Some(secret) => SessionCookies::restore(secret)?,
            None => SessionCookies::default(),
        });
        Ok(NeteaseClient {
            http: Client::builder()
                .no_proxy()
                .cookie_provider(cookies.clone())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            origin: origin.clone(),
            cookies,
        })
    });
    let pending = Arc::new(StdMutex::new(VecDeque::from(replies)));
    let items = pending.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let response = items
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected HTTP request");
            tokio::spawn(async move {
                let mut request = Vec::new();
                loop {
                    let mut buffer = [0u8; 1024];
                    let n = socket.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    request.extend_from_slice(&buffer[..n]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("content-length: ")?.parse::<usize>().ok()
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let text = String::from_utf8_lossy(&request);
                let path = text
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                assert_eq!(path.split('?').next().unwrap(), response.path);
                if let Some(mut expected) = response.expect_json {
                    assert!(text.starts_with("POST "));
                    let body = text.split_once("\r\n\r\n").unwrap().1;
                    expected
                        .as_object_mut()
                        .unwrap()
                        .entry("csrf_token")
                        .or_insert(json!(""));
                    assert_eq!(crate::weapi::decode_test_request(body), expected);
                }
                if response.path.starts_with("/weapi/login/qrcode/") {
                    let body = text.split_once("\r\n\r\n").unwrap().1;
                    assert!(body.contains("params=") && body.contains("encSecKey="));
                    assert!(!body.contains("synthetic-key"));
                    assert!(text.contains("os=pc; appver=2.7.1.198277"));
                    assert!(text.contains("Mozilla/5.0"));
                }
                if response.no_login_cookie {
                    assert!(!text.contains("MUSIC_U="));
                }
                if let Some(expected) = response.expect_cookie {
                    assert!(text.contains(expected), "expected session cookie");
                    if expected.starts_with("__csrf=") {
                        assert!(path.contains("csrf_token=synthetic-session"));
                    }
                }
                if let Some((entered, release)) = response.hold {
                    entered.notify_one();
                    release.notified().await;
                }
                let body = response.body.to_string();
                let header = response
                    .cookie
                    .map(|cookie| format!("Set-Cookie: {cookie}\r\n"))
                    .unwrap_or_default();
                let output = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{header}\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(output.as_bytes()).await;
            });
        }
    });
    Server {
        factory,
        pending,
        task,
    }
}

async fn login(service: &AccountService) -> QrProgress {
    let qr = service.begin_login().await.unwrap();
    let mut progress = service.poll_login(&qr.attempt_id).await.unwrap();
    progress.session = service.flush_credentials().await;
    progress
}

fn library_reply(path: &'static str, body: Value, parameters: Value) -> Reply {
    let mut response = reply(path, body);
    response.expect_cookie = Some("MUSIC_U=library");
    response.expect_json = Some(parameters);
    response
}

fn access_reply(id: u64, owner: u64, special_type: u64) -> Reply {
    let mut response = reply(
        "/api/v6/playlist/detail",
        json!({"code":200,"playlist":{
            "id":id,"creator":{"userId":owner},"specialType":special_type
        }}),
    );
    response.expect_cookie = Some("MUSIC_U=library");
    response
}

fn library_login() -> Vec<Reply> {
    vec![
        key(),
        success("MUSIC_U=library; Path=/"),
        profile(42, "MUSIC_U=library"),
    ]
}

#[tokio::test]
async fn library_writes_use_authenticated_weapi_and_return_typed_results() {
    let big = 9007199254740993_u64;
    let mut replies = library_login();
    replies.extend([
        library_reply("/weapi/radio/like", json!({"code":200}), json!({"alg":"itembased","trackId":big,"like":true,"time":3})),
        library_reply("/weapi/radio/like", json!({"code":200}), json!({"alg":"itembased","trackId":big,"like":false,"time":3})),
        library_reply("/weapi/playlist/create", json!({"code":200,"playlist":{
            "id":big,"name":"测试歌单","coverImgUrl":null,"trackCount":0,"creator":{"userId":42},"specialType":0
        }}), json!({"name":"测试歌单","privacy":10,"type":"NORMAL"})),
        access_reply(big, 42, 0),
        library_reply("/weapi/playlist/manipulate/tracks", json!({"code":200}), json!({"op":"add","pid":big,"trackIds":format!("[{big},7]")})),
        access_reply(big, 42, 0),
        library_reply("/weapi/playlist/manipulate/tracks", json!({"code":200}), json!({"op":"del","pid":big,"trackIds":"[7]"})),
        access_reply(big, 43, 0),
        library_reply("/weapi/playlist/subscribe", json!({"code":200}), json!({"id":big,"t":1})),
        access_reply(big, 43, 0),
        library_reply("/weapi/playlist/subscribe", json!({"code":200}), json!({"id":big,"t":2})),
        access_reply(big, 42, 0),
        library_reply("/weapi/playlist/remove", json!({"code":200}), json!({"ids":format!("[{big}]")})),
    ]);
    let server = server(replies).await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    login(&service).await;
    assert!(service.track_like(&big.to_string(), true).await.unwrap());
    assert!(!service.track_like(&big.to_string(), false).await.unwrap());
    let created = service
        .playlist_create("  测试歌单  ", Some(10))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(created).unwrap(),
        json!({
            "id":big.to_string(),"title":"测试歌单","coverUrl":null,"trackCount":0,
            "ownerId":"42","isCreator":true,"subscribed":false,"isLikedPlaylist":false
        })
    );
    assert_eq!(
        service
            .playlist_tracks_op(
                &big.to_string(),
                &[big.to_string(), "7".into(), "0007".into()],
                "add"
            )
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        service
            .playlist_tracks_op(&big.to_string(), &["7".into()], "del")
            .await
            .unwrap(),
        1
    );
    service
        .playlist_subscribe(&big.to_string(), true)
        .await
        .unwrap();
    service
        .playlist_subscribe(&big.to_string(), false)
        .await
        .unwrap();
    service.playlist_delete(&big.to_string()).await.unwrap();
    server.done();
}

#[tokio::test]
async fn library_writes_preserve_dynamic_csrf_and_create_accepts_the_unicode_name_boundary() {
    let mut creation = key();
    creation.cookie = Some("__csrf=synthetic-session; Path=/");
    let name = "测".repeat(40);
    let mut write = library_reply(
        "/weapi/radio/like",
        json!({"code":200}),
        json!({
            "alg":"itembased","trackId":7,"like":true,"time":3,"csrf_token":"synthetic-session"
        }),
    );
    write.expect_cookie = Some("__csrf=synthetic-session");
    let mut create = library_reply(
        "/weapi/playlist/create",
        json!({"code":200,"playlist":{
            "id":1,"name":name,"coverImgUrl":"https://images.invalid/cover","trackCount":0,"creator":{"userId":42}
        }}),
        json!({"name":name,"privacy":0,"type":"NORMAL","csrf_token":"synthetic-session"}),
    );
    create.expect_cookie = Some("__csrf=synthetic-session");
    let server = server(vec![
        creation,
        success("MUSIC_U=library; Path=/"),
        profile(42, "MUSIC_U=library"),
        write,
        create,
    ])
    .await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    login(&service).await;
    assert!(service.track_like("7", true).await.unwrap());
    let created = service.playlist_create(&name, None).await.unwrap();
    assert_eq!(created.title, name);
    assert_eq!(
        created.cover_url.as_deref(),
        Some("https://images.invalid/cover")
    );
    server.done();
}

#[tokio::test]
async fn library_writes_require_login_and_validate_input_without_network() {
    let server = server(library_login()).await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    assert!(matches!(
        service.track_like("1", true).await,
        Err(BackendError::Unauthorized)
    ));
    assert!(matches!(
        service.playlist_create("name", None).await,
        Err(BackendError::Unauthorized)
    ));
    assert!(matches!(
        service.playlist_delete("1").await,
        Err(BackendError::Unauthorized)
    ));
    assert!(matches!(
        service.playlist_tracks_op("1", &["1".into()], "add").await,
        Err(BackendError::Unauthorized)
    ));
    assert!(matches!(
        service.playlist_subscribe("1", true).await,
        Err(BackendError::Unauthorized)
    ));
    login(&service).await;
    for id in ["", "0", "-1", "abc", "18446744073709551616"] {
        assert!(matches!(
            service.track_like(id, true).await,
            Err(BackendError::InvalidInput(_))
        ));
        assert!(matches!(
            service.playlist_delete(id).await,
            Err(BackendError::InvalidInput(_))
        ));
        assert!(matches!(
            service.playlist_subscribe(id, true).await,
            Err(BackendError::InvalidInput(_))
        ));
        assert!(matches!(
            service.playlist_tracks_op(id, &["1".into()], "add").await,
            Err(BackendError::InvalidInput(_))
        ));
    }
    for name in ["".to_string(), " ".into(), "测".repeat(41), "a\nb".into()] {
        assert!(matches!(
            service.playlist_create(&name, None).await,
            Err(BackendError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        service.playlist_create("name", Some(1)).await,
        Err(BackendError::InvalidInput(_))
    ));
    for (ids, op) in [
        (vec![], "add"),
        (vec!["1".into(); 101], "del"),
        (vec!["0".into()], "add"),
        (vec!["1".into()], "replace"),
    ] {
        assert!(matches!(
            service.playlist_tracks_op("1", &ids, op).await,
            Err(BackendError::InvalidInput(_))
        ));
    }
    server.done();
}

#[tokio::test]
async fn protected_or_foreign_playlists_never_reach_delete_and_track_write_endpoints() {
    let mut replies = library_login();
    replies.extend([
        access_reply(1, 42, 5),
        access_reply(1, 43, 0),
        access_reply(1, 42, 1),
        access_reply(1, 42, 5),
        access_reply(1, 43, 0),
        access_reply(1, 42, 0),
    ]);
    replies.push(reply(
        "/api/v6/playlist/detail",
        json!({"code":200,"playlist":{"id":1,"creator":{"userId":42}}}),
    ));
    replies.push(access_reply(2, 42, 0));
    let server = server(replies).await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    login(&service).await;
    for _ in 0..3 {
        assert!(matches!(
            service.playlist_delete("1").await,
            Err(BackendError::InvalidInput(_))
        ));
    }
    for _ in 0..2 {
        assert!(matches!(
            service.playlist_tracks_op("1", &["7".into()], "add").await,
            Err(BackendError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        service.playlist_subscribe("1", true).await,
        Err(BackendError::InvalidInput(_))
    ));
    assert!(matches!(
        service.playlist_delete("1").await,
        Err(BackendError::Protocol(_))
    ));
    assert!(matches!(
        service.playlist_delete("1").await,
        Err(BackendError::Protocol(_))
    ));
    assert!(service.snapshot().await.profile.is_some());
    server.done();
}

#[tokio::test]
async fn library_service_errors_do_not_report_success_and_only_expiry_clears_session() {
    let mut replies = library_login();
    replies.extend([
        library_reply("/weapi/radio/like",json!({"code":500}),json!({"alg":"itembased","trackId":7,"like":true,"time":3})),
        library_reply("/weapi/playlist/create",json!({"code":200,"playlist":{"id":1,"name":"bad","trackCount":0,"creator":{"userId":99}}}),json!({"name":"name","privacy":0,"type":"NORMAL"})),
        library_reply("/weapi/radio/like",json!({"code":301}),json!({"alg":"itembased","trackId":7,"like":true,"time":3})),
    ]);
    let server = server(replies).await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    login(&service).await;
    assert!(matches!(
        service.track_like("7", true).await,
        Err(BackendError::Service(500))
    ));
    assert!(service.snapshot().await.profile.is_some());
    assert!(matches!(
        service.playlist_create("name", None).await,
        Err(BackendError::Protocol(_))
    ));
    assert!(service.snapshot().await.profile.is_some());
    assert!(matches!(
        service.track_like("7", true).await,
        Err(BackendError::Unauthorized)
    ));
    assert!(service.snapshot().await.profile.is_none());
    server.done();
}

#[tokio::test]
async fn logout_during_permission_check_prevents_playlist_deletion() {
    let (entered, release) = store_hold();
    let mut access = access_reply(1, 42, 0);
    access.hold = Some((entered.clone(), release.clone()));
    let mut replies = library_login();
    replies.extend([access, reply("/api/logout", json!({"code":200}))]);
    let server = server(replies).await;
    let service = Arc::new(AccountService::with_store(
        Arc::new(MemoryStore::default()),
        server.factory.clone(),
    ));
    login(&service).await;
    let worker = service.clone();
    let deletion = tokio::spawn(async move { worker.playlist_delete("1").await });
    entered.notified().await;
    responsive(service.logout()).await;
    release.notify_one();
    assert!(matches!(
        deletion.await.unwrap(),
        Err(BackendError::StaleOperation)
    ));
    server.done();
}

#[tokio::test]
async fn writes_are_serialized_and_queued_operations_cannot_use_an_obsolete_account() {
    let (entered, release) = store_hold();
    let mut like = library_reply(
        "/weapi/radio/like",
        json!({"code":200}),
        json!({"alg":"itembased","trackId":7,"like":true,"time":3}),
    );
    like.hold = Some((entered.clone(), release.clone()));
    let mut replies = library_login();
    replies.extend([like, reply("/api/logout", json!({"code":200}))]);
    let server = server(replies).await;
    let service = Arc::new(AccountService::with_store(
        Arc::new(MemoryStore::default()),
        server.factory.clone(),
    ));
    login(&service).await;
    let worker = service.clone();
    let first = tokio::spawn(async move { worker.track_like("7", true).await });
    entered.notified().await;
    let second = service.track_like("7", false);
    tokio::pin!(second);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut second)
            .await
            .is_err()
    );
    responsive(service.logout()).await;
    release.notify_one();
    assert!(matches!(
        first.await.unwrap(),
        Err(BackendError::StaleOperation)
    ));
    assert!(matches!(second.await, Err(BackendError::StaleOperation)));
    server.done();
}

#[tokio::test]
async fn qr_poll_reuses_creation_cookie_with_desktop_identity() {
    let mut creation = key();
    creation.cookie = Some("__csrf=synthetic-session; Path=/");
    let mut poll = reply("/weapi/login/qrcode/client/login", json!({"code":801}));
    poll.expect_cookie = Some("__csrf=synthetic-session");
    let server = server(vec![creation, poll]).await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    let qr = service.begin_login().await.unwrap();
    assert_eq!(
        service.poll_login(&qr.attempt_id).await.unwrap().status,
        QrStatus::WaitingScan
    );
    server.done();
}

#[tokio::test]
async fn qr_and_snapshot_are_available_while_startup_keyring_lookup_is_held() {
    let server = server(vec![key()]).await;
    let store = Arc::new(MemoryStore::default());
    let (entered, release) = store_hold();
    *store.hold_load.lock().unwrap() = Some((entered.clone(), release.clone()));
    let service = Arc::new(AccountService::with_store(store, server.factory.clone()));
    let worker = service.clone();
    let restore = tokio::spawn(async move { worker.restore().await });
    entered.notified().await;
    assert!(matches!(
        responsive(service.restore()).await,
        Err(BackendError::Busy)
    ));
    let qr = responsive(service.begin_login()).await.unwrap();
    assert!(responsive(service.snapshot()).await.profile.is_none());
    responsive(service.cancel_login(&qr.attempt_id))
        .await
        .unwrap();
    release.notify_one();
    assert!(matches!(
        restore.await.unwrap(),
        Err(BackendError::StaleOperation)
    ));
    service.flush_credentials().await;
    server.done();
}

#[tokio::test]
async fn keyring_cleanup_does_not_delay_qr_or_cancel_and_still_runs_after_cancel() {
    let server = server(vec![key()]).await;
    let store = Arc::new(MemoryStore::default());
    *store.secret.lock().unwrap() = Some(b"synthetic-old-secret".to_vec());
    let (entered, release) = store_hold();
    *store.hold_clear.lock().unwrap() = Some((entered.clone(), release.clone()));
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    let qr = responsive(service.begin_login()).await.unwrap();
    entered.notified().await;
    assert_eq!(
        responsive(service.snapshot()).await.persistence,
        Persistence::CleanupRequired
    );
    responsive(service.cancel_login(&qr.attempt_id))
        .await
        .unwrap();
    release.notify_one();
    assert_eq!(
        service.flush_credentials().await.persistence,
        Persistence::None
    );
    assert!(store.secret.lock().unwrap().is_none());
    server.done();
}

#[tokio::test]
async fn authenticated_reply_precedes_save_and_logout_cleans_a_late_save() {
    let server = server(vec![
        key(),
        success("MUSIC_U=late; Path=/"),
        profile(42, "MUSIC_U=late"),
        reply("/api/logout", json!({"code":200})),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    let (entered, release) = store_hold();
    *store.hold_save.lock().unwrap() = Some((entered.clone(), release.clone()));
    let service = Arc::new(AccountService::with_store(
        store.clone(),
        server.factory.clone(),
    ));
    let qr = service.begin_login().await.unwrap();
    service.flush_credentials().await;
    let progress = responsive(service.poll_login(&qr.attempt_id))
        .await
        .unwrap();
    assert_eq!(progress.status, QrStatus::Authenticated);
    assert_eq!(progress.session.profile.unwrap().id, "42");
    assert_eq!(progress.session.persistence, Persistence::MemoryOnly);
    entered.notified().await;
    assert_eq!(
        responsive(service.snapshot()).await.profile.unwrap().id,
        "42"
    );
    let worker = service.clone();
    let logout = tokio::spawn(async move { worker.logout().await });
    responsive(async {
        while service.snapshot().await.profile.is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    release.notify_one();
    assert!(logout.await.unwrap().credentials_cleared);
    assert!(store.secret.lock().unwrap().is_none());
    assert_eq!(
        service.flush_credentials().await.persistence,
        Persistence::None
    );
    server.done();
}

#[tokio::test]
async fn new_account_wins_over_a_previous_account_save_in_flight() {
    let server = server(vec![
        key(),
        success("MUSIC_U=old; Path=/"),
        profile(1, "MUSIC_U=old"),
        key(),
        success("MUSIC_U=new; Path=/"),
        profile(2, "MUSIC_U=new"),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    let (entered, release) = store_hold();
    *store.hold_save.lock().unwrap() = Some((entered.clone(), release.clone()));
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    let first = service.begin_login().await.unwrap();
    service.poll_login(&first.attempt_id).await.unwrap();
    entered.notified().await;
    let second = responsive(service.begin_login()).await.unwrap();
    assert_eq!(
        responsive(service.poll_login(&second.attempt_id))
            .await
            .unwrap()
            .session
            .profile
            .unwrap()
            .id,
        "2"
    );
    release.notify_one();
    let snapshot = service.flush_credentials().await;
    assert_eq!(snapshot.profile.unwrap().id, "2");
    assert_eq!(snapshot.persistence, Persistence::Secure);
    let secret = store.load().await.unwrap().unwrap();
    let client = (server.factory)(Some(&secret)).unwrap();
    use reqwest::cookie::CookieStore;
    let cookies = client.cookies.cookies(&client.origin).unwrap();
    assert!(cookies.to_str().unwrap().contains("MUSIC_U=new"));
    assert!(!cookies.to_str().unwrap().contains("MUSIC_U=old"));
    server.done();
}

#[tokio::test]
async fn startup_restore_cannot_invalidate_a_qr_creation_in_flight() {
    let (entered, release) = store_hold();
    let mut delayed = key();
    delayed.hold = Some((entered.clone(), release.clone()));
    let server = server(vec![delayed]).await;
    let service = Arc::new(AccountService::with_store(
        Arc::new(MemoryStore::default()),
        server.factory.clone(),
    ));
    let worker = service.clone();
    let begin = tokio::spawn(async move { worker.begin_login().await });
    entered.notified().await;
    assert!(matches!(
        responsive(service.restore()).await,
        Err(BackendError::Busy)
    ));
    assert!(matches!(
        responsive(service.begin_login()).await,
        Err(BackendError::Busy)
    ));
    release.notify_one();
    let qr = begin.await.unwrap().unwrap();
    service.cancel_login(&qr.attempt_id).await.unwrap();
    service.flush_credentials().await;
    server.done();
}

#[tokio::test]
async fn failed_qr_creation_allows_a_fresh_attempt() {
    let server = server(vec![
        reply("/weapi/login/qrcode/unikey", json!({"code":500})),
        key(),
    ])
    .await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    assert!(matches!(
        service.begin_login().await,
        Err(BackendError::Service(500))
    ));
    let qr = service.begin_login().await.unwrap();
    service.cancel_login(&qr.attempt_id).await.unwrap();
    service.flush_credentials().await;
    server.done();
}

#[tokio::test]
async fn authorized_cookie_is_retained_when_profile_validation_needs_retry() {
    let server = server(vec![
        key(),
        success("MUSIC_U=verified; Path=/"),
        reply("/api/nuser/account/get", json!({"code":500})),
        profile(42, "MUSIC_U=verified"),
    ])
    .await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    let qr = service.begin_login().await.unwrap();
    assert!(matches!(
        service.poll_login(&qr.attempt_id).await,
        Err(BackendError::Service(500))
    ));
    assert!(service.snapshot().await.profile.is_none());
    service
        .state
        .lock()
        .await
        .pending
        .as_mut()
        .unwrap()
        .next_poll = Instant::now();
    let progress = service.poll_login(&qr.attempt_id).await.unwrap();
    assert_eq!(progress.status, QrStatus::Authenticated);
    assert_eq!(
        service.flush_credentials().await.persistence,
        Persistence::Secure
    );
    server.done();
}

#[tokio::test]
async fn qr_poll_code_200_requires_cookie_and_verified_identity() {
    let server = server(vec![
        key(),
        reply("/weapi/login/qrcode/client/login", json!({"code": 200})),
        key(),
        Reply {
            cookie: Some("MUSIC_U=verified; Path=/; HttpOnly"),
            ..reply("/weapi/login/qrcode/client/login", json!({"code": 200}))
        },
        profile(42, "MUSIC_U=verified"),
    ])
    .await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    let first = service.begin_login().await.unwrap();
    assert!(matches!(
        service.poll_login(&first.attempt_id).await,
        Err(BackendError::Protocol(_))
    ));
    assert!(service.snapshot().await.profile.is_none());
    let second = service.begin_login().await.unwrap();
    let progress = service.poll_login(&second.attempt_id).await.unwrap();
    assert_eq!(progress.status, QrStatus::Authenticated);
    assert_eq!(progress.session.profile.unwrap().id, "42");
    assert_eq!(
        service.flush_credentials().await.persistence,
        Persistence::Secure
    );
    server.done();
}

#[tokio::test]
async fn login_personal_content_restore_and_logout_keep_cookies_private() {
    let mut playlists = reply(
        "/api/user/playlist",
        json!({"code":200,"more":false,"playlist":[{"id":8,"name":"Synthetic list","trackCount":2,"creator":{"userId":42},"subscribed":false,"specialType":5},{"id":9,"name":"Other creator","trackCount":1,"creator":{"userId":43},"subscribed":true}]}),
    );
    playlists.expect_cookie = Some("MUSIC_U=alpha");
    let mut likes = reply(
        "/api/song/like/get",
        json!({"code":200,"ids":[9007199254740993_u64,7]}),
    );
    likes.expect_cookie = Some("MUSIC_U=alpha");
    let mut daily = reply(
        "/api/v3/discovery/recommend/songs",
        json!({"code":200,"data":{"dailySongs":[{"id":7,"name":"Synthetic","ar":[],"al":{"id":1,"name":"Album"},"dt":1000}]}}),
    );
    daily.expect_cookie = Some("MUSIC_U=alpha");
    let mut stream = reply(
        "/weapi/song/enhance/player/url/v1",
        json!({"code":200,"data":[{"id":7,"code":200,"url":"https://audio.invalid/fixture","br":128000}]}),
    );
    stream.expect_cookie = Some("MUSIC_U=alpha");
    stream.expect_json = Some(json!({"ids":"[7]","level":"hires","encodeType":"flac"}));
    let server = server(vec![
        key(),
        success("MUSIC_U=alpha; Path=/; HttpOnly"),
        profile(42, "MUSIC_U=alpha"),
        playlists,
        likes,
        daily,
        stream,
        profile(42, "MUSIC_U=alpha"),
        reply("/api/logout", json!({"code":200})),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    let progress = login(&service).await;
    assert_eq!(progress.status, QrStatus::Authenticated);
    assert_eq!(progress.session.persistence, Persistence::Secure);
    assert!(!format!("{progress:?}").contains("alpha"));
    assert!(
        !serde_json::to_string(&progress)
            .unwrap()
            .contains("MUSIC_U")
    );
    let lists = service.playlists(0, 20).await.unwrap();
    assert_eq!(lists.items[0].owner_id, "42");
    assert!(lists.items[0].is_creator);
    assert!(lists.items[0].is_liked_playlist);
    assert!(!lists.items[1].is_creator);
    assert_eq!(lists.liked_playlist_id.as_deref(), Some("8"));
    assert_eq!(
        serde_json::to_value(&lists).unwrap()["items"][0]["isCreator"],
        true
    );
    assert_eq!(service.liked_tracks().await.unwrap()[0], "9007199254740993");
    assert_eq!(service.daily_tracks().await.unwrap()[0].id, "7");
    let source = service
        .stream_quality("7", crate::model::SoundQuality::Hires)
        .await
        .unwrap();
    assert_eq!(source.track_id, "7");
    assert_eq!(source.quality, Some(crate::model::SoundQuality::Standard));
    drop(service);
    let restored = AccountService::with_store(store.clone(), server.factory.clone());
    assert_eq!(restored.restore().await.unwrap().profile.unwrap().id, "42");
    let report = restored.logout().await;
    assert!(report.local_cleared && report.credentials_cleared && report.server_revoked);
    assert!(store.secret.lock().unwrap().is_none());
    assert!(matches!(
        restored.daily_tracks().await,
        Err(BackendError::Unauthorized)
    ));
    server.done();
}

#[tokio::test]
async fn authenticated_quality_requests_send_all_levels_with_session_and_csrf() {
    use crate::model::SoundQuality;
    let qualities = [
        SoundQuality::Standard,
        SoundQuality::Higher,
        SoundQuality::Exhigh,
        SoundQuality::Lossless,
        SoundQuality::Hires,
    ];
    let mut replies = vec![
        key(),
        success("MUSIC_U=alpha; Path=/; HttpOnly"),
        profile(42, "MUSIC_U=alpha"),
    ];
    for quality in qualities {
        let mut response = reply(
            "/weapi/song/enhance/player/url/v1",
            json!({"code":200,"data":[{"id":7,"code":200,"url":"https://audio.invalid/synthetic","br":320000,"type":"mp3"}]}),
        );
        response.expect_cookie = Some("MUSIC_U=alpha");
        response.expect_json =
            Some(json!({"ids":"[7]","level":quality.level(),"encodeType":"flac"}));
        replies.push(response);
    }
    let server = server(replies).await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    login(&service).await;
    for quality in qualities {
        let source = service.stream_quality("7", quality).await.unwrap();
        assert_eq!(source.quality, Some(SoundQuality::Exhigh));
    }
    server.done();
}

#[tokio::test]
async fn cancellation_discards_late_success_and_new_login_has_clean_cookies() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut delayed = success("MUSIC_U=old; Path=/");
    delayed.hold = Some((entered.clone(), release.clone()));
    let server = server(vec![key(), delayed, key()]).await;
    let store = Arc::new(MemoryStore::default());
    let service = Arc::new(AccountService::with_store(
        store.clone(),
        server.factory.clone(),
    ));
    let first = service.begin_login().await.unwrap();
    let worker = service.clone();
    let id = first.attempt_id.clone();
    let poll = tokio::spawn(async move { worker.poll_login(&id).await });
    entered.notified().await;
    assert!(matches!(
        service.poll_login(&first.attempt_id).await,
        Err(BackendError::Busy)
    ));
    service.cancel_login(&first.attempt_id).await.unwrap();
    let second = service.begin_login().await.unwrap();
    release.notify_one();
    assert!(matches!(
        poll.await.unwrap(),
        Err(BackendError::StaleOperation)
    ));
    assert!(service.snapshot().await.profile.is_none());
    assert_eq!(store.saves.load(Ordering::SeqCst), 0);
    assert!(matches!(
        service.cancel_login(&first.attempt_id).await,
        Err(BackendError::StaleOperation)
    ));
    service.cancel_login(&second.attempt_id).await.unwrap();
    server.done();
}

#[tokio::test]
async fn waiting_throttle_confirmation_and_expiry_are_explicit() {
    let server = server(vec![
        key(),
        reply("/weapi/login/qrcode/client/login", json!({"code":801})),
        reply("/weapi/login/qrcode/client/login", json!({"code":802})),
        reply("/weapi/login/qrcode/client/login", json!({"code":800})),
        key(),
    ])
    .await;
    let service =
        AccountService::with_store(Arc::new(MemoryStore::default()), server.factory.clone());
    let qr = service.begin_login().await.unwrap();
    assert_eq!(
        service.poll_login(&qr.attempt_id).await.unwrap().status,
        QrStatus::WaitingScan
    );
    assert!(matches!(
        service.poll_login(&qr.attempt_id).await,
        Err(BackendError::Busy)
    ));
    service
        .state
        .lock()
        .await
        .pending
        .as_mut()
        .unwrap()
        .next_poll = Instant::now();
    assert_eq!(
        service.poll_login(&qr.attempt_id).await.unwrap().status,
        QrStatus::WaitingConfirmation
    );
    service
        .state
        .lock()
        .await
        .pending
        .as_mut()
        .unwrap()
        .next_poll = Instant::now();
    assert_eq!(
        service.poll_login(&qr.attempt_id).await.unwrap().status,
        QrStatus::Expired
    );
    let qr = service.begin_login().await.unwrap();
    service.state.lock().await.pending.as_mut().unwrap().expires = Instant::now();
    assert_eq!(
        service.poll_login(&qr.attempt_id).await.unwrap().status,
        QrStatus::Expired
    );
    server.done();
}

#[tokio::test]
async fn failed_persistence_falls_back_and_failed_deletion_is_not_success() {
    let server = server(vec![
        key(),
        success("MUSIC_U=memory; Path=/"),
        profile(9, "MUSIC_U=memory"),
        reply("/api/logout", json!({"code":500})),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    store.fail_save.store(true, Ordering::SeqCst);
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    assert_eq!(
        login(&service).await.session.persistence,
        Persistence::MemoryOnly
    );
    store.fail_clear.store(true, Ordering::SeqCst);
    let report = service.logout().await;
    assert!(report.local_cleared);
    assert!(!report.credentials_cleared);
    assert!(!report.server_revoked);
    assert_eq!(
        service.snapshot().await.persistence,
        Persistence::CleanupRequired
    );
    server.done();
}

#[tokio::test]
async fn switching_accounts_does_not_send_previous_cookie_or_keep_previous_secret() {
    let server = server(vec![
        key(),
        success("MUSIC_U=first; Path=/"),
        profile(1, "MUSIC_U=first"),
        key(),
        success("MUSIC_U=second; Path=/"),
        profile(2, "MUSIC_U=second"),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    assert_eq!(login(&service).await.session.profile.unwrap().id, "1");
    store.fail_save.store(true, Ordering::SeqCst);
    let second = login(&service).await;
    assert_eq!(second.session.profile.unwrap().id, "2");
    assert_eq!(second.session.persistence, Persistence::MemoryOnly);
    assert!(store.secret.lock().unwrap().is_none());
    server.done();
}

#[tokio::test]
async fn server_session_expiry_clears_memory_and_credentials() {
    let server = server(vec![
        key(),
        success("MUSIC_U=session; Path=/"),
        profile(4, "MUSIC_U=session"),
        reply("/api/song/like/get", json!({"code":301})),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    login(&service).await;
    assert!(matches!(
        service.liked_tracks().await,
        Err(BackendError::Unauthorized)
    ));
    assert!(service.snapshot().await.profile.is_none());
    assert!(store.secret.lock().unwrap().is_none());
    server.done();
}

#[tokio::test]
async fn untrusted_success_without_cookie_or_profile_does_not_log_in() {
    let server = server(vec![
        key(),
        reply("/weapi/login/qrcode/client/login", json!({"code":803})),
        key(),
        success("MUSIC_U=bad; Path=/"),
        reply(
            "/api/nuser/account/get",
            json!({"code":200,"account":null,"profile":null}),
        ),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    let first = service.begin_login().await.unwrap();
    assert!(matches!(
        service.poll_login(&first.attempt_id).await,
        Err(BackendError::Protocol(_))
    ));
    assert!(matches!(
        service.poll_login(&first.attempt_id).await,
        Err(BackendError::StaleOperation)
    ));
    let second = service.begin_login().await.unwrap();
    assert!(matches!(
        service.poll_login(&second.attempt_id).await,
        Err(BackendError::Unauthorized)
    ));
    assert!(matches!(
        service.poll_login(&second.attempt_id).await,
        Err(BackendError::StaleOperation)
    ));
    assert!(service.snapshot().await.profile.is_none());
    assert_eq!(store.saves.load(Ordering::SeqCst), 0);
    server.done();
}

#[tokio::test]
async fn restore_removes_invalid_secret_but_preserves_on_server_failure() {
    let server = server(vec![
        key(),
        success("MUSIC_U=saved; Path=/"),
        profile(8, "MUSIC_U=saved"),
        reply("/api/nuser/account/get", json!({"code":500})),
        reply(
            "/api/nuser/account/get",
            json!({"code":200,"account":null,"profile":null}),
        ),
    ])
    .await;
    let store = Arc::new(MemoryStore::default());
    let original = AccountService::with_store(store.clone(), server.factory.clone());
    login(&original).await;
    drop(original);
    let service = AccountService::with_store(store.clone(), server.factory.clone());
    assert!(matches!(
        service.restore().await,
        Err(BackendError::Service(500))
    ));
    assert!(store.secret.lock().unwrap().is_some());
    assert!(service.restore().await.unwrap().profile.is_none());
    assert!(store.secret.lock().unwrap().is_none());
    server.done();
}

#[tokio::test]
async fn obsolete_personal_response_is_rejected_after_logout() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut delayed = reply("/api/song/like/get", json!({"code":200,"ids":[7]}));
    delayed.hold = Some((entered.clone(), release.clone()));
    let server = server(vec![
        key(),
        success("MUSIC_U=first; Path=/"),
        profile(1, "MUSIC_U=first"),
        delayed,
        reply("/api/logout", json!({"code":200})),
    ])
    .await;
    let service = Arc::new(AccountService::with_store(
        Arc::new(MemoryStore::default()),
        server.factory.clone(),
    ));
    login(&service).await;
    let worker = service.clone();
    let read = tokio::spawn(async move { worker.liked_tracks().await });
    entered.notified().await;
    assert!(service.logout().await.credentials_cleared);
    release.notify_one();
    assert!(matches!(
        read.await.unwrap(),
        Err(BackendError::StaleOperation)
    ));
    assert!(service.snapshot().await.profile.is_none());
    server.done();
}

#[tokio::test]
async fn unavailable_keyring_and_corrupt_secret_do_not_create_a_session() {
    let store = Arc::new(MemoryStore::default());
    let service = AccountService::with_store(
        store.clone(),
        Arc::new(|secret| match secret {
            Some(secret) => NeteaseClient::restore(secret),
            None => NeteaseClient::new(),
        }),
    );
    store.fail_load.store(true, Ordering::SeqCst);
    assert_eq!(
        service.restore().await.unwrap().persistence,
        Persistence::MemoryOnly
    );
    store.fail_load.store(false, Ordering::SeqCst);
    *store.secret.lock().unwrap() = Some(b"not-json".to_vec());
    assert!(matches!(
        service.restore().await,
        Err(BackendError::CredentialStorage)
    ));
    assert!(store.secret.lock().unwrap().is_none());
    assert!(service.snapshot().await.profile.is_none());
}

#[tokio::test]
async fn private_playlist_uses_session_for_details_and_songs_and_rejects_late_response() {
    let mut detail = reply(
        "/api/v6/playlist/detail",
        json!({"code":200,"playlist":{"name":"private synthetic","trackCount":1,"trackIds":[{"id":7}]}}),
    );
    detail.expect_cookie = Some("MUSIC_U=private");
    let mut songs = reply(
        "/api/song/detail/",
        json!({"code":200,"songs":[{"id":7,"name":"synthetic","ar":[],"al":{"id":1,"name":"a"},"dt":1000}]}),
    );
    songs.expect_cookie = Some("MUSIC_U=private");
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let mut late = reply(
        "/api/v6/playlist/detail",
        json!({"code":200,"playlist":{"name":"obsolete","trackCount":0,"trackIds":[]}}),
    );
    late.hold = Some((entered.clone(), release.clone()));
    let server = server(vec![
        key(),
        success("MUSIC_U=private; Path=/"),
        profile(1, "MUSIC_U=private"),
        detail,
        songs,
        late,
        reply("/api/logout", json!({"code":200})),
    ])
    .await;
    let service = Arc::new(AccountService::with_store(
        Arc::new(MemoryStore::default()),
        server.factory.clone(),
    ));
    login(&service).await;
    assert_eq!(
        service.playlist("1", 0, 20).await.unwrap().tracks.items[0].id,
        "7"
    );
    let worker = service.clone();
    let task = tokio::spawn(async move { worker.playlist("2", 0, 20).await });
    entered.notified().await;
    service.logout().await;
    release.notify_one();
    assert!(matches!(
        task.await.unwrap(),
        Err(BackendError::StaleOperation)
    ));
    server.done();
}
