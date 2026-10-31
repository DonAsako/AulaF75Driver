use std::num::NonZeroU8;

use super::per_key::{self, PerKey};
use super::{Command, Connection, Packet, dongle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Layer {
    Base,
    Fn,
}

/// HID modifier bits, as in a boot keyboard report.
pub mod modifier {
    pub const LEFT_CTRL: u8 = 0x01;
    pub const LEFT_SHIFT: u8 = 0x02;
    pub const LEFT_ALT: u8 = 0x04;
    pub const LEFT_GUI: u8 = 0x08;
    pub const RIGHT_CTRL: u8 = 0x10;
    pub const RIGHT_SHIFT: u8 = 0x20;
    pub const RIGHT_ALT: u8 = 0x40;
    pub const RIGHT_GUI: u8 = 0x80;
}

/// Mouse functions. Some software labels `0x08` "Mouse Button 5" and `0x10`
/// "Mouse Button 4"; the variants follow HID button numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum MouseAction {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
    DoubleClick,
    ScrollUp,
    ScrollDown,
}

impl MouseAction {
    fn bytes(self) -> [u8; 2] {
        const BUTTONS: u8 = 0x01;
        const WHEEL: u8 = 0x03;
        match self {
            Self::Left => [BUTTONS, 0x01],
            Self::Right => [BUTTONS, 0x02],
            Self::Middle => [BUTTONS, 0x04],
            Self::Button4 => [BUTTONS, 0x08],
            Self::Button5 => [BUTTONS, 0x10],
            Self::DoubleClick => [BUTTONS, 0x03],
            Self::ScrollUp => [WHEEL, 0x01],
            Self::ScrollDown => [WHEEL, 0xFF],
        }
    }
}

/// How a key plays its macro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "value", rename_all = "camelCase")
)]
pub enum MacroPlay {
    Once,
    Times(NonZeroU8),
    UntilPressedAgain,
    WhileHeld,
    /// "Click to play"; how it differs from `Once` is unknown.
    Click,
}

impl MacroPlay {
    fn bytes(self) -> [u8; 2] {
        match self {
            Self::Once => [0, 0],
            Self::Times(count) => [1, count.get()],
            Self::UntilPressedAgain => [2, 0],
            Self::WhileHeld => [3, 0],
            Self::Click => [4, 0],
        }
    }
}

/// The multimedia functions' consumer usages, for [`KeyAction::Consumer`].
pub const MEDIA_KEYS: [(&str, u16); 7] = [
    ("play", 0xCD),
    ("stop", 0xB7),
    ("prev", 0xB6),
    ("next", 0xB5),
    ("volup", 0xE9),
    ("voldown", 0xEA),
    ("mute", 0xE2),
];

/// What a remapped key sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "value", rename_all = "camelCase")
)]
pub enum KeyAction {
    /// The key does nothing.
    Disabled,
    /// A keyboard usage (HID page 7) with [`modifier`] bits held. A modifier
    /// usage (`0xE0..=0xE7`) is folded into the modifier bits.
    Key {
        modifiers: u8,
        usage: u8,
    },
    /// A consumer control usage (HID page 12), such as `0xCD` play/pause.
    Consumer(u16),
    Mouse(MouseAction),
    /// Up to three keyboard usages pressed together; unused ones are 0.
    MultiKey([u8; 3]),
    /// Plays the macro at this index of the list sent with [`set_macros`](super::set_macros).
    Macro {
        index: u8,
        play: MacroPlay,
    },
}

impl KeyAction {
    fn bytes(self) -> [u8; 4] {
        match self {
            Self::Disabled => [0x05, 0x03, 0, 0],
            Self::Key { modifiers, usage } => match usage {
                0xE0..=0xE7 => [0x02, modifiers | 1 << (usage - 0xE0), 0, 0],
                _ => [0x02, modifiers, usage, 0],
            },
            Self::Consumer(usage) => {
                let [lo, hi] = usage.to_le_bytes();
                [0x03, lo, hi, 0]
            }
            Self::Mouse(action) => {
                let [kind, value] = action.bytes();
                [0x01, kind, value, 0]
            }
            Self::MultiKey([a, b, c]) => [0x07, a, b, c],
            Self::Macro { index, play } => {
                let [mode, count] = play.bytes();
                [0x06, index, mode, count]
            }
        }
    }
}

/// Unset keys keep their default function.
pub type Keymap = PerKey<KeyAction>;

pub fn set_keymap(connection: Connection, layer: Layer, keymap: &Keymap) -> Vec<Packet> {
    let block = keymap.block(|_, action| action.bytes());
    match (connection, layer) {
        (Connection::Usb, Layer::Base) => per_key::transaction(Command::KeymapBase, &block),
        (Connection::Usb, Layer::Fn) => per_key::transaction(Command::KeymapFn, &block),
        (Connection::Dongle, Layer::Base) => {
            dongle::exact_chunks(dongle::command::KEYMAP_BASE, &block)
        }
        (Connection::Dongle, Layer::Fn) => dongle::exact_chunks(dongle::command::KEYMAP_FN, &block),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{feature, frame};
    use super::*;

    #[test]
    fn action_bytes() {
        let ctrl_c = KeyAction::Key {
            modifiers: modifier::LEFT_CTRL,
            usage: 0x06,
        };
        assert_eq!(ctrl_c.bytes(), [0x02, 0x01, 0x06, 0]);

        let right_shift = KeyAction::Key {
            modifiers: 0,
            usage: 0xE5,
        };
        assert_eq!(right_shift.bytes(), [0x02, 0x20, 0, 0]);

        assert_eq!(KeyAction::Consumer(0x0224).bytes(), [0x03, 0x24, 0x02, 0]);
        assert_eq!(
            KeyAction::Mouse(MouseAction::ScrollDown).bytes(),
            [0x01, 0x03, 0xFF, 0]
        );
        let three_times = MacroPlay::Times(NonZeroU8::new(3).unwrap());
        assert_eq!(
            KeyAction::Macro {
                index: 2,
                play: three_times
            }
            .bytes(),
            [0x06, 2, 1, 3]
        );
    }

    #[test]
    fn layers() {
        let keymap = Keymap::default();
        assert_eq!(
            set_keymap(Connection::Usb, Layer::Base, &keymap)[1],
            feature(frame(&[0x04, 0x11, 0, 0, 0, 0, 0, 0, 9]), true)
        );
        assert_eq!(
            set_keymap(Connection::Usb, Layer::Fn, &keymap)[1],
            feature(frame(&[0x04, 0x27, 0, 0, 0, 0, 0, 0, 9]), true)
        );
    }
}
