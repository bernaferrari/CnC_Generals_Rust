//! Native flammable timers, ignition callbacks, and partition-based fire spread.
use crate::game_logic::{GameLogic, ObjectId};

impl GameLogic {
    /// Run flammable Body callbacks through this receiving GameLogic. Native
    /// callbacks remain synchronous at the Object observation point; headless
    /// callbacks keep the existing owner-bound Body particle operation.
    fn with_receiving_flammable_body_fx<R>(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        operation: impl FnOnce(
            &mut crate::game_logic::Object,
            &mut crate::game_logic::object::BodyStateFxReceiver<'_>,
        ) -> R,
    ) -> Option<R> {
        let player = self.local_player_id();
        let frame = self.frame as u32;
        let bone_source =
            player.and_then(|player| self.current_native_slave_source(id, player).ok());

        #[cfg(feature = "game_client")]
        {
            let victim = self.objects.get(&id)?;
            let source = victim.last_damage_source;
            let secondary = source
                .and_then(|source| self.objects.get(&source))
                .map(|object| {
                    Self::observe_host_fx_object(object, player, self.world_services.shroud())
                });
            let native_body = (
                self.live_draw_playback_identity(victim),
                self.is_loading_map(),
                self.native_lighting_time_of_day(),
                self.receiving_audio_locality_for_player(
                    victim
                        .owner_player_id
                        .map(|player| player as i32)
                        .unwrap_or(-1),
                ),
            );
            let environment = self.observe_object_fx_environment_for(id);
            let audio_policy = self.queued_audio_events.policy().clone();
            let shroud = self.world_services.shroud();
            let terrain = self.world_services.terrain();
            let logic_random = &self.logic_random;
            Self::with_object_fx_services(
                &mut self.session_view,
                audio_policy,
                player,
                shroud,
                effects,
                &environment,
                terrain,
                logic_random,
                |view| {
                    let object = self.objects.get_mut(&id)?;
                    let mut body_fx = |object: &mut crate::game_logic::Object,
                                       effect: crate::game_logic::object::BodyStateFx,
                                       source: Option<
                        &crate::game_logic::host_transition_damage_fx::HostDamageFxVictim,
                    >| {
                        if let crate::game_logic::object::BodyStateFx::NativeVisualState(state) =
                            &effect
                        {
                            if let Some(native_view) = view.effects.view.as_mut() {
                                let mut change = crate::game_logic::NativeBodyVisualChange {
                                    audio_policy: view.audio_policy.clone(),
                                    identity: native_body.0.clone(),
                                    state: gamelogic::common::types::BodyDamageType::Pristine,
                                    loading_map: native_body.1,
                                    time_of_day: native_body.2,
                                    locality: native_body.3,
                                };
                                let audio = view
                                    .effects
                                    .audio
                                    .as_mut()
                                    .and_then(|audio| audio.native_ambient());
                                change.deliver(*state, &mut **native_view, audio);
                            }
                            view.update_body_auto_particles(
                                &mut self.combat_particles,
                                object,
                                *state,
                                frame,
                            );
                        } else {
                            Self::execute_body_state_fx_with_view(
                                object,
                                effect,
                                source,
                                secondary.as_ref(),
                                player,
                                shroud,
                                &mut self.combat_particles,
                                &mut self.queued_audio_events,
                                frame,
                                &self.damage_fx_definitions,
                                view,
                            );
                        }
                    };
                    Some(operation(object, &mut body_fx))
                },
            )
        }

        #[cfg(not(feature = "game_client"))]
        {
            let mut body_particles =
                crate::game_logic::body_particle_runtime::HeadlessBodyParticleUpdate {
                    particles: &mut self.combat_particles,
                    random: &mut self.session_view.client_random,
                    player,
                    shroud: self.world_services.shroud(),
                    frame,
                    source: bone_source,
                    pristine_bones: effects.pristine_bones,
                };
            let audio = &mut self.queued_audio_events;
            let random = &self.logic_random;
            let object = self.objects.get_mut(&id)?;
            let mut body_fx = |object: &mut crate::game_logic::Object,
                               effect: crate::game_logic::object::BodyStateFx,
                               source: Option<
                &crate::game_logic::host_transition_damage_fx::HostDamageFxVictim,
            >| {
                Self::execute_body_state_fx_without_view_with_particles(
                    object,
                    effect,
                    source,
                    audio,
                    random,
                    &self.damage_fx_definitions,
                    &mut body_particles,
                )
            };
            Some(operation(object, &mut body_fx))
        }
    }

    /// C++ FlammableUpdate::setAflame visual callback with this world owner.
    fn apply_flammable_visuals_with_receiving_particles(
        &mut self,
        id: ObjectId,
        aflame: bool,
        smoldering: bool,
        burned: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let frame = self.frame as u32;
        self.with_receiving_flammable_body_fx(id, effects, |object, body_fx| {
            object.apply_flammable_visuals_with_body_fx(
                aflame, smoldering, burned, !aflame, None, frame, body_fx,
            );
        })
        .is_some()
    }

    /// C++ FireSpreadUpdate selects and immediately ignites its closest target.
    pub(in crate::game_logic) fn ignite_object_fire_spread(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let frame = self.frame as u32;
        let ignited = self
            .with_receiving_flammable_body_fx(id, effects, |object, body_fx| {
                object.try_ignite_fire_spread_with_body_fx(frame, body_fx)
            })
            .unwrap_or(false);
        if !ignited {
            return false;
        }
        self.fire_spread_reg.record_ignition();
        true
    }

