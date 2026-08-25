//! Host objects `impl GameLogic` — `object_ai_combat`.
//! update_object_ai, update_object_combat, player upgrades. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub(in super::super) fn update_object_ai(&mut self, object_id: ObjectId, _dt: f32) {
        // Get object state for AI processing
        let (ai_state, target_id, _position) = {
            if let Some(obj) = self.objects.get(&object_id) {
                (obj.ai_state.clone(), obj.target, obj.get_position())
            } else {
                return;
            }
        };

        if matches!(ai_state, AIState::Attacking | AIState::AttackMoving) {
            if let Some(target_id) = target_id {
                // Check if target still exists; fire when in range.
                // Out-of-range chase is owned by update_combat (assign_unit_path) —
                // do not stop_attack merely for distance (that aborted chases).
                if let Some(target) = self.objects.get(&target_id) {
                    if !target.is_alive() {
                        self.stop_attack_decision_aware(object_id);
                    } else if ai_state == AIState::Attacking {
                        if let Some(attacker) = self.objects.get(&object_id) {
                            if attacker.can_target(target) {
                                let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                                let (tgt_inf, tgt_faerie) = self
                                    .objects
                                    .get(&target_id)
                                    .map(|t| (t.is_kind_of(KindOf::Infantry), t.is_faerie_fire()))
                                    .unwrap_or((false, false));
                                if let Some(attacker) = self.objects.get_mut(&object_id) {
                                    // can_fire without target uses base ROF; fire_at_ex applies
                                    // TARGET_FAERIE_FIRE ROF residual against painted targets.
                                    if attacker.can_fire(current_time)
                                        || (tgt_faerie
                                            && attacker.weapon.as_ref().is_some_and(|w| {
                                                Object::weapon_ready_vs_target(
                                                    w,
                                                    current_time,
                                                    true,
                                                )
                                            }))
                                    {
                                        attacker.fire_at_ex(
                                            target_id,
                                            current_time,
                                            tgt_inf,
                                            tgt_faerie,
                                        );
                                    }
                                }
                            }
                            // else: OOR or weapon rules — combat chase / wait residual
                        }
                    }
                } else {
                    // Target no longer exists
                    self.stop_attack_decision_aware(object_id);
                }
            }
        }

        // Handle AttackingGround: fire at target_location.
        if ai_state == AIState::AttackingGround {
            let can_fire_ground = self
                .objects
                .get(&object_id)
                .map(|attacker| {
                    attacker.can_attack()
                        && attacker.can_fire(self.frame as f32 * LOGIC_FRAME_TIMESTEP)
                        && attacker.target_location.is_some()
                })
                .unwrap_or(false);

            if can_fire_ground {
                if let Some(attacker) = self.objects.get(&object_id) {
                    let shooter_pos = attacker.get_position();
                    let weapon_damage = attacker.weapon.as_ref().map(|w| w.damage).unwrap_or(25.0);
                    if let Some(target_loc) = attacker.target_location {
                        let wname = attacker.thing.template.primary_weapon_name.as_deref();
                        let scatter = wname
                            .map(|n| {
                                crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                                    n, false, /* ground force-fire: base ScatterRadius only */
                                )
                            })
                            .unwrap_or(0.0);
                        let proj_speed = attacker
                            .weapon
                            .as_ref()
                            .map(|w| {
                                if w.projectile_speed > 0.0 {
                                    w.projectile_speed
                                } else {
                                    200.0
                                }
                            })
                            .unwrap_or(200.0);
                        super::super::combat::queue_projectile(super::super::combat::PendingProjectile {
                            shooter_id: object_id,
                            shooter_pos,
                            source_context: Some(super::super::combat::ProjectileLaunchContext {
                                source_team: attacker.team,
                                source_owner_player_id: attacker.owner_player_id,
                                source_veterancy: attacker.experience.level,
                                source_orientation: attacker.get_orientation(),
                                source_velocity: attacker.movement.velocity,
                            }),
                            target_id: None,
                            target_pos: Some(target_loc),
                            damage: weapon_damage,
                            speed: proj_speed,
                            splash_radius: attacker
                                .weapon
                                .as_ref()
                                .map(|w| w.splash_radius)
                                .unwrap_or(0.0),
                            is_homing: false,
                            damage_type: crate::game_logic::combat::DamageType::Bullet,
                            death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                            projectile_object_name: wname
                                .map(crate::game_logic::weapon_bootstrap::host_projectile_name_for_weapon_name)
                                .unwrap_or_default(),

                            projectile_lifecycle: None,
                            fire_fx_name: wname
                                .map(|name| {
                                    crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
                                        name,
                                        attacker.experience.level,
                                    )
                                })
                                .unwrap_or_default(),
                            fire_ocl_name: wname
                                .map(|name| {
                                    crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(
                                        name,
                                        attacker.experience.level,
                                    )
                                })
                                .unwrap_or_default(),
                            detonation_fx_name: wname
                                .map(|name| {
                                    crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
                                        name,
                                        attacker.experience.level,
                                    )
                                })
                                .unwrap_or_default(),
                            detonation_ocl_name: wname
                                .map(|name| {
                                    crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name_at_veterancy(
                                        name,
                                        attacker.experience.level,
                                    )
                                })
                                .unwrap_or_default(),
                            exhaust_name: crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_unit_slot_at_veterancy(
                                attacker.template_name.as_str(),
                                attacker.thing.template.primary_weapon_name.as_deref(),
                                attacker.thing.template.secondary_weapon_name.as_deref(),
                                0,
                                attacker.experience.level,
                            ),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
            projectile_collides: crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT,
            scatter_radius: scatter,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: wname
                .map(crate::game_logic::weapon_bootstrap::host_die_on_detonate_for_weapon_name)
                .unwrap_or(false),

        });
                    }
                }
                if let Some(attacker) = self.objects.get_mut(&object_id) {
                    if let Some(w) = attacker.weapon.as_mut() {
                        w.last_fire_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                    }
                }
            }
        }
    }

    pub(in super::super) fn update_object_combat(&mut self, attacker_id: ObjectId, _dt: f32) {
        // Get attacker and target info
        let (weapon_damage, target_id, attacker_team, _attacker_owner_player_id) = {
            if let Some(attacker) = self.objects.get(&attacker_id) {
                if let (Some(weapon), Some(target_id)) = (&attacker.weapon, attacker.target) {
                    (
                        weapon.damage,
                        target_id,
                        attacker.team,
                        self.player_owner_for_host_object(attacker),
                    )
                } else {
                    return;
                }
            } else {
                return;
            }
        };

        // Apply damage to target (BodyModule last_damage_source residual).
        let (destroyed, kill_xp, victim_pos, victim_team, hp_lost) = {
            let Some(target) = self.objects.get_mut(&target_id) else {
                return;
            };
            let before = target.health.current;
            let destroyed = target.take_damage_from(weapon_damage, Some(attacker_id));
            let hp_lost = (before - target.health.current).max(0.0);
            if destroyed {
                let kill_xp = target.kill_experience_value();
                let victim_pos = target.get_position();
                let victim_team = target.team;
                (true, kill_xp, victim_pos, victim_team, hp_lost)
            } else {
                (false, 0.0, glam::Vec3::ZERO, Team::Neutral, hp_lost)
            }
        };
        // C++ Object.cpp:1846-1854 then TheRadar->tryUnderAttackEvent(this).
        if hp_lost > 0.0 {
            let _ = self.try_under_attack_from_damage(target_id);
        }
        // C++ ActiveBody: friend retaliation even if victim dies.
        let _ = self.try_friends_retaliate(target_id, attacker_id);
        if destroyed {
            log::debug!("Object {} destroyed object {}", attacker_id, target_id);
            // Skill points are awarded once on the destroy-list scoreTheKill path
            // (C++ Player::addSkillPointsForKill / victim SkillPointValue).
            self.mark_object_for_destruction(target_id, Some(attacker_team));
            let wname = self.objects.get(&attacker_id).and_then(|a| {
                a.thing
                    .template
                    .primary_weapon_name
                    .clone()
                    .or_else(|| a.thing.template.secondary_weapon_name.clone())
            });
            self.continue_or_stop_after_kill(
                attacker_id,
                target_id,
                victim_pos,
                victim_team,
                wname.as_deref(),
                kill_xp,
            );
        }
    }

    pub(in super::super) fn update_player_upgrades(&mut self) {
        // Residual: complete research when residual frames elapse for entries
        // that are NOT still advancing on a building PRODUCTION_UPGRADE queue.
        // Building-path completions are applied in `update_production`.
        // Frame event clear runs at the start of the production phase so
        // building-path `record_complete` events survive presentation freeze.

        use crate::game_logic::buildings::ProductionKind;
        use crate::game_logic::host_upgrades::HostUpgradePhase;

        // Upgrades currently researching on a producer building.
        let mut building_researching: std::collections::HashSet<(u32, String)> =
            std::collections::HashSet::new();
        for obj in self.objects.values() {
            let Some(building) = obj.building_data.as_ref() else {
                continue;
            };
            let Some(player_id) = self.player_owner_for_host_object(obj) else {
                continue;
            };
            for item in &building.production_queue {
                if item.kind == ProductionKind::Upgrade {
                    building_researching.insert((
                        player_id,
                        crate::game_logic::host_upgrades::normalize_upgrade_identity(
                            &item.template_name,
                        ),
                    ));
                }
            }
        }

        let frame = self.frame;
        let mut completed: Vec<(Team, u32, String)> = Vec::new();
        let mut cancelled: Vec<(u32, String)> = Vec::new();
        for entry in self.host_upgrades.entries_snapshot() {
            if entry.phase != HostUpgradePhase::Queued {
                continue;
            }
            let key = (
                entry.player_id,
                crate::game_logic::host_upgrades::normalize_upgrade_identity(&entry.name),
            );
            // Building owns the timer while the PRODUCTION_UPGRADE entry is live.
            if building_researching.contains(&key) {
                continue;
            }
            // C++ ProductionUpdate.cpp:636-648 / 1109-1112 — research lives
            // only on the producer's queue. Sold, dead, or a vanished queue
            // must cancel; never complete leftover residual frames.
            if let Some(source_id) = entry.source_object {
                let queue_live = self.objects.get(&source_id).is_some_and(|obj| {
                    obj.is_alive()
                        && !obj.status.sold
                        && obj.building_data.as_ref().is_some_and(|building| {
                            building.production_queue.iter().any(|item| {
                                item.kind == ProductionKind::Upgrade
                                    && crate::game_logic::host_upgrades::normalize_upgrade_identity(
                                        &item.template_name,
                                    ) == key.1
                            })
                        })
                });
                if !queue_live {
                    cancelled.push((entry.player_id, entry.name.clone()));
                    continue;
                }
            }
            let needed = entry.residual_research_frames.max(1);
            // Count the current simulation step as one research frame residual
            // (frame counter increments after update_simulation returns).
            let elapsed = frame.saturating_sub(entry.queue_frame).saturating_add(1);
            if elapsed >= needed {
                completed.push((entry.team, entry.player_id, entry.name.clone()));
            }
        }

        for (player_id, name) in cancelled {
            if let Some(player) = self.players.get_mut(&player_id) {
                if let Some(queued) = player.find_queued_upgrade_name(&name) {
                    player.queued_upgrades.remove(&queued);
                }
            }
            self.host_upgrades.record_cancel(&name, player_id);
        }

        // Direct player.queue_upgrade without host record (unit-test path):
        // complete after one simulation frame residual.
        for player in self.players.values() {
            for name in &player.queued_upgrades {
                let key = (
                    player.id,
                    crate::game_logic::host_upgrades::normalize_upgrade_identity(name),
                );
                if building_researching.contains(&key) {
                    continue;
                }
                let already = completed
                    .iter()
                    .any(|(t, pid, n)| *pid == player.id && n.eq_ignore_ascii_case(name));
                if already {
                    continue;
                }
                // No host entry → residual complete this update (legacy test path).
                let has_host = self.host_upgrades.entries_snapshot().iter().any(|e| {
                    e.player_id == player.id
                        && e.phase == HostUpgradePhase::Queued
                        && crate::game_logic::host_upgrades::normalize_upgrade_identity(&e.name)
                            == key.1
                });
                if !has_host {
                    completed.push((player.team, player.id, name.clone()));
                }
            }
        }

        for (team, player_id, name) in completed {
            let already = self
                .players
                .get(&player_id)
                .map(|p| p.has_unlocked_upgrade(&name))
                .unwrap_or(false);
            if let Some(player) = self.players.get_mut(&player_id) {
                player.complete_researched_upgrade(&name);
            }
            if !already {
                self.apply_host_upgrade_complete(team, player_id, &name);
            }
        }
    }

    /// Record that a player queued upgrade research (host residual honesty).
    pub fn record_host_upgrade_queued(
        &mut self,
        player_id: u32,
        team: Team,
        upgrade_name: &str,
        source_object: Option<ObjectId>,
    ) {
        self.host_upgrades
            .record_queue(upgrade_name, team, player_id, self.frame, source_object);
    }

    /// Record that a player cancelled upgrade research (host residual honesty).
    pub fn record_host_upgrade_cancelled(&mut self, player_id: u32, upgrade_name: &str) {
        self.host_upgrades.record_cancel(upgrade_name, player_id);
    }

    /// Apply unlock effects for a completed upgrade and record honesty.
    /// Matches C++ ProductionUpdate upgrade-complete: player mask + object giveUpgrade.

    /// C++ StatusBitsUpgrade::upgradeImplementation residual for team units.

    /// C++ PassengersFireUpgrade residual for Helix BattleBunker unlock.

    /// C++ ActiveShroudUpgrade::upgradeImplementation residual.
    pub fn apply_active_shroud_upgrade(&mut self, id: ObjectId, new_shroud_range: f32) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.set_shroud_range(new_shroud_range);
        self.active_shroud_upgrade_reg
            .record_apply(obj.shroud_range);
        // C++ ActiveShroudUpgrade::upgradeImplementation: handlePartitionCellMaintenance
        // so Object::shroud cover applies without waiting for the next look tick.
        self.update_main_crate_vision();
        true
    }

    pub(in super::super) fn apply_active_shroud_upgrade_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_active_shroud_upgrade::{
            peel_applies_to_template, peels_for_upgrade,
        };
        let peels = peels_for_upgrade(upgrade_name);
        if peels.is_empty() {
            return 0;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive() && super::weapon_upgrades::upgrade_targets_object(o, team)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            for peel in &peels {
                if !peel_applies_to_template(peel, &obj.template_name) {
                    continue;
                }
                obj.set_shroud_range(peel.new_shroud_range);
                self.active_shroud_upgrade_reg
                    .record_apply(obj.shroud_range);
                n = n.saturating_add(1);
            }
        }
        if n > 0 {
            self.update_main_crate_vision();
        }
        n
    }

    pub(in super::super) fn apply_passengers_fire_upgrade_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_passengers_fire_upgrade::should_enable_passengers_fire;
        if !crate::game_logic::host_passengers_fire_upgrade::is_passengers_fire_upgrade(
            upgrade_name,
        ) {
            return 0;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive() && super::weapon_upgrades::upgrade_targets_object(o, team)
            })
            .filter(|(_, o)| should_enable_passengers_fire(upgrade_name, &o.template_name))
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.passengers_allowed_to_fire = true;
                n = n.saturating_add(1);
            }
        }
        if n > 0 {
            self.passengers_fire_upgrade_reg.record_apply(n);
        }
        n
    }

    pub(in super::super) fn apply_status_bits_upgrade_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_status_bits_upgrade::collect_status_bits_for_upgrade;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive() && super::weapon_upgrades::upgrade_targets_object(o, team)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut touched = 0u32;
        for id in ids {
            let template_name = match self.objects.get(&id) {
                Some(obj) => obj.template_name.clone(),
                None => continue,
            };
            let pairs = collect_status_bits_for_upgrade(upgrade_name, &template_name);
            if pairs.is_empty() {
                continue;
            }
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let mut any = false;
            for (set, clear) in &pairs {
                let set_refs: Vec<&str> = set.iter().map(String::as_str).collect();
                let clear_refs: Vec<&str> = clear.iter().map(String::as_str).collect();
                let (set_c, clear_c) = obj.apply_status_bits_upgrade_masks(&set_refs, &clear_refs);
                self.status_bits_upgrade_reg.record_apply(set_c, clear_c);
                any = true;
            }
            if any {
                touched = touched.saturating_add(1);
            }
        }
        touched
    }

    pub(crate) fn apply_host_upgrade_complete(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
    ) {
        use crate::game_logic::host_upgrades::{
            HostUpgradeKind, is_object_scoped_upgrade, upgrade_mux_target_ids,
        };

        // All fan-out routines retain a `*_to_team` compatibility API, but a
        // live completion belongs to this exact player.  Scope the owner for
        // the synchronous dispatch so same-faction players do not inherit it.
        let _owner_scope = super::weapon_upgrades::enter_upgrade_owner_scope(player_id);
        let kind = HostUpgradeKind::from_name(upgrade_name);
        let source = self
            .host_upgrades
            .last_source_object_for(player_id, upgrade_name);
        let object_scoped = is_object_scoped_upgrade(upgrade_name);

        // C++ ProductionUpdate: PLAYER upgrades update the player mask then
        // every object `updateUpgradeModules`. OBJECT upgrades `giveUpgrade`
        // only on the producer.
        let units_affected = if object_scoped {
            0
        } else {
            match kind {
                HostUpgradeKind::FlashBangGrenade => {
                    self.apply_flashbang_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::TowMissile => self.apply_tow_unlock_to_team(team, upgrade_name),
                HostUpgradeKind::CaptureBuilding => {
                    self.apply_capture_unlock_tags_to_team(team, upgrade_name)
                }
                HostUpgradeKind::SupplyLines => {
                    self.apply_supply_lines_tags_to_team(team, upgrade_name)
                }
                HostUpgradeKind::NeutronShells => {
                    self.apply_neutron_shells_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::BunkerBusters => {
                    self.apply_bunker_busters_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::ComancheRocketPods => {
                    self.apply_comanche_rocket_pods_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::SentryDroneGun => {
                    self.apply_sentry_drone_gun_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::Camouflage => {
                    self.apply_camouflage_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::CamoNetting => {
                    self.apply_camo_netting_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::CompositeArmor => {
                    self.apply_composite_armor_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::WorkerShoes => {
                    self.apply_worker_shoes_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::NuclearTanks => {
                    self.apply_nuclear_tanks_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::BoobyTrap => {
                    self.apply_booby_trap_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::AnthraxGamma => {
                    self.apply_anthrax_gamma_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::SuicideBomb => {
                    self.apply_demo_suicide_bomb_unlock_to_team(team, upgrade_name)
                }
                HostUpgradeKind::AdvancedControlRods => {
                    self.apply_advanced_control_rods_to_team(team, upgrade_name)
                }
                HostUpgradeKind::SubliminalMessaging => {
                    self.apply_subliminal_messaging_to_team(team, upgrade_name)
                }
                HostUpgradeKind::ScorpionRocket => {
                    self.apply_scorpion_rocket_to_team(team, upgrade_name)
                }
                HostUpgradeKind::ApRockets => self.apply_ap_rockets_to_team(team, upgrade_name),
                HostUpgradeKind::LaserMissiles => {
                    self.apply_laser_missiles_to_team(team, upgrade_name)
                }
                HostUpgradeKind::Nationalism => self.apply_nationalism_to_team(team, upgrade_name),
                HostUpgradeKind::Fanaticism => self.apply_fanaticism_to_team(team, upgrade_name),

                HostUpgradeKind::ChainGuns => self.apply_chain_guns_to_team(team, upgrade_name),
                HostUpgradeKind::UraniumShells => {
                    self.apply_uranium_shells_to_team(team, upgrade_name)
                }
                HostUpgradeKind::BlackNapalm => self.apply_black_napalm_to_team(team, upgrade_name),
                HostUpgradeKind::ApBullets => self.apply_ap_bullets_to_team(team, upgrade_name),
                HostUpgradeKind::AnthraxBeta => self.apply_anthrax_beta_to_team(team, upgrade_name),
                HostUpgradeKind::ToxinShells => self.apply_toxin_shells_to_team(team, upgrade_name),
                HostUpgradeKind::AdvancedTraining => {
                    self.apply_advanced_training_to_team(team, upgrade_name)
                }
                HostUpgradeKind::TacticalNukeMig => {
                    self.apply_tactical_nuke_mig_to_team(team, upgrade_name)
                }
                HostUpgradeKind::DroneArmor => self.apply_drone_armor_to_team(team, upgrade_name),
                HostUpgradeKind::AircraftArmor => {
                    self.apply_aircraft_armor_to_team(team, upgrade_name)
                }
                HostUpgradeKind::ChinaMines => {
                    self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_ChinaMines")
                }
                HostUpgradeKind::EmpMines => {
                    self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_ChinaEMPMines")
                }
                HostUpgradeKind::FortifiedStructure => {
                    self.apply_fortified_structure_to_team(team, upgrade_name)
                }
                HostUpgradeKind::Radar => self.apply_radar_research_to_team(team, upgrade_name),
                HostUpgradeKind::RadarVanScan => {
                    self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_GLARadarVanScan")
                }
                HostUpgradeKind::ChemicalSuits => {
                    self.apply_chemical_suits_to_team(team, upgrade_name)
                }
                HostUpgradeKind::Moab => {
                    self.apply_player_unlock_upgrade(team, upgrade_name, "Upgrade_AmericaMOAB")
                }
                HostUpgradeKind::SatelliteHack => {
                    self.apply_satellite_hack_to_team(team, upgrade_name)
                }
                HostUpgradeKind::Countermeasures => {
                    self.apply_countermeasures_to_team(team, upgrade_name)
                }
                HostUpgradeKind::SlaveDrone => {
                    self.apply_slave_drone_upgrade_to_team(team, upgrade_name)
                }
                HostUpgradeKind::CashBounty => {
                    self.apply_cash_bounty_upgrade_to_team(team, upgrade_name)
                }
                HostUpgradeKind::HelixNapalmBomb => self.apply_helix_bomb_upgrade_to_team(
                    team,
                    upgrade_name,
                    crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NAPALM_BOMB,
                ),
                HostUpgradeKind::HelixNukeBomb => self.apply_helix_bomb_upgrade_to_team(
                    team,
                    upgrade_name,
                    crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NUKE_BOMB,
                ),
                HostUpgradeKind::Other => 0,
            }
        };

        let owner_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, obj)| {
                obj.is_alive() && super::weapon_upgrades::upgrade_targets_object(obj, team)
            })
            .map(|(id, _)| *id)
            .collect();
        let mux_ids = upgrade_mux_target_ids(object_scoped, source, owner_ids);
        let mut mux_n = 0u32;
        for id in mux_ids {
            // C++ Object::updateUpgradeModules → UpgradeMux::attemptUpgrade
            // (ReplaceObject / GrantScience / Unpause / StatusBits / add-ons).
            self.apply_upgrade_to_object(id, upgrade_name);
            mux_n = mux_n.saturating_add(1);
        }

        if !object_scoped {
            // C++ StatusBitsUpgrade::upgradeImplementation residual.
            let _ = self.apply_status_bits_upgrade_to_team(team, upgrade_name);
            // C++ PassengersFireUpgrade::upgradeImplementation residual.
            let _ = self.apply_passengers_fire_upgrade_to_team(team, upgrade_name);
            // C++ ActiveShroudUpgrade::upgradeImplementation residual.
            let _ = self.apply_active_shroud_upgrade_to_team(team, upgrade_name);
        }

        let units_affected = units_affected.max(mux_n);

        // Ensure registry has a queue entry even if command path skipped record
        // (e.g. direct Player::queue_upgrade in unit tests).
        self.host_upgrades.record_queue(
            upgrade_name,
            team,
            player_id,
            self.frame.saturating_sub(1),
            None,
        );
        self.host_upgrades
            .record_complete(upgrade_name, player_id, self.frame, units_affected);

        log::info!(
            "Host upgrade complete: player={} team={:?} '{}' kind={} units_affected={}",
            player_id,
            team,
            upgrade_name,
            kind.label(),
            units_affected
        );

        // C++ ProductionUpdate.cpp:888-917: only the local producer presents
        // completion audio. A valid authored ResearchSound is positional on
        // that producer and replaces EVA; UnitSpecificSound is independent
        // and is submitted after either branch.
        if self.is_local_player(player_id) {
            let completion_sounds = gamelogic::upgrade::center::with_upgrade_center(|center| {
                center
                    .find_upgrade(upgrade_name)
                    .filter(|template| !template.get_display_name().is_empty())
                    .map(|template| {
                        let research = template.get_research_sound();
                        let unit_specific = template.get_unit_specific_sound();
                        (
                            research
                                .is_valid()
                                .then(|| research.playable_event_name().to_string()),
                            unit_specific
                                .is_valid()
                                .then(|| unit_specific.playable_event_name().to_string()),
                        )
                    })
            });

            if let Some((research_sound, unit_specific_sound)) = completion_sounds {
                if let Some(event_name) = research_sound {
                    let mut request = AudioEventRequest::new(&event_name);
                    if let Some(producer) = source {
                        request = request.with_object(producer);
                    }
                    self.queue_audio_event(request);
                } else {
                    self.try_eva_upgrade_complete(player_id);
                }

                if let Some(event_name) = unit_specific_sound {
                    let mut request = AudioEventRequest::new(&event_name);
                    if let Some(producer) = source {
                        request = request.with_object(producer);
                    }
                    self.queue_audio_event(request);
                }
            }
        }

        // C++ TheRadar->createEvent(pos, RADAR_EVENT_UPGRADE) residual.
        self.try_radar_upgrade_complete(player_id, team, upgrade_name, source);
    }
}

#[cfg(test)]
mod live_upgrade_mux_tests {
    use crate::game_logic::host_overlord_addons::UPGRADE_OVERLORD_BUNKER;
    use crate::game_logic::host_upgrades::UPGRADE_CHINA_RADAR;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    fn add_player(logic: &mut GameLogic, id: u32, team: Team) {
        if logic.get_player(id).is_none() {
            let mut player = Player::new(id, team, "TestPlayer", true);
            player.resources.supplies = 100_000;
            logic.add_player(player);
        }
    }

    fn register_upgrade_completion_sounds(
        name: &str,
        research_sound: Option<&str>,
        unit_specific_sound: Option<&str>,
    ) {
        let mut source = format!("{name}\nDisplayName = CONTROLBAR:{name}\n");
        if let Some(sound) = research_sound {
            source.push_str(&format!("ResearchSound = {sound}\n"));
        }
        if let Some(sound) = unit_specific_sound {
            source.push_str(&format!("UnitSpecificSound = {sound}\n"));
        }
        source.push_str("End\n");
        gamelogic::upgrade::center::with_upgrade_center_mut(|center| {
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source(&source, |ini| {
                ini.read_line()?;
                center
                    .parse_upgrade_definition(ini)
                    .map_err(|_| game_engine::common::ini::INIError::InvalidData)
            })
            .expect("register upgrade completion sounds");
        });
    }

    fn upgrade_audio_fixture(upgrade_name: &str) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut producer = ThingTemplate::new("UpgradeAudioProducer");
        producer.set_health(1000.0);
        producer.add_kind_of(KindOf::Structure);
        logic
            .templates
            .insert("UpgradeAudioProducer".into(), producer);
        let producer_id = logic
            .create_object_for_player("UpgradeAudioProducer", 0, Vec3::ZERO)
            .expect("upgrade producer");
        logic.record_host_upgrade_queued(0, Team::USA, upgrade_name, Some(producer_id));
        // Object creation may enqueue unrelated lifecycle audio; this fixture
        // measures only the subsequent upgrade-completion branch.
        logic.queued_audio_events.clear();
        (logic, producer_id)
    }

    #[test]
    fn authored_upgrade_sounds_queue_on_producer_and_suppress_eva() {
        const UPGRADE: &str = "Upgrade_TestAuthoredCompletionSounds_hq_ib5c9";
        const RESEARCH: &str = "TestResearchComplete_hq_ib5c9";
        const UNIT: &str = "TestUnitSpecificComplete_hq_ib5c9";
        register_upgrade_completion_sounds(UPGRADE, Some(RESEARCH), Some(UNIT));
        let (mut logic, producer) = upgrade_audio_fixture(UPGRADE);

        logic.apply_host_upgrade_complete(Team::USA, 0, UPGRADE);

        assert_eq!(
            logic
                .queued_audio_events
                .iter()
                .filter(|event| event.event_type == RESEARCH || event.event_type == UNIT)
                .count(),
            2
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|event| event.event_type == RESEARCH && event.object_id == Some(producer))
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|event| event.event_type == UNIT && event.object_id == Some(producer))
        );
        assert!(!logic.honesty_eva_upgrade_complete_ok());
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|event| event.event_type != "UpgradeComplete")
        );
    }

    #[test]
    fn absent_research_sound_uses_eva_but_keeps_authored_unit_sound() {
        const UPGRADE: &str = "Upgrade_TestUnitOnlyCompletionSound_hq_ib5c9";
        const UNIT: &str = "TestUnitOnlyComplete_hq_ib5c9";
        register_upgrade_completion_sounds(UPGRADE, None, Some(UNIT));
        let (mut logic, producer) = upgrade_audio_fixture(UPGRADE);

        logic.apply_host_upgrade_complete(Team::USA, 0, UPGRADE);

        assert!(logic.honesty_eva_upgrade_complete_ok());
        let unit_events: Vec<_> = logic
            .queued_audio_events
            .iter()
            .filter(|event| event.event_type == UNIT)
            .collect();
        assert_eq!(unit_events.len(), 1);
        assert_eq!(unit_events[0].object_id, Some(producer));
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|event| event.event_type != "UpgradeComplete")
        );
    }

    #[test]
    fn host_complete_runs_replace_object_mux() {
        // C++ Object.cpp:2410 updateUpgradeModules → ReplaceObjectUpgrade
        // (hq-xmqu4). Live complete must call apply_upgrade_to_object.
        let mut logic = GameLogic::new();
        add_player(&mut logic, 2, Team::GLA);
        let mut fake = ThingTemplate::new("FakeGLABarracks");
        fake.set_health(500.0);
        fake.add_kind_of(KindOf::Structure);
        logic.templates.insert("FakeGLABarracks".into(), fake);
        let mut real = ThingTemplate::new("GLABarracks");
        real.set_health(1000.0);
        real.add_kind_of(KindOf::Structure);
        logic.templates.insert("GLABarracks".into(), real);

        let id = logic
            .create_object_for_player("FakeGLABarracks", 2, Vec3::new(10.0, 0.0, 20.0))
            .expect("fake barracks");
        logic.record_host_upgrade_queued(2, Team::GLA, "Upgrade_BecomeRealGLABarracks", Some(id));
        logic.apply_host_upgrade_complete(Team::GLA, 2, "Upgrade_BecomeRealGLABarracks");
        assert!(
            logic.objects.get(&id).is_none(),
            "fake must be removed via live complete mux"
        );
        assert!(
            logic
                .objects
                .values()
                .any(|obj| obj.template_name == "GLABarracks" && obj.is_alive()),
            "real barracks must spawn from live complete"
        );
    }

    #[test]
    fn host_complete_grants_moab_science() {
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut t = ThingTemplate::new("AmericaStrategyCenter");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("AmericaStrategyCenter".into(), t);
        let id = logic
            .create_object_for_player("AmericaStrategyCenter", 0, Vec3::ZERO)
            .expect("strategy center");
        logic.record_host_upgrade_queued(0, Team::USA, "Upgrade_AmericaMOAB", Some(id));
        logic.apply_host_upgrade_complete(Team::USA, 0, "Upgrade_AmericaMOAB");
        let player = logic.get_player(0).expect("player");
        assert!(
            player.has_unlocked_science("SCIENCE_MOAB"),
            "GrantScienceUpgrade must run on live complete"
        );
    }

    #[test]
    fn object_scoped_bunker_does_not_fan_out() {
        // C++ OBJECT upgrade giveUpgrade is producer-only (hq-w4syv).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::China);
        let mut t = ThingTemplate::new("ChinaTankOverlord");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("ChinaTankOverlord".into(), t);

        let a = logic
            .create_object_for_player("ChinaTankOverlord", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("overlord a");
        let b = logic
            .create_object_for_player("ChinaTankOverlord", 1, Vec3::new(40.0, 0.0, 0.0))
            .expect("overlord b");
        logic.record_host_upgrade_queued(1, Team::China, UPGRADE_OVERLORD_BUNKER, Some(a));
        logic.apply_host_upgrade_complete(Team::China, 1, UPGRADE_OVERLORD_BUNKER);

        let producer = logic.host_object(a).expect("producer");
        let other = logic.host_object(b).expect("other");
        assert_eq!(
            producer.overlord_bunker_slot_capacity(),
            5,
            "producer bunker must install"
        );
        assert_eq!(
            other.overlord_bunker_slot_capacity(),
            0,
            "sibling Overlord must not receive the object upgrade"
        );
    }

    #[test]
    fn china_command_center_radar_requires_research() {
        // C++ RadarUpgrade TriggeredBy Upgrade_ChinaRadar (hq-zx8e6).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::China);
        let mut t = ThingTemplate::new("ChinaCommandCenter");
        t.set_health(2000.0);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter);
        logic.templates.insert("ChinaCommandCenter".into(), t);
        let cc = logic
            .create_object_for_player("ChinaCommandCenter", 1, Vec3::ZERO)
            .expect("china cc");
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "China CC must not grant radar before research"
        );
        logic
            .get_player_mut(1)
            .expect("player")
            .add_completed_upgrade(UPGRADE_CHINA_RADAR);
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "China CC grants radar after Upgrade_ChinaRadar"
        );
    }

    #[test]
    fn gla_command_center_does_not_grant_radar() {
        // Leftover/INI: GLACommandCenter has no RadarUpgrade (hq-0clzf).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::GLA);
        let mut t = ThingTemplate::new("GLACommandCenter");
        t.set_health(2000.0);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter);
        logic.templates.insert("GLACommandCenter".into(), t);
        let cc = logic
            .create_object_for_player("GLACommandCenter", 1, Vec3::ZERO)
            .expect("gla cc");
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "GLA CC must not invent radar without a Radar Van"
        );
    }

    #[test]
    fn emp_revokes_command_center_radar_via_leftover_disabled_edge() {
        // leftover Object::on_disabled_edge removeRadar (hq-13kyr).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::USA);
        let mut t = ThingTemplate::new("AmericaCommandCenter");
        t.set_health(2000.0);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter);
        logic.templates.insert("AmericaCommandCenter".into(), t);
        let cc = logic
            .create_object_for_player("AmericaCommandCenter", 1, Vec3::ZERO)
            .expect("usa cc");
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "America CC grants radar"
        );
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.apply_disabled_emp(100);
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "EMP on the only CC must take radar offline"
        );
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.tick_disabled_emp(100);
        }
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "radar returns when EMP expires"
        );
    }

    #[test]
    fn emp_revokes_radar_van_even_when_disable_proof() {
        // Disable-proof only survives player brownout, not EMP on the van.
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::GLA);
        let mut t = ThingTemplate::new("GLAVehicleRadarVan");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("GLAVehicleRadarVan".into(), t);
        let van = logic
            .create_object_for_player("GLAVehicleRadarVan", 1, Vec3::ZERO)
            .expect("van");
        if let Some(obj) = logic.host_object_mut(van) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "Radar Van grants radar"
        );
        if let Some(obj) = logic.host_object_mut(van) {
            obj.apply_disabled_hacked(80);
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "hacked Radar Van must drop radar"
        );
    }

    #[test]
    fn object_upgrade_complete_stays_off_player_set() {
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::China);
        let mut t = ThingTemplate::new("ChinaTankOverlord");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("ChinaTankOverlord".into(), t);
        let a = logic
            .create_object_for_player("ChinaTankOverlord", 1, Vec3::ZERO)
            .expect("overlord");
        logic.get_player_mut(1).expect("player").queue_upgrade(
            UPGRADE_OVERLORD_BUNKER,
            &crate::game_logic::Resources {
                supplies: 0,
                power: 0,
            },
        );
        logic.record_host_upgrade_queued(1, Team::China, UPGRADE_OVERLORD_BUNKER, Some(a));
        logic.apply_host_upgrade_complete(Team::China, 1, UPGRADE_OVERLORD_BUNKER);
        logic
            .get_player_mut(1)
            .expect("player")
            .complete_researched_upgrade(UPGRADE_OVERLORD_BUNKER);
        assert!(
            !logic
                .get_player(1)
                .expect("player")
                .has_unlocked_upgrade(UPGRADE_OVERLORD_BUNKER),
            "OBJECT bunker must not stamp the player unlock set"
        );
    }

    #[test]
    fn dead_producer_cancels_residual_research() {
        use crate::game_logic::host_upgrades::HostUpgradePhase;
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut t = ThingTemplate::new("AmericaWarFactory");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), t);
        let id = logic
            .create_object_for_player("AmericaWarFactory", 0, Vec3::ZERO)
            .expect("factory");
        logic.get_player_mut(0).expect("player").queue_upgrade(
            "Upgrade_AmericaCompositeArmor",
            &crate::game_logic::Resources {
                supplies: 0,
                power: 0,
            },
        );
        logic.record_host_upgrade_queued(0, Team::USA, "Upgrade_AmericaCompositeArmor", Some(id));
        logic.objects.remove(&id);
        logic.update_player_upgrades();
        let player = logic.get_player(0).expect("player");
        assert!(
            !player.has_unlocked_upgrade("Upgrade_AmericaCompositeArmor"),
            "research must die with the producer"
        );
        assert!(!player.has_queued_upgrade("Upgrade_AmericaCompositeArmor"));
        assert!(
            logic.host_upgrades().entries_snapshot().iter().any(|e| e
                .name
                .eq_ignore_ascii_case("Upgrade_AmericaCompositeArmor")
                && e.phase == HostUpgradePhase::Cancelled),
            "host residual must record cancel, not complete"
        );
    }

    #[test]
    fn vanished_producer_queue_does_not_complete_residual() {
        // C++ ProductionUpdate.cpp:636-648 empty queue returns; 647-648 SOLD
        // freezes production; 1109-1112 onDie cancelAndRefundAllProduction.
        // Residual frames must not finish research after the queue vanishes.
        use crate::game_logic::buildings::{BuildingData, BuildingType};
        use crate::game_logic::host_upgrades::HostUpgradePhase;
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut t = ThingTemplate::new("AmericaWarFactory");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), t);
        let id = logic
            .create_object_for_player("AmericaWarFactory", 0, Vec3::ZERO)
            .expect("factory");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.building_data = Some(BuildingData::new(BuildingType::WarFactory));
        }
        logic.get_player_mut(0).expect("player").queue_upgrade(
            "Upgrade_AmericaCompositeArmor",
            &crate::game_logic::Resources {
                supplies: 0,
                power: 0,
            },
        );
        logic.record_host_upgrade_queued(0, Team::USA, "Upgrade_AmericaCompositeArmor", Some(id));
        logic.update_player_upgrades();
        let player = logic.get_player(0).expect("player");
        assert!(
            !player.has_unlocked_upgrade("Upgrade_AmericaCompositeArmor"),
            "vanished queue must not complete residual research"
        );
        assert!(!player.has_queued_upgrade("Upgrade_AmericaCompositeArmor"));
        assert!(
            logic.host_upgrades().entries_snapshot().iter().any(|e| e
                .name
                .eq_ignore_ascii_case("Upgrade_AmericaCompositeArmor")
                && e.phase == HostUpgradePhase::Cancelled),
            "host residual must record cancel, not complete"
        );
        assert!(
            logic.host_object(id).is_some_and(|o| o.is_alive()),
            "producer is still alive — only the queue vanished"
        );
    }
}
