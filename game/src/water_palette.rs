//! Underwater colour from the supplied disc's paired RGB555 palettes.
//! All 8192 entries halve red5, preserving green, blue and STP. Room/object
//! setup at 0x24FD0..0x25000 and 0x251BC..0x251EC selects this palette for
//! wet rooms or a wet camera; 0x23F14..0x23F20 samples the camera room flag.
use std::collections::HashMap;

use bevy::{prelude::*, render::render_resource::TextureFormat};
use engine::{
    camera::CameraState,
    psx::PsxMaterial,
    render::{CameraRig, GameCamera, InRoom, LevelSystems, textures::TextureOverride},
    schedule::Avatar,
    world::WorldGeometry,
};

#[derive(Component)]
struct WaterTexture(Handle<Image>);

#[derive(Resource, Default)]
struct WaterTextures(HashMap<AssetId<Image>, Handle<Image>>);

pub struct WaterPalettePlugin;

/// Developer comparison switch; the original palette is enabled by default.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct WaterPalette {
    pub enabled: bool,
}

impl Plugin for WaterPalettePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaterTextures>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((Name::new("WaterPalette"), WaterPalette { enabled: true }));
            })
            .add_systems(Update, select.after(LevelSystems::Meshes).before(LevelSystems::Textures));
    }
}

/// The baked atlas expands five-bit channels with floor(channel*255/31).
/// Recover that red value, halve it, and expand it again before lighting.
fn underwater_red(red: u8) -> u8 {
    let red5 = (u32::from(red) * 31 + 127) / 255;
    ((red5 / 2) * 255 / 31) as u8
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn select(
    mut commands: Commands,
    world: Option<Res<WorldGeometry>>,
    camera: Res<CameraState>,
    rigs: Query<&CameraRig, With<GameCamera>>,
    controls: Query<&WaterPalette>,
    avatars: Query<&Avatar>,
    hierarchy: Query<(Option<&InRoom>, Option<&ChildOf>)>,
    meshes: Query<(Entity, &MeshMaterial3d<PsxMaterial>, Option<&WaterTexture>, Option<&TextureOverride>)>,
    materials: Res<Assets<PsxMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<WaterTextures>,
) {
    let Some(world) = world else { return };
    let enabled = controls.single().is_ok_and(|c| c.enabled);
    let camera_room = rigs
        .single()
        .ok()
        .filter(|r| r.manual)
        .and_then(|r| {
            world
                .floor_at_for(r.position, camera.room, true)
                .or_else(|| avatars.single().ok().and_then(|a| world.floor_at_for(r.position, a.room, true)))
        })
        .map_or(camera.room, |hit| hit.room);
    let camera_wet = world.data.rooms.get(camera_room).is_some_and(|r| r.water);
    cache.0.retain(|id, _| images.contains(*id));
    for (entity, material, water, replacement) in &meshes {
        // Room meshes, static objects, posed bones and secondary motion all
        // inherit a room through the ordinary scene hierarchy. HUD and sky
        // have no room, and keep their own textures.
        let mut ancestor = entity;
        let mut room = None;
        while let Ok((found, parent)) = hierarchy.get(ancestor) {
            if let Some(found) = found {
                room = Some(found.0);
                break;
            }
            let Some(parent) = parent else { break };
            ancestor = parent.parent();
        }
        let Some(room) = room else { continue };
        let wet = enabled && (camera_wet || world.data.rooms.get(room).is_some_and(|r| r.water));
        let texture = if let Some(water) = water {
            water.0.clone()
        } else {
            let Some(normal) = materials.get(&material.0).and_then(|m| m.texture.as_ref()) else { continue };
            let texture = if let Some(cached) = cache.0.get(&normal.id()) {
                cached.clone()
            } else {
                let Some(mut image) = images.get(normal).cloned() else { continue };
                if image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb {
                    continue;
                }
                let Some(data) = image.data.as_mut() else { continue };
                for pixel in data.as_chunks_mut::<4>().0 {
                    pixel[0] = underwater_red(pixel[0]);
                }
                let handle = images.add(image);
                cache.0.insert(normal.id(), handle.clone());
                handle
            };
            commands.entity(entity).insert(WaterTexture(texture.clone()));
            texture
        };
        let desired = wet.then_some(texture);
        if replacement.map(|r| &r.0) != Some(&desired) {
            commands.entity(entity).insert(TextureOverride(desired));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_red_matches_every_observed_five_bit_level() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/data/original/underwater_palette.json")).unwrap();
        let pairs = fixture["representatives"].as_array().unwrap();
        assert_eq!(pairs.len(), 32);
        for pair in pairs {
            let normal = pair["normal"].as_u64().unwrap() as u16;
            let underwater = pair["underwater"].as_u64().unwrap() as u16;
            assert_eq!(underwater_red(((normal & 31) * 255 / 31) as u8), ((underwater & 31) * 255 / 31) as u8);
            assert_eq!(normal & !31, underwater & !31);
        }
    }
}
