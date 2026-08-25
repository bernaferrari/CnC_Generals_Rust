//! C++ `HLodClass::Update_Obj_Space_Bounding_Volumes` BOUNDINGBOX scan.

use super::w3d_primitive_protos::W3dBoxProto;
use super::{W3DModel, W3dHlod};
use glam::{Mat4, Vec3};

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

pub fn should_skip_obbox_child(model: &W3DModel, child_name: &str) -> bool {
    model
        .boxes
        .iter()
        .any(|box_proto| box_proto.is_oriented() && box_matches_child(box_proto, child_name))
        && hlod_child_leaf_name(child_name).eq_ignore_ascii_case("BOUNDINGBOX")
}
