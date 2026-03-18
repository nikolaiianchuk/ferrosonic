//! Discord Rich Presence via IPC

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use tracing::{debug, info, warn};

static NONCE: AtomicU64 = AtomicU64::new(0);

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
}

/// Messages sent to the Discord background thread
pub enum DiscordMessage {
    Update(Activity),
    Clear,
}

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;

fn socket_path() -> Option<String> {
    // Candidate base directories, tried in order:
    //   $XDG_RUNTIME_DIR                              — standard Linux
    //   $XDG_RUNTIME_DIR/app/com.discordapp.Discord   — Discord Flatpak on Linux
    //   $TMPDIR                                       — macOS
    //   /tmp                                          — fallback
    let mut dirs: Vec<String> = Vec::new();
    if let Ok(d) = std::env::var("XDG_RUNTIME_DIR") {
        dirs.push(format!("{}/app/com.discordapp.Discord", d));
        dirs.push(d);
    }
    if let Ok(d) = std::env::var("TMPDIR") {
        dirs.push(d.trim_end_matches('/').to_string());
    }
    dirs.push("/tmp".to_string());

    for dir in &dirs {
        for i in 0..10 {
            let path = format!("{}/discord-ipc-{}", dir, i);
            if std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }
    }
    None
}

fn write_frame(stream: &mut UnixStream, op: u32, payload: &str) -> std::io::Result<()> {
    let bytes = payload.as_bytes();
    let len = bytes.len() as u32;
    let mut buf = Vec::with_capacity(8 + bytes.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(bytes);
    stream.write_all(&buf)
}

fn read_frame(stream: &mut UnixStream) -> std::io::Result<serde_json::Value> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;
    let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    match serde_json::from_slice(&payload) {
        Ok(v) => Ok(v),
        Err(e) => {
            warn!("Discord: failed to parse frame payload: {}", e);
            Ok(serde_json::Value::Null)
        }
    }
}

fn connect(client_id: u64) -> Option<UnixStream> {
    let path = match socket_path() {
        Some(p) => { info!("Discord IPC socket: {}", p); p }
        None => { warn!("Discord IPC socket not found"); return None; }
    };

    let mut stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => { warn!("Discord socket connect failed: {}", e); return None; }
    };

    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok()?;

    let handshake = serde_json::json!({"v": 1, "client_id": client_id.to_string()});
    if let Err(e) = write_frame(&mut stream, OP_HANDSHAKE, &handshake.to_string()) {
        warn!("Discord handshake write failed: {}", e);
        return None;
    }

    match read_frame(&mut stream) {
        Ok(response) => {
            debug!("Discord handshake response: {}", response);
            if response["evt"].as_str() == Some("READY") {
                info!("Discord RPC connected (app_id={})", client_id);
                Some(stream)
            } else {
                warn!("Discord RPC unexpected handshake response: {}", response);
                None
            }
        }
        Err(e) => { warn!("Discord handshake read failed: {}", e); None }
    }
}

fn send_activity(stream: &mut UnixStream, activity: &Activity) -> std::io::Result<()> {
    let pid = std::process::id();
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed).to_string();

    let mut act = serde_json::json!({
        "type": 2,  // 2 = Listening (shows "Listening to <app name>")
        "details": activity.details,
        "state": activity.state,
    });
    if let Some(ref img) = activity.large_image {
        act["assets"] = serde_json::json!({ "large_image": img });
    }
    match (activity.start_timestamp, activity.end_timestamp) {
        (Some(start), Some(end)) => {
            act["timestamps"] = serde_json::json!({ "start": start, "end": end });
        }
        (Some(start), None) => {
            act["timestamps"] = serde_json::json!({ "start": start });
        }
        _ => {}
    }

    let payload = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": pid, "activity": act },
        "nonce": nonce,
    });

    debug!("Discord SET_ACTIVITY: {} - {}", activity.details, activity.state);
    write_frame(stream, OP_FRAME, &payload.to_string())?;
    let response = read_frame(stream)?;
    debug!("Discord SET_ACTIVITY response: {}", response);
    Ok(())
}

fn clear_activity(stream: &mut UnixStream) -> std::io::Result<()> {
    let pid = std::process::id();
    let nonce = NONCE.fetch_add(1, Ordering::Relaxed).to_string();

    let payload = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": pid, "activity": null },
        "nonce": nonce,
    });

    write_frame(stream, OP_FRAME, &payload.to_string())?;
    read_frame(stream)?;
    Ok(())
}

/// Start the Discord Rich Presence background thread.
/// Returns a sender for sending activity updates.
pub fn start_discord_thread(client_id: u64) -> mpsc::SyncSender<DiscordMessage> {
    let (tx, rx) = mpsc::sync_channel::<DiscordMessage>(32);

    std::thread::spawn(move || {
        let mut stream: Option<UnixStream> = None;

        loop {
            let msg = match rx.recv() {
                Ok(m) => m,
                Err(_) => break, // sender dropped, shut down
            };

            if stream.is_none() {
                stream = connect(client_id);
                if stream.is_none() {
                    debug!("Discord not available, skipping activity update");
                    continue;
                }
            }

            if let Some(ref mut s) = stream {
                let result = match &msg {
                    DiscordMessage::Update(a) => send_activity(s, a),
                    DiscordMessage::Clear => clear_activity(s),
                };
                if let Err(e) = result {
                    warn!("Discord RPC error: {}", e);
                    stream = None;
                }
            }
        }

        info!("Discord RPC thread exiting");
    });

    tx
}
