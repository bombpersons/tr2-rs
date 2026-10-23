//! Using objects on the real levels: Lara's use clips, the pickup frame,
//! a switch whose trigger waits for its own clip, and the door it opens.
//! Bounds, alignment, clips and frames come from the original's routines
//! (docs/interaction-evidence.md).
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    animation::ObjectAnimation,
    controller::Button,
    objects::{ObjectTypes, SpawnContext, WorldObject},
    schedule::Avatar,
    testing::{TickApp, headless_app},
    world::{Level, WorldGeometry, angle, position},
};
use game::{
    creature::ItemStatus,
    interactions::{InteractionProgress, InteractionsPlugin, Inventory, UsableItem},
    player::{
        AnimationState, GunStatus, Motion, Player, PlayerAnimations, PlayerMode, SubmergedMotion, Traversal,
        WeaponState,
        contract::{clip, frame},
    },
};

/// A level's objects as the renderer spawns them, without Lara; `lara` is
/// placed instead, standing.
fn app(level: &str, lara: Player) -> (App, Entity) {
    app_with(level, lara, |data| data)
}

/// As [`app`], with the level changed first (flipped, say).
fn app_with(level: &str, lara: Player, edit: impl FnOnce(Level) -> Level) -> (App, Entity) {
    let data: Level =
        edit(serde_json::from_slice(&std::fs::read(format!("assets/imported/levels/{level}/level.json")).unwrap()).unwrap());
    let clips = PlayerAnimations::from(&data);
    let world = WorldGeometry::new(data);
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((game::player::PlayerRulesPlugin, InteractionsPlugin))
        .insert_resource(clips)
        .insert_resource(world.clone());
    let root = app.world_mut().spawn_empty().id();
    for (index, placement) in world.data.placements.iter().enumerate() {
        if placement.kind == game::player::LARA {
            continue;
        }
        let hook = app.world().resource::<ObjectTypes>().get(placement.kind).and_then(|t| t.spawn_hook().cloned());
        let first = world.data.model(placement.kind).map_or(0, |m| m.animations[0]);
        let mut commands = app.world_mut().commands();
        let mut entity = commands.spawn(
            Transform::from_translation(position(placement.position))
                .with_rotation(Quat::from_rotation_y(angle(placement.rotation))),
        );
        match hook {
            Some(hook) => hook(
                &mut entity,
                &SpawnContext { index, placement, world: &world, render_root: root, meshes: &[], lit_material: None },
            ),
            None => {
                entity.insert((
                    WorldObject::from_placement(index, placement),
                    ObjectAnimation { animation: first, frame: 0.0 },
                ));
            }
        }
    }
    let lara = app.world_mut().spawn((lara, AnimationState::at_frame(clip::STAND_IDLE, 0.0))).id();
    app.world_mut().flush();
    (app, lara)
}

fn object(app: &mut App, index: usize) -> Entity {
    app.world_mut().query::<(Entity, &WorldObject)>().iter(app.world()).find(|(_, o)| o.index == index).unwrap().0
}

