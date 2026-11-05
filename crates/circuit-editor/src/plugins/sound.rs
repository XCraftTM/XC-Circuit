//! Sounds editor plugins play (`xc.playSound`): one audio thread keeps the
//! output device open and plays each sound as it comes. Without an audio
//! device nothing plays, and nothing fails.

use std::io::Cursor;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock};

/// A sound file's bytes (WAV, MP3 or Ogg Vorbis) and how loud (1 = as recorded).
type Request = (Vec<u8>, f32);

static PLAYER: OnceLock<Mutex<Option<Sender<Request>>>> = OnceLock::new();

/// Plays a sound, without waiting for it.
pub fn play(bytes: Vec<u8>, volume: f32) {
    let player = PLAYER.get_or_init(|| Mutex::new(start()));
    if let Ok(player) = player.lock()
        && let Some(tx) = player.as_ref()
    {
        let _ = tx.send((bytes, volume.clamp(0.0, 1.0)));
    }
}

fn start() -> Option<Sender<Request>> {
    let (tx, rx) = channel::<Request>();
    std::thread::Builder::new()
        .name("xc-sound".into())
        .spawn(move || {
            let Ok(mut device) = rodio::DeviceSinkBuilder::open_default_sink() else { return };
            device.log_on_drop(false);
            while let Ok((bytes, volume)) = rx.recv() {
                let Ok(source) = rodio::Decoder::new(Cursor::new(bytes)) else { continue };
                let player = rodio::Player::connect_new(device.mixer());
                player.set_volume(volume as rodio::Float);
                player.append(source);
                player.detach();
            }
        })
        .ok()?;
    Some(tx)
}
