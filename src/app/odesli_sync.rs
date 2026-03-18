//! Reactive odesli prefetch sync.
//!
//! `App::sync_odesli` is called every event-loop tick. It prefetches odesli
//! info for both the current and next song whenever either changes.

use super::App;

impl App {
    /// Kick off odesli prefetches whenever the playing or next song changes.
    /// Must be called regularly (e.g. every event-loop tick).
    pub(super) async fn sync_odesli(&mut self) {
        let state = self.state.read().await;
        let song = state.now_playing.song.clone();
        let next_song = state
            .queue_position
            .and_then(|p| state.queue.get(p + 1).cloned());
        drop(state);

        let song_id = song.as_ref().map(|s| s.id.clone());
        let next_song_id = next_song.as_ref().map(|s| s.id.clone());

        if song_id != self.odesli_song_id {
            self.odesli_song_id = song_id;
            if let Some(ref s) = song {
                self.fetch_odesli_info(s);
            }
        }

        if next_song_id != self.odesli_next_song_id {
            self.odesli_next_song_id = next_song_id;
            if let Some(ref s) = next_song {
                self.fetch_odesli_info(s);
            }
        }
    }
}
