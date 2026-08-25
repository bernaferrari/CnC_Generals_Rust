//! Cache-only rendering of C++ aggregate and hierarchical render objects.
//!
//! `HLodClass` constructs every aggregate through
//! `WW3DAssetManager::Create_Render_Obj`, attaches it to one exact parent
//! HTree bone, and then renders it as an ordinary child render object.
//! `HModelPrototypeClass` follows the same exact-prototype path, but constructs
//! a one-LOD `HLodClass` whose rigid NODE/COLLISION_NODE children attach to the
//! HMODEL's own HTree. Main intentionally keeps the source prototype table
//! separate from the normal presentation-model cache, so this collector accepts
//! only already-resident exact source prototypes. Archive I/O belongs to the
//! bounded prewarm path; a missing child is never allowed to become an alias or
//! diagnostic mesh. A SKIN_NODE is admitted only after its exact HMODEL
//! bind-pose palette and complete source influence data are both proven; it
//! never borrows a parent or whole-file palette.

use super::*;
use crate::assets::{W3DMesh, W3DModel, W3dHlodAggregatePose, W3dRenderObjectPrototypeKind};
use crate::graphics::render_item::{RenderItemBonePaletteSource, RenderItemOwner};

/// Guard malformed aggregate graphs without changing the normal, shallow C++
/// object hierarchy.  The C++ source assumes trusted W3D data; Main must not
/// allow an accidental or hostile aggregate cycle to recurse forever during
/// rendering.
pub(super) const MAX_HLOD_AGGREGATE_RENDER_DEPTH: usize = 16;

/// Return only aggregate prototype identities that Main's bounded parent-HLOD
/// evaluator can actually attach in bind pose.  Prewarm uses this before a
/// frame reaches cache-only collection; malformed topology and invalid bones
/// therefore do not trigger unrelated archive requests.
pub(super) fn aggregate_prototype_names_for_prewarm(model: &W3DModel) -> Vec<String> {
    model
        .aggregate_attachment_poses_for_binding(None, 0.0)
        .into_iter()
        .flatten()
        .map(|pose| pose.name)
        .filter(|name| !name.is_empty())
        .collect()
}

/// Return every exact child identity an independently created HLOD prototype
/// can render in its constructor-selected bind pose.  This uses the registry's
/// immutable HLOD index rather than scanning the first HLOD in a source W3D
/// file, so a file with several HLOD chunks warms only the selected definition
/// and its own `AdditionalModels`.
pub(super) fn hlod_prototype_names_for_prewarm(model: &W3DModel, hlod_index: usize) -> Vec<String> {
    let Some(topology) = model.hlod_prototype_bind_pose(hlod_index) else {
        return Vec::new();
    };

    topology
        .selected_lod_children
        .into_iter()
        .chain(topology.additional_models)
        .map(|pose| pose.name)
        .filter(|name| !name.is_empty())
        .collect()
}

/// Return only the rigid exact prototype identities that one already-resolved
/// HMODEL can create in its default HTree pose. This shares the renderer's
/// malformed-node, named-HTree, and default-root rules, so the synchronous
/// prewarm lane cannot enqueue a child that frozen cache-only collection will
/// refuse to render. SKIN_NODE deliberately has no entry here.
pub(super) fn hmodel_rigid_node_names_for_prewarm(
    model: &W3DModel,
    hmodel_index: usize,
) -> Vec<String> {
    model
        .hmodel_rigid_node_poses(hmodel_index)
        .unwrap_or_default()
        .into_iter()
        .map(|pose| pose.name)
        .filter(|name| !name.is_empty())
        .collect()
}

/// A source token resolved by the strict C++-shaped prototype registry.
///
/// This deliberately carries an owned model snapshot.  The AssetManager lock
/// must be released before a renderer mutates its GPU-side model cache, and a
/// render item may only point at the private strict cache key derived from this
/// exact source record.
#[derive(Debug, Clone)]
struct ResolvedHlodAggregateRenderObject {
    /// Complete source prototype identity, retained for the recursion guard.
    prototype_name: String,
    /// GraphicsSystem-only key for the immutable strict source model.
    source_model_cache_key: String,
    kind: W3dRenderObjectPrototypeKind,
    source_model: W3DModel,
}

/// Common parent-bone attachment record for HLOD AdditionalModels and rigid
/// HMODEL connections. Both C++ paths call `Set_Transform` with a transform
/// local to their owning HTree; only their source ownership differs.
#[derive(Debug, Clone)]
struct RigidAttachmentPose {
    name: String,
    parent_transform: Mat4,
    visible: bool,
}

impl From<&W3dHlodAggregatePose> for RigidAttachmentPose {
    fn from(pose: &W3dHlodAggregatePose) -> Self {
        Self {
            name: pose.name.clone(),
            parent_transform: pose.parent_transform,
            visible: pose.visible,
        }
    }
}

/// Collect external aggregate children for one parent HLOD after its parent
/// HTree has sampled its frozen animation/control state.
///
/// This boundary is deliberately cache-only: it calls neither normal model
/// resolution nor the AssetManager's blocking prototype resolver.  An absent
/// record remains absent for this frame, exactly avoiding timing-dependent
/// archive I/O in render collection.
pub(super) fn collect_cached_hlod_aggregate_render_items(
    graphics_system: &mut GraphicsSystem,
    parent_item: &RenderItem,
    parent_attachment_poses: &[W3dHlodAggregatePose],
    camera_position: Vec3,
) -> Vec<RenderItem> {
    let mut resolve =
        |full_name: &str| resolve_cached_hlod_aggregate_render_object(graphics_system, full_name);
    collect_hlod_aggregate_render_items_with_resolver(
        parent_item,
        parent_item.world_matrix,
        parent_attachment_poses,
        camera_position,
        &mut resolve,
    )
}

/// Resolve only a resident exact source prototype and place its exact source
/// model in GraphicsSystem's private strict cache namespace.
fn resolve_cached_hlod_aggregate_render_object(
    graphics_system: &mut GraphicsSystem,
    full_name: &str,
) -> Option<ResolvedHlodAggregateRenderObject> {
    let asset_manager_arc = crate::assets::get_asset_manager()?;
    let asset_manager = asset_manager_arc.lock().ok()?;
    let prototype = asset_manager.cached_w3d_render_object_prototype(full_name)?;
    // A registry lookup is case-insensitive like C++ `Find_Prototype`, but
    // keep the requested complete identity honest before it reaches a draw.
    if !prototype.full_name().eq_ignore_ascii_case(full_name) {
        return None;
    }
    let source_model = asset_manager
        .cached_w3d_render_object_source_model(&prototype)?
        .clone();
    let source_model_cache_key = strict_render_object_model_cache_key(prototype.source_file_stem());
    let kind = prototype.kind();
    let prototype_name = prototype.full_name().to_string();
    drop(asset_manager);

    // This namespace is written only by this strict registry bridge.  The
    // model must remain the exact immutable source model paired with the token
    // above; no diagnostic-model lookup participates here.
    if graphics_system.get_model(&source_model_cache_key).is_none() {
        graphics_system.cache_model(source_model_cache_key.clone(), source_model.clone());
    }

    Some(ResolvedHlodAggregateRenderObject {
        prototype_name,
        source_model_cache_key,
        kind,
        source_model,
    })
}

/// Keep strict render-object geometry out of the normal presentation-model
/// cache namespace.  C++ source filenames are case-insensitive, so this key
/// deliberately follows the registry's ASCII-insensitive identity rule.
fn strict_render_object_model_cache_key(source_file_stem: &str) -> String {
    format!(
        "__strict_w3d_render_object_source__::{}",
        source_file_stem.to_ascii_lowercase()
    )
}

