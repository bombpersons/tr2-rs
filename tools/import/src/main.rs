//! `cargo run -p import -- DISC [OUT]`: imports from a disc image into OUT
//! (default `game/assets/imported`), as the game does on first start.
//! `--level NAME` imports only that level, into `OUT/levels/NAME/`.
use import::{DiscReader, ImportProgress};
use std::path::{Path, PathBuf};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let level = args.iter().position(|a| a == "--level").map(|i| {
        let name = args.get(i + 1).cloned().unwrap_or_else(|| fail("--level needs a name"));
        args.drain(i..i + 2);
        name
    });
    let [disc, rest @ ..] = args.as_slice() else { fail("usage: import DISC [OUT] [--level NAME]") };
    let out = rest
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../game/assets/imported"));
    let reader = DiscReader::open(Path::new(disc)).unwrap_or_else(|e| fail(&e));
    if !reader.accepted() {
        fail(&format!("{disc} is {}, not a disc import::SERIALS accepts", reader.serial));
    }
    let progress = ImportProgress::default();
    let result = match &level {
        Some(name) => import::import_levels(&reader, &out.join("levels"), Some(name), &progress),
        None => import::import(&reader, &out, &progress),
    };
    result.unwrap_or_else(|e| fail(&e));
    println!("imported {disc} ({}) into {}", reader.serial, out.display());
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}
