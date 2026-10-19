//! Camera-relative background geometry on an isolated render layer. Ordinary
//! world geometry draws afterward with a fresh depth buffer, so even a small
//! enclosing model remains behind the level. No world translation or fog is
//! applied by the background camera.
use bevy::{camera::visibility::RenderLayers, prelude::*};

use super::{GameCamera, LevelSystems, follow_camera};
use crate::psx::PsxView;

/// Put backdrop meshes on this layer and give at least one of them [`Backdrop`].
pub const LAYER: usize = 1;

/// A background mesh whose camera follows only the world camera's orientation.
/// Materials should opt out of fog and use the intended baked lighting.
#[derive(Component)]
pub struct Backdrop;

#[derive(Component)]
struct BackdropCamera;

pub(super) struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, sync.after(follow_camera).in_set(LevelSystems::Pose));
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera { order: -1, is_active: false, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Transform::default(),
        PsxView,
        RenderLayers::layer(LAYER),
        BackdropCamera,
        Name::new("BackdropCamera"),
    ));
}

#[allow(clippy::type_complexity)]
fn sync(
    backdrops: Query<(), With<Backdrop>>,
    mut world: Query<(&Transform, &Projection, &mut Camera), (With<GameCamera>, Without<BackdropCamera>)>,
    mut background: Query<(&mut Transform, &mut Projection, &mut Camera), (With<BackdropCamera>, Without<GameCamera>)>,
) {
    let (Ok((world_pose, world_projection, mut world_camera)), Ok((mut pose, mut projection, mut camera))) =
        (world.single_mut(), background.single_mut())
    else {
        return;
    };
    camera.is_active = !backdrops.is_empty();
    world_camera.clear_color =
        if camera.is_active { ClearColorConfig::None } else { ClearColorConfig::Custom(Color::BLACK) };
    *pose = Transform::from_rotation(world_pose.rotation);
    *projection = world_projection.clone();
    camera.sub_camera_view = world_camera.sub_camera_view.clone();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_preserves_orientation_and_crop_without_camera_translation_or_world_depth() {
        let mut app = App::new();
        app.add_systems(Update, sync);
        let rotation = Quat::from_rotation_y(1.2) * Quat::from_rotation_x(-0.4);
        let world = app
            .world_mut()
            .spawn((
                GameCamera,
                Transform::from_translation(Vec3::new(12.0, 7.0, 90.0)).with_rotation(rotation),
                Projection::Perspective(PerspectiveProjection { fov: 0.97, ..default() }),
                Camera {
                    sub_camera_view: Some(bevy::camera::SubCameraView {
                        full_size: UVec2::new(384, 240),
                        offset: Vec2::ZERO,
                        size: UVec2::new(368, 240),
                    }),
                    ..default()
                },
            ))
            .id();
        let background = app
            .world_mut()
            .spawn((BackdropCamera, Transform::default(), Projection::default(), Camera::default()))
            .id();
        app.update();
        assert!(!app.world().get::<Camera>(background).unwrap().is_active);
        let mesh = app.world_mut().spawn(Backdrop).id();
        app.update();
        let pose = app.world().get::<Transform>(background).unwrap();
        assert_eq!(pose.translation, Vec3::ZERO);
        assert!(pose.rotation.angle_between(rotation) < 0.0001);
        let camera = app.world().get::<Camera>(background).unwrap();
        assert!(camera.is_active);
        assert_eq!(camera.sub_camera_view.as_ref().unwrap().size, UVec2::new(368, 240));
        assert!(matches!(app.world().get::<Camera>(world).unwrap().clear_color, ClearColorConfig::None));
        let Projection::Perspective(projection) = app.world().get::<Projection>(background).unwrap() else { panic!() };
        assert_eq!(projection.fov, 0.97);
        app.world_mut().get_mut::<Transform>(world).unwrap().translation += Vec3::splat(100.0);
        app.update();
        assert_eq!(app.world().get::<Transform>(background).unwrap().translation, Vec3::ZERO);
        app.world_mut().despawn(mesh);
        app.update();
        assert!(!app.world().get::<Camera>(background).unwrap().is_active);
        assert!(matches!(app.world().get::<Camera>(world).unwrap().clear_color, ClearColorConfig::Custom(_)));
    }
}