/// Generic core used by the cache-only runtime resolver and focused tests.
///
/// The resolver is deliberately injected: tests can build exact source model
/// graphs without touching a global AssetManager or a WGPU device, while the
/// production caller supplies only cached registry records.
fn collect_hlod_aggregate_render_items_with_resolver<F>(
    parent_item: &RenderItem,
    parent_world_matrix: Mat4,
    parent_attachment_poses: &[W3dHlodAggregatePose],
    camera_position: Vec3,
    resolve: &mut F,
) -> Vec<RenderItem>
where
    F: FnMut(&str) -> Option<ResolvedHlodAggregateRenderObject>,
{
    let mut output = Vec::new();
    let mut ancestor_prototype_names = HashSet::new();
    let parent_attachment_poses = parent_attachment_poses
        .iter()
        .map(RigidAttachmentPose::from)
        .collect::<Vec<_>>();
    collect_hlod_aggregate_render_items_recursive(
        parent_item,
        parent_world_matrix,
        &parent_attachment_poses,
        camera_position,
        0,
        &mut ancestor_prototype_names,
        resolve,
        &mut output,
    );
    output
}

#[allow(clippy::too_many_arguments)]
fn collect_hlod_aggregate_render_items_recursive<F>(
    parent_item: &RenderItem,
    parent_world_matrix: Mat4,
    parent_attachment_poses: &[RigidAttachmentPose],
    camera_position: Vec3,
    depth: usize,
    ancestor_prototype_names: &mut HashSet<String>,
    resolve: &mut F,
    output: &mut Vec<RenderItem>,
) where
    F: FnMut(&str) -> Option<ResolvedHlodAggregateRenderObject>,
{
    for pose in parent_attachment_poses {
        // `HLodClass::Update_Sub_Object_Transforms` sets the child animation
        // hidden bit from this exact parent bone.  Its `Render` then returns
        // before either mesh or nested aggregate work when hidden.
        if !pose.visible || !transform_is_reasonable_for_mesh(pose.parent_transform) {
            continue;
        }

        // C++ child `Set_Transform(HTree->Get_Transform(bone))` is local to
        // the parent RenderObj.  Main's RenderItem keeps the object root in
        // `world_matrix`, so compose parent root first, then parent HTree bone.
        let attachment_world = parent_world_matrix * pose.parent_transform;
        if !transform_is_reasonable_for_mesh(attachment_world) {
            continue;
        }

        let Some(resolved) = resolve(&pose.name) else {
            // One missing prototype must not hide the parent or a sibling.
            continue;
        };
        if !resolved.prototype_name.eq_ignore_ascii_case(&pose.name) {
            continue;
        }

        match resolved.kind {
            W3dRenderObjectPrototypeKind::Mesh { mesh_index } => {
                let Some(mesh) = resolved.source_model.meshes.get(mesh_index) else {
                    continue;
                };
                append_mesh_prototype_item(
                    parent_item,
                    &resolved.source_model_cache_key,
                    mesh_index,
                    mesh,
                    attachment_world,
                    Mat4::IDENTITY,
                    camera_position,
                    output,
                );
            }
            W3dRenderObjectPrototypeKind::Hlod { hlod_index } => {
                // `HLodPrototypeClass::Create` constructs exactly the indexed
                // source definition held by this registry token.  Never use a
                // whole-file mesh scan, HLOD zero, or name fallback here:
                // one W3D file can register multiple independent HLODs with
                // different HTree ownership and selected LOD topology.
                let Some(topology) = resolved.source_model.hlod_prototype_bind_pose(hlod_index)
                else {
                    continue;
                };

                let prototype_key = resolved.prototype_name.to_ascii_lowercase();
                if !ancestor_prototype_names.insert(prototype_key.clone()) {
                    // A cycle is not a valid finite C++ render tree.  Sibling
                    // references remain legal because this key is removed on
                    // return below.
                    continue;
                }

                // C++ `HLodClass::Render` emits constructor-selected LOD
                // objects first and then `AdditionalModels`. Both sets are
                // independently created exact prototypes beneath this HLOD's
                // own bind-pose HTree. They therefore pass through the same
                // strict resolver and recursion guard as a top-level
                // aggregate, rather than borrowing the source file's meshes.
                if depth < MAX_HLOD_AGGREGATE_RENDER_DEPTH {
                    let selected_lod_children = topology
                        .selected_lod_children
                        .iter()
                        .map(RigidAttachmentPose::from)
                        .collect::<Vec<_>>();
                    collect_hlod_aggregate_render_items_recursive(
                        parent_item,
                        attachment_world,
                        &selected_lod_children,
                        camera_position,
                        depth + 1,
                        ancestor_prototype_names,
                        resolve,
                        output,
                    );

                    let additional_models = topology
                        .additional_models
                        .iter()
                        .map(RigidAttachmentPose::from)
                        .collect::<Vec<_>>();
                    collect_hlod_aggregate_render_items_recursive(
                        parent_item,
                        attachment_world,
                        &additional_models,
                        camera_position,
                        depth + 1,
                        ancestor_prototype_names,
                        resolve,
                        output,
                    );
                }

                ancestor_prototype_names.remove(&prototype_key);
            }
            W3dRenderObjectPrototypeKind::Hmodel { hmodel_index } => {
                let prototype_key = resolved.prototype_name.to_ascii_lowercase();
                if !ancestor_prototype_names.insert(prototype_key.clone()) {
                    // HMODEL node connections use the same source prototype
                    // registry as HLOD AdditionalModels. A cycle is not a
                    // finite C++ render tree; keep siblings independent.
                    continue;
                }

                // `HModelPrototypeClass::Create` constructs a one-LOD HLOD
                // and attaches every source NODE/COLLISION_NODE at the
                // HMODEL's own default HTree pivot. Its connections can in
                // turn resolve to meshes, HLODs, or other HMODELs. Keep the
                // existing bounded recursion rule for rigid children.
                if depth < MAX_HLOD_AGGREGATE_RENDER_DEPTH {
                    if let Some(child_poses) =
                        resolved.source_model.hmodel_rigid_node_poses(hmodel_index)
                    {
                        let child_poses = child_poses
                            .into_iter()
                            .map(|pose| RigidAttachmentPose {
                                name: pose.name,
                                parent_transform: pose.parent_transform,
                                // A freshly constructed HMODEL HTree has no
                                // selected HAnim; its valid rigid nodes are
                                // visible in the default pose.
                                visible: true,
                            })
                            .collect::<Vec<_>>();
                        collect_hlod_aggregate_render_items_recursive(
                            parent_item,
                            attachment_world,
                            &child_poses,
                            camera_position,
                            depth + 1,
                            ancestor_prototype_names,
                            resolve,
                            output,
                        );
                    }

                    // A C++ SKIN_NODE is also a child MeshClass, but skin
                    // deformation reads its *container* HTree rather than
                    // the connection bone as an outer transform. Main uses
                    // the HMODEL attachment root once and identifies the
                    // separately owned palette explicitly on the child item.
                    // Do not recurse through a non-Mesh token: a malformed
                    // skin connection must fail closed rather than become a
                    // rigid HLOD/HMODEL attachment.
                    let Some(hmodel_palette) =
                        resolved.source_model.hmodel_bind_pose_palette(hmodel_index)
                    else {
                        ancestor_prototype_names.remove(&prototype_key);
                        continue;
                    };
                    let Some(skin_nodes) = resolved
                        .source_model
                        .hmodel_skin_node_bindings(hmodel_index)
                    else {
                        ancestor_prototype_names.remove(&prototype_key);
                        continue;
                    };
                    for skin_node in skin_nodes {
                        let Some(skin_resolved) = resolve(&skin_node.name) else {
                            continue;
                        };
                        if !skin_resolved
                            .prototype_name
                            .eq_ignore_ascii_case(&skin_node.name)
                        {
                            continue;
                        }
                        let W3dRenderObjectPrototypeKind::Mesh { mesh_index } = skin_resolved.kind
                        else {
                            continue;
                        };
                        let Some(skin_mesh) = skin_resolved.source_model.meshes.get(mesh_index)
                        else {
                            continue;
                        };
                        if !skin_mesh.has_complete_skin_influences_for_palette(hmodel_palette.len())
                        {
                            continue;
                        }
                        append_hmodel_skin_mesh_prototype_item(
                            parent_item,
                            &skin_resolved.source_model_cache_key,
                            mesh_index,
                            skin_mesh,
                            attachment_world,
                            &resolved.source_model_cache_key,
                            hmodel_index,
                            camera_position,
                            output,
                        );
                    }
                }

                ancestor_prototype_names.remove(&prototype_key);
            }
            W3dRenderObjectPrototypeKind::Collection { collection_index } => {
                let Some(collection) = resolved.source_model.collections.get(collection_index)
                else {
                    continue;
                };
                if depth >= MAX_HLOD_AGGREGATE_RENDER_DEPTH {
                    continue;
                }
                let child_poses = collection
                    .object_names
                    .iter()
                    .map(|name| RigidAttachmentPose {
                        name: name.clone(),
                        parent_transform: Mat4::IDENTITY,
                        visible: true,
                    })
                    .collect::<Vec<_>>();
                collect_hlod_aggregate_render_items_recursive(
                    parent_item,
                    attachment_world,
                    &child_poses,
                    camera_position,
                    depth + 1,
                    ancestor_prototype_names,
                    resolve,
                    output,
                );
            }
            W3dRenderObjectPrototypeKind::Aggregate { aggregate_index } => {
                let Some(aggregate) = resolved.source_model.aggregates.get(aggregate_index) else {
                    continue;
                };
                if depth >= MAX_HLOD_AGGREGATE_RENDER_DEPTH {
                    continue;
                }
                let mut names = Vec::new();
                if !aggregate.base_model_name.is_empty() {
                    names.push(aggregate.base_model_name.clone());
                }
                names.extend(
                    aggregate
                        .subobjects
                        .iter()
                        .map(|sub| sub.subobject_name.clone()),
                );
                let child_poses = names
                    .into_iter()
                    .map(|name| RigidAttachmentPose {
                        name,
                        parent_transform: Mat4::IDENTITY,
                        visible: true,
                    })
                    .collect::<Vec<_>>();
                collect_hlod_aggregate_render_items_recursive(
                    parent_item,
                    attachment_world,
                    &child_poses,
                    camera_position,
                    depth + 1,
                    ancestor_prototype_names,
                    resolve,
                    output,
                );
            }
            W3dRenderObjectPrototypeKind::DistLod { dist_lod_index } => {
                let Some(dist_lod) = resolved.source_model.dist_lods.get(dist_lod_index) else {
                    continue;
                };
                let Some(first) = dist_lod.lods.first() else {
                    continue;
                };
                if depth >= MAX_HLOD_AGGREGATE_RENDER_DEPTH {
                    continue;
                }
                let child_poses = vec![RigidAttachmentPose {
                    name: first.render_obj_name.clone(),
                    parent_transform: Mat4::IDENTITY,
                    visible: true,
                }];
                collect_hlod_aggregate_render_items_recursive(
                    parent_item,
                    attachment_world,
                    &child_poses,
                    camera_position,
                    depth + 1,
                    ancestor_prototype_names,
                    resolve,
                    output,
                );
            }
            W3dRenderObjectPrototypeKind::Box { box_index } => {
                // CLASSID_OBBOX BOUNDINGBOX is instantiated but never drawn.
                let _ = resolved.source_model.boxes.get(box_index);
            }
            W3dRenderObjectPrototypeKind::Emitter { .. }
            | W3dRenderObjectPrototypeKind::Dazzle { .. }
            | W3dRenderObjectPrototypeKind::Ring { .. }
            | W3dRenderObjectPrototypeKind::Sphere { .. }
            | W3dRenderObjectPrototypeKind::Null { .. } => {
                // Instantiated as an HLOD child. Particle/dazzle/primitive
                // submit lives on those render objects, not the mesh path.
            }
        }
    }
}

