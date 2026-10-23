//! Raw Tomb Raider II tables -> the engine's level format.
//!
//! Everything here is an exact re-expression: the engine's rules read the
//! converted values with the same integer arithmetic they used on the raw
//! tables. Anything the conversion cannot express exactly is an error.
use std::collections::BTreeMap;

use engine::world::format::{self as native, FORMAT_VERSION};

use crate::raw;

pub type Result<T> = std::result::Result<T, String>;

/// Floor-data functions (low five bits of a header word).
const PORTAL: u16 = 1;
const FLOOR_SLOPE: u16 = 2;
const CEILING_SLOPE: u16 = 3;
const TRIGGER: u16 = 4;
const KILL: u16 = 5;
const CLIMB: u16 = 6;
/// Last word of a chain / of a trigger's action list.
const END: u16 = 0x8000;
/// Activation code bits in trigger settings and placement flags.
const CODE_BITS: u16 = 0x3e00;

/// Trigger action codes the engine and the game know by name.
fn action_name(action: u16) -> String {
    match action {
        0 => "activate".into(),
        1 => "camera".into(),
        7 => "end_level".into(),
        8 => "music".into(),
        10 => "secret".into(),
        other => format!("action_{other}"),
    }
}

fn trigger_kind(kind: u16) -> native::TriggerKind {
    use native::TriggerKind::*;
    match kind {
        0 => Floor,
        1 => Pad,
        2 => Switch,
        3 => Key,
        4 => Pickup,
        6 => AntiPad,
        7 => Combat,
        other => Other(other),
    }
}

/// Decode the floor-data chain starting at `offset`: functions until one has
/// the end bit, or until an unknown function (which the original's readers
/// also stop at).
pub fn floor_info(data: &[u16], offset: usize) -> Result<native::FloorInfo> {
    let mut info = native::FloorInfo::default();
    let mut i = offset;
    for _ in 0..64 {
        let Some(&header) = data.get(i) else { return Ok(info) };
        let begin = i;
        i += 1;
        match header & 31 {
            PORTAL => {
                info.portal = data.get(i).map(|&r| usize::from(r));
                i += 1;
            }
            FLOOR_SLOPE | CEILING_SLOPE => {
                if let Some(&tilt) = data.get(i) {
                    let slope = [tilt as u8 as i8, (tilt >> 8) as u8 as i8];
                    if header & 31 == FLOOR_SLOPE { info.floor_slope = slope } else { info.ceiling_slope = slope }
                }
                i += 1;
            }
            TRIGGER => {
                let Some(&settings) = data.get(i) else { return Ok(info) };
                i += 1;
                let mut actions = Vec::new();
                let mut ended = false;
                for _ in 0..128 {
                    let Some(&word) = data.get(i) else { break };
                    i += 1;
                    let code = (word >> 10) & 31;
                    let mut action = native::Action { action: action_name(code), argument: word & 1023, camera: None };
                    let ending = if code == 1 {
                        // A camera action carries one more word: its shot settings.
                        let operand = data.get(i).copied().unwrap_or(END);
                        i += 1;
                        action.camera = Some(native::CameraShot {
                            timer: operand as u8,
                            once: operand & 256 != 0,
                            speed: ((operand & CODE_BITS) >> 9) as u8,
                        });
                        operand
                    } else {
                        word
                    };
                    actions.push(action);
                    if ending & END != 0 {
                        ended = true;
                        break;
                    }
                }
                if !ended {
                    return Err(format!("trigger at floor data {begin} has no end within 128 actions"));
                }
                info.triggers.push(native::Trigger {
                    id: begin,
                    kind: trigger_kind((header >> 8) & 127),
                    timer: settings as u8,
                    once: settings & 256 != 0,
                    mask: ((settings & CODE_BITS) >> 9) as u8,
                    actions,
                });
            }
            KILL => info.kill = true,
            CLIMB => info.climb = Some(((header >> 8) & 127) as u8),
            _ => return Ok(info),
        }
        if header & END != 0 {
            return Ok(info);
        }
    }
    Err(format!("floor data chain at {offset} is longer than 64 functions"))
}

