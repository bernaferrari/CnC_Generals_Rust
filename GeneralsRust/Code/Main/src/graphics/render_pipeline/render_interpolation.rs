//! Opt-in render interpolation (`-interpolate`).
//!
//! Logic advances in fixed 30 Hz steps while windowed presents may run much
//! faster (`-fps=240 -novsync`). When enabled, object/projectile root
//! transforms from the previous and current presentation logic frames are
//! blended by `alpha = accumulated_time / LOGIC_FRAME_TIMESTEP` and the
//! resulting root correction is applied to every render item owned by that
//! root. Disabled (flag absent) the collector is untouched.

use game_engine::common::fast_hash::FxHashMap;
use glam::Mat4;

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
    /// Presentation frame admitted by `begin_collect` this collect. `None`
    /// disables both recording and correction for that collect.
    roots_frame: Option<u32>,
    prev_roots: FxHashMap<InterpKey, Mat4>,
    curr_roots: FxHashMap<InterpKey, Mat4>,
    /// Per-collect correction cache; computed once per key, not per mesh.
    corrections: FxHashMap<InterpKey, Option<Mat4>>,
}

impl RenderInterpolation {
    /// Rotate root snapshots when the installed presentation logic frame
    /// advances. A consecutive frame hands curr to prev; any jump (load,
    /// reset, replay seek) clears both so nothing blends across it.
    /// Re-collecting the same frame keeps both snapshots untouched.
    fn begin_collect(&mut self, frame_id: Option<u32>) {
        if !self.enabled {
            return;
        }
        self.corrections.clear();
        self.roots_frame = frame_id;
        let Some(id) = frame_id else {
            return;
        };
        if self.curr_frame != Some(id) {
            if self.curr_frame == id.checked_sub(1) {
                self.prev_roots = std::mem::take(&mut self.curr_roots);
            } else {
                self.prev_roots.clear();
            }
            self.curr_roots.clear();
            self.curr_frame = Some(id);
        }
    }

    /// Record one render-space root matrix for this frame. Repeated records
    /// within a frame keep the first value so multiple renders of the same
    /// logic frame stay identical.
    fn record_root(&mut self, key: InterpKey, root: Mat4) {
        if self.enabled && self.roots_frame.is_some() {
            self.curr_roots.entry(key).or_insert(root);
        }
    }

    fn correction(&self, key: InterpKey, alpha: f32) -> Option<Mat4> {
        let prev = self.prev_roots.get(&key).copied()?;
        let curr = self.curr_roots.get(&key).copied()?;
        if prev == curr {
            return Some(Mat4::IDENTITY);
        }
        let blended = blend_roots(prev, curr, alpha)?;
        Some(blended * curr.inverse())
    }

    /// Rewrites `items`' world matrices to the alpha-blended root pose.
    /// `C = blend(prev, curr, alpha) * curr.inverse()` maps the current root
    /// pose onto the blended pose while preserving each mesh's local offset.
    fn apply_to_items(&mut self, items: &mut [RenderItem]) {
        if !self.enabled || self.roots_frame.is_none() {
            return;
        }
        let Some(alpha) = self.alpha else {
            return;
        };
        if !(alpha < 1.0) {
            return;
        }
        for item in items {
            let key = match item.owner {
                RenderItemOwner::Object(id) => InterpKey::Object(id),
                RenderItemOwner::PresentationProjectile(id) => InterpKey::Projectile(id),
                _ => continue,
            };
            let correction = if let Some(cached) = self.corrections.get(&key) {
                *cached
            } else {
                let computed = self.correction(key, alpha);
                self.corrections.insert(key, computed);
                computed
            };
            let Some(correction) = correction else {
                continue;
            };
            item.set_world_matrix(correction * item.world_matrix);
        }
    }
}

