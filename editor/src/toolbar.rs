//! The strip under the menu bar: the mode, the tool, the mode's actions, and
//! its settings. Every button calls the same function as its key and menu
//! item, so they can never disagree. See docs/editor-design.md.
use crate::{Editor, viewport::ViewportFocus};
use bevy::prelude::*;
use bevy_egui::{egui, input::EguiWantsInput};
use engine::{editor::STEP, world::Level};

/// What a click or drag in the 3D view does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Click to select; no handles.
    Select,
    /// Arrows move the selection.
    #[default]
    Move,
    /// The ring turns selected objects (object mode).
    Rotate,
    /// Click or drag across faces to paint the brush (mesh mode).
    Paint,
}

impl Tool {
    pub const ALL: [Tool; 4] = [Tool::Select, Tool::Move, Tool::Rotate, Tool::Paint];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Move => "Move",
            Tool::Rotate => "Rotate",
            Tool::Paint => "Paint",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(text.trim()))
    }

    fn key(self) -> KeyCode {
        match self {
            Tool::Select => KeyCode::Digit1,
            Tool::Move => KeyCode::Digit2,
            Tool::Rotate => KeyCode::Digit3,
            Tool::Paint => KeyCode::Digit4,
        }
    }

    fn shortcut(self) -> &'static str {
        match self {
            Tool::Select => "1",
            Tool::Move => "2",
            Tool::Rotate => "3",
            Tool::Paint => "4",
        }
    }

    /// Why the tool does nothing in this mode, if it doesn't.
    pub fn unavailable(self, mesh: bool) -> Option<&'static str> {
        match self {
            Tool::Rotate if mesh => Some("Rotate turns objects; mesh moves stay on the sector grid"),
            Tool::Paint if !mesh => Some("Paint works on room faces in mesh mode (Tab)"),
            _ => None,
        }
    }

    fn help(self) -> &'static str {
        match self {
            Tool::Select => "Click to select; Shift- or Ctrl-click adds or removes",
            Tool::Move => "Drag an arrow to move the selection",
            Tool::Rotate => "Drag the ring to turn the selected objects",
            Tool::Paint => "Click or drag across faces to paint the brush; Alt-click picks a face's texture",
        }
    }
}

impl Editor {
    /// Choose a tool; one that does nothing in this mode is refused with the reason.
    pub fn set_tool(&mut self, tool: Tool) {
        if let Some(reason) = tool.unavailable(self.mesh.on) {
            self.status = reason.into();
            return;
        }
        self.commit();
        self.end_stroke();
        self.tool = tool;
        self.status = format!("{} tool: {}", tool.name(), tool.help());
    }
}

pub struct ToolbarPlugin;
impl Plugin for ToolbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, keys.after(crate::viewport::focus));
    }
}

fn keys(
    mut editor: ResMut<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    egui: Res<EguiWantsInput>,
    focus: Res<ViewportFocus>,
) {
    if egui.wants_any_keyboard_input() || !focus.active || editor.leaving.is_some() {
        return;
    }
    if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::AltLeft, KeyCode::AltRight]) {
        return;
    }
    for tool in Tool::ALL {
        if keys.just_pressed(tool.key()) {
            editor.set_tool(tool);
        }
    }
}

/// A button that runs an action, greyed out with the reason when it can't.
fn action(ui: &mut egui::Ui, text: &str, shortcut: &str, help: &str, blocked: Option<&str>) -> bool {
    let button = egui::Button::new(text);
    let button = if shortcut.is_empty() { button } else { button.shortcut_text(shortcut) };
    let response = ui.add_enabled(blocked.is_none(), button);
    match blocked {
        Some(reason) => response.on_disabled_hover_text(format!("{help}\n{reason}")),
        None => response.on_hover_text(help),
    }
    .clicked()
}

/// Draw the toolbar at `top`; returns its height.
pub fn toolbar(
    ctx: &egui::Context,
    editor: &mut Editor,
    top: f32,
    level: Option<&Level>,
    atlas: Option<egui::TextureId>,
) -> f32 {
    let width = ctx.viewport_rect().width();
    egui::Area::new("toolbar".into())
        .fixed_pos([0.0, top])
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            egui::Frame::side_top_panel(ui.style())
                .show(ui, |ui| {
                    ui.set_width(width - 16.0);
                    ui.horizontal_wrapped(|ui| {
                        modes(ui, editor);
                        ui.separator();
                        tools(ui, editor);
                        ui.separator();
                        if editor.mesh.on {
                            mesh_actions(ui, editor);
                            ui.separator();
                            mesh_settings(ui, editor, level, atlas);
                        } else {
                            object_actions(ui, editor);
                            ui.separator();
                            ui.checkbox(&mut editor.snap, "Snap").on_hover_text(
                                "Moves snap to 256 units and turns to 15 degrees; Ctrl inverts while dragging",
                            );
                        }
                    });
                })
                .response
                .rect
                .height()
        })
        .inner
}

