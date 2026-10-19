//! Selection and multi-object edits over the document's validated transactions.
//!
//! Drags write to an overlay that the viewport and windows show at once; the
//! overlay becomes a single undo step when the drag ends.
use crate::Editor;
use serde_json::Value;

/// A turn in the format's binary angle units.
pub const TURN: i64 = 65536;

/// How a field changes on every selected object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change<'a> {
    /// Every object takes this value.
    Set(&'a Value),
    /// Every object moves by this much from its value before the drag.
    Add(i64),
}

impl Editor {
    /// Click semantics shared by the viewport and the Objects window: a plain
    /// click selects only `hit` (or nothing); `extend` toggles `hit`, making
    /// an unselected object active and deselecting the active one.
    pub fn click(&mut self, hit: Option<usize>, extend: bool) {
        match (hit, extend) {
            (Some(index), true) => {
                if self.selection.last() == Some(&index) {
                    self.selection.pop();
                } else {
                    self.selection.retain(|&i| i != index);
                    self.selection.push(index);
                }
            }
            (None, true) => (),
            (Some(index), false) => self.selection = vec![index],
            (None, false) => self.selection.clear(),
        }
        self.commit();
    }

    pub fn active(&self) -> Option<usize> {
        self.selection.last().copied()
    }

    /// The value shown for a field: the drag in progress, or the document's.
    pub fn value(&self, pointer: &str) -> Option<&Value> {
        self.overlay.get(pointer).or_else(|| self.doc.value().pointer(pointer))
    }

    /// The document's integer at `pointer`, with absent optional fields as 0.
    fn saved(&self, pointer: &str) -> i64 {
        self.doc.value().pointer(pointer).and_then(Value::as_i64).unwrap_or(0)
    }

    /// Change `field` (such as `position/0` or `invisible`) on every selected
    /// placement. `live` shows the result until [`Editor::commit`].
    pub fn edit_selected(&mut self, field: &str, change: Change, live: bool) {
        let edits: Vec<_> = self
            .selection
            .iter()
            .map(|index| {
                let pointer = format!("/placements/{index}/{field}");
                let value = match change {
                    Change::Set(value) => value.clone(),
                    Change::Add(delta) => {
                        let value = self.saved(&pointer).saturating_add(delta);
                        Value::from(if field == "rotation" { value.rem_euclid(TURN) } else { value })
                    }
                };
                (pointer, value)
            })
            .collect();
        self.edit(edits, live);
    }

    /// Show `edits` now; when not `live`, commit them with anything pending.
    pub fn edit(&mut self, edits: Vec<(String, Value)>, live: bool) {
        self.overlay.extend(edits);
        if !live {
            self.commit();
        }
    }

    /// Turn the overlay into one undo step. Moving or turning objects updates
    /// their entities in place; other changes rebuild the preview.
    pub fn commit(&mut self) {
        if self.overlay.is_empty() {
            return;
        }
        let edits = std::mem::take(&mut self.overlay);
        let reload = edits.keys().any(|pointer| !is_transform(pointer));
        match self.doc.apply(edits) {
            Ok(()) => {
                self.status = "Edited - Ctrl+Z to undo".into();
                self.revision += 1;
                self.preview |= reload;
            }
            Err(error) => self.status = error,
        }
    }
}

/// A placement's position or heading, which the viewport syncs without a reload.
fn is_transform(pointer: &str) -> bool {
    let mut parts = pointer.split('/').skip(1);
    parts.next() == Some("placements")
        && parts.next().is_some_and(|index| index.parse::<usize>().is_ok())
        && matches!(parts.next(), Some("position" | "rotation"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_select_one_extend_toggle_and_empty_space_clears() {
        let mut editor = crate::sandbox();
        editor.click(Some(1), false);
        assert_eq!(editor.selection, [1]);
        editor.click(Some(0), true);
        editor.click(Some(2), true);
        assert_eq!(editor.selection, [1, 0, 2]);
        assert_eq!(editor.active(), Some(2));
        // Shift-clicking a selected, inactive object makes it active first.
        editor.click(Some(0), true);
        assert_eq!(editor.selection, [1, 2, 0]);
        editor.click(Some(0), true);
        assert_eq!(editor.selection, [1, 2]);
        editor.click(None, true);
        assert_eq!(editor.selection, [1, 2]);
        editor.click(Some(2), false);
        assert_eq!(editor.selection, [2]);
        editor.click(None, false);
        assert!(editor.selection.is_empty());
    }

    #[test]
    fn drags_move_every_selected_object_relatively_as_one_undo_step() {
        let mut editor = crate::sandbox();
        let before = editor.doc.value().clone();
        let x = |e: &Editor, i: usize| e.value(&format!("/placements/{i}/position/0")).unwrap().as_i64().unwrap();
        let (a, b) = (x(&editor, 0), x(&editor, 1));
        editor.selection = vec![0, 1];
        // A drag reports its total offset each frame; the document is untouched until release.
        editor.edit_selected("position/0", Change::Add(100), true);
        editor.edit_selected("position/0", Change::Add(256), true);
        assert_eq!((x(&editor, 0), x(&editor, 1)), (a + 256, b + 256));
        assert_eq!(editor.doc.value(), &before);
        editor.commit();
        assert_eq!((x(&editor, 0), x(&editor, 1)), (a + 256, b + 256));
        assert!(!editor.preview, "moving objects syncs entities without rebuilding the level");
        editor.undo();
        assert_eq!(editor.doc.value(), &before);
        assert!(!editor.doc.is_dirty());
    }

    #[test]
    fn typed_values_set_every_selected_object_and_headings_wrap() {
        let mut editor = crate::sandbox();
        editor.selection = vec![0, 1];
        editor.edit_selected("invisible", Change::Set(&Value::Bool(true)), false);
        assert_eq!(editor.doc.value()["placements"][0]["invisible"], true);
        assert_eq!(editor.doc.value()["placements"][1]["invisible"], true);
        assert!(editor.preview);
        editor.edit_selected("rotation", Change::Set(&Value::from(60000)), false);
        editor.edit_selected("rotation", Change::Add(10000), false);
        assert_eq!(editor.doc.value()["placements"][0]["rotation"], 4464);
        editor.undo();
        editor.undo();
        editor.undo();
        assert!(editor.doc.value()["placements"][1].get("invisible").is_none(), "undo removes an added field");
        assert!(!editor.doc.is_dirty());
        // An invalid value on any object rejects the whole edit.
        editor.edit_selected("room", Change::Set(&Value::from(9999)), false);
        assert!(!editor.doc.is_dirty());
        assert!(editor.overlay.is_empty());
    }
}
