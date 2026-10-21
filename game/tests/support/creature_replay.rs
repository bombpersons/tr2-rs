//! Creatures against the original, tick by tick, from a played run
//! (tools/record_wall_traps.py): the port runs the recorded creatures with
//! Lara as their input.
//!
//! Before each tick the port is given Lara's pose, animation, speed and
//! health from the previous tick's row, each creature's touch bits and hit
//! flag, and the control random seed (as the creatures' control started,
//! where the recording has it); after her phase, the creatures her
//! trigger activated (in its action order) and the hit points her shots
//! left. Between ticks the idle search makes the slot visits the original
//! made, from where its cursor stood, and each creature searches in the AI
//! slot it holds there (other creatures, not replayed, hold the rest).
//! Everything a creature does itself is compared exactly, with its AI
//! slot's fields while it holds one.
use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, AnimationState, ControllerAnimated, ObjectAnimation},
    objects::{Initialized, WorldObject},
    random::GameRandom,
    schedule::{InteractionSystems, WorldSystems},
    testing::{TickApp, Trajectory, headless_app},
    triggers::{ObjectActivated, TriggerSystems},
    world::{Level, NO_BOX, WorldGeometry},
};
use game::{
    creature::{Creature, IdleSearch, ItemBody, Mood, spawn_components},
    interactions::{Enemy, InteractionsPlugin},
    items::ItemStatus,
    player::{Motion, Player, SimulationEnabled},
};

/// Item fields compared for every creature.
pub const ITEM_FIELDS: [&str; 15] = [
    "x",
    "y",
    "z",
    "pitch",
    "yaw",
    "state",
    "goal",
    "required",
    "animation",
    "frame",
    "room",
    "speed",
    "fall_speed",
    "floor",
    "health",
];

#[derive(Resource)]
struct Replayed {
    row: usize,
    original: Trajectory,
    starts: Vec<usize>,
    lara: Entity,
    creatures: Vec<(usize, Entity)>,
}

impl Replayed {
    fn get(&self, row: usize, name: &str) -> f64 {
        self.original.get(row, name).unwrap_or_else(|| panic!("no column {name}"))
    }
}

/// A recorded case being replayed.
pub struct CreatureReplay {
    pub app: App,
    pub original: Trajectory,
    pub starts: Vec<usize>,
    pub lara: Entity,
    /// The replayed items, in the order their trigger activates them, with
    /// their entities.
    pub creatures: Vec<(usize, Entity)>,
    /// AI slot fields compared (as recorded, `s{slot}_{field}`).
    slot_fields: Vec<&'static str>,
}

