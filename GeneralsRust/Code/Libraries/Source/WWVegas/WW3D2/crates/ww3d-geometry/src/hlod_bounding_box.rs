//! C++ `HLodClass` hidden `BOUNDINGBOX` OBBOX (`hlod.cpp:3280-3310`, `1304-1417`).
//!
//! Highest-LOD child named `BOUNDINGBOX` with `CLASSID_OBBOX` drives animated
//! object-space bounds. Render skips that child.

use crate::hlod::{HLod, RenderObject};
use crate::{AABox, Sphere};
use glam::{Mat4, Vec3};

pub const CLASSID_OBBOX: u32 = 27;

/// Leaf name after the first `.`, matching C++ `strchr(name, '.') + 1`.
pub fn hlod_child_leaf_name(name: &str) -> &str {
    name.split_once('.').map(|(_, leaf)| leaf).unwrap_or(name)
}

pub fn is_bounding_box_obbox(name: &str, class_id: u32) -> bool {
    class_id == CLASSID_OBBOX && hlod_child_leaf_name(name).eq_ignore_ascii_case("BOUNDINGBOX")
}

/// Scan the highest LOD for a hidden BOUNDINGBOX OBBOX. Returns -1 if none.
pub fn scan_bounding_box_index(hlod: &HLod) -> i32 {
    let Some(high) = hlod.lods.last() else {
        return -1;
    };
    for (index, node) in high.models.iter().enumerate().rev() {
        let Some(model) = node.model.as_ref() else {
            continue;
        };
        if is_bounding_box_obbox(model.get_name(), model.class_id()) {
            return index as i32;
        }
    }
    -1
}

/// `Transform_Center_Extent_AABox` for an OBBOX expressed in `box_to_hlod`.
pub fn transform_center_extent_aabox(center: Vec3, extent: Vec3, box_to_hlod: Mat4) -> AABox {
    let world_center = box_to_hlod.transform_point3(center);
    let x = box_to_hlod.x_axis.truncate().abs() * extent.x;
    let y = box_to_hlod.y_axis.truncate().abs() * extent.y;
    let z = box_to_hlod.z_axis.truncate().abs() * extent.z;
    AABox::new(world_center, x + y + z)
}

pub fn obj_space_box_from_obbox(
    hlod_world: Mat4,
    box_world: Mat4,
    local_center: Vec3,
    local_extent: Vec3,
) -> AABox {
    let world_to_hlod = hlod_world.inverse();
    let box_to_hlod = world_to_hlod * box_world;
    transform_center_extent_aabox(local_center, local_extent, box_to_hlod)
}

pub fn sphere_from_aabox(box_: &AABox) -> Sphere {
    Sphere::new(box_.center, box_.extent.length())
}

pub fn should_skip_obbox_render(model: &dyn RenderObject) -> bool {
    model.class_id() == CLASSID_OBBOX
}
