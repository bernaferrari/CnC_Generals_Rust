//! Frozen, CPU-only scheduling contract for C++ client `Drawable::draw`.
//!
//! This is deliberately *not* a renderer hook.  C++ first updates client
//! Drawable visibility in `GameClient.cpp:660-700`, then
//! `W3DDisplay::updateViews` runs before particle/render work
//! (`GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp:1824-1838`).
//! Each `W3DView` obtains its region and invokes `drawDrawable` once per
//! candidate (`W3DView.cpp:677-681, 1362-1368`); that worker calls
//! `Drawable::draw`.  `Drawable::draw` applies its hidden/stealth/shroud
//! early-out and otherwise invokes Draw modules in declaration order
//! (`GameClient/Drawable.cpp:2611-2655`).
//!
//! Main currently has no frozen, complete source Draw-module input from which
//! to invoke that behavior.  This module therefore only validates and lowers
//! an already-frozen candidate list into ordered commands.  The future host
//! boundary must construct [`PreparedVisualFrame`] after visibility/update
//! facts are frozen and before WGPU collects render items.  Neither this
//! module nor a consumer of its commands may read live simulation state.

use crate::game_logic::{INVALID_OBJECT_ID, ObjectId};
use std::collections::HashSet;

/// Uniquely identifies one C++-equivalent view update.
///
/// `visual_frame_epoch` is intentionally a presentation/display epoch, not a
/// logic frame.  C++ may redraw a view more than once while the simulation is
/// unchanged.  `view_ordinal` then distinguishes the views updated by one
/// display frame, so the same Drawable may validly be dispatched once for
/// each distinct view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisualDispatchPassToken {
    pub visual_frame_epoch: u64,
    pub view_ordinal: u32,
}

impl VisualDispatchPassToken {
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.visual_frame_epoch != 0
    }
}

/// The exact client Drawable identity being scheduled.
///
/// C++ `DrawableID` and gameplay `ObjectID` are separate identity domains.
/// A placement preview, FX list, or other standalone client drawable has no
/// `ObjectID` and must never be made to look like an object merely to pass
/// FOW or renderer bookkeeping.  A bound object additionally carries the
/// authority generation so a recycled raw `ObjectID` cannot inherit a stale
/// visual command after reset/restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreparedVisualOwner {
    BoundObject {
        object_id: ObjectId,
        object_generation: u64,
        drawable_id: u32,
    },
    StandaloneClientDrawable {
        drawable_id: u32,
    },
}

impl PreparedVisualOwner {
    #[must_use]
    pub const fn drawable_id(self) -> u32 {
        match self {
            Self::BoundObject { drawable_id, .. }
            | Self::StandaloneClientDrawable { drawable_id } => drawable_id,
        }
    }

    #[must_use]
    pub const fn is_standalone(self) -> bool {
        matches!(self, Self::StandaloneClientDrawable { .. })
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        match self {
            Self::BoundObject {
                object_id,
                object_generation,
                drawable_id,
            } => object_id.0 != INVALID_OBJECT_ID.0 && object_generation != 0 && drawable_id != 0,
            Self::StandaloneClientDrawable { drawable_id } => drawable_id != 0,
        }
    }
}

/// Frozen reason that a candidate does or does not reach C++ Draw modules.
///
/// `OutsideView` is decided by the frozen view-region query and never calls
/// `Drawable::draw`.  The other non-visible variants did reach the candidate
/// pass; a bridge must clear their current output so an earlier visible pass
/// cannot leave a stale W3D submission alive after C++ returns early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedVisualVisibility {
    OutsideView,
    Visible,
    Hidden,
    HiddenByStealth,
    FullyObscuredByShroud,
}

impl PreparedVisualVisibility {
    #[must_use]
    pub const fn visits_drawable(self) -> bool {
        !matches!(self, Self::OutsideView)
    }

    #[must_use]
    pub const fn dispatches_modules(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// One Draw module in the exact C++ declaration order.
///
/// `declaration_ordinal` is the source Draw-module index, never a model or
/// mesh index.  A future source freezer must provide every module reached by
/// C++ `Drawable::draw`, including non-W3D modules that can have visual side
/// effects.  The dispatcher refuses an incomplete/ambiguous name rather than
/// silently collapsing modules into a renderer heuristic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedVisualDrawModule {
    pub declaration_ordinal: u32,
    pub source_name: String,
}

impl PreparedVisualDrawModule {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.source_name.trim().is_empty()
    }
}

/// A single frozen Drawable candidate for one [`VisualDispatchPassToken`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedVisualDrawable {
    pub owner: PreparedVisualOwner,
    pub visibility: PreparedVisualVisibility,
    pub draw_modules: Vec<PreparedVisualDrawModule>,
}