impl CreatureReplay {
    /// Set up `items` (all of type `kind`) as they stand in the recording's
    /// first row. `pre_initialised` marks components a type's own set-up
    /// would otherwise add and run again.
    pub fn new(
        csv: &str,
        kind: u16,
        items: &[usize],
        slot_fields: &[&'static str],
        pre_initialised: impl Fn(&mut EntityWorldMut),
    ) -> Self {
        let original = Trajectory::parse_csv(csv).unwrap();
        let data: Level = serde_json::from_slice(include_bytes!("../../assets/levels/wall/level.json")).unwrap();
        let world = WorldGeometry::new(data);
        let starts: Vec<usize> = world.data.animations.iter().map(|c| c.frame_start).collect();
        let mut app = headless_app(game::tick_rate(game::REGION));
        app.add_plugins(InteractionsPlugin).insert_resource(world.clone());
        let row0 = |name: &str| original.get(0, name).unwrap_or_else(|| panic!("no column {name}"));
        let lara =
            app.world_mut().spawn((Player::at(Vec3::ZERO, 0.0, row0("lara_room") as usize), Motion::default())).id();
        let mut creatures = Vec::new();
        for &index in items {
            let object = WorldObject::from_placement(index, &world.data.placements[index]);
            assert_eq!(object.kind, kind);
            let (creature, mut body) = spawn_components(&world, &object, Enemy::setup_for_kind(kind).unwrap());
            let field = |name: &str| row0(&format!("i{index}_{name}"));
            body.position = [field("x") as i32, field("y") as i32, field("z") as i32];
            body.yaw = field("yaw") as i16;
            body.floor = field("floor") as i32;
            body.current_state = field("state") as usize;
            body.box_number = NO_BOX;
            let clip = field("animation") as usize;
            let mut entity = app.world_mut().spawn((
                object,
                Enemy::for_kind(kind).unwrap(),
                creature,
                body,
                ControllerAnimated,
                AnimationGoal { state: field("goal") as usize },
                ObjectAnimation { animation: clip, frame: (field("frame") as usize - starts[clip]) as f32 },
                Transform::default(),
                // Built here, so the registry may not build it again.
                Initialized,
            ));
            pre_initialised(&mut entity);
            creatures.push((index, entity.id()));
        }
        app.insert_resource(Replayed {
            row: 0,
            original: original.clone(),
            starts: starts.clone(),
            lara,
            creatures: creatures.clone(),
        });
        app.add_systems(
            FixedUpdate,
            (
                replay.in_set(WorldSystems::Prepare),
                replay_lara_effects.in_set(InteractionSystems::Triggers).before(TriggerSystems::Dispatch),
                pin_slots.in_set(WorldSystems::Camera),
            ),
        );
        app.insert_resource(SimulationEnabled(true));
        let visits = original.get(0, "idle_steps").unwrap() as usize;
        let cursor = original.get(0, "search_cursor").unwrap() as usize;
        app.insert_resource(IdleSearch { steps: None, cursor: (cursor + visits) % 5 });
        Self { app, original, starts, lara, creatures, slot_fields: slot_fields.to_vec() }
    }

    /// The compared columns: each creature's item fields, then its slot's.
    pub fn columns(&self) -> Vec<String> {
        let mut columns = Vec::new();
        for &(index, _) in &self.creatures {
            columns.extend(ITEM_FIELDS.iter().map(|f| format!("i{index}_{f}")));
            columns.extend(self.slot_fields.iter().map(|f| format!("c{index}_{f}")));
        }
        columns
    }

    /// The original's values of [`Self::columns`] at `row`. A slot's fields
    /// only mean something while the creature holds it; a released slot
    /// reads as zeros in both.
    pub fn expected(&self, row: usize) -> Vec<f64> {
        let mut values = Vec::new();
        for &(index, _) in &self.creatures {
            values.extend(ITEM_FIELDS.iter().map(|f| self.original.get(row, &format!("i{index}_{f}")).unwrap()));
            match slot_of(&self.original, row, index) {
                Some(slot) => values
                    .extend(self.slot_fields.iter().map(|f| self.original.get(row, &format!("s{slot}_{f}")).unwrap())),
                None => values.extend(vec![0.0; self.slot_fields.len()]),
            }
        }
        values
    }

    /// Run the tick that produces `row` from the row before it, and return
    /// the port's values of [`Self::columns`].
    pub fn step(&mut self, row: usize) -> Vec<f64> {
        self.app.world_mut().resource_mut::<Replayed>().row = row - 1;
        let visits = self.original.get(row, "idle_steps").unwrap() as usize;
        self.app.world_mut().resource_mut::<IdleSearch>().steps = Some(visits);
        self.app.run_tick(&[]);
        self.port()
    }

    fn port(&self) -> Vec<f64> {
        let world = self.app.world();
        let mut values = Vec::new();
        for &(_, entity) in &self.creatures {
            let body = world.get::<ItemBody>(entity).unwrap();
            let animation = world.get::<ObjectAnimation>(entity).unwrap();
            let goal = world.get::<AnimationGoal>(entity).unwrap();
            let enemy = world.get::<Enemy>(entity).unwrap();
            let creature = world.get::<Creature>(entity).unwrap();
            values.extend([
                f64::from(body.position[0]),
                f64::from(body.position[1]),
                f64::from(body.position[2]),
                f64::from(body.pitch),
                f64::from(body.yaw),
                body.current_state as f64,
                goal.state as f64,
                body.required_state as f64,
                animation.animation as f64,
                (self.starts[animation.animation] + animation.frame as usize) as f64,
                body.room as f64,
                f64::from(body.speed),
                f64::from(body.fall_speed),
                f64::from(body.floor),
                f64::from(enemy.health),
            ]);
            match creature.search.as_ref() {
                Some(search) => values.extend(self.slot_fields.iter().map(|&f| match f {
                    "mood" => mood_code(creature.mood),
                    "max_turn" => f64::from(creature.max_turn),
                    "flags" => f64::from(creature.flags),
                    "head" => f64::from(creature.head),
                    "neck" => f64::from(creature.neck),
                    "target_x" => f64::from(creature.target[0]),
                    "target_y" => f64::from(creature.target[1]),
                    "target_z" => f64::from(creature.target[2]),
                    "required_box" => f64::from(search.required_box),
                    _ => panic!("no port value for slot field {f}"),
                })),
                None => values.extend(vec![0.0; self.slot_fields.len()]),
            }
        }
        values
    }

    /// Replay every row, comparing exactly. `CREATURE_DUMP=path` also writes
    /// both trajectories.
    pub fn assert_matches(mut self, mut after_tick: impl FnMut(&mut Self, usize)) {
        let columns = self.columns();
        let names: Vec<&str> = columns.iter().map(String::as_str).collect();
        let mut expected = Trajectory::new(&names);
        let mut rebuild = Trajectory::new(&names);
        expected.push(self.expected(0));
        rebuild.push(self.expected(0));
        for row in 1..self.original.ticks() {
            let values = self.step(row);
            after_tick(&mut self, row);
            let mut original = self.expected(row);
            // A slot keeps its last holder's target until its new holder's
            // first control computes one (InitialiseSlot leaves it).
            for &(index, _) in &self.creatures {
                if slot_of(&self.original, row - 1, index).is_none() && slot_of(&self.original, row, index).is_some() {
                    for axis in ["x", "y", "z"] {
                        let column = format!("c{index}_target_{axis}");
                        if let Some(i) = columns.iter().position(|c| *c == column) {
                            original[i] = values[i];
                        }
                    }
                }
            }
            expected.push(original);
            rebuild.push(values);
        }
        if let Ok(path) = std::env::var("CREATURE_DUMP") {
            std::fs::write(format!("{path}.port.csv"), rebuild.to_csv()).unwrap();
            std::fs::write(format!("{path}.original.csv"), expected.to_csv()).unwrap();
        }
        let tolerances: Vec<_> = names.iter().map(|&c| (c, 0.0)).collect();
        engine::testing::compare(&expected, &rebuild, &tolerances).unwrap().assert_passed();
    }
}

fn replay(
    enabled: Res<SimulationEnabled>,
    replayed: Res<Replayed>,
    mut random: ResMut<GameRandom>,
    mut players: Query<(&mut Player, &mut Motion, &mut AnimationState)>,
    mut creatures: Query<&mut ItemBody>,
) {
    if !enabled.0 {
        return;
    }
    let r = &*replayed;
    let row = |name: &str| r.get(r.row, name);
    if let Ok((mut player, mut motion, mut animation)) = players.get_mut(r.lara) {
        player.position = Vec3::new(row("lara_x") as f32, -row("lara_y") as f32, row("lara_z") as f32) / 1024.0;
        player.yaw = (row("lara_yaw") as f32 * std::f32::consts::TAU / 65536.0).rem_euclid(std::f32::consts::TAU);
        player.room = row("lara_room") as usize;
        player.health = row("lara_health") as i32;
        motion.velocity = player.forward() * (row("lara_speed").abs() as f32 * 30.0 / 1024.0);
        let clip = row("lara_animation") as usize;
        *animation = AnimationState::at_frame(clip, (row("lara_frame") as usize - r.starts[clip]) as f32);
    }
    for &(index, entity) in &r.creatures {
        if let Ok(mut body) = creatures.get_mut(entity) {
            body.touch_bits = row(&format!("i{index}_touch")) as u32;
            body.hit = row(&format!("i{index}_status")) as u32 & 0x10 != 0;
        }
    }
    // Creatures not replayed here draw from it earlier in the tick; where
    // the recording noted the seed as the replayed type's control started,
    // that is used.
    random.seed = match r.original.get(r.row + 1, "control_seed") {
        Some(seed) if seed >= 0.0 => seed as u32,
        _ => row("seed") as u32,
    };
}

/// Lara's phase: her trigger activates creatures, her shots take hit
/// points. Only a creature's own death writes -16384, so that is never
/// replayed.
fn replay_lara_effects(
    replayed: Res<Replayed>,
    mut creatures: Query<(&mut WorldObject, &ItemBody, &mut Enemy)>,
    mut activated: MessageWriter<ObjectActivated>,
) {
    let r = &*replayed;
    for &(index, entity) in &r.creatures {
        let Ok((mut object, body, mut enemy)) = creatures.get_mut(entity) else { continue };
        if r.get(r.row + 1, &format!("i{index}_status")) as u32 & 1 != 0 && body.status == ItemStatus::Inactive {
            object.flags |= engine::objects::CODE_BITS;
            object.update_active();
            activated.write(ObjectActivated { entity, index });
        }
        let health = r.get(r.row + 1, &format!("i{index}_health")) as i32;
        if health != -16384 {
            enemy.health = health;
        }
    }
}

/// Before the idle search each creature takes the slot it holds in the
/// original at the end of the tick.
fn pin_slots(replayed: Res<Replayed>, mut creatures: Query<&mut Creature>) {
    let r = &*replayed;
    for &(index, entity) in &r.creatures {
        if let (Ok(mut creature), Some(slot)) = (creatures.get_mut(entity), slot_of(&r.original, r.row + 1, index)) {
            creature.slot = slot;
        }
    }
}

fn mood_code(mood: Mood) -> f64 {
    match mood {
        Mood::Bored => 0.0,
        Mood::Attack => 1.0,
        Mood::Escape => 2.0,
        Mood::Stalk => 3.0,
    }
}

/// The AI slot a creature holds in the original at `row`, if any.
pub fn slot_of(original: &Trajectory, row: usize, index: usize) -> Option<usize> {
    (0..5).find(|slot| original.get(row, &format!("s{slot}_item")) == Some(index as f64))
}
