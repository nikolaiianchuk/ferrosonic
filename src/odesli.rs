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

    let page_url = match json["pageUrl"].as_str() {
        Some(u) => u.to_owned(),
        None => return Ok(None),
    };

    let thumbnail_url = json["entitiesByUniqueId"]
        .as_object()
        .and_then(|entities| {
            entities.values().find_map(|entity| {
                entity["thumbnailUrl"].as_str().map(str::to_owned)
            })
        });

    Ok(Some(OdesliInfo { page_url, thumbnail_url }))
}
