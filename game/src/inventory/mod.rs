//! What Lara carries and the ring she picks it from.
//!
//! [`Inventory`] holds the original's two item lists, the main ring and the
//! keys ring, each item with a count, plus the ammunition of guns Lara has.
//! Adding, removing and counting follow Inv_AddItem `0x80033A70`,
//! Inv_RemoveItem `0x8003462C` and Inv_RequestItem `0x80034530`; the items
//! themselves (their ring order, pose when selected and name) are
//! `data/inventory.json` ([`ItemTable`]). [`ring`] is the ring's own logic
//! and [`view`] draws it. Evidence: `docs/inventory-evidence.md`.
pub mod ring;
pub mod view;

use bevy::prelude::*;
use engine::{
    audio::SoundRequest,
    controller::{Button, ControllerState},
    schedule::{SimulationEnabled, WorldSystems},
    shell::GameSystems,
};
use serde::{Deserialize, Serialize};

use crate::player::{
    Player, PlayerMode,
    weapons::{Gun, WeaponState},
};
pub use ring::{Ring, RingExit, RingMode};

/// The inventory's rules: what Lara carries, the ring opening on Select (or
/// for a keyhole), its frames while the world waits, and using what it
/// returns. Renderer-free; [`view::InventoryViewPlugin`] draws it.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<SimulationEnabled>() {
            app.insert_resource(SimulationEnabled(true));
        }
        app.add_message::<SoundRequest>()
            .init_resource::<Inventory>()
            .init_resource::<ItemTable>()
            .init_resource::<GameStrings>()
            .init_resource::<InventoryRing>()
            .configure_sets(FixedUpdate, InventorySystems::Ring.in_set(WorldSystems::Prepare))
            .add_systems(FixedUpdate, run_ring.in_set(InventorySystems::Ring).in_set(GameSystems::Game));
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum InventorySystems {
    /// Opens the ring and runs its frame, at the start of a tick: while it
    /// is open the world's rules wait ([`SimulationEnabled`]).
    Ring,
}

/// The ring while it is open, and what Display_Inventory keeps between
/// openings.
#[derive(Resource, Default)]
pub struct InventoryRing {
    pub ring: Option<Ring>,
    /// Each item's state on the ring, kept between openings.
    pub poses: ring::Poses,
    /// An opening asked for during the last tick (a keyhole's).
    pub request: Option<RingMode>,
    /// The item the last ring closed on (`0x80089032`), which a keyhole
    /// tests and clears.
    pub chosen: Option<u16>,
    /// The keys ring's item in front (`0x80088B36`), kept between openings.
    pub keys_current: i16,
}

impl InventoryRing {
    pub fn is_open(&self) -> bool {
        self.ring.is_some()
    }
}

/// The ring's input word for a tick (`0x8008AAD4`): directions while held;
/// Cross, Triangle and Select on the tick they are pressed; Action and Draw
/// (Cross and Triangle held), and any other button, while held, so a held
/// button keeps the ring's repeat stop.
pub fn input_word(pad: &ControllerState) -> u32 {
    use ring::input::*;
    let mut word = 0;
    for (button, bit) in [(Button::Up, FORWARD), (Button::Down, BACK), (Button::Left, LEFT), (Button::Right, RIGHT)] {
        if pad.held(button) {
            word |= bit;
        }
    }
    for (button, bit) in [(Button::Cross, SELECT), (Button::Triangle, DESELECT), (Button::Select, OPTION)] {
        if pad.pressed(button) {
            word |= bit;
        }
    }
    if pad.held(Button::Cross) {
        word |= 0x40;
    }
    if pad.held(Button::Triangle) {
        word |= 0x20;
    }
    let others = [Button::Circle, Button::Square, Button::L1, Button::R1, Button::L2, Button::R2, Button::Start];
    if others.into_iter().any(|b| pad.held(b)) {
        word |= 0x1000;
    }
    word
}