/// Emit one source `MeshClass` prototype.  A mesh prototype starts with the
/// RenderObj identity transform in C++; Main's `W3DMesh::transform` can carry
/// a separate HMODEL residual for whole-file rendering, so it must not leak
/// into an independently created `MeshClass` aggregate.
#[allow(clippy::too_many_arguments)]
fn append_mesh_prototype_item(
    parent_item: &RenderItem,
    source_model_cache_key: &str,
    mesh_index: usize,
    mesh: &W3DMesh,
    world_matrix: Mat4,
    mesh_local_transform: Mat4,
    camera_position: Vec3,
    output: &mut Vec<RenderItem>,
) {
    if !transform_is_reasonable_for_mesh(mesh_local_transform) {
        return;
    }
    let world_position = world_matrix.w_axis.truncate();
    if !world_position.is_finite() {
        return;
    }
    let mut item = aggregate_render_item_from_parent(
        parent_item,
        source_model_cache_key,
        mesh_index,
        &mesh.material,
        world_position,
        world_matrix,
    );
    item.set_mesh_local_transform(mesh_local_transform);
    item.apply_house_color_livery(&mesh.name);
    item.distance = world_position.distance(camera_position);
    output.push(item);
}

/// Emit one exact HMODEL `SKIN_NODE` MeshClass.
///
/// The source mesh has already proven a complete per-vertex influence table
/// against the HMODEL palette. Unlike a rigid HMODEL child, the node's pivot
/// is not multiplied into `world_matrix`: C++ `MeshClass::Get_Deformed_Vertices`
/// deforms against the container HTree and DX8 then renders those world-space
/// vertices with identity. WGPU performs the equivalent in the opposite
/// split—local HMODEL palette first, then this one outer attachment root.
#[allow(clippy::too_many_arguments)]
fn append_hmodel_skin_mesh_prototype_item(
    parent_item: &RenderItem,
    source_model_cache_key: &str,
    mesh_index: usize,
    mesh: &W3DMesh,
    hmodel_attachment_world: Mat4,
    palette_source_model_cache_key: &str,
    hmodel_index: usize,
    camera_position: Vec3,
    output: &mut Vec<RenderItem>,
) {
    if source_model_cache_key.is_empty()
        || source_model_cache_key.as_bytes().contains(&0)
        || palette_source_model_cache_key.is_empty()
        || palette_source_model_cache_key.as_bytes().contains(&0)
        || !transform_is_reasonable_for_mesh(hmodel_attachment_world)
    {
        return;
    }
    let world_position = hmodel_attachment_world.w_axis.truncate();
    if !world_position.is_finite() {
        return;
    }

    let mut item = aggregate_render_item_from_parent(
        parent_item,
        source_model_cache_key,
        mesh_index,
        &mesh.material,
        world_position,
        hmodel_attachment_world,
    );
    // A standalone `MeshClass` prototype has no whole-file residual. Keep
    // identity here as well so the HMODEL skin node bone cannot be applied a
    // second time after GPU deformation.
    item.set_mesh_local_transform(Mat4::IDENTITY);
    item.bone_palette_source = RenderItemBonePaletteSource::HmodelBindPose {
        source_model_cache_key: palette_source_model_cache_key.to_string(),
        hmodel_index,
    };
    item.distance = world_position.distance(camera_position);
    item.apply_house_color_livery(&mesh.name);
    output.push(item);
}

