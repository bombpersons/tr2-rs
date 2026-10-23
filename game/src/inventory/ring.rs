//! The inventory ring's logic: Display_Inventory `0x80030F30`, one call of
//! [`Ring::frame`] per pass of its loop.
//!
//! The ring is a circle of items around a camera. Each change of the ring
//! (opening, closing, swapping to another ring, bringing an item forward)
//! is a [`Motion`]: targets reached by fixed steps over a count of vsyncs
//! (RingDoMotions `0x80034CD0`). Turning the ring is separate: 24 steps of
//! a 24th of the angle between items. A frame's input is the original's
//! input word ([`input`]); Cross, Triangle and Select arrive only on the
//! frame they are pressed, the directions while held. The game runs a frame
//! per logic tick, two vsyncs; tests replay the original's own counts.
//!
//! Angles are binary (65536 to a turn), distances world units, as the
//! original's shorts and ints.
use std::collections::HashMap;

use bevy::prelude::*;

use super::{Inventory, ItemTable, item};

/// The input word's bits the ring reads (`0x8008AAD4`).
pub mod input {
    pub const FORWARD: u32 = 0x1;
    pub const BACK: u32 = 0x2;
    pub const LEFT: u32 = 0x4;
    pub const RIGHT: u32 = 0x8;
    /// The Select button: open or close the inventory.
    pub const OPTION: u32 = 0x100;
    /// Cross: choose the item in front.
    pub const SELECT: u32 = 0x100000;
    /// Triangle: put it back, or close the ring.
    pub const DESELECT: u32 = 0x200000;
}

/// Why the ring was opened (Display_Inventory's argument). The title (1),
/// save (3), load (4) and death (5) passports are not ported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum RingMode {
    /// Select in play (0): the main ring, or the options when it is empty.
    #[default]
    Game,
    /// Action at a keyhole or puzzle hole (2): only the keys ring, which
    /// cannot be left for another.
    Keys,
}

/// Which items the ring shows (`ring+0x4`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum RingKind {
    #[default]
    Main,
    Options,
    Keys,
}

/// The ring's state (`motion+0x2`), in the original's numbering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum Status {
    #[default]
    Opening = 0,
    Open = 1,
    /// Shrinking away: closing, or before another ring.
    Closing = 2,
    MainToOptions = 3,
    MainToKeys = 4,
    KeysToMain = 5,
    OptionsToMain = 6,
    /// Bringing an item forward, then showing it.
    Selecting = 7,
    Selected = 8,
    /// Putting it back.
    Deselecting = 9,
    Deselect = 10,
    /// Waiting for the item to finish closing.
    ClosingItem = 11,
    Exiting = 12,
    Done = 13,
}

/// A change of the ring over `count` vsyncs (IMOTION_INFO at `ring+0x50`).
/// Targets are set when it ends; rates are added every vsync until then.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
pub struct Motion {
    pub count: i16,
    pub status: Status,
    pub status_target: Status,
    pub radius_target: i16,
    pub radius_rate: i16,
    pub camera_y_target: i16,
    pub camera_y_rate: i16,
    pub camera_pitch_target: i16,
    pub camera_pitch_rate: i16,
    pub rotate_target: i16,
    pub rotate_rate: i16,
    pub item_pt_xrot_target: i16,
    pub item_pt_xrot_rate: i16,
    pub item_x_rot_target: i16,
    pub item_x_rot_rate: i16,
    pub item_y_target: i32,
    pub item_y_rate: i32,
    pub item_z_target: i32,
    pub item_z_rate: i32,
}

/// An item's state on the ring, kept between openings (the live fields of
/// its record at `0x80087FCC`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
pub struct ItemPose {
    /// Its object: the passport becomes the open passport while selected.
    pub object: u16,
    /// Animation frame, the frame it heads for, its direction and the
    /// countdown between frames.
    pub frame: i16,
    pub goal: i16,
    pub direction: i16,
    pub count: i16,
    pub pt_xrot: i16,
    pub x_rot: i16,
    pub y_rot: i16,
    pub y: i32,
    pub z: i32,
    /// Joints drawn, a bit each.
    pub meshes: i32,
}

/// What the ring left chosen when it closed (`0x80089032`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RingExit {
    pub chosen: Option<u16>,
}

