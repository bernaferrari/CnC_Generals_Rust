//! Host combat `impl GameLogic` — `registries`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

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
) -> glam::Vec3 {
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    let cell_size = grid.grid_size().max(1.0);
    if grid.width() > 0 && grid.height() > 0 {
        let seed = grid.world_to_grid(start);
        if let Some(adj) = grid.adjust_destination(seed, SURFACE_GROUND, false, 400) {
            let world = grid.grid_to_world(adj);
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

    /// C++ GarrisonContain::onRemoving Patch 1.01 — Fire Base (non-enclosing)
    /// occupants snap host Y (C++ Z) to ground so they do not leave at STATION
    /// bone altitude. Terrain sample wins; else the container's ground Y.
    fn snap_garrison_exit_occupant_to_ground(&mut self, unit_id: ObjectId, container_id: ObjectId) {
        let Some(pos) = self.objects.get(&unit_id).map(|p| p.get_position()) else {
            return;
        };
        let Some(ground_y) = self
            .terrain_height_at(pos)
            .or_else(|| self.objects.get(&container_id).map(|c| c.get_position().y))
        else {
            return;
        };
        if let Some(p) = self.objects.get_mut(&unit_id) {
            let mut next = p.get_position();
            next.y = ground_y;
            p.set_position(next);
        }
    }

    /// C++ GarrisonContain::exitObjectViaDoor — EVAC_TO_LEFT / RIGHT / BURST.
    /// Occupant should already be removed from the container roster.
    pub fn garrison_exit_occupant_via_door(
        &mut self,
        unit_id: ObjectId,
        container_id: ObjectId,
    ) -> bool {
        let Some(container) = self.objects.get(&container_id) else {
            return false;
        };
        if !container.is_garrison_contain() {
            return false;
        }
        let container_name = container.name.clone();
        if let Some(disp) = gamelogic::object::contain::named_evac_disposition(&container_name) {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.set_garrison_evac_disposition(disp as u8);
            }
        }
        let Some(container) = self.objects.get(&container_id) else {
            return false;
        };
        let evac = container.garrison_evac_disposition();
        let enclosing = container.is_enclosing_garrison_container();
        let geom = container.thing.template.geometry_info;
        let major = if geom.authored {
            geom.major_radius.max(1.0)
        } else {
            20.0
        };
        let minor = if geom.authored {
            geom.minor_radius.max(1.0)
        } else {
            20.0
        };
        let yaw = container.get_orientation();
        let building_pos = container.get_position();
        let seed = unit_id.0.wrapping_add(self.frame);

        if evac == 1 || evac == 2 {
            let (start, end) =
                garrison_evac_side_points(building_pos, yaw, major, minor, evac, seed);
            let Some(p) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            p.set_contained_by(None);
            p.target = None;
            p.set_position(start);
            p.set_orientation(yaw);
            p.set_destination(end);
            p.set_ai_state(AIState::Moving);
            p.status.moving = true;
        } else {
            let mut start =
                garrison_evac_cliff_start(building_pos, yaw, major, &self.pathfinding_system.grid);
            if enclosing {
                if let Some(gh) = self.terrain_height_at(start) {
                    start.y = gh;
                }
            }
            let end = garrison_evac_burst_end(start, major, &self.pathfinding_system.grid);
            let Some(p) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            p.set_contained_by(None);
            p.target = None;
            if enclosing {
                p.set_position(start);
            }
            p.set_orientation(yaw);
            p.set_destination(end);
            p.set_ai_state(AIState::Moving);
            p.status.moving = true;
        }
        if !enclosing {
            // C++ GarrisonContain::onRemoving Patch 1.01: Fire Base station
            // occupants snap Z to ground so sell/evac does not leave them
            // floating at STATION bone height.
            self.snap_garrison_exit_occupant_to_ground(unit_id, container_id);
        }
        // C++ GarrisonContain::onRemoving setSafeOcclusionFrame(frame + OcclusionDelay).
        if let Some(p) = self.objects.get_mut(&unit_id) {
            p.stamp_safe_occlusion_frame(self.frame);
        }

        if crate::gameworld_shadow::gameworld_movement_authority_live() {
            if let Some(p) = self.objects.get(&unit_id) {
                let pos = p.get_position();
                crate::game_logic::host_move_log::record(unit_id, Some([pos.x, pos.y, pos.z]));
            }
        }
        // C++ OpenContain::removeFromContain doUnloadSound — leftover TheAudio.
        self.play_container_exit_sound(container_id);
        self.reset_rider_mood_check_on_exit(unit_id);
        self.play_container_removing_template_sounds(container_id, unit_id);
        // C++ GarrisonContain::exitObjectViaDoor ends in recalcApparentControllingPlayer.
        self.recalc_garrison_apparent_controller(container_id);
        true
    }

    /// C++ GarrisonContain::onBodyDamageStateChange → orderAllPassengersToExit.
    pub fn flush_pending_garrison_really_damaged_ejects(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, obj)| obj.status.pending_garrison_really_damaged_eject)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                let _ = obj.take_pending_garrison_really_damaged_eject();
            }
            let _ = self.evacuate_container_now(id, false);
        }
    }

    /// C++ move-to-and-evacuate arrival residual: dump all occupants near container.
    /// When `and_exit`, mark the transport sold/destroyed after unload (script exit residual).
    pub fn evacuate_container_now(&mut self, container_id: ObjectId, and_exit: bool) -> bool {
        let Some((alive, is_chinook_dropper, subdued)) = self.objects.get(&container_id).map(|c| {
            (
                c.is_alive(),
                c.is_combat_chinook_style_container() && c.chinook_ai.is_some(),
                c.is_subdued_disabled(),
            )
        }) else {
            return false;
        };
        if !alive {
            return false;
        }
        // C++ AIUpdateInterface::privateEvacuate / privateExit:
        // DISABLED_SUBDUED holds the doors shut while a Microwave cooks.
        if subdued {
            return false;
        }
        // C++ privateEvacuate: markAllPassengersDetected before dump.
        self.mark_all_passengers_detected(container_id);

        if is_chinook_dropper {
            if let Some(c) = self.objects.get_mut(&container_id) {
                let p = c.get_position();
                if let Some(ai) = c.chinook_ai.as_mut() {
                    ai.pos = [p.x, p.z, p.y];
                    if ai.state
                        == crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToCombatDrop
                    {
                        ai.arrive_for_combat_drop();
                    }
                }
            }
            let doing_drop = self.objects.get(&container_id).is_some_and(|c| {
                c.chinook_ai.as_ref().is_some_and(|ai| {
                    ai.flight_status
                        == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::DoingCombatDrop
                })
            });
            if doing_drop {
                return self.combat_drop_rappel_unload(container_id, and_exit);
            }
            let any_rappeller = self.objects.get(&container_id).is_some_and(|c| {
                c.contained_units().iter().any(|pid| {
                    self.objects.get(pid).is_some_and(|p| {
                        crate::game_logic::host_combat_chinook::HostChinookAI::passenger_kind_can_rappel(
                            p.is_kind_of(KindOf::Infantry),
                        )
                    })
                })
            });
            let wait = self.objects.get(&container_id).is_some_and(|c| {
                c.chinook_ai.as_ref().is_some_and(|ai| {
                    ai.ai_free_to_exit(any_rappeller)
                        != crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
                })
            });
            if wait {
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

        let Some(container) = self.objects.get(&container_id) else {
            return false;
        };
        let is_garrison = container.is_garrison_contain();
        let is_tunnel = container.is_tunnel_network_style_container();
        let is_cave = container.is_cave_style_container();
        let container_name = container.name.clone();
        if is_garrison {
            if let Some(disp) = gamelogic::object::contain::named_evac_disposition(&container_name)
            {
                if let Some(c) = self.objects.get_mut(&container_id) {
                    c.set_garrison_evac_disposition(disp as u8);
                }
            }
        }
        let Some(container) = self.objects.get(&container_id) else {
            return false;
        };
        let evac = if is_garrison {
            container.garrison_evac_disposition()
        } else {
            0
        };
        let enclosing = container.is_enclosing_garrison_container();
        let geom = container.thing.template.geometry_info;
        let major = if geom.authored {
            geom.major_radius.max(1.0)
        } else {
            20.0
        };
        let minor = if geom.authored {
            geom.minor_radius.max(1.0)
        } else {
            20.0
        };
        let yaw = container.get_orientation();
        let building_pos = container.get_position();
        let mut passengers: Vec<ObjectId> = container.contained_units();
        if is_cave {
            let idx = container.cave_index;
            for uid in self.cave_system.contained_for_index(idx) {
                if !passengers.contains(&uid) {
                    passengers.push(uid);
                }
            }
        }
        if passengers.is_empty() && !and_exit {
            // Still clear pending flags.
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = false;
                c.pending_exit_after_evacuate = false;
                c.pending_stream_exit = false;
            }

            return false;
        }
        let uses_exit_busy = !is_garrison
            && self
                .objects
                .get(&container_id)
                .is_some_and(|c| c.uses_transport_contain_exit_busy());
        if uses_exit_busy
            && self
                .objects
                .get(&container_id)
                .is_some_and(|c| c.is_transport_exit_busy(self.frame))
        {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = true;
                c.pending_exit_after_evacuate = and_exit;
            }
            return false;
        }
        let stagger = uses_exit_busy
            && self
                .objects
                .get(&container_id)
                .is_some_and(|c| c.transport_exit_delay_frames() > 0);
        let more_remain = stagger && passengers.len() > 1;
        if stagger && passengers.len() > 1 {
            passengers.truncate(1);
        }

        // C++ burst start is cliff-adjusted building origin; dest is then
        // adjustToPossibleDestination so occupants walk out (not Idle at center).
        let (burst_start, burst_end) = if is_garrison && evac != 1 && evac != 2 {
            let mut start =
                garrison_evac_cliff_start(building_pos, yaw, major, &self.pathfinding_system.grid);
            if enclosing {
                if let Some(gh) = self.terrain_height_at(start) {
                    start.y = gh;
                }
            }
            let end = garrison_evac_burst_end(start, major, &self.pathfinding_system.grid);
            (Some(start), Some(end))
        } else {
            (None, None)
        };

        let mut any = false;
        let mut packing_hackers: Vec<ObjectId> = Vec::new();
        for (i, pid) in passengers.iter().enumerate() {
            let mut walked_transport = false;
            // Remove from container first.
            if let Some(c) = self.objects.get_mut(&container_id) {
                let _ = c.remove_occupant(*pid);
            }
            if let Some(p) = self.objects.get_mut(pid) {
                // Wave 201: host_contain_log last-writer (do not bypass set_contained_by).
                p.set_contained_by(None);
                p.target = None;
                if is_garrison && (evac == 1 || evac == 2) {
                    let seed = pid.0.wrapping_add(i as u32).wrapping_add(self.frame);
                    let (start, end) =
                        garrison_evac_side_points(building_pos, yaw, major, minor, evac, seed);
                    p.set_position(start);
                    p.set_destination(end);
                    p.set_ai_state(AIState::Moving);
                    p.status.moving = true;
                } else if is_garrison {
                    // C++ EVAC_BURST_FROM_CENTER: enclosing occupants snap to
                    // start (Y to ground), then aiFollowPath to adjusted dest.
                    if enclosing {
                        if let Some(start) = burst_start {
                            p.set_position(start);
                        }
                    }
                    p.set_orientation(yaw);
                    if let Some(end) = burst_end {
                        p.set_destination(end);
                    }
                    p.set_ai_state(AIState::Moving);
                    p.status.moving = true;
                } else {
                    // C++ OpenContain::exitObjectViaDoor — walk ExitStart/End.
                    // Do not teleport to an Idle 8-unit ring.
                    walked_transport = true;
                }
                if is_garrison || is_tunnel || is_cave {
                    // C++ GarrisonContain/TunnelContain/CaveContain::onRemoving.
                    p.stamp_safe_occlusion_frame(self.frame);
                }

                // C++ HackInternetAIUpdate::aiDoCommand (HackInternetAIUpdate.cpp:105)
                // PACKING on evacuate/exit. Cash must stop immediately.
                if p.thing.template.hack_internet_ai_update.is_some() {
                    packing_hackers.push(*pid);
                }
                any = true;
            }
            if is_garrison && !enclosing {
                self.snap_garrison_exit_occupant_to_ground(*pid, container_id);
            }
            if walked_transport {
                self.walk_unit_via_open_contain_exit(*pid, container_id);
            } else {
                self.reset_rider_mood_check_on_exit(*pid);
                self.play_container_removing_template_sounds(container_id, *pid);
            }
            if is_cave {
                // Leftover CaveContain::onRemoving: record_exit + LastEmpty team revert.
                let _ = self.exit_cave_unit(*pid, container_id);
            }
            self.record_transport_residual_unload();
        }
        if is_garrison {
            // C++ GarrisonContain::removeAllContained → recalcApparentControllingPlayer.
            self.recalc_garrison_apparent_controller(container_id);
        }
        if any {
            // C++ OpenContain::doUnloadSound once per frame per container.
            self.play_container_exit_sound(container_id);
        }
        for hid in packing_hackers {
            self.hacker_income.stop_hacking(hid);
        }

        if let Some(c) = self.objects.get_mut(&container_id) {
            if uses_exit_busy {
                let delay = c.transport_exit_delay_frames();
                c.frame_exit_not_busy = self.frame.saturating_add(delay);
            }
            c.pending_evacuate_on_stop = more_remain;
            c.pending_stream_exit = more_remain;
            c.pending_exit_after_evacuate = and_exit && !more_remain;

            let contained_count = if more_remain {
                c.contained_units().len() as u32
            } else {
                0
            };
            if let Some(ai) = c.chinook_ai.as_mut() {
                ai.contained_count = contained_count;
                ai.wanting_enter_or_exit = more_remain;
                if and_exit && !more_remain {
                    // Keep spawn `original_pos`; do not re-issue evac at the LZ.
                    ai.begin_takeoff_and_exit();
                    return any;
                }
            }
        }

        if and_exit && !more_remain {
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

    /// C++ `ChinookCombatDropState::update`: rope delay + `aiRappelInto` at `m_rappelSpeed`.
    fn combat_drop_rappel_unload(&mut self, container_id: ObjectId, and_exit: bool) -> bool {
        let now = self.frame;
        let hover = match self.objects.get(&container_id) {
            Some(c) if c.is_alive() => c.get_position(),
            _ => return false,
        };
        let passengers: Vec<ObjectId> = self
            .objects
            .get(&container_id)
            .map(|c| c.contained_units())
            .unwrap_or_default();
        let rappeller = passengers.iter().copied().find(|pid| {
            self.objects.get(pid).is_some_and(|p| {
                crate::game_logic::host_combat_chinook::HostChinookAI::passenger_kind_can_rappel(
                    p.is_kind_of(KindOf::Infantry),
                )
            })
        });
        let Some(pid) = rappeller else {
            if let Some(c) = self.objects.get_mut(&container_id) {
                let remaining = c.contained_units().len() as u32;
                if let Some(ai) = c.chinook_ai.as_mut() {
                    ai.finish_combat_drop();
                    ai.contained_count = remaining;
                    ai.wanting_enter_or_exit = remaining > 0;
                    if remaining > 0 {
                        ai.tick_idle_auto_land();
                    }
                }
                c.pending_evacuate_on_stop = remaining > 0;
                c.pending_exit_after_evacuate = and_exit && remaining == 0;
            }
            return false;
        };
        let can_release = self.objects.get(&container_id).is_some_and(|c| {
            c.chinook_ai
                .as_ref()
                .is_some_and(|ai| ai.can_release_rappeller(now))
        });
        if !can_release {
            if let Some(c) = self.objects.get_mut(&container_id) {
                c.pending_evacuate_on_stop = true;
                c.pending_exit_after_evacuate = and_exit;
            }
            return false;
        }
        let rappel_speed = self
            .objects
            .get(&container_id)
            .and_then(|c| c.chinook_ai.as_ref())
            .map(|ai| ai.apply_rappel_speed())
            .unwrap_or(crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_RAPPEL_SPEED);
        // C++ `aiRappelInto(getMachineGoalObject(), *getMachineGoalPosition())`.
        let drop_target = self.objects.get(&container_id).and_then(|c| {
            c.target.or_else(|| {
                c.chinook_ai
                    .as_ref()
                    .and_then(|ai| ai.combat_drop_target)
                    .map(ObjectId)
            })
        });
        let (target_is_bldg, dest_y) = self.combat_drop_rappel_dest(hover, drop_target);

        if let Some(c) = self.objects.get_mut(&container_id) {
            let _ = c.remove_occupant(pid);
        }
        if let Some(p) = self.objects.get_mut(&pid) {
            p.set_contained_by(None);
            p.target = drop_target;
            p.set_position(hover);
            let dest = glam::Vec3::new(hover.x, dest_y, hover.z);
            p.movement.path.clear();
            p.movement.current_path_index = 0;
            p.movement.target_position = Some(dest);
            p.movement.max_speed = rappel_speed;
            p.begin_rappel(dest_y, target_is_bldg, drop_target);
            p.set_ai_state(AIState::Moving);
            p.status.moving = true;
        }
        self.record_transport_residual_unload();
        let more = self.objects.get(&container_id).is_some_and(|c| {
            c.contained_units().iter().any(|id| {
                self.objects.get(id).is_some_and(|p| {
                    crate::game_logic::host_combat_chinook::HostChinookAI::passenger_kind_can_rappel(
                        p.is_kind_of(KindOf::Infantry),
                    )
                })
            })
        });
        if let Some(c) = self.objects.get_mut(&container_id) {
            let remaining = c.contained_units().len() as u32;
            if let Some(ai) = c.chinook_ai.as_mut() {
                ai.on_rappeller_released(now);
                ai.contained_count = remaining;
            }
            // Stay pending so the next evac call finishes DO_COMBAT_DROP.
            c.pending_evacuate_on_stop = true;
            c.pending_exit_after_evacuate = and_exit && !more;
        }

        true
    }

    /// C++ `AIRappelState::onEnter` dest Z: layer height, plus structure roof.
    fn combat_drop_rappel_dest(
        &self,
        hover: glam::Vec3,
        drop_target: Option<ObjectId>,
    ) -> (bool, f32) {
        let layer_y = self.terrain_height_at(hover).unwrap_or(0.0);
        let Some(tid) = drop_target else {
            return (false, layer_y);
        };
        let Some(bldg) = self.objects.get(&tid) else {
            return (false, layer_y);
        };
        if !bldg.is_alive() || !bldg.is_kind_of(KindOf::Structure) {
            return (false, layer_y);
        }
        let roof = bldg
            .thing
            .template
            .geometry_info
            .max_height_above_position();
        (true, layer_y + roof)
    }

    /// C++ `AIRappelState::update`: descend at rappel rate, then kill-2 / addToContain.
    pub(crate) fn tick_rappel_into(&mut self, pid: ObjectId) {
        let Some(obj) = self.objects.get(&pid) else {
            return;
        };
        if !obj.status.rappelling || !obj.is_alive() {
            return;
        }
        let mut target_is_bldg = obj.status.rappel_target_is_bldg;
        let mut dest_y = obj.status.rappel_dest_y;
        let target_id = obj.status.rappel_target;
        let pos = obj.get_position();
        let speed = obj.movement.max_speed;
        if target_is_bldg {
            let gone = target_id
                .map(|id| {
                    self.objects
                        .get(&id)
                        .map_or(true, |b| !b.is_alive() || !b.is_kind_of(KindOf::Structure))
                })
                .unwrap_or(true);
            if gone {
                target_is_bldg = false;
                dest_y = self.terrain_height_at(pos).unwrap_or(0.0);
            }
        }
        if !target_is_bldg {
            dest_y = self.terrain_height_at(pos).unwrap_or(dest_y);
        }
        let sink = speed * LOGIC_FRAME_TIMESTEP;
        let mut new_y = pos.y - sink;
        let arrived = new_y <= dest_y;
        if arrived {
            new_y = dest_y;
        }
        if let Some(obj) = self.objects.get_mut(&pid) {
            let mut p = obj.get_position();
            p.y = new_y;
            obj.set_position(p);
            obj.movement.velocity = glam::Vec3::new(0.0, -speed, 0.0);
            obj.status.rappel_target_is_bldg = target_is_bldg;
            obj.status.rappel_dest_y = dest_y;
            if arrived {
                obj.clear_rappel();
                obj.stop_moving();
            }
        }
        if arrived && target_is_bldg {
            if let Some(tid) = target_id {
                self.rappel_arrive_at_building(pid, tid);
            }
        }
    }

    /// C++ `AIRappelState::update` arrival: kill up to 2, else addToContain / scatter.
    fn rappel_arrive_at_building(&mut self, pid: ObjectId, bldg_id: ObjectId) {
        let Some(bldg) = self.objects.get(&bldg_id) else {
            return;
        };
        if !bldg.is_alive() {
            return;
        }
        const MAX_TO_KILL: i32 = 2;
        let num_killed = self.kill_enemies_in_container(pid, bldg_id, MAX_TO_KILL);
        // C++ AIStates.cpp:561-564: getPerUnitFX(CombatDropKillFX) on the
        // rappeller, FXList::doFXObj on the building, even if the rappeller dies.
        if num_killed > 0 {
            if let Some(fx) = self.objects.get(&pid).and_then(|p| {
                crate::game_logic::host_combat_chinook::leftover_combat_drop_kill_fx_name(
                    &p.template_name,
                )
            }) {
                let _ = self.dispatch_fx_list_at_host_object(&fx, bldg_id, None);
            }
        }
        if num_killed == MAX_TO_KILL {
            if let Some(p) = self.objects.get_mut(&pid) {
                p.kill();
            }
            return;
        }
        let can_enter = self.objects.get(&bldg_id).is_some_and(|b| {
            b.is_alive()
                && b.can_contain()
                && b.has_capacity_for(1)
                && b.garrison_container_accepts_entry()
        });
        if can_enter {
            self.rappel_add_to_contain(pid, bldg_id);
        } else {
            self.rappel_scatter_at_building(pid, bldg_id);
        }
    }

    /// C++ `killEnemiesInContainer` (AIStates.cpp:415).
    fn kill_enemies_in_container(
        &mut self,
        killer_id: ObjectId,
        bldg_id: ObjectId,
        max_to_kill: i32,
    ) -> i32 {
        let mut num_killed = 0;
        while num_killed < max_to_kill {
            let occupants = self
                .objects
                .get(&bldg_id)
                .map(|b| b.contained_units())
                .unwrap_or_default();
            let killer_team = self.objects.get(&killer_id).map(|k| k.team);
            let Some(enemy_id) = occupants.into_iter().find(|&id| {
                self.objects.get(&id).is_some_and(|e| {
                    e.is_alive()
                        && killer_team.is_some_and(|kt| {
                            kt != e.team && kt != Team::Neutral && e.team != Team::Neutral
                        })
                })
            }) else {
                break;
            };
            if let Some(b) = self.objects.get_mut(&bldg_id) {
                let _ = b.remove_occupant(enemy_id);
            }
            if let Some(e) = self.objects.get_mut(&enemy_id) {
                e.set_contained_by(None);
                e.kill();
            }
            self.award_score_the_kill_experience(killer_id, enemy_id);
            num_killed += 1;
        }
        num_killed
    }

    /// C++ `bldg->getContain()->addToContain(obj)` + garrison onContaining.
    fn rappel_add_to_contain(&mut self, pid: ObjectId, bldg_id: ObjectId) {
        // C++ OpenContain::addToContain checkAndDetonateBoobyTrap(rider).
        if self.should_cancel_containment_after_booby_trap(bldg_id, pid) {
            return;
        }
        let entered = self
            .objects
            .get_mut(&bldg_id)
            .map(|b| b.add_occupant(pid))
            .unwrap_or(false);
        if !entered {
            self.rappel_scatter_at_building(pid, bldg_id);
            return;
        }
        let container_pos = self
            .objects
            .get(&bldg_id)
            .map(|b| b.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        let enclosing = self
            .objects
            .get(&bldg_id)
            .map(|c| c.is_enclosing_garrison_container())
            .unwrap_or(true);
        if let Some(obj) = self.objects.get_mut(&pid) {
            obj.stop_moving();
            obj.set_contained_by(Some(bldg_id));
            obj.target = Some(bldg_id);
            if enclosing {
                obj.set_position(container_pos);
            }
            obj.set_ai_state(AIState::Garrisoned);
            obj.set_status_moving(false);
            obj.deselect();
        }
        self.record_garrison_residual_enter();
        self.apply_garrison_contain_on_enter(bldg_id, pid);
        // C++ OpenContain::addToContain doLoadSound.
        self.play_container_enter_sound(bldg_id);
    }

    /// C++ full-building scatter + `aiFollowPath` (AIStates.cpp:581-613).
    fn rappel_scatter_at_building(&mut self, pid: ObjectId, bldg_id: ObjectId) {
        let Some(bldg) = self.objects.get(&bldg_id) else {
            return;
        };
        let bldg_pos = bldg.get_position();
        let exit_angle = bldg.get_orientation();
        let bldg_r = bldg.thing.template.geometry_info.bounding_circle_radius();
        let pax_r = self
            .objects
            .get(&pid)
            .map(|p| p.thing.template.geometry_info.bounding_circle_radius())
            .unwrap_or(8.0);
        let offset = pax_r.min(bldg_r).max(1.0);
        let angle = std::f32::consts::PI + garrison_evac_rand(pid.0, 0.0, std::f32::consts::PI);
        let mut start = bldg_pos;
        start.x += offset * angle.cos();
        start.z += offset * angle.sin();
        start.y = self.terrain_height_at(start).unwrap_or(0.0);
        let mut end = start;
        end.z -= 20.0;
        end.y = self.terrain_height_at(end).unwrap_or(end.y);
        if let Some(obj) = self.objects.get_mut(&pid) {
            obj.set_position(start);
            obj.set_orientation(exit_angle);
            obj.movement.path = vec![start, end];
            obj.movement.current_path_index = 1;
            obj.movement.target_position = Some(end);
            obj.set_ai_state(AIState::Moving);
            obj.status.moving = true;
        }
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

    /// C++ ApplyRandomForceNugget::create residual on primary (dying) object.
    /// C++ NeutronMissileUpdate::update residual.
    pub fn update_neutron_missile_flights(&mut self) {
        use crate::game_logic::host_neutron_missile_update::{
            NEUTRON_DEFAULT_BOUNDING_SPHERE, NeutronMissileFlightPhase, NeutronMissileWorld,
        };

        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.neutron_missile_update.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let others: Vec<(ObjectId, glam::Vec3)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive())
            .map(|(id, o)| (*id, o.get_position()))
            .collect();
        let mut destroy = Vec::new();
        let mut intermediate_hits = 0u32;
        for id in ids {
            let (pos, producer, launcher, sphere) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let data = o.neutron_missile_update.as_ref();
                (
                    o.get_position(),
                    o.producer_id,
                    data.and_then(|d| d.launcher_id),
                    data.map(|d| d.bounding_sphere_radius)
                        .unwrap_or(NEUTRON_DEFAULT_BOUNDING_SPHERE),
                )
            };
            let terrain_y = self.terrain_height_at(pos);
            let colliding_other = others.iter().find_map(|(oid, opos)| {
                if *oid == id {
                    return None;
                }
                if launcher == Some(oid.0) {
                    return None;
                }
                if (*opos - pos).length() <= sphere.max(0.0) {
                    Some(oid.0)
                } else {
                    None
                }
            });
            let (grounded, phase, is_cruise, producer, launch_fx, ignition_fx) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let producer = o.producer_id.or(producer);
                let Some(data) = o.neutron_missile_update.as_mut() else {
                    continue;
                };
                let was_inter = data.reached_intermediate;
                let is_cruise = data.is_cruise;
                let tick = data.tick_world(
                    pos,
                    self.frame,
                    NeutronMissileWorld {
                        terrain_height_y: terrain_y,
                        bounding_sphere_radius: Some(sphere),
                        colliding_other,
                    },
                );
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
                // C++ NeutronMissileUpdate.cpp:226/241 FXList::doFXObj after
                // the tilted transform is set. No invented DeathExplosion.
                if launch_fx {
                    let _ = self.dispatch_fx_list_at_host_object(
                        crate::game_logic::host_neutron_missile_update::NEUTRON_LAUNCH_FX,
                        id,
                        None,
                    );
                }
                if ignition_fx {
                    let _ = self.dispatch_fx_list_at_host_object(
                        crate::game_logic::host_neutron_missile_update::NEUTRON_IGNITION_FX,
                        id,
                        None,
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
            let (pos, ground) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                (o.get_position(), o.ground_height)
            };
            let terrain_y = self.terrain_height_at(pos).unwrap_or(ground);
            let surface = self.ground_or_structure_height_at(pos, terrain_y);
            let structure_h = (surface - terrain_y).max(0.0);
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
                let tick = data.tick_at_surface(pos, self.frame, terrain_y, structure_h);
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
                // C++ MissileAIUpdate.cpp:451-474 doIgnitionState doFXObj.
                // No invented DeathExplosion at ignition.
                let _ = self.dispatch_fx_list_at_host_object(
                    crate::game_logic::special_power_strikes::SCUD_STORM_MISSILE_IGNITION_FX,
                    id,
                    None,
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
                            // C++ getControllingPlayer()->hasScience, not faction union.
                            if let Some(owner) = self.player_owner_for_host_object(o) {
                                if let Some(p) = self.get_player(owner) {
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
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 150.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
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
            o.note_producer(source_id);
            let mut data = HostCarpetBombFlightData::start(edge, target, tier);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.carpet_bomb_transport = Some(data);
            o.set_orientation(dz.atan2(dx));
        }
        self.carpet_bomb_flight_reg.record_transport();
        self.carpet_bomb_flight_reg.schedule_drops(
            self.frame,
            source_id.0,
            target,
            edge,
            tier,
            tid.0,
        );
        // C++ DeliverPayloadAIUpdate::deliverPayload createRadiusDecal (formation 0).
        let _ = self.create_delivery_radius_decal_with_radius(
            tid,
            target,
            tier.delivery_decal_radius(),
        );
        Some(tid)
    }

    pub fn update_carpet_bomb_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            CARPET_BOMB_DAMAGE, CARPET_BOMB_PAYLOAD_OBJECT, CARPET_BOMB_RADIUS,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        // Move transports. C++ HeadOffMapState + CleanUpState: fly off then destroy.
        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.carpet_bomb_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.carpet_bomb_transport.as_mut() else {
                continue;
            };
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            drop(data);
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction(id, None);
        }

        // C++ payload is contained on the transport. Dead bomber cancels remaining drops.
        let carpet_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.carpet_bomb_transport.is_some())
            .map(|(id, _)| *id)
            .collect();
        let mut alive_transports = Vec::new();
        for id in carpet_ids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let alive = o.is_alive();
            if let Some(data) = o.carpet_bomb_transport.as_mut() {
                data.transport_alive = alive;
                if data.transport_alive {
                    alive_transports.push(id.0);
                }
            }
        }
        let due = self
            .carpet_bomb_flight_reg
            .take_due_drops(self.frame, &alive_transports);
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
        // C++ DeliveringState success → HeadOffMapState (not while payload remains).
        let pending_tids: std::collections::HashSet<u32> = self
            .carpet_bomb_flight_reg
            .pending_drops
            .iter()
            .map(|p| p.transport_id)
            .collect();
        let mark: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.carpet_bomb_transport.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in mark {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            if let Some(data) = o.carpet_bomb_transport.as_mut() {
                if !pending_tids.contains(&id.0) {
                    data.delivery_complete = true;
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
    /// CREATE_AT_EDGE_FARTHEST_FROM_TARGET (z += 300) + FormationSize 12/24/36.
    pub fn spawn_artillery_barrage_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightData;
        use crate::game_logic::host_ocl_special_power::OCL_CREATE_ABOVE_LOCATION_HEIGHT;
        use crate::game_logic::host_spectre_gunship_deployment::find_farthest_edge_point_residual;
        use crate::game_logic::special_power_strikes::{
            ARTILLERY_BARRAGE_CANNON_MAX_HEALTH, ARTILLERY_BARRAGE_DECAL_RADIUS,
            ARTILLERY_BARRAGE_DELIVERY_DISTANCE, ARTILLERY_BARRAGE_FORMATION_SPACING,
            ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, ARTILLERY_BARRAGE_PREFERRED_HEIGHT,
            ARTILLERY_BARRAGE_TRANSPORT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};
        use gamelogic::object_creation_list::DeliverPayloadNugget;

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        // C++ CREATE_AT_EDGE_FARTHEST_FROM_TARGET + CREATE_ABOVE_LOCATION_HEIGHT.
        let mut edge = find_farthest_edge_point_residual(
            target,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
            0.0,
        );
        edge.y += OCL_CREATE_ABOVE_LOCATION_HEIGHT;
        // C++ StartAtPreferredHeight overwrites z after deliverPayload.
        let spawn_height = ARTILLERY_BARRAGE_PREFERRED_HEIGHT.max(edge.y);
        // Leftover Coord3D is C++ x/y ground, z height. Host is x/z ground, y height.
        let primary = Vec3::new(edge.x, edge.z, edge.y);
        let secondary = Vec3::new(target.x, target.z, target.y);
        if !self.templates.contains_key(ARTILLERY_BARRAGE_TRANSPORT) {
            let mut t = ThingTemplate::new(ARTILLERY_BARRAGE_TRANSPORT);
            t.set_health(ARTILLERY_BARRAGE_CANNON_MAX_HEALTH)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates
                .insert(ARTILLERY_BARRAGE_TRANSPORT.to_string(), t);
        }
        let formation_size = tier.formation_size().max(1);
        let mut first = None;
        for i in 0..formation_size {
            // Leftover DeliverPayloadNugget::formation_flight_pose matches C++.
            // WeaponErrorRadius is applied by schedule_drops, not spawn offsets.
            let pose = DeliverPayloadNugget::formation_flight_pose(
                &primary,
                &secondary,
                i as i32,
                formation_size as i32,
                ARTILLERY_BARRAGE_FORMATION_SPACING,
                0.0,
                ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
                None,
            );
            let mut launch = Vec3::new(pose.start_pos.x, pose.start_pos.z, pose.start_pos.y);
            launch.y = spawn_height;
            let mut move_to = Vec3::new(pose.move_to_pos.x, pose.move_to_pos.z, pose.move_to_pos.y);
            move_to.y = spawn_height;
            let delay = if ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES > 0 {
                gamelogic::helpers::get_game_logic_random_value(
                    0,
                    ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES as i32,
                )
                .max(0) as u32
            } else {
                0
            };
            let tid = self.create_object(ARTILLERY_BARRAGE_TRANSPORT, team, launch)?;
            if let Some(o) = self.objects.get_mut(&tid) {
                o.note_producer(source_id);
                let mut data = HostArtilleryBarrageFlightData::start(launch, move_to, tier);
                data.map_min = self.world_min;
                data.map_max = self.world_max;
                data.delay_until_frame = self.frame.saturating_add(delay);
                o.artillery_barrage_transport = Some(data);
                o.set_orientation(pose.orient);
            }
            self.artillery_barrage_flight_reg.record_transport();
            if first.is_none() {
                first = Some(tid);
                // C++ only formation index 0 gets a DeliveryDecal ring.
                let _ = self.create_delivery_radius_decal_with_radius(
                    tid,
                    target,
                    ARTILLERY_BARRAGE_DECAL_RADIUS,
                );
            }
        }
        self.artillery_barrage_flight_reg
            .schedule_drops(self.frame, source_id.0, target, tier);
        first
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
        let mut leave = Vec::new();
        let world_min = self.world_min;
        let world_max = self.world_max;
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.artillery_barrage_transport.as_mut() else {
                continue;
            };
            if !data.map_extent_ok() {
                data.map_min = world_min;
                data.map_max = world_max;
            }
            // C++ setDisabledUntil(DISABLED_DEFAULT, DelayDeliveryMax) — hold until ready.
            if data.is_hold_for_delay(self.frame) {
                continue;
            }
            let should_drop = !data.delivery_complete && data.in_delivery_band(pos);
            if should_drop {
                data.delivery_complete = true;
            }
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            let _ = data;
            if should_drop {
                o.kill_delivery_radius_decal();
            }
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction(id, None);
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
    /// CREATE_AT_EDGE_NEAR_SOURCE + FormationSize 1/2/3 AmericaJetA10Thunderbolt.
    pub fn spawn_a10_strike_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightData;
        use crate::game_logic::special_power_strikes::{A10_FORMATIONION_SPACING, A10_TRANSPORT};
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
        let edge = self.closest_map_edge_point(source_pos);
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        let exit = crate::game_logic::host_deliver_payload::head_off_map_exit_point_residual(
            edge,
            dx,
            dz,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
        );
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let px = -dz / dist;
        let pz = dx / dist;
        if !self.templates.contains_key(A10_TRANSPORT) {
            let mut t = ThingTemplate::new(A10_TRANSPORT);
            t.set_health(600.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(A10_TRANSPORT.to_string(), t);
        }
        let jets = tier.formation_size().max(1);
        let half = (jets as f32 - 1.0) * 0.5;
        let mut first = None;
        for j in 0..jets {
            let lat = (j as f32 - half) * A10_FORMATIONION_SPACING;
            let launch = Vec3::new(edge.x + px * lat, 160.0, edge.z + pz * lat);
            let tid = self.create_object(A10_TRANSPORT, team, launch)?;
            if let Some(o) = self.objects.get_mut(&tid) {
                o.note_producer(source_id);
                let mut data = HostA10StrikeFlightData::start_with_exit(launch, target, exit, tier);
                data.map_min = self.world_min;
                data.map_max = self.world_max;
                o.a10_strike_transport = Some(data);
                o.set_orientation(dz.atan2(dx));
            }
            self.a10_strike_flight_reg.record_transport();
            if first.is_none() {
                first = Some(tid);
                // C++ only formation index 0 gets a DeliveryDecal ring.
                let _ = self.create_delivery_radius_decal_with_radius(
                    tid,
                    target,
                    crate::game_logic::special_power_strikes::A10_DELIVERY_DECAL_RADIUS,
                );
            }
            for i in 0..crate::game_logic::host_a10_strike_flight::A10_NUM_BONES {
                let pair = i / crate::game_logic::host_a10_strike_flight::A10_ITEMS_PER_DROP;
                let drop_frame = self.frame.saturating_add(pair.saturating_mul(
                    crate::game_logic::host_a10_strike_flight::A10_DROP_DELAY_FRAMES,
                ));
                self.a10_strike_flight_reg.pending_drops.push(
                    crate::game_logic::host_a10_strike_flight::PendingA10MissileDrop {
                        drop_frame,
                        target,
                        source_id: tid.0,
                        missile_index: i,
                    },
                );
                self.a10_strike_flight_reg.missiles_scheduled = self
                    .a10_strike_flight_reg
                    .missiles_scheduled
                    .saturating_add(1);
            }
        }
        first
    }

    /// C++ TerrainLogic::findClosestEdgePoint residual (world_min/max).
    pub fn closest_map_edge_point(&self, pos: Vec3) -> Vec3 {
        let min = self.world_min;
        let max = self.world_max;
        let dl = (pos.x - min.x).abs();
        let dr = (max.x - pos.x).abs();
        let db = (pos.z - min.z).abs();
        let dt = (max.z - pos.z).abs();
        let m = dl.min(dr).min(db).min(dt);
        if (m - dl).abs() <= f32::EPSILON {
            Vec3::new(min.x, 160.0, pos.z.clamp(min.z, max.z))
        } else if (m - dr).abs() <= f32::EPSILON {
            Vec3::new(max.x, 160.0, pos.z.clamp(min.z, max.z))
        } else if (m - db).abs() <= f32::EPSILON {
            Vec3::new(pos.x.clamp(min.x, max.x), 160.0, min.z)
        } else {
            Vec3::new(pos.x.clamp(min.x, max.x), 160.0, max.z)
        }
    }

    pub fn update_a10_strike_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_a10_strike_flight::{
            A10_START_DIVE_SOUND, A10_VULCAN_DELAY_FRAMES, tick_a10_dive,
        };
        use crate::game_logic::special_power_strikes::{
            A10_MISSILE_PRIMARY_DAMAGE, A10_MISSILE_PRIMARY_RADIUS, A10_PAYLOAD_TEMPLATE,
            A10_TRANSPORT, A10_VULCAN_PRIMARY_DAMAGE, A10_VULCAN_PRIMARY_RADIUS,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.a10_strike_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut leave = Vec::new();
        let mut vulcan: Vec<(ObjectId, Option<ObjectId>, crate::game_logic::Team, Vec3)> =
            Vec::new();
        let mut dive_starts: Vec<(ObjectId, Vec3)> = Vec::new();
        let mut payload_drops: Vec<(ObjectId, crate::game_logic::Team, Vec3, Vec3, Vec3)> =
            Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.a10_strike_transport.as_mut() else {
                continue;
            };
            let target = data.target;
            let (mut new_pos, mut vel, at_exit) = data.tick_transport(pos);
            let dive = tick_a10_dive(&mut data.dive_state, new_pos, target, vel, 22.0);
            new_pos.y = dive.new_y;
            vel.y = new_pos.y - pos.y;
            if dive.start_dive {
                dive_starts.push((id, new_pos));
            }
            let due_vulcan = dive.should_strafe
                && self.frame.saturating_sub(data.last_vulcan_frame) >= A10_VULCAN_DELAY_FRAMES;
            if due_vulcan {
                data.last_vulcan_frame = self.frame;
                vulcan.push((id, o.producer_id, o.team, dive.strafe_point));
            }
            // C++ DeliveringState::update — drop VisiblePayload from the jet
            // only while isCloseEnoughToTarget (DeliveryDistance 450).
            if data.is_close_enough_to_target(new_pos) {
                let remaining_before = data.missiles_remaining;
                let n = data.take_visible_payload_drops(self.frame);
                let team = o.team;
                let yaw = o.get_orientation();
                let inherit = vel;
                for k in 0..n {
                    let slot = crate::game_logic::host_a10_strike_flight::A10_NUM_BONES
                        .saturating_sub(remaining_before)
                        .saturating_add(k)
                        .saturating_add(1);
                    let spawn = crate::game_logic::host_a10_strike_drop_log::a10_weapon_a_world_pos(
                        new_pos, yaw, slot,
                    );
                    payload_drops.push((id, team, spawn, target, inherit));
                }
            }
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for (id, producer, team, pos) in vulcan {
            self.apply_fuel_air_radius_damage(
                id,
                producer,
                team,
                pos,
                A10_VULCAN_PRIMARY_DAMAGE,
                A10_VULCAN_PRIMARY_RADIUS,
                DamageType::Bullet,
            );
        }
        for (id, pos) in dive_starts {
            use crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound;
            if let Some(name) = resolve_per_unit_sound(A10_TRANSPORT, A10_START_DIVE_SOUND) {
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(&name)
                        .with_object(id)
                        .with_position(pos)
                        .with_priority(160),
                );
            }
        }
        for id in leave {
            self.mark_object_for_destruction(id, None);
        }

        if !payload_drops.is_empty() {
            if !self.templates.contains_key(A10_PAYLOAD_TEMPLATE) {
                let mut t = ThingTemplate::new(A10_PAYLOAD_TEMPLATE);
                t.set_health(40.0).add_kind_of(KindOf::Projectile);
                self.templates.insert(A10_PAYLOAD_TEMPLATE.to_string(), t);
            }
            for (jet_id, team, drop_pos, target, inherit) in payload_drops {
                if let Some(mid) = self.create_object(A10_PAYLOAD_TEMPLATE, team, drop_pos) {
                    if let Some(o) = self.objects.get_mut(&mid) {
                        o.producer_id = Some(jet_id);
                        o.a10_strike_missile = true;
                        o.movement.velocity =
                            crate::game_logic::host_a10_strike_drop_log::a10_missile_fire_velocity(
                                drop_pos, target, inherit,
                            );
                        let _ = o.set_smart_bomb_target(target);
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
                p += o.movement.velocity;
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
        use crate::game_logic::host_leaflet_drop::{
            LEAFLET_CONTAINER_OBJECT, LEAFLET_DECAL_RADIUS, LEAFLET_TRANSPORT,
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
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 150.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
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
            o.note_producer(source_id);
            o.leaflet_transport_target = Some(target);
            o.set_orientation(dz.atan2(dx));
        }
        self.host_leaflet_drops.transports_spawned =
            self.host_leaflet_drops.transports_spawned.saturating_add(1);
        let _ = self.create_delivery_radius_decal_with_radius(tid, target, LEAFLET_DECAL_RADIUS);
        Some(tid)
    }

    pub fn update_leaflet_b52_flights(&mut self) {
        use crate::game_logic::host_deliver_payload::{
            head_off_map_exit_point_residual, is_off_map_residual,
        };
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
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(target) = o.leaflet_transport_target else {
                continue;
            };
            let pos = o.get_position();
            let dest_off_map = is_off_map_residual(
                target,
                self.world_min.x,
                self.world_min.z,
                self.world_max.x,
                self.world_max.z,
            );
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
            if !dest_off_map && dist <= LEAFLET_DELIVERY_DISTANCE * 0.5 {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                let exit = head_off_map_exit_point_residual(
                    new_pos,
                    dx,
                    dz,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                );
                o.leaflet_transport_target = Some(exit);
                o.kill_delivery_radius_decal();
                drops.push((id, team, target, producer));
            }
            if dest_off_map
                && is_off_map_residual(
                    new_pos,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                )
            {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction(id, None);
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

        // C++ LeafletDropBehavior::update first tick: create + attachToObject.
        let containers: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.leaflet_container && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in containers {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let template = o.template_name.clone();
            let first_fx = self
                .host_leaflet_drops
                .try_attach_leaflet_fx(id.0, &template);
            if first_fx {
                let fx_name =
                    crate::game_logic::host_leaflet_drop::leaflet_fx_particle_name(&template);
                let p = o.get_position();
                let _ = self.combat_particles.spawn_named(
                    CombatParticleKind::ParticleSysBone,
                    fx_name,
                    p,
                    self.frame,
                    Some(id),
                    None,
                );
            }
            let mut p = o.get_position();
            p.y += o.movement.velocity.y;
            o.set_position(p);
            if p.y <= 5.0 {
                p.y = p.y.max(0.0);
                o.set_position(p);
                o.movement.velocity = Vec3::ZERO;
                o.leaflet_container = false;
            }
        }
    }

    /// C++ SUPERWEAPON_Paradrop AmericaJetCargoPlane residual.
    pub fn spawn_paradrop_cargo_plane(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_paradrop::{
            PARADROP_DECAL_RADIUS, PARADROP_PARACHUTE_CONTAINER, PARADROP_TRANSPORT,
        };
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
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 160.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
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
            o.note_producer(source_id);
            o.paradrop_transport_target = Some(target);
            o.set_orientation(dz.atan2(dx));
        }
        self.host_paradrops.transports_spawned =
            self.host_paradrops.transports_spawned.saturating_add(1);
        let _ = self.create_delivery_radius_decal_with_radius(tid, target, PARADROP_DECAL_RADIUS);
        Some(tid)
    }

    pub fn update_paradrop_cargo_planes(&mut self) {
        use crate::game_logic::host_deliver_payload::{
            head_off_map_exit_point_residual, is_off_map_residual,
        };
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
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(target) = o.paradrop_transport_target else {
                continue;
            };
            let pos = o.get_position();
            let dest_off_map = is_off_map_residual(
                target,
                self.world_min.x,
                self.world_min.z,
                self.world_max.x,
                self.world_max.z,
            );
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
            if !dest_off_map && dist <= PARADROP_DELIVERY_DISTANCE {
                let team = o.team;
                let owner_player_id = o.owner_player_id;
                let producer = o.producer_id.unwrap_or(id);
                let exit = head_off_map_exit_point_residual(
                    new_pos,
                    dx,
                    dz,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                );
                o.paradrop_transport_target = Some(exit);
                o.kill_delivery_radius_decal();
                drops.push((team, owner_player_id, target, producer));
            }
            if dest_off_map
                && is_off_map_residual(
                    new_pos,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                )
            {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction(id, None);
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
    /// `findOCL` UpgradeOCL SCIENCE_MOAB upgrades a Daisy call to B3 + MOAB
    /// (DeliveryDistance / PreOpenDistance 160). Explicit Moab tier is kept.
    pub fn spawn_daisy_cutter_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_daisy_cutter_flight::{
            DaisyFlightPayloadTier, HostDaisyCutterFlightData,
        };
        use crate::game_logic::host_ocl_special_power::{find_ocl_name, peel_for_special_power};
        use crate::game_logic::host_replace_object_upgrade::grant_science_for_upgrade;
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
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 160.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        // C++ findOCL: first owned UpgradeOCL science wins. SCIENCE_MOAB is
        // granted by Upgrade_AmericaMOAB (same SPECIAL_DAISY_CUTTER click).
        let tier = {
            let unlocked: Vec<String> = self
                .objects
                .get(&source_id)
                .and_then(|obj| self.player_owner_for_host_object(obj))
                .and_then(|pid| self.get_player(pid))
                .map(|p| {
                    let mut names: Vec<String> = p.unlocked_sciences.iter().cloned().collect();
                    names.extend(p.completed_upgrades.iter().cloned());
                    for u in &p.completed_upgrades {
                        if let Some(sci) = grant_science_for_upgrade(u) {
                            names.push(sci.to_string());
                        }
                    }
                    names
                })
                .unwrap_or_default();
            let from_ocl = peel_for_special_power("SuperweaponDaisyCutter")
                .map(|peel| {
                    let ocl =
                        find_ocl_name(peel, |s| unlocked.iter().any(|u| u.eq_ignore_ascii_case(s)));
                    DaisyFlightPayloadTier::from_ocl_name(ocl)
                })
                .unwrap_or(tier);
            // findOCL SCIENCE_MOAB upgrades Daisy; explicit Moab tier is kept.
            if from_ocl == DaisyFlightPayloadTier::Moab {
                from_ocl
            } else {
                tier
            }
        };
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
            o.note_producer(source_id);
            let mut data = HostDaisyCutterFlightData::start(edge, target, tier);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.daisy_cutter_transport = Some(data);
            o.set_orientation(dz.atan2(dx));
        }
        self.daisy_cutter_flight_reg.record_transport(tier);
        let _ = self.create_delivery_radius_decal_with_radius(
            tid,
            target,
            crate::game_logic::special_power_strikes::DAISY_CUTTER_RADIUS_CURSOR,
        );
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
        let mut leave = Vec::new();
        let world_min = self.world_min;
        let world_max = self.world_max;
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.daisy_cutter_transport.as_mut() else {
                continue;
            };
            if !data.map_extent_ok() {
                data.map_min = world_min;
                data.map_max = world_max;
            }
            let should_drop = !data.delivery_complete && data.in_delivery_band(pos);
            if should_drop {
                data.delivery_complete = true;
            }
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            let target = data.target;
            let tier = data.tier;
            let _ = data;
            if should_drop {
                o.kill_delivery_radius_decal();
            }
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if should_drop {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                drops.push((team, target, producer, tier));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction(id, None);
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

    /// C++ SUPERWEAPON_AnthraxBomb / AnthraxBombGamma GLAJetCargoPlane residual.
    pub fn spawn_anthrax_bomb_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_anthrax_bomb_flight::{
            ANTHRAX_BOMB_GAMMA_OBJECT, ANTHRAX_TRANSPORT, AnthraxBombPayloadTier,
            HostAnthraxBombFlightData,
        };
        use crate::game_logic::host_ocl_special_power::{
            deliver_payload_for_ocl, resolve_anthrax_bomb_ocl,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let (team, source_pos, owner_pid, source_template) =
            if let Some(o) = self.objects.get(&source_id) {
                (
                    o.team,
                    o.get_position(),
                    o.owner_player_id,
                    o.template_name.clone(),
                )
            } else {
                (Team::Neutral, target, None, String::new())
            };
        let mut names: Vec<String> = Vec::new();
        if let Some(pid) = owner_pid {
            if let Some(p) = self.players.get(&pid) {
                names.extend(p.unlocked_sciences.iter().cloned());
                names.extend(p.completed_upgrades.iter().cloned());
            }
            if let Some(ident) = self.player_template_identity(pid) {
                names.push(ident.template_name.clone());
            }
        }
        names.push(source_template.clone());
        // C++ Chem_GLACommandCenter default OCL=SUPERWEAPON_AnthraxBombGamma.
        // Leftover findOCL + Chem identity; never hardcode Base.
        let ocl = resolve_anthrax_bomb_ocl(&source_template, names.iter().map(|s| s.as_str()));
        let mut tier = AnthraxBombPayloadTier::from_ocl(ocl);
        if let Some(deliver) = deliver_payload_for_ocl(ocl) {
            tier = AnthraxBombPayloadTier::from_ocl(&deliver.ocl_name);
            if deliver
                .payload
                .eq_ignore_ascii_case(ANTHRAX_BOMB_GAMMA_OBJECT)
            {
                tier = AnthraxBombPayloadTier::Gamma;
            }
        }
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 150.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        if !self.templates.contains_key(ANTHRAX_TRANSPORT) {
            let mut t = ThingTemplate::new(ANTHRAX_TRANSPORT);
            t.set_health(600.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(ANTHRAX_TRANSPORT.to_string(), t);
        }
        let bomb = tier.bomb();
        if !self.templates.contains_key(bomb) {
            let mut t = ThingTemplate::new(bomb);
            t.set_health(80.0).add_kind_of(KindOf::Projectile);
            self.templates.insert(bomb.to_string(), t);
        }
        if matches!(tier, AnthraxBombPayloadTier::Gamma)
            && !self.templates.contains_key(ANTHRAX_BOMB_GAMMA_OBJECT)
        {
            let mut t = ThingTemplate::new(ANTHRAX_BOMB_GAMMA_OBJECT);
            t.set_health(80.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(ANTHRAX_BOMB_GAMMA_OBJECT.to_string(), t);
        }
        let tid = self.create_object(ANTHRAX_TRANSPORT, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            let mut data = HostAnthraxBombFlightData::start(edge, target, tier);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.anthrax_bomb_transport = Some(data);
            o.movement.target_position = None;
            o.set_orientation(dz.atan2(dx));
        }
        self.anthrax_bomb_flight_reg.record_transport();
        let _ = self.create_delivery_radius_decal_with_radius(
            tid,
            target,
            crate::game_logic::host_anthrax_bomb_flight::ANTHRAX_DELIVERY_DECAL_RADIUS,
        );
        Some(tid)
    }
}

#[cfg(test)]
mod garrison_evac_door_tests {
    use crate::game_logic::{AIState, GameLogic, KindOf, ThingTemplate};

    #[test]
    fn garrison_exit_occupant_via_door_bursts_from_center() {
        let mut logic = GameLogic::new();
        let mut bunker_t = ThingTemplate::new("GARR_DOOR");
        bunker_t.add_kind_of(KindOf::Structure).set_health(1000.0);
        bunker_t.contain_module = crate::game_logic::ContainModuleMetadata {
            kind: crate::game_logic::ContainModuleKind::Garrison,
            slots: Some(5),
            is_enclosing_container: true,
            ..Default::default()
        };
        bunker_t.geometry_info.authored = true;
        bunker_t.geometry_info.major_radius = 20.0;
        bunker_t.geometry_info.minor_radius = 10.0;
        logic.templates.insert("GARR_DOOR".into(), bunker_t);
        let mut pax_t = ThingTemplate::new("GARR_DOOR_P");
        pax_t.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("GARR_DOOR_P".into(), pax_t);
        let origin = glam::Vec3::new(10.0, 0.0, 20.0);
        let bunker = logic
            .create_object("GARR_DOOR", crate::game_logic::Team::USA, origin)
            .unwrap();
        let pax = logic
            .create_object(
                "GARR_DOOR_P",
                crate::game_logic::Team::USA,
                glam::Vec3::new(12.0, 0.0, 20.0),
            )
            .unwrap();
        assert!(logic.garrison_exit_occupant_via_door(pax, bunker));
        let p = logic.host_object(pax).unwrap();
        assert_eq!(p.ai_state, AIState::Moving);
        let dest = p.movement.target_position.expect("door dest");
        assert!(
            (dest - origin).length() > 8.0,
            "burst dest must leave origin, not a 6-unit ring: dest={dest:?}"
        );
        assert!((p.get_position() - origin).length() < 2.0);
        assert_eq!(
            p.safe_occlusion_frame,
            logic.frame
                + crate::game_logic::host_gamedata_lobby_residual::DEFAULT_OCCLUSION_DELAY_FRAMES_RESIDUAL
                    as u32,
            "GarrisonContain::onRemoving must stamp OcclusionDelay"
        );
    }

    /// hq-ppag1: Fire Base exit snaps station height to Patch 1.01 ground Z.
    #[test]
    fn firebase_exit_snaps_station_height_to_ground() {
        let mut logic = GameLogic::new();
        let mut fb_t = ThingTemplate::new("AmericaFireBase");
        fb_t.add_kind_of(KindOf::Structure).set_health(1000.0);
        fb_t.contain_module = crate::game_logic::ContainModuleMetadata {
            kind: crate::game_logic::ContainModuleKind::Garrison,
            slots: Some(4),
            is_enclosing_container: false,
            ..Default::default()
        };
        logic.templates.insert("AmericaFireBase".into(), fb_t);
        let mut pax_t = ThingTemplate::new("FB_PAX");
        pax_t.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("FB_PAX".into(), pax_t);
        let origin = glam::Vec3::new(10.0, 0.0, 20.0);
        let fb = logic
            .create_object("AmericaFireBase", crate::game_logic::Team::USA, origin)
            .unwrap();
        let pax = logic
            .create_object(
                "FB_PAX",
                crate::game_logic::Team::USA,
                glam::Vec3::new(12.0, 8.0, 22.0),
            )
            .unwrap();
        {
            let p = logic.host_object_mut(pax).unwrap();
            p.set_contained_by(Some(fb));
            p.set_position(glam::Vec3::new(12.0, 8.0, 22.0));
        }
        assert!(logic.garrison_exit_occupant_via_door(pax, fb));
        let p = logic.host_object(pax).unwrap();
        assert!(
            (p.get_position().y - origin.y).abs() < 0.01,
            "Fire Base exit must snap station Y to ground, got y={}",
            p.get_position().y
        );

        let pax2 = logic
            .create_object(
                "FB_PAX",
                crate::game_logic::Team::USA,
                glam::Vec3::new(11.0, 9.0, 21.0),
            )
            .unwrap();
        assert!(logic.host_object_mut(fb).unwrap().add_occupant(pax2));
        if let Some(p) = logic.host_object_mut(pax2) {
            p.set_contained_by(Some(fb));
            p.set_position(glam::Vec3::new(11.0, 9.0, 21.0));
        }
        assert!(logic.evacuate_container_now(fb, false));
        let p2 = logic.host_object(pax2).unwrap();
        assert!(
            (p2.get_position().y - origin.y).abs() < 0.01,
            "Fire Base evac must snap station Y to ground, got y={}",
            p2.get_position().y
        );
    }
}
