//! Modal editing, file selection and unsaved-work choices.
use crate::{
    Editor,
    files::{self, Focus},
    ui::{self, Action, Input, Mode},
};
use bevy::prelude::*;

const INK: Color = Color::srgb(0.86, 0.90, 0.97);
const MUTED: Color = Color::srgb(0.60, 0.70, 0.80);

#[derive(Component)]
pub(super) struct Dialog;

pub(super) fn refresh(
    mut commands: Commands,
    editor: Res<Editor>,
    cameras: Query<Entity, With<Camera2d>>,
    dialogs: Query<Entity, With<Dialog>>,
    mut previous: Local<Option<(Option<Input>, String)>>,
) {
    let state = (editor.input.clone(), editor.status.clone());
    if previous.as_ref() == Some(&state) {
        return;
    }
    *previous = Some(state);
    for entity in &dialogs {
        commands.entity(entity).despawn();
    }
    let Some(input) = &editor.input else { return };
    let Ok(camera) = cameras.single() else { return };
    let title = match &input.mode {
        Mode::File(picker) => {
            if picker.save {
                "Save a copy".into()
            } else {
                "Open a level".into()
            }
        }
        Mode::ConfirmLeave { .. } => "Save your changes?".into(),
        Mode::Find(_) => "Find in this section".into(),
        Mode::Field(pointer) => format!("Advanced: edit {} as JSON", pointer),
        Mode::Value(pointer) => format!("Edit {}", field_name(pointer)),
        Mode::Angle(_) => "Edit heading in degrees".into(),
    };
    commands.spawn((
        Dialog, UiTargetCamera(camera), GlobalZIndex(100),
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
    )).with_children(|root| {
        root.spawn((
            Node { width: px(if matches!(input.mode, Mode::File(_)) { 940 } else { 740 }), max_width: percent(96), max_height: percent(96), padding: UiRect::all(px(22)), flex_direction: FlexDirection::Column, row_gap: px(10), overflow: Overflow::clip(), ..default() },
            BackgroundColor(Color::srgb(0.07, 0.10, 0.15)),
        )).with_children(|dialog| {
            dialog.spawn(ui::label(title, 24.0, INK));
            match &input.mode {
                Mode::File(picker) => file_browser(dialog, &editor, input, picker),
                Mode::ConfirmLeave { quit } => {
                    dialog.spawn(ui::label(format!("{} has unsaved changes.", editor.doc.path().file_name().unwrap_or_default().to_string_lossy()), 17.0, INK));
                    dialog.spawn(ui::label(if *quit { "Save before closing, discard changes, or return to editing." } else { "Save before browsing, or discard only when another level opens.\nCancel keeps your current work." }, 15.0, MUTED));
                }
                Mode::Find(candidates) => {
                    dialog.spawn(ui::label("Search by number or name. Click a result, or Enter selects the first.", 15.0, MUTED));
                    input_box(dialog, input);
                    let matches = ui::find_matches(candidates, &input.text);
                    dialog.spawn(ui::label(format!("{} matches", matches.len()), 13.0, MUTED));
                    for (pointer, name) in matches.into_iter().take(8) { ui::button(dialog, name, Action::Pick(pointer.clone())); }
                }
                _ => {
                    dialog.spawn(ui::label("Type a value. Enter applies it; Escape keeps the original.", 15.0, MUTED));
                    input_box(dialog, input);
                }
            }
            if !editor.status.is_empty() {
                dialog.spawn(ui::label(&editor.status, 14.0, Color::srgb(1.0, 0.75, 0.4)));
            }
            dialog.spawn(Node { column_gap: px(8), justify_content: JustifyContent::End, ..default() }).with_children(|row| {
                if matches!(input.mode, Mode::ConfirmLeave { .. }) {
                    ui::button(row, "Save and continue", Action::SaveContinue);
                    ui::button(row, "Discard changes", Action::DiscardContinue);
                } else {
                    ui::button(row, match &input.mode { Mode::File(p) if p.save => "Save copy", Mode::File(_) => "Open selected", _ => "Apply" }, Action::Apply);
                }
                ui::button(row, "Cancel", Action::Cancel);
            });
        });
    });
}

fn field_name(pointer: &str) -> String {
    let parts: Vec<_> = pointer.split('/').collect();
    if let Some(axis) = parts.last().and_then(|s| s.parse::<usize>().ok())
        && parts.get(parts.len().saturating_sub(2)) == Some(&"position")
    {
        return format!("{} position", ["X", "Y", "Z"].get(axis).unwrap_or(&"Coordinate"));
    }
    ui::humanize(parts.last().unwrap_or(&"value"))
}

fn input_box(parent: &mut ChildSpawnerCommands, input: &Input) {
    parent
        .spawn((
            Node {
                padding: UiRect::all(px(10)),
                min_height: px(42),
                max_height: px(160),
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgb(0.13, 0.22, 0.32)),
        ))
        .with_child(ui::label(
            if input.text.is_empty() {
                "Start typing...".into()
            } else {
                input.text.chars().rev().take(1000).collect::<String>().chars().rev().collect::<String>()
            },
            16.0,
            if input.text.is_empty() { MUTED } else { INK },
        ));
    parent.spawn(ui::label(
        if input.select_all && !input.text.is_empty() {
            "Text selected - typing replaces it. Ctrl+A selects all."
        } else {
            "Ctrl+A selects all. Backspace deletes. Esc cancels."
        },
        12.0,
        MUTED,
    ));
}

