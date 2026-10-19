//! Selection and multi-object edits over the document's validated transactions.
//!
//! Drags write to an overlay that the viewport and windows show at once; the
//! overlay becomes a single undo step when the drag ends.
use crate::Editor;
use engine::editor::{self as authoring, RoomChange};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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
        let mut edits = std::mem::take(&mut self.overlay);
        let (rooms, note) = self.moved_rooms(&edits);
        edits.extend(rooms);
        let reload = edits.keys().any(|pointer| !is_transform(pointer));
        match self.doc.apply(edits) {
            Ok(()) => {
                self.status = note.unwrap_or_else(|| "Edited - Ctrl+Z to undo".into());
                self.revision += 1;
                self.preview |= reload;
            }
            Err(error) => self.status = error,
        }
    }

    /// Room changes for placements that `edits` move, and what to tell the
    /// user about them. A placement keeps its authored room unless the move
    /// crosses into another (`engine::editor::moved_room`); an edit that sets
    /// the room itself is left alone.
    fn moved_rooms(&self, edits: &BTreeMap<String, Value>) -> (Vec<(String, Value)>, Option<String>) {
        let moved: BTreeSet<usize> = edits
            .keys()
            .filter_map(|p| {
                let rest = p.strip_prefix("/placements/")?;
                let (index, field) = rest.split_once('/')?;
                field.starts_with("position").then(|| index.parse().ok())?
            })
            .filter(|index| !edits.contains_key(&format!("/placements/{index}/room")))
            .collect();
        if moved.is_empty() {
            return (vec![], None);
        }
        let Ok(level) = self.doc.level() else { return (vec![], None) };
        let (mut out, mut into, mut outside) = (vec![], vec![], vec![]);
        for index in moved {
            let Some(placement) = level.placements.get(index) else { continue };
            let mut to = placement.position;
            for (axis, value) in to.iter_mut().enumerate() {
                if let Some(v) = edits.get(&format!("/placements/{index}/position/{axis}")).and_then(Value::as_i64) {
                    *value = v as i32;
                }
            }
            match authoring::moved_room(&level, placement.room, placement.position, to) {
                RoomChange::Keep => (),
                RoomChange::Into(room) => {
                    out.push((format!("/placements/{index}/room"), Value::from(room)));
                    into.push(format!("{index} into room {room}"));
                }
                RoomChange::Outside => outside.push(index.to_string()),
            }
        }
        let note = match (into.is_empty(), outside.is_empty()) {
            (true, true) => None,
            (false, true) => Some(format!("Moved object {} - Ctrl+Z to undo", into.join(", "))),
            (_, false) => Some(format!(
                "Object {} is in no room (solid or outside every grid); its room is unchanged",
                outside.join(", ")
            )),
        };
        (out, note)
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
    fn moves_keep_the_room_in_open_floor_and_warn_when_dropped_into_solid() {
        let mut editor = crate::sandbox();
        editor.selection = vec![0];
        editor.edit_selected("position/0", Change::Add(1024), false);
        assert_eq!(editor.doc.value()["placements"][0]["position"][0], 2560 + 1024);
        assert_eq!(editor.doc.value()["placements"][0]["room"], 0);
        assert!(editor.status.starts_with("Edited"), "{}", editor.status);
        // The sandbox's border ring is solid: the move is kept, with a warning.
        editor.edit_selected("position/0", Change::Set(&Value::from(512)), false);
        assert_eq!(editor.doc.value()["placements"][0]["position"][0], 512);
        assert_eq!(editor.doc.value()["placements"][0]["room"], 0);
        assert!(editor.status.contains("in no room"), "{}", editor.status);
        // Setting the room by hand is never second-guessed.
        editor.edit(
            vec![("/placements/0/position/0".into(), Value::from(2560)), ("/placements/0/room".into(), Value::from(0))],
            false,
        );
        assert!(editor.status.starts_with("Edited"), "{}", editor.status);
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
