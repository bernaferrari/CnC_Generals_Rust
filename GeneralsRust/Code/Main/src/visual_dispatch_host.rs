//! Freeze a [`PreparedVisualFrame`] from already-frozen host presentation facts.
//!
//! C++ `W3DView::drawDrawable` → `Drawable::draw` runs after client visibility
//! is frozen and before GPU collection. This module only assembles that input
//! from presentation-direct binding keys and authored Draw-module names. It
//! does not read live GameLogic or invent a module class.

use crate::assets::AuthoredDrawModel;
use crate::visual_dispatch::{
    PreparedVisualDispatchBatch, PreparedVisualDispatchError, PreparedVisualDispatcher,
    PreparedVisualDrawModule, PreparedVisualDrawable, PreparedVisualFrame, PreparedVisualOwner,
    PreparedVisualVisibility, VisualDispatchPassToken,
};

/// Map C++ `Drawable::draw` early-outs onto the frozen visibility enum.
///
/// `scene_effectively_hidden` is `m_hidden || m_hiddenByStealth`. The sidecar
/// does not split those two bits; Hidden is the honest combined reason.
#[must_use]
pub const fn visibility_from_direct_sidecar(
    outside_view: bool,
    scene_effectively_hidden: bool,
    fully_obscured: bool,
) -> PreparedVisualVisibility {
    if outside_view {
        PreparedVisualVisibility::OutsideView
    } else if scene_effectively_hidden {
        PreparedVisualVisibility::Hidden
    } else if fully_obscured {
        PreparedVisualVisibility::FullyObscuredByShroud
    } else {
        PreparedVisualVisibility::Visible
    }
}

/// Build one Drawable candidate. Returns `None` when the owner is invalid or a
/// visible candidate is missing a source module name.
#[must_use]
pub fn freeze_host_visual_drawable(
    owner: PreparedVisualOwner,
    visibility: PreparedVisualVisibility,
    draw_models: &[AuthoredDrawModel],
) -> Option<PreparedVisualDrawable> {
    if !owner.is_valid() {
        return None;
    }
    let draw_modules = if visibility.dispatches_modules() {
        let mut modules = Vec::with_capacity(draw_models.len());
        for model in draw_models {
            let module = PreparedVisualDrawModule {
                declaration_ordinal: model.module_index,
                source_name: model.source_name.clone(),
            };
            if !module.is_valid() {
                return None;
            }
            if modules
                .last()
                .is_some_and(|previous: &PreparedVisualDrawModule| {
                    previous.declaration_ordinal >= module.declaration_ordinal
                })
            {
                return None;
            }
            modules.push(module);
        }
        modules
    } else {
        Vec::new()
    };
    Some(PreparedVisualDrawable {
        owner,
        visibility,
        draw_modules,
    })
}

/// Validate and lower a frozen host candidate list.
pub fn dispatch_frozen_host_visuals(
    dispatcher: &mut PreparedVisualDispatcher,
    pass: VisualDispatchPassToken,
    drawables: Vec<PreparedVisualDrawable>,
) -> Result<PreparedVisualDispatchBatch, PreparedVisualDispatchError> {
    dispatcher.dispatch(&PreparedVisualFrame { pass, drawables })
}

