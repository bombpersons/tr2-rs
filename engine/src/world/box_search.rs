//! Path search over the level's navigation boxes, as a creature carries it:
//! a reverse breadth-first search from a target box, expanded a few boxes at
//! a time, and the steering target read back along its exit links.
//!
//! Everything here is integer world units on the level's own tables: `boxes`
//! (floor height, extent in sectors, overlap list index and flags),
//! `overlaps` and the per-box movement `zones`. Which zone tables a creature
//! uses, and its step, drop and fly limits, are the caller's choice.
use super::Level;
use bevy::prelude::*;

/// "No box": an empty link, queue end or unset target.
pub const NO_BOX: i16 = -1;
/// Overlap-list entries end with this bit set.
const LAST_OVERLAP: u16 = 0x8000;
/// A node reached only through a blocked box carries this search-number bit.
const BLOCKED_SEARCH: u16 = 0x8000;

/// Overlap-word bit: the box can be blocked at run time (a door's or a
/// block's box).
pub const BLOCKABLE: u16 = 0x8000;
/// Overlap-word bit: the box is blocked now; creatures' searches with this
/// bit in their block mask treat it as closed.
pub const BLOCKED: u16 = 0x4000;

/// The navigation boxes' overlap words as they stand now: the level's, with
/// the blocked bit changed at run time by shut doors and placed blocks (the
/// original edits its loaded box table). Reset when a level loads.
#[derive(Resource, Clone, Debug, Default)]
pub struct BoxFlags {
    overlap: Vec<u16>,
}

impl BoxFlags {
    pub fn new(level: &Level) -> Self {
        Self { overlap: level.boxes.iter().map(|b| b.overlap as u16).collect() }
    }

    /// The overlap word of box `b` (its overlap-list index and flags).
    pub fn overlap(&self, level: &Level, b: usize) -> u16 {
        self.overlap.get(b).copied().unwrap_or_else(|| level.boxes.get(b).map_or(0, |b| b.overlap as u16))
    }

    /// Block or unblock box `b`, if it is blockable. Returns whether it is.
    pub fn set_blocked(&mut self, b: usize, blocked: bool) -> bool {
        let Some(word) = self.overlap.get_mut(b) else { return false };
        if *word & BLOCKABLE == 0 {
            return false;
        }
        if blocked {
            *word |= BLOCKED;
        } else {
            *word &= !BLOCKED;
        }
        true
    }
}

/// Keeps [`BoxFlags`] in step with the loaded level.
pub struct BoxFlagsPlugin;

impl Plugin for BoxFlagsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoxFlags>().add_systems(
            FixedUpdate,
            reset_box_flags.in_set(crate::schedule::LifecycleSystems::Reset).in_set(crate::shell::GameSystems::Game),
        );
    }
}

/// A newly loaded level starts from its own box table.
fn reset_box_flags(world: Option<Res<super::WorldGeometry>>, mut flags: ResMut<BoxFlags>) {
    if let Some(world) = world.filter(|w| w.is_changed()) {
        *flags = BoxFlags::new(&world.data);
    }
}

/// One box's entry in a search: the original's eight-byte node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct SearchNode {
    /// The next box toward the search's target box.
    pub exit_box: i16,
    pub search_number: u16,
    /// Queue link to the next box to expand.
    pub next_expansion: i16,
    /// The `zone_count` boxes of this creature's zone, listed from node 0.
    pub box_number: i16,
}

impl Default for SearchNode {
    fn default() -> Self {
        Self { exit_box: NO_BOX, search_number: 0, next_expansion: NO_BOX, box_number: 0 }
    }
}

/// How [`BoxSearch::calculate_target`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    /// The path ran out (no search result yet, or a blocked box).
    None,
    /// The target box is reached along open boxes: steer at the search target.
    Direct,
    /// A corner of the corridor: steer there first.
    Secondary,
}

