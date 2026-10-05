use crate::{
    api::{NeteaseClient, WireTrack, decode, pagination, validate_id},
    error::Result,
    model::Track,
};
use serde::{Deserialize, Serialize};

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
        struct Creator {
            #[serde(rename = "userId")]
            id: u64,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct List {
            id: u64,
            name: String,
            cover_img_url: Option<String>,
            track_count: u64,
            creator: Creator,
            subscribed: Option<bool>,
            #[serde(rename = "specialType")]
            special_type: Option<u64>,
        }
        #[derive(Deserialize)]
        struct Response {
            playlist: Vec<List>,
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
}
