//! C++ ActiveBody.cpp:646–653: score, onDie, fresh source lookup, DamageFX.
//! This owner keeps both objects installed and finishes before the next hit.
use super::super::*;
use crate::game_logic::object::{DamageApplication, DamageHitContext};

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

    /// Residual callers retain their original batched death dispatch, but Body
    /// and Player mutations execute immediately on this receiving session.
    pub(in crate::game_logic) fn apply_residual_damage_owned(
        &mut self,
        victim_id: ObjectId,
        amount: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
    ) -> bool {
        let Some(previous_source) = self
            .objects
            .get(&victim_id)
            .map(|object| object.last_damage_source)
        else {
            return false;
        };
        let source_players = [source_id, previous_source].map(|source| {
            source.and_then(|id| {
                self.objects
                    .get(&id)
                    .and_then(|object| object.owner_player_id.map(|player| (id, player)))
            })
        });
        let frame = self.frame;
        let context = DamageHitContext::default()
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
        let players = &mut self.players;
        let victim = self
            .objects
            .get_mut(&victim_id)
            .expect("validated receiving victim");
        let application = victim.begin_damage_with_context_and_player_effects(
            amount,
            source_id,
            damage_type,
            death_type,
            fx_override,
            frame,
            &context,
            &mut self.health_events,
            Some(&mut self.ownership_events),
            |effect| {
                Self::apply_body_player_effect(players, frame, &source_players, effect);
            },
        );
        application.finish_standalone(victim, None, &self.enable_repulsors)
    }

    /// CPP Object1930–1940: preserve the requested damage/death types and
    /// invalid source, with the kill bit carried by this owned impact context.
    pub(in crate::game_logic) fn apply_owned_kill(
        &mut self,
        victim_id: ObjectId,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
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
        self.apply_owned_damage(
            victim_id,
            amount,
            None,
            damage_type,
            death_type,
            None,
            &DamageHitContext::for_kill(),
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
        self.apply_owned_damage_with_killer_team(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            context,
            None,
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
        self.apply_owned_damage_with_killer_team_and_after_fx(
            victim_id,
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            context,
            killer_team_override,
            |_, _| {},
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
        let players = &mut self.players;
        let frame = self.frame;
        let victim = self.objects.get_mut(&victim_id)?;
        let before_hp = victim.health.current;
        let mut application: DamageApplication = victim
            .begin_damage_with_context_and_player_effects(
                damage,
                source_id,
                damage_type,
                death_type,
                fx_override,
                self.frame,
                &context,
                &mut self.health_events,
                Some(&mut self.ownership_events),
                |effect| {
                    Self::apply_body_player_effect(players, frame, &source_players, effect);
                },
            );
        let hp_lost = (before_hp - victim.health.current).max(0.0);
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
        if hp_lost > 0.0
            && damage_type != crate::game_logic::combat::DamageType::Healing
            && !has_installed_reaction_weapons
        {
            if let Some(weapon) = self
                .objects
                .get_mut(&victim_id)
                .and_then(|object| object.take_pending_fire_when_damaged_weapon())
            {
                reaction_callback_ran = true;
                let _ = self.apply_fire_weapon_when_damaged_named(victim_id, &weapon);
            }
        }
        let bridge_callbacks = application.bridge_callbacks();
        if let Some(callbacks) = bridge_callbacks {
            self.complete_owned_bridge_body_callbacks(callbacks);
        }
        let callback_source = application
            .source_id()
            .and_then(|id| self.objects.get(&id))
            .map(crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source);
        if let Some(victim) = self.objects.get_mut(&victim_id) {
            application.complete_body_state_callbacks(victim, callback_source.as_ref());
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
            self.mark_object_for_destruction_with_topple(
                victim_id,
                killer_team,
                crate::game_logic::object::OnDieInput {
                    source: source_id,
                    damage_type,
                    death_type,
                },
                context.topple_interpreter(),
            );
        }
        let source = application
            .source_id()
            .and_then(|id| self.objects.get(&id))
            .map(crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source);
        // mark_object_for_destruction queues final removal; the synchronous
        // onDie boundary must leave the victim installed for its DamageFX.
        let victim = self
            .objects
            .get_mut(&victim_id)
            .expect("damage victim remains installed until destroy-list cleanup");
        let after_fx = application.dispatch_damage_fx(victim, source.as_ref());
        self.admit_object_drawable_deselection(victim_id);
        self.admit_host_radar_disabled_edges(victim_id);
        if after_fx.had_active_body() {
            after_damage_fx(self, victim_id);
        }
        let enable_repulsors = self.enable_repulsors;
        let victim = self
            .objects
            .get_mut(&victim_id)
            .expect("post-FX victim remains installed until destroy-list cleanup");
        let destroyed = after_fx.finish(victim, source.as_ref(), enable_repulsors);
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
        let projectile_name = pending.projectile_object_name.clone();
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
                self.apply_owned_combat_damage(&event, ImpactRelationships::QueuedProjectileless);
            }
        }
    }

    pub(in crate::game_logic) fn apply_owned_projectileless_delayed_damage(&mut self) {
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
                self.apply_owned_combat_damage(&event, ImpactRelationships::QueuedProjectileless);
            }
        }
    }

    /// Host-only owner: CombatSystem remains installed through every callback.
    pub(in crate::game_logic) fn update_owned_projectile_impacts(
        &mut self,
        dt: f32,
    ) -> Vec<ObjectId> {
        let ids = self.combat_system.projectile_update_ids();
        let limit = game_engine::common::global_data::read().historic_damage_limit;
        let mut retired = Vec::new();
        for id in ids {
            let removals = self.update_owned_projectile_impact(id, dt, limit);
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
        let terrain = crate::game_logic::game_logic::FlightTerrainView {
            grid: &self.pathfinding_system.grid,
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
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
            self.apply_owned_combat_damage(&event, ImpactRelationships::MovingProjectile);
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
            if let Some(result) = self.apply_owned_damage(
                target_id,
                damage,
                Some(shooter_id),
                damage_type,
                death_type,
                None,
                &context,
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
            self.apply_owned_area_damage(event, relationships);
        }
    }
}

impl GameLogic {
    fn apply_owned_area_damage(
        &mut self,
        event: &crate::game_logic::combat::DamageEvent,
        relationships: ImpactRelationships,
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
                self.apply_owned_damage(
                    victim_id,
                    damage,
                    Some(*shooter_id),
                    *damage_type,
                    *death_type,
                    None,
                    &context,
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
