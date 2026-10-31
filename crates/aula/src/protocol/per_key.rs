use super::{
    Command, DATA_MARKER, Error, PAYLOAD_LEN, Packet, begin, commit, data_frames, end, header,
};

/// The block's trailing marker overlaps slot 143.
pub const MAX_SLOT: u8 = 142;

pub(super) const BLOCK_LEN: usize = 576;

/// One value per key slot ([`crate::layout`]). Unset slots are sent as zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerKey<T>([Option<T>; MAX_SLOT as usize + 1]);

impl<T: Copy> Default for PerKey<T> {
    fn default() -> Self {
        Self([None; MAX_SLOT as usize + 1])
    }
}

impl<T: Copy> PerKey<T> {
    pub fn set(&mut self, slot: u8, value: T) -> Result<(), Error> {
        if !(1..=MAX_SLOT).contains(&slot) {
            return Err(Error::InvalidSlot(slot));
        }
        self.0[usize::from(slot)] = Some(value);
        Ok(())
    }

    pub fn from_slots(slots: impl IntoIterator<Item = (u8, T)>) -> Result<Self, Error> {
        let mut per_key = Self::default();
        for (slot, value) in slots {
            per_key.set(slot, value)?;
        }
        Ok(per_key)
    }

    pub fn get(&self, slot: u8) -> Option<T> {
        self.0.get(usize::from(slot)).copied().flatten()
    }

    /// Set slots in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = (u8, T)> + '_ {
        (1..=MAX_SLOT).filter_map(|slot| Some((slot, self.get(slot)?)))
    }

    /// Each set slot is encoded as 4 bytes at offset `4 * slot`.
    pub(super) fn block(&self, encode: impl Fn(u8, T) -> [u8; 4]) -> Block {
        let mut block = [0; BLOCK_LEN];
        for (slot, value) in self.iter() {
            let at = 4 * usize::from(slot);
            block[at..at + 4].copy_from_slice(&encode(slot, value));
        }
        block[BLOCK_LEN - 2..].copy_from_slice(&DATA_MARKER);
        block
    }
}

pub(super) type Block = [u8; BLOCK_LEN];

/// Sends a per-key block over USB.
pub(super) fn transaction(command: Command, block: &Block) -> Vec<Packet> {
    let mut packets = vec![begin(), header(command, (BLOCK_LEN / PAYLOAD_LEN) as u16)];
    packets.extend(data_frames(block));
    packets.push(end());
    packets.push(commit(true));
    packets
}

#[cfg(test)]
mod tests {
    use super::super::{feature, frame};
    use super::*;

    #[test]
    fn slot_bounds() {
        let mut keys = PerKey::default();
        assert_eq!(keys.set(0, ()), Err(Error::InvalidSlot(0)));
        assert_eq!(keys.set(143, ()), Err(Error::InvalidSlot(143)));
        assert_eq!(keys.set(142, ()), Ok(()));
    }

    #[test]
    fn wired_transaction() {
        let mut keys = PerKey::default();
        keys.set(1, 0xAA).unwrap();
        keys.set(121, 0xBB).unwrap();
        let packets = transaction(Command::KeyColors, &keys.block(|slot, v| [slot, v, 0, v]));

        assert_eq!(packets.len(), 13);
        assert_eq!(
            packets[1],
            feature(frame(&[0x04, 0x23, 0, 0, 0, 0, 0, 0, 9]), true)
        );
        assert_eq!(packets[12], feature(frame(&[0x04, 0xF0]), true));

        let mut block = [0; BLOCK_LEN];
        block[4..8].copy_from_slice(&[1, 0xAA, 0, 0xAA]);
        block[484..488].copy_from_slice(&[121, 0xBB, 0, 0xBB]);
        block[574..].copy_from_slice(&DATA_MARKER);
        assert!(
            packets[2..11]
                .iter()
                .eq(data_frames(&block).collect::<Vec<_>>().iter())
        );
    }
}
