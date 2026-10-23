//! 16-bit PCM WAV files, written as Python's `wave` module writes them (the
//! format the game's audio loader reads).
use std::io::Write;
use std::path::Path;

/// The 44-byte header for `data_bytes` of 16-bit samples.
pub fn header(channels: u16, rate: u32, data_bytes: u32) -> [u8; 44] {
    let mut h = [0u8; 44];
    let block = channels * 2;
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data_bytes).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&(rate * u32::from(block)).to_le_bytes());
    h[32..34].copy_from_slice(&block.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_bytes.to_le_bytes());
    h
}

pub fn write_pcm(path: &Path, channels: u16, rate: u32, samples: &[i16]) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend(header(channels, rate, (samples.len() * 2) as u32));
    bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    let mut file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}