/// Opens the ring on Select in play (the main loop's check at `0x8001DAC8`,
/// not while Lara dies) or for a keyhole's request, runs a frame of it each
/// tick after that, and when it closes uses what it chose. The world waits
/// from the tick it opens to the tick it closes, which then runs in full:
/// the original's tick resumes after Display_Inventory returns. The ring
/// does not open while something else (the pause menu) holds the world.
#[allow(clippy::too_many_arguments)]
fn run_ring(
    pad: Res<ControllerState>,
    mut state: ResMut<InventoryRing>,
    mut inventory: ResMut<Inventory>,
    table: Res<ItemTable>,
    mut enabled: ResMut<SimulationEnabled>,
    mut sounds: MessageWriter<SoundRequest>,
    mut lara: Query<(&mut Player, &mut WeaponState)>,
) {
    let state = &mut *state;
    if let Some(ring) = state.ring.as_mut() {
        let exit = ring.frame(input_word(&pad), 2, &table, &mut state.poses);
        sounds.write_batch(ring.sounds.iter().map(|&s| SoundRequest(usize::from(s))));
        if let Some(exit) = exit {
            let mode = ring.mode;
            state.keys_current = ring.currents[ring::RingKind::Keys as usize];
            state.ring = None;
            state.chosen = exit.chosen;
            enabled.0 = true;
            if mode == RingMode::Game
                && let (Some(item), Ok((mut player, mut weapons))) = (exit.chosen, lara.single_mut())
            {
                use_item(item, &mut inventory, &table, &mut player, &mut weapons, &mut sounds);
            }
        }
    } else if enabled.0 {
        let alive = lara.single().is_ok_and(|(p, _)| p.health > 0 && p.mode != PlayerMode::Dead);
        let mode = state.request.take().or((pad.pressed(Button::Select) && alive).then_some(RingMode::Game));
        if let Some(mode) = mode {
            // Display_Inventory(2) with no keys returns at once, choosing nothing.
            if mode == RingMode::Keys && inventory.keys.is_empty() {
                state.chosen = None;
            } else {
                state.ring = Some(Ring::open(mode, &inventory, &table, &mut state.poses, state.keys_current));
                enabled.0 = false;
            }
        }
    }
}

/// What choosing an item in play does (UseItem `0x800432D4`): a gun is
/// asked for, a medi pack heals when Lara is hurt but alive. The other guns
/// and flares are not ported yet and do nothing.
fn use_item(
    item: u16,
    inventory: &mut Inventory,
    table: &ItemTable,
    player: &mut Player,
    weapons: &mut WeaponState,
    sounds: &mut MessageWriter<SoundRequest>,
) {
    match item {
        item::PISTOLS => weapons.requested = Gun::Pistols,
        item::SMALL_MEDIPACK | item::LARGE_MEDIPACK if inventory.use_medipack(table, item, player) => {
            sounds.write(SoundRequest(116));
        }
        _ => {}
    }
}

/// One item of a ring: its display object and how many Lara has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct Slot {
    pub item: u16,
    pub count: u16,
}

/// Rounds for the guns Lara has (`0x8008C5AC..0x8008C5C0`). The pistols
/// never run out. A gun's ammunition picked up before the gun stays an item
/// on the ring until the gun converts it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct Ammo {
    pub autos: i32,
    pub uzis: i32,
    pub shotgun: i32,
    pub harpoon: i32,
    pub grenades: i32,
    pub m16: i32,
}

/// What Lara carries: the main ring (weapons, ammunition, medi packs,
/// flares, statistics) and the keys ring (keys, puzzle pieces, pickups),
/// each in ring order.
#[derive(Resource, Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct Inventory {
    pub main: Vec<Slot>,
    pub keys: Vec<Slot>,
    pub ammo: Ammo,
    /// Secrets found in this level, a bit each (`0x800DE682`).
    pub secrets: u8,
}

/// Item objects the rules name.
pub mod item {
    pub const STATISTICS: u16 = 121;
    pub const PASSPORT: u16 = 133;
    /// The passport while it is open on the ring.
    pub const PASSPORT_OPEN: u16 = 120;
    pub const LARAS_HOME: u16 = 122;
    pub const PISTOLS: u16 = 157;
    pub const SHOTGUN: u16 = 158;
    pub const AUTOS: u16 = 159;
    pub const UZIS: u16 = 160;
    pub const HARPOON: u16 = 161;
    pub const M16: u16 = 162;
    pub const GRENADE_LAUNCHER: u16 = 163;
    pub const SHOTGUN_AMMO: u16 = 165;
    pub const AUTOS_AMMO: u16 = 166;
    pub const UZI_AMMO: u16 = 167;
    pub const HARPOONS: u16 = 168;
    pub const M16_AMMO: u16 = 169;
    pub const GRENADES: u16 = 170;
    pub const SMALL_MEDIPACK: u16 = 171;
    pub const LARGE_MEDIPACK: u16 = 172;
    pub const FLARES: u16 = 173;
    pub const SCREEN_ADJUST: u16 = 153;
    pub const SOUND: u16 = 154;
    pub const CONTROLS: u16 = 155;
}

/// The pickup of a box of flares, which holds six (`0x80033A70`).
const FLARE_BOX: u16 = 151;

