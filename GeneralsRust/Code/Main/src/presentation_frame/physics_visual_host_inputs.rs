//! Host INI / terrain facts for `PhysicsVisualBody` freeze.
//!
//! C++ `Drawable.cpp` treads/wheels read `TheTerrainLogic->getLayerHeight`
//! (`:1685`, `:1944`, `:2239`) and `GeometryInfo` radii. Host Object does not
//! carry those fields; this module samples the presentation-time sources that
//! do exist without inventing KindOf variants or geometry on Object/Thing.

use crate::game_logic::Object;
use crate::game_logic::host_partition_collision_physics_residual::{
    GEOMETRY_BOX_RESIDUAL, GEOMETRY_CYLINDER_RESIDUAL, GEOMETRY_SPHERE_RESIDUAL,
    geometry_bounding_circle_radius, geometry_max_height_above_position,
};
use glam::Vec3;
use std::cell::RefCell;
use std::collections::HashMap;

/// C++ `MAP_XY_FACTOR` residual used by host `slope_at_world` when the
/// live heightmap scale is not reachable from this crate boundary.
const HOST_NORMAL_SAMPLE_XY: f32 = 10.0;

#[derive(Debug, Clone, Default)]
pub struct ObjectVisualIni {
    pub major_radius: Option<f32>,
    pub minor_radius: Option<f32>,
    pub height: Option<f32>,
    pub geometry: Option<String>,
    pub kindof: Option<String>,
}

#[cfg(test)]
thread_local! {
    static TEST_OBJECT_INI: RefCell<HashMap<String, ObjectVisualIni>> =
        RefCell::new(HashMap::new());
}

#[cfg(test)]
pub fn set_test_object_visual_ini(template_name: &str, ini: ObjectVisualIni) {
    TEST_OBJECT_INI.with(|slot| {
        slot.borrow_mut().insert(template_name.to_string(), ini);
    });
}

#[cfg(test)]
pub fn clear_test_object_visual_ini() {
    TEST_OBJECT_INI.with(|slot| slot.borrow_mut().clear());
}

/// C++ Z-up unit normal from four host Y-up height samples.
///
/// Host `TerrainData::slope_at_world` (`terrain.rs:166-177`) is the residual
/// — not `BaseHeightMap.cpp:914-970` 12-point cross product, and not
/// `W3DTerrainLogic.cpp:281-330` bridge/wall layer replacement. Missing
/// samples keep the C++ null-renderer fallback `(0,0,1)`.
pub fn terrain_normal_zup_from_height_samples<F>(sample: F, host_pos: Vec3) -> (f32, f32, f32)
where
    F: Fn(Vec3) -> Option<f32>,
{
    let dx = HOST_NORMAL_SAMPLE_XY;
    let Some(h_l) = sample(Vec3::new(host_pos.x - dx, host_pos.y, host_pos.z)) else {
        return (0.0, 0.0, 1.0);
    };
    let Some(h_r) = sample(Vec3::new(host_pos.x + dx, host_pos.y, host_pos.z)) else {
        return (0.0, 0.0, 1.0);
    };
    let Some(h_d) = sample(Vec3::new(host_pos.x, host_pos.y, host_pos.z - dx)) else {
        return (0.0, 0.0, 1.0);
    };
    let Some(h_u) = sample(Vec3::new(host_pos.x, host_pos.y, host_pos.z + dx)) else {
        return (0.0, 0.0, 1.0);
    };
    let gx = (h_r - h_l) / (2.0 * dx);
    let gz = (h_u - h_d) / (2.0 * dx);
    let nx = -gx;
    let ny = -gz;
    let nz = 1.0;
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if !len.is_finite() || len <= f32::EPSILON {
        return (0.0, 0.0, 1.0);
    }
    (nx / len, ny / len, nz / len)
}

pub fn object_visual_ini(template_name: &str) -> ObjectVisualIni {
    #[cfg(test)]
    {
        if let Some(ini) = TEST_OBJECT_INI.with(|slot| slot.borrow().get(template_name).cloned()) {
            return ini;
        }
    }
    object_visual_ini_from_asset_manager(template_name).unwrap_or_default()
}

fn object_visual_ini_from_asset_manager(template_name: &str) -> Option<ObjectVisualIni> {
    let manager = crate::assets::get_asset_manager()?;
    let guard = manager.lock().ok()?;
    let definition = guard.get_object_definition(template_name)?;
    let attr = |name: &str| {
        definition
            .attributes
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    };
    Some(ObjectVisualIni {
        major_radius: attr("GeometryMajorRadius").and_then(|value| value.parse().ok()),
        minor_radius: attr("GeometryMinorRadius").and_then(|value| value.parse().ok()),
        height: attr("GeometryHeight").and_then(|value| value.parse().ok()),
        geometry: attr("Geometry").map(str::to_string),
        kindof: attr("KindOf")
            .or_else(|| attr("kindof"))
            .map(str::to_string),
    })
}

pub fn host_kindof_token(ini: &ObjectVisualIni, token: &str) -> bool {
    let Some(kindof) = ini.kindof.as_deref() else {
        return false;
    };
    // Same split as `spawn_templates.rs:233-239`. Host `KindOf` does not
    // retain SHRUBBERY / LOW_OVERLAPPABLE (`host_types.rs`); this is the
    // authored INI string, not a fabricated enum variant.
    kindof
        .split(|character: char| character.is_ascii_whitespace() || matches!(character, ',' | '|'))
        .any(|candidate| candidate.eq_ignore_ascii_case(token))
}

pub fn host_geometry_radii(obj: &Object, ini: &ObjectVisualIni) -> (f32, f32, f32, f32) {
    let fallback = obj.selection_radius.max(1.0);
    let Some(major) = ini
        .major_radius
        .filter(|value| value.is_finite() && *value > 0.0)
    else {
        return (fallback, fallback, fallback, fallback.max(0.5));
    };
    let minor = ini
        .minor_radius
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(major);
    let height = ini
        .height
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(major);
    let geom_type = parse_geometry_type(ini.geometry.as_deref());
    let bounding = geometry_bounding_circle_radius(geom_type, major, minor).unwrap_or(major);
    let max_height = geometry_max_height_above_position(geom_type, height, major).unwrap_or(height);
    (major, minor, bounding, max_height)
}

fn parse_geometry_type(name: Option<&str>) -> u32 {
    match name.map(str::trim) {
        Some(name) if name.eq_ignore_ascii_case("SPHERE") => GEOMETRY_SPHERE_RESIDUAL,
        Some(name) if name.eq_ignore_ascii_case("CYLINDER") => GEOMETRY_CYLINDER_RESIDUAL,
        Some(name) if name.eq_ignore_ascii_case("BOX") => GEOMETRY_BOX_RESIDUAL,
        _ => GEOMETRY_BOX_RESIDUAL,
    }
}
