//! The Diagnostics window: problems with how rooms join, where objects are
//! and the navigation boxes (`engine::editor::diagnose`), split into those an
//! edit introduced and those the file already had when it was opened.
//!
//! Imported levels carry thousands of deliberate inconsistencies (see the
//! engine module), so inherited problems are hidden until asked for, and the
//! status bar counts only new ones.
use crate::Editor;
use bevy_egui::egui::{self, RichText};
use engine::{
    editor::{Check, Problem, Surface, apply_fix, diagnose, sector_face},
    world::Level,
};
use std::{collections::HashSet, sync::Arc};

type Key = (Check, usize, Option<(usize, usize)>, Option<usize>);

/// The problems of the previewed level, and the level as it was opened.
#[derive(Default)]
pub struct Diagnostics {
    /// The level as opened: the baseline for new problems and stale boxes.
    pub opened: Option<Arc<Level>>,
    baseline: Option<HashSet<Key>>,
    /// The previewed level the problems were found in.
    checked: Option<Arc<Level>>,
    /// Problems, each with whether the opened level had it too.
    pub problems: Vec<(Problem, bool)>,
    pub show_inherited: bool,
}

impl Diagnostics {
    /// Start over for a newly opened level.
    pub fn opened(level: Option<Level>) -> Self {
        Self { opened: level.map(Arc::new), ..Default::default() }
    }

    /// Check `level` if it changed since the last check.
    pub fn refresh(&mut self, level: &Arc<Level>) {
        if self.checked.as_ref().is_some_and(|c| Arc::ptr_eq(c, level)) {
            return;
        }
        let opened = self.opened.clone();
        let baseline = self.baseline.get_or_insert_with(|| {
            opened.as_deref().map(|o| diagnose(o, Some(o)).iter().map(Problem::key).collect()).unwrap_or_default()
        });
        self.problems = diagnose(level, opened.as_deref())
            .into_iter()
            .map(|p| {
                let inherited = baseline.contains(&p.key());
                (p, inherited)
            })
            .collect();
        self.checked = Some(level.clone());
    }

    /// How many problems an edit introduced.
    pub fn new_count(&self) -> usize {
        self.problems.iter().filter(|(_, inherited)| !inherited).count()
    }
}

impl Editor {
    /// Apply the fixes of `problems` as one undo step.
    pub fn fix_problems(&mut self, problems: &[Problem]) {
        let fixes: Vec<_> = problems.iter().filter_map(|p| p.fix.clone()).collect();
        if fixes.is_empty() {
            return;
        }
        let count = fixes.len();
        self.level_edit(&format!("Fixed {count} problem{}", if count == 1 { "" } else { "s" }), |level| {
            fixes.iter().try_for_each(|fix| apply_fix(level, fix))
        });
    }

    /// Rebuild navigation boxes, overlaps and zones wherever sectors lack a
    /// box, lie outside theirs, or changed height since the level was opened.
    pub fn rebuild_navigation(&mut self) {
        let opened = self.diagnostics.opened.clone();
        let report =
            self.level_edit("Rebuilt navigation", |level| engine::editor::rebuild_navigation(level, opened.as_deref()));
        match report {
            Some(r) if r.built == 0 && r.removed == 0 => self.status = "Navigation is up to date".into(),
            Some(r) => {
                self.status = format!(
                    "Rebuilt navigation: {} boxes for {} sectors{} - Ctrl+Z to undo",
                    r.built,
                    r.sectors,
                    if r.removed > 0 { format!(", {} old boxes removed", r.removed) } else { String::new() }
                )
            }
            None => (),
        }
    }

    /// Show where a problem is: its object, or its room (and in mesh mode the
    /// sector's floor).
    pub fn go_to(&mut self, problem: &Problem, level: &Level) {
        if matches!(problem.check, Check::ObjectInOtherRoom | Check::ObjectOutside) {
            if let Some(index) = problem.other {
                self.click(Some(index), false);
                self.frame = true;
            }
            return;
        }
        self.select_room(problem.room);
        self.frame_room();
        if let (true, Some((x, z)), Some(room)) = (self.mesh.on, problem.sector, level.rooms.get(problem.room)) {
            let floor = (0..room.quads.len()).find(|&q| {
                sector_face(room, &room.quads[q]).is_some_and(|f| (f.x, f.z, f.surface) == (x, z, Surface::Floor))
            });
            if let Some(q) = floor {
                self.mesh_click(problem.room, Some(crate::mesh::Element::Face(engine::editor::Face::Quad(q))), false);
                self.frame_room = false;
            }
        }
        self.status = problem.detail.clone();
    }
}

/// Rows drawn per check before "and N more".
const ROWS: usize = 100;

