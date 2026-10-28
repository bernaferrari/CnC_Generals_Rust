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

/// Receiving equivalent of `host_world`; RandomBone consumes the owning
/// GameLogic stream instead of selecting the process-wide active RNG.
fn host_world_receiving(
    object: &crate::game_logic::Object,
    loc: Option<&HostTransitionLoc>,
    random: &game_engine::common::random_value::SessionRandom,
) -> glam::Vec3 {
    let local = loc
        .map(|loc| host_local_receiving(object, &loc.as_particle(), random))
        .unwrap_or_default();
    object.get_transform_matrix().transform_point3(local)
}

fn host_local_receiving(
    object: &crate::game_logic::Object,
    definition: &HostTransitionParticle,
    random: &game_engine::common::random_value::SessionRandom,
) -> glam::Vec3 {
    let model = object
        .get_template()
        .model_name_for_conditions(object.model_condition_bits);
    let scale = object.get_template().asset_scale;
    let position =
        gamelogic::common::Coord3D::new(definition.loc[0], definition.loc[1], definition.loc[2]);
    let local = match definition.bone.as_deref() {
        None => position,
        Some(bone) if !definition.random_bone => {
            gamelogic::object::draw::lookup_pristine_bone_translation(&model, scale, bone)
                .unwrap_or(position)
        }
        Some(bone) => {
            let mut positions = Vec::new();
            for index in 1..=32 {
                let name = format!("{bone}{index:02}");
                match gamelogic::object::draw::lookup_pristine_bone_translation(
                    &model, scale, &name,
                ) {
                    Some(position) => positions.push(position),
                    None => break,
                }
            }
            if positions.is_empty() {
                position
            } else {
                let upper = i32::try_from(positions.len() - 1).unwrap_or(i32::MAX);
                let index = usize::try_from(random.next_int(0, upper)).unwrap_or_default();
                positions.get(index).copied().unwrap_or(position)
            }
        }
    };
    leftover_to_host_local(local)
}