/// Wall's secret 21 (type 192): Lara stands on it, facing +z.
#[test]
fn a_pickup_lands_in_the_hand_on_frame_42_and_counts_once() {
    let at = position([64000, 384, 36352]);
    let (mut app, lara) = app("wall", Player::at(at - Vec3::Z * 0.05, 0.0, 38));
    let secret = object(&mut app, 21);
    app.run_tick(&[Button::Cross]);
    let player = app.world().get::<Player>(lara).unwrap();
    assert_eq!(player.mode, PlayerMode::Using { animation: clip::PICKUP });
    // AlignLaraPosition: 100 units behind the item along Lara's facing.
    assert!((player.position - (at - Vec3::Z * 100.0 / 1024.0)).length() < 1e-3, "{}", player.position);
    assert_eq!(app.world().get::<WeaponState>(lara).unwrap().status, GunStatus::HandsBusy);
    let a = app.world().get::<AnimationState>(lara).unwrap();
    assert_eq!((a.animation, a.frame), (clip::PICKUP, 0.0));
    for tick in 1..frame::PICKUP_TAKE as usize {
        app.run_tick(&[Button::Cross]);
        assert!(!app.world().get::<WorldObject>(secret).unwrap().consumed, "taken early, tick {tick}");
    }
    app.run_tick(&[Button::Cross]);
    assert!(app.world().get::<WorldObject>(secret).unwrap().consumed);
    assert_eq!(app.world().get::<UsableItem>(secret).unwrap().status, ItemStatus::Invisible);
    // The clip runs to its end (66 frames), then standing frees the hands.
    app.run_ticks(30, &[Button::Cross]);
    assert_eq!(app.world().get::<Player>(lara).unwrap().mode, PlayerMode::Idle);
    assert_eq!(app.world().get::<WeaponState>(lara).unwrap().status, GunStatus::HandsFree);
    assert_eq!(app.world().resource::<Inventory>().items.get(&192), Some(&1));
    assert_eq!(app.world().resource::<InteractionProgress>().pickups, 1);
    assert_eq!(app.world().resource::<InteractionProgress>().secrets, 1);
}

/// Home's switch 32 (type 103, room 64) opens door 34 through its switch
/// trigger, which waits for the switch's own clip to deactivate it.
#[test]
fn a_switch_fires_its_trigger_once_its_own_clip_has_played() {
    let switch_at = position([36352, 3072, 75264]);
    let yaw = angle(16384);
    let lara = Player::at(switch_at + Vec3::X * 400.0 / 1024.0, yaw, 64);
    let (mut app, lara) = app("assault", lara);
    let switch = object(&mut app, 32);
    let door = object(&mut app, 34);
    app.run_tick(&[Button::Cross]);
    let player = app.world().get::<Player>(lara).unwrap();
    assert_eq!(player.mode, PlayerMode::Using { animation: clip::LEVER });
    // Type 103 lines Lara up 292 units in front of it.
    assert!((player.position.x - (36352.0 + 292.0) / 1024.0).abs() < 1e-3, "{}", player.position);
    assert_eq!(app.world().get::<UsableItem>(switch).unwrap().status, ItemStatus::Active);
    let mut fired = None;
    for tick in 0..200 {
        app.run_tick(&[]);
        if fired.is_none() && app.world().get::<WorldObject>(door).unwrap().active {
            fired = Some(tick);
        }
    }
    assert!(fired.is_some(), "the door never opened");
    assert_eq!(app.world().get::<UsableItem>(switch).unwrap().status, ItemStatus::Inactive);
    assert_eq!(app.world().get::<Player>(lara).unwrap().mode, PlayerMode::Idle);
    println!("door activated on tick {:?}", fired);
}

