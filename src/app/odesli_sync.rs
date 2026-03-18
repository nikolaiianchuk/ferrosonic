//! Reactive odesli prefetch sync.
//!
//! `App::sync_odesli` is called every event-loop tick. It compares the
//! current song against the last one fetched and kicks off a background
//! prefetch whenever the song changes.

use super::App;

impl App {
    /// Kick off an odesli prefetch whenever the playing song changes.
    /// Must be called regularly (e.g. every event-loop tick).
    pub(super) async fn sync_odesli(&mut self) {
        let state = self.state.read().await;
        let song = state.now_playing.song.clone();
        drop(state);

        let song_id = song.as_ref().map(|s| s.id.clone());

        if song_id == self.odesli_song_id {
            return;
        }

        self.odesli_song_id = song_id;

        if let Some(ref s) = song {
            self.fetch_odesli_info(s);
        }
    }
}
