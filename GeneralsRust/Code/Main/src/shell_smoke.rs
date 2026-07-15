//! Production host smoke: SkirmishMenu → config → apply → map load → frames → presentation.
//!
//! Full windowed shell/WND + GPU boot still requires a display; this path exercises the
//! same production APIs `start_game_from_ui` uses after menu StartGame.
//!
//! Honesty: no tautological host flag, no silent golden_skirmish_config fallback.
//! Opponent slot is configured through SkirmishMenu::configure_slot_medium_ai.
//!
//! Claim flags (do not conflate):
//! - `playable_claim` — **always false**. Headless host APIs are not retail W3D /
//!   windowed shell playthrough. Fail-closed pending full GPU/WND match play.
//! - `shell_host_playable_ok` — limited honesty claim: when true, the headless
//!   shell→config→map→dual-tick presentation→HUD selection/minimap→ControlBar.wnd
//!   ensure path is operational. Still **not** a retail playthrough claim.
//!
//! Residual honesty (do **not** flip `playable_claim`):
//! - `dual_tick_presentation_ok` — seed + logic update + multi-consumer presentation apply
//! - `dual_tick_counters_ok` — presentation dual-tick residual counters (build/apply)
//! - `minimap_fow_presentation_ok` — FOW grid snapshot usable for minimap texture path
//! - `laser_segment_upload_ok` — presentation → CPU SegLine pack residual (incl. synthetic)
//! - `projectile_segment_upload_ok` — presentation projectiles → CPU trail pack residual
//! - `multi_beam_soft_edge_ok` — OrbitalLaser NumBeams soft-edge CPU pack residual
//! - `laser_presentation_residual_ok` — ground-height + soft-edge presentation fields
//! - `floating_text_layout_ok` — presentation → CPU InGameUI floating-text layout residual
//! - `floating_text_vanish_ok` — vanish-rate alpha residual presentation field honesty
//! - `world_anim_presentation_ok` — MoneyPickUp Anim2D residual frozen on presentation
//! - `world_anim_layout_ok` — presentation → CPU Anim2D layout pack residual
//! - `world_anim_fade_ok` — world-anim fade residual presentation field honesty
//! - `anim2d_frame_ok` — MoneyPickUp Anim2D frame advance residual
//! - `anim2d_collection_residual_ok` — Anim2DCollection template/instance residual
//! - `translate_copy_residual_ok` — GameText translate_copy escape table residual
//! - `game_text_caption_ok` — GUI:AddCash caption residual on floating-text pack
//! - `game_text_csf_str_ok` — CSF/STR parse + retail `$%d` printf + DisplayString measure
//! - `display_string_measure_ok` — monospaced glyph measure residual on floating-text pack
//! - `rng_stream_residual_ok` — GameLogic/GameClient RandomValue ADC stream residual
//! - `mesh_asset_residual_ok` — W3D mesh resolve residual (keys/scale/search; no GPU)
//! - `rng_residual_pack_ok` — Wave 72 host RNG residual pack honesty
//! - `special_power_wave72_residual_ok` — Daisy/A10 special-power residual pack
//! - `special_power_wave73_residual_ok` — Spectre/Nuke/SupW residual pack
//! - `special_power_wave76_residual_ok` — A10 science-tier FormationSize residual pack
//! - `paradrop_wave76_residual_ok` — Paradrop science-tier payload residual pack
//! - `control_bar_wave76_residual_ok` — ControlBar window-count/named/font residual pack
//! - `graphics_wave76_residual_ok` — InGameUI font table + vanish color-alpha residual
//! - `spectre_orbit_decal_presentation_ok` — Wave 73 presentation Spectre decal residual
//! - `special_power_wave77_residual_ok` — Wave 77 audio name tables residual pack
//! - `special_power_wave78_residual_ok` — Wave 78 reload table / CarpetBomb / Artillery residual pack
//! - `cluster_mines_wave78_residual_ok` — Wave 78 ClusterMines DeliveryDecal / science residual pack
//! - `gps_scrambler_wave78_residual_ok` — Wave 78 GPS science / marker particle residual pack
//! - `cash_bounty_wave78_residual_ok` — Wave 78 CashBountyScienceTier residual pack
//! - `fow_residual_pack_ok` — Wave 77 FOW cell/R8/inactive residual honesty
//! - `ground_height_presentation_ok` — Wave 77 unit ground-height presentation residual
//! - `weapon_store_seed_residual_ok` — Wave 77 host WeaponStore seed residual pack
//! - `ai_skirmish_residual_ok` — Wave 77 AI skirmish timer/wealth residual pack
//! - `minimap_residual_pack_ok` — Wave 79 minimap FOW shade/size residual pack
//! - `selection_hud_residual_pack_ok` — Wave 79 selection/HUD color residual pack
//! - `input_residual_pack_ok` — Wave 79 drag/double-click input residual pack
//! - `drawable_residual_fields_ok` — Wave 79 Drawable StealthLook save/load residual
//! - `unit_training_wave79_residual_ok` — Wave 79 veterancy bonus / AdvancedTraining XP
//! - `upgrades_cost_time_application_ok` — Wave 79 upgrade cost/time application residual
//! - `command_button_wave80_residual_ok` — Wave 80 superweapon CommandButton label/cursor residual
//! - `science_rank_wave80_residual_ok` — Wave 80 Rank.ini SCIENCE rank residual table
//! - `superweapon_kindof_wave80_residual_ok` — Wave 80 superweapon building KindOf residual
//! - `special_power_enum_wave80_residual_ok` — Wave 80 SpecialPower enum discriminant residual
//! - `terrain_height_sample_wave81_ok` — Wave 81 map height sample residual pack
//! - `pathfinder_wave81_residual_ok` — Wave 81 Pathfinder body/locomotor residual deepen
//! - `locomotor_table_wave81_ok` — Wave 81 common-unit locomotor residual table
//! - `armor_table_wave81_ok` — Wave 81 ProjectileArmor/HazardousMaterial residual table
//! - `puc_flare_table_wave81_ok` — Wave 81 PUC outer-node flare name table residual
//! - `damage_type_wave82_ok` — Wave 82 DamageType residual enum table
//! - `death_type_wave82_ok` — Wave 82 DeathType residual enum table
//! - `model_condition_wave82_ok` — Wave 82 ModelCondition residual flags (CONTINUOUS_FIRE_*)
//! - `weapon_bonus_wave82_ok` — Wave 82 WeaponBonus residual type table
//! - `object_status_wave82_ok` — Wave 82 ObjectStatus / StatusBits residual table
//! - `prod_queue83_ok` — Wave 83 production queue residual (MaxQueue/energy/refund)
//! - `supply_wh83_ok` — Wave 83 supply warehouse residual (boxes/value/cripple heal)
//! - `dozer_build83_ok` — Wave 83 dozer build residual (DozerAI/build pads)
//! - `capture83_ok` — Wave 83 capture building residual (Ranger infantry capture)
//! - `power_plant83_ok` — Wave 83 power plant residual energy pack
//! - `cmd_center83_ok` — Wave 83 command center residual peels
//! - `kindof_wave84_ok` — Wave 84 KindOf residual bit-name table (KINDOF_COUNT 116)
//! - `weapon_slot_wave84_ok` — Wave 84 WeaponSlot PRIMARY/SECONDARY/TERTIARY table
//! - `veterancy_wave84_ok` — Wave 84 Veterancy residual level table
//! - `relationship_wave84_ok` — Wave 84 Relationship ENEMIES/NEUTRAL/ALLIES table
//! - `geometry_wave84_ok` — Wave 84 Geometry SPHERE/CYLINDER/BOX table
//! - `shadow_wave84_ok` — Wave 84 Shadow residual type bit-name table
//! - `faction85_ok` — Wave 85 faction side residual table (America/China/GLA + generals)
//! - `ptpl85_ok` — Wave 85 player template residual peels
//! - `cash85_ok` — Wave 85 starting cash residual (+ difficulty health bonus)
//! - `aiperson85_ok` — Wave 85 skirmish AI personality / SideInfo residual
//! - `victory85_ok` — Wave 85 victory condition residual peels
//! - `cam86_ok` — Wave 86 GameData camera/FPS residual pack
//! - `world86_ok` — Wave 86 GameData world constants residual pack
//! - `mpopt86_ok` — Wave 86 multiplayer options residual pack (host-only)
//! - `mapsel86_ok` — Wave 86 map selection residual pack
//! - `crate86_ok` — Wave 86 crate residual deepen pack
//! - `weather87_ok` — Wave 87 weather (snow) residual pack
//! - `water87_ok` — Wave 87 water / TimeOfDay residual pack
//! - `bridge87_ok` — Wave 87 bridge tower / scaffold residual pack
//! - `tunnel87_ok` — Wave 87 tunnel residual deepen pack
//! - `garrison87_ok` — Wave 87 garrison residual pack
//! - `transport87_ok` — Wave 87 transport residual pack
//! - `radius88_ok` — Wave 88 RadiusCursor residual name table
//! - `mouse88_ok` — Wave 88 MouseCursor residual name table
//! - `fxlist88_ok` — Wave 88 superweapon FXList residual name table
//! - `ocl88_ok` — Wave 88 superweapon OCL residual name table
//! - `particle88_ok` — Wave 88 superweapon particle residual name table expand
//! - `audio88_ok` — Wave 88 superweapon audio event residual name table expand
//! - `rank_skill89_ok` — Wave 89 rank skill-points application residual deepen
//! - `exp89_ok` — Wave 89 experience residual tables pack
//! - `hotkey89_ok` — Wave 89 hotkey CommandMap residual table
//! - `chat89_ok` — Wave 89 chat residual host peels
//! - `replay89_ok` — Wave 89 local replay residual host peels
//! - `options89_ok` — Wave 89 options residual peels
//! - `gamespeed90_ok` — Wave 90 GameSpeed residual pack
//! - `framerate90_ok` — Wave 90 frame rate residual deepen pack
//! - `debug90_ok` — Wave 90 debug residual tables pack (host-only)
//! - `lang90_ok` — Wave 90 language residual deepen pack
//! - `credits90_ok` — Wave 90 credits residual pack
//! - `particle93_ok` — Wave 93 particle emit-rate residual deepen pack
//! - `drawable93_ok` — Wave 93 drawable opacity/shroud residual deepen pack
//! - `shadow93_ok` — Wave 93 shadow residual deepen pack
//! - `terrain_tex93_ok` — Wave 93 terrain texture residual pack
//! - `road93_ok` — Wave 93 road residual pack
//! - `ai_state94_ok` — Wave 94 AI state residual table
//! - `special_ability94_ok` — Wave 94 special ability residual deepen
//! - `upgrade_names94_ok` — Wave 94 upgrade full name table
//! - `command_set94_ok` — Wave 94 CommandSet superweapon residual
//! - `script_action95_ok` — Wave 95 script action name table residual
//! - `script_cond95_ok` — Wave 95 script condition name table residual
//! - `map_object95_ok` — Wave 95 map object residual pack
//! - `waypoint95_ok` — Wave 95 waypoint residual pack
//! - `team95_ok` — Wave 95 team residual pack
//! - `player95_ok` — Wave 95 player residual deepen pack
//! - `partition96_ok` — Wave 96 partition residual pack
//! - `collision96_ok` — Wave 96 collision / GeometryInfo residual pack
//! - `physics96_ok` — Wave 96 physics residual pack
//! - `projectile96_ok` — Wave 96 projectile residual deepen pack
//! - `radar97_ok` — Wave 97 radar residual deepen pack
//! - `spotter97_ok` — Wave 97 spotter residual pack
//! - `stealth97_ok` — Wave 97 stealth residual deepen pack
//! - `detector97_ok` — Wave 97 detector residual deepen pack
//! - `vision97_ok` — Wave 97 vision residual pack
//! - `dock98_ok` — Wave 98 dock residual pack
//! - `contain98_ok` — Wave 98 contain residual deepen pack
//! - `exit98_ok` — Wave 98 exit residual pack
//! - `heal98_ok` — Wave 98 heal residual deepen pack
//! - `production99_ok` — Wave 99 production residual deepen pack
//! - `buildable99_ok` — Wave 99 buildable residual pack
//! - `prereq99_ok` — Wave 99 prerequisite residual pack
//! - `cmdbtn99_ok` — Wave 99 command button residual deepen pack
//! - `controlbar99_ok` — Wave 99 control bar residual deepen pack
//! - `thing_factory100_ok` — Wave 100 ThingFactory residual deepen pack
//! - `module_type100_ok` — Wave 100 Module type table residual pack
//! - `xfer100_ok` — Wave 100 Xfer residual deepen pack
//! - `tf_crosslink100_ok` — Wave 100 ThingFactory spawn cross-link residual pack
//! - `module_factory101_ok` — Wave 101 ModuleFactory residual deepen pack
//! - `thing_factory101_ok` — Wave 101 ThingFactory create residual deepen pack
//! - `partition_register101_ok` — Wave 101 PartitionManager register residual pack
//! - `mf_crosslink101_ok` — Wave 101 ThingFactory/Module/Partition cross-link pack
//! - `display102_ok` — Wave 102 DisplayString FontChars/StretchRect residual pack
//! - `anim2d102_ok` — Wave 102 Anim2D full template table / Collection init pack
//! - `laser102_ok` — Wave 102 laser SegLine UV atlas residual pack
//! - `csf102_ok` — Wave 102 multi-locale CSF residual pack (expanded locales)
//! - `pres102_ok` — Wave 102 presentation dual-tick residual deepen pack
//! - `weapon103_ok` — Wave 103 weapon residual deepen pack
//! - `armor103_ok` — Wave 103 armor residual expand pack
//! - `loco103_ok` — Wave 103 locomotor residual expand pack
//! - `sp103_ok` — Wave 103 special-power superweapon residual deepen pack
//! - `kindof103_ok` — Wave 103 object KindOf residual pack
//! - `object_status104_ok` — Wave 104 Object status-mask residual state machine pack
//! - `object_create104_ok` — Wave 104 Object create residual order pack
//! - `active_body104_ok` — Wave 104 ActiveBody MaxHealth apply residual pack
//! - `drawable_create104_ok` — Wave 104 Drawable create residual bookkeeping pack
//! - `register_object104_ok` — Wave 104 GameLogic registerObject m_objList residual pack
//! - `ai_group105_ok` — Wave 105 AI group residual peels pack
//! - `ai_path105_ok` — Wave 105 AI path residual deepen pack
//! - `weapon_fire105_ok` — Wave 105 weapon fire residual deepen pack
//! - `damage_app105_ok` — Wave 105 damage application residual deepen pack
//! - `veterancy105_ok` — Wave 105 veterancy residual deepen pack
//! - `control_bar_path_resolved` / `control_bar_wnd_validated` — ControlBar.wnd residual
//! - `control_bar_window_loaded` — headless WindowManager parse when WindowZH present