/// Home's front-door button (switch 13, type 103) and the door beside it
/// (12), against `.psx/rec_home_button`: Lara walks one step up to the
/// button from the handoff checkpoint, then presses Action. Ticks count from
/// the press; the recording's switch and door frames are absolute (clip 234
/// starts at 2, clip 245 at 1).
#[test]
fn the_front_door_button_matches_the_recording() {
    let lara = Player::at(position([46515, 2560, 69766]), angle(33932), 50);
    let (mut app, lara) = app("assault", lara);
    let switch = object(&mut app, 13);
    let door = object(&mut app, 12);
    for tick in 0..=80 {
        app.run_tick(if tick == 0 { &[Button::Cross] } else { &[] });
        let player = app.world().get::<Player>(lara).unwrap();
        let animation = app.world().get::<AnimationState>(lara).unwrap();
        let hands = app.world().get::<WeaponState>(lara).unwrap().status;
        let avatar = app.world().get::<Avatar>(lara).unwrap();
        let s = app.world().get::<ObjectAnimation>(switch).unwrap();
        let used = app.world().get::<UsableItem>(switch).unwrap().status;
        let d = app.world().get::<ObjectAnimation>(door).unwrap();
        let open = app.world().get::<WorldObject>(door).unwrap().active;
        // AlignLaraPosition: 292 in front of the button, facing it.
        assert!((player.position - position([46592, 2560, 69852])).length() < 1e-3, "tick {tick}");
        assert!((player.yaw - angle(32768)).abs() < 1e-4, "tick {tick}");
        if tick < 47 {
            assert_eq!((animation.animation, animation.frame), (clip::LEVER, tick as f32), "tick {tick}");
            assert_eq!(hands, GunStatus::HandsBusy, "tick {tick}");
            assert_eq!((s.animation, s.frame, used), (234, tick as f32, ItemStatus::Active), "tick {tick}");
            assert!(!open, "tick {tick}");
        } else {
            assert_eq!(hands, GunStatus::HandsFree, "tick {tick}");
            assert_eq!((s.animation, used), (233, ItemStatus::Inactive), "tick {tick}");
            assert!(open, "tick {tick}");
        }
        // The state handler's camera request, from the tick after the press
        // to the clip's last tick.
        let requested = (1..=47).contains(&tick);
        assert_eq!(avatar.camera_angle, if requested { 14560 } else { 0 }, "tick {tick}");
        assert_eq!(avatar.camera_distance, requested.then_some(1024), "tick {tick}");
        assert_eq!(avatar.camera_speed, requested.then_some(6), "tick {tick}");
        match tick {
            47 => {
                assert_eq!((animation.animation, animation.frame), (clip::STAND, 0.0));
                assert_eq!((d.animation, d.frame), (244, 0.0));
            }
            48..80 => assert_eq!((d.animation, d.frame), (245, (tick - 48) as f32), "tick {tick}"),
            80 => assert_eq!((d.animation, d.frame), (246, 0.0)),
            _ => {}
        }
    }
}

/// Offshore Rig's cell button (switch 21, type 93, timer 14 s) and the cell
/// door (19), against `.psx/rec_rig_timed_switch`: from the handoff
/// checkpoint Lara presses Action once and waits. The switch trigger
/// starts the switch's timer at 420 ticks as Lara's clip ends; the door
/// stays open while it runs, then the switch asks to go back and the door
/// shuts. Switch clips start at 0 (388), 1 (389), 2 (390) and 85 (391); the
/// door's at 0 (405), 1 (406), 62 (407) and 63 (408).
#[test]
fn the_rig_timed_button_matches_the_recording() {
    let lara = Player::at(position([53147, -1536, 94689]), angle(16337), 5);
    let (mut app, lara) = app("rig", lara);
    let switch = object(&mut app, 21);
    let door = object(&mut app, 19);
    for tick in 0..=600 {
        app.run_tick(if tick == 0 { &[Button::Cross] } else { &[] });
        let player = app.world().get::<Player>(lara).unwrap();
        let animation = app.world().get::<AnimationState>(lara).unwrap();
        let hands = app.world().get::<WeaponState>(lara).unwrap().status;
        let s = app.world().get::<ObjectAnimation>(switch).unwrap();
        let timer = app.world().get::<WorldObject>(switch).unwrap().timer;
        let d = app.world().get::<ObjectAnimation>(door).unwrap();
        let open = app.world().get::<WorldObject>(door).unwrap().active;
        // AlignLaraPosition: 362 in front of the button, facing it.
        assert!((player.position - position([53098, -1536, 94720])).length() < 1e-3, "tick {tick}");
        assert!((player.yaw - angle(16384)).abs() < 1e-4, "tick {tick}");
        match tick {
            ..83 => {
                assert_eq!((animation.animation, animation.frame), (clip::BUTTON_OFF, tick as f32), "tick {tick}");
                assert_eq!(hands, GunStatus::HandsBusy, "tick {tick}");
            }
            83 => assert_eq!((animation.animation, animation.frame, hands), (clip::STAND, 0.0, GunStatus::HandsFree)),
            _ => assert_eq!(hands, GunStatus::HandsFree, "tick {tick}"),
        }
        let pressed = match tick {
            ..83 => (390, (tick) as f32, 0),
            83..504 => (389, 0.0, 420 - (tick as i16 - 83)),
            504..587 => (391, (tick - 504) as f32, 0),
            _ => (388, 0.0, 0),
        };
        // TriggerActive leaves -1 once the count runs out (tick 503).
        let pressed = if tick == 503 { (pressed.0, pressed.1, -1) } else { pressed };
        assert_eq!((s.animation, s.frame, timer), pressed, "tick {tick}");
        let door_clip = match tick {
            ..84 => (405, 0.0),
            84..145 => (408, (tick - 84) as f32),
            145..504 => (407, 0.0),
            504..565 => (406, (tick - 504) as f32),
            _ => (405, 0.0),
        };
        assert_eq!((d.animation, d.frame), door_clip, "tick {tick}");
        // The trigger sets the door's code bits as the button's clip ends,
        // and clears them when the button has come back out (tick 587); the
        // door's own copy of the timer closes it before that.
        assert_eq!(open, (83..587).contains(&tick), "tick {tick}");
    }
}

