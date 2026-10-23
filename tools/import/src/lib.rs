//! Importers: the player's Tomb Raider II disc -> engine-native assets under
//! `game/assets/imported/`. The game runs [`import`] itself on first start
//! when it finds the disc (`game/src/disc.rs`); `cargo run -p import -- DISC`
//! runs the same code by hand.
//!
//! The original's raw formats live only here: [`level_file`] decodes each
//! `DATA/*.PSX` into raw tables, [`convert`] re-expresses them exactly in the
//! engine's level format, [`screens`] decodes the RNC-compressed pictures
//! and [`music`] copies the CD audio tracks; [`gameflow`] reads the names and
//! strings the game shows. Each output records where it came from (`source`
//! in `level.json`, `manifest.json` beside the screens and music).
pub use psx_assets;
pub use psx_media;

pub mod convert;
pub mod gameflow;
pub mod level_file;
pub mod music;
pub mod raw;
pub mod screens;
mod wav;

use psx_media::Disc;
use psx_media::iso9660::{Entry, FileSystem};
use psx_media::system_cnf::{DEFAULT_BOOT, SystemCnf};
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// The original game's name, for the "put your disc next to the game" screen.
pub const GAME: &str = "Tomb Raider II (PlayStation, USA)";

/// Accepted discs, by the boot executable named in `SYSTEM.CNF`. The
/// importers were written against the USA disc, revision 3.
pub const SERIALS: &[&str] = &["SLUS_004.37"];

/// Bump whenever [`import`]'s output changes: players' older imports are
/// redone on the next start.
pub const VERSION: u32 = 3;

/// Converts the disc into `out` (an empty folder that becomes
/// `game/assets/imported/`): `levels/<name>/` (level, atlas, sounds),
/// `screens/`, `music/` and `gameflow.json`.
pub fn import(disc: &DiscReader, out: &Path, progress: &ImportProgress) -> Result<(), String> {
    import_levels(disc, &out.join("levels"), None, &progress.part(0.0, 0.7))?;
    import_screens(disc, &out.join("screens"), &progress.part(0.7, 0.72))?;
    import_gameflow(disc, out)?;
    music::import(disc, &out.join("music"), &progress.part(0.72, 1.0))
}

/// Every `DATA/*.PSX` level (or only `only`, case-insensitive) into
/// `out/<name>/`, with a `manifest.json` of what each holds.
pub fn import_levels(
    disc: &DiscReader,
    out: &Path,
    only: Option<&str>,
    progress: &ImportProgress,
) -> Result<(), String> {
    let files: Vec<&Entry> = disc
        .files_in("/DATA", "PSX")
        .into_iter()
        .filter(|e| only.is_none_or(|name| stem(&e.path).eq_ignore_ascii_case(name)))
        .collect();
    if files.is_empty() {
        return Err("no DATA/*.PSX levels on the disc".into());
    }
    let mut manifest = Vec::new();
    for (i, entry) in files.iter().enumerate() {
        let name = stem(&entry.path).to_ascii_lowercase();
        progress.set(i as f32 / files.len() as f32, format!("Level {} of {}: {name}", i + 1, files.len()));
        let bytes = disc.read_entry(entry)?;
        manifest.push(import_level(&name, &bytes, &out.join(&name)).map_err(|e| format!("{}: {e}", entry.path))?);
    }
    if only.is_none() {
        write(&out.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap() + "\n")?;
    }
    Ok(())
}

/// One level file into `dir`: `level.json`, `atlas.png` and `audio/NNN.wav`.
pub fn import_level(name: &str, bytes: &[u8], dir: &Path) -> Result<serde_json::Value, String> {
    let file = level_file::decode(&name.to_ascii_uppercase(), bytes)?;
    let level = convert::level(&file.level)?;
    std::fs::create_dir_all(dir.join("audio")).map_err(|e| format!("{}: {e}", dir.display()))?;
    file.atlas.save(dir.join("atlas.png")).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (i, pcm) in file.samples.iter().enumerate() {
        wav::write_pcm(&dir.join(format!("audio/{i:03}.wav")), 1, level_file::SAMPLE_RATE, pcm)?;
    }
    let json = serde_json::to_vec(&level).map_err(|e| e.to_string())?;
    write(&dir.join("level.json"), &json)?;
    Ok(serde_json::json!({
        "level": name,
        "source": level.source,
        "rooms": level.rooms.len(),
        "floor_infos": level.floor_infos.len(),
        "triggers": level.floor_infos.iter().map(|f| f.triggers.len()).sum::<usize>(),
        "animations": level.animations.len(),
        "placements": level.placements.len(),
        "samples": file.samples.len(),
        "level_json_bytes": json.len(),
    }))
}

