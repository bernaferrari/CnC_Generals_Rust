pub mod audio_dispatch_impl;
pub mod buildings;
pub mod combat;
pub mod combat_particles;
pub mod game_logic;
pub mod host_ambush;
pub mod host_angry_mob;
pub mod host_aurora_bomb;
pub mod host_avenger;
pub mod host_base_defense;
pub mod host_battle_bus;
pub mod host_battlemaster;
pub mod host_black_market;
pub mod host_bomb_truck_detonate;
pub mod host_bomb_truck_disguise;
pub mod host_booby_trap;
pub mod host_bunker_buster;
pub mod host_car_bomb;
pub mod host_cash_bounty;
pub mod host_cia_intelligence;
pub mod host_cleanup_area;
pub mod host_colonel_burton;
pub mod host_comanche_rocket_pods;
pub mod host_combat_chinook;
pub mod host_combat_cycle;
pub mod host_dragon_tank;
pub mod host_ecm_jam;
pub mod host_emergency_repair;
pub mod host_emp_pulse;
pub mod host_firewall;
pub mod host_fire_base;
pub mod host_frenzy;
pub mod host_gattling_tank;
pub mod host_gla_rebel;
pub mod host_gla_worker;
pub mod host_gps_scrambler;
pub mod host_hacker_disable;
pub mod host_hacker_income;
pub mod host_heal;
pub mod host_helix_minigun;
pub mod host_helix_napalm;
pub mod host_hero_abilities;
pub mod host_humvee;
pub mod host_inferno_cannon;
pub mod host_jarmen_kell;
pub mod host_leaflet_drop;
pub mod host_listening_outpost;
pub mod host_marauder;
pub mod host_microwave;
pub mod host_mig;
pub mod host_mines;
pub mod host_minigunner;
pub mod host_missile_defender;
pub mod host_neutron_shell;
pub mod host_nuclear_tanks;
pub mod host_nuke_cannon;
pub mod host_oil_derrick;
pub mod host_overlord_addons;
pub mod host_overlord_gun;
pub mod host_paradrop;
pub mod host_pathfinder;
pub mod host_point_defense;
pub mod host_propaganda;
pub mod host_quad_cannon;
pub mod host_radar;
pub mod host_radar_scan;
pub mod host_ranger;
pub mod host_raptor;
pub mod host_red_guard;
pub mod host_repair;
pub mod host_rocket_buggy;
pub mod host_rpg_trooper;
pub mod host_saboteur;
pub mod host_scorpion;
pub mod host_scud_launcher;
pub mod host_sentry_drone;
pub mod host_slave_drones;
pub mod host_sneak_attack;
pub mod host_spy_satellite;
pub mod host_stealth_fighter;
pub mod host_strategy_center;
pub mod host_supply_drop_zone;
pub mod host_tank_hunter;
pub mod host_technical;
pub mod host_terrorist;
pub mod host_tomahawk;
pub mod host_toxin_tractor;
pub mod host_troop_crawler;
pub mod host_tunnel_network;
pub mod host_upgrades;
pub mod host_usa_pilot;
pub mod host_usa_tanks;
pub mod locomotor_bootstrap;
pub mod mission_scripts;
pub mod object;
pub mod partition_manager;
pub mod pathfinding;
pub mod radar_notifications;
pub mod resources;
pub mod script_events;
pub mod script_loader;
pub mod special_power_strikes;
pub mod terrain;
pub mod thing;
pub mod units;
pub mod victory;
pub mod victory_conditions;
pub mod weapon_bootstrap;