/// The open ring: Display_Inventory's ring and motion structs and the
/// globals it keeps across frames.
#[derive(Clone, Debug, Reflect)]
pub struct Ring {
    pub mode: RingMode,
    pub kind: RingKind,
    /// The items of each ring, by object, in ring order.
    #[reflect(ignore)]
    pub lists: [Vec<u16>; 3],
    /// Each ring's item in front (`0x80088AA2`, `0x80088BCA`, `0x80088B36`).
    pub currents: [i16; 3],
    pub radius: i16,
    pub camera_pitch: i16,
    pub rotating: bool,
    pub rot_count: i16,
    pub current: i16,
    pub target: i16,
    pub count: i16,
    pub angle_adder: i16,
    pub rot_adder: i16,
    pub rot_adder_l: i16,
    pub rot_adder_r: i16,
    /// The ring's turn (`ring+0x2A`).
    pub yaw: i16,
    pub camera_y: i32,
    pub camera_z: i32,
    pub motion: Motion,
    /// The camera tilt a swap to another ring starts from (a local of
    /// Display_Inventory).
    pitch_change: i32,
    /// The held input that stops repeats (`0x8008902C`).
    latch: u32,
    /// The last pass's raw and once-only input, for its state step, which
    /// the original runs after that pass's vsync wait.
    pending: Option<(u32, u32)>,
    pub chosen: Option<u16>,
    /// Sounds started this frame.
    pub sounds: Vec<u16>,
}

/// An item's pose state, by object.
pub type Poses = HashMap<u16, ItemPose>;

const OPEN_RADIUS: i16 = 688;
const ROTATE_STEPS: i16 = 24;

impl Ring {
    /// Builds the rings and opens one (Construct_Inventory `0x800325E8` and
    /// RingInit `0x800349E8`). `poses` keeps each item's state between
    /// openings; `keys_current` is the keys ring's remembered item.
    pub fn open(
        mode: RingMode,
        inventory: &Inventory,
        table: &ItemTable,
        poses: &mut Poses,
        keys_current: i16,
    ) -> Self {
        let main: Vec<u16> = inventory.main.iter().map(|s| s.item).collect();
        let keys: Vec<u16> = inventory.keys.iter().map(|s| s.item).collect();
        // The options ring's own order (0x80088BCC); the title adds Lara's Home.
        let options = vec![item::PASSPORT, item::SOUND, item::SCREEN_ADJUST, item::CONTROLS];
        for (i, &object) in main.iter().chain(&options).enumerate() {
            let data = table.get(object);
            let pose = poses.entry(object).or_insert_with(|| ItemPose {
                object,
                x_rot: data.map_or(0, |d| d.x_rot_closed),
                meshes: -1,
                ..default()
            });
            pose.frame = 0;
            pose.goal = 0;
            pose.count = 0;
            pose.y_rot = 0;
            if i < main.len() {
                pose.meshes = data.map_or(-1, |d| d.meshes);
            }
        }
        for &object in &keys {
            poses.entry(object).or_insert_with(|| ItemPose { object, meshes: -1, ..default() });
        }
        let (kind, current) = match mode {
            // Display_Inventory(2) passes the main ring's item, just reset to 0.
            RingMode::Keys => (RingKind::Keys, 0),
            RingMode::Game if !main.is_empty() => (RingKind::Main, 0),
            RingMode::Game => (RingKind::Options, 0),
        };
        let mut ring = Self {
            mode,
            kind,
            lists: [main, options, keys],
            currents: [0, 0, keys_current],
            radius: 0,
            camera_pitch: 0,
            rotating: false,
            rot_count: 0,
            current,
            target: 0,
            count: 0,
            angle_adder: 0,
            rot_adder: 0,
            rot_adder_l: 0,
            rot_adder_r: 0,
            yaw: 0,
            camera_y: -0x600,
            camera_z: 0x380,
            motion: Motion::default(),
            pitch_change: 0,
            latch: 0,
            pending: None,
            chosen: None,
            sounds: vec![],
        };
        ring.count = ring.list().len() as i16;
        ring.angle_adder = (0x10000 / i32::from(ring.count.max(1))) as i16;
        ring.motion = Motion { count: 32, status: Status::Opening, status_target: Status::Open, ..default() };
        ring.motion_radius(OPEN_RADIUS);
        ring.motion_camera_y(-0x100);
        ring.motion_rotation(i16::MIN, ring.front_yaw());
        ring.yaw = ring.motion.rotate_target.wrapping_add(i16::MIN);
        // The ring opens with a sound (0x80031144).
        ring.sounds.push(111);
        ring
    }

