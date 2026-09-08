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

/// Host logic rate — the `GameLogic` fixed step (`LOGICFRAMES_PER_SECOND`).
pub const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

/// C++ `W3DView::pickDrawable` uses a 1wu floor so a degenerate sphere still hits.
pub const MIN_PICK_SPHERE_RADIUS: f32 = 1.0;
/// Longest stale-snapshot lead the pick will extrapolate (half a second).
/// Beyond it the freeze is too old to advance honestly and the frozen pose
/// stands, matching the C++ click-what-you-see behavior.
pub const MAX_PICK_EXTRAPOLATION_FRAMES: u32 = 15;

/// Geometry-sized fallback pick radius. Never inflate to the old 20wu terrain
/// pad, and never leak the screen-space health-box width into world units —
/// the removed `health_box_width * 0.5` term is what over-picked long
/// structures. The presentation freeze already pads `selection_radius` to a
/// 5wu floor (`presentation_frame/build.rs` `obj.selection_radius.max(5.0)`),
/// so this floor only matters for hand-built frames.
pub fn presentation_mesh_pick_radius(selection_radius: f32) -> f32 {
    selection_radius.max(MIN_PICK_SPHERE_RADIUS)
}

/// Advance a frozen snapshot pose to `now_logic_frame`.
///
/// C++ point-clicks cast against the live client scene at message time
/// (`W3DView::pickDrawable`, W3DView.cpp:2183-2230). This host picks frozen
/// `PresentationFrame`s; when a click lands after the stamp, moving units
/// have advanced by `velocity × (now − stamp)` and the frozen pose misses.
/// `velocity` is world-units-per-second, so the lead is
/// `frames / LOGIC_FRAMES_PER_SECOND`.
pub fn extrapolated_pick_position(
    object: &RenderableObject,
    frame_stamp: u32,
    now_logic_frame: u32,
) -> Vec3 {
    let frames = now_logic_frame
        .saturating_sub(frame_stamp)
        .min(MAX_PICK_EXTRAPOLATION_FRAMES);
    if frames == 0 || object.velocity.length_squared() < 1.0e-12 {
        return object.position;
    }
    object.position + object.velocity * (frames as f32 / LOGIC_FRAMES_PER_SECOND)
}

/// Ray hit `t` against a vertical capsule: cylinder body of `height` over
/// `base` plus rounded end caps, all at `radius`.
///
/// The host's stand-in for the C++ mesh `castRay` silhouette test
/// (W3DView.cpp:2219-2222, `RayCollisionTestClass(..., COLL_TYPE_ALL)`):
/// authored `GeometryInfo` (thing.template.geometry_info major/minor/height,
/// construct.rs:373-378) freezes into `selection_radius` (the bounding
/// circle — major extents for structures) and
/// `max_height_above_position` (geometry height — infantry are 12-15wu
/// tall), so the pick volume matches the visible silhouette instead of a
/// feet-centered origin sphere.
fn ray_vertical_capsule_hit_t(
    ray_start: Vec3,
    ray_dir: Vec3,
    base: Vec3,
    height: f32,
    radius: f32,
) -> Option<f32> {
    let top = base.y + height;
    let mut best: Option<f32> = None;
    let mut consider = |t: f32, best: &mut Option<f32>| {
        if t >= 0.0 && best.is_none_or(|b| t < b) {
            *best = Some(t);
        }
    };
    // Cylinder body: quadratic in XZ only (the axis is vertical).
    let ox = ray_start.x - base.x;
    let oz = ray_start.z - base.z;
    let a = ray_dir.x * ray_dir.x + ray_dir.z * ray_dir.z;
    if a > 1.0e-12 {
        let b = 2.0 * (ox * ray_dir.x + oz * ray_dir.z);
        let c = ox * ox + oz * oz - radius * radius;
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let sqrt_disc = disc.sqrt();
            let t0 = (-b - sqrt_disc) / (2.0 * a);
            let t1 = (-b + sqrt_disc) / (2.0 * a);
            // Entry t; when the ray starts inside, use the exit like
            // `ray_sphere_hit_t` does.
            let t_enter = if t0 >= 0.0 { t0 } else { t1 };
            if t_enter >= 0.0 {
                let y = ray_start.y + ray_dir.y * t_enter;
                if y >= base.y && y <= top {
                    consider(t_enter, &mut best);
                }
            }
        }
    }
    // Rounded caps (only the domes outside the body span count).
    if let Some(t) = ray_sphere_hit_t(ray_start, ray_dir, Vec3::new(base.x, base.y, base.z), radius)
    {
        let y = ray_start.y + ray_dir.y * t;
        if y <= base.y {
            consider(t, &mut best);
        }
    }
    if let Some(t) = ray_sphere_hit_t(ray_start, ray_dir, Vec3::new(base.x, top, base.z), radius) {
        let y = ray_start.y + ray_dir.y * t;
        if y >= top {
            consider(t, &mut best);
        }
    }
    best
}