/// The pictures in `PIX/` and `PIXUS/` as PNGs, with a `manifest.json`.
pub fn import_screens(disc: &DiscReader, out: &Path, progress: &ImportProgress) -> Result<(), String> {
    #[derive(Serialize)]
    struct Screen {
        source: String,
        path: String,
        width: u32,
        height: u32,
        source_sha256: String,
    }
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let files: Vec<&Entry> = ["/PIX", "/PIXUS"].iter().flat_map(|dir| disc.files_in(dir, "RAW")).collect();
    let mut manifest = Vec::new();
    for (i, entry) in files.iter().enumerate() {
        progress.set(i as f32 / files.len() as f32, format!("Screen {}", entry.path));
        let bytes = disc.read_entry(entry)?;
        let picture = screens::picture(&bytes).map_err(|e| format!("{}: {e}", entry.path))?;
        let path = screens::file_name(&entry.path);
        picture.save(out.join(&path)).map_err(|e| format!("{path}: {e}"))?;
        manifest.push(Screen {
            source: entry.path.trim_start_matches('/').to_string(),
            path,
            width: picture.width(),
            height: picture.height(),
            source_sha256: psx_media::disc::sha256_bytes(&bytes),
        });
    }
    write(&out.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap() + "\n")
}

/// The gameflow script's strings as `gameflow.json`.
pub fn import_gameflow(disc: &DiscReader, out: &Path) -> Result<(), String> {
    let bytes = disc.read(gameflow::PATH)?;
    let flow = gameflow::decode(&bytes).map_err(|e| format!("{}: {e}", gameflow::PATH))?;
    write(&out.join("gameflow.json"), serde_json::to_string_pretty(&flow).unwrap() + "\n")
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// A disc image opened for import.
pub struct DiscReader {
    pub path: std::path::PathBuf,
    pub disc: Disc,
    pub fs: FileSystem,
    /// The boot executable's name (`SLUS_004.37`), or `PSX.EXE` without `SYSTEM.CNF`.
    pub serial: String,
}

impl DiscReader {
    /// Opens a `.cue`, `.chd`, `.iso` or bare `.bin` and reads its file system.
    pub fn open(path: &Path) -> Result<Self, String> {
        let error = |e: psx_media::MediaError| format!("{}: {e}", path.display());
        let disc = Disc::open(path).map_err(error)?;
        let fs = FileSystem::read(&disc).map_err(error)?;
        let boot = match fs.find("/SYSTEM.CNF") {
            Some(entry) => SystemCnf::parse(&fs.read_file(&disc, entry).map_err(error)?).boot,
            None => None,
        };
        let serial = serial_of(&boot.unwrap_or_else(|| DEFAULT_BOOT.into()));
        Ok(Self { path: path.to_path_buf(), disc, fs, serial })
    }

    /// Whether this is a disc [`SERIALS`] accepts.
    pub fn accepted(&self) -> bool {
        SERIALS.is_empty() || SERIALS.iter().any(|s| s.eq_ignore_ascii_case(&self.serial))
    }

    /// A file's bytes by ISO9660 path (`/DATA/WALL.PSX`).
    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let entry = self.fs.entry(path).map_err(|e| e.to_string())?;
        self.read_entry(entry)
    }

    pub fn read_entry(&self, entry: &Entry) -> Result<Vec<u8>, String> {
        self.fs.read_file(&self.disc, entry).map_err(|e| format!("{}: {e}", entry.path))
    }

    /// Every file and directory on the disc.
    pub fn entries(&self) -> &[Entry] {
        &self.fs.entries
    }

    /// The files directly in `dir` (`/DATA`) with this extension, by path.
    pub fn files_in(&self, dir: &str, extension: &str) -> Vec<&Entry> {
        let mut files: Vec<&Entry> = self
            .entries()
            .iter()
            .filter(|e| !e.is_dir)
            .filter(|e| e.path.rsplit_once('/').is_some_and(|(parent, _)| parent.eq_ignore_ascii_case(dir)))
            .filter(|e| e.path.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case(extension)))
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }
}

/// `cdrom:\SLUS_004.37;1` -> `SLUS_004.37`.
pub fn serial_of(boot: &str) -> String {
    let name = boot.rsplit(['\\', '/', ':']).next().unwrap_or(boot);
    name.split(';').next().unwrap_or(name).to_ascii_uppercase()
}

/// Builds tiny ISO images for tests: `test_disc::iso("VOL", &[("SYSTEM.CNF", b"BOOT = ...")])`.
pub mod test_disc {
    const SECTOR: usize = 2048;

    fn both32(out: &mut [u8], v: u32) {
        out[..4].copy_from_slice(&v.to_le_bytes());
        out[4..8].copy_from_slice(&v.to_be_bytes());
    }

