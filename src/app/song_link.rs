//! Odesli prefetching and copy-to-clipboard

use super::App;
use crate::subsonic::models::Child;

impl App {
    /// Prefetch odesli info for a song in the background.
    /// Stores the result in odesli_cache and bumps odesli_cache_seq so that
    /// sync_discord picks up the thumbnail on its next tick.
    pub fn fetch_odesli_info(&self, song: &Child) {
        let Some(client) = self.subsonic.clone() else { return };

        let state = self.state.clone();
        let song_id = song.id.clone();

        tokio::spawn(async move {
            // Skip if already cached
            if state.read().await.odesli_cache.contains_key(&song_id) {
                return;
            }

            let isrc = match client.get_isrc(&song_id).await {
                Ok(Some(i)) => i,
                _ => return,
            };

            let info = match crate::odesli::get_song_info(client.http(), &isrc).await {
                Ok(Some(i)) => i,
                _ => return,
            };

            let mut s = state.write().await;
            s.odesli_cache.insert(song_id, info);
            s.odesli_cache_seq += 1;
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
                let mut s = state.write().await;
                s.odesli_cache.insert(song_id, info);
                s.odesli_cache_seq += 1;
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
