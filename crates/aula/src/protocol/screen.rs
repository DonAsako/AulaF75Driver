use std::num::NonZeroU8;

use super::{Command, Error, Packet, SCREEN_CHUNK_LEN, begin, control, end, feature};

pub const SCREEN_WIDTH: usize = 128;
pub const SCREEN_HEIGHT: usize = 128;
pub const MAX_FRAMES: usize = 255;

/// Frame count, one delay byte per frame, `0xFF` padding.
const HEADER_LEN: usize = 256;
const FRAME_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const FRAME_LEN: usize = FRAME_PIXELS * 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenFrame {
    /// RGB565, row-major.
    pixels: Vec<u16>,
    /// `max(1, d / 2)` for a frame delay `d` in ms: 2 ms ticks, checked on hardware.
    delay: NonZeroU8,
}

impl ScreenFrame {
    pub fn new(pixels: Vec<u16>, delay: NonZeroU8) -> Result<Self, Error> {
        if pixels.len() != FRAME_PIXELS {
            return Err(Error::FrameSize {
                expected: FRAME_PIXELS,
                got: pixels.len(),
            });
        }
        Ok(Self { pixels, delay })
    }
}

pub fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    (u16::from(r) >> 3) << 11 | (u16::from(g) >> 2) << 5 | u16::from(b) >> 3
}

/// USB only. Pixels are RGB565, little-endian (checked on hardware).
pub fn upload_screen(slot: u8, frames: &[ScreenFrame]) -> Result<Vec<Packet>, Error> {
    if frames.is_empty() {
        return Err(Error::NoFrames);
    }
    if frames.len() > MAX_FRAMES {
        return Err(Error::TooManyFrames(frames.len()));
    }

    let chunks = (HEADER_LEN + frames.len() * FRAME_LEN).div_ceil(SCREEN_CHUNK_LEN);
    let mut blob = vec![0xFF; chunks * SCREEN_CHUNK_LEN];
    blob[0] = frames.len() as u8;
    for (i, frame) in frames.iter().enumerate() {
        blob[1 + i] = frame.delay.get();
        let start = HEADER_LEN + i * FRAME_LEN;
        for (bytes, pixel) in blob[start..start + FRAME_LEN]
            .chunks_exact_mut(2)
            .zip(&frame.pixels)
        {
            bytes.copy_from_slice(&pixel.to_le_bytes());
        }
    }

    let mut header = control(Command::Screen, chunks as u16);
    header[2] = slot;

    let mut packets = vec![begin(), feature(header, true)];
    packets.extend(
        blob.chunks_exact(SCREEN_CHUNK_LEN)
            .map(|chunk| Packet::ScreenChunk(Box::new(chunk.try_into().unwrap()))),
    );
    packets.push(end());
    Ok(packets)
}

#[cfg(test)]
mod tests {
    use super::super::frame;
    use super::*;

    fn solid(pixel: u16, delay: u8) -> ScreenFrame {
        ScreenFrame::new(vec![pixel; FRAME_PIXELS], NonZeroU8::new(delay).unwrap()).unwrap()
    }

    #[test]
    fn rgb565_packing() {
        assert_eq!(rgb565(0xFF, 0xFF, 0xFF), 0xFFFF);
        assert_eq!(rgb565(0xFF, 0, 0), 0xF800);
        assert_eq!(rgb565(0, 0xFF, 0), 0x07E0);
        assert_eq!(rgb565(0, 0, 0xFF), 0x001F);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            ScreenFrame::new(vec![0; 10], NonZeroU8::MIN),
            Err(Error::FrameSize {
                expected: FRAME_PIXELS,
                got: 10
            })
        );
        assert_eq!(upload_screen(1, &[]), Err(Error::NoFrames));
        assert_eq!(
            upload_screen(1, &vec![solid(0, 1); MAX_FRAMES + 1]),
            Err(Error::TooManyFrames(256))
        );
    }

    #[test]
    fn upload() {
        let packets = upload_screen(3, &[solid(0x1234, 50), solid(0xABCD, 7)]).unwrap();

        // 256 + 2 * 32768 bytes is 17 chunks.
        assert_eq!(packets.len(), 1 + 1 + 17 + 1);
        assert_eq!(
            packets[1],
            feature(frame(&[0x04, 0x72, 3, 0, 0, 0, 0, 0, 17]), true)
        );
        assert_eq!(packets[19], feature(frame(&[0x04, 0x02]), true));

        let blob: Vec<u8> = packets[2..19]
            .iter()
            .flat_map(|p| match p {
                Packet::ScreenChunk(chunk) => chunk.to_vec(),
                _ => panic!("expected a screen chunk"),
            })
            .collect();
        assert_eq!(blob[..3], [2, 50, 7]);
        assert!(blob[3..HEADER_LEN].iter().all(|&b| b == 0xFF));
        assert_eq!(blob[HEADER_LEN..][..2], [0x34, 0x12]);
        assert_eq!(blob[HEADER_LEN + FRAME_LEN..][..2], [0xCD, 0xAB]);
        assert!(
            blob[HEADER_LEN + 2 * FRAME_LEN..]
                .iter()
                .all(|&b| b == 0xFF)
        );
    }
}
