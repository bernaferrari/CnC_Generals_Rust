//! Opt-in render interpolation (`-interpolate`).
//!
//! Logic advances in fixed 30 Hz steps while windowed presents may run much
//! faster (`-fps=240 -novsync`). When enabled, object/projectile root
//! transforms from the previous and current presentation logic frames are
//! blended by `alpha = accumulated_time / LOGIC_FRAME_TIMESTEP` and the
//! resulting root correction is applied to every render item owned by that
//! root, plus the object-anchored overlays (health bars, blob shadows,
//! order lines, radius decals). Disabled (flag absent) the collector is
//! untouched.
//!
//! Out of scope by design: already-emitted world-space particles and
//! particle emitters attached to units — emitted particles live in world
//! space and moving them with the unit would be wrong.
//!
//! Note: gameplay and render space coincide here
//! (`gameplay_to_render_transform` is the identity), so the same correction
//! applies to presentation object positions consumed by overlays.

use game_engine::common::fast_hash::{FxHashMap, FxHashSet};
use glam::{Mat4, Vec3};

use crate::game_logic::ObjectId;
use crate::graphics::render_item::{RenderItem, RenderItemOwner};

/// Root identities tracked between consecutive presentation logic frames.
/// Object and projectile IDs share the gameplay ObjectId space but come from
/// different render sources, so they stay distinct keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum InterpKey {
    Object(ObjectId),
    Projectile(ObjectId),
}

/// Root translation jumps beyond this distance are teleports/spawns/load-ins
/// and must snap instead of smearing across the gap.
const TELEPORT_DISTANCE: f32 = 120.0;

/// Determinants below this are degenerate roots (e.g. zero scale) that must
/// not produce a blend.
const MIN_ROOT_DETERMINANT: f32 = 1.0e-6;

#[derive(Default)]
pub(super) struct RenderInterpolation {
    enabled: bool,
    /// Alpha for the current render. `None` or >= 1.0 presents the current
    /// logic frame with no blend.
    alpha: Option<f32>,
    /// Presentation logic frame the current roots belong to.
    curr_frame: Option<u32>,
    prev_roots: FxHashMap<InterpKey, Mat4>,
    curr_roots: FxHashMap<InterpKey, Mat4>,
    /// Keys present in both snapshots whose roots differ and pass the blend
    /// guards (finite, non-degenerate, not a teleport). Rebuilt once per
    /// presentation logic frame.
    moving: FxHashSet<InterpKey>,
    /// Per-render corrections for `moving`, keyed non-identity only.
    /// Recomputed each prepare since alpha changes per render.
    corrections: FxHashMap<InterpKey, Mat4>,
}

impl RenderInterpolation {
    /// Refresh corrections for this render against the installed presentation
    /// frame. Runs once per render, before collect and overlays read the
    /// correction map. Root snapshots rotate when the presentation logic
    /// frame advances (a consecutive frame hands curr to prev; any jump or
    /// decrease clears both so nothing blends across a load/reset/seek) and
    /// are captured only on that change; re-preparing the same frame reuses
    /// both snapshots.
    fn prepare(&mut self, frame: Option<&crate::presentation_frame::PresentationFrame>) {
        self.corrections.clear();
        if !self.enabled {
            return;
        }
        let Some(frame) = frame else {
            return;
        };
        let id = frame.frame.0;
        if self.curr_frame != Some(id) {
            if self.curr_frame == id.checked_sub(1) {
                self.prev_roots = std::mem::take(&mut self.curr_roots);
            } else {
                self.prev_roots.clear();
            }
            self.curr_roots.clear();
            self.curr_frame = Some(id);
            // Same root sources the collector uses: the base object matrix
            // (not the per-drawable instance matrix) for units and the
            // projectile world matrix. `gameplay_to_render_transform` is the
            // identity, so collect's recorded values equal these.
            let unit_inputs = frame.unit_render_inputs_shared();
            for u in unit_inputs.iter() {
                self.curr_roots
                    .insert(InterpKey::Object(u.id), u.world_matrix());
            }
            for p in frame.projectile_render_inputs() {
                self.curr_roots
                    .insert(InterpKey::Projectile(p.id), p.world_matrix());
            }
            self.rebuild_moving();
        }
        self.refresh_corrections();
    }