fn file_browser(parent: &mut ChildSpawnerCommands, editor: &Editor, input: &Input, picker: &files::Picker) {
    parent.spawn(ui::label(
        match picker.focus {
            Focus::Search => "Search this folder",
            Focus::Location => "Folder or file location",
            Focus::Filename => "New filename",
        },
        14.0,
        INK,
    ));
    input_box(parent, input);
    parent
        .spawn(Node { align_items: AlignItems::Center, column_gap: px(5), flex_wrap: FlexWrap::Wrap, ..default() })
        .with_children(|row| {
            if let Some(path) = picker.directory.parent() {
                ui::button(row, "Up", Action::Browse(path.to_path_buf()));
            }
            let mut ancestors: Vec<_> = picker.directory.ancestors().take(4).collect();
            ancestors.reverse();
            for path in ancestors {
                ui::button(
                    row,
                    &path.file_name().map_or_else(|| "/".into(), |s| s.to_string_lossy().into_owned()),
                    Action::Browse(path.to_path_buf()),
                );
            }
            ui::button(row, "Type location...", Action::Location);
        });
    parent.spawn(ui::label(picker.directory.display().to_string(), 12.0, MUTED));
    parent.spawn(Node { column_gap: px(18), min_height: px(300), ..default() }).with_children(|body| {
        body.spawn(Node {
            width: px(205),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        })
        .with_children(|side| {
            side.spawn(ui::label("PLACES", 13.0, MUTED));
            for (name, path) in files::locations(editor.doc.path().parent().unwrap()) {
                ui::button(side, &name, Action::Browse(path));
            }
            if !picker.save && !editor.recent.is_empty() {
                side.spawn(ui::label("RECENT LEVELS", 13.0, MUTED));
                for path in editor.recent.iter().take(4) {
                    let name = format!(
                        "{}/{}",
                        path.parent().and_then(std::path::Path::file_name).unwrap_or_default().to_string_lossy(),
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    ui::button(side, &name, Action::Browse(path.clone()));
                }
            }
        });
        body.spawn(Node {
            flex_grow: 1.0,
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            ..default()
        })
        .with_children(|list| {
            list.spawn(ui::label(
                if picker.save {
                    "Choose a folder, then name your copy above"
                } else {
                    "Click a folder to browse or a file to open"
                },
                13.0,
                MUTED,
            ));
            match picker.entries(&input.text) {
                Ok(paths) => {
                    if paths.is_empty() {
                        list.spawn(ui::label("No matching folders or JSON files.", 15.0, MUTED));
                    }
                    for (index, path) in
                        paths.iter().enumerate().skip(picker.page * files::PAGE_SIZE).take(files::PAGE_SIZE)
                    {
                        let name = format!(
                            "{}  {}",
                            if path.is_dir() { "Folder" } else { "JSON  " },
                            path.file_name().unwrap_or_default().to_string_lossy()
                        );
                        list.spawn((
                            Action::Browse(path.clone()),
                            bevy::picking::hover::Hovered::default(),
                            ui::ControlColor(if index == picker.selected {
                                Color::srgb(0.16, 0.30, 0.42)
                            } else {
                                Color::srgb(0.12, 0.17, 0.24)
                            }),
                            Node {
                                padding: UiRect::axes(px(10), px(7)),
                                width: percent(100),
                                overflow: Overflow::clip(),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            BackgroundColor(if index == picker.selected {
                                Color::srgb(0.16, 0.30, 0.42)
                            } else {
                                Color::srgb(0.12, 0.17, 0.24)
                            }),
                        ))
                        .observe(ui::clicked)
                        .with_child(ui::label(name, 15.0, INK));
                    }
                    list.spawn(Node { column_gap: px(6), ..default() }).with_children(|row| {
                        if picker.page > 0 {
                            ui::button(row, "Previous", Action::Files(false));
                        }
                        row.spawn(ui::label(
                            format!(
                                "{} items  |  Page {} / {}",
                                paths.len(),
                                picker.page + 1,
                                paths.len().div_ceil(files::PAGE_SIZE).max(1)
                            ),
                            12.0,
                            MUTED,
                        ));
                        if (picker.page + 1) * files::PAGE_SIZE < paths.len() {
                            ui::button(row, "Next", Action::Files(true));
                        }
                    });
                }
                Err(error) => {
                    list.spawn(ui::label(error, 15.0, INK));
                }
            }
        });
    });
    parent.spawn(ui::label(
        if picker.save {
            "Saves JSON only. Keep atlas.png and audio beside the level. Existing files are protected."
        } else {
            "Up/Down selects; Enter opens. Type to filter. Ctrl+L enters a location."
        },
        13.0,
        MUTED,
    ));
}
