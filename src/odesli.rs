//! Odesli (song.link) API client

use crate::error::SubsonicError;

/// Information retrieved from the odesli (song.link) API
#[derive(Debug, Clone)]
pub struct OdesliInfo {
    /// song.link page URL
    pub page_url: String,
    /// Thumbnail image URL (from the first available entity)
    pub thumbnail_url: Option<String>,
}

/// Resolve an ISRC to song.link info via the odesli API.
pub async fn get_song_info(http: &reqwest::Client, isrc: &str) -> Result<Option<OdesliInfo>, SubsonicError> {
    let mut url = reqwest::Url::parse("https://api.song.link/v1-alpha.1/links")
        .map_err(SubsonicError::UrlParse)?;
    url.query_pairs_mut()
        .append_pair("platform", "isrc")
        .append_pair("type", "song")
        .append_pair("id", isrc);

    let response = http.get(url).send().await?;
    let status = response.status();

    if !status.is_success() {
        return Err(SubsonicError::Api {
            code: status.as_u16() as i32,
            message: format!("odesli returned {}", status),
        });
    }

    let json: serde_json::Value = response.json().await?;

    let entities = match json["entitiesByUniqueId"].as_object() {
        Some(e) => e,
        None => return Ok(None),
    };

    // Priority: Spotify > iTunes > Tidal > Amazon
    // Build song.link short URLs from the entity id
    let page_url = 'found: {
        for (prefix, base) in &[
            // ("SPOTIFY_SONG::", "https://song.link/s/"), <-- Recently there's been an issue with spotify-based IDs, so this will fall back to other services
            ("ITUNES_SONG::", "https://song.link/i/"),
            ("TIDAL_SONG::", "https://song.link/t/"),
            ("AMAZON_SONG::", "https://song.link/a/"),
        ] {
            if let Some(entity) = entities.keys().find(|k| k.starts_with(prefix))
                .and_then(|k| entities.get(k))
            {
                if let Some(id) = entity["id"].as_str() {
                    break 'found format!("{}{}", base, id);
                }
            }
        }
        // Fall back to pageUrl if no preferred platform found
        match json["pageUrl"].as_str() {
            Some(u) => u.to_owned(),
            None => return Ok(None),
        }
    };

    let thumbnail_url = entities.values().find_map(|entity| {
        entity["thumbnailUrl"].as_str().map(str::to_owned)
    });

    Ok(Some(OdesliInfo { page_url, thumbnail_url }))
}