pub use buildings::*;
pub use combat::*;
pub use combat_particles::{CombatParticleKind, CombatParticleRegistry, CombatParticleSystemEntry};
pub use game_logic::*;
pub use host_ambush::{
    HostAmbushKind, HostAmbushMission, HostAmbushPhase, HostAmbushRegistry,
    AMBUSH_RESIDUAL_TEMPLATE, AMBUSH_SPAWN_RADIUS, GLA_AMBUSH1_UNIT_COUNT,
};
pub use host_angry_mob::{
    angry_mob_damage_for_tick, is_angry_mob_nexus_template, is_legal_angry_mob_damage_target,
    HostAngryMobRegistry, HostAngryMobState, ANGRY_MOB_ATTACK_RANGE,
    ANGRY_MOB_DAMAGE_PER_MEMBER_TICK, ANGRY_MOB_EXPAND_INTERVAL_FRAMES, ANGRY_MOB_INITIAL_MEMBERS,
    ANGRY_MOB_MAX_MEMBERS, ANGRY_MOB_RESIDUAL_WEAPON, ANGRY_MOB_TICK_INTERVAL_FRAMES,
    UPGRADE_GLA_ARM_THE_MOB,
};
pub use host_aurora_bomb::{
    aurora_bomb_damage_at_distance, aurora_bomb_kind_for_template, aurora_bomb_weapon,
    is_aurora_aircraft_template, HostAuroraBombKind, HostAuroraBombMission, HostAuroraBombPhase,
    HostAuroraBombRegistry, AURORA_BOMB_ATTACK_RANGE, AURORA_BOMB_DAMAGE,
    AURORA_BOMB_DIVE_DELAY_FRAMES, AURORA_BOMB_PRIMARY_WEAPON, AURORA_BOMB_RADIUS,
    AURORA_FUEL_AIR_DAMAGE, AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, AURORA_FUEL_AIR_RADIUS,
};
pub use host_avenger::{
    is_avenger_template, HostAvengerRegistry, AVENGER_AIR_LASER, AVENGER_TARGET_DESIGNATOR,
    FAERIE_FIRE_ROF_MULTIPLIER,
};
pub use host_base_defense::{
    gattling_building_air_weapon, gattling_building_ground_weapon, is_base_defense_structure,
    is_dual_slot_base_defense, is_gattling_cannon_structure, is_laser_patriot_template,
    is_legal_base_defense_target, is_legal_supw_patriot_emp_target, is_patriot_battery_structure,
    is_stinger_site_structure, is_supw_patriot_template, patriot_air_weapon,
    patriot_air_weapon_for_template, patriot_ground_weapon, patriot_ground_weapon_for_template,
    preferred_dual_defense_slot, preferred_gattling_building_slot,
    primary_weapon_name_for_defense, secondary_weapon_name_for_defense, stinger_air_weapon,
    stinger_ground_weapon, supw_patriot_emp_until_frame, GATTLING_BUILDING_AIR_DAMAGE,
    GATTLING_BUILDING_BASE_DELAY_FRAMES, GATTLING_BUILDING_GROUND_DAMAGE,
    GATTLING_BUILDING_GROUND_RANGE, GATTLING_BUILDING_PRIMARY_WEAPON,
    GATTLING_BUILDING_SECONDARY_WEAPON, LAZR_PATRIOT_AIR_DAMAGE, LAZR_PATRIOT_GROUND_DAMAGE,
    LAZR_PATRIOT_PRIMARY_WEAPON, LAZR_PATRIOT_SECONDARY_WEAPON, PATRIOT_PRIMARY_WEAPON,
    PATRIOT_SECONDARY_WEAPON, STINGER_PRIMARY_WEAPON, STINGER_SECONDARY_WEAPON,
    SUPW_PATRIOT_AIR_DAMAGE, SUPW_PATRIOT_EMP_DURATION_FRAMES, SUPW_PATRIOT_EMP_RADIUS,
    SUPW_PATRIOT_GROUND_DAMAGE, SUPW_PATRIOT_PRIMARY_WEAPON, SUPW_PATRIOT_SECONDARY_WEAPON,
};
pub use host_battle_bus::{
    battle_bus_passenger_dummy_weapon, is_battle_bus_template, rider_has_viable_weapon,
    HostBattleBusRegistry, BATTLE_BUS_TRANSPORT_SLOTS,
};
pub use host_battlemaster::{
    battlemaster_weapon, has_nationalism_upgrade, has_uranium_shells_upgrade,
    is_battlemaster_template, should_apply_battlemaster_residual, BATTLE_MASTER_DAMAGE,
    BATTLE_MASTER_RANGE, BATTLE_MASTER_TANK_GUN, UPGRADE_CHINA_URANIUM_SHELLS, UPGRADE_NATIONALISM,
};
pub use host_black_market::{
    deposit_interval_frames_from_ms, is_black_market_structure, is_black_market_template,
    is_legal_black_market_income_source, HostBlackMarketRegistry, BLACK_MARKET_DEPOSIT_AMOUNT,
    BLACK_MARKET_DEPOSIT_AUDIO, BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
    BLACK_MARKET_DEPOSIT_TIMING_MS,
};
pub use host_bomb_truck_detonate::{
    bomb_truck_blast_damage_at, is_bomb_truck_template as is_bomb_truck_detonate_template,
    BombTruckDetonationProfile, HostBombTruckDetonateRegistry, BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE,
    BOMB_TRUCK_DEFAULT_PRIMARY_RADIUS, BOMB_TRUCK_HE_PRIMARY_DAMAGE, BOMB_TRUCK_HE_PRIMARY_RADIUS,
    UPGRADE_BOMB_TRUCK_BIO, UPGRADE_BOMB_TRUCK_HE,
};
pub use host_bunker_buster::{
    bunker_buster_structure_damage, is_bunker_buster_carrier, is_bunker_structure_name,
    is_kill_garrisoned_clearer, kill_garrisoned_count, should_apply_bunker_buster,
    should_apply_kill_garrisoned, HostBunkerBusterRegistry, BUNKER_BUSTER_STRUCTURE_DAMAGE_MULT,
    STEALTH_JET_MISSILE_WEAPON, UPGRADE_AMERICA_BUNKER_BUSTERS,
};
pub use host_car_bomb::{
    car_bomb_damage_at_distance, suicide_car_bomb_weapon, HostCarBombRegistry,
    CAR_BOMB_CONVERT_AUDIO, CAR_BOMB_DETONATE_AUDIO, HIJACK_AUDIO, SUICIDE_CAR_BOMB_ATTACK_RANGE,
    SUICIDE_CAR_BOMB_DAMAGE, SUICIDE_CAR_BOMB_RADIUS,
};
pub use host_cash_bounty::{
    cash_bounty_percent_for_science, compute_bounty_award, HostCashBountyRegistry,
    CASH_BOUNTY1_PERCENT, CASH_BOUNTY2_PERCENT, CASH_BOUNTY3_PERCENT, SCIENCE_CASH_BOUNTY1,
    SCIENCE_CASH_BOUNTY2, SCIENCE_CASH_BOUNTY3,
};
pub use host_cia_intelligence::{
    HostCiaIntelligence, HostCiaIntelligenceRegistry, HostCiaIntelligenceSpiedUnit,
    CIA_INTELLIGENCE_ACTIVATE_AUDIO, CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS,
    CIA_INTELLIGENCE_DURATION_FRAMES,
};
pub use host_cleanup_area::{
    in_cleanup_radius_2d, is_cleanup_area_caster, HostCleanupArea, HostCleanupAreaRegistry,
    CLEANUP_AREA_ACTIVATE_AUDIO, CLEANUP_AREA_HAZARD_AUDIO, CLEANUP_AREA_MINE_AUDIO,
    HOST_CLEANUP_AREA_RADIUS, HOST_CLEANUP_MAX_MOVE_DISTANCE, HOST_CLEANUP_SCAN_RANGE,
};
pub use host_comanche_rocket_pods::{
    comanche_antitank_damage_at, comanche_antitank_weapon, comanche_cannon_weapon,
    comanche_rocket_pod_weapon, is_comanche_template, rocket_pod_damage_at_distance,
    rocket_pod_ground_fire_active, should_apply_comanche_antitank_residual,
    should_apply_comanche_cannon_residual, should_apply_comanche_residual,
    should_apply_rocket_pod_area_attack, COMANCHE_ANTITANK_WEAPON, COMANCHE_AT_PRIMARY_DAMAGE,
    COMANCHE_AT_SECONDARY_RADIUS, COMANCHE_CANNON_DAMAGE, COMANCHE_PRIMARY_WEAPON,
    COMANCHE_ROCKET_POD_WEAPON, ROCKET_POD_ATTACK_RANGE, ROCKET_POD_PRIMARY_DAMAGE,
    ROCKET_POD_SECONDARY_DAMAGE, ROCKET_POD_SECONDARY_RADIUS, UPGRADE_COMANCHE_ROCKET_PODS,
};
pub use host_combat_chinook::{
    combat_chinook_rider_has_viable_weapon, is_combat_chinook_template, is_passenger_dummy_weapon,
    listening_outpost_upgraded_dummy_weapon, HostCombatChinookRegistry,
    COMBAT_CHINOOK_TRANSPORT_SLOTS,
};
pub use host_combat_cycle::{
    combat_cycle_weapon_for_rider, default_spawn_rider, default_spawn_rider_for_template,
    is_combat_cycle_template, rider_from_template_name, should_apply_combat_cycle_residual,
    CombatCycleRider, COMBAT_CYCLE_TRANSPORT_SLOTS, REBEL_BIKER_MG, REBEL_MG_DAMAGE,
    TUNNEL_DEFENDER_BIKER_ROCKET,
};
pub use host_dragon_tank::{
    dragon_flame_damage_at, dragon_flame_stats, dragon_flame_weapon, dragon_flame_weapon_name,
    has_black_napalm_upgrade, is_dragon_tank_template, should_apply_dragon_flame_residual,
    DRAGON_PRIMARY_DAMAGE, DRAGON_RANGE, DRAGON_SECONDARY_DAMAGE, DRAGON_SECONDARY_RADIUS,
    DRAGON_TANK_FLAME_WEAPON, DRAGON_TANK_FLAME_WEAPON_UPGRADED, UPGRADE_CHINA_BLACK_NAPALM,
};
pub use host_ecm_jam::{is_ecm_jammer, is_legal_ecm_jam_target, HOST_ECM_JAM_RADIUS};
pub use host_emergency_repair::{
    is_legal_emergency_repair_target, HostEmergencyRepair, HostEmergencyRepairLevel,
    HostEmergencyRepairRegistry, EMERGENCY_REPAIR_ACTIVATE_AUDIO, HOST_EMERGENCY_REPAIR_RADIUS,
};
pub use host_emp_pulse::{
    is_legal_emp_disable_target, HostEmpPulse, HostEmpPulseRegistry, EMP_PULSE_ACTIVATE_AUDIO,
    EMP_PULSE_DISABLED_DURATION_FRAMES, HOST_EMP_PULSE_RADIUS,
};
pub use host_firewall::{
    HostFireWall, HostFireWallRegistry, HostFireWallSegment, FIREWALL_ACTIVATE_AUDIO,
    FIREWALL_DAMAGE_PER_TICK, FIREWALL_DURATION_FRAMES, FIREWALL_SEGMENT_RADIUS,
    FIREWALL_TICK_INTERVAL_FRAMES,
};
pub use host_frenzy::{
    is_legal_frenzy_target, HostFrenzy, HostFrenzyLevel, HostFrenzyRegistry, FRENZY_ACTIVATE_AUDIO,
    HOST_FRENZY_RADIUS,
};
pub use host_strategy_center::{
    is_legal_battle_plan_member, is_strategy_center_template, HostBattlePlan,
    HostBattlePlanRegistry, HostBattlePlanSelection, BOMBARDMENT_DAMAGE_MULT,
    HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR, SEARCH_AND_DESTROY_RANGE_MULT,
    SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR,
};
pub use host_gattling_tank::{
    gattling_air_weapon, gattling_delay_frames_for_level, gattling_ground_weapon,
    gattling_on_shot_fired, has_chain_guns_upgrade, is_gattling_tank_template,
    preferred_gattling_slot, GattlingFireLevel, GATTLING_BASE_DELAY_FRAMES, GATTLING_GROUND_DAMAGE,
    GATTLING_GROUND_RANGE, GATTLING_TANK_GUN, GATTLING_TANK_GUN_AIR, UPGRADE_CHINA_CHAIN_GUNS,
};
pub use host_gla_rebel::{
    is_gla_rebel_template, rebel_weapon, should_apply_rebel_residual, REBEL_DAMAGE,
    REBEL_MACHINE_GUN, REBEL_RANGE, UPGRADE_GLA_AP_BULLETS as REBEL_UPGRADE_AP_BULLETS,
};
pub use host_gla_worker::{
    is_gla_worker_template, residual_worker_shoes_drop_off_boost, worker_residual_speed,
    HostGlaWorkerRegistry, UPGRADE_GLA_WORKER_SHOES, WORKER_SHOES_SPEED, WORKER_SHOES_SUPPLY_BOOST,
};
pub use host_gps_scrambler::{
    is_legal_gps_scrambler_target, HostGpsScrambler, HostGpsScramblerRegistry,
    GPS_SCRAMBLER_ACTIVATE_AUDIO, HOST_GPS_SCRAMBLER_RADIUS,
};
pub use host_hacker_income::{
    cash_amount_for_level, cash_interval_frames, is_hacker_template, is_internet_center_template,
    is_legal_hacker_income_source, HostHackerIncomeRegistry, HACKER_CASH_INTERVAL_FAST_FRAMES,
    HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_PING_AUDIO, HACKER_CASH_REGULAR,
    HACKER_XP_PER_CASH_UPDATE,
};
pub use host_heal::{
    is_ambulance_healer, is_legal_ambulance_infantry_heal_target, HOST_AMBULANCE_HEAL_RADIUS,
    HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC,
};
pub use host_helix_minigun::{
    helix_minigun_weapon, is_legal_helix_minigun_target, should_apply_helix_minigun_residual,
    HELIX_MINIGUN_DAMAGE, HELIX_MINIGUN_DELAY_FRAMES, HELIX_MINIGUN_RANGE, HELIX_MINIGUN_WEAPON,
};
pub use host_helix_napalm::{
    helix_napalm_blast_damage_at, helix_napalm_unlocked, is_helix_napalm_caster,
    HostHelixNapalmRegistry, HELIX_FIRESTORM_DAMAGE_PER_TICK, HELIX_FIRESTORM_DURATION_FRAMES,
    HELIX_FIRESTORM_RADIUS, HELIX_FIRESTORM_TICK_INTERVAL_FRAMES, HELIX_NAPALM_PRIMARY_DAMAGE,
    HELIX_NAPALM_PRIMARY_RADIUS, HELIX_NAPALM_SECONDARY_DAMAGE, HELIX_NAPALM_SECONDARY_RADIUS,
    UPGRADE_HELIX_NAPALM_BOMB,
};
pub use host_hero_abilities::{
    HostHeroAbilityRegistry, DISABLE_VEHICLE_HACK_AUDIO, DISABLE_VEHICLE_HACK_DURATION_FRAMES,
    SNIPE_VEHICLE_AUDIO, STEAL_CASH_AUDIO, STEAL_CASH_DEFAULT_AMOUNT,
};
pub use host_humvee::{is_humvee_template, HUMVEE_MISSILE_WEAPON_AIR, HUMVEE_TRANSPORT_SLOTS};
pub use host_inferno_cannon::{
    has_black_napalm_upgrade as has_inferno_black_napalm_upgrade, is_inferno_cannon_template,
    HostInfernoFireZone, HostInfernoFireZoneRegistry, INFERNO_CANNON_FIRE_AUDIO,
    INFERNO_CANNON_PRIMARY_WEAPON, INFERNO_CANNON_SHELL_DAMAGE, INFERNO_CANNON_UPGRADED_WEAPON,
    INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
    INFERNO_FIRE_DURATION_FRAMES, INFERNO_FIRE_RADIUS, INFERNO_FIRE_TICK_INTERVAL_FRAMES,
};
pub use host_leaflet_drop::{
    is_legal_leaflet_disable_target, HostLeafletDropKind, HostLeafletDropMission,
    HostLeafletDropPhase, HostLeafletDropRegistry, HOST_LEAFLET_RADIUS, LEAFLET_DELAY_FRAMES,
    LEAFLET_DISABLED_DURATION_FRAMES,
};
pub use host_listening_outpost::{
    is_listening_outpost_template, listening_outpost_detection_range,
    listening_outpost_spawn_is_detector, HostListeningOutpostRegistry,
    LISTENING_OUTPOST_DETECTION_RANGE, LISTENING_OUTPOST_TRANSPORT_SLOTS,
};
pub use host_marauder::{
    is_marauder_template, marauder_weapon_for_tier, marauder_weapon_name_for_tier,
    marauder_weapon_stats, should_apply_marauder_residual, MarauderWeaponTier, MARAUDER_DAMAGE,
    MARAUDER_RANGE, MARAUDER_TANK_GUN, MARAUDER_TANK_GUN_UPGRADE_ONE,
    MARAUDER_TANK_GUN_UPGRADE_TWO,
};
pub use host_mines::{
    can_clear_mine_kind, demo_trap_damage_at, demo_trap_profile, is_mine_clearer, DemoTrapProfile,
    HostMineData, HostMineDetonateReason, HostMineDetonationPlan, HostMineKind,
    DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE, MINE_CLEARED_AUDIO,
};
pub use host_missile_defender::{
    is_missile_defender_template, missile_defender_laser_guided_weapon,
    missile_defender_primary_weapon, should_apply_missile_defender_residual,
    MISSILE_DEFENDER_DAMAGE, MISSILE_DEFENDER_LASER_GUIDED_WEAPON, MISSILE_DEFENDER_MISSILE_WEAPON,
    MISSILE_DEFENDER_PRIMARY_RANGE,
};
pub use host_neutron_shell::{
    is_nuke_cannon_template, neutron_effect_for_target, should_apply_neutron_blast, NeutronEffect,
    HOST_NEUTRON_BLAST_RADIUS, NUKE_CANNON_NEUTRON_WEAPON, NUKE_CANNON_PRIMARY_WEAPON,
    UPGRADE_CHINA_NEUTRON_SHELLS,
};
pub use host_nuke_cannon::{
    is_nuke_cannon_template as is_nuke_cannon_primary_template, nuke_cannon_primary_damage_at,
    should_apply_nuke_cannon_primary, HostNukeCannonRegistry, MEDIUM_RADIATION_DAMAGE_PER_TICK,
    MEDIUM_RADIATION_RADIUS, NUKE_CANNON_PRIMARY_DAMAGE, NUKE_CANNON_PRIMARY_RADIUS,
};
pub use host_oil_derrick::{
    is_legal_oil_derrick_income_source, is_oil_derrick_structure, is_oil_derrick_template,
    HostOilDerrickRegistry, OIL_DERRICK_CAPTURE_BONUS_AUDIO, OIL_DERRICK_DEPOSIT_AMOUNT,
    OIL_DERRICK_DEPOSIT_AUDIO, OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES, OIL_DERRICK_DEPOSIT_TIMING_MS,
    OIL_DERRICK_INITIAL_CAPTURE_BONUS,
};
pub use host_overlord_addons::{
    is_emperor_template, is_helix_template, is_overlord_family_host, is_overlord_tank_template,
    overlord_gattling_air_weapon, should_apply_overlord_gattling_residual,
    HostOverlordAddonRegistry, HELIX_TRANSPORT_SLOTS, OVERLORD_GATTLING_AIR_DAMAGE,
    OVERLORD_GATTLING_GROUND_DAMAGE, OVERLORD_PROPAGANDA_RADIUS, UPGRADE_HELIX_GATTLING,
    UPGRADE_HELIX_PROPAGANDA, UPGRADE_OVERLORD_GATTLING, UPGRADE_OVERLORD_PROPAGANDA,
};
pub use host_paradrop::{
    HostParadropKind, HostParadropMission, HostParadropPhase, HostParadropRegistry,
    AMERICA_PARADROP_UNIT_COUNT, PARADROP_DROP_SPACING, PARADROP_RESIDUAL_TEMPLATE,
};
pub use host_pathfinder::{
    is_pathfinder_template, pathfinder_detection_range, pathfinder_spawn_is_detector,
    PATHFINDER_DETECTION_RANGE, PATHFINDER_SNIPER_WEAPON,
};
pub use host_point_defense::{
    is_missile_name_residual, is_point_defense_carrier, is_primary_intercept_target, pdl_damage,
    pdl_delay_frames, pdl_fire_range, pdl_scan_range, AVENGER_PDL_FIRE_RANGE,
    PALADIN_PDL_FIRE_RANGE, PDL_INTERCEPT_AUDIO,
};
pub use host_propaganda::{
    is_legal_propaganda_target, is_propaganda_tower, propaganda_heal_amount,
    HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC, HOST_PROPAGANDA_TOWER_RADIUS,
    HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC, UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
};
pub use host_radar::{
    is_legal_radar_provider, is_radar_command_center_template, is_radar_provider_template,
    is_radar_van_template, HostRadarRegistry, RADAR_OFFLINE_AUDIO, RADAR_ONLINE_AUDIO,
};
pub use host_radar_scan::{
    HostRadarScan, HostRadarScanRegistry, RADAR_SCAN_ACTIVATE_AUDIO, RADAR_SCAN_DURATION_FRAMES,
    RADAR_SCAN_RADIUS,
};
pub use host_red_guard::{
    is_red_guard_template, red_guard_weapon, should_apply_bayonet_residual,
    should_apply_red_guard_residual, REDGUARD_DAMAGE, REDGUARD_MACHINE_GUN, REDGUARD_RANGE,
};
pub use host_rpg_trooper::{
    is_rpg_trooper_template, rpg_trooper_weapon, should_apply_rpg_trooper_residual,
    RPG_TROOPER_DAMAGE, RPG_TROOPER_RANGE, TUNNEL_DEFENDER_ROCKET_WEAPON,
    UPGRADE_GLA_AP_ROCKETS as RPG_UPGRADE_AP_ROCKETS,
};
pub use host_saboteur::{
    classify_sabotage_target, is_saboteur_template, HostSaboteurRegistry, SaboteurEffectKind,
    SABOTEUR_STEAL_CASH_AMOUNT, SABOTEUR_SUCCESS_AUDIO,
};
pub use host_sentry_drone::{
    is_sentry_drone_template, sentry_detection_range, sentry_spawn_is_detector,
    SENTRY_DETECTION_RANGE, SENTRY_DRONE_GUN_WEAPON, UPGRADE_AMERICA_SENTRY_DRONE_GUN,
};
pub use host_slave_drones::{
    is_hellfire_drone_template, is_scout_drone_template, is_slave_drone_master_template,
    scout_detection_range, scout_spawn_is_detector, SlaveDroneKind, HELLFIRE_MISSILE_WEAPON,
    SCOUT_DETECTION_RANGE, UPGRADE_AMERICA_HELLFIRE_DRONE, UPGRADE_AMERICA_SCOUT_DRONE,
};
pub use host_sneak_attack::{
    is_legal_sneak_shockwave_target, HostSneakAttackKind, HostSneakAttackMission,
    HostSneakAttackPhase, HostSneakAttackRegistry, GLA_SNEAK_TUNNEL_TEMPLATE,
    HOST_SNEAK_ATTACK_RADIUS, SNEAK_ATTACK_RESIDUAL_TEMPLATE, SNEAK_ATTACK_SHOCKWAVE_DAMAGE,
    SNEAK_ATTACK_SHOCKWAVE_RADIUS, SNEAK_ATTACK_SPAWN_DELAY_FRAMES,
};
pub use host_spy_satellite::{
    HostSpySatellite, HostSpySatelliteRegistry, SPY_SATELLITE_ACTIVATE_AUDIO,
    SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_RADIUS,
};
pub use host_stealth_fighter::{
    is_stealth_fighter_science, is_stealth_fighter_template, player_may_produce_stealth_aircraft,
    requires_stealth_fighter_science, should_apply_stealth_fighter_residual,
    stealth_fighter_damage_at, stealth_fighter_weapon, HostStealthFighterRegistry,
    AMERICA_JET_STEALTH_FIGHTER, SCIENCE_STEALTH_FIGHTER, STEALTH_FIGHTER_DAMAGE,
    STEALTH_FIGHTER_MIN_RANGE, STEALTH_FIGHTER_PRIMARY_RADIUS, STEALTH_FIGHTER_RANGE,
    STEALTH_JET_MISSILE_WEAPON as STEALTH_FIGHTER_MISSILE_WEAPON, USA_STEALTH_FIGHTER,
};
pub use host_supply_drop_zone::{
    drop_cash_amount, drop_interval_frames_from_ms, is_legal_supply_drop_zone_income_source,
    is_supply_drop_zone_structure, is_supply_drop_zone_template, HostSupplyDropZoneRegistry,
    SUPPLY_DROP_ZONE_CRATE_COUNT, SUPPLY_DROP_ZONE_DELAY_MS, SUPPLY_DROP_ZONE_DROP_AUDIO,
    SUPPLY_DROP_ZONE_DROP_CASH, SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES,
    SUPPLY_DROP_ZONE_INTERVAL_FRAMES, SUPPLY_DROP_ZONE_MONEY_PER_CRATE,
    SUPPLY_DROP_ZONE_SUPPLY_LINES_BOOST_PER_CRATE,
};
pub use host_tank_hunter::{
    is_tank_hunter_template, should_apply_tank_hunter_residual, tank_hunter_weapon,
    TANK_HUNTER_DAMAGE, TANK_HUNTER_MISSILE_WEAPON, TANK_HUNTER_RANGE,
};
pub use host_terrorist::{
    is_demo_general_template as is_terrorist_demo_general_template, is_terrorist_template,
    should_apply_terrorist_residual, suicide_dynamite_damage_at,
    suicide_dynamite_damage_at_profile, terrorist_death_profile, terrorist_suicide_weapon,
    terrorist_suicide_weapon_for_profile, TerroristDeathProfile, SUICIDE_DYNAMITE_PACK,
    SUICIDE_DYNAMITE_PRIMARY_DAMAGE, SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO,
    SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA, SUICIDE_DYNAMITE_PRIMARY_RADIUS,
    SUICIDE_DYNAMITE_SECONDARY_DAMAGE, SUICIDE_DYNAMITE_SECONDARY_RADIUS, TERRORIST_SUICIDE_WEAPON,
};
pub use host_troop_crawler::{
    is_troop_crawler_template, resolve_payload_template_name,
    should_apply_troop_crawler_assault_deploy, troop_crawler_assault_weapon,
    troop_crawler_detection_range, troop_crawler_spawn_is_detector, HostTroopCrawlerRegistry,
    TROOP_CRAWLER_ASSAULT_RANGE, TROOP_CRAWLER_ASSAULT_WEAPON, TROOP_CRAWLER_DETECTION_RANGE,
    TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT, TROOP_CRAWLER_TRANSPORT_SLOTS, TROOP_CRAWLER_VISION_RANGE,
};
pub use host_tunnel_network::{
    is_tunnel_network_template, tunnel_network_gun_weapon, unit_can_use_tunnel,
    HostTunnelNetworkRegistry, MAX_TUNNEL_CAPACITY, TUNNEL_FULL_HEAL_FRAMES, TUNNEL_NETWORK_GUN,
    TUNNEL_NETWORK_GUN_DAMAGE, TUNNEL_NETWORK_GUN_RANGE,
};
pub use host_upgrades::{
    HostUpgradeKind, HostUpgradePhase, HostUpgradeRegistry, HostUpgradeResearch,
};
pub use host_usa_pilot::{
    is_pilot_template, is_recrewable_unmanned_vehicle, should_recrew_on_enter,
    HostUsaPilotRegistry, PILOT_RECREW_AUDIO,
};
pub use host_usa_tanks::{
    is_composite_armor_unit_template, is_crusader_template, is_laser_general_tank_template,
    is_paladin_template, usa_tank_gun_weapon_for_template, CRUSADER_TANK_GUN,
    LAZR_CRUSADER_TANK_GUN, LAZR_CRUSADER_TANK_GUN_DAMAGE, LAZR_PALADIN_TANK_GUN,
    LAZR_PALADIN_TANK_GUN_DAMAGE, PALADIN_TANK_GUN, UPGRADE_AMERICA_COMPOSITE_ARMOR,
};
pub use locomotor_bootstrap::{
    ensure_host_locomotor_store, locomotor_name_for_unit, resolve_host_movement,
    BASIC_HUMAN_LOCOMOTOR, BATTLE_MASTER_LOCOMOTOR, CRUSADER_LOCOMOTOR, HUMVEE_LOCOMOTOR,
    REDGUARD_LOCOMOTOR, SCORPION_LOCOMOTOR, TECHNICAL_LOCOMOTOR,
};
pub use mission_scripts::*;
pub use object::*;
pub use partition_manager::*;
pub use pathfinding::*;
pub use radar_notifications::*;
pub use resources::*;
pub use script_events::*;
pub use script_loader::*;
pub use special_power_strikes::{
    HostRadiationField, HostSpecialPowerStrike, HostSpecialPowerStrikeRegistry, HostStrikePhase,
    HostSuperweaponKind, NUKE_RADIATION_DAMAGE_PER_TICK, NUKE_RADIATION_DURATION_FRAMES,
    NUKE_RADIATION_RADIUS, NUKE_RADIATION_TICK_INTERVAL_FRAMES,
};
pub use terrain::*;
pub use thing::*;
pub use units::*;
pub use victory::*;
pub use victory_conditions::*;
pub use weapon_bootstrap::{
    ensure_host_weapon_store, primary_weapon_name_for_unit, secondary_weapon_name_for_unit,
    GATTLING_BUILDING_PRIMARY_WEAPON as HOST_GATTLING_BUILDING_PRIMARY_WEAPON,
    GLA_REBEL_PRIMARY_WEAPON, HUMVEE_PRIMARY_WEAPON, HUMVEE_SECONDARY_WEAPON,
    PATRIOT_PRIMARY_WEAPON as HOST_PATRIOT_PRIMARY_WEAPON, RANGER_PRIMARY_WEAPON,
    RANGER_SECONDARY_WEAPON, REDGUARD_PRIMARY_WEAPON,
};

