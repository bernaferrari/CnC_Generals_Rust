//! Completed Drawable second-material values, frozen once per render handoff.
//! The renderer never advances this residual or infers it from stealth flags.

use super::*;
use crate::presentation_frame::{PresentationFrame, UnitRenderInput};
use game_client::core::game_client::PresentationDirectDrawableBindingKey;
use std::rc::Weak;

struct CompletedMaterialDraw {
    opacity: f32,
    scene_effectively_hidden: bool,
    fully_obscured: bool,
}

pub(super) struct FrozenMaterialDrawFrame {
    frame: Weak<PresentationFrame>,
    values: HashMap<PresentationDirectDrawableBindingKey, CompletedMaterialDraw>,
}

impl FrozenMaterialDrawFrame {
    pub(super) fn capture(
        frame: &Rc<PresentationFrame>,
        client: &game_client::core::GameClient,
        host_epoch: u64,
    ) -> Self {
        let roster: HashSet<_> = frame
            .direct_host_drawables
            .iter()
            .filter(|direct| direct.resident)
            .filter(|direct| {
                direct
                    .object
                    .drawable_shroud
                    .direct_game_client_status()
                    .is_some()
            })
            .map(|direct| direct.object.id.0)
            .collect();
        let values = client
            .capture_presentation_material_opacities()
            .into_iter()
            .filter_map(|(key, opacity)| {
                if key.host_epoch != host_epoch || !roster.contains(&key.object_id) {
                    return None;
                }
                let current =
                    client.presentation_direct_drawable_state(host_epoch, key.object_id)?;
                (current.binding_key == key).then_some((
                    key,
                    CompletedMaterialDraw {
                        opacity,
                        scene_effectively_hidden: current.scene_effectively_hidden,
                        fully_obscured: current.fully_obscured,
                    },
                ))
            })
            .collect();
        Self {
            frame: Rc::downgrade(frame),
            values,
        }
    }

    pub(super) fn apply_to_inputs(
        &self,
        frame: &Rc<PresentationFrame>,
        shroud: &HashMap<ObjectID, FrozenDirectDrawableShroudState>,
        inputs: &mut [UnitRenderInput],
    ) {
        let matching_frame = self.frame.ptr_eq(&Rc::downgrade(frame));
        for input in inputs {
            // Missing/replaced bindings never inherit another owner's value.
            input.second_material_pass_opacity = 0.0;
            if !matching_frame || !input.drawable_shroud.lifetime.is_direct_host_object() {
                continue;
            }
            let Some(state) = shroud.get(&input.id) else {
                continue;
            };
            let key = PresentationDirectDrawableBindingKey {
                host_epoch: state.host_epoch,
                object_id: state.object_id.0,
                drawable_id: game_client::core::DrawableId(state.drawable_id),
                binding_generation: state.binding_generation,
            };
            if let Some(completed) = self.values.get(&key) {
                if completed.scene_effectively_hidden == state.scene_effectively_hidden
                    && completed.fully_obscured == state.fully_obscured
                {
                    input.second_material_pass_opacity = completed.opacity;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "material_draw_inputs_tests.rs"]
mod tests;
