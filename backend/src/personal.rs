use crate::{
    api::{NeteaseClient, WireTrack, decode, pagination, protocol, validate_id},
    error::{BackendError, Result},
    model::Track,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub id: String,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPlaylist {
    pub id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub track_count: u64,
    pub owner_id: String,
    pub subscribed: bool,
    pub is_creator: bool,
    pub is_liked_playlist: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPlaylists {
    pub items: Vec<UserPlaylist>,
    pub offset: u32,
    pub has_more: bool,
    pub liked_playlist_id: Option<String>,
}

#[derive(Deserialize)]
struct Creator {
    #[serde(rename = "userId")]
    id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireUserPlaylist {
    id: u64,
    name: String,
    cover_img_url: Option<String>,
    track_count: u64,
    creator: Creator,
    subscribed: Option<bool>,
    special_type: Option<u64>,
}

pub(crate) fn playlist_name(name: &str, privacy: Option<u32>) -> Result<(&str, u32)> {
    let name = name.trim();
    let privacy = privacy.unwrap_or(0);
    if !(1..=40).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(BackendError::InvalidInput(
            "playlist name must contain 1..40 characters without controls".into(),
        ));
    }
    if !matches!(privacy, 0 | 10) {
        return Err(BackendError::InvalidInput("privacy must be 0 or 10".into()));
    }
    Ok((name, privacy))
}

pub(crate) fn playlist_track_ids(ids: &[String], op: &str) -> Result<Vec<u64>> {
    if !matches!(op, "add" | "del") || !(1..=100).contains(&ids.len()) {
        return Err(BackendError::InvalidInput(
            "op must be add/del; supply 1..100 track IDs".into(),
        ));
    }
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for id in ids {
        let id = validate_id(id)?;
        if seen.insert(id) {
            unique.push(id);
        }
    }
    Ok(unique)
}

impl NeteaseClient {
    pub(crate) async fn profile(&self) -> Result<Option<UserProfile>> {
        let value = self
            .request(self.http.post(self.endpoint("api/nuser/account/get")))
            .await?;
        if value["account"].is_null() && value["profile"].is_null() {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct Profile {
            #[serde(rename = "userId")]
            id: u64,
            nickname: String,
            #[serde(rename = "avatarUrl")]
            avatar_url: Option<String>,
        }
        #[derive(Deserialize)]
        struct Account {
            id: u64,
        }
        let profile: Profile = decode(value["profile"].clone())?;
        let account: Account = decode(value["account"].clone())?;
        if profile.id == 0 || profile.id != account.id {
            return Err(crate::api::protocol("account/profile identity mismatch"));
        }
        Ok(Some(UserProfile {
            id: profile.id.to_string(),
            nickname: profile.nickname,
            avatar_url: profile.avatar_url,
        }))
    }
    pub(crate) async fn user_playlists(
        &self,
        user_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<UserPlaylists> {
        let requested_user = validate_id(user_id)?;
        pagination(offset, limit)?;
        let value = self
            .request(self.http.get(self.endpoint("api/user/playlist")).query(&[
                ("uid", user_id.to_owned()),
                ("offset", offset.to_string()),
                ("limit", limit.to_string()),
            ]))
            .await?;
        #[derive(Deserialize)]
        struct Response {
            playlist: Vec<WireUserPlaylist>,
            more: bool,
        }
        let response: Response = decode(value)?;
        let items = response
            .playlist
            .into_iter()
            .map(|list| {
                let is_liked_playlist = list.special_type == Some(5);
                UserPlaylist {
                    id: list.id.to_string(),
                    title: list.name,
                    cover_url: list.cover_img_url,
                    track_count: list.track_count,
                    owner_id: list.creator.id.to_string(),
                    subscribed: list.subscribed.unwrap_or(false),
                    is_creator: list.creator.id == requested_user,
                    is_liked_playlist,
                }
            })
            .collect::<Vec<_>>();
        let liked_playlist_id = items
            .iter()
            .find(|playlist| playlist.is_liked_playlist)
            .map(|playlist| playlist.id.clone());
        Ok(UserPlaylists {
            items,
            offset,
            has_more: response.more,
            liked_playlist_id,
        })
    }
    pub(crate) async fn liked_tracks(&self, user_id: &str) -> Result<Vec<String>> {
        validate_id(user_id)?;
        let value = self
            .request(
                self.http
                    .get(self.endpoint("api/song/like/get"))
                    .query(&[("uid", user_id)]),
            )
            .await?;
        let ids: Vec<u64> = decode(value["ids"].clone())?;
        Ok(ids.into_iter().map(|id| id.to_string()).collect())
    }
    pub(crate) async fn daily_tracks(&self) -> Result<Vec<Track>> {
        let value = self
            .request(
                self.http
                    .get(self.endpoint("api/v3/discovery/recommend/songs")),
            )
            .await?;
        let songs: Vec<WireTrack> = decode(value["data"]["dailySongs"].clone())?;
        Ok(songs.into_iter().map(Track::from).collect())
    }

    pub(crate) async fn track_like(&self, track_id: &str, like: bool) -> Result<bool> {
        let track_id = validate_id(track_id)?;
        self.request_weapi_codes(
            "weapi/radio/like",
            json!({
                "alg":"itembased", "trackId":track_id, "like":like, "time":3
            }),
            &[200],
        )
        .await?;
        Ok(like)
    }

    pub(crate) async fn playlist_create(
        &self,
        user_id: &str,
        name: &str,
        privacy: Option<u32>,
    ) -> Result<UserPlaylist> {
        let user_id = validate_id(user_id)?;
        let (name, privacy) = playlist_name(name, privacy)?;
        let value = self
            .request_weapi_codes(
                "weapi/playlist/create",
                json!({
                    "name":name, "privacy":privacy, "type":"NORMAL"
                }),
                &[200],
            )
            .await?;
        let list: WireUserPlaylist = decode(value["playlist"].clone())?;
        if list.id == 0
            || list.creator.id != user_id
            || list.track_count != 0
            || list.name.trim().is_empty()
            || list.special_type.is_some_and(|kind| kind != 0)
        {
            return Err(protocol("invalid created playlist identity or metadata"));
        }
        Ok(UserPlaylist {
            id: list.id.to_string(),
            title: list.name,
            cover_url: list.cover_img_url,
            track_count: 0,
            owner_id: user_id.to_string(),
            subscribed: false,
            is_creator: true,
            is_liked_playlist: false,
        })
    }

    pub(crate) async fn check_playlist_access(
        &self,
        playlist_id: &str,
        user_id: &str,
        owned: bool,
    ) -> Result<()> {
        let id = validate_id(playlist_id)?;
        let user_id = validate_id(user_id)?;
        let value = self
            .request(
                self.http
                    .get(self.endpoint("api/v6/playlist/detail"))
                    .query(&[("id", id.to_string()), ("n", "0".into()), ("s", "0".into())]),
            )
            .await?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Access {
            id: u64,
            creator: Creator,
            special_type: u64,
        }
        let access: Access = decode(value["playlist"].clone())?;
        if access.id != id || access.creator.id == 0 {
            return Err(protocol("playlist identity mismatch"));
        }
        if access.special_type != 0 || (access.creator.id == user_id) != owned {
            return Err(BackendError::InvalidInput(
                "playlist ownership or system playlist restriction".into(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn playlist_delete(&self, playlist_id: &str) -> Result<()> {
        let id = validate_id(playlist_id)?;
        self.request_weapi_codes(
            "weapi/playlist/remove",
            json!({"ids":json!([id]).to_string()}),
            &[200],
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn playlist_tracks_op(
        &self,
        playlist_id: &str,
        track_ids: &[u64],
        op: &str,
    ) -> Result<usize> {
        let id = validate_id(playlist_id)?;
        self.request_weapi_codes(
            "weapi/playlist/manipulate/tracks",
            json!({
                "op":op, "pid":id, "trackIds":json!(track_ids).to_string()
            }),
            &[200],
        )
        .await?;
        Ok(track_ids.len())
    }

    pub(crate) async fn playlist_subscribe(
        &self,
        playlist_id: &str,
        subscribe: bool,
    ) -> Result<()> {
        let id = validate_id(playlist_id)?;
        self.request_weapi_codes(
            "weapi/playlist/subscribe",
            json!({"id":id,"t":if subscribe {1} else {2}}),
            &[200],
        )
        .await?;
        Ok(())
    }
}
