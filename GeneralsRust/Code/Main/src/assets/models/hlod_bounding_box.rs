//! C++ `HLodClass::Update_Obj_Space_Bounding_Volumes` BOUNDINGBOX scan.

use super::w3d_anim::compute_bind_pose_global_transforms;
use super::w3d_primitive_protos::W3dBoxProto;
use super::{W3DMesh, W3DModel, W3dHlod};
use glam::{Mat4, Vec2, Vec3};

/// Leaf after the first `.` (`strchr(name, '.') + 1` in `hlod.cpp:3301`).
pub fn hlod_child_leaf_name(name: &str) -> &str {
    name.split_once('.').map(|(_, leaf)| leaf).unwrap_or(name)
}

pub fn box_matches_child(box_proto: &W3dBoxProto, child_name: &str) -> bool {
    let leaf = hlod_child_leaf_name(child_name);
    box_proto.name.eq_ignore_ascii_case(child_name)
        || box_proto.name.eq_ignore_ascii_case(leaf)
        || hlod_child_leaf_name(&box_proto.name).eq_ignore_ascii_case(leaf)
}

/// Highest-LOD child index whose name is BOUNDINGBOX and whose proto is OBBOX.
pub fn hlod_bounding_box_child_index(hlod: &W3dHlod, boxes: &[W3dBoxProto]) -> Option<usize> {
    let high = hlod.lods.last()?;
    for (index, child) in high.subobjects.iter().enumerate().rev() {
        if !hlod_child_leaf_name(&child.name).eq_ignore_ascii_case("BOUNDINGBOX") {
            continue;
        }
        if boxes
            .iter()
            .any(|box_proto| box_proto.is_oriented() && box_matches_child(box_proto, &child.name))
        {
            return Some(index);
        }
    }
    None
}

pub fn hlod_bounding_box_proto<'a>(
    model: &'a W3DModel,
    hlod_index: usize,
) -> Option<(usize, &'a W3dBoxProto, u32)> {
    let hlod = model.hlods.get(hlod_index)?;
    let child_index = hlod_bounding_box_child_index(hlod, &model.boxes)?;
    let child = hlod.lods.last()?.subobjects.get(child_index)?;
    let box_proto = model
        .boxes
        .iter()
        .find(|box_proto| box_proto.is_oriented() && box_matches_child(box_proto, &child.name))?;
    Some((child_index, box_proto, child.bone_index))
}

/// Object-space AABox from the posed OBBOX (`hlod.cpp:1318-1332`).
pub fn posed_obbox_obj_space(
    hlod_world: Mat4,
    box_world: Mat4,
    local_center: Vec3,
    local_extent: Vec3,
) -> (Vec3, Vec3) {
    let box_to_hlod = hlod_world.inverse() * box_world;
    let center = box_to_hlod.transform_point3(local_center);
    let extent = box_to_hlod.x_axis.truncate().abs() * local_extent.x
        + box_to_hlod.y_axis.truncate().abs() * local_extent.y
        + box_to_hlod.z_axis.truncate().abs() * local_extent.z;
    (center, extent)
}

impl W3DModel {
    /// C++ object-space XY box size for the exact constructor render object.
    ///
    /// Supports a plain mesh, an HLOD with a named OBBOX, or the one-child
    /// subset of the HLOD fallback that combines every LOD and AdditionalModel.
    /// The caller supplies the already-baked constructor asset scale.
    pub(crate) fn constructor_projected_decal_size_xy(
        &self,
        render_object_name: &str,
        asset_scale: f32,
    ) -> Option<Vec2> {
        if self.hlod_parse_failed || !asset_scale.is_finite() || asset_scale <= 0.0 {
            return None;
        }

        let size = match self.hlods.as_slice() {
            [] => {
                let [mesh] = self.meshes.as_slice() else {
                    return None;
                };
                if !mesh.container_name.is_empty()
                    || !mesh.name.eq_ignore_ascii_case(render_object_name)
                {
                    return None;
                }
                source_mesh_size(mesh)? * asset_scale
            }
            [hlod]
                if hlod.name.eq_ignore_ascii_case(render_object_name)
                    && !hlod.has_invalid_trailing_records =>
            {
                self.constructor_hlod_decal_size(hlod, asset_scale)?
            }
            _ => return None,
        };
        (size.is_finite() && size.min_element() >= 0.0).then_some(size.truncate())
    }

    fn constructor_hlod_decal_size(&self, hlod: &W3dHlod, asset_scale: f32) -> Option<Vec3> {
        let high = hlod.lods.last()?;
        let poses = match self.source_hierarchy_for_hlod(hlod) {
            Some(hierarchy) => compute_bind_pose_global_transforms(hierarchy)?,
            None => vec![Mat4::IDENTITY.to_cols_array()],
        };

        // HLodClass scans the highest detail level in reverse for an
        // instantiated obbox whose own name ends in the BOUNDINGBOX designator.
        for child in high.subobjects.iter().rev() {
            if !hlod_child_leaf_name(&child.name).eq_ignore_ascii_case("BOUNDINGBOX") {
                continue;
            }
            let Some(box_proto) = self.boxes.iter().find(|candidate| {
                candidate.is_oriented() && candidate.name.eq_ignore_ascii_case(&child.name)
            }) else {
                continue;
            };
            let local_extent = Vec3::from_array(box_proto.extent);
            if !local_extent.is_finite() || local_extent.min_element() < 0.0 {
                return None;
            }
            let pose = Mat4::from_cols_array(poses.get(child.bone_index as usize)?);
            let (_, extent) = posed_obbox_obj_space(
                Mat4::IDENTITY,
                pose,
                Vec3::from_array(box_proto.center),
                local_extent,
            );
            // BoxRenderObjClass inherits the no-op RenderObjClass::Scale.
            // HTree::Scale changes pivot translation, which cannot change width.
            return Some(extent * 2.0);
        }

        if hlod.lods.len() != 1
            || high.subobjects.len() != 1
            || hlod
                .aggregates
                .as_ref()
                .is_some_and(|array| !array.subobjects.is_empty())
        {
            return None;
        }
        let (mesh, bone_index) = self.meshes.iter().find_map(|mesh| {
            let (_, bone_index) =
                Self::find_rigid_hlod_child_for_mesh(hlod, high, mesh, poses.len())?;
            Some((mesh, bone_index))
        })?;
        let pose = Mat4::from_cols_array(&poses[bone_index]);
        let (_, extent) = posed_obbox_obj_space(
            Mat4::IDENTITY,
            pose,
            Vec3::ZERO,
            source_mesh_size(mesh)? * (asset_scale * 0.5),
        );
        Some(extent * 2.0)
    }
}

fn source_mesh_size(mesh: &W3DMesh) -> Option<Vec3> {
    let bounds = mesh.source_object_bounds?;
    let size = bounds.max - bounds.min;
    (size.is_finite() && size.min_element() >= 0.0).then_some(size)
}

pub fn should_skip_obbox_child(model: &W3DModel, child_name: &str) -> bool {
    model
        .boxes
        .iter()
        .any(|box_proto| box_proto.is_oriented() && box_matches_child(box_proto, child_name))
        && hlod_child_leaf_name(child_name).eq_ignore_ascii_case("BOUNDINGBOX")
}
