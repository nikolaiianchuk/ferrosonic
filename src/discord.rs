//! Discord Rich Presence via discord-rich-presence crate

use std::sync::mpsc;

use discord_rich_presence::activity::{
    Activity as DrpcActivity, ActivityType, Assets, Button, Timestamps,
};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use tracing::{debug, info, warn};

/// Activity to display in Discord
pub struct Activity {
    /// Song title (shown as "details" in Discord)
    pub details: String,
    /// Artist name (shown as "state" in Discord)
    pub state: String,
    /// Thumbnail image URL
    pub large_image: Option<String>,
    /// Unix timestamp of when playback started at position 0.
    /// Combined with end_timestamp, Discord renders "0:23 / 3:45".
    pub start_timestamp: Option<u64>,
    /// Unix timestamp of when the track will end.
    /// Combined with start_timestamp, Discord renders the full X:XX / Y:YY display.
    pub end_timestamp: Option<u64>,
    /// Optional song.link URL shown as a clickable button below the activity.
    pub song_link: Option<String>,
}

/// Messages sent to the Discord background thread
pub enum DiscordMessage {
    Update(Activity),
    Clear,
}

/// Start the Discord Rich Presence background thread.
/// Returns a sender for sending activity updates.
pub fn start_discord_thread(client_id: u64) -> mpsc::SyncSender<DiscordMessage> {
    let (tx, rx) = mpsc::sync_channel::<DiscordMessage>(32);

    std::thread::spawn(move || {
        let client_id_str = client_id.to_string();
        let mut client: Option<DiscordIpcClient> = None;

        loop {
            let msg = match rx.recv() {
                Ok(m) => m,
                Err(_) => break, // sender dropped, shut down
            };

            if client.is_none() {
                let mut c = DiscordIpcClient::new(&client_id_str);
                match c.connect() {
                    Ok(()) => {
                        info!("Discord RPC connected (app_id={})", client_id);
                        client = Some(c);
                    }
                    Err(e) => {
                        debug!("Discord not available, skipping activity update: {}", e);
                        continue;
                    }
                }
            }

            if let Some(ref mut c) = client {
                let result = match &msg {
                    DiscordMessage::Update(a) => {
                        let mut act = DrpcActivity::new()
                            .activity_type(ActivityType::Listening)
                            .details(&a.details)
                            .state(&a.state);

                        if let Some(ref img) = a.large_image {
                            act = act.assets(Assets::new().large_image(img));
                        }

                        match (a.start_timestamp, a.end_timestamp) {
                            (Some(start), Some(end)) => {
                                act = act.timestamps(
                                    Timestamps::new().start(start as i64).end(end as i64),
                                );
                            }
                            (Some(start), None) => {
                                act = act.timestamps(Timestamps::new().start(start as i64));
                            }
                            _ => {}
                        }

                        if let Some(ref url) = a.song_link {
                            act = act.buttons(vec![Button::new("Open on song.link", url)]);
                        }

                        debug!("Discord SET_ACTIVITY: {} - {}", a.details, a.state);
                        c.set_activity(act)
                    }
                    DiscordMessage::Clear => c.clear_activity(),
                };

                if let Err(e) = result {
                    warn!("Discord RPC error: {}", e);
                    client = None;
                }
            }
        }

        if let Some(mut c) = client {
            let _ = c.close();
        }
        info!("Discord RPC thread exiting");
    });

    tx
}