fn modes(ui: &mut egui::Ui, editor: &mut Editor) {
    let mesh = editor.mesh.on;
    if ui.selectable_label(!mesh, "Objects").on_hover_text("Place and edit objects (Tab)").clicked() && mesh {
        editor.set_mesh_mode(false);
    }
    if ui.selectable_label(mesh, "Mesh").on_hover_text("Edit room faces and the sector grid (Tab)").clicked() && !mesh {
        editor.set_mesh_mode(true);
    }
    ui.add_enabled(false, egui::Button::selectable(false, "Rooms"))
        .on_disabled_hover_text("Planned: create, connect and split rooms (docs/editor-design.md)");
}

fn tools(ui: &mut egui::Ui, editor: &mut Editor) {
    for tool in Tool::ALL {
        let blocked = tool.unavailable(editor.mesh.on);
        let response = ui.add_enabled(blocked.is_none(), egui::Button::selectable(editor.tool == tool, tool.name()));
        let help = format!("{} ({})", tool.help(), tool.shortcut());
        let response = match blocked {
            Some(reason) => response.on_disabled_hover_text(format!("{help}\n{reason}")),
            None => response.on_hover_text(help),
        };
        if response.clicked() {
            editor.set_tool(tool);
        }
    }
}

fn object_actions(ui: &mut egui::Ui, editor: &mut Editor) {
    let none = editor.selection.is_empty().then_some("Select objects first");
    if action(ui, "Frame", "F", "Frame the selection", none) {
        editor.frame = true;
    }
    if action(ui, "Select all", "Ctrl+A", "Select every placement", None) {
        crate::ui::select_all(editor);
    }
}

fn mesh_actions(ui: &mut egui::Ui, editor: &mut Editor) {
    let faces = editor.mesh.faces.is_empty().then_some("Select faces first");
    if action(ui, "Raise", "+", "Raise floor or ceiling faces 256 units", faces) {
        editor.mesh_extrude(-STEP);
    }
    if action(ui, "Lower", "-", "Lower floor or ceiling faces 256 units", faces) {
        editor.mesh_extrude(STEP);
    }
    if action(ui, "Push wall", "P", "Open the sector behind a wall face", faces) {
        editor.mesh_push();
    }
    let fill = faces.or(editor.mesh.brush.is_none().then_some("Choose a brush texture first"));
    if action(ui, "Fill", "", "Paint the selected faces with the brush", fill) {
        editor.mesh_paint(None);
    }
    if action(ui, "Turn texture", "R", "Turn the selected faces' textures a quarter turn", faces) {
        editor.mesh_rotate_texture();
    }
}

fn mesh_settings(ui: &mut egui::Ui, editor: &mut Editor, level: Option<&Level>, atlas: Option<egui::TextureId>) {
    let rooms = editor.doc.value()["rooms"].as_array().map_or(0, Vec::len);
    let mut room = editor.mesh.room;
    egui::ComboBox::from_id_salt("toolbar room")
        .selected_text(format!("Room {room}"))
        .height(400.0)
        .show_ui(ui, |ui| {
            for index in 0..rooms {
                ui.selectable_value(&mut room, index, format!("Room {index}"));
            }
        })
        .response
        .on_hover_text("The room being edited; clicking another room's face also switches");
    if room != editor.mesh.room {
        editor.mesh_room(room);
    }
    if ui.button("Frame room").on_hover_text("Frame the room being edited").clicked() {
        editor.frame = true;
        editor.overview = true;
    }
    ui.separator();
    ui.label("Brush");
    let brush = editor.mesh.brush.and_then(|b| Some((b, level?.textures.get(b)?)));
    let response = match (brush, atlas) {
        (Some((index, texture)), Some(id)) => {
            let (min, max) = texture.uv.iter().fold(([1.0f32; 2], [0.0f32; 2]), |(lo, hi), p| {
                ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
            });
            let image =
                egui::Image::new((id, egui::vec2(20.0, 20.0))).uv(egui::Rect::from_min_max(min.into(), max.into()));
            ui.add(egui::Button::image_and_text(image, index.to_string()))
        }
        (Some((index, _)), None) => ui.button(index.to_string()),
        _ => ui.button("none"),
    };
    if response.on_hover_text("Open the Textures window to choose the brush").clicked() {
        editor.windows.textures = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_that_do_nothing_in_a_mode_are_refused_and_leaving_a_mode_drops_them() {
        let mut editor = crate::sandbox();
        assert_eq!(editor.tool, Tool::Move);
        editor.set_tool(Tool::Paint);
        assert_eq!(editor.tool, Tool::Move, "painting needs mesh mode");
        assert!(editor.status.contains("mesh mode"), "{}", editor.status);
        editor.set_tool(Tool::Rotate);
        assert_eq!(editor.tool, Tool::Rotate);
        editor.set_mesh_mode(true);
        assert_eq!(editor.tool, Tool::Move, "rotate does nothing to faces");
        editor.set_tool(Tool::Paint);
        assert_eq!(editor.tool, Tool::Paint);
        editor.set_mesh_mode(false);
        assert_eq!(editor.tool, Tool::Move);
        assert_eq!(Tool::parse("paint"), Some(Tool::Paint));
        assert_eq!(Tool::parse("zoom"), None);
    }
}