impl Inventory {
    /// The ring order an item takes (`item+0x3C`): below 100 the main ring.
    fn ring_of(table: &ItemTable, item: u16) -> bool {
        table.get(item).is_some_and(|i| i.position < 100)
    }

    fn find(&self, item: u16) -> Option<(bool, usize)> {
        let index = |list: &[Slot]| list.iter().position(|s| s.item == item);
        index(&self.main).map(|i| (true, i)).or_else(|| index(&self.keys).map(|i| (false, i)))
    }

    /// How many of an item (or of the item a pickup becomes) Lara has
    /// (Inv_RequestItem `0x80034530`).
    pub fn count(&self, table: &ItemTable, kind: u16) -> u16 {
        let Some(item) = table.item_for(kind) else { return 0 };
        match self.find(item) {
            Some((true, i)) => self.main[i].count,
            Some((false, i)) => self.keys[i].count,
            None => 0,
        }
    }

    /// Takes one away, dropping the item from its ring at none (Inv_RemoveItem
    /// `0x8003462C`). False if Lara has none.
    pub fn remove(&mut self, table: &ItemTable, kind: u16) -> bool {
        let Some(item) = table.item_for(kind) else { return false };
        let Some((main, i)) = self.find(item) else { return false };
        let list = if main { &mut self.main } else { &mut self.keys };
        list[i].count -= 1;
        if list[i].count == 0 {
            list.remove(i);
        }
        true
    }

    /// Puts a new item on its ring after those of the same or earlier ring
    /// order, with one of it (Inv_InsertItem `0x80034284`).
    fn insert(&mut self, table: &ItemTable, item: u16) {
        let position = |item: u16| table.get(item).map_or(i16::MAX, |i| i.position);
        let list = if Self::ring_of(table, item) { &mut self.main } else { &mut self.keys };
        let at = list.iter().position(|s| position(item) < position(s.item)).unwrap_or(list.len());
        list.insert(at, Slot { item, count: 1 });
    }

    /// Ammunition for a gun Lara gets: rounds `per_box` for each box she
    /// already carried of it, and for the one in the gun.
    fn take_ammo(&mut self, table: &ItemTable, ammo_item: u16, per_box: i32) -> i32 {
        let mut rounds = per_box;
        for _ in 0..self.count(table, ammo_item) {
            self.remove(table, ammo_item);
            rounds += per_box;
        }
        rounds
    }

    /// Adds a picked-up object or a ring item (Inv_AddItem `0x80033A70`).
    /// More of something Lara has adds to its count (six for a box of
    /// flares). A new gun takes in the boxes of its ammunition she
    /// carried; ammunition for a gun she has goes into the gun. Returns
    /// whether the object is something the inventory holds.
    pub fn add(&mut self, table: &ItemTable, kind: u16) -> bool {
        use item::*;
        if let Some(item) = table.item_for(kind)
            && let Some((main, i)) = self.find(item)
        {
            let slot = if main { &mut self.main[i] } else { &mut self.keys[i] };
            slot.count += if main && kind == FLARE_BOX { 6 } else { 1 };
            return true;
        }
        let has = |inv: &Self, gun: u16| inv.count(table, gun) > 0;
        match kind {
            121 | 134 => self.insert(table, STATISTICS),
            135 | 157 => self.insert(table, PISTOLS),
            136 | 158 => {
                self.ammo.shotgun += self.take_ammo(table, SHOTGUN_AMMO, 12);
                self.insert(table, SHOTGUN);
            }
            137 | 159 => {
                self.ammo.autos += self.take_ammo(table, AUTOS_AMMO, 40);
                self.insert(table, AUTOS);
            }
            138 | 160 => {
                self.ammo.uzis += self.take_ammo(table, UZI_AMMO, 80);
                self.insert(table, UZIS);
            }
            139 | 161 => {
                self.ammo.harpoon += self.take_ammo(table, HARPOONS, 3);
                self.insert(table, HARPOON);
            }
            140 | 162 => {
                self.ammo.m16 += self.take_ammo(table, M16_AMMO, 40);
                self.insert(table, M16);
            }
            141 | 163 => {
                self.ammo.grenades += self.take_ammo(table, GRENADES, 2);
                self.insert(table, GRENADE_LAUNCHER);
            }
            143 | 165 if has(self, SHOTGUN) => self.ammo.shotgun += 12,
            144 | 166 if has(self, AUTOS) => self.ammo.autos += 40,
            145 | 167 if has(self, UZIS) => self.ammo.uzis += 80,
            146 | 168 if has(self, HARPOON) => self.ammo.harpoon += 3,
            147 | 169 if has(self, M16) => self.ammo.m16 += 40,
            148 | 170 if has(self, GRENADE_LAUNCHER) => self.ammo.grenades += 2,
            143 | 165 => self.insert(table, SHOTGUN_AMMO),
            144 | 166 => self.insert(table, AUTOS_AMMO),
            145 | 167 => self.insert(table, UZI_AMMO),
            146 | 168 => self.insert(table, HARPOONS),
            147 | 169 => self.insert(table, M16_AMMO),
            148 | 170 => self.insert(table, GRENADES),
            FLARE_BOX | FLARES => {
                self.insert(table, FLARES);
                for _ in 0..5 {
                    self.add(table, 152);
                }
            }
            152 => self.insert(table, FLARES),
            149 | 171 => self.insert(table, SMALL_MEDIPACK),
            150 | 172 => self.insert(table, LARGE_MEDIPACK),
            190..=192 => self.secrets |= 1 << (kind - 190),
            174..=177 | 193..=196 | 205 | 206 | 178..=181 | 197..=200 | 207 | 208 => {
                let Some(item) = table.item_for(kind) else { return false };
                self.insert(table, item);
            }
            _ => return false,
        }
        true
    }