/// A creature's search state over the navigation boxes (the original's LOT).
#[derive(Clone, Debug, Reflect)]
pub struct BoxSearch {
    pub nodes: Vec<SearchNode>,
    pub head: i16,
    pub tail: i16,
    pub search_number: u16,
    /// Boxes whose overlap word has any of these bits are blocked.
    pub block_mask: u16,
    /// Largest rise and drop (negative) between neighbouring boxes.
    pub step: i32,
    pub drop: i32,
    /// Flying creatures' height above a box floor; 0 walks.
    pub fly: i32,
    pub zone_count: usize,
    pub target_box: i16,
    pub required_box: i16,
    /// The point aimed at inside the target box.
    pub target: [i32; 3],
    /// Indices of the level zone tables used in normal and flipped rooms.
    pub zones: [usize; 2],
}

impl BoxSearch {
    /// A cleared search over `boxes` boxes.
    pub fn new(boxes: usize, step: i32, drop: i32, fly: i32, zones: [usize; 2]) -> Self {
        let mut search = Self {
            nodes: vec![SearchNode::default(); boxes],
            head: NO_BOX,
            tail: NO_BOX,
            search_number: 0,
            block_mask: 0x4000,
            step,
            drop,
            fly,
            zone_count: 0,
            target_box: NO_BOX,
            required_box: NO_BOX,
            target: [0; 3],
            zones,
        };
        search.clear();
        search
    }

    /// ClearLOT `0x80049250`: an empty queue and every node unvisited.
    /// The zone list (`box_number`) is kept.
    pub fn clear(&mut self) {
        self.head = NO_BOX;
        self.tail = NO_BOX;
        self.search_number = 0;
        self.target_box = NO_BOX;
        self.required_box = NO_BOX;
        for node in &mut self.nodes {
            node.exit_box = NO_BOX;
            node.next_expansion = NO_BOX;
            node.search_number = 0;
        }
    }

