//! Reactive cover art prefetch sync.
//!
//! `App::sync_cover_art` is called every event-loop tick. It triggers a
//! background fetch whenever the now-playing song changes and its cover art
//! is not yet cached.

use super::App;

impl App {
    /// Kick off a cover art fetch whenever the playing song changes.
    /// Must be called regularly (e.g. every event-loop tick).
    pub(super) async fn sync_cover_art(&mut self) {
        let state = self.state.read().await;
        let song = state.now_playing.song.clone();
        drop(state);

        let song_id = song.as_ref().map(|s| s.id.clone());

        if song_id == self.cover_art_song_id {
            return;
        }

        self.cover_art_song_id = song_id;

        if let Some(ref s) = song {
            if let Some(ref id) = s.cover_art {
                self.fetch_cover_art(id.clone());
            }
        }
    }
}
