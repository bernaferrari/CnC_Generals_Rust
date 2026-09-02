//! Mechanical split from `game_logic/game_logic.rs`. No behavior change.
#![allow(non_snake_case, unused_imports, dead_code)]
use super::authority::*;
use super::crate_tick::*;
use super::host::*;
use super::player::*;
use super::prelude::*;
use super::script_camera::*;
use super::*;

impl GameLogic {
    /// Leftover ScriptingEngine handle. Always None on the live host (hq-8ta4n).
    pub(super) fn script_engine_handle(&self) -> Option<Arc<ScriptingEngine>> {
        self.script_engine.as_ref().map(Arc::clone)
    }

    /// Leftover ScriptingEngine event ingest. No-op on live host (hq-8ta4n).
    pub(super) fn forward_event_to_scripts(&self, event: &ScriptEvent) {
        let engine = match self.script_engine_handle() {
            Some(engine) => engine,
            None => return,
        };

        let mission_event = match self.convert_script_event(event) {
            Some(evt) => evt,
            None => return,
        };

        if let Err(err) = engine.fire_event_sync(mission_event) {
            log::error!("Scripting engine failed to accept event: {}", err);
        }
    }

    pub fn new() -> Self {
        log::debug!("GameLogic::new() - creating new GameLogic instance");
        let world_width = 512.0;
        let world_height = 512.0;
        let world_min = Vec3::new(-world_width * 0.5, 0.0, -world_height * 0.5);
        let world_max = Vec3::new(world_width * 0.5, 0.0, world_height * 0.5);

        let mission_hooks = MissionScriptHooks::new().expect("Mission script runtime init failed");

        let mut instance = Self {
            attack_priority_sets: std::collections::HashMap::new(),
            team_common_attack_targets: std::collections::HashMap::new(),
            guard_next_enemy_scan: HashMap::new(),
            hunt_next_enemy_scan: HashMap::new(),
            guard_guardee_pos: HashMap::new(),

            enable_repulsors: false,
            retaliate_friends_radius: 120.0,
            max_retaliate_distance: 210.0,
            gameworld_authority:
                crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority::DEFAULT_OFF,
            objects: HostObjectStore::new(),
            host_view_dirty: HashSet::new(),
            vision_last_looks: HashMap::new(),
            vision_last_reveal_all: HashMap::new(),
            vision_last_shroud: HashMap::new(),

            players: HashMap::new(),
            player_template_bindings: HashMap::new(),
            next_object_id: ObjectId(1), // Start at 1, 0 is invalid
            next_formation_id: 1,
            frame: 0,
            next_weapon_discharge_sequence: 1,
            weapon_discharge_log:
                crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeLog::default(),
            visual_world_epoch: 1,
            next_visual_object_generation: 1,
            game_mode: GameMode::None,
            skirmish_rules: SkirmishRulesState::default(),
            world_width,
            world_height,
            world_min,
            world_max,
            victory_conditions: VictoryConditions::new(),
            objects_to_destroy: VecDeque::new(),
            combat_particles: CombatParticleRegistry::new(),
            special_power_strikes:
                crate::game_logic::special_power_strikes::HostSpecialPowerStrikeRegistry::new(),
            host_paradrops: crate::game_logic::host_paradrop::HostParadropRegistry::new(),
            host_ambushes: crate::game_logic::host_ambush::HostAmbushRegistry::new(),
            last_cash_hack_request_amount: 0,
            last_cash_hack_stolen_amount: 0,
            last_crate_drop_spawned: 0,
            host_leaflet_drops: crate::game_logic::host_leaflet_drop::HostLeafletDropRegistry::new(
            ),
            host_sneak_attacks: crate::game_logic::host_sneak_attack::HostSneakAttackRegistry::new(
            ),
            host_upgrades: crate::game_logic::host_upgrades::HostUpgradeRegistry::new(),
            supply_lines_bonus_cash_total: 0,
            cash_bounty: crate::game_logic::host_cash_bounty::HostCashBountyRegistry::new(),
            garrison_residual_enters: 0,
            garrison_residual_exits: 0,
            garrison_residual_fires: 0,
            transport_residual_loads: 0,
            transport_residual_unloads: 0,
            overlord_bunker_residual_enters: 0,
            overlord_bunker_residual_exits: 0,
            battle_bus: crate::game_logic::host_battle_bus::HostBattleBusRegistry::new(),
            highlander_body_reg:
                crate::game_logic::host_highlander_body::HostHighlanderBodyRegistry::new(),
            deploy_style_reg: crate::game_logic::host_deploy_style::HostDeployStyleRegistry::new(),
            tensile_formation_reg: crate::game_logic::host_tensile_formation::HostTensileFormationRegistry::new(),
            status_bits_upgrade_reg: crate::game_logic::host_status_bits_upgrade::HostStatusBitsUpgradeRegistry::new(),
            fire_spread_reg: crate::game_logic::host_fire_spread::HostFireSpreadRegistry::new(),
            base_regenerate_reg: crate::game_logic::host_base_regenerate::HostBaseRegenerateRegistry::new(),
            enemy_near_reg: crate::game_logic::host_enemy_near::HostEnemyNearRegistry::new(),
            passengers_fire_upgrade_reg: crate::game_logic::host_passengers_fire_upgrade::HostPassengersFireUpgradeRegistry::new(),
            animation_steering_reg: crate::game_logic::host_animation_steering::HostAnimationSteeringRegistry::new(),
            active_shroud_upgrade_reg: crate::game_logic::host_active_shroud_upgrade::HostActiveShroudUpgradeRegistry::new(),
            float_update_reg: crate::game_logic::host_float_update::HostFloatUpdateRegistry::new(),
            prone_update_reg: crate::game_logic::host_prone_update::HostProneUpdateRegistry::new(),
            radius_decal_update_reg: crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateRegistry::new(),
            checkpoint_update_reg: crate::game_logic::host_checkpoint_update::HostCheckpointUpdateRegistry::new(),
            spectre_gunship_deployment_reg: crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentRegistry::new(),
            smart_bomb_target_homing_reg: crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingRegistry::new(),
            ocl_special_power_reg: crate::game_logic::host_ocl_special_power::HostOclSpecialPowerRegistry::new(),
            ocl_create_debris_reg: crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisRegistry::new(),
            ocl_fire_weapon_attack_reg:
                crate::game_logic::host_ocl_fire_weapon_attack::HostOclFireWeaponAttackRegistry::new(),
            fuel_air_gas_reg: crate::game_logic::host_fuel_air_gas_slow_death::HostFuelAirGasRegistry::new(),
            ocl_apply_random_force_reg:
                crate::game_logic::host_ocl_apply_random_force::HostOclApplyRandomForceRegistry::new(),
            neutron_missile_update_reg:
                crate::game_logic::host_neutron_missile_update::HostNeutronMissileUpdateRegistry::new(),
            scud_storm_missile_flight_reg:
                crate::game_logic::host_scud_storm_missile_flight::HostScudStormMissileFlightRegistry::new(),
            carpet_bomb_flight_reg:
                crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightRegistry::new(),
            artillery_barrage_flight_reg:
                crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightRegistry::new(),
            a10_strike_flight_reg:
                crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightRegistry::new(),
            daisy_cutter_flight_reg:
                crate::game_logic::host_daisy_cutter_flight::HostDaisyCutterFlightRegistry::new(),
            anthrax_bomb_flight_reg:
                crate::game_logic::host_anthrax_bomb_flight::HostAnthraxBombFlightRegistry::new(),
            cluster_mines_flight_reg:
                crate::game_logic::host_cluster_mines_flight::HostClusterMinesFlightRegistry::new(),
            emp_pulse_flight_reg:
                crate::game_logic::host_emp_pulse_flight::HostEmpPulseFlightRegistry::new(),
            command_button_hunt_reg: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntRegistry::new(),
            preorder_create_reg: crate::game_logic::host_preorder_create::HostPreorderCreateRegistry::new(),
            upgrade_die_reg: crate::game_logic::host_upgrade_die::HostUpgradeDieRegistry::new(),
            tunnel_network: crate::game_logic::host_tunnel_network::HostTunnelNetworkRegistry::new(
            ),
            cave_system: crate::game_logic::host_cave_system::HostCaveSystem::new(),
            bridge_behavior:
                crate::game_logic::host_bridge_behavior::HostBridgeBehaviorRegistry::new(),
            combat_chinook: crate::game_logic::host_combat_chinook::HostCombatChinookRegistry::new(
            ),
            listening_outpost:
                crate::game_logic::host_listening_outpost::HostListeningOutpostRegistry::new(),
            troop_crawler: crate::game_logic::host_troop_crawler::HostTroopCrawlerRegistry::new(),
            mine_residual_places: 0,
            mine_residual_proximity_detonations: 0,
            mine_residual_timed_detonations: 0,
            mine_residual_manual_detonations: 0,
            mine_residual_clears: 0,
            repair_residual_structure_commands: 0,
            repair_residual_structure_heals: 0,
            repair_residual_vehicle_heals: 0,
            heal_residual_ambulance_heals: 0,
            ambulance_auto_heal_pulse_accum: 0.0,
            heal_residual_heal_pad_heals: 0,

            propaganda_residual_heals: 0,
            propaganda_residual_buffs: 0,
            propaganda_scan: crate::game_logic::host_propaganda::HostPropagandaScanState::new(),

            ecm_residual_jams: 0,
            microwaves: crate::game_logic::host_microwave::HostMicrowaveRegistry::new(),
            airfield_parking_spaces: std::collections::HashMap::new(),
            runway_reservations: std::collections::HashMap::new(),
            airfield_runway_next_in_line: std::collections::HashMap::new(),
            airfield_runway_was_in_line: std::collections::HashMap::new(),
            airfield_pending_helipad_exits: std::collections::HashMap::new(),
            heli_takeoff_or_landing: std::collections::HashMap::new(),
            airfield_healing: std::collections::HashMap::new(),
            airfield_next_heal_frame: std::collections::HashMap::new(),


            flight_decks: std::collections::HashMap::new(),
            emp_pulses: crate::game_logic::host_emp_pulse::HostEmpPulseRegistry::new(),
            baikonur_launches:
                crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry::new(),
            defector_special: crate::game_logic::host_defector_special_power::HostDefectorSpecialPowerRegistry::new(),
            upgrade_module_residuals: crate::game_logic::host_upgrade_module_residuals::HostUpgradeModuleResidualLog::default(),
            replace_grant_command_upgrades: crate::game_logic::host_replace_object_upgrade::HostReplaceGrantCommandUpgradeLog::default(),
            sub_objects_upgrades: crate::game_logic::host_sub_objects_upgrade::HostSubObjectsUpgradeLog::default(),
            frenzies: crate::game_logic::host_frenzy::HostFrenzyRegistry::new(),
            battle_plans: crate::game_logic::host_strategy_center::HostBattlePlanRegistry::new(),
            strategy_center_gun_scatter_applied: 0,
            strategy_center_gun_scatter_misses: 0,
            emergency_repairs:
                crate::game_logic::host_emergency_repair::HostEmergencyRepairRegistry::new(),
            cleanup_areas: crate::game_logic::host_cleanup_area::HostCleanupAreaRegistry::new(),
            gps_scramblers: crate::game_logic::host_gps_scrambler::HostGpsScramblerRegistry::new(),
            base_defense_residual_fires: 0,
            point_defense_residual_intercepts: 0,
            ecm_missiles_jammed: 0,
            ecm_laser_beams_spawned: 0,
            point_defense_laser_beams_spawned: 0,
            point_defense_next_ready_frame: HashMap::new(),
            point_defense_next_ready_frame_1: HashMap::new(),
            avenger: crate::game_logic::host_avenger::HostAvengerRegistry::new(),
            neutron_shell_residual_blasts: 0,
            neutron_shell_residual_infantry_kills: 0,
            neutron_shell_residual_vehicles_unmanned: 0,
            bunker_buster: crate::game_logic::host_bunker_buster::HostBunkerBusterRegistry::new(),
            comanche_rocket_pod_residual_area_attacks: 0,
            comanche_rocket_pod_residual_units_hit: 0,
            comanche_rocket_pod_shot_index: std::collections::HashMap::new(),
            comanche_rocket_pod_projectiles_spawned: 0,
            sentry_drone_residual_auto_fires: 0,
            sentry_drone_residual_detects: 0,
            pathfinder_residual_detects: 0,
            pathfinder_residual_sniper_fires: 0,
            scout_drone_residual_detects: 0,
            scout_drone_residual_attaches: 0,
            hellfire_drone_residual_auto_fires: 0,
            hellfire_drone_residual_attaches: 0,
            hellfire_scatter_applied: 0,
            hellfire_scatter_misses: 0,
            radar_scans: crate::game_logic::host_radar_scan::HostRadarScanRegistry::new(),
            spy_satellites: crate::game_logic::host_spy_satellite::HostSpySatelliteRegistry::new(),
            spy_drones: crate::game_logic::host_spy_drone::HostSpyDroneRegistry::new(),
            countermeasures:
                crate::game_logic::host_countermeasures::HostCountermeasuresRegistry::new(),
            cia_intelligence:
                crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry::new(),
            hero_abilities: crate::game_logic::host_hero_abilities::HostHeroAbilityRegistry::new(),
            black_markets: crate::game_logic::host_black_market::HostBlackMarketRegistry::new(),
            oil_derricks: crate::game_logic::host_oil_derrick::HostOilDerrickRegistry::new(),
            hacker_income: crate::game_logic::host_hacker_income::HostHackerIncomeRegistry::new(),
            supply_drop_zones:
                crate::game_logic::host_supply_drop_zone::HostSupplyDropZoneRegistry::new(),
            host_deliver_payloads:
                crate::game_logic::host_deliver_payload::HostDeliverPayloadRegistry::new(),
            host_money_crates: crate::game_logic::host_money_crate::HostMoneyCrateRegistry::new(),
            host_radar: crate::game_logic::host_radar::HostRadarRegistry::new(),
            car_bomb: crate::game_logic::host_car_bomb::HostCarBombRegistry::new(),
            saboteur: crate::game_logic::host_saboteur::HostSaboteurRegistry::new(),
            usa_pilot: crate::game_logic::host_usa_pilot::HostUsaPilotRegistry::new(),
            gla_worker: crate::game_logic::host_gla_worker::HostGlaWorkerRegistry::new(),
            bomb_truck_disguise:
                crate::game_logic::host_bomb_truck_disguise::HostBombTruckDisguiseRegistry::new(),
            bomb_truck_detonate:
                crate::game_logic::host_bomb_truck_detonate::HostBombTruckDetonateRegistry::new(),
            nuclear_tanks: crate::game_logic::host_nuclear_tanks::HostNuclearTanksRegistry::new(),
            booby_trap: crate::game_logic::host_booby_trap::HostBoobyTrapRegistry::new(),
            booby_trap_objects_spawned: 0,
            helix_napalm: crate::game_logic::host_helix_napalm::HostHelixNapalmRegistry::new(),
            fire_walls: crate::game_logic::host_firewall::HostFireWallRegistry::new(),
            inferno_fire_zones:
                crate::game_logic::host_inferno_cannon::HostInfernoFireZoneRegistry::new(),
            aurora_bombs: crate::game_logic::host_aurora_bomb::HostAuroraBombRegistry::new(),
            aurora_fuel_air_gas_spawned: 0,
            angry_mobs: crate::game_logic::host_angry_mob::HostAngryMobRegistry::new(),
            stealth_fighter_science:
                crate::game_logic::host_stealth_fighter::HostStealthFighterRegistry::new(),
            unit_training: crate::game_logic::host_unit_training::HostUnitTrainingRegistry::new(),
            demo_suicide_bomb:
                crate::game_logic::host_demo_suicide_bomb::HostDemoSuicideBombRegistry::new(),
            rocket_buggy_residual_fires: 0,
            rocket_buggy_residual_units_hit: 0,
            rocket_buggy_residual_scatter_misses: 0,
            quad_cannon_residual_ground_fires: 0,
            quad_cannon_residual_aa_fires: 0,
            quad_cannon_residual_barrel_upgrades: 0,
            scud_poison_zones: crate::game_logic::host_scud_launcher::HostScudPoisonRegistry::new(),
            overlord_addons:
                crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry::new(),
            nuke_cannon_residual: crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry::new(
            ),
            technical_residual_fires: 0,
            technical_residual_units_hit: 0,
            technical_residual_weapon_upgrades: 0,
            technical_residual_loads: 0,
            technical_residual_unloads: 0,
            toxin_tractor: crate::game_logic::host_toxin_tractor::HostToxinTractorRegistry::new(),
            marauder_residual_fires: 0,
            marauder_residual_units_hit: 0,
            marauder_residual_weapon_upgrades: 0,
            scorpion_residual_fires: 0,
            scorpion_residual_units_hit: 0,
            scorpion_residual_rocket_upgrades: 0,
            scorpion_residual_salvage_upgrades: 0,
            scorpion_residual_missile_fires: 0,
            tomahawk_residual_fires: 0,
            tomahawk_residual_units_hit: 0,
            raptor_residual_fires: 0,
            raptor_residual_units_hit: 0,
            raptor_residual_laser_missiles_upgrades: 0,
            mig_residual_fires: 0,
            mig_residual_units_hit: 0,
            mig_residual_black_napalm_upgrades: 0,
            mig_residual_tactical_nuke_upgrades: 0,
            mig_residual_fire_fields: 0,
            mig_residual_radiation_fields: 0,
            mig_scatter_applied: 0,
            mig_scatter_misses: 0,
            fire_base_residual_fires: 0,
            fire_base_residual_units_hit: 0,
            stealth_fighter_residual_fires: 0,
            stealth_fighter_residual_units_hit: 0,
            stealth_jet_missiles_spawned: 0,
            stealth_jet_scatter_applied: 0,
            stealth_jet_scatter_misses: 0,
            scud_missiles_spawned: 0,
            tomahawk_missiles_spawned: 0,
            tomahawk_scatter_applied: 0,
            tomahawk_scatter_misses: 0,
            rocket_buggy_missiles_spawned: 0,
            rocket_buggy_scatter_applied: 0,
            scud_launcher_scatter_applied: 0,
            scud_launcher_scatter_misses: 0,
            neutron_shells_spawned: 0,
            neutron_shell_scatter_applied: 0,
            neutron_shell_scatter_misses: 0,
            rpg_trooper_missiles_spawned: 0,
            rpg_trooper_scatter_applied: 0,
            rpg_trooper_scatter_misses: 0,
            tank_hunter_missiles_spawned: 0,
            tank_hunter_scatter_applied: 0,
            tank_hunter_scatter_misses: 0,
            missile_defender_missiles_spawned: 0,
            missile_defender_scatter_applied: 0,
            missile_defender_scatter_misses: 0,
            scorpion_shells_spawned: 0,
            scorpion_scatter_applied: 0,
            scorpion_scatter_misses: 0,
            scorpion_missiles_spawned: 0,
            nuke_cannon_shells_spawned: 0,
            nuke_cannon_scatter_applied: 0,
            nuke_cannon_scatter_misses: 0,
            usa_tank_shells_spawned: 0,
            usa_tank_scatter_applied: 0,
            usa_tank_scatter_misses: 0,
            battlemaster_shells_spawned: 0,
            battlemaster_scatter_applied: 0,
            battlemaster_scatter_misses: 0,
            overlord_shells_spawned: 0,
            overlord_scatter_applied: 0,
            overlord_scatter_misses: 0,
            inferno_shells_spawned: 0,
            inferno_scatter_applied: 0,
            inferno_scatter_misses: 0,
            marauder_shells_spawned: 0,
            marauder_scatter_applied: 0,
            marauder_scatter_misses: 0,
            fire_base_shells_spawned: 0,
            fire_base_scatter_applied: 0,
            fire_base_scatter_misses: 0,
            raptor_missiles_spawned: 0,
            raptor_scatter_applied: 0,
            raptor_scatter_misses: 0,
            mig_missiles_spawned: 0,
            flashbang_grenades_spawned: 0,
            flashbang_scatter_applied: 0,
            flashbang_scatter_misses: 0,
            humvee_tow_missiles_spawned: 0,
            humvee_tow_scatter_applied: 0,
            humvee_tow_scatter_misses: 0,
            humvee_tow_residual_fires: 0,
            dragon_flame_missiles_spawned: 0,
            toxin_stream_missiles_spawned: 0,
            technical_rpg_missiles_spawned: 0,
            technical_cannon_shells_spawned: 0,
            technical_cannon_scatter_applied: 0,
            technical_cannon_scatter_misses: 0,
            cleanup_stream_missiles_spawned: 0,
            angry_mob_projectiles_spawned: 0,
            usa_tank_residual_units_hit: 0,
            comanche_cannon_residual_fires: 0,
            comanche_cannon_residual_units_hit: 0,
            comanche_antitank_residual_fires: 0,
            comanche_antitank_residual_units_hit: 0,
            comanche_at_scatter_applied: 0,
            comanche_at_scatter_misses: 0,
            helix_minigun_residual_fires: 0,
            helix_minigun_residual_units_hit: 0,
            inferno_black_napalm_residual_upgrades: 0,
            inferno_black_napalm_residual_zones: 0,
            battle_drone_residual_attaches: 0,
            battle_drone_residual_fires: 0,
            battle_drone_residual_units_hit: 0,
            battle_drone_residual_repairs: 0,
            battle_drone_residual_repair_amount: 0.0,
            battle_drone_weld_states: std::collections::HashMap::new(),
            overlord_gun_residual_fires: 0,
            overlord_gun_residual_units_hit: 0,
            overlord_gun_residual_uranium_upgrades: 0,
            jarmen_kell_residual_fires: 0,
            jarmen_kell_residual_units_hit: 0,
            jarmen_kell_residual_ap_upgrades: 0,
            battlemaster_residual_fires: 0,
            battlemaster_residual_units_hit: 0,
            battlemaster_residual_uranium_upgrades: 0,
            battlemaster_residual_nationalism_upgrades: 0,
            battlemaster_residual_horde_grants: 0,
            red_guard_residual_fires: 0,
            red_guard_residual_bayonet_kills: 0,
            red_guard_residual_nationalism_upgrades: 0,
            red_guard_residual_horde_grants: 0,
            tank_hunter_residual_fires: 0,
            tank_hunter_residual_units_hit: 0,
            tank_hunter_residual_tnt_plants: 0,
            tank_hunter_residual_nationalism_upgrades: 0,
            tank_hunter_residual_horde_grants: 0,
            tank_hunter_tnt_last_frame: HashMap::new(),
            rebel_residual_fires: 0,
            rebel_residual_ap_upgrades: 0,
            ranger_residual_rifle_fires: 0,
            ranger_residual_flashbang_fires: 0,
            ranger_residual_units_hit: 0,
            hacker_disable_building_count: 0,
            minigunner_residual_ground_fires: 0,
            minigunner_residual_aa_fires: 0,
            minigunner_residual_ramp_mean: 0,
            minigunner_residual_ramp_fast: 0,
            minigunner_residual_chain_gun_upgrades: 0,
            minigunner_residual_nationalism_upgrades: 0,
            minigunner_residual_horde_grants: 0,
            burton_residual_sniper_fires: 0,
            burton_residual_knife_kills: 0,
            rpg_trooper_residual_fires: 0,
            rpg_trooper_residual_units_hit: 0,
            rpg_trooper_residual_ap_upgrades: 0,
            terrorist_residual_detonations: 0,
            terrorist_residual_units_hit: 0,
            terrorist_residual_damage_dealt: 0.0,
            missile_defender_residual_fires: 0,
            missile_defender_residual_units_hit: 0,
            missile_defender_residual_laser_specials: 0,
            missile_defender_residual_laser_fires: 0,
            missile_defender_laser_beams_spawned: 0,
            combat_cycle_residual_fires: 0,
            combat_cycle_residual_units_hit: 0,
            combat_cycle_residual_rider_switches: 0,
            combat_cycle_residual_loads: 0,
            combat_cycle_residual_suicides: 0,
            dragon_tank_residual_fires: 0,
            dragon_tank_residual_units_hit: 0,
            dragon_tank_residual_black_napalm_upgrades: 0,
            gattling_tank_residual_ground_fires: 0,
            gattling_tank_residual_aa_fires: 0,
            gattling_tank_residual_ramp_mean: 0,
            gattling_tank_residual_ramp_fast: 0,
            gattling_tank_residual_chain_gun_upgrades: 0,
            gattling_building_residual_ground_fires: 0,
            gattling_building_residual_aa_fires: 0,
            gattling_building_residual_ramp_mean: 0,
            gattling_building_residual_ramp_fast: 0,
            gattling_building_residual_chain_gun_upgrades: 0,
            stinger_site_residual_ground_fires: 0,
            stinger_site_residual_aa_fires: 0,
            stinger_site_residual_ap_rockets_upgrades: 0,
            stinger_hive_residual_slave_hits: 0,
            stinger_hive_residual_slave_kills: 0,
            stinger_hive_residual_swallows: 0,
            stinger_hive_residual_respawns: 0,
            stinger_hive_residual_closest_slave_hits: 0,
            camo_netting_heat_vision_count: 0,
            camo_netting_structure_residual_reveals: 0,
            camo_netting_order_idle_enemies_count: 0,
            camo_netting_structure_residual_recloaks: 0,
            camo_netting_opacity_cloak_count: 0,
            camo_netting_opacity_reveal_count: 0,
            camo_netting_sub_object_show_count: 0,
            stinger_slave_order_attack_count: 0,
            patriot_residual_ground_fires: 0,
            patriot_residual_aa_fires: 0,
            patriot_scatter_applied: 0,
            patriot_scatter_misses: 0,
            stinger_scatter_applied: 0,
            stinger_scatter_misses: 0,
            supw_patriot_emp_residual_grants: 0,
            supw_patriot_emp_spheroids_spawned: 0,
            supw_patriot_emp_sparks_spawned: 0,
            supw_emp_scatter_applied: 0,
            supw_emp_scatter_misses: 0,
            patriot_assist_residual_requests: 0,
            patriot_assist_residual_fires: 0,
            patriot_assist_residual_accepts: 0,
            patriot_assist_laser_from_assisted: 0,
            patriot_assist_laser_to_target: 0,
            patriot_assist_lasers: Vec::new(),
            weapon_lasers: Vec::new(),
            weapon_laser_beams_spawned: 0,
            projectile_streams:
                crate::game_logic::host_projectile_stream::ProjectileStreamRegistry::new(),
            pending_patriot_assists: Vec::new(),
            stealth_detector_rate_scans: 0,
            is_paused: false,
            sim_time_seconds: 0.0,
            accumulated_time: 0.0,
            last_fixed_step_diagnostics: FixedStepDiagnostics::default(),
            templates: HashMap::new(),
            unresolved_spawn_templates: HashSet::new(),

            map_name: String::new(),
            map_loaded: false,
            combat_system: CombatSystem::new(),
            pathfinding_system: PathfindingSystem::new_with_origin(
                world_min,
                world_width,
                world_height,
            ),
            ai_manager: AIManager::new(),
            scripts_loaded: false,
            mission_script_counter: 0,
            queued_audio_events: Vec::new(),
            command_queue: VecDeque::new(),
            accepted_gather_commands: VecDeque::new(),
            supply_dropoff_events: VecDeque::new(),
            pending_special_abilities: HashMap::new(),
            selected_objects: Vec::new(),
            partition_manager: PartitionManager::new(),
            radar_notifications: radar_notifications::global_radar_notifications(),
            last_radar_kind_time: [-10.0; 3],
            last_radar_audio_time: -10.0,
            last_radar_event: None,
            under_attack_event_history: Vec::new(),
            under_attack_events: 0,
            eva_base_under_attack: 0,
            eva_ally_under_attack: 0,
            eva_low_power: 0,
            eva_low_power_next_frame: 0,
            eva_low_power_active: false,
            eva_insufficient_funds: 0,

            eva_upgrade_complete: 0,
            eva_general_level_up: 0,
            eva_superweapon_ready: 0,
            eva_superweapon_detected: 0,
            eva_superweapon_launched: 0,
            eva_beacon_detected: 0,
            eva_hero_detected: 0,
            eva_special_launched_misc: 0,
            radar_upgrade_events: 0,
            structure_complete_events: 0,
            unit_ready_events: 0,
            radar_extend_starts: 0,
            radar_extend_completes: 0,
            radar_construction_events: 0,
            production_door_cycles: 0,
            construction_model_condition_updates: 0,
            actively_constructing_updates: 0,
            sell_list: Vec::new(),
            sell_process_starts: 0,
            sell_process_finishes: 0,
            sell_owned_mines_destroyed: 0,
            sell_passengers_ejected: 0,
            sell_parked_units_killed: 0,
            sell_tunnel_last_ejects: 0,
            capture_kick_outs: 0,
            capture_ai_auto_sells: 0,
            capture_deselections: 0,
            capture_tunnel_transfers: 0,
            capture_tunnel_last_ejects: 0,
            capture_tech_model_updates: 0,
            unmanned_reclaims: 0,
            carbomb_unmanned_detonations: 0,
            overcharge_toggles: 0,
            overcharge_drain_ticks: 0,
            overcharge_exhaustions: 0,
            control_rods_upgrades: 0,
            control_rods_plants_affected: 0,
            subliminal_messaging_upgrades: 0,
            subliminal_towers_affected: 0,
            construction_complete_clears: 0,
            dozer_cancel_task_events: 0,
            resume_construction_events: 0,
            repair_complete_events: 0,
            sole_benefactor_repair_rejects: 0,
            dozer_bored_repair_events: 0,
            dozer_bored_mine_clear_events: 0,
            rebuild_hole_spawns: 0,
            supply_create_warehouse_registers: 0,
            supply_create_center_registers: 0,
            structure_minefield_placements: 0,
            special_power_completion_log: crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionLog::default(),
            sticky_bomb_follow_ticks: 0,
            sticky_bomb_target_deaths: 0,
            rebuild_hole_reconstructs: 0,
            rebuild_hole_workers: 0,
            rebuild_hole_heals: 0,
            rebuild_hole_completes: 0,
            rebuild_hole_attack_transfers: 0,
            rebuild_hole_bomb_transfers: 0,
            rebuild_hole_recon_deaths: 0,
            rebuild_hole_worker_restarts: 0,
            pending_camera_focus: None,
            script_camera_focus_estimate: Vec3::ZERO,
            script_camera_move_to: None,
            script_camera_path: None,
            camera_follow_target: None,
            camera_tether_play: None,
            script_look_toward_object_id: None,
            script_look_toward_hold_seconds: 0.0,
            script_default_camera_pitch: 1.0,
            script_default_camera_angle: 0.0,
            script_default_camera_max_height: 1.0,
            script_camera_freeze_time_armed: false,
            script_camera_freeze_time: false,
            script_camera_rotate_remaining: 0.0,
            script_camera_zoom_remaining: 0.0,
            script_camera_pitch_remaining: 0.0,
            script_camera_freeze_angle_armed: false,
            script_camera_pending_final_speed_multiplier: None,
            visual_speed_multiplier: 1.0,
            script_time_frozen_by_script: false,
            pending_script_fps_limit: None,
            pending_camera_zoom_reset: false,
            pending_camera_zoom_reset_duration: 0.0,
            pending_camera_zoom_reset_ease_in: 0.0,
            pending_camera_zoom_reset_ease_out: 0.0,
            pending_camera_zoom: None,
            pending_camera_pitch: None,
            pending_camera_rotate: None,
            pending_camera_look_toward: None,
            pending_camera_slave_mode_enable: None,
            pending_camera_slave_mode_disable: false,
            pending_screen_shakes: Vec::new(),
            pending_camera_add_shakers: Vec::new(),
            pending_popup_messages: Vec::new(),
            pending_view_guardband: None,
            pending_camera_bw_mode: None,
            pending_camera_motion_blur: Vec::new(),
            script_skybox_enabled: true,
            script_cameo_flash_count: HashMap::new(),
            script_named_timers: HashMap::new(),
            script_named_timer_display_shown: true,
            script_superweapon_display_enabled: true,
            script_superweapon_hidden_objects: HashSet::new(),
            eva_superweapon_science_hidden: HashMap::new(),
            host_beacons: Vec::new(),
            recent_beacons: Vec::new(),
            script_engine: None,
            script_event_pump_in_flight: Arc::new(AtomicBool::new(false)),
            script_event_pump_busy_frames: 0,
            loaded_script_lists: Vec::new(),
            script_source_path: None,
            mission_scripts: mission_hooks,
            script_broadcasts: Vec::new(),
            new_script_messages: Vec::new(),
            cinematic_letterbox: false,
            cinematic_text: None,
            cinematic_font: None,
            military_caption: None,
            radar_enabled: true,
            radar_forced: false,
            pending_music_stop: false,
            pending_movie: None,
            pending_radar_movie: None,
            mission_objectives: Self::seed_sample_objectives(),
            objective_lookup: HashMap::new(),
            campaign_manager: global_campaign_manager().ok(),
            last_map_settings: None,
            spawned_map_object_ids: Vec::new(),
            terrain: None,
            runtime_road_segments: Vec::new(),
            runtime_terrain_texture_classes: Vec::new(),
            pathfinding_height_samples: None,
            weather_state: RuntimeWeatherState::default(),
            host_sleepy: super::world_tick::HostSleepyHeap::new(),
            replay_observer_player_id: None,
            install_multiplayer_scripts: false,
        };
        instance.rebuild_objective_lookup();
        // Fresh instance = clean authority barrier: deep readers on this thread
        // see this instance's (default-off) switches, never a prior test's.
        instance.publish_gameworld_authority_context();
        GameLogic::register_leftover_object_create_overrides_overlay();
        // C++ TheGameLogic::clearGameData tears per-world shroud down with the
        // world (ThePartitionManager::reset + ThePlayerList::reset destroy
        // PartitionData shroudedness and each Player's Shroud). The residual
        // process-global ShroudManager bridge must not leak a previous
        // world's object-shroud entries into this one (Object IDs recycle).
        gamelogic::system::shroud_manager::get_shroud_manager()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reset_for_new_game();
        instance
    }

