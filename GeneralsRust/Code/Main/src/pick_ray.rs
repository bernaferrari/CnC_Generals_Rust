//! Camera-ray drawable pick (C++ `W3DView::pickDrawable` / `castRay`).
//!
//! Live click/hover used to rank 3D distance from the terrain hit to each
//! object's origin inside a 20wu pad. C++ point-clicks first cast a camera
//! ray against visible render-object geometry.

use crate::game_logic::host_residual_acquire::{
    PriorityAcquireCandidate, pick_best_priority_residual_target,
};
use crate::game_logic::{ObjectId, Team};
use crate::presentation_frame::{PresentationFrame, RenderableObject};
use crate::unit_control::UnitControlSystem;
use glam::Vec3;

/// C++ `W3DView::pickDrawable` uses a 1wu floor so a degenerate sphere still hits.
pub const MIN_PICK_SPHERE_RADIUS: f32 = 1.0;

/// Geometry-sized pick sphere. Never inflate to the old 20wu terrain pad.
pub fn presentation_mesh_pick_radius(selection_radius: f32, health_box_width: f32) -> f32 {
    let geometry = if health_box_width > 0.0 {
        health_box_width * 0.5
    } else {
        0.0
    };
    selection_radius.max(geometry).max(MIN_PICK_SPHERE_RADIUS)
}

