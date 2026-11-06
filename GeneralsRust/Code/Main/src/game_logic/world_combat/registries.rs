//! Host combat `impl GameLogic` — `registries`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

mod container_evacuation_and_rappel;
mod strike_missile_flights;
mod support_flights;
#[cfg(test)]
mod garrison_evac_door_tests;

mod garrison_exit;

#[path = "a10_delivery.rs"]
mod a10_delivery;

#[cfg(test)]
#[path = "a10_delivery_tests.rs"]
mod a10_delivery_tests;

#[cfg(test)]
#[path = "rappel_kill_owner_tests.rs"]
mod rappel_kill_owner_tests;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransportEvacuationMode {
    Command,
    /// ChinookEvacuateState::onEnter calls removeAllContained(FALSE), without
    /// the exit-busy polling or stagger used by the ordinary exit command.
    ChinookFlight,
}

fn garrison_evac_rand(seed: u32, lo: f32, hi: f32) -> f32 {
    let t = (seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223) >> 8) as f32
        / ((1u32 << 24) as f32);
    lo + (hi - lo) * t
}

/// C++ GarrisonContain::exitObjectViaDoor EVAC_TO_LEFT / EVAC_TO_RIGHT.
fn garrison_evac_side_points(
    origin: glam::Vec3,
    yaw: f32,
    major: f32,
    minor: f32,
    evac: u8,
    seed: u32,
) -> (glam::Vec3, glam::Vec3) {
    let scalar = if evac == 1 { 1.0 } else { -1.0 };
    let door_x = garrison_evac_rand(seed, -major / 4.0, major / 4.0);
    let door_y = garrison_evac_rand(seed.wrapping_add(1), minor / 2.0, minor * 2.0) * scalar;
    let walk_x = garrison_evac_rand(seed.wrapping_add(2), -major, major);
    let walk_y = minor * 10.0 * scalar;
    let (sin, cos) = yaw.sin_cos();
    let start = glam::Vec3::new(
        origin.x + door_x * cos - door_y * sin,
        origin.y,
        origin.z + door_x * sin + door_y * cos,
    );
    let end = glam::Vec3::new(
        origin.x + walk_x * cos - walk_y * sin,
        origin.y,
        origin.z + walk_x * sin + walk_y * cos,
    );
    (start, end)
}

/// C++ GarrisonContain::exitObjectViaDoor cliff-bunker start walk.
/// If the building origin is not standable, try front then back along facing.
fn garrison_evac_cliff_start(
    origin: glam::Vec3,
    yaw: f32,
    major: f32,
    grid: &super::super::pathfinding::PathfindingGrid,
) -> glam::Vec3 {
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    if grid.width() <= 0 || grid.height() <= 0 {
        return origin;
    }
    let standable = |pos: glam::Vec3| {
        let cell = grid.world_to_grid(pos);
        grid.is_valid_pos(cell) && grid.cell_passable_for(cell, SURFACE_GROUND, false)
    };
    if standable(origin) {
        return origin;
    }
    let (sin, cos) = yaw.sin_cos();
    let mut front = origin;
    front.x -= major * cos;
    front.z -= major * sin;
    if standable(front) {
        return front;
    }
    let mut back = origin;
    back.x += major * cos;
    back.z += major * sin;
    if standable(back) {
        return back;
    }
    origin
}

