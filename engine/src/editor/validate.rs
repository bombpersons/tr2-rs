use crate::world::{Level, format::FORMAT_VERSION};

/// Validate the engine's own file format, never game-specific ids or rules.
/// Error paths are JSON pointers, matching the editor's field browser.
pub fn validate_level(level: &Level) -> Result<(), String> {
    let check = |ok: bool, path: String, reason: &str| if ok { Ok(()) } else { Err(format!("{path}: {reason}")) };
    check(level.format == FORMAT_VERSION, "/format".into(), "unsupported format version")?;
    check(!level.rooms.is_empty(), "/rooms".into(), "at least one room is required")?;
    let room_ref = |r: Option<usize>| r.is_none_or(|r| r < level.rooms.len());
    let faces = |path: &str, vertices: usize, quads: &[[usize; 5]], triangles: &[[usize; 4]]| {
        for (kind, fs) in [
            ("quads", quads.iter().map(|f| f.as_slice()).collect::<Vec<_>>()),
            ("triangles", triangles.iter().map(|f| f.as_slice()).collect()),
        ] {
            for (i, f) in fs.iter().enumerate() {
                check(
                    f[..f.len() - 1].iter().all(|&v| v < vertices),
                    format!("{path}/{kind}/{i}"),
                    "vertex index out of range",
                )?;
                check(
                    f[f.len() - 1] < level.textures.len(),
                    format!("{path}/{kind}/{i}"),
                    "texture index out of range",
                )?;
            }
        }
        Ok::<(), String>(())
    };
    for (i, r) in level.rooms.iter().enumerate() {
        let p = format!("/rooms/{i}");
        check(
            r.x_sectors > 0 && r.z_sectors > 0 && r.x_sectors.checked_mul(r.z_sectors) == Some(r.sectors.len()),
            p.clone(),
            "sector dimensions must match sectors",
        )?;
        faces(&p, r.vertices.len(), &r.quads, &r.triangles)?;
        check(room_ref(r.alternate), format!("{p}/alternate"), "room index out of range")?;
        for (j, s) in r.sectors.iter().enumerate() {
            check(room_ref(s.above) && room_ref(s.below), format!("{p}/sectors/{j}"), "room index out of range")?;
            check(
                s.floor_info.is_none_or(|f| f < level.floor_infos.len()),
                format!("{p}/sectors/{j}/floor_info"),
                "floor info out of range",
            )?;
            check(
                s.nav_box.is_none_or(|b| b < level.boxes.len()),
                format!("{p}/sectors/{j}/box"),
                "navigation box out of range",
            )?;
        }
        for (j, portal) in r.portals.iter().enumerate() {
            check(
                room_ref(Some(portal.room)) && portal.vertices.len() == 4,
                format!("{p}/portals/{j}"),
                "portal needs a valid room and four corners",
            )?;
        }
        for (j, s) in r.static_placements.iter().enumerate() {
            check(
                level.static_meshes.iter().any(|m| m.id == s.mesh_id),
                format!("{p}/static_placements/{j}/mesh_id"),
                "unknown static mesh",
            )?;
        }
    }
    for (i, mesh) in level.meshes.iter().enumerate() {
        faces(&format!("/meshes/{i}"), mesh.vertices.len(), &mesh.quads, &mesh.triangles)?;
        check(
            mesh.normals.is_empty() || mesh.normals.len() == mesh.vertices.len(),
            format!("/meshes/{i}/normals"),
            "one normal per vertex is required",
        )?;
    }
    for (i, model) in level.models.iter().enumerate() {
        let p = format!("/models/{i}");
        check(model.meshes.iter().all(|&m| m < level.meshes.len()), format!("{p}/meshes"), "mesh index out of range")?;
        check(model.joints.len() == model.meshes.len(), format!("{p}/joints"), "one joint per mesh is required")?;
        for (j, joint) in model.joints.iter().enumerate() {
            check(joint.parent.is_none_or(|p| p < j), format!("{p}/joints/{j}/parent"), "parent must precede joint")?;
        }
        check(
            model.initial_pose.as_ref().is_none_or(|p| p.angles.len() == model.meshes.len()),
            format!("{p}/initial_pose"),
            "one angle per joint is required",
        )?;
        // Empty ranges (also reversed ones) have no clips; model_pose uses the initial pose.
        check(
            model.animations[0] >= model.animations[1] || model.animations[1] <= level.animations.len(),
            format!("{p}/animations"),
            "clip range out of bounds",
        )?;
    }
    for (i, m) in level.static_meshes.iter().enumerate() {
        check(m.mesh < level.meshes.len(), format!("/static_meshes/{i}/mesh"), "mesh index out of range")?;
    }
    for (i, p) in level.placements.iter().enumerate() {
        check(room_ref(Some(p.room)), format!("/placements/{i}/room"), "room index out of range")?;
        check(p.code <= 31, format!("/placements/{i}/code"), "activation code uses five bits")?;
    }
    for (i, c) in level.cameras.iter().enumerate() {
        check(room_ref(c.room), format!("/cameras/{i}/room"), "room index out of range")?;
    }
    for (i, f) in level.floor_infos.iter().enumerate() {
        check(room_ref(f.portal), format!("/floor_infos/{i}/portal"), "room index out of range")?;
        for (j, t) in f.triggers.iter().enumerate() {
            check(t.mask <= 31, format!("/floor_infos/{i}/triggers/{j}/mask"), "activation mask uses five bits")?;
            for (k, a) in t.actions.iter().enumerate() {
                let p = format!("/floor_infos/{i}/triggers/{j}/actions/{k}");
                check((a.action == "camera") == a.camera.is_some(), p.clone(), "camera action requires shot settings")?;
                if a.action == "activate" {
                    check((a.argument as usize) < level.placements.len(), p.clone(), "placement index out of range")?;
                }
                if a.action == "camera" {
                    check((a.argument as usize) < level.cameras.len(), p, "camera index out of range")?;
                }
            }
        }
    }
    for (i, a) in level.animations.iter().enumerate() {
        check(a.bounds.len() >= a.poses.len(), format!("/animations/{i}/bounds"), "each pose requires bounds")?;
        check(a.frame_start <= a.frame_end, format!("/animations/{i}"), "reversed frame range")?;
        check(
            a.next_animation < level.animations.len(),
            format!("/animations/{i}/next_animation"),
            "animation index out of range",
        )?;
        for (j, change) in a.state_changes.iter().enumerate() {
            check(
                change.animation < level.animations.len(),
                format!("/animations/{i}/state_changes/{j}/animation"),
                "animation index out of range",
            )?;
            check(
                change.first_frame <= change.last_frame,
                format!("/animations/{i}/state_changes/{j}"),
                "reversed dispatch window",
            )?;
        }
        // Imported jumps can name frames outside the destination's ordinary
        // key range. Preserve them: the timing step supports signed offsets.
    }
    for (i, s) in level.sound_map.iter().enumerate() {
        check(s.is_none_or(|s| s < level.sounds.len()), format!("/sound_map/{i}"), "sound index out of range")?;
    }
    Ok(())
}
