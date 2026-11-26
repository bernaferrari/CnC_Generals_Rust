//! Source-mesh extraction for C++ `W3DTreeBuffer::addTreeType`.

use crate::assets::{AssetManager, W3dRenderObjectPrototypeKind};
use game_client::terrain::TreeTypeMesh;
use glam::{Mat4, Vec3};

/// Resolve the exact render-object prototype used by `W3DTreeDraw`, then
/// extract the direct mesh or the HLOD's first created child.
///
/// The source parser stores geometry in Main's Y-up basis and reverses winding
/// during import. This routine reverses both operations to reproduce the
/// source Z-up `MeshModelClass` arrays consumed by W3DTreeBuffer.
pub(super) fn load_native_tree_mesh(
    assets: &mut AssetManager,
    model_name: &str,
) -> Option<(TreeTypeMesh, Vec3, Option<f32>)> {
    let prototype = assets.resolve_w3d_render_object_prototype_blocking(model_name)?;
    let parent = assets.cached_w3d_render_object_source_model(&prototype)?;

    let (mesh_model, mesh_index, offset, shadow_size) = match prototype.kind() {
        W3dRenderObjectPrototypeKind::Mesh { mesh_index } => {
            let mesh = parent.meshes.get(mesh_index)?;
            let shadow_size = mesh.source_object_bounds.map(|bounds| {
                let size = bounds.max - bounds.min;
                (size.x + size.y) * 0.5
            });
            (parent, mesh_index, Vec3::ZERO, shadow_size)
        }
        W3dRenderObjectPrototypeKind::Hlod { hlod_index } => {
            let hlod = parent.hlods.get(hlod_index)?;
            let poses = parent.tree_hlod_source_bind_pose(hlod_index)?;
            let root = poses.first()?;
            let lods = hlod.lods.iter().flat_map(|lod| &lod.subobjects);
            let aggregates = hlod.aggregates.iter().flat_map(|array| &array.subobjects);
            // HLodClass::Get_Sub_Object(0) visits every LOD in source order
            // and then AdditionalModels. The constructor omits missing
            // prototypes, but successful non-mesh children still count.
            let children: Vec<_> = lods
                .map(|child| (child, poses.get(child.bone_index as usize).unwrap_or(root)))
                .chain(
                    aggregates
                        .filter_map(|child| Some((child, poses.get(child.bone_index as usize)?))),
                )
                .map(|(child, pose)| {
                    (
                        child.name.clone(),
                        Mat4::from_cols_array(pose).w_axis.truncate(),
                    )
                })
                .collect();
            let shadow_size = parent
                .constructor_projected_decal_size_xy(model_name, 1.0)
                .map(|size| (size.x + size.y) * 0.5);
            let (child, offset) = children.into_iter().find_map(|(name, offset)| {
                assets
                    .resolve_w3d_render_object_prototype_blocking(&name)
                    .map(|prototype| (prototype, offset))
            })?;
            let W3dRenderObjectPrototypeKind::Mesh { mesh_index } = child.kind() else {
                return None;
            };
            let child_model = assets.cached_w3d_render_object_source_model(&child)?;
            (child_model, mesh_index, offset, shadow_size)
        }
        _ => return None,
    };
    let mesh = mesh_model.meshes.get(mesh_index)?;
    if mesh.vertices.is_empty() || mesh.indices.len() % 3 != 0 {
        return None;
    }

    let positions: Vec<[f32; 3]> = mesh
        .vertices
        .iter()
        .map(|vertex| [vertex.position[0], vertex.position[2], vertex.position[1]])
        .collect();
    let normals = (!mesh.vertices.is_empty()).then(|| {
        mesh.vertices
            .iter()
            .map(|vertex| [vertex.normal[0], vertex.normal[2], vertex.normal[1]])
            .collect()
    });
    let uvs = mesh
        .stage_texcoords
        .first()
        .filter(|stage| stage.len() == mesh.vertices.len())
        .cloned()
        .unwrap_or_else(|| mesh.vertices.iter().map(|vertex| vertex.uv).collect());
    let colors = mesh.has_explicit_vertex_colors.then(|| {
        mesh.vertices
            .iter()
            .map(|vertex| {
                let channel = |index: usize| (vertex.color[index].clamp(0.0, 1.0) * 255.0) as u32;
                (channel(3) << 24) | (channel(0) << 16) | (channel(1) << 8) | channel(2)
            })
            .collect()
    });
    // W3DModel import reverses each triangle after its Y/Z swap. Undo that
    // winding conversion along with the coordinate conversion above.
    let polygons = mesh
        .indices
        .chunks_exact(3)
        .map(|triangle| {
            let a = u16::try_from(triangle[0]).ok()?;
            let b = u16::try_from(triangle[1]).ok()?;
            let c = u16::try_from(triangle[2]).ok()?;
            Some([a, c, b])
        })
        .collect::<Option<Vec<_>>>()?;

    Some((
        TreeTypeMesh {
            positions,
            normals,
            uvs,
            colors,
            polygons,
            emissive: mesh.material.emissive_color.to_array(),
        },
        offset,
        shadow_size,
    ))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::graphics::render_pipeline::{native_draw_world::NativeDrawWorld, pipeline_collect};
    use game_client::terrain::{TreeRandom, terrain_visual::TerrainVisualImpl};
    use gamelogic::object::draw::w3d_tree_draw::{TerrainTreeAddRequest, W3DTreeDrawModuleData};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct TreeRandomCalls(Vec<(f32, f32)>);

    impl TreeRandom for TreeRandomCalls {
        fn int_range(&mut self, min: i32, max: i32) -> i32 {
            self.0.push((min as f32, max as f32));
            min
        }

        fn real_range(&mut self, min: f32, max: f32) -> f32 {
            self.0.push((min, max));
            min
        }
    }

    #[test]
    fn mounted_native_dogwoods_share_authored_atlas_and_keep_distinct_hlod_meshes() {
        let retail = crate::fixture_content::retail_content_root()
            .join("Command & Conquer Generals Zero Hour/Data");
        let mut assets = AssetManager::new().unwrap();
        let archive = assets.archive_file_system_mut();
        archive.set_local_search_roots(Vec::new());
        for name in ["W3DZH.big", "TerrainZH.big"] {
            futures::executor::block_on(archive.load_big_file(retail.join(name))).unwrap();
        }
        let assets = Arc::new(Mutex::new(assets));
        let texture = "PTDogwod01.tga";
        let mut first_owner = NativeDrawWorld::default();
        let mut second_owner = NativeDrawWorld::default();
        for (owner, epoch, model, expected_vertices) in [
            (&mut first_owner, 41, "PTDogwodHI1", 132),
            (&mut second_owner, 42, "PTDogwodHI2", 88),
        ] {
            let mut terrain = TerrainVisualImpl::new();
            let mut failed = pipeline_collect::FailedModelResolutions::default();
            let mut installed =
                owner.borrow_installed_poses(&mut failed, 0, Some(Arc::clone(&assets)));
            installed.bind_native_prop_epoch(epoch);
            let request = TerrainTreeAddRequest {
                drawable_id: 7,
                position: glam::Vec3::new(5.0, 6.0, 4.0),
                angle: 0.75,
                scale: 1.25,
                module_data: W3DTreeDrawModuleData {
                    model_name: model.into(),
                    texture_name: texture.into(),
                    ..W3DTreeDrawModuleData::new()
                },
            };
            assert!(
                !installed
                    .pending_native_trees
                    .push(epoch - 1, request.clone())
            );
            assert!(installed.pending_native_trees.push(epoch, request));
            let mut random = TreeRandomCalls::default();
            installed.admit_pending_native_trees(&mut terrain, &mut random);
            assert_eq!(random.0.len(), 2, "original scale roll and sway type");
            assert_eq!(
                random.0[0],
                (1.0, 1.0),
                "map scale already lives on the Drawable"
            );
            let trees = terrain.tree_buffer_mut();
            assert_eq!(trees.trees().len(), 1);
            assert_eq!(trees.trees()[0].drawable_id, 7);
            assert_eq!(trees.trees()[0].location, Vec3::new(5.0, 6.0, 4.0));
            assert_eq!(trees.trees()[0].scale, 1.25);
            let source = &trees.tree_types()[0];
            assert_eq!(source.data.texture_name, texture);
            assert_eq!(source.data.model_name, model);
            assert_eq!(
                source.mesh.as_ref().unwrap().positions.len(),
                expected_vertices
            );
            trees.sync_tree_atlas_for_draw(0);
            assert_eq!(trees.num_tiles(), 1);
            assert!(!trees.atlas_upload_levels().is_empty());
        }
    }
}