use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for game objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId(pub u32);

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectId = ObjectId(0);

/// Team/faction identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    GLA,
    USA,
    China,
    Neutral,
}

impl Team {
    /// Convert player ID to team
    pub fn from_player_id(player_id: u32) -> Self {
        match player_id {
            0 => Team::USA,
            1 => Team::China,
            2 => Team::GLA,
            _ => Team::Neutral,
        }
    }

    /// Get the team's primary color for UI display
    pub fn get_color(&self) -> [f32; 4] {
        match self {
            Team::USA => [0.2, 0.4, 0.8, 1.0],     // Blue
            Team::China => [0.8, 0.2, 0.2, 1.0],   // Red
            Team::GLA => [0.8, 0.6, 0.2, 1.0],     // Desert/Tan
            Team::Neutral => [0.5, 0.5, 0.5, 1.0], // Gray
        }
    }

    /// Get the team's name as a string
    pub fn get_name(&self) -> &'static str {
        match self {
            Team::USA => "USA",
            Team::China => "China",
            Team::GLA => "GLA",
            Team::Neutral => "Neutral",
        }
    }

    /// Get the team's secondary color for highlights
    pub fn get_highlight_color(&self) -> [f32; 4] {
        match self {
            Team::USA => [0.4, 0.6, 1.0, 1.0],     // Light blue
            Team::China => [1.0, 0.4, 0.4, 1.0],   // Light red
            Team::GLA => [1.0, 0.8, 0.4, 1.0],     // Light tan
            Team::Neutral => [0.7, 0.7, 0.7, 1.0], // Light gray
        }
    }
}

