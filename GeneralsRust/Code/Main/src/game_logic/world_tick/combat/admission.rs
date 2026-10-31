//! Combat admission, specialty auto-fire delegation and reload readiness.
use super::*;

impl GameLogic {
    #[cfg(test)]
    pub(super) fn prepare_combat_attack(
        &mut self,
        attacker_id: ObjectId,
    ) -> Option<CombatAttackInput> {
        self.prepare_combat_attack_with_effects(
            attacker_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("valid explicit standalone combat fixture")
    }

    pub(super) fn prepare_combat_attack_with_effects(
        &mut self,
        attacker_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<CombatAttackInput>> {
        Ok({
            // Combat can run before an inactive host AI has cleared its attack
            // state. Apply the same controller-scoped pause at admission so an
            // already-targeted object cannot discharge during that interval.
            if self.skirmish_ai_auto_engage_paused(attacker_id) {
                return Ok(None);
            }
            // Empty-clip RTB is JetAI (guard/hunt interrupt or idle), not every attack.

            // Early gates + docked/garrisoned flags in one immutable scope.
            let (attacker_birth, docked_sortie, docked_passenger, garrisoned) = {
                let Some(attacker) = self.objects.get(&attacker_id) else {
                    return Ok(None);
                };
                // Need at least one weapon slot bound.
                if ![0u8, 1, 2]
                    .into_iter()
                    .any(|slot| attacker.weapon_slot(slot).is_some())
                {
                    return Ok(None);
                }
                // ECM jam residual: C++ canFireWeapon DISABLED_SUBDUED — no fire while jammed.
                if attacker.status.weapons_jammed || attacker.is_disabled() {
                    return Ok(None);
                }
                // Nested AttackStateMachine residual owns aim/fire/approach for these units.
                if attacker.status.is_aiming_weapon
                    || attacker.status.is_firing_weapon
                    || !matches!(
                        attacker.attack_substate(),
                        crate::game_logic::AttackSubState::AimAtTarget
                    )
                {
                    return Ok(None);
                }
                // Interaction orders set `target` without being attacks.
                if matches!(
                    attacker.ai_state(),
                    AIState::Capturing
                        | AIState::SpecialAbility
                        | AIState::Repairing
                        | AIState::Entering
                        | AIState::Docking
                        | AIState::Constructing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::SeekingRepair
                        | AIState::SeekingHealing
                        | AIState::FacingObject
                        | AIState::FacingPosition
                ) {
                    return Ok(None);
                }
                let is_ac = attacker.is_kind_of(KindOf::Aircraft)
                    || attacker.object_type == ObjectType::Aircraft;
                let docked_sortie =
                    attacker.ai_state() == AIState::Docked && is_ac && attacker.target.is_some();
                let docked_passenger = attacker.ai_state() == AIState::Docked && !docked_sortie;
                let garrisoned = attacker.ai_state() == AIState::Garrisoned;
                (
                    std::sync::Arc::downgrade(&attacker.visual_draw_identity_token),
                    docked_sortie,
                    docked_passenger,
                    garrisoned,
                )
            };
            if docked_sortie {
                let _ = self.try_runway_takeoff_from_airfield_with_effects(
                    attacker_id,
                    presentation_effects,
                );
                if !self.objects.get(&attacker_id).is_some_and(|attacker| {
                    std::sync::Weak::ptr_eq(
                        &attacker_birth,
                        &std::sync::Arc::downgrade(&attacker.visual_draw_identity_token),
                    )
                }) {
                    return Ok(None);
                }
            } else if docked_passenger {
                self.try_transport_passenger_residual_fire_with_effects(
                    attacker_id,
                    presentation_effects,
                );
                return Ok(None);
            }
            if garrisoned {
                self.try_garrison_residual_fire_with_effects(attacker_id, presentation_effects);
                return Ok(None);
            }
            let Some(attacker) = self.objects.get(&attacker_id) else {
                return Ok(None);
            };
            // Base-defense residual: Patriot / Gattling (and FSBaseDefense) auto-acquire
            // and fire at nearby enemies without a manual AttackObject order.
            // Respect skirmish AI pause so golden clear is not structure-counterfired.
            {
                let is_defense = crate::game_logic::host_base_defense::is_base_defense_structure(
                    &attacker.template_name,
                    attacker.is_kind_of(KindOf::Structure),
                    attacker.is_kind_of(KindOf::FSBaseDefense),
                );
                let defense_auto_ok = is_defense
                    && attacker.is_constructed()
                    && attacker.can_attack()
                    && matches!(
                        attacker.ai_state(),
                        AIState::Idle | AIState::Attacking | AIState::Patrolling
                    )
                    && !self.skirmish_ai_auto_engage_paused(attacker_id);
                if defense_auto_ok {
                    // Residual owns base-defense fire (nearest-in-range each shot).
                    // Manual AttackObject is not required; structures never chase.
                    self.try_base_defense_residual_fire_with_effects(
                        attacker_id,
                        presentation_effects,
                    )?;
                    return Ok(None);
                }
            }
            // Strategy Center Bombardment turret residual: StrategyCenterGun auto-fire
            // only while Bombardment plan is active (C++ enableTurret residual).
            {
                use crate::game_logic::host_strategy_center::{
                    HostBattlePlan, is_strategy_center_template,
                };
                let is_sc = is_strategy_center_template(&attacker.template_name)
                    || attacker.is_kind_of(KindOf::FSStrategyCenter);
                let sc_auto_ok = is_sc
                    && attacker.is_constructed()
                    && attacker.weapon.is_some()
                    && attacker.can_attack()
                    && matches!(
                        attacker.ai_state(),
                        AIState::Idle | AIState::Attacking | AIState::Patrolling
                    )
                    && !self.skirmish_ai_auto_engage_paused(attacker_id);
                if sc_auto_ok {
                    // Player residual gate: active plan must be Bombardment.
                    let pid = self.player_id_for_team(attacker.team).unwrap_or(0);
                    if self.battle_plans.active_plan_for_player(pid)
                        == Some(HostBattlePlan::Bombardment)
                    {
                        // Strategy's ordinary turret Idle/Hold acquires the goal;
                        // its next AIM update rotates, and FIRE retains the existing
                        // residual discharge authority. Do not advance aim here.
                        return Ok(None);
                    }
                }
            }
            // Sentry Drone residual: with gun upgrade, AutoAcquireEnemiesWhenIdle
            // fires at nearest enemy without manual AttackObject.
            // Fail-closed: not full DeployStyle pack/unpack / turret-only-deployed.
            {
                use crate::game_logic::host_sentry_drone::{
                    is_sentry_drone_template, sentry_auto_fire_eligible,
                };
                let is_sentry = is_sentry_drone_template(&attacker.template_name);
                let idle_ok = matches!(
                    attacker.ai_state(),
                    AIState::Idle | AIState::Attacking | AIState::Patrolling
                );
                let sentry_auto_ok = sentry_auto_fire_eligible(
                is_sentry,
                attacker.weapon.is_some(),
                attacker.is_alive(),
                attacker.can_attack(),
                idle_ok,
            ) && !self.skirmish_ai_auto_engage_paused(attacker_id)
                // Only residual-own auto-fire when no explicit player target.
                && attacker.target.is_none()
                && attacker.target_location.is_none();
                if sentry_auto_ok {
                    self.try_sentry_drone_residual_fire_with_effects(
                        attacker_id,
                        presentation_effects,
                    );
                    return Ok(None);
                }
            }
            // Portable Overlord/Helix gattling: independent auto-acquire (HelixContain.cpp:340).
            if self
                .objects
                .get(&attacker_id)
                .map(|a| a.has_overlord_gattling_residual())
                .unwrap_or(false)
            {
                self.try_overlord_gattling_addon_independent_fire_with_effects(
                    attacker_id,
                    presentation_effects,
                );
            }
            let Some(attacker) = self.objects.get(&attacker_id) else {
                return Ok(None);
            };
            let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

            // TARGET_FAERIE_FIRE residual: painted targets grant 150% ROF readiness.
            let target_has_faerie = attacker
                .target
                .and_then(|tid| self.objects.get(&tid))
                .map(|t| t.is_faerie_fire())
                .unwrap_or(false);

            // Any auto-legal slot ready on reload timer? Button-only
            // AutoChooseSources=NONE secondaries (Jarmen snipe / MD laser)
            // must not keep the cycle alive or the chooser will either
            // auto-fire them or fall through to chase.
            let secondary_explicit = attacker.active_weapon_slot == 1
                || (attacker.weapon_lock_type != WeaponLockType::NotLocked
                    && attacker.weapon_lock_slot == 1);
            let tertiary_explicit = attacker.active_weapon_slot == 2
                || (attacker.weapon_lock_type != WeaponLockType::NotLocked
                    && attacker.weapon_lock_slot == 2);
            let rates = self.receiving_weapon_rates_of_fire(attacker);
            let any_ready = attacker.weapon_ready_vs_target_with_owned_rof(
                0,
                current_time,
                target_has_faerie,
                rates[0],
            ) || ((secondary_explicit
                || attacker.thing().template.slot_allows_auto_choose(1))
                && attacker.weapon_ready_vs_target_with_owned_rof(
                    1,
                    current_time,
                    target_has_faerie,
                    rates[1],
                ))
                || (tertiary_explicit
                    && attacker.weapon_ready_vs_target_with_owned_rof(
                        2,
                        current_time,
                        target_has_faerie,
                        rates[2],
                    ));
            if !any_ready {
                return Ok(None);
            }

            let attacker_team = attacker.team;
            let target_id = attacker.target;
            let target_location = attacker.target_location;
            let overcharge = attacker.overcharge_enabled;
            drop(attacker);
            Some(CombatAttackInput {
                attacker_id,
                current_time,
                attacker_team,
                target_id,
                target_location,
                overcharge,
            })
        })
    }
}
