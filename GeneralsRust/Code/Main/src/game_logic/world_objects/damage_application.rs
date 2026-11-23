//! C++ ActiveBody.cpp:646–653: score, onDie, fresh source lookup, DamageFX.
//! This owner keeps both objects installed and finishes before the next hit.
use super::super::*;
use crate::game_logic::object::{DamageApplication, DamageHitContext, KillPilotAction};
#[path = "damage_callbacks.rs"]
mod damage_callbacks;
#[cfg(test)]
#[path = "owned_shock_damage_tests.rs"]
mod owned_shock_damage_tests;

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

#[derive(Clone, Copy)]
enum RetiredProjectileActor {
    Destroy,
    KeepAfterDeath,
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
    ) -> gamelogic::GameLogicResult<Option<bool>> {
        let Some(victim) = self.objects.get(&victim_id) else { return Ok(None) };
        if !victim.is_alive() {
            return Ok(None);
        }
        if victim.is_significantly_above_terrain() {
            return Ok(Some(false));
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
        .map(|result| result.map(|result| result.destroyed()))
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
    ) -> gamelogic::GameLogicResult<bool> {
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
    ) -> gamelogic::GameLogicResult<bool> {
        // Residual weapons and world effects use the same installed Body owner
        // as ordinary impacts. Finish reentrant bridge, score and onDie callbacks
        // before returning to the caller's original next-candidate boundary.
        Ok(self.apply_owned_damage_with_effects(
            victim_id,
            amount,
            source_id,
            damage_type,
            death_type,
            fx_override,
            &DamageHitContext::default(),
            effects,
        )?
        .is_some_and(|result| result.destroyed))
    }

    /// CPP Object1930–1940: preserve the requested damage/death types and
    /// invalid source, with the kill bit carried by this owned impact context.
    pub(in crate::game_logic) fn apply_owned_kill(
        &mut self,
        victim_id: ObjectId,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
        let Some(victim) = self.objects.get(&victim_id) else { return Ok(None) };
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
        self.apply_owned_healing_with_effects(victim_id, amount, source_id, &mut Default::default())
    }

    pub(crate) fn apply_owned_healing_with_effects(
        &mut self,
        victim_id: ObjectId,
        amount: f32,
        source_id: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
        let result = self.apply_owned_damage_with_killer_team_and_after_fx_with_effects(
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
        )?;
        // C++ ActiveBody::attemptDamage DAMAGE_KILL_GARRISONED kills
        // floor(amount) occupants synchronously inside the damage call
        // (ActiveBody.cpp:421-460); the body only records the count.
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::KillGarrisoned
        ) {
            let source_team = source_id
                .and_then(|id| self.objects.get(&id))
                .map_or(Team::Neutral, |source| source.team);
            self.flush_pending_kill_garrisoned_with_effects(
                victim_id,
                source_id,
                source_team,
                effects,
            )?;
        }
        Ok(result)
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
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
        let Some(previous_source) = self.objects.get(&victim_id).map(|victim| victim.last_damage_source) else { return Ok(None) };
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
            );
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
        )? {
            return Ok(Some(result));
        }
        let frame = self.frame;
        let Some(victim) = self.objects.get(&victim_id) else {
            return Ok(None);
        };
        let before_hp = victim.health.current;
        let can_subdue_active_producer = damage_type.is_subdual()
            && victim.producer_energy_on_disabled_edge().is_some()
            && !victim.has_disabled_type();
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
            let Some(victim) = self.objects.get(&victim_id) else { return Ok(None) };
            let source = victim.owner_player_id.map(|id| id as i32).unwrap_or(-1);
            (
                self.live_draw_playback_identity(victim),
                self.is_loading_map(),
                self.native_lighting_time_of_day(),
                self.receiving_audio_locality_for_player(source),
            )
        };
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment_for(victim_id);
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
                        .reborrow_for_world(&mut self.partition_manager, &self.host_trigger_world),
                    true,
                )
            },
        );
        #[cfg(not(feature = "game_client"))]
        let mut application: DamageApplication = {
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
            let Some(victim) = self.objects.get_mut(&victim_id) else { return Ok(None) };
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
                        .reborrow_for_world(&mut self.partition_manager, &self.host_trigger_world),
                    true,
                )
        };
        if let Some(continuation) = application.take_kill_pilot() {
            if continuation.action == KillPilotAction::Unman {
                let Some(victim) = self.objects.get_mut(&victim_id) else {
                    return Ok(None);
                };
                let birth = std::sync::Arc::downgrade(&victim.visual_draw_identity_token);
                if !victim.is_unmanned() {
                    victim.status.unmanned_owner_team = Some(victim.team);
                    victim.status.unmanned_owner_player_id = victim.owner_player_id;
                }
                // ActiveBody.cpp:400-413. Disable runs its power, radar and
                // car-bomb callbacks before deselection, virtual Idle and Team.
                self.set_owned_unmanned(victim_id, effects)?;
                if !self.object_birth_matches(victim_id, &birth) {
                    return Ok(None);
                }
                self.deselect_drawable(victim_id);
                self.unit_command_ai_idle_owned_with_effects(victim_id, effects)?;
                if !self.object_birth_matches(victim_id, &birth) {
                    return Ok(None);
                }
                self.transfer_unmanned_to_neutral_team(victim_id, effects)?;
                if !self.object_birth_matches(victim_id, &birth) {
                    return Ok(None);
                }
            }
            if let Some(transition) = continuation.transition.as_ref() {
                let Some(victim) = self.objects.get(&victim_id) else {
                    return Ok(None);
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
                )?;
                if !self.objects.get(&victim_id).is_some_and(|object| {
                    self.live_draw_playback_identity(object) == rider_identity
                }) {
                    return Ok(None);
                }
            }

            #[cfg(feature = "game_client")]
            {
                let environment = self.observe_object_fx_environment_for(victim_id);
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
                        victim.finish_kill_pilot_after_transition(
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
                    victim.finish_kill_pilot_after_transition(
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
        let subdued_producer = can_subdue_active_producer && victim.status.disabled_subdued;
        self.admit_object_drawable_deselection(victim_id);
        self.admit_host_radar_disabled_edges(victim_id);
        if subdued_producer {
            self.admit_owned_producer_energy_edge(victim_id, true);
        }
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
            )?
        } else {
            None
        };
        if let Some(ran) = authored_callbacks {
            reaction_callback_ran = ran;
            // Captured callbacks own the actual source-order walk. Do not
            // replay the single-weapon compatibility latch afterward.
            if !healing {
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
                        let _ = self
                            .objects
                            .get_mut(&victim_id)
                            .and_then(|object| object.take_pending_fire_when_damaged_weapon());
                        self.execute_temporary_weapon_on_damage_with_effects(
                            victim_id,
                            actual_damage,
                            damage_type.to_store() as u32,
                            effects,
                        )?;
                    }
                } else if let Some(weapon) = self
                    .objects
                    .get_mut(&victim_id)
                    .and_then(|object| object.take_pending_fire_when_damaged_weapon())
                {
                    reaction_callback_ran = true;
                    self.apply_fire_weapon_when_damaged_named_with_effects(
                        victim_id, &weapon, effects,
                    )?;
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
            return Ok(None);
        }
        let bridge_callbacks = application.bridge_callbacks();
        if let Some(callbacks) = bridge_callbacks {
            self.complete_owned_bridge_body_callbacks_with_effects(callbacks, effects)?;
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
            {
                return Ok(None);
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
        let fx_operation_environment = self.observe_object_fx_environment_for(victim_id);
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
            )?;
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
            {
                return Ok(None);
            }
            let environment = self.observe_object_fx_environment_for(victim_id);
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
            )?;
            if !self
                .objects
                .get(&victim_id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == victim_identity)
            {
                return Ok(None);
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
                // CPP MinefieldBehavior::onDamage runs before ActiveBody's die
                // check and either detonates remaining charges or restores an
                // empty regenerating pad to MIN_HEALTH. The mine sync owner
                // finishes that pad's death after its callback, as Object does.
                before_hp > 0.0
                    && self.objects.get(&victim_id).is_some_and(|o| {
                        o.health.current <= 0.0
                            && !o
                                .mine_data
                                .as_ref()
                                .is_some_and(|md| md.defers_lethal_body_destroy())
                    })
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
                return Ok(None);
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
                    actual_damage_dealt: application.actual_damage_dealt().unwrap_or(hp_lost),
                    actual_damage_clipped: hp_lost,
                },
                context.topple_interpreter(),
                effects,
            )?;
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
        let fx_operation_environment = self.observe_object_fx_environment_for(victim_id);
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
        let fx_operation_environment = self.observe_object_fx_environment_for(victim_id);
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
        Ok(Some(OwnedDamageResult {
            destroyed,
            hp_lost,
            victim_position,
            victim_team,
        }))
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
    ) -> gamelogic::GameLogicResult<Option<OwnedDamageResult>> {
        use crate::game_logic::host_base_defense::{
            HostHiveDamageClass, hive_damage_class_for_type,
        };

        let (template_name, position, team, hive) = {
            let Some(parent) = self.objects.get(&victim_id) else { return Ok(None) };
            (
                parent.template_name.clone(),
                parent.get_position(),
                parent.team,
                parent.is_kind_of(crate::game_logic::KindOf::SpawnsAreTheWeapons),
            )
        };
        // HiveStructureBody delegates to SpawnBehavior only when the receiving
        // object actually has that interface. Name similarity alone is not
        // enough: otherwise this path would swallow damage C++ sends to the
        // ordinary structure body.
        let Some(parent) = self.objects.get(&victim_id) else {
            return Ok(None);
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
            // Programmatic templates have no admitted object definition; they
            // carry the authored KINDOF_SPAWNS_ARE_THE_WEAPONS hive identity.
            // Once a receiving definition exists, its effective Body class is
            // authoritative and an ActiveBody spawner is not a HiveBody.
            .unwrap_or(hive);
        if !authored_hive_body {
            return Ok(None);
        }
        // C++ HiveStructureBody.cpp:55-100: SpawnBehavior interface first,
        // else the Contain module's riders; neither means ordinary Body.
        let spawn_roster = GameLogic::supports_spawn_behavior_child_roster(parent);
        if !spawn_roster
            && parent.thing().template.contain_module.kind
                == crate::game_logic::ContainModuleKind::None
        {
            return Ok(None);
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
                    let data = entry.data.read();
                    let data = data.as_any().downcast_ref::<
                        gamelogic::object::body::hive_structure_body::HiveStructureBodyModuleData,
                    >()?;
                    let kind = damage_type.to_store();
                    Some((
                        data.damage_types_to_propagate_to_slaves.test_damage_type(kind),
                        data.damage_types_to_swallow.test_damage_type(kind),
                    ))
                });
            let Some(data) = data else {
                log::error!(
                    "Admitted HiveStructureBody for '{}' has no typed receiving module data",
                    template_name
                );
                // Keep invalid admission from becoming an ordinary hull hit.
                return Ok(Some(OwnedDamageResult {
                    destroyed: false,
                    hp_lost: 0.0,
                    victim_position: position,
                    victim_team: team,
                }));
            };
            data
        } else {
            let class = hive_damage_class_for_type(damage_type);
            (
                class != HostHiveDamageClass::HitStructure,
                class == HostHiveDamageClass::SwallowIfNoSlaves,
            )
        };
        if !propagate {
            return Ok(None);
        }
        let Some(source) = source_id.and_then(|id| self.objects.get(&id)) else {
            // HiveStructureBody only searches/forwards/swallow when its source
            // Object resolves. Missing source falls through to the structure.
            return Ok(None);
        };
        let source_position = source.get_position();
        let mut closest: Option<(ObjectId, f32)> = None;
        if !spawn_roster {
            // OpenContain::getClosestRider: FROM_CENTER_2D over m_containList,
            // replacing only on a strictly closer rider.
            let riders = self
                .objects
                .get(&victim_id)
                .map(|parent| parent.contained_units())
                .unwrap_or_default();
            for rider_id in riders {
                let Some(rider) = self.objects.get(&rider_id) else {
                    continue;
                };
                let p = rider.get_position();
                let dx = p.x - source_position.x;
                let dz = p.z - source_position.z;
                let distance = dx * dx + dz * dz;
                if closest.is_none_or(|(_, best)| distance < best) {
                    closest = Some((rider_id, distance));
                }
            }
        }
        let children = if spawn_roster {
            self.objects
                .get(&victim_id)
                .map(|parent| parent.spawn_behavior_children())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
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
            let Some(child_before) = self.objects.get(&child_id).map(|child| child.health.current) else { return Ok(None) };
            let Some(child_after_fx) = after_damage_fx.take() else { return Ok(None) };
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
            )?;
            let Some(child_result) = child_result else {
                // The synchronous Body callback re-entered and retired the
                // selected birth. C++ still returns from HiveStructureBody;
                // do not fall through and damage the parent hull.
                return Ok(Some(OwnedDamageResult {
                    destroyed: false,
                    hp_lost: 0.0,
                    victim_position: position,
                    victim_team: team,
                }));
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
                    &mut effects.reborrow_for_world(&mut self.partition_manager, &self.host_trigger_world),
                );
            }
            self.combat_system.queue_under_attack_if_dealt(
                child_id,
                damage_type,
                child_result.hp_lost,
            );
            return Ok(Some(OwnedDamageResult {
                destroyed: false,
                hp_lost: child_result.hp_lost,
                victim_position: position,
                victim_team: team,
            }));
        }
        if swallow {
            let _ = (damage, context);
            return Ok(Some(OwnedDamageResult {
                destroyed: false,
                hp_lost: 0.0,
                victim_position: position,
                victim_team: team,
            }));
        }
        Ok(None)
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
    ) -> gamelogic::GameLogicResult<()> {
        self.materialize_owned_passive_shot_with_effects(
            pending,
            stream_binding,
            None,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn materialize_owned_passive_shot_with_effects(
        &mut self,
        pending: crate::game_logic::combat::PendingProjectile,
        stream_binding: Option<crate::game_logic::combat::ProjectileStreamBinding>,
        accepted_rule: Option<&gamelogic::weapon::WeaponTemplate>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let ready =
            self.materialize_owned_shot_with_effects(pending, stream_binding, accepted_rule, effects)?;
        self.update_projectile_weapon_bonus_upgrades(None);
        if let Some(shot) = ready {
            let limit = self.combat_system.historic_damage_limit();
            let events = self.prepare_owned_projectileless_impact(&shot, limit);
            for event in events {
                self.apply_owned_combat_damage_with_effects(&event, effects)?;
            }
        }
        Ok(())
    }

    /// Receiving-world radius-damage angle for exactly this accepted
    /// projectileless impact; unbound programmatic shots keep the C++
    /// full-circle default.
    fn prepare_owned_projectileless_impact(
        &mut self,
        shot: &crate::game_logic::combat::LiveProjectilelessDelayedDamage,
        historic_limit: u32,
    ) -> Vec<crate::game_logic::combat::DamageEvent> {
        let angle = self
            .weapon_definitions
            .as_ref()
            .and_then(|definitions| definitions.get(&shot.weapon_key().to_ascii_lowercase()))
            .map_or(std::f32::consts::PI, |rule| rule.radius_damage_angle);
        self.combat_system.prepare_projectileless_delayed_shot(
            shot,
            self.objects.read(),
            self.frame,
            historic_limit,
            angle,
        )
    }

    /// Weapon.cpp:1010-1032: the authored LaserName beam is born before the
    /// same shot's Body damage, aimed at the already-resolved scatter aim.
    /// Non-projectile attached targets on the ground get the +10 raise.
    fn emit_accepted_weapon_laser(
        &mut self,
        shot: &crate::game_logic::combat::LiveProjectilelessDelayedDamage,
        target: Option<ObjectId>,
        accepted_rule: Option<&gamelogic::weapon::WeaponTemplate>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let rule = accepted_rule.or_else(|| {
            self.weapon_definitions
                .as_ref()?
                .get(&shot.weapon_key().to_ascii_lowercase())
                .map(AsRef::as_ref)
        });
        let Some(rule) = rule.filter(|rule| rule.is_laser()) else {
            return;
        };
        let (name, bone) = (rule.laser_name.clone(), rule.laser_bone_name.clone());
        let (source, from, mut to) = shot.laser_endpoints();
        if target
            .and_then(|id| self.objects.lookup(&id))
            .is_some_and(|object| {
                !object.is_kind_of(crate::game_logic::KindOf::Projectile)
                    && !object.status.airborne_target
            })
        {
            to.y += 10.0;
        }
        self.weapon_lasers
            .push(crate::game_logic::host_weapon_laser::ResidualWeaponLaser::with_bone(
                name.clone(),
                bone,
                source,
                target,
                (from.x, from.y, from.z),
                (to.x, to.y, to.z),
                self.frame,
            ));
        let _ = self.spawn_weapon_laser_beam_object_with_effects(
            &name, source, target, from, to, effects,
        );
    }

    /// Materialize the batch selected at the existing queue boundary. Shots
    /// accepted reentrantly remain queued for the next boundary.
    pub(in crate::game_logic) fn materialize_owned_queued_projectiles_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let pending = self.combat_system.take_pending_projectiles();
        for shot in pending {
            if let Some(immediate) = self.materialize_owned_shot_with_effects(shot, None, None, effects)? {
                self.combat_system.defer_projectileless_shot(immediate);
            }
        }
        Ok(())
    }

    /// The receiving content currently supports at-destination flight for a
    /// HeightDie emitter. MissileAIUpdate and DumbProjectileBehavior keep
    /// their own flight profiles.
    pub(in crate::game_logic) fn receives_stationary_projectile(&self, name: &str) -> bool {
        let name = name.trim();
        !name.is_empty()
            && !name.eq_ignore_ascii_case("none")
            && self.projectile_definitions.get(name).is_none()
            && self.receiving_object_definitions.as_ref()
                .and_then(|definitions| definitions.get(name))
                .is_some_and(|definition| {
                    let declares = |class: &str| {
                        definition.behavior_modules.iter()
                            .any(|module| module.class_name.eq_ignore_ascii_case(class))
                    };
                    declares("HeightDieUpdate") && declares("NeutronBlastBehavior")
                })
    }

    fn create_accepted_projectile_actor(
        &mut self,
        name: &str,
        team: Team,
        owner: Option<u32>,
        position: Vec3,
        team_id: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<(ObjectId, (u64, u64))> {
        match self.create_projectile_with_birth_checked(name, team, owner, position, team_id, effects) {
            Ok(Some(birth)) => Ok(birth),
            Ok(None) => Err(gamelogic::GameLogicError::Configuration(format!(
                "Could not create projectile {name}"
            ))),
            Err(error) => {
                if let Some((id, birth)) = error.committed
                    && self.spawn_identity_matches(id, birth.0, birth.1)
                {
                    self.destroy_object_with_effects(id, effects)?;
                }
                Err(error.error)
            }
        }
    }

    fn materialize_owned_shot_with_effects(
        &mut self,
        pending: crate::game_logic::combat::PendingProjectile,
        stream_binding: Option<crate::game_logic::combat::ProjectileStreamBinding>,
        accepted_rule: Option<&gamelogic::weapon::WeaponTemplate>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<crate::game_logic::combat::LiveProjectilelessDelayedDamage>> {
        use crate::game_logic::combat::{MaterializedWeaponShot, ProjectilePlacement};
        use crate::game_logic::weapon_bootstrap::HostProjectileFlight;

        let projectile_name = pending.projectile_object_name.clone();
        let profile = self.projectile_definitions.get(&projectile_name);
        let missile = profile.is_some_and(|profile| matches!(profile.flight, HostProjectileFlight::Missile(_)));
        let at_destination = profile.is_none() && self.receives_stationary_projectile(&projectile_name);
        let profile_lifecycle = profile.map(|profile| profile.lifecycle);
        if !missile && matches!(
            pending.projectile_lifecycle,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::Missile { .. })
        ) {
            return Err(gamelogic::GameLogicError::Configuration(format!(
                "Missile {projectile_name} has no receiving flight profile"
            )));
        }
        let source = pending.source_context;
        let source_object = self.objects.get(&pending.shooter_id);
        let team = source.map(|context| context.source_team)
            .or_else(|| source_object.map(|object| object.team))
            .unwrap_or(crate::game_logic::Team::Neutral);
        let owner = match source {
            Some(context) => context.source_owner_player_id,
            None => source_object.and_then(|object| object.owner_player_id),
        };
        let transform = source.and_then(|context| context.launch_transform).or_else(|| {
            source_object.map(|object| {
                let mut matrix = object.get_transform_matrix();
                matrix.w_axis = pending.shooter_pos.extend(1.0);
                matrix
            })
        });
        let velocity = source.map(|context| context.source_velocity)
            .or_else(|| source_object.map(|object| object.movement.velocity))
            .unwrap_or_default();
        let position = transform.map_or(pending.shooter_pos, |matrix| matrix.w_axis.truncate());
        // Weapon fire effects are emitted by the shared materializer even
        // when no aim remains. Do not run newObject/onCreate until the same
        // live-target-or-captured-position test can produce an aim.
        let has_aim = pending.target_pos.is_some()
            || pending.target_id.is_some_and(|id| self.objects.lookup(&id).is_some());
        let detonate_calls_kill = pending.projectile_lifecycle
            .or(profile_lifecycle)
            .is_some_and(crate::game_logic::weapon_bootstrap::HostProjectileLifecycle::detonate_calls_kill);
        let shooter_id = pending.shooter_id;
        let birth = if (missile || detonate_calls_kill) && has_aim {
            if missile && transform.is_none() {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Missile {projectile_name} has no launch transform"
                )));
            }
            Some(self.create_accepted_projectile_actor(
                &projectile_name, team, owner, position,
                source.and_then(|context| context.projectile_team_id), effects,
            )?)
        } else {
            None
        };
        let placement = if at_destination {
            ProjectilePlacement::AtDestination
        } else {
            ProjectilePlacement::Flight { actor_id: birth.map(|(id, _)| id) }
        };
        let accepted = crate::game_logic::combat::materialize_accepted_projectile(
            &mut self.combat_system,
            self.objects.read(),
            self.frame,
            pending,
            stream_binding,
            self.projectile_definitions.get(&projectile_name),
            self.projectile_definitions.geometry(&projectile_name),
            Some(&self.partition_manager),
            &mut self.next_object_id,
            placement,
            &self.logic_random,
        );
        let ready = match accepted {
            Some(MaterializedWeaponShot::ImmediateDamage(damage)) => Some(damage),
            Some(MaterializedWeaponShot::Laser { damage, target }) => {
                // Weapon.cpp:1010-1032 births the beam before Body damage.
                self.emit_accepted_weapon_laser(&damage, target, accepted_rule, effects);
                Some(damage)
            }
            Some(MaterializedWeaponShot::PlaceProjectile(at)) => {
                let (id, birth) = self.create_accepted_projectile_actor(
                    &projectile_name, team, owner, Vec3::ZERO,
                    source.and_then(|context| context.projectile_team_id), effects,
                )?;
                if self.spawn_identity_matches(id, birth.0, birth.1) {
                    let actor = self.objects.get_mut(&id).expect("receiving projectile birth");
                    actor.producer_id = Some(shooter_id);
                    actor.set_position_with_partition_owner(
                        at, self.frame, effects, &mut self.partition_manager,
                    );
                }
                return Ok(None);
            }
            None => None,
        };
        if let Some((id, birth)) = birth {
            let valid = self.spawn_identity_matches(id, birth.0, birth.1);
            if detonate_calls_kill && !missile && valid {
                if let Some(projectile) = self.combat_system.projectile_mut(id) {
                    projectile.projectile_actor_birth = Some(birth);
                    self.objects.get_mut(&id).expect("receiving projectile birth").producer_id = Some(shooter_id);
                    return Ok(ready);
                }
            }
            let motion = if missile && valid {
                match (
                    self.projectile_definitions.get(&projectile_name),
                    transform,
                    self.combat_system.get_projectiles().get(&id),
                    self.objects.get_mut(&id),
                ) {
                    (Some(profile), Some(transform), Some(projectile), Some(actor)) => {
                        crate::game_logic::combat::MissileMotion::launch_pose(
                            transform, projectile.target_position,
                        ).and_then(|pose| {
                            actor.set_transform_matrix_with_partition_owner(
                                pose, self.frame, effects, &mut self.partition_manager,
                            );
                            crate::game_logic::combat::MissileMotion::launch(
                                actor, profile, transform, velocity,
                                projectile.target_position, projectile.speed, self.frame,
                            ).map(|motion| (motion, actor.get_position(), actor.movement.velocity))
                        })
                    }
                    _ => None,
                }
            } else {
                None
            };
            if let Some((motion, position, velocity)) = motion {
                if let Some(projectile) = self.combat_system.projectile_mut(id) {
                    projectile.position = position;
                    projectile.velocity = velocity;
                    projectile.missile_motion = Some(motion);
                    projectile.projectile_actor_birth = Some(birth);
                }
            } else {
                let had_projectile = self.combat_system.remove_projectile(id);
                if self.spawn_identity_matches(id, birth.0, birth.1) {
                    self.destroy_object_with_effects(id, effects)?;
                }
                if had_projectile || !valid {
                    return Err(gamelogic::GameLogicError::Configuration(format!(
                        "Could not launch projectile {projectile_name}"
                    )));
                }
            }
        }
        Ok(ready)
    }

    pub(in crate::game_logic) fn apply_owned_projectileless_delayed_damage(&mut self) -> gamelogic::GameLogicResult<()> {
        self.apply_owned_projectileless_delayed_damage_with_effects(&mut Default::default())
    }

    pub(in crate::game_logic) fn apply_owned_projectileless_delayed_damage_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Weapon.cpp:1589-1604 — the original walks m_weaponDDI by iterator,
        // erasing each due entry only after dealDamageInternal returns. A
        // Body callback (e.g. TransitionDamageFX firing a retaliation weapon)
        // appends to the same installed queue and is still visited by this
        // walk. Entries remaining after an error also stay queued rather
        // than being discarded with an eagerly extracted batch.
        let limit = self.combat_system.historic_damage_limit();
        let mut cursor = 0;
        while let Some(shot) = crate::game_logic::combat::next_ready_projectileless_delayed_damage(
            &mut self.combat_system,
            self.frame,
            &mut cursor,
        ) {
            let events = self.prepare_owned_projectileless_impact(&shot, limit);
            for event in events {
                self.apply_owned_combat_damage_with_effects(&event, effects)?;
            }
        }
        Ok(())
    }

    /// Host-only owner: CombatSystem remains installed through every callback.
    pub(in crate::game_logic) fn update_owned_projectile_impacts(
        &mut self,
        dt: f32,
    ) -> gamelogic::GameLogicResult<Vec<ObjectId>> {
        self.update_owned_projectile_impacts_with_effects(dt, &mut Default::default())
    }

    pub(in crate::game_logic) fn update_owned_projectile_impacts_with_effects(
        &mut self,
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Vec<ObjectId>> {
        let ids = self.combat_system.projectile_update_ids();
        let limit = self.combat_system.historic_damage_limit();
        let mut retired = Vec::new();
        let mut error = None;
        for id in ids {
            let birth = self.combat_system.get_projectiles().get(&id)
                .and_then(|shot| shot.projectile_actor_birth);
            let (retirement, result) = self.resolve_owned_projectile_callback(
                id, crate::game_logic::combat::ProjectileCallback::Update(dt), limit, effects,
            );
            if let Some(actor) = retirement {
                retired.push((id, birth, actor));
            }
            if let Err(callback_error) = result {
                error = Some(callback_error);
                break;
            }
        }
        // Terminal effects can already have run when a callback fails. Finish
        // retirement at the batch boundary so a retry cannot replay them.
        // A callback can replace an ID, so retire only the selected incarnation.
        for &(id, birth, actor) in &retired {
            if let Err(destroy_error) = self.finish_owned_projectile_retirement(id, birth, actor, effects) {
                error.get_or_insert(destroy_error);
            }
        }
        if let Some(error) = error {
            return Err(error);
        }
        Ok(retired.into_iter().map(|(id, _, _)| id).collect())
    }

    fn finish_owned_projectile_retirement(
        &mut self,
        id: ObjectId,
        birth: Option<(u64, u64)>,
        retirement: RetiredProjectileActor,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let current = self.combat_system.get_projectiles().get(&id);
        if current.is_some_and(|shot| shot.projectile_actor_birth == birth) {
            self.combat_system.retire_projectile_impacts(&[id]);
            if matches!(retirement, RetiredProjectileActor::Destroy)
                && let Some((generation, epoch)) = birth
                && self.spawn_identity_matches(id, generation, epoch)
            {
                self.destroy_object_with_effects(id, effects)?;
            }
        }
        Ok(())
    }

    /// PhysicsUpdate.cpp:1141–1149 gives the projectile its contact before bounce.
    pub(in crate::game_logic) fn try_owned_missile_contact(
        &mut self,
        missile: ObjectId,
        other: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(shot) = self.combat_system.get_projectiles().get(&missile) else {
            return Ok(false);
        };
        let Some(birth) = shot.projectile_actor_birth.filter(|_| shot.missile_motion.is_some()) else {
            return Ok(false);
        };
        if !self.spawn_identity_matches(missile, birth.0, birth.1) {
            return Ok(false);
        }
        let objects = self.objects.read();
        let actor = objects.lookup(&missile).expect("validated missile actor");
        if actor.status.no_collisions || !shot.accepts_missile_contact(
            objects.with_players(&self.players).with_team_factory(&self.team_factory),
            other,
            self.frame,
        ) {
            return Ok(true);
        }
        let (retired, result) = self.resolve_owned_projectile_callback(
            missile,
            crate::game_logic::combat::ProjectileCallback::ObjectContact {
                target: other,
                pose: actor.get_position(),
            },
            self.combat_system.historic_damage_limit(),
            effects,
        );
        if let Some(retirement) = retired {
            let cleanup = self.finish_owned_projectile_retirement(missile, Some(birth), retirement, effects);
            result.and(cleanup)?;
        } else {
            result?;
        }
        Ok(true)
    }

    /// One callback without holding the projectile arena across Body or OCL.
    /// The retirement decision is final even if an effect fails.
    fn resolve_owned_projectile_callback(
        &mut self,
        id: ObjectId,
        callback: crate::game_logic::combat::ProjectileCallback,
        historic_damage_limit: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (Option<RetiredProjectileActor>, gamelogic::GameLogicResult<()>) {
        let projectile = self.combat_system.get_projectiles().get(&id);
        let birth = projectile.and_then(|projectile| projectile.projectile_actor_birth);
        let detonate_calls_kill = projectile.is_some_and(|projectile| projectile.detonate_calls_kill());
        let missile_calls_on_die = projectile
            .is_some_and(|p| p.missile_motion.is_some() && p.die_on_detonate);
        let self_damage_on_detonation = detonate_calls_kill || missile_calls_on_die;
        let kill_at_missile_retirement = projectile.is_some_and(|projectile| {
            projectile.missile_motion.is_some()
                && projectile.is_missile_kill_self_holding()
                && self
                    .projectile_definitions
                    .get(&projectile.projectile_object_name)
                    .and_then(|definition| definition.missile_launch)
                    .is_some_and(|policy| policy.detonate_calls_kill)
        });
        if let Some(birth) = birth {
            if !self.spawn_identity_matches(id, birth.0, birth.1) {
                return (Some(RetiredProjectileActor::Destroy), Ok(()));
            }
            if detonate_calls_kill && self.objects[&id].status.effectively_dead {
                return (Some(RetiredProjectileActor::KeepAfterDeath), Ok(()));
            }
        }
        // Weapon.cpp:1504-1508 and 1797-1816 compute the detonation
        // weapon's bonus from the projectile's live conditions OR the
        // launcher conditions captured by its flight module at launch.
        let projectile_bonus = self.combat_system.get_projectiles().get(&id).and_then(|shot| {
            let mask = shot.launcher_bonus_mask | shot.projectile_bonus_mask;
            if shot.computed_bonus_mask == Some(mask) {
                return None;
            }
            let definition = self.weapon_definitions.as_ref()?
                .get(&shot.historic_weapon_key.to_ascii_lowercase())?;
            let flags = gamelogic::weapon::WeaponBonusConditionFlags::from_bits(mask);
            let mut bonus = gamelogic::WeaponBonus::new();
            self.solo_weapon_bonus_set.append_bonuses(flags, &mut bonus);
            if let Some(extra) = definition.extra_bonus.as_ref() {
                extra.append_bonuses(flags, &mut bonus);
            }
            Some((
                mask,
                definition.get_primary_damage(&bonus),
                definition.get_secondary_damage(&bonus),
                definition.get_primary_damage_radius(&bonus),
                definition.get_secondary_damage_radius(&bonus),
            ))
        });
        if let Some((mask, primary, secondary, primary_radius, secondary_radius)) = projectile_bonus {
            if let Some(shot) = self.combat_system.projectile_mut(id) {
                shot.damage = primary;
                shot.secondary_damage = secondary;
                shot.explosion_radius = primary_radius;
                shot.secondary_damage_radius = secondary_radius;
                shot.computed_bonus_mask = Some(mask);
            }
        }
        #[cfg(feature = "game_client")]
        let player = self.local_player_id();
        let terrain = crate::game_logic::game_logic::FlightTerrainView {
            logical: Some(self.world_services.terrain().as_ref()),
            grid: &self.pathfinding_system.grid,
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        #[cfg(feature = "game_client")]
        let (events, retired, detonation_pose) = {
            let shroud = self.world_services.shroud();
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
                        callback,
                        &mut self.objects,
                        Some(&mut self.countermeasures),
                        self.frame,
                        Some(&terrain),
                        Some(&mut self.partition_manager),
                        &mut self.ownership_events,
                        historic_damage_limit,
                        self.receiving_object_definitions.as_ref(),
                        view,
                    )
                },
            )
        };
        #[cfg(not(feature = "game_client"))]
        let (events, retired, detonation_pose) = self.combat_system.prepare_projectile_impact_with_runtime(
            id,
            callback,
            &mut self.objects,
            Some(&mut self.countermeasures),
            self.frame,
            Some(&terrain),
            Some(&mut self.partition_manager),
            &mut self.ownership_events,
            historic_damage_limit,
            self.receiving_object_definitions.as_ref(),
            &mut (),
        );

        if self_damage_on_detonation
            && let Some((generation, epoch)) = birth
            && self.spawn_identity_matches(id, generation, epoch)
        {
            if let Some(projectile) = self.combat_system.get_projectiles().get(&id) {
                let actor = self.objects.get_mut(&id).expect("validated projectile birth");
                let position = detonation_pose.unwrap_or(projectile.position);
                if actor.get_position() != position {
                    actor.set_position_with_partition_owner(
                        position, self.frame, effects, &mut self.partition_manager,
                    );
                }
            }
        }
        let mut retirement = retired.contains(&id).then_some(RetiredProjectileActor::Destroy);
        // Weapon::handleProjectileDetonation runs FX/OCL and Body damage first.
        // The flight behavior then submits DEATH_DETONATED to its own Body.
        let result = self.flush_projectile_impact_fx_with_effects(effects).and_then(|()| {
            for event in events {
                self.apply_owned_combat_damage_with_effects(&event, effects)?;
            }
            if detonation_pose.is_some()
                && let Some((generation, epoch)) = birth
                && self.spawn_identity_matches(id, generation, epoch)
            {
                if detonate_calls_kill {
                    retirement = Some(RetiredProjectileActor::KeepAfterDeath);
                }
                self.apply_projectile_actor_detonation(
                    id, (generation, epoch), self_damage_on_detonation, effects,
                )?;
                if !self.spawn_identity_matches(id, generation, epoch) {
                    retirement = Some(RetiredProjectileActor::KeepAfterDeath);
                }
            }
            // MissileAIUpdate::doKillSelfState — an authored DetonateCallsKill
            // retirement chooses Object::kill (forced NORMAL death) instead of
            // destroy so the actor's Die modules own corpse/removal. This is
            // distinct from the detonation-time DEATH_DETONATED self-hit.
            if kill_at_missile_retirement
                && matches!(retirement, Some(RetiredProjectileActor::Destroy))
                && let Some((generation, epoch)) = birth
                && self.spawn_identity_matches(id, generation, epoch)
            {
                retirement = Some(RetiredProjectileActor::KeepAfterDeath);
                let maximum = self.objects[&id].health.maximum;
                self.apply_owned_damage_with_effects(
                    id,
                    maximum,
                    None,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &DamageHitContext::for_kill(),
                    effects,
                )?;
            }
            Ok(())
        });
        (retirement, result)
    }

    fn apply_projectile_actor_detonation(
        &mut self,
        id: ObjectId,
        birth: (u64, u64),
        self_damage: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_usa_pilot::HostDeathType;

        let result = if self_damage {
            let maximum = self.objects[&id].health.maximum;
            self.apply_owned_damage_with_effects(
                id, maximum, None, DamageType::Unresistable, HostDeathType::Detonated,
                None, &DamageHitContext::default(), effects,
            ).map(|_| ())
        } else {
            Ok(())
        };
        // MissileAIUpdate::detonate hides its Drawable even when the weapon
        // leaves the actor alive for the contrail during KILL_SELF.
        if self.spawn_identity_matches(id, birth.0, birth.1) {
            self.objects.get_mut(&id).expect("receiving projectile birth").set_drawable_hidden(true);
        }
        result
    }

    fn apply_owned_combat_damage_with_effects(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::combat::DamageEvent;
        if let DamageEvent::Direct {
            target_id,
            damage,
            damage_type,
            death_type,
            shooter_id,
            damage_status_type,
            ..
        } = *event
        {
            let context = DamageHitContext::new(self.objects.get(&shooter_id), None, damage_type)
                .with_damage_status(damage_status_type);
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
            )? {
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
                // Intended-only residual (PrimaryDamageRadius 0): count each
                // victim a Helix's Direct shot actually damaged, never asserted
                // at discharge time.
                if result.hp_lost > 0.0
                    && self
                        .objects
                        .get(&shooter_id)
                        .map(|shooter| shooter.is_helix_transport)
                        .unwrap_or(false)
                {
                    self.helix_minigun_residual_units_hit =
                        self.helix_minigun_residual_units_hit.saturating_add(1);
                }
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
            self.apply_owned_area_damage_with_effects(event, effects)?;
        }
        Ok(())
    }
}

