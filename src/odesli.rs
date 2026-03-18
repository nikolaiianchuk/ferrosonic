//! Odesli (song.link) API client

use crate::error::SubsonicError;

/// Resolve an ISRC to a song.link page URL via the odesli API.
pub async fn get_page_url(http: &reqwest::Client, isrc: &str) -> Result<Option<String>, SubsonicError> {
    let mut url = reqwest::Url::parse("https://api.song.link/v1-alpha.1/links")
        .map_err(|e| SubsonicError::UrlParse(e))?;
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
    Ok(json["pageUrl"].as_str().map(str::to_owned))
}
