//! The disc import protocol, headless: tiny generated ISOs stand in for the
//! player's disc. A disc of another game is refused, an accepted one with
//! nothing to import fails without touching the old import, and a finished
//! import of the current version starts the game without the disc.
use engine::testing::headless_app;
use game::disc::{GameData, GameDataPlugin, ImportStamp, STAMP_FILE, read_stamp};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("game-disc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A disc image whose `SYSTEM.CNF` boots `serial`, and nothing else.
fn write_disc(dir: &Path, serial: &str) -> PathBuf {
    let cnf = format!("BOOT = cdrom:\\{serial};1\r\n");
    let files: [(&str, &[u8]); 2] = [("SYSTEM.CNF", cnf.as_bytes()), (serial, &[0; 2048])];
    let path = dir.join("game.iso");
    std::fs::write(&path, import::test_disc::iso("TEST_DISC", &files)).unwrap();
    path
}

fn app(disc: Option<&Path>, out: &Path) -> bevy::app::App {
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(GameDataPlugin {
        import: true,
        disc: disc.map(Path::to_path_buf),
        screens: false,
        out: Some(out.into()),
    });
    app
}

fn state(app: &bevy::app::App) -> GameData {
    app.world().resource::<bevy::state::state::State<GameData>>().get().clone()
}

/// Updates until the state settles on one of `until` (the import runs on a thread).
fn run_until(app: &mut bevy::app::App, until: &[GameData]) -> GameData {
    for _ in 0..500 {
        app.update();
        if until.contains(&state(app)) {
            return state(app);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("stuck in {:?}", state(app));
}

const SETTLED: [GameData; 3] = [GameData::Ready, GameData::NeedDisc, GameData::Failed];

#[test]
fn another_game_is_not_imported() {
    let dir = scratch("other-game");
    let disc = write_disc(&dir, "SLUS_000.01");
    let out = dir.join("imported");
    let mut app = app(Some(&disc), &out);
    assert_eq!(run_until(&mut app, &SETTLED), GameData::NeedDisc);
    assert!(!out.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_failed_import_keeps_the_previous_one() {
    let dir = scratch("failed");
    let disc = write_disc(&dir, import::SERIALS[0]);
    let out = dir.join("imported");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("kept.txt"), "old").unwrap();
    let mut app = app(Some(&disc), &out);
    // The disc is accepted but has no DATA/*.PSX levels.
    assert_eq!(run_until(&mut app, &SETTLED), GameData::Failed);
    assert!(out.join("kept.txt").exists());
    assert_eq!(read_stamp(&out), None);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_current_import_starts_without_the_disc_and_an_old_one_asks_for_it() {
    let dir = scratch("stamps");
    let out = dir.join("imported");
    let empty = dir.join("no-disc-here");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::create_dir_all(&empty).unwrap();
    let stamp = |version| ImportStamp { version, serial: import::SERIALS[0].into(), disc: "game.cue".into() };
    std::fs::write(out.join(STAMP_FILE), serde_json::to_vec(&stamp(import::VERSION)).unwrap()).unwrap();
    let mut current = app(Some(&empty), &out);
    assert_eq!(run_until(&mut current, &SETTLED), GameData::Ready);

    std::fs::write(out.join(STAMP_FILE), serde_json::to_vec(&stamp(import::VERSION.wrapping_sub(1))).unwrap()).unwrap();
    let mut old = app(Some(&empty), &out);
    assert_eq!(run_until(&mut old, &SETTLED), GameData::NeedDisc);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn without_importing_the_game_starts_ready() {
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(GameDataPlugin::default());
    app.update();
    assert_eq!(state(&app), GameData::Ready);
}

/// The importer reproduces the imported Great Wall from the disc in `rom/`
/// (skipped without it).
#[cfg(feature = "baked-assets")]
#[test]
fn importing_the_wall_again_writes_the_same_level() {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rom");
    let Some(cue) = std::fs::read_dir(&rom)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "cue"))
    else {
        eprintln!("no disc image in {}; skipped", rom.display());
        return;
    };
    let reader = import::DiscReader::open(&cue).unwrap();
    let dir = scratch("wall");
    import::import_levels(&reader, &dir, Some("wall"), &import::ImportProgress::default()).unwrap();
    let imported = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/imported/levels/wall");
    for file in ["level.json", "audio/000.wav"] {
        assert!(
            std::fs::read(dir.join("wall").join(file)).unwrap() == std::fs::read(imported.join(file)).unwrap(),
            "{file}"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
