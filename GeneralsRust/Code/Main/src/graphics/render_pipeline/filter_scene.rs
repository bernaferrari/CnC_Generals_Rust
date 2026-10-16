//! W3DView::draw (1525–1574): a refreshed filtered scene omits the
//! camera-lock Drawable, then renders that exact binding after postRender.
use super::*;

/// The short-lived key comes from this collection's current guarded sidecar,
/// never from a position, a raw Drawable registry, or another session's lock.
pub(super) fn camera_lock_binding(
    object_id: Option<ObjectID>,
    sidecar: &HashMap<ObjectID, FrozenDirectDrawableShroudState>,
    host_epoch: Option<u64>,
    candidates: &[FrozenDirectDrawableSceneCandidate],
) -> Option<FrozenDirectDrawableSceneCandidate> {
    let object_id = object_id?;
    candidates.iter().copied().find(|candidate| {
        candidate.object_id == object_id
            && frozen_direct_scene_decision_matches_current_binding(
                sidecar,
                host_epoch,
                candidates,
                FrozenDirectDrawableSceneDecision {
                    host_epoch: candidate.host_epoch,
                    object_id,
                    drawable_id: candidate.drawable_id,
                    binding_generation: candidate.binding_generation,
                    outcome: FrozenDirectDrawableSceneOutcome::HiddenDirectDrawable,
                },
            )
    })
}

/// Only a direct item admitted by the exact scene decision can be split off.
/// Standalone Drawables, projectiles, ghosts and hidden/missing items must not
/// be resurrected by the special draw, even when their numeric ID is equal.
pub(super) fn is_locked_filter_item(
    item: &RenderItem,
    binding: Option<FrozenDirectDrawableSceneCandidate>,
) -> bool {
    binding.is_some_and(|binding| {
        item.owner == RenderItemOwner::Object(binding.object_id)
            && item.frozen_direct_scene_shroud.is_some()
            && item.ghost_render_state.is_none()
            && item.render_pass != RenderPass::Ghost
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::common::types::ObjectShroudStatus;

    fn candidate() -> FrozenDirectDrawableSceneCandidate {
        FrozenDirectDrawableSceneCandidate {
            host_epoch: 3,
            object_id: ObjectID(7),
            drawable_id: 9,
            binding_generation: 11,
            raw_status: ObjectShroudStatus::Clear,
            effectively_dead: false,
        }
    }
    #[test]
    fn locked_filter_requires_current_full_binding_and_visible_scene_candidate() {
        let c = candidate();
        let state = FrozenDirectDrawableShroudState {
            host_epoch: c.host_epoch,
            object_id: c.object_id,
            drawable_id: c.drawable_id,
            binding_generation: c.binding_generation,
            scene_effectively_hidden: false,
            fully_obscured: false,
        };
        let mut sidecar = HashMap::from([(c.object_id, state)]);
        assert_eq!(
            camera_lock_binding(Some(c.object_id), &sidecar, Some(3), &[c]),
            Some(c)
        );
        assert_eq!(camera_lock_binding(None, &sidecar, Some(3), &[c]), None);
        assert_eq!(
            camera_lock_binding(Some(c.object_id), &sidecar, Some(4), &[c]),
            None
        );
        assert_eq!(
            camera_lock_binding(Some(c.object_id), &sidecar, Some(3), &[]),
            None
        );
        for changed in [
            FrozenDirectDrawableShroudState {
                host_epoch: 4,
                ..state
            },
            FrozenDirectDrawableShroudState {
                drawable_id: 10,
                ..state
            },
            FrozenDirectDrawableShroudState {
                binding_generation: 12,
                ..state
            },
            FrozenDirectDrawableShroudState {
                scene_effectively_hidden: true,
                ..state
            },
            FrozenDirectDrawableShroudState {
                fully_obscured: true,
                ..state
            },
        ] {
            sidecar.insert(c.object_id, changed);
            assert_eq!(
                camera_lock_binding(Some(c.object_id), &sidecar, Some(3), &[c]),
                None
            );
        }
    }
    #[test]
    fn locked_filter_splits_only_admitted_object_meshes_not_equal_numeric_domains() {
        let c = candidate();
        let mut item = RenderItem::new(
            c.object_id,
            "lock".into(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        assert!(!is_locked_filter_item(&item, Some(c)));
        item.set_frozen_direct_scene_shroud(FrozenDirectSceneShroudRenderState {
            final_status: ObjectShroudStatus::Clear,
            pushes_projected_shroud_pass: false,
        });
        assert!(is_locked_filter_item(&item, Some(c)));
        assert!(!is_locked_filter_item(&item, None));
        item.owner = RenderItemOwner::Object(ObjectID(8));
        assert!(!is_locked_filter_item(&item, Some(c)));
        item.owner = RenderItemOwner::PresentationProjectile(c.object_id);
        assert!(!is_locked_filter_item(&item, Some(c)));
        item.owner = RenderItemOwner::UnboundClientDrawable(c.object_id.0);
        assert!(!is_locked_filter_item(&item, Some(c)));
        item.owner = RenderItemOwner::Object(c.object_id);
        item.render_pass = RenderPass::Ghost;
        assert!(!is_locked_filter_item(&item, Some(c)));
    }
}
