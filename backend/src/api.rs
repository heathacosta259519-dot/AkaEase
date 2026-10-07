use crate::session::{SessionCookies, StoredSession};
use crate::{
    error::{BackendError, Result},
    lyrics::{LyricLine, parse_lyrics},
    model::*,
};
use reqwest::{Client, RequestBuilder, Url, header};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct NeteaseClient {
    pub(crate) http: Client,
    pub(crate) origin: Url,
    pub(crate) cookies: Arc<SessionCookies>,
}

pub(crate) fn protocol(field: &str) -> BackendError {
    BackendError::Protocol(field.into())
}
pub(crate) fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|_| protocol("invalid or missing fields"))
}

pub fn validate_id(id: &str) -> Result<u64> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BackendError::InvalidInput(
            "expected positive decimal ID".into(),
        ));
    }
    id.parse::<u64>()
        .ok()
        .filter(|&id| id > 0)
        .ok_or_else(|| BackendError::InvalidInput("ID outside supported range".into()))
}

pub(crate) fn pagination(offset: u32, limit: u32) -> Result<()> {
    if !(1..=100).contains(&limit) || offset > 1_000_000 {
        return Err(BackendError::InvalidInput(
            "limit must be 1..100; offset at most 1000000".into(),
        ));
    }
    Ok(())
}

impl NeteaseClient {
    #[cfg(feature = "test-support")]
    pub fn local_fixture(origin: &str) -> Result<Self> {
        let origin = Url::parse(origin).map_err(|_| protocol("invalid fixture URL"))?;
        if origin.scheme() != "http" || origin.host_str() != Some("127.0.0.1") {
            return Err(protocol("fixture must use a local HTTP server"));
        }
        let mut client = Self::configured(&crate::storage::ProxyConfig::Direct)?;
        client.origin = origin;
        Ok(client)
    }

    pub fn new() -> Result<Self> {
        Self::configured(&crate::storage::ProxyConfig::System)
    }

    pub fn configured(proxy: &crate::storage::ProxyConfig) -> Result<Self> {
        Self::with_cookies(Arc::new(SessionCookies::default()), proxy)
    }

    pub(crate) fn restore(secret: &StoredSession) -> Result<Self> {
        Self::restore_configured(secret, &crate::storage::ProxyConfig::System)
    }

    pub(crate) fn restore_configured(
        secret: &StoredSession,
        proxy: &crate::storage::ProxyConfig,
    ) -> Result<Self> {
        Self::with_cookies(Arc::new(SessionCookies::restore(secret)?), proxy)
    }

