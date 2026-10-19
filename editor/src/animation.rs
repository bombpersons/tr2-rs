//! Authoring preview: engine clip rules, no game controllers and no audio.
use crate::Editor;
use bevy::prelude::*;
use bevy_egui::egui;
use engine::{
    animation::{AnimationGoal, ObjectAnimation, advance_authored_animation, step_authored_animation},
    objects::WorldObject,
    render::{InRoom, LevelEntity, LevelSystems},
    world::WorldGeometry,
};

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationPreview {
    pub active: bool,
    pub isolate: bool,
    pub placement: usize,
    pub clip: usize,
    /// Clip-relative authored frame; editable while paused over the MCP.
    pub frame: f32,
    pub goal: usize,
    pub playing: bool,
    pub step: bool,
    pub caption: String,
}

pub struct AnimationPreviewPlugin;
impl Plugin for AnimationPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, preview.after(LevelSystems::Load).before(LevelSystems::Pose));
    }
}

const IDLE: &str = "Select an animation in Data, or an object with authored clips, then Preview selection.";

/// Playback controls; the caption describes the clip being shown.
pub fn window(ui: &mut egui::Ui, editor: &mut Editor, p: &mut AnimationPreview, world: Option<&WorldGeometry>) {
    ui.label(if p.caption.is_empty() { IDLE } else { &p.caption });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Preview selection").clicked()
            && let Err(error) = select(editor, p)
        {
            editor.status = error;
        }
        ui.add_enabled_ui(p.active, |ui| {
            if ui.button(if p.playing { "Pause" } else { "Play" }).clicked() {
                p.playing = !p.playing;
            }
            if ui.button("Step").clicked() {
                p.step = true;
                p.playing = false;
            }
            if ui.button("Frame -").clicked() {
                p.frame = (p.frame - 1.0).max(0.0);
                p.playing = false;
            }
            if ui.button("Next goal").clicked()
                && let Some(world) = world
            {
                next_goal(world, p);
            }
            ui.toggle_value(&mut p.isolate, "Isolate");
            if ui.button("Stop").clicked() {
                p.active = false;
                p.playing = false;
                editor.preview = true;
            }
        });
    });
}

fn next_goal(world: &WorldGeometry, p: &mut AnimationPreview) {
    let Some(model) = world.data.placements.get(p.placement).and_then(|o| world.data.model(o.kind)) else { return };
    let mut states: Vec<_> = world
        .data
        .animations
        .iter()
        .skip(model.animations[0])
        .take(model.animations[1].saturating_sub(model.animations[0]))
        .map(|c| c.state)
        .collect();
    states.sort_unstable();
    states.dedup();
    if !states.is_empty() {
        let index = states.iter().position(|&state| state == p.goal).unwrap_or(0);
        p.goal = states[(index + 1) % states.len()];
    }
}

fn select(editor: &mut Editor, preview: &mut AnimationPreview) -> Result<(), String> {
    let mut parts = editor.pointer.split('/').skip(1);
    let (table, index) = match (editor.active(), parts.next(), parts.next().and_then(|s| s.parse().ok())) {
        (_, Some("animations"), Some(clip)) => ("animations", clip),
        (Some(index), ..) => ("placements", index),
        _ => return Err("Select an animation or placement first".into()),
    };
    let data = editor.doc.level()?;
    let (clip, placement) = if table == "placements" {
        let object = data.placements.get(index).ok_or("No such placement")?;
        let model = data.model(object.kind).ok_or("The placement has no model")?;
        (model.animations[0], index)
    } else if table == "animations" {
        let model = data
            .models
            .iter()
            .find(|m| (m.animations[0]..m.animations[1]).contains(&index))
            .ok_or("No model uses this clip")?;
        let placement = data
            .placements
            .iter()
            .position(|p| p.kind == model.id)
            .ok_or("The model has no placement in this level")?;
        (index, placement)
    } else {
        return Err("Select an animation or placement first".into());
    };
    let source = data.animations.get(clip).ok_or("This model has no authored clips")?;
    *preview =
        AnimationPreview { active: true, isolate: true, placement, clip, frame: 0.0, goal: source.state, ..default() };
    editor.selection = vec![placement];
    editor.frame = true;
    Ok(())
}

#[allow(clippy::type_complexity)]
fn preview(
    time: Res<Time>,
    world: Option<Res<WorldGeometry>>,
    mut preview: Query<&mut AnimationPreview>,
    mut objects: Query<(&WorldObject, &mut ObjectAnimation)>,
    mut visibility: Query<(&mut Visibility, Option<&WorldObject>), (With<LevelEntity>, With<InRoom>)>,
) {
    let (Some(world), Ok(mut p)) = (world, preview.single_mut()) else { return };
    let mut caption = String::new();
    if p.active {
        if world.data.animations.get(p.clip).is_some() {
            if !p.frame.is_finite() {
                p.frame = 0.0;
            }
            let mut animation = ObjectAnimation { animation: p.clip, frame: p.frame };
            let mut goal = AnimationGoal { state: p.goal };
            if p.step {
                step_authored_animation(&world.data, &mut animation, &mut goal);
                p.step = false;
            } else if p.playing {
                advance_authored_animation(
                    &world.data,
                    &mut animation,
                    &mut goal,
                    time.delta_secs() * engine::animation::ANIMATION_FPS,
                );
            }
            p.clip = animation.animation;
            p.frame = animation.frame;
            p.goal = goal.state;
            for (object, mut a) in &mut objects {
                if object.index == p.placement {
                    *a = animation.clone();
                }
            }
            let clip = &world.data.animations[p.clip];
            caption = format!(
                "Item {} | Clip {} | Frame {:.2} / {} | State {} -> Goal {} | {}\nPreview only; authored cues are silent. Level data is unchanged.",
                p.placement,
                p.clip,
                clip.frame_start as f32 + p.frame,
                clip.frame_end,
                clip.state,
                p.goal,
                if p.playing { "playing" } else { "paused" }
            );
        } else {
            p.active = false;
            caption = "The selected clip no longer exists.".into();
        }
    }
    for (mut visibility, object) in &mut visibility {
        *visibility = if p.active && p.isolate && !object.is_some_and(|o| o.index == p.placement) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if p.caption != caption {
        p.caption = caption;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_a_clip_frames_its_placement_without_changing_the_document() {
        let doc = engine::editor::LevelDocument::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../engine/assets/levels/sandbox/level.json"
        ))
        .unwrap();
        let mut editor = Editor::new(doc);
        editor.navigate("/animations/1".into());
        editor.frame = false;
        let mut p = AnimationPreview::default();
        select(&mut editor, &mut p).unwrap();
        assert!(p.active);
        assert_eq!(p.clip, 1);
        assert_eq!(p.placement, 1);
        assert!(editor.frame);
        assert_eq!(editor.selection, [1]);
        assert!(!editor.doc.is_dirty());
    }
}