/// Running into Home's closed front door (12) from standing at the button,
/// against `.psx/rec_home_door_push` (UP from run frame 120). The original
/// pins Lara at x 46185 by the door's push and keeps her running, sliding 2
/// units a tick along it: its run collision (0x8003A844) runs after object
/// collision and probes from the pushed position, which never reaches the
/// shut sector (docs/interaction-evidence.md, "Recordings").
#[test]
fn running_into_a_closed_door_matches_the_recording() {
    let lara = Player::at(position([46556, 2560, 69988]), angle(49748), 50);
    let (mut app, lara) = app("assault", lara);
    app.world_mut().entity_mut(lara).insert(AnimationState::at_frame(clip::STAND_IDLE, 32.0));
    let before = [
        (46533, 69989),
        (46508, 69990),
        (46481, 69991),
        (46452, 69992),
        (46421, 69993),
        (46389, 69994),
        (46355, 69995),
        (46319, 69997),
        (46281, 69999),
        (46242, 70001),
        (46201, 70003),
    ];
    for tick in 0..40 {
        app.run_tick(&[Button::Up]);
        let p = app.world().get::<Player>(lara).unwrap();
        let (x, z) = ((p.position.x * 1024.0).round() as i32, (p.position.z * 1024.0).round() as i32);
        let expected = before.get(tick).copied().unwrap_or((46185, 70005 + 2 * (tick as i32 - 11)));
        assert_eq!((x, z), expected, "tick {tick}");
        if tick > 0 {
            assert_eq!(p.mode, PlayerMode::Run, "tick {tick}");
        }
    }
}