/// Up to `count` animation commands from `offset`; an unknown command ends
/// the list (the original's readers stop there too).
fn commands(raw: &[i16], offset: usize, count: usize) -> Result<Vec<native::Command>> {
    use native::Command::*;
    let mut out = Vec::new();
    let mut i = offset;
    let arg = |i: usize| raw.get(i).copied().ok_or_else(|| format!("animation command operand {i} out of range"));
    for _ in 0..count {
        let Some(&command) = raw.get(i) else { break };
        i += 1;
        match command {
            1 => {
                out.push(SetPosition([arg(i)?, arg(i + 1)?, arg(i + 2)?].map(i32::from)));
                i += 3;
            }
            2 => {
                out.push(JumpVelocity([arg(i)?, arg(i + 1)?].map(i32::from)));
                i += 2;
            }
            3 => out.push(EmptyHands),
            4 => out.push(Kill),
            5 | 6 => {
                let frame = usize::try_from(arg(i)?).map_err(|_| format!("negative command frame at {i}"))?;
                let value = arg(i + 1)? as u16;
                i += 2;
                out.push(if command == 5 {
                    let environment = match value >> 14 {
                        0 => native::SoundEnvironment::Anywhere,
                        1 => native::SoundEnvironment::OnLand,
                        2 => native::SoundEnvironment::InWater,
                        _ => native::SoundEnvironment::Never,
                    };
                    Sound { frame, sound: usize::from(value & 0x3fff), environment }
                } else {
                    Effect { frame, effect: value }
                });
            }
            _ => break,
        }
    }
    Ok(out)
}

/// 1024-per-turn pose angles as binary angles (exact: a factor of 64).
fn pose(pose: &raw::Pose) -> native::Pose {
    native::Pose { root: pose.root, angles: pose.angles.iter().map(|a| a.map(|v| v * 64)).collect() }
}

fn animation(level: &raw::Level, index: usize) -> Result<native::Animation> {
    let a = &level.animations[index];
    let rate = a.frame_rate.max(1);
    // Every key the clip plays, plus the closing key its last interval
    // blends toward, when the frame data has it.
    let keys = (a.frame_end.saturating_sub(a.frame_start)) / rate + 2;
    let mut bounds = Vec::new();
    for k in 0..keys {
        let start = a.frame_offset / 2 + k * a.frame_size;
        let Some(words) = level.frame_data.get(start..start + 6) else { break };
        let key: [i32; 6] = std::array::from_fn(|i| i32::from(words[i] as i16));
        if let Some(p) = a.poses.get(k)
            && p.bounds != key
        {
            return Err(format!(
                "animation {index} key {k}: pose bounds {:?} differ from frame data {key:?}",
                p.bounds
            ));
        }
        bounds.push(key);
    }
    let state_changes = level
        .animation_states
        .iter()
        .skip(a.state_offset)
        .take(a.state_count)
        .flat_map(|state| level.animation_ranges.iter().skip(state[2]).take(state[1]).map(move |r| (state[0], r)))
        // Ranges with negative fields are unused slots.
        .filter(|(_, range)| range.iter().all(|&v| v >= 0))
        .map(|(state, range)| native::StateChange {
            state,
            first_frame: range[0] as usize,
            last_frame: range[1] as usize,
            animation: range[2] as usize,
            frame: range[3] as usize,
        })
        .collect();
    Ok(native::Animation {
        frame_rate: a.frame_rate,
        frame_start: a.frame_start,
        frame_end: a.frame_end,
        next_animation: a.next_animation,
        next_frame: a.next_frame,
        state: a.state,
        speed: a.speed,
        accel: a.accel,
        poses: a.poses.iter().map(pose).collect(),
        bounds,
        state_changes,
        commands: commands(&level.animation_commands, a.command_offset, a.command_count)?,
    })
}

const NO_ROOM: usize = 255;
const NO_BOX: usize = 0xffff;
const WATER: u16 = 0x01;
const WIND: u16 = 0x20;
const INVERTED: u16 = 0x4000;
const INVISIBLE: u16 = 0x0100;
const ONCE: i32 = 0x0100;

