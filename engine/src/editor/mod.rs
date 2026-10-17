//! Authoring the engine's formats, without game controllers or disc data.
//!
//! `LevelDocument` retains the original bytes and unknown JSON fields. A
//! no-op save (including edit followed by undo) is byte-exact. Changes use
//! the same typed format and reference validation as the preview.
mod validate;

use crate::world::Level;
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
pub use validate::validate_level;

#[derive(Debug)]
struct Edit {
    pointer: String,
    before: Value,
    after: Value,
}

/// One level being authored. JSON pointers address every engine-format field,
/// including nested animation, geometry and trigger tables.
pub struct LevelDocument {
    path: PathBuf,
    bytes: Vec<u8>,
    saved: Value,
    value: Value,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    dirty: bool,
}

impl LevelDocument {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let doc = Self { path, bytes, saved: value.clone(), value, undo: vec![], redo: vec![], dirty: false };
        doc.level()?;
        Ok(doc)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn level(&self) -> Result<Level, String> {
        let level: Level = serde_json::from_value(self.value.clone()).map_err(|e| e.to_string())?;
        validate_level(&level)?;
        Ok(level)
    }

    /// Replace a field or a whole table transactionally. Invalid edits never
    /// enter the document or its history. Replacing arrays permits insertion
    /// and deletion without discarding unknown fields elsewhere in the file.
    pub fn replace(&mut self, pointer: &str, text: &str) -> Result<(), String> {
        let after: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let before = self.value.pointer(pointer).ok_or("field no longer exists")?.clone();
        if before == after {
            return Ok(());
        }
        *self.value.pointer_mut(pointer).ok_or("field no longer exists")? = after.clone();
        if let Err(error) = self.level() {
            *self.value.pointer_mut(pointer).unwrap() = before;
            return Err(error);
        }
        self.undo.push(Edit { pointer: pointer.into(), before, after });
        self.redo.clear();
        self.dirty = self.value != self.saved;
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop() else { return false };
        *self.value.pointer_mut(&edit.pointer).unwrap() = edit.before.clone();
        self.redo.push(edit);
        self.dirty = self.value != self.saved;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop() else { return false };
        *self.value.pointer_mut(&edit.pointer).unwrap() = edit.after.clone();
        self.undo.push(edit);
        self.dirty = self.value != self.saved;
        true
    }

    /// Save atomically, refusing to overwrite external edits. Save As requires
    /// a new filename. Companion atlas/audio files stay in their level folder.
    pub fn save(&mut self) -> Result<(), String> {
        self.save_to(self.path.clone())
    }

    pub fn save_to(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        let parent = fs::canonicalize(path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        let target = parent.join(path.file_name().ok_or("missing filename")?);
        let same = target == self.path;
        if same {
            if fs::read(&target).map_err(|e| e.to_string())? != self.bytes {
                return Err("File changed on disk. Open it again, or Save As a new file.".into());
            }
        } else if target.exists() {
            return Err("Save As needs a new filename; the destination already exists.".into());
        }
        self.level()?;
        let bytes = if self.dirty {
            let mut bytes = serde_json::to_vec_pretty(&self.value).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            bytes
        } else {
            self.bytes.clone()
        };
        if same && bytes == self.bytes {
            return Ok(());
        }
        let temporary = parent.join(format!(
            ".{}.{}.editor-tmp",
            target.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ));
        let mut file =
            fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|e| e.to_string())?;
        let result = (|| -> std::io::Result<()> {
            file.write_all(&bytes)?;
            if same {
                file.set_permissions(fs::metadata(&target)?.permissions())?;
            }
            file.sync_all()?;
            // Hard-link for Save As: creation remains exclusive even if another
            // process creates the destination between the check and this point.
            if same {
                fs::rename(&temporary, &target)?;
            } else {
                fs::hard_link(&temporary, &target)?;
                fs::remove_file(&temporary)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&temporary);
            return Err(e.to_string());
        }
        self.path = target;
        self.bytes = bytes;
        self.saved = self.value.clone();
        self.dirty = false;
        Ok(())
    }
}

/// An escaped JSON pointer child; names containing `/` or `~` remain editable.
pub fn child_pointer(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}