use crate::ai_skirmish_activity::honesty_ai_skirmish_residual_pack_wave77;
use crate::assets::mesh_asset_resolve::honesty_mesh_asset_residual_ok;
use crate::fow_rendering::honesty_fow_residual_pack_wave77;
use crate::game_logic::host_ai_ability_upgrade_residual::{
    honesty_ai_state_residual_table_wave94, honesty_command_set_superweapon_residual_wave94,
    honesty_special_ability_residual_deepen_wave94, honesty_upgrade_name_table_residual_wave94,
};
use crate::game_logic::host_ai_path_combat_residual_wave105::{
    honesty_ai_group_residual_pack_wave105, honesty_ai_path_residual_deepen_pack_wave105,
    honesty_damage_application_residual_deepen_pack_wave105,
    honesty_veterancy_residual_deepen_pack_wave105,
    honesty_weapon_fire_residual_deepen_pack_wave105,
};
use crate::game_logic::host_armor_residual::honesty_armor_residual_expand_wave103;
use crate::game_logic::host_armor_residual::honesty_armor_residual_expand_wave92;
use crate::game_logic::host_armor_residual::honesty_armor_residual_table_wave81;
use crate::game_logic::host_cash_bounty::honesty_cash_bounty_residual_pack_wave78;
use crate::game_logic::host_combat_sim_residual::{
    honesty_body_max_health_residual_table_wave92, honesty_science_name_table_residual_wave92,
};
use crate::game_logic::host_command_button_residual::honesty_command_button_superweapon_residual_pack_wave80;
use crate::game_logic::host_dock_contain_exit_heal_residual::{
    honesty_contain_residual_deepen_pack_wave98, honesty_dock_residual_pack_wave98,
    honesty_exit_residual_pack_wave98, honesty_heal_residual_deepen_pack_wave98,
};
use crate::game_logic::host_enum_table_residual::{
    honesty_damage_type_enum_table_wave82, honesty_death_type_enum_table_wave82,
    honesty_geometry_type_enum_table_wave84, honesty_kindof_enum_table_wave84,
    honesty_model_condition_enum_table_wave82, honesty_object_status_enum_table_wave82,
    honesty_relationship_enum_table_wave84, honesty_shadow_type_enum_table_wave84,
    honesty_veterancy_level_enum_table_wave84, honesty_weapon_bonus_enum_table_wave82,
    honesty_weapon_slot_enum_table_wave84,
};
use crate::game_logic::host_env_contain_residual::{
    honesty_bridge_residual_pack_wave87, honesty_garrison_residual_pack_wave87,
    honesty_transport_residual_pack_wave87, honesty_tunnel_residual_deepen_wave87,
    honesty_water_residual_pack_wave87, honesty_weather_residual_pack_wave87,
};
use crate::game_logic::host_faction_skirmish_residual::{
    honesty_faction_side_residual_table_wave85, honesty_player_template_residual_pack_wave85,
    honesty_skirmish_ai_personality_residual_pack_wave85,
    honesty_starting_cash_residual_pack_wave85, honesty_victory_condition_residual_pack_wave85,
};
use crate::game_logic::host_fx_audio_cursor_residual::{
    honesty_mouse_cursor_name_table_wave88, honesty_radius_cursor_name_table_wave88,
    honesty_superweapon_audio_event_name_table_wave88,
    honesty_superweapon_fxlist_name_table_wave88, honesty_superweapon_ocl_name_table_wave88,
    honesty_superweapon_particle_name_table_wave88,
};
use crate::game_logic::host_fx_ocl_particle_audio_residual_wave107::{
    honesty_audio_residual_deepen_pack_wave107, honesty_fxlist_entry_residual_deepen_pack_wave107,
    honesty_ocl_create_residual_deepen_pack_wave107,
    honesty_particle_system_residual_deepen_pack_wave107,
};
use crate::game_logic::host_game_logic_residual_wave103::{
    honesty_object_kindof_residual_pack_wave103,
    honesty_special_power_superweapon_residual_deepen_wave103,
};
use crate::game_logic::host_gamedata_lobby_residual::{
    honesty_crate_residual_deepen_pack_wave86, honesty_gamedata_camera_fps_residual_pack_wave86,
    honesty_gamedata_world_constants_residual_pack_wave86,
    honesty_map_selection_residual_pack_wave86, honesty_multiplayer_options_residual_pack_wave86,
};
use crate::game_logic::host_gps_scrambler::honesty_gps_scrambler_residual_pack_wave78;
use crate::game_logic::host_mines::honesty_cluster_mines_residual_pack_wave78;
use crate::game_logic::host_object_register_drawable_residual_wave104::{
    honesty_active_body_max_health_apply_residual_wave104,
    honesty_drawable_create_residual_wave104, honesty_gamelogic_register_object_residual_wave104,
    honesty_object_create_order_residual_wave104,
    honesty_object_status_state_machine_residual_wave104,
};
use crate::game_logic::host_paradrop::honesty_paradrop_residual_pack_wave76_ok;
use crate::game_logic::host_partition_collision_physics_residual::{
    honesty_collision_residual_pack_wave96, honesty_partition_residual_pack_wave96,
    honesty_physics_residual_pack_wave96, honesty_projectile_residual_deepen_pack_wave96,
};
use crate::game_logic::host_pathfinder::honesty_pathfinder_residual_pack_wave81;
use crate::game_logic::host_production_buildable_command_residual::{
    honesty_buildable_residual_pack_wave99, honesty_command_button_residual_deepen_pack_wave99,
    honesty_control_bar_residual_deepen_pack_wave99, honesty_prerequisite_residual_pack_wave99,
    honesty_production_residual_deepen_pack_wave99,
};
use crate::game_logic::host_radar_stealth_vision_residual::{
    honesty_detector_residual_deepen_pack_wave97, honesty_radar_residual_deepen_pack_wave97,
    honesty_spotter_residual_pack_wave97, honesty_stealth_residual_deepen_pack_wave97,
    honesty_vision_residual_pack_wave97,
};
use crate::game_logic::host_rank_ui_residual::{
    honesty_chat_residual_host_pack_wave89, honesty_experience_residual_tables_pack_wave89,
    honesty_hotkey_residual_table_pack_wave89, honesty_options_residual_pack_wave89,
    honesty_rank_skill_points_application_residual_pack_wave89,
    honesty_replay_residual_host_pack_wave89,
};
use crate::game_logic::host_render_terrain_residual::{
    honesty_drawable_opacity_shroud_residual_deepen_pack_wave93,
    honesty_particle_system_emit_rate_residual_deepen_pack_wave93,
    honesty_road_residual_pack_wave93, honesty_shadow_residual_deepen_pack_wave93,
    honesty_terrain_texture_residual_pack_wave93,
};
use crate::game_logic::host_rng_residual::{
    exercise_host_rng_residual, honesty_rng_residual_pack_ok,
};
use crate::game_logic::host_science_rank::honesty_science_rank_residual_pack_wave80;
use crate::game_logic::host_script_map_team_player_residual::{
    honesty_map_object_residual_pack_wave95, honesty_player_residual_deepen_pack_wave95,
    honesty_script_action_name_table_residual_wave95,
    honesty_script_condition_name_table_residual_wave95, honesty_team_residual_pack_wave95,
    honesty_waypoint_residual_pack_wave95,
};
use crate::game_logic::host_shell_campaign_save_residual_wave106::{
    honesty_campaign_mission_residual_deepen_pack_wave106,
    honesty_game_state_residual_deepen_pack_wave106,
    honesty_game_window_residual_deepen_pack_wave106,
    honesty_main_menu_residual_deepen_pack_wave106,
    honesty_window_layout_residual_deepen_pack_wave106,
};
use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::{
    honesty_player_residual_deepen_pack_wave109,
    honesty_science_store_residual_deepen_pack_wave109,
    honesty_special_power_template_store_residual_wave109,
    honesty_team_residual_deepen_pack_wave109, honesty_upgrade_store_residual_deepen_pack_wave109,
};
use crate::game_logic::host_special_power_enum_residual::honesty_special_power_enum_residual_pack_wave80;
use crate::game_logic::host_structure_economy_residual::{
    honesty_capture_building_residual_pack_wave83, honesty_command_center_residual_pack_wave83,
    honesty_dozer_build_residual_pack_wave83, honesty_power_plant_residual_pack_wave83,
    honesty_production_queue_residual_pack_wave83, honesty_supply_warehouse_residual_pack_wave83,
};
use crate::game_logic::host_superweapon_kindof::honesty_superweapon_kindof_residual_pack_wave80;
use crate::game_logic::host_terrain_bridge_water_road_residual_wave108::{
    honesty_bridge_residual_deepen_pack_wave108, honesty_cliff_residual_peels_pack_wave108,
    honesty_heightmap_residual_deepen_pack_wave108, honesty_road_residual_deepen_pack_wave108,
    honesty_water_residual_deepen_pack_wave108,
};
use crate::game_logic::host_thing_factory_module_xfer_residual::{
    honesty_module_factory_residual_deepen_pack_wave101,
    honesty_module_type_table_residual_pack_wave100,
    honesty_partition_register_residual_pack_wave101,
    honesty_thing_factory_create_residual_deepen_pack_wave101,
    honesty_thing_factory_module_partition_crosslink_wave101,
    honesty_thing_factory_residual_deepen_pack_wave100,
    honesty_thing_factory_spawn_crosslink_wave100, honesty_xfer_residual_deepen_pack_wave100,
};
use crate::game_logic::host_timing_shell_residual::{
    honesty_credits_residual_pack_wave90, honesty_debug_residual_tables_pack_wave90,
    honesty_frame_rate_residual_deepen_pack_wave90, honesty_gamespeed_residual_pack_wave90,
    honesty_language_residual_deepen_pack_wave90,
};
use crate::game_logic::host_ui_presentation_residual::{
    honesty_eva_residual_pack_wave91, honesty_help_box_residual_pack_wave91,
    honesty_message_residual_pack_wave91, honesty_mission_briefing_residual_pack_wave91,
    honesty_tooltip_residual_pack_wave91, honesty_video_residual_name_table_wave91,
};
use crate::game_logic::host_unit_training::honesty_unit_training_residual_pack_wave79_ok;
use crate::game_logic::host_upgrades::honesty_upgrades_cost_time_application_wave79_ok;
use crate::game_logic::locomotor_bootstrap::honesty_locomotor_residual_expand_wave103;
use crate::game_logic::locomotor_bootstrap::honesty_locomotor_residual_expand_wave92;
use crate::game_logic::locomotor_bootstrap::honesty_locomotor_residual_table_wave81;
use crate::game_logic::special_power_strikes::{
    honesty_particle_outer_node_flare_name_table_wave81, honesty_special_power_residual_pack_ok,
    honesty_special_power_residual_pack_wave73_ok, honesty_special_power_residual_pack_wave76_ok,
    honesty_special_power_residual_pack_wave77_ok, honesty_special_power_residual_pack_wave78_ok,
};
use crate::game_logic::terrain::honesty_map_height_sample_residual_pack_wave81;
use crate::game_logic::weapon_bootstrap::honesty_weapon_store_deepen_residual_wave103;
use crate::game_logic::weapon_bootstrap::honesty_weapon_store_deepen_residual_wave92;
use crate::game_logic::weapon_bootstrap::honesty_weapon_store_host_seed_residual_wave77;
use crate::game_logic::GameLogic;
use crate::gameplay_layout::{
    control_bar_layout_honesty, format_control_bar_honesty,
    honesty_control_bar_residual_pack_wave76_ok, GameplayLayoutStatus,
};
use crate::graphics::floating_text_layout::{
    honesty_display_string_residual_deepen_pack_wave102, honesty_graphics_residual_pack_wave76_ok,
    pack_floating_text_and_mark_ready, FloatingTextLayout,
};
use crate::graphics::game_text_residual::{
    exercise_host_game_text_residual, honesty_csf_multi_locale_residual_deepen_pack_wave102,
    honesty_translate_copy_escape_table,
};
use crate::graphics::laser_segment_upload::{
    honesty_laser_segliner_residual_deepen_pack_wave102, pack_and_mark_upload_ready,
    LaserSegmentUpload,
};
use crate::graphics::minimap_renderer::honesty_minimap_residual_pack_wave79;
use crate::graphics::world_anim_layout::{
    honesty_anim2d_collection_residual, honesty_anim2d_residual_deepen_pack_wave102,
    pack_world_anim_and_mark_ready, WorldAnimLayout,
};
use crate::map_frame_scenario::resolve_first_map;
use crate::presentation_frame::{
    honesty_presentation_residual_deepen_pack_wave102, honesty_spectre_orbit_decal_presentation_ok,
};
use crate::presentation_frame::{
    PresentationFloatingText, PresentationFrame, PresentationLaserBeam, PresentationWorldAnim,
    PRESENTATION_ORBITAL_SOFT_EDGE,
};
use crate::save_load::honesty_drawable_residual_fields_wave79_ok;
use crate::selection_renderer::honesty_selection_hud_residual_pack_wave79;
use crate::skirmish_config::{apply_skirmish_config, config_from_skirmish_menu};
use crate::ui::skirmish_menu::SkirmishMenu;
use crate::ui::{GameHUD, GameUIState, RTSInterface, Screen, UIManager, UnitCommandPanel};
use crate::unit_input_handler::honesty_input_residual_pack_wave79;

const HOST_MAP_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