/// Wall's first keyhole (1, type 201) and the door it opens (0), against
/// `.psx/rec_wall_keyhole`: from the handoff checkpoint, Action opens the
/// inventory and a second Action chooses key 193 (197 in the ring). The
/// game's ticks stop while the ring is open; tick 0 here is the first one
/// after it closes (run frame 149). The port has no ring yet and uses the
/// key on the press.
#[test]
fn the_first_wall_keyhole_matches_the_recording() {
    let lara = Player::at(position([30821, 0, 33213]), angle(49432), 0);
    let (mut app, lara) = app("wall", lara);
    // The level's first tick clears the keys Lara carried in; give her the
    // key after it.
    app.run_tick(&[]);
    app.world_mut().resource_mut::<Inventory>().items.insert(193, 1);
    let keyhole = object(&mut app, 1);
    let door = object(&mut app, 0);
    for tick in 0..=168 {
        app.run_tick(if tick == 0 { &[Button::Cross] } else { &[] });
        let player = app.world().get::<Player>(lara).unwrap();
        let animation = app.world().get::<AnimationState>(lara).unwrap();
        let hands = app.world().get::<WeaponState>(lara).unwrap().status;
        let avatar = app.world().get::<Avatar>(lara).unwrap();
        let used = app.world().get::<UsableItem>(keyhole).unwrap().status;
        let d = app.world().get::<ObjectAnimation>(door).unwrap();
        let open = app.world().get::<WorldObject>(door).unwrap().active;
        // AlignLaraPosition: 362 in front of the keyhole, facing it.
        assert!((player.position - position([30870, 0, 33280])).length() < 1e-3, "tick {tick}");
        assert!((player.yaw - angle(49152)).abs() < 1e-4, "tick {tick}");
        if tick < 135 {
            assert_eq!((animation.animation, animation.frame), (clip::USE_KEY, tick as f32), "tick {tick}");
            assert_eq!((hands, used, open), (GunStatus::HandsBusy, ItemStatus::Active, false), "tick {tick}");
        } else {
            assert_eq!((hands, used, open), (GunStatus::HandsFree, ItemStatus::Deactivated, true), "tick {tick}");
        }
        let requested = (1..=135).contains(&tick);
        assert_eq!(avatar.camera_angle, if requested { -14560 } else { 0 }, "tick {tick}");
        assert_eq!(avatar.camera_distance, requested.then_some(1024), "tick {tick}");
        assert_eq!(avatar.camera_speed, None, "tick {tick}");
        match tick {
            135 => {
                assert_eq!((animation.animation, animation.frame), (clip::STAND, 0.0));
                assert_eq!((d.animation, d.frame), (327, 0.0));
            }
            136..168 => assert_eq!((d.animation, d.frame), (328, (tick - 136) as f32), "tick {tick}"),
            168 => assert_eq!((d.animation, d.frame), (329, 0.0)),
            _ => {}
        }
    }
    assert_eq!(app.world().resource::<Inventory>().items.get(&193).copied().unwrap_or(0), 0);
}

/// Opera House's first puzzle hole (195, type 182) beside the lift, against
/// `.psx/rec_opera_puzzle_hole`: from the handoff checkpoint Lara walks up
/// to the wall, Action opens the inventory ring and a second Action chooses
/// the Relay Box (174). Tick 0 here is the first after the ring closes
/// (run frame 230). The port has no ring yet and uses the piece on the
/// press.
#[test]
fn the_opera_puzzle_hole_matches_the_recording() {
    let lara = Player::at(position([78307, 2304, 43931]), angle(107), 183);
    let (mut app, lara) = app("opera", lara);
    app.world_mut().entity_mut(lara).insert(AnimationState::at_frame(clip::STAND_IDLE, 14.0));
    app.run_tick(&[]);
    app.world_mut().resource_mut::<Inventory>().items.insert(174, 1);
    let hole = object(&mut app, 195);
    for tick in 0..=120 {
        app.run_tick(if tick == 0 { &[Button::Cross] } else { &[] });
        let player = app.world().get::<Player>(lara).unwrap();
        let animation = app.world().get::<AnimationState>(lara).unwrap();
        let hands = app.world().get::<WeaponState>(lara).unwrap().status;
        let avatar = app.world().get::<Avatar>(lara).unwrap();
        let object = app.world().get::<WorldObject>(hole).unwrap();
        let used = app.world().get::<UsableItem>(hole).unwrap().status;
        // AlignLaraPosition: 327 in front of the hole (0x800897E0), facing it.
        assert!((player.position - position([78336, 2304, 43847])).length() < 1e-3, "tick {tick}");
        assert!(player.yaw.abs() < 1e-4, "tick {tick}");
        assert_eq!(object.kind, if tick < 80 { 182 } else { 186 }, "tick {tick}");
        if tick < 109 {
            assert_eq!((animation.animation, animation.frame), (clip::USE_PUZZLE, tick as f32), "tick {tick}");
            assert_eq!((hands, used), (GunStatus::HandsBusy, ItemStatus::Active), "tick {tick}");
        } else {
            assert_eq!((hands, used), (GunStatus::HandsFree, ItemStatus::Deactivated), "tick {tick}");
        }
        if tick == 109 {
            assert_eq!((animation.animation, animation.frame), (clip::STAND, 0.0));
        }
        let requested = (1..=109).contains(&tick);
        assert_eq!(avatar.camera_angle, if requested { -14560 } else { 0 }, "tick {tick}");
        assert_eq!(avatar.camera_distance, requested.then_some(1024), "tick {tick}");
        assert_eq!(avatar.camera_speed, None, "tick {tick}");
    }
    assert_eq!(app.world().resource::<Inventory>().items.get(&174).copied().unwrap_or(0), 0);
}