    /// World bounds used for minimap/FOW projections.
    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        (self.world_min, self.world_max)
    }

    pub fn fixed_step_diagnostics(&self) -> FixedStepDiagnostics {
        self.last_fixed_step_diagnostics
    }

    /// Override world dimensions when terrain provides authoritative size.
    pub fn override_world_size(&mut self, width: f32, height: f32) {
        self.world_width = width;
        self.world_height = height;
        self.world_min = Vec3::new(-width * 0.5, 0.0, -height * 0.5);
        self.world_max = Vec3::new(width * 0.5, 0.0, height * 0.5);
        self.pathfinding_system = PathfindingSystem::new_with_origin(self.world_min, width, height);
        // Terrain-provided extent must seed TheRadar samples (C++ newMap).
        self.host_radar_on_map_loaded();
    }

    /// Reset method - matching C++ GameLogic interface
    pub fn reset(&mut self) {
        log::debug!("GameLogic::reset() - resetting game state");
        crate::assets::clear_live_draw_playback();
        // Same per-world shroud teardown as GameLogic::new — start_new_game /
        // clearGameData route through here (C++ GameLogic.cpp newGame calls
        // clearGameData before rebuilding the player list).
        gamelogic::system::shroud_manager::get_shroud_manager()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reset_for_new_game();

        self.objects.clear();
        self.hunt_next_enemy_scan.clear();
        self.host_view_dirty.clear();
        self.vision_last_looks.clear();
        self.vision_last_reveal_all.clear();
        self.vision_last_shroud.clear();

        self.players.clear();
        self.player_template_bindings.clear();
        self.next_object_id = ObjectId(1);
        self.next_formation_id = 1;
        self.frame = 0;
        self.next_weapon_discharge_sequence = 1;
        self.weapon_discharge_log.clear();
        self.visual_world_epoch = self.visual_world_epoch.wrapping_add(1).max(1);
        self.next_visual_object_generation = 1;
        self.objects_to_destroy.clear();
        self.accepted_gather_commands.clear();
        self.supply_dropoff_events.clear();
        self.combat_particles.clear();
        self.special_power_strikes.clear();
        self.host_paradrops.clear();
        self.host_ambushes.clear();
        self.host_leaflet_drops.clear();
        self.host_sneak_attacks.clear();
        self.host_upgrades.clear();
        self.supply_lines_bonus_cash_total = 0;
        self.cash_bounty.clear();
        self.garrison_residual_enters = 0;
        self.garrison_residual_exits = 0;
        self.garrison_residual_fires = 0;
        self.transport_residual_loads = 0;
        self.transport_residual_unloads = 0;
        self.overlord_bunker_residual_enters = 0;
        self.overlord_bunker_residual_exits = 0;
        self.battle_bus.clear();
        self.highlander_body_reg.clear();
        self.deploy_style_reg.clear();
        self.tensile_formation_reg.clear();
        self.status_bits_upgrade_reg.clear();
        self.fire_spread_reg.clear();
        self.base_regenerate_reg.clear();
        self.enemy_near_reg.clear();
        self.passengers_fire_upgrade_reg.clear();
        self.animation_steering_reg.clear();
        self.active_shroud_upgrade_reg.clear();
        self.float_update_reg.clear();
        self.prone_update_reg.clear();
        self.radius_decal_update_reg.clear();
        self.checkpoint_update_reg.clear();
        self.spectre_gunship_deployment_reg.clear();
        self.smart_bomb_target_homing_reg.clear();
        self.ocl_special_power_reg.clear();
        self.ocl_create_debris_reg.clear();
        self.ocl_fire_weapon_attack_reg.clear();
        self.fuel_air_gas_reg.clear();
        self.ocl_apply_random_force_reg.clear();
        self.neutron_missile_update_reg.clear();
        self.scud_storm_missile_flight_reg.clear();
        self.carpet_bomb_flight_reg.clear();
        self.artillery_barrage_flight_reg.clear();
        self.a10_strike_flight_reg.clear();
        self.daisy_cutter_flight_reg.clear();
        self.anthrax_bomb_flight_reg.clear();
        self.cluster_mines_flight_reg.clear();
        self.emp_pulse_flight_reg.clear();
        self.command_button_hunt_reg.clear();
        self.preorder_create_reg.clear();
        self.upgrade_die_reg.clear();
        self.tunnel_network.clear();
        self.combat_chinook.clear();
        self.listening_outpost.clear();
        self.troop_crawler.clear();
        self.mine_residual_places = 0;
        self.mine_residual_proximity_detonations = 0;
        self.mine_residual_timed_detonations = 0;
        self.mine_residual_manual_detonations = 0;
        self.mine_residual_clears = 0;
        self.repair_residual_structure_commands = 0;
        self.repair_residual_structure_heals = 0;
        self.repair_residual_vehicle_heals = 0;
        self.heal_residual_ambulance_heals = 0;
        self.ambulance_auto_heal_pulse_accum = 0.0;
        self.heal_residual_heal_pad_heals = 0;

        self.propaganda_residual_heals = 0;
        self.propaganda_residual_buffs = 0;
        self.propaganda_scan.clear();

        self.ecm_residual_jams = 0;
        self.microwaves.clear();
        self.airfield_parking_spaces.clear();
        self.runway_reservations.clear();
        self.airfield_runway_next_in_line.clear();
        self.airfield_runway_was_in_line.clear();
        self.airfield_pending_helipad_exits.clear();
        self.heli_takeoff_or_landing.clear();
        self.airfield_healing.clear();
        self.airfield_next_heal_frame.clear();

        self.flight_decks.clear();
        self.emp_pulses.clear();
        self.baikonur_launches =
            crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry::new();
        self.defector_special =
            crate::game_logic::host_defector_special_power::HostDefectorSpecialPowerRegistry::new();
        self.upgrade_module_residuals =
            crate::game_logic::host_upgrade_module_residuals::HostUpgradeModuleResidualLog::default(
            );
        self.frenzies.clear();
        self.cleanup_areas.clear();
        self.base_defense_residual_fires = 0;
        self.point_defense_residual_intercepts = 0;
        self.ecm_missiles_jammed = 0;
        self.ecm_laser_beams_spawned = 0;
        self.point_defense_laser_beams_spawned = 0;
        self.point_defense_next_ready_frame.clear();
        self.point_defense_next_ready_frame_1.clear();
        self.avenger.clear();
        self.neutron_shell_residual_blasts = 0;
        self.neutron_shell_residual_infantry_kills = 0;
        self.neutron_shell_residual_vehicles_unmanned = 0;
        self.bunker_buster.clear();
        self.comanche_rocket_pod_residual_area_attacks = 0;
        self.comanche_rocket_pod_residual_units_hit = 0;
        self.comanche_rocket_pod_shot_index.clear();
        self.comanche_rocket_pod_projectiles_spawned = 0;
        self.sentry_drone_residual_auto_fires = 0;
        self.sentry_drone_residual_detects = 0;
        self.pathfinder_residual_detects = 0;
        self.pathfinder_residual_sniper_fires = 0;
        self.scout_drone_residual_detects = 0;
        self.scout_drone_residual_attaches = 0;
        self.hellfire_drone_residual_auto_fires = 0;
        self.hellfire_drone_residual_attaches = 0;
        self.hellfire_scatter_applied = 0;
        self.hellfire_scatter_misses = 0;
        self.radar_scans.clear();
        self.spy_satellites.clear();
        self.spy_drones.clear();
        self.countermeasures.clear();
        self.hero_abilities.clear();
        self.black_markets.clear();
        self.oil_derricks.clear();
        self.hacker_income.clear();
        self.supply_drop_zones.clear();
        self.host_deliver_payloads.clear();
        self.host_money_crates.clear();
        self.host_radar.clear();
        self.car_bomb.clear();
        self.saboteur.clear();
        self.usa_pilot.clear();
        self.gla_worker.clear();
        self.bomb_truck_disguise.clear();
        self.bomb_truck_detonate.clear();
        self.nuclear_tanks.clear();
        self.booby_trap.clear();
        self.booby_trap_objects_spawned = 0;
        self.helix_napalm.clear();
        self.fire_walls.clear();
        self.inferno_fire_zones.clear();
        self.aurora_bombs.clear();
        self.aurora_fuel_air_gas_spawned = 0;
        self.angry_mobs.clear();
        self.stealth_fighter_science.clear();
        self.unit_training.clear();
        self.demo_suicide_bomb.clear();
        self.rocket_buggy_residual_fires = 0;
        self.rocket_buggy_residual_units_hit = 0;
        self.rocket_buggy_residual_scatter_misses = 0;
        self.quad_cannon_residual_ground_fires = 0;
        self.quad_cannon_residual_aa_fires = 0;
        self.quad_cannon_residual_barrel_upgrades = 0;
        self.scud_poison_zones.clear();
        self.overlord_addons.clear();
        self.nuke_cannon_residual.clear();
        self.technical_residual_fires = 0;
        self.technical_residual_units_hit = 0;
        self.technical_residual_weapon_upgrades = 0;
        self.technical_residual_loads = 0;
        self.technical_residual_unloads = 0;
        self.toxin_tractor.clear();
        self.marauder_residual_fires = 0;
        self.marauder_residual_units_hit = 0;
        self.marauder_residual_weapon_upgrades = 0;
        self.scorpion_residual_fires = 0;
        self.scorpion_residual_units_hit = 0;
        self.scorpion_residual_rocket_upgrades = 0;
        self.scorpion_residual_salvage_upgrades = 0;
        self.scorpion_residual_missile_fires = 0;
        self.tomahawk_residual_fires = 0;
        self.tomahawk_residual_units_hit = 0;
        self.raptor_residual_fires = 0;
        self.raptor_residual_units_hit = 0;
        self.raptor_residual_laser_missiles_upgrades = 0;
        self.mig_residual_fires = 0;
        self.mig_residual_units_hit = 0;
        self.mig_residual_black_napalm_upgrades = 0;
        self.mig_residual_tactical_nuke_upgrades = 0;
        self.mig_residual_fire_fields = 0;
        self.mig_residual_radiation_fields = 0;
        self.mig_scatter_applied = 0;
        self.mig_scatter_misses = 0;
        self.fire_base_residual_fires = 0;
        self.fire_base_residual_units_hit = 0;
        self.stealth_fighter_residual_fires = 0;
        self.stealth_fighter_residual_units_hit = 0;
        self.stealth_jet_missiles_spawned = 0;
        self.stealth_jet_scatter_applied = 0;
        self.stealth_jet_scatter_misses = 0;
        self.scud_missiles_spawned = 0;
        self.tomahawk_missiles_spawned = 0;
        self.tomahawk_scatter_applied = 0;
        self.tomahawk_scatter_misses = 0;
        self.rocket_buggy_missiles_spawned = 0;
        self.rocket_buggy_scatter_applied = 0;
        self.scud_launcher_scatter_applied = 0;
        self.scud_launcher_scatter_misses = 0;
        self.neutron_shells_spawned = 0;
        self.neutron_shell_scatter_applied = 0;
        self.neutron_shell_scatter_misses = 0;
        self.rpg_trooper_missiles_spawned = 0;
        self.rpg_trooper_scatter_applied = 0;
        self.rpg_trooper_scatter_misses = 0;
        self.tank_hunter_missiles_spawned = 0;
        self.tank_hunter_scatter_applied = 0;
        self.tank_hunter_scatter_misses = 0;
        self.missile_defender_missiles_spawned = 0;
        self.missile_defender_scatter_applied = 0;
        self.missile_defender_scatter_misses = 0;
        self.scorpion_shells_spawned = 0;
        self.scorpion_scatter_applied = 0;
        self.scorpion_scatter_misses = 0;
        self.scorpion_missiles_spawned = 0;
        self.nuke_cannon_shells_spawned = 0;
        self.nuke_cannon_scatter_applied = 0;
        self.nuke_cannon_scatter_misses = 0;
        self.usa_tank_shells_spawned = 0;
        self.usa_tank_scatter_applied = 0;
        self.usa_tank_scatter_misses = 0;
        self.battlemaster_shells_spawned = 0;
        self.battlemaster_scatter_applied = 0;
        self.battlemaster_scatter_misses = 0;
        self.overlord_shells_spawned = 0;
        self.overlord_scatter_applied = 0;
        self.overlord_scatter_misses = 0;
        self.inferno_shells_spawned = 0;
        self.inferno_scatter_applied = 0;
        self.inferno_scatter_misses = 0;
        self.marauder_shells_spawned = 0;
        self.marauder_scatter_applied = 0;
        self.marauder_scatter_misses = 0;
        self.fire_base_shells_spawned = 0;
        self.fire_base_scatter_applied = 0;
        self.fire_base_scatter_misses = 0;
        self.raptor_missiles_spawned = 0;
        self.raptor_scatter_applied = 0;
        self.raptor_scatter_misses = 0;
        self.mig_missiles_spawned = 0;
        self.flashbang_grenades_spawned = 0;
        self.flashbang_scatter_applied = 0;
        self.flashbang_scatter_misses = 0;
        self.usa_tank_residual_units_hit = 0;
        self.comanche_cannon_residual_fires = 0;
        self.comanche_cannon_residual_units_hit = 0;
        self.comanche_antitank_residual_fires = 0;
        self.comanche_antitank_residual_units_hit = 0;
        self.comanche_at_scatter_applied = 0;
        self.comanche_at_scatter_misses = 0;
        self.strategy_center_gun_scatter_applied = 0;
        self.strategy_center_gun_scatter_misses = 0;
        self.helix_minigun_residual_fires = 0;
        self.helix_minigun_residual_units_hit = 0;
        self.inferno_black_napalm_residual_upgrades = 0;
        self.inferno_black_napalm_residual_zones = 0;
        self.battle_drone_residual_attaches = 0;
        self.battle_drone_residual_fires = 0;
        self.battle_drone_residual_units_hit = 0;
        self.battle_drone_residual_repairs = 0;
        self.battle_drone_residual_repair_amount = 0.0;
        self.battle_drone_weld_states.clear();
        self.overlord_gun_residual_fires = 0;
        self.overlord_gun_residual_units_hit = 0;
        self.overlord_gun_residual_uranium_upgrades = 0;
        self.jarmen_kell_residual_fires = 0;
        self.jarmen_kell_residual_units_hit = 0;
        self.jarmen_kell_residual_ap_upgrades = 0;
        self.battlemaster_residual_fires = 0;
        self.battlemaster_residual_units_hit = 0;
        self.battlemaster_residual_uranium_upgrades = 0;
        self.battlemaster_residual_nationalism_upgrades = 0;
        self.battlemaster_residual_horde_grants = 0;
        self.red_guard_residual_fires = 0;
        self.red_guard_residual_bayonet_kills = 0;
        self.red_guard_residual_nationalism_upgrades = 0;
        self.red_guard_residual_horde_grants = 0;
        self.tank_hunter_residual_fires = 0;
        self.tank_hunter_residual_units_hit = 0;
        self.tank_hunter_residual_tnt_plants = 0;
        self.tank_hunter_residual_nationalism_upgrades = 0;
        self.tank_hunter_residual_horde_grants = 0;
        self.tank_hunter_tnt_last_frame.clear();
        self.rebel_residual_fires = 0;
        self.rebel_residual_ap_upgrades = 0;
        self.ranger_residual_rifle_fires = 0;
        self.ranger_residual_flashbang_fires = 0;
        self.ranger_residual_units_hit = 0;
        self.hacker_disable_building_count = 0;
        self.minigunner_residual_ground_fires = 0;
        self.minigunner_residual_aa_fires = 0;
        self.minigunner_residual_ramp_mean = 0;
        self.minigunner_residual_ramp_fast = 0;
        self.minigunner_residual_chain_gun_upgrades = 0;
        self.minigunner_residual_nationalism_upgrades = 0;
        self.minigunner_residual_horde_grants = 0;
        self.burton_residual_sniper_fires = 0;
        self.burton_residual_knife_kills = 0;
        self.rpg_trooper_residual_fires = 0;
        self.rpg_trooper_residual_units_hit = 0;
        self.rpg_trooper_residual_ap_upgrades = 0;
        self.terrorist_residual_detonations = 0;
        self.terrorist_residual_units_hit = 0;
        self.terrorist_residual_damage_dealt = 0.0;
        self.missile_defender_residual_fires = 0;
        self.missile_defender_residual_units_hit = 0;
        self.missile_defender_residual_laser_specials = 0;
        self.missile_defender_residual_laser_fires = 0;
        self.missile_defender_laser_beams_spawned = 0;
        self.combat_cycle_residual_fires = 0;
        self.combat_cycle_residual_units_hit = 0;
        self.combat_cycle_residual_rider_switches = 0;
        self.combat_cycle_residual_loads = 0;
        self.combat_cycle_residual_suicides = 0;
        self.dragon_tank_residual_fires = 0;
        self.dragon_tank_residual_units_hit = 0;
        self.dragon_tank_residual_black_napalm_upgrades = 0;
        self.gattling_tank_residual_ground_fires = 0;
        self.gattling_tank_residual_aa_fires = 0;
        self.gattling_tank_residual_ramp_mean = 0;
        self.gattling_tank_residual_ramp_fast = 0;
        self.gattling_tank_residual_chain_gun_upgrades = 0;
        self.gattling_building_residual_ground_fires = 0;
        self.gattling_building_residual_aa_fires = 0;
        self.gattling_building_residual_ramp_mean = 0;
        self.gattling_building_residual_ramp_fast = 0;
        self.gattling_building_residual_chain_gun_upgrades = 0;
        self.stinger_site_residual_ground_fires = 0;
        self.stinger_site_residual_aa_fires = 0;
        self.stinger_site_residual_ap_rockets_upgrades = 0;
        self.stinger_hive_residual_slave_hits = 0;
        self.stinger_hive_residual_slave_kills = 0;
        self.stinger_hive_residual_swallows = 0;
        self.stinger_hive_residual_respawns = 0;
        self.stinger_hive_residual_closest_slave_hits = 0;
        self.camo_netting_heat_vision_count = 0;
        self.camo_netting_structure_residual_reveals = 0;
        self.camo_netting_order_idle_enemies_count = 0;
        self.camo_netting_structure_residual_recloaks = 0;
        self.camo_netting_opacity_cloak_count = 0;
        self.camo_netting_opacity_reveal_count = 0;
        self.camo_netting_sub_object_show_count = 0;
        self.stinger_slave_order_attack_count = 0;
        self.patriot_residual_ground_fires = 0;
        self.patriot_residual_aa_fires = 0;
        self.patriot_scatter_applied = 0;
        self.patriot_scatter_misses = 0;
        self.stinger_scatter_applied = 0;
        self.stinger_scatter_misses = 0;
        self.supw_patriot_emp_residual_grants = 0;
        self.supw_patriot_emp_spheroids_spawned = 0;
        self.supw_patriot_emp_sparks_spawned = 0;
        self.supw_emp_scatter_applied = 0;
        self.supw_emp_scatter_misses = 0;
        self.patriot_assist_residual_requests = 0;
        self.patriot_assist_residual_fires = 0;
        self.patriot_assist_residual_accepts = 0;
        self.patriot_assist_laser_from_assisted = 0;
        self.patriot_assist_laser_to_target = 0;
        self.patriot_assist_lasers.clear();
        self.pending_patriot_assists.clear();
        self.stealth_detector_rate_scans = 0;
        self.is_paused = false;
        self.sim_time_seconds = 0.0;
        self.accumulated_time = 0.0;
        self.last_fixed_step_diagnostics = FixedStepDiagnostics::default();
        self.map_loaded = false;
        self.victory_conditions.reset();
        self.scripts_loaded = false;
        self.replay_observer_player_id = None;
        self.install_multiplayer_scripts = false;
        self.script_event_pump_in_flight
            .store(false, Ordering::Release);
        self.script_event_pump_busy_frames = 0;
        self.loaded_script_lists.clear();
        self.script_source_path = None;
        self.mission_scripts.install_lists(&[]);
        self.script_broadcasts.clear();
        self.new_script_messages.clear();
        self.cinematic_letterbox = false;
        self.cinematic_text = None;
        self.cinematic_font = None;
        self.military_caption = None;
        self.radar_enabled = true;
        self.radar_forced = false;
        self.pending_music_stop = false;
        self.pending_movie = None;
        self.pending_radar_movie = None;
        self.spawned_map_object_ids.clear();
        self.pending_special_abilities.clear();
        self.mission_objectives = Self::seed_sample_objectives();
        self.rebuild_objective_lookup();
        self.last_radar_event = None;
        self.under_attack_event_history.clear();
        self.under_attack_events = 0;
        self.eva_base_under_attack = 0;
        self.eva_ally_under_attack = 0;
        self.eva_low_power = 0;
        self.eva_low_power_next_frame = 0;
        self.eva_low_power_active = false;
        self.eva_insufficient_funds = 0;

        self.eva_upgrade_complete = 0;
        self.eva_general_level_up = 0;
        self.eva_superweapon_ready = 0;
        self.eva_superweapon_detected = 0;
        self.eva_superweapon_launched = 0;
        self.eva_beacon_detected = 0;
        self.eva_hero_detected = 0;
        self.eva_special_launched_misc = 0;
        self.radar_upgrade_events = 0;
        self.structure_complete_events = 0;
        self.unit_ready_events = 0;
        self.radar_extend_starts = 0;
        self.radar_extend_completes = 0;
        self.radar_construction_events = 0;
        self.production_door_cycles = 0;
        self.construction_model_condition_updates = 0;
        self.actively_constructing_updates = 0;
        self.sell_list.clear();
        self.sell_process_starts = 0;
        self.sell_process_finishes = 0;
        self.sell_owned_mines_destroyed = 0;
        self.sell_passengers_ejected = 0;
        self.sell_parked_units_killed = 0;
        self.sell_tunnel_last_ejects = 0;
        self.capture_kick_outs = 0;
        self.capture_ai_auto_sells = 0;
        self.capture_deselections = 0;
        self.capture_tunnel_transfers = 0;
        self.capture_tunnel_last_ejects = 0;
        self.capture_tech_model_updates = 0;
        self.unmanned_reclaims = 0;
        self.carbomb_unmanned_detonations = 0;
        self.overcharge_toggles = 0;
        self.overcharge_drain_ticks = 0;
        self.overcharge_exhaustions = 0;
        self.control_rods_upgrades = 0;
        self.control_rods_plants_affected = 0;
        self.subliminal_messaging_upgrades = 0;
        self.subliminal_towers_affected = 0;
        self.construction_complete_clears = 0;
        self.dozer_cancel_task_events = 0;
        self.resume_construction_events = 0;
        self.repair_complete_events = 0;
        self.sole_benefactor_repair_rejects = 0;
        self.dozer_bored_repair_events = 0;
        self.dozer_bored_mine_clear_events = 0;
        self.rebuild_hole_spawns = 0;
        self.supply_create_warehouse_registers = 0;
        self.supply_create_center_registers = 0;
        self.structure_minefield_placements = 0;
        self.special_power_completion_log =
            crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionLog::default();
        self.sticky_bomb_follow_ticks = 0;
        self.sticky_bomb_target_deaths = 0;
        self.rebuild_hole_reconstructs = 0;
        self.rebuild_hole_workers = 0;
        self.rebuild_hole_heals = 0;
        self.rebuild_hole_completes = 0;
        self.rebuild_hole_attack_transfers = 0;
        self.rebuild_hole_bomb_transfers = 0;
        self.rebuild_hole_recon_deaths = 0;
        self.rebuild_hole_worker_restarts = 0;
        self.last_radar_audio_time = -10.0;
        self.last_radar_kind_time = [-10.0; 3];
        self.pending_camera_focus = None;
        self.script_camera_focus_estimate = Vec3::ZERO;
        self.script_camera_move_to = None;
        self.script_camera_path = None;
        self.camera_follow_target = None;
        self.camera_tether_play = None;
        self.script_look_toward_object_id = None;
        self.script_look_toward_hold_seconds = 0.0;
        self.script_default_camera_pitch = 1.0;
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = 1.0;
        self.script_camera_freeze_time_armed = false;
        self.script_camera_freeze_time = false;
        self.script_camera_rotate_remaining = 0.0;
        self.script_camera_zoom_remaining = 0.0;
        self.script_camera_pitch_remaining = 0.0;
        self.script_camera_freeze_angle_armed = false;
        self.script_camera_pending_final_speed_multiplier = None;
        self.visual_speed_multiplier = 1.0;
        self.script_time_frozen_by_script = false;
        self.pending_script_fps_limit = None;
        self.pending_camera_zoom_reset = false;
        self.pending_camera_zoom_reset_duration = 0.0;
        self.pending_camera_zoom_reset_ease_in = 0.0;
        self.pending_camera_zoom_reset_ease_out = 0.0;
        self.pending_camera_zoom = None;
        self.pending_camera_pitch = None;
        self.pending_camera_rotate = None;
        self.pending_camera_look_toward = None;
        self.pending_camera_slave_mode_enable = None;
        self.pending_camera_slave_mode_disable = false;
        self.pending_screen_shakes.clear();
        self.pending_camera_add_shakers.clear();
        self.pending_popup_messages.clear();
        self.pending_view_guardband = None;
        self.pending_camera_bw_mode = None;
        self.pending_camera_motion_blur.clear();
        self.script_skybox_enabled = true;
        self.script_cameo_flash_count.clear();
        self.script_named_timers.clear();
        self.script_named_timer_display_shown = true;
        self.script_superweapon_display_enabled = true;
        self.script_superweapon_hidden_objects.clear();
        self.eva_superweapon_science_hidden.clear();
        self.host_beacons.clear();
        self.recent_beacons.clear();
        self.terrain = None;
        self.runtime_road_segments.clear();
        self.pathfinding_height_samples = None;
        self.weather_state = RuntimeWeatherState::default();
        // Host AI is match-scoped. Wipe so rematch / start_new_game cannot leave
        // orphan AI slots with stale object_ids while players were cleared above.
        // load_map does not call reset, so preserve_host_players still keeps AI.
        self.ai_manager = AIManager::new();
        log::debug!("GameLogic::reset() complete");
    }
}