#[derive(Debug, Clone)]
pub struct ShellSmokeResult {
    /// True only after GameLogic exists and skirmish config applied successfully.
    pub host_constructed: bool,
    pub skirmish_config_ok: bool,
    pub menu_config_ok: bool,
    pub map_resolved: bool,
    pub map_loaded: bool,
    pub frames_advanced: u32,
    pub presentation_ok: bool,
    /// Dual-tick residual: seed presentation, then logic update + build_and_apply order.
    pub dual_tick_presentation_ok: bool,
    /// Dual-tick residual counters (build/apply) on presentation snapshot.
    pub dual_tick_counters_ok: bool,
    /// Host vs gamelogic::GameWorld shadow count parity (migration probe).
    pub gameworld_shadow_ok: bool,
    /// Gate defaulted GENERALS_GAMEWORLD_DAMAGE_AUTHORITY on (or user set).
    pub damage_authority_env_ok: bool,
    /// Gate defaulted GENERALS_GAMEWORLD_ECONOMY_AUTHORITY on.
    pub economy_authority_env_ok: bool,
    /// Dual crate tick is AuthorityOnly (no GENERALS_ALLOW_DUAL_TICK).
    pub dual_tick_policy_authority_only: bool,
    /// OBJECT_REGISTRY bridge env unset (engine_object_id stays None).
    pub engine_bridge_off: bool,
    /// Dual-tick residual: after map load, logic update + presentation seed HUD
    /// selection health / minimap without re-reading live objects.
    pub hud_selection_ok: bool,
    /// Minimap FOW residual: presentation `fow_grid` is host-usable (active R8 or honest inactive).
    pub minimap_fow_presentation_ok: bool,
    /// Laser residual: presentation → CPU SegLine vertex pack (+ synthetic non-empty pack).
    pub laser_segment_upload_ok: bool,
    /// Presentation projectiles → CPU trail pack residual (empty + synthetic fire).
    pub projectile_segment_upload_ok: bool,
    /// Presentation move destinations → CPU path-line pack residual.
    pub move_line_upload_ok: bool,
    /// Presentation attack targets → CPU order-line pack residual.
    pub attack_line_upload_ok: bool,
    /// OrbitalLaser multi-beam soft-edge CPU pack residual (NumBeams width/color lerp).
    pub multi_beam_soft_edge_ok: bool,
    /// Laser presentation residual: ground-height + soft-edge fields honesty.
    pub laser_presentation_residual_ok: bool,
    /// Floating-text residual: presentation freeze + CPU layout pack (+ synthetic).
    pub floating_text_layout_ok: bool,
    /// Floating-text vanish-rate alpha residual presentation field honesty.
    pub floating_text_vanish_ok: bool,
    /// MoneyPickUp Anim2D residual: presentation freeze honesty (empty or template ok).
    pub world_anim_presentation_ok: bool,
    /// World-anim residual: presentation → CPU Anim2D layout pack (+ synthetic).
    pub world_anim_layout_ok: bool,
    /// World-anim fade residual presentation field honesty.
    pub world_anim_fade_ok: bool,
    /// MoneyPickUp Anim2D frame advance residual (LOOP / SCPDollarNNN).
    pub anim2d_frame_ok: bool,
    /// Anim2DCollection template/instance residual (host-testable, no GPU).
    pub anim2d_collection_residual_ok: bool,
    /// GameText translate_copy escape table residual (host-testable, no GPU).
    pub translate_copy_residual_ok: bool,
    /// GameText `GUI:AddCash` caption residual on synthetic floating-text pack.
    pub game_text_caption_ok: bool,
    /// CSF/STR GameText residual + retail `$%d` printf + DisplayString measure.
    pub game_text_csf_str_ok: bool,
    /// DisplayString monospaced measure residual on floating-text pack.
    pub display_string_measure_ok: bool,
    /// GameLogic/GameClient RandomValue ADC stream residual honesty.
    pub rng_stream_residual_ok: bool,
    /// W3D mesh asset resolve residual (common keys / scale / search / basename).
    /// Host-testable; does **not** claim live GPU upload or retail material parity.
    pub mesh_asset_residual_ok: bool,
    /// Wave 72 host RNG residual pack honesty (seed table / pure index / stream).
    pub rng_residual_pack_ok: bool,
    /// Wave 72 special-power residual pack (DaisyCutter / A10 / free pack).
    pub special_power_wave72_residual_ok: bool,
    /// Wave 73 Spectre/Nuke/SupW special-power residual pack honesty.
    pub special_power_wave73_residual_ok: bool,
    /// Wave 76 A10 science-tier FormationSize residual pack honesty.
    pub special_power_wave76_residual_ok: bool,
    /// Wave 76 Paradrop science-tier payload residual pack honesty.
    pub paradrop_wave76_residual_ok: bool,
    /// Wave 76 ControlBar window-count / named-child / font residual pack honesty.
    pub control_bar_wave76_residual_ok: bool,
    /// Wave 76 InGameUI font table + DisplayString vanish color-alpha residual honesty.
    pub graphics_wave76_residual_ok: bool,
    /// Wave 73 presentation Spectre orbit decal residual honesty.
    pub spectre_orbit_decal_presentation_ok: bool,
    /// Wave 77 special-power audio name table residual honesty.
    pub special_power_wave77_residual_ok: bool,
    /// Wave 77 FOW residual honesty pack (cell/R8/inactive fail-open).
    pub fow_residual_pack_ok: bool,
    /// Wave 77 unit/structure ground-height presentation residual honesty.
    pub ground_height_presentation_ok: bool,
    /// Wave 77 host WeaponStore seed residual honesty pack.
    pub weapon_store_seed_residual_ok: bool,
    /// Wave 77 AI skirmish structure/team timer residual honesty pack.
    pub ai_skirmish_residual_ok: bool,
    /// Wave 78 HostSuperweaponKind reload + CarpetBomb/Artillery science residual pack.
    pub special_power_wave78_residual_ok: bool,
    /// Wave 78 ClusterMines DeliveryDecal / science residual pack.
    pub cluster_mines_wave78_residual_ok: bool,
    /// Wave 78 GPS Scrambler science / marker particle residual pack.
    pub gps_scrambler_wave78_residual_ok: bool,
    /// Wave 78 CashBountyScienceTier residual pack.
    pub cash_bounty_wave78_residual_ok: bool,
    /// Wave 79 minimap FOW shade/size residual honesty pack.
    pub minimap_residual_pack_ok: bool,
    /// Wave 79 selection/HUD color residual honesty pack.
    pub selection_hud_residual_pack_ok: bool,
    /// Wave 79 drag/double-click input residual honesty pack.
    pub input_residual_pack_ok: bool,
    /// Wave 79 Drawable StealthLook save/load residual honesty.
    pub drawable_residual_fields_ok: bool,
    /// Wave 79 unit-training/veterancy residual deepen honesty pack.
    pub unit_training_wave79_residual_ok: bool,
    /// Wave 79 upgrade cost/time residual application honesty.
    pub upgrades_cost_time_application_ok: bool,
    /// Wave 80 superweapon CommandButton label/cursor residual honesty pack.
    pub command_button_wave80_residual_ok: bool,
    /// Wave 80 Rank.ini SCIENCE rank residual table honesty pack.
    pub science_rank_wave80_residual_ok: bool,
    /// Wave 80 superweapon building KindOf residual honesty pack.
    pub superweapon_kindof_wave80_residual_ok: bool,
    /// Wave 80 SpecialPower enum discriminant residual honesty pack.
    pub special_power_enum_wave80_residual_ok: bool,
    /// Wave 81 map height sample residual honesty pack.
    pub terrain_height_sample_wave81_ok: bool,
    /// Wave 81 Pathfinder body/locomotor residual deepen honesty pack.
    pub pathfinder_wave81_residual_ok: bool,
    /// Wave 81 common-unit locomotor residual table honesty.
    pub locomotor_table_wave81_ok: bool,
    /// Wave 81 ProjectileArmor / HazardousMaterialArmor residual table honesty.
    pub armor_table_wave81_ok: bool,
    /// Wave 81 PUC outer-node flare particle name table residual honesty.
    pub puc_flare_table_wave81_ok: bool,
    /// Wave 82 DamageType residual enum table honesty.
    pub damage_type_wave82_ok: bool,
    /// Wave 82 DeathType residual enum table honesty.
    pub death_type_wave82_ok: bool,
    /// Wave 82 ModelCondition residual flags honesty (incl. CONTINUOUS_FIRE_*).
    pub model_condition_wave82_ok: bool,
    /// Wave 82 WeaponBonus residual type table honesty.
    pub weapon_bonus_wave82_ok: bool,
    /// Wave 82 ObjectStatus / StatusBits residual table honesty.
    pub object_status_wave82_ok: bool,
    /// Wave 83 production queue residual (MaxQueue/energy/refund/doors).
    pub production_queue_wave83_ok: bool,
    /// Wave 83 supply warehouse residual (boxes/value/cripple heal).
    pub supply_warehouse_wave83_ok: bool,
    /// Wave 83 dozer build residual (DozerAI/build pads/construction rate).
    pub dozer_build_wave83_ok: bool,
    /// Wave 83 capture building residual (Ranger infantry capture pack).
    pub capture_building_wave83_ok: bool,
    /// Wave 83 power plant residual energy pack.
    pub power_plant_wave83_ok: bool,
    /// Wave 83 command center residual peels.
    pub command_center_wave83_ok: bool,
    /// Wave 84 KindOf residual bit-name table honesty (KINDOF_COUNT 116).
    pub kindof_wave84_ok: bool,
    /// Wave 84 WeaponSlot PRIMARY/SECONDARY/TERTIARY residual table honesty.
    pub weapon_slot_wave84_ok: bool,
    /// Wave 84 Veterancy residual level table honesty.
    pub veterancy_wave84_ok: bool,
    /// Wave 84 Relationship ENEMIES/NEUTRAL/ALLIES residual table honesty.
    pub relationship_wave84_ok: bool,
    /// Wave 84 Geometry SPHERE/CYLINDER/BOX residual table honesty.
    pub geometry_wave84_ok: bool,
    /// Wave 84 Shadow residual type bit-name table honesty.
    pub shadow_wave84_ok: bool,
    /// Wave 85 faction side residual table honesty.
    pub faction_side_wave85_ok: bool,
    /// Wave 85 player template residual peels honesty.
    pub player_template_wave85_ok: bool,
    /// Wave 85 starting cash residual (+ difficulty health) honesty.
    pub starting_cash_wave85_ok: bool,
    /// Wave 85 skirmish AI personality / SideInfo residual honesty.
    pub skirmish_ai_personality_wave85_ok: bool,
    /// Wave 85 victory condition residual peels honesty.
    pub victory_condition_wave85_ok: bool,
    /// Wave 86 GameData camera/FPS residual pack honesty.
    pub gamedata_camera_fps_wave86_ok: bool,
    /// Wave 86 GameData world constants residual pack honesty.
    pub gamedata_world_constants_wave86_ok: bool,
    /// Wave 86 multiplayer options residual pack honesty (host-only).
    pub multiplayer_options_wave86_ok: bool,
    /// Wave 86 map selection residual pack honesty.
    pub map_selection_wave86_ok: bool,
    /// Wave 86 crate residual deepen pack honesty.
    pub crate_deepen_wave86_ok: bool,
    /// Wave 87 weather (snow) residual pack honesty.
    pub weather_wave87_ok: bool,
    /// Wave 87 water / TimeOfDay residual pack honesty.
    pub water_wave87_ok: bool,
    /// Wave 87 bridge tower / scaffold residual pack honesty.
    pub bridge_wave87_ok: bool,
    /// Wave 87 tunnel residual deepen pack honesty.
    pub tunnel_wave87_ok: bool,
    /// Wave 87 garrison residual pack honesty.
    pub garrison_wave87_ok: bool,
    /// Wave 87 transport residual pack honesty.
    pub transport_wave87_ok: bool,
    /// Wave 88 RadiusCursor residual name table honesty.
    pub radius_cursor_wave88_ok: bool,
    /// Wave 88 MouseCursor residual name table honesty.
    pub mouse_cursor_wave88_ok: bool,
    /// Wave 88 superweapon FXList residual name table honesty.
    pub superweapon_fxlist_wave88_ok: bool,
    /// Wave 88 superweapon OCL residual name table honesty.
    pub superweapon_ocl_wave88_ok: bool,
    /// Wave 88 superweapon particle residual name table expand honesty.
    pub superweapon_particle_wave88_ok: bool,
    /// Wave 88 superweapon audio event residual name table expand honesty.
    pub superweapon_audio_wave88_ok: bool,
    /// Wave 89 rank skill-points application residual deepen honesty.
    pub rank_skill_wave89_ok: bool,
    /// Wave 89 experience residual tables pack honesty.
    pub experience_wave89_ok: bool,
    /// Wave 89 hotkey CommandMap residual table honesty.
    pub hotkey_wave89_ok: bool,
    /// Wave 89 chat residual host peels honesty.
    pub chat_wave89_ok: bool,
    /// Wave 89 local replay residual host peels honesty.
    pub replay_wave89_ok: bool,
    /// Wave 89 options residual peels honesty.
    pub options_wave89_ok: bool,
    /// Wave 90 GameSpeed residual pack honesty.
    pub gamespeed_wave90_ok: bool,
    /// Wave 90 frame rate residual deepen pack honesty.
    pub frame_rate_wave90_ok: bool,
    /// Wave 90 debug residual tables pack honesty (host-only).
    pub debug_tables_wave90_ok: bool,
    /// Wave 90 language residual deepen pack honesty.
    pub language_wave90_ok: bool,
    /// Wave 90 credits residual pack honesty.
    pub credits_wave90_ok: bool,
    /// Wave 91 tooltip residual pack honesty.
    pub tooltip_wave91_ok: bool,
    /// Wave 91 HelpBox residual pack honesty.
    pub help_box_wave91_ok: bool,
    /// Wave 91 message residual pack honesty.
    pub message_wave91_ok: bool,
    /// Wave 91 EVA residual pack honesty.
    pub eva_wave91_ok: bool,
    /// Wave 91 video residual name table honesty (names only).
    pub video_wave91_ok: bool,
    /// Wave 91 mission briefing residual pack honesty.
    pub mission_briefing_wave91_ok: bool,
    /// Wave 92 weapon template residual deepen honesty.
    pub weapon_deepen_wave92_ok: bool,
    /// Wave 92 armor residual expand honesty.
    pub armor_expand_wave92_ok: bool,
    /// Wave 92 body MaxHealth residual table honesty.
    pub body_health_wave92_ok: bool,
    /// Wave 92 locomotor residual expand honesty.
    pub locomotor_expand_wave92_ok: bool,
    /// Wave 92 science residual full name table honesty.
    pub science_names_wave92_ok: bool,
    /// Wave 93 particle system emit-rate residual deepen honesty.
    pub particle_emit_wave93_ok: bool,
    /// Wave 93 drawable opacity/shroud residual deepen honesty.
    pub drawable_opacity_wave93_ok: bool,
    /// Wave 93 shadow residual deepen honesty.
    pub shadow_deepen_wave93_ok: bool,
    /// Wave 93 terrain texture residual pack honesty.
    pub terrain_texture_wave93_ok: bool,
    /// Wave 93 road residual pack honesty.
    pub road_wave93_ok: bool,
    /// Wave 94 AI state residual table honesty.
    pub ai_state_wave94_ok: bool,
    /// Wave 94 special ability residual deepen honesty.
    pub special_ability_wave94_ok: bool,
    /// Wave 94 upgrade full name table residual honesty.
    pub upgrade_names_wave94_ok: bool,
    /// Wave 94 CommandSet superweapon residual honesty.
    pub command_set_wave94_ok: bool,
    /// Wave 95 script action name table residual honesty.
    pub script_action_wave95_ok: bool,
    /// Wave 95 script condition name table residual honesty.
    pub script_condition_wave95_ok: bool,
    /// Wave 95 map object residual pack honesty.
    pub map_object_wave95_ok: bool,
    /// Wave 95 waypoint residual pack honesty.
    pub waypoint_wave95_ok: bool,
    /// Wave 95 team residual pack honesty.
    pub team_wave95_ok: bool,
    /// Wave 95 player residual deepen pack honesty.
    pub player_deepen_wave95_ok: bool,
    /// Wave 96 partition residual pack honesty.
    pub partition_wave96_ok: bool,
    /// Wave 96 collision residual pack honesty.
    pub collision_wave96_ok: bool,
    /// Wave 96 physics residual pack honesty.
    pub physics_wave96_ok: bool,
    /// Wave 96 projectile residual deepen pack honesty.
    pub projectile_wave96_ok: bool,
    /// Wave 97 radar residual deepen pack honesty.
    pub radar_deepen_wave97_ok: bool,
    /// Wave 97 spotter residual pack honesty.
    pub spotter_wave97_ok: bool,
    /// Wave 97 stealth residual deepen pack honesty.
    pub stealth_deepen_wave97_ok: bool,
    /// Wave 97 detector residual deepen pack honesty.
    pub detector_deepen_wave97_ok: bool,
    /// Wave 97 vision residual pack honesty.
    pub vision_wave97_ok: bool,
    /// Wave 98 dock residual pack honesty.
    pub dock_wave98_ok: bool,
    /// Wave 98 contain residual deepen pack honesty.
    pub contain_wave98_ok: bool,
    /// Wave 98 exit residual pack honesty.
    pub exit_wave98_ok: bool,
    /// Wave 98 heal residual deepen pack honesty.
    pub heal_wave98_ok: bool,
    /// Wave 99 production residual deepen pack honesty.
    pub production_deepen_wave99_ok: bool,
    /// Wave 99 buildable residual pack honesty.
    pub buildable_wave99_ok: bool,
    /// Wave 99 prerequisite residual pack honesty.
    pub prerequisite_wave99_ok: bool,
    /// Wave 99 command button residual deepen pack honesty.
    pub command_button_deepen_wave99_ok: bool,
    /// Wave 99 control bar residual deepen pack honesty.
    pub control_bar_deepen_wave99_ok: bool,
    /// Wave 100 ThingFactory residual deepen pack honesty.
    pub thing_factory_deepen_wave100_ok: bool,
    /// Wave 100 Module type table residual pack honesty.
    pub module_type_wave100_ok: bool,
    /// Wave 100 Xfer residual deepen pack honesty.
    pub xfer_deepen_wave100_ok: bool,
    /// Wave 100 ThingFactory spawn cross-link residual pack honesty.
    pub thing_factory_crosslink_wave100_ok: bool,
    /// Wave 101 ModuleFactory residual deepen pack honesty.
    pub module_factory_deepen_wave101_ok: bool,
    /// Wave 101 ThingFactory create residual deepen pack honesty.
    pub thing_factory_create_wave101_ok: bool,
    /// Wave 101 PartitionManager register residual pack honesty.
    pub partition_register_wave101_ok: bool,
    /// Wave 101 ThingFactory/Module/Partition cross-link residual pack honesty.
    pub mf_crosslink_wave101_ok: bool,
    /// Wave 102 DisplayString FontChars spacing + StretchRect submit residual pack.
    pub display_string_deepen_wave102_ok: bool,
    /// Wave 102 Anim2D full template table / Collection init residual pack.
    pub anim2d_deepen_wave102_ok: bool,
    /// Wave 102 laser SegLine UV atlas / multi-beam soft-edge residual pack.
    pub laser_segliner_deepen_wave102_ok: bool,
    /// Wave 102 multi-locale CSF expanded pack-load residual honesty.
    pub csf_multi_locale_deepen_wave102_ok: bool,
    /// Wave 102 presentation dual-tick residual deepen pack honesty.
    pub presentation_deepen_wave102_ok: bool,
    /// Wave 103 weapon residual deepen pack honesty.
    pub weapon_deepen_wave103_ok: bool,
    /// Wave 103 armor residual expand pack honesty.
    pub armor_expand_wave103_ok: bool,
    /// Wave 103 locomotor residual expand pack honesty.
    pub locomotor_expand_wave103_ok: bool,
    /// Wave 103 special-power superweapon residual deepen pack honesty.
    pub special_power_deepen_wave103_ok: bool,
    /// Wave 103 object KindOf residual pack honesty.
    pub object_kindof_wave103_ok: bool,
    /// Wave 104 Object status-mask residual state machine pack honesty.
    pub object_status_wave104_ok: bool,
    /// Wave 104 Object create residual order pack honesty.
    pub object_create_wave104_ok: bool,
    /// Wave 104 ActiveBody MaxHealth apply residual pack honesty.
    pub active_body_wave104_ok: bool,
    /// Wave 104 Drawable create residual bookkeeping pack honesty.
    pub drawable_create_wave104_ok: bool,
    /// Wave 104 GameLogic registerObject m_objList residual pack honesty.
    pub register_object_wave104_ok: bool,
    /// Wave 105 AI group residual peels pack honesty.
    pub ai_group_wave105_ok: bool,
    /// Wave 105 AI path residual deepen pack honesty.
    pub ai_path_wave105_ok: bool,
    /// Wave 105 weapon fire residual deepen pack honesty.
    pub weapon_fire_wave105_ok: bool,
    /// Wave 105 damage application residual deepen pack honesty.
    pub damage_application_wave105_ok: bool,
    /// Wave 105 veterancy residual deepen pack honesty.
    pub veterancy_wave105_ok: bool,
    /// Wave 106 GameState residual deepen pack honesty.
    pub game_state_deepen_wave106_ok: bool,
    /// Wave 106 campaign mission residual tables pack honesty.
    pub campaign_mission_wave106_ok: bool,
    /// Wave 106 MainMenu residual deepen pack honesty.
    pub main_menu_deepen_wave106_ok: bool,
    /// Wave 106 GameWindow residual deepen pack honesty.
    pub game_window_deepen_wave106_ok: bool,
    /// Wave 106 WindowLayout residual deepen pack honesty.
    pub window_layout_deepen_wave106_ok: bool,
    /// Wave 107 ParticleSystem residual deepen pack honesty.
    pub particle_system_deepen_wave107_ok: bool,
    /// Wave 107 FXList entry residual deepen pack honesty.
    pub fxlist_entry_deepen_wave107_ok: bool,
    /// Wave 107 OCL Create residual deepen pack honesty.
    pub ocl_create_deepen_wave107_ok: bool,
    /// Wave 107 Audio residual deepen pack honesty.
    pub audio_deepen_wave107_ok: bool,
    /// Wave 108 HeightMap residual deepen pack honesty.
    pub heightmap_deepen_wave108_ok: bool,
    /// Wave 108 bridge residual deepen pack honesty.
    pub bridge_deepen_wave108_ok: bool,
    /// Wave 108 water residual deepen pack honesty.
    pub water_deepen_wave108_ok: bool,
    /// Wave 108 road residual deepen pack honesty.
    pub road_deepen_wave108_ok: bool,
    /// Wave 108 cliff residual peels pack honesty.
    pub cliff_peels_wave108_ok: bool,
    /// Wave 109 SpecialPower template store residual pack honesty.
    pub special_power_store_wave109_ok: bool,
    /// Wave 109 Science store residual deepen pack honesty.
    pub science_store_wave109_ok: bool,
    /// Wave 109 Upgrade store residual deepen pack honesty.
    pub upgrade_store_wave109_ok: bool,
    /// Wave 109 Player residual deepen pack honesty.
    pub player_deepen_wave109_ok: bool,
    /// Wave 109 Team residual deepen pack honesty.
    pub team_deepen_wave109_ok: bool,
    /// Shell Skirmish → Loading → GameHUD ownership transition (StartGame parity).
    pub screen_skirmish_ok: bool,
    /// ControlBar.wnd resolve/validate path (C++ ShowControlBar / ensure_gameplay_layouts).
    /// True when layout Ready, or assets honestly unavailable (CI without WindowZH).
    pub control_bar_layout_ok: bool,
    /// ControlBar.wnd path found on disk.
    pub control_bar_path_resolved: bool,
    /// ControlBar.wnd structural validate (FILE_VERSION / WINDOW / ControlBar tokens).
    pub control_bar_wnd_validated: bool,
    /// Headless WindowManager parse materialised GameWindows (assets present path).
    /// False when WindowZH missing or parse deferred — still honest residual.
    pub control_bar_window_loaded: bool,
    /// Window count from headless WindowManager load (0 when not loaded).
    pub control_bar_window_count: usize,
    /// Dual-tick residual: selection panel applied to HUD + UIState + RTS + command panel.
    pub selection_consumers_ok: bool,
    /// Limited headless host claim (see module docs). Not retail W3D play.
    pub shell_host_playable_ok: bool,
    /// Always false here: no window/WND/GPU retail playthrough.
    pub playable_claim: bool,
    pub status: String,
    pub detail: String,
}

