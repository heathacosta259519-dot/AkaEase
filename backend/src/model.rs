use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetail {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
    pub aliases: Vec<String>,
    pub brief_description: Option<String>,
    pub music_size: u64,
    pub album_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
    pub artist: Option<Artist>,
    pub artists: Vec<Artist>,
    pub publish_time_ms: Option<u64>,
    pub track_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetail {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
    pub artist: Option<Artist>,
    pub artists: Vec<Artist>,
    pub description: Option<String>,
    pub publish_time_ms: Option<u64>,
    pub company: Option<String>,
    pub track_count: u64,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artists: Vec<Artist>,
    pub album: Album,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub offset: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPage {
    pub id: String,
    pub title: String,
    pub tracks: Page<Track>,
    /// Missing metadata does not silently change the requested page boundaries.
    pub unavailable_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSource {
    pub track_id: String,
    pub url: String,
    pub bitrate: u32,
    pub expires_in_seconds: Option<u64>,
    pub is_preview: bool,
}