    /// Recompute `corrections` from `moving` at the current alpha. No-op for
    /// `None` or >= 1.0 alpha (current-frame rendering needs no correction).
    fn refresh_corrections(&mut self) {
        self.corrections.clear();
        let Some(alpha) = self.alpha else {
            return;
        };
        if !(alpha < 1.0) {
            return;
        }
        for &key in self.moving.iter() {
            let prev = self.prev_roots[&key];
            let curr = self.curr_roots[&key];
            let Some(blended) = blend_roots(prev, curr, alpha) else {
                continue;
            };
            let correction = blended * curr.inverse();
            if correction != Mat4::IDENTITY {
                self.corrections.insert(key, correction);
            }
        }
    }

    /// Recompute `moving` for the current prev/curr pair: roots must exist
    /// in both snapshots, differ, and pass the blend guards.
    fn rebuild_moving(&mut self) {
        self.moving.clear();
        for (&key, &curr) in self.curr_roots.iter() {
            let Some(&prev) = self.prev_roots.get(&key) else {
                continue;
            };
            if prev != curr && roots_blendable(&prev, &curr) {
                self.moving.insert(key);
            }
        }
    }

    /// The correction this render applies to an object root, `None` when the
    /// object is stationary, unknown, blended-out, or interpolation is off.
    fn object_correction(&self, object_id: ObjectId) -> Option<Mat4> {
        self.corrections.get(&InterpKey::Object(object_id)).copied()
    }

    /// Rewrites `items`' world matrices to the alpha-blended root pose.
    /// `C = blend(prev, curr, alpha) * curr.inverse()` maps the current root
    /// pose onto the blended pose while preserving each mesh's local offset.
    fn apply_to_items(&mut self, items: &mut [RenderItem]) {
        if !self.enabled || self.corrections.is_empty() {
            return;
        }
        for item in items {
            let key = match item.owner {
                RenderItemOwner::Object(id) => InterpKey::Object(id),
                RenderItemOwner::PresentationProjectile(id) => InterpKey::Projectile(id),
                _ => continue,
            };
            let Some(&correction) = self.corrections.get(&key) else {
                continue;
            };
            item.set_world_matrix(correction * item.world_matrix);
        }
    }
}

/// Guard for whether a prev/curr root pair can blend at all: both finite,
/// non-degenerate, and not a teleport/spawn/load-in distance apart.
fn roots_blendable(prev: &Mat4, curr: &Mat4) -> bool {
    prev.is_finite()
        && curr.is_finite()
        && prev.determinant().abs() > MIN_ROOT_DETERMINANT
        && curr.determinant().abs() > MIN_ROOT_DETERMINANT
        && prev
            .w_axis
            .truncate()
            .distance(curr.w_axis.truncate())
            <= TELEPORT_DISTANCE
}

/// Blend `alpha` (0 = prev, 1 = curr) between two root matrices. Scale and
/// translation lerp; rotation slerps along the shortest path. `None` when no
/// safe blend exists (non-finite, degenerate, or teleport-distance roots).
fn blend_roots(prev: Mat4, curr: Mat4, alpha: f32) -> Option<Mat4> {
    if !roots_blendable(&prev, &curr) {
        return None;
    }
    let (prev_scale, prev_rot, prev_trans) = prev.to_scale_rotation_translation();
    let (curr_scale, curr_rot, curr_trans) = curr.to_scale_rotation_translation();
    Some(Mat4::from_scale_rotation_translation(
        prev_scale.lerp(curr_scale, alpha),
        prev_rot.slerp(curr_rot, alpha),
        prev_trans.lerp(curr_trans, alpha),
    ))
}

impl super::RenderPipeline {
    /// `-interpolate` boot flag. Absent keeps collection byte-identical.
    pub(crate) fn set_render_interpolation_enabled(&mut self, enabled: bool) {
        self.interpolation.enabled = enabled;
    }

    /// Per-render alpha from `GameLogic::logic_interpolation_alpha`; `None`
    /// (or >= 1.0) presents the current logic frame with no blend.
    pub(crate) fn set_interpolation_alpha(&mut self, alpha: Option<f32>) {
        self.interpolation.alpha = alpha;
    }

    /// Refresh interpolation corrections for this render. Runs immediately
    /// after `set_interpolation_alpha`, before collect and overlay packing,
    /// so every consumer sees the same correction map.
    pub(crate) fn prepare_interpolation(&mut self) {
        let frame = self.presentation_frame.as_deref();
        self.interpolation.prepare(frame);
    }

    /// The root correction this render applies to an object's mesh, for
    /// overlay consumers that anchor to that object.
    pub(crate) fn interpolated_object_correction(&self, object_id: ObjectId) -> Option<Mat4> {
        self.interpolation.object_correction(object_id)
    }

