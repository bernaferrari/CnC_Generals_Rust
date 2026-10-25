//! CPP TransitionDamageFX.cpp:299–394: clear old systems, then FX/OCL/PSys per slot.
use super::*;

pub(super) enum TransitionSlotEffect<'a> {
    Fx {
        slot: usize,
        name: &'a str,
        loc: Option<&'a HostTransitionLoc>,
    },
    Ocl {
        slot: usize,
        name: &'a str,
        loc: Option<&'a HostTransitionLoc>,
    },
    Particle {
        slot: usize,
        definition: &'a HostTransitionParticle,
    },
}

/// Keep failed receipts in their original slot instead of orphaning a live resource.
fn retire_old_transition_ids(ids: &mut [u32], mut retire: impl FnMut(u32) -> bool) {
    for id in ids.iter_mut().filter(|id| **id != 0) {
        if retire(*id) {
            *id = 0;
        }
    }
}

/// Re-entry must never overwrite a receipt whose owner could not retire it.
fn prepare_transition_slot(
    ids: &mut Vec<u32>,
    slot: usize,
    mut retire: impl FnMut(u32) -> bool,
) -> bool {
    ids.resize(ids.len().max(slot + 1), 0);
    if ids[slot] != 0 {
        if !retire(ids[slot]) {
            return false;
        }
        ids[slot] = 0;
    }
    true
}

fn named(name: &str) -> bool {
    !name.trim().is_empty() && !name.eq_ignore_ascii_case("none")
}