    /// Lara's inventory at the start of the Great Wall in a new game, as the
    /// original holds it there (`.psx/use_wall_game`): statistics, pistols,
    /// the shotgun with 12 shells, two flares and a medi pack of each size.
    /// The original builds it from the gameflow script (`0x8002E468`),
    /// which is not ported.
    pub fn new_game(table: &ItemTable) -> Self {
        let mut inventory = Self::default();
        for kind in [item::STATISTICS, item::PISTOLS, item::SHOTGUN, 152, 152, 150, 149] {
            inventory.add(table, kind);
        }
        inventory
    }

    /// What Lara carries when a game starts at `level`: at Lara's Home
    /// (`assault`) only the statistics, elsewhere the Wall's new-game
    /// inventory, as the level-start script is not ported.
    pub fn start_at(table: &ItemTable, level: &str) -> Self {
        if level == "assault" {
            let mut inventory = Self::default();
            inventory.add(table, item::STATISTICS);
            inventory
        } else {
            Self::new_game(table)
        }
    }

    /// Uses a medi pack (UseItem `0x800433B8`, `0x80043428`): when Lara is
    /// hurt but alive (1..=999) it heals 500, or 1000 for a large one, up to
    /// 1000, and is used up. False, and nothing used, otherwise.
    pub fn use_medipack(&mut self, table: &ItemTable, item: u16, player: &mut Player) -> bool {
        if !(1..=999).contains(&player.health) || self.count(table, item) == 0 {
            return false;
        }
        let heal = if item == item::SMALL_MEDIPACK { 500 } else { 1000 };
        player.health = (player.health + heal).min(1000);
        self.remove(table, item)
    }

    /// Drops what belongs to a level: keys, puzzle pieces, pickups and found
    /// secrets. Weapons, ammunition, flares and medi packs carry on.
    pub fn leave_level(&mut self) {
        self.keys.clear();
        self.secrets = 0;
    }
}

/// Where an item's name comes from in the imported gameflow script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemName {
    /// A game string (`game 35`) or PlayStation string (`psx 0`).
    Game(usize),
    Psx(usize),
    /// The current level's puzzle (`puzzles 0..3`), pickup or key slot.
    Puzzle(usize),
    Pickup(usize),
    Key(usize),
}

impl<'de> Deserialize<'de> for ItemName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let (table, index) = text.split_once(' ').ok_or_else(|| serde::de::Error::custom("name is `table index`"))?;
        let index: usize = index.parse().map_err(serde::de::Error::custom)?;
        Ok(match table {
            "game" => Self::Game(index),
            "psx" => Self::Psx(index),
            "puzzles" => Self::Puzzle(index),
            "pickups" => Self::Pickup(index),
            "keys" => Self::Key(index),
            other => return Err(serde::de::Error::custom(format!("unknown string table {other}"))),
        })
    }
}

