//! Reactive cover art prefetch sync.
//!
//! `App::sync_cover_art` is called every event-loop tick. It prefetches cover
//! art for both the current and next song whenever either changes.

use super::App;

impl App {
    /// Kick off cover art fetches whenever the playing or next song changes.
    /// Must be called regularly (e.g. every event-loop tick).
    pub(super) async fn sync_cover_art(&mut self) {
        let state = self.state.read().await;
        let song = state.now_playing.song.clone();
        let next_song = state
            .queue_position
            .and_then(|p| state.queue.get(p + 1).cloned());
        drop(state);

        let song_id = song.as_ref().map(|s| s.id.clone());
        let next_song_id = next_song.as_ref().map(|s| s.id.clone());

        if song_id != self.cover_art_song_id {
            self.cover_art_song_id = song_id;
            if let Some(ref s) = song {
                if let Some(ref id) = s.cover_art {
                    self.fetch_cover_art(id.clone());
                }
            }
        }

        if next_song_id != self.cover_art_next_song_id {
            self.cover_art_next_song_id = next_song_id;
            if let Some(ref s) = next_song {
                if let Some(ref id) = s.cover_art {
                    self.fetch_cover_art(id.clone());
                }
            }
        }
    }
}
