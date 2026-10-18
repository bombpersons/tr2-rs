//! Common object edits, over the same validated transactions as the data view.
use crate::{
    Editor,
    ui::{self, Action},
};
use bevy::prelude::*;
use engine::objects::ObjectTypes;

#[derive(Component)]
pub(super) struct Panel;

pub(super) fn refresh(
    mut commands: Commands,
    editor: Res<Editor>,
    types: Res<ObjectTypes>,
    panels: Query<(Entity, Option<&Children>), With<Panel>>,
    mut previous: Local<Option<(String, u64, std::path::PathBuf)>>,
) {
    let pointer = ui::selected_pointer(&editor);
    let state = (pointer.clone(), editor.revision, editor.doc.path().to_path_buf());
    if previous.as_ref() == Some(&state) {
        return;
    }
    *previous = Some(state);
    let Ok((panel, children)) = panels.single() else { return };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let mut parts = pointer.split('/').skip(1);
    let (Some("placements"), Some(index)) = (parts.next(), parts.next()) else { return };
    let base = format!("/placements/{index}");
    let Some(object) = editor.doc.value().pointer(&base) else { return };
    let kind = object["kind"].as_u64().unwrap_or(0) as u16;
    let name = types.get(kind).map_or_else(|| format!("Type {kind}"), |t| t.name.clone());
    commands.entity(panel).with_children(|root| {
        root.spawn(ui::label(format!("OBJECT {index}  /  {name}"), 15.0, Color::srgb(0.4, 0.8, 1.0)));
        root.spawn(ui::label("Position - level units (Y increases downward)", 12.0, Color::srgb(0.6, 0.7, 0.8)));
        for (axis, label) in ["X", "Y", "Z"].iter().enumerate() {
            let path = format!("{base}/position/{axis}");
            root.spawn(Node { align_items: AlignItems::Center, column_gap: px(7), ..default() }).with_children(|row| {
                row.spawn(ui::label(*label, 14.0, Color::WHITE));
                ui::button(row, "-128", Action::Nudge(path.clone(), -128));
                ui::button(row, &object["position"][axis].to_string(), Action::EditValue(path.clone()));
                ui::button(row, "+128", Action::Nudge(path, 128));
            });
        }
        root.spawn(Node { align_items: AlignItems::Center, column_gap: px(7), ..default() }).with_children(|row| {
            row.spawn(ui::label("Heading", 14.0, Color::WHITE));
            ui::button(row, "-45", Action::Nudge(format!("{base}/rotation"), -8192));
            let degrees = object["rotation"].as_u64().unwrap_or(0) as f64 * 360.0 / 65536.0;
            ui::button(row, &format!("{degrees:.1} deg"), Action::EditAngle(format!("{base}/rotation")));
            ui::button(row, "+45", Action::Nudge(format!("{base}/rotation"), 8192));
        });
        root.spawn(Node { column_gap: px(7), ..default() }).with_children(|row| {
            ui::button(row, &format!("Room {}", object["room"]), Action::EditValue(format!("{base}/room")));
            let hidden = object.get("invisible").and_then(serde_json::Value::as_bool).unwrap_or(false);
            ui::button(
                row,
                if hidden { "[x] Hidden" } else { "[ ] Hidden" },
                Action::Toggle(format!("{base}/invisible")),
            );
            ui::button(row, "Focus", Action::Frame);
        });
    });
}