/// Fine pick volume for one frozen object, optionally led forward to
/// `now_logic_frame` (see [`extrapolated_pick_position`]).
pub fn object_hit_along_ray(
    object: &RenderableObject,
    frame_stamp: u32,
    now_logic_frame: u32,
    ray_start: Vec3,
    ray_dir: Vec3,
) -> Option<f32> {
    let center = extrapolated_pick_position(object, frame_stamp, now_logic_frame);
    let radius = presentation_mesh_pick_radius(object.selection_radius);
    let height = object.max_height_above_position;
    if height > 0.0 && height.is_finite() {
        // Coarse broad-phase: the capsule's bounding sphere. A miss here is a
        // miss everywhere; a hit falls through to the exact capsule.
        let half = height * 0.5;
        let broad_center = Vec3::new(center.x, center.y + half, center.z);
        let broad_radius = (radius * radius + half * half).sqrt();
        if ray_sphere_hit_t(ray_start, ray_dir, broad_center, broad_radius).is_none() {
            return None;
        }
        let t = ray_vertical_capsule_hit_t(ray_start, ray_dir, center, height, radius)?;
        (t <= 1.0).then_some(t)
    } else {
        // Final fallback: origin sphere on the frozen 5wu-padded radius
        // (geometry unauthored — no height available).
        let t = ray_sphere_hit_t(ray_start, ray_dir, center, radius)?;
        (t <= 1.0).then_some(t)
    }
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

/// C++ `iterateDrawablesInRegion` point path: `pickDrawable` then priority
/// bands, at the snapshot's own frame (no velocity lead).
pub fn pick_object_id_along_camera_ray(
    frame: &PresentationFrame,
    ray_start: Vec3,
    ray_end: Vec3,
    player_team: Option<Team>,
    prioritize_enemy_targets: bool,
) -> Option<ObjectId> {
    pick_object_id_along_camera_ray_ex(
        frame,
        ray_start,
        ray_end,
        player_team,
        prioritize_enemy_targets,
        frame.frame.0,
    )
}

/// Fresh-state variant of [`pick_object_id_along_camera_ray`]: candidate
/// poses are advanced to `now_logic_frame` (the live logic frame at click
/// time) before the volume test, so a click lands where the unit is rather
/// than where the last freeze stamped it.
pub fn pick_object_id_along_camera_ray_ex(
    frame: &PresentationFrame,
    ray_start: Vec3,
    ray_end: Vec3,
    player_team: Option<Team>,
    prioritize_enemy_targets: bool,
    now_logic_frame: u32,
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
            let t = object_hit_along_ray(object, frame.frame.0, now_logic_frame, ray_start, ray_dir)?;
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
    fn pick_radius_comes_from_geometry_not_the_health_box() {
        // The health-box width is a screen-space UI metric; leaking it into
        // the world-unit pick radius is what over-picked long structures.
        assert_eq!(presentation_mesh_pick_radius(8.0), 8.0);
        assert_eq!(presentation_mesh_pick_radius(5.0), 5.0);
        assert_eq!(
            presentation_mesh_pick_radius(0.0),
            MIN_PICK_SPHERE_RADIUS
        );
        assert!(presentation_mesh_pick_radius(5.0) < 20.0);
    }

    #[test]
    fn torso_height_click_hits_infantry_capsule_not_origin_sphere() {
        // Contract (a): retail infantry geometry is a tall cylinder
        // (12-15wu). The old feet-centered sphere missed torsos/heads
        // whenever health_box_width was not frozen (the GameWorld append
        // path keeps the serde default 0), degenerating the old formula to
        // the bare 5wu-padded selection radius.
        use crate::game_logic::{GameLogic, HostGeometryType, KindOf, ThingTemplate};
        use crate::presentation_frame::PresentationFrame;

        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("CapsuleRanger");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.geometry_info.authored = true;
        t.geometry_info.geom_type = HostGeometryType::Cylinder;
        t.geometry_info.major_radius = 2.0;
        t.geometry_info.minor_radius = 2.0;
        t.geometry_info.height = 15.0;
        logic.templates.insert("CapsuleRanger".into(), t);
        let id = logic
            .create_object("CapsuleRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        {
            let obj = frame.objects.iter().find(|o| o.id == id).expect("row");
            assert!((obj.max_height_above_position - 15.0).abs() < f32::EPSILON);
            assert_eq!(obj.health_box_width, 40.0);
        }
        // Model the live degenerate freeze: no health box on the row.
        frame.objects.iter_mut().for_each(|o| o.health_box_width = 0.0);

        // Click the torso at (3, 8, 0) — inside the visible silhouette.
        let torso = Vec3::new(3.0, 8.0, 0.0);
        let old_radius = presentation_mesh_pick_radius(5.0); // health-box term is 0 on this row
        let camera = Vec3::new(0.0, 60.0, 60.0);
        let old = ray_sphere_hit_t(camera, torso - camera, Vec3::ZERO, old_radius);
        assert!(
            old.is_none() || old.is_some_and(|t| t > 1.0),
            "feet-centered sphere must miss the torso click, got {old:?}"
        );
        assert_eq!(
            pick_object_id_along_camera_ray(&frame, camera, torso, Some(Team::USA), false),
            Some(id)
        );

        // The head at (0, 14, 0) lands on the rounded top cap.
        let head_camera = Vec3::new(0.0, 60.0, 60.0);
        let head = Vec3::new(0.0, 14.0, 0.0);
        let old = ray_sphere_hit_t(head_camera, head - head_camera, Vec3::ZERO, old_radius);
        assert!(
            old.is_none() || old.is_some_and(|t| t > 1.0),
            "feet-centered sphere must miss the head click, got {old:?}"
        );
        assert_eq!(
            pick_object_id_along_camera_ray(&frame, head_camera, head, Some(Team::USA), false),
            Some(id)
        );
    }

    #[test]
    fn click_beside_a_long_structure_no_longer_over_picks_it() {
        // Contract (b): the old radius max(selection_radius,
        // health_box_width*0.5) fattened a 100x20wu structure to a 60wu
        // origin sphere; a capsule on the authored bounding circle (≈51wu)
        // leaves the near-miss ray unpicked.
        use crate::game_logic::{GameLogic, HostGeometryType, KindOf, ThingTemplate};
        use crate::presentation_frame::PresentationFrame;

        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("LongStruct");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        t.geometry_info.authored = true;
        t.geometry_info.geom_type = HostGeometryType::Box;
        t.geometry_info.major_radius = 50.0;
        t.geometry_info.minor_radius = 10.0;
        t.geometry_info.height = 30.0;
        logic.templates.insert("LongStruct".into(), t);
        let _id = logic
            .create_object("LongStruct", Team::USA, Vec3::ZERO)
            .expect("structure");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let obj = frame
            .objects
            .iter()
            .find(|o| o.template_name == "LongStruct")
            .expect("row");
        assert_eq!(obj.health_box_width, 120.0);
        assert!((obj.selection_radius - (50.0f32 * 50.0 + 10.0 * 10.0).sqrt()).abs() < 0.01);

        // A ray passing 55wu beside the structure center, inside its wall.
        let camera = Vec3::new(55.0, 40.0, 80.0);
        let beside = Vec3::new(55.0, 5.0, 0.0);
        let old = ray_sphere_hit_t(camera, beside - camera, Vec3::ZERO, 60.0);
        assert!(
            old.is_some_and(|t| (0.0..=1.0).contains(&t)),
            "old health-box-inflated sphere over-picked this ray, got {old:?}"
        );
        assert_eq!(
            pick_object_id_along_camera_ray(&frame, camera, beside, Some(Team::USA), false),
            None,
            "capsule on the authored extents must not pick the near-miss"
        );
    }

    #[test]
    fn stale_snapshot_pick_leads_moving_units_by_velocity() {
        // Contract (c): a click 3 logic frames after the freeze must pick
        // the unit where it actually is (pose + velocity × 3/30s), not where
        // the stale stamp left it.
        use crate::game_logic::{GameLogic, HostGeometryType, KindOf, ThingTemplate};
        use crate::presentation_frame::PresentationFrame;

        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("FastPlane");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        t.geometry_info.authored = true;
        t.geometry_info.geom_type = HostGeometryType::Cylinder;
        t.geometry_info.major_radius = 4.0;
        t.geometry_info.height = 10.0;
        logic.templates.insert("FastPlane".into(), t);
        logic.set_current_frame(100);
        let id = logic
            .create_object("FastPlane", Team::USA, Vec3::ZERO)
            .expect("unit");
        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        assert_eq!(frame.frame.0, 100, "snapshot must carry the logic stamp");
        {
            let obj = frame.objects.iter_mut().find(|o| o.id == id).expect("row");
            // 150 wu/s × (103−100)/30s = 15wu of lead.
            obj.velocity = Vec3::new(150.0, 0.0, 0.0);
        }

        // The player clicks where the unit is at now=103: (15, ~2, 0).
        let camera = Vec3::new(15.0, 60.0, 60.0);
        let click = Vec3::new(15.0, 2.0, 0.0);
        assert_eq!(
            pick_object_id_along_camera_ray_ex(
                &frame, camera, click, Some(Team::USA), false, 100
            ),
            None,
            "frozen pose at the origin must miss the click"
        );
        assert_eq!(
            pick_object_id_along_camera_ray_ex(
                &frame, camera, click, Some(Team::USA), false, 103
            ),
            Some(id),
            "velocity lead must move the pick volume onto the unit"
        );
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
