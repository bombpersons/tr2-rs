//! The supplied disc draws model254's first mesh with only its first authored
//! joint rotation, after zeroing view translation (0x80023F98..0x80023FF0).
//! Outdoor room flag8 enables it (0x80023ED0..0x80023EE0).
use bevy::{camera::visibility::RenderLayers, mesh::VertexAttributeValues, prelude::*};
use engine::{
    animation::model_pose,
    psx::PsxMaterial,
    render::{InRoom, LevelEntity, LevelLoaded, LevelSystems, backdrop, object_mesh, view::LEVEL_TO_RENDER},
    world::WorldGeometry,
};

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Sky {
    /// Developer switch for comparing the background with an original capture.
    pub enabled: bool,
}

pub struct SkyPlugin;
impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Sky>()
            .add_systems(Update, spawn.after(LevelSystems::Load).before(LevelSystems::Pose))
            .add_systems(Update, visibility.after(LevelSystems::Visibility));
    }
}

fn spawn(
    mut commands: Commands,
    mut loaded: MessageReader<LevelLoaded>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PsxMaterial>>,
) {
    for level in loaded.read() {
        let Some(model) = level.data.models.iter().find(|m| m.id == 254) else { continue };
        let Some(mesh) = model.meshes.first().and_then(|&i| level.data.meshes.get(i)) else { continue };
        let rotation = model_pose(&level.data, model, model.animations[0], 0.0)
            .first()
            .map_or(Quat::IDENTITY, |pose| pose.rotation);
        let mut built = object_mesh(mesh, &level.data.textures);
        // The sky renderer writes neutral-plus6 modulation to every vertex
        // (0x800676DC..0x800676F0). Its textured bands use134; the solid
        // caps retain their palette colour, as GPU capture commands7..15 show.
        if let Some(VertexAttributeValues::Float32x4(colors)) = built.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
            let shade = Color::srgb(134.0 / 128.0, 134.0 / 128.0, 134.0 / 128.0).to_linear().to_f32_array();
            for color in colors.iter_mut().take(mesh.quads.len() * 6) {
                *color = shade;
            }
        }
        let root = commands
            .spawn((
                Transform::from_scale(LEVEL_TO_RENDER),
                Visibility::default(),
                LevelEntity,
                Sky { enabled: true },
                Name::new("Sky"),
            ))
            .id();
        commands.spawn((
            Mesh3d(meshes.add(built)),
            MeshMaterial3d(materials.add(PsxMaterial {
                texture: Some(server.load(format!("levels/{}/atlas.png", level.name))),
                lit: false,
                fog: false,
                double_sided: true,
                ..default()
            })),
            Transform::from_rotation(rotation),
            RenderLayers::layer(backdrop::LAYER),
            backdrop::Backdrop,
            ChildOf(root),
            Name::new("SkyMesh"),
        ));
    }
}

fn visibility(
    world: Option<Res<WorldGeometry>>,
    rooms: Query<(&InRoom, &Visibility), Without<Sky>>,
    mut sky: Query<(&Sky, &mut Visibility)>,
) {
    let Some(world) = world else { return };
    let outdoors = rooms.iter().any(|(room, visible)| {
        *visible != Visibility::Hidden && world.data.rooms.get(room.0).is_some_and(|r| r.other_flags & 8 != 0)
    });
    for (sky, mut visible) in &mut sky {
        *visible = if sky.enabled && outdoors { Visibility::Inherited } else { Visibility::Hidden };
    }
}
