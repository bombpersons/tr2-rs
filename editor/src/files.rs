//! File selection is browse-first; typing a location is an optional shortcut.
use std::path::{Path, PathBuf};

pub const PAGE_SIZE: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    Location,
    Filename,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Picker {
    pub directory: PathBuf,
    pub page: usize,
    pub selected: usize,
    pub save: bool,
    pub focus: Focus,
}

impl Picker {
    pub fn new(path: &Path, save: bool) -> (Self, String) {
        let mut text = if save {
            format!("{}-copy.json", path.file_stem().unwrap_or_default().to_string_lossy())
        } else {
            String::new()
        };
        if save {
            let mut number = 2;
            while path.parent().unwrap_or(Path::new(".")).join(&text).exists() {
                text = format!("{}-copy-{number}.json", path.file_stem().unwrap_or_default().to_string_lossy());
                number += 1;
            }
        }
        (
            Self {
                directory: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
                page: 0,
                selected: 0,
                save,
                focus: if save { Focus::Filename } else { Focus::Search },
            },
            text,
        )
    }

    pub fn entries(&self, text: &str) -> Result<Vec<PathBuf>, String> {
        let filter = if self.focus == Focus::Search { text.to_lowercase() } else { String::new() };
        let mut entries = std::fs::read_dir(&self.directory)
            .map_err(|e| format!("Cannot read this folder: {e}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() || path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
            .filter(|path| path.file_name().unwrap_or_default().to_string_lossy().to_lowercase().contains(&filter))
            .collect::<Vec<_>>();
        entries.sort_by_key(|path| (!path.is_dir(), path.file_name().unwrap_or_default().to_ascii_lowercase()));
        Ok(entries)
    }
}

/// Find nearby level libraries by directory layout, without knowing any game.
pub fn locations(current: &Path) -> Vec<(String, PathBuf)> {
    let mut locations = vec![("Current level folder".into(), current.to_path_buf())];
    let workspace = |path: &Path| {
        path.ancestors()
            .find(|path| {
                std::fs::read_to_string(path.join("Cargo.toml")).is_ok_and(|text| text.contains("[workspace]"))
            })
            .map(Path::to_path_buf)
    };
    if let Some(root) = workspace(current).or_else(|| std::env::current_dir().ok().and_then(|path| workspace(&path))) {
        locations.push(("Project folder".into(), root.clone()));
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut libraries: Vec<_> = entries
                .filter_map(Result::ok)
                .map(|entry| (entry.file_name(), entry.path().join("assets/levels")))
                .filter(|(_, path)| path.is_dir())
                .collect();
            libraries.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, path) in libraries {
                locations.push((format!("{} levels", name.to_string_lossy()), path));
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        locations.push(("Home folder".into(), home.into()));
    }
    locations
}