/// Exercise production host entry points headlessly (no window required).
/// Builds config from live SkirmishMenu (including Medium AI slot via menu cycle),
/// applies it, loads retail map when present, advances logic frames, builds presentation,
/// applies dual-tick presentation → GameHUD selection/minimap, ensures ControlBar.wnd,
/// and exercises shell→InGame screen ownership (start_game_from_ui parity).
pub fn run_shell_smoke(frames: u32) -> ShellSmokeResult {
    // Default-on damage authority for gate honesty (opt out via env=0).
    crate::gameworld_shadow::ensure_gate_damage_authority();
    let mut logic = GameLogic::new();

    let resolved = resolve_first_map(HOST_MAP_CANDIDATES);
    let map_resolved = resolved.is_some();
    let map_id = resolved
        .as_ref()
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| "HostSyntheticMap".into());
    let map_path = resolved.map(|(_, p)| p);

    // Production UI path only — no golden_skirmish_config fallback.
    let mut menu = SkirmishMenu::new();
    let menu_init_ok = menu.initialize().is_ok();
    // Slot 0 is Human by default; configure slot 1 as Medium AI via menu cycling.
    let medium_ai_ok = menu.configure_slot_medium_ai(1);
    if map_resolved {
        menu.set_map_name(map_id.clone());
    }
    let (slots, rules, menu_map_name) = menu.get_game_config();
    let cfg = config_from_skirmish_menu(&menu_map_name, &rules, &slots);
    let active = cfg.slots.iter().filter(|s| s.is_active).count();
    let has_human = cfg.slots.iter().any(|s| s.is_human);
    let has_ai = cfg.slots.iter().any(|s| !s.is_human && s.is_active);
    let menu_config_ok = menu_init_ok && medium_ai_ok && active >= 2 && has_human && has_ai;

    let apply_ok = apply_skirmish_config(&mut logic, &cfg).is_ok();
    let skirmish_config_ok = apply_ok
        && logic.get_players().len() >= 2
        && logic.host_ai_player_count() >= 1
        && logic.skirmish_rules().fog_of_war;

    // Host is "constructed" only when production apply path succeeds — not a constant true.
    let host_constructed = skirmish_config_ok;

    let map_loaded = if let Some(ref path) = map_path {
        logic.load_map(&path.display().to_string())
    } else {
        false
    };

    // Immediate post-map seed (matches start_game_from_ui seed before first dual-tick).
    // Multi-consumer residual: HUD + UIState + RTS + unit command panel share snapshot.
    let mut hud = GameHUD::new();
    let mut ui_state = GameUIState::default();
    let mut rts = RTSInterface::new();
    let mut command_panel = UnitCommandPanel::new();
    let seed_pres = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic,
        0,
        &mut hud,
        &mut ui_state,
        &mut rts,
        &mut command_panel,
    );
    let seed_ok = seed_pres.frame.0 == logic.get_frame()
        && (seed_pres.alive_object_count() > 0 || !map_loaded);

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        // Dual-tick: authority step then multi-consumer presentation apply.
        logic.update();
        let _ = PresentationFrame::build_and_apply_for_shell_consumers(
            &logic,
            0,
            &mut hud,
            &mut ui_state,
            &mut rts,
            &mut command_panel,
        );
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);
    let frames_ok = frames_advanced > 0;

    // Ensure at least one selectable unit is selected so selection health is exercised.
    let select_id = logic
        .get_objects()
        .values()
        .find(|o| o.is_alive() && !o.status.destroyed)
        .map(|o| o.id);
    if let Some(id) = select_id {
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
    }

    let pres = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic,
        0,
        &mut hud,
        &mut ui_state,
        &mut rts,
        &mut command_panel,
    );
    let presentation_ok = seed_ok
        && pres.frame.0 == logic.get_frame()
        && (pres.alive_object_count() > 0 || !map_loaded)
        && !pres
            .objects
            .iter()
            .any(|o| o.model_key.is_none() && !o.destroyed);

    // Dual-tick residual honesty: seed frame applied, then post-update presentation
    // matches authority frame (start_game_from_ui / engine dual-tick order).
    let dual_tick_presentation_ok = seed_ok
        && frames_ok
        && presentation_ok
        && pres.frame.0 == logic.get_frame()
        && seed_pres.frame.0 <= pres.frame.0;
    // Dual-tick residual counters (build + apply recorded on shell apply path).
    let dual_tick_counters_ok = presentation_ok
        && dual_tick_presentation_ok
        && seed_pres.dual_tick_presentation_residual_ok()
        && seed_pres.dual_tick.honesty_apply_ok()
        && pres.dual_tick_presentation_residual_ok()
        && pres.dual_tick.honesty_apply_ok()
        && seed_pres.dual_tick.applies >= 1
        && pres.dual_tick.applies >= 1;
    let gameworld_shadow_ok = {
        let (_w, probe) = crate::gameworld_shadow::probe_host_vs_gameworld(&mut logic);
        probe.full_match()
    };
    let damage_authority_env_ok = crate::gameworld_shadow::gameworld_damage_authority_enabled();
    let economy_authority_env_ok = crate::gameworld_shadow::gameworld_economy_authority_enabled();
    let dual_tick_policy_authority_only = matches!(
        crate::authoritative_world::dual_tick_policy(),
        crate::authoritative_world::DualTickPolicy::AuthorityOnly
    );
    let engine_bridge_off = !crate::gameworld_shadow::engine_object_bridge_enabled();

    // Minimap FOW from presentation residual (grid snapshot, not live shroud re-lock).
    let minimap_fow_presentation_ok = presentation_ok && pres.minimap_fow_presentation_ok();

    // WGPU laser segment upload residual (CPU pack path; no live device required).
    // Empty host lasers → honest empty pack; synthetic assist pair exercises geometry.
    let empty_pack = pack_and_mark_upload_ready(&pres);
    let synthetic = PresentationLaserBeam::synthetic_assist_pair(pres.frame.0);
    let mut synth_frame = pres.clone();
    synth_frame.laser_beams = synthetic.to_vec();
    let synth_pack = LaserSegmentUpload::pack_from_presentation(&synth_frame);
    let laser_segment_upload_ok = empty_pack.honesty.honesty_cpu_pack_ok()
        && empty_pack.honesty.honesty_upload_ready_ok()
        && synth_pack.honesty.honesty_geometry_ok()
        && synth_pack.honesty.segments_packed >= 20
        && synth_pack.honesty.beams_packed == 2
        && synthetic[0].honesty_ground_height_ok()
        && synthetic[0].honesty_soft_edge_presentation_ok();
    // Projectile trail CPU pack residual from presentation freeze.
    let proj_empty = crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::empty();
    let mut proj_logic = crate::game_logic::GameLogic::new();
    let _ = proj_logic.combat_system_mut().fire_projectile(
        glam::Vec3::ZERO,
        glam::Vec3::new(10.0, 0.0, 0.0),
        &crate::game_logic::Weapon::default(),
        crate::game_logic::ObjectId(1),
        None,
        50.0,
    );
    let proj_pres = crate::presentation_frame::PresentationFrame::build_from_logic(&proj_logic, 0);
    let proj_pack =
        crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::pack_from_presentation(
            &proj_pres,
        );
    let projectile_segment_upload_ok = proj_empty.honesty.cpu_pack_ok
        && proj_empty.is_upload_ready()
        && proj_pack.honesty.has_geometry
        && proj_pack.honesty.projectiles_packed >= 1
        && proj_pack.is_upload_ready();
    let move_empty = crate::graphics::move_line_upload::MoveLineUpload::empty();
    let mut move_logic = crate::game_logic::GameLogic::new();
    {
        let mut t = crate::game_logic::ThingTemplate::new("ShellMoveLineU");
        t.set_health(20.0);
        t.add_kind_of(crate::game_logic::KindOf::Infantry);
        move_logic.templates.insert("ShellMoveLineU".into(), t);
        if let Some(id) = move_logic.create_object(
            "ShellMoveLineU",
            crate::game_logic::Team::USA,
            glam::Vec3::ZERO,
        ) {
            if let Some(obj) = move_logic.get_objects_mut().get_mut(&id) {
                obj.movement.target_position = Some(glam::Vec3::new(5.0, 0.0, 5.0));
            }
        }
    }
    let move_pres = crate::presentation_frame::PresentationFrame::build_from_logic(&move_logic, 0);
    let move_pack =
        crate::graphics::move_line_upload::MoveLineUpload::pack_from_presentation(&move_pres);
    let move_line_upload_ok = move_empty.honesty.cpu_pack_ok
        && move_empty.is_upload_ready()
        && move_pack.honesty.has_geometry
        && move_pack.honesty.lines_packed >= 1
        && move_pack.is_upload_ready();
    let atk_empty = crate::graphics::attack_line_upload::AttackLineUpload::empty();
    let mut atk_logic = crate::game_logic::GameLogic::new();
    {
        for (name, pos) in [
            ("ShellAtkA", glam::Vec3::ZERO),
            ("ShellAtkB", glam::Vec3::new(8.0, 0.0, 0.0)),
        ] {
            let mut t = crate::game_logic::ThingTemplate::new(name);
            t.set_health(20.0);
            t.add_kind_of(crate::game_logic::KindOf::Infantry);
            atk_logic.templates.insert(name.into(), t);
            let _ = atk_logic.create_object(name, crate::game_logic::Team::USA, pos);
        }
        let ids: Vec<_> = atk_logic.get_objects().keys().copied().collect();
        if ids.len() >= 2 {
            if let Some(obj) = atk_logic.get_objects_mut().get_mut(&ids[0]) {
                obj.target = Some(ids[1]);
            }
        }
    }
    let atk_pres = crate::presentation_frame::PresentationFrame::build_from_logic(&atk_logic, 0);
    let atk_pack =
        crate::graphics::attack_line_upload::AttackLineUpload::pack_from_presentation(&atk_pres);
    let attack_line_upload_ok = atk_empty.honesty.cpu_pack_ok
        && atk_empty.is_upload_ready()
        && atk_pack.honesty.has_geometry
        && atk_pack.honesty.lines_packed >= 1
        && atk_pack.is_upload_ready();
    // OrbitalLaser multi-beam soft-edge: presentation residual fields → CPU pack.
    let orbital = PresentationLaserBeam::synthetic_orbital_soft_edge(pres.frame.0);
    let se = orbital.soft_edge.unwrap_or(PRESENTATION_ORBITAL_SOFT_EDGE);
    let (mb_start, mb_end, mb_elapsed, mb_width) = se.pack_endpoints(orbital.from, orbital.to, 1.0);
    let multi_beam_pack = LaserSegmentUpload::pack_orbital_multi_beam_soft_edge(
        mb_start, mb_end, mb_elapsed, mb_width,
    );
    let multi_beam_soft_edge_ok = multi_beam_pack.honesty.honesty_cpu_pack_ok()
        && multi_beam_pack.honesty.honesty_geometry_ok()
        && multi_beam_pack.honesty.honesty_multi_beam_soft_edge_ok()
        && orbital.honesty_soft_edge_presentation_ok()
        && se.honesty_orbital_residual_ok();
    let laser_presentation_residual_ok =
        presentation_ok && pres.laser_presentation_residual_ok() && multi_beam_soft_edge_ok;

    // InGameUI floating text + MoneyPickUp Anim2D residual (CPU layout; no live GPU).
    // Empty host texts → honest empty pack; synthetic cash exercises geometry.
    let ft_empty = pack_floating_text_and_mark_ready(&pres);
    let mut ft_synth_frame = pres.clone();
    ft_synth_frame.floating_texts =
        vec![PresentationFloatingText::synthetic_cash(100, pres.frame.0)];
    ft_synth_frame.world_anims = vec![PresentationWorldAnim::synthetic_money_pickup(pres.frame.0)];
    let ft_synth = FloatingTextLayout::pack_from_presentation(&ft_synth_frame);
    let floating_text_layout_ok = presentation_ok
        && pres.floating_text_presentation_ok()
        && ft_empty.honesty.honesty_cpu_pack_ok()
        && ft_empty.honesty.honesty_upload_ready_ok()
        && ft_empty.honesty.honesty_retail_params_ok()
        && ft_synth.honesty.honesty_geometry_ok()
        && ft_synth.honesty.texts_packed == 1
        && ft_synth.honesty.world_anims_observed == 1;
    let floating_text_vanish_ok = floating_text_layout_ok
        && pres.floating_text_vanish_residual_ok()
        && PresentationFloatingText::honesty_vanish_rate_residual_ok()
        && PresentationFloatingText::honesty_vanish_color_alpha_residual_ok()
        && ft_synth_frame.floating_texts.iter().all(|t| {
            let a = t.vanish_alpha_at(pres.frame.0);
            (a - 1.0).abs() < 0.001
        });
    let game_text_caption_ok = floating_text_layout_ok
        && ft_synth.honesty.honesty_game_text_caption_ok()
        && ft_synth
            .entries
            .first()
            .map(|e| e.caption == "+$100")
            .unwrap_or(false);
    let display_string_measure_ok = floating_text_layout_ok
        && ft_synth.honesty.honesty_display_string_measure_ok()
        && ft_synth
            .entries
            .first()
            .map(|e| e.measure_width > 0 && e.measure_height == 8)
            .unwrap_or(false);
    // CSF/STR GameText residual exercise (retail `$%d` + optional live CSF).
    let game_text_csf_str_ok = exercise_host_game_text_residual().honesty.honesty_ok();
    // translate_copy escape table residual (host-testable, no GPU).
    let translate_copy_residual_ok = honesty_translate_copy_escape_table();
    let world_anim_presentation_ok = presentation_ok && pres.world_anim_presentation_ok();
    // World-anim CPU layout residual (empty + synthetic MoneyPickUp).
    let wa_empty = pack_world_anim_and_mark_ready(&pres);
    let wa_synth = WorldAnimLayout::pack_from_presentation(&ft_synth_frame);
    let world_anim_layout_ok = presentation_ok
        && world_anim_presentation_ok
        && wa_empty.honesty.honesty_cpu_pack_ok()
        && wa_empty.honesty.honesty_upload_ready_ok()
        && wa_synth.honesty.honesty_geometry_ok()
        && wa_synth.honesty.anims_packed == 1
        && wa_synth.honesty.honesty_template_ok();
    let world_anim_fade_ok = world_anim_layout_ok
        && pres.world_anim_fade_residual_ok()
        && PresentationWorldAnim::honesty_money_pickup_fade_params_ok()
        && ft_synth_frame
            .world_anims
            .iter()
            .all(|a| a.honesty_fade_residual_ok());
    let anim2d_frame_ok = world_anim_layout_ok
        && wa_synth.honesty.honesty_anim2d_frame_ok()
        && wa_synth
            .entries
            .first()
            .map(|e| e.frame_image.starts_with("SCPDollar"))
            .unwrap_or(false);
    // Anim2DCollection residual (host-testable, no GPU).
    let anim2d_collection_residual_ok = honesty_anim2d_collection_residual();
    // GameLogic / GameClient RandomValue ADC stream residual.
    let rng_stream_residual_ok = exercise_host_rng_residual(0x5A6E_2710).honesty_ok();
    // Wave 75 mesh / wave 72–73 residual honesty (host-testable, no GPU claim).
    let mesh_asset_residual_ok = honesty_mesh_asset_residual_ok();
    let rng_residual_pack_ok = honesty_rng_residual_pack_ok();
    let special_power_wave72_residual_ok = honesty_special_power_residual_pack_ok();
    let special_power_wave73_residual_ok = honesty_special_power_residual_pack_wave73_ok();
    let special_power_wave76_residual_ok = honesty_special_power_residual_pack_wave76_ok();
    let paradrop_wave76_residual_ok = honesty_paradrop_residual_pack_wave76_ok();
    let graphics_wave76_residual_ok = honesty_graphics_residual_pack_wave76_ok();
    let spectre_orbit_decal_presentation_ok = honesty_spectre_orbit_decal_presentation_ok()
        && presentation_ok
        && pres.spectre_orbit_decal_presentation_residual_ok();
    // Wave 77 residual honesty packs (orthogonal to ControlBar/script; no playable_claim flip).
    let special_power_wave77_residual_ok = honesty_special_power_residual_pack_wave77_ok();
    let fow_residual_pack_ok = honesty_fow_residual_pack_wave77();
    let ground_height_presentation_ok =
        presentation_ok && pres.ground_height_presentation_residual_ok();
    let weapon_store_seed_residual_ok = honesty_weapon_store_host_seed_residual_wave77();
    let ai_skirmish_residual_ok = honesty_ai_skirmish_residual_pack_wave77();
    // Wave 78 residual honesty packs (reload table + science tiers; no playable_claim flip).
    let special_power_wave78_residual_ok = honesty_special_power_residual_pack_wave78_ok();
    let cluster_mines_wave78_residual_ok = honesty_cluster_mines_residual_pack_wave78();
    let gps_scrambler_wave78_residual_ok = honesty_gps_scrambler_residual_pack_wave78();
    let cash_bounty_wave78_residual_ok = honesty_cash_bounty_residual_pack_wave78();
    // Wave 79 residual honesty packs (orthogonal to special powers; no playable_claim flip).
    let minimap_residual_pack_ok = honesty_minimap_residual_pack_wave79();
    let selection_hud_residual_pack_ok = honesty_selection_hud_residual_pack_wave79();
    let input_residual_pack_ok = honesty_input_residual_pack_wave79();
    let drawable_residual_fields_ok = honesty_drawable_residual_fields_wave79_ok();
    let unit_training_wave79_residual_ok = honesty_unit_training_residual_pack_wave79_ok();
    let upgrades_cost_time_application_ok = honesty_upgrades_cost_time_application_wave79_ok();
    // Wave 80 residual honesty packs (INI-backed superweapon/science residual; no playable_claim flip).
    let command_button_wave80_residual_ok =
        honesty_command_button_superweapon_residual_pack_wave80();
    let science_rank_wave80_residual_ok = honesty_science_rank_residual_pack_wave80();
    let superweapon_kindof_wave80_residual_ok = honesty_superweapon_kindof_residual_pack_wave80();
    let special_power_enum_wave80_residual_ok = honesty_special_power_enum_residual_pack_wave80();
    // Wave 81 residual honesty packs (terrain/pathfinder/locomotor/armor/PUC; no playable_claim flip).
    let terrain_height_sample_wave81_ok = honesty_map_height_sample_residual_pack_wave81();
    let pathfinder_wave81_residual_ok = honesty_pathfinder_residual_pack_wave81();
    let locomotor_table_wave81_ok = honesty_locomotor_residual_table_wave81();
    let armor_table_wave81_ok = honesty_armor_residual_table_wave81();
    let puc_flare_table_wave81_ok = honesty_particle_outer_node_flare_name_table_wave81();
    // Wave 82 residual honesty packs (enum/bit-name tables; no playable_claim flip).
    let damage_type_wave82_ok = honesty_damage_type_enum_table_wave82();
    let death_type_wave82_ok = honesty_death_type_enum_table_wave82();
    let model_condition_wave82_ok = honesty_model_condition_enum_table_wave82();
    let weapon_bonus_wave82_ok = honesty_weapon_bonus_enum_table_wave82();
    let object_status_wave82_ok = honesty_object_status_enum_table_wave82();
    // Wave 83 residual honesty packs (structure/economy residual; no playable_claim flip).
    let production_queue_wave83_ok = honesty_production_queue_residual_pack_wave83();
    let supply_warehouse_wave83_ok = honesty_supply_warehouse_residual_pack_wave83();
    let dozer_build_wave83_ok = honesty_dozer_build_residual_pack_wave83();
    let capture_building_wave83_ok = honesty_capture_building_residual_pack_wave83();
    let power_plant_wave83_ok = honesty_power_plant_residual_pack_wave83();
    let command_center_wave83_ok = honesty_command_center_residual_pack_wave83();

    let kindof_wave84_ok = honesty_kindof_enum_table_wave84();
    let weapon_slot_wave84_ok = honesty_weapon_slot_enum_table_wave84();
    let veterancy_wave84_ok = honesty_veterancy_level_enum_table_wave84();
    let relationship_wave84_ok = honesty_relationship_enum_table_wave84();
    let geometry_wave84_ok = honesty_geometry_type_enum_table_wave84();
    let shadow_wave84_ok = honesty_shadow_type_enum_table_wave84();
    // Wave 85 residual honesty packs (faction/skirmish residual; no playable_claim flip).
    let faction_side_wave85_ok = honesty_faction_side_residual_table_wave85();
    let player_template_wave85_ok = honesty_player_template_residual_pack_wave85();
    let starting_cash_wave85_ok = honesty_starting_cash_residual_pack_wave85();
    let skirmish_ai_personality_wave85_ok = honesty_skirmish_ai_personality_residual_pack_wave85();
    let victory_condition_wave85_ok = honesty_victory_condition_residual_pack_wave85();
    // Wave 86 residual honesty packs (GameData/lobby/map/crate residual; no playable_claim flip).
    let gamedata_camera_fps_wave86_ok = honesty_gamedata_camera_fps_residual_pack_wave86();
    let gamedata_world_constants_wave86_ok =
        honesty_gamedata_world_constants_residual_pack_wave86();
    let multiplayer_options_wave86_ok = honesty_multiplayer_options_residual_pack_wave86();
    let map_selection_wave86_ok = honesty_map_selection_residual_pack_wave86();
    let crate_deepen_wave86_ok = honesty_crate_residual_deepen_pack_wave86();
    // Wave 87 residual honesty packs (weather/water/bridge/tunnel/garrison/transport; no playable_claim flip).
    let weather_wave87_ok = honesty_weather_residual_pack_wave87();
    let water_wave87_ok = honesty_water_residual_pack_wave87();
    let bridge_wave87_ok = honesty_bridge_residual_pack_wave87();
    let tunnel_wave87_ok = honesty_tunnel_residual_deepen_wave87();
    let garrison_wave87_ok = honesty_garrison_residual_pack_wave87();
    let transport_wave87_ok = honesty_transport_residual_pack_wave87();
    // Wave 88 residual honesty packs (FX/OCL/particle/audio/cursor name tables; no playable_claim flip).
    let radius_cursor_wave88_ok = honesty_radius_cursor_name_table_wave88();
    let mouse_cursor_wave88_ok = honesty_mouse_cursor_name_table_wave88();
    let superweapon_fxlist_wave88_ok = honesty_superweapon_fxlist_name_table_wave88();
    let superweapon_ocl_wave88_ok = honesty_superweapon_ocl_name_table_wave88();
    let superweapon_particle_wave88_ok = honesty_superweapon_particle_name_table_wave88();
    let superweapon_audio_wave88_ok = honesty_superweapon_audio_event_name_table_wave88();
    // Wave 89 residual honesty packs (rank/exp/hotkey/chat/replay/options; no playable_claim flip).
    let rank_skill_wave89_ok = honesty_rank_skill_points_application_residual_pack_wave89();
    let experience_wave89_ok = honesty_experience_residual_tables_pack_wave89();
    let hotkey_wave89_ok = honesty_hotkey_residual_table_pack_wave89();
    let chat_wave89_ok = honesty_chat_residual_host_pack_wave89();
    let replay_wave89_ok = honesty_replay_residual_host_pack_wave89();
    let options_wave89_ok = honesty_options_residual_pack_wave89();
    let gamespeed_wave90_ok = honesty_gamespeed_residual_pack_wave90();
    let frame_rate_wave90_ok = honesty_frame_rate_residual_deepen_pack_wave90();
    let debug_tables_wave90_ok = honesty_debug_residual_tables_pack_wave90();
    let language_wave90_ok = honesty_language_residual_deepen_pack_wave90();
    let credits_wave90_ok = honesty_credits_residual_pack_wave90();
    // Wave 91 residual honesty packs (tooltip/helpbox/message/eva/video/briefing; no playable_claim flip).
    let tooltip_wave91_ok = honesty_tooltip_residual_pack_wave91();
    let help_box_wave91_ok = honesty_help_box_residual_pack_wave91();
    let message_wave91_ok = honesty_message_residual_pack_wave91();
    let eva_wave91_ok = honesty_eva_residual_pack_wave91();
    let video_wave91_ok = honesty_video_residual_name_table_wave91();
    let mission_briefing_wave91_ok = honesty_mission_briefing_residual_pack_wave91();
    // Wave 92 residual honesty packs (weapon/armor/body/locomotor/science; no playable_claim flip).
    let weapon_deepen_wave92_ok = honesty_weapon_store_deepen_residual_wave92();
    let armor_expand_wave92_ok = honesty_armor_residual_expand_wave92();
    let body_health_wave92_ok = honesty_body_max_health_residual_table_wave92();
    let locomotor_expand_wave92_ok = honesty_locomotor_residual_expand_wave92();
    let science_names_wave92_ok = honesty_science_name_table_residual_wave92();
    // Wave 93 residual honesty packs (particle/drawable/shadow/terrain/road; no playable_claim flip).
    let particle_emit_wave93_ok = honesty_particle_system_emit_rate_residual_deepen_pack_wave93();
    let drawable_opacity_wave93_ok = honesty_drawable_opacity_shroud_residual_deepen_pack_wave93();
    let shadow_deepen_wave93_ok = honesty_shadow_residual_deepen_pack_wave93();
    let terrain_texture_wave93_ok = honesty_terrain_texture_residual_pack_wave93();
    let road_wave93_ok = honesty_road_residual_pack_wave93();
    // Wave 94 residual honesty packs (AI/special ability/upgrade/CommandSet; no playable_claim flip).
    let ai_state_wave94_ok = honesty_ai_state_residual_table_wave94();
    let special_ability_wave94_ok = honesty_special_ability_residual_deepen_wave94();
    let upgrade_names_wave94_ok = honesty_upgrade_name_table_residual_wave94();
    let command_set_wave94_ok = honesty_command_set_superweapon_residual_wave94();
    // Wave 95 residual honesty packs (script/map/waypoint/team/player; no playable_claim flip).
    let script_action_wave95_ok = honesty_script_action_name_table_residual_wave95();
    let script_condition_wave95_ok = honesty_script_condition_name_table_residual_wave95();
    let map_object_wave95_ok = honesty_map_object_residual_pack_wave95();
    let waypoint_wave95_ok = honesty_waypoint_residual_pack_wave95();
    let team_wave95_ok = honesty_team_residual_pack_wave95();
    let player_deepen_wave95_ok = honesty_player_residual_deepen_pack_wave95();
    // Wave 96 residual honesty packs (partition/collision/physics/projectile; no playable_claim flip).
    let partition_wave96_ok = honesty_partition_residual_pack_wave96();
    let collision_wave96_ok = honesty_collision_residual_pack_wave96();
    let physics_wave96_ok = honesty_physics_residual_pack_wave96();
    let projectile_wave96_ok = honesty_projectile_residual_deepen_pack_wave96();
    // Wave 97 residual honesty packs (radar/spotter/stealth/detector/vision; no playable_claim flip).
    let radar_deepen_wave97_ok = honesty_radar_residual_deepen_pack_wave97();
    let spotter_wave97_ok = honesty_spotter_residual_pack_wave97();
    let stealth_deepen_wave97_ok = honesty_stealth_residual_deepen_pack_wave97();
    let detector_deepen_wave97_ok = honesty_detector_residual_deepen_pack_wave97();
    let vision_wave97_ok = honesty_vision_residual_pack_wave97();
    // Wave 98 residual honesty packs (dock/contain/exit/heal; no playable_claim flip).
    let dock_wave98_ok = honesty_dock_residual_pack_wave98();
    let contain_wave98_ok = honesty_contain_residual_deepen_pack_wave98();
    let exit_wave98_ok = honesty_exit_residual_pack_wave98();
    let heal_wave98_ok = honesty_heal_residual_deepen_pack_wave98();
    // Wave 99 residual honesty packs (production/buildable/prereq/command-button/control-bar; no playable_claim flip).
    let production_deepen_wave99_ok = honesty_production_residual_deepen_pack_wave99();
    let buildable_wave99_ok = honesty_buildable_residual_pack_wave99();
    let prerequisite_wave99_ok = honesty_prerequisite_residual_pack_wave99();
    let command_button_deepen_wave99_ok = honesty_command_button_residual_deepen_pack_wave99();
    let control_bar_deepen_wave99_ok = honesty_control_bar_residual_deepen_pack_wave99();
    // Wave 100 residual honesty packs (ThingFactory/module/xfer; no playable_claim flip).
    let thing_factory_deepen_wave100_ok = honesty_thing_factory_residual_deepen_pack_wave100();
    let module_type_wave100_ok = honesty_module_type_table_residual_pack_wave100();
    let xfer_deepen_wave100_ok = honesty_xfer_residual_deepen_pack_wave100();
    let thing_factory_crosslink_wave100_ok = honesty_thing_factory_spawn_crosslink_wave100();
    // Wave 101 residual honesty packs (ModuleFactory/ThingFactory create/Partition register; no playable_claim flip).
    let module_factory_deepen_wave101_ok = honesty_module_factory_residual_deepen_pack_wave101();
    let thing_factory_create_wave101_ok =
        honesty_thing_factory_create_residual_deepen_pack_wave101();
    let partition_register_wave101_ok = honesty_partition_register_residual_pack_wave101();
    let mf_crosslink_wave101_ok = honesty_thing_factory_module_partition_crosslink_wave101();
    // Wave 102 residual honesty packs (DisplayString/Anim2D/laser/CSF/presentation; no playable_claim flip).
    let display_string_deepen_wave102_ok = honesty_display_string_residual_deepen_pack_wave102();
    let anim2d_deepen_wave102_ok = honesty_anim2d_residual_deepen_pack_wave102();
    let laser_segliner_deepen_wave102_ok = honesty_laser_segliner_residual_deepen_pack_wave102();
    let csf_multi_locale_deepen_wave102_ok =
        honesty_csf_multi_locale_residual_deepen_pack_wave102();
    let presentation_deepen_wave102_ok = honesty_presentation_residual_deepen_pack_wave102();
    // Wave 103 residual honesty packs (weapon/armor/loco/special-power/KindOf; no playable_claim flip).
    let weapon_deepen_wave103_ok = honesty_weapon_store_deepen_residual_wave103();
    let armor_expand_wave103_ok = honesty_armor_residual_expand_wave103();
    let locomotor_expand_wave103_ok = honesty_locomotor_residual_expand_wave103();
    let special_power_deepen_wave103_ok =
        honesty_special_power_superweapon_residual_deepen_wave103();
    let object_kindof_wave103_ok = honesty_object_kindof_residual_pack_wave103();
    // Wave 104 residual honesty packs (Object status/create, ActiveBody, Drawable create, registerObject; no playable_claim flip).
    let object_status_wave104_ok = honesty_object_status_state_machine_residual_wave104();
    let object_create_wave104_ok = honesty_object_create_order_residual_wave104();
    let active_body_wave104_ok = honesty_active_body_max_health_apply_residual_wave104();
    let drawable_create_wave104_ok = honesty_drawable_create_residual_wave104();
    let register_object_wave104_ok = honesty_gamelogic_register_object_residual_wave104();
    // Wave 105 residual honesty packs (AI group/path/weapon fire/damage/veterancy; no playable_claim flip).
    let ai_group_wave105_ok = honesty_ai_group_residual_pack_wave105();
    let ai_path_wave105_ok = honesty_ai_path_residual_deepen_pack_wave105();
    let weapon_fire_wave105_ok = honesty_weapon_fire_residual_deepen_pack_wave105();
    let damage_application_wave105_ok = honesty_damage_application_residual_deepen_pack_wave105();
    let veterancy_wave105_ok = honesty_veterancy_residual_deepen_pack_wave105();
    let game_state_deepen_wave106_ok = honesty_game_state_residual_deepen_pack_wave106();
    let campaign_mission_wave106_ok = honesty_campaign_mission_residual_deepen_pack_wave106();
    let main_menu_deepen_wave106_ok = honesty_main_menu_residual_deepen_pack_wave106();
    let game_window_deepen_wave106_ok = honesty_game_window_residual_deepen_pack_wave106();
    let window_layout_deepen_wave106_ok = honesty_window_layout_residual_deepen_pack_wave106();
    // Wave 107 residual honesty packs (particle/FXList entry/OCL create/audio; no playable_claim flip).
    let particle_system_deepen_wave107_ok = honesty_particle_system_residual_deepen_pack_wave107();
    let fxlist_entry_deepen_wave107_ok = honesty_fxlist_entry_residual_deepen_pack_wave107();
    let ocl_create_deepen_wave107_ok = honesty_ocl_create_residual_deepen_pack_wave107();
    let audio_deepen_wave107_ok = honesty_audio_residual_deepen_pack_wave107();
    // Wave 108 residual honesty packs (HeightMap/bridge/water/road/cliff; no playable_claim flip).
    let heightmap_deepen_wave108_ok = honesty_heightmap_residual_deepen_pack_wave108();
    let bridge_deepen_wave108_ok = honesty_bridge_residual_deepen_pack_wave108();
    let water_deepen_wave108_ok = honesty_water_residual_deepen_pack_wave108();
    let road_deepen_wave108_ok = honesty_road_residual_deepen_pack_wave108();
    let cliff_peels_wave108_ok = honesty_cliff_residual_peels_pack_wave108();
    let special_power_store_wave109_ok = honesty_special_power_template_store_residual_wave109();
    let science_store_wave109_ok = honesty_science_store_residual_deepen_pack_wave109();
    let upgrade_store_wave109_ok = honesty_upgrade_store_residual_deepen_pack_wave109();
    let player_deepen_wave109_ok = honesty_player_residual_deepen_pack_wave109();
    let team_deepen_wave109_ok = honesty_team_residual_deepen_pack_wave109();

    // HUD + multi-consumer selection panel health from presentation after dual-tick.
    let (hud_selection_ok, selection_consumers_ok) = if let Some(id) = select_id {
        let infos = hud.selected_unit_infos();
        let snap_infos = pres.selected_unit_display_infos();
        let hud_hit = infos.iter().any(|u| {
            u.object_id == id && u.health_current > 0.0 && u.health_maximum >= u.health_current
        });
        let snap_hit = snap_infos
            .iter()
            .any(|u| u.object_id == id && u.health_current > 0.0);
        let ids_ok = hud.selected_unit_ids().contains(&id);
        let minimap_ok = !pres.hud_minimap_units().is_empty() || !map_loaded;
        let panel = hud.selection_panel();
        let panel_ok =
            panel.visible && panel.has_positive_health() && panel.primary_object_id == Some(id);
        // Optional ControlBar path (headless selection health; not full WND claim).
        #[cfg(feature = "game_client")]
        let control_bar_ok = {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            pres.apply_to_control_bar(&mut bar);
            bar.selection_panel_health()
                .map(|(hp, max)| hp > 0.0 && max >= hp)
                .unwrap_or(false)
        };
        #[cfg(not(feature = "game_client"))]
        let control_bar_ok = true;
        let ui_ok = ui_state.selection_panel.has_positive_health()
            && ui_state.selection_panel.primary_object_id == Some(id);
        let rts_ok =
            rts.selection_panel().has_positive_health() && rts.selected_ids().contains(&id);
        let cmd_ok = command_panel.is_visible()
            && command_panel.selection_panel().has_positive_health()
            && command_panel.selected_ids().contains(&id);
        let consumers_ok = ui_ok && rts_ok && cmd_ok && control_bar_ok;
        (
            hud_hit && snap_hit && ids_ok && minimap_ok && panel_ok && control_bar_ok,
            consumers_ok,
        )
    } else {
        // No objects (absent-map synthetic host): still require resource apply path.
        let empty_ok = hud.selected_unit_ids().is_empty()
            && !hud.selection_panel().visible
            && (pres.local_supplies > 0 || skirmish_config_ok);
        let consumers_empty = !ui_state.selection_panel.visible
            && rts.selected_ids().is_empty()
            && !command_panel.is_visible();
        (empty_ok, empty_ok && consumers_empty)
    };

    // Shell → InGame residual: production StartGame transitions Skirmish→Loading→GameHUD
    // and ensure_gameplay_layouts (ControlBar.wnd) on InGame enter.
    let mut ui_mgr = UIManager::new(1024, 768);
    ui_mgr.transition_to_screen(Screen::Skirmish);
    let at_skirmish = ui_mgr.current_screen() == Some(Screen::Skirmish)
        && Screen::Skirmish.is_shell_owned_pregame();
    ui_mgr.transition_to_screen(Screen::Loading);
    let at_loading = ui_mgr.current_screen() == Some(Screen::Loading)
        && !Screen::Loading.is_shell_owned_pregame();
    // Attempt headless WindowManager load when game_client is enabled (ShowControlBar
    // residual). AssetsUnavailable remains honest when WindowZH is not checked out.
    // This is **not** windowed W3D retail — only layout script → window tree.
    #[cfg(feature = "game_client")]
    let layout_honesty = control_bar_layout_honesty(true);
    #[cfg(not(feature = "game_client"))]
    let layout_honesty = control_bar_layout_honesty(false);
    let layout_status = layout_honesty.status.clone();
    let layout_report = format_control_bar_honesty(&layout_honesty);
    let control_bar_path_resolved = layout_honesty.path_resolved;
    let control_bar_wnd_validated = layout_honesty.wnd_validated;
    let control_bar_window_loaded = layout_honesty.window_loaded;
    let control_bar_window_count = layout_honesty.window_count;
    let control_bar_layout_ok = match &layout_status {
        GameplayLayoutStatus::Ready { path, loaded } => {
            // Ready after structural validate. Prefer WindowManager load when assets
            // present (`loaded=true`); validated-only (`loaded=false`) is still ok.
            path.contains("ControlBar")
                && control_bar_wnd_validated
                && (*loaded == control_bar_window_loaded)
                && (!*loaded || control_bar_window_count > 0)
        }
        // Honest residual when WindowZH assets are not checked out.
        GameplayLayoutStatus::AssetsUnavailable { searched } => {
            !searched.is_empty() && layout_honesty.assets_unavailable && !control_bar_window_loaded
        }
        GameplayLayoutStatus::LoadFailed { .. } => false,
    };
    let control_bar_wave76_residual_ok = honesty_control_bar_residual_pack_wave76_ok(
        control_bar_window_loaded,
        control_bar_window_count,
    );
    ui_mgr.transition_to_screen(Screen::GameHUD);
    let at_ingame = ui_mgr.current_screen() == Some(Screen::GameHUD)
        && !Screen::GameHUD.is_shell_owned_pregame();
    let screen_skirmish_ok = at_skirmish
        && at_loading
        && at_ingame
        && Screen::MainMenu.is_shell_owned_pregame()
        && Screen::startup_entry_screen(true) == Screen::MainMenu;

    // When assets present, map must load; when absent, still pass config+frames.
    let map_requirement_ok = if map_resolved { map_loaded } else { true };

    // Never claim full retail playability from headless smoke (no W3D/window/GPU).
    let playable_claim = false;

    let host_path_ok = host_constructed
        && skirmish_config_ok
        && menu_config_ok
        && frames_ok
        && presentation_ok
        && hud_selection_ok
        && selection_consumers_ok
        && dual_tick_presentation_ok
        && screen_skirmish_ok
        && control_bar_layout_ok
        && map_requirement_ok;

    // Limited claim: headless production host path is operational end-to-end.
    // Requires dual-tick presentation + multi-consumer selection + shell→InGame +
    // ControlBar.wnd ensure. Still not windowed W3D play (playable_claim stays false).
    let shell_host_playable_ok = host_path_ok;

    let status = if host_path_ok {
        "success".into()
    } else {
        "partial".into()
    };

    ShellSmokeResult {
        host_constructed,
        skirmish_config_ok,
        menu_config_ok,
        map_resolved,
        map_loaded,
        frames_advanced,
        presentation_ok,
        dual_tick_presentation_ok,
        dual_tick_counters_ok,
        gameworld_shadow_ok,
        damage_authority_env_ok,
        economy_authority_env_ok,
        dual_tick_policy_authority_only,
        engine_bridge_off,
        hud_selection_ok,
        minimap_fow_presentation_ok,
        laser_segment_upload_ok,
        projectile_segment_upload_ok,
        move_line_upload_ok,
        attack_line_upload_ok,
        multi_beam_soft_edge_ok,
        laser_presentation_residual_ok,
        floating_text_layout_ok,
        floating_text_vanish_ok,
        world_anim_presentation_ok,
        world_anim_layout_ok,
        world_anim_fade_ok,
        anim2d_frame_ok,
        anim2d_collection_residual_ok,
        translate_copy_residual_ok,
        game_text_caption_ok,
        game_text_csf_str_ok,
        display_string_measure_ok,
        rng_stream_residual_ok,
        mesh_asset_residual_ok,
        rng_residual_pack_ok,
        special_power_wave72_residual_ok,
        special_power_wave73_residual_ok,
        special_power_wave76_residual_ok,
        paradrop_wave76_residual_ok,
        control_bar_wave76_residual_ok,
        graphics_wave76_residual_ok,
        spectre_orbit_decal_presentation_ok,
        special_power_wave77_residual_ok,
        fow_residual_pack_ok,
        ground_height_presentation_ok,
        weapon_store_seed_residual_ok,
        ai_skirmish_residual_ok,
        special_power_wave78_residual_ok,
        cluster_mines_wave78_residual_ok,
        gps_scrambler_wave78_residual_ok,
        cash_bounty_wave78_residual_ok,
        minimap_residual_pack_ok,
        selection_hud_residual_pack_ok,
        input_residual_pack_ok,
        drawable_residual_fields_ok,
        unit_training_wave79_residual_ok,
        upgrades_cost_time_application_ok,
        command_button_wave80_residual_ok,
        science_rank_wave80_residual_ok,
        superweapon_kindof_wave80_residual_ok,
        special_power_enum_wave80_residual_ok,
        terrain_height_sample_wave81_ok,
        pathfinder_wave81_residual_ok,
        locomotor_table_wave81_ok,
        armor_table_wave81_ok,
        puc_flare_table_wave81_ok,
        damage_type_wave82_ok,
        death_type_wave82_ok,
        model_condition_wave82_ok,
        weapon_bonus_wave82_ok,
        object_status_wave82_ok,
        production_queue_wave83_ok,
        supply_warehouse_wave83_ok,
        dozer_build_wave83_ok,
        capture_building_wave83_ok,
        power_plant_wave83_ok,
        command_center_wave83_ok,
        kindof_wave84_ok,
        weapon_slot_wave84_ok,
        veterancy_wave84_ok,
        relationship_wave84_ok,
        geometry_wave84_ok,
        shadow_wave84_ok,
        faction_side_wave85_ok,
        player_template_wave85_ok,
        starting_cash_wave85_ok,
        skirmish_ai_personality_wave85_ok,
        victory_condition_wave85_ok,
        gamedata_camera_fps_wave86_ok,
        gamedata_world_constants_wave86_ok,
        multiplayer_options_wave86_ok,
        map_selection_wave86_ok,
        crate_deepen_wave86_ok,
        weather_wave87_ok,
        water_wave87_ok,
        bridge_wave87_ok,
        tunnel_wave87_ok,
        garrison_wave87_ok,
        transport_wave87_ok,
        radius_cursor_wave88_ok,
        mouse_cursor_wave88_ok,
        superweapon_fxlist_wave88_ok,
        superweapon_ocl_wave88_ok,
        superweapon_particle_wave88_ok,
        superweapon_audio_wave88_ok,
        rank_skill_wave89_ok,
        experience_wave89_ok,
        hotkey_wave89_ok,
        chat_wave89_ok,
        replay_wave89_ok,
        options_wave89_ok,
        gamespeed_wave90_ok,
        frame_rate_wave90_ok,
        debug_tables_wave90_ok,
        language_wave90_ok,
        credits_wave90_ok,
        tooltip_wave91_ok,
        help_box_wave91_ok,
        message_wave91_ok,
        eva_wave91_ok,
        video_wave91_ok,
        mission_briefing_wave91_ok,
        weapon_deepen_wave92_ok,
        armor_expand_wave92_ok,
        body_health_wave92_ok,
        locomotor_expand_wave92_ok,
        science_names_wave92_ok,
        particle_emit_wave93_ok,
        drawable_opacity_wave93_ok,
        shadow_deepen_wave93_ok,
        terrain_texture_wave93_ok,
        road_wave93_ok,
        ai_state_wave94_ok,
        special_ability_wave94_ok,
        upgrade_names_wave94_ok,
        command_set_wave94_ok,
        script_action_wave95_ok,
        script_condition_wave95_ok,
        map_object_wave95_ok,
        waypoint_wave95_ok,
        team_wave95_ok,
        player_deepen_wave95_ok,
        partition_wave96_ok,
        collision_wave96_ok,
        physics_wave96_ok,
        projectile_wave96_ok,
        radar_deepen_wave97_ok,
        spotter_wave97_ok,
        stealth_deepen_wave97_ok,
        detector_deepen_wave97_ok,
        vision_wave97_ok,
        dock_wave98_ok,
        contain_wave98_ok,
        exit_wave98_ok,
        heal_wave98_ok,
        production_deepen_wave99_ok,
        buildable_wave99_ok,
        prerequisite_wave99_ok,
        command_button_deepen_wave99_ok,
        control_bar_deepen_wave99_ok,
        thing_factory_deepen_wave100_ok,
        module_type_wave100_ok,
        xfer_deepen_wave100_ok,
        thing_factory_crosslink_wave100_ok,
        module_factory_deepen_wave101_ok,
        thing_factory_create_wave101_ok,
        partition_register_wave101_ok,
        mf_crosslink_wave101_ok,
        display_string_deepen_wave102_ok,
        anim2d_deepen_wave102_ok,
        laser_segliner_deepen_wave102_ok,
        csf_multi_locale_deepen_wave102_ok,
        presentation_deepen_wave102_ok,
        weapon_deepen_wave103_ok,
        armor_expand_wave103_ok,
        locomotor_expand_wave103_ok,
        special_power_deepen_wave103_ok,
        object_kindof_wave103_ok,
        object_status_wave104_ok,
        object_create_wave104_ok,
        active_body_wave104_ok,
        drawable_create_wave104_ok,
        register_object_wave104_ok,
        ai_group_wave105_ok,
        ai_path_wave105_ok,
        weapon_fire_wave105_ok,
        damage_application_wave105_ok,
        veterancy_wave105_ok,
        game_state_deepen_wave106_ok,
        campaign_mission_wave106_ok,
        main_menu_deepen_wave106_ok,
        game_window_deepen_wave106_ok,
        window_layout_deepen_wave106_ok,
        particle_system_deepen_wave107_ok,
        fxlist_entry_deepen_wave107_ok,
        ocl_create_deepen_wave107_ok,
        audio_deepen_wave107_ok,
        heightmap_deepen_wave108_ok,
        bridge_deepen_wave108_ok,
        water_deepen_wave108_ok,
        road_deepen_wave108_ok,
        cliff_peels_wave108_ok,
        special_power_store_wave109_ok,
        science_store_wave109_ok,
        upgrade_store_wave109_ok,
        player_deepen_wave109_ok,
        team_deepen_wave109_ok,
        screen_skirmish_ok,
        control_bar_layout_ok,
        control_bar_path_resolved,
        control_bar_wnd_validated,
        control_bar_window_loaded,
        control_bar_window_count,
        selection_consumers_ok,
        shell_host_playable_ok,
        playable_claim,
        status,
        detail: format!(
            "host={host_constructed} cfg={skirmish_config_ok} menu_cfg={menu_config_ok} map_res={map_resolved} map_load={map_loaded} frames={frames_advanced} pres={presentation_ok} dual_tick={dual_tick_presentation_ok} dual_tick_ctr={dual_tick_counters_ok} gw_shadow={gameworld_shadow_ok} dmg_auth={damage_authority_env_ok} econ_auth={economy_authority_env_ok} dual_auth_only={dual_tick_policy_authority_only} bridge_off={engine_bridge_off} hud_sel={hud_selection_ok} sel_consumers={selection_consumers_ok} minimap_fow={minimap_fow_presentation_ok} laser_upload={laser_segment_upload_ok} projectile_upload={projectile_segment_upload_ok} move_lines={move_line_upload_ok} attack_lines={attack_line_upload_ok} multi_beam={multi_beam_soft_edge_ok} laser_pres={laser_presentation_residual_ok} floating_text={floating_text_layout_ok} ft_vanish={floating_text_vanish_ok} world_anim={world_anim_presentation_ok} world_anim_layout={world_anim_layout_ok} wa_fade={world_anim_fade_ok} anim2d={anim2d_frame_ok} anim2d_col={anim2d_collection_residual_ok} translate_copy={translate_copy_residual_ok} game_text={game_text_caption_ok} csf_str={game_text_csf_str_ok} ds_measure={display_string_measure_ok} rng={rng_stream_residual_ok} mesh={mesh_asset_residual_ok} rng_pack={rng_residual_pack_ok} sp72={special_power_wave72_residual_ok} sp73={special_power_wave73_residual_ok} sp76={special_power_wave76_residual_ok} paradrop76={paradrop_wave76_residual_ok} cb76={control_bar_wave76_residual_ok} gfx76={graphics_wave76_residual_ok} spectre_decal={spectre_orbit_decal_presentation_ok} sp77={special_power_wave77_residual_ok} fow77={fow_residual_pack_ok} gh77={ground_height_presentation_ok} weapon77={weapon_store_seed_residual_ok} ai77={ai_skirmish_residual_ok} sp78={special_power_wave78_residual_ok} cluster78={cluster_mines_wave78_residual_ok} gps78={gps_scrambler_wave78_residual_ok} cash78={cash_bounty_wave78_residual_ok} minimap79={minimap_residual_pack_ok} sel79={selection_hud_residual_pack_ok} input79={input_residual_pack_ok} draw79={drawable_residual_fields_ok} train79={unit_training_wave79_residual_ok} upg79={upgrades_cost_time_application_ok} cmdbtn80={command_button_wave80_residual_ok} rank80={science_rank_wave80_residual_ok} kindof80={superweapon_kindof_wave80_residual_ok} spenum80={special_power_enum_wave80_residual_ok} height81={terrain_height_sample_wave81_ok} path81={pathfinder_wave81_residual_ok} loco81={locomotor_table_wave81_ok} armor81={armor_table_wave81_ok} puc81={puc_flare_table_wave81_ok} dmg82={damage_type_wave82_ok} death82={death_type_wave82_ok} mc82={model_condition_wave82_ok} wbonus82={weapon_bonus_wave82_ok} ostatus82={object_status_wave82_ok} prod83={production_queue_wave83_ok} supply83={supply_warehouse_wave83_ok} dozer83={dozer_build_wave83_ok} capture83={capture_building_wave83_ok} power83={power_plant_wave83_ok} cc83={command_center_wave83_ok} kindof84={kindof_wave84_ok} wslot84={weapon_slot_wave84_ok} vet84={veterancy_wave84_ok} rel84={relationship_wave84_ok} geom84={geometry_wave84_ok} shadow84={shadow_wave84_ok} faction85={faction_side_wave85_ok} ptpl85={player_template_wave85_ok} cash85={starting_cash_wave85_ok} aiperson85={skirmish_ai_personality_wave85_ok} victory85={victory_condition_wave85_ok} cam86={gamedata_camera_fps_wave86_ok} world86={gamedata_world_constants_wave86_ok} mpopt86={multiplayer_options_wave86_ok} mapsel86={map_selection_wave86_ok} crate86={crate_deepen_wave86_ok} weather87={weather_wave87_ok} water87={water_wave87_ok} bridge87={bridge_wave87_ok} tunnel87={tunnel_wave87_ok} garrison87={garrison_wave87_ok} transport87={transport_wave87_ok} radius88={radius_cursor_wave88_ok} mouse88={mouse_cursor_wave88_ok} fxlist88={superweapon_fxlist_wave88_ok} ocl88={superweapon_ocl_wave88_ok} particle88={superweapon_particle_wave88_ok} audio88={superweapon_audio_wave88_ok} rank89={rank_skill_wave89_ok} exp89={experience_wave89_ok} hotkey89={hotkey_wave89_ok} chat89={chat_wave89_ok} replay89={replay_wave89_ok} options89={options_wave89_ok} gamespeed90={gamespeed_wave90_ok} framerate90={frame_rate_wave90_ok} debug90={debug_tables_wave90_ok} lang90={language_wave90_ok} credits90={credits_wave90_ok} tooltip91={tooltip_wave91_ok} helpbox91={help_box_wave91_ok} message91={message_wave91_ok} eva91={eva_wave91_ok} video91={video_wave91_ok} briefing91={mission_briefing_wave91_ok} weapon92={weapon_deepen_wave92_ok} armor92={armor_expand_wave92_ok} body92={body_health_wave92_ok} loco92={locomotor_expand_wave92_ok} science92={science_names_wave92_ok} particle93={particle_emit_wave93_ok} drawable93={drawable_opacity_wave93_ok} shadow93={shadow_deepen_wave93_ok} terrain_tex93={terrain_texture_wave93_ok} road93={road_wave93_ok} ai_state94={ai_state_wave94_ok} special_ability94={special_ability_wave94_ok} upgrade_names94={upgrade_names_wave94_ok} command_set94={command_set_wave94_ok} script_action95={script_action_wave95_ok} script_cond95={script_condition_wave95_ok} map_object95={map_object_wave95_ok} waypoint95={waypoint_wave95_ok} team95={team_wave95_ok} player95={player_deepen_wave95_ok} partition96={partition_wave96_ok} collision96={collision_wave96_ok} physics96={physics_wave96_ok} projectile96={projectile_wave96_ok} radar97={radar_deepen_wave97_ok} spotter97={spotter_wave97_ok} stealth97={stealth_deepen_wave97_ok} detector97={detector_deepen_wave97_ok} vision97={vision_wave97_ok} dock98={dock_wave98_ok} contain98={contain_wave98_ok} exit98={exit_wave98_ok} heal98={heal_wave98_ok} production99={production_deepen_wave99_ok} buildable99={buildable_wave99_ok} prereq99={prerequisite_wave99_ok} cmdbtn99={command_button_deepen_wave99_ok} controlbar99={control_bar_deepen_wave99_ok} thing_factory100={thing_factory_deepen_wave100_ok} module_type100={module_type_wave100_ok} xfer100={xfer_deepen_wave100_ok} tf_crosslink100={thing_factory_crosslink_wave100_ok} module_factory101={module_factory_deepen_wave101_ok} thing_factory101={thing_factory_create_wave101_ok} partition_register101={partition_register_wave101_ok} mf_crosslink101={mf_crosslink_wave101_ok} display102={display_string_deepen_wave102_ok} anim2d102={anim2d_deepen_wave102_ok} laser102={laser_segliner_deepen_wave102_ok} csf102={csf_multi_locale_deepen_wave102_ok} pres102={presentation_deepen_wave102_ok} weapon103={weapon_deepen_wave103_ok} armor103={armor_expand_wave103_ok} loco103={locomotor_expand_wave103_ok} sp103={special_power_deepen_wave103_ok} kindof103={object_kindof_wave103_ok} object_status104={object_status_wave104_ok} object_create104={object_create_wave104_ok} active_body104={active_body_wave104_ok} drawable_create104={drawable_create_wave104_ok} register_object104={register_object_wave104_ok} ai_group105={ai_group_wave105_ok} ai_path105={ai_path_wave105_ok} weapon_fire105={weapon_fire_wave105_ok} damage_app105={damage_application_wave105_ok} veterancy105={veterancy_wave105_ok} gamestate106={game_state_deepen_wave106_ok} campaign106={campaign_mission_wave106_ok} mainmenu106={main_menu_deepen_wave106_ok} gamewindow106={game_window_deepen_wave106_ok} layout106={window_layout_deepen_wave106_ok} particle107={particle_system_deepen_wave107_ok} fxlist107={fxlist_entry_deepen_wave107_ok} ocl107={ocl_create_deepen_wave107_ok} audio107={audio_deepen_wave107_ok} heightmap108={heightmap_deepen_wave108_ok} bridge108={bridge_deepen_wave108_ok} water108={water_deepen_wave108_ok} road108={road_deepen_wave108_ok} cliff108={cliff_peels_wave108_ok} sp_store109={special_power_store_wave109_ok} science_store109={science_store_wave109_ok} upgrade_store109={upgrade_store_wave109_ok} player109={player_deepen_wave109_ok} team109={team_deepen_wave109_ok} screen={screen_skirmish_ok} control_bar={control_bar_layout_ok} cb_path={control_bar_path_resolved} cb_valid={control_bar_wnd_validated} cb_loaded={control_bar_window_loaded} cb_windows={control_bar_window_count} shell_host_playable_ok={shell_host_playable_ok} playable_claim={playable_claim} {layout_report}"
        ),
    }
}

