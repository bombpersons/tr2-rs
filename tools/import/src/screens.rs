//! `PIX/*.RAW` and `PIXUS/*.RAW`: title, loading and credit screens,
//! 256-line RGB555 pictures compressed with RNC method 2 (headerless, or
//! with an `RNC\x02` header). First decoded by `tools/extract_pix.py`
//! (bitstream after OpenLara's `LoadRNC`, before this project's reference
//! restriction), ported here as it was.
use image::RgbImage;

pub type Result<T> = std::result::Result<T, String>;

/// Bits read most significant first, sharing their byte stream with the
/// literal bytes between them.
struct Bits<'a> {
    bytes: &'a [u8],
    at: usize,
    left: u32,
    current: u8,
}

impl Bits<'_> {
    fn byte(&mut self) -> Result<u8> {
        let byte = *self.bytes.get(self.at).ok_or_else(|| format!("compressed data ends at {}", self.at))?;
        self.at += 1;
        Ok(byte)
    }
    fn bit(&mut self) -> Result<bool> {
        if self.left == 0 {
            self.current = self.byte()?;
            self.left = 8;
        }
        self.left -= 1;
        Ok((self.current >> self.left) & 1 != 0)
    }
    fn bits(&mut self, n: u32) -> Result<usize> {
        (0..n).try_fold(0, |v, _| Ok((v << 1) | usize::from(self.bit()?)))
    }
    /// A match distance: a variable-length high part, then a byte.
    fn offset(&mut self) -> Result<usize> {
        let mut v = 0;
        if self.bit()? {
            v = usize::from(self.bit()?);
            if self.bit()? {
                v = ((v << 1) | usize::from(self.bit()?)) | 4;
                if !self.bit()? {
                    v = (v << 1) | usize::from(self.bit()?);
                }
            } else if v == 0 {
                v = usize::from(self.bit()?) + 2;
            }
        }
        Ok(((v << 8) | usize::from(self.byte()?)) + 1)
    }
}

/// RNC method 2. Returns the decoded bytes, a whole number of 256-pixel columns.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let (mut size, mut data) = (2048 * 2048 * 2, data);
    if data.starts_with(b"RNC\x02") && data.len() >= 18 {
        let word = |at: usize| u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let packed = word(8);
        size = word(4);
        data = &data[18..(18 + packed).min(data.len())];
    }
    let bits = &mut Bits { bytes: data, at: 0, left: 0, current: 0 };
    bits.bits(2)?;
    let mut out = Vec::new();
    while bits.at < data.len() && out.len() < size {
        if !bits.bit()? {
            out.push(bits.byte()?);
            continue;
        }
        let (n, offset);
        if bits.bit()? {
            if bits.bit()? {
                if bits.bit()? {
                    let count = usize::from(bits.byte()?) + 8;
                    if count == 8 {
                        if !bits.bit()? {
                            break;
                        }
                        continue;
                    }
                    n = count;
                } else {
                    n = 3;
                }
                offset = bits.offset()?;
            } else {
                n = 2;
                offset = usize::from(bits.byte()?) + 1;
            }
        } else {
            let mut count = usize::from(bits.bit()?) + 4;
            if bits.bit()? {
                count = ((count - 1) << 1) + usize::from(bits.bit()?);
            }
            if count == 9 {
                for _ in 0..bits.bits(4)? * 4 + 12 {
                    out.push(bits.byte()?);
                }
                continue;
            }
            n = count;
            offset = bits.offset()?;
        }
        if offset == 0 || offset > out.len() {
            return Err(format!("match distance {offset} with {} bytes out", out.len()));
        }
        for _ in 0..n {
            out.push(out[out.len() - offset]);
        }
    }
    if out.len() % 512 != 0 {
        return Err(format!("{} decoded bytes are not whole columns", out.len()));
    }
    Ok(out)
}

/// The decoded screen as RGB, 256 lines high.
pub fn picture(data: &[u8]) -> Result<RgbImage> {
    let raw = decompress(data)?;
    let width = (raw.len() / 512) as u32;
    let mut pixels = Vec::with_capacity(raw.len() / 2 * 3);
    for c in raw.as_chunks::<2>().0.iter().map(|&w| u16::from_le_bytes(w)) {
        pixels.extend([0, 5, 10].map(|shift| (u32::from((c >> shift) & 31) * 255 / 31) as u8));
    }
    RgbImage::from_raw(width, 256, pixels).ok_or_else(|| "picture size".into())
}

/// `PIXUS/TITLEUS.RAW` -> `pixus_titleus.png`.
pub fn file_name(disc_path: &str) -> String {
    let mut parts = disc_path.trim_start_matches('/').rsplit('/');
    let stem = parts.next().unwrap_or_default();
    let stem = stem.rsplit_once('.').map_or(stem, |(stem, _)| stem);
    format!("{}_{}.png", parts.next().unwrap_or_default(), stem).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_then_a_two_byte_match() {
        // Two skipped bits; literal 'A' (bit 0); literal 'B'; a match of 2
        // (bits 1 1 0) from distance byte+1 = 2: "ABAB". A whole 512-byte
        // column is too long for a test, so check the error names the
        // short output instead.
        let stream = [0b0000_1100, b'A', b'B', 1];
        let error = decompress(&stream).unwrap_err();
        assert!(error.contains("4 decoded bytes"), "{error}");
    }

    #[test]
    fn names_follow_the_folder_and_stem() {
        assert_eq!(file_name("/PIXUS/TITLEUS.RAW"), "pixus_titleus.png");
        assert_eq!(file_name("/PIX/ASSAULT.RAW"), "pix_assault.png");
    }
}