    #[cfg(test)]
    pub(in super::super) fn update_fire_spread(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_fire_spread_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_fire_spread_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let frame = self.frame as u32;
        // C++ GameLogic::update runs due Flammable/FireSpread updates without
        // an isEffectivelyDead check; only physically destroyed objects stop.
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| !o.status.destroyed && o.has_fire_spread())
            .map(|(id, _)| *id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }

        let mut ignited_this_update = std::collections::HashSet::new();
        for id in ids {
            if ignited_this_update.contains(&id) {
                continue;
            }
            let (position, begin, burning_sound, incarnation) = {
                let Some(object) = self.objects.get_mut(&id) else {
                    continue;
                };
                let position = object.get_position();
                let Some(fire) = object.fire_spread.as_mut() else {
                    continue;
                };
                (
                    position,
                    fire.begin_tick_flammable(frame),
                    fire.burning_sound_name.clone(),
                    std::sync::Arc::clone(&object.visual_draw_identity_token),
                )
            };
            // C++ FlammableUpdate::update calls doAflameDamage before the
            // burned and aflame-end timers; allow its Body callbacks to finish
            // before advancing this object's timer tail.
            if begin.aflame_damage > 0.0 {
                let _ = self.apply_residual_damage_owned_with_effects(
                    id,
                    begin.aflame_damage,
                    Some(id),
                    crate::game_logic::combat::DamageType::Flame,
                    crate::game_logic::host_usa_pilot::HostDeathType::Burned,
                    None,
                    presentation_effects,
                )?;
            }
            // DOT runs the full synchronous Body callback chain. It may remove
            // this object or replace the slot with a new incarnation using the
            // same ObjectId; never apply this tick's saved timer continuation
            // to that replacement.
            if !self.objects.get(&id).is_some_and(|object| {
                std::sync::Arc::ptr_eq(&object.visual_draw_identity_token, &incarnation)
            }) {
                continue;
            }
            if begin.start_burning_sound {
                self.start_fire_spread_burning_sound(id, position, &burning_sound);
            }
            let Some((tail, spread_due, position, smoldering, burning_sound, ember_ocl, range)) =
                ({
                    let Some(object) = self.objects.get_mut(&id) else {
                        continue;
                    };
                    let position = object.get_position();
                    let Some(fire) = object.fire_spread.as_mut() else {
                        continue;
                    };
                    Some((
                        fire.finish_tick_flammable(frame),
                        fire.spread_due(frame),
                        position,
                        fire.smoldering,
                        fire.burning_sound_name.clone(),
                        fire.ocl_embers.clone(),
                        fire.spread_try_range,
                    ))
                })
            else {
                continue;
            };

            if tail.became_smoldering {
                let _ = self.apply_flammable_visuals_with_receiving_particles(
                    id,
                    true,
                    true,
                    false,
                    presentation_effects,
                );
            }
            // FlammableUpdate::update handles the burned timer first, then
            // stops audio before clearing AFLAME and invoking Body::setAflame.
            if tail.stop_burning_sound {
                self.stop_fire_spread_burning_sound(id, position, &burning_sound);
            }
            if tail.became_burned {
                self.fire_spread_reg.record_burned();
                let _ = self.apply_flammable_visuals_with_receiving_particles(
                    id,
                    false,
                    true,
                    true,
                    presentation_effects,
                );
            } else if tail.returned_to_normal {
                let _ = self.apply_flammable_visuals_with_receiving_particles(
                    id,
                    false,
                    false,
                    false,
                    presentation_effects,
                );
            } else if tail.aflame && !tail.became_smoldering {
                let _ = self.apply_flammable_visuals_with_receiving_particles(
                    id,
                    true,
                    smoldering || tail.smoldering,
                    false,
                    presentation_effects,
                );
            }
            if spread_due && !ember_ocl.is_empty() {
                self.fire_spread_reg.record_embers();
                let _ = self.spawn_fire_spread_embers_with_effects(
                    id,
                    position,
                    &ember_ocl,
                    presentation_effects,
                );
            }
            if spread_due {
                self.fire_spread_reg.record_spread();
                // C++ FireSpreadUpdate.cpp:118 getClosestObject +
                // PartitionFilterFlammable::allow (FireSpreadUpdate.cpp:36-48):
                // admit only objects whose FlammableUpdate wouldIgnite();
                // destroyed objects left the partition. The cell walk keeps the
                // original strict 3-D range and first COI on ties — no
                // whole-world scan.
                let nearest = if range != 0.0 {
                    self.partition_manager.closest_in_radius(
                        position.x,
                        position.z,
                        range,
                        |raw| {
                            let candidate = ObjectId(raw);
                            if candidate == id {
                                return None;
                            }
                            let object = self.objects.lookup(&candidate)?;
                            (!object.status.destroyed
                                && object
                                    .fire_spread
                                    .as_ref()
                                    .is_some_and(|fire| fire.would_ignite()))
                            .then(|| position.distance_squared(object.get_position()))
                        },
                    ).map(ObjectId)
                } else {
                    None
                };
                if let Some(target) = nearest {
                    if self.ignite_object_fire_spread(target, presentation_effects) {
                        ignited_this_update.insert(target);
                    }
                }
                // FireSpreadUpdate calculates its next wake only after the
                // target's ignition has completed, so its draw follows any
                // draw made by that target's startFireSpreading callback.
                if let Some(object) = self.objects.get_mut(&id).filter(|object| {
                    std::sync::Arc::ptr_eq(&object.visual_draw_identity_token, &incarnation)
                }) {
                    if let Some(fire) = object.fire_spread.as_mut() {
                        fire.schedule_next_spread_with_random(frame, &self.logic_random);
                    }
                }
            }
        }
        Ok(())
    }
}
