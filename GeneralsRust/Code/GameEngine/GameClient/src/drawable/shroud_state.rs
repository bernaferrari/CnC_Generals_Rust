//! Drawable-owned shroud-clear timing used by the C++ GameClient and W3D scene.
//!
//! C++ stores `Drawable::m_shroudClearFrame` as volatile client state.  It is
//! written only by `RTS3DScene::renderOneObject` when an eligible direct
//! drawable reaches that scene pass with `OBJECTSHROUD_CLEAR`
//! (`W3DScene.cpp:553-625`).  `GameClient::update` then uses the same value
//! for its `m_drawableFullyObscuredByShroud` decision, but deliberately turns
//! a grace-period object into *Clear* there while the scene turns it into
//! *PartialClear*.  Keeping the two evaluations separate is essential: the
//! latter still receives W3D's projected shroud material pass.
//!
//! This module is pure client state.  It deliberately has no Xfer support:
//! C++ initializes the field to zero and does not serialize it in
//! `Drawable::xfer` (`Drawable.cpp:382, 5161+`).  A later view-dispatch
//! adapter owns lifecycle calls on drawable creation, replacement, successful
//! load reconstruction, destruction, and world reset.  C++ does *not* reset
//! the timer during an ordinary `Drawable::friend_bindToObject` rebind, so a
//! live Drawable instance must retain it across that operation.  The adapter
//! must never infer this timer from scalar FOW alpha.

use gamelogic::common::types::ObjectShroudStatus;

/// C++ `LOGICFRAMES_PER_SECOND` for this source path.
pub const LOGIC_FRAMES_PER_SECOND: u32 = 30;
const CLEAR_GRACE_FRAMES: u32 = 2 * LOGIC_FRAMES_PER_SECOND;
const DEAD_CLEAR_GRACE_EXTENSION_FRAMES: u32 = 3 * LOGIC_FRAMES_PER_SECOND;

/// Volatile C++ `Drawable::m_shroudClearFrame` state.
///
/// Zero is intentionally both the initial value and the C++ sentinel.  A
/// direct clear dispatch at logic frame zero therefore remains unable to
/// create a later grace period; do not replace this with an `Option` or a
/// nonzero frame encoding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrawableShroudClearState {
    clear_frame: u32,
}

impl DrawableShroudClearState {
    /// Current raw C++ frame value.  Zero means no usable clear history.
    #[must_use]
    pub const fn clear_frame(self) -> u32 {
        self.clear_frame
    }

    /// Forget volatile state after a new/replaced drawable, successful load
    /// reconstruction, destruction, or a complete world reset.  Do not call
    /// this for an ordinary live `friend_bindToObject` rebind: C++ retains the
    /// field there.
    pub fn reset(&mut self) {
        self.clear_frame = 0;
    }

    /// Evaluate the direct-drawable branch of C++
    /// `RTS3DScene::renderOneObject`.
    ///
    /// Call this only after the frozen view has selected the render object as
    /// a scene candidate.  Passing an off-view object here would incorrectly
    /// refresh `m_shroudClearFrame`; C++ does not call `renderOneObject` for
    /// that case.  An effectively hidden candidate returns before the status
    /// query and also must not refresh the timestamp.
    #[must_use]
    pub fn evaluate_scene_direct(
        &mut self,
        logic_frame: u32,
        raw_status: ObjectShroudStatus,
        effectively_dead: bool,
        effectively_hidden: bool,
    ) -> SceneShroudDecision {
        if effectively_hidden {
            return SceneShroudDecision::HiddenDirectDrawable;
        }

        let final_status = if raw_status == ObjectShroudStatus::Clear {
            // Exact source ordering: write even at frame zero, where zero
            // remains the later sentinel by design.
            self.clear_frame = logic_frame;
            raw_status
        } else if is_fogged_or_worse(raw_status)
            && self.has_active_clear_grace(logic_frame, effectively_dead)
        {
            ObjectShroudStatus::PartialClear
        } else {
            raw_status
        };

        SceneShroudDecision::RenderDrawable {
            final_status,
            pushes_projected_shroud_pass: pushes_projected_shroud_pass(final_status),
        }
    }

    /// Evaluate the separate C++ `GameClient::update` obscuration decision.
    ///
    /// Unlike [`Self::evaluate_scene_direct`], this does not mutate state and
    /// changes an active grace-period status to `Clear`, not `PartialClear`.
    /// C++ runs it while updating all client drawables, including objects that
    /// are outside the current W3D view.
    #[must_use]
    pub fn evaluate_client_visibility(
        self,
        logic_frame: u32,
        raw_status: ObjectShroudStatus,
        effectively_dead: bool,
    ) -> ClientShroudVisibility {
        let effective_status = if is_fogged_or_worse(raw_status)
            && self.has_active_clear_grace(logic_frame, effectively_dead)
        {
            ObjectShroudStatus::Clear
        } else {
            raw_status
        };

        ClientShroudVisibility {
            effective_status,
            fully_obscured: is_fogged_or_worse(effective_status),
        }
    }