    fn with_cookies(
        cookies: Arc<SessionCookies>,
        proxy: &crate::storage::ProxyConfig,
    ) -> Result<Self> {
        let client = proxy
            .apply(Client::builder())?
            .cookie_provider(cookies.clone())
            .user_agent("AkaNetease-desktop/0.1")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http: client,
            cookies,
            origin: Url::parse("https://music.163.com/").expect("static URL"),
        })
    }

    pub(crate) fn endpoint(&self, path: &str) -> Url {
        self.origin.join(path).expect("internal endpoint path")
    }

    pub(crate) async fn request(&self, request: RequestBuilder) -> Result<Value> {
        self.request_codes(request, &[200]).await
    }

    pub(crate) async fn request_codes(
        &self,
        request: RequestBuilder,
        allowed: &[i64],
    ) -> Result<Value> {
        let mut response = request
            .header("Referer", "https://music.163.com/")
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(BackendError::Http(response.status().as_u16()));
        }
        const MAX_BODY: usize = 8 * 1024 * 1024;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > MAX_BODY {
                return Err(protocol("response exceeds size limit"));
            }
            body.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&body).map_err(|_| protocol("expected JSON"))?;
        let code = value
            .get("code")
            .and_then(Value::as_i64)
            .ok_or_else(|| protocol("missing service code"))?;
        if !allowed.contains(&code) {
            return Err(if code == 301 {
                BackendError::Unauthorized
            } else {
                BackendError::Service(code)
            });
        }
        Ok(value)
    }

    pub(crate) async fn request_weapi_codes(
        &self,
        path: &str,
        mut value: serde_json::Value,
        allowed: &[i64],
    ) -> Result<Value> {
        let csrf_token = self.cookies.csrf_token(&self.origin)?;
        if let Some(object) = value.as_object_mut() {
            object.insert("csrf_token".into(), Value::String(csrf_token.clone()));
        }
        let fields = crate::weapi::encode(&value)?;
        let cookies = self.cookies.weapi_cookie_header(&self.origin)?;
        self.request_codes(
            self.http
                .post(self.endpoint(path))
                .query(&[("csrf_token", csrf_token)])
                .header(
                    header::USER_AGENT,
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                )
                .header(header::COOKIE, cookies)
                .header(header::ACCEPT, "*/*")
                .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.5")
                .form(&fields),
            allowed,
        )
        .await
    }

    pub async fn search(&self, query: &str, offset: u32, limit: u32) -> Result<Page<Track>> {
        pagination(offset, limit)?;
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 200 {
            return Err(BackendError::InvalidInput(
                "query must contain 1..200 characters".into(),
            ));
        }
        let value = self
            .request(self.http.post(self.endpoint("api/search/get")).form(&[
                ("s", query.to_owned()),
                ("type", "1".into()),
                ("offset", offset.to_string()),
                ("limit", limit.to_string()),
            ]))
            .await?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Search {
            #[serde(default)]
            songs: Vec<WireTrack>,
            song_count: u64,
            has_more: Option<bool>,
        }
        let result: Search = decode(value["result"].clone())?;
        let items = result
            .songs
            .into_iter()
            .map(Track::from)
            .collect::<Vec<_>>();
        let has_more = result
            .has_more
            .unwrap_or(u64::from(offset) + (items.len() as u64) < result.song_count);
        Ok(Page {
            items,
            total: result.song_count,
            offset,
            has_more,
        })
    }

    pub async fn tracks(&self, ids: &[String]) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        if ids.len() > 100 {
            return Err(BackendError::InvalidInput(
                "at most 100 track IDs per request".into(),
            ));
        }
        let ids = ids
            .iter()
            .map(|id| validate_id(id))
            .collect::<Result<Vec<_>>>()?;
        let value = self
            .request(
                self.http
                    .get(self.endpoint("api/song/detail/"))
                    .query(&[("ids", json!(ids).to_string())]),
            )
            .await?;
        let tracks: Vec<WireTrack> = decode(value["songs"].clone())?;
        let by_id: HashMap<String, Track> = tracks
            .into_iter()
            .map(Track::from)
            .map(|track| (track.id.clone(), track))
            .collect();
        Ok(ids
            .into_iter()
            .filter_map(|id| by_id.get(&id.to_string()).cloned())
            .collect())
    }

    pub async fn artist_detail(&self, id: &str) -> Result<ArtistDetail> {
        let id = validate_id(id)?;
        let value = self
            .request_weapi_codes(&format!("weapi/v1/artist/{id}"), json!({}), &[200])
            .await?;
        let artist: WireArtistDetail = decode(value["artist"].clone())?;
        let WireArtistDetail {
            id,
            name,
            cover_url,
            alias,
            brief_description,
            music_size,
            album_size,
        } = artist;
        Ok(ArtistDetail {
            id: id.to_string(),
            name,
            cover_url,
            aliases: alias,
            brief_description,
            music_size,
            album_size,
        })
    }

    pub async fn artist_songs(&self, id: &str) -> Result<Vec<Track>> {
        let id = validate_id(id)?;
        let value = self
            .request(
                self.http
                    .get(self.endpoint("api/artist/top/song"))
                    .query(&[("id", id)]),
            )
            .await?;
        let songs: Vec<WireTrack> = decode(value["songs"].clone())?;
        Ok(songs.into_iter().take(50).map(Track::from).collect())
    }

    pub async fn artist_albums(
        &self,
        id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<Page<AlbumSummary>> {
        let id = validate_id(id)?;
        pagination(offset, limit)?;
        let value = self
            .request(
                self.http
                    .get(self.endpoint(&format!("api/artist/albums/{id}")))
                    .query(&[("offset", offset), ("limit", limit)]),
            )
            .await?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AlbumCount {
            album_size: u64,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Albums {
            artist: AlbumCount,
            hot_albums: Vec<WireAlbumDetail>,
            more: bool,
        }
        let result: Albums = decode(value)?;
        Ok(Page {
            items: result
                .hot_albums
                .into_iter()
                .map(|album| AlbumSummary {
                    id: album.id.to_string(),
                    name: album.name,
                    cover_url: crate::covers::album_cover(&album.id.to_string(), album.cover_url),
                    artist: album.artist.map(Artist::from),
                    artists: album.artists.into_iter().map(Artist::from).collect(),
                    publish_time_ms: album.publish_time_ms,
                    track_count: album.track_count,
                })
                .collect(),
            total: result.artist.album_size,
            offset,
            has_more: result.more,
        })
    }

    pub async fn album_detail(&self, id: &str) -> Result<AlbumDetail> {
        let id = validate_id(id)?;
        let value = self
            .request_weapi_codes(&format!("weapi/v1/album/{id}"), json!({}), &[200])
            .await?;
        let album: WireAlbumDetail = decode(value["album"].clone())?;
        let songs: Vec<WireTrack> = decode(value["songs"].clone())?;
        let artists = album
            .artists
            .into_iter()
            .map(Artist::from)
            .collect::<Vec<_>>();
        let artist = album.artist.map(Artist::from);
        Ok(AlbumDetail {
            id: album.id.to_string(),
            name: album.name,
            cover_url: crate::covers::album_cover(&album.id.to_string(), album.cover_url),
            artist,
            artists,
            description: album.description,
            publish_time_ms: album.publish_time_ms,
            company: album.company,
            track_count: album.track_count,
            tracks: songs.into_iter().map(Track::from).collect(),
        })
    }

    pub async fn playlist(&self, id: &str, offset: u32, limit: u32) -> Result<PlaylistPage> {
        let id = validate_id(id)?;
        pagination(offset, limit)?;
        let value = self
            .request(
                self.http
                    .get(self.endpoint("api/v6/playlist/detail"))
                    .query(&[("id", id.to_string()), ("n", "0".into()), ("s", "0".into())]),
            )
            .await?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct List {
            name: String,
            track_count: u64,
            track_ids: Vec<WireId>,
        }
        let list: List = decode(value["playlist"].clone())?;
        if list.track_count != list.track_ids.len() as u64 {
            return Err(protocol("playlist ID list is incomplete"));
        }
        let ids: Vec<String> = list
            .track_ids
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|track| track.id.to_string())
            .collect();
        let items = self.tracks(&ids).await?;
        let unavailable_ids = ids
            .iter()
            .filter(|id| !items.iter().any(|track| &track.id == *id))
            .cloned()
            .collect();
        Ok(PlaylistPage {
            id: id.to_string(),
            title: list.name,
            tracks: Page {
                items,
                total: list.track_count,
                offset,
                has_more: u64::from(offset) + (ids.len() as u64) < list.track_count,
            },
            unavailable_ids,
        })
    }

    pub async fn lyrics(&self, id: &str) -> Result<Vec<LyricLine>> {
        let id = validate_id(id)?;
        let value = self
            .request(self.http.get(self.endpoint("api/song/lyric")).query(&[
                ("id", id.to_string()),
                ("lv", "-1".into()),
                ("tv", "-1".into()),
            ]))
            .await?;
        if value["nolyric"] == true || value["uncollected"] == true {
            return Ok(Vec::new());
        }
        let original = value
            .pointer("/lrc/lyric")
            .and_then(Value::as_str)
            .ok_or_else(|| protocol("missing lyric content"))?;
        Ok(parse_lyrics(
            original,
            value
                .pointer("/tlyric/lyric")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ))
    }

    pub async fn stream(&self, id: &str) -> Result<StreamSource> {
        self.stream_quality(id, SoundQuality::default()).await
    }

    pub async fn stream_quality(&self, id: &str, quality: SoundQuality) -> Result<StreamSource> {
        let id = validate_id(id)?;
        let value = self
            .request_weapi_codes(
                "weapi/song/enhance/player/url/v1",
                json!({"ids":json!([id]).to_string(),"level":quality.level(),"encodeType":"flac"}),
                &[200],
            )
            .await?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Source {
            id: u64,
            code: i64,
            url: Option<String>,
            #[serde(default)]
            br: u32,
            level: Option<String>,
            #[serde(rename = "type")]
            format: Option<String>,
            expi: Option<u64>,
            free_trial_info: Option<Value>,
        }
        let sources: Vec<Source> = decode(value["data"].clone())?;
        let source = sources
            .into_iter()
            .find(|source| source.id == id)
            .ok_or_else(|| protocol("missing requested stream"))?;
        let url = source
            .url
            .filter(|url| !url.is_empty())
            .ok_or(BackendError::Unavailable)?;
        if source.code != 200 {
            return Err(BackendError::Unavailable);
        }
        let parsed = Url::parse(&url).map_err(|_| protocol("invalid stream URL"))?;
        if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
            return Err(protocol("invalid stream URL scheme"));
        }
        let format = source.format.map(|f| f.to_ascii_lowercase());
        let level = source
            .level
            .and_then(|l| serde_json::from_value(Value::String(l)).ok());
        Ok(StreamSource {
            track_id: id.to_string(),
            url,
            bitrate: source.br,
            quality: SoundQuality::from_source(source.br, format.as_deref(), level),
            format,
            expires_in_seconds: source.expi,
            is_preview: source.free_trial_info.is_some(),
        })
    }
}