    pub fn list(&self) -> &[u16] {
        &self.lists[self.kind as usize]
    }

    /// The object of the item in front.
    pub fn current_item(&self) -> Option<u16> {
        self.list().get(self.current as usize).copied()
    }

    /// The ring's turn that brings the current item to the front.
    fn front_yaw(&self) -> i16 {
        (-0x4000i32 - i32::from(self.current) * i32::from(self.angle_adder)) as i16
    }

    fn motion_setup(&mut self, status: Status, target: Status, frames: i16) {
        let m = &mut self.motion;
        (m.count, m.status, m.status_target, m.radius_rate, m.camera_y_rate) = (frames, status, target, 0, 0);
    }
    fn motion_radius(&mut self, target: i16) {
        self.motion.radius_target = target;
        self.motion.radius_rate = ((i32::from(target) - i32::from(self.radius)) / i32::from(self.motion.count)) as i16;
    }
    fn motion_rotation(&mut self, delta: i16, target: i16) {
        self.motion.rotate_target = target;
        self.motion.rotate_rate = delta / self.motion.count;
    }
    fn motion_camera_y(&mut self, target: i16) {
        self.motion.camera_y_target = target;
        self.motion.camera_y_rate = ((i32::from(target) - self.camera_y) / i32::from(self.motion.count)) as i16;
    }
    fn motion_camera_pitch(&mut self, target: i16) {
        self.motion.camera_pitch_target = target;
        self.motion.camera_pitch_rate = target / self.motion.count;
    }
    /// MotionItemSelect `0x80035140`: towards the item's selected pose.
    fn motion_item_select(&mut self, table: &ItemTable, object: u16) {
        let Some(d) = table.get(object) else { return };
        let n = self.motion.count;
        let m = &mut self.motion;
        m.item_pt_xrot_target = d.pt_xrot;
        m.item_pt_xrot_rate = d.pt_xrot / n;
        m.item_x_rot_target = d.x_rot;
        m.item_x_rot_rate = ((i32::from(d.x_rot) - i32::from(d.x_rot_closed)) / i32::from(n)) as i16;
        m.item_y_target = d.y;
        m.item_y_rate = d.y / i32::from(n);
        m.item_z_target = d.z;
        m.item_z_rate = d.z / i32::from(n);
    }
    /// MotionItemDeselect `0x80035270`: back to its pose on the ring.
    fn motion_item_deselect(&mut self, table: &ItemTable, object: u16) {
        let Some(d) = table.get(object) else { return };
        let n = self.motion.count;
        let m = &mut self.motion;
        m.item_pt_xrot_target = 0;
        m.item_pt_xrot_rate = -(d.pt_xrot / n);
        m.item_x_rot_target = d.x_rot_closed;
        m.item_x_rot_rate = ((i32::from(d.x_rot_closed) - i32::from(d.x_rot)) / i32::from(n)) as i16;
        m.item_y_target = 0;
        m.item_y_rate = -(d.y / i32::from(n));
        m.item_z_target = 0;
        m.item_z_rate = -(d.z / i32::from(n));
    }

