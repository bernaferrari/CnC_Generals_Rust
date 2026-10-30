//! C++ ActiveBody.cpp:646–653: score, onDie, fresh source lookup, DamageFX.
//! This owner keeps both objects installed and finishes before the next hit.
use super::super::*;
use crate::game_logic::object::{DamageApplication, DamageHitContext};
#[path = "damage_callbacks.rs"]
mod damage_callbacks;

/// Retarget observations taken after body callbacks, before world onDie.
#[derive(Debug)]
pub(crate) struct OwnedDamageResult {
    pub(in crate::game_logic) destroyed: bool,
    /// HP delta for this impact, before world callbacks can damage/heal again.
    pub(in crate::game_logic) hp_lost: f32,
    pub(in crate::game_logic) victim_position: glam::Vec3,
    pub(in crate::game_logic) victim_team: Team,
}

impl OwnedDamageResult {
    pub(crate) fn destroyed(&self) -> bool {
        self.destroyed
    }
}

impl GameLogic {
    /// Actual Body::setMaxHealth: no attemptHealing or damage callback walk.
    pub(in crate::game_logic) fn set_owned_body_max_health_with_effects(
        &mut self,
        id: ObjectId,
        maximum: f32,
        mode: crate::game_logic::object::MaxHealthChange,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<()> {
        #[cfg(feature = "game_client")]
        let mut change = {
            let object = self.objects.get(&id)?;
            let player = object.owner_player_id.map(|id| id as i32).unwrap_or(-1);
            crate::game_logic::NativeBodyVisualChange {
                audio_policy: self.queued_audio_events.policy().clone(),
                identity: self.live_draw_playback_identity(object),
                state: gamelogic::common::types::BodyDamageType::Pristine,
                loading_map: self.is_loading_map(),
                time_of_day: self.native_lighting_time_of_day(),
                locality: self.receiving_audio_locality_for_player(player),
            }
        };
        let player = self.local_player_id();
        let bone_source =
            player.and_then(|player| self.current_native_slave_source(id, player).ok());
        let frame = self.frame;
        let shroud = self.world_services.shroud();
        let particles = &mut self.combat_particles;
        let random = &mut self.session_view.client_random;
        let object = self.objects.get_mut(&id)?;
        object.set_body_max_health_with_mode_and_body_fx(
            maximum,
            mode,
            &mut |object, effect, _source| match effect {
                crate::game_logic::object::BodyStateFx::NativeVisualState(state) => {
                    #[cfg(feature = "game_client")]
                    if let Some(view) = effects.view.as_mut() {
                        let audio = effects
                            .audio
                            .as_mut()
                            .and_then(|audio| audio.native_ambient());
                        change.deliver(state, &mut **view, audio);
                    }
                    crate::game_logic::body_particle_runtime::update_body_auto_particles(
                        particles,
                        object,
                        state,
                        frame,
                        player,
                        shroud,
                        effects,
                        random,
                        bone_source.as_ref(),
                    );
                }
                crate::game_logic::object::BodyStateFx::BodyParticleState => {
                    crate::game_logic::body_particle_runtime::update_body_auto_particles(
                        particles,
                        object,
                        object.body_damage_state,
                        frame,
                        player,
                        shroud,
                        effects,
                        random,
                        bone_source.as_ref(),
                    );
                }
                _ => {}
            },
        );
        Some(())
    }

    pub(in crate::game_logic) fn record_neutral_vehicle_sniped(
        players: &mut std::collections::HashMap<u32, crate::game_logic::Player>,
    ) {
        let neutral = players
            .iter()
            .filter_map(|(id, player)| player.is_reserved_neutral().then_some(*id))
            .min();
        if let Some(player) = neutral.and_then(|id| players.get_mut(&id)) {
            player.academy_stats.record_vehicle_sniped();
        }
    }

    pub(in crate::game_logic) fn apply_body_player_effect(
        players: &mut std::collections::HashMap<u32, crate::game_logic::Player>,
        frame: u32,
        source_players: &[Option<(ObjectId, u32)>],
        effect: crate::game_logic::object::BodyPlayerEffect,
    ) {
        match effect {
            crate::game_logic::object::BodyPlayerEffect::AttackedBy {
                victim_player,
                selected_source,
            } => Self::notify_residual_attacked_by(
                players,
                frame,
                source_players,
                victim_player,
                selected_source,
            ),
            crate::game_logic::object::BodyPlayerEffect::NeutralVehicleSniped => {
                Self::record_neutral_vehicle_sniped(players)
            }
        }
    }

    /// Shared bounded Body callback; source observations belong to this owner
    /// and are captured before its non-reentrant body mutation.
    pub(in crate::game_logic) fn notify_residual_attacked_by(
        players: &mut std::collections::HashMap<u32, crate::game_logic::Player>,
        frame: u32,
        source_players: &[Option<(ObjectId, u32)>],
        victim_player: Option<u32>,
        selected_source: Option<ObjectId>,
    ) {
        let Some(victim_player) = victim_player else {
            return;
        };
        let Some(attacker) = selected_source.and_then(|source| {
            source_players
                .iter()
                .flatten()
                .find(|(id, _)| *id == source)
                .map(|(_, player)| *player)
        }) else {
            return;
        };
        if let Some(player) = players.get_mut(&victim_player) {
            player.set_attacked_by_at(attacker as i32, frame);
        }
    }

    /// Weapon.cpp:1351/1470 excludes airborne victims, then completes each
    /// attemptDamage (including onDie) before advancing the victim iterator.
    pub(in crate::game_logic) fn apply_owned_radiation_field_tick(
        &mut self,
        victim_id: ObjectId,
        amount: f32,
        source_id: ObjectId,
        source_team: Team,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<bool> {
        let victim = self.objects.get(&victim_id)?;
        if !victim.is_alive() {
            return None;
        }
        if victim.is_significantly_above_terrain() {
            return Some(false);
        }
        self.apply_owned_damage_with_killer_team_with_effects(
            victim_id,
            amount,
            Some(source_id),
            crate::game_logic::combat::DamageType::Radiation,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
            Some(source_team),
            effects,
        )
        .map(|result| result.destroyed())
    }

    /// Complete residual Body, Player and world callbacks on this receiving
    /// session before the caller resumes its candidate loop.
    pub(in crate::game_logic) fn apply_residual_damage_owned(
        &mut self,
        victim_id: ObjectId,
        amount: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
    ) -> bool {
        self.apply_residual_damage_owned_with_effects(
            victim_id,
            amount,
            source_id,
            damage_type,
            death_type,
            fx_override,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn apply_residual_damage_owned_with_effects(
        &mut self,
        victim_id: ObjectId,
        amount: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        // Residual weapons and world effects use the same installed Body owner
        // as ordinary impacts. Finish reentrant bridge, score and onDie callbacks
        // before returning to the caller's original next-candidate boundary.
        self.apply_owned_damage_with_effects(
            victim_id,
            amount,
            source_id,
            damage_type,
            death_type,
            fx_override,
            &DamageHitContext::default(),
            effects,
        )
        .is_some_and(|result| result.destroyed)
    }

    /// CPP Object1930–1940: preserve the requested damage/death types and
    /// invalid source, with the kill bit carried by this owned impact context.
    pub(in crate::game_logic) fn apply_owned_kill(
        &mut self,
        victim_id: ObjectId,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> Option<OwnedDamageResult> {
        self.apply_owned_kill_with_effects(
            victim_id,
            damage_type,
            death_type,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn apply_owned_kill_with_effects(
        &mut self,
        victim_id: ObjectId,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<OwnedDamageResult> {
        let victim = self.objects.get(&victim_id)?;
        let amount = if victim.thing().template.body_kind.is_some() {
            // CPP Object::kill takes the actual receiving Body max health,
            // including zero for InactiveBody; the kill bit is independent.
            victim.health.maximum
        } else if victim.health.maximum > 0.0 {
            victim.health.maximum
        } else {
            victim.max_health.max(1.0)
        };
        self.apply_owned_damage_with_effects(
            victim_id,
            amount,
            None,
            damage_type,
            death_type,
            None,
            &DamageHitContext::for_kill(),
            effects,
        )
    }

    /// Admit one unapplied health operation through the receiving body's real
    /// synchronous callbacks. Completed receipts never enter this method.

    pub(crate) fn apply_owned_damage(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        context: &DamageHitContext,
    ) -> Option<OwnedDamageResult> {
        self.apply_owned_damage_with_effects(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            context,
            &mut Default::default(),
        )
    }

    pub(crate) fn apply_owned_damage_with_effects(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        context: &DamageHitContext,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<OwnedDamageResult> {
        self.apply_owned_damage_with_killer_team_with_effects(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            context,
            None,
            effects,
        )
    }

    /// CPP Object::attemptHealing calls Body::attemptHealing directly. Keep
    /// repair admission distinct from weapons carrying DAMAGE_HEALING while
    /// sharing the receiving owner's synchronous Body/callback completion.
    pub(crate) fn apply_owned_healing(
        &mut self,
        victim_id: ObjectId,
        amount: f32,
        source_id: Option<ObjectId>,
    ) -> Option<OwnedDamageResult> {
        self.apply_owned_healing_with_effects(victim_id, amount, source_id, &mut Default::default())
    }

    pub(crate) fn apply_owned_healing_with_effects(
        &mut self,
        victim_id: ObjectId,
        amount: f32,
        source_id: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<OwnedDamageResult> {
        let context = DamageHitContext::new(
            source_id.and_then(|id| self.objects.get(&id)),
            None,
            crate::game_logic::combat::DamageType::Healing,
        )
        .with_healing_entry();
        self.apply_owned_damage_with_effects(
            victim_id,
            amount,
            source_id,
            crate::game_logic::combat::DamageType::Healing,
            crate::game_logic::host_usa_pilot::HostDeathType::None,
            None,
            &context,
            effects,
        )
    }

    /// Callers that already capture a killer team preserve that provenance.
    /// Queued and moving impacts pass no override, retaining their live-source
    /// team lookup. Source identity still drives XP and the fresh DamageFX lookup.
    pub(in crate::game_logic) fn apply_owned_damage_with_killer_team(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        context: &DamageHitContext,
        killer_team_override: Option<Team>,
    ) -> Option<OwnedDamageResult> {
        self.apply_owned_damage_with_killer_team_with_effects(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            context,
            killer_team_override,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn apply_owned_damage_with_killer_team_with_effects(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        context: &DamageHitContext,
        killer_team_override: Option<Team>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<OwnedDamageResult> {
        self.apply_owned_damage_with_killer_team_and_after_fx_with_effects(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            context,
            killer_team_override,
            |_, _| {},
            effects,
        )
    }

    /// The observer runs at this same real synchronous DamageFX boundary;
    /// policy is then read from the driving world, never captured at ingress.
    pub(in crate::game_logic) fn apply_owned_damage_with_killer_team_and_after_fx(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        context: &DamageHitContext,
        killer_team_override: Option<Team>,
        after_damage_fx: impl FnOnce(&mut Self, ObjectId),
    ) -> Option<OwnedDamageResult> {
        self.apply_owned_damage_with_killer_team_and_after_fx_with_effects(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            context,
            killer_team_override,
            after_damage_fx,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn apply_owned_damage_with_killer_team_and_after_fx_with_effects(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        context: &DamageHitContext,
        killer_team_override: Option<Team>,
        after_damage_fx: impl FnOnce(&mut Self, ObjectId),
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<OwnedDamageResult> {
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();

        let mut receiving_effects = effects.reborrow();
        if let Some(interpreter) = context.topple_interpreter() {
            receiving_effects.script_events =
                Some(crate::game_logic::ScriptEventRecipient::new(interpreter));
        }
        let effects = &mut receiving_effects;

        // Before any reentrant world callback, ActiveBody can select either
        // this impact's source or its retained same/previous-frame source.
        // Resolve both from this receiving object store; the bounded body phase
        // cannot remove or transfer either source before its Player callback.
        let previous_source = self.objects.get(&victim_id)?.last_damage_source;
        let source_players = [source_id, previous_source].map(|source| {
            source.and_then(|id| {
                self.objects
                    .get(&id)
                    .and_then(|object| object.owner_player_id.map(|player| (id, player)))
            })
        });
        let context = context
            .clone()
            .with_last_damage_sources(
                source_id.and_then(|id| self.objects.get(&id)),
                previous_source.and_then(|id| self.objects.get(&id)),
            )
            .with_neutral_player_id(
                self.players
                    .iter()
                    .filter_map(|(id, player)| player.is_reserved_neutral().then_some(*id))
                    .min(),
            )
            .with_owned_hive_body();
        // C++ HiveStructureBody forwards the same DamageInfo synchronously to
        // the closest Object in SpawnBehavior::m_spawnIDs. Main owns that
        // relationship on the receiving parent; do the forwarding before the
        // parent enters ActiveBody so no virtual slot mirror can become a
        // second damage authority.
        let mut after_damage_fx = Some(after_damage_fx);
        if let Some(result) = self.try_owned_hive_structure_damage(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            killer_team_override,
            &mut after_damage_fx,
            &context,
            effects,
        ) {
            return Some(result);
        }
        let frame = self.frame;
        let before_hp = self.objects.get(&victim_id)?.health.current;
        let callback_player = self.local_player_id();
        #[cfg(feature = "game_client")]
        let begin_player = callback_player;
        #[cfg(feature = "game_client")]
        let begin_armor_source = source_id
            .and_then(|id| self.objects.get(&id))
            .map(crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source);
        #[cfg(feature = "game_client")]
        let begin_secondaries = [source_id, previous_source].map(|id| {
            id.and_then(|id| self.objects.get(&id)).map(|object| {
                Self::observe_host_fx_object(object, begin_player, self.world_services.shroud())
            })
        });
        #[cfg(feature = "game_client")]
        let native_body = {
            let victim = self.objects.get(&victim_id)?;
            let source = victim.owner_player_id.map(|id| id as i32).unwrap_or(-1);
            (
                self.live_draw_playback_identity(victim),
                self.is_loading_map(),
                self.native_lighting_time_of_day(),
                self.receiving_audio_locality_for_player(source),
            )
        };
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();
        #[cfg(feature = "game_client")]
        let mut application = Self::with_object_fx_services(
            &mut self.session_view,
            self.queued_audio_events.policy().clone(),
            begin_player,
            self.world_services.shroud(),
            effects,
            &fx_operation_environment,
            self.world_services.terrain(),
            &self.logic_random,
            |view| {
                let victim = self
                    .objects
                    .get_mut(&victim_id)
                    .expect("validated receiving victim");
                let players = &mut self.players;
                victim.begin_damage_with_context_and_effects_with_operation(
                    damage,
                    source_id,
                    damage_type,
                    death_type,
                    fx_override,
                    frame,
                    &context,
                    &mut self.health_events,
                    Some(&mut self.ownership_events),
                    |effect| {
                        Self::apply_body_player_effect(players, frame, &source_players, effect)
                    },
                    &mut |object, effect, source| {
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
                            // ActiveBody::evaluateVisualCondition updates its
                            // native drawable first, then synchronously replaces
                            // body auto-particles before entering later callbacks.
                            view.update_body_auto_particles(
                                &mut self.combat_particles,
                                object,
                                *state,
                                frame,
                            );
                            return;
                        }
                        let source = if matches!(
                            &effect,
                            crate::game_logic::object::BodyStateFx::ArmorDamage { .. }
                        ) {
                            begin_armor_source.as_ref()
                        } else {
                            source
                        };
                        let secondary_id = if matches!(
                            &effect,
                            crate::game_logic::object::BodyStateFx::ArmorDamage { .. }
                        ) {
                            source.map(|source| ObjectId(source.id))
                        } else {
                            object.last_damage_source
                        };
                        let secondary = begin_secondaries
                            .iter()
                            .flatten()
                            .find(|pose| Some(ObjectId(pose.id)) == secondary_id);
                        Self::execute_body_state_fx_with_view(
                            object,
                            effect,
                            source,
                            secondary,
                            begin_player,
                            self.world_services.shroud(),
                            &mut self.combat_particles,
                            &mut self.queued_audio_events,
                            frame,
                            &self.damage_fx_definitions,
                            view,
                        );
                    },
                    self.frame,
                    &mut crate::game_logic::ScriptPresentationEffects::default()
                        .reborrow_for_object(&mut self.partition_manager),
                    true,
                )
            },
        );
        #[cfg(not(feature = "game_client"))]
        let mut application: DamageApplication = {
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            let body_particle_player = callback_player;
            #[cfg(not(feature = "game_client"))]
            let mut body_particles =
                crate::game_logic::body_particle_runtime::HeadlessBodyParticleUpdate {
                    source: body_particle_player.and_then(|player| {
                        self.current_native_slave_source(victim_id, player).ok()
                    }),
                    pristine_bones: effects.pristine_bones,
                    particles: &mut self.combat_particles,
                    random: &mut self.session_view.client_random,
                    player: body_particle_player,
                    shroud: self.world_services.shroud(),
                    frame: self.frame,
                };
            let players = &mut self.players;
            self.objects
                .get_mut(&victim_id)?
                .begin_damage_with_context_and_effects_with_operation(
                    damage,
                    source_id,
                    damage_type,
                    death_type,
                    fx_override,
                    frame,
                    &context,
                    &mut self.health_events,
                    Some(&mut self.ownership_events),
                    |effect| {
                        Self::apply_body_player_effect(players, frame, &source_players, effect)
                    },
                    &mut |object, effect, source| {
                        Self::execute_body_state_fx_without_view_with_particles(
                            object,
                            effect,
                            source,
                            &mut self.queued_audio_events,
                            &self.logic_random,
                            &self.damage_fx_definitions,
                            &mut body_particles,
                        )
                    },
                    self.frame,
                    &mut crate::game_logic::ScriptPresentationEffects::default()
                        .reborrow_for_object(&mut self.partition_manager),
                    true,
                )
        };
        if let Some(continuation) = application.take_rider_change_kill_pilot() {
            if let Some(transition) = continuation.transition.as_ref() {
                let Some(victim) = self.objects.get(&victim_id) else {
                    return None;
                };
                let rider_identity = self.live_draw_playback_identity(victim);
                self.execute_receiving_transition_damage_fx(
                    victim_id,
                    &rider_identity,
                    transition.clone(),
                    continuation.source_fx.as_ref(),
                    self.frame,
                    callback_player,
                    effects,
                );
                if !self.objects.get(&victim_id).is_some_and(|object| {
                    self.live_draw_playback_identity(object) == rider_identity
                }) {
                    return None;
                }
            }

            #[cfg(feature = "game_client")]
            {
                let environment = self.observe_object_fx_environment();
                Self::with_object_fx_services(
                    &mut self.session_view,
                    self.queued_audio_events.policy().clone(),
                    callback_player,
                    self.world_services.shroud(),
                    effects,
                    &environment,
                    self.world_services.terrain(),
                    &self.logic_random,
                    |view| {
                        let players = &mut self.players;
                        let Some(victim) = self.objects.get_mut(&victim_id) else {
                            return;
                        };
                        victim.finish_rider_change_kill_pilot_after_transition(
                            &continuation,
                            &mut self.health_events,
                            |effect| {
                                Self::apply_body_player_effect(
                                    players,
                                    frame,
                                    &source_players,
                                    effect,
                                )
                            },
                            &mut |object, effect, source| {
                                let source = if matches!(
                                    &effect,
                                    crate::game_logic::object::BodyStateFx::ArmorDamage { .. }
                                ) {
                                    continuation.source_fx.as_ref()
                                } else {
                                    source
                                };
                                let secondary_id = if matches!(
                                    &effect,
                                    crate::game_logic::object::BodyStateFx::ArmorDamage { .. }
                                ) {
                                    source.map(|source| ObjectId(source.id))
                                } else {
                                    object.last_damage_source
                                };
                                let secondary = begin_secondaries
                                    .iter()
                                    .flatten()
                                    .find(|pose| Some(ObjectId(pose.id)) == secondary_id);
                                Self::execute_body_state_fx_with_view(
                                    object,
                                    effect,
                                    source,
                                    secondary,
                                    callback_player,
                                    self.world_services.shroud(),
                                    &mut self.combat_particles,
                                    &mut self.queued_audio_events,
                                    frame,
                                    &self.damage_fx_definitions,
                                    view,
                                );
                            },
                        );
                    },
                );
            }
            #[cfg(not(feature = "game_client"))]
            {
                let mut body_particles =
                    crate::game_logic::body_particle_runtime::HeadlessBodyParticleUpdate {
                        source: callback_player.and_then(|player| {
                            self.current_native_slave_source(victim_id, player).ok()
                        }),
                        pristine_bones: effects.pristine_bones,
                        particles: &mut self.combat_particles,
                        random: &mut self.session_view.client_random,
                        player: callback_player,
                        shroud: self.world_services.shroud(),
                        frame: frame as u32,
                    };
                let players = &mut self.players;
                if let Some(victim) = self.objects.get_mut(&victim_id) {
                    victim.finish_rider_change_kill_pilot_after_transition(
                        &continuation,
                        &mut self.health_events,
                        |effect| {
                            Self::apply_body_player_effect(players, frame, &source_players, effect)
                        },
                        &mut |object, effect, source| {
                            Self::execute_body_state_fx_without_view_with_particles(
                                object,
                                effect,
                                source,
                                &mut self.queued_audio_events,
                                &self.logic_random,
                                &self.damage_fx_definitions,
                                &mut body_particles,
                            )
                        },
                    );
                }
            }
        }
        let victim = self
            .objects
            .get(&victim_id)
            .expect("Body phase preserves receiving victim");
        let hp_lost = (before_hp - victim.health.current).max(0.0);
        let hp_gained = (victim.health.current - before_hp).max(0.0);
        let victim_identity = self.live_draw_playback_identity(victim);
        let victim_position = victim.get_position();
        let victim_team = victim.team;
        self.admit_object_drawable_deselection(victim_id);
        self.admit_host_radar_disabled_edges(victim_id);
        // CPP ActiveBody585–600 calls onDamage before scoreTheKill/onDie.
        // Release the body loan before forceFireWeapon can damage other owners
        // or reenter this same object; every next hit resolves it afresh.
        let mut reaction_callback_ran = false;
        let mut transition_callback_ran = false;
        let has_installed_reaction_weapons = self
            .objects
            .get(&victim_id)
            .is_some_and(|object| !object.temporary_weapon_runtime.damaged.is_empty());
        let healing = damage_type == crate::game_logic::combat::DamageType::Healing;
        // ActiveBody.cpp:824 admits onHealing only for a real health increase;
        // clamping an already over-maximum Body downward is not onDamage.
        let authored_callbacks = if (!healing && hp_lost > 0.0) || (healing && hp_gained > 0.0) {
            self.run_authored_mine_and_weapon_damage_callbacks(
                victim_id,
                application.actual_damage_dealt().unwrap_or(hp_lost),
                source_id,
                damage_type,
                healing,
                effects,
            )
        } else {
            None
        };
        if let Some(ran) = authored_callbacks {
            reaction_callback_ran = ran;
            // Captured callbacks own the actual source-order walk. Do not
            // replay the single-weapon compatibility latch afterward.
            if !healing {
                #[cfg(feature = "game_client")]
                let fx_operation_environment = self.observe_object_fx_environment();
                if let Some(object) = self.objects.get_mut(&victim_id) {
                    let _ = object.take_pending_fire_when_damaged_weapon();
                }
            }
        } else {
            if hp_lost > 0.0 && damage_type != crate::game_logic::combat::DamageType::Healing {
                if has_installed_reaction_weapons {
                    if let Some(actual_damage) = application.actual_damage_dealt() {
                        // Offer the installed source-ordered modules while their
                        // Body state is this hit's state, before bridge/state/Die
                        // callbacks. A callback may reenter even with zero hits.
                        reaction_callback_ran = true;
                        #[cfg(feature = "game_client")]
                        let fx_operation_environment = self.observe_object_fx_environment();
                        let _ = self
                            .objects
                            .get_mut(&victim_id)
                            .and_then(|object| object.take_pending_fire_when_damaged_weapon());
                        let _ = self.execute_temporary_weapon_on_damage_with_effects(
                            victim_id,
                            actual_damage,
                            damage_type.to_store() as u32,
                            effects,
                        );
                    }
                    #[cfg(feature = "game_client")]
                    let fx_operation_environment = self.observe_object_fx_environment();
                } else if let Some(weapon) = self
                    .objects
                    .get_mut(&victim_id)
                    .and_then(|object| object.take_pending_fire_when_damaged_weapon())
                {
                    reaction_callback_ran = true;
                    let _ = self.apply_fire_weapon_when_damaged_named_with_effects(
                        victim_id, &weapon, effects,
                    );
                }
            }
        }
        if reaction_callback_ran
            && !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
        {
            // A reentrant callback retired/reset this incarnation. Its
            // continuation must never be completed against a replacement ID.
            return None;
        }
        let bridge_callbacks = application.bridge_callbacks();
        if let Some(callbacks) = bridge_callbacks {
            self.complete_owned_bridge_body_callbacks_with_effects(callbacks, effects);
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
            {
                return None;
            }
        }
        let callback_source = application
            .source_id()
            .and_then(|id| self.objects.get(&id))
            .map(crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source);
        #[cfg(feature = "game_client")]
        let callback_player = self.local_player_id();
        #[cfg(feature = "game_client")]
        let callback_secondary = self.observe_fx_list_die_secondary(victim_id);
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();
        #[cfg(feature = "game_client")]
        let mut transition_yield = None;
        #[cfg(feature = "game_client")]
        Self::with_object_fx_services(
            &mut self.session_view,
            self.queued_audio_events.policy().clone(),
            callback_player,
            self.world_services.shroud(),
            effects,
            &fx_operation_environment,
            self.world_services.terrain(),
            &self.logic_random,
            |view| {
                if let Some(victim) = self.objects.get_mut(&victim_id) {
                    transition_yield = application.complete_body_state_callbacks_yield_transition(
                        victim,
                        callback_source.as_ref(),
                        &self.logic_random,
                        &mut |object, effect, source| {
                            Self::execute_body_state_fx_with_view(
                                object,
                                effect,
                                source,
                                callback_secondary.as_ref(),
                                callback_player,
                                self.world_services.shroud(),
                                &mut self.combat_particles,
                                &mut self.queued_audio_events,
                                self.frame,
                                &self.damage_fx_definitions,
                                view,
                            );
                        },
                    );
                }
            },
        );
        #[cfg(feature = "game_client")]
        if let Some(event) = transition_yield.take() {
            transition_callback_ran = true;
            self.execute_receiving_transition_damage_fx(
                victim_id,
                &victim_identity,
                event,
                callback_source.as_ref(),
                self.frame,
                callback_player,
                effects,
            );
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
            {
                return None;
            }
            let environment = self.observe_object_fx_environment();
            Self::with_object_fx_services(
                &mut self.session_view,
                self.queued_audio_events.policy().clone(),
                callback_player,
                self.world_services.shroud(),
                effects,
                &environment,
                self.world_services.terrain(),
                &self.logic_random,
                |view| {
                    if let Some(victim) = self.objects.get_mut(&victim_id) {
                        application.finish_body_state_callbacks_after_transition(
                            victim,
                            callback_source.as_ref(),
                            &self.logic_random,
                            &mut |object, effect, source| {
                                Self::execute_body_state_fx_with_view(
                                    object,
                                    effect,
                                    source,
                                    callback_secondary.as_ref(),
                                    callback_player,
                                    self.world_services.shroud(),
                                    &mut self.combat_particles,
                                    &mut self.queued_audio_events,
                                    self.frame,
                                    &self.damage_fx_definitions,
                                    view,
                                );
                            },
                        );
                    }
                },
            );
        }
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();
        #[cfg(not(feature = "game_client"))]
        let mut transition_yield = None;
        #[cfg(not(feature = "game_client"))]
        let body_particle_player = self.local_player_id();
        #[cfg(not(feature = "game_client"))]
        let mut body_particles =
            crate::game_logic::body_particle_runtime::HeadlessBodyParticleUpdate {
                source: body_particle_player
                    .and_then(|player| self.current_native_slave_source(victim_id, player).ok()),
                pristine_bones: effects.pristine_bones,
                particles: &mut self.combat_particles,
                random: &mut self.session_view.client_random,
                player: body_particle_player,
                shroud: self.world_services.shroud(),
                frame: self.frame,
            };
        #[cfg(not(feature = "game_client"))]
        if let Some(victim) = self.objects.get_mut(&victim_id) {
            transition_yield = application.complete_body_state_callbacks_yield_transition(
                victim,
                callback_source.as_ref(),
                &self.logic_random,
                &mut |object, effect, source| {
                    Self::execute_body_state_fx_without_view_with_particles(
                        object,
                        effect,
                        source,
                        &mut self.queued_audio_events,
                        &self.logic_random,
                        &self.damage_fx_definitions,
                        &mut body_particles,
                    )
                },
            );
        }
        #[cfg(not(feature = "game_client"))]
        if let Some(event) = transition_yield.take() {
            transition_callback_ran = true;
            self.execute_receiving_transition_damage_fx(
                victim_id,
                &victim_identity,
                event,
                callback_source.as_ref(),
                self.frame,
                None,
                effects,
            );
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
            {
                return None;
            }
            let same_victim = self
                .objects
                .get(&victim_id)
                .is_some_and(|victim| self.live_draw_playback_identity(victim) == victim_identity);
            if same_victim {
                let body_particle_player = self.local_player_id();
                let mut body_particles =
                    crate::game_logic::body_particle_runtime::HeadlessBodyParticleUpdate {
                        source: body_particle_player.and_then(|player| {
                            self.current_native_slave_source(victim_id, player).ok()
                        }),
                        pristine_bones: effects.pristine_bones,
                        particles: &mut self.combat_particles,
                        random: &mut self.session_view.client_random,
                        player: body_particle_player,
                        shroud: self.world_services.shroud(),
                        frame: self.frame,
                    };
                if let Some(victim) = self.objects.get_mut(&victim_id) {
                    application.finish_body_state_callbacks_after_transition(
                        victim,
                        callback_source.as_ref(),
                        &self.logic_random,
                        &mut |object, effect, source| {
                            Self::execute_body_state_fx_without_view_with_particles(
                                object,
                                effect,
                                source,
                                &mut self.queued_audio_events,
                                &self.logic_random,
                                &self.damage_fx_definitions,
                                &mut body_particles,
                            )
                        },
                    );
                }
            }
        }
        // Original ActiveBody rereads live health after synchronous callbacks.
        let killed =
            if (bridge_callbacks.is_some() || reaction_callback_ran || transition_callback_ran)
                && damage_type != crate::game_logic::combat::DamageType::Healing
            {
                before_hp > 0.0
                    && self
                        .objects
                        .get(&victim_id)
                        .is_some_and(|o| o.health.current <= 0.0)
            } else {
                application.killed()
            };
        if bridge_callbacks.is_some() || reaction_callback_ran || transition_callback_ran {
            application.set_killed_after_callbacks(killed);
        }
        if killed {
            // Keep the existing once-only gates and XP-sink routing. Do not
            // predict a promotion or cache XP before the callbacks.
            // CPP InactiveBody calls only onDie, never ActiveBody's
            // scoreTheKill phase or damage/healing callbacks.
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| object.is_inactive_body())
            {
                if let Some(source) = source_id {
                    self.award_score_the_kill_experience_with_effects_and_interpreter(
                        source,
                        victim_id,
                        context.topple_interpreter(),
                        effects,
                    )
                    .expect("damage kill XP callback must complete");
                }
            }
            // The XP promotion callback can reenter world code and replace the
            // victim under the same numeric ID. Do not run onDie against that
            // new birth; this method already owns the original identity.
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
            {
                return None;
            }
            let killer_team = killer_team_override
                .or_else(|| source_id.and_then(|id| self.objects.get(&id).map(|obj| obj.team)));
            self.mark_object_for_destruction_with_topple_with_effects(
                victim_id,
                killer_team,
                crate::game_logic::object::OnDieInput {
                    source: source_id,
                    damage_type,
                    death_type,
                },
                context.topple_interpreter(),
                effects,
            );
        }
        let source = application
            .source_id()
            .and_then(|id| self.objects.get(&id))
            .map(crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source);
        // mark_object_for_destruction queues final removal; the synchronous
        // onDie boundary must leave the victim installed for its DamageFX.
        #[cfg(feature = "game_client")]
        let armor_player = self.local_player_id();
        #[cfg(feature = "game_client")]
        let armor_secondary = application
            .source_id()
            .and_then(|id| self.objects.get(&id))
            .map(|object| {
                Self::observe_host_fx_object(object, armor_player, self.world_services.shroud())
            });
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();
        #[cfg(feature = "game_client")]
        let after_fx = Self::with_object_fx_services(
            &mut self.session_view,
            self.queued_audio_events.policy().clone(),
            armor_player,
            self.world_services.shroud(),
            effects,
            &fx_operation_environment,
            self.world_services.terrain(),
            &self.logic_random,
            |view| {
                let victim = self
                    .objects
                    .get_mut(&victim_id)
                    .expect("damage victim remains installed until destroy-list cleanup");
                application.dispatch_damage_fx_with_body_fx(
                    victim,
                    source.as_ref(),
                    &mut |object, effect, source| {
                        Self::execute_body_state_fx_with_view(
                            object,
                            effect,
                            source,
                            armor_secondary.as_ref(),
                            armor_player,
                            self.world_services.shroud(),
                            &mut self.combat_particles,
                            &mut self.queued_audio_events,
                            self.frame,
                            &self.damage_fx_definitions,
                            view,
                        )
                    },
                )
            },
        );
        #[cfg(not(feature = "game_client"))]
        let after_fx = {
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            let body_particle_player = self.local_player_id();
            #[cfg(not(feature = "game_client"))]
            let mut body_particles =
                crate::game_logic::body_particle_runtime::HeadlessBodyParticleUpdate {
                    source: body_particle_player.and_then(|player| {
                        self.current_native_slave_source(victim_id, player).ok()
                    }),
                    pristine_bones: effects.pristine_bones,
                    particles: &mut self.combat_particles,
                    random: &mut self.session_view.client_random,
                    player: body_particle_player,
                    shroud: self.world_services.shroud(),
                    frame: self.frame,
                };
            let victim = self
                .objects
                .get_mut(&victim_id)
                .expect("damage victim remains installed until destroy-list cleanup");
            application.dispatch_damage_fx_with_body_fx(
                victim,
                source.as_ref(),
                &mut |object, effect, source| {
                    Self::execute_body_state_fx_without_view_with_particles(
                        object,
                        effect,
                        source,
                        &mut self.queued_audio_events,
                        &self.logic_random,
                        &self.damage_fx_definitions,
                        &mut body_particles,
                    )
                },
            )
        };
        self.admit_object_drawable_deselection(victim_id);
        self.admit_host_radar_disabled_edges(victim_id);
        if after_fx.had_active_body() {
            after_damage_fx
                .take()
                .expect("non-forwarded damage retains its FX continuation")(
                self, victim_id
            );
        }
        let enable_repulsors = self.enable_repulsors;
        #[cfg(feature = "game_client")]
        let finish_secondary = self.observe_fx_list_die_secondary(victim_id);
        #[cfg(feature = "game_client")]
        let finish_player = self.local_player_id();
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();
        let body_particle_player = self.local_player_id();
        #[cfg(not(feature = "game_client"))]
        let mut body_particles =
            crate::game_logic::body_particle_runtime::HeadlessBodyParticleUpdate {
                source: body_particle_player
                    .and_then(|player| self.current_native_slave_source(victim_id, player).ok()),
                pristine_bones: effects.pristine_bones,
                particles: &mut self.combat_particles,
                random: &mut self.session_view.client_random,
                player: body_particle_player,
                shroud: self.world_services.shroud(),
                frame: self.frame,
            };
        #[cfg(feature = "game_client")]
        let destroyed = Self::with_object_fx_services(
            &mut self.session_view,
            self.queued_audio_events.policy().clone(),
            finish_player,
            self.world_services.shroud(),
            effects,
            &fx_operation_environment,
            self.world_services.terrain(),
            &self.logic_random,
            |view| {
                let victim = self
                    .objects
                    .get_mut(&victim_id)
                    .expect("post-FX victim remains installed until destroy-list cleanup");
                after_fx.finish_with_body_fx_and_partition(
                    victim,
                    source.as_ref(),
                    enable_repulsors,
                    &mut |object, effect, source| {
                        Self::execute_body_state_fx_with_view(
                            object,
                            effect,
                            source,
                            finish_secondary.as_ref(),
                            finish_player,
                            self.world_services.shroud(),
                            &mut self.combat_particles,
                            &mut self.queued_audio_events,
                            self.frame,
                            &self.damage_fx_definitions,
                            view,
                        )
                    },
                    self.frame,
                    &mut self.partition_manager,
                )
            },
        );
        #[cfg(not(feature = "game_client"))]
        let destroyed = {
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            let victim = self
                .objects
                .get_mut(&victim_id)
                .expect("post-FX victim remains installed until destroy-list cleanup");
            after_fx.finish_with_body_fx_and_partition(
                victim,
                source.as_ref(),
                enable_repulsors,
                &mut |object, effect, source| {
                    Self::execute_body_state_fx_without_view_with_particles(
                        object,
                        effect,
                        source,
                        &mut self.queued_audio_events,
                        &self.logic_random,
                        &self.damage_fx_definitions,
                        &mut body_particles,
                    )
                },
                self.frame,
                &mut self.partition_manager,
            )
        };
        self.admit_object_drawable_deselection(victim_id);
        self.admit_host_radar_disabled_edges(victim_id);
        Some(OwnedDamageResult {
            destroyed,
            hp_lost,
            victim_position,
            victim_team,
        })
    }

    fn try_owned_hive_structure_damage(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        killer_team_override: Option<Team>,
        after_damage_fx: &mut Option<impl FnOnce(&mut Self, ObjectId)>,
        context: &DamageHitContext,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<OwnedDamageResult> {
        use crate::game_logic::host_base_defense::{
            HostHiveDamageClass, hive_damage_class_for_type, is_stinger_site_structure,
        };

        let (template_name, position, team) = {
            let parent = self.objects.get(&victim_id)?;
            (
                parent.template_name.clone(),
                parent.get_position(),
                parent.team,
            )
        };
        if !is_stinger_site_structure(&template_name) {
            return None;
        }
        // HiveStructureBody delegates to SpawnBehavior only when the receiving
        // object actually has that interface. Name similarity alone is not
        // enough: otherwise this path would swallow damage C++ sends to the
        // ordinary structure body.
        let Some(parent) = self.objects.get(&victim_id) else {
            return None;
        };
        let authored_hive_body = parent
            .thing()
            .template
            .receiving_object_definition
            .as_ref()
            .map(|definition| {
                definition
                    .behavior_modules
                    .iter()
                    .find(|module| {
                        crate::game_logic::thing::BodyKind::from_module_class(&module.class_name)
                            .is_some()
                    })
                    .is_some_and(|module| {
                        module.class_name.eq_ignore_ascii_case("HiveStructureBody")
                    })
            })
            // Existing synthetic/programmatic host templates have no admitted
            // object definition. Preserve their legacy name-based fallback;
            // once a receiving definition exists, its effective Body class is
            // authoritative and an ActiveBody Stinger is not a HiveBody.
            .unwrap_or(true);
        if !authored_hive_body || !GameLogic::supports_spawn_behavior_child_roster(parent) {
            return None;
        }
        // The effective typed body belongs to this receiving template. A
        // synthetic template with no admitted definition keeps the legacy
        // host matrix; admitted content never falls back to that matrix.
        let (propagate, swallow) = if parent
            .thing()
            .template
            .receiving_object_definition
            .is_some()
        {
            let data = parent
                .thing()
                .template
                .receiving_client_template
                .as_ref()
                .and_then(|receiving| {
                    receiving.behavior_modules().iter().find(|entry| {
                        entry
                            .name
                            .as_str()
                            .eq_ignore_ascii_case("HiveStructureBody")
                    })
                })
                .and_then(|entry| {
                    entry.data.as_any().downcast_ref::<
                        gamelogic::object::body::hive_structure_body::HiveStructureBodyModuleData,
                    >()
                });
            let Some(data) = data else {
                log::error!(
                    "Admitted HiveStructureBody for '{}' has no typed receiving module data",
                    template_name
                );
                // The damage API cannot return an error; keep the invalid
                // admission from being interpreted as an ordinary hull hit.
                return Some(OwnedDamageResult {
                    destroyed: false,
                    hp_lost: 0.0,
                    victim_position: position,
                    victim_team: team,
                });
            };
            let kind = damage_type.to_store();
            (
                data.damage_types_to_propagate_to_slaves
                    .test_damage_type(kind),
                data.damage_types_to_swallow.test_damage_type(kind),
            )
        } else {
            let class = hive_damage_class_for_type(damage_type);
            (
                class != HostHiveDamageClass::HitStructure,
                class == HostHiveDamageClass::SwallowIfNoSlaves,
            )
        };
        if !propagate {
            return None;
        }
        let Some(source) = source_id.and_then(|id| self.objects.get(&id)) else {
            // HiveStructureBody only searches/forwards/swallow when its source
            // Object resolves. Missing source falls through to the structure.
            return None;
        };
        let source_position = source.get_position();
        let children = self
            .objects
            .get(&victim_id)
            .map(|parent| parent.spawn_behavior_children())
            .unwrap_or_default();
        let mut closest: Option<(ObjectId, f32)> = None;
        for (child_id, birth) in &children {
            let Some(child) = self.objects.get(child_id) else {
                continue;
            };
            let Some(parent) = self.objects.get(&victim_id) else {
                break;
            };
            if !parent.spawn_behavior_child_birth_matches(*child_id, birth, child) {
                continue;
            }
            let p = child.get_position();
            let dx = p.x - source_position.x;
            let dz = p.z - source_position.z;
            let distance = dx * dx + dz * dz;
            if closest.is_none_or(|(_, best)| distance < best) {
                // Strictly less preserves C++'s first roster entry on ties.
                closest = Some((*child_id, distance));
            }
        }
        let closest = closest.map(|(child_id, _)| child_id);

        if let Some(child_id) = closest {
            let child_before = self.objects.get(&child_id)?.health.current;
            let child_after_fx = after_damage_fx.take()?;
            let child_context = context.clone();
            let child_result = self.apply_owned_damage_with_killer_team_and_after_fx_with_effects(
                child_id,
                damage,
                source_id,
                damage_type,
                death_type,
                fx_override,
                &child_context,
                killer_team_override,
                child_after_fx,
                effects,
            );
            let Some(child_result) = child_result else {
                // The synchronous Body callback re-entered and retired the
                // selected birth. C++ still returns from HiveStructureBody;
                // do not fall through and damage the parent hull.
                return Some(OwnedDamageResult {
                    destroyed: false,
                    hp_lost: 0.0,
                    victim_position: position,
                    victim_team: team,
                });
            };
            let child_alive = self
                .objects
                .get(&child_id)
                .is_some_and(|child| child.health.is_alive());
            let child_on_die_started = self
                .objects
                .get(&child_id)
                .is_some_and(|child| child.status.on_die_started);
            self.combat_system.note_kill_for_on_die(
                child_id,
                child_before,
                child_result.destroyed,
                child_alive,
                child_on_die_started,
            );
            if let Some(force) = context.shock_wave_force()
                && let Some(child) = self.objects.get_mut(&child_id)
            {
                let _ = child.apply_shock_wave_impulse_with_operation(
                    force,
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
            }
            self.combat_system.queue_under_attack_if_dealt(
                child_id,
                damage_type,
                child_result.hp_lost,
            );
            return Some(OwnedDamageResult {
                destroyed: false,
                hp_lost: child_result.hp_lost,
                victim_position: position,
                victim_team: team,
            });
        }
        if swallow {
            let _ = (damage, context);
            return Some(OwnedDamageResult {
                destroyed: false,
                hp_lost: 0.0,
                victim_position: position,
                victim_team: team,
            });
        }
        None
    }
}

impl GameLogic {
    /// Complete one selected queued batch without detaching the CombatSystem.
    /// C++ ActiveBody.cpp:640–653 requires XP/onDie before fresh-source FX.
    /// Resolve only this accepted shot; earlier queued shots keep their phase.
    pub(in crate::game_logic) fn materialize_owned_passive_shot(
        &mut self,
        pending: crate::game_logic::combat::PendingProjectile,
        stream_binding: Option<crate::game_logic::combat::ProjectileStreamBinding>,
    ) {
        self.materialize_owned_passive_shot_with_effects(
            pending,
            stream_binding,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn materialize_owned_passive_shot_with_effects(
        &mut self,
        pending: crate::game_logic::combat::PendingProjectile,
        stream_binding: Option<crate::game_logic::combat::ProjectileStreamBinding>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let projectile_name = pending.projectile_object_name.clone();
        let projectile_terrain = crate::game_logic::game_logic::FlightTerrainView {
            grid: &self.pathfinding_system.grid,
            logical: Some(self.world_services.terrain()),
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        let ready = crate::game_logic::combat::materialize_accepted_projectile(
            &mut self.combat_system,
            self.objects.read(),
            self.frame,
            pending,
            stream_binding,
            self.projectile_definitions.get(&projectile_name),
            Some(&projectile_terrain),
            &mut self.next_object_id,
            &self.logic_random,
        );
        if let Some(shot) = ready {
            let limit = self.combat_system.historic_damage_limit();
            let events = self.combat_system.prepare_projectileless_delayed_shot(
                &shot,
                self.objects.read(),
                self.frame,
                limit,
            );
            for event in events {
                self.apply_owned_combat_damage_with_effects(&event, effects);
            }
        }
    }

    pub(in crate::game_logic) fn apply_owned_projectileless_delayed_damage(&mut self) {
        self.apply_owned_projectileless_delayed_damage_with_effects(&mut Default::default())
    }

    pub(in crate::game_logic) fn apply_owned_projectileless_delayed_damage_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::take_ready_projectileless_delayed_damage;
        let ready = take_ready_projectileless_delayed_damage(&mut self.combat_system, self.frame);
        let limit = self.combat_system.historic_damage_limit();
        for shot in ready {
            let events = self.combat_system.prepare_projectileless_delayed_shot(
                &shot,
                self.objects.read(),
                self.frame,
                limit,
            );
            for event in events {
                self.apply_owned_combat_damage_with_effects(&event, effects);
            }
        }
    }

    /// Host-only owner: CombatSystem remains installed through every callback.
    pub(in crate::game_logic) fn update_owned_projectile_impacts(
        &mut self,
        dt: f32,
    ) -> Vec<ObjectId> {
        self.update_owned_projectile_impacts_with_effects(dt, &mut Default::default())
    }

    pub(in crate::game_logic) fn update_owned_projectile_impacts_with_effects(
        &mut self,
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        let ids = self.combat_system.projectile_update_ids();
        let limit = self.combat_system.historic_damage_limit();
        let mut retired = Vec::new();
        for id in ids {
            let removals = self.update_owned_projectile_impact_with_effects(id, dt, limit, effects);
            retired.extend(removals);
        }
        // Retain registered entries across callbacks until the existing removal
        // boundary. C++ destroyObject marks first; physical deletion is later.
        self.combat_system.retire_projectile_impacts(&retired);
        retired
    }

    /// One bounded callback, suitable for the session UpdateModule dispatcher.
    /// No object/projectile borrow crosses the synchronous Body completion.
    pub(in crate::game_logic) fn update_owned_projectile_impact(
        &mut self,
        id: ObjectId,
        dt: f32,
        historic_damage_limit: u32,
    ) -> Vec<ObjectId> {
        self.update_owned_projectile_impact_with_effects(
            id,
            dt,
            historic_damage_limit,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn update_owned_projectile_impact_with_effects(
        &mut self,
        id: ObjectId,
        dt: f32,
        historic_damage_limit: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();

        #[cfg(feature = "game_client")]
        let player = self.local_player_id();
        let terrain = crate::game_logic::game_logic::FlightTerrainView {
            logical: Some(self.world_services.terrain().as_ref()),
            grid: &self.pathfinding_system.grid,
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        #[cfg(feature = "game_client")]
        let (events, retired) = {
            let shroud = self.world_services.shroud();
            #[cfg(feature = "game_client")]
            let fx_operation_environment = self.observe_object_fx_environment();
            Self::with_object_fx_services(
                &mut self.session_view,
                self.queued_audio_events.policy().clone(),
                player,
                shroud,
                effects,
                &fx_operation_environment,
                self.world_services.terrain(),
                &self.logic_random,
                |view| {
                    self.combat_system.prepare_projectile_impact_with_runtime(
                        id,
                        dt,
                        &mut self.objects,
                        Some(&mut self.countermeasures),
                        self.frame,
                        Some(&terrain),
                        &mut self.ownership_events,
                        historic_damage_limit,
                        self.receiving_object_definitions.as_ref(),
                        view,
                    )
                },
            )
        };
        #[cfg(not(feature = "game_client"))]
        let (events, retired) = self.combat_system.prepare_projectile_impact_with_runtime(
            id,
            dt,
            &mut self.objects,
            Some(&mut self.countermeasures),
            self.frame,
            Some(&terrain),
            &mut self.ownership_events,
            historic_damage_limit,
            self.receiving_object_definitions.as_ref(),
            &mut (),
        );

        for event in events {
            self.apply_owned_combat_damage_with_effects(&event, effects);
        }
        // Return deletion requests; do not physically remove the callback's
        // projectile while other callbacks can still inspect the arena.
        retired
    }

    fn apply_owned_combat_damage_with_effects(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::DamageEvent;
        if let DamageEvent::Direct {
            target_id,
            damage,
            damage_type,
            death_type,
            shooter_id,
            ..
        } = *event
        {
            let context = DamageHitContext::new(self.objects.get(&shooter_id), None, damage_type);
            let before = self
                .objects
                .get(&target_id)
                .map(|o| o.health.current)
                .unwrap_or(0.0);
            if let Some(result) = self.apply_owned_damage_with_effects(
                target_id,
                damage,
                Some(shooter_id),
                damage_type,
                death_type,
                None,
                &context,
                effects,
            ) {
                // Preserve excluded special-body deferred death fallback. Ordinary
                // owned deaths have already set on_die_started and are never queued twice.
                if let Some(target) = self.objects.get(&target_id) {
                    self.combat_system.note_kill_for_on_die(
                        target_id,
                        before,
                        result.destroyed,
                        target.health.is_alive(),
                        target.status.on_die_started,
                    );
                }
                self.combat_system.queue_under_attack_if_dealt(
                    target_id,
                    damage_type,
                    result.hp_lost,
                );
                if result.destroyed {
                    log::debug!(
                        "Projectile destroyed object {} (damage: {:.1}, type: {:?})",
                        target_id,
                        damage,
                        damage_type
                    );
                }
            }
        } else {
            self.apply_owned_area_damage_with_effects(event, effects);
        }
    }
}

impl GameLogic {
    fn apply_owned_area_damage_with_effects(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::{AreaDamage, DamageEvent};
        let DamageEvent::Area {
            shooter_id,
            damage_type,
            death_type,
            radius_weapon_name,
            ..
        } = event
        else {
            return;
        };
        let (radius_damage_angle, damage_at_self) = if radius_weapon_name.is_empty() {
            (std::f32::consts::PI, false)
        } else if let Some(definitions) = &self.weapon_definitions {
            let Some(weapon) = definitions.get(&radius_weapon_name.to_ascii_lowercase()) else {
                log::error!(
                    "Receiving weapon definition '{}' is missing for directional splash",
                    radius_weapon_name
                );
                return;
            };
            (
                weapon.radius_damage_angle,
                weapon.damage_dealt_at_self_position,
            )
        } else {
            // Unbound programmatic shots have the C++ full-circle default.
            (std::f32::consts::PI, false)
        };
        let area = AreaDamage::new(
            event,
            self.objects.read(),
            radius_damage_angle,
            damage_at_self,
        )
        .expect("Area event");
        let candidates = area.candidates(self.objects.read());
        for (victim_id, distance) in candidates {
            let Some(victim) = self.objects.get(&victim_id) else {
                continue;
            };
            let Some((damage, shock)) = area.impact(
                victim_id,
                victim,
                distance,
                &self.players,
                &self.team_factory,
                &self.templates,
            ) else {
                continue;
            };
            let before = victim.health.current;
            let result = if damage > 0.0 {
                let context =
                    DamageHitContext::new(self.objects.get(shooter_id), None, *damage_type)
                        .with_shock_wave_force(shock);
                self.apply_owned_damage_with_effects(
                    victim_id,
                    damage,
                    Some(*shooter_id),
                    *damage_type,
                    *death_type,
                    None,
                    &context,
                    effects,
                )
            } else {
                None
            };
            // C++ Object.cpp:1794–1860: body (including XP/onDie/FX), shock,
            // then under-attack accounting. No victim borrow crosses completion.
            if let Some(victim) = self.objects.get_mut(&victim_id) {
                if let Some(result) = &result {
                    self.combat_system.note_kill_for_on_die(
                        victim_id,
                        before,
                        result.destroyed,
                        victim.health.is_alive(),
                        victim.status.on_die_started,
                    );
                }
                if let Some(force) = shock {
                    let _ = victim.apply_shock_wave_impulse_with_operation(
                        force,
                        self.frame,
                        &mut effects.reborrow_for_object(&mut self.partition_manager),
                    );
                }
            }
            if let Some(result) = result {
                self.combat_system.queue_under_attack_if_dealt(
                    victim_id,
                    *damage_type,
                    result.hp_lost,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "repulsor_owner_tests.rs"]
mod repulsor_owner_tests;

#[cfg(test)]
#[path = "repulsor_fx_order_tests.rs"]
mod repulsor_fx_order_tests;

#[cfg(test)]
#[path = "sniper_academy_owner_tests.rs"]
mod sniper_academy_owner_tests;

#[cfg(test)]
#[path = "rider_kill_pilot_transition_owner_tests.rs"]
mod rider_kill_pilot_transition_owner_tests;

#[cfg(all(test, feature = "game_client"))]
#[path = "early_native_body_tests.rs"]
mod early_native_body_tests;