pub fn level(raw: &raw::Level) -> Result<native::Level> {
    let mut infos = Vec::new();
    let mut by_offset = BTreeMap::new();
    let mut info_for = |offset: usize| -> Result<Option<usize>> {
        if offset == 0 {
            return Ok(None);
        }
        if let Some(&i) = by_offset.get(&offset) {
            return Ok(Some(i));
        }
        infos.push(floor_info(&raw.floor_data, offset)?);
        by_offset.insert(offset, infos.len() - 1);
        Ok(Some(infos.len() - 1))
    };
    let mut rooms = Vec::new();
    for (index, r) in raw.rooms.iter().enumerate() {
        if r.id != index {
            return Err(format!("room {index} has id {}", r.id));
        }
        let mut sectors = Vec::new();
        for s in &r.sectors {
            sectors.push(native::Sector {
                floor: s.floor,
                ceiling: s.ceiling,
                below: (s.below != NO_ROOM).then_some(s.below),
                above: (s.above != NO_ROOM).then_some(s.above),
                nav_box: (s.navigation_box != NO_BOX).then_some(s.navigation_box),
                floor_info: info_for(s.floor_data)?,
            });
        }
        if r.vertices.iter().any(|v| v[4] != 0) {
            return Err(format!("room {index}: unexpected fifth vertex field"));
        }
        rooms.push(native::Room {
            origin: r.origin,
            top: r.top,
            bottom: r.bottom,
            vertices: r.vertices.iter().map(|v| [v[0], v[1], v[2], v[3]]).collect(),
            quads: r.quads.clone(),
            triangles: r.triangles.clone(),
            portals: r
                .portals
                .iter()
                .map(|p| native::Portal { room: p.room, normal: p.normal, vertices: p.vertices.clone() })
                .collect(),
            x_sectors: r.x_sectors,
            z_sectors: r.z_sectors,
            sectors,
            static_placements: r
                .static_placements
                .iter()
                .map(|s| native::StaticPlacement {
                    mesh_id: s.mesh_id,
                    position: s.position,
                    rotation: s.rotation,
                    shade: s.light,
                })
                .collect(),
            ambient: native::Ambient { shade: r.ambient[0], shade2: r.ambient[1], mode: r.ambient[2] },
            lights: r
                .lights
                .iter()
                .map(|l| native::Light {
                    position: [l[0], l[1], l[2]].map(|v| v as i32),
                    // The light reader takes the low halfword as signed.
                    intensity: l[3] as i16,
                    intensity2: l[4] as i16,
                    falloff: l[5] as i32,
                    falloff2: l[6] as i32,
                })
                .collect(),
            alternate: usize::try_from(r.alternate).ok(),
            water: r.flags & WATER != 0,
            wind: r.flags & WIND != 0,
            other_flags: r.flags & !(WATER | WIND),
        });
    }
    let mesh_of = |i: usize| raw.mesh_indices.get(i).copied().ok_or_else(|| format!("mesh index {i} out of range"));
    let models = raw
        .models
        .iter()
        .map(|m| {
            Ok(native::Model {
                id: m.id,
                meshes: (m.mesh_start..m.mesh_start + m.mesh_count).map(mesh_of).collect::<Result<_>>()?,
                joints: m
                    .joints
                    .iter()
                    .map(|j| native::Joint { parent: usize::try_from(j.parent).ok(), offset: j.translation })
                    .collect(),
                animations: [m.animation, m.animation_end],
                initial_pose: m.initial_pose.as_ref().map(pose),
            })
        })
        .collect::<Result<_>>()?;
    let placements = raw
        .entities
        .iter()
        .map(|e| native::Placement {
            kind: e.id,
            room: e.room,
            position: e.position,
            rotation: e.rotation,
            code: ((e.flags & CODE_BITS) >> 9) as u8,
            inverted: e.flags & INVERTED != 0,
            invisible: e.flags & INVISIBLE != 0,
            other_flags: e.flags & !(CODE_BITS | INVERTED | INVISIBLE),
        })
        .collect();
    Ok(native::Level {
        format: FORMAT_VERSION,
        name: raw.name.clone(),
        source: Some(native::Source {
            file: format!("DATA/{}.PSX", raw.name.to_uppercase()),
            sha256: raw.source_sha256.clone(),
            size: raw.source_size,
        }),
        rooms,
        floor_infos: infos,
        textures: raw.textures.iter().map(|t| native::Texture { uv: t.atlas_uv, attribute: t.attribute }).collect(),
        sprites: raw.sprites.iter().map(|s| native::Sprite { atlas: s.atlas, rect: s.rect }).collect(),
        sprite_sequences: raw
            .sprite_sequences
            .iter()
            .map(|s| native::SpriteSequence { id: s.id, first: s.first, count: s.count })
            .collect(),
        meshes: raw
            .meshes
            .iter()
            .map(|m| native::Mesh {
                center: m.center,
                radius: m.radius,
                vertices: m.vertices.clone(),
                normals: m.normals.iter().map(|n| [n[0], n[1], n[2]]).collect(),
                quads: m.quads.clone(),
                triangles: m.triangles.clone(),
            })
            .collect(),
        models,
        static_meshes: raw
            .static_meshes
            .iter()
            .map(|s| {
                Ok(native::StaticMesh {
                    id: s.id,
                    mesh: mesh_of(s.mesh)?,
                    collision_box: s.collision_box,
                    flags: s.flags,
                })
            })
            .collect::<Result<_>>()?,
        animations: (0..raw.animations.len()).map(|i| animation(raw, i)).collect::<Result<_>>()?,
        placements,
        cameras: raw
            .cameras
            .iter()
            .map(|c| native::FixedCamera {
                position: [c[0], c[1], c[2]],
                room: usize::try_from(c[3]).ok(),
                once: c[4] & ONCE != 0,
                other_flags: (c[4] & !ONCE) as u16,
            })
            .collect(),
        boxes: raw
            .boxes
            .iter()
            .map(|b| native::NavBox { z_min: b[0], z_max: b[1], x_min: b[2], x_max: b[3], floor: b[4], overlap: b[5] })
            .collect(),
        overlaps: raw.overlaps.clone(),
        zones: raw.zones.clone(),
        sound_map: raw.sound_map.iter().map(|&s| usize::try_from(s).ok()).collect(),
        sounds: raw
            .sound_info
            .iter()
            .map(|s| native::Sound { sample: usize::from(s[0]), volume: s[1], range: s[2], flags: s[3] })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use native::TriggerKind;

    #[test]
    fn camera_operand_keeps_its_timer_once_and_speed_bits() {
        let info = floor_info(&[0, 0x8004, 0x3e00, 1026, 0x8703], 1).unwrap();
        let action = &info.triggers[0].actions[0];
        assert_eq!((action.action.as_str(), action.argument), ("camera", 2));
        assert_eq!(action.camera, Some(native::CameraShot { timer: 3, once: true, speed: 3 }));
    }

    #[test]
    fn trigger_decoder_skips_camera_operand_without_losing_exit() {
        let info = floor_info(&[0, 0x8004, 0x3e00, 1 << 10 | 7, 33, 0x8000 | 7 << 10], 1).unwrap();
        assert_eq!(info.triggers.len(), 1);
        let names: Vec<_> = info.triggers[0].actions.iter().map(|a| (a.action.as_str(), a.argument)).collect();
        assert_eq!(names, [("camera", 7), ("end_level", 0)]);
        assert_eq!(info.triggers[0].kind, TriggerKind::Floor);
        assert_eq!(info.triggers[0].mask, 31);
    }

    #[test]
    fn slopes_portals_and_kill_share_one_chain() {
        // Floor slope (x -3, z 2), portal to room 7, then kill with the end bit.
        let tilt = (2u16 << 8) | (-3i8 as u8 as u16);
        let info = floor_info(&[0, 2, tilt, 1, 7, 0x8005], 1).unwrap();
        assert_eq!(info.floor_slope, [-3, 2]);
        assert_eq!(info.portal, Some(7));
        assert!(info.kill);
    }

    #[test]
    fn unknown_commands_end_the_list() {
        let list = commands(&[1, 0, -512, 128, 99, 4], 0, 3).unwrap();
        assert_eq!(list, [native::Command::SetPosition([0, -512, 128])]);
    }
}
