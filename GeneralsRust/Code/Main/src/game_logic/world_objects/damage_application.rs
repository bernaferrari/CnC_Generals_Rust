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
pub(in crate::game_logic) struct OwnedDamageResult {
    pub(in crate::game_logic) destroyed: bool,
    /// HP delta for this impact, before world callbacks can damage/heal again.
    pub(in crate::game_logic) hp_lost: f32,
    pub(in crate::game_logic) victim_position: glam::Vec3,
    pub(in crate::game_logic) victim_team: Team,
}

impl GameLogic {
    /// CPP Object1930–1940: preserve the requested damage/death types and
    /// invalid source, with the kill bit carried by this owned impact context.
    pub(in crate::game_logic) fn apply_owned_kill(
        &mut self,
        victim_id: ObjectId,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> Option<OwnedDamageResult> {
        let victim = self.objects.get(&victim_id)?;
        let amount = if victim.health.maximum > 0.0 {
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

    pub(in crate::game_logic) fn apply_owned_damage(
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
        let victim = self.objects.get_mut(&victim_id)?;
        let before_hp = victim.health.current;
        let application: DamageApplication = victim.begin_damage_with_context(
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            self.frame,
            context,
        );
        let hp_lost = (before_hp - victim.health.current).max(0.0);
        let victim_position = victim.get_position();
        let victim_team = victim.team;
        if application.killed() {
            // Keep the existing once-only gates and XP-sink routing. Do not
            // predict a promotion or cache XP before the callbacks.
            if let Some(source) = source_id {
                self.award_score_the_kill_experience(source, victim_id);
            }
            let killer_team = killer_team_override
                .or_else(|| source_id.and_then(|id| self.objects.get(&id).map(|obj| obj.team)));
            self.mark_object_for_destruction(victim_id, killer_team);
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
        Some(OwnedDamageResult {
            destroyed: application.finish(victim, source.as_ref()),
            hp_lost,
            victim_position,
            victim_team,
        })
    }
}

impl GameLogic {
    /// Complete one selected queued batch without detaching the CombatSystem.
    /// C++ ActiveBody.cpp:640–653 requires XP/onDie before fresh-source FX.
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
        let (events, retired) = self.combat_system.prepare_projectile_impacts(
            dt,
            &mut self.objects,
            Some(&mut self.countermeasures),
            self.frame,
        );
        for event in events {
            self.apply_owned_combat_damage(&event, ImpactRelationships::MovingProjectile);
        }
        self.combat_system.retire_projectile_impacts(&retired);
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