impl GameLogic {
    fn apply_owned_area_damage_with_effects(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::combat::{AreaDamage, DamageEvent};
        let DamageEvent::Area {
            shooter_id,
            damage_type,
            death_type,
            radius_weapon_name,
            shooter_owner_player_id,
            damage_status_type,
            ..
        } = event
        else {
            return Ok(());
        };
        // Weapon.cpp:1434 — mask from the live source (projectile controller
        // for moving detonations) before sourceID may become the launcher.
        let source_player_mask = shooter_owner_player_id
            .map(|pid| 1u32 << pid.min(31))
            .unwrap_or(0);
        let (radius_damage_angle, damage_at_self) = if radius_weapon_name.is_empty() {
            (std::f32::consts::PI, false)
        } else if let Some(definitions) = &self.weapon_definitions {
            let Some(weapon) = definitions.get(&radius_weapon_name.to_ascii_lowercase()) else {
                log::error!(
                    "Receiving weapon definition '{}' is missing for directional splash",
                    radius_weapon_name
                );
                return Ok(());
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
                        .with_damage_status(*damage_status_type)
                        .with_shock_wave_force(shock)
                        .with_source_player_mask(source_player_mask);
                self.apply_owned_damage_with_effects(
                    victim_id,
                    damage,
                    Some(*shooter_id),
                    *damage_type,
                    *death_type,
                    None,
                    &context,
                    effects,
                )?
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
                        &mut effects.reborrow_for_world(&mut self.partition_manager, &self.host_trigger_world),
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
        Ok(())
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
