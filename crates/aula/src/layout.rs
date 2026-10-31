//! The keyboard's keys. A key has the same slot in the keymap and in the
//! per-key colours.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Key {
    pub slot: u8,
    /// HID keyboard usage. Fn's `0xAF` is vendor-specific.
    pub usage: u8,
    pub name: &'static str,
    /// `[x, y, width, height]`, `UNIT` per key unit, Esc's corner at the origin.
    pub rect: [u16; 4],
}

/// Key pitch, i.e. one key unit (1u).
const UNIT: f64 = 40.0;
/// Space between neighbouring caps.
const GAP: f64 = 6.0;

const fn key(slot: u8, usage: u8, name: &'static str, rect: [u16; 4]) -> Key {
    Key {
        slot,
        usage,
        name,
        rect,
    }
}

/// A 1u-tall cap at `col`, `row`, `width`, in key units.
const fn at(col: f64, row: f64, width: f64) -> [u16; 4] {
    [
        (col * UNIT) as u16,
        (row * UNIT) as u16,
        (width * UNIT - GAP) as u16,
        (UNIT - GAP) as u16,
    ]
}

/// Row by row, left to right. The F-row and its groups of four are set 0.25u
/// apart; the other rows span 16u.
pub const KEYS: [Key; 80] = [
    key(1, 0x29, "esc", at(0.0, 0.0, 1.0)),
    key(2, 0x3A, "f1", at(1.25, 0.0, 1.0)),
    key(3, 0x3B, "f2", at(2.25, 0.0, 1.0)),
    key(4, 0x3C, "f3", at(3.25, 0.0, 1.0)),
    key(5, 0x3D, "f4", at(4.25, 0.0, 1.0)),
    key(6, 0x3E, "f5", at(5.5, 0.0, 1.0)),
    key(7, 0x3F, "f6", at(6.5, 0.0, 1.0)),
    key(8, 0x40, "f7", at(7.5, 0.0, 1.0)),
    key(9, 0x41, "f8", at(8.5, 0.0, 1.0)),
    key(10, 0x42, "f9", at(9.75, 0.0, 1.0)),
    key(11, 0x43, "f10", at(10.75, 0.0, 1.0)),
    key(12, 0x44, "f11", at(11.75, 0.0, 1.0)),
    key(13, 0x45, "f12", at(12.75, 0.0, 1.0)),
    key(19, 0x35, "grave", at(0.0, 1.25, 1.0)),
    key(20, 0x1E, "1", at(1.0, 1.25, 1.0)),
    key(21, 0x1F, "2", at(2.0, 1.25, 1.0)),
    key(22, 0x20, "3", at(3.0, 1.25, 1.0)),
    key(23, 0x21, "4", at(4.0, 1.25, 1.0)),
    key(24, 0x22, "5", at(5.0, 1.25, 1.0)),
    key(25, 0x23, "6", at(6.0, 1.25, 1.0)),
    key(26, 0x24, "7", at(7.0, 1.25, 1.0)),
    key(27, 0x25, "8", at(8.0, 1.25, 1.0)),
    key(28, 0x26, "9", at(9.0, 1.25, 1.0)),
    key(29, 0x27, "0", at(10.0, 1.25, 1.0)),
    key(30, 0x2D, "minus", at(11.0, 1.25, 1.0)),
    key(31, 0x2E, "equal", at(12.0, 1.25, 1.0)),
    key(103, 0x2A, "backspace", at(13.0, 1.25, 2.0)),
    key(119, 0x4C, "delete", at(15.0, 1.25, 1.0)),
    key(37, 0x2B, "tab", at(0.0, 2.25, 1.5)),
    key(38, 0x14, "q", at(1.5, 2.25, 1.0)),
    key(39, 0x1A, "w", at(2.5, 2.25, 1.0)),
    key(40, 0x08, "e", at(3.5, 2.25, 1.0)),
    key(41, 0x15, "r", at(4.5, 2.25, 1.0)),
    key(42, 0x17, "t", at(5.5, 2.25, 1.0)),
    key(43, 0x1C, "y", at(6.5, 2.25, 1.0)),
    key(44, 0x18, "u", at(7.5, 2.25, 1.0)),
    key(45, 0x0C, "i", at(8.5, 2.25, 1.0)),
    key(46, 0x12, "o", at(9.5, 2.25, 1.0)),
    key(47, 0x13, "p", at(10.5, 2.25, 1.0)),
    key(48, 0x2F, "lbracket", at(11.5, 2.25, 1.0)),
    key(49, 0x30, "rbracket", at(12.5, 2.25, 1.0)),
    key(67, 0x31, "backslash", at(13.5, 2.25, 1.5)),
    key(118, 0x4B, "pageup", at(15.0, 2.25, 1.0)),
    key(55, 0x39, "capslock", at(0.0, 3.25, 1.75)),
    key(56, 0x04, "a", at(1.75, 3.25, 1.0)),
    key(57, 0x16, "s", at(2.75, 3.25, 1.0)),
    key(58, 0x07, "d", at(3.75, 3.25, 1.0)),
    key(59, 0x09, "f", at(4.75, 3.25, 1.0)),
    key(60, 0x0A, "g", at(5.75, 3.25, 1.0)),
    key(61, 0x0B, "h", at(6.75, 3.25, 1.0)),
    key(62, 0x0D, "j", at(7.75, 3.25, 1.0)),
    key(63, 0x0E, "k", at(8.75, 3.25, 1.0)),
    key(64, 0x0F, "l", at(9.75, 3.25, 1.0)),
    key(65, 0x33, "semicolon", at(10.75, 3.25, 1.0)),
    key(66, 0x34, "quote", at(11.75, 3.25, 1.0)),
    key(85, 0x28, "enter", at(12.75, 3.25, 2.25)),
    key(121, 0x4E, "pagedown", at(15.0, 3.25, 1.0)),
    key(73, 0xE1, "lshift", at(0.0, 4.25, 2.25)),
    key(74, 0x1D, "z", at(2.25, 4.25, 1.0)),
    key(75, 0x1B, "x", at(3.25, 4.25, 1.0)),
    key(76, 0x06, "c", at(4.25, 4.25, 1.0)),
    key(77, 0x19, "v", at(5.25, 4.25, 1.0)),
    key(78, 0x05, "b", at(6.25, 4.25, 1.0)),
    key(79, 0x11, "n", at(7.25, 4.25, 1.0)),
    key(80, 0x10, "m", at(8.25, 4.25, 1.0)),
    key(81, 0x36, "comma", at(9.25, 4.25, 1.0)),
    key(82, 0x37, "period", at(10.25, 4.25, 1.0)),
    key(83, 0x38, "slash", at(11.25, 4.25, 1.0)),
    key(84, 0xE5, "rshift", at(12.25, 4.25, 1.75)),
    key(101, 0x52, "up", at(14.0, 4.25, 1.0)),
    key(120, 0x4D, "end", at(15.0, 4.25, 1.0)),
    key(91, 0xE0, "lctrl", at(0.0, 5.25, 1.25)),
    key(92, 0xE3, "lwin", at(1.25, 5.25, 1.25)),
    key(93, 0xE2, "lalt", at(2.5, 5.25, 1.25)),
    key(94, 0x2C, "space", at(3.75, 5.25, 6.25)),
    key(95, 0xE6, "ralt", at(10.0, 5.25, 1.25)),
    key(96, 0xAF, "fn", at(11.25, 5.25, 1.25)),
    key(99, 0x50, "left", at(13.0, 5.25, 1.0)),
    key(100, 0x51, "down", at(14.0, 5.25, 1.0)),
    key(102, 0x4F, "right", at(15.0, 5.25, 1.0)),
];