/// Immutable inputs for one view's visual dispatch phase.
///
/// This is intentionally not part of `PresentationFrame` serialization.  It
/// is a transient pre-render command input whose source facts must be frozen
/// after the C++-ordered client visibility update and before GPU collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedVisualFrame {
    pub pass: VisualDispatchPassToken,
    pub drawables: Vec<PreparedVisualDrawable>,
}

impl PreparedVisualFrame {
    fn validate(&self) -> Result<(), PreparedVisualDispatchError> {
        if !self.pass.is_valid() {
            return Err(PreparedVisualDispatchError::InvalidPassToken(self.pass));
        }

        let mut drawable_ids = HashSet::with_capacity(self.drawables.len());
        let mut bound_objects = HashSet::with_capacity(self.drawables.len());
        for (drawable_index, drawable) in self.drawables.iter().enumerate() {
            if !drawable.owner.is_valid() {
                return Err(PreparedVisualDispatchError::InvalidOwner { drawable_index });
            }
            if !drawable_ids.insert(drawable.owner.drawable_id()) {
                return Err(PreparedVisualDispatchError::DuplicateDrawableId {
                    drawable_id: drawable.owner.drawable_id(),
                });
            }
            if let PreparedVisualOwner::BoundObject {
                object_id,
                object_generation,
                ..
            } = drawable.owner
            {
                if !bound_objects.insert((object_id, object_generation)) {
                    return Err(PreparedVisualDispatchError::DuplicateBoundObject {
                        object_id,
                        object_generation,
                    });
                }
            }

            // C++ returns before it reaches `getDrawModules()` for a hidden
            // candidate and never calls `drawDrawable` for an off-view one.
            // Those records therefore need only a valid owner/visibility
            // fact; requiring unavailable module state would invent a second
            // source query after the exact frame boundary.
            if !drawable.visibility.dispatches_modules() {
                continue;
            }

            let mut previous_ordinal = None;
            for (module_index, module) in drawable.draw_modules.iter().enumerate() {
                if !module.is_valid() {
                    return Err(PreparedVisualDispatchError::InvalidModule {
                        drawable_index,
                        module_index,
                    });
                }
                if let Some(previous) = previous_ordinal {
                    if previous >= module.declaration_ordinal {
                        return Err(PreparedVisualDispatchError::NonMonotonicModuleOrder {
                            drawable_index,
                            previous,
                            current: module.declaration_ordinal,
                        });
                    }
                }
                previous_ordinal = Some(module.declaration_ordinal);
            }
        }

        Ok(())
    }
}

/// CPU command consumed by the future bridge adapter, never by WGPU directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedVisualCommand {
    /// Start a candidate's current view output from empty.  This is emitted
    /// before the C++ hidden/stealth/shroud return as a transport safeguard;
    /// it corresponds to clearing bridge output, not an invented C++ module.
    ClearDrawable { owner: PreparedVisualOwner },
    /// Invoke exactly one source Draw module in declaration order.
    DispatchModule {
        owner: PreparedVisualOwner,
        module: PreparedVisualDrawModule,
    },
}

/// Ordered commands for one accepted view update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedVisualDispatchBatch {
    pub pass: VisualDispatchPassToken,
    pub commands: Vec<PreparedVisualCommand>,
}

/// Fail-closed validation error for the dispatch seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedVisualDispatchError {
    InvalidPassToken(VisualDispatchPassToken),
    StaleVisualFrame {
        newest_visual_frame_epoch: u64,
        received_visual_frame_epoch: u64,
    },
    DuplicateViewPass(VisualDispatchPassToken),
    InvalidOwner {
        drawable_index: usize,
    },
    DuplicateDrawableId {
        drawable_id: u32,
    },
    DuplicateBoundObject {
        object_id: ObjectId,
        object_generation: u64,
    },
    InvalidModule {
        drawable_index: usize,
        module_index: usize,
    },
    NonMonotonicModuleOrder {
        drawable_index: usize,
        previous: u32,
        current: u32,
    },
}

/// Stateful, CPU-only duplicate/staleness guard for prepared view frames.
///
/// The ledger deliberately retains only the newest visual epoch and its view
/// ordinals.  The production boundary must call [`Self::reset`] when it
/// starts a new authority/world generation whose visual epoch restarts; it
/// must never reset merely because a WGPU pass begins.
#[derive(Debug, Default)]
pub struct PreparedVisualDispatcher {
    newest_visual_frame_epoch: Option<u64>,
    dispatched_view_ordinals: HashSet<u32>,
}

impl PreparedVisualDispatcher {
    /// Discard epoch history at an explicit authority/world replacement seam.
    pub fn reset(&mut self) {
        self.newest_visual_frame_epoch = None;
        self.dispatched_view_ordinals.clear();
    }