/// Execute immediately; this iterator neither queues work nor flattens slot holes.
pub(super) fn for_each_transition_slot(
    event: &HostTransitionDamageFxEvent,
    mut execute: impl FnMut(TransitionSlotEffect<'_>),
) {
    let fx_len = if event.extra_fx_names.is_empty() {
        usize::from(event.fx_name.is_some())
    } else {
        event.extra_fx_names.len() + 1
    };
    let len = fx_len
        .max(event.ocl_names.len())
        .max(event.particles.len())
        .min(MAX_TRANSITION_EFFECT_SLOTS);
    for slot in 0..len {
        let fx = if slot == 0 {
            event.fx_name.as_deref()
        } else {
            event.extra_fx_names.get(slot - 1).map(String::as_str)
        };
        if let Some(name) = fx.filter(|name| named(name)) {
            execute(TransitionSlotEffect::Fx {
                slot,
                name,
                loc: event.fx_locs.get(slot),
            });
        }
        if let Some(name) = event.ocl_names.get(slot).filter(|name| named(name)) {
            execute(TransitionSlotEffect::Ocl {
                slot,
                name,
                loc: event.ocl_locs.get(slot),
            });
        }
        if let Some(definition) = event.particles.get(slot).filter(|p| named(&p.name)) {
            execute(TransitionSlotEffect::Particle { slot, definition });
        }
    }
}

/// Read the actual host pose at each effect, rather than selecting a same-ID Core object.
#[cfg(feature = "game_client")]
fn host_local(
    object: &crate::game_logic::Object,
    definition: &HostTransitionParticle,
) -> glam::Vec3 {
    let model = object
        .get_template()
        .model_name_for_conditions(object.model_condition_bits);
    leftover_to_host_local(leftover_local_effect_pos_live(
        definition,
        None,
        &model,
        object.get_template().asset_scale,
    ))
}

#[cfg(feature = "game_client")]
fn host_world(object: &crate::game_logic::Object, loc: Option<&HostTransitionLoc>) -> glam::Vec3 {
    let local = loc
        .map(|loc| host_local(object, &loc.as_particle()))
        .unwrap_or_default();
    // Thing::convertBonePosToWorldPos applies the canonical transform,
    // including pitch/roll/scale; rebuilding yaw also reverses Main's Y rotation.
    object.get_transform_matrix().transform_point3(local)
}

#[cfg(feature = "game_client")]
impl crate::game_logic::GameLogic {
    pub(crate) fn execute_transition_damage_fx_with_view(
        object: &mut crate::game_logic::Object,
        mut event: HostTransitionDamageFxEvent,
        source: Option<&HostDamageFxVictim>,
        particles: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
        frame: u32,
        player: Option<u32>,
        shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
        view: &mut crate::game_logic::host_fx_list_dispatch::ObjectFxServices<'_, '_>,
    ) {
        // C++ destroys all old-state systems BEFORE testing/playing the worse state.
        if let Some(old) = event.clear_old_state.take() {
            if let Some(ids) = object
                .transition_damage_fx
                .as_mut()
                .and_then(|config| config.attached_ids.get_mut(old as usize))
            {
                retire_old_transition_ids(ids, |id| {
                    let retired = particles.try_deactivate(id, view);
                    if !retired {
                        log::warn!(
                            "could not retire transition particle {} for object {}",
                            id,
                            object.id.0
                        );
                    }
                    retired
                });
            }
        }
        for_each_transition_slot(&event, |effect| match effect {
            TransitionSlotEffect::Fx { name, loc, .. } => {
                // TransitionDamageFX uses doFXPos with NO bone transform matrix.
                let position = host_world(object, loc);
                Self::dispatch_fx_list_at_position_with_view(
                    name, position, None, 0.0, 0.0, None, view,
                );
            }
            TransitionSlotEffect::Ocl { name, loc, .. } => {
                let position = host_world(object, loc);
                play_authored_transition_ocl_with_source(name, object.id.0, position, source);
            }
            TransitionSlotEffect::Particle { slot, definition } => {
                let Some(ids) = object
                    .transition_damage_fx
                    .as_mut()
                    .and_then(|config| config.attached_ids.get_mut(event.new_state as usize))
                else {
                    return;
                };
                if !prepare_transition_slot(ids, slot, |id| particles.try_deactivate(id, view)) {
                    log::warn!(
                        "retained transition particle in slot {} for object {}",
                        slot,
                        object.id.0
                    );
                    return;
                }
                let local = host_local(object, definition);
                let is_shrouded = Self::host_fx_visibility_for(player, shroud, object.id.0);
                if let Some(id) = particles.attach_named_to_object_local_with_visibility(
                    object.id,
                    object.get_position(),
                    object.get_orientation(),
                    local,
                    frame,
                    &definition.name,
                    crate::game_logic::combat_particles::CombatParticleKind::DeathSmoke,
                    None,
                    is_shrouded,
                    view,
                ) {
                    if let Some(ids) = object
                        .transition_damage_fx
                        .as_mut()
                        .and_then(|config| config.attached_ids.get_mut(event.new_state as usize))
                    {
                        ids.resize(ids.len().max(slot + 1), 0);
                        ids[slot] = id;
                    }
                }
            }
        });
        // Only audio delivery remains queued; a later AI tick cannot replay FX,
        // create particles twice, or retire the newly installed systems.
        if event.audio_name.is_some() {
            event.fx_name = None;
            event.extra_fx_names.clear();
            event.ocl_names.clear();
            event.particles.clear();
            object.pending_transition_damage_fx.push(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_slots_keep_fx_ocl_particle_interleaving_and_locations() {
        let event = HostTransitionDamageFxEvent {
            fx_name: None,
            extra_fx_names: vec!["".into(), "FX_Third".into()],
            ocl_names: vec!["OCL_First".into(), "None".into(), "OCL_Third".into()],
            particles: vec![
                HostTransitionParticle {
                    name: "PS_First".into(),
                    ..Default::default()
                },
                HostTransitionParticle::default(),
                HostTransitionParticle {
                    name: "PS_Third".into(),
                    ..Default::default()
                },
            ],
            fx_locs: vec![
                HostTransitionLoc::default(),
                HostTransitionLoc::default(),
                HostTransitionLoc {
                    loc: [3., 4., 5.],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut trace = Vec::new();
        for_each_transition_slot(&event, |effect| match effect {
            TransitionSlotEffect::Fx { slot, name, loc } => {
                assert_eq!(loc.unwrap().loc, [3., 4., 5.]);
                trace.push((slot, "fx", name.to_string()));
            }
            TransitionSlotEffect::Ocl { slot, name, .. } => trace.push((slot, "ocl", name.into())),
            TransitionSlotEffect::Particle { slot, definition } => {
                trace.push((slot, "particle", definition.name.clone()))
            }
        });
        assert_eq!(
            trace,
            vec![
                (0, "ocl", "OCL_First".into()),
                (0, "particle", "PS_First".into()),
                (2, "fx", "FX_Third".into()),
                (2, "ocl", "OCL_Third".into()),
                (2, "particle", "PS_Third".into())
            ]
        );
    }
}

#[cfg(test)]
#[path = "transition_slot_tests.rs"]
mod owner_tests;
