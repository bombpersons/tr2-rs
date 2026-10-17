//! Door pose trajectories recorded from the supplied disc with psx. The
//! native initial item is already initialized; restore that state directly.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ObjectAnimation, ObjectAnimationCue},
    objects::{Initialized, WorldObject},
    schedule::WorldSystems,
    testing::{TickApp, headless_app},
    world::{Command, Level, WorldGeometry},
};
use game::interactions::{Door, InteractionsPlugin};

#[derive(Resource, Default)]
struct SoundCues(Vec<(usize, i64, usize)>);
fn collect(mut cues: MessageReader<ObjectAnimationCue>, mut sounds: ResMut<SoundCues>) {
    for event in cues.read() {
        if let Command::Sound { sound, .. } = event.cue.command {
            sounds.0.push((event.cue.animation, event.cue.frame, sound));
        }
    }
}

fn compare(csv: &str) -> Vec<(usize, i64, usize)> {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/wall/level.json")).unwrap();
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(InteractionsPlugin)
        .insert_resource(WorldGeometry::new(data))
        .init_resource::<SoundCues>()
        .add_systems(FixedUpdate, collect.in_set(WorldSystems::Effects));
    let rows: Vec<Vec<&str>> = csv.lines().skip(1).map(|s| s.split(',').collect()).collect();
    let initial = &rows[0];
    // run_frame, field, kind, state, goal, animation, animation_frame, flags
    let clip: usize = initial[5].parse().unwrap();
    let absolute: i32 = initial[6].parse().unwrap();
    let frame = absolute as f32 - app.world().resource::<WorldGeometry>().data.animations[clip].frame_start as f32;
    let placement = app.world().resource::<WorldGeometry>().data.placements[72].clone();
    let mut object = WorldObject::from_placement(72, &placement);
    object.flags = u16::from_str_radix(initial[7].trim_start_matches("0x"), 16).unwrap();
    object.update_active();
    let door = app
        .world_mut()
        .spawn((
            Initialized,
            object,
            Door::default(),
            ObjectAnimation { animation: clip, frame },
            AnimationGoal { state: initial[4].parse().unwrap() },
        ))
        .id();
    for row in rows.iter().skip(2).step_by(2) {
        let field: usize = row[0].parse().unwrap();
        let flags = u16::from_str_radix(row[7].trim_start_matches("0x"), 16).unwrap();
        let mut object = app.world_mut().get_mut::<WorldObject>(door).unwrap();
        object.flags = flags;
        object.update_active();
        app.run_tick(&[]);
        let a = app.world().get::<ObjectAnimation>(door).unwrap();
        let goal = app.world().get::<AnimationGoal>(door).unwrap();
        let c = &app.world().resource::<WorldGeometry>().data.animations[a.animation];
        let frame = c.frame_start as f32 + a.frame;
        assert_eq!(
            (a.animation, frame, c.state, goal.state),
            (row[5].parse().unwrap(), row[6].parse().unwrap(), row[3].parse().unwrap(), row[4].parse().unwrap()),
            "native run field {field}"
        );
    }
    std::mem::take(&mut app.world_mut().resource_mut::<SoundCues>().0)
}

#[test]
fn native_unmodified_open_door_holds_its_final_pose() {
    assert!(compare(include_str!("data/original/door_control.csv")).is_empty());
}

#[test]
fn native_close_open_cycle_matches_every_pose_goal_and_frame() {
    assert_eq!(compare(include_str!("data/original/door_cycle.csv")), [(332, 1, 64), (334, 63, 64)]);
}

#[test]
fn heldout_reopen_during_close_finishes_closing_before_opening() {
    // Captured after the rule was written; only activation flags were poked.
    assert_eq!(compare(include_str!("data/original/door_heldout.csv")), [(332, 1, 64), (334, 63, 64)]);
}
