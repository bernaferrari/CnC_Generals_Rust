//! C++ ActiveBody.cpp:646–653: score, onDie, fresh source lookup, DamageFX.
//! This owner keeps both objects installed and finishes before the next hit.
use super::super::*;
use crate::game_logic::object::{DamageApplication, DamageHitContext};
#[path = "damage_callbacks.rs"]
mod damage_callbacks;

/// Preserve the two existing admission policies; moving projectile impacts
/// resolve team-instance relationships while queued projectileless hits do not.
#[derive(Clone, Copy)]
enum ImpactRelationships {
    QueuedProjectileless,
    MovingProjectile,
}

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
            );
        let frame = self.frame;
        let before_hp = self.objects.get(&victim_id)?.health.current;
        #[cfg(feature = "game_client")]
        let begin_player = self.local_player_id();
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
        let mut application = Self::with_host_fx_view_parts_and_terrain(
            &mut self.session_view,
            begin_player,
            self.world_services.shroud(),
            effects
                .terrain
                .as_mut()
                .map(|terrain| &mut **terrain as &mut dyn crate::game_logic::NativeTerrainEffects),
            |view| {
                let victim = self
                    .objects
                    .get_mut(&victim_id)
                    .expect("validated receiving victim");
                let players = &mut self.players;
                victim.begin_damage_with_context_and_effects(
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
                            view,
                        );
                    },
                )
            },
        );
        #[cfg(not(feature = "game_client"))]
        let mut application: DamageApplication = {
            let players = &mut self.players;
            self.objects
                .get_mut(&victim_id)?
                .begin_damage_with_context_and_effects(
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
                        Self::execute_body_state_fx_without_view(
                            object,
                            effect,
                            source,
                            &mut self.queued_audio_events,
                        )
                    },
                )
        };
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
                        let _ = self.execute_temporary_weapon_on_damage_with_effects(
                            victim_id,
                            actual_damage,
                            damage_type.to_store() as u32,
                            effects,
                        );
                    }
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
        Self::with_host_fx_view_parts_and_terrain(
            &mut self.session_view,
            callback_player,
            self.world_services.shroud(),
            effects
                .terrain
                .as_mut()
                .map(|terrain| &mut **terrain as &mut dyn crate::game_logic::NativeTerrainEffects),
            |view| {
                if let Some(victim) = self.objects.get_mut(&victim_id) {
                    application.complete_body_state_callbacks_with_body_fx(
                        victim,
                        callback_source.as_ref(),
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
                                view,
                            );
                        },
                    );
                }
            },
        );
        #[cfg(not(feature = "game_client"))]
        if let Some(victim) = self.objects.get_mut(&victim_id) {
            application.complete_body_state_callbacks_with_body_fx(
                victim,
                callback_source.as_ref(),
                &mut |object, effect, source| {
                    Self::execute_body_state_fx_without_view(
                        object,
                        effect,
                        source,
                        &mut self.queued_audio_events,
                    )
                },
            );
        }
        // Original ActiveBody rereads live health after synchronous callbacks.
        let killed = if (bridge_callbacks.is_some() || reaction_callback_ran)
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
        if bridge_callbacks.is_some() || reaction_callback_ran {
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
                    self.award_score_the_kill_experience(source, victim_id);
                }
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
        let after_fx = Self::with_host_fx_view_parts_and_terrain(
            &mut self.session_view,
            armor_player,
            self.world_services.shroud(),
            effects
                .terrain
                .as_mut()
                .map(|terrain| &mut **terrain as &mut dyn crate::game_logic::NativeTerrainEffects),
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
                            view,
                        )
                    },
                )
            },
        );
        #[cfg(not(feature = "game_client"))]
        let after_fx = {
            let victim = self
                .objects
                .get_mut(&victim_id)
                .expect("damage victim remains installed until destroy-list cleanup");
            application.dispatch_damage_fx_with_body_fx(
                victim,
                source.as_ref(),
                &mut |object, effect, source| {
                    Self::execute_body_state_fx_without_view(
                        object,
                        effect,
                        source,
                        &mut self.queued_audio_events,
                    )
                },
            )
        };
        self.admit_object_drawable_deselection(victim_id);
        self.admit_host_radar_disabled_edges(victim_id);
        if after_fx.had_active_body() {
            after_damage_fx(self, victim_id);
        }
        let enable_repulsors = self.enable_repulsors;
        #[cfg(feature = "game_client")]
        let finish_secondary = self.observe_fx_list_die_secondary(victim_id);
        #[cfg(feature = "game_client")]
        let finish_player = self.local_player_id();
        #[cfg(feature = "game_client")]
        let destroyed = Self::with_host_fx_view_parts_and_terrain(
            &mut self.session_view,
            finish_player,
            self.world_services.shroud(),
            effects
                .terrain
                .as_mut()
                .map(|terrain| &mut **terrain as &mut dyn crate::game_logic::NativeTerrainEffects),
            |view| {
                let victim = self
                    .objects
                    .get_mut(&victim_id)
                    .expect("post-FX victim remains installed until destroy-list cleanup");
                after_fx.finish_with_body_fx(
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
                            view,
                        )
                    },
                )
            },
        );
        #[cfg(not(feature = "game_client"))]
        let destroyed = {
            let victim = self
                .objects
                .get_mut(&victim_id)
                .expect("post-FX victim remains installed until destroy-list cleanup");
            after_fx.finish_with_body_fx(
                victim,
                source.as_ref(),
                enable_repulsors,
                &mut |object, effect, source| {
                    Self::execute_body_state_fx_without_view(
                        object,
                        effect,
                        source,
                        &mut self.queued_audio_events,
                    )
                },
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
            &self.objects,
            self.frame,
            pending,
            stream_binding,
            self.projectile_definitions.get(&projectile_name),
            if self.weapon_definitions.is_some() {
                crate::game_logic::projectile_definitions::ProjectileDefinitionMode::ReceivingContent
            } else {
                crate::game_logic::projectile_definitions::ProjectileDefinitionMode::LegacyBootstrap
            },
            Some(&projectile_terrain),
        );
        if let Some(shot) = ready {
            let limit = game_engine::common::global_data::read().historic_damage_limit;
            let events = self.combat_system.prepare_projectileless_delayed_shot(
                &shot,
                &self.objects,
                self.frame,
                limit,
            );
            for event in events {
                self.apply_owned_combat_damage_with_effects(
                    &event,
                    ImpactRelationships::QueuedProjectileless,
                    effects,
                );
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
        let limit = game_engine::common::global_data::read().historic_damage_limit;
        for shot in ready {
            let events = self.combat_system.prepare_projectileless_delayed_shot(
                &shot,
                &self.objects,
                self.frame,
                limit,
            );
            for event in events {
                self.apply_owned_combat_damage_with_effects(
                    &event,
                    ImpactRelationships::QueuedProjectileless,
                    effects,
                );
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
        let limit = game_engine::common::global_data::read().historic_damage_limit;
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
            Self::with_host_fx_view_parts_and_terrain(
                &mut self.session_view,
                player,
                shroud,
                effects.terrain.as_mut().map(|terrain| {
                    &mut **terrain as &mut dyn crate::game_logic::NativeTerrainEffects
                }),
                |view| {
                    self.combat_system.prepare_projectile_impact_with_fx(
                        id,
                        dt,
                        &mut self.objects,
                        Some(&mut self.countermeasures),
                        self.frame,
                        Some(&terrain),
                        &mut self.ownership_events,
                        historic_damage_limit,
                        &mut |name, object| {
                            let pose = match object {
                                crate::game_logic::combat::ProjectileFxObject::Object(object) => {
                                    Self::observe_host_fx_object(object, player, shroud)
                                }
                                crate::game_logic::combat::ProjectileFxObject::Projectile(
                                    projectile,
                                ) => {
                                    use gamelogic::common::types::ObjectShroudStatus;
                                    let is_shrouded = match (player, shroud.lock()) {
                                        (Some(player), Ok(shroud)) => shroud
                                            .get_host_object_shroud_status(player, projectile.id.0)
                                            .is_some_and(|status| {
                                                (status as u8) >= ObjectShroudStatus::Fogged as u8
                                            }),
                                        _ => true,
                                    };
                                    let pos = projectile.position;
                                    let position = gamelogic::common::types::Coord3D {
                                        x: pos.x,
                                        y: pos.z,
                                        z: pos.y,
                                    };
                                    gamelogic::helpers::HostFxObjectPose {
                                        id: projectile.id.0,
                                        position,
                                        transform: glam::Mat4::from_translation(glam::Vec3::new(
                                            pos.x, pos.z, pos.y,
                                        )) * glam::Mat4::from_rotation_z(
                                            projectile.velocity.z.atan2(projectile.velocity.x),
                                        ),
                                        player_index: projectile
                                            .source_owner_player_id
                                            .map(|id| id as i32)
                                            .unwrap_or(-1),
                                        bounding_circle_radius: 0.0,
                                        is_shrouded,
                                    }
                                }
                            };
                            Self::dispatch_fx_list_at_object_with_view(name, &pose, None, view);
                        },
                    )
                },
            )
        };
        #[cfg(not(feature = "game_client"))]
        let (events, retired) = self.combat_system.prepare_projectile_impact(
            id,
            dt,
            &mut self.objects,
            Some(&mut self.countermeasures),
            self.frame,
            Some(&terrain),
            &mut self.ownership_events,
            historic_damage_limit,
        );
        for event in events {
            self.apply_owned_combat_damage_with_effects(
                &event,
                ImpactRelationships::MovingProjectile,
                effects,
            );
        }
        // Return deletion requests; do not physically remove the callback's
        // projectile while other callbacks can still inspect the arena.
        retired
    }

    fn apply_owned_combat_damage(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        relationships: ImpactRelationships,
    ) {
        self.apply_owned_combat_damage_with_effects(event, relationships, &mut Default::default())
    }

    fn apply_owned_combat_damage_with_effects(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        relationships: ImpactRelationships,
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
            self.apply_owned_area_damage_with_effects(event, relationships, effects);
        }
    }
}

impl GameLogic {
    fn apply_owned_area_damage(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        relationships: ImpactRelationships,
    ) {
        self.apply_owned_area_damage_with_effects(event, relationships, &mut Default::default())
    }

    fn apply_owned_area_damage_with_effects(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        relationships: ImpactRelationships,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::{AreaDamage, DamageEvent};
        let DamageEvent::Area {
            shooter_id,
            damage_type,
            death_type,
            ..
        } = event
        else {
            return;
        };
        let area = AreaDamage::new(event, &self.objects).expect("Area event");
        let candidates = area.candidates(&self.objects);
        for (victim_id, distance) in candidates {
            let Some(victim) = self.objects.get(&victim_id) else {
                continue;
            };
            let Some((damage, shock)) = area.impact(
                victim_id,
                victim,
                distance,
                Some(&self.players),
                match relationships {
                    ImpactRelationships::QueuedProjectileless => None,
                    ImpactRelationships::MovingProjectile => Some(&self.team_factory),
                },
            ) else {
                continue;
            };
            let before = victim.health.current;
            let result = if damage > 0.0 {
                let context =
                    DamageHitContext::new(self.objects.get(shooter_id), None, *damage_type);
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
                    let _ = victim.apply_shock_wave_impulse(force);
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
