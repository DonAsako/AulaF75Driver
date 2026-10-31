use super::{
    Command, Connection, DATA_MARKER, Error, PAYLOAD_LEN, Packet, control, data_frames, dongle,
    end, feature, header,
};

pub const MAX_MACROS: usize = 100;
/// Shorter delays are clamped up to this.
pub const MIN_DELAY_MS: u16 = 10;

const TABLE_LEN: usize = 4 * MAX_MACROS;
const ENTRY_HEADER_LEN: usize = 8;
/// Largest macro block.
const MAX_BLOCK_LEN: usize = 0xE00;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum MouseButton {
    Left = 0x01,
    Right = 0x02,
    Middle = 0x04,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "kind", content = "value", rename_all = "camelCase")
)]
pub enum MacroEvent {
    /// Keyboard usage (HID page 7).
    KeyDown(u8),
    KeyUp(u8),
    MouseDown(MouseButton),
    MouseUp(MouseButton),
}

/// An event followed by a pause before the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct MacroStep {
    pub event: MacroEvent,
    pub delay_ms: u16,
}

pub type Macro = Vec<MacroStep>;

/// Keys refer to a macro by its
/// index in `macros`.
pub fn set_macros(connection: Connection, macros: &[Macro]) -> Result<Vec<Packet>, Error> {
    let block = macro_block(macros)?;
    // Both connections pad with a spare frame or chunk before the marker.
    match connection {
        Connection::Usb => {
            let mut block = block;
            let len = block.len();
            let frames = len / PAYLOAD_LEN + if len % PAYLOAD_LEN == 0 { 1 } else { 2 };
            block.resize(frames * PAYLOAD_LEN, 0);
            if block.len() > MAX_BLOCK_LEN {
                return Err(Error::MacrosTooLong);
            }
            let marker_at = block.len() - 2;
            block[marker_at..].copy_from_slice(&DATA_MARKER);

            let mut packets = vec![
                feature(control(Command::BeginMacros, 0), true),
                header(Command::Macros, frames as u16),
            ];
            packets.extend(data_frames(&block));
            packets.push(end());
            Ok(packets)
        }
        Connection::Dongle => {
            let (block, last_len) = dongle::pad_with_spare_chunk(block);
            if block.len() > MAX_BLOCK_LEN {
                return Err(Error::MacrosTooLong);
            }
            Ok(dongle::chunks(dongle::command::MACROS, &block, last_len))
        }
    }
}

/// A table of [`MAX_MACROS`] 4-byte entries holding each macro's `u16`
/// offset (`FF FF FF FF` for an empty macro), then per macro a `u16` record
/// count, 6 zero bytes and 4-byte records. Every event record is followed by
/// a delay record.
fn macro_block(macros: &[Macro]) -> Result<Vec<u8>, Error> {
    if macros.len() > MAX_MACROS {
        return Err(Error::TooManyMacros(macros.len()));
    }

    let mut block = vec![0; TABLE_LEN];
    for (index, steps) in macros.iter().enumerate() {
        let entry = 4 * index..4 * index + 4;
        if steps.is_empty() {
            block[entry].fill(0xFF);
            continue;
        }
        let offset = u16::try_from(block.len()).map_err(|_| Error::MacrosTooLong)?;
        block[entry][..2].copy_from_slice(&offset.to_le_bytes());

        let records = (2 * steps.len()) as u16;
        block.extend(records.to_le_bytes());
        block.extend([0; ENTRY_HEADER_LEN - 2]);
        for step in steps {
            block.extend(event_record(step.event));
            let [lo, hi] = step.delay_ms.max(MIN_DELAY_MS).to_le_bytes();
            block.extend([lo, hi, 0, 0x50]);
        }
    }

    Ok(block)
}

fn event_record(event: MacroEvent) -> [u8; 4] {
    match event {
        MacroEvent::KeyDown(usage) => [0, 0, usage, 0xB0],
        MacroEvent::KeyUp(usage) => [0, 0, usage, 0x30],
        MacroEvent::MouseDown(button) => [0, 0, button as u8, 0x90],
        MacroEvent::MouseUp(button) => [0, 0, button as u8, 0x10],
    }
}

#[cfg(test)]
mod tests {
    use super::super::frame;
    use super::*;

    fn step(event: MacroEvent, delay_ms: u16) -> MacroStep {
        MacroStep { event, delay_ms }
    }

    #[test]
    fn one_macro() {
        let typed_a = vec![
            step(MacroEvent::KeyDown(0x04), 50),
            step(MacroEvent::KeyUp(0x04), 3),
        ];
        let packets = set_macros(Connection::Usb, &[vec![], typed_a]).unwrap();

        // 400-byte table + 8-byte header + 4 records = 424 bytes: 6 full frames + 2.
        assert_eq!(packets[0], feature(frame(&[0x04, 0x19]), true));
        assert_eq!(
            packets[1],
            feature(frame(&[0x04, 0x15, 0, 0, 0, 0, 0, 0, 8]), true)
        );
        assert_eq!(packets.len(), 2 + 8 + 1);

        let mut block = vec![0; 8 * PAYLOAD_LEN];
        block[..4].fill(0xFF);
        block[4..6].copy_from_slice(&400u16.to_le_bytes());
        block[400] = 4;
        block[408..424].copy_from_slice(&[
            0, 0, 0x04, 0xB0, 50, 0, 0, 0x50, //
            0, 0, 0x04, 0x30, 10, 0, 0, 0x50,
        ]);
        block[510..].copy_from_slice(&DATA_MARKER);
        assert!(
            packets[2..10]
                .iter()
                .eq(data_frames(&block).collect::<Vec<_>>().iter())
        );
    }

    #[test]
    fn spare_frame_when_aligned() {
        // 400 + 8 + 5 * 8 = 448 bytes, exactly 7 frames, plus the spare one.
        let steps = vec![step(MacroEvent::KeyDown(0x04), 10); 5];
        let packets = set_macros(Connection::Usb, &[steps]).unwrap();
        assert_eq!(
            packets[1],
            feature(frame(&[0x04, 0x15, 0, 0, 0, 0, 0, 0, 8]), true)
        );
    }

    #[test]
    fn dongle_chunks() {
        // 424 bytes = 15 chunks + 4 bytes: 17 chunks, the last declaring 4.
        let typed_a = vec![
            step(MacroEvent::KeyDown(0x04), 50),
            step(MacroEvent::KeyUp(0x04), 3),
        ];
        let packets = set_macros(Connection::Dongle, &[vec![], typed_a]).unwrap();
        assert_eq!(packets.len(), 17);
        let Packet::Dongle(last) = &packets[16] else {
            unreachable!()
        };
        assert_eq!(last[..3], [0x09, 4, 16]);
        assert_eq!(last[29..31], DATA_MARKER);
    }

    #[test]
    fn limits() {
        assert_eq!(
            set_macros(Connection::Usb, &vec![vec![]; 101]),
            Err(Error::TooManyMacros(101))
        );
        let long = vec![step(MacroEvent::KeyDown(0x04), 10); 400];
        assert_eq!(
            set_macros(Connection::Usb, &[long]),
            Err(Error::MacrosTooLong)
        );
    }
}
