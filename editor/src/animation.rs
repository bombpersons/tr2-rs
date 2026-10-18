//! Authoring preview: engine clip rules, no game controllers and no audio.
use crate::Editor;
use bevy::prelude::*;
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
}
#[derive(Component)]
struct Caption;
#[derive(Component, Clone, Copy)]
enum Action {
    Select,
    Play,
    Step,
    Previous,
    Goal,
    Stop,
    Isolate,
}

pub struct AnimationPreviewPlugin;
impl Plugin for AnimationPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, setup)
            .add_systems(Update, preview.after(LevelSystems::Load).before(LevelSystems::Pose));
    }
}

fn setup(mut commands: Commands, cameras: Query<Entity, With<Camera2d>>) {
    let Ok(camera) = cameras.single() else { return };
    commands
        .spawn((
            UiTargetCamera(camera),
            Node {
                position_type: PositionType::Absolute,
                left: px(516),
                right: px(16),
                bottom: px(16),
                padding: UiRect::all(px(10)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
            BackgroundColor(Color::srgb(0.055, 0.07, 0.10)),
        ))
        .with_children(|root| {
            root.spawn((
                Caption,
                Text::new("Select an animation or placement, then Preview selection."),
                TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() },
            ));
            root.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: px(5), row_gap: px(5), ..default() })
                .with_children(|row| {
                    for (label, action) in [
                        ("Preview selection", Action::Select),
                        ("Play / Pause", Action::Play),
                        ("Step", Action::Step),
                        ("Frame -", Action::Previous),
                        ("Next goal", Action::Goal),
                        ("Scene / Isolate", Action::Isolate),
                        ("Stop", Action::Stop),
                    ] {
                        row.spawn((
                            action,
                            Node { padding: UiRect::axes(px(8), px(5)), ..default() },
                            BackgroundColor(Color::srgb(0.12, 0.17, 0.24)),
                        ))
                        .observe(click)
                        .with_child((
                            Text::new(label),
                            TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() },
                        ));
                    }
                });
        });
}

fn select(editor: &mut Editor, preview: &mut AnimationPreview) -> Result<(), String> {
    let pointer = crate::ui::selected_pointer(editor);
    let mut parts = pointer.split('/').skip(1);
    let table = parts.next().unwrap_or("");
    let index: usize = parts.next().and_then(|s| s.parse().ok()).ok_or("Select an animation or placement first")?;
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
    editor.pointer = format!("/placements/{placement}");
    editor.page = 0;
    editor.selected = 0;
    editor.frame = true;
    Ok(())
}

fn click(
    mut event: On<bevy::picking::events::PointerClick>,
    actions: Query<&Action>,
    mut editor: ResMut<Editor>,
    mut previews: Query<&mut AnimationPreview>,
    world: Option<Res<WorldGeometry>>,
) {
    if event.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    let Ok(action) = actions.get(event.entity) else { return };
    if editor.input.is_some() {
        return;
    }
    let Ok(mut p) = previews.single_mut() else { return };
    match action {
        Action::Select => {
            if let Err(error) = select(&mut editor, &mut p) {
                editor.status = error;
            }
        }
        Action::Play => {
            p.playing = !p.playing;
        }
        Action::Step => {
            p.step = true;
            p.playing = false;
        }
        Action::Previous => {
            p.frame = (p.frame - 1.0).max(0.0);
            p.playing = false;
        }
        Action::Goal => {
            if let Some(world) = world
                && let Some(object) = world.data.placements.get(p.placement)
                && let Some(model) = world.data.model(object.kind)
            {
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
        }
        Action::Isolate => p.isolate = !p.isolate,
        Action::Stop => {
            p.active = false;
            p.playing = false;
            editor.preview = true;
        }
    }
    event.propagate(false);
}

#[allow(clippy::type_complexity)]
fn preview(
    time: Res<Time>,
    world: Option<Res<WorldGeometry>>,
    mut preview: Query<&mut AnimationPreview>,
    mut objects: Query<(&WorldObject, &mut ObjectAnimation)>,
    mut captions: Query<&mut Text, With<Caption>>,
    mut visibility: Query<(&mut Visibility, Option<&WorldObject>), (With<LevelEntity>, With<InRoom>)>,
) {
    let (Some(world), Ok(mut p)) = (world, preview.single_mut()) else { return };
    let mut caption = "Select an animation or placement, then Preview selection.".to_string();
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
    for mut text in &mut captions {
        if **text != caption {
            **text = caption.clone();
        }
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
        let mut editor = Editor {
            doc,
            pointer: "/animations".into(),
            selected: 1,
            page: 0,
            status: String::new(),
            input: None,
            revision: 0,
            preview: false,
            frame: false,
            recent: vec![],
            exit_requested: false,
            overview: false,
        };
        let mut p = AnimationPreview::default();
        select(&mut editor, &mut p).unwrap();
        assert!(p.active);
        assert_eq!(p.clip, 1);
        assert_eq!(p.placement, 1);
        assert!(editor.frame);
        assert!(!editor.doc.is_dirty());
    }
}