#[cfg(feature = "game_client")]
impl crate::game_logic::GameLogic {
    /// Resume one yielded native TransitionDamageFX callback against its
    /// receiving GameLogic. View loans are deliberately scoped around each FX
    /// or particle slot so the synchronous OCL can borrow the whole owner.
    pub(crate) fn execute_receiving_transition_damage_fx(
        &mut self,
        victim_id: crate::game_logic::ObjectId,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        mut event: HostTransitionDamageFxEvent,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        frame: u32,
        player: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let environment = self.observe_object_fx_environment();
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let terrain = std::sync::Arc::clone(self.world_services.terrain());
        let audio_policy = self.queued_audio_events.policy().clone();

        let still_same_victim = |world: &Self| {
            world
                .objects
                .get(&victim_id)
                .is_some_and(|object| world.live_draw_playback_identity(object) == *identity)
        };

        // C++ retires every old-state particle before starting slot zero.
        if let Some(old) = event.clear_old_state.take() {
            if !still_same_victim(self) {
                return;
            }
            Self::with_object_fx_services(
                &mut self.session_view,
                audio_policy.clone(),
                player,
                &shroud,
                effects,
                &environment,
                &terrain,
                &self.logic_random,
                |view| {
                    let (objects, particles) = (&mut self.objects, &mut self.combat_particles);
                    let Some(object) = objects.get_mut(&victim_id) else {
                        return;
                    };
                    let Some(ids) = object
                        .transition_damage_fx
                        .as_mut()
                        .and_then(|config| config.attached_ids.get_mut(old as usize))
                    else {
                        return;
                    };
                    retire_old_transition_ids(ids, |id| particles.try_deactivate(id, view));
                },
            );
        }

        for_each_transition_slot(&event, |effect| {
            if !still_same_victim(self) {
                return;
            }
            match effect {
                TransitionSlotEffect::Fx { name, loc, .. } => {
                    let Some(position) = self
                        .objects
                        .get(&victim_id)
                        .map(|object| host_world_receiving(object, loc, &self.logic_random))
                    else {
                        return;
                    };
                    Self::with_object_fx_services(
                        &mut self.session_view,
                        audio_policy.clone(),
                        player,
                        &shroud,
                        effects,
                        &environment,
                        &terrain,
                        &self.logic_random,
                        |view| {
                            Self::dispatch_fx_list_at_position_with_view(
                                name, position, None, 0.0, 0.0, None, view,
                            );
                        },
                    );
                }
                TransitionSlotEffect::Ocl { name, loc, .. } => {
                    let Some((position, team, veterancy, orientation, velocity)) =
                        self.objects.get(&victim_id).map(|object| {
                            (
                                host_world_receiving(object, loc, &self.logic_random),
                                object.team,
                                object.experience.level,
                                object.get_orientation(),
                                object.movement.velocity,
                            )
                        })
                    else {
                        return;
                    };
                    if !still_same_victim(self) {
                        return;
                    }
                    // The receiving dispatcher executes synchronously at the
                    // C++ OCL slot. It currently runs only the receiving
                    // GenericObjectCreationNugget subset; other authored
                    // nuggets and C++'s secondary damage-source position remain
                    // explicit parity gaps in hq-lxksk. Never substitute the
                    // process-global OCL store for those unsupported cases.
                    let _ = self.execute_parsed_weapon_ocl_at_with_effects(
                        name,
                        Some(victim_id),
                        team,
                        veterancy,
                        orientation,
                        velocity,
                        position,
                        effects,
                    );
                }
                TransitionSlotEffect::Particle { slot, definition } => {
                    let Some((position, orientation, local, is_shrouded)) =
                        self.objects.get(&victim_id).map(|object| {
                            (
                                object.get_position(),
                                object.get_orientation(),
                                host_local_receiving(object, definition, &self.logic_random),
                                Self::host_fx_visibility_for(player, &shroud, victim_id.0),
                            )
                        })
                    else {
                        return;
                    };
                    let mut created = None;
                    Self::with_object_fx_services(
                        &mut self.session_view,
                        audio_policy.clone(),
                        player,
                        &shroud,
                        effects,
                        &environment,
                        &terrain,
                        &self.logic_random,
                        |view| {
                            let (objects, particles) =
                                (&mut self.objects, &mut self.combat_particles);
                            let Some(object) = objects.get_mut(&victim_id) else {
                                return;
                            };
                            let Some(ids) =
                                object.transition_damage_fx.as_mut().and_then(|config| {
                                    config.attached_ids.get_mut(event.new_state as usize)
                                })
                            else {
                                return;
                            };
                            if !prepare_transition_slot(ids, slot, |id| {
                                particles.try_deactivate(id, view)
                            }) {
                                log::warn!(
                                    "retained transition particle in slot {} for object {}",
                                    slot,
                                    victim_id.0
                                );
                                return;
                            }
                            created = particles.attach_named_to_object_local_with_visibility(
                                victim_id,
                                position,
                                orientation,
                                local,
                                frame,
                                &definition.name,
                                crate::game_logic::combat_particles::CombatParticleKind::DeathSmoke,
                                None,
                                is_shrouded,
                                view,
                            );
                            if let Some(id) = created {
                                ids.resize(ids.len().max(slot + 1), 0);
                                ids[slot] = id;
                            }
                        },
                    );
                }
            }
        });

        if still_same_victim(self) {
            if let Some(object) = self.objects.get_mut(&victim_id) {
                if event.audio_name.is_some() {
                    event.fx_name = None;
                    event.extra_fx_names.clear();
                    event.ocl_names.clear();
                    event.particles.clear();
                    event.clear_old_state = None;
                    object.pending_transition_damage_fx.push(event);
                }
            }
        }
        let _ = source; // Secondary OCL position is tracked as unsupported above.
    }

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

#[cfg(not(feature = "game_client"))]
impl crate::game_logic::GameLogic {
    /// Headless host continuation. The original host has no receiving native
    /// particle/view owner, but authored OCL creation still runs synchronously
    /// against this GameLogic rather than the process-global Core store.
    pub(crate) fn execute_receiving_transition_damage_fx(
        &mut self,
        victim_id: crate::game_logic::ObjectId,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        mut event: HostTransitionDamageFxEvent,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        _frame: u32,
        _player: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let same_victim = |world: &Self| {
            world
                .objects
                .get(&victim_id)
                .is_some_and(|object| world.live_draw_playback_identity(object) == *identity)
        };
        for_each_transition_slot(&event, |effect| {
            if !same_victim(self) {
                return;
            }
            match effect {
                TransitionSlotEffect::Fx { name, loc, .. } => {
                    let Some(position) = self
                        .objects
                        .get(&victim_id)
                        .map(|object| host_world_receiving(object, loc, &self.logic_random))
                    else {
                        return;
                    };
                    let _ = crate::game_logic::dispatch_fx_list_at_pos(name, position);
                }
                TransitionSlotEffect::Ocl { name, loc, .. } => {
                    let Some((position, team, veterancy, orientation, velocity)) =
                        self.objects.get(&victim_id).map(|object| {
                            (
                                host_world_receiving(object, loc, &self.logic_random),
                                object.team,
                                object.experience.level,
                                object.get_orientation(),
                                object.movement.velocity,
                            )
                        })
                    else {
                        return;
                    };
                    let _ = self.execute_parsed_weapon_ocl_at_with_effects(
                        name,
                        Some(victim_id),
                        team,
                        veterancy,
                        orientation,
                        velocity,
                        position,
                        effects,
                    );
                }
                TransitionSlotEffect::Particle { .. } => {
                    // No native particle runtime is available in this build.
                }
            }
        });
        if same_victim(self) && event.audio_name.is_some() {
            event.fx_name = None;
            event.extra_fx_names.clear();
            event.ocl_names.clear();
            event.particles.clear();
            event.clear_old_state = None;
            if let Some(object) = self.objects.get_mut(&victim_id) {
                object.pending_transition_damage_fx.push(event);
            }
        }
        let _ = source; // Generic OCL currently lacks C++ secondary-position input.
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