    /// One vsync of the ring's motion and turning (RingDoMotions
    /// `0x80034CD0`).
    fn do_motions(&mut self, poses: &mut Poses) {
        let object = self.current_item();
        if self.motion.count != 0 {
            let m = self.motion;
            self.radius = self.radius.wrapping_add(m.radius_rate);
            self.camera_y += i32::from(m.camera_y_rate);
            self.yaw = self.yaw.wrapping_add(m.rotate_rate);
            self.camera_pitch = self.camera_pitch.wrapping_add(m.camera_pitch_rate);
            let pose = object.and_then(|o| poses.get_mut(&o));
            if let Some(pose) = pose {
                pose.pt_xrot = pose.pt_xrot.wrapping_add(m.item_pt_xrot_rate);
                pose.x_rot = pose.x_rot.wrapping_add(m.item_x_rot_rate);
                pose.y += m.item_y_rate;
                pose.z += m.item_z_rate;
            }
            self.motion.count -= 1;
            if self.motion.count == 0 {
                let m = &mut self.motion;
                m.status = m.status_target;
                if m.radius_rate != 0 {
                    m.radius_rate = 0;
                    self.radius = m.radius_target;
                }
                if m.camera_y_rate != 0 {
                    m.camera_y_rate = 0;
                    self.camera_y = i32::from(m.camera_y_target);
                }
                if m.rotate_rate != 0 {
                    m.rotate_rate = 0;
                    self.yaw = m.rotate_target;
                }
                if let Some(pose) = object.and_then(|o| poses.get_mut(&o)) {
                    if m.item_pt_xrot_rate != 0 {
                        m.item_pt_xrot_rate = 0;
                        pose.pt_xrot = m.item_pt_xrot_target;
                    }
                    if m.item_x_rot_rate != 0 {
                        m.item_x_rot_rate = 0;
                        pose.x_rot = m.item_x_rot_target;
                    }
                    if m.item_y_rate != 0 {
                        m.item_y_rate = 0;
                        pose.y = m.item_y_target;
                    }
                    if m.item_z_rate != 0 {
                        m.item_z_rate = 0;
                        pose.z = m.item_z_target;
                    }
                }
                if m.camera_pitch_rate != 0 {
                    m.camera_pitch_rate = 0;
                    self.camera_pitch = m.camera_pitch_target;
                }
            }
        }
        if self.rotating {
            self.rot_count -= 1;
            self.yaw = self.yaw.wrapping_add(self.rot_adder);
            if self.rot_count == 0 {
                self.rotating = false;
                self.current = self.target;
                self.yaw = self.front_yaw();
            }
        }
    }

    fn rotate_left(&mut self) {
        self.rotating = true;
        self.target = if self.current == 0 { self.count - 1 } else { self.current - 1 };
        self.rot_count = ROTATE_STEPS;
        self.rot_adder = self.rot_adder_l;
    }
    fn rotate_right(&mut self) {
        self.rotating = true;
        self.target = if self.current + 1 >= self.count { 0 } else { self.current + 1 };
        self.rot_count = ROTATE_STEPS;
        self.rot_adder = self.rot_adder_r;
    }

    /// Shrink the ring away, turning half round, and finish (`0x800318F8`).
    fn close(&mut self) {
        self.motion_setup(Status::Closing, Status::Done, 32);
        self.motion_radius(0);
        self.motion_camera_y(-0x600);
        self.motion_rotation(i16::MIN, self.yaw.wrapping_add(i16::MIN));
    }

    /// Shrink away towards another ring, tilting the camera by `pitch`.
    fn leave_for(&mut self, target: Status, pitch: i16) {
        self.motion_setup(Status::Closing, target, 24);
        self.motion_radius(0);
        self.motion_rotation(i16::MIN, self.yaw.wrapping_add(i16::MIN));
        self.motion_camera_pitch(pitch);
        self.pitch_change = i32::from(pitch);
    }

    /// Open `kind` from the tilt the last ring left at (`0x80031E4C..0x800320B0`).
    fn arrive_at(&mut self, kind: RingKind) {
        self.motion_setup(Status::Opening, Status::Open, 24);
        self.motion_radius(OPEN_RADIUS);
        self.camera_pitch = (-self.pitch_change) as i16;
        self.motion.camera_pitch_target = 0;
        self.motion.camera_pitch_rate = (self.pitch_change / 24) as i16;
        self.currents[self.kind as usize] = self.current;
        self.kind = kind;
        self.count = self.list().len() as i16;
        self.current = self.currents[kind as usize];
        self.calc_adders();
        self.motion_rotation(i16::MIN, self.front_yaw());
        self.yaw = self.motion.rotate_target.wrapping_add(i16::MIN);
    }

    /// RingCalcAdders `0x80034C4C`.
    fn calc_adders(&mut self) {
        self.angle_adder = (0x10000 / i32::from(self.count.max(1))) as i16;
        self.rot_adder_l = self.angle_adder / ROTATE_STEPS;
        self.rot_adder_r = -(self.angle_adder / ROTATE_STEPS);
    }