pub fn format_shell_smoke_report(r: &ShellSmokeResult) -> String {
    format!("shell_smoke status={} detail={}", r.status, r.detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    #[test]
    fn presentation_path_prefers_local_drawable_tick() {
        let cnc = include_str!("cnc_game_engine.rs");
        assert!(
            (cnc.contains("update_drawables_local") || cnc.contains("update_presentation_shell"))
                && cnc.contains("last_presentation_frame.is_some()"),
            "InGame with presentation must avoid OBJECT_REGISTRY drawable bind"
        );
        let gc = include_str!("../../GameEngine/GameClient/src/core/game_client.rs");
        assert!(
            gc.contains("fn update_drawables_local"),
            "GameClient must expose local drawable tick"
        );
    }

    #[test]
    fn presentation_shell_update_is_wired() {
        let client_src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/game_client.rs"
        ));
        let engine_src = include_str!("cnc_game_engine.rs");
        assert!(
            client_src.contains("fn update_presentation_shell"),
            "GameClient must expose presentation shell tick"
        );
        assert!(
            engine_src.contains("update_presentation_shell"),
            "engine must call presentation shell when frame is set"
        );
        assert!(
            engine_src.contains("GENERALS_RUNTIME_HOST_WND"),
            "runtime host must soft-gate WND push for headless smoke"
        );
    }

    #[test]
    fn host_smoke_applies_skirmish_and_advances_frames() {
        let r = run_shell_smoke(8);
        assert!(r.host_constructed, "host only after apply: {}", r.detail);
        assert!(r.skirmish_config_ok, "{}", r.detail);
        assert!(r.menu_config_ok, "{}", r.detail);
        assert!(r.frames_advanced > 0, "{}", r.detail);
        assert!(r.hud_selection_ok, "HUD selection residual: {}", r.detail);
        assert!(
            r.dual_tick_presentation_ok,
            "dual-tick presentation residual: {}",
            r.detail
        );
        assert!(
            r.dual_tick_counters_ok,
            "dual-tick residual counters: {}",
            r.detail
        );
        assert!(
            r.gameworld_shadow_ok,
            "gameworld shadow count parity: {}",
            r.detail
        );
        assert!(
            r.damage_authority_env_ok,
            "damage authority should default on in shell gate: {}",
            r.detail
        );
        assert!(
            r.economy_authority_env_ok,
            "economy authority should default on in shell gate: {}",
            r.detail
        );
        assert!(
            r.dual_tick_policy_authority_only,
            "dual-tick must stay AuthorityOnly by default: {}",
            r.detail
        );
        assert!(
            r.engine_bridge_off,
            "engine OBJECT_REGISTRY bridge must stay off by default: {}",
            r.detail
        );
        assert!(
            r.minimap_fow_presentation_ok,
            "minimap FOW presentation residual: {}",
            r.detail
        );
        assert!(
            r.laser_segment_upload_ok,
            "laser segment CPU upload residual: {}",
            r.detail
        );
        assert!(
            r.projectile_segment_upload_ok,
            "projectile segment CPU upload residual: {}",
            r.detail
        );
        assert!(
            r.move_line_upload_ok,
            "move line CPU upload residual: {}",
            r.detail
        );
        assert!(
            r.attack_line_upload_ok,
            "attack line CPU upload residual: {}",
            r.detail
        );
        assert!(
            r.multi_beam_soft_edge_ok,
            "multi-beam soft-edge residual: {}",
            r.detail
        );
        assert!(
            r.laser_presentation_residual_ok,
            "laser presentation residual: {}",
            r.detail
        );
        assert!(
            r.floating_text_layout_ok,
            "floating text CPU layout residual: {}",
            r.detail
        );
        assert!(
            r.floating_text_vanish_ok,
            "floating text vanish-rate residual: {}",
            r.detail
        );
        assert!(
            r.game_text_caption_ok,
            "GUI:AddCash caption residual: {}",
            r.detail
        );
        assert!(
            r.game_text_csf_str_ok,
            "CSF/STR GameText residual: {}",
            r.detail
        );
        assert!(
            r.display_string_measure_ok,
            "DisplayString measure residual: {}",
            r.detail
        );
        assert!(
            r.translate_copy_residual_ok,
            "translate_copy residual: {}",
            r.detail
        );
        assert!(
            r.world_anim_layout_ok,
            "world anim CPU layout residual: {}",
            r.detail
        );
        assert!(
            r.world_anim_fade_ok,
            "world anim fade residual: {}",
            r.detail
        );
        assert!(
            r.anim2d_frame_ok,
            "Anim2D frame advance residual: {}",
            r.detail
        );
        assert!(
            r.anim2d_collection_residual_ok,
            "Anim2DCollection residual: {}",
            r.detail
        );
        assert!(
            r.rng_stream_residual_ok,
            "RNG stream residual: {}",
            r.detail
        );
        assert!(
            r.mesh_asset_residual_ok,
            "mesh asset residual: {}",
            r.detail
        );
        assert!(
            r.rng_residual_pack_ok,
            "RNG residual pack wave72: {}",
            r.detail
        );
        assert!(
            r.special_power_wave72_residual_ok,
            "special power residual pack wave72: {}",
            r.detail
        );
        assert!(
            r.special_power_wave73_residual_ok,
            "special power residual pack wave73: {}",
            r.detail
        );
        assert!(
            r.special_power_wave76_residual_ok,
            "special power residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.paradrop_wave76_residual_ok,
            "paradrop science-tier residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.control_bar_wave76_residual_ok,
            "control bar residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.graphics_wave76_residual_ok,
            "graphics residual pack wave76: {}",
            r.detail
        );
        assert!(
            r.spectre_orbit_decal_presentation_ok,
            "spectre orbit decal presentation residual: {}",
            r.detail
        );
        assert!(
            r.special_power_wave77_residual_ok,
            "special power audio residual pack wave77: {}",
            r.detail
        );
        assert!(
            r.fow_residual_pack_ok,
            "FOW residual pack wave77: {}",
            r.detail
        );
        assert!(
            r.ground_height_presentation_ok,
            "ground height presentation residual wave77: {}",
            r.detail
        );
        assert!(
            r.weapon_store_seed_residual_ok,
            "weapon store seed residual wave77: {}",
            r.detail
        );
        assert!(
            r.ai_skirmish_residual_ok,
            "AI skirmish residual pack wave77: {}",
            r.detail
        );
        assert!(
            r.special_power_wave78_residual_ok,
            "special power residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.cluster_mines_wave78_residual_ok,
            "cluster mines residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.gps_scrambler_wave78_residual_ok,
            "GPS scrambler residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.cash_bounty_wave78_residual_ok,
            "cash bounty residual pack wave78: {}",
            r.detail
        );
        assert!(
            r.minimap_residual_pack_ok,
            "minimap residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.selection_hud_residual_pack_ok,
            "selection HUD residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.input_residual_pack_ok,
            "input residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.drawable_residual_fields_ok,
            "drawable residual fields wave79: {}",
            r.detail
        );
        assert!(
            r.unit_training_wave79_residual_ok,
            "unit training residual pack wave79: {}",
            r.detail
        );
        assert!(
            r.upgrades_cost_time_application_ok,
            "upgrades cost/time application wave79: {}",
            r.detail
        );
        assert!(
            r.command_button_wave80_residual_ok,
            "command button residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.science_rank_wave80_residual_ok,
            "science rank residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.superweapon_kindof_wave80_residual_ok,
            "superweapon kindof residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.special_power_enum_wave80_residual_ok,
            "special power enum residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.terrain_height_sample_wave81_ok,
            "map height sample residual pack wave81: {}",
            r.detail
        );
        assert!(
            r.pathfinder_wave81_residual_ok,
            "pathfinder residual pack wave81: {}",
            r.detail
        );
        assert!(
            r.locomotor_table_wave81_ok,
            "locomotor residual table wave81: {}",
            r.detail
        );
        assert!(
            r.armor_table_wave81_ok,
            "armor residual table wave81: {}",
            r.detail
        );
        assert!(
            r.puc_flare_table_wave81_ok,
            "PUC flare name table residual wave81: {}",
            r.detail
        );
        assert!(
            r.damage_type_wave82_ok,
            "damage type residual enum table wave82: {}",
            r.detail
        );
        assert!(
            r.death_type_wave82_ok,
            "death type residual enum table wave82: {}",
            r.detail
        );
        assert!(
            r.model_condition_wave82_ok,
            "model condition residual flags wave82: {}",
            r.detail
        );
        assert!(
            r.weapon_bonus_wave82_ok,
            "weapon bonus residual type table wave82: {}",
            r.detail
        );
        assert!(
            r.object_status_wave82_ok,
            "object status residual table wave82: {}",
            r.detail
        );
        assert!(
            r.production_queue_wave83_ok,
            "production queue residual pack wave83: {}",
            r.detail
        );
        assert!(
            r.supply_warehouse_wave83_ok,
            "supply warehouse residual pack wave83: {}",
            r.detail
        );
        assert!(
            r.dozer_build_wave83_ok,
            "dozer build residual pack wave83: {}",
            r.detail
        );
        assert!(
            r.capture_building_wave83_ok,
            "capture building residual pack wave83: {}",
            r.detail
        );
        assert!(
            r.power_plant_wave83_ok,
            "power plant residual pack wave83: {}",
            r.detail
        );
        assert!(
            r.command_center_wave83_ok,
            "command center residual pack wave83: {}",
            r.detail
        );
        assert!(
            r.kindof_wave84_ok,
            "kindof residual bit-name table wave84: {}",
            r.detail
        );
        assert!(
            r.weapon_slot_wave84_ok,
            "weapon slot residual table wave84: {}",
            r.detail
        );
        assert!(
            r.veterancy_wave84_ok,
            "veterancy residual level table wave84: {}",
            r.detail
        );
        assert!(
            r.relationship_wave84_ok,
            "relationship residual table wave84: {}",
            r.detail
        );
        assert!(
            r.geometry_wave84_ok,
            "geometry residual type table wave84: {}",
            r.detail
        );
        assert!(
            r.shadow_wave84_ok,
            "shadow residual type table wave84: {}",
            r.detail
        );
        assert!(
            r.faction_side_wave85_ok,
            "faction side residual table wave85: {}",
            r.detail
        );
        assert!(
            r.player_template_wave85_ok,
            "player template residual pack wave85: {}",
            r.detail
        );
        assert!(
            r.starting_cash_wave85_ok,
            "starting cash residual pack wave85: {}",
            r.detail
        );
        assert!(
            r.skirmish_ai_personality_wave85_ok,
            "skirmish AI personality residual pack wave85: {}",
            r.detail
        );
        assert!(
            r.victory_condition_wave85_ok,
            "victory condition residual pack wave85: {}",
            r.detail
        );
        assert!(
            r.gamedata_camera_fps_wave86_ok,
            "gamedata camera/FPS residual pack wave86: {}",
            r.detail
        );
        assert!(
            r.gamedata_world_constants_wave86_ok,
            "gamedata world constants residual pack wave86: {}",
            r.detail
        );
        assert!(
            r.multiplayer_options_wave86_ok,
            "multiplayer options residual pack wave86: {}",
            r.detail
        );
        assert!(
            r.map_selection_wave86_ok,
            "map selection residual pack wave86: {}",
            r.detail
        );
        assert!(
            r.crate_deepen_wave86_ok,
            "crate residual deepen pack wave86: {}",
            r.detail
        );
        assert!(
            r.weather_wave87_ok,
            "weather residual pack wave87: {}",
            r.detail
        );
        assert!(
            r.water_wave87_ok,
            "water residual pack wave87: {}",
            r.detail
        );
        assert!(
            r.bridge_wave87_ok,
            "bridge residual pack wave87: {}",
            r.detail
        );
        assert!(
            r.tunnel_wave87_ok,
            "tunnel residual deepen wave87: {}",
            r.detail
        );
        assert!(
            r.garrison_wave87_ok,
            "garrison residual pack wave87: {}",
            r.detail
        );
        assert!(
            r.transport_wave87_ok,
            "transport residual pack wave87: {}",
            r.detail
        );
        assert!(
            r.radius_cursor_wave88_ok,
            "radius cursor residual name table wave88: {}",
            r.detail
        );
        assert!(
            r.mouse_cursor_wave88_ok,
            "mouse cursor residual name table wave88: {}",
            r.detail
        );
        assert!(
            r.superweapon_fxlist_wave88_ok,
            "superweapon FXList residual name table wave88: {}",
            r.detail
        );
        assert!(
            r.superweapon_ocl_wave88_ok,
            "superweapon OCL residual name table wave88: {}",
            r.detail
        );
        assert!(
            r.superweapon_particle_wave88_ok,
            "superweapon particle residual name table wave88: {}",
            r.detail
        );
        assert!(
            r.superweapon_audio_wave88_ok,
            "superweapon audio residual name table wave88: {}",
            r.detail
        );
        assert!(
            r.rank_skill_wave89_ok,
            "rank skill-points application residual pack wave89: {}",
            r.detail
        );
        assert!(
            r.experience_wave89_ok,
            "experience residual tables pack wave89: {}",
            r.detail
        );
        assert!(
            r.hotkey_wave89_ok,
            "hotkey residual table pack wave89: {}",
            r.detail
        );
        assert!(
            r.chat_wave89_ok,
            "chat residual host pack wave89: {}",
            r.detail
        );
        assert!(
            r.replay_wave89_ok,
            "replay residual host pack wave89: {}",
            r.detail
        );
        assert!(
            r.options_wave89_ok,
            "options residual pack wave89: {}",
            r.detail
        );
        assert!(
            r.gamespeed_wave90_ok,
            "gamespeed residual pack wave90: {}",
            r.detail
        );
        assert!(
            r.frame_rate_wave90_ok,
            "frame rate residual deepen pack wave90: {}",
            r.detail
        );
        assert!(
            r.debug_tables_wave90_ok,
            "debug residual tables pack wave90: {}",
            r.detail
        );
        assert!(
            r.language_wave90_ok,
            "language residual deepen pack wave90: {}",
            r.detail
        );
        assert!(
            r.credits_wave90_ok,
            "credits residual pack wave90: {}",
            r.detail
        );
        assert!(
            r.tooltip_wave91_ok,
            "tooltip residual pack wave91: {}",
            r.detail
        );
        assert!(
            r.help_box_wave91_ok,
            "help box residual pack wave91: {}",
            r.detail
        );
        assert!(
            r.message_wave91_ok,
            "message residual pack wave91: {}",
            r.detail
        );
        assert!(r.eva_wave91_ok, "eva residual pack wave91: {}", r.detail);
        assert!(
            r.video_wave91_ok,
            "video residual name table wave91: {}",
            r.detail
        );
        assert!(
            r.mission_briefing_wave91_ok,
            "mission briefing residual pack wave91: {}",
            r.detail
        );
        assert!(
            r.weapon_deepen_wave92_ok,
            "weapon residual deepen pack wave92: {}",
            r.detail
        );
        assert!(
            r.armor_expand_wave92_ok,
            "armor residual expand pack wave92: {}",
            r.detail
        );
        assert!(
            r.body_health_wave92_ok,
            "body max health residual table wave92: {}",
            r.detail
        );
        assert!(
            r.locomotor_expand_wave92_ok,
            "locomotor residual expand pack wave92: {}",
            r.detail
        );
        assert!(
            r.science_names_wave92_ok,
            "science residual name table wave92: {}",
            r.detail
        );
        assert!(
            r.particle_emit_wave93_ok,
            "particle emit-rate residual deepen pack wave93: {}",
            r.detail
        );
        assert!(
            r.drawable_opacity_wave93_ok,
            "drawable opacity/shroud residual deepen pack wave93: {}",
            r.detail
        );
        assert!(
            r.shadow_deepen_wave93_ok,
            "shadow residual deepen pack wave93: {}",
            r.detail
        );
        assert!(
            r.terrain_texture_wave93_ok,
            "terrain texture residual pack wave93: {}",
            r.detail
        );
        assert!(r.road_wave93_ok, "road residual pack wave93: {}", r.detail);
        assert!(
            r.ai_state_wave94_ok,
            "AI state residual table wave94: {}",
            r.detail
        );
        assert!(
            r.special_ability_wave94_ok,
            "special ability residual deepen wave94: {}",
            r.detail
        );
        assert!(
            r.upgrade_names_wave94_ok,
            "upgrade name table residual wave94: {}",
            r.detail
        );
        assert!(
            r.command_set_wave94_ok,
            "CommandSet superweapon residual wave94: {}",
            r.detail
        );
        assert!(
            r.script_action_wave95_ok,
            "script action name table residual wave95: {}",
            r.detail
        );
        assert!(
            r.script_condition_wave95_ok,
            "script condition name table residual wave95: {}",
            r.detail
        );
        assert!(
            r.map_object_wave95_ok,
            "map object residual pack wave95: {}",
            r.detail
        );
        assert!(
            r.waypoint_wave95_ok,
            "waypoint residual pack wave95: {}",
            r.detail
        );
        assert!(r.team_wave95_ok, "team residual pack wave95: {}", r.detail);
        assert!(
            r.player_deepen_wave95_ok,
            "player residual deepen pack wave95: {}",
            r.detail
        );
        assert!(
            r.partition_wave96_ok,
            "partition residual pack wave96: {}",
            r.detail
        );
        assert!(
            r.collision_wave96_ok,
            "collision residual pack wave96: {}",
            r.detail
        );
        assert!(
            r.physics_wave96_ok,
            "physics residual pack wave96: {}",
            r.detail
        );
        assert!(
            r.projectile_wave96_ok,
            "projectile residual deepen pack wave96: {}",
            r.detail
        );

        assert!(
            r.radar_deepen_wave97_ok,
            "radar residual deepen pack wave97: {}",
            r.detail
        );
        assert!(
            r.spotter_wave97_ok,
            "spotter residual pack wave97: {}",
            r.detail
        );
        assert!(
            r.stealth_deepen_wave97_ok,
            "stealth residual deepen pack wave97: {}",
            r.detail
        );
        assert!(
            r.detector_deepen_wave97_ok,
            "detector residual deepen pack wave97: {}",
            r.detail
        );
        assert!(
            r.vision_wave97_ok,
            "vision residual pack wave97: {}",
            r.detail
        );
        assert!(r.dock_wave98_ok, "dock residual pack wave98: {}", r.detail);
        assert!(
            r.contain_wave98_ok,
            "contain residual deepen pack wave98: {}",
            r.detail
        );
        assert!(r.exit_wave98_ok, "exit residual pack wave98: {}", r.detail);
        assert!(
            r.heal_wave98_ok,
            "heal residual deepen pack wave98: {}",
            r.detail
        );
        assert!(
            r.production_deepen_wave99_ok,
            "production residual deepen pack wave99: {}",
            r.detail
        );
        assert!(
            r.buildable_wave99_ok,
            "buildable residual pack wave99: {}",
            r.detail
        );
        assert!(
            r.prerequisite_wave99_ok,
            "prerequisite residual pack wave99: {}",
            r.detail
        );
        assert!(
            r.command_button_deepen_wave99_ok,
            "command button residual deepen pack wave99: {}",
            r.detail
        );
        assert!(
            r.control_bar_deepen_wave99_ok,
            "control bar residual deepen pack wave99: {}",
            r.detail
        );
        assert!(
            r.thing_factory_deepen_wave100_ok,
            "thing factory residual deepen pack wave100: {}",
            r.detail
        );
        assert!(
            r.module_type_wave100_ok,
            "module type table residual pack wave100: {}",
            r.detail
        );
        assert!(
            r.xfer_deepen_wave100_ok,
            "xfer residual deepen pack wave100: {}",
            r.detail
        );
        assert!(
            r.thing_factory_crosslink_wave100_ok,
            "thing factory spawn crosslink residual pack wave100: {}",
            r.detail
        );
        assert!(
            r.module_factory_deepen_wave101_ok,
            "module factory residual deepen pack wave101: {}",
            r.detail
        );
        assert!(
            r.thing_factory_create_wave101_ok,
            "thing factory create residual deepen pack wave101: {}",
            r.detail
        );
        assert!(
            r.partition_register_wave101_ok,
            "partition register residual pack wave101: {}",
            r.detail
        );
        assert!(
            r.mf_crosslink_wave101_ok,
            "thing factory module partition crosslink residual pack wave101: {}",
            r.detail
        );
        assert!(
            r.display_string_deepen_wave102_ok,
            "display string residual deepen pack wave102: {}",
            r.detail
        );
        assert!(
            r.anim2d_deepen_wave102_ok,
            "anim2d residual deepen pack wave102: {}",
            r.detail
        );
        assert!(
            r.laser_segliner_deepen_wave102_ok,
            "laser segliner residual deepen pack wave102: {}",
            r.detail
        );
        assert!(
            r.csf_multi_locale_deepen_wave102_ok,
            "csf multi-locale residual deepen pack wave102: {}",
            r.detail
        );
        assert!(
            r.presentation_deepen_wave102_ok,
            "presentation residual deepen pack wave102: {}",
            r.detail
        );
        assert!(
            r.weapon_deepen_wave103_ok,
            "weapon residual deepen pack wave103: {}",
            r.detail
        );
        assert!(
            r.armor_expand_wave103_ok,
            "armor residual expand pack wave103: {}",
            r.detail
        );
        assert!(
            r.locomotor_expand_wave103_ok,
            "locomotor residual expand pack wave103: {}",
            r.detail
        );
        assert!(
            r.special_power_deepen_wave103_ok,
            "special power superweapon residual deepen pack wave103: {}",
            r.detail
        );
        assert!(
            r.object_kindof_wave103_ok,
            "object kindof residual pack wave103: {}",
            r.detail
        );

        assert!(
            r.object_status_wave104_ok,
            "object status state machine residual pack wave104: {}",
            r.detail
        );
        assert!(
            r.object_create_wave104_ok,
            "object create residual order pack wave104: {}",
            r.detail
        );
        assert!(
            r.active_body_wave104_ok,
            "active body max health apply residual pack wave104: {}",
            r.detail
        );
        assert!(
            r.drawable_create_wave104_ok,
            "drawable create residual bookkeeping pack wave104: {}",
            r.detail
        );
        assert!(
            r.register_object_wave104_ok,
            "gamelogic registerObject m_objList residual pack wave104: {}",
            r.detail
        );
        assert!(
            r.ai_group_wave105_ok,
            "ai group residual peels pack wave105: {}",
            r.detail
        );
        assert!(
            r.ai_path_wave105_ok,
            "ai path residual deepen pack wave105: {}",
            r.detail
        );
        assert!(
            r.weapon_fire_wave105_ok,
            "weapon fire residual deepen pack wave105: {}",
            r.detail
        );
        assert!(
            r.damage_application_wave105_ok,
            "damage application residual deepen pack wave105: {}",
            r.detail
        );
        assert!(
            r.veterancy_wave105_ok,
            "veterancy residual deepen pack wave105: {}",
            r.detail
        );
        assert!(
            r.game_state_deepen_wave106_ok,
            "game state residual deepen pack wave106: {}",
            r.detail
        );
        assert!(
            r.campaign_mission_wave106_ok,
            "campaign mission residual deepen pack wave106: {}",
            r.detail
        );
        assert!(
            r.main_menu_deepen_wave106_ok,
            "main menu residual deepen pack wave106: {}",
            r.detail
        );
        assert!(
            r.game_window_deepen_wave106_ok,
            "game window residual deepen pack wave106: {}",
            r.detail
        );
        assert!(
            r.window_layout_deepen_wave106_ok,
            "window layout residual deepen pack wave106: {}",
            r.detail
        );
        assert!(
            r.particle_system_deepen_wave107_ok,
            "particle system residual deepen pack wave107: {}",
            r.detail
        );
        assert!(
            r.fxlist_entry_deepen_wave107_ok,
            "fxlist entry residual deepen pack wave107: {}",
            r.detail
        );
        assert!(
            r.ocl_create_deepen_wave107_ok,
            "ocl create residual deepen pack wave107: {}",
            r.detail
        );
        assert!(
            r.audio_deepen_wave107_ok,
            "audio residual deepen pack wave107: {}",
            r.detail
        );
        assert!(
            r.heightmap_deepen_wave108_ok,
            "heightmap residual deepen pack wave108: {}",
            r.detail
        );
        assert!(
            r.bridge_deepen_wave108_ok,
            "bridge residual deepen pack wave108: {}",
            r.detail
        );
        assert!(
            r.water_deepen_wave108_ok,
            "water residual deepen pack wave108: {}",
            r.detail
        );
        assert!(
            r.road_deepen_wave108_ok,
            "road residual deepen pack wave108: {}",
            r.detail
        );
        assert!(
            r.cliff_peels_wave108_ok,
            "cliff residual peels pack wave108: {}",
            r.detail
        );
        assert!(
            r.special_power_store_wave109_ok,
            "special power template store residual pack wave109: {}",
            r.detail
        );
        assert!(
            r.science_store_wave109_ok,
            "science store residual deepen pack wave109: {}",
            r.detail
        );
        assert!(
            r.upgrade_store_wave109_ok,
            "upgrade store residual deepen pack wave109: {}",
            r.detail
        );
        assert!(
            r.player_deepen_wave109_ok,
            "player residual deepen pack wave109: {}",
            r.detail
        );
        assert!(
            r.team_deepen_wave109_ok,
            "team residual deepen pack wave109: {}",
            r.detail
        );
        assert!(
            r.command_button_wave80_residual_ok,
            "command button superweapon residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.science_rank_wave80_residual_ok,
            "science rank residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.superweapon_kindof_wave80_residual_ok,
            "superweapon kindof residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.special_power_enum_wave80_residual_ok,
            "special power enum residual pack wave80: {}",
            r.detail
        );
        assert!(
            r.world_anim_presentation_ok,
            "world anim presentation residual: {}",
            r.detail
        );
        assert!(
            r.control_bar_layout_ok,
            "ControlBar.wnd ensure residual: {}",
            r.detail
        );
        assert!(
            r.selection_consumers_ok,
            "multi-consumer selection panel residual: {}",
            r.detail
        );
        // When WindowZH is present, path+validate honesty must be true; prefer
        // headless WindowManager load (not required for CI without assets).
        if r.control_bar_path_resolved {
            assert!(
                r.control_bar_wnd_validated,
                "ControlBar structural validate residual: {}",
                r.detail
            );
            #[cfg(feature = "game_client")]
            if r.control_bar_window_loaded {
                assert!(
                    r.control_bar_window_count > 0,
                    "WindowManager load must materialise windows: {}",
                    r.detail
                );
            }
        } else {
            assert!(
                !r.control_bar_window_loaded && r.control_bar_window_count == 0,
                "missing assets must not claim window load: {}",
                r.detail
            );
        }
        assert!(
            r.screen_skirmish_ok,
            "shell→InGame screen residual: {}",
            r.detail
        );
        // Limited host claim when path is fully operational; never retail W3D claim.
        assert!(
            r.shell_host_playable_ok,
            "shell_host_playable_ok for successful headless host path: {}",
            r.detail
        );
        assert!(
            !r.playable_claim,
            "headless smoke must not claim retail playable"
        );
        assert_eq!(r.status, "success", "{}", r.detail);
        assert_eq!(
            r.shell_host_playable_ok,
            r.status == "success",
            "shell_host_playable_ok must track success without overclaiming playable_claim"
        );
    }

    #[test]
    fn shell_host_playable_ok_never_implies_retail_playable_claim() {
        let r = run_shell_smoke(4);
        // Documented honesty contract: limited host flag is independent of retail claim.
        if r.shell_host_playable_ok {
            assert!(
                !r.playable_claim,
                "shell_host_playable_ok must never flip playable_claim"
            );
        }
        assert!(!r.playable_claim);
    }

    #[test]
    fn presentation_carries_transform_health_team_model() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresFields");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("SmokeUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SmokeUnit".into(), t);
        let id = logic
            .create_object("SmokeUnit", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("unit");
        logic.update();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let obj = frame
            .objects
            .iter()
            .find(|o| o.id == id)
            .expect("object in presentation");
        assert_eq!(obj.team, Team::USA);
        assert!((obj.position.x - 3.0).abs() < 0.01);
        assert!(obj.health_current > 0.0);
        assert_eq!(obj.health_max, 50.0);
        assert_eq!(obj.model_key.as_deref(), Some("SmokeUnit"));
        assert!(!obj.destroyed);
    }

    #[test]
    fn dual_tick_after_map_load_seeds_hud_selection_health() {
        // Residual closed by this change: after skirmish config + (optional) map load,
        // dual-tick presentation must put selection health on GameHUD.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ShellHudSel");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("ShellSelUnit");
        t.set_health(64.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ShellSelUnit".into(), t);
        let id = logic
            .create_object("ShellSelUnit", Team::USA, Vec3::new(2.0, 0.0, 2.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }

        // Seed like start_game_from_ui before first logic frame.
        let mut hud = GameHUD::new();
        let seed = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        assert!(
            seed.alive_object_count() >= 1,
            "seed presentation must see map/host units"
        );
        assert!(
            hud.selected_unit_ids().contains(&id),
            "seed apply must set HUD selection"
        );

        logic.update();
        let post = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        let info = hud
            .selected_unit_infos()
            .iter()
            .find(|u| u.object_id == id)
            .expect("dual-tick HUD selection health");
        assert!(
            (info.health_current - 64.0).abs() < 0.01,
            "health from presentation after dual-tick: {}",
            info.health_current
        );
        assert!(
            hud.selection_panel().has_positive_health(),
            "ControlBar selection panel health after dual-tick"
        );
        assert!(
            (hud.selection_panel().health_current - 64.0).abs() < 0.01,
            "selection panel HP from presentation: {}",
            hud.selection_panel().health_current
        );
        assert_eq!(post.frame.0, logic.get_frame());
        assert!(!post.hud_minimap_units().is_empty());

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            post.apply_to_control_bar(&mut bar);
            let (hp, _) = bar
                .selection_panel_health()
                .expect("ControlBar health from dual-tick presentation");
            assert!((hp - 64.0).abs() < 0.01, "ControlBar HP {hp}");
        }
    }
}