/// Object kinds for type checking and behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KindOf {
    Structure,
    Infantry,
    Vehicle,
    Aircraft,
    Projectile,
    Resource,
    Selectable,
    Attackable,
    CommandCenter,
    Worker,
    Hero,
    SupplyCenter,
    PowerPlant,
    FSBarracks,
    FSWarFactory,
    FSAirfield,
    FSInternetCenter,
    FSPower,
    FSBaseDefense,
    FSSupplyDropzone,
    FSSupplyCenter,
    FSSuperweapon,
    FSStrategyCenter,
    FSFake,
    FSTechnology,
    FSBlackMarket,
    FSAdvancedTech,
    Harvestable,
    /// C++ KINDOF_POWERED: object gets DISABLED_UNDERPOWERED when player
    /// power consumption exceeds supply (defenses, factories, etc).
    Powered,
}

/// Object status flags
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ObjectStatus {
    pub destroyed: bool,
    pub under_construction: bool,
    pub selected: bool,
    pub moving: bool,
    pub attacking: bool,
    pub airborne_target: bool,
    /// C++ OBJECT_STATUS_STEALTHED residual.
    pub stealthed: bool,
    /// C++ OBJECT_STATUS_DETECTED residual (revealed by detector / temporary reveal).
    /// Stealthed + not detected => not targetable / not visible to enemies.
    pub detected: bool,
    /// C++ DISABLED_UNDERPOWERED: set when player's power supply < demand.
    pub disabled_underpowered: bool,
    /// C++ DISABLED_UNMANNED residual (DAMAGE_KILLPILOT / Jarmen Kell snipe).
    /// Vehicle stays alive but cannot act; team is typically Neutral.
    #[serde(default)]
    pub disabled_unmanned: bool,
    /// C++ DISABLED_HACKED residual (Black Lotus DisableVehicleHack).
    /// Vehicle stays alive on its team but cannot move/attack until frame expires.
    #[serde(default)]
    pub disabled_hacked: bool,
    /// Absolute host logic frame when DISABLED_HACKED expires (0 = inactive).
    #[serde(default)]
    pub disabled_hacked_until_frame: u32,
    /// C++ DISABLED_EMP residual (EMPUpdate / SuperweaponEMPPulse).
    /// Vehicle/structure stays alive but cannot move/attack/produce until frame expires.
    #[serde(default)]
    pub disabled_emp: bool,
    /// Absolute host logic frame when DISABLED_EMP expires (0 = inactive).
    #[serde(default)]
    pub disabled_emp_until_frame: u32,
    /// Host ECM tank / jammer residual: weapons cannot fire while inside jam radius.
    /// C++ DISABLED_SUBDUED cannot-fire residual (Microwave/ECM vehicle disabler).
    /// Fail-closed: continuous aura (not full subdual damage accumulate/heal).
    #[serde(default)]
    pub weapons_jammed: bool,
    /// C++ DISABLED_SUBDUED residual on structures cooked by Microwave Tank
    /// (MicrowaveTankBuildingDisabler / SUBDUAL_BUILDING). Full disable while cooked
    /// (production / powered functions stop). Fail-closed continuous while attacking.
    #[serde(default)]
    pub disabled_subdued: bool,
    /// C++ OBJECT_STATUS_IS_CARBOMB residual (ConvertToCarBombCrateCollide).
    /// Vehicle uses SuicideCarBomb weapon set residual and detonates on attack fire.
    #[serde(default)]
    pub is_carbomb: bool,
    /// C++ OBJECT_STATUS_HIJACKED residual (ConvertToHijackedVehicleCrateCollide).
    #[serde(default)]
    pub hijacked: bool,
    /// C++ OBJECT_STATUS_DISGUISED residual (Bomb Truck StealthUpdate disguise).
    /// Disguised units are not pure-stealth invisible; enemies see disguise team.
    #[serde(default)]
    pub disguised: bool,
    /// C++ OBJECT_STATUS_FAERIE_FIRE residual (AvengerTargetDesignator paint).
    /// Attackers shooting a painted target gain TARGET_FAERIE_FIRE 150% ROF.
    #[serde(default)]
    pub faerie_fire: bool,
    /// C++ OBJECT_STATUS_BOOBY_TRAPPED residual (Rebel SpecialAbilityBoobyTrap).
    #[serde(default)]
    pub booby_trapped: bool,
}

