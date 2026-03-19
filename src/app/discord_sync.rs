//! Reactive Discord Rich Presence sync.
//!
//! `App::sync_discord` is called every event-loop tick. It compares the
//! current NowPlaying state against what was last sent to Discord and only
//! sends a new activity when something actually changed (song, play state,
//! or a thumbnail arriving in the odesli cache).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::App;
use crate::app::state::PlaybackState;
use crate::discord::{Activity, DiscordMessage};

impl App {
    /// Sync Discord Rich Presence with the current playback state.
    /// Must be called regularly (e.g. every event-loop tick).
    pub(super) async fn sync_discord(&mut self) {
        let Some(ref tx) = self.discord_tx else { return };

        let state = self.state.read().await;
        let play_state = state.now_playing.state;
        let song = state.now_playing.song.clone();
        let position = state.now_playing.position;
        let duration = state.now_playing.duration;
        let odesli_seq = state.odesli_cache_seq;

        let song_id = song.as_ref().map(|s| s.id.clone());
        let song_changed = song_id != self.discord_song_id;
        let state_changed = play_state != self.discord_play_state;
        let thumbnail_arrived = odesli_seq != self.discord_odesli_seq
            && play_state != PlaybackState::Stopped;

        if !song_changed && !state_changed && !thumbnail_arrived {
            return;
        }

        // Thumbnail and song.link URL from odesli cache (present once fetch_odesli_info completes)
        let (thumbnail, song_link) = song.as_ref().map(|s| {
            let info = state.odesli_cache.get(&s.id);
            (
                info.and_then(|i| i.thumbnail_url.clone()),
                info.map(|i| i.page_url.clone()),
            )
        }).unwrap_or((None, None));
        drop(state);

        match play_state {
            PlaybackState::Stopped => {
                let _ = tx.try_send(DiscordMessage::Clear);
                self.discord_song_id = None;
                self.discord_play_state = PlaybackState::Stopped;
                self.discord_track_start = None;
            }

            PlaybackState::Playing => {
                // Recalculate the "position 0" wall-clock time when song or resume
                if song_changed || self.discord_play_state != PlaybackState::Playing {
                    self.discord_track_start = SystemTime::now()
                        .checked_sub(Duration::from_secs_f64(position));
                }

                let start_timestamp = self.discord_track_start
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());

                if let Some(ref s) = song {
                    let end_timestamp = start_timestamp
                        .filter(|_| duration > 0.0)
                        .map(|t| t + duration.round() as u64);
                    let _ = tx.try_send(DiscordMessage::Update(Activity {
                        details: s.title.clone(),
                        state: s.artist.clone().unwrap_or_default(),
                        large_image: thumbnail,
                        start_timestamp,
                        end_timestamp,
                        song_link: song_link.clone(),
                    }));
                }

                self.discord_song_id = song_id;
                self.discord_play_state = PlaybackState::Playing;
                self.discord_odesli_seq = odesli_seq;
            }

            PlaybackState::Paused => {
                let _ = tx.try_send(DiscordMessage::Clear);
                self.discord_track_start = None;
                self.discord_song_id = song_id;
                self.discord_play_state = PlaybackState::Paused;
                self.discord_odesli_seq = odesli_seq;
            }
        }
    }
}