/// Blend `alpha` (0 = prev, 1 = curr) between two root matrices. Scale and
/// translation lerp; rotation slerps along the shortest path. `None` when no
/// safe blend exists (non-finite, degenerate, or teleport-distance roots).
fn blend_roots(prev: Mat4, curr: Mat4, alpha: f32) -> Option<Mat4> {
    if !prev.is_finite() || !curr.is_finite() {
        return None;
    }
    if prev.determinant().abs() <= MIN_ROOT_DETERMINANT
        || curr.determinant().abs() <= MIN_ROOT_DETERMINANT
    {
        return None;
    }
    if prev
        .w_axis
        .truncate()
        .distance(curr.w_axis.truncate())
        > TELEPORT_DISTANCE
    {
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

    /// Root rotation at the start of `collect_render_items`.
    pub(super) fn interpolation_begin_collect(
        &mut self,
        frame: Option<&crate::presentation_frame::PresentationFrame>,
    ) {
        self.interpolation.begin_collect(frame.map(|f| f.frame.0));
    }

    /// Base object matrix root for one collected object (not the per-drawable
    /// instance matrix; mesh-local offsets ride the correction).
    pub(super) fn interpolation_record_object_root(&mut self, object_id: ObjectId, root: Mat4) {
        self.interpolation.record_root(InterpKey::Object(object_id), root);
    }

    /// Root for one collected presentation projectile.
    pub(super) fn interpolation_record_projectile_root(
        &mut self,
        projectile_id: ObjectId,
        root: Mat4,
    ) {
        self.interpolation
            .record_root(InterpKey::Projectile(projectile_id), root);
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
        interp.roots_frame = Some(2);
        interp.alpha = alpha;
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

    #[test]
    fn frame_jump_or_decrease_clears_prev() {
        let mut interp = RenderInterpolation {
            enabled: true,
            ..Default::default()
        };
        interp.begin_collect(Some(4));
        interp.record_root(InterpKey::Object(ObjectId(1)), Mat4::IDENTITY);

        // Jump 4 -> 6: prev cleared, no blend.
        interp.begin_collect(Some(6));
        interp.record_root(
            InterpKey::Object(ObjectId(1)),
            Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
        );
        interp.alpha = Some(0.5);
        let mut items = vec![object_item(1, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))];
        interp.apply_to_items(&mut items);
        assert_eq!(items[0].world_position, Vec3::new(10.0, 0.0, 0.0));

        // Consecutive 6 -> 7: prev = frame-6 roots, blend applies.
        interp.begin_collect(Some(7));
        interp.record_root(
            InterpKey::Object(ObjectId(1)),
            Mat4::from_translation(Vec3::new(20.0, 0.0, 0.0)),
        );
        let mut items = vec![object_item(1, Mat4::from_translation(Vec3::new(20.0, 0.0, 0.0)))];
        interp.apply_to_items(&mut items);
        assert!(items[0].world_position.abs_diff_eq(Vec3::new(15.0, 0.0, 0.0), 1e-4));

        // Decrease 7 -> 5 (load/replay seek): prev cleared, no blend.
        interp.begin_collect(Some(5));
        interp.record_root(InterpKey::Object(ObjectId(1)), Mat4::IDENTITY);
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
            ..Default::default()
        };
        interp.begin_collect(Some(4));
        interp.record_root(InterpKey::Object(ObjectId(1)), Mat4::IDENTITY);
        interp.begin_collect(Some(5));
        interp.record_root(
            InterpKey::Object(ObjectId(1)),
            Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
        );

        // Second collect of the same frame must not rotate the snapshots.
        interp.begin_collect(Some(5));
        interp.record_root(
            InterpKey::Object(ObjectId(1)),
            Mat4::from_translation(Vec3::new(99.0, 0.0, 0.0)),
        );
        assert_eq!(
            interp.curr_roots.get(&InterpKey::Object(ObjectId(1))),
            Some(&Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))
        );
        assert_eq!(
            interp.prev_roots.get(&InterpKey::Object(ObjectId(1))),
            Some(&Mat4::IDENTITY)
        );

        interp.alpha = Some(0.5);
        let mut items = vec![object_item(1, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)))];
        interp.apply_to_items(&mut items);
        assert!(items[0].world_position.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-4));
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