#[derive(Deserialize)]
struct WireId {
    id: u64,
}
#[derive(Deserialize)]
struct WireArtist {
    id: u64,
    name: String,
}
impl From<WireArtist> for Artist {
    fn from(value: WireArtist) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireArtistDetail {
    id: u64,
    name: String,
    #[serde(rename = "picUrl", alias = "cover", default)]
    cover_url: Option<String>,
    #[serde(default)]
    alias: Vec<String>,
    #[serde(rename = "briefDesc", default)]
    brief_description: Option<String>,
    #[serde(rename = "musicSize", default)]
    music_size: u64,
    #[serde(rename = "albumSize", default)]
    album_size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAlbumDetail {
    id: u64,
    name: String,
    #[serde(rename = "picUrl", alias = "cover", default)]
    cover_url: Option<String>,
    #[serde(default)]
    artist: Option<WireArtist>,
    #[serde(default)]
    artists: Vec<WireArtist>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "publishTime", default)]
    publish_time_ms: Option<u64>,
    #[serde(default)]
    company: Option<String>,
    #[serde(rename = "size", default)]
    track_count: u64,
}

#[derive(Deserialize)]
struct WireAlbum {
    id: u64,
    name: String,
    #[serde(rename = "picUrl")]
    cover: Option<String>,
}
#[derive(Deserialize)]
pub(crate) struct WireTrack {
    id: u64,
    name: String,
    #[serde(alias = "ar")]
    artists: Vec<WireArtist>,
    #[serde(alias = "al")]
    album: WireAlbum,
    #[serde(alias = "dt")]
    duration: u64,
}
impl From<WireTrack> for Track {
    fn from(value: WireTrack) -> Self {
        Self {
            id: value.id.to_string(),
            title: value.name,
            duration_ms: value.duration,
            artists: value.artists.into_iter().map(Artist::from).collect(),
            album: Album {
                id: value.album.id.to_string(),
                name: value.album.name,
                cover_url: crate::covers::album_cover(
                    &value.album.id.to_string(),
                    value.album.cover,
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn mock(
        responses: Vec<(&'static str, String)>,
    ) -> (NeteaseClient, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let task = tokio::spawn(async move {
            for (expected, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                loop {
                    let mut buffer = [0u8; 1024];
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buffer[..n]);
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                assert!(String::from_utf8_lossy(&request).starts_with(expected));
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        (
            NeteaseClient {
                http: Client::builder()
                    .no_proxy()
                    .timeout(Duration::from_secs(2))
                    .build()
                    .unwrap(),
                origin,
                cookies: Arc::new(SessionCookies::default()),
            },
            task,
        )
    }
    fn song(id: u64) -> Value {
        json!({"id":id,"name":"自建歌曲","ar":[{"id":7,"name":"Test artist"}],"al":{"id":8,"name":"Test album"},"dt":4567})
    }
    #[tokio::test]
    async fn stream_quality_reports_returned_metadata_and_accepts_downgrades() {
        let cases = [
            (128000, "mp3", "lossless", Some(SoundQuality::Standard)),
            (192000, "mp3", "higher", Some(SoundQuality::Higher)),
            (320000, "mp3", "hires", Some(SoundQuality::Exhigh)),
            (999000, "FLAC", "lossless", Some(SoundQuality::Lossless)),
            (2304000, "flac", "hires", Some(SoundQuality::Hires)),
            (0, "flac", "unexpected", Some(SoundQuality::Lossless)),
            (0, "unknown", "unexpected", None),
        ];
        let (client, task) = mock(cases.iter().map(|(br, format, level, _)| {
            ("POST /weapi/song/enhance/player/url/v1?", json!({"code":200,"data":[{"id":1,"code":200,"url":"https://audio.invalid/synthetic","br":br,"type":format,"level":level,"expi":600,"freeTrialInfo":{"start":0,"end":20}}]}).to_string())
        }).collect()).await;
        for (br, format, _, expected) in cases {
            let source = client
                .stream_quality("1", SoundQuality::Hires)
                .await
                .unwrap();
            assert_eq!(source.bitrate, br);
            assert_eq!(source.quality, expected);
            assert_eq!(
                source.format.as_deref(),
                Some(format.to_ascii_lowercase().as_str())
            );
            assert!(source.is_preview);
        }
        task.await.unwrap();
    }
    #[tokio::test]
    async fn missing_album_cover_falls_back_across_details_lists_and_tracks() {
        let missing = "https://p4.music.126.net/Uk_AkI1cDZNn1fn_jl_Snw==/18268385696067264.jpg";
        let fallback = "https://p1.music.126.net/xdCW-LmznfEJFivWyV7a_Q==/109951164116268622.jpg";
        let album = json!({"id":79543884,"name":"By The Way","picUrl":missing,"size":16,"publishTime":123,"company":"Synthetic company","artist":{"id":41927,"name":"Red Hot Chili Peppers"}});
        let mut track = song(1);
        track["al"] = album.clone();
        let songs = json!([track.clone(), track]);
        let (client, task) = mock(vec![
            (
                "POST /weapi/v1/album/79543884?",
                json!({"code":200,"album":album,"songs":songs}).to_string(),
            ),
            (
                "GET /api/artist/albums/41927?",
                json!({"code":200,"artist":{"albumSize":183},"hotAlbums":[album],"more":false})
                    .to_string(),
            ),
            (
                "GET /api/song/detail/?",
                json!({"code":200,"songs":songs}).to_string(),
            ),
        ])
        .await;
        let detail = client.album_detail("79543884").await.unwrap();
        assert_eq!(detail.id, "79543884");
        assert_eq!(detail.name, "By The Way");
        assert_eq!(detail.track_count, 16);
        assert_eq!(detail.publish_time_ms, Some(123));
        assert_eq!(detail.company.as_deref(), Some("Synthetic company"));
        assert_eq!(detail.cover_url.as_deref(), Some(fallback));
        assert_eq!(detail.tracks.len(), 2);
        for track in &detail.tracks {
            assert_eq!(track.album.id, "79543884");
            assert_eq!(track.album.cover_url.as_deref(), Some(fallback));
        }
        let page = client.artist_albums("41927", 0, 20).await.unwrap();
        assert_eq!(page.items[0].id, detail.id);
        assert_eq!(page.items[0].cover_url, detail.cover_url);
        assert_eq!(page.total, 183);
        let tracks = client.tracks(&["1".into(), "1".into()]).await.unwrap();
        assert_eq!(tracks, detail.tracks);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn artist_songs_preserve_top_order_cap_and_empty_list() {
        let songs: Vec<_> = (1..=51).rev().map(song).collect();
        let (client, task) = mock(vec![
            (
                "GET /api/artist/top/song?id=7 ",
                json!({"code":200,"songs":songs}).to_string(),
            ),
            (
                "GET /api/artist/top/song?id=8 ",
                json!({"code":200,"songs":[]}).to_string(),
            ),
        ])
        .await;
        let tracks = client.artist_songs("7").await.unwrap();
        assert_eq!(tracks.len(), 50);
        assert_eq!(tracks.first().unwrap().id, "51");
        assert_eq!(tracks.last().unwrap().id, "2");
        assert_eq!(tracks[0].duration_ms, 4567);
        assert!(client.artist_songs("8").await.unwrap().is_empty());
        task.await.unwrap();
    }
    #[tokio::test]
    async fn artist_albums_preserve_metadata_order_and_upstream_pagination() {
        let albums = json!([
            {"id":9007199254740993_u64,"name":"First","picUrl":"https://example.com/cover.jpg","artist":{"id":7,"name":"Singer"},"artists":[{"id":7,"name":"Singer"}],"publishTime":123456,"size":5},
            {"id":2,"name":"Second","picUrl":null,"publishTime":null,"size":0}
        ]);
        let (client, task) = mock(vec![
            (
                "GET /api/artist/albums/7?offset=0&limit=3 ",
                json!({"code":200,"artist":{"albumSize":8},"hotAlbums":albums,"more":true})
                    .to_string(),
            ),
            (
                "GET /api/artist/albums/7?offset=3&limit=3 ",
                json!({"code":200,"artist":{"albumSize":8},"hotAlbums":[],"more":false})
                    .to_string(),
            ),
        ])
        .await;
        let page = client.artist_albums("7", 0, 3).await.unwrap();
        assert_eq!(page.total, 8);
        assert_eq!(page.offset, 0);
        assert!(page.has_more);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].id, "9007199254740993");
        assert_eq!(page.items[1].id, "2");
        assert!(page.items[1].artist.is_none());
        assert!(page.items[1].artists.is_empty());
        assert!(page.items[1].cover_url.is_none());
        let json = serde_json::to_value(&page).unwrap();
        assert_eq!(json["items"][0]["publishTimeMs"], 123456);
        assert_eq!(json["items"][0]["trackCount"], 5);
        assert_eq!(json["items"][0]["artists"][0]["id"], "7");
        let last = client.artist_albums("7", 3, 3).await.unwrap();
        assert_eq!(last.offset, 3);
        assert_eq!(last.total, 8);
        assert!(!last.has_more);
        assert!(last.items.is_empty());
        task.await.unwrap();
    }
    #[tokio::test]
    async fn artist_queries_reject_service_and_incomplete_responses() {
        let (client, task) = mock(vec![
            ("GET /api/artist/top/song?", json!({"code":500}).to_string()),
            ("GET /api/artist/top/song?", json!({"code":200}).to_string()),
            (
                "GET /api/artist/albums/7?",
                json!({"code":200,"artist":{"albumSize":8},"hotAlbums":[]}).to_string(),
            ),
            (
                "GET /api/artist/albums/7?",
                json!({"code":200,"hotAlbums":[],"more":false}).to_string(),
            ),
        ])
        .await;
        assert!(matches!(
            client.artist_songs("7").await,
            Err(BackendError::Service(500))
        ));
        assert!(matches!(
            client.artist_songs("7").await,
            Err(BackendError::Protocol(_))
        ));
        assert!(matches!(
            client.artist_albums("7", 0, 20).await,
            Err(BackendError::Protocol(_))
        ));
        assert!(matches!(
            client.artist_albums("7", 0, 20).await,
            Err(BackendError::Protocol(_))
        ));
        task.await.unwrap();
    }
    #[tokio::test]
    async fn playlist_preserves_requested_order_duplicates_and_missing_ids() {
        let (client, task) = mock(vec![
            ("GET /api/v6/playlist/detail?",json!({"code":200,"playlist":{"name":"测试","trackCount":4,"trackIds":[{"id":1},{"id":3},{"id":3},{"id":2}]}}).to_string()),
            ("GET /api/song/detail/?",json!({"code":200,"songs":[song(3)]}).to_string())
        ]).await;
        let page = client.playlist("123", 1, 3).await.unwrap();
        assert_eq!(
            page.tracks
                .items
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            vec!["3", "3"]
        );
        assert_eq!(page.unavailable_ids, vec!["2"]);
        assert!(!page.tracks.has_more);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn rejects_service_errors_and_unavailable_audio() {
        let (client, task) = mock(vec![
            (
                "POST /api/search/get",
                json!({"code":301,"message":"secret-value"}).to_string(),
            ),
            (
                "POST /weapi/song/enhance/player/url/v1?",
                json!({"code":200,"data":[{"id":1,"code":404,"url":null,"br":0}]}).to_string(),
            ),
            ("GET /api/song/lyric?", "<html>secret-value</html>".into()),
        ])
        .await;
        assert!(matches!(
            client.search("query", 0, 20).await,
            Err(BackendError::Unauthorized)
        ));
        assert!(matches!(
            client.stream("1").await,
            Err(BackendError::Unavailable)
        ));
        let error = client.lyrics("1").await.unwrap_err();
        assert!(matches!(error, BackendError::Protocol(_)));
        assert!(!error.to_string().contains("secret-value"));
        task.await.unwrap();
    }
    #[tokio::test]
    async fn maps_legacy_fields_and_large_ids_without_precision_loss() {
        let (client, task) = mock(vec![("POST /api/search/get", json!({"code":200,"result":{"songCount":2,"songs":[{"id":9007199254740993_u64,"name":"Synthetic","artists":[],"album":{"id":1,"name":"album"},"duration":123}]}}).to_string())]).await;
        let page = client.search("test", 0, 1).await.unwrap();
        assert_eq!(page.items[0].id, "9007199254740993");
        assert!(page.has_more);
        assert_eq!(
            serde_json::to_value(&page).unwrap()["items"][0]["durationMs"],
            123
        );
        task.await.unwrap();
    }
    #[tokio::test]
    async fn rejects_bad_input_before_network() {
        let client = NeteaseClient::new().unwrap();
        assert!(matches!(
            client.search(" ", 0, 20).await,
            Err(BackendError::InvalidInput(_))
        ));
        assert!(client.tracks(&["1&cookie=x".into()]).await.is_err());
        assert!(client.playlist("1", 0, 0).await.is_err());
        assert!(matches!(
            client.artist_songs("0").await,
            Err(BackendError::InvalidInput(_))
        ));
        assert!(client.artist_songs("1&cookie=x").await.is_err());
        for (id, offset, limit) in [
            ("0", 0, 20),
            ("1", 0, 0),
            ("1", 0, 101),
            ("1", 1_000_001, 20),
        ] {
            assert!(matches!(
                client.artist_albums(id, offset, limit).await,
                Err(BackendError::InvalidInput(_))
            ));
        }
        assert!(validate_id("0").is_err());
    }
}

#[cfg(test)]
mod proxy_tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    #[tokio::test]
    async fn configured_proxy_receives_request_without_resolving_origin() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy =
            crate::storage::ProxyConfig::Http(format!("http://{}", listener.local_addr().unwrap()));
        let mut client = NeteaseClient::configured(&proxy).unwrap();
        client.origin = Url::parse("http://unresolved.invalid/").unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 4096];
            let n = socket.read(&mut bytes).await.unwrap();
            let request = String::from_utf8_lossy(&bytes[..n]);
            assert!(request.starts_with("GET http://unresolved.invalid/api/song/lyric?"));
            assert!(!request.to_ascii_lowercase().contains("proxy-authorization"));
            let body = r#"{"code":200,"nolyric":true}"#;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        assert!(client.lyrics("1").await.unwrap().is_empty());
        server.await.unwrap();
    }
}
