//! Admission of logic-origin material writes into their receiving Drawable.
//! The same operation serves match seeding and the ordinary native shell.
use crate::game_logic::GameLogic;
use crate::presentation_frame::PresentationFrame;
use game_client::core::GameClient;
use game_client::core::game_client::NativeMaterialObservation;
use game_client::drawable::drawable::StealthLook;

pub(super) fn prepare(
    logic: &mut GameLogic,
    frame: Option<&PresentationFrame>,
    client: &mut GameClient,
    host_epoch: u64,
) -> Result<Vec<NativeMaterialObservation>, String> {
    let Some(frame) = frame else {
        return Ok(Vec::new());
    };
    // C++ destroys an Object's Drawable the moment the Object is destroyed
    // (GameLogic::sendObjectDestroyed -> GameClient::destroyDrawable,
    // GameLogic.cpp:4134-4151 / GameClient.cpp:799-805 remove it from
    // m_drawableList), so a later client draw only ever walks drawables whose
    // owner is alive. A completed frame may predate this logic step (the shell
    // map steps logic between renders); an entry whose owner has since been
    // destroyed has no Drawable to receive materials and is skipped.
    let observations = frame
        .direct_host_drawables
        .iter()
        .filter(|direct| direct.resident)
        .filter_map(|direct| {
            let object = logic.host_object(direct.object.id)?;
            Some((direct, object))
        })
        .map(|(direct, object)| {
            let frozen = &direct.object;
            let (_, effectively_dead) = frozen
                .drawable_shroud
                .direct_game_client_status()
                .ok_or_else(|| {
                    format!(
                        "resident material object {} lacks direct shroud facts",
                        frozen.id.0
                    )
                })?;
            let binding_key = client
                .presentation_direct_drawable_state(host_epoch, frozen.id.0)
                .ok_or_else(|| {
                    format!(
                        "resident material object {} lacks its current Drawable binding",
                        frozen.id.0
                    )
                })?
                .binding_key;
            let look = match object.camo_stealth_look {
                1 => StealthLook::VisibleFriendly,
                2 => StealthLook::DisguisedEnemy,
                3 => StealthLook::VisibleDetected,
                4 => StealthLook::VisibleFriendlyDetected,
                5 => StealthLook::Invisible,
                _ => StealthLook::None,
            };
            Ok(NativeMaterialObservation {
                binding_key,
                effectively_dead,
                frenzy: object.drawable_tint_status & 0x10 != 0,
                look,
                is_disguised: object.status.disguised
                    || object.disguise_as_template.is_some()
                    || object.disguise_pending_template.is_some(),
                is_mine: object.is_kind_of(crate::game_logic::KindOf::Mine),
                friendly_opacity: object.camo_friendly_opacity,
                pending_opacity: object.pending_second_material_pass(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    client
        .prepare_presentation_materials(&observations)
        .map_err(|error| error.to_string())?;
    for observation in &observations {
        if let Some(expected) = observation.pending_opacity {
            if let Some(object) = logic.host_object_mut(crate::game_logic::ObjectId(
                observation.binding_key.object_id,
            )) {
                object.acknowledge_second_material_pass(expected);
            }
        }
    }
    Ok(observations)
}

#[cfg(test)]
#[path = "presentation_materials_tests.rs"]
mod tests;
