//! Per-mesh texture changes without changing a shared source material.
use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::psx::PsxMaterial;

/// Replace a mesh's texture while retaining its source material's current
/// lighting and render settings. `None`, or removing this component, restores
/// the source material. Other meshes sharing that material are unaffected.
#[derive(Component, Default)]
#[require(TextureSource)]
pub struct TextureOverride(pub Option<Handle<Image>>);

#[derive(Component, Default)]
pub(super) struct TextureSource {
    original: Option<Handle<PsxMaterial>>,
    output: Option<Handle<PsxMaterial>>,
}

type VariantKey = (AssetId<PsxMaterial>, AssetId<Image>);

#[derive(Resource, Default)]
pub(super) struct TextureVariants(HashMap<VariantKey, Handle<PsxMaterial>>);

pub(super) fn apply(
    mut meshes: Query<(&mut MeshMaterial3d<PsxMaterial>, &mut TextureSource, Option<&TextureOverride>)>,
    mut materials: ResMut<Assets<PsxMaterial>>,
    mut variants: ResMut<TextureVariants>,
) {
    let mut active = HashSet::new();
    for (mut binding, mut source, replacement) in &mut meshes {
        // Accept external material changes, including a new preview/skin.
        if source.output.as_ref() != Some(&binding.0) {
            source.original = Some(binding.0.clone());
        }
        let Some(original) = source.original.as_ref() else { continue };
        let next = if let Some(texture) = replacement.and_then(|r| r.0.as_ref()) {
            let key = (original.id(), texture.id());
            if active.insert(key) {
                let Some(mut material) = materials.get(original).cloned() else { continue };
                material.texture = Some(texture.clone());
                if let Some(handle) = variants.0.get(&key) {
                    // Copy from the source each frame: item lighting can move
                    // independently of the selected texture.
                    if let Some(mut target) = materials.get_mut(handle) {
                        *target = material;
                    }
                } else {
                    variants.0.insert(key, materials.add(material));
                }
            }
            let Some(handle) = variants.0.get(&key) else { continue };
            handle.clone()
        } else {
            original.clone()
        };
        if binding.0 != next {
            binding.0 = next.clone();
        }
        source.output = Some(next);
    }
    variants.0.retain(|key, _| active.contains(key));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_are_shared_but_keep_source_lighting_and_restore_on_removal() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<PsxMaterial>>()
            .init_resource::<TextureVariants>()
            .add_systems(Update, apply);
        let image = app.world_mut().resource_mut::<Assets<Image>>().add(Image::default());
        let source = app.world_mut().resource_mut::<Assets<PsxMaterial>>().add(PsxMaterial::default());
        let first = app.world_mut().spawn((MeshMaterial3d(source.clone()), TextureOverride(Some(image.clone())))).id();
        let second = app.world_mut().spawn((MeshMaterial3d(source.clone()), TextureOverride(Some(image.clone())))).id();
        let plain = app.world_mut().spawn(MeshMaterial3d(source.clone())).id();
        app.update();
        let output = app.world().get::<MeshMaterial3d<PsxMaterial>>(first).unwrap().0.clone();
        assert_ne!(output, source);
        assert_eq!(app.world().get::<MeshMaterial3d<PsxMaterial>>(second).unwrap().0, output);
        assert_eq!(app.world().get::<MeshMaterial3d<PsxMaterial>>(plain).unwrap().0, source);
        app.world_mut().resource_mut::<Assets<PsxMaterial>>().get_mut(&source).unwrap().lit = true;
        app.update();
        let materials = app.world().resource::<Assets<PsxMaterial>>();
        assert!(materials.get(&output).unwrap().lit);
        assert_eq!(materials.get(&output).unwrap().texture, Some(image));
        assert!(materials.get(&source).unwrap().texture.is_none());
        app.world_mut().entity_mut(first).remove::<TextureOverride>();
        app.world_mut().entity_mut(second).insert(TextureOverride(None));
        app.update();
        for entity in [first, second] {
            assert_eq!(app.world().get::<MeshMaterial3d<PsxMaterial>>(entity).unwrap().0, source);
        }
        assert!(app.world().resource::<TextureVariants>().0.is_empty());
    }
}