    /// One frame of the ring: the state step of Display_Inventory's last
    /// pass (it runs after the pass's vsync wait, on that pass's input),
    /// then the next pass up to its wait: `input_word` is the word it reads
    /// and `vsyncs` the count the last frame took. Returns the outcome once
    /// the ring has closed.
    pub fn frame(&mut self, input_word: u32, vsyncs: u32, table: &ItemTable, poses: &mut Poses) -> Option<RingExit> {
        self.sounds.clear();
        if let Some((mut input, mut db)) = self.pending.take()
            && !self.rotating
        {
            self.run_status(&mut input, &mut db, vsyncs, table, poses);
        }
        self.calc_adders();
        // The repeat stop (0x80032F8C): a word counts once until all is released.
        let input = input_word;
        let db = if input == 0 {
            self.latch = 0;
            0
        } else if self.latch != 0 {
            0
        } else {
            self.latch = input;
            input
        };
        for _ in 0..vsyncs {
            self.do_motions(poses);
        }
        self.camera_z = i32::from(self.radius) + 598;
        self.spin_items(input, vsyncs, table, poses);
        self.pending = Some((input, db));
        (self.motion.status == Status::Done).then_some(RingExit { chosen: self.chosen })
    }

    /// The front item turns to its chosen angle while chosen and spins while
    /// the ring rests; the others turn back to rest (`0x800313F4..0x800315D8`).
    fn spin_items(&mut self, input: u32, vsyncs: u32, table: &ItemTable, poses: &mut Poses) {
        let showing = matches!(
            self.motion.status,
            Status::Selecting | Status::Selected | Status::Deselecting | Status::Deselect | Status::ClosingItem
        );
        for (i, object) in self.lists[self.kind as usize].iter().enumerate() {
            let Some(pose) = poses.get_mut(object) else { continue };
            let chosen_yaw = table.get(*object).map_or(0, |d| d.y_rot);
            for _ in 0..vsyncs {
                if i as i16 != self.current {
                    if pose.y_rot != 0 {
                        pose.y_rot = pose.y_rot.wrapping_add(if pose.y_rot < 0 { 0x100 } else { -0x100 });
                    }
                } else if self.rotating {
                    if pose.y_rot != 0 {
                        pose.y_rot = pose.y_rot.wrapping_add(if pose.y_rot < 0 { 0x200 } else { -0x200 });
                    }
                } else if showing {
                    if pose.y_rot != chosen_yaw {
                        // An unsigned compare of the whole difference: any
                        // angle below the current one is reached downwards.
                        let ahead = (i32::from(chosen_yaw) - i32::from(pose.y_rot) - 1) as u32;
                        let step: i16 = if ahead < 0x7FFF { 0x400 } else { -0x400 };
                        pose.y_rot = (pose.y_rot.wrapping_add(step) as u16 & 0xFC00) as i16;
                    }
                } else if self.count == 1 || input & (input::LEFT | input::RIGHT) == 0 {
                    pose.y_rot = pose.y_rot.wrapping_add(0x100);
                }
            }
        }
    }

