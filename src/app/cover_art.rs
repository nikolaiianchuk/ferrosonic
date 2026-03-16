//! Cover art fetching and caching

use tracing::debug;

use super::App;

impl App {
    /// Spawn a background task to fetch and decode a cover art image.
    /// Returns immediately; the cache is populated when the task completes.
    /// No-op if already cached or no Subsonic client is available.
    pub(super) fn fetch_cover_art(&self, cover_art_id: String) {
        let Some(client) = self.subsonic.clone() else {
            return;
        };
        let state = self.state.clone();

        tokio::spawn(async move {
            // Skip if another task already cached it
            {
                let s = state.read().await;
                if s.cover_art_cache.contains_key(&cover_art_id) {
                    return;
                }
            }

            match client.get_cover_art(&cover_art_id, 300).await {
                Ok(bytes) => match image::load_from_memory(&bytes) {
                    Ok(img) => {
                        let mut s = state.write().await;
                        s.cover_art_cache.insert(cover_art_id.clone(), img);
                        debug!("Cached cover art: {}", cover_art_id);
                    }
                    Err(e) => debug!("Failed to decode cover art {}: {}", cover_art_id, e),
                },
                Err(e) => debug!("Failed to fetch cover art {}: {}", cover_art_id, e),
            }
        });
    }
}