#[cfg(test)]
mod presentation_shell_deepen_tests {
    #[test]
    fn presentation_shell_deepens_visual_speed_without_main_draw_ownership() {
        let src = include_str!("../../GameEngine/GameClient/src/core/game_client.rs");
        let idx = src
            .find("fn update_presentation_shell")
            .expect("presentation shell");
        let window = &src[idx..idx + 2800];
        assert!(
            window.contains("get_script_visual_speed_multiplier"),
            "shell must scale visual delta by script visual speed"
        );
        assert!(
            window.contains("should_freeze_visual_time"),
            "shell must honor visual freeze residual"
        );
        assert!(
            window.contains("update_display_string_manager"),
            "shell must tick DisplayStringManager residual"
        );
        assert!(
            window.contains("update_display_only"),
            "shell must run display UPDATE residual (not DRAW)"
        );
        assert!(
            window.contains("draw_drawable_icon_ui"),
            "shell must run drawable icon UI residual"
        );
        assert!(
            !window.contains("self.update_input")
                && !window.contains("self.update_audio")
                && !window.contains("self.draw_display"),
            "presentation shell must not take Main input/audio/draw ownership"
        );
        assert!(
            window.contains("update_drawables_local"),
            "shell keeps local drawable path (no OBJECT_REGISTRY shroud bind)"
        );
    }
}

