//! Native open/save dialogs. They run on their own thread so the editor keeps
//! drawing; the chosen path comes back on the next frame.
use crate::Editor;
use bevy::prelude::*;
use std::{
    path::PathBuf,
    sync::{Mutex, mpsc},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    Open,
    SaveAs,
}

/// What the user chose in a dialog; `None` when it was canceled.
type Chosen = (Ask, Option<PathBuf>);

#[derive(Resource, Default)]
pub struct FileDialogs {
    pending: Option<Mutex<mpsc::Receiver<Chosen>>>,
}

pub fn show(mut editor: ResMut<Editor>, mut dialogs: ResMut<FileDialogs>) {
    if dialogs.pending.is_some() {
        editor.bypass_change_detection().ask = None;
        return;
    }
    let Some(ask) = editor.ask.take() else { return };
    let path = editor.doc.path();
    let folder = path.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let dialog = rfd::FileDialog::new().add_filter("Level", &["json"]).set_directory(folder);
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let path = match ask {
            Ask::Open => dialog.set_title("Open level").pick_file(),
            Ask::SaveAs => dialog.set_title("Save level as").set_file_name(name).save_file(),
        };
        let _ = send.send((ask, path));
    });
    dialogs.pending = Some(Mutex::new(receive));
}

pub fn receive(mut editor: ResMut<Editor>, mut dialogs: ResMut<FileDialogs>) {
    let Some(pending) = &dialogs.pending else { return };
    let result = pending.lock().unwrap().try_recv();
    let (ask, path) = match result {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Empty) => return,
        Err(mpsc::TryRecvError::Disconnected) => (Ask::Open, None),
    };
    dialogs.pending = None;
    match (ask, path) {
        (Ask::Open, Some(path)) => editor.open(&path),
        (Ask::SaveAs, Some(path)) => editor.save_as(path),
        (_, None) => (),
    }
}

/// Find nearby level libraries by directory layout, without knowing any game.
pub fn libraries(current: &std::path::Path) -> Vec<(String, PathBuf)> {
    let workspace = |path: &std::path::Path| {
        path.ancestors()
            .find(|path| {
                std::fs::read_to_string(path.join("Cargo.toml")).is_ok_and(|text| text.contains("[workspace]"))
            })
            .map(std::path::Path::to_path_buf)
    };
    let mut levels = vec![];
    let Some(root) = workspace(current).or_else(|| std::env::current_dir().ok().and_then(|path| workspace(&path)))
    else {
        return levels;
    };
    let Ok(crates) = std::fs::read_dir(root) else { return levels };
    let mut crates: Vec<_> = crates.filter_map(Result::ok).map(|entry| entry.path().join("assets/levels")).collect();
    crates.sort();
    for library in crates {
        let Ok(entries) = std::fs::read_dir(&library) else { continue };
        let mut found: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("level.json"))
            .filter(|path| path.is_file())
            .collect();
        found.sort();
        let owner = library.parent().and_then(|p| p.parent()).and_then(|p| p.file_name());
        let owner = owner.unwrap_or_default().to_string_lossy().into_owned();
        for path in found {
            let name = path.parent().and_then(|p| p.file_name()).unwrap_or_default().to_string_lossy().into_owned();
            levels.push((format!("{owner}/{name}"), path));
        }
    }
    levels
}