/// Declaration ordinals C++ would invoke for `owner`, if that Drawable was
/// visited. `None` means the owner was not in this view pass.
#[must_use]
pub fn dispatched_module_ordinals(
    batch: &PreparedVisualDispatchBatch,
    owner: PreparedVisualOwner,
) -> Option<Vec<u32>> {
    let mut visited = false;
    let mut ordinals = Vec::new();
    for command in &batch.commands {
        match command {
            crate::visual_dispatch::PreparedVisualCommand::ClearDrawable {
                owner: command_owner,
            } if *command_owner == owner => {
                visited = true;
            }
            crate::visual_dispatch::PreparedVisualCommand::DispatchModule {
                owner: command_owner,
                module,
            } if *command_owner == owner => {
                visited = true;
                ordinals.push(module.declaration_ordinal);
            }
            _ => {}
        }
    }
    visited.then_some(ordinals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;

    fn owner() -> PreparedVisualOwner {
        PreparedVisualOwner::BoundObject {
            object_id: ObjectId(11),
            object_generation: 3,
            drawable_id: 900,
        }
    }

    fn model(module_index: u32, source_name: &str) -> AuthoredDrawModel {
        AuthoredDrawModel {
            module_index,
            source_name: source_name.to_string(),
            model_key: "UVLiteTank".to_string(),
            ..AuthoredDrawModel::default()
        }
    }

    #[test]
    fn visible_host_candidate_dispatches_named_modules_in_declaration_order() {
        let drawable = freeze_host_visual_drawable(
            owner(),
            PreparedVisualVisibility::Visible,
            &[model(0, "W3DModelDraw"), model(2, "W3DTruckDraw")],
        )
        .expect("named modules stay valid");
        let batch = dispatch_frozen_host_visuals(
            &mut PreparedVisualDispatcher::default(),
            VisualDispatchPassToken {
                visual_frame_epoch: 4,
                view_ordinal: 0,
            },
            vec![drawable],
        )
        .expect("valid host frame");
        assert_eq!(
            dispatched_module_ordinals(&batch, owner()),
            Some(vec![0, 2])
        );
    }

    #[test]
    fn hidden_and_shrouded_host_candidates_clear_without_modules() {
        for (view_ordinal, visibility) in [
            PreparedVisualVisibility::Hidden,
            PreparedVisualVisibility::FullyObscuredByShroud,
        ]
        .into_iter()
        .enumerate()
        {
            let drawable =
                freeze_host_visual_drawable(owner(), visibility, &[model(0, "W3DModelDraw")])
                    .expect("hidden candidates do not require module names");
            let batch = dispatch_frozen_host_visuals(
                &mut PreparedVisualDispatcher::default(),
                VisualDispatchPassToken {
                    visual_frame_epoch: 5,
                    view_ordinal: view_ordinal as u32,
                },
                vec![drawable],
            )
            .expect("visibility-only candidate");
            assert_eq!(dispatched_module_ordinals(&batch, owner()), Some(vec![]));
        }
    }

    #[test]
    fn unnamed_or_nonmonotonic_visible_modules_fail_closed() {
        assert!(
            freeze_host_visual_drawable(
                owner(),
                PreparedVisualVisibility::Visible,
                &[model(0, "")],
            )
            .is_none()
        );
        assert!(
            freeze_host_visual_drawable(
                owner(),
                PreparedVisualVisibility::Visible,
                &[model(2, "Later"), model(1, "Earlier")],
            )
            .is_none()
        );
        assert!(
            freeze_host_visual_drawable(
                PreparedVisualOwner::BoundObject {
                    object_id: ObjectId(11),
                    object_generation: 0,
                    drawable_id: 900,
                },
                PreparedVisualVisibility::Visible,
                &[model(0, "W3DModelDraw")],
            )
            .is_none()
        );
    }

    #[test]
    fn collect_runs_host_visual_dispatch_before_mesh_submission() {
        let collect = include_str!("graphics/render_pipeline/pipeline_collect.rs");
        assert!(collect.contains("self.dispatch_host_visual_modules(&unit_inputs)"));
        assert!(collect.contains("visual_visited.contains(&object_id)"));
        assert!(collect.contains("visual_allowed_modules"));
    }

    #[test]
    fn draw_declaration_source_name_is_the_first_token() {
        assert_eq!(
            crate::assets::authored_draw_module_source_name("W3DTruckDraw ModuleTag_01"),
            "W3DTruckDraw"
        );
        assert_eq!(crate::assets::authored_draw_module_source_name("   "), "");
    }

    #[test]
    fn sidecar_hidden_is_not_friendly_stealth_visible() {
        assert_eq!(
            visibility_from_direct_sidecar(false, true, false),
            PreparedVisualVisibility::Hidden
        );
        assert_eq!(
            visibility_from_direct_sidecar(false, false, true),
            PreparedVisualVisibility::FullyObscuredByShroud
        );
        assert_eq!(
            visibility_from_direct_sidecar(true, false, false),
            PreparedVisualVisibility::OutsideView
        );
    }
}