    fn record(lba: u32, size: u32, dir: bool, name: &[u8]) -> Vec<u8> {
        let len = (33 + name.len() + 1) & !1;
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        both32(&mut r[2..10], lba);
        both32(&mut r[10..18], size);
        r[18..25].copy_from_slice(&[99, 1, 1, 0, 0, 0, 0]);
        r[25] = if dir { 2 } else { 0 };
        r[28..30].copy_from_slice(&1u16.to_le_bytes());
        r[30..32].copy_from_slice(&1u16.to_be_bytes());
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    /// A Mode 1 (`.iso`) image with these files in its root directory.
    pub fn iso(volume_id: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut lba = 19u32;
        let mut placed = Vec::new();
        for (name, bytes) in files {
            placed.push((format!("{name};1"), lba, *bytes));
            lba += bytes.len().div_ceil(SECTOR).max(1) as u32;
        }
        let mut image = vec![0u8; lba as usize * SECTOR];
        let pvd = &mut image[16 * SECTOR..17 * SECTOR];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        let mut id = [b' '; 32];
        id[..volume_id.len().min(32)].copy_from_slice(&volume_id.as_bytes()[..volume_id.len().min(32)]);
        pvd[40..72].copy_from_slice(&id);
        both32(&mut pvd[80..88], lba);
        pvd[128..130].copy_from_slice(&(SECTOR as u16).to_le_bytes());
        pvd[130..132].copy_from_slice(&(SECTOR as u16).to_be_bytes());
        pvd[156..190].copy_from_slice(&record(18, SECTOR as u32, true, &[0]));
        image[17 * SECTOR] = 255;
        image[17 * SECTOR + 1..17 * SECTOR + 6].copy_from_slice(b"CD001");
        let mut dir = record(18, SECTOR as u32, true, &[0]);
        dir.extend(record(18, SECTOR as u32, true, &[1]));
        for (name, lba, bytes) in &placed {
            dir.extend(record(*lba, bytes.len() as u32, false, name.as_bytes()));
            let at = *lba as usize * SECTOR;
            image[at..at + bytes.len()].copy_from_slice(bytes);
        }
        image[18 * SECTOR..18 * SECTOR + dir.len()].copy_from_slice(&dir);
        image
    }
}

/// How far an import is, shared with the game's progress screen.
#[derive(Clone)]
pub struct ImportProgress {
    shared: Arc<Mutex<(f32, String)>>,
    /// The share of the whole import this handle reports: `set(f)` stores
    /// `from + f * (to - from)`.
    from: f32,
    to: f32,
}

impl Default for ImportProgress {
    fn default() -> Self {
        Self { shared: Arc::default(), from: 0.0, to: 1.0 }
    }
}

impl ImportProgress {
    /// `fraction` in 0..=1 (of this handle's part) and what is being imported now.
    pub fn set(&self, fraction: f32, step: impl Into<String>) {
        let fraction = self.from + fraction.clamp(0.0, 1.0) * (self.to - self.from);
        *self.shared.lock().unwrap() = (fraction, step.into());
    }

    pub fn get(&self) -> (f32, String) {
        self.shared.lock().unwrap().clone()
    }

    /// A handle for one step, reporting into `from..to` of this one.
    pub fn part(&self, from: f32, to: f32) -> Self {
        let span = self.to - self.from;
        Self { shared: self.shared.clone(), from: self.from + from * span, to: self.from + to * span }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn opens_a_disc_and_finds_its_files() {
        let dir = std::env::temp_dir().join(format!("import-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("game.iso");
        let cnf = b"BOOT = cdrom:\\SLUS_004.37;1\r\nTCB = 4\r\n";
        let files: [(&str, &[u8]); 3] = [("SYSTEM.CNF", cnf), ("SLUS_004.37", &[0; 3000]), ("B.RAW", &[1])];
        std::fs::write(&image, super::test_disc::iso("TOMBRAIDER2", &files)).unwrap();
        let disc = super::DiscReader::open(&image).unwrap();
        assert_eq!(disc.serial, "SLUS_004.37");
        assert!(disc.accepted());
        assert_eq!(disc.read("/slus_004.37;1").unwrap().len(), 3000);
        assert_eq!(disc.files_in("/", "raw").len(), 0, "the root is ''");
        assert_eq!(disc.files_in("", "raw").iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["/B.RAW"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn progress_parts_nest() {
        let progress = super::ImportProgress::default();
        progress.part(0.5, 1.0).part(0.0, 0.5).set(0.5, "half of a quarter");
        assert_eq!(progress.get(), (0.625, "half of a quarter".to_string()));
    }

    #[test]
    fn serial_from_boot_line() {
        assert_eq!(super::serial_of("cdrom:\\SLUS_004.37;1"), "SLUS_004.37");
        assert_eq!(super::serial_of("cdrom:PSX.EXE;1"), "PSX.EXE");
    }
}
