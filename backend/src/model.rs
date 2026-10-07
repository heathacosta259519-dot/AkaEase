use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SoundQuality {
    Standard,
    Higher,
    #[default]
    Exhigh,
    Lossless,
    Hires,
}

impl SoundQuality {
    pub fn level(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Higher => "higher",
            Self::Exhigh => "exhigh",
            Self::Lossless => "lossless",
            Self::Hires => "hires",
        }
    }

    pub(crate) fn from_source(
        bitrate: u32,
        format: Option<&str>,
        level: Option<Self>,
    ) -> Option<Self> {
        // A returned level may echo the request; lossy bitrate takes precedence.
        let lossless = format.is_some_and(|f| matches!(f, "flac" | "alac" | "wav" | "ape"));
        if !lossless && bitrate > 0 && bitrate <= 320_000 {
            return Some(if bitrate <= 128_000 {
                Self::Standard
            } else if bitrate <= 192_000 {
                Self::Higher
            } else {
                Self::Exhigh
            });
        }
        if lossless || bitrate > 320_000 {
            return Some(if level == Some(Self::Hires) {
                Self::Hires
            } else {
                Self::Lossless
            });
        }
        level
    }
}

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
    pub quality: Option<SoundQuality>,
    pub format: Option<String>,
    pub expires_in_seconds: Option<u64>,
    pub is_preview: bool,
}