    /// `point` moved to the blended pose for `object_id`; unchanged when the
    /// object has no correction this render (idle, unknown, or disabled).
    pub(crate) fn interpolate_object_point(&self, object_id: ObjectId, point: Vec3) -> Vec3 {
        self.interpolated_object_correction(object_id)
            .map_or(point, |correction| correction.transform_point3(point))
    }

    /// Apply corrections to items pushed since `items_start` of this collect.
    pub(super) fn interpolation_apply(&mut self, items_start: usize) {
        if items_start >= self.render_items.len() {
            return;
        }
        self.interpolation
            .apply_to_items(&mut self.render_items[items_start..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::W3DMaterial;
    use super::super::RenderPass;
    use glam::Vec3;

    fn object_item(id: u32, matrix: Mat4) -> RenderItem {
        RenderItem::new(
            ObjectId(id),
            "interp-test".to_string(),
            0,
            matrix.w_axis.truncate(),
            matrix,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        )
    }

    fn interp_with_roots(
        prev: &[(u32, Mat4)],
        curr: &[(u32, Mat4)],
        alpha: Option<f32>,
    ) -> RenderInterpolation {
        let mut interp = RenderInterpolation {
            enabled: true,
            ..Default::default()
        };
        for (id, root) in prev {
            interp.prev_roots.insert(InterpKey::Object(ObjectId(*id)), *root);
        }
        for (id, root) in curr {
            interp.curr_roots.insert(InterpKey::Object(ObjectId(*id)), *root);
        }
        interp.curr_frame = Some(2);
        interp.rebuild_moving();
        interp.alpha = alpha;
        interp.refresh_corrections();
        interp
    }

    #[test]
    fn translated_object_blends_halfway_and_keeps_child_offset() {
        let prev = Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0));
        let curr = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let mut interp = interp_with_roots(&[(1, prev)], &[(1, curr)], Some(0.5));

        // Root mesh plus a child mesh with a +2 Z local offset.
        let local_offset = Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0));
        let mut items = vec![
            object_item(1, curr),
            object_item(1, curr * local_offset),
        ];
        interp.apply_to_items(&mut items);