/// Build a normal RenderItem while preserving the parent Drawable's frozen
/// ownership and presentation-visual state. Aggregates are render-object
/// children, not new gameplay objects or unbound drawables.
fn aggregate_render_item_from_parent(
    parent_item: &RenderItem,
    source_model_cache_key: &str,
    mesh_index: usize,
    material: &W3DMaterial,
    world_position: Vec3,
    world_matrix: Mat4,
) -> RenderItem {
    let mut item = match parent_item.owner {
        RenderItemOwner::Object(object_id) => RenderItem::new(
            object_id,
            source_model_cache_key.to_string(),
            mesh_index,
            world_position,
            world_matrix,
            material,
            RenderPipeline::render_pass_for_material(material),
        ),
        RenderItemOwner::PresentationProjectile(projectile_id) => {
            RenderItem::new_presentation_projectile(
                projectile_id,
                source_model_cache_key.to_string(),
                mesh_index,
                world_position,
                world_matrix,
                material,
                RenderPipeline::render_pass_for_material(material),
            )
        }
        RenderItemOwner::UnboundClientDrawable(drawable_id) => {
            RenderItem::new_unbound_client_drawable(
                drawable_id,
                source_model_cache_key.to_string(),
                mesh_index,
                world_position,
                world_matrix,
                material,
                RenderPipeline::render_pass_for_material(material),
            )
        }
        // Ghost HLOD children require their own exact snapshot/materializer;
        // the ordinary aggregate helper must never invent one from a child
        // asset.  Keep this path fail-closed until the dedicated ghost
        // consumer owns HLOD state.
        RenderItemOwner::W3dGhost(_) => return parent_item.clone(),
    };
    item.copy_frozen_presentation_visuals_from(parent_item);
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{
        W3DLoader, W3DMaterial, W3DMesh, W3dHierarchy, W3dHlod, W3dHlodAttachmentArray, W3dHlodLod,
        W3dHlodSubObject, W3dHmodel, W3dHmodelNode, W3dHmodelNodeKind, W3dPivot,
    };
    use crate::fow_rendering::ObjectVisibility;
    use crate::game_logic::ObjectId;
    use std::collections::HashMap;

    fn pivot(name: &str, parent_idx: u32, translation: [f32; 3]) -> W3dPivot {
        W3dPivot {
            name: name.to_string(),
            parent_idx,
            translation,
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn parent_item() -> RenderItem {
        RenderItem::new(
            ObjectId(31),
            "PARENT".to_string(),
            0,
            Vec3::new(5.0, 0.0, 0.0),
            Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)),
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        )
    }

    fn direct_mesh_model(name: &str, mesh_name: &str) -> W3DModel {
        let mut model = W3DModel::new(name.to_string());
        let mut mesh = W3DMesh::new(mesh_name.to_string());
        // This is a whole-file residual in Main. A direct C++ Mesh prototype
        // must start at identity below the parent aggregate bone instead.
        mesh.transform = Mat4::from_translation(Vec3::new(99.0, 0.0, 0.0));
        model.meshes.push(mesh);
        model
    }

    fn single_hlod_model(
        name: &str,
        hlod_name: &str,
        mesh_name: &str,
        child_translation_x: f32,
        aggregates: Vec<W3dHlodSubObject>,
    ) -> W3DModel {
        let mut model = W3DModel::new(name.to_string());
        let hierarchy = W3dHierarchy {
            name: format!("{hlod_name}_HIER"),
            pivots: vec![
                pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]),
                pivot("ATTACH", 0, [child_translation_x, 0.0, 0.0]),
            ],
            pivot_fixups: Vec::new(),
        };
        model.hierarchies.push(hierarchy.clone());
        model.hierarchy = Some(hierarchy);
        model.hlods.push(W3dHlod {
            version: 0x0001_0000,
            name: hlod_name.to_string(),
            hierarchy_name: format!("{hlod_name}_HIER"),
            lods: vec![W3dHlodLod {
                max_screen_size: f32::MAX,
                subobjects: vec![W3dHlodSubObject {
                    name: format!("{hlod_name}.{mesh_name}"),
                    bone_index: 1,
                }],
            }],
            aggregates: (!aggregates.is_empty()).then_some(W3dHlodAttachmentArray {
                max_screen_size: f32::MAX,
                subobjects: aggregates,
            }),
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        let mut mesh = W3DMesh::new(mesh_name.to_string());
        mesh.container_name = hlod_name.to_string();
        model.meshes.push(mesh);
        model
    }

    fn hmodel_source_model(
        hmodel_name: &str,
        nodes: Vec<W3dHmodelNode>,
        mesh_names: &[&str],
    ) -> W3DModel {
        let mut model = W3DModel::new(format!("{hmodel_name}_SOURCE"));
        let hierarchy = W3dHierarchy {
            name: format!("{hmodel_name}_TREE"),
            pivots: vec![
                pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]),
                pivot("HMODEL_BONE", 0, [2.0, 0.0, 0.0]),
            ],
            pivot_fixups: Vec::new(),
        };
        // HMODEL must resolve this retained source record rather than infer a
        // palette from the convenience whole-model field.
        model.hierarchies.push(hierarchy.clone());
        model.hierarchy = Some(hierarchy.clone());
        model.hmodels.push(W3dHmodel {
            version: 0x0004_0002,
            name: hmodel_name.to_string(),
            hierarchy_name: hierarchy.name,
            nodes,
            source_snap_points: Vec::new(),
            has_invalid_records: false,
        });
        model.meshes.extend(mesh_names.iter().map(|name| {
            let mut mesh = W3DMesh::new((*name).to_string());
            // A direct Mesh prototype begins at identity below the HMODEL
            // HTree pivot; this residual must not leak through the resolver.
            mesh.transform = Mat4::from_translation(Vec3::new(99.0, 0.0, 0.0));
            mesh
        }));
        model
    }

    fn resolved(
        prototype_name: &str,
        source_model_cache_key: &str,
        kind: W3dRenderObjectPrototypeKind,
        source_model: W3DModel,
    ) -> ResolvedHlodAggregateRenderObject {
        ResolvedHlodAggregateRenderObject {
            prototype_name: prototype_name.to_string(),
            source_model_cache_key: source_model_cache_key.to_string(),
            kind,
            source_model,
        }
    }

    #[test]
    fn aggregate_mesh_uses_exact_cached_prototype_and_parent_world_then_bone_order() {
        let parent = parent_item();
        let poses = [W3dHlodAggregatePose {
            name: "ATTACHED.Body".to_string(),
            bone_index: 1,
            parent_transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            visible: true,
        }];
        let source = direct_mesh_model("ATTACHED", "Body");
        let mut resolve = |name: &str| {
            name.eq_ignore_ascii_case("ATTACHED.Body").then(|| {
                resolved(
                    "ATTACHED.Body",
                    "__strict_w3d_render_object_source__::attached",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    source.clone(),
                )
            })
        };

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(
            item.model_name,
            "__strict_w3d_render_object_source__::attached"
        );
        assert_eq!(item.mesh_index, 0);
        assert_eq!(item.world_matrix.w_axis.x, 15.0);
        assert_eq!(item.mesh_local_transform, Mat4::IDENTITY);
        assert_eq!(
            (item.world_matrix * item.mesh_local_transform).w_axis.x,
            15.0,
            "a direct MeshClass prototype must not inherit Main's whole-file HMODEL residual"
        );
        assert_eq!(item.owner, parent.owner);
        assert_eq!(item.fow_visibility, parent.fow_visibility);
    }

    #[test]
    fn aggregate_item_preserves_presentation_projectile_ownership() {
        let projectile_id = ObjectId(91);
        let parent = RenderItem::new_presentation_projectile(
            projectile_id,
            "PROJECTILE_PARENT".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );

        let aggregate = aggregate_render_item_from_parent(
            &parent,
            "PROJECTILE_ATTACHMENT",
            0,
            &W3DMaterial::default(),
            Vec3::ZERO,
            Mat4::IDENTITY,
        );

        assert_eq!(aggregate.object_id, projectile_id);
        assert_eq!(
            aggregate.owner,
            RenderItemOwner::PresentationProjectile(projectile_id)
        );
        assert_eq!(aggregate.frozen_direct_scene_shroud, None);
    }

    #[test]
    fn aggregate_mesh_replays_the_parent_frozen_selection_poison_and_fow_visuals() {
        let frozen_fow = ObjectVisibility {
            visibility_alpha: 0.42,
            is_explored: 1.0,
            visibility_falloff: 0.75,
        };
        let selection_intensity = 0.6;
        let team_color = [0.1, 0.3, 0.8, 1.0];

        // This represents a normal source mesh built at the direct unit
        // collector boundary. Its material and the attached prototype use the
        // same baseline so equality proves modifiers are replayed, not copied
        // as a wrong parent material.
        let mut normal_item = RenderItem::new(
            ObjectId(31),
            "PARENT".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        normal_item.apply_frozen_presentation_visuals(
            frozen_fow,
            selection_intensity,
            team_color,
            true,
            [1.0, 1.0, 1.0, 1.0],
        );

        let mut aggregate_parent = parent_item();
        aggregate_parent.apply_frozen_presentation_visuals(
            frozen_fow,
            selection_intensity,
            team_color,
            true,
            [1.0, 1.0, 1.0, 1.0],
        );

        let poses = [W3dHlodAggregatePose {
            name: "ATTACHED.Body".to_string(),
            bone_index: 1,
            parent_transform: Mat4::IDENTITY,
            visible: true,
        }];
        let source = direct_mesh_model("ATTACHED", "Body");
        let mut resolve = |name: &str| {
            name.eq_ignore_ascii_case("ATTACHED.Body").then(|| {
                resolved(
                    "ATTACHED.Body",
                    "__strict_w3d_render_object_source__::attached",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    source.clone(),
                )
            })
        };

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &aggregate_parent,
            aggregate_parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(
            items.len(),
            1,
            "this pure regression needs no fallback mesh"
        );
        let aggregate_item = &items[0];
        assert_eq!(aggregate_item.fow_visibility, normal_item.fow_visibility);
        assert_eq!(
            aggregate_item.selection_flash_intensity,
            normal_item.selection_flash_intensity
        );
        assert_eq!(aggregate_item.poison_tinted, normal_item.poison_tinted);
        assert_eq!(
            aggregate_item.material.diffuse_color,
            normal_item.material.diffuse_color
        );
        assert_eq!(
            aggregate_item.material.emissive_color,
            normal_item.material.emissive_color
        );
    }

    #[test]
    fn aggregate_hmodel_skips_skin_without_exact_influences_without_affecting_rigid_nodes() {
        let parent = parent_item();
        let poses = [W3dHlodAggregatePose {
            name: "RIG_HMODEL".to_string(),
            bone_index: 1,
            parent_transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            visible: true,
        }];
        let hmodel = hmodel_source_model(
            "RIG_HMODEL",
            vec![
                W3dHmodelNode {
                    name: "RIG_HMODEL.Body".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::Node,
                },
                W3dHmodelNode {
                    name: "RIG_HMODEL.Collision".to_string(),
                    bone_index: 0,
                    kind: W3dHmodelNodeKind::CollisionNode,
                },
                W3dHmodelNode {
                    name: "RIG_HMODEL.Skin".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::SkinNode,
                },
                W3dHmodelNode {
                    name: "RIG_HMODEL.BadPivot".to_string(),
                    bone_index: 99,
                    kind: W3dHmodelNodeKind::Node,
                },
            ],
            &["Body", "Collision", "Skin", "BadPivot"],
        );
        let sources = HashMap::from([
            (
                "rig_hmodel".to_string(),
                resolved(
                    "RIG_HMODEL",
                    "__strict_w3d_render_object_source__::rig_hmodel",
                    W3dRenderObjectPrototypeKind::Hmodel { hmodel_index: 0 },
                    hmodel.clone(),
                ),
            ),
            (
                "rig_hmodel.body".to_string(),
                resolved(
                    "RIG_HMODEL.Body",
                    "__strict_w3d_render_object_source__::rig_hmodel",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    hmodel.clone(),
                ),
            ),
            (
                "rig_hmodel.collision".to_string(),
                resolved(
                    "RIG_HMODEL.Collision",
                    "__strict_w3d_render_object_source__::rig_hmodel",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 1 },
                    hmodel.clone(),
                ),
            ),
            // The exact skin Mesh token exists, but this fixture deliberately
            // has no parsed per-vertex influence table. It must fail closed
            // without changing the valid rigid sibling path.
            (
                "rig_hmodel.skin".to_string(),
                resolved(
                    "RIG_HMODEL.Skin",
                    "__strict_w3d_render_object_source__::rig_hmodel",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 2 },
                    hmodel.clone(),
                ),
            ),
            (
                "rig_hmodel.badpivot".to_string(),
                resolved(
                    "RIG_HMODEL.BadPivot",
                    "__strict_w3d_render_object_source__::rig_hmodel",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 3 },
                    hmodel.clone(),
                ),
            ),
        ]);
        let mut resolve = |name: &str| sources.get(&name.to_ascii_lowercase()).cloned();

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(items.len(), 2, "only valid rigid siblings may render");
        assert!(items.iter().any(|item| {
            item.mesh_index == 0 && (item.world_matrix * item.mesh_local_transform).w_axis.x == 17.0
        }));
        assert!(items.iter().any(|item| {
            item.mesh_index == 1 && (item.world_matrix * item.mesh_local_transform).w_axis.x == 15.0
        }));
        assert!(items.iter().all(|item| item.mesh_index < 2));
    }

    #[test]
    fn aggregate_hmodel_skin_uses_its_outer_attachment_once_and_own_palette_source() {
        fn source_chunk(chunk_type: u32, payload: Vec<u8>, container: bool) -> Vec<u8> {
            let mut output = Vec::with_capacity(8 + payload.len());
            output.extend_from_slice(&chunk_type.to_le_bytes());
            let raw_size = (payload.len() as u32) | if container { 0x8000_0000 } else { 0 };
            output.extend_from_slice(&raw_size.to_le_bytes());
            output.extend_from_slice(&payload);
            output
        }

        fn fixed_source_name(name: &str) -> Vec<u8> {
            let mut output = vec![0; 16];
            let bytes = name.as_bytes();
            let len = bytes.len().min(output.len());
            output[..len].copy_from_slice(&bytes[..len]);
            output
        }

        fn parsed_skin_mesh_source() -> W3DModel {
            let mut header = vec![0; 116];
            header[0..4].copy_from_slice(&0x0004_0002u32.to_le_bytes());
            header[8..24].copy_from_slice(&fixed_source_name("Skin"));
            header[40..44].copy_from_slice(&1u32.to_le_bytes());
            header[44..48].copy_from_slice(&1u32.to_le_bytes());

            let mut triangle = Vec::with_capacity(32);
            triangle.extend_from_slice(&0u32.to_le_bytes());
            triangle.extend_from_slice(&0u32.to_le_bytes());
            triangle.extend_from_slice(&0u32.to_le_bytes());
            triangle.extend_from_slice(&[0; 20]);
            let mut influence = Vec::with_capacity(8);
            influence.extend_from_slice(&1u16.to_le_bytes());
            influence.extend_from_slice(&[9, 8, 7, 6, 5, 4]);
            let mut vertices = Vec::with_capacity(12);
            for value in [0.0f32, 0.0, 0.0] {
                vertices.extend_from_slice(&value.to_le_bytes());
            }

            let mesh = source_chunk(
                0x0000_0000,
                [
                    source_chunk(0x0000_001F, header, false),
                    source_chunk(0x0000_0002, vertices, false),
                    source_chunk(0x0000_0020, triangle, false),
                    source_chunk(0x0000_000E, influence, false),
                ]
                .concat(),
                true,
            );
            W3DLoader::new()
                .load_model_from_bytes(&mesh, "skin_source_from_parser")
                .expect("the source-shaped W3D skin mesh should parse")
        }

        let parent = parent_item();
        let poses = [W3dHlodAggregatePose {
            name: "SKIN_HMODEL".to_string(),
            bone_index: 1,
            parent_transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            visible: true,
        }];
        let hmodel = hmodel_source_model(
            "SKIN_HMODEL",
            vec![
                W3dHmodelNode {
                    name: "SKIN_HMODEL.Rigid".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::Node,
                },
                W3dHmodelNode {
                    name: "SKIN_HMODEL.Skin".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::SkinNode,
                },
            ],
            &["Rigid"],
        );
        let skin_mesh_source = parsed_skin_mesh_source();
        let parsed_influences = skin_mesh_source.meshes[0]
            .vertex_influences
            .as_ref()
            .expect("the aggregate path must receive parser-retained source links");
        assert_eq!(parsed_influences.len(), 1);
        assert_eq!(parsed_influences[0].bone_idx, 1);
        assert_eq!(parsed_influences[0].pad, [9, 8, 7, 6, 5, 4]);

        let sources = HashMap::from([
            (
                "skin_hmodel".to_string(),
                resolved(
                    "SKIN_HMODEL",
                    "__strict_w3d_render_object_source__::skin_hmodel",
                    W3dRenderObjectPrototypeKind::Hmodel { hmodel_index: 0 },
                    hmodel.clone(),
                ),
            ),
            (
                "skin_hmodel.rigid".to_string(),
                resolved(
                    "SKIN_HMODEL.Rigid",
                    "__strict_w3d_render_object_source__::skin_hmodel",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    hmodel,
                ),
            ),
            (
                "skin_hmodel.skin".to_string(),
                resolved(
                    "SKIN_HMODEL.Skin",
                    "__strict_w3d_render_object_source__::skin_mesh",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    skin_mesh_source,
                ),
            ),
        ]);
        let mut resolve = |name: &str| sources.get(&name.to_ascii_lowercase()).cloned();

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|item| {
            item.model_name == "__strict_w3d_render_object_source__::skin_hmodel"
                && item.mesh_index == 0
                && (item.world_matrix * item.mesh_local_transform).w_axis.x == 17.0
                && item.bone_palette_source == RenderItemBonePaletteSource::FrozenDrawState
        }));

        let skin_item = items
            .iter()
            .find(|item| item.model_name == "__strict_w3d_render_object_source__::skin_mesh")
            .expect("the exact skinned mesh token should render");
        assert_eq!(skin_item.mesh_local_transform, Mat4::IDENTITY);
        assert_eq!(skin_item.world_matrix.w_axis.x, 15.0);
        assert_eq!(
            skin_item.bone_palette_source,
            RenderItemBonePaletteSource::HmodelBindPose {
                source_model_cache_key: "__strict_w3d_render_object_source__::skin_hmodel"
                    .to_string(),
                hmodel_index: 0,
            },
            "the mesh source may differ, but the palette must stay on the exact HMODEL"
        );
        assert_ne!(
            skin_item.world_matrix.w_axis.x, 17.0,
            "the SKIN_NODE connection bone must not be applied as a second outer transform"
        );
    }

    #[test]
    fn aggregate_hmodel_recurses_through_exact_children_and_cuts_cycles() {
        let parent = parent_item();
        let poses = [W3dHlodAggregatePose {
            name: "TOP".to_string(),
            bone_index: 0,
            parent_transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            visible: true,
        }];
        let mut source = hmodel_source_model(
            "TOP",
            vec![W3dHmodelNode {
                name: "TOP.NESTED".to_string(),
                bone_index: 0,
                kind: W3dHmodelNodeKind::Node,
            }],
            &["Body"],
        );
        source.hmodels.push(W3dHmodel {
            version: 0x0004_0002,
            name: "TOP.NESTED".to_string(),
            // C++ falls back to its one-root default HTree for this missing
            // tree, so both authored connection pivots are valid root zero.
            hierarchy_name: "MISSING_NESTED_TREE".to_string(),
            nodes: vec![
                W3dHmodelNode {
                    name: "TOP.NESTED.Body".to_string(),
                    bone_index: 0,
                    kind: W3dHmodelNodeKind::Node,
                },
                W3dHmodelNode {
                    name: "TOP".to_string(),
                    bone_index: 0,
                    kind: W3dHmodelNodeKind::Node,
                },
            ],
            source_snap_points: Vec::new(),
            has_invalid_records: false,
        });
        let sources = HashMap::from([
            (
                "top".to_string(),
                resolved(
                    "TOP",
                    "__strict_w3d_render_object_source__::top",
                    W3dRenderObjectPrototypeKind::Hmodel { hmodel_index: 0 },
                    source.clone(),
                ),
            ),
            (
                "top.nested".to_string(),
                resolved(
                    "TOP.NESTED",
                    "__strict_w3d_render_object_source__::top",
                    W3dRenderObjectPrototypeKind::Hmodel { hmodel_index: 1 },
                    source.clone(),
                ),
            ),
            (
                "top.nested.body".to_string(),
                resolved(
                    "TOP.NESTED.Body",
                    "__strict_w3d_render_object_source__::top",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    source,
                ),
            ),
        ]);
        let mut resolve = |name: &str| sources.get(&name.to_ascii_lowercase()).cloned();

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(
            items.len(),
            1,
            "the TOP cycle is cut, but its valid sibling renders"
        );
        assert_eq!(
            items[0].model_name,
            "__strict_w3d_render_object_source__::top"
        );
        assert_eq!(
            (items[0].world_matrix * items[0].mesh_local_transform)
                .w_axis
                .x,
            15.0,
            "parent world × outer attachment × default HMODEL roots"
        );
    }

    #[test]
    fn hmodel_prewarm_names_share_rigid_renderer_validation() {
        let source = hmodel_source_model(
            "PREWARM_HMODEL",
            vec![
                W3dHmodelNode {
                    name: "PREWARM_HMODEL.Rigid".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::Node,
                },
                W3dHmodelNode {
                    name: "PREWARM_HMODEL.Collision".to_string(),
                    bone_index: 0,
                    kind: W3dHmodelNodeKind::CollisionNode,
                },
                W3dHmodelNode {
                    name: "PREWARM_HMODEL.Skin".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::SkinNode,
                },
                W3dHmodelNode {
                    name: "PREWARM_HMODEL.Bad".to_string(),
                    bone_index: 7,
                    kind: W3dHmodelNodeKind::Node,
                },
            ],
            &[],
        );

        assert_eq!(
            hmodel_rigid_node_names_for_prewarm(&source, 0),
            vec![
                "PREWARM_HMODEL.Rigid".to_string(),
                "PREWARM_HMODEL.Collision".to_string(),
            ],
            "prewarm must not queue SKIN_NODE or an unrenderable pivot"
        );
    }

    #[test]
    fn indexed_hlod_prototype_uses_its_own_chunk_hierarchy_lod_and_aggregates() {
        let parent = parent_item();
        let poses = [
            W3dHlodAggregatePose {
                name: "SECOND_HLOD".to_string(),
                bone_index: 0,
                parent_transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
                visible: true,
            },
            W3dHlodAggregatePose {
                name: "OUT_OF_RANGE_HLOD".to_string(),
                bone_index: 0,
                parent_transform: Mat4::IDENTITY,
                visible: true,
            },
            W3dHlodAggregatePose {
                name: "MALFORMED_HLOD".to_string(),
                bone_index: 0,
                parent_transform: Mat4::IDENTITY,
                visible: true,
            },
        ];

        let first_tree = W3dHierarchy {
            name: "FIRST_TREE".to_string(),
            pivots: vec![
                pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]),
                pivot("FIRST_BONE", 0, [1.0, 0.0, 0.0]),
            ],
            pivot_fixups: Vec::new(),
        };
        let second_tree = W3dHierarchy {
            name: "SECOND_TREE".to_string(),
            pivots: vec![
                pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]),
                pivot("SECOND_BONE", 0, [4.0, 0.0, 0.0]),
            ],
            pivot_fixups: Vec::new(),
        };
        let mut source = W3DModel::new("MULTI_HLOD_SOURCE".to_string());
        source.hierarchies = vec![first_tree.clone(), second_tree.clone()];
        // The legacy convenience field deliberately names the *other* HLOD.
        // The indexed prototype evaluator must use each header's exact HTree.
        source.hierarchy = Some(first_tree);
        source.hlods = vec![
            W3dHlod {
                version: 0x0001_0000,
                name: "FIRST_HLOD".to_string(),
                hierarchy_name: "FIRST_TREE".to_string(),
                lods: vec![W3dHlodLod {
                    max_screen_size: f32::MAX,
                    subobjects: vec![W3dHlodSubObject {
                        name: "FIRST_HLOD.FirstOnly".to_string(),
                        bone_index: 1,
                    }],
                }],
                aggregates: Some(W3dHlodAttachmentArray {
                    max_screen_size: f32::MAX,
                    subobjects: vec![W3dHlodSubObject {
                        name: "FIRST_GRAND.Body".to_string(),
                        bone_index: 1,
                    }],
                }),
                proxies: None,
                has_unrendered_aggregates: false,
                has_invalid_trailing_records: false,
            },
            W3dHlod {
                version: 0x0001_0000,
                name: "SECOND_HLOD".to_string(),
                hierarchy_name: second_tree.name.clone(),
                lods: vec![
                    W3dHlodLod {
                        max_screen_size: 0.5,
                        subobjects: vec![W3dHlodSubObject {
                            name: "SECOND_HLOD.LowOnly".to_string(),
                            bone_index: 1,
                        }],
                    },
                    W3dHlodLod {
                        max_screen_size: f32::MAX,
                        subobjects: vec![W3dHlodSubObject {
                            name: "SECOND_HLOD.SelectedOnly".to_string(),
                            bone_index: 1,
                        }],
                    },
                ],
                aggregates: Some(W3dHlodAttachmentArray {
                    max_screen_size: f32::MAX,
                    subobjects: vec![W3dHlodSubObject {
                        name: "SECOND_GRAND.Body".to_string(),
                        bone_index: 1,
                    }],
                }),
                proxies: None,
                has_unrendered_aggregates: false,
                has_invalid_trailing_records: false,
            },
        ];
        for (container_name, mesh_name) in [
            ("FIRST_HLOD", "FirstOnly"),
            ("SECOND_HLOD", "LowOnly"),
            ("SECOND_HLOD", "SelectedOnly"),
        ] {
            let mut mesh = W3DMesh::new(mesh_name.to_string());
            mesh.container_name = container_name.to_string();
            source.meshes.push(mesh);
        }

        assert_eq!(
            hlod_prototype_names_for_prewarm(&source, 1),
            vec![
                "SECOND_HLOD.SelectedOnly".to_string(),
                "SECOND_GRAND.Body".to_string(),
            ],
            "prewarm must follow the indexed HLOD's constructor-selected level and own aggregate list"
        );
        assert!(
            hlod_prototype_names_for_prewarm(&source, 2).is_empty(),
            "an out-of-range registry index must not turn into HLOD zero or a whole-file scan"
        );

        let malformed_source = {
            let mut malformed = source.clone();
            malformed.hlods[1].has_invalid_trailing_records = true;
            malformed
        };
        let first_grand = direct_mesh_model("FIRST_GRAND", "Body");
        let second_grand = direct_mesh_model("SECOND_GRAND", "Body");
        let sources = HashMap::from([
            (
                "second_hlod".to_string(),
                resolved(
                    "SECOND_HLOD",
                    "__strict_w3d_render_object_source__::multi_hlod",
                    W3dRenderObjectPrototypeKind::Hlod { hlod_index: 1 },
                    source.clone(),
                ),
            ),
            (
                "second_hlod.selectedonly".to_string(),
                resolved(
                    "SECOND_HLOD.SelectedOnly",
                    "__strict_w3d_render_object_source__::multi_hlod",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 2 },
                    source.clone(),
                ),
            ),
            (
                "second_grand.body".to_string(),
                resolved(
                    "SECOND_GRAND.Body",
                    "__strict_w3d_render_object_source__::second_grand",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    second_grand,
                ),
            ),
            (
                "out_of_range_hlod".to_string(),
                resolved(
                    "OUT_OF_RANGE_HLOD",
                    "__strict_w3d_render_object_source__::multi_hlod",
                    W3dRenderObjectPrototypeKind::Hlod { hlod_index: 2 },
                    source.clone(),
                ),
            ),
            (
                "malformed_hlod".to_string(),
                resolved(
                    "MALFORMED_HLOD",
                    "__strict_w3d_render_object_source__::multi_hlod",
                    W3dRenderObjectPrototypeKind::Hlod { hlod_index: 1 },
                    malformed_source,
                ),
            ),
            // Keep these exact records available so an accidental index-zero,
            // low-LOD, or whole-file fallback would visibly fail this test.
            (
                "first_hlod.firstonly".to_string(),
                resolved(
                    "FIRST_HLOD.FirstOnly",
                    "__strict_w3d_render_object_source__::multi_hlod",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    source.clone(),
                ),
            ),
            (
                "first_grand.body".to_string(),
                resolved(
                    "FIRST_GRAND.Body",
                    "__strict_w3d_render_object_source__::first_grand",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    first_grand,
                ),
            ),
            (
                "second_hlod.lowonly".to_string(),
                resolved(
                    "SECOND_HLOD.LowOnly",
                    "__strict_w3d_render_object_source__::multi_hlod",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 1 },
                    source,
                ),
            ),
        ]);
        let mut resolve = |name: &str| sources.get(&name.to_ascii_lowercase()).cloned();

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(items.len(), 2, "only the valid indexed HLOD may render");
        assert!(items.iter().any(|item| {
            item.model_name == "__strict_w3d_render_object_source__::multi_hlod"
                && item.mesh_index == 2
                && (item.world_matrix * item.mesh_local_transform).w_axis.x == 19.0
        }));
        assert!(items.iter().any(|item| {
            item.model_name == "__strict_w3d_render_object_source__::second_grand"
                && (item.world_matrix * item.mesh_local_transform).w_axis.x == 19.0
        }));
        assert!(!items.iter().any(|item| {
            (item.model_name == "__strict_w3d_render_object_source__::multi_hlod"
                && matches!(item.mesh_index, 0 | 1))
                || item.model_name == "__strict_w3d_render_object_source__::first_grand"
        }));
    }

    #[test]
    fn aggregate_hlod_renders_its_own_selected_topology_and_nested_additional_models() {
        let parent = parent_item();
        let poses = [W3dHlodAggregatePose {
            name: "CHILD_HLOD".to_string(),
            bone_index: 1,
            parent_transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            visible: true,
        }];
        let child = single_hlod_model(
            "CHILD_HLOD",
            "CHILD_HLOD",
            "Body",
            2.0,
            vec![W3dHlodSubObject {
                name: "GRAND.Body".to_string(),
                bone_index: 1,
            }],
        );
        let grand = direct_mesh_model("GRAND", "Body");
        let sources = HashMap::from([
            (
                "child_hlod".to_string(),
                resolved(
                    "CHILD_HLOD",
                    "__strict_w3d_render_object_source__::child_hlod",
                    W3dRenderObjectPrototypeKind::Hlod { hlod_index: 0 },
                    child.clone(),
                ),
            ),
            (
                "child_hlod.body".to_string(),
                resolved(
                    "CHILD_HLOD.Body",
                    "__strict_w3d_render_object_source__::child_hlod",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    child,
                ),
            ),
            (
                "grand.body".to_string(),
                resolved(
                    "GRAND.Body",
                    "__strict_w3d_render_object_source__::grand",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    grand,
                ),
            ),
        ]);
        let mut resolve = |name: &str| sources.get(&name.to_ascii_lowercase()).cloned();

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(
            items.len(),
            2,
            "child HLOD and its nested aggregate both render"
        );
        assert!(items.iter().any(|item| {
            item.model_name == "__strict_w3d_render_object_source__::child_hlod"
                && (item.world_matrix * item.mesh_local_transform).w_axis.x == 17.0
        }));
        assert!(items.iter().any(|item| {
            item.model_name == "__strict_w3d_render_object_source__::grand"
                && (item.world_matrix * item.mesh_local_transform).w_axis.x == 17.0
        }));
    }

    #[test]
    fn aggregate_hlod_without_rigid_parent_mesh_still_renders_cached_additional_model() {
        let parent = parent_item();
        let poses = [W3dHlodAggregatePose {
            name: "CHILD_HLOD".to_string(),
            bone_index: 1,
            parent_transform: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            visible: true,
        }];
        let mut child = single_hlod_model(
            "CHILD_HLOD",
            "CHILD_HLOD",
            "UnusedRigidMesh",
            2.0,
            vec![W3dHlodSubObject {
                name: "GRAND.Body".to_string(),
                bone_index: 1,
            }],
        );
        // C++ `HLodClass` still owns and renders AdditionalModels when the
        // selected parent LOD has no rigid render objects.  Do not make an
        // independently resolved aggregate conditional on a dummy parent mesh.
        child.meshes.clear();
        child.hlods[0].lods[0].subobjects.clear();

        let grand = direct_mesh_model("GRAND", "Body");
        let sources = HashMap::from([
            (
                "child_hlod".to_string(),
                resolved(
                    "CHILD_HLOD",
                    "__strict_w3d_render_object_source__::child_hlod",
                    W3dRenderObjectPrototypeKind::Hlod { hlod_index: 0 },
                    child,
                ),
            ),
            (
                "grand.body".to_string(),
                resolved(
                    "GRAND.Body",
                    "__strict_w3d_render_object_source__::grand",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    grand,
                ),
            ),
        ]);
        let mut resolve = |name: &str| sources.get(&name.to_ascii_lowercase()).cloned();

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].model_name,
            "__strict_w3d_render_object_source__::grand"
        );
        assert_eq!(
            (items[0].world_matrix * items[0].mesh_local_transform)
                .w_axis
                .x,
            17.0
        );
    }

    #[test]
    fn aggregate_missing_hidden_invalid_and_cyclic_records_skip_individually() {
        let parent = parent_item();
        let poses = [
            W3dHlodAggregatePose {
                name: "MISSING.Body".to_string(),
                bone_index: 1,
                parent_transform: Mat4::IDENTITY,
                visible: true,
            },
            W3dHlodAggregatePose {
                name: "HIDDEN.Body".to_string(),
                bone_index: 1,
                parent_transform: Mat4::IDENTITY,
                visible: false,
            },
            W3dHlodAggregatePose {
                name: "BAD.Body".to_string(),
                bone_index: 1,
                parent_transform: Mat4::from_cols_array(&[
                    f32::NAN,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                ]),
                visible: true,
            },
            W3dHlodAggregatePose {
                name: "LOOP".to_string(),
                bone_index: 1,
                parent_transform: Mat4::IDENTITY,
                visible: true,
            },
        ];
        let loop_model = single_hlod_model(
            "LOOP",
            "LOOP",
            "Body",
            1.0,
            vec![W3dHlodSubObject {
                name: "LOOP".to_string(),
                bone_index: 1,
            }],
        );
        let mut resolve = |name: &str| {
            let resolved = if name.eq_ignore_ascii_case("LOOP") {
                resolved(
                    "LOOP",
                    "__strict_w3d_render_object_source__::loop",
                    W3dRenderObjectPrototypeKind::Hlod { hlod_index: 0 },
                    loop_model.clone(),
                )
            } else if name.eq_ignore_ascii_case("LOOP.Body") {
                resolved(
                    "LOOP.Body",
                    "__strict_w3d_render_object_source__::loop",
                    W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 },
                    loop_model.clone(),
                )
            } else {
                return None;
            };
            Some(resolved)
        };

        let items = collect_hlod_aggregate_render_items_with_resolver(
            &parent,
            parent.world_matrix,
            &poses,
            Vec3::ZERO,
            &mut resolve,
        );

        assert_eq!(
            items.len(),
            1,
            "missing/bad records and the recursive LOOP edge must not suppress the valid parent-attached HLOD mesh"
        );
        assert_eq!(
            items[0].model_name,
            "__strict_w3d_render_object_source__::loop"
        );
    }

    #[test]
    fn cache_only_runtime_resolver_never_calls_blocking_or_fallback_model_paths() {
        let source = include_str!("hlod_aggregate_render.rs");
        let blocking = ["resolve_w3d_render_object", "_blocking"].concat();
        let fallback = ["get_model", "_or_fallback"].concat();
        let resolver_body = source
            .split("fn resolve_cached_hlod_aggregate_render_object")
            .nth(1)
            .and_then(|rest| rest.split("/// Keep strict render-object geometry").next())
            .expect("the runtime cached resolver body must remain delimited");
        assert!(source.contains("cached_w3d_render_object_prototype"));
        assert!(source.contains("cached_w3d_render_object_source_model"));
        assert!(
            !resolver_body.contains(&blocking),
            "render collection must never open an archive for an aggregate"
        );
        assert!(
            !resolver_body.contains(&fallback),
            "an aggregate may not borrow Main's diagnostic fallback mesh"
        );
    }
}
