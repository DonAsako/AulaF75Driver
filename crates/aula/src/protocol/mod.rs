//! Wire format; `docs/protocol.md` describes it byte by byte.
//!
//! Over the dongle, see [`dongle`]. Over USB a write is a transaction of
//! 64-byte feature reports:
//!
//! ```text
//! 04 18            begin
//! 04 <cmd> … <n>   header; bytes 8–9 count the data frames that follow
//! <n data frames>
//! 04 02            end
//! 04 F0            commit (some commands)
//! ```

mod clock;
pub mod dongle;
mod keymap;
mod lighting;
mod macros;
mod per_key;
mod screen;
mod settings;

use std::fmt;

pub use clock::{ClockTime, sync_clock};
pub use keymap::{
    KeyAction, Keymap, Layer, MEDIA_KEYS, MacroPlay, MouseAction, modifier, set_keymap,
};
pub use lighting::{
    Effect, KeyColors, Level, Lighting, MAX_STREAMED_KEYS, Rgb, read_key_colors, set_key_colors,
    set_lighting, stream_key_colors,
};
pub use macros::{MAX_MACROS, MIN_DELAY_MS, Macro, MacroEvent, MacroStep, MouseButton, set_macros};
pub use per_key::{MAX_SLOT, PerKey};
pub use screen::{MAX_FRAMES, SCREEN_HEIGHT, SCREEN_WIDTH, ScreenFrame, rgb565, upload_screen};
pub use settings::{Debounce, FnMode, Settings, Sleep, set_settings};

pub const PAYLOAD_LEN: usize = 64;
pub const SCREEN_CHUNK_LEN: usize = 4096;

pub type Payload = [u8; PAYLOAD_LEN];

/// How the keyboard is attached, which selects the wire format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Connection {
    Usb,
    Dongle,
}

/// A report to send. All kinds use report ID 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Packet {
    /// USB control interface.
    Feature { payload: Payload, read_reply: bool },
    /// Output report on the USB screen interface, acknowledged by one input report.
    ScreenChunk(Box<[u8; SCREEN_CHUNK_LEN]>),
    /// Output report to the dongle, acknowledged by one input report.
    Dongle(dongle::Report),
}

impl Packet {
    pub fn connection(&self) -> Connection {
        match self {
            Packet::Feature { .. } | Packet::ScreenChunk(_) => Connection::Usb,
            Packet::Dongle(_) => Connection::Dongle,
        }
    }
}

/// A read: send `request`, then read `reply_frames` 64-byte feature reports
/// after a status report, decode them with `parse`, and send `finish`.
pub struct Query<T> {
    pub request: Payload,
    pub reply_frames: usize,
    pub parse: fn(&[u8]) -> T,
    pub finish: Packet,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("key slot {0} is outside 1..={MAX_SLOT}")]
    InvalidSlot(u8),
    #[error("a screen image needs at least one frame")]
    NoFrames,
    #[error("a screen image has at most {MAX_FRAMES} frames, got {0}")]
    TooManyFrames(usize),
    #[error("a screen frame has {expected} pixels, got {got}")]
    FrameSize { expected: usize, got: usize },
    #[error("at most {MAX_MACROS} macros fit, got {0}")]
    TooManyMacros(usize),
    #[error("the macros do not fit in the keyboard's macro memory")]
    MacrosTooLong,
    #[error("at most {MAX_STREAMED_KEYS} keys can be streamed, got {0}")]
    TooManyStreamedKeys(usize),
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum Command {
    Begin = 0x18,
    BeginMacros = 0x19,
    End = 0x02,
    /// Probably persists settings to flash; unconfirmed.
    Commit = 0xF0,
    KeymapBase = 0x11,
    Lighting = 0x13,
    Macros = 0x15,
    KeyColors = 0x23,
    KeymapFn = 0x27,
    Clock = 0x28,
    Settings = 0x17,
    StreamKeyColors = 0x20,
    ReadKeyColors = 0xF5,
    Screen = 0x72,
}

