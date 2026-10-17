//! Import Tomb Raider II levels into the engine's level format.
//!
//! `tools/extract_tr2.py` decodes the disc into `research/extracted/levels/`
//! (raw tables, atlases, decoded audio). This tool converts each level's
//! raw tables into `game/assets/levels/<name>/level.json` and copies the
//! files the engine loads next to it. The raw extraction is never shipped.
//!
//! cargo run -p tr2-import -- [--raw DIR] [--out DIR] [--level NAME]
mod convert;
mod raw;

use std::fs;
use std::path::{Path, PathBuf};

fn copy_assets(from: &Path, to: &Path) -> Result<usize, String> {
    let io = |e: std::io::Error| e.to_string();
    let mut copied = 0;
    fs::create_dir_all(to.join("audio")).map_err(io)?;
    for name in ["atlas.png"] {
        fs::copy(from.join(name), to.join(name)).map_err(|e| format!("{}: {e}", from.join(name).display()))?;
        copied += 1;
    }
    let mut samples: Vec<_> = fs::read_dir(from.join("audio")).map_err(io)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>().map_err(io)?;
    samples.sort();
    for sample in samples.into_iter().filter(|p| p.extension().is_some_and(|e| e == "wav")) {
        fs::copy(&sample, to.join("audio").join(sample.file_name().unwrap())).map_err(io)?;
        copied += 1;
    }
    Ok(copied)
}

fn import(raw_dir: &Path, out_dir: &Path) -> Result<serde_json::Value, String> {
    let path = raw_dir.join("level.json");
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let raw: raw::Level = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let level = convert::level(&raw).map_err(|e| format!("{}: {e}", raw.name))?;
    let name = raw_dir.file_name().unwrap().to_string_lossy().into_owned();
    let out = out_dir.join(&name);
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let files = copy_assets(raw_dir, &out)?;
    let json = serde_json::to_vec(&level).map_err(|e| e.to_string())?;
    fs::write(out.join("level.json"), &json).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "level": name,
        "source": level.source,
        "rooms": level.rooms.len(),
        "floor_infos": level.floor_infos.len(),
        "triggers": level.floor_infos.iter().map(|f| f.triggers.len()).sum::<usize>(),
        "animations": level.animations.len(),
        "placements": level.placements.len(),
        "files_copied": files,
        "level_json_bytes": json.len(),
    }))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw_root = value("--raw").map(PathBuf::from).unwrap_or_else(|| root.join("research/extracted/levels"));
    let out_root = value("--out").map(PathBuf::from).unwrap_or_else(|| root.join("game/assets/levels"));
    let mut dirs: Vec<PathBuf> = match fs::read_dir(&raw_root) {
        Ok(entries) => entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("level.json").exists()).collect(),
        Err(e) => {
            eprintln!("{}: {e}; run tools/extract_tr2.py first", raw_root.display());
            std::process::exit(1);
        }
    };
    dirs.sort();
    if let Some(level) = value("--level") {
        dirs.retain(|d| d.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&level)));
    }
    let mut report = Vec::new();
    for dir in dirs {
        match import(&dir, &out_root) {
            Ok(summary) => {
                println!("{summary}");
                report.push(summary);
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
    if value("--level").is_none() {
        let manifest = out_root.join("manifest.json");
        fs::write(&manifest, serde_json::to_string_pretty(&report).unwrap() + "\n").expect("write manifest");
    }
}