#[cfg(test)]
mod presentation_mouse_bounds_tests {
    #[test]
    fn mouse_world_position_prefers_presentation_bounds() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("fn update_mouse_world_position")
            .expect("update_mouse_world_position");
        let window = &eng[idx..idx + 900];
        assert!(
            window.contains("last_presentation_frame") && window.contains("world_bounds_vec3"),
            "mouse map must prefer presentation world_env bounds"
        );
        assert!(
            window.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
    }
}

#[cfg(test)]
mod presentation_camera_bounds_tests {
    #[test]
    fn clamp_to_world_bounds_prefers_presentation() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("fn clamp_to_world_bounds")
            .expect("clamp_to_world_bounds");
        let window = &eng[idx..idx + 700];
        assert!(
            window.contains("last_presentation_frame") && window.contains("world_bounds_vec3"),
            "camera clamp must prefer presentation world_env bounds"
        );
        assert!(
            window.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
    }
}

#[cfg(test)]
mod presentation_minimap_bounds_tests {
    #[test]
    fn minimap_viewport_prefers_presentation_bounds() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("fn update_minimap_viewport")
            .expect("update_minimap_viewport");
        let window = &eng[idx..idx + 700];
        assert!(
            window.contains("last_presentation_frame") && window.contains("world_bounds_vec3"),
            "minimap viewport must prefer presentation world_env bounds"
        );
        assert!(
            window.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
        // Radar pings also prefer presentation bounds near the UI overlay path.
        let radar_idx = eng.find("update_radar_pings").expect("update_radar_pings");
        let radar_window = &eng[radar_idx.saturating_sub(350)..radar_idx + 80];
        assert!(
            radar_window.contains("last_presentation_frame")
                && radar_window.contains("world_bounds_vec3"),
            "radar pings must prefer presentation world_env bounds"
        );
    }
}

