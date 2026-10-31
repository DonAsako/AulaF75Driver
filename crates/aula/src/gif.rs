//! GIF decoding for the screen.

use std::io::{BufRead, Seek};
use std::num::NonZeroU8;

use image::codecs::gif::GifDecoder;
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, ImageDecoder, Limits};

use crate::protocol::{self, MAX_FRAMES, SCREEN_HEIGHT, SCREEN_WIDTH, ScreenFrame, rgb565};

/// Largest canvas side accepted. Each decoded frame is a full RGBA canvas.
const MAX_SIDE: u32 = 2048;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    Frame(#[from] protocol::Error),
}

/// Centre-crops each frame to a square and scales it to the screen. GIF
/// delays become 2 ms ticks.
///
/// Stops after `MAX_FRAMES + 1` frames, enough for
/// [`upload_screen`](protocol::upload_screen) to reject the image.
pub fn decode(reader: impl BufRead + Seek) -> Result<Vec<ScreenFrame>, Error> {
    let mut decoder = GifDecoder::new(reader)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    decoder.set_limits(limits)?;
    decoder
        .into_frames()
        .take(MAX_FRAMES + 1)
        .map(|frame| {
            let frame = frame?;
            let (numer, denom) = frame.delay().numer_denom_ms();
            let ticks = (numer / denom.max(1) / 2).min(255) as u8;
            let buffer = frame.buffer();
            let side = buffer.width().min(buffer.height());
            let square = imageops::crop_imm(
                buffer,
                (buffer.width() - side) / 2,
                (buffer.height() - side) / 2,
                side,
                side,
            );
            let image = imageops::resize(
                &*square,
                SCREEN_WIDTH as u32,
                SCREEN_HEIGHT as u32,
                FilterType::Triangle,
            );
            let pixels = image.pixels().map(|p| rgb565(p[0], p[1], p[2])).collect();
            let delay = NonZeroU8::new(ticks).unwrap_or(NonZeroU8::MIN);
            Ok(ScreenFrame::new(pixels, delay)?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A 1x1 frame on a canvas of `width` x `height`.
    fn gif(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend(width.to_le_bytes());
        bytes.extend(height.to_le_bytes());
        // Global colour table of 2 entries, then black and white.
        bytes.extend([0x80, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF]);
        // Image descriptor at (0, 0), 1x1, then LZW data and the trailer.
        bytes.extend([0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0]);
        bytes.extend([0x02, 0x02, 0x44, 0x01, 0x00, 0x3B]);
        bytes
    }

    #[test]
    fn decodes_small_canvas() {
        let frames = decode(Cursor::new(gif(1, 1))).unwrap();
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn rejects_huge_canvas() {
        assert!(matches!(
            decode(Cursor::new(gif(20000, 20000))),
            Err(Error::Image(image::ImageError::Limits(_)))
        ));
    }
}
