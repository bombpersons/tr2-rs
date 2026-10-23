//! The CD audio tracks (2 onwards; track 1 is data) as stereo 44.1 kHz WAV
//! files, `music/NN.wav`, each holding every sector its track stores
//! (the two-second pregap included, as the disc image has it).
use std::io::{BufWriter, Write};
use std::path::Path;

use psx_media::cue::TrackMode;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{DiscReader, ImportProgress, wav};

const RATE: u32 = 44_100;
/// Bytes of 16-bit stereo audio per second.
const BYTES_PER_SECOND: f64 = 176_400.0;

#[derive(Serialize)]
struct Track {
    track: u8,
    path: String,
    seconds: f64,
    source_sha256: String,
}

pub fn import(disc: &DiscReader, out: &Path, progress: &ImportProgress) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let tracks: Vec<_> = disc.disc.tracks.iter().filter(|t| t.mode == TrackMode::Audio).collect();
    let mut manifest = Vec::new();
    for (i, track) in tracks.iter().enumerate() {
        progress.set(i as f32 / tracks.len() as f32, format!("Music track {} of {}", i + 1, tracks.len()));
        let path = format!("{:02}.wav", track.number);
        let bytes = track.sectors * 2352;
        let file = out.join(&path);
        let error = |e: std::io::Error| format!("{}: {e}", file.display());
        let mut writer = BufWriter::new(std::fs::File::create(&file).map_err(error)?);
        writer.write_all(&wav::header(2, RATE, bytes)).map_err(error)?;
        let mut hash = Sha256::new();
        for lba in track.first_lba..track.first_lba + track.sectors {
            let (sector, _) = disc.disc.read_raw(lba).map_err(|e| format!("track {}: {e}", track.number))?;
            hash.update(&sector);
            writer.write_all(&sector).map_err(error)?;
        }
        writer.flush().map_err(error)?;
        manifest.push(Track {
            track: track.number,
            path,
            seconds: f64::from(bytes) / BYTES_PER_SECOND,
            source_sha256: hash.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        });
    }
    let json = serde_json::to_string_pretty(&manifest).unwrap() + "\n";
    std::fs::write(out.join("manifest.json"), json).map_err(|e| e.to_string())
}