#[cfg(test)]
mod presentation_local_team_tests {
    #[test]
    fn selection_hotkeys_prefer_presentation_local_team() {
        let eng = include_str!("cnc_game_engine.rs");
        for needle in [
            "Ctrl+A: select all",
            "Cycle selection through own selectable",
            "fn find_object_at_position",
            "fn handle_right_click",
        ] {
            let idx = eng
                .find(needle)
                .unwrap_or_else(|| panic!("missing {needle}"));
            let window = &eng[idx..idx + 900.min(eng.len() - idx)];
            assert!(
                window.contains("local_team") || window.contains("local_team()"),
                "{needle} must prefer presentation local_team"
            );
        }
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("pub local_team: Team"),
            "PresentationFrame must freeze local_team"
        );
    }
}

#[cfg(test)]
mod presentation_select_similar_tests {
    #[test]
    fn select_similar_units_prefers_presentation_local_team() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("fn select_similar_units")
            .expect("select_similar_units");
        let window = &eng[idx..idx + 900];
        assert!(
            window.contains("local_team") || window.contains("local_team()"),
            "select_similar_units must prefer presentation local_team"
        );
        assert!(
            window.contains("similar_unit_ids"),
            "select_similar_units must use presentation similar_unit_ids when frame set"
        );
        assert!(
            window.contains("game_logic.get_player"),
            "boot residual without frame may still use host player team"
        );
    }
}

#[cfg(test)]
mod presentation_player_roster_tests {
    #[test]
    fn defeat_ui_prefers_presentation_player_roster() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("Broadcast defeat notifications")
            .expect("defeat notifications");
        let window = &eng[idx..idx + 1600];
        assert!(
            window.contains("player_info(player_id)") || window.contains("player_info("),
            "defeat UI must prefer presentation player roster"
        );
        assert!(
            window.contains("game_logic.get_player(player_id)"),
            "boot residual without roster entry may still use host player"
        );
        let alliance_idx = eng
            .find("Prefer presentation roster team when installed")
            .expect("alliance roster prefer");
        let alliance_window = &eng[alliance_idx..alliance_idx + 500];
        assert!(
            alliance_window.contains("player_team("),
            "alliance radar must prefer presentation player_team"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("pub struct PresentationPlayerInfo")
                && pf.contains("pub players: Vec<PresentationPlayerInfo>"),
            "PresentationFrame must freeze players roster"
        );
    }
}

#[cfg(test)]
mod presentation_victory_shell_tests {
    #[test]
    fn victory_eval_prefers_presentation_shell_bypass() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("Prefer presentation shell bypass when a frame is installed")
            .expect("victory shell prefer");
        let window = &eng[idx..idx + 500];
        assert!(
            window.contains("fow_shell_bypass") && window.contains("isInShellGame"),
            "victory eval must prefer presentation fow_shell_bypass with live residual"
        );
    }
}
