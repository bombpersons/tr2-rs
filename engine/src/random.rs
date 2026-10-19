//! A seedable game random generator for rules that must replay exactly.
//!
//! The classic 32-bit linear congruential step (multiplier 1103515245,
//! increment 12345), returning bits 10-24 as a value in 0..32768. Games keep
//! one generator for gameplay decisions; tests seed it from recorded state.
use bevy::prelude::*;

#[derive(Resource, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Resource)]
pub struct GameRandom {
    pub seed: u32,
}

impl Default for GameRandom {
    fn default() -> Self {
        Self { seed: 0x1154_8b12 }
    }
}

impl GameRandom {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Advance the generator and return a value in `0..32768`.
    pub fn draw(&mut self) -> i32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ((self.seed >> 10) & 0x7fff) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_like_the_recorded_generator() {
        // Two consecutive stores of one recorded shot (pistols_tiger_volley).
        let mut random = GameRandom::new(0xEC6C_465E);
        random.draw();
        assert_eq!(random.seed, 0xF23E_CA3F);
        random.draw();
        assert_eq!(random.seed, 0xB215_7F0C);
    }
}