/// Wall's second key (15, type 194) at the top of the ladder, against
/// `.psx/rec_wall_pickup`: Lara stands almost on it and presses Action once.
#[test]
fn the_ladder_key_pickup_matches_the_recording() {
    let lara = Player::at(position([25059, -3584, 36378]), angle(54417), 32);
    let (mut app, lara) = app("wall", lara);
    let key = object(&mut app, 15);
    for tick in 0..=70 {
        app.run_tick(if tick == 0 { &[Button::Cross] } else { &[] });
        let player = app.world().get::<Player>(lara).unwrap();
        let animation = app.world().get::<AnimationState>(lara).unwrap();
        let hands = app.world().get::<WeaponState>(lara).unwrap().status;
        let avatar = app.world().get::<Avatar>(lara).unwrap();
        let taken = app.world().get::<WorldObject>(key).unwrap().consumed;
        let key_yaw = app.world().get::<Transform>(key).unwrap().rotation.to_euler(EulerRot::YXZ).0;
        // AlignLaraPosition: 100 behind the key along Lara's heading, which
        // the key takes on as she starts (PickUpCollision returns without
        // restoring the item's rotation).
        assert!((player.position - position([25175, -3584, 36303])).length() < 1e-3, "tick {tick}");
        assert!((key_yaw.rem_euclid(std::f32::consts::TAU) - angle(54417)).abs() < 1e-4, "tick {tick}");
        assert_eq!(taken, tick >= 42, "tick {tick}");
        if tick < 66 {
            assert_eq!((animation.animation, animation.frame), (clip::PICKUP, tick as f32), "tick {tick}");
            assert_eq!(hands, GunStatus::HandsBusy, "tick {tick}");
        } else {
            assert_eq!(hands, GunStatus::HandsFree, "tick {tick}");
        }
        let requested = (1..=66).contains(&tick);
        assert_eq!(avatar.camera_angle, if requested { -23660 } else { 0 }, "tick {tick}");
        assert_eq!(avatar.camera_distance, requested.then_some(1024), "tick {tick}");
    }
    assert_eq!(app.world().resource::<Inventory>().items.get(&194), Some(&1));
}