    fn has_active_clear_grace(self, logic_frame: u32, effectively_dead: bool) -> bool {
        if self.clear_frame == 0 {
            return false;
        }
        // `UnsignedInt` arithmetic and comparison are intentional: C++ uses
        // `frame < limit + m_shroudClearFrame`, so retain its wrapping sum and
        // ordinary unsigned comparison rather than inventing saturating time.
        logic_frame
            < self
                .clear_frame
                .wrapping_add(clear_grace_frames(effectively_dead))
    }
}

/// C++ scene outcome for one already-selected RenderObj.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneShroudDecision {
    /// `Drawable::isDrawableEffectivelyHidden()` returned true.  C++ exits
    /// before a shroud query or render.
    HiddenDirectDrawable,
    /// A normal direct drawable reached its render path.
    RenderDrawable {
        /// Final scene-local `ObjectShroudStatus` after clear-grace handling.
        final_status: ObjectShroudStatus,
        /// Exact `ss > OBJECTSHROUD_CLEAR` material-pass condition.
        pushes_projected_shroud_pass: bool,
    },
    /// `DrawableInfo` existed but its Drawable was null.  C++ renders this
    /// ghost through its fog-light environment and returns before the normal
    /// projected shroud pass.
    RenderGhostWithFogLight,
    /// A normal RenderObj had no `DrawableInfo`; C++ renders it normally.
    RenderWithoutDrawable,
}

/// C++ GameClient update result for a bound direct drawable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientShroudVisibility {
    /// Local status after C++'s grace-only update adjustment.
    pub effective_status: ObjectShroudStatus,
    /// Value passed to `Drawable::setFullyObscuredByShroud`.
    pub fully_obscured: bool,
}

/// Evaluate the C++ object-less Drawable branch.
///
/// A hidden Drawable exits before this branch.  Otherwise a missing anchor or
/// an anchor below `Fogged` stays clear; an anchor at `Fogged` or above forces
/// `Shrouded`, exactly as the prison-camp special case in
/// `RTS3DScene::renderOneObject` does.  There is no clear timer for this
/// branch.
#[must_use]
pub const fn evaluate_objectless_drawable_scene(
    shroud_status_anchor: Option<ObjectShroudStatus>,
    effectively_hidden: bool,
) -> SceneShroudDecision {
    if effectively_hidden {
        return SceneShroudDecision::HiddenDirectDrawable;
    }
    let final_status = match shroud_status_anchor {
        Some(status) if is_fogged_or_worse(status) => ObjectShroudStatus::Shrouded,
        _ => ObjectShroudStatus::Clear,
    };
    SceneShroudDecision::RenderDrawable {
        final_status,
        pushes_projected_shroud_pass: pushes_projected_shroud_pass(final_status),
    }
}

/// Evaluate a W3D ghost `DrawableInfo` whose Drawable is null.
#[must_use]
pub const fn evaluate_ghost_scene() -> SceneShroudDecision {
    SceneShroudDecision::RenderGhostWithFogLight
}

/// Evaluate a RenderObj without `DrawableInfo`.
#[must_use]
pub const fn evaluate_non_drawable_scene() -> SceneShroudDecision {
    SceneShroudDecision::RenderWithoutDrawable
}

#[must_use]
pub const fn pushes_projected_shroud_pass(status: ObjectShroudStatus) -> bool {
    (status as u8) > (ObjectShroudStatus::Clear as u8)
}

#[must_use]
pub const fn is_fogged_or_worse(status: ObjectShroudStatus) -> bool {
    (status as u8) >= (ObjectShroudStatus::Fogged as u8)
}