/// One item's record (`data/inventory.json`, the original's 0x54-byte
/// records at `0x80087FCC`).
#[derive(Clone, Debug, Deserialize)]
pub struct ItemData {
    /// The model drawn for it.
    pub object: u16,
    pub name: ItemName,
    /// The placed objects that become it.
    #[serde(default)]
    pub pickups: Vec<u16>,
    /// Frames of its opening animation, and the one where it is open.
    pub frames: i16,
    pub open_frame: i16,
    /// Its pose when selected: tilt towards the camera, x and y turns,
    /// height and distance (binary angles, world units).
    pub pt_xrot: i16,
    pub x_rot: i16,
    /// Its x turn on the ring.
    pub x_rot_closed: i16,
    pub y_rot: i16,
    pub y: i32,
    pub z: i32,
    /// Joints drawn, a bit each (-1 all).
    pub meshes: i32,
    /// Ring order; 100 and over is the keys ring.
    pub position: i16,
}

/// Every inventory item, from `data/inventory.json`.
#[derive(Resource, Clone, Debug)]
pub struct ItemTable {
    pub items: Vec<ItemData>,
}

impl Default for ItemTable {
    fn default() -> Self {
        #[derive(Deserialize)]
        struct File {
            items: Vec<ItemData>,
        }
        let file: File =
            serde_json::from_str(include_str!("../../data/inventory.json")).expect("data/inventory.json is valid");
        Self { items: file.items }
    }
}

impl ItemTable {
    pub fn get(&self, item: u16) -> Option<&ItemData> {
        self.items.iter().find(|i| i.object == item)
    }

    /// The ring item a placed object or item becomes (Inv_GetItemOption
    /// `0x80034894`).
    pub fn item_for(&self, kind: u16) -> Option<u16> {
        self.items.iter().find(|i| i.object == kind || i.pickups.contains(&kind)).map(|i| i.object)
    }
}

/// The imported gameflow script's strings (`imported/gameflow.json`).
#[derive(Resource, Clone, Debug, Default)]
pub struct GameStrings(pub import::gameflow::Gameflow);

impl GameStrings {
    /// An item's name in the given level.
    pub fn item(&self, name: &ItemName, level: usize) -> &str {
        let flow = &self.0;
        fn at(list: &[Vec<String>], slot: usize, level: usize) -> Option<&str> {
            list.get(slot).and_then(|l| l.get(level)).map(String::as_str)
        }
        let text = match name {
            ItemName::Game(i) => flow.game_strings.get(*i).map(String::as_str),
            ItemName::Psx(i) => flow.psx_strings.get(*i).map(String::as_str),
            ItemName::Puzzle(slot) => at(&flow.puzzles, *slot, level),
            ItemName::Pickup(slot) => at(&flow.pickups, *slot, level),
            ItemName::Key(slot) => at(&flow.keys, *slot, level),
        };
        text.unwrap_or_default()
    }

    pub fn game(&self, index: usize) -> &str {
        self.0.game_strings.get(index).map_or("", String::as_str)
    }

    pub fn psx(&self, index: usize) -> &str {
        self.0.psx_strings.get(index).map_or("", String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(list: &[Slot]) -> Vec<(u16, u16)> {
        list.iter().map(|s| (s.item, s.count)).collect()
    }

    #[test]
    fn the_wall_start_matches_the_original() {
        let table = ItemTable::default();
        let inventory = Inventory::new_game(&table);
        // `.psx/use_wall_game`: 0x80088AD8 items, 0x80088AA8 counts.
        assert_eq!(items(&inventory.main), [(121, 1), (157, 1), (158, 1), (173, 2), (172, 1), (171, 1)]);
        assert_eq!(inventory.ammo.shotgun, 12);
    }

    #[test]
    fn a_gun_takes_in_the_ammunition_carried_for_it() {
        let table = ItemTable::default();
        let mut inventory = Inventory::default();
        inventory.add(&table, 144);
        inventory.add(&table, 144);
        assert_eq!(items(&inventory.main), [(item::AUTOS_AMMO, 2)]);
        inventory.add(&table, 137);
        assert_eq!(items(&inventory.main), [(item::AUTOS, 1)]);
        assert_eq!(inventory.ammo.autos, 120);
        inventory.add(&table, 144);
        assert_eq!(inventory.ammo.autos, 160);
    }

    #[test]
    fn flares_come_in_sixes_and_keys_go_on_their_own_ring() {
        let table = ItemTable::default();
        let mut inventory = Inventory::default();
        inventory.add(&table, 151);
        inventory.add(&table, 151);
        inventory.add(&table, 194);
        inventory.add(&table, 193);
        assert_eq!(items(&inventory.main), [(item::FLARES, 12)]);
        assert_eq!(items(&inventory.keys), [(197, 1), (198, 1)]);
        assert!(inventory.remove(&table, 193));
        assert_eq!(items(&inventory.keys), [(198, 1)]);
        assert!(!inventory.remove(&table, 193));
    }
}