    /// The movement zone of every box, for the current flip state.
    pub fn zone<'a>(&self, level: &'a Level, flipped: bool) -> &'a [i16] {
        level.zones.get(self.zones[usize::from(flipped)]).map_or(&[], Vec::as_slice)
    }

    /// CreateZone `0x800490EC`: list the boxes sharing `start`'s zone in
    /// either flip state, for random box choices.
    pub fn create_zone(&mut self, level: &Level, start: i16) {
        let (normal, flipped) = (self.zone(level, false), self.zone(level, true));
        let index = start as usize;
        let (zone, flip_zone) = (normal.get(index).copied(), flipped.get(index).copied());
        self.zone_count = 0;
        for i in 0..level.boxes.len() {
            if normal.get(i).copied() == zone || flipped.get(i).copied() == flip_zone {
                self.nodes[self.zone_count].box_number = i as i16;
                self.zone_count += 1;
            }
        }
    }

    fn node(&mut self, index: i16) -> &mut SearchNode {
        &mut self.nodes[index as usize]
    }

    /// SearchLOT `0x800150D8`: expand up to `expansion` queued boxes, from
    /// the target back over overlaps of the same zone whose floor change
    /// fits `step` and `drop`. Returns false once the queue is empty.
    pub fn search(&mut self, level: &Level, flags: &BoxFlags, flipped: bool, expansion: usize) -> bool {
        let zone = self.zone(level, flipped);
        let Some(&search_zone) = zone.get(self.head.max(0) as usize) else {
            // No zone data: nothing can be searched.
            self.head = NO_BOX;
            self.tail = NO_BOX;
            return false;
        };
        for _ in 0..expansion {
            if self.head == NO_BOX {
                self.tail = NO_BOX;
                return false;
            }
            let head = self.head;
            let current = level.boxes[head as usize];
            let current_search = self.nodes[head as usize].search_number;
            let mut index = (current.overlap & 0x3fff) as usize;
            loop {
                let entry = level.overlaps.get(index).copied().unwrap_or(LAST_OVERLAP);
                index += 1;
                let done = entry & LAST_OVERLAP != 0;
                let box_number = (entry & 0x7fff) as i16;
                if zone.get(box_number as usize) == Some(&search_zone) {
                    let next = level.boxes[box_number as usize];
                    let change = next.floor - current.floor;
                    if change <= self.step && change >= self.drop {
                        let overlap = flags.overlap(level, box_number as usize);
                        self.expand(box_number, head, current_search, overlap);
                    }
                }
                if done {
                    break;
                }
            }
            let next = self.nodes[head as usize].next_expansion;
            self.head = next;
            self.node(head).next_expansion = NO_BOX;
        }
        true
    }

    /// One overlap of the box being expanded (`0x80015238..0x800152DC`).
    fn expand(&mut self, box_number: i16, from: i16, search: u16, overlap: u16) {
        let (number, blocked) = (search & !BLOCKED_SEARCH, search & BLOCKED_SEARCH != 0);
        let node = &mut self.nodes[box_number as usize];
        let node_number = node.search_number & !BLOCKED_SEARCH;
        if number < node_number {
            return;
        }
        if blocked {
            if number == node_number {
                return;
            }
            node.search_number = search;
        } else {
            if number == node_number && node.search_number & BLOCKED_SEARCH == 0 {
                return;
            }
            if overlap & self.block_mask != 0 {
                node.search_number = search | BLOCKED_SEARCH;
            } else {
                node.search_number = search;
                node.exit_box = from;
            }
        }
        if node.next_expansion == NO_BOX && box_number != self.tail {
            // The queue is never empty here: the box being expanded is in it.
            let tail = self.tail;
            if tail != NO_BOX {
                self.node(tail).next_expansion = box_number;
            }
            self.tail = box_number;
        }
    }

    /// UpdateLOT `0x80015320`: when a new box is required, restart the search
    /// from it with a new search number; then expand up to `expansion` boxes.
    pub fn update(&mut self, level: &Level, flags: &BoxFlags, flipped: bool, expansion: usize) -> bool {
        if self.required_box != NO_BOX && self.required_box != self.target_box {
            self.target_box = self.required_box;
            let target = self.target_box;
            if self.nodes[target as usize].next_expansion == NO_BOX && self.tail != target {
                let head = self.head;
                self.node(target).next_expansion = head;
                if self.head == NO_BOX {
                    self.tail = target;
                }
                self.head = target;
            }
            self.search_number = self.search_number.wrapping_add(1);
            let number = self.search_number;
            let node = self.node(target);
            node.search_number = number;
            node.exit_box = NO_BOX;
        }
        self.search(level, flags, flipped, expansion)
    }

    /// TargetBox `0x800153E0`: require `box` and aim at a point in it. Its
    /// random spread is `((extent - 1) >> 5)` units per random step, so in
    /// ordinary boxes the point is half a sector in from the low corner.
    pub fn target_box(&mut self, level: &Level, box_number: i16, random: &mut impl FnMut() -> i32) {
        let box_number = box_number & 0x7fff;
        let b = level.boxes[box_number as usize];
        let spread = |min: i32, max: i32| ((max - min - 1) as u32 >> 5) as i32;
        self.target[2] = (b.z_min << 10) + random().wrapping_mul(spread(b.z_min, b.z_max)) + 512;
        self.target[0] = (b.x_min << 10) + random().wrapping_mul(spread(b.x_min, b.x_max)) + 512;
        self.required_box = box_number;
        self.target[1] = if self.fly == 0 { b.floor } else { b.floor - 384 };
    }

    /// CalculateTarget `0x80015DD4`: expand the search five boxes, then
    /// follow exit links from `item_box`, narrowing the corridor through the
    /// boxes passed and clipping the steering target into it.
    pub fn calculate_target(
        &mut self,
        level: &Level,
        flags: &BoxFlags,
        flipped: bool,
        position: [i32; 3],
        item_box: i16,
        random: &mut impl FnMut() -> i32,
    ) -> ([i32; 3], TargetKind) {
        self.update(level, flags, flipped, 5);
        let [x, _, z] = position;
        let mut target = position;
        if item_box == NO_BOX {
            return (target, TargetKind::None);
        }
        const LEFT: i32 = 1;
        const RIGHT: i32 = 2;
        const TOP: i32 = 4;
        const BOTTOM: i32 = 8;
        const SECONDARY: i32 = 16;
        const ALL: i32 = 15;
        let (mut left, mut right, mut top, mut bottom) = (0, 0, 0, 0);
        let mut free = ALL;
        let mut box_number = item_box;
        let secondary = |target| (target, TargetKind::Secondary);
        let mut b;
        loop {
            b = level.boxes[box_number as usize];
            let height = if self.fly == 0 { b.floor } else { b.floor - 1024 };
            if height < target[1] {
                target[1] = height;
            }
            let (box_left, box_right) = (b.z_min << 10, (b.z_max << 10) - 1);
            let (box_top, box_bottom) = (b.x_min << 10, (b.x_max << 10) - 1);
            let inside_z = z >= box_left && z <= box_right;
            let inside_x = x >= box_top && x <= box_bottom;
            if inside_z && inside_x {
                (left, right, top, bottom) = (box_left, box_right, box_top, box_bottom);
            } else {
                // Clip on z (0x80015F2C..0x80016020).
                if z < box_left {
                    if free & LEFT != 0 && inside_x {
                        if target[2] < box_left + 512 {
                            target[2] = box_left + 512;
                        }
                        if free & SECONDARY != 0 {
                            return secondary(target);
                        }
                        top = top.max(box_top);
                        bottom = bottom.min(box_bottom);
                        free = LEFT;
                    } else if free != LEFT {
                        target[2] = right - 512;
                        if free != ALL {
                            return secondary(target);
                        }
                        free |= SECONDARY;
                    }
                } else if z > box_right {
                    if free & RIGHT != 0 && inside_x {
                        if target[2] > box_right - 512 {
                            target[2] = box_right - 512;
                        }
                        if free & SECONDARY != 0 {
                            return secondary(target);
                        }
                        top = top.max(box_top);
                        bottom = bottom.min(box_bottom);
                        free = RIGHT;
                    } else if free != RIGHT {
                        target[2] = left + 512;
                        if free != ALL {
                            return secondary(target);
                        }
                        free |= SECONDARY;
                    }
                }
                // Clip on x (0x80016024..0x8001612C).
                if x < box_top {
                    if free & TOP != 0 && inside_z {
                        if target[0] < box_top + 512 {
                            target[0] = box_top + 512;
                        }
                        if free & SECONDARY != 0 {
                            return secondary(target);
                        }
                        left = left.max(box_left);
                        right = right.min(box_right);
                        free = TOP;
                    } else if free != TOP {
                        target[0] = bottom - 512;
                        if free != ALL {
                            return secondary(target);
                        }
                        free |= SECONDARY;
                    }
                } else if x > box_bottom {
                    if free & BOTTOM != 0 && inside_z {
                        if target[0] > box_bottom - 512 {
                            target[0] = box_bottom - 512;
                        }
                        if free & SECONDARY != 0 {
                            return secondary(target);
                        }
                        left = left.max(box_left);
                        right = right.min(box_right);
                        free = BOTTOM;
                    } else if free != BOTTOM {
                        target[0] = top + 512;
                        if free != ALL {
                            return secondary(target);
                        }
                        free |= SECONDARY;
                    }
                }
            }
            let clamp = |value: i32, min: i32, max: i32| {
                if value < min + 512 {
                    min + 512
                } else if value > max - 512 {
                    max - 512
                } else {
                    value
                }
            };
            if box_number == self.target_box {
                if free & (LEFT | RIGHT) != 0 {
                    target[2] = self.target[2];
                } else if free & SECONDARY == 0 {
                    target[2] = clamp(target[2], box_left, box_right);
                }
                if free & (TOP | BOTTOM) != 0 {
                    target[0] = self.target[0];
                } else if free & SECONDARY == 0 {
                    target[0] = clamp(target[0], box_top, box_bottom);
                }
                target[1] = self.target[1];
                return (target, TargetKind::Direct);
            }
            box_number = self.nodes[box_number as usize].exit_box;
            if box_number == NO_BOX || flags.overlap(level, box_number as usize) & self.block_mask != 0 {
                // The path ends here (0x80016218).
                if free & (LEFT | RIGHT) != 0 {
                    let spread = random().wrapping_mul(box_right - box_left - 1024) >> 15;
                    target[2] = box_left + 512 + spread;
                } else if free & SECONDARY == 0 {
                    target[2] = clamp(target[2], box_left, box_right);
                }
                if free & (TOP | BOTTOM) != 0 {
                    let spread = random().wrapping_mul(box_bottom - box_top - 1024) >> 15;
                    target[0] = box_top + 512 + spread;
                } else if free & SECONDARY == 0 {
                    target[0] = clamp(target[0], box_top, box_bottom);
                }
                target[1] = if self.fly == 0 { b.floor } else { b.floor - 384 };
                return (target, TargetKind::None);
            }
        }
    }
}