const fn clear_grace_frames(effectively_dead: bool) -> u32 {
    if effectively_dead {
        CLEAR_GRACE_FRAMES + DEAD_CLEAR_GRACE_EXTENSION_FRAMES
    } else {
        CLEAR_GRACE_FRAMES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_clear_records_the_scene_timestamp_and_skips_the_pass() {
        let mut state = DrawableShroudClearState::default();

        assert_eq!(
            state.evaluate_scene_direct(17, ObjectShroudStatus::Clear, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Clear,
                pushes_projected_shroud_pass: false,
            }
        );
        assert_eq!(state.clear_frame(), 17);
    }

    #[test]
    fn scene_uses_partial_clear_for_exactly_the_two_second_grace_window() {
        let mut state = DrawableShroudClearState::default();
        let _ = state.evaluate_scene_direct(10, ObjectShroudStatus::Clear, false, false);

        assert_eq!(
            state.evaluate_scene_direct(69, ObjectShroudStatus::Fogged, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::PartialClear,
                pushes_projected_shroud_pass: true,
            }
        );
        assert_eq!(
            state.evaluate_scene_direct(70, ObjectShroudStatus::Fogged, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Fogged,
                pushes_projected_shroud_pass: true,
            }
        );
    }

    #[test]
    fn dead_drawables_extend_the_scene_grace_to_five_seconds() {
        let mut state = DrawableShroudClearState::default();
        let _ = state.evaluate_scene_direct(10, ObjectShroudStatus::Clear, false, false);

        assert_eq!(
            state.evaluate_scene_direct(159, ObjectShroudStatus::Shrouded, true, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::PartialClear,
                pushes_projected_shroud_pass: true,
            }
        );
        assert_eq!(
            state.evaluate_scene_direct(160, ObjectShroudStatus::Shrouded, true, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Shrouded,
                pushes_projected_shroud_pass: true,
            }
        );
    }

    #[test]
    fn client_visibility_and_scene_status_are_intentionally_different_during_grace() {
        let mut state = DrawableShroudClearState::default();
        let _ = state.evaluate_scene_direct(10, ObjectShroudStatus::Clear, false, false);

        assert_eq!(
            state.evaluate_client_visibility(69, ObjectShroudStatus::Fogged, false),
            ClientShroudVisibility {
                effective_status: ObjectShroudStatus::Clear,
                fully_obscured: false,
            }
        );
        assert_eq!(
            state.evaluate_scene_direct(69, ObjectShroudStatus::Fogged, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::PartialClear,
                pushes_projected_shroud_pass: true,
            }
        );
    }

    #[test]
    fn zero_frame_clear_is_the_source_sentinel_not_a_grace_timestamp() {
        let mut state = DrawableShroudClearState::default();
        let _ = state.evaluate_scene_direct(0, ObjectShroudStatus::Clear, false, false);

        assert_eq!(state.clear_frame(), 0);
        assert_eq!(
            state.evaluate_scene_direct(1, ObjectShroudStatus::Fogged, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Fogged,
                pushes_projected_shroud_pass: true,
            }
        );
    }

    #[test]
    fn hidden_direct_drawable_cannot_refresh_clear_history() {
        let mut state = DrawableShroudClearState::default();
        let _ = state.evaluate_scene_direct(10, ObjectShroudStatus::Clear, false, false);

        assert_eq!(
            state.evaluate_scene_direct(69, ObjectShroudStatus::Clear, false, true),
            SceneShroudDecision::HiddenDirectDrawable
        );
        assert_eq!(state.clear_frame(), 10);
        assert_eq!(
            state.evaluate_scene_direct(70, ObjectShroudStatus::Fogged, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Fogged,
                pushes_projected_shroud_pass: true,
            }
        );
    }

    #[test]
    fn raw_partial_clear_and_invalid_keep_their_source_pass_ordering() {
        let mut state = DrawableShroudClearState::default();

        assert_eq!(
            state.evaluate_scene_direct(31, ObjectShroudStatus::PartialClear, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::PartialClear,
                pushes_projected_shroud_pass: true,
            }
        );
        assert_eq!(
            state.evaluate_scene_direct(31, ObjectShroudStatus::Invalid, false, false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Invalid,
                pushes_projected_shroud_pass: false,
            }
        );
        assert_eq!(
            state.evaluate_client_visibility(31, ObjectShroudStatus::Invalid, false),
            ClientShroudVisibility {
                effective_status: ObjectShroudStatus::Invalid,
                fully_obscured: false,
            }
        );
    }

    #[test]
    fn objectless_anchor_and_ghost_use_their_distinct_scene_branches() {
        assert_eq!(
            evaluate_objectless_drawable_scene(Some(ObjectShroudStatus::PartialClear), false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Clear,
                pushes_projected_shroud_pass: false,
            }
        );
        assert_eq!(
            evaluate_objectless_drawable_scene(Some(ObjectShroudStatus::Fogged), false),
            SceneShroudDecision::RenderDrawable {
                final_status: ObjectShroudStatus::Shrouded,
                pushes_projected_shroud_pass: true,
            }
        );
        assert_eq!(
            evaluate_ghost_scene(),
            SceneShroudDecision::RenderGhostWithFogLight
        );
        assert_eq!(
            evaluate_non_drawable_scene(),
            SceneShroudDecision::RenderWithoutDrawable
        );
        assert_eq!(
            evaluate_objectless_drawable_scene(None, true),
            SceneShroudDecision::HiddenDirectDrawable
        );
    }

    #[test]
    fn reset_for_lifecycle_replacement_drops_old_clear_history() {
        let mut state = DrawableShroudClearState::default();
        let _ = state.evaluate_scene_direct(10, ObjectShroudStatus::Clear, false, false);
        state.reset();

        assert_eq!(state.clear_frame(), 0);
        assert_eq!(
            state.evaluate_client_visibility(11, ObjectShroudStatus::Fogged, false),
            ClientShroudVisibility {
                effective_status: ObjectShroudStatus::Fogged,
                fully_obscured: true,
            }
        );
    }
}