    /// The ring's state machine (the jump table at `0x800103CC`).
    fn run_status(&mut self, input: &mut u32, db: &mut u32, vsyncs: u32, table: &ItemTable, poses: &mut Poses) {
        use input::*;
        let other_rings = self.mode == RingMode::Game;
        match self.motion.status {
            Status::Open => {
                if *input & RIGHT != 0 && self.count >= 2 {
                    self.rotate_left();
                    self.sounds.push(108);
                    return;
                }
                if *input & LEFT != 0 && self.count >= 2 {
                    self.rotate_right();
                    self.sounds.push(108);
                    return;
                }
                if *db & (DESELECT | OPTION) != 0 {
                    self.sounds.push(112);
                    self.chosen = None;
                    self.currents[self.kind as usize] = self.current;
                    self.close();
                    (*input, *db) = (0, 0);
                }
                if *db & SELECT != 0 {
                    self.currents[self.kind as usize] = self.current;
                    let object = self.current_item();
                    self.motion_setup(Status::Selecting, Status::Selected, 16);
                    if let Some(object) = object {
                        if let (Some(pose), Some(d)) = (poses.get_mut(&object), table.get(object)) {
                            pose.goal = d.open_frame;
                            pose.direction = 1;
                        }
                        self.motion_rotation(0, self.front_yaw());
                        self.motion_item_select(table, object);
                        self.sounds.push(match object {
                            item::LARAS_HOME => 109,
                            item::STATISTICS => 113,
                            item::PISTOLS..=item::GRENADE_LAUNCHER => 114,
                            _ => 111,
                        });
                    }
                    (*input, *db) = (0, 0);
                }
                if *db & FORWARD != 0 && other_rings {
                    match self.kind {
                        RingKind::Main if !self.lists[RingKind::Keys as usize].is_empty() => {
                            self.leave_for(Status::MainToKeys, 0x2000);
                        }
                        RingKind::Options if !self.lists[RingKind::Main as usize].is_empty() => {
                            self.leave_for(Status::OptionsToMain, 0x2000);
                        }
                        RingKind::Keys => return,
                        _ => {}
                    }
                    if self.kind == RingKind::Main {
                        *input = 0;
                    }
                    *db = 0;
                    return;
                }
                if *db & BACK != 0 && other_rings {
                    match self.kind {
                        RingKind::Keys if !self.lists[RingKind::Main as usize].is_empty() => {
                            self.leave_for(Status::KeysToMain, -0x2000);
                        }
                        RingKind::Main if !self.lists[RingKind::Options as usize].is_empty() => {
                            self.leave_for(Status::MainToOptions, -0x2000);
                        }
                        RingKind::Options => return,
                        _ => {}
                    }
                    if self.kind == RingKind::Keys {
                        *input = 0;
                    }
                    *db = 0;
                }
            }
            Status::MainToOptions => self.arrive_at(RingKind::Options),
            Status::MainToKeys => self.arrive_at(RingKind::Keys),
            Status::KeysToMain | Status::OptionsToMain => self.arrive_at(RingKind::Main),
            Status::Selected => {
                let Some(object) = self.current_item() else { return };
                let chosen_yaw = table.get(object).map_or(0, |d| d.y_rot);
                if let Some(pose) = poses.get_mut(&object) {
                    if pose.object == item::PASSPORT {
                        pose.object = item::PASSPORT_OPEN;
                    }
                    let mut busy = false;
                    for _ in 0..vsyncs {
                        busy = pose.y_rot == chosen_yaw && animate(pose, table.get(object));
                    }
                    if busy {
                        return;
                    }
                    item_options(pose, table.get(object), db, &mut self.sounds);
                }
                if *db & DESELECT != 0 {
                    self.motion_setup(Status::ClosingItem, Status::Deselect, 0);
                    (*input, *db) = (0, 0);
                }
                if *db & SELECT != 0 {
                    self.chosen = Some(object);
                    self.currents[self.kind as usize] = self.current;
                    self.motion_setup(Status::ClosingItem, Status::Exiting, 0);
                    (*input, *db) = (0, 0);
                }
            }
            Status::ClosingItem => {
                let Some(object) = self.current_item() else { return };
                let data = table.get(object);
                for _ in 0..vsyncs {
                    let Some(pose) = poses.get_mut(&object) else { return };
                    if animate(pose, data) {
                        continue;
                    }
                    if pose.object == item::PASSPORT_OPEN {
                        pose.object = item::PASSPORT;
                        pose.frame = 0;
                    }
                    self.motion.count = 16;
                    self.motion.status = self.motion.status_target;
                    self.motion_item_deselect(table, object);
                    return;
                }
            }
            Status::Deselect => {
                self.sounds.push(112);
                self.motion_setup(Status::Deselecting, Status::Open, 16);
                self.motion_rotation(0, self.front_yaw());
                (*input, *db) = (0, 0);
            }
            Status::Exiting => {
                if self.motion.count == 0 {
                    self.close();
                }
            }
            Status::Opening | Status::Closing | Status::Selecting | Status::Deselecting | Status::Done => {}
        }
    }
}

