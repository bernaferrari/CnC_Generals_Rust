//! Host combat `impl GameLogic` — `registries`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.is_paused = paused;
        log::debug!("Game {}", if paused { "paused" } else { "unpaused" });
    }

    /// Host combat particle registry (kill/fire feedback). Fail-closed residual.
    pub fn combat_particles(&self) -> &CombatParticleRegistry {
        &self.combat_particles
    }

    /// Host combat system (projectiles) for presentation freeze.
    pub fn combat_system(&self) -> &crate::game_logic::combat::CombatSystem {
        &self.combat_system
    }

    pub fn combat_system_mut(&mut self) -> &mut crate::game_logic::combat::CombatSystem {
        &mut self.combat_system
    }

    /// Mutable access for tests / presentation drain of frame events.
    pub fn combat_particles_mut(&mut self) -> &mut CombatParticleRegistry {
        &mut self.combat_particles
    }

    /// Host superweapon strike registry (queue + complete residual).
    pub fn special_power_strikes(
        &self,
    ) -> &crate::game_logic::special_power_strikes::HostSpecialPowerStrikeRegistry {
        &self.special_power_strikes
    }

    /// Mutable host superweapon strike registry (tests / presentation drain).
    pub fn special_power_strikes_mut(
        &mut self,
    ) -> &mut crate::game_logic::special_power_strikes::HostSpecialPowerStrikeRegistry {
        &mut self.special_power_strikes
    }

    /// Host America Paradrop / Airborne mission registry (queue + drop residual).
    pub fn host_paradrops(&self) -> &crate::game_logic::host_paradrop::HostParadropRegistry {
        &self.host_paradrops
    }

    /// Mutable host paradrop registry (tests / honesty drain).
    pub fn host_paradrops_mut(
        &mut self,
    ) -> &mut crate::game_logic::host_paradrop::HostParadropRegistry {
        &mut self.host_paradrops
    }

    /// Host GLA Rebel Ambush mission registry (queue + spawn residual).
    pub fn host_ambushes(&self) -> &crate::game_logic::host_ambush::HostAmbushRegistry {
        &self.host_ambushes
    }

    /// Mutable host ambush registry (tests / honesty drain).
    pub fn host_ambushes_mut(&mut self) -> &mut crate::game_logic::host_ambush::HostAmbushRegistry {
        &mut self.host_ambushes
    }

    /// Host USA Leaflet Drop mission registry (queue + delayed disable residual).
    pub fn host_leaflet_drops(
        &self,
    ) -> &crate::game_logic::host_leaflet_drop::HostLeafletDropRegistry {
        &self.host_leaflet_drops
    }

    /// Residual honesty: LeafletDrop activated at least once.
    pub fn honesty_leaflet_drop_activate_ok(&self) -> bool {
        self.host_leaflet_drops.honesty_activate_ok()
    }

    /// Residual honesty: LeafletDrop applied DISABLED_EMP at least once.
    pub fn honesty_leaflet_drop_disable_ok(&self) -> bool {
        self.host_leaflet_drops.honesty_disable_ok()
    }

    /// Combined host path honesty for LeafletDrop residual.
    pub fn honesty_leaflet_drop_ok(&self) -> bool {
        self.host_leaflet_drops.honesty_host_path_ok()
    }

    /// Host GLA Sneak Attack mission registry (queue + tunnel spawn residual).
    pub fn host_sneak_attacks(
        &self,
    ) -> &crate::game_logic::host_sneak_attack::HostSneakAttackRegistry {
        &self.host_sneak_attacks
    }

    /// Residual honesty: SneakAttack activated at least once.
    pub fn honesty_sneak_attack_activate_ok(&self) -> bool {
        self.host_sneak_attacks.honesty_activate_ok()
    }

    /// Residual honesty: SneakAttack spawned a tunnel at least once.
    pub fn honesty_sneak_attack_tunnel_ok(&self) -> bool {
        self.host_sneak_attacks.honesty_tunnel_spawn_ok()
    }

    /// Residual honesty: SneakAttack shockwave hit at least once.
    pub fn honesty_sneak_attack_shockwave_ok(&self) -> bool {
        self.host_sneak_attacks.honesty_shockwave_ok()
    }

    /// Combined host path honesty for SneakAttack residual (activate + tunnel).
    pub fn honesty_sneak_attack_ok(&self) -> bool {
        self.host_sneak_attacks.honesty_host_path_ok()
    }

    /// Host upgrade research registry (queue + complete residual).
    pub fn host_upgrades(&self) -> &crate::game_logic::host_upgrades::HostUpgradeRegistry {
        &self.host_upgrades
    }

    /// Mutable host upgrade research registry (tests / honesty drain).
    pub fn host_upgrades_mut(
        &mut self,
    ) -> &mut crate::game_logic::host_upgrades::HostUpgradeRegistry {
        &mut self.host_upgrades
    }

    /// Residual Supply Lines economy honesty: at least one boosted drop-off credited.
    /// Fail-closed: does not claim full Chinook/Worker INI boost matrix parity.
    pub fn honesty_supply_lines_economy_ok(&self) -> bool {
        self.supply_lines_bonus_cash_total > 0
            && self.host_upgrades.honesty_supply_lines_complete_ok()
    }

    /// Total residual cash credited from Supply Lines drop-off boost (observability).
    pub fn supply_lines_bonus_cash_total(&self) -> u32 {
        self.supply_lines_bonus_cash_total
    }

    /// Residual GLA Black Market honesty: at least one AutoDeposit cash credit.
    /// Fail-closed: not full Fake ActualMoney=No / capture-bonus / InGameUI GPU draw.
    pub fn honesty_black_market_ok(&self) -> bool {
        self.black_markets.honesty_ok()
    }

    /// Residual Black Market floating cash text honesty.
    pub fn honesty_black_market_floating_text_ok(&self) -> bool {
        self.black_markets.honesty_floating_text_ok()
    }

    /// Residual Black Market deposit count (observability).
    pub fn black_market_residual_deposits(&self) -> u32 {
        self.black_markets.deposits()
    }

    /// Total residual cash credited via Black Market AutoDeposit (observability).
    pub fn black_market_residual_cash_total(&self) -> u32 {
        self.black_markets.cash_total()
    }

    /// Residual Tech Oil Derrick honesty: at least one deposit or capture bonus.
    /// Fail-closed: not full InGameUI GPU / STEALTHED local display gate.
    pub fn honesty_oil_derrick_ok(&self) -> bool {
        self.oil_derricks.honesty_ok()
    }

    /// Residual Oil Derrick periodic deposit honesty.
    pub fn honesty_oil_derrick_deposit_ok(&self) -> bool {
        self.oil_derricks.honesty_deposit_ok()
    }

    /// Residual Oil Derrick capture bonus honesty.
    pub fn honesty_oil_derrick_capture_bonus_ok(&self) -> bool {
        self.oil_derricks.honesty_capture_bonus_ok()
    }

    /// Residual Oil Derrick SupplyLines UpgradedBoost honesty.
    pub fn honesty_oil_derrick_supply_lines_boost_ok(&self) -> bool {
        self.oil_derricks.honesty_supply_lines_boost_ok()
    }

    /// Residual Oil Derrick floating cash text honesty.
    pub fn honesty_oil_derrick_floating_text_ok(&self) -> bool {
        self.oil_derricks.honesty_floating_text_ok()
    }

    /// Residual Oil Derrick deposit count (observability).
    pub fn oil_derrick_residual_deposits(&self) -> u32 {
        self.oil_derricks.deposits()
    }

    /// Total residual cash from Oil Derrick periodic AutoDeposit (observability).
    pub fn oil_derrick_residual_cash_total(&self) -> u32 {
        self.oil_derricks.cash_total()
    }

    /// Total residual cash from Oil Derrick InitialCaptureBonus (observability).
    pub fn oil_derrick_capture_bonus_cash_total(&self) -> u32 {
        self.oil_derricks.capture_bonus_cash_total()
    }

    /// Total residual SupplyLines boost cash on oil derrick deposits (observability).
    pub fn oil_derrick_supply_lines_boost_cash_total(&self) -> u32 {
        self.oil_derricks.supply_lines_boost_cash_total()
    }

    /// Residual Hacker / Internet Center honesty: at least one cash ping.
    /// Fail-closed: not full unpack/pack / floating text.
    pub fn honesty_hacker_income_ok(&self) -> bool {
        self.hacker_income.honesty_ok()
    }

    /// Residual Hacker floating cash text honesty.
    pub fn honesty_hacker_floating_text_ok(&self) -> bool {
        self.hacker_income.honesty_floating_text_ok()
    }

    /// Residual Hacker Internet Center deposit honesty.
    pub fn honesty_hacker_internet_center_ok(&self) -> bool {
        self.hacker_income.honesty_internet_center_ok()
    }

    /// Residual Hacker deposit count (observability).
    pub fn hacker_residual_deposits(&self) -> u32 {
        self.hacker_income.deposits()
    }

    /// Total residual cash from Hacker income (observability).
    pub fn hacker_residual_cash_total(&self) -> u32 {
        self.hacker_income.cash_total()
    }

    /// Residual America Supply Drop Zone honesty: at least one OCL cash credit.
    /// Fail-closed: not full CreateAtEdge aircraft Object / parachute fall physics.
    pub fn honesty_supply_drop_zone_ok(&self) -> bool {
        self.supply_drop_zones.honesty_ok()
    }

    /// Residual Supply Drop Zone drop honesty (alias).
    pub fn honesty_supply_drop_zone_drop_ok(&self) -> bool {
        self.supply_drop_zones.honesty_drop_ok()
    }

    /// Residual Supply Drop Zone cargo flight started honesty (OCL create residual).
    pub fn honesty_supply_drop_zone_flight_ok(&self) -> bool {
        self.supply_drop_zones.honesty_flight_ok()
    }

    /// Residual DeliverPayload cargo host path honesty (crates spawned + cash).
    /// Fail-closed: not full AmericaJetCargoPlane Object flight path.
    pub fn honesty_deliver_payload_cargo_ok(&self) -> bool {
        self.host_deliver_payloads.honesty_host_path_ok()
    }

    /// Residual Supply Drop Zone cargo DeliverPayload host path honesty.
    pub fn honesty_supply_drop_cargo_deliver_payload_ok(&self) -> bool {
        self.host_deliver_payloads
            .honesty_supply_drop_cargo_host_path_ok()
    }

    /// Residual DropDelay per-item stagger honesty (host DeliverPayload).
    pub fn honesty_deliver_payload_drop_delay_stagger_ok(&self) -> bool {
        self.host_deliver_payloads.honesty_drop_delay_stagger_ok()
    }

    /// Residual MoneyCrateCollide unit pickup honesty.
    pub fn honesty_money_crate_unit_pickup_ok(&self) -> bool {
        self.host_money_crates.honesty_unit_pickup_ok()
    }

    /// Residual MoneyCrateCollide path honesty (unit or building).
    pub fn honesty_money_crate_collide_ok(&self) -> bool {
        self.host_money_crates.honesty_money_crate_collide_ok()
    }

    /// Residual MoneyPickUp Anim2D ExecuteAnimation honesty.
    pub fn honesty_money_pickup_anim_ok(&self) -> bool {
        self.host_money_crates.honesty_money_pickup_anim_ok()
    }

    /// Residual floating cash text presentation honesty.
    pub fn honesty_money_floating_text_ok(&self) -> bool {
        self.host_money_crates.honesty_money_floating_text_ok()
    }

    /// Residual above-terrain unit pickup reject honesty.
    pub fn honesty_money_crate_above_terrain_reject_ok(&self) -> bool {
        self.host_money_crates.honesty_above_terrain_reject_ok()
    }

    /// AmericaCrateParachute cargo fall-physics residual honesty.
    pub fn honesty_crate_parachute_fall_physics_ok(&self) -> bool {
        self.host_deliver_payloads
            .honesty_crate_parachute_fall_physics_ok()
    }

    /// CreateAtEdge AmericaJetCargoPlane flight residual honesty.
    pub fn honesty_create_at_edge_flight_ok(&self) -> bool {
        self.host_deliver_payloads
            .honesty_create_at_edge_flight_ok()
    }

    /// AmericaCrateParachute bone attach residual honesty.
    pub fn honesty_crate_parachute_bone_attach_ok(&self) -> bool {
        self.host_deliver_payloads.honesty_crate_bone_attach_ok()
    }

    /// Host MoneyCrateCollide registry (observability / tests).
    pub fn host_money_crates(
        &self,
    ) -> &crate::game_logic::host_money_crate::HostMoneyCrateRegistry {
        &self.host_money_crates
    }

    /// Residual Supply Drop Zone drop count (observability).
    pub fn supply_drop_zone_residual_drops(&self) -> u32 {
        self.supply_drop_zones.drops()
    }

    /// Residual Supply Drop Zone cargo flights started (observability).
    pub fn supply_drop_zone_residual_flights(&self) -> u32 {
        self.supply_drop_zones.flights_started()
    }

    /// Total residual cash credited via Supply Drop Zone OCL residual (observability).
    pub fn supply_drop_zone_residual_cash_total(&self) -> u32 {
        self.supply_drop_zones.cash_total()
    }

    /// Residual SupplyLines boost cash from Supply Drop Zone crates (observability).
    pub fn supply_drop_zone_supply_lines_boost_cash_total(&self) -> u32 {
        self.supply_drop_zones.supply_lines_boost_cash_total()
    }

    /// Host DeliverPayload registry (observability / tests).
    pub fn host_deliver_payloads(
        &self,
    ) -> &crate::game_logic::host_deliver_payload::HostDeliverPayloadRegistry {
        &self.host_deliver_payloads
    }

    /// Residual CommandCenter / RadarVan radar-online honesty.
    /// Fail-closed: not full RadarUpgrade/RadarUpdate module matrix.
    pub fn honesty_radar_online_ok(&self) -> bool {
        self.host_radar.honesty_ok()
    }

    /// Residual garrison honesty: successful structure enter count.
    pub fn garrison_residual_enters(&self) -> u32 {
        self.garrison_residual_enters
    }

    /// Residual garrison honesty: successful exit/evacuate count.
    pub fn garrison_residual_exits(&self) -> u32 {
        self.garrison_residual_exits
    }

    /// Residual garrison honesty: fire-from-garrison shots applied.
    pub fn garrison_residual_fires(&self) -> u32 {
        self.garrison_residual_fires
    }

    /// Residual transport honesty: successful vehicle load count.
    pub fn transport_residual_loads(&self) -> u32 {
        self.transport_residual_loads
    }

    /// Residual transport honesty: successful unload/evacuate count.
    pub fn transport_residual_unloads(&self) -> u32 {
        self.transport_residual_unloads
    }

    /// Residual Overlord BattleBunker honesty: successful infantry enter count.
    pub fn overlord_bunker_residual_enters(&self) -> u32 {
        self.overlord_bunker_residual_enters
    }

    /// Residual Overlord BattleBunker honesty: successful exit/evacuate count.
    pub fn overlord_bunker_residual_exits(&self) -> u32 {
        self.overlord_bunker_residual_exits
    }

    /// Record a residual structure-garrison enter (tests / host path).
    pub fn record_garrison_residual_enter(&mut self) {
        self.garrison_residual_enters = self.garrison_residual_enters.saturating_add(1);
    }

    /// Record a residual garrison exit (tests / host path).
    pub fn record_garrison_residual_exit(&mut self) {
        self.garrison_residual_exits = self.garrison_residual_exits.saturating_add(1);
    }

    /// Record a residual transport load (tests / host path).
    pub fn record_transport_residual_load(&mut self) {
        self.transport_residual_loads = self.transport_residual_loads.saturating_add(1);
    }

    /// Record a residual transport unload/evacuate (tests / host path).

    /// C++ move-to-and-evacuate arrival residual: dump all occupants near container.
    /// When `and_exit`, mark the transport sold/destroyed after unload (script exit residual).
    pub fn evacuate_container_now(&mut self, container_id: ObjectId, and_exit: bool) -> bool {
        let Some(container) = self.objects.get(&container_id) else {
            return false;
        };
        if !container.is_alive() {
            return false;
        }
        if container.is_combat_chinook_style_container() {
            if let Some(ai) = container.chinook_ai.as_ref() {
                if ai.ai_free_to_exit(false)
                    != crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
                {
                    if let Some(c) = self.objects.get_mut(&container_id) {
                        let p = c.get_position();
                        let contained = c.contained_units().len() as u32;
                        if let Some(ai) = c.chinook_ai.as_mut() {
                            ai.pos = [p.x, p.z, p.y];
                            ai.wanting_enter_or_exit = true;
                            ai.parent_idle = true;
                            ai.contained_count = contained;
                            if and_exit {
                                ai.command_evac([p.x, p.z, 0.0], true);
                            } else {
                                ai.tick_idle_auto_land();
                            }
                        }
                        c.pending_evacuate_on_stop = true;
                        c.pending_exit_after_evacuate = and_exit;
                    }
                    return false;
                }
            }
        }

        let origin = container
            .building_data
            .as_ref()
            .and_then(|b| b.rally_point)
            .unwrap_or_else(|| container.get_position());
        let passengers: Vec<ObjectId> = container.contained_units();
        if passengers.is_empty() && !and_exit {
            // Still clear pending flags.
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = false;
                c.pending_exit_after_evacuate = false;
            }
            return false;
        }

        let mut any = false;
        for (i, pid) in passengers.iter().enumerate() {
            // Remove from container first.
            if let Some(c) = self.objects.get_mut(&container_id) {
                let _ = c.remove_occupant(*pid);
            }
            if let Some(p) = self.objects.get_mut(pid) {
                // Wave 201: host_contain_log last-writer (do not bypass set_contained_by).
                p.set_contained_by(None);
                p.target = None;
                // Spread slightly so units don't stack.
                let angle = (i as f32) * 0.9;
                let drop = origin + glam::Vec3::new(angle.cos() * 8.0, 0.0, angle.sin() * 8.0);
                p.set_position(drop);
                p.stop_moving();
                p.set_ai_state(AIState::Idle);
                p.status.moving = false;
                any = true;
            }
            self.record_transport_residual_unload();
        }

        if let Some(c) = self.objects.get_mut(&container_id) {
            c.pending_evacuate_on_stop = false;
            c.pending_exit_after_evacuate = false;
            let p = c.get_position();
            if let Some(ai) = c.chinook_ai.as_mut() {
                ai.contained_count = 0;
                ai.wanting_enter_or_exit = false;
                if and_exit {
                    ai.command_evac([p.x, p.z, 0.0], true);
                    return any;
                }
            }
        }

        if and_exit {
            // C++ evacuate-and-exit: transport returns/deletes itself.
            // Wave 747: under damage authority, do not zero host HP mid-frame
            // (dual with GW HP writeback). Project lethal via damage log + flags;
            // non-authority path keeps host HP clear.
            if let Some(c) = self.objects.get_mut(&container_id) {
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = c.health.current.max(1.0);
                    crate::game_logic::host_damage_log::record(container_id, hp, None, true);
                } else {
                    c.health.current = 0.0;
                }
                c.status.destroyed = true;
                c.set_ai_state(AIState::Idle);
            }
            any = true;
        }
        any
    }

    pub fn record_transport_residual_unload(&mut self) {
        self.transport_residual_unloads = self.transport_residual_unloads.saturating_add(1);
    }

    /// Residual Battle Bus honesty: successful infantry load count.
    pub fn battle_bus_residual_loads(&self) -> u32 {
        self.battle_bus.loads
    }

    /// Residual Battle Bus honesty: successful unload/evacuate count.
    pub fn battle_bus_residual_unloads(&self) -> u32 {
        self.battle_bus.unloads
    }

    /// Residual Battle Bus honesty: passenger fire-from-bus shots.
    pub fn battle_bus_residual_passenger_fires(&self) -> u32 {
        self.battle_bus.passenger_fires
    }

    /// Residual Battle Bus honesty: armed-riders weapon-set upgrades.
    pub fn battle_bus_residual_weapon_set_upgrades(&self) -> u32 {
        self.battle_bus.weapon_set_upgrades
    }

    /// Record a residual Battle Bus load (tests / host path).
    pub fn record_battle_bus_residual_load(&mut self) {
        self.battle_bus.record_load();
    }

    /// Record a residual Battle Bus unload/evacuate (tests / host path).
    pub fn record_battle_bus_residual_unload(&mut self) {
        self.battle_bus.record_unload();
    }

    /// Residual honesty: Battle Bus load → docked → unload path.
    pub fn honesty_battle_bus_load_unload_ok(&self) -> bool {
        self.battle_bus.honesty_load_unload_ok()
    }

    pub fn honesty_toxin_fire_ocl_ok(&self) -> bool {
        self.toxin_tractor.fire_ocl_spawns > 0
    }

    pub fn honesty_preorder_create_ok(&self) -> bool {
        self.preorder_create_reg.honesty_ok()
    }

    pub fn set_player_did_preorder(&mut self, team: Team, did: bool) {
        for p in self.players.values_mut() {
            if p.team == team {
                p.did_preorder = did;
            }
        }
    }

    pub fn honesty_command_button_hunt_ok(&self) -> bool {
        self.command_button_hunt_reg.honesty_hunt_ok()
    }

    pub fn honesty_deploy_style_ok(&self) -> bool {
        self.deploy_style_reg.honesty_deploy_ok() && self.deploy_style_reg.honesty_undeploy_ok()
    }

    pub fn honesty_tensile_formation_ok(&self) -> bool {
        self.tensile_formation_reg.honesty_host_path_ok()
            && crate::game_logic::host_tensile_formation::honesty_tensile_formation_residual_ok()
    }

    pub fn honesty_status_bits_upgrade_ok(&self) -> bool {
        self.status_bits_upgrade_reg.honesty_host_path_ok()
            && crate::game_logic::host_status_bits_upgrade::honesty_status_bits_upgrade_residual_ok(
            )
    }

    pub fn honesty_fire_spread_ok(&self) -> bool {
        self.fire_spread_reg.honesty_host_path_ok()
            && crate::game_logic::host_fire_spread::honesty_fire_spread_residual_ok()
    }

    pub fn honesty_base_regenerate_ok(&self) -> bool {
        self.base_regenerate_reg.honesty_host_path_ok()
            && crate::game_logic::host_base_regenerate::honesty_base_regenerate_residual_ok()
    }

    pub fn honesty_enemy_near_ok(&self) -> bool {
        self.enemy_near_reg.honesty_host_path_ok()
            && crate::game_logic::host_enemy_near::honesty_enemy_near_residual_ok()
    }

    pub fn honesty_passengers_fire_upgrade_ok(&self) -> bool {
        self.passengers_fire_upgrade_reg.honesty_host_path_ok()
            && crate::game_logic::host_passengers_fire_upgrade::honesty_passengers_fire_upgrade_residual_ok()
    }

    pub fn honesty_animation_steering_ok(&self) -> bool {
        self.animation_steering_reg.honesty_host_path_ok()
            && crate::game_logic::host_animation_steering::honesty_animation_steering_residual_ok()
    }

    pub fn honesty_active_shroud_upgrade_ok(&self) -> bool {
        self.active_shroud_upgrade_reg.honesty_host_path_ok()
            && crate::game_logic::host_active_shroud_upgrade::honesty_active_shroud_upgrade_residual_ok()
    }

    pub fn honesty_float_update_ok(&self) -> bool {
        self.float_update_reg.honesty_host_path_ok()
            && crate::game_logic::host_float_update::honesty_float_update_residual_ok()
    }

    pub fn honesty_prone_update_ok(&self) -> bool {
        self.prone_update_reg.honesty_host_path_ok()
            && crate::game_logic::host_prone_update::honesty_prone_update_residual_ok()
    }

    pub fn honesty_radius_decal_update_ok(&self) -> bool {
        self.radius_decal_update_reg.honesty_host_path_ok()
            && crate::game_logic::host_radius_decal_update::honesty_radius_decal_update_residual_ok(
            )
    }

    pub fn honesty_checkpoint_update_ok(&self) -> bool {
        self.checkpoint_update_reg.honesty_host_path_ok()
            && crate::game_logic::host_checkpoint_update::honesty_checkpoint_update_residual_ok()
    }

    pub fn honesty_spectre_gunship_deployment_ok(&self) -> bool {
        self.spectre_gunship_deployment_reg.honesty_host_path_ok()
            && crate::game_logic::host_spectre_gunship_deployment::honesty_spectre_gunship_deployment_residual_ok()
    }

    pub fn honesty_smart_bomb_target_homing_ok(&self) -> bool {
        self.smart_bomb_target_homing_reg.honesty_host_path_ok()
            && crate::game_logic::host_smart_bomb_target_homing::honesty_smart_bomb_target_homing_residual_ok()
    }

    /// C++ ApplyRandomForceNugget::create residual on primary (dying) object.
    /// C++ NeutronMissileUpdate::update residual.
    pub fn update_neutron_missile_flights(&mut self) {
        use crate::game_logic::host_neutron_missile_update::NeutronMissileFlightPhase;

        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.neutron_missile_update.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        let mut intermediate_hits = 0u32;
        for id in ids {
            let (grounded, phase, is_cruise, producer, launch_fx, ignition_fx) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let pos = o.get_position();
                let producer = o.producer_id;
                let Some(data) = o.neutron_missile_update.as_mut() else {
                    continue;
                };
                let was_inter = data.reached_intermediate;
                let is_cruise = data.is_cruise;
                let tick = data.tick(pos, self.frame);
                if !was_inter && data.reached_intermediate {
                    intermediate_hits += 1;
                }
                let grounded = tick.grounded;
                let phase = tick.phase;
                let new_pos = tick.pos;
                let vel = tick.vel;
                let launch_fx = tick.launch_fx;
                let ignition_fx = tick.ignition_fx;
                drop(data);
                o.set_position(new_pos);
                o.movement.velocity = vel;
                if vel.length_squared() > 1e-6 {
                    let yaw = vel.z.atan2(vel.x);
                    o.set_orientation(yaw);
                }
                (grounded, phase, is_cruise, producer, launch_fx, ignition_fx)
            };
            if launch_fx || ignition_fx {
                let p = self
                    .objects
                    .get(&id)
                    .map(|o| o.get_position())
                    .unwrap_or(Vec3::ZERO);
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    p,
                    self.frame,
                    Some(id),
                    None,
                );
                if launch_fx {
                    let _ = crate::game_logic::dispatch_fx_list_at_pos(
                        crate::game_logic::host_neutron_missile_update::NEUTRON_LAUNCH_FX,
                        p,
                    );
                }
                if ignition_fx {
                    let _ = crate::game_logic::dispatch_fx_list_at_pos(
                        crate::game_logic::host_neutron_missile_update::NEUTRON_IGNITION_FX,
                        p,
                    );
                }
            }
            if grounded || matches!(phase, NeutronMissileFlightPhase::Dead) {
                // Impact residuals: neutron SlowDeath vs cruise MOAB detonation.
                let (pos, team, producer) = self
                    .objects
                    .get(&id)
                    .map(|o| (o.get_position(), o.team, o.producer_id.or(producer)))
                    .unwrap_or((Vec3::ZERO, Team::Neutral, producer));
                let source = producer.unwrap_or(id);
                // Kill delivery decals on missile + launcher.
                if let Some(o) = self.objects.get_mut(&id) {
                    if let Some(rd) = o.radius_decal_update.as_mut() {
                        rd.kill_radius_decal();
                        self.radius_decal_update_reg.record_kill(false);
                    }
                }
                if let Some(lid) = producer {
                    if let Some(o) = self.objects.get_mut(&lid) {
                        if let Some(rd) = o.radius_decal_update.as_mut() {
                            rd.kill_radius_decal();
                            self.radius_decal_update_reg.record_kill(false);
                        }
                    }
                }
                if is_cruise {
                    use crate::game_logic::combat::DamageType;
                    use crate::game_logic::special_power_strikes::{
                        CRUISE_MISSILE_DAMAGE, CRUISE_MISSILE_RADIUS, MOAB_FLAME_DAMAGE,
                    };
                    self.apply_fuel_air_radius_damage(
                        id,
                        producer,
                        team,
                        pos,
                        CRUISE_MISSILE_DAMAGE,
                        CRUISE_MISSILE_RADIUS,
                        DamageType::Explosive,
                    );
                    self.apply_fuel_air_radius_damage(
                        id,
                        producer,
                        team,
                        pos,
                        MOAB_FLAME_DAMAGE,
                        CRUISE_MISSILE_RADIUS,
                        DamageType::Flame,
                    );
                } else {
                    self.special_power_strikes
                        .spawn_neutron_slow_death_field(source, team, pos, self.frame, 0);
                }
                if let Some(o) = self.objects.get_mut(&id) {
                    o.fire_create_object_die();
                    o.fire_fx_list_die();
                }
                self.apply_pending_create_object_die(id);
                self.neutron_missile_update_reg.record_ground();
                destroy.push(id);
            }
        }
        for _ in 0..intermediate_hits {
            self.neutron_missile_update_reg.record_intermediate();
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ ScudStormMissile MissileAIUpdate ballistic residual tick.
    pub fn update_scud_storm_missile_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            SCUD_STORM_PRIMARY_RADIUS, SCUD_STORM_SECONDARY_RADIUS,
        };

        // ClipSize staggered launches residual.
        self.spawn_due_scud_storm_missiles();

        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.scud_storm_missile_flight.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in ids {
            let (grounded, ignition_fx, target, producer) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let pos = o.get_position();
                let producer = o.producer_id;
                let Some(data) = o.scud_storm_missile_flight.as_mut() else {
                    continue;
                };
                let target = data.target;
                let tick = data.tick(pos, self.frame);
                let grounded = tick.grounded;
                let ignition_fx = tick.ignition_fx;
                let new_pos = tick.pos;
                let vel = tick.vel;
                drop(data);
                o.set_position(new_pos);
                o.movement.velocity = vel;
                if vel.length_squared() > 1e-6 {
                    o.set_orientation(vel.z.atan2(vel.x));
                }
                (grounded, ignition_fx, target, producer)
            };
            if ignition_fx {
                let p = self
                    .objects
                    .get(&id)
                    .map(|o| o.get_position())
                    .unwrap_or(Vec3::ZERO);
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    p,
                    self.frame,
                    Some(id),
                    None,
                );
                let _ = crate::game_logic::dispatch_fx_list_at_pos(
                    crate::game_logic::special_power_strikes::SCUD_STORM_MISSILE_IGNITION_FX,
                    p,
                );
                self.scud_storm_missile_flight_reg.record_ignition();
            } else if !grounded {
                // ScudMissileExhaust residual (trail honesty; sparse spawn).
                if self.frame % 5 == 0 {
                    let p = self
                        .objects
                        .get(&id)
                        .map(|o| o.get_position())
                        .unwrap_or(Vec3::ZERO);
                    let _ = self.combat_particles.spawn(
                        CombatParticleKind::DeathExplosion,
                        p,
                        self.frame,
                        Some(id),
                        None,
                    );
                    self.scud_storm_missile_flight_reg.record_exhaust();
                }
            }
            if grounded {
                let team = self
                    .objects
                    .get(&id)
                    .map(|o| o.team)
                    .unwrap_or(Team::Neutral);
                // Resolve Anthrax Beta/Gamma from producer owner sciences/upgrades.
                use crate::game_logic::special_power_strikes::ScudStormAnthraxTier;
                let anthrax = {
                    let mut names: Vec<String> = Vec::new();
                    if let Some(pid) = producer {
                        if let Some(o) = self.objects.get(&pid) {
                            // Collect unlocked sciences for owner team players.
                            for p in self.players.values() {
                                if p.team == o.team {
                                    names.extend(p.unlocked_sciences.iter().cloned());
                                }
                            }
                        }
                    }
                    // Also accept Upgrade_ naming via sciences list residual.
                    ScudStormAnthraxTier::highest_from_upgrades(names.iter().map(|s| s.as_str()))
                };
                // ScudStormDamageWeapon primary + secondary residual at scatter impact.
                // Chem_GLAScudStorm residual: primary is anthrax warhead (no HE primary).
                let source = producer.unwrap_or(id);
                let chem_primary = producer
                    .and_then(|pid| self.objects.get(&pid))
                    .map(|o| {
                        crate::game_logic::host_scud_launcher::scud_uses_anthrax_primary(
                            &o.template_name,
                        ) || o.template_name.to_ascii_lowercase().contains("chem_")
                            && o.template_name.to_ascii_lowercase().contains("scudstorm")
                    })
                    .unwrap_or(false);
                if chem_primary {
                    // Toxin primary blast residual (use secondary damage as toxin splash).
                    self.apply_fuel_air_radius_damage(
                        id,
                        producer,
                        team,
                        target,
                        anthrax
                            .secondary_damage()
                            .max(anthrax.primary_damage() * 0.4),
                        SCUD_STORM_SECONDARY_RADIUS,
                        DamageType::Toxin,
                    );
                } else {
                    self.apply_fuel_air_radius_damage(
                        id,
                        producer,
                        team,
                        target,
                        anthrax.primary_damage(),
                        SCUD_STORM_PRIMARY_RADIUS,
                        DamageType::Explosive,
                    );
                    self.apply_fuel_air_radius_damage(
                        id,
                        producer,
                        team,
                        target,
                        anthrax.secondary_damage(),
                        SCUD_STORM_SECONDARY_RADIUS,
                        DamageType::Explosive,
                    );
                }
                // DeathFire OCL poison field residual (OCL_PoisonFieldLarge / upgraded).
                let _ = self
                    .special_power_strikes
                    .spawn_scud_poison_field_with_tier(
                        source, team, target, self.frame, 0, anthrax,
                    );
                self.scud_storm_missile_flight_reg.record_ground();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_CarpetBomb DeliverPayload residual.
    pub fn spawn_carpet_bomb_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::CarpetBombFactionTier,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightData;
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        // Edge spawn residual: offset opposite target.
        let dx = target.x - source_pos.x;
        let dz = target.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let edge = Vec3::new(
            source_pos.x - dx / dist * 350.0,
            150.0,
            source_pos.z - dz / dist * 350.0,
        );
        let transport = tier.transport();
        if !self.templates.contains_key(transport) {
            let mut t = ThingTemplate::new(transport);
            t.set_health(500.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(transport.to_string(), t);
        }
        let tid = self.create_object(transport, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.producer_id = Some(source_id);
            o.carpet_bomb_transport = Some(HostCarpetBombFlightData::start(edge, target, tier));
            o.set_orientation(dz.atan2(dx));
        }
        self.carpet_bomb_flight_reg.record_transport();
        self.carpet_bomb_flight_reg
            .schedule_drops(self.frame, source_id.0, target, tier);
        Some(tid)
    }

    pub fn update_carpet_bomb_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            CARPET_BOMB_DAMAGE, CARPET_BOMB_PAYLOAD_OBJECT, CARPET_BOMB_RADIUS,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        // Move transports.
        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.carpet_bomb_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.carpet_bomb_transport.as_mut() else {
                continue;
            };
            let (new_pos, vel, _over) = data.tick_transport(pos);
            drop(data);
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
        }

        // Drop due bombs.
        let due = self.carpet_bomb_flight_reg.take_due_drops(self.frame);
        if !due.is_empty() {
            if !self.templates.contains_key(CARPET_BOMB_PAYLOAD_OBJECT) {
                let mut t = ThingTemplate::new(CARPET_BOMB_PAYLOAD_OBJECT);
                t.set_health(100.0).add_kind_of(KindOf::Projectile);
                self.templates
                    .insert(CARPET_BOMB_PAYLOAD_OBJECT.to_string(), t);
            }
            for p in due {
                let team = self
                    .objects
                    .get(&ObjectId(p.source_id))
                    .map(|o| o.team)
                    .unwrap_or(Team::Neutral);
                // Drop from above target residual.
                let drop_pos = Vec3::new(p.target.x, 80.0, p.target.z);
                if let Some(bid) = self.create_object(CARPET_BOMB_PAYLOAD_OBJECT, team, drop_pos) {
                    if let Some(o) = self.objects.get_mut(&bid) {
                        o.producer_id = Some(ObjectId(p.source_id));
                        o.carpet_bomb_payload = true;
                        o.movement.velocity = Vec3::new(0.0, -15.0, 0.0);
                        let _ = o.set_smart_bomb_target(p.target);
                    }
                    self.carpet_bomb_flight_reg.record_drop();
                }
            }
        }

        // Fall payloads and detonate near ground.
        let bombs: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.carpet_bomb_payload && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in bombs {
            let (pos, producer, team) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let mut p = o.get_position();
                p.y += o.movement.velocity.y;
                o.set_position(p);
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                self.apply_fuel_air_radius_damage(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    CARPET_BOMB_DAMAGE,
                    CARPET_BOMB_RADIUS,
                    DamageType::Explosive,
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.carpet_bomb_flight_reg.record_impact();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_ArtilleryBarrage DeliverPayload residual.
    pub fn spawn_artillery_barrage_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightData;
        use crate::game_logic::special_power_strikes::ARTILLERY_BARRAGE_TRANSPORT;
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        let dx = target.x - source_pos.x;
        let dz = target.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let edge = Vec3::new(
            source_pos.x - dx / dist * 280.0,
            200.0,
            source_pos.z - dz / dist * 280.0,
        );
        if !self.templates.contains_key(ARTILLERY_BARRAGE_TRANSPORT) {
            let mut t = ThingTemplate::new(ARTILLERY_BARRAGE_TRANSPORT);
            t.set_health(800.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates
                .insert(ARTILLERY_BARRAGE_TRANSPORT.to_string(), t);
        }
        let tid = self.create_object(ARTILLERY_BARRAGE_TRANSPORT, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.producer_id = Some(source_id);
            o.artillery_barrage_transport =
                Some(HostArtilleryBarrageFlightData::start(edge, target, tier));
            o.set_orientation(dz.atan2(dx));
        }
        self.artillery_barrage_flight_reg.record_transport();
        self.artillery_barrage_flight_reg
            .schedule_drops(self.frame, source_id.0, target, tier);
        Some(tid)
    }

    pub fn update_artillery_barrage_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_RADIUS, ARTILLERY_BARRAGE_SHELL_OBJECT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.artillery_barrage_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.artillery_barrage_transport.as_mut() else {
                continue;
            };
            let (new_pos, vel, _over) = data.tick_transport(pos);
            let _ = data;
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
        }

        let due = self.artillery_barrage_flight_reg.take_due_drops(self.frame);
        if !due.is_empty() {
            if !self.templates.contains_key(ARTILLERY_BARRAGE_SHELL_OBJECT) {
                let mut t = ThingTemplate::new(ARTILLERY_BARRAGE_SHELL_OBJECT);
                t.set_health(50.0).add_kind_of(KindOf::Projectile);
                self.templates
                    .insert(ARTILLERY_BARRAGE_SHELL_OBJECT.to_string(), t);
            }
            for p in due {
                let team = self
                    .objects
                    .get(&ObjectId(p.source_id))
                    .map(|o| o.team)
                    .unwrap_or(Team::Neutral);
                let drop_pos = Vec3::new(p.target.x, 100.0, p.target.z);
                if let Some(sid) =
                    self.create_object(ARTILLERY_BARRAGE_SHELL_OBJECT, team, drop_pos)
                {
                    if let Some(o) = self.objects.get_mut(&sid) {
                        o.producer_id = Some(ObjectId(p.source_id));
                        o.artillery_barrage_shell = true;
                        o.movement.velocity = Vec3::new(0.0, -18.0, 0.0);
                        let _ = o.set_smart_bomb_target(p.target);
                    }
                    self.artillery_barrage_flight_reg.record_drop();
                }
            }
        }

        let shells: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.artillery_barrage_shell && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in shells {
            let (pos, producer, team) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let mut p = o.get_position();
                p.y += o.movement.velocity.y;
                o.set_position(p);
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                self.apply_fuel_air_radius_damage(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    ARTILLERY_BARRAGE_DAMAGE,
                    ARTILLERY_BARRAGE_RADIUS,
                    DamageType::Explosive,
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.artillery_barrage_flight_reg.record_impact();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_A10ThunderboltMissileStrike DeliverPayload residual.
    pub fn spawn_a10_strike_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightData;
        use crate::game_logic::special_power_strikes::A10_TRANSPORT;
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        let dx = target.x - source_pos.x;
        let dz = target.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let edge = Vec3::new(
            source_pos.x - dx / dist * 400.0,
            160.0,
            source_pos.z - dz / dist * 400.0,
        );
        if !self.templates.contains_key(A10_TRANSPORT) {
            let mut t = ThingTemplate::new(A10_TRANSPORT);
            t.set_health(600.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(A10_TRANSPORT.to_string(), t);
        }
        let tid = self.create_object(A10_TRANSPORT, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.producer_id = Some(source_id);
            o.a10_strike_transport = Some(HostA10StrikeFlightData::start(edge, target, tier));
            o.set_orientation(dz.atan2(dx));
        }
        self.a10_strike_flight_reg.record_transport();
        self.a10_strike_flight_reg
            .schedule_drops(self.frame, source_id.0, target, tier);
        Some(tid)
    }

    pub fn update_a10_strike_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            A10_MISSILE_PRIMARY_DAMAGE, A10_MISSILE_PRIMARY_RADIUS, A10_PAYLOAD_TEMPLATE,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.a10_strike_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.a10_strike_transport.as_mut() else {
                continue;
            };
            let (new_pos, vel, _over) = data.tick_transport(pos);
            let _ = data;
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
        }

        let due = self.a10_strike_flight_reg.take_due_drops(self.frame);
        if !due.is_empty() {
            if !self.templates.contains_key(A10_PAYLOAD_TEMPLATE) {
                let mut t = ThingTemplate::new(A10_PAYLOAD_TEMPLATE);
                t.set_health(40.0).add_kind_of(KindOf::Projectile);
                self.templates.insert(A10_PAYLOAD_TEMPLATE.to_string(), t);
            }
            for p in due {
                let team = self
                    .objects
                    .get(&ObjectId(p.source_id))
                    .map(|o| o.team)
                    .unwrap_or(Team::Neutral);
                let drop_pos = Vec3::new(p.target.x, 90.0, p.target.z);
                if let Some(mid) = self.create_object(A10_PAYLOAD_TEMPLATE, team, drop_pos) {
                    if let Some(o) = self.objects.get_mut(&mid) {
                        o.producer_id = Some(ObjectId(p.source_id));
                        o.a10_strike_missile = true;
                        o.movement.velocity = Vec3::new(0.0, -20.0, 0.0);
                        let _ = o.set_smart_bomb_target(p.target);
                    }
                    self.a10_strike_flight_reg.record_drop();
                }
            }
        }

        let missiles: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.a10_strike_missile && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in missiles {
            let (pos, producer, team) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let mut p = o.get_position();
                p.y += o.movement.velocity.y;
                o.set_position(p);
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                self.apply_fuel_air_radius_damage(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    A10_MISSILE_PRIMARY_DAMAGE,
                    A10_MISSILE_PRIMARY_RADIUS,
                    DamageType::Explosive,
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.a10_strike_flight_reg.record_impact();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_LeafletDrop AmericaJetB52 + LeafletContainer residual.
    pub fn spawn_leaflet_b52_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_leaflet_drop::{LEAFLET_CONTAINER_OBJECT, LEAFLET_TRANSPORT};
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        let dx = target.x - source_pos.x;
        let dz = target.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let edge = Vec3::new(
            source_pos.x - dx / dist * 320.0,
            150.0,
            source_pos.z - dz / dist * 320.0,
        );
        if !self.templates.contains_key(LEAFLET_TRANSPORT) {
            let mut t = ThingTemplate::new(LEAFLET_TRANSPORT);
            t.set_health(500.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(LEAFLET_TRANSPORT.to_string(), t);
        }
        // Ensure container template for drop residual.
        if !self.templates.contains_key(LEAFLET_CONTAINER_OBJECT) {
            let mut t = ThingTemplate::new(LEAFLET_CONTAINER_OBJECT);
            t.set_health(100.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(LEAFLET_CONTAINER_OBJECT.to_string(), t);
        }
        let tid = self.create_object(LEAFLET_TRANSPORT, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.producer_id = Some(source_id);
            o.leaflet_transport_target = Some(target);
            o.set_orientation(dz.atan2(dx));
        }
        self.host_leaflet_drops.transports_spawned =
            self.host_leaflet_drops.transports_spawned.saturating_add(1);
        Some(tid)
    }

    pub fn update_leaflet_b52_flights(&mut self) {
        use crate::game_logic::host_leaflet_drop::{
            LEAFLET_CONTAINER_OBJECT, LEAFLET_DELIVERY_DISTANCE,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.leaflet_transport_target.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(ObjectId, Team, Vec3, ObjectId)> = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(target) = o.leaflet_transport_target else {
                continue;
            };
            let pos = o.get_position();
            let dx = target.x - pos.x;
            let dz = target.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 20.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(140.0);
            if dist > 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                o.set_position(new_pos);
                o.movement.velocity = new_pos - pos;
                o.set_orientation(dz.atan2(dx));
            }
            if dist <= LEAFLET_DELIVERY_DISTANCE * 0.5 {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                o.leaflet_transport_target = None; // drop once
                drops.push((id, team, target, producer));
            }
        }
        for (_tid, team, target, producer) in drops {
            let drop_pos = Vec3::new(target.x, 80.0, target.z);
            if let Some(cid) = self.create_object(LEAFLET_CONTAINER_OBJECT, team, drop_pos) {
                if let Some(o) = self.objects.get_mut(&cid) {
                    o.producer_id = Some(producer);
                    o.leaflet_container = true;
                    o.movement.velocity = Vec3::new(0.0, -12.0, 0.0);
                    let _ = o.set_smart_bomb_target(target);
                }
                self.host_leaflet_drops.containers_dropped =
                    self.host_leaflet_drops.containers_dropped.saturating_add(1);
            }
        }

        // Fall containers; ground arrival is visual residual (disable timer separate).
        let containers: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.leaflet_container && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in containers {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let mut p = o.get_position();
            p.y += o.movement.velocity.y;
            o.set_position(p);
            if p.y <= 5.0 {
                // LeafletParticles1 residual cue.
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    p,
                    self.frame,
                    Some(id),
                    None,
                );
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_Paradrop AmericaJetCargoPlane residual.
    pub fn spawn_paradrop_cargo_plane(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_paradrop::{PARADROP_PARACHUTE_CONTAINER, PARADROP_TRANSPORT};
        use crate::game_logic::{KindOf, ThingTemplate};

        let (team, source_owner_player_id) = {
            let source = self.objects.get(&source_id)?;
            let owner_player_id = if source.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(source)?)
            } else {
                None
            };
            (source.team, owner_player_id)
        };
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        let dx = target.x - source_pos.x;
        let dz = target.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let edge = Vec3::new(
            source_pos.x - dx / dist * 380.0,
            160.0,
            source_pos.z - dz / dist * 380.0,
        );
        if !self.templates.contains_key(PARADROP_TRANSPORT) {
            let mut t = ThingTemplate::new(PARADROP_TRANSPORT);
            t.set_health(800.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(PARADROP_TRANSPORT.to_string(), t);
        }
        if !self.templates.contains_key(PARADROP_PARACHUTE_CONTAINER) {
            let mut t = ThingTemplate::new(PARADROP_PARACHUTE_CONTAINER);
            t.set_health(50.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(PARADROP_PARACHUTE_CONTAINER.to_string(), t);
        }
        let tid = self.create_object_for_owner_or_team(
            PARADROP_TRANSPORT,
            team,
            source_owner_player_id,
            edge,
        )?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.producer_id = Some(source_id);
            o.paradrop_transport_target = Some(target);
            o.set_orientation(dz.atan2(dx));
        }
        self.host_paradrops.transports_spawned =
            self.host_paradrops.transports_spawned.saturating_add(1);
        Some(tid)
    }

    pub fn update_paradrop_cargo_planes(&mut self) {
        use crate::game_logic::host_paradrop::{
            PARADROP_DELIVERY_DISTANCE, PARADROP_PARACHUTE_CONTAINER,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.paradrop_transport_target.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(Team, Option<u32>, Vec3, ObjectId)> = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(target) = o.paradrop_transport_target else {
                continue;
            };
            let pos = o.get_position();
            let dx = target.x - pos.x;
            let dz = target.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 18.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(150.0);
            if dist > 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                o.set_position(new_pos);
                o.movement.velocity = new_pos - pos;
                o.set_orientation(dz.atan2(dx));
            }
            if dist <= PARADROP_DELIVERY_DISTANCE {
                let team = o.team;
                let owner_player_id = o.owner_player_id;
                let producer = o.producer_id.unwrap_or(id);
                o.paradrop_transport_target = None;
                drops.push((team, owner_player_id, target, producer));
            }
        }
        for (team, owner_player_id, target, producer) in drops {
            // Drop a residual parachute marker over the LZ (infantry still from host_paradrops).
            let drop_pos = Vec3::new(target.x, 100.0, target.z);
            if let Some(pid) = self.create_object_for_owner_or_team(
                PARADROP_PARACHUTE_CONTAINER,
                team,
                owner_player_id,
                drop_pos,
            ) {
                if let Some(o) = self.objects.get_mut(&pid) {
                    o.producer_id = Some(producer);
                    o.paradrop_parachute = true;
                    o.movement.velocity = Vec3::new(0.0, -8.0, 0.0);
                    let _ = o.set_smart_bomb_target(target);
                    let _ = o.apply_eject_parachuting();
                }
                self.host_paradrops.parachutes_dropped =
                    self.host_paradrops.parachutes_dropped.saturating_add(1);
            }
        }

        let chutes: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.paradrop_parachute && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in chutes {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let mut p = o.get_position();
            p.y += o.movement.velocity.y;
            // Slow parachute residual.
            if o.movement.velocity.y < -2.0 {
                o.movement.velocity.y = -2.5;
            }
            o.set_position(p);
            if p.y <= 5.0 {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_DaisyCutter / SUPERWEAPON_MOAB jet + bomb residual.
    pub fn spawn_daisy_cutter_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_daisy_cutter_flight::HostDaisyCutterFlightData;
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        let dx = target.x - source_pos.x;
        let dz = target.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let edge = Vec3::new(
            source_pos.x - dx / dist * 360.0,
            160.0,
            source_pos.z - dz / dist * 360.0,
        );
        let transport = tier.transport();
        let bomb = tier.bomb();
        if !self.templates.contains_key(transport) {
            let mut t = ThingTemplate::new(transport);
            t.set_health(500.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(transport.to_string(), t);
        }
        if !self.templates.contains_key(bomb) {
            let mut t = ThingTemplate::new(bomb);
            t.set_health(100.0).add_kind_of(KindOf::Projectile);
            self.templates.insert(bomb.to_string(), t);
        }
        let tid = self.create_object(transport, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.producer_id = Some(source_id);
            o.daisy_cutter_transport = Some(HostDaisyCutterFlightData::start(edge, target, tier));
            o.set_orientation(dz.atan2(dx));
        }
        self.daisy_cutter_flight_reg.record_transport(tier);
        Some(tid)
    }

    pub fn update_daisy_cutter_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.daisy_cutter_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(Team, Vec3, ObjectId, DaisyFlightPayloadTier)> = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.daisy_cutter_transport.as_mut() else {
                continue;
            };
            let (new_pos, vel, over) = data.tick_transport(pos);
            let target = data.target;
            let tier = data.tier;
            let _ = data;
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if over {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                o.daisy_cutter_transport = None;
                drops.push((team, target, producer, tier));
            }
        }
        for (team, target, producer, tier) in drops {
            let bomb = tier.bomb();
            let drop_pos = Vec3::new(target.x, 90.0, target.z);
            if let Some(bid) = self.create_object(bomb, team, drop_pos) {
                if let Some(o) = self.objects.get_mut(&bid) {
                    o.producer_id = Some(producer);
                    o.daisy_cutter_bomb = true;
                    // Stash tier via MOAB name residual for detonation path.
                    if tier == DaisyFlightPayloadTier::Moab {
                        o.template_name = bomb.to_string();
                    }
                    o.movement.velocity = Vec3::new(0.0, -16.0, 0.0);
                    let _ = o.set_smart_bomb_target(target);
                }
                self.daisy_cutter_flight_reg.record_drop();
            }
        }

        let bombs: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.daisy_cutter_bomb && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in bombs {
            let (pos, producer, team, is_moab) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let mut p = o.get_position();
                p.y += o.movement.velocity.y;
                o.set_position(p);
                let is_moab = o.template_name == "MOAB" || o.template_name.contains("MOAB");
                (p, o.producer_id, o.team, is_moab)
            };
            if pos.y <= 5.0 {
                let tier = if is_moab {
                    DaisyFlightPayloadTier::Moab
                } else {
                    DaisyFlightPayloadTier::DaisyCutter
                };
                self.apply_fuel_air_radius_damage(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    tier.primary_damage(),
                    tier.primary_radius(),
                    DamageType::Explosive,
                );
                if let Some(o) = self.objects.get_mut(&id) {
                    o.ensure_fuel_air_gas_slow_death(self.frame);
                    if o.fuel_air_gas_slow_death.is_some() {
                        self.fuel_air_gas_reg.record_install();
                    }
                }
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.daisy_cutter_flight_reg.record_detonation();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_AnthraxBomb GLAJetCargoPlane + AnthraxBomb residual.
    pub fn spawn_anthrax_bomb_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_anthrax_bomb_flight::{
            AnthraxBombPayloadTier, HostAnthraxBombFlightData, ANTHRAX_TRANSPORT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        let dx = target.x - source_pos.x;
        let dz = target.z - source_pos.z;
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let edge = Vec3::new(
            source_pos.x - dx / dist * 340.0,
            150.0,
            source_pos.z - dz / dist * 340.0,
        );
        if !self.templates.contains_key(ANTHRAX_TRANSPORT) {
            let mut t = ThingTemplate::new(ANTHRAX_TRANSPORT);
            t.set_health(600.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(ANTHRAX_TRANSPORT.to_string(), t);
        }
        let tier = AnthraxBombPayloadTier::Base;
        let bomb = tier.bomb();
        if !self.templates.contains_key(bomb) {
            let mut t = ThingTemplate::new(bomb);
            t.set_health(80.0).add_kind_of(KindOf::Projectile);
            self.templates.insert(bomb.to_string(), t);
        }
        let tid = self.create_object(ANTHRAX_TRANSPORT, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.producer_id = Some(source_id);
            o.anthrax_bomb_transport = Some(HostAnthraxBombFlightData::start(edge, target, tier));
            o.set_orientation(dz.atan2(dx));
        }
        self.anthrax_bomb_flight_reg.record_transport();
        Some(tid)
    }
}
