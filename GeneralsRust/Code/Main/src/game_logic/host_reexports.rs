//! Flattened `pub use` of always-on host gameplay items.
//!
//! Keeps public paths such as `crate::game_logic::HostAmbushKind`.

#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_ai_ability_upgrade_residual::{
    honesty_ai_ability_upgrade_residual_pack_wave94, honesty_ai_state_residual_table_wave94,
    honesty_command_set_superweapon_residual_wave94,
    honesty_special_ability_residual_deepen_wave94, honesty_upgrade_name_table_residual_wave94,
};
pub use super::host_ai_path_combat_residual_wave105::{
    honesty_ai_group_residual_pack_wave105, honesty_ai_path_combat_residual_pack_wave105,
    honesty_ai_path_residual_deepen_pack_wave105,
    honesty_damage_application_residual_deepen_pack_wave105,
    honesty_veterancy_residual_deepen_pack_wave105,
    honesty_weapon_fire_residual_deepen_pack_wave105,
};
pub use super::host_ambush::{
    AMBUSH_RESIDUAL_TEMPLATE, AMBUSH_SPAWN_RADIUS, GLA_AMBUSH1_UNIT_COUNT, HostAmbushKind,
    HostAmbushMission, HostAmbushPhase, HostAmbushRegistry,
};
pub use super::host_angry_mob::{
    ANGRY_MOB_ATTACK_RANGE, ANGRY_MOB_DAMAGE_PER_MEMBER_TICK, ANGRY_MOB_EXPAND_INTERVAL_FRAMES,
    ANGRY_MOB_INITIAL_MEMBERS, ANGRY_MOB_MAX_MEMBERS, ANGRY_MOB_RESIDUAL_WEAPON,
    ANGRY_MOB_TICK_INTERVAL_FRAMES, HostAngryMobRegistry, HostAngryMobState,
    UPGRADE_GLA_ARM_THE_MOB, angry_mob_damage_for_tick, angry_mob_skips_stealthed_undetected,
    is_angry_mob_nexus_template, is_legal_angry_mob_damage_target,
};
pub use super::host_armor_residual::honesty_armor_residual_expand_wave92;
pub use super::host_armor_residual::honesty_armor_residual_expand_wave103;
pub use super::host_aurora_bomb::{
    AURORA_BOMB_ATTACK_RANGE, AURORA_BOMB_DAMAGE, AURORA_BOMB_DIVE_DELAY_FRAMES,
    AURORA_BOMB_PRIMARY_WEAPON, AURORA_BOMB_RADIUS, AURORA_FUEL_AIR_DAMAGE,
    AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES, AURORA_FUEL_AIR_RADIUS, AURORA_FUEL_AIR_SUPW_DAMAGE,
    AURORA_FUEL_AIR_SUPW_RADIUS, HostAuroraBombKind, HostAuroraBombMission, HostAuroraBombPhase,
    HostAuroraBombRegistry, aurora_bomb_damage_at_distance, aurora_bomb_kind_for_template,
    aurora_bomb_weapon, is_aurora_aircraft_template,
};
pub use super::host_avenger::{
    AVENGER_AIR_LASER, AVENGER_TARGET_DESIGNATOR, FAERIE_FIRE_ROF_MULTIPLIER, HostAvengerRegistry,
    is_avenger_template,
};
pub use super::host_base_defense::{
    GATTLING_BUILDING_AIR_DAMAGE, GATTLING_BUILDING_BASE_DELAY_FRAMES,
    GATTLING_BUILDING_GROUND_DAMAGE, GATTLING_BUILDING_GROUND_RANGE,
    GATTLING_BUILDING_PRIMARY_WEAPON, GATTLING_BUILDING_SECONDARY_WEAPON, LAZR_PATRIOT_AIR_DAMAGE,
    LAZR_PATRIOT_GROUND_DAMAGE, LAZR_PATRIOT_PRIMARY_WEAPON, LAZR_PATRIOT_SECONDARY_WEAPON,
    PATRIOT_PRIMARY_WEAPON, PATRIOT_SECONDARY_WEAPON, STINGER_PRIMARY_WEAPON,
    STINGER_SECONDARY_WEAPON, SUPW_PATRIOT_AIR_DAMAGE, SUPW_PATRIOT_EMP_DURATION_FRAMES,
    SUPW_PATRIOT_EMP_RADIUS, SUPW_PATRIOT_GROUND_DAMAGE, SUPW_PATRIOT_PRIMARY_WEAPON,
    SUPW_PATRIOT_SECONDARY_WEAPON, gattling_building_air_weapon, gattling_building_ground_weapon,
    is_base_defense_structure, is_dual_slot_base_defense, is_gattling_cannon_structure,
    is_laser_patriot_template, is_legal_base_defense_target, is_legal_supw_patriot_emp_target,
    is_patriot_battery_structure, is_stinger_site_structure, is_supw_patriot_template,
    patriot_air_weapon, patriot_air_weapon_for_template, patriot_ground_weapon,
    patriot_ground_weapon_for_template, preferred_dual_defense_slot,
    preferred_gattling_building_slot, primary_weapon_name_for_defense,
    secondary_weapon_name_for_defense, stinger_air_weapon, stinger_ground_weapon,
    supw_patriot_emp_until_frame,
};
pub use super::host_battle_bus::{
    BATTLE_BUS_TRANSPORT_SLOTS, HostBattleBusRegistry, battle_bus_passenger_dummy_weapon,
    is_battle_bus_template, rider_has_viable_weapon,
};
pub use super::host_battlemaster::{
    BATTLE_MASTER_DAMAGE, BATTLE_MASTER_RANGE, BATTLE_MASTER_TANK_GUN,
    UPGRADE_CHINA_URANIUM_SHELLS, UPGRADE_NATIONALISM, battlemaster_weapon,
    has_nationalism_upgrade, has_uranium_shells_upgrade, is_battlemaster_template,
    should_apply_battlemaster_residual,
};
pub use super::host_black_market::{
    BLACK_MARKET_DEPOSIT_AMOUNT, BLACK_MARKET_DEPOSIT_AUDIO, BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
    BLACK_MARKET_DEPOSIT_TIMING_MS, HostBlackMarketRegistry, deposit_interval_frames_from_ms,
    is_black_market_structure, is_black_market_template, is_legal_black_market_income_source,
};
pub use super::host_bomb_truck_detonate::{
    BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE, BOMB_TRUCK_DEFAULT_PRIMARY_RADIUS,
    BOMB_TRUCK_HE_PRIMARY_DAMAGE, BOMB_TRUCK_HE_PRIMARY_RADIUS, BombTruckDetonationProfile,
    HostBombTruckDetonateRegistry, UPGRADE_BOMB_TRUCK_BIO, UPGRADE_BOMB_TRUCK_HE,
    bomb_truck_blast_damage_at, is_bomb_truck_template as is_bomb_truck_detonate_template,
};
pub use super::host_bunker_buster::{
    BUNKER_BUSTER_STRUCTURE_DAMAGE_MULT, HostBunkerBusterRegistry, STEALTH_JET_MISSILE_WEAPON,
    UPGRADE_AMERICA_BUNKER_BUSTERS, bunker_buster_structure_damage, is_bunker_buster_carrier,
    is_bunker_structure_name, is_kill_garrisoned_clearer, kill_garrisoned_count,
    should_apply_bunker_buster, should_apply_kill_garrisoned,
};
pub use super::host_car_bomb::{
    CAR_BOMB_CONVERT_AUDIO, CAR_BOMB_DETONATE_AUDIO, HIJACK_AUDIO, HostCarBombRegistry,
    SUICIDE_CAR_BOMB_ATTACK_RANGE, SUICIDE_CAR_BOMB_DAMAGE, SUICIDE_CAR_BOMB_RADIUS,
    car_bomb_damage_at_distance, suicide_car_bomb_weapon,
};
pub use super::host_cash_bounty::{
    CASH_BOUNTY1_PERCENT, CASH_BOUNTY2_PERCENT, CASH_BOUNTY3_PERCENT, HostCashBountyRegistry,
    SCIENCE_CASH_BOUNTY1, SCIENCE_CASH_BOUNTY2, SCIENCE_CASH_BOUNTY3,
    cash_bounty_percent_for_science, compute_bounty_award,
};
pub use super::host_cia_intelligence::{
    CIA_INTELLIGENCE_ACTIVATE_AUDIO, CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS,
    CIA_INTELLIGENCE_DURATION_FRAMES, HostCiaIntelligence, HostCiaIntelligenceRegistry,
    HostCiaIntelligenceSpiedUnit,
};
pub use super::host_cleanup_area::{
    CLEANUP_AREA_ACTIVATE_AUDIO, CLEANUP_AREA_HAZARD_AUDIO, CLEANUP_AREA_MINE_AUDIO,
    HOST_CLEANUP_AREA_RADIUS, HOST_CLEANUP_MAX_MOVE_DISTANCE, HOST_CLEANUP_SCAN_RANGE,
    HostCleanupArea, HostCleanupAreaRegistry, in_cleanup_radius_2d, is_cleanup_area_caster,
};
pub use super::host_comanche_rocket_pods::{
    COMANCHE_ANTITANK_WEAPON, COMANCHE_AT_PRIMARY_DAMAGE, COMANCHE_AT_SECONDARY_RADIUS,
    COMANCHE_CANNON_DAMAGE, COMANCHE_PRIMARY_WEAPON, COMANCHE_ROCKET_POD_WEAPON,
    ROCKET_POD_ATTACK_RANGE, ROCKET_POD_PRIMARY_DAMAGE, ROCKET_POD_SECONDARY_DAMAGE,
    ROCKET_POD_SECONDARY_RADIUS, UPGRADE_COMANCHE_ROCKET_PODS, comanche_antitank_damage_at,
    comanche_antitank_weapon, comanche_cannon_weapon, comanche_rocket_pod_weapon,
    is_comanche_template, rocket_pod_damage_at_distance, rocket_pod_ground_fire_active,
    should_apply_comanche_antitank_residual, should_apply_comanche_cannon_residual,
    should_apply_comanche_residual, should_apply_rocket_pod_area_attack,
};
pub use super::host_combat_chinook::{
    COMBAT_CHINOOK_TRANSPORT_SLOTS, HostCombatChinookRegistry,
    combat_chinook_rider_has_viable_weapon, is_combat_chinook_template, is_passenger_dummy_weapon,
    listening_outpost_upgraded_dummy_weapon,
};
pub use super::host_combat_cycle::{
    COMBAT_CYCLE_TRANSPORT_SLOTS, CombatCycleRider, REBEL_BIKER_MG, REBEL_MG_DAMAGE,
    TUNNEL_DEFENDER_BIKER_ROCKET, combat_cycle_weapon_for_rider, default_spawn_rider,
    default_spawn_rider_for_template, is_combat_cycle_template, rider_from_template_name,
    should_apply_combat_cycle_residual,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_combat_sim_residual::{
    honesty_body_max_health_residual_table_wave92, honesty_combat_sim_residual_pack_wave92,
    honesty_science_name_table_residual_wave92,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_command_button_residual::honesty_command_button_superweapon_residual_pack_wave80;
pub use super::host_deliver_payload::{
    CARGO_PLANE_APPROACH_DELAY_FRAMES, CARGO_PLANE_DOOR_DELAY_FRAMES, CARGO_PLANE_DOOR_DELAY_MS,
    HostDeliverPayloadDropPlan, HostDeliverPayloadItemPlan, HostDeliverPayloadKind,
    HostDeliverPayloadMission, HostDeliverPayloadPhase, HostDeliverPayloadRegistry,
    PARADROP_CARGO_TRANSPORT, PARADROP_MAX_ATTEMPTS, PARADROP_PRE_OPEN_DISTANCE,
    PARADROP_PUT_IN_CONTAINER, SUPPLY_DROP_CARGO_TRANSPORT, SUPPLY_DROP_CRATE_SPACING,
    SUPPLY_DROP_DELIVERY_DISTANCE, SUPPLY_DROP_DROP_DELAY_FRAMES, SUPPLY_DROP_DROP_DELAY_MS,
    SUPPLY_DROP_DROP_OFFSET_Y, SUPPLY_DROP_MAX_ATTEMPTS, SUPPLY_DROP_PAYLOAD_COUNT,
    SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE, SUPPLY_DROP_PAYLOAD_TEMPLATE,
    SUPPLY_DROP_PRE_OPEN_DISTANCE, SUPPLY_DROP_PUT_IN_CONTAINER, drop_delay_frames_from_ms,
    residual_allowed_delivery_distance,
};
pub use super::host_demo_suicide_bomb::{
    DEMO_COMMAND_TERTIARY_SUICIDE, DEMO_DESTROYED_PRIMARY_DAMAGE, DEMO_DESTROYED_PRIMARY_RADIUS,
    DEMO_DESTROYED_SECONDARY_DAMAGE, DEMO_DESTROYED_SECONDARY_RADIUS, DEMO_DESTROYED_WEAPON,
    DEMO_PLUS_FIRE_PRIMARY_DAMAGE, DEMO_PLUS_FIRE_PRIMARY_RADIUS, DEMO_PLUS_FIRE_SECONDARY_DAMAGE,
    DEMO_PLUS_FIRE_SECONDARY_RADIUS, DEMO_SUICIDE_DYNAMITE_PLUS_FIRE, HostDemoSuicideBombRegistry,
    UPGRADE_DEMO_SUICIDE_BOMB, can_issue_demo_tertiary_suicide,
    command_set_enables_tertiary_suicide, demo_command_set_upgrade_for_template,
    demo_destroyed_damage_at, demo_plus_fire_damage_at, has_demo_suicide_bomb_upgrade,
    is_demo_suicide_bomb_eligible_template, is_demo_suicide_bomb_upgrade, plan_demo_plus_fire_hits,
};
pub use super::host_dock_contain_exit_heal_residual::{
    honesty_contain_residual_deepen_pack_wave98,
    honesty_dock_contain_exit_heal_residual_pack_wave98, honesty_dock_residual_pack_wave98,
    honesty_exit_residual_pack_wave98, honesty_heal_residual_deepen_pack_wave98,
};
pub use super::host_dragon_tank::{
    DRAGON_PRIMARY_DAMAGE, DRAGON_RANGE, DRAGON_SECONDARY_DAMAGE, DRAGON_SECONDARY_RADIUS,
    DRAGON_TANK_FLAME_WEAPON, DRAGON_TANK_FLAME_WEAPON_UPGRADED, UPGRADE_CHINA_BLACK_NAPALM,
    dragon_flame_damage_at, dragon_flame_stats, dragon_flame_weapon, dragon_flame_weapon_name,
    has_black_napalm_upgrade, is_dragon_tank_template, should_apply_dragon_flame_residual,
};
pub use super::host_ecm_jam::{HOST_ECM_JAM_RADIUS, is_ecm_jammer, is_legal_ecm_jam_target};
pub use super::host_emergency_repair::{
    EMERGENCY_REPAIR_ACTIVATE_AUDIO, HOST_EMERGENCY_REPAIR_RADIUS, HostEmergencyRepair,
    HostEmergencyRepairLevel, HostEmergencyRepairRegistry, emergency_repair_is_ally,
    is_legal_emergency_repair_target,
};
pub use super::host_emp_pulse::{
    EMP_PULSE_ACTIVATE_AUDIO, EMP_PULSE_DISABLED_DURATION_FRAMES, HOST_EMP_PULSE_RADIUS,
    HostEmpPulse, HostEmpPulseRegistry, is_legal_emp_disable_target,
};
pub use super::host_enum_table_residual::{
    honesty_damage_type_enum_table_wave82, honesty_death_type_enum_table_wave82,
    honesty_enum_table_residual_pack_wave82, honesty_enum_table_residual_pack_wave84,
    honesty_geometry_type_enum_table_wave84, honesty_kindof_enum_table_wave84,
    honesty_model_condition_enum_table_wave82, honesty_object_status_enum_table_wave82,
    honesty_relationship_enum_table_wave84, honesty_shadow_type_enum_table_wave84,
    honesty_veterancy_level_enum_table_wave84, honesty_weapon_bonus_enum_table_wave82,
    honesty_weapon_slot_enum_table_wave84,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_env_contain_residual::{
    honesty_bridge_residual_pack_wave87, honesty_env_contain_residual_pack_wave87,
    honesty_garrison_residual_pack_wave87, honesty_transport_residual_pack_wave87,
    honesty_tunnel_residual_deepen_wave87, honesty_water_residual_pack_wave87,
    honesty_weather_residual_pack_wave87,
};
pub use super::host_faction_skirmish_residual::{
    honesty_faction_side_residual_table_wave85, honesty_faction_skirmish_residual_pack_wave85,
    honesty_player_template_residual_pack_wave85,
    honesty_skirmish_ai_personality_residual_pack_wave85,
    honesty_starting_cash_residual_pack_wave85, honesty_victory_condition_residual_pack_wave85,
};
pub use super::host_firewall::{
    FIREWALL_ACTIVATE_AUDIO, FIREWALL_DAMAGE_PER_TICK, FIREWALL_DURATION_FRAMES,
    FIREWALL_SEGMENT_RADIUS, FIREWALL_TICK_INTERVAL_FRAMES, HostFireWall, HostFireWallRegistry,
    HostFireWallSegment,
};
pub use super::host_frenzy::{
    FRENZY_ACTIVATE_AUDIO, HOST_FRENZY_RADIUS, HostFrenzy, HostFrenzyLevel, HostFrenzyRegistry,
    is_legal_frenzy_target,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_fx_audio_cursor_residual::{
    honesty_fx_audio_cursor_residual_pack_wave88, honesty_mouse_cursor_name_table_wave88,
    honesty_radius_cursor_name_table_wave88, honesty_superweapon_audio_event_name_table_wave88,
    honesty_superweapon_fxlist_name_table_wave88, honesty_superweapon_ocl_name_table_wave88,
    honesty_superweapon_particle_name_table_wave88,
};
pub use super::host_fx_ocl_particle_audio_residual_wave107::{
    honesty_audio_residual_deepen_pack_wave107,
    honesty_fx_ocl_particle_audio_residual_pack_wave107,
    honesty_fxlist_entry_residual_deepen_pack_wave107,
    honesty_ocl_create_residual_deepen_pack_wave107,
    honesty_particle_system_residual_deepen_pack_wave107,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_game_logic_residual_wave103::{
    honesty_game_logic_residual_pack_wave103, honesty_object_kindof_residual_pack_wave103,
    honesty_special_power_superweapon_residual_deepen_wave103,
};
pub use super::host_gamedata_lobby_residual::{
    honesty_crate_residual_deepen_pack_wave86, honesty_gamedata_camera_fps_residual_pack_wave86,
    honesty_gamedata_lobby_residual_pack_wave86,
    honesty_gamedata_world_constants_residual_pack_wave86,
    honesty_map_selection_residual_pack_wave86, honesty_multiplayer_options_residual_pack_wave86,
};
pub use super::host_gattling_tank::{
    GATTLING_BASE_DELAY_FRAMES, GATTLING_GROUND_DAMAGE, GATTLING_GROUND_RANGE, GATTLING_TANK_GUN,
    GATTLING_TANK_GUN_AIR, GattlingFireLevel, UPGRADE_CHINA_CHAIN_GUNS, gattling_air_weapon,
    gattling_delay_frames_for_level, gattling_ground_weapon, gattling_on_shot_fired,
    has_chain_guns_upgrade, is_gattling_tank_template, preferred_gattling_slot,
};
pub use super::host_gla_rebel::{
    REBEL_DAMAGE, REBEL_MACHINE_GUN, REBEL_RANGE,
    UPGRADE_GLA_AP_BULLETS as REBEL_UPGRADE_AP_BULLETS, is_gla_rebel_template, rebel_weapon,
    should_apply_rebel_residual,
};
pub use super::host_gla_worker::{
    HostGlaWorkerRegistry, UPGRADE_GLA_WORKER_SHOES, WORKER_SHOES_SPEED, WORKER_SHOES_SUPPLY_BOOST,
    is_gla_worker_template, residual_worker_shoes_drop_off_boost, worker_residual_speed,
};
pub use super::host_gps_scrambler::{
    GPS_SCRAMBLER_ACTIVATE_AUDIO, HOST_GPS_SCRAMBLER_RADIUS, HostGpsScrambler,
    HostGpsScramblerRegistry, host_has_gps_stealth_module, is_immune_to_default_gps_stealth,
    is_legal_gps_scrambler_target,
};
pub use super::host_hacker_income::{
    HACKER_CASH_INTERVAL_FAST_FRAMES, HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_PING_AUDIO,
    HACKER_CASH_REGULAR, HACKER_XP_PER_CASH_UPDATE, HostHackerIncomeRegistry,
    cash_amount_for_level, cash_interval_frames, is_hacker_template, is_internet_center_template,
    is_legal_hacker_income_source,
};
pub use super::host_heal::{
    AMBULANCE_HEAL_DELAY_FRAMES, AMBULANCE_INFANTRY_HEAL_AMOUNT,
    AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC, AMBULANCE_TRANSPORT_SLOTS,
    AMBULANCE_VEHICLE_HEAL_AMOUNT, AMBULANCE_VEHICLE_SKIP_SELF_FOR_HEALING,
    DEFAULT_AUTO_HEAL_AMOUNT, DEFAULT_AUTO_HEAL_DELAY_FRAMES, DEFAULT_AUTO_HEAL_START_DELAY_FRAMES,
    HOST_AMBULANCE_HEAL_RADIUS, HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC,
    HOST_AMBULANCE_VEHICLE_HEAL_HP_PER_SEC, HostAmbulanceHealExclusivity, HostDefaultAutoHealData,
    ambulance_embarked_heal_hp_per_sec, ambulance_heal_is_ally, ambulance_pulse_ready,
    honesty_ambulance_auto_heal_constants_ok, is_ambulance_healer,
    is_legal_ambulance_infantry_heal_target, is_legal_ambulance_vehicle_heal_target,
};
pub use super::host_satellite_hack::{
    SATELLITE_HACK_TWO_DURATION_FRAMES, SATELLITE_HACK_TWO_INTERVAL_FRAMES, SatelliteHackSpySpec,
    is_satellite_hack_upgrade, object_authors_spy_vision_update, satellite_hack_spy_spec,
};

pub use super::host_helix_minigun::{
    HELIX_MINIGUN_DAMAGE, HELIX_MINIGUN_DELAY_FRAMES, HELIX_MINIGUN_RANGE, HELIX_MINIGUN_WEAPON,
    helix_minigun_weapon, is_legal_helix_minigun_target, should_apply_helix_minigun_residual,
};
pub use super::host_helix_napalm::{
    HELIX_FIRESTORM_DAMAGE_PER_TICK, HELIX_FIRESTORM_DURATION_FRAMES, HELIX_FIRESTORM_RADIUS,
    HELIX_FIRESTORM_TICK_INTERVAL_FRAMES, HELIX_NAPALM_PRIMARY_DAMAGE, HELIX_NAPALM_PRIMARY_RADIUS,
    HELIX_NAPALM_SECONDARY_DAMAGE, HELIX_NAPALM_SECONDARY_RADIUS, HostHelixNapalmRegistry,
    UPGRADE_HELIX_NAPALM_BOMB, helix_napalm_blast_damage_at, helix_napalm_unlocked,
    is_helix_napalm_caster,
};
pub use super::host_hero_abilities::{
    DISABLE_VEHICLE_HACK_DURATION_FRAMES, HostHeroAbilityRegistry, SNIPE_VEHICLE_AUDIO,
    STEAL_CASH_DEFAULT_AMOUNT,
};

pub use super::host_humvee::{
    HUMVEE_MISSILE_WEAPON_AIR, HUMVEE_TRANSPORT_SLOTS, is_humvee_template,
};
pub use super::host_inferno_cannon::{
    HostInfernoFireZone, HostInfernoFireZoneRegistry, INFERNO_CANNON_FIRE_AUDIO,
    INFERNO_CANNON_PRIMARY_WEAPON, INFERNO_CANNON_SHELL_DAMAGE, INFERNO_CANNON_UPGRADED_WEAPON,
    INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
    INFERNO_FIRE_DURATION_FRAMES, INFERNO_FIRE_RADIUS, INFERNO_FIRE_TICK_INTERVAL_FRAMES,
    has_black_napalm_upgrade as has_inferno_black_napalm_upgrade, is_inferno_cannon_template,
};
pub use super::host_leaflet_drop::{
    HOST_LEAFLET_RADIUS, HostLeafletDropKind, HostLeafletDropMission, HostLeafletDropPhase,
    HostLeafletDropRegistry, LEAFLET_DELAY_FRAMES, LEAFLET_DISABLED_DURATION_FRAMES,
    is_legal_leaflet_disable_target,
};
pub use super::host_listening_outpost::{
    HostListeningOutpostRegistry, LISTENING_OUTPOST_DETECTION_RANGE,
    LISTENING_OUTPOST_TRANSPORT_SLOTS, is_listening_outpost_template,
    listening_outpost_detection_range, listening_outpost_spawn_is_detector,
};
pub use super::host_marauder::{
    MARAUDER_DAMAGE, MARAUDER_RANGE, MARAUDER_TANK_GUN, MARAUDER_TANK_GUN_UPGRADE_ONE,
    MARAUDER_TANK_GUN_UPGRADE_TWO, MarauderWeaponTier, is_marauder_template,
    marauder_weapon_for_tier, marauder_weapon_name_for_tier, marauder_weapon_stats,
    should_apply_marauder_residual,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_message_stream_meta_ingameui_residual_wave110::{
    honesty_game_message_argument_type_residual_wave110, honesty_ingame_ui_residual_wave110,
    honesty_message_stream_marker_residual_wave110,
    honesty_message_stream_meta_ingameui_residual_pack_wave110,
    honesty_meta_event_category_residual_wave110,
};
pub use super::host_mines::{
    DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE, DemoTrapProfile, HostMineData,
    HostMineDetonateReason, HostMineDetonationPlan, HostMineKind, MINE_CLEARED_AUDIO,
    can_clear_mine_kind, demo_trap_damage_at, demo_trap_profile, is_mine_clearer,
};
pub use super::host_missile_defender::{
    MISSILE_DEFENDER_DAMAGE, MISSILE_DEFENDER_LASER_GUIDED_WEAPON, MISSILE_DEFENDER_MISSILE_WEAPON,
    MISSILE_DEFENDER_PRIMARY_RANGE, is_missile_defender_template,
    missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
    should_apply_missile_defender_residual,
};
pub use super::host_money_crate::{
    HostMoneyCrateEntry, HostMoneyCratePickup, HostMoneyCrateRegistry,
    MONEY_CRATE_BUILDING_PICKUP_RADIUS, MONEY_CRATE_PICKUP_AUDIO, MONEY_CRATE_UNIT_PICKUP_RADIUS,
    SUPPLY_DROP_CRATE_MONEY_PROVIDED, SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST,
};
pub use super::host_neutron_shell::{
    HOST_NEUTRON_BLAST_RADIUS, NUKE_CANNON_NEUTRON_WEAPON, NUKE_CANNON_PRIMARY_WEAPON,
    NeutronEffect, UPGRADE_CHINA_NEUTRON_SHELLS, is_nuke_cannon_template,
    neutron_effect_for_target, should_apply_neutron_blast,
};
pub use super::host_nuke_cannon::{
    HostNukeCannonRegistry, MEDIUM_RADIATION_DAMAGE_PER_TICK, MEDIUM_RADIATION_RADIUS,
    NUKE_CANNON_PRIMARY_DAMAGE, NUKE_CANNON_PRIMARY_RADIUS,
    is_nuke_cannon_template as is_nuke_cannon_primary_template, nuke_cannon_primary_damage_at,
    should_apply_nuke_cannon_primary,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_object_register_drawable_residual_wave104::{
    honesty_active_body_max_health_apply_residual_wave104,
    honesty_drawable_create_residual_wave104, honesty_gamelogic_register_object_residual_wave104,
    honesty_object_create_order_residual_wave104,
    honesty_object_register_drawable_crosslink_wave104,
    honesty_object_register_drawable_residual_pack_wave104,
    honesty_object_status_state_machine_residual_wave104,
};
pub use super::host_oil_derrick::{
    HostOilDerrickRegistry, OIL_DERRICK_CAPTURE_BONUS_AUDIO, OIL_DERRICK_DEFAULT_STRUCTURE_RADIUS,
    OIL_DERRICK_DEPOSIT_AMOUNT, OIL_DERRICK_DEPOSIT_AUDIO, OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES,
    OIL_DERRICK_DEPOSIT_TIMING_MS, OIL_DERRICK_FLOATING_TEXT_SCATTER_SCALE,
    OIL_DERRICK_INITIAL_CAPTURE_BONUS, is_legal_oil_derrick_income_source,
    is_oil_derrick_structure, is_oil_derrick_template, oil_derrick_deposit_amount,
    structure_floating_text_scatter,
};
pub use super::host_overlord_addons::{
    HELIX_TRANSPORT_SLOTS, HostOverlordAddonRegistry, OVERLORD_GATTLING_AIR_DAMAGE,
    OVERLORD_GATTLING_GROUND_DAMAGE, OVERLORD_PROPAGANDA_RADIUS, UPGRADE_HELIX_GATTLING,
    UPGRADE_HELIX_PROPAGANDA, UPGRADE_OVERLORD_GATTLING, UPGRADE_OVERLORD_PROPAGANDA,
    is_emperor_template, is_helix_template, is_overlord_family_host, is_overlord_tank_template,
    overlord_gattling_air_weapon, should_apply_overlord_gattling_residual,
};
pub use super::host_paradrop::{
    AMERICA_PARADROP_UNIT_COUNT, HostParadropKind, HostParadropMission, HostParadropPhase,
    HostParadropRegistry, PARADROP_DROP_SPACING, PARADROP_RESIDUAL_TEMPLATE,
};
pub use super::host_partition_collision_physics_residual::{
    honesty_collision_residual_pack_wave96,
    honesty_partition_collision_physics_residual_pack_wave96,
    honesty_partition_residual_pack_wave96, honesty_physics_residual_pack_wave96,
    honesty_projectile_residual_deepen_pack_wave96,
};
pub use super::host_pathfinder::{
    PATHFINDER_DETECTION_RANGE, PATHFINDER_SNIPER_WEAPON, is_pathfinder_template,
    pathfinder_detection_range, pathfinder_spawn_is_detector,
};
pub use super::host_point_defense::{
    AVENGER_PDL_FIRE_RANGE, PALADIN_PDL_FIRE_RANGE, PDL_INTERCEPT_AUDIO, is_missile_name_residual,
    is_point_defense_carrier, is_primary_intercept_target, pdl_damage, pdl_delay_frames,
    pdl_fire_range, pdl_scan_range,
};
pub use super::host_production_buildable_command_residual::{
    honesty_buildable_residual_pack_wave99, honesty_command_button_residual_deepen_pack_wave99,
    honesty_control_bar_residual_deepen_pack_wave99, honesty_prerequisite_residual_pack_wave99,
    honesty_production_buildable_command_residual_pack_wave99,
    honesty_production_residual_deepen_pack_wave99,
};
pub use super::host_propaganda::{
    HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC, HOST_PROPAGANDA_TOWER_RADIUS,
    HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC, UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
    is_legal_propaganda_target, is_propaganda_tower, propaganda_heal_amount,
};
pub use super::host_radar::{
    HostRadarRegistry, RADAR_OFFLINE_AUDIO, RADAR_ONLINE_AUDIO, is_legal_radar_provider,
    is_radar_command_center_template, is_radar_provider_template, is_radar_van_template,
};
pub use super::host_radar_scan::{
    HostRadarScan, HostRadarScanRegistry, RADAR_SCAN_ACTIVATE_AUDIO, RADAR_SCAN_DURATION_FRAMES,
    RADAR_SCAN_RADIUS, RADAR_SCAN_SHRINK_DELAY_FRAMES, RADAR_SCAN_SHRINK_TIME_FRAMES,
    RADAR_SCAN_STEALTH_DETECTION_RANGE, RADAR_SCAN_VISION_RANGE, RADAR_VAN_PING_TEMPLATE,
    honesty_radar_scan_dynamic_shroud_constants_ok, radar_scan_dynamic_shroud_radius_at_elapsed,
};
pub use super::host_radar_stealth_vision_residual::{
    honesty_detector_residual_deepen_pack_wave97, honesty_radar_residual_deepen_pack_wave97,
    honesty_radar_stealth_vision_residual_pack_wave97, honesty_spotter_residual_pack_wave97,
    honesty_stealth_residual_deepen_pack_wave97, honesty_vision_residual_pack_wave97,
};
pub use super::host_railed_transport::{
    RAILED_ARRIVE_DISTANCE, RAILED_INVALID_PATH, RAILED_MAX_WAYPOINT_PATHS,
    default_railed_current_path, inject_railed_waypoint, railed_waypoint_overlay_reset,
};
pub use super::host_railroad::{
    HostConductorState, HostRailroadCar, HostTrainTrack, RAILROAD_SPEED_MAX,
    RAILROAD_WAIT_AT_STATION_FRAMES, honesty_railroad_residual_ok, inject_railroad_track,
    is_railroad_carriage_template, is_railroad_locomotive_template, is_railroad_template,
    railroad_car, railroad_registry_reset, restore_railroad_car,
};

pub use super::host_bridge_behavior::{
    BRIDGE_SCAFFOLD_TEMPLATE, BRIDGE_SPLAT_DAMAGE, HostBridgeBehaviorRegistry,
    is_bridge_or_tower_template, is_bridge_span_template,
};
pub use super::host_cave_system::{HostCaveSystem, MAX_CAVE_CAPACITY, is_cave_template};
pub use super::host_rank_ui_residual::{
    honesty_chat_residual_host_pack_wave89, honesty_experience_residual_tables_pack_wave89,
    honesty_hotkey_residual_table_pack_wave89, honesty_options_residual_pack_wave89,
    honesty_rank_skill_points_application_residual_pack_wave89,
    honesty_rank_ui_residual_pack_wave89, honesty_replay_residual_host_pack_wave89,
};
pub use super::host_red_guard::{
    REDGUARD_DAMAGE, REDGUARD_MACHINE_GUN, REDGUARD_RANGE, is_red_guard_template, red_guard_weapon,
    should_apply_bayonet_residual, should_apply_red_guard_residual,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_render_terrain_residual::{
    honesty_drawable_opacity_shroud_residual_deepen_pack_wave93,
    honesty_particle_system_emit_rate_residual_deepen_pack_wave93,
    honesty_render_terrain_residual_pack_wave93, honesty_road_residual_pack_wave93,
    honesty_shadow_residual_deepen_pack_wave93, honesty_terrain_texture_residual_pack_wave93,
};
pub use super::host_residual_acquire::{
    PilotVehicleCandidate, ResidualAcquireCandidate, pick_nearest_pilot_vehicle_target,
    pick_nearest_residual_service_target, pick_nearest_residual_target, residual_combat_kind,
};
pub use super::host_rng_residual::{
    HostRandomState, HostRngResidualHonesty, client_stream_structure_scatter,
    exercise_host_rng_residual, logic_stream_error_radius_offset, pure_client_structure_scatter,
    pure_logic_random_int, pure_logic_random_real,
};
pub use super::host_rpg_trooper::{
    RPG_TROOPER_DAMAGE, RPG_TROOPER_RANGE, TUNNEL_DEFENDER_ROCKET_WEAPON,
    UPGRADE_GLA_AP_ROCKETS as RPG_UPGRADE_AP_ROCKETS, is_rpg_trooper_template, rpg_trooper_weapon,
    should_apply_rpg_trooper_residual,
};
pub use super::host_saboteur::{
    HostSaboteurRegistry, SABOTEUR_STEAL_CASH_AMOUNT, SABOTEUR_SUCCESS_AUDIO, SaboteurEffectKind,
    classify_sabotage_target, is_saboteur_template,
};
pub use super::host_science_rank::honesty_science_rank_residual_pack_wave80;
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_script_map_team_player_residual::{
    honesty_map_object_residual_pack_wave95, honesty_player_residual_deepen_pack_wave95,
    honesty_script_action_name_table_residual_wave95,
    honesty_script_condition_name_table_residual_wave95,
    honesty_script_map_team_player_residual_pack_wave95, honesty_team_residual_pack_wave95,
    honesty_waypoint_residual_pack_wave95,
};
pub use super::host_sentry_drone::{
    SENTRY_DETECTION_RANGE, SENTRY_DRONE_GUN_WEAPON, UPGRADE_AMERICA_SENTRY_DRONE_GUN,
    is_sentry_drone_template, sentry_detection_range, sentry_spawn_is_detector,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_shell_campaign_save_residual_wave106::{
    honesty_campaign_mission_residual_deepen_pack_wave106,
    honesty_game_state_residual_deepen_pack_wave106,
    honesty_game_window_residual_deepen_pack_wave106,
    honesty_main_menu_residual_deepen_pack_wave106,
    honesty_shell_campaign_save_residual_pack_wave106,
    honesty_window_layout_residual_deepen_pack_wave106,
};
pub use super::host_slave_drones::{
    HELLFIRE_MISSILE_WEAPON, SCOUT_DETECTION_RANGE, SlaveDroneKind, UPGRADE_AMERICA_HELLFIRE_DRONE,
    UPGRADE_AMERICA_SCOUT_DRONE, is_hellfire_drone_template, is_scout_drone_template,
    is_slave_drone_master_template, scout_detection_range, scout_spawn_is_detector,
};
pub use super::host_sneak_attack::{
    GLA_SNEAK_TUNNEL_TEMPLATE, HOST_SNEAK_ATTACK_RADIUS, HostSneakAttackKind,
    HostSneakAttackMission, HostSneakAttackPhase, HostSneakAttackRegistry,
    SNEAK_ATTACK_RESIDUAL_TEMPLATE, SNEAK_ATTACK_SHOCKWAVE_DAMAGE, SNEAK_ATTACK_SHOCKWAVE_RADIUS,
    SNEAK_ATTACK_SPAWN_DELAY_FRAMES, is_legal_sneak_shockwave_target,
};
pub use super::host_sp_science_upgrade_player_team_residual_wave109::{
    honesty_player_residual_deepen_pack_wave109,
    honesty_science_store_residual_deepen_pack_wave109,
    honesty_sp_science_upgrade_player_team_residual_pack_wave109,
    honesty_special_power_template_store_residual_wave109,
    honesty_team_residual_deepen_pack_wave109, honesty_upgrade_store_residual_deepen_pack_wave109,
};
pub use super::host_special_power_enum_residual::honesty_special_power_enum_residual_pack_wave80;
pub use super::host_spy_satellite::{
    HostSpySatellite, HostSpySatelliteRegistry, SPY_SATELLITE_ACTIVATE_AUDIO,
    SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_GROW_TIME_FRAMES, SPY_SATELLITE_PING_TEMPLATE,
    SPY_SATELLITE_RADIUS, SPY_SATELLITE_SHRINK_DELAY_FRAMES, SPY_SATELLITE_SHRINK_TIME_FRAMES,
    SPY_SATELLITE_STEALTH_DETECTION_RANGE, SPY_SATELLITE_VISION_RANGE,
    honesty_spy_satellite_dynamic_shroud_constants_ok,
    spy_satellite_dynamic_shroud_radius_at_elapsed,
};
pub use super::host_stealth_fighter::{
    AMERICA_JET_STEALTH_FIGHTER, HostStealthFighterRegistry, SCIENCE_STEALTH_FIGHTER,
    STEALTH_FIGHTER_DAMAGE, STEALTH_FIGHTER_MIN_RANGE, STEALTH_FIGHTER_PRIMARY_RADIUS,
    STEALTH_FIGHTER_RANGE, STEALTH_JET_MISSILE_WEAPON as STEALTH_FIGHTER_MISSILE_WEAPON,
    USA_STEALTH_FIGHTER, is_stealth_fighter_science, is_stealth_fighter_template,
    player_may_produce_stealth_aircraft, requires_stealth_fighter_science,
    should_apply_stealth_fighter_residual, stealth_fighter_damage_at, stealth_fighter_weapon,
};
pub use super::host_strategy_center::{
    BATTLE_PLAN_PARALYZE_FRAMES, BATTLE_PLAN_PARALYZE_TIME_MS, BOMBARDMENT_DAMAGE_MULT,
    HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR, HostBattlePlan, HostBattlePlanRegistry,
    HostBattlePlanSelection, SEARCH_AND_DESTROY_RANGE_MULT, SEARCH_AND_DESTROY_SIGHT_RANGE_SCALAR,
    battle_plan_paralyze_frames_from_ms, battle_plan_paralyze_until_frame,
    is_legal_battle_plan_member, is_strategy_center_template,
};
pub use super::host_structure_economy_residual::{
    MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD, honesty_capture_building_residual_pack_wave83,
    honesty_command_center_residual_pack_wave83, honesty_dozer_build_residual_pack_wave83,
    honesty_power_plant_residual_pack_wave83, honesty_production_queue_residual_pack_wave83,
    honesty_structure_economy_residual_pack_wave83, honesty_supply_warehouse_residual_pack_wave83,
};
pub use super::host_superweapon_kindof::honesty_superweapon_kindof_residual_pack_wave80;
pub use super::host_supply_drop_zone::{
    HostSupplyDropZoneRegistry, SUPPLY_DROP_ZONE_CRATE_COUNT, SUPPLY_DROP_ZONE_DELAY_MS,
    SUPPLY_DROP_ZONE_DROP_AUDIO, SUPPLY_DROP_ZONE_DROP_CASH,
    SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES, SUPPLY_DROP_ZONE_INTERVAL_FRAMES,
    SUPPLY_DROP_ZONE_MONEY_PER_CRATE, SUPPLY_DROP_ZONE_SUPPLY_LINES_BOOST_PER_CRATE,
    drop_cash_amount, drop_interval_frames_from_ms, is_legal_supply_drop_zone_income_source,
    is_supply_drop_zone_structure, is_supply_drop_zone_template,
};
pub use super::host_tank_hunter::{
    TANK_HUNTER_DAMAGE, TANK_HUNTER_MISSILE_WEAPON, TANK_HUNTER_RANGE, is_tank_hunter_template,
    should_apply_tank_hunter_residual, tank_hunter_weapon,
};
pub use super::host_terrain_bridge_water_road_residual_wave108::{
    honesty_bridge_residual_deepen_pack_wave108, honesty_cliff_residual_peels_pack_wave108,
    honesty_heightmap_residual_deepen_pack_wave108, honesty_road_residual_deepen_pack_wave108,
    honesty_terrain_bridge_water_road_residual_pack_wave108,
    honesty_water_residual_deepen_pack_wave108,
};
pub use super::host_terrorist::{
    SUICIDE_DYNAMITE_PACK, SUICIDE_DYNAMITE_PRIMARY_DAMAGE, SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO,
    SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA, SUICIDE_DYNAMITE_PRIMARY_RADIUS,
    SUICIDE_DYNAMITE_SECONDARY_DAMAGE, SUICIDE_DYNAMITE_SECONDARY_RADIUS, TERRORIST_SUICIDE_WEAPON,
    TerroristDeathProfile, is_demo_general_template as is_terrorist_demo_general_template,
    is_terrorist_template, should_apply_terrorist_residual, suicide_dynamite_damage_at,
    suicide_dynamite_damage_at_profile, terrorist_death_profile, terrorist_suicide_weapon,
    terrorist_suicide_weapon_for_profile,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_thing_factory_module_xfer_residual::{
    honesty_module_factory_residual_deepen_pack_wave101,
    honesty_module_type_table_residual_pack_wave100,
    honesty_partition_register_residual_pack_wave101,
    honesty_thing_factory_create_residual_deepen_pack_wave101,
    honesty_thing_factory_module_partition_crosslink_wave101,
    honesty_thing_factory_module_partition_residual_pack_wave101,
    honesty_thing_factory_module_xfer_residual_pack_wave100,
    honesty_thing_factory_residual_deepen_pack_wave100,
    honesty_thing_factory_spawn_crosslink_wave100, honesty_xfer_residual_deepen_pack_wave100,
};
#[cfg(any(test, feature = "host-residuals"))]
pub use super::host_timing_shell_residual::{
    honesty_credits_residual_pack_wave90, honesty_debug_residual_tables_pack_wave90,
    honesty_frame_rate_residual_deepen_pack_wave90, honesty_gamespeed_residual_pack_wave90,
    honesty_language_residual_deepen_pack_wave90, honesty_timing_shell_residual_pack_wave90,
};
pub use super::host_troop_crawler::{
    HostTroopCrawlerRegistry, TROOP_CRAWLER_ASSAULT_RANGE, TROOP_CRAWLER_ASSAULT_WEAPON,
    TROOP_CRAWLER_DETECTION_RANGE, TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT,
    TROOP_CRAWLER_TRANSPORT_SLOTS, TROOP_CRAWLER_VISION_RANGE, is_troop_crawler_template,
    resolve_payload_template_name, should_apply_troop_crawler_assault_deploy,
    troop_crawler_assault_weapon, troop_crawler_detection_range, troop_crawler_spawn_is_detector,
};
pub use super::host_tunnel_network::{
    HostTunnelNetworkRegistry, MAX_TUNNEL_CAPACITY, TUNNEL_FULL_HEAL_FRAMES, TUNNEL_NETWORK_GUN,
    TUNNEL_NETWORK_GUN_DAMAGE, TUNNEL_NETWORK_GUN_DUMMY, TUNNEL_NETWORK_GUN_DUMMY_DAMAGE,
    TUNNEL_NETWORK_GUN_RANGE, is_sneak_attack_tunnel_template, is_tunnel_network_template,
    tunnel_network_gun_dummy_weapon, tunnel_network_gun_weapon, tunnel_network_primary_weapon,
    tunnel_system_key, unit_can_use_tunnel,
};

pub use super::host_ui_presentation_residual::{
    honesty_eva_residual_pack_wave91, honesty_help_box_residual_pack_wave91,
    honesty_message_residual_pack_wave91, honesty_mission_briefing_residual_pack_wave91,
    honesty_tooltip_residual_pack_wave91, honesty_ui_presentation_residual_pack_wave91,
    honesty_video_residual_name_table_wave91,
};
pub use super::host_unit_training::{
    HostUnitTrainingRegistry, SCIENCE_ARTILLERY_TRAINING, SCIENCE_BATTLEMASTER_TRAINING,
    SCIENCE_INFA_RED_GUARD_TRAINING, SCIENCE_RED_GUARD_TRAINING, SCIENCE_TECHNICAL_TRAINING,
    UnitTrainingScience, is_unit_training_science, unit_training_level_for_template,
    unit_training_science_from_name,
};
pub use super::host_upgrades::{
    HostUpgradeKind, HostUpgradePhase, HostUpgradeRegistry, HostUpgradeResearch,
};
pub use super::host_usa_pilot::{
    EJECT_PILOT_TEMPLATE, HostDeathType, HostUsaPilotRegistry, PILOT_EJECT_AUDIO,
    PILOT_RECREW_AUDIO, is_pilot_template, is_recrewable_unmanned_vehicle,
    pilot_collide_would_like_to_collide_with, should_recrew_on_enter,
};
pub use super::host_usa_tanks::{
    CRUSADER_TANK_GUN, LAZR_CRUSADER_TANK_GUN, LAZR_CRUSADER_TANK_GUN_DAMAGE,
    LAZR_PALADIN_TANK_GUN, LAZR_PALADIN_TANK_GUN_DAMAGE, PALADIN_TANK_GUN,
    UPGRADE_AMERICA_COMPOSITE_ARMOR, is_composite_armor_unit_template, is_crusader_template,
    is_laser_general_tank_template, is_paladin_template, usa_tank_gun_weapon_for_template,
};
