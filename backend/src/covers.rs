use reqwest::Url;

const BY_THE_WAY_ID: &str = "79543884";
const MISSING_BY_THE_WAY_PATH: &str = "/Uk_AkI1cDZNn1fn_jl_Snw==/18268385696067264.jpg";
const BY_THE_WAY_DELUXE_COVER: &str =
    "https://p1.music.126.net/xdCW-LmznfEJFivWyV7a_Q==/109951164116268622.jpg";

pub(crate) fn album_cover(id: &str, cover: Option<String>) -> Option<String> {
    if id != BY_THE_WAY_ID {
        return cover;
    }
    // The standard edition's old object is confirmed missing on all four CDN nodes.
    let missing = cover.as_deref().is_none_or(|cover| {
        Url::parse(cover).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && matches!(
                    url.host_str(),
                    Some(
                        "p1.music.126.net"
                            | "p2.music.126.net"
                            | "p3.music.126.net"
                            | "p4.music.126.net"
                    )
                )
                && url.path() == MISSING_BY_THE_WAY_PATH
        })
    });
    if missing {
        Some(BY_THE_WAY_DELUXE_COVER.into())
    } else {
        cover
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_the_known_missing_cover_and_keeps_future_repairs() {
        for scheme in ["http", "https"] {
            for node in 1..=4 {
                let original = format!(
                    "{scheme}://p{node}.music.126.net{MISSING_BY_THE_WAY_PATH}?param=300y300"
                );
                assert_eq!(
                    album_cover(BY_THE_WAY_ID, Some(original.clone())).as_deref(),
                    Some(BY_THE_WAY_DELUXE_COVER)
                );
                assert_eq!(
                    album_cover("1985329", Some(original.clone())),
                    Some(original)
                );
            }
        }
        assert_eq!(
            album_cover(BY_THE_WAY_ID, None).as_deref(),
            Some(BY_THE_WAY_DELUXE_COVER)
        );
        assert_eq!(album_cover("1", None), None);
        for repaired in [
            "https://p1.music.126.net/new-cover.jpg",
            BY_THE_WAY_DELUXE_COVER,
            "https://example.com/Uk_AkI1cDZNn1fn_jl_Snw==/18268385696067264.jpg",
        ] {
            assert_eq!(
                album_cover(BY_THE_WAY_ID, Some(repaired.into())).as_deref(),
                Some(repaired)
            );
        }
    }
}