pub fn window(ui: &mut egui::Ui, editor: &mut Editor, level: Option<&Level>) {
    let Some(level) = level else {
        ui.label("Loading...");
        return;
    };
    let problems = editor.diagnostics.problems.clone();
    let new = problems.iter().filter(|(_, inherited)| !inherited).count();
    let inherited = problems.len() - new;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("{new} new")).strong());
        ui.label(format!("and {inherited} already in the file when it was opened"));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut editor.diagnostics.show_inherited, "Show inherited")
            .on_hover_text("Imported levels break these rules on purpose in many places; see docs/editor.md");
        let fixable: Vec<_> = problems.iter().filter(|(p, i)| !i && p.fix.is_some()).map(|(p, _)| p.clone()).collect();
        if ui
            .add_enabled(!fixable.is_empty(), egui::Button::new(format!("Fix all new ({})", fixable.len())))
            .on_hover_text("Apply every fix offered for a new problem, as one undo step")
            .clicked()
        {
            editor.fix_problems(&fixable);
        }
        let navigation = problems
            .iter()
            .filter(|(p, _)| matches!(p.check, Check::NavBoxMissing | Check::NavBoxOutside | Check::NavBoxStale))
            .count();
        if ui
            .add_enabled(navigation > 0, egui::Button::new(format!("Rebuild navigation ({navigation})")))
            .on_hover_text(
                "Build navigation boxes, overlaps and zones for sectors without a box, outside theirs, or whose \
                 floor changed; every other box stays as authored",
            )
            .clicked()
        {
            editor.rebuild_navigation();
        }
    });
    ui.separator();
    let show_inherited = editor.diagnostics.show_inherited;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if problems.is_empty() {
            ui.label("No problems found.");
        }
        for check in Check::ALL {
            let rows: Vec<_> = problems.iter().filter(|(p, i)| p.check == check && (show_inherited || !i)).collect();
            if rows.is_empty() {
                continue;
            }
            let fresh = rows.iter().filter(|(_, i)| !i).count();
            let title = format!("{}  ({fresh} new, {} inherited)", check.title(), rows.len() - fresh);
            egui::CollapsingHeader::new(title).id_salt(check).default_open(fresh > 0).show(ui, |ui| {
                for (problem, inherited) in rows.iter().take(ROWS) {
                    ui.horizontal(|ui| {
                        if ui.small_button("Go to").clicked() {
                            editor.go_to(problem, level);
                        }
                        if problem.fix.is_some() && ui.small_button("Fix").on_hover_text(fix_hint(problem)).clicked() {
                            editor.fix_problems(std::slice::from_ref(problem));
                        }
                        let text = RichText::new(&problem.detail);
                        ui.label(if *inherited { text.weak() } else { text });
                    });
                }
                if rows.len() > ROWS {
                    ui.label(RichText::new(format!("and {} more", rows.len() - ROWS)).weak());
                }
            });
        }
    });
}

fn fix_hint(problem: &Problem) -> String {
    match problem.fix {
        Some(engine::editor::Fix::ReturnPortal { .. }) => {
            "Add the mirror of this portal to the other room, so each sees the other".into()
        }
        Some(engine::editor::Fix::ObjectRoom { room, .. }) => format!("Set the object's room to {room}"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::Change;
    use serde_json::Value;

    #[test]
    fn edits_are_told_apart_from_problems_the_file_already_had() {
        let mut editor = crate::sandbox();
        let level = Arc::new(editor.doc.level().unwrap());
        editor.diagnostics.refresh(&level);
        let inherited = editor.diagnostics.problems.len();
        assert_eq!(editor.diagnostics.new_count(), 0);
        assert!(editor.diagnostics.problems.iter().all(|(_, inherited)| *inherited));
        // Dropping an object into the solid border is new.
        editor.selection = vec![0];
        editor.edit_selected("position/0", Change::Set(&Value::from(512)), false);
        let edited = Arc::new(editor.doc.level().unwrap());
        editor.diagnostics.refresh(&edited);
        assert_eq!(editor.diagnostics.new_count(), 1);
        let (problem, _) = editor.diagnostics.problems.iter().find(|(_, i)| !i).unwrap().clone();
        assert_eq!((problem.check, problem.other), (Check::ObjectOutside, Some(0)));
        editor.selection.clear();
        editor.go_to(&problem, &edited);
        assert_eq!(editor.selection, [0], "going to an object problem selects it");
        editor.undo();
        let undone = Arc::new(editor.doc.level().unwrap());
        editor.diagnostics.refresh(&undone);
        assert_eq!((editor.diagnostics.new_count(), editor.diagnostics.problems.len()), (0, inherited));
    }

    #[test]
    fn rebuilding_navigation_gives_every_open_sector_a_box_in_one_undo_step() {
        let mut editor = crate::sandbox();
        let navigation = |editor: &Editor| {
            let level = Arc::new(editor.doc.level().unwrap());
            diagnose(&level, None)
                .iter()
                .filter(|p| matches!(p.check, Check::NavBoxMissing | Check::NavBoxOutside | Check::NavBoxStale))
                .count()
        };
        assert_eq!(navigation(&editor), 16, "the sandbox ships without navigation");
        editor.rebuild_navigation();
        assert!(editor.status.starts_with("Rebuilt navigation"), "{}", editor.status);
        assert_eq!(navigation(&editor), 0);
        assert_eq!(editor.doc.undo_steps(), 1);
        editor.rebuild_navigation();
        assert_eq!(editor.status, "Navigation is up to date");
        editor.undo();
        assert!(!editor.doc.is_dirty());
    }
}