/// Wall's first key (17, type 193) on the floor of a flooded corridor,
/// against `.psx/rec_wall_underwater_pickup2`: from the checkpoint Lara treads
/// above it holding Action, lets go for ten ticks and holds it again.
/// Each tick she swims 16 units toward a spot above and behind the key,
/// tilting to its nose-down 4550 (MoveLaraPosition), then picks it up from
/// treading and treads again. Ticks are the recording's from run frame 0.
#[test]
fn the_underwater_key_pickup_matches_the_recording() {
    let mut lara = Player::at(position([45211, 6838, 39505]), angle(17073), 34);
    lara.mode = PlayerMode::Swimming;
    let (mut app, lara) = app("wall", lara);
    app.world_mut().entity_mut(lara).insert((
        AnimationState::at_frame(clip::TREAD, (1765 - 1736) as f32),
        Motion { grounded: false, pitch: binary(-3822), ..default() },
        SubmergedMotion::default(),
    ));
    app.world_mut().get_mut::<Traversal>(lara).unwrap().goal = 13;
    let key = object(&mut app, 17);
    // Lara's position and pitch after each tick until she is there.
    let approach = [
        (45221, 6828, 39500, -4186),
        (45231, 6818, 39495, -4550),
        (45241, 6808, 39490, -4550),
        (45251, 6798, 39485, -4550),
        (45261, 6788, 39480, -4550),
        (45271, 6778, 39475, -4550),
        (45281, 6768, 39470, -4550),
        (45291, 6758, 39465, -4550),
        (45301, 6748, 39459, -4550),
        (45311, 6738, 39454, -4550),
        (45320, 6727, 39448, -4550),
        (45330, 6716, 39442, -4550),
        (45335, 6711, 39439, -4550),
    ];
    for tick in 0..=55 {
        let action = (0..=3).contains(&tick) || (14..=47).contains(&tick);
        app.run_tick(if action { &[Button::Cross] } else { &[] });
        let player = app.world().get::<Player>(lara).unwrap();
        let motion = app.world().get::<Motion>(lara).unwrap();
        let animation = app.world().get::<AnimationState>(lara).unwrap();
        let avatar = app.world().get::<Avatar>(lara).unwrap();
        let (x, y, z, pitch) = approach[if tick < 4 { tick } else { (tick.max(13) - 10).min(12) }];
        let at = original(player.position);
        assert_eq!((at, turn(motion.pitch), turn(player.yaw)), ([x, y, z], pitch, 17073), "tick {tick}");
        assert_eq!(app.world().get::<WeaponState>(lara).unwrap().status, GunStatus::HandsFree, "tick {tick}");
        assert_eq!(player.mode, PlayerMode::Swimming, "tick {tick}");
        match tick {
            ..22 => assert_eq!(animation.animation, clip::TREAD, "tick {tick}"),
            22..=54 => {
                assert_eq!((animation.animation, animation.frame), (clip::UNDERWATER_PICKUP, (tick - 22) as f32))
            }
            _ => assert_eq!((animation.animation, animation.frame), (clip::TREAD, 21.0)),
        }
        assert_eq!(app.world().get::<WorldObject>(key).unwrap().consumed, tick >= 40, "tick {tick}");
        // The key keeps the tilt and heading the test gave it.
        let (key_yaw, key_pitch, _) = app.world().get::<Transform>(key).unwrap().rotation.to_euler(EulerRot::YXZ);
        assert_eq!((turn(key_yaw), turn(-key_pitch)), (17073, -4550), "tick {tick}");
        let requested = (23..=55).contains(&tick);
        assert_eq!(avatar.camera_angle, if requested { -23660 } else { 0 }, "tick {tick}");
        assert_eq!(avatar.camera_distance, requested.then_some(1024), "tick {tick}");
    }
    assert_eq!(app.world().resource::<Inventory>().items.get(&193), Some(&1));
}