/// One vsync of an item's opening or closing animation (AnimateInventoryItem
/// `0x80032838`): a frame every other call, wrapping, until it reaches its
/// goal; then its joints for that frame (`0x80032784`). True while moving.
fn animate(pose: &mut ItemPose, data: Option<&super::ItemData>) -> bool {
    let frames = data.map_or(1, |d| d.frames);
    if pose.frame == pose.goal {
        pose.meshes = meshes_for(pose, data);
        return false;
    }
    if pose.count == 0 {
        pose.frame += pose.direction;
        pose.count = 1;
        if pose.frame >= frames {
            pose.frame = 0;
        } else if pose.frame < 0 {
            pose.frame = frames - 1;
        }
    } else {
        pose.count -= 1;
    }
    pose.meshes = meshes_for(pose, data);
    true
}

/// The joints the passport and stopwatch show at a frame (`0x80032784`).
fn meshes_for(pose: &ItemPose, data: Option<&super::ItemData>) -> i32 {
    match pose.object {
        item::PASSPORT_OPEN => match pose.frame {
            ..=3 => 0x17,
            4..=16 => 0x57,
            17..=18 => 0x5F,
            19 => 0x5B,
            20..=23 => 0x7B,
            24..=28 => 0x3B,
            29 => 0x13,
            _ => pose.meshes,
        },
        item::STATISTICS if pose.frame == 0 || pose.frame > 0x11 => data.map_or(-1, |d| d.meshes),
        _ => -1,
    }
}

/// An item's own response while chosen (DoInventoryOptions `0x8006EA54`,
/// dispatching on the item's current model through `0x80010E28`): what
/// Lara uses or holds confirms at once; ammunition does nothing; the open
/// passport turns its pages and closes on Triangle ([`passport_options`]);
/// anything else closes again on Cross or Triangle. The statistics and the
/// option pages are not ported; they wait for Triangle.
fn item_options(pose: &mut ItemPose, data: Option<&super::ItemData>, db: &mut u32, sounds: &mut Vec<u16>) {
    match pose.object {
        item::PASSPORT_OPEN => passport_options(pose, data, db, sounds),
        item::STATISTICS | item::SCREEN_ADJUST | item::SOUND | item::CONTROLS => {}
        item::PISTOLS..=item::GRENADE_LAUNCHER
        | item::SMALL_MEDIPACK
        | item::LARGE_MEDIPACK
        | 178..=181
        | 197..=200
        | 207
        | 208 => *db |= input::SELECT,
        164..=170 => {}
        _ => {
            if *db & (input::SELECT | input::DESELECT) != 0 {
                pose.goal = 0;
                pose.direction = -1;
            }
        }
    }
}

/// The open passport (`0x8006EB58`): its pages are five frames apart from
/// the opening frame (load, save, exit). Left and Right turn a page (sound
/// 115) within them; Triangle closes it, backwards to the first frame or,
/// from the last page, on to the last. Choosing a page with Cross (the
/// load and save requesters, leaving the game) is not ported, nor the
/// turn past the load page when there are no saves.
fn passport_options(pose: &mut ItemPose, data: Option<&super::ItemData>, db: &mut u32, sounds: &mut Vec<u16>) {
    const PAGE: i16 = 5;
    const TURN_SOUND: u16 = 115;
    let Some(data) = data else { return };
    // Between pages (still turning) is no page.
    let from_open = pose.goal - data.open_frame;
    let page = if from_open % PAGE == 0 { from_open / PAGE } else { -1 };
    if *db & input::LEFT != 0 {
        pose.direction = -1;
        if pose.goal - PAGE < data.open_frame {
            pose.goal = data.open_frame;
        } else {
            pose.goal -= PAGE;
            sounds.push(TURN_SOUND);
        }
        *db = 0;
    }
    if *db & input::RIGHT != 0 {
        pose.direction = 1;
        if pose.goal + PAGE > data.frames - 6 {
            pose.goal = data.frames - 6;
        } else {
            pose.goal += PAGE;
            sounds.push(TURN_SOUND);
        }
        *db = 0;
    }
    if *db & input::DESELECT != 0 {
        if page == 2 {
            pose.direction = 1;
            pose.goal = data.frames - 1;
        } else {
            pose.goal = 0;
            pose.direction = -1;
        }
    }
}
