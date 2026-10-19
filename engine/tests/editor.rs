use engine::editor::{LevelDocument, child_pointer};
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engine-editor-{}-{name}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.json");
    fs::write(&path, include_bytes!("../assets/levels/sandbox/level.json")).unwrap();
    path
}

#[test]
fn unchanged_and_undone_saves_preserve_every_byte_and_unknown_fields() {
    let path = fixture("exact");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["future/field~"] = serde_json::json!({"preserve": [1, 2, 3]});
    let bytes = format!(" \r\n{}\r\n", serde_json::to_string_pretty(&value).unwrap()).into_bytes();
    fs::write(&path, &bytes).unwrap();
    let mut doc = LevelDocument::open(&path).unwrap();
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    doc.replace("/name", "\"changed\"").unwrap();
    assert!(doc.is_dirty());
    assert!(doc.undo());
    assert!(!doc.is_dirty());
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(doc.redo());
    doc.save().unwrap();
    let reopened = LevelDocument::open(&path).unwrap();
    assert_eq!(reopened.level().unwrap().name, "changed");
    assert_eq!(reopened.value().pointer(&child_pointer("", "future/field~")), Some(&value["future/field~"]));
}

#[test]
fn invalid_edits_are_transactional_and_disk_conflicts_never_overwrite() {
    let path = fixture("invalid");
    let bytes = fs::read(&path).unwrap();
    let mut doc = LevelDocument::open(&path).unwrap();
    for (pointer, text) in [
        ("/format", "999"),
        ("/placements/0/room", "99999"),
        ("/rooms/0/z_sectors", "0"),
        ("/rooms/0/quads/0/0", "999999"),
        ("/rooms/0/sectors/0/floor_info", "99999"),
        ("/models/0/joints/0/parent", "0"),
        ("/name", "123"),
        ("/name", "invalid json"),
    ] {
        assert!(doc.replace(pointer, text).is_err(), "{pointer}");
        assert!(!doc.is_dirty());
    }
    doc.replace("/rooms/0/sectors/18/floor", "-128").unwrap();
    doc.replace("/placements/1/position/0", "4700").unwrap();
    let mut external = bytes;
    external.push(b' ');
    fs::write(&path, &external).unwrap();
    assert!(doc.save().unwrap_err().contains("changed on disk"));
    assert_eq!(fs::read(&path).unwrap(), external);
    let copy = path.with_file_name("copy.json");
    let _ = fs::remove_file(&copy);
    doc.save_to(&copy).unwrap();
    assert!(!doc.is_dirty());
    let reopened = LevelDocument::open(&copy).unwrap();
    assert_eq!(reopened.level().unwrap().placements[1].position[0], 4700);
    assert_eq!(reopened.level().unwrap().rooms[0].sectors[18].floor, -128);
    assert!(doc.save_to(&path).is_err());
}

#[test]
fn registry_metadata_is_portable_and_never_installs_controller_hooks() {
    let names = engine::objects::ObjectTypes::names_from_json(
        r#"{"types":[{"id":7,"name":"walker","controller_specific":{"speed":3}},{"id":9,"name":"pillar"}]}"#,
    )
    .unwrap();
    assert_eq!(
        names.iter().map(|(id, t)| (id, t.name.as_str())).collect::<Vec<_>>(),
        vec![(7, "walker"), (9, "pillar")]
    );
    assert!(names.get(7).unwrap().spawn_hook().is_none());
    assert!(names.get(0).is_none());
}

#[test]
fn animation_links_are_validated_and_authored_edits_survive_save_reopen() {
    let path = fixture("animation");
    let mut doc = LevelDocument::open(&path).unwrap();
    assert!(doc.replace("/animations/0/next_animation", "999999").is_err());
    assert!(doc.replace("/animations/0/state_changes/0/animation", "999999").is_err());
    doc.replace("/animations/1/poses/1/root/1", "-384").unwrap();
    doc.save().unwrap();
    let reopened = LevelDocument::open(&path).unwrap();
    assert_eq!(reopened.level().unwrap().animations[1].poses[1].root[1], -384);
}

#[test]
fn multi_field_edits_are_one_validated_undo_step_and_can_add_optional_fields() {
    let path = fixture("multi");
    let bytes = fs::read(&path).unwrap();
    let mut doc = LevelDocument::open(&path).unwrap();
    let edits = |x: i64, room: i64| {
        vec![
            ("/placements/0/position/0".to_string(), serde_json::json!(x)),
            ("/placements/1/invisible".to_string(), serde_json::json!(true)),
            ("/placements/1/room".to_string(), serde_json::json!(room)),
        ]
    };
    // One invalid field rejects every change.
    assert!(doc.apply(edits(4096, 99999)).is_err());
    assert!(!doc.is_dirty());
    assert!(doc.value()["placements"][1].get("invisible").is_none());
    doc.apply(edits(4096, 0)).unwrap();
    assert_eq!(doc.value()["placements"][0]["position"][0], 4096);
    assert_eq!(doc.value()["placements"][1]["invisible"], true);
    assert!(doc.undo());
    assert!(!doc.can_undo());
    assert!(doc.value()["placements"][1].get("invisible").is_none(), "undo removes an added field");
    doc.save().unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(doc.redo());
    assert_eq!(doc.value()["placements"][1]["invisible"], true);
    assert!(doc.apply([("/no/such/table".to_string(), serde_json::json!(1))]).is_err());
}

#[test]
fn save_replacing_overwrites_a_confirmed_destination_atomically() {
    let path = fixture("replace");
    let mut doc = LevelDocument::open(&path).unwrap();
    let target = path.with_file_name("existing.json");
    fs::write(&target, "old contents").unwrap();
    assert!(doc.save_to(&target).is_err(), "plain Save As never replaces");
    assert_eq!(fs::read(&target).unwrap(), b"old contents");
    doc.replace("/name", "\"replaced\"").unwrap();
    doc.save_replacing(&target).unwrap();
    assert_eq!(doc.path(), fs::canonicalize(&target).unwrap());
    assert_eq!(LevelDocument::open(&target).unwrap().level().unwrap().name, "replaced");
    assert!(!doc.is_dirty());
}

#[test]
fn typed_level_edits_keep_unknown_fields_and_the_bytes_of_untouched_rooms() {
    let path = fixture("typed");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    // Two rooms in a compact file; the first carries a property the format does not know.
    let room = value["rooms"][0].clone();
    value["rooms"].as_array_mut().unwrap().push(room);
    value["rooms"][0]["sectors"][7]["note"] = serde_json::json!("keep me");
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    let mut doc = LevelDocument::open(&path).unwrap();
    let mut level = doc.level().unwrap();
    level.rooms[0].sectors[7].floor = -256;
    level.rooms[0].sectors[7].ceiling = -3072;
    level.rooms[0].vertices.push([0, 0, 0, 128]);
    doc.apply_level(&level).unwrap();
    assert_eq!(doc.value()["rooms"][0]["sectors"][7]["note"], "keep me");
    assert_eq!(doc.value()["rooms"][0]["sectors"][7]["floor"], -256);
    doc.save().unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    let untouched = serde_json::to_string(&value["rooms"][1]).unwrap();
    assert!(saved.contains(&untouched), "the untouched room keeps its exact text");
    assert!(!saved.contains('\n'), "a compact file stays compact");
    let reopened = LevelDocument::open(&path).unwrap();
    assert_eq!(reopened.level().unwrap().rooms[0].vertices.len(), level.rooms[0].vertices.len());
    assert!(doc.undo());
    assert!(doc.is_dirty(), "undoing past a save differs from the file");
}
