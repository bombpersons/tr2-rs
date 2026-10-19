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

/// One field of an undoable edit. `None` is an optional property that is
/// absent from the file, so undo removes it again.
#[derive(Debug)]
struct Change {
    pointer: String,
    before: Option<Value>,
    after: Value,
}

/// One undo step; a multi-object edit changes several fields at once.
#[derive(Debug)]
struct Edit(Vec<Change>);

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
        self.apply([(pointer.to_string(), after)])
    }

    /// Set several fields as one validated, undoable step. A missing
    /// property of an existing object is added (and removed again by undo).
    pub fn apply(&mut self, edits: impl IntoIterator<Item = (String, Value)>) -> Result<(), String> {
        let mut changes = vec![];
        for (pointer, after) in edits {
            let before = self.value.pointer(&pointer).cloned();
            if before.is_none() && parent_object(&mut self.value, &pointer).is_none() {
                Self::revert(&mut self.value, &changes);
                return Err("field no longer exists".into());
            }
            if before.as_ref() == Some(&after) {
                continue;
            }
            set(&mut self.value, &pointer, Some(after.clone()));
            changes.push(Change { pointer, before, after });
        }
        if changes.is_empty() {
            return Ok(());
        }
        if let Err(error) = self.level() {
            Self::revert(&mut self.value, &changes);
            return Err(error);
        }
        self.undo.push(Edit(changes));
        self.redo.clear();
        self.dirty = self.value != self.saved;
        Ok(())
    }

    fn revert(value: &mut Value, changes: &[Change]) {
        for change in changes.iter().rev() {
            set(value, &change.pointer, change.before.clone());
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop() else { return false };
        Self::revert(&mut self.value, &edit.0);
        self.redo.push(edit);
        self.dirty = self.value != self.saved;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop() else { return false };
        for change in &edit.0 {
            set(&mut self.value, &change.pointer, Some(change.after.clone()));
        }
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
        self.write(path.as_ref(), false)
    }

    /// Save As to a destination the user already agreed to replace, such as
    /// one confirmed in a native save dialog. The swap is still atomic.
    pub fn save_replacing(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        self.write(path.as_ref(), true)
    }

    fn write(&mut self, path: &Path, replace: bool) -> Result<(), String> {
        let parent = fs::canonicalize(path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        let target = parent.join(path.file_name().ok_or("missing filename")?);
        let same = target == self.path;
        if same {
            if fs::read(&target).map_err(|e| e.to_string())? != self.bytes {
                return Err("File changed on disk. Open it again, or Save As a new file.".into());
            }
        } else if target.exists() && !replace {
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
            let swap = same || (replace && target.exists());
            if swap {
                file.set_permissions(fs::metadata(&target)?.permissions())?;
            }
            file.sync_all()?;
            // Hard-link for Save As: creation remains exclusive even if another
            // process creates the destination between the check and this point.
            if swap {
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

/// The object that would hold `pointer`'s final property.
fn parent_object<'a>(value: &'a mut Value, pointer: &str) -> Option<&'a mut serde_json::Map<String, Value>> {
    let (parent, _) = pointer.rsplit_once('/')?;
    value.pointer_mut(parent)?.as_object_mut()
}

/// Set (or, with `None`, remove) the field at `pointer`.
fn set(value: &mut Value, pointer: &str, after: Option<Value>) {
    if let Some(slot) = value.pointer_mut(pointer) {
        match after {
            Some(after) => *slot = after,
            None => {
                if let Some((_, key)) = pointer.rsplit_once('/') {
                    parent_object(value, pointer).map(|object| object.remove(&unescape(key)));
                }
            }
        }
    } else if let (Some(after), Some((_, key))) = (after, pointer.rsplit_once('/')) {
        let key = unescape(key);
        if let Some(object) = parent_object(value, pointer) {
            object.insert(key, after);
        }
    }
}

fn unescape(key: &str) -> String {
    key.replace("~1", "/").replace("~0", "~")
}