/// C++ EVAC_BURST_FROM_CENTER: end starts at start, then
/// `Pathfinder::adjustToPossibleDestination` so the occupant walks out.
fn garrison_evac_burst_end(
    start: glam::Vec3,
    major: f32,
    grid: &super::super::pathfinding::PathfindingGrid,
    ground: &PathfindingGround<'_>,
) -> glam::Vec3 {
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    let cell_size = grid.grid_size().max(1.0);
    if grid.width() > 0 && grid.height() > 0 {
        let seed = grid.world_to_grid(start);
        if let Some(adj) = grid.adjust_destination(seed, SURFACE_GROUND, false, 400, ground) {
            let world = grid.grid_to_world(adj, ground);
            let dx = world.x - start.x;
            let dz = world.z - start.z;
            if dx * dx + dz * dz > 0.25 {
                return glam::Vec3::new(world.x, start.y, world.z);
            }
        }
    }
    // Empty / unstamped grid: first C++ spiral step is +X. Still leave the
    // enclosing footprint so burst does not idle at the origin.
    glam::Vec3::new(start.x + major.max(cell_size), start.y, start.z)
}

#[cfg(test)]
pub(in super::super) fn garrison_evac_side_points_for_test(
    origin: glam::Vec3,
    yaw: f32,
    major: f32,
    minor: f32,
    evac: u8,
    seed: u32,
) -> (glam::Vec3, glam::Vec3) {
    garrison_evac_side_points(origin, yaw, major, minor, evac, seed)
}

impl GameLogic {
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.is_paused = paused;
        // C++ GameLogic::setGamePaused (GameLogic.cpp:4164-4222) default
        // pauseMusic=TRUE: TheAudio->pauseAudio / resumeAudio.
        gamelogic::helpers::TheGameLogic::set_game_paused(paused, true);
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

    /// C++ GarrisonContain::onBodyDamageStateChange → orderAllPassengersToExit.
    pub fn flush_pending_garrison_really_damaged_ejects(&mut self) {
        self.flush_pending_garrison_really_damaged_ejects_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
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
            && crate::game_logic::host_spectre_gunship_update::honesty_spectre_gunship_update_residual_ok()
    }

    pub fn honesty_smart_bomb_target_homing_ok(&self) -> bool {
        self.smart_bomb_target_homing_reg.honesty_host_path_ok()
            && crate::game_logic::host_smart_bomb_target_homing::honesty_smart_bomb_target_homing_residual_ok()
    }

}

#[cfg(feature = "game_client")]
impl crate::game_logic::GameLogic {
    pub(crate) fn with_native_particle_object_lifecycle<R>(
        &mut self,
        operation: impl FnOnce(
            &mut game_client::effects::particle_manager::NativeParticleObjectLifecycle<'_>,
            &mut crate::command_system::ReplayPendingState,
            Option<&mut game_engine::common::thing::module_factory::ModuleFactory>,
        ) -> R,
    ) -> R {
        let player = self.local_player_id();
        let shroud = self.world_services.shroud();
        let objects = &self.objects;
        let projectiles = self.combat_system.get_projectiles();
        let terrain = self.world_services.terrain();
        let ground = |position: glam::Vec3| {
            let terrain = terrain.read().unwrap_or_else(|error| error.into_inner());
            Some(terrain.get_ground_height(position.x, position.y, None))
        };
        let particles = &mut self.combat_particles;
        let exists =
            |id| objects.contains_key(&ObjectId(id)) || projectiles.contains_key(&ObjectId(id));
        let pose = |id| {
            objects
                .get(&ObjectId(id))
                .map(|object| Self::observe_host_fx_object(object, player, shroud))
                .or_else(|| {
                    projectiles.get(&ObjectId(id)).map(|projectile| {
                        Self::observe_host_fx_projectile(projectile, player, shroud)
                    })
                })
        };
        let mut lost =
            |manager: &game_client::effects::particle_manager::ParticleSystemOwner, id, object| {
                particles.admit_client_object_attachment_lost(manager, id, object);
            };
        let mut lifecycle = game_client::effects::particle_manager::NativeParticleObjectLifecycle {
            object_is_live: &exists,
            object_pose: &pose,
            ground_height: &ground,
            attached_object_lost: &mut lost,
        };
        operation(
            &mut lifecycle,
            &mut self.replay_pending,
            self.receiving_module_content
                .as_mut()
                .map(|content| content.module_factory_mut()),
        )
    }
}
