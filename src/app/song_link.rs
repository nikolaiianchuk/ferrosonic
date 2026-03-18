//! Odesli prefetching and copy-to-clipboard

use super::App;
use crate::discord::{Activity, DiscordMessage};
use crate::subsonic::models::Child;

impl App {
    /// Call on every song start. Sends an immediate Discord update (title + artist,
    /// no thumbnail) then spawns a background task that fetches odesli info and
    /// updates Discord again with the thumbnail once it is available.
    ///
    /// The two-send pattern is intentional: the first send is instant (no network),
    /// the second adds the cover art after the odesli fetch completes.
    pub fn notify_song_started(&self, song: &Child) {
        if let Some(ref tx) = self.discord_tx {
            let _ = tx.try_send(DiscordMessage::Update(Activity {
                details: song.title.clone(),
                state: song.artist.clone().unwrap_or_default(),
                large_image: None,
            }));
        }
        self.fetch_odesli_info(song);
    }

    /// Prefetch odesli info for a song in the background.
    /// Stores result in the odesli cache and updates Discord Rich Presence.
    /// Called on every song change so that copy_song_link is instant.
    fn fetch_odesli_info(&self, song: &Child) {
        let Some(client) = self.subsonic.clone() else { return };

        let state = self.state.clone();
        let discord_tx = self.discord_tx.clone();
        let song_id = song.id.clone();
        let song_title = song.title.clone();
        let song_artist = song.artist.clone().unwrap_or_default();

        tokio::spawn(async move {
            // Check cache first
            {
                let s = state.read().await;
                if let Some(info) = s.odesli_cache.get(&song_id) {
                    // Already cached — just update Discord
                    if let Some(tx) = &discord_tx {
                        let _ = tx.try_send(DiscordMessage::Update(Activity {
                            details: song_title,
                            state: song_artist,
                            large_image: info.thumbnail_url.clone(),
                        }));
                    }
                    return;
                }
            }

            let isrc = match client.get_isrc(&song_id).await {
                Ok(Some(i)) => i,
                _ => return,
            };

            let info = match crate::odesli::get_song_info(client.http(), &isrc).await {
                Ok(Some(i)) => i,
                _ => return,
            };

            if let Some(tx) = &discord_tx {
                let _ = tx.try_send(DiscordMessage::Update(Activity {
                    details: song_title,
                    state: song_artist,
                    large_image: info.thumbnail_url.clone(),
                }));
            }

            state.write().await.odesli_cache.insert(song_id, info);
        });
    }

    /// Copy a song.link URL for the given song to the clipboard.
    /// Uses the odesli cache populated by fetch_odesli_info, falling back to
    /// a live fetch if the cache entry is missing.
    pub fn copy_song_link(&self, song: &Child) {
        let Some(client) = self.subsonic.clone() else {
            let state = self.state.clone();
            tokio::spawn(async move {
                state.write().await.notify_error("Not connected to server");
            });
            return;
        };

        let state = self.state.clone();
        let song_id = song.id.clone();

        tokio::spawn(async move {
            // Check cache first for an instant result
            let cached_url = {
                let s = state.read().await;
                s.odesli_cache.get(&song_id).map(|i| i.page_url.clone())
            };

            let page_url = if let Some(url) = cached_url {
                url
            } else {
                let isrc = match client.get_isrc(&song_id).await {
                    Ok(Some(i)) => i,
                    Ok(None) => {
                        state.write().await.notify_error("No ISRC available for this song");
                        return;
                    }
                    Err(e) => {
                        state.write().await.notify_error(format!("Failed to look up ISRC: {}", e));
                        return;
                    }
                };

                let info = match crate::odesli::get_song_info(client.http(), &isrc).await {
                    Ok(Some(i)) => i,
                    Ok(None) => {
                        state.write().await.notify_error("Could not find song.link page for this ISRC");
                        return;
                    }
                    Err(e) => {
                        state.write().await.notify_error(format!("Failed to reach song.link: {}", e));
                        return;
                    }
                };

                let url = info.page_url.clone();
                state.write().await.odesli_cache.insert(song_id, info);
                url
            };

            let mut s = state.write().await;
            match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&page_url)) {
                Ok(()) => s.notify(format!("Copied: {}", page_url)),
                Err(e) => s.notify_error(format!("Failed to copy to clipboard: {}", e)),
            }
        });
    }
}