/// Venice's underwater switch (40, type 105, room 49), against
/// `.psx/rec_venice_uw_switch`: from the handoff checkpoint Lara treads in
/// front of it and holds Action. Each tick she swims 16 units toward a spot
/// 108 in front of the switch, levelling out and turning to face it
/// (MoveLaraPosition), then pulls it from treading (SwitchCollision2
/// 0x80050AC4) and treads again once the clip's EmptyHands has freed her
/// hands. Ticks are the recording's from run frame 0; clip 412 starts at 2.
/// The level had flipped by then: room 49 holds its flooded alternate, 51.
#[test]
fn the_underwater_switch_matches_the_recording() {
    let mut lara = Player::at(position([33590, 1220, 36175]), angle((-14969i32) as u16), 49);
    lara.mode = PlayerMode::Swimming;
    let (mut app, lara) = app_with("boat", lara, |level| engine::editor::flipped(&level));
    app.world_mut().entity_mut(lara).insert((
        AnimationState::at_frame(clip::TREAD, (1781 - 1736) as f32),
        Motion { grounded: false, pitch: binary(546), ..default() },
        SubmergedMotion::default(),
    ));
    app.world_mut().get_mut::<Traversal>(lara).unwrap().goal = 13;
    let switch = object(&mut app, 40);
    // Lara's position, pitch and yaw after each tick until she is there.
    let approach = [
        (33576, 1218, 36181, 182, -15333),
        (33562, 1216, 36187, 0, -15697),
        (33548, 1214, 36193, 0, -16061),
        (33534, 1212, 36199, 0, -16384),
        (33520, 1210, 36205, 0, -16384),
        (33506, 1208, 36211, 0, -16384),
        (33492, 1206, 36217, 0, -16384),
        (33478, 1204, 36223, 0, -16384),
        (33464, 1202, 36229, 0, -16384),
        (33450, 1200, 36235, 0, -16384),
        (33436, 1198, 36241, 0, -16384),
        (33422, 1196, 36247, 0, -16384),
        (33408, 1194, 36253, 0, -16384),
        (33394, 1192, 36259, 0, -16384),
        (33380, 1190, 36265, 0, -16384),
        (33366, 1188, 36271, 0, -16384),
        (33352, 1186, 36277, 0, -16384),
        (33338, 1184, 36283, 0, -16384),
        (33324, 1182, 36289, 0, -16384),
        (33310, 1180, 36295, 0, -16384),
        (33296, 1178, 36301, 0, -16384),
        (33282, 1175, 36307, 0, -16384),
        (33268, 1172, 36312, 0, -16384),
        (33254, 1169, 36318, 0, -16384),
        (33240, 1166, 36324, 0, -16384),
        (33226, 1163, 36330, 0, -16384),
        (33212, 1161, 36335, 0, -16384),
        (33198, 1158, 36341, 0, -16384),
        (33184, 1155, 36347, 0, -16384),
    ];
    for tick in 0..=200 {
        app.run_tick(if tick < 120 { &[Button::Cross] } else { &[] });
        let player = app.world().get::<Player>(lara).unwrap();
        let motion = app.world().get::<Motion>(lara).unwrap();
        let animation = app.world().get::<AnimationState>(lara).unwrap();
        let hands = app.world().get::<WeaponState>(lara).unwrap().status;
        let avatar = app.world().get::<Avatar>(lara).unwrap();
        let s = app.world().get::<ObjectAnimation>(switch).unwrap();
        let item = app.world().get::<UsableItem>(switch).unwrap();
        let (x, y, z, pitch, yaw) = approach.get(tick).copied().unwrap_or((33172, 1152, 36352, 0, -16384));
        let at = original(player.position);
        assert_eq!((at, turn(motion.pitch), turn(player.yaw)), ([x, y, z], pitch, yaw), "tick {tick}");
        assert_eq!(player.mode, PlayerMode::Swimming, "tick {tick}");
        assert_eq!(hands == GunStatus::HandsBusy, (29..=174).contains(&tick), "tick {tick}");
        match tick {
            ..29 => assert_eq!(animation.animation, clip::TREAD, "tick {tick}"),
            29..=174 => assert_eq!(
                (animation.animation, animation.frame),
                (clip::UNDERWATER_SWITCH, (tick - 29) as f32),
                "tick {tick}"
            ),
            _ => assert_eq!((animation.animation, animation.frame), (clip::TREAD, (21 + tick - 175) as f32)),
        }
        match tick {
            ..29 => assert_eq!((s.animation, s.frame, item.status), (410, 0.0, ItemStatus::Inactive), "tick {tick}"),
            29..=150 => {
                assert_eq!((s.animation, s.frame, item.state), (412, (tick - 29) as f32, 1), "tick {tick}");
                assert_eq!(item.status, ItemStatus::Active, "tick {tick}");
            }
            _ => assert_eq!((s.animation, s.frame, item.state), (411, 0.0, 0), "tick {tick}"),
        }
        // The state-40 handler's camera request, as on land.
        let requested = (30..=175).contains(&tick);
        assert_eq!(avatar.camera_angle, if requested { 14560 } else { 0 }, "tick {tick}");
        assert_eq!(avatar.camera_distance, requested.then_some(1024), "tick {tick}");
        assert_eq!(avatar.camera_speed, requested.then_some(6), "tick {tick}");
    }
}

fn binary(angle: i32) -> f32 {
    angle as f32 * std::f32::consts::TAU / 65536.0
}

fn turn(radians: f32) -> i32 {
    (radians * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32
}

fn original(p: Vec3) -> [i32; 3] {
    let v = (p * 1024.0).round();
    [v.x as i32, -v.y as i32, v.z as i32]
}