/// Basic geometry information for objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryInfo {
    pub position: Vec3,
    pub rotation: f32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub radius: f32,
}

impl Default for GeometryInfo {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: 0.0,
            bounds_min: Vec3::splat(-1.0),
            bounds_max: Vec3::splat(1.0),
            radius: 1.0,
        }
    }
}

/// Health and damage system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(max_health: f32) -> Self {
        Self {
            current: max_health,
            maximum: max_health,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.maximum
    }

    pub fn percentage(&self) -> f32 {
        if self.maximum > 0.0 {
            self.current / self.maximum
        } else {
            0.0
        }
    }

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
    }
}

/// Movement and pathfinding state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movement {
    pub target_position: Option<Vec3>,
    pub velocity: Vec3,
    pub max_speed: f32,
    pub acceleration: f32,
    pub turn_rate: f32,
    pub path: Vec<Vec3>,
    pub current_path_index: usize,
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            target_position: None,
            velocity: Vec3::ZERO,
            max_speed: 10.0,
            acceleration: 5.0,
            turn_rate: std::f32::consts::PI,
            path: Vec::new(),
            current_path_index: 0,
        }
    }
}

/// Economic resources
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Resources {
    pub supplies: u32,
    pub power: i32, // Can be negative
}

/// Experience and veterancy system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VeterancyLevel {
    Rookie,
    Veteran,
    Elite,
    Heroic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub current: f32,
    pub level: VeterancyLevel,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            current: 0.0,
            level: VeterancyLevel::Rookie,
        }
    }
}

/// Weapon and combat stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    pub damage: f32,
    pub range: f32,
    /// C++ parity (WeaponTemplate::m_minimumAttackRange): weapons cannot fire
    /// at targets closer than this distance.  0.0 = no minimum range.
    pub min_range: f32,
    pub reload_time: f32,
    pub last_fire_time: f32,
    pub ammo: Option<u32>,
    pub can_target_air: bool,
    pub can_target_ground: bool,
    /// C++ parity (WeaponTemplate::m_weaponSpeed): projectile travel speed.
    /// 0.0 = instant-hit (laser/flame weapons).
    pub projectile_speed: f32,
    /// C++ parity (WeaponTemplate::m_preAttackDelay): delay before firing
    /// after a target is acquired, in seconds.  0.0 = no delay.
    pub pre_attack_delay: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 25.0,
            range: 100.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ammo: None,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 200.0,
            pre_attack_delay: 0.0,
        }
    }
}