/// Hit `t` along `ray_dir` (segment is `t` in `[0, 1]` when `ray_dir = end - start`).
pub fn ray_sphere_hit_t(ray_start: Vec3, ray_dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let a = ray_dir.dot(ray_dir);
    if a < 1.0e-12 {
        return None;
    }
    let oc = ray_start - center;
    let b = 2.0 * oc.dot(ray_dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_disc = disc.sqrt();
    let t0 = (-b - sqrt_disc) / (2.0 * a);
    let t1 = (-b + sqrt_disc) / (2.0 * a);
    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

fn object_hit_along_ray(object: &RenderableObject, ray_start: Vec3, ray_dir: Vec3) -> Option<f32> {
    let radius = presentation_mesh_pick_radius(object.selection_radius, object.health_box_width);
    let t = ray_sphere_hit_t(ray_start, ray_dir, object.position, radius)?;
    (t <= 1.0).then_some(t)
}

/// C++ `iterateDrawablesInRegion` point path: `pickDrawable` then priority bands.
pub fn pick_object_id_along_camera_ray(
    frame: &PresentationFrame,
    ray_start: Vec3,
    ray_end: Vec3,
    player_team: Option<Team>,
    prioritize_enemy_targets: bool,
) -> Option<ObjectId> {
    let ray_dir = ray_end - ray_start;
    let cands: Vec<_> = frame
        .objects
        .iter()
        .filter_map(|object| {
            if UnitControlSystem::presentation_pick_skips_dead(object) {
                return None;
            }
            // C++ CanSelectDrawable / SelectionInfo: fogged or undetected
            // stealth neutrals+enemies are not pickable.
            if frame.box_pick_hides_non_local(object) {
                return None;
            }
            let t = object_hit_along_ray(object, ray_start, ray_dir)?;
            let selectable = UnitControlSystem::presentation_is_selectable(object);
            let attackable = UnitControlSystem::presentation_is_attackable(object);
            let priority = if prioritize_enemy_targets {
                match player_team {
                    Some(_) if frame.is_enemy_of_local(object) && attackable => Some(0),
                    Some(_) if frame.is_owned_by_local(object) && selectable => Some(1),
                    _ if attackable => Some(2),
                    _ if selectable => Some(3),
                    _ => None,
                }
            } else {
                match player_team {
                    Some(_) if frame.is_owned_by_local(object) && selectable => Some(0),
                    Some(_) if selectable => Some(1),
                    Some(_) => None,
                    None if selectable => Some(0),
                    None => None,
                }
            };
            let priority = if priority.is_none()
                && prioritize_enemy_targets
                && (object.is_crate || object.is_salvage_crate)
            {
                Some(4)
            } else {
                priority
            };
            Some(PriorityAcquireCandidate {
                id: object.id,
                position: ray_start + ray_dir * t,
                is_alive: true,
                priority,
            })
        })
        .collect();
    pick_best_priority_residual_target(
        ObjectId(0),
        ray_start,
        (ray_start.x, ray_start.z),
        f32::MAX,
        cands,
    )
    .map(|(id, _, _)| id)
}

/// Opaque GUI walk: any non-`SEE_THRU` ancestor refuses the pick.
pub fn opaque_window_chain_blocks_pick(see_thru_from_leaf_to_root: &[bool]) -> bool {
    see_thru_from_leaf_to_root.iter().any(|see_thru| !see_thru)
}

/// Win32 `SM_CXDOUBLECLK` / `SM_CYDOUBLECLK` default — a 4px rectangle.
pub const OS_DOUBLE_CLICK_SLOP_PX: f32 = 4.0;

/// C++ `Mouse.cpp` promotes `MBS_DoubleClick` from the OS, never a 10wu pad.
pub fn is_os_style_double_click(
    time_delta_ms: u128,
    screen_dx: f32,
    screen_dy: f32,
    time_limit_ms: u128,
    slop_px: f32,
) -> bool {
    time_delta_ms < time_limit_ms && screen_dx.abs() <= slop_px && screen_dy.abs() <= slop_px
}

/// C++ `SelectionXlat.cpp:577-581`: quit menu DESTROYs world LMB.
pub fn world_lmb_selection_allowed(quit_menu_visible: bool) -> bool {
    !quit_menu_visible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_click_beside_a_unit_does_not_hit_its_mesh_sphere() {
        // Given: infantry-sized sphere at origin (8wu), camera looking at origin.
        let camera = Vec3::new(0.0, 120.0, 120.0);
        let ground_beside = Vec3::new(20.0, 0.0, 0.0);
        let dir = ground_beside - camera;
        // When: the click ray goes to terrain 20wu beside the origin.
        let hit = ray_sphere_hit_t(camera, dir, Vec3::ZERO, 8.0);
        // Then: the ray misses the mesh sphere (old 20wu terrain pad would hit).
        assert!(
            hit.is_none() || hit.is_some_and(|t| t > 1.0),
            "terrain-adjacent click must not select via origin proximity, hit={hit:?}"
        );
    }

    #[test]
    fn long_vehicle_mesh_far_from_origin_is_hittable() {
        // Given: a 40wu-wide vehicle (health box 80 → radius 40) at origin.
        let radius = presentation_mesh_pick_radius(8.0, 80.0);
        assert!((radius - 40.0).abs() < f32::EPSILON);
        let camera = Vec3::new(30.0, 80.0, 80.0);
        let mesh_point = Vec3::new(30.0, 4.0, 0.0);
        let dir = mesh_point - camera;
        // When: the player clicks the hull far from the origin.
        let hit = ray_sphere_hit_t(camera, dir, Vec3::ZERO, radius);
        // Then: the geometry sphere registers the click.
        assert!(
            hit.is_some_and(|t| (0.0..=1.0).contains(&t)),
            "long-mesh click must hit, hit={hit:?}"
        );
    }

    #[test]
    fn pick_radius_never_inflates_to_twenty_world_units() {
        assert_eq!(presentation_mesh_pick_radius(5.0, 0.0), 5.0);
        assert_eq!(
            presentation_mesh_pick_radius(0.0, 0.0),
            MIN_PICK_SPHERE_RADIUS
        );
        assert!(presentation_mesh_pick_radius(5.0, 0.0) < 20.0);
    }

    #[test]
    fn opaque_hud_ancestor_refuses_the_pick() {
        assert!(!opaque_window_chain_blocks_pick(&[true, true]));
        assert!(opaque_window_chain_blocks_pick(&[true, false]));
        assert!(opaque_window_chain_blocks_pick(&[false]));
        assert!(!opaque_window_chain_blocks_pick(&[]));
    }

    #[test]
    fn double_click_uses_screen_pixels_not_world_units() {
        assert!(is_os_style_double_click(
            200,
            3.0,
            0.0,
            500,
            OS_DOUBLE_CLICK_SLOP_PX
        ));
        assert!(!is_os_style_double_click(
            200,
            6.0,
            0.0,
            500,
            OS_DOUBLE_CLICK_SLOP_PX
        ));
        assert!(!is_os_style_double_click(
            600,
            0.0,
            0.0,
            500,
            OS_DOUBLE_CLICK_SLOP_PX
        ));
    }

    #[test]
    fn quit_menu_destroys_world_left_click() {
        assert!(!world_lmb_selection_allowed(true));
        assert!(world_lmb_selection_allowed(false));
    }
}