const DATA_MARKER: [u8; 2] = [0xAA, 0x55];

fn control(command: Command, data_frames: u16) -> Payload {
    let mut payload = [0; PAYLOAD_LEN];
    payload[0] = 0x04;
    payload[1] = command as u8;
    payload[8..10].copy_from_slice(&data_frames.to_le_bytes());
    payload
}

fn feature(payload: Payload, read_reply: bool) -> Packet {
    Packet::Feature {
        payload,
        read_reply,
    }
}

fn begin() -> Packet {
    feature(control(Command::Begin, 0), true)
}

fn header(command: Command, data_frames: u16) -> Packet {
    feature(control(command, data_frames), true)
}

fn end() -> Packet {
    feature(control(Command::End, 0), true)
}

fn commit(read_reply: bool) -> Packet {
    feature(control(Command::Commit, 0), read_reply)
}

/// Frames of a multi-frame block are sent back to back without replies.
fn data_frames(block: &[u8]) -> impl Iterator<Item = Packet> + '_ {
    assert_eq!(block.len() % PAYLOAD_LEN, 0);
    block
        .chunks_exact(PAYLOAD_LEN)
        .map(|frame| feature(frame.try_into().unwrap(), false))
}

impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Packet::Feature {
                payload,
                read_reply,
            } => {
                let used = payload.iter().rposition(|&b| b != 0).map_or(1, |i| i + 1);
                f.write_str("feature")?;
                for byte in &payload[..used] {
                    write!(f, " {byte:02x}")?;
                }
                if *read_reply {
                    f.write_str("  (read reply)")?;
                }
                Ok(())
            }
            Packet::ScreenChunk(_) => write!(f, "screen  <{SCREEN_CHUNK_LEN} bytes>"),
            Packet::Dongle(report) => {
                f.write_str("dongle ")?;
                report.iter().try_for_each(|byte| write!(f, " {byte:02x}"))
            }
        }
    }
}

#[cfg(test)]
fn frame(prefix: &[u8]) -> Payload {
    let mut frame = [0; PAYLOAD_LEN];
    frame[..prefix.len()].copy_from_slice(prefix);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_frame() {
        assert_eq!(
            control(Command::Screen, 0x0102),
            frame(&[0x04, 0x72, 0, 0, 0, 0, 0, 0, 0x02, 0x01])
        );
    }

    #[test]
    fn display_trims_trailing_zeros() {
        assert_eq!(begin().to_string(), "feature 04 18  (read reply)");
    }
}

/// The JSON shapes the GUI's frontend builds.
#[cfg(all(test, feature = "serde"))]
mod json {
    use serde_json::{from_value, json};

    use super::*;

    #[test]
    fn shapes() {
        let action: KeyAction = from_value(json!({
            "kind": "macro",
            "value": {"index": 2, "play": {"kind": "times", "value": 3}}
        }))
        .unwrap();
        assert_eq!(
            action,
            KeyAction::Macro {
                index: 2,
                play: MacroPlay::Times(3.try_into().unwrap())
            }
        );

        let step: MacroStep =
            from_value(json!({"event": {"kind": "mouseDown", "value": "left"}, "delayMs": 30}))
                .unwrap();
        assert_eq!(step.event, MacroEvent::MouseDown(MouseButton::Left));

        let settings: Settings = from_value(json!({
            "disableAltTab": false, "disableAltF4": false, "disableWin": true,
            "fnMode": "toggle", "sleep": "fiveMinutes", "debounce": 2
        }))
        .unwrap();
        assert_eq!(settings.sleep, Sleep::FiveMinutes);

        let lighting = json!({
            "effect": "Static", "color": {"r": 255, "g": 0, "b": 0},
            "colorful": false, "brightness": 6, "speed": 3, "direction": 0
        });
        assert!(
            from_value::<Lighting>(lighting).is_err(),
            "brightness over Level::MAX"
        );
    }
}
