//! 2.4G dongle framing. Every packet is one 32-byte
//! output report, `[command, b1, b2, 28 data bytes, checksum]`, acknowledged
//! by an input report echoing its first three bytes.

use super::{DATA_MARKER, Packet, Payload};

pub const REPORT_LEN: usize = 32;

pub type Report = [u8; REPORT_LEN];

const CHUNK_LEN: usize = 28;
/// Wired data bytes carried by a single-packet command, before the marker.
const SHORT_DATA_LEN: usize = 14;

pub(super) mod command {
    pub const LIGHTING: u8 = 0x05;
    pub const SETTINGS: u8 = 0x07;
    pub const MACROS: u8 = 0x09;
    pub const CLOCK: u8 = 0x0C;
    pub const KEYMAP_BASE: u8 = 0x10;
    pub const KEYMAP_FN: u8 = 0x12;
    pub const KEY_COLORS: u8 = 0x14;
    pub const BATTERY: u8 = 0x20;
}

fn report(head: [u8; 3], data: &[u8]) -> Packet {
    let mut report = [0; REPORT_LEN];
    report[..3].copy_from_slice(&head);
    report[3..3 + data.len()].copy_from_slice(data);
    report[REPORT_LEN - 1] = report.iter().fold(0, |sum: u8, &b| sum.wrapping_add(b));
    Packet::Dongle(report)
}

/// A wired single-frame command: its first 14 data bytes and the marker.
pub(super) fn short(command: u8, data: &Payload) -> Packet {
    let mut short = [0; SHORT_DATA_LEN + 2];
    short[..SHORT_DATA_LEN].copy_from_slice(&data[..SHORT_DATA_LEN]);
    short[SHORT_DATA_LEN..].copy_from_slice(&DATA_MARKER);
    report([command, 0x10, 0], &short)
}

/// Splits `block` into 28-byte chunks `[command, length, index]`. The last
/// chunk declares `last_len`, which is not always derived from the block
/// size (see `set_macros`).
pub(super) fn chunks(command: u8, block: &[u8], last_len: usize) -> Vec<Packet> {
    let count = block.len().div_ceil(CHUNK_LEN);
    block
        .chunks(CHUNK_LEN)
        .enumerate()
        .map(|(index, chunk)| {
            let len = if index + 1 == count {
                last_len
            } else {
                CHUNK_LEN
            };
            report([command, len as u8, index as u8], chunk)
        })
        .collect()
}

/// [`chunks`] with the last chunk declaring its actual length.
pub(super) fn exact_chunks(command: u8, block: &[u8]) -> Vec<Packet> {
    let last_len = (block.len() - 1) % CHUNK_LEN + 1;
    chunks(command, block, last_len)
}

/// Pads a macro block: one spare chunk,
/// the marker at the very end. Returns the padded block and the last chunk's
/// declared length.
pub(super) fn pad_with_spare_chunk(mut block: Vec<u8>) -> (Vec<u8>, usize) {
    let len = block.len();
    let count = len / CHUNK_LEN + if len % CHUNK_LEN == 0 { 1 } else { 2 };
    block.resize(count * CHUNK_LEN, 0);
    let marker_at = block.len() - 2;
    block[marker_at..].copy_from_slice(&DATA_MARKER);
    (block, len % CHUNK_LEN)
}

/// Reply byte 3 is the battery percentage.
pub fn read_battery() -> Packet {
    report([command::BATTERY, 0x01, 0], &[])
}

/// The battery level in percent from the reply to [`read_battery`].
pub fn battery_percent(reply: &Report) -> Option<u8> {
    (reply[0] == command::BATTERY && reply[3] != 0).then(|| reply[3].min(100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum() {
        let Packet::Dongle(battery) = read_battery() else {
            unreachable!()
        };
        assert_eq!(battery[..4], [0x20, 0x01, 0, 0]);
        assert_eq!(battery[REPORT_LEN - 1], 0x21);
    }

    #[test]
    fn short_command() {
        let mut data = [0; 64];
        data[..3].copy_from_slice(&[1, 2, 3]);
        data[62..].copy_from_slice(&DATA_MARKER);
        let Packet::Dongle(packet) = short(command::LIGHTING, &data) else {
            unreachable!()
        };
        assert_eq!(packet[..6], [0x05, 0x10, 0, 1, 2, 3]);
        assert_eq!(packet[17..19], DATA_MARKER);
    }

    #[test]
    fn chunking() {
        let block = [7; 576];
        let packets = exact_chunks(command::KEYMAP_BASE, &block);
        assert_eq!(packets.len(), 21);
        let Packet::Dongle(last) = &packets[20] else {
            unreachable!()
        };
        assert_eq!(last[..3], [0x10, 16, 20]);
        assert_eq!(last[3..19], [7; 16]);
        assert_eq!(last[19..31], [0; 12]);
    }

    #[test]
    fn spare_chunk() {
        let (block, last_len) = pad_with_spare_chunk(vec![1; 30]);
        assert_eq!(block.len(), 3 * CHUNK_LEN);
        assert_eq!(last_len, 2);
        assert_eq!(block[82..], DATA_MARKER);
    }

    #[test]
    fn battery_reply() {
        let mut reply = [0; REPORT_LEN];
        reply[..4].copy_from_slice(&[0x20, 0x01, 0, 87]);
        assert_eq!(battery_percent(&reply), Some(87));
        reply[3] = 0;
        assert_eq!(battery_percent(&reply), None);
    }
}
