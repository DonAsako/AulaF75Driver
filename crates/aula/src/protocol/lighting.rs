use std::fmt;

use super::per_key::{self, BLOCK_LEN, MAX_SLOT, PerKey};
use super::{
    Command, Connection, DATA_MARKER, Error, PAYLOAD_LEN, Packet, Payload, Query, begin, commit,
    control, data_frames, dongle, end, feature, header,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Brightness or speed step, `0..=5`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "u8", into = "u8"))]
pub struct Level(u8);

impl Level {
    pub const MAX: u8 = 5;

    pub fn new(value: u8) -> Option<Self> {
        (value <= Self::MAX).then_some(Self(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Level {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, String> {
        Self::new(value).ok_or_else(|| format!("expected 0..={}", Self::MAX))
    }
}

impl From<Level> for u8 {
    fn from(level: Level) -> u8 {
        level.0
    }
}

/// Global effects. The discriminant is the mode byte; checked on hardware for
/// modes 1–3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Effect {
    Static = 1,
    SingleOn,
    SingleOff,
    Glittering,
    Falling,
    Colourful,
    Breath,
    Spectrum,
    Outward,
    Scrolling,
    Rolling,
    Rotating,
    Explode,
    Launch,
    Ripples,
    Flowing,
    Pulsating,
    Tilt,
    Shuttle,
}

impl Effect {
    pub const ALL: [Self; 19] = [
        Self::Static,
        Self::SingleOn,
        Self::SingleOff,
        Self::Glittering,
        Self::Falling,
        Self::Colourful,
        Self::Breath,
        Self::Spectrum,
        Self::Outward,
        Self::Scrolling,
        Self::Rolling,
        Self::Rotating,
        Self::Explode,
        Self::Launch,
        Self::Ripples,
        Self::Flowing,
        Self::Pulsating,
        Self::Tilt,
        Self::Shuttle,
    ];
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// The payload offsets of `colorful`, `speed` and `direction` are unconfirmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lighting {
    /// `None` turns the lights off.
    pub effect: Option<Effect>,
    pub color: Rgb,
    pub colorful: bool,
    pub brightness: Level,
    pub speed: Level,
    pub direction: u8,
}

pub fn set_lighting(connection: Connection, lighting: &Lighting) -> Vec<Packet> {
    let mut data = [0; PAYLOAD_LEN];
    if let Some(effect) = lighting.effect {
        data[0] = effect as u8;
        let Rgb { r, g, b } = lighting.color;
        data[1..4].copy_from_slice(&[r, g, b]);
        data[8] = lighting.colorful.into();
        data[9] = lighting.brightness.get();
        data[10] = lighting.speed.get();
        data[11] = lighting.direction;
    }
    data[14..16].copy_from_slice(&DATA_MARKER);
    match connection {
        Connection::Usb => lighting_transaction(data),
        Connection::Dongle => vec![dongle::short(dongle::command::LIGHTING, &data)],
    }
}

pub type KeyColors = PerKey<Rgb>;

const PER_KEY_MODE: u8 = 0x80;

/// Switches to per-key mode, then sends the colours. Each key is
/// `[slot, r, g, b]` at offset `4 * slot`.
pub fn set_key_colors(
    connection: Connection,
    brightness: Level,
    colors: &KeyColors,
) -> Vec<Packet> {
    let mut mode = [0; PAYLOAD_LEN];
    mode[0] = PER_KEY_MODE;
    mode[9] = brightness.get();
    mode[14..16].copy_from_slice(&DATA_MARKER);
    let block = colors.block(|slot, Rgb { r, g, b }| [slot, r, g, b]);

    match connection {
        Connection::Usb => {
            let mut packets = lighting_transaction(mode);
            packets.extend(per_key::transaction(Command::KeyColors, &block));
            packets
        }
        Connection::Dongle => {
            let mut packets = vec![dongle::short(dongle::command::LIGHTING, &mode)];
            packets.extend(dongle::exact_chunks(dongle::command::KEY_COLORS, &block));
            packets
        }
    }
}

/// The real-time block holds this many `[slot, r, g, b]` entries.
pub const MAX_STREAMED_KEYS: usize = 128;

const STREAM_BLOCK_LEN: usize = 4 * MAX_STREAMED_KEYS;

/// Shows colours immediately without storing them; meant
/// to be sent repeatedly with [`Keyboard::stream`](crate::Keyboard::stream). Entries are packed
/// in slot order rather than indexed by slot.
pub fn stream_key_colors(colors: &KeyColors) -> Result<Vec<Packet>, Error> {
    let count = colors.iter().count();
    if count > MAX_STREAMED_KEYS {
        return Err(Error::TooManyStreamedKeys(count));
    }
    let mut block = [0; STREAM_BLOCK_LEN];
    for (entry, (slot, Rgb { r, g, b })) in block.chunks_exact_mut(4).zip(colors.iter()) {
        entry.copy_from_slice(&[slot, r, g, b]);
    }

    let mut packets = vec![header(
        Command::StreamKeyColors,
        (STREAM_BLOCK_LEN / PAYLOAD_LEN) as u16,
    )];
    packets.extend(data_frames(&block));
    packets.push(end());
    Ok(packets)
}

/// Reads the stored per-key colours.
pub fn read_key_colors() -> Query<KeyColors> {
    Query {
        request: control(Command::ReadKeyColors, (BLOCK_LEN / PAYLOAD_LEN) as u16),
        reply_frames: BLOCK_LEN / PAYLOAD_LEN,
        parse: |block| {
            let mut colors = KeyColors::default();
            for slot in 1..=MAX_SLOT {
                let at = 4 * usize::from(slot);
                let [r, g, b] = block[at + 1..at + 4].try_into().unwrap();
                colors.set(slot, Rgb { r, g, b }).unwrap();
            }
            colors
        },
        finish: end(),
    }
}

fn lighting_transaction(data: Payload) -> Vec<Packet> {
    vec![
        begin(),
        header(Command::Lighting, 1),
        feature(data, false),
        end(),
        commit(false),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::frame;
    use super::*;

    fn level(value: u8) -> Level {
        Level::new(value).unwrap()
    }

    fn with_marker(prefix: &[u8]) -> Payload {
        let mut data = frame(prefix);
        data[14..16].copy_from_slice(&DATA_MARKER);
        data
    }

    #[test]
    fn level_bounds() {
        assert_eq!(Level::new(5).map(Level::get), Some(5));
        assert_eq!(Level::new(6), None);
    }

    #[test]
    fn effect_mode_bytes() {
        assert!(Effect::ALL.iter().map(|&e| e as u8).eq(1..=19));
    }

    #[test]
    fn lighting() {
        let lighting = Lighting {
            effect: Some(Effect::Rolling),
            color: Rgb {
                r: 0x11,
                g: 0x22,
                b: 0x33,
            },
            colorful: true,
            brightness: level(4),
            speed: level(3),
            direction: 1,
        };
        assert_eq!(
            set_lighting(Connection::Usb, &lighting),
            [
                feature(frame(&[0x04, 0x18]), true),
                feature(frame(&[0x04, 0x13, 0, 0, 0, 0, 0, 0, 1]), true),
                feature(
                    with_marker(&[11, 0x11, 0x22, 0x33, 0, 0, 0, 0, 1, 4, 3, 1]),
                    false
                ),
                feature(frame(&[0x04, 0x02]), true),
                feature(frame(&[0x04, 0xF0]), false),
            ]
        );
    }

    #[test]
    fn lighting_off_zeroes_other_fields() {
        let lighting = Lighting {
            effect: None,
            color: Rgb {
                r: 0xFF,
                g: 0xFF,
                b: 0xFF,
            },
            colorful: true,
            brightness: level(5),
            speed: level(5),
            direction: 3,
        };
        assert_eq!(
            set_lighting(Connection::Usb, &lighting)[2],
            feature(with_marker(&[]), false)
        );
    }

    #[test]
    fn stream() {
        let mut colors = KeyColors::default();
        colors.set(5, Rgb { r: 1, g: 2, b: 3 }).unwrap();
        colors.set(1, Rgb { r: 4, g: 5, b: 6 }).unwrap();
        let packets = stream_key_colors(&colors).unwrap();

        assert_eq!(packets.len(), 1 + 8 + 1);
        assert_eq!(
            packets[0],
            feature(frame(&[0x04, 0x20, 0, 0, 0, 0, 0, 0, 8]), true)
        );
        assert_eq!(packets[1], feature(frame(&[1, 4, 5, 6, 5, 1, 2, 3]), false));
    }

    #[test]
    fn read_back() {
        let query = read_key_colors();
        assert_eq!(query.request, frame(&[0x04, 0xF5, 0, 0, 0, 0, 0, 0, 9]));
        let mut block = [0; BLOCK_LEN];
        block[4 * 121..4 * 122].copy_from_slice(&[121, 7, 8, 9]);
        let colors = (query.parse)(&block);
        assert_eq!(colors.get(121), Some(Rgb { r: 7, g: 8, b: 9 }));
        assert_eq!(colors.get(1), Some(Rgb::default()));
    }

    #[test]
    fn dongle_lighting() {
        let lighting = Lighting {
            effect: Some(Effect::Breath),
            color: Rgb { r: 1, g: 2, b: 3 },
            colorful: false,
            brightness: level(5),
            speed: level(3),
            direction: 0,
        };
        let [Packet::Dongle(report)] = &set_lighting(Connection::Dongle, &lighting)[..] else {
            panic!("expected one dongle packet");
        };
        assert_eq!(
            report[..16],
            [0x05, 0x10, 0, 7, 1, 2, 3, 0, 0, 0, 0, 0, 5, 3, 0, 0]
        );
        assert_eq!(report[17..19], DATA_MARKER);
    }

    #[test]
    fn dongle_key_colors() {
        let packets = set_key_colors(Connection::Dongle, level(4), &KeyColors::default());
        assert_eq!(packets.len(), 1 + 21);
    }

    #[test]
    fn key_colors() {
        let mut colors = KeyColors::default();
        colors.set(121, Rgb { r: 1, g: 2, b: 3 }).unwrap();
        let packets = set_key_colors(Connection::Usb, level(2), &colors);

        assert_eq!(packets.len(), 5 + 13);
        assert_eq!(
            packets[2],
            feature(with_marker(&[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 2]), false)
        );
        assert_eq!(
            packets[6],
            feature(frame(&[0x04, 0x23, 0, 0, 0, 0, 0, 0, 9]), true)
        );
        assert_eq!(
            packets[14].to_string(),
            format!("feature{}", " 00".repeat(36) + " 79 01 02 03")
        );
    }
}
