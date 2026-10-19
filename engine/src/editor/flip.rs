//! Flip rooms: a room's `alternate` is a second version of it that the level
//! swaps in when it flips (a flooded room, a fallen ceiling).
//!
//! In every imported level each alternate pairs with exactly one room, lies
//! on the same grid as it, and nothing links into the alternate by number:
//! its doorways, holes and portals name the same rooms its base's do, and
//! the rooms around it name the base. When the level flips, the base's room
//! number holds the alternate's data. [`flipped`] builds the level as it is
//! then, so the same checks and lookups apply to both states.
use crate::world::Level;

/// For each room, the room it is the alternate of.
pub fn flip_bases(level: &Level) -> Vec<Option<usize>> {
    let mut bases = vec![None; level.rooms.len()];
    for (i, r) in level.rooms.iter().enumerate() {
        if let Some(b) = r.alternate.and_then(|a| bases.get_mut(a))
            && b.is_none()
        {
            *b = Some(i);
        }
    }
    bases
}

/// The room a flip swaps with `room`: its alternate, or the room it is the
/// alternate of.
pub fn flip_partner(level: &Level, room: usize) -> Option<usize> {
    let r = level.rooms.get(room)?;
    r.alternate.filter(|&a| a < level.rooms.len()).or_else(|| flip_bases(level).get(room).copied().flatten())
}

/// Which rooms are in the level before it flips (`false`) or after: every
/// room but the alternates, or every room but those an alternate replaces.
pub fn live_rooms(level: &Level, flipped: bool) -> Vec<bool> {
    let bases = flip_bases(level);
    level
        .rooms
        .iter()
        .zip(&bases)
        .map(|(r, base)| if flipped { r.alternate.is_none_or(|a| a >= level.rooms.len()) } else { base.is_none() })
        .collect()
}

/// The level after it flips: each paired room's number holds its alternate's
/// data, and the alternate's number the original's. Rooms keep their
/// `alternate` fields, so flipping twice gives the level back.
pub fn flipped(level: &Level) -> Level {
    let mut out = level.clone();
    for (base, r) in level.rooms.iter().enumerate() {
        let Some(alternate) = r.alternate.filter(|&a| a < level.rooms.len() && a != base) else { continue };
        if flip_bases(level)[alternate] != Some(base) {
            continue;
        }
        out.rooms.swap(base, alternate);
        out.rooms[base].alternate = level.rooms[base].alternate;
        out.rooms[alternate].alternate = level.rooms[alternate].alternate;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Room;

    fn level() -> Level {
        let room = |top| Room { top, ..Default::default() };
        let mut rooms = vec![room(0), room(1), room(2), room(3)];
        rooms[1].alternate = Some(3);
        Level { rooms, ..Default::default() }
    }

    #[test]
    fn alternates_pair_with_their_rooms_and_swap_in_when_flipped() {
        let level = level();
        assert_eq!(flip_bases(&level), [None, None, None, Some(1)]);
        assert_eq!(
            (flip_partner(&level, 1), flip_partner(&level, 3), flip_partner(&level, 0)),
            (Some(3), Some(1), None)
        );
        assert_eq!(live_rooms(&level, false), [true, true, true, false]);
        assert_eq!(live_rooms(&level, true), [true, false, true, true]);
        let tops = |l: &Level| l.rooms.iter().map(|r| r.top).collect::<Vec<_>>();
        let once = flipped(&level);
        assert_eq!(tops(&once), [0, 3, 2, 1]);
        assert_eq!(once.rooms[1].alternate, Some(3), "the pairing stays");
        assert_eq!(tops(&flipped(&once)), tops(&level));
    }
}
