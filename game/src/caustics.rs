//! Animated wet-room vertex lighting, from the supplied disc's room renderer
//! at 0x62734. Each authored vertex has a repeatable phase; the signed wave is
//! added before subdivision averages the corner shades. Camera water alone
//! selects a palette but does not enable this effect in dry rooms.
use std::sync::Arc;

use bevy::prelude::*;
use engine::{
    render::rooms::{RoomFace, RoomMeshOutput, RoomMeshPolicy, RoomTessellator, RoomView},
    schedule::WorldSystems,
    shell::GameSystems,
    world::Level,
};
use serde::Deserialize;

use crate::subdivision::RoomSubdivision;

/// Current visual wave phase, in source display fields. 0x6D234..0x6D244
/// advances it by the elapsed fields and wraps 64; ordinary play uses two
/// fields per logic tick. Reflected for paused comparison with a PSX frame.
#[derive(Component, Reflect, Clone, Copy)]
#[reflect(Component)]
pub struct Caustics {
    pub enabled: bool,
    pub phase: u8,
}

#[derive(Deserialize)]
struct Tables {
    wave: Vec<i8>,
    vertex_phase: Vec<u8>,
}

/// Baked startup tables from 0x8B4C0/0x8B500. Their generation and deterministic
/// draw-random seed are verified in tools/record_caustics.py, not emulated.
#[derive(Resource, Clone)]
pub struct CausticTables(Arc<Tables>);

impl Default for CausticTables {
    fn default() -> Self {
        let tables: Tables = serde_json::from_str(include_str!("../data/caustics.json")).expect("baked caustic tables");
        assert_eq!(tables.wave.len(), 64);
        assert_eq!(tables.vertex_phase.len(), 64);
        Self(Arc::new(tables))
    }
}

impl CausticTables {
    /// 0x6D1F4 supplies ~phase to 0x62754; vertex identity wraps every 64.
    pub fn shade(&self, vertex: usize, base: u8, phase: u8) -> u8 {
        let offset = self.0.vertex_phase[vertex % 64].wrapping_sub(phase).wrapping_sub(1) % 64;
        (i16::from(base) + i16::from(self.0.wave[offset as usize])).clamp(0, 255) as u8
    }
}

pub struct CausticsPlugin;

impl Plugin for CausticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CausticTables>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((Name::new("Caustics"), Caustics { enabled: true, phase: 0 }));
            })
            .add_systems(FixedUpdate, advance.in_set(WorldSystems::Effects).in_set(GameSystems::Game));
    }
}

fn advance(mut effects: Query<&mut Caustics>) {
    for mut effect in &mut effects {
        effect.phase = (u32::from(effect.phase) + crate::FIELDS_PER_TICK) as u8 % 64;
    }
}

pub fn room_policy(
    subdivision: RoomSubdivision,
    caustics: Caustics,
    level: &Level,
    tables: CausticTables,
) -> RoomMeshPolicy {
    RoomMeshPolicy(Box::new(RoomEffects {
        subdivision,
        caustics,
        wet_rooms: level.rooms.iter().map(|room| room.water).collect(),
        tables,
    }))
}

struct RoomEffects {
    subdivision: RoomSubdivision,
    caustics: Caustics,
    wet_rooms: Vec<bool>,
    tables: CausticTables,
}

impl RoomTessellator for RoomEffects {
    fn tessellate(&self, face: &RoomFace, view: &RoomView, output: &mut RoomMeshOutput) {
        if !self.caustics.enabled || !self.wet_rooms.get(view.room).copied().unwrap_or(false) {
            self.subdivision.tessellate(face, view, output);
            return;
        }
        let mut vertices = face.vertices.clone();
        for (vertex, &index) in vertices.iter_mut().zip(&face.source_indices) {
            vertex.shade = self.tables.shade(index, vertex.shade, self.caustics.phase);
        }
        self.subdivision.tessellate(
            &RoomFace {
                vertices,
                source_indices: face.source_indices.clone(),
                texture_attribute: face.texture_attribute,
            },
            view,
            output,
        );
    }
}
