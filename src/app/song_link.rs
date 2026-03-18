//! Copy song.link URL to clipboard via ISRC lookup

use super::App;

impl App {
    /// Copy a song.link URL for the given song to the clipboard using its ISRC.
    /// Spawns a background task so the UI is not blocked during network calls.
    pub fn copy_song_link(&self, song: &crate::subsonic::models::Child) {
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

            let page_url = match crate::odesli::get_page_url(client.http(), &isrc).await {
                Ok(Some(u)) => u,
                Ok(None) => {
                    state.write().await.notify_error("Could not find song.link page for this ISRC");
                    return;
                }
                Err(e) => {
                    state.write().await.notify_error(format!("Failed to reach song.link: {}", e));
                    return;
                }
            };

            let mut s = state.write().await;
            match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&page_url)) {
                Ok(()) => s.notify(format!("Copied: {}", page_url)),
                Err(e) => s.notify_error(format!("Failed to copy to clipboard: {}", e)),
            }
        });
    }
}