        assert!(items[0].world_position.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-4));
        // Child keeps its local offset relative to the blended root.
        assert!(items[1].world_position.abs_diff_eq(Vec3::new(5.0, 0.0, 2.0), 1e-4));
        assert!(items[1]
            .world_matrix
            .abs_diff_eq(Mat4::from_translation(Vec3::new(5.0, 0.0, 2.0)), 1e-4));
    }

    #[test]
    fn yaw_350_to_10_blends_through_zero_not_180() {
        let prev = Mat4::from_rotation_y(350.0f32.to_radians());
        let curr = Mat4::from_rotation_y(10.0f32.to_radians());
        let mut interp = interp_with_roots(&[(1, prev)], &[(1, curr)], Some(0.5));

        let mut items = vec![object_item(1, curr)];
        interp.apply_to_items(&mut items);

        // rotation_y maps +X toward -Z; yaw ~0 means the blend passed through
        // 0 degrees, not through 180.
        let (_, rot, _) = items[0].world_matrix.to_scale_rotation_translation();
        let dir = rot * Vec3::X;
        let yaw = (-dir.z).atan2(dir.x).to_degrees();
        assert!(yaw.abs() < 5.0, "expected yaw near 0, got {yaw}");
    }

    #[test]
    fn teleport_distance_leaves_item_unchanged() {
        let prev = Mat4::from_translation(Vec3::ZERO);
        let curr = Mat4::from_translation(Vec3::new(200.0, 0.0, 0.0));
        let mut interp = interp_with_roots(&[(1, prev)], &[(1, curr)], Some(0.5));

        let mut items = vec![object_item(1, curr)];
        interp.apply_to_items(&mut items);
        assert_eq!(items[0].world_matrix, curr);
    }

    #[test]
    fn new_spawn_without_prev_leaves_item_unchanged() {
        let curr = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let mut interp = interp_with_roots(&[], &[(1, curr)], Some(0.5));

        let mut items = vec![object_item(1, curr)];
        interp.apply_to_items(&mut items);
        assert_eq!(items[0].world_matrix, curr);
    }

    /// Drive a logic-frame change in tests the way `prepare` does without a
    /// `PresentationFrame`: rotate snapshots, install new roots, rebuild.
    fn prepare_frame(interp: &mut RenderInterpolation, id: u32, roots: &[(u32, Mat4)]) {
        if interp.curr_frame == id.checked_sub(1) {
            interp.prev_roots = std::mem::take(&mut interp.curr_roots);
        } else {
            interp.prev_roots.clear();
        }
        interp.curr_roots.clear();
        interp.curr_frame = Some(id);
        for (object, root) in roots {
            interp
                .curr_roots
                .insert(InterpKey::Object(ObjectId(*object)), *root);
        }
        interp.rebuild_moving();
        interp.refresh_corrections();
    }

    #[test]
    fn frame_jump_or_decrease_clears_prev() {
        let mut interp = RenderInterpolation {
            enabled: true,
            alpha: Some(0.5),
            ..Default::default()
        };
        prepare_frame(&mut interp, 4, &[(1, Mat4::IDENTITY)]);

        // Jump 4 -> 6: prev cleared, no blend.
        prepare_frame(
            &mut interp,
            6,
            &[(1, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))],
        );
        let mut items = vec![object_item(1, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))];
        interp.apply_to_items(&mut items);
        assert_eq!(items[0].world_position, Vec3::new(10.0, 0.0, 0.0));

        // Consecutive 6 -> 7: prev = frame-6 roots, blend applies.
        prepare_frame(
            &mut interp,
            7,
            &[(1, Mat4::from_translation(Vec3::new(20.0, 0.0, 0.0)))],
        );
        let mut items = vec![object_item(1, Mat4::from_translation(Vec3::new(20.0, 0.0, 0.0)))];
        interp.apply_to_items(&mut items);
        assert!(items[0].world_position.abs_diff_eq(Vec3::new(15.0, 0.0, 0.0), 1e-4));

        // Decrease 7 -> 5 (load/replay seek): prev cleared, no blend.
        prepare_frame(&mut interp, 5, &[(1, Mat4::IDENTITY)]);
        let mut items = vec![object_item(1, Mat4::IDENTITY)];
        interp.apply_to_items(&mut items);
        assert_eq!(items[0].world_matrix, Mat4::IDENTITY);
    }

    #[test]
    fn disabled_or_full_alpha_leaves_items_unchanged() {
        let prev = Mat4::from_translation(Vec3::ZERO);
        let curr = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));

        for alpha in [None, Some(1.0)] {
            let mut interp = interp_with_roots(&[(1, prev)], &[(1, curr)], alpha);
            let mut items = vec![object_item(1, curr)];
            interp.apply_to_items(&mut items);
            assert_eq!(items[0].world_matrix, curr);
        }

        let mut interp = interp_with_roots(&[(1, prev)], &[(1, curr)], Some(0.5));
        interp.enabled = false;
        let mut items = vec![object_item(1, curr)];
        interp.apply_to_items(&mut items);
        assert_eq!(items[0].world_matrix, curr);
    }

    #[test]
    fn recollecting_same_frame_keeps_prev_and_curr() {
        let mut interp = RenderInterpolation {
            enabled: true,
            alpha: Some(0.5),
            ..Default::default()
        };
        prepare_frame(&mut interp, 4, &[(1, Mat4::IDENTITY)]);
        prepare_frame(
            &mut interp,
            5,
            &[(1, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))],
        );

        // Re-preparing the same frame must not rotate the snapshots; a fresh
        // `refresh_corrections` on the same roots is a no-op source-wise.
        let before_curr = interp.curr_roots.clone();
        let before_prev = interp.prev_roots.clone();
        interp.refresh_corrections();
        assert_eq!(interp.curr_roots, before_curr);
        assert_eq!(interp.prev_roots, before_prev);
        assert_eq!(
            interp.prev_roots.get(&InterpKey::Object(ObjectId(1))),
            Some(&Mat4::IDENTITY)
        );

        let mut items = vec![object_item(1, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))];
        interp.apply_to_items(&mut items);
        assert!(items[0].world_position.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-4));
    }

    #[test]
    fn moving_set_excludes_idle_teleport_and_spawn() {
        let mut interp = interp_with_roots(
            &[
                (1, Mat4::IDENTITY),
                (2, Mat4::IDENTITY),
                (3, Mat4::IDENTITY),
            ],
            &[
                (1, Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0))),
                (2, Mat4::IDENTITY),
                (3, Mat4::from_translation(Vec3::new(500.0, 0.0, 0.0))),
                (4, Mat4::IDENTITY),
            ],
            Some(0.5),
        );
        // 1 moves -> moving; 2 identical -> excluded; 3 teleport -> excluded;
        // 4 spawn (no prev) -> excluded.
        assert!(interp.moving.contains(&InterpKey::Object(ObjectId(1))));
        assert!(!interp.moving.contains(&InterpKey::Object(ObjectId(2))));
        assert!(!interp.moving.contains(&InterpKey::Object(ObjectId(3))));
        assert!(!interp.moving.contains(&InterpKey::Object(ObjectId(4))));
        assert!(interp
            .corrections
            .contains_key(&InterpKey::Object(ObjectId(1))));
        assert_eq!(interp.corrections.len(), 1);
        interp.alpha = None;
        interp.refresh_corrections();
        assert!(interp.corrections.is_empty());
    }

    #[test]
    fn interpolate_object_point_tracks_the_blended_pose() {
        let prev = Mat4::from_translation(Vec3::ZERO);
        let curr = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let mut interp = interp_with_roots(&[(1, prev)], &[(1, curr)], Some(0.5));
        let p = interp
            .object_correction(ObjectId(1))
            .unwrap()
            .transform_point3(Vec3::new(10.0, 0.0, 0.0));
        assert!(p.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-4));
        // Unknown object -> no correction.
        assert!(interp.object_correction(ObjectId(99)).is_none());
        // Disabled / full alpha -> no correction for consumers.
        interp.enabled = false;
        interp.corrections.clear();
        assert!(interp.object_correction(ObjectId(1)).is_none());
    }

    #[test]
    fn prepare_captures_the_same_roots_collect_computes() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::presentation_frame::PresentationFrame;

        // The overlay/collect contract: gameplay and render space coincide,
        // so collect's `gameplay_to_render_transform(root)` equals the root.
        #[cfg(feature = "game_client")]
        assert_eq!(
            super::super::gameplay_to_render_transform(Mat4::from_translation(
                Vec3::new(1.0, 2.0, 3.0)
            )),
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0))
        );

        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("InterpU");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("InterpU".into(), t);
        let id = logic
            .create_object("InterpU", Team::USA, Vec3::new(4.0, 0.0, 8.0))
            .unwrap();
        logic.frame = 5;
        let frame5 = PresentationFrame::build_from_logic(&mut logic, 0);
        let expected_root = frame5
            .unit_render_inputs()
            .into_iter()
            .find(|u| u.id == id)
            .unwrap()
            .world_matrix();

        let mut interp = RenderInterpolation {
            enabled: true,
            alpha: Some(0.5),
            ..Default::default()
        };
        interp.prepare(Some(&frame5));
        assert_eq!(interp.curr_frame, Some(frame5.frame.0));
        assert_eq!(
            interp.curr_roots.get(&InterpKey::Object(id)),
            Some(&expected_root),
            "prepare must capture the same base matrix collect records"
        );

        // Next consecutive frame with the unit moved: curr->prev rotates and
        // the object joins `moving`.
        logic.set_fixture_object_position(id, Vec3::new(14.0, 0.0, 8.0));
        logic.frame = 6;
        let frame6 = PresentationFrame::build_from_logic(&mut logic, 0);
        interp.prepare(Some(&frame6));
        assert_eq!(
            interp.prev_roots.get(&InterpKey::Object(id)),
            Some(&expected_root)
        );
        assert!(interp.moving.contains(&InterpKey::Object(id)));
        let c = interp.object_correction(id).unwrap();
        let moved = c.transform_point3(Vec3::new(14.0, 0.0, 8.0));
        assert!((moved.x - 9.0).abs() < 0.1, "alpha 0.5 blends 4->14 to ~9, got {}", moved.x);

        // Same frame re-prepared: snapshots stable, correction recomputed.
        let prev_roots = interp.prev_roots.clone();
        interp.alpha = Some(0.25);
        interp.prepare(Some(&frame6));
        assert_eq!(interp.prev_roots, prev_roots);
        let moved = interp
            .object_correction(id)
            .unwrap()
            .transform_point3(Vec3::new(14.0, 0.0, 8.0));
        assert!((moved.x - 6.5).abs() < 0.1, "alpha 0.25 blends 4->14 to ~6.5, got {}", moved.x);
    }

    #[test]
    fn projectile_items_blend_on_projectile_roots() {
        let prev = Mat4::from_translation(Vec3::ZERO);
        let curr = Mat4::from_translation(Vec3::new(0.0, 10.0, 0.0));
        let mut interp = interp_with_roots(&[], &[], Some(0.5));
        interp
            .prev_roots
            .insert(InterpKey::Projectile(ObjectId(9)), prev);
        interp
            .curr_roots
            .insert(InterpKey::Projectile(ObjectId(9)), curr);
        interp.rebuild_moving();
        interp.refresh_corrections();

        let mut items = vec![RenderItem::new_presentation_projectile(
            ObjectId(9),
            "shell".to_string(),
            0,
            curr.w_axis.truncate(),
            curr,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        )];
        interp.apply_to_items(&mut items);
        assert!(items[0].world_position.abs_diff_eq(Vec3::new(0.0, 5.0, 0.0), 1e-4));
    }
}