    #[must_use]
    pub const fn newest_visual_frame_epoch(&self) -> Option<u64> {
        self.newest_visual_frame_epoch
    }

    /// Validate and lower a frozen view candidate list.
    ///
    /// No callback is invoked here.  A future adapter consumes the returned
    /// batch on the CPU authority side, then freezes its output for the
    /// renderer.  Validation precedes ledger mutation, so malformed input can
    /// never reserve a pass token or partially dispatch a Drawable.
    pub fn dispatch(
        &mut self,
        frame: &PreparedVisualFrame,
    ) -> Result<PreparedVisualDispatchBatch, PreparedVisualDispatchError> {
        frame.validate()?;

        match self.newest_visual_frame_epoch {
            Some(newest) if frame.pass.visual_frame_epoch < newest => {
                return Err(PreparedVisualDispatchError::StaleVisualFrame {
                    newest_visual_frame_epoch: newest,
                    received_visual_frame_epoch: frame.pass.visual_frame_epoch,
                });
            }
            Some(newest) if frame.pass.visual_frame_epoch > newest => {
                self.newest_visual_frame_epoch = Some(frame.pass.visual_frame_epoch);
                self.dispatched_view_ordinals.clear();
            }
            None => self.newest_visual_frame_epoch = Some(frame.pass.visual_frame_epoch),
            Some(_) => {}
        }

        if !self
            .dispatched_view_ordinals
            .insert(frame.pass.view_ordinal)
        {
            return Err(PreparedVisualDispatchError::DuplicateViewPass(frame.pass));
        }

        let mut commands = Vec::new();
        for drawable in &frame.drawables {
            if !drawable.visibility.visits_drawable() {
                continue;
            }

            // The active Rust bridge must clear prior module output before
            // Drawable's C++ visibility early-return; retain that discipline
            // here so hidden/shrouded state cannot leak a prior visible draw.
            commands.push(PreparedVisualCommand::ClearDrawable {
                owner: drawable.owner,
            });
            if drawable.visibility.dispatches_modules() {
                commands.extend(drawable.draw_modules.iter().cloned().map(|module| {
                    PreparedVisualCommand::DispatchModule {
                        owner: drawable.owner,
                        module,
                    }
                }));
            }
        }

        Ok(PreparedVisualDispatchBatch {
            pass: frame.pass,
            commands,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // C++ references for this test suite:
    // - GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp:677-681
    // - GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp:1362-1368
    // - GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp:2611-2655

    fn pass(visual_frame_epoch: u64, view_ordinal: u32) -> VisualDispatchPassToken {
        VisualDispatchPassToken {
            visual_frame_epoch,
            view_ordinal,
        }
    }

    fn bound_owner() -> PreparedVisualOwner {
        PreparedVisualOwner::BoundObject {
            object_id: ObjectId(44),
            object_generation: 9,
            drawable_id: 700,
        }
    }

    fn module(declaration_ordinal: u32, source_name: &str) -> PreparedVisualDrawModule {
        PreparedVisualDrawModule {
            declaration_ordinal,
            source_name: source_name.to_string(),
        }
    }

    fn prepared(
        pass: VisualDispatchPassToken,
        owner: PreparedVisualOwner,
        visibility: PreparedVisualVisibility,
        draw_modules: Vec<PreparedVisualDrawModule>,
    ) -> PreparedVisualFrame {
        PreparedVisualFrame {
            pass,
            drawables: vec![PreparedVisualDrawable {
                owner,
                visibility,
                draw_modules,
            }],
        }
    }

    #[test]
    fn visible_candidate_clears_then_dispatches_modules_in_declaration_order() {
        let owner = bound_owner();
        let frame = prepared(
            pass(12, 0),
            owner,
            PreparedVisualVisibility::Visible,
            vec![module(0, "W3DModelDraw"), module(2, "W3DTruckDraw")],
        );

        let batch = PreparedVisualDispatcher::default()
            .dispatch(&frame)
            .expect("valid C++-ordered candidate");

        assert_eq!(
            batch.commands,
            vec![
                PreparedVisualCommand::ClearDrawable { owner },
                PreparedVisualCommand::DispatchModule {
                    owner,
                    module: module(0, "W3DModelDraw"),
                },
                PreparedVisualCommand::DispatchModule {
                    owner,
                    module: module(2, "W3DTruckDraw"),
                },
            ]
        );
    }

    #[test]
    fn hidden_stealth_and_shroud_candidates_clear_without_module_dispatch() {
        let owner = bound_owner();
        let mut dispatcher = PreparedVisualDispatcher::default();

        for (epoch, visibility) in [
            (1, PreparedVisualVisibility::Hidden),
            (2, PreparedVisualVisibility::HiddenByStealth),
            (3, PreparedVisualVisibility::FullyObscuredByShroud),
        ] {
            // C++ Drawable.cpp:2627 returns before DrawModule dispatch for
            // each of these states.  The bridge clear prevents old output.
            let batch = dispatcher
                .dispatch(&prepared(pass(epoch, 0), owner, visibility, Vec::new()))
                .expect("visibility-only candidate stays valid");
            assert_eq!(
                batch.commands,
                vec![PreparedVisualCommand::ClearDrawable { owner }]
            );
        }
    }

    #[test]
    fn standalone_client_drawable_dispatches_without_an_object_or_fow_identity() {
        let owner = PreparedVisualOwner::StandaloneClientDrawable { drawable_id: 701 };
        let batch = PreparedVisualDispatcher::default()
            .dispatch(&prepared(
                pass(4, 0),
                owner,
                PreparedVisualVisibility::Visible,
                vec![module(0, "FXListDraw")],
            ))
            .expect("standalone C++ client drawables are not gameplay objects");

        assert!(owner.is_standalone());
        assert_eq!(owner.drawable_id(), 701);
        assert_eq!(batch.commands.len(), 2);
        assert!(matches!(
            batch.commands.get(1),
            Some(PreparedVisualCommand::DispatchModule {
                owner: PreparedVisualOwner::StandaloneClientDrawable { .. },
                ..
            })
        ));
    }

    #[test]
    fn outside_view_emits_no_command_for_the_current_view_pass() {
        let batch = PreparedVisualDispatcher::default()
            .dispatch(&prepared(
                pass(5, 0),
                bound_owner(),
                PreparedVisualVisibility::OutsideView,
                Vec::new(),
            ))
            .expect("off-view is a valid W3DView region result");

        // W3DView.cpp:1362-1368 never calls drawDrawable for an off-region
        // item, so no current-epoch bridge command may be created.
        assert!(batch.commands.is_empty());
    }

    #[test]
    fn duplicate_view_token_is_rejected_without_a_second_dispatch() {
        let frame = prepared(
            pass(6, 1),
            bound_owner(),
            PreparedVisualVisibility::Visible,
            vec![module(0, "W3DModelDraw")],
        );
        let mut dispatcher = PreparedVisualDispatcher::default();
        let first = dispatcher.dispatch(&frame).expect("first view update");
        assert_eq!(first.commands.len(), 2);

        assert_eq!(
            dispatcher.dispatch(&frame),
            Err(PreparedVisualDispatchError::DuplicateViewPass(pass(6, 1)))
        );
    }

    #[test]
    fn distinct_views_can_dispatch_in_the_same_visual_frame() {
        let mut dispatcher = PreparedVisualDispatcher::default();
        for view_ordinal in [0, 1] {
            let batch = dispatcher
                .dispatch(&prepared(
                    pass(7, view_ordinal),
                    bound_owner(),
                    PreparedVisualVisibility::Visible,
                    vec![module(0, "W3DModelDraw")],
                ))
                .expect("C++ dispatches candidates once per distinct W3DView");
            assert_eq!(batch.commands.len(), 2);
        }
    }

    #[test]
    fn stale_or_ambiguous_prepared_input_fails_closed_before_dispatch() {
        let mut dispatcher = PreparedVisualDispatcher::default();
        dispatcher
            .dispatch(&prepared(
                pass(9, 0),
                bound_owner(),
                PreparedVisualVisibility::Visible,
                vec![module(0, "W3DModelDraw")],
            ))
            .expect("newer pass");

        assert_eq!(
            dispatcher.dispatch(&prepared(
                pass(8, 0),
                bound_owner(),
                PreparedVisualVisibility::Visible,
                vec![module(0, "W3DModelDraw")],
            )),
            Err(PreparedVisualDispatchError::StaleVisualFrame {
                newest_visual_frame_epoch: 9,
                received_visual_frame_epoch: 8,
            })
        );

        let malformed = prepared(
            pass(10, 0),
            bound_owner(),
            PreparedVisualVisibility::Visible,
            vec![module(2, "Later"), module(1, "Earlier")],
        );
        assert_eq!(
            dispatcher.dispatch(&malformed),
            Err(PreparedVisualDispatchError::NonMonotonicModuleOrder {
                drawable_index: 0,
                previous: 2,
                current: 1,
            })
        );
        // Validation happens before ledger mutation, so the valid epoch-10
        // pass remains available after the malformed preparation is rejected.
        assert!(
            dispatcher
                .dispatch(&prepared(
                    pass(10, 0),
                    bound_owner(),
                    PreparedVisualVisibility::Visible,
                    vec![module(0, "W3DModelDraw")],
                ))
                .is_ok()
        );
    }
}