pub fn by_name(name: &str) -> Option<Key> {
    KEYS.iter()
        .copied()
        .find(|key| key.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MAX_SLOT;
    use std::collections::HashSet;

    #[test]
    fn slots_and_names_are_unique_and_in_range() {
        let slots: HashSet<_> = KEYS.iter().map(|k| k.slot).collect();
        let names: HashSet<_> = KEYS.iter().map(|k| k.name).collect();
        assert_eq!(slots.len(), KEYS.len());
        assert_eq!(names.len(), KEYS.len());
        assert!(slots.iter().all(|&s| (1..=MAX_SLOT).contains(&s)));
    }

    fn overlaps(a: [u16; 4], b: [u16; 4]) -> bool {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }

    #[test]
    fn keys_do_not_overlap() {
        for (i, a) in KEYS.iter().enumerate() {
            for b in &KEYS[i + 1..] {
                assert!(!overlaps(a.rect, b.rect), "{} overlaps {}", a.name, b.name);
            }
        }
    }

    #[test]
    fn rows_run_left_to_right_and_span_the_board() {
        let right_edge = (16.0 * UNIT - GAP) as u16;
        for row in KEYS.chunk_by(|a, b| a.rect[1] == b.rect[1]) {
            assert!(row.windows(2).all(|w| w[0].rect[0] < w[1].rect[0]));
            let last = row[row.len() - 1].rect;
            if row[0].name == "esc" {
                assert!(last[0] + last[2] <= right_edge);
            } else {
                assert_eq!(row[0].rect[0], 0);
                assert_eq!(last[0] + last[2], right_edge, "row of {}", row[0].name);
            }
        }
    }

    #[test]
    fn lookup_ignores_case() {
        assert_eq!(by_name("Esc").map(|k| k.slot), Some(1));
        assert_eq!(by_name("nope"), None);
    }
}
