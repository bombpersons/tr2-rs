//! Exercise the shipped, ROM-independent level boundary against every baked asset.
#![cfg(feature = "baked-assets")]
use std::{fs, path::PathBuf, sync::Arc};

use engine::world::{Level, WorldGeometry, format::FORMAT_VERSION, position};

#[test]
fn every_baked_level_deserializes_and_has_valid_geometry_and_collision() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/levels");
    let mut dirs: Vec<_> =
        fs::read_dir(&root).unwrap().map(Result::unwrap).map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    dirs.sort();
    assert_eq!(dirs.len(), 25, "the supplied game contains 25 level/cutscene files");
    let mut floors_tested = 0;
    for dir in dirs {
        let path = dir.join("level.json");
        let data: Level =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let label = data.name.clone();
        assert_eq!(data.format, FORMAT_VERSION, "{label}: format version");
        engine::editor::validate_level(&data).unwrap_or_else(|error| panic!("{label}: editor: {error}"));
        assert!(!data.rooms.is_empty(), "{label}: rooms");
        assert!(!data.textures.is_empty(), "{label}: textures");
        let check_face = |vertices: &[usize], texture: usize, count: usize| {
            assert!(vertices.iter().all(|&index| index < count), "{label}: face vertex");
            assert!(texture < data.textures.len(), "{label}: face texture");
        };
        for mesh in &data.meshes {
            for face in &mesh.quads {
                check_face(&face[..4], face[4], mesh.vertices.len());
            }
            for face in &mesh.triangles {
                check_face(&face[..3], face[3], mesh.vertices.len());
            }
        }
        for texture in &data.textures {
            assert!(texture.uv.iter().flatten().all(|uv| uv.is_finite()), "{label}: finite UV");
        }
        for room in &data.rooms {
            assert_eq!(room.sectors.len(), room.x_sectors * room.z_sectors, "{label}: sector grid");
            for face in &room.quads {
                check_face(&face[..4], face[4], room.vertices.len());
            }
            for face in &room.triangles {
                check_face(&face[..3], face[3], room.vertices.len());
            }
            for portal in &room.portals {
                assert!(portal.room < data.rooms.len(), "{label}: portal room");
                assert_eq!(portal.vertices.len(), 4, "{label}: portal polygon");
            }
            for sector in &room.sectors {
                assert!(sector.floor_info.is_none_or(|i| i < data.floor_infos.len()), "{label}: floor info");
                for target in [sector.above, sector.below].into_iter().flatten() {
                    assert!(target < data.rooms.len(), "{label}: vertical portal");
                }
                assert!(sector.nav_box.is_none_or(|b| b < data.boxes.len()), "{label}: navigation box");
            }
        }
        for info in &data.floor_infos {
            for trigger in &info.triggers {
                for action in &trigger.actions {
                    assert_eq!(action.camera.is_some(), action.action == "camera", "{label}: camera shot settings");
                }
            }
        }
        for model in &data.models {
            assert!(model.meshes.iter().all(|&m| m < data.meshes.len()), "{label}: model meshes");
            assert_eq!(model.joints.len(), model.meshes.len(), "{label}: skeleton length");
            for (joint, bone) in model.joints.iter().enumerate() {
                assert!(bone.parent.is_none_or(|p| p < joint), "{label}: bone parent");
            }
            if let Some(pose) = &model.initial_pose {
                assert_eq!(pose.angles.len(), model.meshes.len(), "{label}: initial pose");
            }
        }
        for animation in &data.animations {
            assert!(animation.bounds.len() >= animation.poses.len(), "{label}: key bounds");
        }
        for object in &data.static_meshes {
            assert!(object.mesh < data.meshes.len(), "{label}: static mesh");
        }
        for placement in &data.placements {
            assert!(placement.room < data.rooms.len(), "{label}: placement room");
        }
        let atlas = fs::read(dir.join("atlas.png")).unwrap();
        assert!(atlas.starts_with(b"\x89PNG\r\n\x1a\n"), "{label}: standalone texture atlas");
        let geometry = WorldGeometry { data: Arc::new(data) };
        // Invoke the actual portal/slope collision query over every sector.
        // Solid or out-of-stack sectors may return None, but valid hits must be finite.
        for (room_index, room) in geometry.data.rooms.iter().enumerate() {
            for (index, sector) in room.sectors.iter().enumerate() {
                let x = room.origin[0] + (index / room.z_sectors) as i32 * 1024 + 512;
                let z = room.origin[2] + (index % room.z_sectors) as i32 * 1024 + 512;
                let y = (sector.floor + sector.ceiling) / 2;
                if let Some(hit) = geometry.floor_at(position([x, y, z]), room_index) {
                    assert!(hit.height.is_finite() && hit.ceiling.is_finite(), "{label}: finite sector collision");
                    assert!(hit.room < geometry.data.rooms.len(), "{label}: collision room");
                    floors_tested += 1;
                }
            }
        }
    }
    assert!(floors_tested > 1000, "real sector collision queries must have been exercised");
}
