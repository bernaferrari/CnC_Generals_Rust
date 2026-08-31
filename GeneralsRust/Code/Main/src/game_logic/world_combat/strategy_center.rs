//! Host combat `impl GameLogic` — `strategy_center`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Residual Strategy Center Bombardment turret fire (StrategyCenterGun).
    ///
    /// C++ BattlePlanUpdate::enableTurret(true) residual path:
    /// PrimaryDamage **200** / radius **25**, range **400**, min **100**,
    /// Delay **7000**ms (210 frames). Fail-closed: not full turret recenter /
    /// ScatterRadius / projectile lob matrix.
    pub(in super::super) fn try_strategy_center_bombardment_turret_fire(
        &mut self,
        center_id: ObjectId,
    ) {
        use crate::game_logic::host_strategy_center::{
            STRATEGY_CENTER_GUN_FIRE_AUDIO, STRATEGY_CENTER_GUN_PRIMARY_RADIUS,
            is_legal_strategy_center_gun_target, strategy_center_gun_damage_at,
            strategy_center_gun_in_range,
        };

        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&center_id) else {
            return;
        };
        if !attacker.is_alive()
            || !attacker.is_constructed()
            || attacker.weapon.is_none()
            || !attacker.can_attack()
        {
            return;
        }
        let Some(weapon) = attacker.weapon.as_ref() else {
            return;
        };
        if !Object::weapon_ready(weapon, current_time) {
            return;
        }

        let team = attacker.team;
        let fire_pos = attacker.get_position();
        let range = weapon.range;
        let min_range = weapon.min_range;
        // Ownership: while mood flag is set and mood target is still legal/in-range,
        // prefer that engagee over a full nearest-enemy re-scan (keeps flag vs target
        // in sync). Otherwise fire path owns acquisition and clears mood flag below.
        let mood_prefer = if attacker.turret_mood_target {
            attacker.target
        } else {
            None
        };

        // Mood-prefer residual: keep engagee when still legal/in-range.
        let mut best: Option<(ObjectId, f32, bool)> = None;
        if let Some(mid) = mood_prefer {
            if let Some(obj) = self.objects.get(&mid) {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                let is_air = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                let dist = fire_pos.distance(obj.get_position());
                if is_legal_strategy_center_gun_target(
                    obj.is_alive(),
                    obj.team == team,
                    obj.team == Team::Neutral,
                    obj.status.under_construction,
                    combat_kind,
                    is_air,
                ) && !(obj.is_effectively_stealthed() && obj.team != team)
                    && !(obj.is_eject_invulnerable() && obj.team != team)
                    && strategy_center_gun_in_range(dist)
                    && dist >= min_range
                    && dist <= range
                {
                    best = Some((mid, dist, is_air));
                }
            }
        }
        if best.is_none() {
            // Pure residual acquire query (fire decision choice phase).
            let candidates: Vec<_> = self
                .objects
                .iter()
                .map(|(&id, obj)| {
                    let combat_kind =
                        crate::game_logic::host_residual_acquire::residual_combat_kind(
                            obj.is_kind_of(KindOf::Attackable),
                            obj.is_kind_of(KindOf::Structure),
                            obj.is_kind_of(KindOf::Infantry),
                            obj.is_kind_of(KindOf::Vehicle),
                            obj.is_kind_of(KindOf::Aircraft),
                        );
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: obj.is_alive(),
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: obj.status.under_construction,
                        combat_kind,
                        effectively_stealthed: obj.is_effectively_stealthed(),
                        is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                        eject_invulnerable: obj.is_eject_invulnerable(),
                    }
                })
                .collect();
            best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
                center_id,
                team,
                fire_pos,
                candidates,
                |_| range,
                |c| {
                    let dist = fire_pos.distance(c.position);
                    is_legal_strategy_center_gun_target(
                        c.is_alive,
                        c.team == team,
                        c.is_neutral,
                        c.under_construction,
                        c.combat_kind,
                        c.is_air,
                    ) && !(c.eject_invulnerable && c.team != team)
                        && strategy_center_gun_in_range(dist)
                        && dist >= min_range
                },
            )
            .map(|(id, dist, air)| (id, dist, air));
        }

        let Some((target_id, _, _)) = best else {
            return;
        };

        // C++ TurretAIAimTurretState: FIRE only after yaw+pitch align.
        // InitiallyDisabled Strategy Center cannot fire until Bombardment
        // enableTurret(true).
        if self
            .objects
            .get(&center_id)
            .map(|o| !o.turret_enabled)
            .unwrap_or(true)
        {
            return;
        }
        self.set_turret_target_object(center_id, Some(target_id), false);
        if !matches!(
            self.tick_turret_aim(center_id, 1.0),
            AttackAimResult::Success
        ) {
            return;
        }

        let impact = self
            .objects
            .get(&target_id)
            .map(|t| t.get_position())
            .unwrap_or(fire_pos);
        use crate::game_logic::host_strategy_center::{
            strategy_center_gun_scatter_aim, strategy_center_gun_scatter_misses,
        };
        let intended_is_infantry = self
            .objects
            .get(&target_id)
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            center_id.0,
            target_id.0,
            self.frame,
        );
        let hit_r = self
            .objects
            .get(&target_id)
            .map(|o| {
                if o.selection_radius > 0.0 {
                    o.selection_radius
                } else {
                    crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                }
            })
            .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
        let (impact, scattered) =
            strategy_center_gun_scatter_aim(impact, intended_is_infantry, seed);
        if scattered {
            self.strategy_center_gun_scatter_applied =
                self.strategy_center_gun_scatter_applied.saturating_add(1);
        }
        let mut intended_scatter_miss = false;
        if strategy_center_gun_scatter_misses(seed, hit_r, intended_is_infantry) {
            let intended_pos = self.objects.get(&target_id).map(|o| o.get_position());
            if let Some(pos) = intended_pos {
                let dx = impact.x - pos.x;
                let dz = impact.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist > STRATEGY_CENTER_GUN_PRIMARY_RADIUS {
                    self.strategy_center_gun_scatter_misses =
                        self.strategy_center_gun_scatter_misses.saturating_add(1);
                    intended_scatter_miss = true;
                }
            }
        }

        // Splash residual: intended + PrimaryDamageRadius ring (no force-hit after scatter miss).
        let impact_xz = (impact.x, impact.z);
        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if *id == center_id {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                let is_air = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                if !is_legal_strategy_center_gun_target(
                    obj.is_alive(),
                    obj.team == team,
                    obj.team == Team::Neutral,
                    obj.status.under_construction,
                    combat_kind,
                    is_air,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = *id == target_id;
                if is_intended && intended_scatter_miss {
                    if dist > STRATEGY_CENTER_GUN_PRIMARY_RADIUS {
                        return None;
                    }
                    return Some((*id, dist, false));
                }
                if is_intended || dist <= STRATEGY_CENTER_GUN_PRIMARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        let mut hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let weapon_snap = self.objects.get(&center_id).and_then(|a| a.weapon.clone());
        for (id, dist, is_intended) in candidates {
            let dmg = strategy_center_gun_damage_at(if is_intended { 0.0 } else { dist });
            if dmg <= 0.0 {
                continue;
            }
            if self
                .objects
                .get(&id)
                .map(|o| o.is_eject_invulnerable())
                .unwrap_or(false)
            {
                // InvulnerableTime residual blocks damage.
                self.usa_pilot.record_invulnerable_block();
                continue;
            }
            let (destroyed, _) = self.residual_auto_fire_apply_damage(
                center_id,
                id,
                dmg,
                fire_pos,
                weapon_snap.as_ref(),
                0,
            );
            hits = hits.saturating_add(1);
            if destroyed {
                destroy_ids.push((id, Some(team)));
            }
        }

        if let Some(attacker) = self.objects.get_mut(&center_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                0,
                self.frame,
                Some(target_id),
                None,
            );
            if let Some(w) = attacker.weapon.as_mut() {
                // Clip/ammo residual parity with fire_at path (not last_fire-only stamp).
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = attacker
                    .weapon
                    .as_ref()
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    0,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            // Mood ownership residual: keep mood flag only when fire engages the
            // same mood-acquired target; otherwise fire owns acquisition (clear flag).
            if attacker.turret_mood_target && attacker.target != Some(target_id) {
                attacker.turret_mood_target = false;
            } else if !attacker.turret_mood_target {
                // Non-mood fire residual: ensure flag stays clear.
                attacker.turret_mood_target = false;
            }
            attacker.set_target(Some(target_id));
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(center_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(center_id, 2);
            }
            // Turret already aligned by tick_turret_aim (no snap).
            attacker.turret_idle_scanning = false;
            attacker.record_host_turret();
            attacker.turret_holding = false;
            attacker.record_host_turret();
            attacker.turret_hold_until_frame = 0;
            attacker.turret_idle_recentering = false;
            attacker.turret_idle_scan_next_frame = self.frame.saturating_add(
                crate::game_logic::host_strategy_center::BATTLE_PLAN_TURRET_RECENTER_FRAMES,
            );
        }
        self.notify_turret_fired(center_id);
        // One turret discharge may splash several victims. Normalize its
        // concrete PRIMARY cursor once here, never inside the per-victim
        // residual damage helper.
        let _ = self.record_accepted_weapon_discharge(center_id, 0);

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.battle_plans.record_turret_fire(hits);

        let muzzle_pos = self
            .objects
            .get(&center_id)
            .map(|a| a.get_position())
            .unwrap_or(fire_pos);
        let _ = self.combat_particles.spawn_weapon_fire_fx(
            muzzle_pos,
            Some(impact),
            self.frame,
            center_id,
            Some(target_id),
        );
        self.queue_audio_event(
            AudioEventRequest::new(STRATEGY_CENTER_GUN_FIRE_AUDIO)
                .with_object(center_id)
                .with_position(muzzle_pos)
                .with_priority(165),
        );
    }

    /// Residual base-defense auto-fire: Patriot / Gattling / FSBaseDefense
    /// structures acquire nearest enemy in weapon range and deal damage without
    /// a manual AttackObject order.
    ///
    /// China Gattling Cannon residual adds:
    /// - dual-slot air/ground chooser (`GattlingBuildingGun` / `GunAir`)
    /// - continuous-fire ramp (One=1 / Two=5 / Coast=2000ms)
    /// - Chain Guns PLAYER_UPGRADE damage × 1.25
    ///
    /// Fail-closed: not full AutoAcquire LOS / turret pitch / CONTINUOUS_FIRE anim.
    pub(in super::super) fn try_base_defense_residual_fire(&mut self, defense_id: ObjectId) {
        use crate::game_logic::host_base_defense::{
            GATTLING_BUILDING_FIRE_AUDIO, PATRIOT_FIRE_AUDIO, STINGER_FIRE_AUDIO,
            is_dual_slot_base_defense, is_gattling_cannon_structure, is_patriot_battery_structure,
            is_stinger_site_structure, preferred_dual_defense_slot,
        };

        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

        let Some(attacker) = self.objects.get(&defense_id) else {
            return;
        };
        if !attacker.is_alive()
            || !attacker.is_constructed()
            || attacker.weapon.is_none()
            || !attacker.can_attack()
        {
            return;
        }
        let template_name = attacker.template_name.clone();
        let is_gattling = is_gattling_cannon_structure(&template_name);
        let is_stinger = is_stinger_site_structure(&template_name);
        let is_patriot = is_patriot_battery_structure(&template_name);
        let dual_slot = is_dual_slot_base_defense(&template_name);
        // SPAWNS_ARE_THE_WEAPONS residual: Stinger Site cannot fire with 0 soldiers.
        if is_stinger
            && !crate::game_logic::host_base_defense::stinger_can_fire_with_slaves(
                attacker.hive_slave_count,
            )
        {
            return;
        }
        let team = attacker.team;
        let fire_pos = attacker.get_position();
        let ground_range = attacker.weapon.as_ref().map(|w| w.range).unwrap_or(0.0);
        let air_range = attacker
            .secondary_weapon
            .as_ref()
            .map(|w| w.range)
            .unwrap_or(0.0);
        // Scan range residual: dual-slot defenses use max(primary, secondary) so AA
        // can acquire out to air range while ground stays at primary range.
        let scan_range = if dual_slot {
            ground_range.max(air_range)
        } else {
            ground_range
        };
        if scan_range <= 0.0 {
            return;
        }

        // Pure residual acquire query (fire decision choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .map(|(&id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind,
                    effectively_stealthed: obj.is_effectively_stealthed(),
                    is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                    eject_invulnerable: obj.is_eject_invulnerable(),
                }
            })
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            defense_id,
            team,
            fire_pos,
            candidates,
            |is_air| {
                if dual_slot {
                    if is_air {
                        air_range.max(ground_range)
                    } else {
                        ground_range
                    }
                } else {
                    scan_range
                }
            },
            |c| {
                crate::game_logic::host_base_defense::is_legal_base_defense_target(
                    c.is_alive,
                    c.team == team,
                    c.is_neutral,
                    c.under_construction,
                    c.combat_kind,
                )
            },
        );

        let Some((target_id, _, target_is_air)) = best else {
            return;
        };
        let slot = if dual_slot {
            preferred_dual_defense_slot(target_is_air)
        } else {
            0
        };

        // C++ WeaponSet auto-choose residual: a dual-slot base defense engages
        // through the slot that owns the acquired victim (AA secondary for
        // airborne, primary otherwise). tick_turret_aim's sweep/pitch/range
        // gates all read the selected slot, so the choose must precede aim.
        if dual_slot {
            if let Some(attacker) = self.objects.get_mut(&defense_id) {
                attacker.set_active_weapon_slot(slot);
            }
        }

        // Readiness residual: use the slot that will fire.
        let ready = {
            let Some(attacker) = self.objects.get(&defense_id) else {
                return;
            };
            if slot == 1 {
                attacker
                    .secondary_weapon
                    .as_ref()
                    .is_some_and(|w| Object::weapon_ready(w, current_time))
            } else {
                attacker
                    .weapon
                    .as_ref()
                    .is_some_and(|w| Object::weapon_ready(w, current_time))
            }
        };
        // C++ TurretAIAimTurretState: do not discharge while the barrel is
        // still traversing. Structures without a Turret block fire immediately.
        let turret_enabled = self
            .objects
            .get(&defense_id)
            .map(|o| o.turret_enabled)
            .unwrap_or(false);
        if turret_enabled {
            self.set_turret_target_object(defense_id, Some(target_id), false);
            let aim_result = self.tick_turret_aim(defense_id, 1.0);
            if !matches!(aim_result, AttackAimResult::Success) {
                return;
            }
        }
        let damage = {
            let Some(attacker) = self.objects.get(&defense_id) else {
                return;
            };
            if slot == 1 {
                attacker
                    .secondary_weapon
                    .as_ref()
                    .map(|w| w.damage)
                    .unwrap_or(0.0)
            } else {
                attacker.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0)
            }
        };
        if damage <= 0.0 {
            return;
        }

        // America Fire Base residual: dual-radius howitzer splash owns damage.
        // ScatterRadiusVsInfantry residual closed via shell aim offset / instant impact.
        // Fail-closed: not full ScaleWeaponSpeed lob matrix.
        let is_fire_base = crate::game_logic::host_fire_base::is_fire_base_template(&template_name);
        let mut destroyed = false;
        if is_fire_base {
            let impact = self
                .objects
                .get(&target_id)
                .map(|t| t.get_position())
                .unwrap_or(fire_pos);
            let from = self
                .objects
                .get(&defense_id)
                .map(|d| d.get_position())
                .unwrap_or(fire_pos);
            let spawned = self
                .spawn_fire_base_shell_projectile(defense_id, from, impact, Some(target_id))
                .is_some();
            let (hits, any_destroyed) = if spawned {
                self.fire_base_residual_fires = self.fire_base_residual_fires.saturating_add(1);
                (1, false)
            } else {
                self.apply_fire_base_residual_at(impact, Some(defense_id), Some(target_id))
            };
            destroyed = any_destroyed;
            let _ = hits;
        } else {
            let weapon_snap = self.objects.get(&defense_id).and_then(|a| {
                if slot == 1 {
                    a.secondary_weapon.clone().or_else(|| a.weapon.clone())
                } else {
                    a.weapon.clone()
                }
            });
            // C++ ScatterRadiusVsInfantry residual: Patriot/Stinger ground fire vs infantry may miss.
            let mut skip_damage = false;
            if (is_patriot || is_stinger) && !target_is_air {
                let target_is_infantry = self
                    .objects
                    .get(&target_id)
                    .map(|o| o.is_kind_of(KindOf::Infantry))
                    .unwrap_or(false);
                if target_is_infantry {
                    let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                        defense_id.0,
                        target_id.0,
                        self.frame,
                    );
                    let hit_r = self
                        .objects
                        .get(&target_id)
                        .map(|o| {
                            if o.selection_radius > 0.0 {
                                o.selection_radius
                            } else {
                                crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                            }
                        })
                        .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
                    if is_patriot {
                        self.patriot_scatter_applied =
                            self.patriot_scatter_applied.saturating_add(1);
                        if crate::game_logic::host_base_defense::patriot_scatter_misses_infantry(
                            true, seed, hit_r,
                        ) {
                            self.patriot_scatter_misses =
                                self.patriot_scatter_misses.saturating_add(1);
                            skip_damage = true;
                        }
                    } else if is_stinger {
                        self.stinger_scatter_applied =
                            self.stinger_scatter_applied.saturating_add(1);
                        if crate::game_logic::host_base_defense::stinger_scatter_misses_infantry(
                            true, seed, hit_r,
                        ) {
                            self.stinger_scatter_misses =
                                self.stinger_scatter_misses.saturating_add(1);
                            skip_damage = true;
                        }
                    }
                }
            }
            if !skip_damage {
                let (d, xp) = self.residual_auto_fire_apply_damage(
                    defense_id,
                    target_id,
                    damage,
                    fire_pos,
                    weapon_snap.as_ref(),
                    slot,
                );
                destroyed = d;
                let _ = xp;
            }
        }
        if let Some(attacker) = self.objects.get_mut(&defense_id) {
            let _ = attacker.capture_pending_weapon_visual_dispatch(
                slot,
                self.frame,
                Some(target_id),
                None,
            );
            if slot == 1 {
                if let Some(w) = attacker.secondary_weapon.as_mut() {
                    crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
                }
            } else if let Some(w) = attacker.weapon.as_mut() {
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
            // AI attack authority: residual fire-intent for GameWorld last-writer.
            if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                let (dmg, rng) = if slot == 1 {
                    attacker
                        .secondary_weapon
                        .as_ref()
                        .or(attacker.weapon.as_ref())
                        .map(|w| (w.damage, w.range))
                        .unwrap_or((0.0, 0.0))
                } else {
                    attacker
                        .weapon
                        .as_ref()
                        .map(|w| (w.damage, w.range))
                        .unwrap_or((0.0, 0.0))
                };
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = attacker.fire_intent_count.saturating_add(1);
                crate::game_logic::host_fire_intent_log::record(
                    attacker.id,
                    target_id.0,
                    slot,
                    dmg,
                    rng,
                    current_time,
                    frame,
                    next_count,
                );
                attacker.fire_intent_count = next_count;
            }
            // Track engagement for UI / subsequent frames without requiring
            // a player AttackObject. Structures stay immobile (no chase).
            attacker.set_target(Some(target_id));
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(defense_id, target_id);
                crate::game_logic::host_ai_decision_log::record_set_state(defense_id, 2);
            }
            if destroyed {
                self.stop_attack_decision_aware(defense_id);
            }
        }
        if turret_enabled {
            self.notify_turret_fired(defense_id);
        }
        // This accepted base-defense shot can miss or splash; it is still one
        // physical slot discharge and must advance/freeze exactly once.
        let _ = self.record_accepted_weapon_discharge(defense_id, slot);

        if destroyed {
            self.award_score_the_kill_experience(defense_id, target_id);
            if !is_fire_base {
                // Fire Base residual already mark_object_for_destruction inside apply.
                self.mark_object_for_destruction(target_id, Some(team));
            }
        }

        // Continuous-fire ramp residual for structure gattling.
        if is_gattling {
            self.advance_gattling_building_continuous_fire(defense_id, Some(target_id), slot);
        }

        // Muzzle + audio residual (shared combat honesty).
        let muzzle_pos = self
            .objects
            .get(&defense_id)
            .map(|a| a.get_position())
            .unwrap_or(fire_pos);
        let impact_pos = self.objects.get(&target_id).map(|t| t.get_position());
        let _ = self.combat_particles.spawn_weapon_fire_fx(
            muzzle_pos,
            impact_pos,
            self.frame,
            defense_id,
            Some(target_id),
        );
        let is_tunnel =
            crate::game_logic::host_tunnel_network::is_tunnel_network_template(&template_name);
        let is_laser_patriot =
            crate::game_logic::host_base_defense::is_laser_patriot_template(&template_name);
        let audio = if is_gattling {
            GATTLING_BUILDING_FIRE_AUDIO
        } else if is_stinger {
            STINGER_FIRE_AUDIO
        } else if is_patriot && is_laser_patriot {
            crate::game_logic::host_base_defense::LAZR_PATRIOT_FIRE_AUDIO
        } else if is_patriot {
            PATRIOT_FIRE_AUDIO
        } else if is_fire_base {
            crate::game_logic::host_fire_base::FIRE_BASE_FIRE_AUDIO
        } else if is_tunnel {
            crate::game_logic::host_tunnel_network::TUNNEL_NETWORK_GUN_AUDIO
        } else {
            "WeaponFire"
        };
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_object(defense_id)
                .with_position(muzzle_pos)
                .with_priority(160),
        );

        self.base_defense_residual_fires = self.base_defense_residual_fires.saturating_add(1);

        // Per-structure residual honesty counters.
        if is_stinger {
            if slot == 1 {
                self.stinger_site_residual_aa_fires =
                    self.stinger_site_residual_aa_fires.saturating_add(1);
            } else {
                self.stinger_site_residual_ground_fires =
                    self.stinger_site_residual_ground_fires.saturating_add(1);
            }
            // Physical soldier attach residual: orderSlavesToAttackTarget.
            // Fail-closed: not full GLAInfantryStingerSoldier AI module.
            use crate::game_logic::host_base_defense::order_hive_slaves_to_attack_target;
            if let Some(site) = self.objects.get_mut(&defense_id) {
                let n = order_hive_slaves_to_attack_target(&mut site.hive_slaves, target_id.0);
                if n > 0 {
                    self.stinger_slave_order_attack_count =
                        self.stinger_slave_order_attack_count.saturating_add(n);
                }
            }
        }
        if is_patriot {
            if slot == 1 {
                self.patriot_residual_aa_fires = self.patriot_residual_aa_fires.saturating_add(1);
            } else {
                self.patriot_residual_ground_fires =
                    self.patriot_residual_ground_fires.saturating_add(1);
            }
            // Superweapon General EMP Patriot: EMPPatriotEffectSpheroid + EMPSparks.
            if crate::game_logic::host_base_defense::is_supw_patriot_template(&template_name) {
                self.apply_supw_patriot_emp_residual_at(
                    impact_pos.unwrap_or(fire_pos),
                    defense_id,
                    team,
                    Some(target_id),
                );
            }
            // AssistedTargetingUpdate residual: RequestAssistRange → neighboring
            // equivalent Patriots fire AssistingClipSize assist-weapon shots +
            // BinaryDataStream LaserFromAssisted / LaserToTarget residual beams.
            // Fail-closed: not full W3DLaserDraw texture/arc GPU draw
            // (endpoint track + draw-param honesty residual closed 2026-07-13).
            if !destroyed {
                self.process_patriot_assist_request(defense_id, target_id, slot);
            }
        }
        if is_tunnel {
            // TunnelNetworkGun residual honesty (base-defense auto-fire path).
            self.tunnel_network.record_gun_fire(true);
        }
    }

    /// C++ Weapon::processRequestAssistance residual for leftover RequestAssistRange.
    ///
    /// Same-team equivalent Patriots within leftover `getRequestAssistRange()` that
    /// are free to assist accept a clip of **4** assist-weapon shots (range **450**).
    /// Range ≤ 0 skips the request (C++ `if (getRequestAssistRange() && victimObj)`).
    pub(in super::super) fn process_patriot_assist_request(
        &mut self,
        requester_id: ObjectId,
        victim_id: ObjectId,
        slot: u8,
    ) {
        use crate::game_logic::host_base_defense::{
            PATRIOT_ASSIST_LASER_AUDIO, PatriotAssistLaserKind, PendingPatriotAssist,
            is_patriot_battery_structure, is_patriot_free_to_assist,
            is_within_patriot_assist_weapon_range, is_within_patriot_request_assist_range,
            make_patriot_assist_lasers, patriot_request_assist_range_for_template,
            patriots_are_assist_equivalent,
        };

        let Some(requester) = self.objects.get(&requester_id) else {
            return;
        };
        if !is_patriot_battery_structure(&requester.template_name) {
            return;
        }
        let requester_team = requester.team;
        let requester_template = requester.template_name.clone();
        let requester_pos = requester.get_position();

        let Some(victim) = self.objects.get(&victim_id) else {
            return;
        };
        if !victim.is_alive() {
            return;
        }
        let victim_pos = victim.get_position();

        // C++ Weapon.cpp:2477 — only fan out when leftover RequestAssistRange > 0.
        let request_range =
            patriot_request_assist_range_for_template(&requester_template, slot == 1);
        if request_range <= 0.0 {
            return;
        }

        self.patriot_assist_residual_requests =
            self.patriot_assist_residual_requests.saturating_add(1);

        // Pure residual acquire: all free equivalent Patriots in leftover
        // RequestAssistRange that can still weapon-range the victim (nearest-first).
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
        let cand_snap: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if id == requester_id || obj.team != requester_team {
                    return None;
                }
                if !is_patriot_battery_structure(&obj.template_name) {
                    return None;
                }
                if !patriots_are_assist_equivalent(&requester_template, &obj.template_name) {
                    return None;
                }
                let already_assisting = self
                    .pending_patriot_assists
                    .iter()
                    .any(|p| p.assistant_id == id && p.shots_remaining > 0);
                let weapon_ready = obj
                    .weapon
                    .as_ref()
                    .is_some_and(|w| Object::weapon_ready(w, current_time));
                if !is_patriot_free_to_assist(
                    obj.is_alive(),
                    obj.is_constructed(),
                    obj.can_attack(),
                    obj.status.under_construction,
                    already_assisting,
                    weapon_ready,
                ) {
                    return None;
                }
                let assistant_pos = obj.get_position();
                let vdist = {
                    let dx = assistant_pos.x - victim_pos.x;
                    let dz = assistant_pos.z - victim_pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                if !is_within_patriot_assist_weapon_range(vdist) {
                    return None;
                }
                Some((
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: assistant_pos,
                        is_alive: true,
                        is_neutral: false,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                    obj.template_name.clone(),
                    assistant_pos,
                ))
            })
            .collect();
        let filtered = crate::game_logic::host_residual_acquire::filter_residual_targets_xz(
            Some(requester_id),
            (requester_pos.x, requester_pos.z),
            request_range,
            cand_snap.iter().map(|(c, _, _)| c.clone()),
            |c| {
                // Distance already gated by filter; keep request-assist helper for parity.
                let dist = {
                    let dx = c.position.x - requester_pos.x;
                    let dz = c.position.z - requester_pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                is_within_patriot_request_assist_range(dist, request_range)
            },
        );
        let candidates: Vec<(ObjectId, String, Vec3)> = filtered
            .into_iter()
            .filter_map(|(id, _, _)| {
                cand_snap
                    .iter()
                    .find(|(c, _, _)| c.id == id)
                    .map(|(_, tmpl, pos)| (id, tmpl.clone(), *pos))
            })
            .collect();

        for (assistant_id, assistant_template, assistant_pos) in candidates {
            self.pending_patriot_assists.push(PendingPatriotAssist::new(
                assistant_id,
                victim_id,
                requester_id,
                self.frame,
                assistant_template,
            ));
            self.patriot_assist_residual_accepts =
                self.patriot_assist_residual_accepts.saturating_add(1);
            // BinaryDataStream laser residual: LaserFromAssisted + LaserToTarget
            // (retail makeFeedbackLaser pair; DeletionUpdate 600ms lifetime).
            // Fail-closed: not full W3DLaserDraw texture/arc GPU draw
            // (endpoint track + draw-param honesty residual closed 2026-07-13).
            let beams = make_patriot_assist_lasers(
                requester_id,
                assistant_id,
                victim_id,
                (requester_pos.x, requester_pos.y, requester_pos.z),
                (assistant_pos.x, assistant_pos.y, assistant_pos.z),
                (victim_pos.x, victim_pos.y, victim_pos.z),
                self.frame,
            );
            for beam in beams {
                match beam.kind {
                    PatriotAssistLaserKind::FromAssisted => {
                        self.patriot_assist_laser_from_assisted =
                            self.patriot_assist_laser_from_assisted.saturating_add(1);
                    }
                    PatriotAssistLaserKind::ToTarget => {
                        self.patriot_assist_laser_to_target =
                            self.patriot_assist_laser_to_target.saturating_add(1);
                    }
                }
                self.patriot_assist_lasers.push(beam);
            }
            // Residual BinaryDataStream laser honesty audio cue.
            self.queue_audio_event(
                AudioEventRequest::new(PATRIOT_ASSIST_LASER_AUDIO)
                    .with_object(assistant_id)
                    .with_position(assistant_pos)
                    .with_priority(140),
            );
        }
    }

    /// Advance residual Patriot BinaryDataStream lasers:
    /// - LaserUpdate endpoint track residual (parent/target positions)
    /// - W3DLaserDraw ScrollRate residual
    /// - DeletionUpdate lifetime expiry
    pub(crate) fn tick_patriot_assist_lasers_sole(&mut self) {
        self.update_patriot_assist_lasers();
    }

    pub(in super::super) fn update_patriot_assist_lasers(&mut self) {
        use crate::game_logic::host_base_defense::{
            expire_patriot_assist_lasers, track_patriot_assist_laser_endpoints,
        };

        // Snapshot live positions for LaserUpdate residual (avoid borrow conflicts).
        let positions: Vec<(ObjectId, f32, f32, f32, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| {
                let p = obj.get_position();
                (*id, p.x, p.y, p.z, obj.is_alive())
            })
            .collect();
        let lookup = |id: ObjectId| {
            positions
                .iter()
                .find(|(oid, _, _, _, _)| *oid == id)
                .map(|(_, x, y, z, alive)| (*x, *y, *z, *alive))
        };
        track_patriot_assist_laser_endpoints(&mut self.patriot_assist_lasers, lookup);
        expire_patriot_assist_lasers(&mut self.patriot_assist_lasers, self.frame);
    }

    /// Advance pending Patriot AssistedTargeting residual clips.
    ///
    /// Fires one assist-weapon shot per DelayBetweenShots (**8** frames) until
    /// AssistingClipSize (**4**) is exhausted or victim dies / leaves range.
    pub(crate) fn tick_pending_patriot_assists_sole(&mut self) {
        self.update_pending_patriot_assists();
    }

    pub(in super::super) fn update_pending_patriot_assists(&mut self) {
        use crate::game_logic::host_base_defense::{
            LAZR_PATRIOT_FIRE_AUDIO, PATRIOT_ASSIST_DELAY_FRAMES, PATRIOT_FIRE_AUDIO,
            is_within_patriot_assist_weapon_range,
        };

        if self.pending_patriot_assists.is_empty() {
            return;
        }

        let frame = self.frame;
        let mut keep: Vec<crate::game_logic::host_base_defense::PendingPatriotAssist> = Vec::new();
        // Drain current pending so we can mutate objects freely.
        let pending = std::mem::take(&mut self.pending_patriot_assists);

        for mut clip in pending {
            if clip.shots_remaining == 0 {
                continue;
            }
            // Drop if assistant / victim gone.
            let Some(assistant) = self.objects.get(&clip.assistant_id) else {
                continue;
            };
            if !assistant.is_alive() || !assistant.can_attack() {
                continue;
            }
            let assistant_team = assistant.team;
            let assistant_pos = assistant.get_position();
            let assistant_template = assistant.template_name.clone();
            let is_laser = crate::game_logic::host_base_defense::is_laser_patriot_template(
                &assistant_template,
            );
            let is_supw =
                crate::game_logic::host_base_defense::is_supw_patriot_template(&assistant_template);

            let Some(victim) = self.objects.get(&clip.victim_id) else {
                continue;
            };
            if !victim.is_alive() {
                continue;
            }
            let victim_pos = victim.get_position();
            let vdist = {
                let dx = assistant_pos.x - victim_pos.x;
                let dz = assistant_pos.z - victim_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            if !is_within_patriot_assist_weapon_range(vdist) {
                // Leave range → cancel remaining clip residual.
                continue;
            }

            if frame < clip.next_shot_frame {
                keep.push(clip);
                continue;
            }

            // Fire one assist residual shot.
            let damage = clip.damage();
            let asst_pos = self
                .objects
                .get(&clip.assistant_id)
                .map(|a| a.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            let weapon_snap = self
                .objects
                .get(&clip.assistant_id)
                .and_then(|a| a.weapon.clone());
            let (destroyed, _) = self.residual_auto_fire_apply_damage(
                clip.assistant_id,
                clip.victim_id,
                damage,
                asst_pos,
                weapon_snap.as_ref(),
                0,
            );
            if destroyed {
                self.mark_object_for_destruction(clip.victim_id, Some(assistant_team));
            }
            if let Some(asst) = self.objects.get_mut(&clip.assistant_id) {
                let _ = asst.capture_pending_weapon_visual_dispatch(
                    0,
                    frame,
                    Some(clip.victim_id),
                    Some(victim_pos),
                );
                // Assist residual marks engagement; keeps primary on clip-reload honesty.
                if let Some(w) = asst.weapon.as_mut() {
                    let t = frame as f32 * LOGIC_FRAME_TIMESTEP;
                    crate::game_logic::Object::consume_ammo_on_fire(w, t);
                }
                // AI attack authority: residual fire-intent for GameWorld last-writer.
                if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                    let (dmg, rng) = asst
                        .weapon
                        .as_ref()
                        .map(|w| (w.damage, w.range))
                        .unwrap_or((0.0, 0.0));
                    let frame = crate::game_logic::host_historic_bonus::logic_frame();
                    let next_count = asst.fire_intent_count.saturating_add(1);
                    crate::game_logic::host_fire_intent_log::record(
                        asst.id,
                        clip.victim_id.0,
                        0,
                        dmg,
                        rng,
                        frame as f32 * LOGIC_FRAME_TIMESTEP,
                        frame,
                        next_count,
                    );
                    asst.fire_intent_count = next_count;
                }
                asst.set_target(Some(clip.victim_id));
                asst.set_ai_state(AIState::Attacking);
                asst.set_status_attacking(true);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(
                        clip.assistant_id,
                        clip.victim_id,
                    );
                    crate::game_logic::host_ai_decision_log::record_set_state(clip.assistant_id, 2);
                }
                // Kill XP awarded after this borrow via award_experience.
            }
            // An assisted-targeting clip is a real primary WeaponSet shot,
            // not one visual event per auto-fire victim.
            let _ = self.record_accepted_weapon_discharge(clip.assistant_id, 0);
            if destroyed {
                self.award_score_the_kill_experience(clip.assistant_id, clip.victim_id);
                // Clear engagement via decision authority (log-only when GW applies).
                self.stop_attack_decision_aware(clip.assistant_id);
            }

            let _ = self.combat_particles.spawn_weapon_fire_fx(
                assistant_pos,
                Some(victim_pos),
                frame,
                clip.assistant_id,
                Some(clip.victim_id),
            );
            let audio = if is_laser {
                LAZR_PATRIOT_FIRE_AUDIO
            } else {
                PATRIOT_FIRE_AUDIO
            };
            self.queue_audio_event(
                AudioEventRequest::new(audio)
                    .with_object(clip.assistant_id)
                    .with_position(assistant_pos)
                    .with_priority(160),
            );

            // SupW assist shells also seed EMPPatriotEffectSpheroid residual.
            if is_supw {
                self.apply_supw_patriot_emp_residual_at(
                    victim_pos,
                    clip.assistant_id,
                    assistant_team,
                    Some(clip.victim_id),
                );
            }

            self.patriot_assist_residual_fires =
                self.patriot_assist_residual_fires.saturating_add(1);
            self.base_defense_residual_fires = self.base_defense_residual_fires.saturating_add(1);

            clip.shots_remaining = clip.shots_remaining.saturating_sub(1);
            if clip.shots_remaining > 0 && !destroyed {
                clip.next_shot_frame = frame.saturating_add(PATRIOT_ASSIST_DELAY_FRAMES);
                keep.push(clip);
            }
        }

        self.pending_patriot_assists = keep;
    }

    /// SupW Patriot EMP residual: DISABLED_EMP for legal victims in EffectRadius 10.
    ///
    /// Retail EMPPatriotEffectSpheroid EMPUpdate residual (DisabledDuration 10000 ms).
    /// Spawns EMPPatriotEffectSpheroid at impact and EMPSparks on disabled victims.
    /// DoesNotAffectMyOwnBuildings skips the firing player's structures
    /// (C++ getControllingPlayer), not the whole team.
    /// Patch 1.01: intended airborne victim → onlyEffectAirborne (skip ground).
    pub(in super::super) fn apply_supw_patriot_emp_residual_at(
        &mut self,
        impact: glam::Vec3,
        source_id: ObjectId,
        source_team: Team,
        intended_target: Option<ObjectId>,
    ) {
        use crate::game_logic::host_base_defense::{
            SUPW_PATRIOT_EMP_AUDIO, SUPW_PATRIOT_EMP_DURATION_FRAMES, SUPW_PATRIOT_EMP_RADIUS,
            is_emp_own_building, is_legal_supw_patriot_emp_target, supw_emp_scatter_aim,
            supw_emp_scatter_misses_infantry, supw_patriot_emp_until_frame,
        };
        use crate::game_logic::host_emp_pulse::is_emp_hardened_name;

        // C++ SupW_EMPBlast ScatterRadiusVsInfantry residual on EMP center.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        if intended_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source_id.0,
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = supw_emp_scatter_aim(impact, true, seed);
            if scattered {
                self.supw_emp_scatter_applied = self.supw_emp_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if supw_emp_scatter_misses_infantry(true, seed, hit_r) {
                if let Some(pos) = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position())
                {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    // Outside EMP radius: intended infantry not force-disabled by miss.
                    if dist > SUPW_PATRIOT_EMP_RADIUS {
                        self.supw_emp_scatter_misses =
                            self.supw_emp_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        // C++ ProjectileDetonationOCL CreateObject EMPPatriotEffectSpheroid at impact.
        let _ = self.spawn_emp_patriot_spheroid(impact, source_id);

        let source_owner = self.objects.get(&source_id).and_then(|o| o.owner_player_id);
        let until = supw_patriot_emp_until_frame(self.frame);
        let radius = SUPW_PATRIOT_EMP_RADIUS;
        use crate::game_logic::host_emp_pulse::{
            emp_intended_victim_near_miss_disables, emp_skip_ground_when_airborne_only,
            in_emp_pulse_radius_from_bounding_sphere_3d, leftover_emp_bounding_sphere_radius,
            should_emp_kill_airborne, should_emp_skip_hardened_airborne,
        };
        // C++ EMPUpdate.cpp:164-175 / 198-201 — producer AI victim airborne.
        let only_effect_airborne = intended_target
            .and_then(|id| self.objects.get(&id))
            .is_some_and(|o| o.status.airborne_target);
        let victims: Vec<(ObjectId, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if *id == source_id || !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                let sphere = leftover_emp_bounding_sphere_radius(
                    obj.thing.geometry.radius,
                    obj.thing.geometry.bounds_min,
                    obj.thing.geometry.bounds_max,
                    obj.selection_radius,
                );
                if !in_emp_pulse_radius_from_bounding_sphere_3d(impact, pos, sphere, radius) {
                    return None;
                }
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                let is_aircraft = obj.is_kind_of(KindOf::Aircraft);
                let is_airborne = obj.status.airborne_target;
                if emp_skip_ground_when_airborne_only(only_effect_airborne, is_airborne) {
                    return None;
                }
                let emp_hardened = is_emp_hardened_name(&obj.template_name);
                let is_structure = obj.is_kind_of(KindOf::Structure);
                let is_own_structure =
                    is_emp_own_building(is_structure, source_owner, obj.owner_player_id);
                // C++ isFactionStructure = any KINDOFMASK_FS bit, not current team.
                // Captured derricks/tech stay non-FS and must not freeze.
                let is_faction_structure = is_structure && obj.is_faction_structure();
                if should_emp_kill_airborne(is_aircraft, is_airborne, emp_hardened) {
                    return Some((*id, true, false));
                }
                // C++ EMPUpdate.cpp:240-241 — EMP_HARDENED airborne continue.
                if should_emp_skip_hardened_airborne(is_aircraft, is_airborne, emp_hardened) {
                    return None;
                }
                if !is_legal_supw_patriot_emp_target(
                    is_vehicle,
                    is_aircraft,
                    is_faction_structure,
                    is_own_structure,
                    true,
                    obj.status.under_construction,
                    emp_hardened,
                ) {
                    return None;
                }
                Some((*id, false, true))
            })
            .collect();
        let mut any = false;
        let mut intended_victim_processed = false;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();
        let mut spark_ids: Vec<ObjectId> = Vec::new();
        for (vid, kill, disable) in victims {
            if kill {
                destroy_ids.push(vid);
                any = true;
                continue;
            }
            if disable {
                if let Some(v) = self.objects.get_mut(&vid) {
                    v.apply_disabled_emp(until);
                    any = true;
                    spark_ids.push(vid);
                    if intended_target == Some(vid) {
                        intended_victim_processed = true;
                    }
                }
            }
        }
        // C++ EMPUpdate.cpp:321-339 leftover near-miss: intended aircraft
        // outside EffectRadius still DISABLED_EMP when dist_sqr <= radius*2
        // or <= 40*40. No sparks on this path.
        if let Some(victim_id) = intended_target {
            if !intended_victim_processed {
                let apply = self.objects.get(&victim_id).is_some_and(|v| {
                    let pos = v.get_position();
                    let dx = pos.x - impact.x;
                    let dy = pos.y - impact.y;
                    let dz = pos.z - impact.z;
                    emp_intended_victim_near_miss_disables(
                        false,
                        v.is_kind_of(KindOf::Aircraft),
                        is_emp_hardened_name(&v.template_name),
                        dx * dx + dy * dy + dz * dz,
                        radius,
                    )
                });
                if apply {
                    if let Some(v) = self.objects.get_mut(&victim_id) {
                        v.apply_disabled_emp(until);
                        any = true;
                    }
                }
            }
        }
        for id in destroy_ids {
            self.mark_object_for_destruction(id, Some(source_team));
        }
        // C++ doDisableAttack EMPSparks on disabled victims (not airborne kills).
        for vid in spark_ids {
            self.spawn_emp_sparks_on_victim(vid, SUPW_PATRIOT_EMP_DURATION_FRAMES);
        }
        if any {
            self.supw_patriot_emp_residual_grants =
                self.supw_patriot_emp_residual_grants.saturating_add(1);
            self.queue_audio_event(
                AudioEventRequest::new(SUPW_PATRIOT_EMP_AUDIO)
                    .with_object(source_id)
                    .with_position(impact)
                    .with_priority(165),
            );
        }
    }

    /// Residual honesty: at least one base-defense auto-fire residual shot.
    pub fn honesty_base_defense_fire_ok(&self) -> bool {
        self.base_defense_residual_fires > 0
    }

    /// Residual honesty counter: base-defense auto-fire residual shots.
    pub fn base_defense_residual_fires(&self) -> u32 {
        self.base_defense_residual_fires
    }

    /// Host PointDefenseLaser residual: Paladin / Avenger destroy nearest
    /// interceptable missile/projectile (primary) or damage infantry (secondary)
    /// in residual fire range without a manual AttackObject order.
    ///
    /// Fail-closed: not full PointDefenseLaserUpdate velocity prediction,
    /// TERTIARY WeaponStore allocate, or laser drawable path.
    /// C++ PointDefenseLaserBeam ThingFactory Object residual (95ms LifetimeUpdate).
    /// C++ Weapon::createLaser ThingFactory residual for combat LaserName beams.
    pub fn spawn_weapon_laser_beam_object(
        &mut self,
        laser_name: &str,
        from_id: ObjectId,
        to_id: Option<ObjectId>,
        from: glam::Vec3,
        to: glam::Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_weapon_laser::{
            WEAPON_LASER_BEAM_MAX_HEALTH, laser_beam_lifetime_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if laser_name.is_empty() {
            return None;
        }
        if !self.templates.contains_key(laser_name) {
            let mut t = ThingTemplate::new(laser_name);
            t.add_kind_of(KindOf::Immobile)
                .set_health(WEAPON_LASER_BEAM_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(laser_name.to_string(), t);
        }
        let team = self
            .objects
            .get(&from_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let mid = glam::Vec3::new(
            (from.x + to.x) * 0.5,
            (from.y + to.y) * 0.5 + 5.0,
            (from.z + to.z) * 0.5,
        );
        let bid = self.create_object(laser_name, team, mid)?;
        let life = laser_beam_lifetime_frames(laser_name).max(1);
        let expires = self.frame.saturating_add(life);
        if let Some(o) = self.objects.get_mut(&bid) {
            o.weapon_laser_beam = true;
            o.producer_id = Some(from_id);
            o.weapon_laser_beam_expires_frame = Some(expires);
            o.health.maximum = WEAPON_LASER_BEAM_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, WEAPON_LASER_BEAM_MAX_HEALTH);
        }
        let _ = to_id;
        self.weapon_laser_beams_spawned = self.weapon_laser_beams_spawned.saturating_add(1);
        Some(bid)
    }

    pub fn update_weapon_laser_beam_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.weapon_laser_beam {
                    if let Some(exp) = o.weapon_laser_beam_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.weapon_laser_beam = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn honesty_weapon_laser_beam_object_ok(&self) -> bool {
        self.weapon_laser_beams_spawned > 0
    }

    pub fn spawn_point_defense_laser_beam(
        &mut self,
        carrier_id: ObjectId,
        carrier_template: &str,
        from: glam::Vec3,
        to: glam::Vec3,
        to_id: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_point_defense::{
            PDL_LASER_BEAM_LIFETIME_FRAMES, PDL_LASER_BEAM_MAX_HEALTH, pdl_laser_beam_name,
        };
        use crate::game_logic::host_weapon_laser::ResidualWeaponLaser;
        use crate::game_logic::{KindOf, ThingTemplate};

        let beam_name = pdl_laser_beam_name(carrier_template);
        if !self.templates.contains_key(beam_name) {
            let mut t = ThingTemplate::new(beam_name);
            t.add_kind_of(KindOf::Immobile)
                .set_health(PDL_LASER_BEAM_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(beam_name.to_string(), t);
        }
        let team = self
            .objects
            .get(&carrier_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let mid = glam::Vec3::new(
            (from.x + to.x) * 0.5,
            (from.y + to.y) * 0.5 + 5.0,
            (from.z + to.z) * 0.5,
        );
        let bid = self.create_object(beam_name, team, mid)?;
        let expires = self
            .frame
            .saturating_add(PDL_LASER_BEAM_LIFETIME_FRAMES.max(1));
        if let Some(o) = self.objects.get_mut(&bid) {
            o.point_defense_laser_beam = true;
            o.producer_id = Some(carrier_id);
            o.point_defense_laser_beam_expires_frame = Some(expires);
            o.health.maximum = PDL_LASER_BEAM_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, PDL_LASER_BEAM_MAX_HEALTH);
        }
        self.weapon_lasers.push(ResidualWeaponLaser::new(
            beam_name,
            carrier_id,
            to_id,
            (from.x, from.y, from.z),
            (to.x, to.y, to.z),
            self.frame,
        ));
        self.point_defense_laser_beams_spawned =
            self.point_defense_laser_beams_spawned.saturating_add(1);
        Some(bid)
    }

    pub fn update_point_defense_laser_beam_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.point_defense_laser_beam {
                    if let Some(exp) = o.point_defense_laser_beam_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.point_defense_laser_beam = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn honesty_point_defense_laser_beam_ok(&self) -> bool {
        self.point_defense_laser_beams_spawned > 0
    }

    pub fn update_point_defense_intercept(&mut self) {
        use crate::game_logic::host_point_defense::{
            PDL_INTERCEPT_AUDIO, intercept_priority, is_point_defense_carrier,
            is_primary_intercept_target, is_secondary_intercept_target, pdl_damage,
            pdl_delay_frames, pdl_fire_range, pdl_module_count,
        };

        // Snapshot carriers first (immutable pass).
        let carriers: Vec<(ObjectId, String, Team, glam::Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                // C++ PointDefenseLaserBeam objects carry only Draw +
                // LifetimeUpdate — never a PointDefenseLaserUpdate module — so
                // the spawned beam visual must not rescan as a carrier.
                if obj.point_defense_laser_beam {
                    return None;
                }
                if !obj.is_alive() || obj.is_disabled() {
                    return None;
                }
                if !is_point_defense_carrier(&obj.template_name) {
                    return None;
                }
                // Under construction / unmanned residual: no laser.
                if obj.status.under_construction || obj.status.disabled_unmanned {
                    return None;
                }
                // Weapons jam residual: cannot fire laser either.
                if obj.status.weapons_jammed {
                    return None;
                }
                Some((*id, obj.template_name.clone(), obj.team, obj.get_position()))
            })
            .collect();

        if carriers.is_empty() {
            return;
        }

        let frame = self.frame;
        let mut intercepts_this_pass = 0u32;

        for (carrier_id, template_name, team, carrier_pos) in carriers {
            let fire_range = pdl_fire_range(&template_name);
            let damage = pdl_damage(&template_name);
            let delay = pdl_delay_frames(&template_name);
            let carrier_xz = (carrier_pos.x, carrier_pos.z);
            // Leftover PointDefenseLaserUpdate is per-module. Avenger authors two
            // independent 500ms clocks; each update scans after prior modules fire
            // so two missiles in the same window can both die.
            let module_count = pdl_module_count(&template_name);

            for module_i in 0..module_count {
                let ready_frame = if module_i == 0 {
                    self.point_defense_next_ready_frame
                        .get(&carrier_id)
                        .copied()
                        .unwrap_or(0)
                } else {
                    self.point_defense_next_ready_frame_1
                        .get(&carrier_id)
                        .copied()
                        .unwrap_or(0)
                };
                if frame < ready_frame {
                    continue;
                }

                // Pure residual acquire: primary missiles first, then secondary infantry;
                // closer wins within the same priority band (XZ range / 3D tiebreak).
                let allow_secondary =
                    crate::game_logic::host_usa_tanks::paladin_allows_secondary_infantry_intercept(
                        &template_name,
                    );
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&tid, target)| {
                        let same_team = target.team == team;
                        let stealthed = target.status.stealthed;
                        let detected = target.status.detected;
                        let disguised = target.status.disguised;
                        // Leftover scan_closest_target: STEALTHED && !DETECTED &&
                        // !DISGUISED continue (primary and secondary).
                        let cloaked = stealthed && !detected && !disguised;
                        let is_primary = !cloaked
                            && is_primary_intercept_target(
                                target.is_kind_of(KindOf::Projectile)
                                    || target.object_type == ObjectType::Projectile,
                                target.is_alive(),
                                same_team,
                                &target.template_name,
                            );
                        // Retail: only Paladin has SecondaryTargetTypes = INFANTRY.
                        // Avenger / King Raptor / Combat Chinook: missiles only.
                        let is_secondary = if allow_secondary {
                            is_secondary_intercept_target(
                                target.is_kind_of(KindOf::Infantry),
                                target.is_alive(),
                                same_team,
                                target.status.under_construction,
                                stealthed,
                                detected,
                                disguised,
                            )
                        } else {
                            false
                        };
                        crate::game_logic::host_residual_acquire::PriorityAcquireCandidate {
                            id: tid,
                            position: target.get_position(),
                            is_alive: target.is_alive(),
                            priority: intercept_priority(is_primary, is_secondary),
                        }
                    })
                    .collect();
                let Some((target_id, prio, _)) =
                    crate::game_logic::host_residual_acquire::pick_best_priority_residual_target(
                        carrier_id,
                        carrier_pos,
                        carrier_xz,
                        fire_range,
                        candidates,
                    )
                else {
                    continue;
                };

                // Primary missiles / projectiles: destroy residual (laser one-shots).
                // Secondary infantry: apply PDL damage residual.
                let mut destroyed = false;
                let mut impact_pos = carrier_pos;
                if prio == 0 {
                    if let Some(target) = self.objects.get_mut(&target_id) {
                        impact_pos = target.get_position();
                    }
                    // Instant destroy residual for interceptable missiles.
                    // Damage-authority aware: HP last-write via damage log; destroy flag host-local.
                    self.mark_destroyed_authority_aware(target_id, Some(carrier_id));
                    destroyed = self
                        .objects
                        .get(&target_id)
                        .map(|t| t.status.destroyed || !t.is_alive() || t.health.current <= 0.0)
                        .unwrap_or(true);
                } else if let Some(target) = self.objects.get_mut(&target_id) {
                    impact_pos = target.get_position();
                    destroyed = target.take_damage_from_immediate_residual(
                        damage,
                        Some(carrier_id),
                        crate::game_logic::host_usa_tanks::PALADIN_PDL_DAMAGE_TYPE,
                        crate::game_logic::host_usa_tanks::PALADIN_PDL_DEATH_TYPE,
                    );
                    // Under damage authority take_damage does not zero host HP; project kill for
                    // mark_object_for_destruction when lethal residual is logged.
                    if !destroyed
                        && crate::gameworld_shadow::gameworld_damage_authority_live()
                        && damage >= target.health.current
                    {
                        destroyed = true;
                    }
                }

                if module_i == 0 {
                    self.point_defense_next_ready_frame
                        .insert(carrier_id, frame.saturating_add(delay));
                } else {
                    self.point_defense_next_ready_frame_1
                        .insert(carrier_id, frame.saturating_add(delay));
                }
                intercepts_this_pass = intercepts_this_pass.saturating_add(1);
                self.point_defense_residual_intercepts =
                    self.point_defense_residual_intercepts.saturating_add(1);
                // C++ Weapon::createLaser / PointDefenseLaserBeam Lifetime residual.
                let _ = self.spawn_point_defense_laser_beam(
                    carrier_id,
                    &template_name,
                    carrier_pos,
                    impact_pos,
                    Some(target_id),
                );
                // AI attack authority: PDL discharge records fire-intent for GameWorld last-writer.
                if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                    if let Some(carrier) = self.objects.get_mut(&carrier_id) {
                        let next_count = carrier.fire_intent_count.saturating_add(1);
                        let sim_t = frame as f32 * LOGIC_FRAME_TIMESTEP;
                        crate::game_logic::host_fire_intent_log::record(
                            carrier_id,
                            target_id.0,
                            0,
                            damage,
                            fire_range,
                            sim_t,
                            frame,
                            next_count,
                        );
                        carrier.fire_intent_count = next_count;
                    }
                }
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(carrier_id, target_id);
                }

                if destroyed {
                    self.mark_object_for_destruction(target_id, Some(team));
                }

                let _ = self.combat_particles.spawn_weapon_fire_fx(
                    carrier_pos,
                    Some(impact_pos),
                    frame,
                    carrier_id,
                    Some(target_id),
                );
                self.queue_audio_event(
                    AudioEventRequest::new(PDL_INTERCEPT_AUDIO)
                        .with_object(carrier_id)
                        .with_position(carrier_pos)
                        .with_priority(150),
                );
            }
        }

        if intercepts_this_pass > 0 {
            log::debug!(
                "PointDefenseLaser residual: {} intercept(s) this pass (total={})",
                intercepts_this_pass,
                self.point_defense_residual_intercepts
            );
        }
    }

    /// Residual honesty: at least one PDL intercept residual shot.
    pub fn honesty_point_defense_intercept_ok(&self) -> bool {
        self.point_defense_residual_intercepts > 0
    }

    /// Residual honesty counter: PDL intercept residual shots.
    pub fn point_defense_residual_intercepts(&self) -> u32 {
        self.point_defense_residual_intercepts
    }

    /// Residual honesty: Avenger FAERIE_FIRE paint residual applied.
    pub fn honesty_avenger_paint_ok(&self) -> bool {
        self.avenger.honesty_paint_ok()
    }

    /// Residual honesty: Avenger air laser residual fire.
    pub fn honesty_avenger_air_laser_ok(&self) -> bool {
        self.avenger.honesty_air_laser_ok()
    }

    /// Residual honesty: TARGET_FAERIE_FIRE ROF grant residual.
    pub fn honesty_avenger_rof_ok(&self) -> bool {
        self.avenger.honesty_rof_ok()
    }

    /// Residual honesty: any Avenger residual path exercised.
    pub fn honesty_avenger_ok(&self) -> bool {
        self.avenger.honesty_ok()
    }

    /// Residual honesty counters: Avenger paints.
    pub fn avenger_residual_paints(&self) -> u32 {
        self.avenger.paints
    }

    /// Residual honesty counters: Avenger air laser fires.
    pub fn avenger_residual_air_laser_fires(&self) -> u32 {
        self.avenger.air_laser_fires
    }

    /// Residual honesty: at least one neutron shell blast residual applied.
    pub fn honesty_neutron_shell_ok(&self) -> bool {
        self.neutron_shell_residual_blasts > 0
            || self.neutron_shells_spawned > 0
            || self.neutron_shell_scatter_applied > 0
    }

    /// Residual honesty counter: neutron shell blasts.
    pub fn neutron_shell_residual_blasts(&self) -> u32 {
        self.neutron_shell_residual_blasts
    }

    /// Residual honesty counter: infantry killed by neutron residual.
    pub fn neutron_shell_residual_infantry_kills(&self) -> u32 {
        self.neutron_shell_residual_infantry_kills
    }

    /// Residual honesty counter: vehicles unmanned by neutron residual.
    pub fn neutron_shell_residual_vehicles_unmanned(&self) -> u32 {
        self.neutron_shell_residual_vehicles_unmanned
    }

    /// Residual honesty: Comanche rocket-pod area attack residual fired.
    pub fn honesty_comanche_rocket_pod_ok(&self) -> bool {
        self.comanche_rocket_pod_residual_area_attacks > 0
    }

    /// Residual honesty counter: rocket-pod area attacks.
    pub fn comanche_rocket_pod_residual_area_attacks(&self) -> u32 {
        self.comanche_rocket_pod_residual_area_attacks
    }

    /// Residual honesty counter: units hit by rocket-pod splash.
    pub fn comanche_rocket_pod_residual_units_hit(&self) -> u32 {
        self.comanche_rocket_pod_residual_units_hit
    }

    /// Residual honesty: Sentry Drone auto-fire residual shot.
    pub fn honesty_sentry_drone_auto_fire_ok(&self) -> bool {
        self.sentry_drone_residual_auto_fires > 0
    }

    /// Residual honesty counter: Sentry auto-fire residual shots.
    pub fn sentry_drone_residual_auto_fires(&self) -> u32 {
        self.sentry_drone_residual_auto_fires
    }

    /// Residual honesty: Sentry detector residual revealed at least one unit.
    pub fn honesty_sentry_drone_detect_ok(&self) -> bool {
        self.sentry_drone_residual_detects > 0
    }

    /// Residual honesty counter: Sentry detector residual reveals.
    pub fn sentry_drone_residual_detects(&self) -> u32 {
        self.sentry_drone_residual_detects
    }

    /// Residual honesty: Pathfinder detector residual revealed at least one unit.
    pub fn honesty_pathfinder_detect_ok(&self) -> bool {
        self.pathfinder_residual_detects > 0
    }

    /// Residual honesty counter: Pathfinder detector residual reveals.
    pub fn pathfinder_residual_detects(&self) -> u32 {
        self.pathfinder_residual_detects
    }

    /// Residual honesty: Pathfinder sniper residual fired at least once.
    pub fn honesty_pathfinder_sniper_ok(&self) -> bool {
        self.pathfinder_residual_sniper_fires > 0
    }

    /// Residual honesty: Scout drone detector residual revealed at least one unit.
    pub fn honesty_scout_drone_detect_ok(&self) -> bool {
        self.scout_drone_residual_detects > 0
    }

    /// Residual honesty: Scout drone attach residual succeeded.
    pub fn honesty_scout_drone_attach_ok(&self) -> bool {
        self.scout_drone_residual_attaches > 0
    }

    /// Residual honesty: Hellfire drone auto-fire residual shot.
    pub fn honesty_hellfire_drone_auto_fire_ok(&self) -> bool {
        self.hellfire_drone_residual_auto_fires > 0
            || self.hellfire_scatter_applied > 0
            || self.hellfire_scatter_misses > 0
    }

    /// Residual honesty: Hellfire ScatterRadiusVsInfantry peels applied.
    pub fn honesty_hellfire_scatter_ok(&self) -> bool {
        self.hellfire_scatter_applied > 0 || self.hellfire_scatter_misses > 0
    }

    /// Residual honesty: Hellfire drone attach residual succeeded.
    pub fn honesty_hellfire_drone_attach_ok(&self) -> bool {
        self.hellfire_drone_residual_attaches > 0
    }

    /// Residual honesty: Rocket Buggy long-range rocket residual fired.
    pub fn honesty_rocket_buggy_ok(&self) -> bool {
        self.rocket_buggy_residual_fires > 0
            || self.rocket_buggy_missiles_spawned > 0
            || self.rocket_buggy_scatter_applied > 0
    }

    /// Residual honesty: Rocket Buggy ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_rocket_buggy_scatter_ok(&self) -> bool {
        self.rocket_buggy_scatter_applied > 0 || self.rocket_buggy_residual_scatter_misses > 0
    }

    pub fn rocket_buggy_residual_fires(&self) -> u32 {
        self.rocket_buggy_residual_fires
    }

    pub fn rocket_buggy_residual_units_hit(&self) -> u32 {
        self.rocket_buggy_residual_units_hit
    }

    pub fn rocket_buggy_residual_scatter_misses(&self) -> u32 {
        self.rocket_buggy_residual_scatter_misses
    }

    /// Residual honesty: Quad Cannon ground or AA residual fired.
    pub fn honesty_quad_cannon_ok(&self) -> bool {
        self.quad_cannon_residual_ground_fires > 0 || self.quad_cannon_residual_aa_fires > 0
    }

    pub fn honesty_quad_cannon_aa_ok(&self) -> bool {
        self.quad_cannon_residual_aa_fires > 0
    }

    pub fn quad_cannon_residual_ground_fires(&self) -> u32 {
        self.quad_cannon_residual_ground_fires
    }

    pub fn quad_cannon_residual_aa_fires(&self) -> u32 {
        self.quad_cannon_residual_aa_fires
    }

    pub fn quad_cannon_residual_barrel_upgrades(&self) -> u32 {
        self.quad_cannon_residual_barrel_upgrades
    }

    /// Residual honesty: SCUD area blast residual.
    pub fn honesty_scud_launcher_ok(&self) -> bool {
        self.scud_poison_zones.honesty_host_path_ok() || self.scud_launcher_scatter_applied > 0
    }

    /// Residual honesty: SCUD Launcher ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_scud_launcher_scatter_ok(&self) -> bool {
        self.scud_launcher_scatter_applied > 0 || self.scud_launcher_scatter_misses > 0
    }

    pub fn honesty_scud_area_ok(&self) -> bool {
        self.scud_poison_zones.honesty_area_ok()
    }

    pub fn honesty_scud_toxin_ok(&self) -> bool {
        self.scud_poison_zones.honesty_toxin_ok()
    }

    pub fn scud_poison_zones(
        &self,
    ) -> &crate::game_logic::host_scud_launcher::HostScudPoisonRegistry {
        &self.scud_poison_zones
    }

    /// Residual honesty: Technical MG/cannon/RPG residual fired.
    pub fn honesty_technical_ok(&self) -> bool {
        self.technical_residual_fires > 0
            || self.technical_residual_weapon_upgrades > 0
            || (self.technical_residual_loads > 0 && self.technical_residual_unloads > 0)
            || self.technical_cannon_shells_spawned > 0
            || self.technical_cannon_scatter_applied > 0
    }

    /// Residual honesty: Technical cannon ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_technical_cannon_scatter_ok(&self) -> bool {
        self.technical_cannon_scatter_applied > 0 || self.technical_cannon_scatter_misses > 0
    }

    pub fn honesty_technical_weapon_upgrade_ok(&self) -> bool {
        self.technical_residual_weapon_upgrades > 0
    }

    pub fn honesty_technical_transport_ok(&self) -> bool {
        self.technical_residual_loads > 0 && self.technical_residual_unloads > 0
    }

    pub fn technical_residual_fires(&self) -> u32 {
        self.technical_residual_fires
    }

    pub fn technical_residual_units_hit(&self) -> u32 {
        self.technical_residual_units_hit
    }
}
