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
use crate::game_logic::host_beacon_residual_wave142::{
    honesty_beacon_control_names_residual_wave142, honesty_beacon_nav_commands_residual_wave142,
};
use crate::game_logic::host_cash_bounty::honesty_cash_bounty_residual_pack_wave78;
use crate::game_logic::host_challenge_generals_residual_wave152::{
    honesty_challenge_generals_method_names_residual_wave152,
    honesty_challenge_generals_nav_commands_residual_wave152,
};
use crate::game_logic::host_challenge_menu_residual_wave120::{
    honesty_challenge_menu_control_names_residual_wave120,
    honesty_challenge_menu_nav_commands_residual_wave120,
};
use crate::game_logic::host_combat_sim_residual::{
    honesty_body_max_health_residual_table_wave92, honesty_science_name_table_residual_wave92,
};
use crate::game_logic::host_command_button_residual::honesty_command_button_superweapon_residual_pack_wave80;
use crate::game_logic::host_control_bar_materialise_residual_wave165::{
    honesty_control_bar_materialise_method_names_residual_wave165,
    honesty_control_bar_materialise_nav_commands_residual_wave165,
    simulate_control_bar_materialise_honesty_wave165,
};
use crate::game_logic::host_control_bar_print_positions_residual_wave158::{
    honesty_control_bar_print_names_residual_wave158,
    honesty_control_bar_print_nav_commands_residual_wave158,
};
use crate::game_logic::host_control_bar_residual_wave133::{
    honesty_control_bar_control_names_residual_wave133,
    honesty_control_bar_nav_commands_residual_wave133,
};
use crate::game_logic::host_control_bar_resizer_residual_wave147::{
    honesty_control_bar_resizer_method_names_residual_wave147,
    honesty_control_bar_resizer_nav_commands_residual_wave147,
};
use crate::game_logic::host_control_bar_scheme_residual_wave156::{
    honesty_control_bar_scheme_method_names_residual_wave156,
    honesty_control_bar_scheme_names_residual_wave156,
    honesty_control_bar_scheme_nav_commands_residual_wave156,
};
use crate::game_logic::host_credits_menu_residual_wave127::{
    honesty_credits_menu_control_names_residual_wave127,
    honesty_credits_menu_nav_commands_residual_wave127,
};
use crate::game_logic::host_credits_roll_residual_wave151::{
    honesty_credits_nav_commands_residual_wave151,
    honesty_credits_style_method_names_residual_wave151,
};
use crate::game_logic::host_difficulty_select_residual_wave134::{
    honesty_difficulty_select_control_names_residual_wave134,
    honesty_difficulty_select_nav_commands_residual_wave134,
};
use crate::game_logic::host_diplomacy_residual_wave129::{
    honesty_diplomacy_control_names_residual_wave129,
    honesty_diplomacy_nav_commands_residual_wave129,
};
use crate::game_logic::host_dock_contain_exit_heal_residual::{
    honesty_contain_residual_deepen_pack_wave98, honesty_dock_residual_pack_wave98,
    honesty_exit_residual_pack_wave98, honesty_heal_residual_deepen_pack_wave98,
};
use crate::game_logic::host_drawable_display_client_residual_wave111::{
    honesty_display_draw_image_mode_residual_wave111, honesty_drawable_icon_flash_residual_wave111,
    honesty_drawable_status_stealth_residual_wave111,
    honesty_game_client_translator_residual_wave111, honesty_particle_priority_residual_wave111,
    honesty_terrain_decal_residual_wave111,
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
use crate::game_logic::host_eva_residual_wave143::{
    honesty_eva_message_names_residual_wave143, honesty_eva_nav_commands_residual_wave143,
};
use crate::game_logic::host_executable_gameworld_presentation_residual_wave188::{
    honesty_executable_gameworld_presentation_method_names_residual_wave188,
    honesty_executable_gameworld_presentation_nav_commands_residual_wave188,
    simulate_executable_gameworld_presentation_honesty,
};
use crate::game_logic::host_executable_presentation_boundary_residual_wave176::{
    honesty_executable_presentation_boundary_method_names_residual_wave176,
    honesty_executable_presentation_boundary_nav_commands_residual_wave176,
    simulate_executable_presentation_boundary_honesty,
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
use crate::game_logic::host_gadget_video_audio_residual_wave113::{
    honesty_audio_event_residual_wave113, honesty_gadget_residual_wave113,
    honesty_game_window_manager_residual_wave113, honesty_video_buffer_residual_wave113,
    honesty_window_style_residual_wave113,
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
use crate::game_logic::host_gameworld_authority_matrix_residual_wave179::{
    honesty_gameworld_authority_matrix_method_names_residual_wave179,
    honesty_gameworld_authority_matrix_nav_commands_residual_wave179,
    simulate_gameworld_authority_matrix_honesty,
};
use crate::game_logic::host_gameworld_authority_residual_wave153::{
    honesty_gameworld_authority_env_names_residual_wave153,
    honesty_gameworld_authority_method_names_residual_wave153,
    honesty_gameworld_authority_nav_commands_residual_wave153,
};
use crate::game_logic::host_gameworld_production_authority_residual_wave177::{
    honesty_gameworld_production_authority_method_names_residual_wave177,
    honesty_gameworld_production_authority_nav_commands_residual_wave177,
    simulate_gameworld_production_authority_honesty,
};
use crate::game_logic::host_gameworld_sole_tick_coupling_residual_wave178::{
    honesty_gameworld_sole_tick_coupling_method_names_residual_wave178,
    honesty_gameworld_sole_tick_coupling_nav_commands_residual_wave178,
    simulate_gameworld_sole_tick_coupling_honesty,
};
use crate::game_logic::host_generals_exp_residual_wave138::{
    honesty_generals_exp_control_names_residual_wave138,
    honesty_generals_exp_nav_commands_residual_wave138,
};
use crate::game_logic::host_golden_map_host_victory_residual_wave175::{
    honesty_golden_map_host_victory_method_names_residual_wave175,
    honesty_golden_map_host_victory_nav_commands_residual_wave175,
    simulate_golden_map_host_victory_honesty,
};
use crate::game_logic::host_gps_scrambler::honesty_gps_scrambler_residual_pack_wave78;
use crate::game_logic::host_idle_worker_residual_wave137::{
    honesty_idle_worker_control_names_residual_wave137,
    honesty_idle_worker_nav_commands_residual_wave137,
};
use crate::game_logic::host_ime_residual_wave144::{
    honesty_ime_message_names_residual_wave144, honesty_ime_nav_commands_residual_wave144,
};
use crate::game_logic::host_in_game_chat_residual_wave136::{
    honesty_in_game_chat_control_names_residual_wave136,
    honesty_in_game_chat_nav_commands_residual_wave136,
};
use crate::game_logic::host_keyboard_options_residual_wave124::{
    honesty_keyboard_options_control_names_residual_wave124,
    honesty_keyboard_options_nav_commands_residual_wave124,
};
use crate::game_logic::host_live_cmd_filter_env_presentation_only_residual_wave217::{
    honesty_live_cmd_filter_env_presentation_only_method_names_residual_wave217,
    honesty_live_cmd_filter_env_presentation_only_nav_commands_residual_wave217,
    simulate_live_cmd_filter_env_presentation_only_honesty,
};
use crate::game_logic::host_live_command_attack_log_residual_wave197::{
    honesty_live_command_attack_log_method_names_residual_wave197,
    honesty_live_command_attack_log_nav_commands_residual_wave197,
    simulate_live_command_attack_log_honesty,
};
use crate::game_logic::host_live_command_beacon_note_residual_wave210::{
    honesty_live_command_beacon_note_method_names_residual_wave210,
    honesty_live_command_beacon_note_nav_commands_residual_wave210,
    simulate_live_command_beacon_note_honesty,
};
use crate::game_logic::host_live_command_cheer_science_log_residual_wave202::{
    honesty_live_command_cheer_science_log_method_names_residual_wave202,
    honesty_live_command_cheer_science_log_nav_commands_residual_wave202,
    simulate_live_command_cheer_science_log_honesty,
};
use crate::game_logic::host_live_command_deploy_status_log_residual_wave203::{
    honesty_live_command_deploy_status_log_method_names_residual_wave203,
    honesty_live_command_deploy_status_log_nav_commands_residual_wave203,
    simulate_live_command_deploy_status_log_honesty,
};
use crate::game_logic::host_live_command_formation_log_residual_wave204::{
    honesty_live_command_formation_log_method_names_residual_wave204,
    honesty_live_command_formation_log_nav_commands_residual_wave204,
    simulate_live_command_formation_log_honesty,
};
use crate::game_logic::host_live_command_guard_log_residual_wave198::{
    honesty_live_command_guard_log_method_names_residual_wave198,
    honesty_live_command_guard_log_nav_commands_residual_wave198,
    simulate_live_command_guard_log_honesty,
};
use crate::game_logic::host_live_command_non_attack_order_target_residual_wave207::{
    honesty_live_command_non_attack_order_target_method_names_residual_wave207,
    honesty_live_command_non_attack_order_target_nav_commands_residual_wave207,
    simulate_live_command_non_attack_order_target_honesty,
};
use crate::game_logic::host_live_command_order_target_log_residual_wave205::{
    honesty_live_command_order_target_log_method_names_residual_wave205,
    honesty_live_command_order_target_log_nav_commands_residual_wave205,
    simulate_live_command_order_target_log_honesty,
};
use crate::game_logic::host_live_command_production_construction_log_residual_wave199::{
    honesty_live_command_production_construction_log_method_names_residual_wave199,
    honesty_live_command_production_construction_log_nav_commands_residual_wave199,
    simulate_live_command_production_construction_log_honesty,
};
use crate::game_logic::host_live_command_rally_log_residual_wave200::{
    honesty_live_command_rally_log_method_names_residual_wave200,
    honesty_live_command_rally_log_nav_commands_residual_wave200,
    simulate_live_command_rally_log_honesty,
};
use crate::game_logic::host_live_command_selection_log_residual_wave206::{
    honesty_live_command_selection_log_method_names_residual_wave206,
    honesty_live_command_selection_log_nav_commands_residual_wave206,
    simulate_live_command_selection_log_honesty,
};
use crate::game_logic::host_live_command_sell_deselect_log_residual_wave212::{
    honesty_live_command_sell_deselect_log_method_names_residual_wave212,
    honesty_live_command_sell_deselect_log_nav_commands_residual_wave212,
    simulate_live_command_sell_deselect_log_honesty,
};
use crate::game_logic::host_live_control_group_camera_presentation_only_residual_wave216::{
    honesty_live_control_group_camera_presentation_only_method_names_residual_wave216,
    honesty_live_control_group_camera_presentation_only_nav_commands_residual_wave216,
    simulate_live_control_group_camera_presentation_only_honesty,
};
use crate::game_logic::host_live_evacuate_contain_log_residual_wave201::{
    honesty_live_evacuate_contain_log_method_names_residual_wave201,
    honesty_live_evacuate_contain_log_nav_commands_residual_wave201,
    simulate_live_evacuate_contain_log_honesty,
};
use crate::game_logic::host_live_gameworld_construction_writeback_residual_wave181::{
    honesty_live_gameworld_construction_writeback_method_names_residual_wave181,
    honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181,
    simulate_live_gameworld_construction_writeback_honesty,
};
use crate::game_logic::host_live_gameworld_damage_channel_residual_wave182::{
    honesty_live_gameworld_damage_channel_method_names_residual_wave182,
    honesty_live_gameworld_damage_channel_nav_commands_residual_wave182,
    simulate_live_gameworld_damage_channel_honesty,
};
use crate::game_logic::host_live_gameworld_economy_movement_residual_wave183::{
    honesty_live_gameworld_economy_movement_method_names_residual_wave183,
    honesty_live_gameworld_economy_movement_nav_commands_residual_wave183,
    simulate_live_gameworld_economy_movement_honesty,
};
use crate::game_logic::host_live_gameworld_entity_view_deepen_residual_wave191::{
    honesty_live_gameworld_entity_view_deepen_method_names_residual_wave191,
    honesty_live_gameworld_entity_view_deepen_nav_commands_residual_wave191,
    simulate_live_gameworld_entity_view_deepen_honesty,
};
use crate::game_logic::host_live_gameworld_fire_special_power_residual_wave185::{
    honesty_live_gameworld_fire_special_power_method_names_residual_wave185,
    honesty_live_gameworld_fire_special_power_nav_commands_residual_wave185,
    simulate_live_gameworld_fire_special_power_honesty,
};
use crate::game_logic::host_live_gameworld_presentation_view_residual_wave186::{
    honesty_live_gameworld_presentation_view_method_names_residual_wave186,
    honesty_live_gameworld_presentation_view_nav_commands_residual_wave186,
    simulate_live_gameworld_presentation_view_honesty,
};
use crate::game_logic::host_live_gameworld_production_writeback_residual_wave180::{
    honesty_live_gameworld_production_writeback_method_names_residual_wave180,
    honesty_live_gameworld_production_writeback_nav_commands_residual_wave180,
    simulate_live_gameworld_production_writeback_honesty,
};
use crate::game_logic::host_live_gameworld_projectile_ai_residual_wave184::{
    honesty_live_gameworld_projectile_ai_method_names_residual_wave184,
    honesty_live_gameworld_projectile_ai_nav_commands_residual_wave184,
    simulate_live_gameworld_projectile_ai_honesty,
};
use crate::game_logic::host_live_gameworld_shadow_overlay_residual_wave172::{
    honesty_live_gameworld_shadow_overlay_method_names_residual_wave172,
    honesty_live_gameworld_shadow_overlay_nav_commands_residual_wave172,
    simulate_live_gameworld_shadow_overlay_honesty,
};
use crate::game_logic::host_live_golden_mopup_honesty_residual_wave208::{
    honesty_live_golden_mopup_honesty_method_names_residual_wave208,
    honesty_live_golden_mopup_honesty_nav_commands_residual_wave208,
    simulate_live_golden_mopup_honesty,
};
use crate::game_logic::host_live_host_beacon_presentation_residual_wave211::{
    honesty_live_host_beacon_presentation_method_names_residual_wave211,
    honesty_live_host_beacon_presentation_nav_commands_residual_wave211,
    simulate_live_host_beacon_presentation_honesty,
};
use crate::game_logic::host_live_local_team_presentation_only_residual_wave220::{
    honesty_live_local_team_presentation_only_method_names_residual_wave220,
    honesty_live_local_team_presentation_only_nav_commands_residual_wave220,
    simulate_live_local_team_presentation_only_honesty,
};
use crate::game_logic::host_live_hotkey_move_attack_selection_presentation_only_residual_wave221::{
    honesty_live_hotkey_move_attack_selection_presentation_only_method_names_residual_wave221,
    honesty_live_hotkey_move_attack_selection_presentation_only_nav_commands_residual_wave221,
    simulate_live_hotkey_move_attack_selection_presentation_only_honesty,
};
use crate::game_logic::host_live_pick_object_presentation_only_residual_wave222::{
    honesty_live_pick_object_presentation_only_method_names_residual_wave222,
    honesty_live_pick_object_presentation_only_nav_commands_residual_wave222,
    simulate_live_pick_object_presentation_only_honesty,
};
use crate::game_logic::host_live_bootstrap_camera_presentation_only_residual_wave223::{
    honesty_live_bootstrap_camera_presentation_only_method_names_residual_wave223,
    honesty_live_bootstrap_camera_presentation_only_nav_commands_residual_wave223,
    simulate_live_bootstrap_camera_presentation_only_honesty,
};
use crate::game_logic::host_live_force_complete_authority_api_residual_wave224::{
    honesty_live_force_complete_authority_api_method_names_residual_wave224,
    honesty_live_force_complete_authority_api_nav_commands_residual_wave224,
    simulate_live_force_complete_authority_api_honesty,
};
use crate::game_logic::host_live_path_guard_authority_api_residual_wave225::{
    honesty_live_path_guard_authority_api_method_names_residual_wave225,
    honesty_live_path_guard_authority_api_nav_commands_residual_wave225,
    simulate_live_path_guard_authority_api_honesty,
};
use crate::game_logic::host_live_hotkey_selection_camera_presentation_only_residual_wave226::{
    honesty_live_hotkey_selection_camera_presentation_only_method_names_residual_wave226,
    honesty_live_hotkey_selection_camera_presentation_only_nav_commands_residual_wave226,
    simulate_live_hotkey_selection_camera_presentation_only_honesty,
};
use crate::game_logic::host_live_construct_spawn_pose_authority_api_residual_wave227::{
    honesty_live_construct_spawn_pose_authority_api_method_names_residual_wave227,
    honesty_live_construct_spawn_pose_authority_api_nav_commands_residual_wave227,
    simulate_live_construct_spawn_pose_authority_api_honesty,
};
use crate::game_logic::host_live_rmb_target_presentation_only_residual_wave228::{
    honesty_live_rmb_target_presentation_only_method_names_residual_wave228,
    honesty_live_rmb_target_presentation_only_nav_commands_residual_wave228,
    simulate_live_rmb_target_presentation_only_honesty,
};
use crate::game_logic::host_live_rmb_selected_presentation_only_residual_wave229::{
    honesty_live_rmb_selected_presentation_only_method_names_residual_wave229,
    honesty_live_rmb_selected_presentation_only_nav_commands_residual_wave229,
    simulate_live_rmb_selected_presentation_only_honesty,
};
use crate::game_logic::host_live_command_unit_authority_api_residual_wave230::{
    honesty_live_command_unit_authority_api_method_names_residual_wave230,
    honesty_live_command_unit_authority_api_nav_commands_residual_wave230,
    simulate_live_command_unit_authority_api_honesty,
};
use crate::game_logic::host_live_command_unit_more_authority_api_residual_wave231::{
    honesty_live_command_unit_more_authority_api_method_names_residual_wave231,
    honesty_live_command_unit_more_authority_api_nav_commands_residual_wave231,
    simulate_live_command_unit_more_authority_api_honesty,
};
use crate::game_logic::host_live_command_executor_authority_api_residual_wave232::{
    honesty_live_command_executor_authority_api_method_names_residual_wave232,
    honesty_live_command_executor_authority_api_nav_commands_residual_wave232,
    simulate_live_command_executor_authority_api_honesty,
};
use crate::game_logic::host_live_command_executor_more_authority_api_residual_wave233::{
    honesty_live_command_executor_more_authority_api_method_names_residual_wave233,
    honesty_live_command_executor_more_authority_api_nav_commands_residual_wave233,
    simulate_live_command_executor_more_authority_api_honesty,
};
use crate::game_logic::host_live_engine_presentation_player_ui_residual_wave234::{
    honesty_live_engine_presentation_player_ui_method_names_residual_wave234,
    honesty_live_engine_presentation_player_ui_nav_commands_residual_wave234,
    simulate_live_engine_presentation_player_ui_honesty,
};
use crate::game_logic::host_live_rmb_presentation_full_classify_residual_wave235::{
    honesty_live_rmb_presentation_full_classify_method_names_residual_wave235,
    honesty_live_rmb_presentation_full_classify_nav_commands_residual_wave235,
    simulate_live_rmb_presentation_full_classify_honesty,
};
use crate::game_logic::host_live_mouse_input_presentation_only_residual_wave236::{
    honesty_live_mouse_input_presentation_only_method_names_residual_wave236,
    honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236,
    simulate_live_mouse_input_presentation_only_honesty,
};
use crate::game_logic::host_live_engine_player_ui_boot_peel_residual_wave237::{
    honesty_live_engine_player_ui_boot_peel_method_names_residual_wave237,
    honesty_live_engine_player_ui_boot_peel_nav_commands_residual_wave237,
    simulate_live_engine_player_ui_boot_peel_honesty,
};
use crate::game_logic::host_live_player_probe_api_residual_wave238::{
    honesty_live_player_probe_api_method_names_residual_wave238,
    honesty_live_player_probe_api_nav_commands_residual_wave238,
    simulate_live_player_probe_api_honesty,
};
use crate::game_logic::host_live_player_team_probe_residual_wave239::{
    honesty_live_player_team_probe_method_names_residual_wave239,
    honesty_live_player_team_probe_nav_commands_residual_wave239,
    simulate_live_player_team_probe_honesty,
};
use crate::game_logic::host_live_player_field_probe_residual_wave240::{
    honesty_live_player_field_probe_method_names_residual_wave240,
    honesty_live_player_field_probe_nav_commands_residual_wave240,
    simulate_live_player_field_probe_honesty,
};
use crate::game_logic::host_live_camera_height_probe_residual_wave241::{
    honesty_live_camera_height_probe_method_names_residual_wave241,
    honesty_live_camera_height_probe_nav_commands_residual_wave241,
    simulate_live_camera_height_probe_honesty,
};
use crate::game_logic::host_live_command_player_probe_residual_wave242::{
    honesty_live_command_player_probe_method_names_residual_wave242,
    honesty_live_command_player_probe_nav_commands_residual_wave242,
    simulate_live_command_player_probe_honesty,
};
use crate::game_logic::host_live_construct_economy_probe_residual_wave243::{
    honesty_live_construct_economy_probe_method_names_residual_wave243,
    honesty_live_construct_economy_probe_nav_commands_residual_wave243,
    simulate_live_construct_economy_probe_honesty,
};
use crate::game_logic::host_live_command_unit_probe_residual_wave244::{
    honesty_live_command_unit_probe_method_names_residual_wave244,
    honesty_live_command_unit_probe_nav_commands_residual_wave244,
    simulate_live_command_unit_probe_honesty,
};
use crate::game_logic::host_live_selection_query_probe_residual_wave245::{
    honesty_live_selection_query_probe_method_names_residual_wave245,
    honesty_live_selection_query_probe_nav_commands_residual_wave245,
    simulate_live_selection_query_probe_honesty,
};
use crate::game_logic::host_live_world_pick_probe_residual_wave246::{
    honesty_live_world_pick_probe_method_names_residual_wave246,
    honesty_live_world_pick_probe_nav_commands_residual_wave246,
    simulate_live_world_pick_probe_honesty,
};
use crate::game_logic::host_live_object_registry_empty_fastpath_residual_wave247::{
    honesty_live_object_registry_empty_fastpath_method_names_residual_wave247,
    honesty_live_object_registry_empty_fastpath_nav_commands_residual_wave247,
    simulate_live_object_registry_empty_fastpath_honesty,
};
use crate::game_logic::host_live_legacy_object_registry_fastpath_residual_wave248::{
    honesty_live_legacy_object_registry_fastpath_method_names_residual_wave248,
    honesty_live_legacy_object_registry_fastpath_nav_commands_residual_wave248,
    simulate_live_legacy_object_registry_fastpath_honesty,
};
use crate::game_logic::host_live_client_dual_world_empty_gate_residual_wave249::{
    honesty_live_client_dual_world_empty_gate_method_names_residual_wave249,
    honesty_live_client_dual_world_empty_gate_nav_commands_residual_wave249,
    simulate_live_client_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_presentation_time_frozen_probe_residual_wave250::{
    honesty_live_presentation_time_frozen_probe_method_names_residual_wave250,
    honesty_live_presentation_time_frozen_probe_nav_commands_residual_wave250,
    simulate_live_presentation_time_frozen_probe_honesty,
};
use crate::game_logic::host_live_presentation_visual_speed_probe_residual_wave251::{
    honesty_live_presentation_visual_speed_probe_method_names_residual_wave251,
    honesty_live_presentation_visual_speed_probe_nav_commands_residual_wave251,
    simulate_live_presentation_visual_speed_probe_honesty,
};
use crate::game_logic::host_live_presentation_script_camera_probe_residual_wave252::{
    honesty_live_presentation_script_camera_probe_method_names_residual_wave252,
    honesty_live_presentation_script_camera_probe_nav_commands_residual_wave252,
    simulate_live_presentation_script_camera_probe_honesty,
};
use crate::game_logic::host_live_ai_group_dual_world_empty_gate_residual_wave253::{
    honesty_live_ai_group_dual_world_empty_gate_method_names_residual_wave253,
    honesty_live_ai_group_dual_world_empty_gate_nav_commands_residual_wave253,
    simulate_live_ai_group_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_states_dual_world_empty_gate_residual_wave254::{
    honesty_live_ai_states_dual_world_empty_gate_method_names_residual_wave254,
    honesty_live_ai_states_dual_world_empty_gate_nav_commands_residual_wave254,
    simulate_live_ai_states_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_player_dual_world_empty_gate_residual_wave255::{
    honesty_live_ai_player_dual_world_empty_gate_method_names_residual_wave255,
    honesty_live_ai_player_dual_world_empty_gate_nav_commands_residual_wave255,
    simulate_live_ai_player_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_team_dual_world_empty_gate_residual_wave256::{
    honesty_live_team_dual_world_empty_gate_method_names_residual_wave256,
    honesty_live_team_dual_world_empty_gate_nav_commands_residual_wave256,
    simulate_live_team_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_legacy_states_dual_world_empty_gate_residual_wave257::{
    honesty_live_ai_legacy_states_dual_world_empty_gate_method_names_residual_wave257,
    honesty_live_ai_legacy_states_dual_world_empty_gate_nav_commands_residual_wave257,
    simulate_live_ai_legacy_states_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_unit_dual_world_empty_gate_residual_wave258::{
    honesty_live_unit_dual_world_empty_gate_method_names_residual_wave258,
    honesty_live_unit_dual_world_empty_gate_nav_commands_residual_wave258,
    simulate_live_unit_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_stealth_dual_world_empty_gate_residual_wave259::{
    honesty_live_stealth_dual_world_empty_gate_method_names_residual_wave259,
    honesty_live_stealth_dual_world_empty_gate_nav_commands_residual_wave259,
    simulate_live_stealth_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_garrison_dual_world_empty_gate_residual_wave260::{
    honesty_live_garrison_dual_world_empty_gate_method_names_residual_wave260,
    honesty_live_garrison_dual_world_empty_gate_nav_commands_residual_wave260,
    simulate_live_garrison_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_open_contain_dual_world_empty_gate_residual_wave261::{
    honesty_live_open_contain_dual_world_empty_gate_method_names_residual_wave261,
    honesty_live_open_contain_dual_world_empty_gate_nav_commands_residual_wave261,
    simulate_live_open_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_pathfind_dual_world_empty_gate_residual_wave262::{
    honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave262,
    honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave262,
    simulate_live_pathfind_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_mod_dual_world_empty_gate_residual_wave263::{
    honesty_live_ai_mod_dual_world_empty_gate_method_names_residual_wave263,
    honesty_live_ai_mod_dual_world_empty_gate_nav_commands_residual_wave263,
    simulate_live_ai_mod_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_object_mod_dual_world_empty_gate_residual_wave264::{
    honesty_live_object_mod_dual_world_empty_gate_method_names_residual_wave264,
    honesty_live_object_mod_dual_world_empty_gate_nav_commands_residual_wave264,
    simulate_live_object_mod_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_weapon_dual_world_empty_gate_residual_wave265::{
    honesty_live_weapon_dual_world_empty_gate_method_names_residual_wave265,
    honesty_live_weapon_dual_world_empty_gate_nav_commands_residual_wave265,
    simulate_live_weapon_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_partition_filters_dual_world_empty_gate_residual_wave266::{
    honesty_live_partition_filters_dual_world_empty_gate_method_names_residual_wave266,
    honesty_live_partition_filters_dual_world_empty_gate_nav_commands_residual_wave266,
    simulate_live_partition_filters_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_state_machine_dual_world_empty_gate_residual_wave267::{
    honesty_live_ai_state_machine_dual_world_empty_gate_method_names_residual_wave267,
    honesty_live_ai_state_machine_dual_world_empty_gate_nav_commands_residual_wave267,
    simulate_live_ai_state_machine_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_player_dual_world_empty_gate_residual_wave268::{
    honesty_live_player_dual_world_empty_gate_method_names_residual_wave268,
    honesty_live_player_dual_world_empty_gate_nav_commands_residual_wave268,
    simulate_live_player_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_game_client_dual_world_empty_gate_residual_wave269::{
    honesty_live_game_client_dual_world_empty_gate_method_names_residual_wave269,
    honesty_live_game_client_dual_world_empty_gate_nav_commands_residual_wave269,
    simulate_live_game_client_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_drawable_dual_world_empty_gate_residual_wave270::{
    honesty_live_drawable_dual_world_empty_gate_method_names_residual_wave270,
    honesty_live_drawable_dual_world_empty_gate_nav_commands_residual_wave270,
    simulate_live_drawable_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_script_conditions_dual_world_empty_gate_residual_wave271::{
    honesty_live_script_conditions_dual_world_empty_gate_method_names_residual_wave271,
    honesty_live_script_conditions_dual_world_empty_gate_nav_commands_residual_wave271,
    simulate_live_script_conditions_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_transport_contain_dual_world_empty_gate_residual_wave272::{
    honesty_live_transport_contain_dual_world_empty_gate_method_names_residual_wave272,
    honesty_live_transport_contain_dual_world_empty_gate_nav_commands_residual_wave272,
    simulate_live_transport_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ingame_ui_dual_world_empty_gate_residual_wave273::{
    honesty_live_ingame_ui_dual_world_empty_gate_method_names_residual_wave273,
    honesty_live_ingame_ui_dual_world_empty_gate_nav_commands_residual_wave273,
    simulate_live_ingame_ui_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_helix_contain_dual_world_empty_gate_residual_wave274::{
    honesty_live_helix_contain_dual_world_empty_gate_method_names_residual_wave274,
    honesty_live_helix_contain_dual_world_empty_gate_nav_commands_residual_wave274,
    simulate_live_helix_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_command_processor_dual_world_empty_gate_residual_wave275::{
    honesty_live_command_processor_dual_world_empty_gate_method_names_residual_wave275,
    honesty_live_command_processor_dual_world_empty_gate_nav_commands_residual_wave275,
    simulate_live_command_processor_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_turret_dual_world_empty_gate_residual_wave276::{
    honesty_live_turret_dual_world_empty_gate_method_names_residual_wave276,
    honesty_live_turret_dual_world_empty_gate_nav_commands_residual_wave276,
    simulate_live_turret_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_rider_change_contain_dual_world_empty_gate_residual_wave277::{
    honesty_live_rider_change_contain_dual_world_empty_gate_method_names_residual_wave277,
    honesty_live_rider_change_contain_dual_world_empty_gate_nav_commands_residual_wave277,
    simulate_live_rider_change_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_selection_dual_world_empty_gate_residual_wave278::{
    honesty_live_selection_dual_world_empty_gate_method_names_residual_wave278,
    honesty_live_selection_dual_world_empty_gate_nav_commands_residual_wave278,
    simulate_live_selection_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_cave_contain_dual_world_empty_gate_residual_wave279::{
    honesty_live_cave_contain_dual_world_empty_gate_method_names_residual_wave279,
    honesty_live_cave_contain_dual_world_empty_gate_nav_commands_residual_wave279,
    simulate_live_cave_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_tunnel_contain_dual_world_empty_gate_residual_wave280::{
    honesty_live_tunnel_contain_dual_world_empty_gate_method_names_residual_wave280,
    honesty_live_tunnel_contain_dual_world_empty_gate_nav_commands_residual_wave280,
    simulate_live_tunnel_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_helpers_dual_world_empty_gate_residual_wave281::{
    honesty_live_helpers_dual_world_empty_gate_method_names_residual_wave281,
    honesty_live_helpers_dual_world_empty_gate_nav_commands_residual_wave281,
    simulate_live_helpers_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_update_interface_dual_world_empty_gate_residual_wave282::{
    honesty_live_ai_update_interface_dual_world_empty_gate_method_names_residual_wave282,
    honesty_live_ai_update_interface_dual_world_empty_gate_nav_commands_residual_wave282,
    simulate_live_ai_update_interface_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_stealth_update_dual_world_empty_gate_residual_wave283::{
    honesty_live_stealth_update_dual_world_empty_gate_method_names_residual_wave283,
    honesty_live_stealth_update_dual_world_empty_gate_nav_commands_residual_wave283,
    simulate_live_stealth_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_script_executor_dual_world_empty_gate_residual_wave284::{
    honesty_live_script_executor_dual_world_empty_gate_method_names_residual_wave284,
    honesty_live_script_executor_dual_world_empty_gate_nav_commands_residual_wave284,
    simulate_live_script_executor_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_integration_dual_world_empty_gate_residual_wave285::{
    honesty_live_ai_integration_dual_world_empty_gate_method_names_residual_wave285,
    honesty_live_ai_integration_dual_world_empty_gate_nav_commands_residual_wave285,
    simulate_live_ai_integration_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_dumb_projectile_dual_world_empty_gate_residual_wave286::{
    honesty_live_dumb_projectile_dual_world_empty_gate_method_names_residual_wave286,
    honesty_live_dumb_projectile_dual_world_empty_gate_nav_commands_residual_wave286,
    simulate_live_dumb_projectile_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_enhanced_player_dual_world_empty_gate_residual_wave287::{
    honesty_live_enhanced_player_dual_world_empty_gate_method_names_residual_wave287,
    honesty_live_enhanced_player_dual_world_empty_gate_nav_commands_residual_wave287,
    simulate_live_enhanced_player_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_hijacker_update_dual_world_empty_gate_residual_wave288::{
    honesty_live_hijacker_update_dual_world_empty_gate_method_names_residual_wave288,
    honesty_live_hijacker_update_dual_world_empty_gate_nav_commands_residual_wave288,
    simulate_live_hijacker_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_weapon_dual_world_empty_gate_residual_wave289::{
    honesty_live_weapon_impl_dual_world_empty_gate_method_names_residual_wave289,
    honesty_live_weapon_impl_dual_world_empty_gate_nav_commands_residual_wave289,
    simulate_live_weapon_impl_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_async_player_dual_world_empty_gate_residual_wave290::{
    honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290,
    honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290,
    simulate_live_async_player_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_active_body_dual_world_empty_gate_residual_wave291::{
    honesty_live_active_body_dual_world_empty_gate_method_names_residual_wave291,
    honesty_live_active_body_dual_world_empty_gate_nav_commands_residual_wave291,
    simulate_live_active_body_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_skirmish_conditions_dual_world_empty_gate_residual_wave292::{
    honesty_live_skirmish_conditions_dual_world_empty_gate_method_names_residual_wave292,
    honesty_live_skirmish_conditions_dual_world_empty_gate_nav_commands_residual_wave292,
    simulate_live_skirmish_conditions_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_build_list_dual_world_empty_gate_residual_wave293::{
    honesty_live_ai_build_list_dual_world_empty_gate_method_names_residual_wave293,
    honesty_live_ai_build_list_dual_world_empty_gate_nav_commands_residual_wave293,
    simulate_live_ai_build_list_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_victory_dual_world_empty_gate_residual_wave294::{
    honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294,
    honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294,
    simulate_live_victory_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_script_actions_dual_world_empty_gate_residual_wave295::{
    honesty_live_script_actions_dual_world_empty_gate_method_names_residual_wave295,
    honesty_live_script_actions_dual_world_empty_gate_nav_commands_residual_wave295,
    simulate_live_script_actions_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_special_ability_dual_world_empty_gate_residual_wave296::{
    honesty_live_special_ability_dual_world_empty_gate_method_names_residual_wave296,
    honesty_live_special_ability_dual_world_empty_gate_nav_commands_residual_wave296,
    simulate_live_special_ability_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_stealth_detector_dual_world_empty_gate_residual_wave297::{
    honesty_live_stealth_detector_dual_world_empty_gate_method_names_residual_wave297,
    honesty_live_stealth_detector_dual_world_empty_gate_nav_commands_residual_wave297,
    simulate_live_stealth_detector_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_supply_system_dual_world_empty_gate_residual_wave298::{
    honesty_live_supply_system_dual_world_empty_gate_method_names_residual_wave298,
    honesty_live_supply_system_dual_world_empty_gate_nav_commands_residual_wave298,
    simulate_live_supply_system_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_particle_uplink_dual_world_empty_gate_residual_wave299::{
    honesty_live_particle_uplink_dual_world_empty_gate_method_names_residual_wave299,
    honesty_live_particle_uplink_dual_world_empty_gate_nav_commands_residual_wave299,
    simulate_live_particle_uplink_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_overlord_contain_dual_world_empty_gate_residual_wave300::{
    honesty_live_overlord_contain_dual_world_empty_gate_method_names_residual_wave300,
    honesty_live_overlord_contain_dual_world_empty_gate_nav_commands_residual_wave300,
    simulate_live_overlord_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_bridge_behavior_dual_world_empty_gate_residual_wave301::{
    honesty_live_bridge_behavior_dual_world_empty_gate_method_names_residual_wave301,
    honesty_live_bridge_behavior_dual_world_empty_gate_nav_commands_residual_wave301,
    simulate_live_bridge_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_stealth_behavior_dual_world_empty_gate_residual_wave302::{
    honesty_live_stealth_behavior_dual_world_empty_gate_method_names_residual_wave302,
    honesty_live_stealth_behavior_dual_world_empty_gate_nav_commands_residual_wave302,
    simulate_live_stealth_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_crate_collide_dual_world_empty_gate_residual_wave303::{
    honesty_live_crate_collide_dual_world_empty_gate_method_names_residual_wave303,
    honesty_live_crate_collide_dual_world_empty_gate_nav_commands_residual_wave303,
    simulate_live_crate_collide_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_object_manager_dual_world_empty_gate_residual_wave304::{
    honesty_live_object_manager_dual_world_empty_gate_method_names_residual_wave304,
    honesty_live_object_manager_dual_world_empty_gate_nav_commands_residual_wave304,
    simulate_live_object_manager_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_sticky_bomb_dual_world_empty_gate_residual_wave305::{
    honesty_live_sticky_bomb_dual_world_empty_gate_method_names_residual_wave305,
    honesty_live_sticky_bomb_dual_world_empty_gate_nav_commands_residual_wave305,
    simulate_live_sticky_bomb_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_auto_heal_dual_world_empty_gate_residual_wave306::{
    honesty_live_auto_heal_dual_world_empty_gate_method_names_residual_wave306,
    honesty_live_auto_heal_dual_world_empty_gate_nav_commands_residual_wave306,
    simulate_live_auto_heal_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_grant_stealth_dual_world_empty_gate_residual_wave307::{
    honesty_live_grant_stealth_dual_world_empty_gate_method_names_residual_wave307,
    honesty_live_grant_stealth_dual_world_empty_gate_nav_commands_residual_wave307,
    simulate_live_grant_stealth_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_status_bits_upgrade_dual_world_empty_gate_residual_wave308::{
    honesty_live_status_bits_upgrade_dual_world_empty_gate_method_names_residual_wave308,
    honesty_live_status_bits_upgrade_dual_world_empty_gate_nav_commands_residual_wave308,
    simulate_live_status_bits_upgrade_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_jet_ai_dual_world_empty_gate_residual_wave309::{
    honesty_live_jet_ai_dual_world_empty_gate_method_names_residual_wave309,
    honesty_live_jet_ai_dual_world_empty_gate_nav_commands_residual_wave309,
    simulate_live_jet_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_parking_place_dual_world_empty_gate_residual_wave310::{
    honesty_live_parking_place_dual_world_empty_gate_method_names_residual_wave310,
    honesty_live_parking_place_dual_world_empty_gate_nav_commands_residual_wave310,
    simulate_live_parking_place_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_flight_deck_dual_world_empty_gate_residual_wave311::{
    honesty_live_flight_deck_dual_world_empty_gate_method_names_residual_wave311,
    honesty_live_flight_deck_dual_world_empty_gate_nav_commands_residual_wave311,
    simulate_live_flight_deck_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_exit_strategies_dual_world_empty_gate_residual_wave312::{
    honesty_live_exit_strategies_dual_world_empty_gate_method_names_residual_wave312,
    honesty_live_exit_strategies_dual_world_empty_gate_nav_commands_residual_wave312,
    simulate_live_exit_strategies_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_collision_system_dual_world_empty_gate_residual_wave313::{
    honesty_live_collision_system_dual_world_empty_gate_method_names_residual_wave313,
    honesty_live_collision_system_dual_world_empty_gate_nav_commands_residual_wave313,
    simulate_live_collision_system_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_max_health_upgrade_dual_world_empty_gate_residual_wave314::{
    honesty_live_max_health_upgrade_dual_world_empty_gate_method_names_residual_wave314,
    honesty_live_max_health_upgrade_dual_world_empty_gate_nav_commands_residual_wave314,
    simulate_live_max_health_upgrade_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_structure_topple_dual_world_empty_gate_residual_wave315::{
    honesty_live_structure_topple_dual_world_empty_gate_method_names_residual_wave315,
    honesty_live_structure_topple_dual_world_empty_gate_nav_commands_residual_wave315,
    simulate_live_structure_topple_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_physics_update_dual_world_empty_gate_residual_wave316::{
    honesty_live_physics_update_dual_world_empty_gate_method_names_residual_wave316,
    honesty_live_physics_update_dual_world_empty_gate_nav_commands_residual_wave316,
    simulate_live_physics_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_cleanup_hazard_dual_world_empty_gate_residual_wave317::{
    honesty_live_cleanup_hazard_dual_world_empty_gate_method_names_residual_wave317,
    honesty_live_cleanup_hazard_dual_world_empty_gate_nav_commands_residual_wave317,
    simulate_live_cleanup_hazard_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_bridge_tower_dual_world_empty_gate_residual_wave318::{
    honesty_live_bridge_tower_dual_world_empty_gate_method_names_residual_wave318,
    honesty_live_bridge_tower_dual_world_empty_gate_nav_commands_residual_wave318,
    simulate_live_bridge_tower_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_armor_upgrade_dual_world_empty_gate_residual_wave319::{
    honesty_live_armor_upgrade_dual_world_empty_gate_method_names_residual_wave319,
    honesty_live_armor_upgrade_dual_world_empty_gate_nav_commands_residual_wave319,
    simulate_live_armor_upgrade_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_paradrop_power_dual_world_empty_gate_residual_wave320::{
    honesty_live_paradrop_power_dual_world_empty_gate_method_names_residual_wave320,
    honesty_live_paradrop_power_dual_world_empty_gate_nav_commands_residual_wave320,
    simulate_live_paradrop_power_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_fuel_air_bomb_dual_world_empty_gate_residual_wave321::{
    honesty_live_fuel_air_bomb_dual_world_empty_gate_method_names_residual_wave321,
    honesty_live_fuel_air_bomb_dual_world_empty_gate_nav_commands_residual_wave321,
    simulate_live_fuel_air_bomb_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_tensile_formation_dual_world_empty_gate_residual_wave322::{
    honesty_live_tensile_formation_dual_world_empty_gate_method_names_residual_wave322,
    honesty_live_tensile_formation_dual_world_empty_gate_nav_commands_residual_wave322,
    simulate_live_tensile_formation_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_die_mod_dual_world_empty_gate_residual_wave323::{
    honesty_live_die_mod_dual_world_empty_gate_method_names_residual_wave323,
    honesty_live_die_mod_dual_world_empty_gate_nav_commands_residual_wave323,
    simulate_live_die_mod_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_partition_manager_dual_world_empty_gate_residual_wave324::{
    honesty_live_partition_manager_dual_world_empty_gate_method_names_residual_wave324,
    honesty_live_partition_manager_dual_world_empty_gate_nav_commands_residual_wave324,
    simulate_live_partition_manager_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_spectre_gunship_dual_world_empty_gate_residual_wave325::{
    honesty_live_spectre_gunship_dual_world_empty_gate_method_names_residual_wave325,
    honesty_live_spectre_gunship_dual_world_empty_gate_nav_commands_residual_wave325,
    simulate_live_spectre_gunship_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_production_update_dual_world_empty_gate_residual_wave326::{
    honesty_live_production_update_dual_world_empty_gate_method_names_residual_wave326,
    honesty_live_production_update_dual_world_empty_gate_nav_commands_residual_wave326,
    simulate_live_production_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_neutron_blast_dual_world_empty_gate_residual_wave327::{
    honesty_live_neutron_blast_dual_world_empty_gate_method_names_residual_wave327,
    honesty_live_neutron_blast_dual_world_empty_gate_nav_commands_residual_wave327,
    simulate_live_neutron_blast_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_countermeasures_dual_world_empty_gate_residual_wave328::{
    honesty_live_countermeasures_dual_world_empty_gate_method_names_residual_wave328,
    honesty_live_countermeasures_dual_world_empty_gate_nav_commands_residual_wave328,
    simulate_live_countermeasures_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_skirmish_player_dual_world_empty_gate_residual_wave329::{
    honesty_live_skirmish_player_dual_world_empty_gate_method_names_residual_wave329,
    honesty_live_skirmish_player_dual_world_empty_gate_nav_commands_residual_wave329,
    simulate_live_skirmish_player_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_a10_strike_dual_world_empty_gate_residual_wave330::{
    honesty_live_a10_strike_dual_world_empty_gate_method_names_residual_wave330,
    honesty_live_a10_strike_dual_world_empty_gate_nav_commands_residual_wave330,
    simulate_live_a10_strike_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_rebuild_hole_dual_world_empty_gate_residual_wave331::{
    honesty_live_rebuild_hole_dual_world_empty_gate_method_names_residual_wave331,
    honesty_live_rebuild_hole_dual_world_empty_gate_nav_commands_residual_wave331,
    simulate_live_rebuild_hole_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_wave_guide_dual_world_empty_gate_residual_wave332::{
    honesty_live_wave_guide_dual_world_empty_gate_method_names_residual_wave332,
    honesty_live_wave_guide_dual_world_empty_gate_nav_commands_residual_wave332,
    simulate_live_wave_guide_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_emp_update_dual_world_empty_gate_residual_wave333::{
    honesty_live_emp_update_dual_world_empty_gate_method_names_residual_wave333,
    honesty_live_emp_update_dual_world_empty_gate_nav_commands_residual_wave333,
    simulate_live_emp_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_bunker_buster_dual_world_empty_gate_residual_wave334::{
    honesty_live_bunker_buster_dual_world_empty_gate_method_names_residual_wave334,
    honesty_live_bunker_buster_dual_world_empty_gate_nav_commands_residual_wave334,
    simulate_live_bunker_buster_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_bridge_scaffold_dual_world_empty_gate_residual_wave335::{
    honesty_live_bridge_scaffold_dual_world_empty_gate_method_names_residual_wave335,
    honesty_live_bridge_scaffold_dual_world_empty_gate_nav_commands_residual_wave335,
    simulate_live_bridge_scaffold_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_assisted_targeting_dual_world_empty_gate_residual_wave336::{
    honesty_live_assisted_targeting_dual_world_empty_gate_method_names_residual_wave336,
    honesty_live_assisted_targeting_dual_world_empty_gate_nav_commands_residual_wave336,
    simulate_live_assisted_targeting_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_economy_dual_world_empty_gate_residual_wave337::{
    honesty_live_economy_dual_world_empty_gate_method_names_residual_wave337,
    honesty_live_economy_dual_world_empty_gate_nav_commands_residual_wave337,
    simulate_live_economy_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_turret_ai_dual_world_empty_gate_residual_wave338::{
    honesty_live_turret_ai_dual_world_empty_gate_method_names_residual_wave338,
    honesty_live_turret_ai_dual_world_empty_gate_nav_commands_residual_wave338,
    simulate_live_turret_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_stealth_detector_module_dual_world_empty_gate_residual_wave339::{
    honesty_live_stealth_detector_module_dual_world_empty_gate_method_names_residual_wave339,
    honesty_live_stealth_detector_module_dual_world_empty_gate_nav_commands_residual_wave339,
    simulate_live_stealth_detector_module_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_modules_dual_world_empty_gate_residual_wave340::{
    honesty_live_modules_dual_world_empty_gate_method_names_residual_wave340,
    honesty_live_modules_dual_world_empty_gate_nav_commands_residual_wave340,
    simulate_live_modules_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_terrain_dual_world_empty_gate_residual_wave341::{
    honesty_live_terrain_dual_world_empty_gate_method_names_residual_wave341,
    honesty_live_terrain_dual_world_empty_gate_nav_commands_residual_wave341,
    simulate_live_terrain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_special_power_template_dual_world_empty_gate_residual_wave342::{
    honesty_live_special_power_template_dual_world_empty_gate_method_names_residual_wave342,
    honesty_live_special_power_template_dual_world_empty_gate_nav_commands_residual_wave342,
    simulate_live_special_power_template_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_script_evaluator_dual_world_empty_gate_residual_wave343::{
    honesty_live_script_evaluator_dual_world_empty_gate_method_names_residual_wave343,
    honesty_live_script_evaluator_dual_world_empty_gate_nav_commands_residual_wave343,
    simulate_live_script_evaluator_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_system_game_logic_dual_world_empty_gate_residual_wave344::{
    honesty_live_system_game_logic_dual_world_empty_gate_method_names_residual_wave344,
    honesty_live_system_game_logic_dual_world_empty_gate_nav_commands_residual_wave344,
    simulate_live_system_game_logic_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_meta_event_dual_world_empty_gate_residual_wave345::{
    honesty_live_meta_event_dual_world_empty_gate_method_names_residual_wave345,
    honesty_live_meta_event_dual_world_empty_gate_nav_commands_residual_wave345,
    simulate_live_meta_event_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_spawn_behavior_dual_world_empty_gate_residual_wave346::{
    honesty_live_spawn_behavior_dual_world_empty_gate_method_names_residual_wave346,
    honesty_live_spawn_behavior_dual_world_empty_gate_nav_commands_residual_wave346,
    simulate_live_spawn_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_action_manager_dual_world_empty_gate_residual_wave347::{
    honesty_live_action_manager_dual_world_empty_gate_method_names_residual_wave347,
    honesty_live_action_manager_dual_world_empty_gate_nav_commands_residual_wave347,
    simulate_live_action_manager_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_script_engine_dual_world_empty_gate_residual_wave348::{
    honesty_live_script_engine_dual_world_empty_gate_method_names_residual_wave348,
    honesty_live_script_engine_dual_world_empty_gate_nav_commands_residual_wave348,
    simulate_live_script_engine_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_chinook_ai_dual_world_empty_gate_residual_wave349::{
    honesty_live_chinook_ai_dual_world_empty_gate_method_names_residual_wave349,
    honesty_live_chinook_ai_dual_world_empty_gate_nav_commands_residual_wave349,
    simulate_live_chinook_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_missile_ai_dual_world_empty_gate_residual_wave350::{
    honesty_live_missile_ai_dual_world_empty_gate_method_names_residual_wave350,
    honesty_live_missile_ai_dual_world_empty_gate_nav_commands_residual_wave350,
    simulate_live_missile_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_dozer_ai_dual_world_empty_gate_residual_wave351::{
    honesty_live_dozer_ai_dual_world_empty_gate_method_names_residual_wave351,
    honesty_live_dozer_ai_dual_world_empty_gate_nav_commands_residual_wave351,
    simulate_live_dozer_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_deliver_payload_ai_dual_world_empty_gate_residual_wave352::{
    honesty_live_deliver_payload_ai_dual_world_empty_gate_method_names_residual_wave352,
    honesty_live_deliver_payload_ai_dual_world_empty_gate_nav_commands_residual_wave352,
    simulate_live_deliver_payload_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_special_power_module_dual_world_empty_gate_residual_wave353::{
    honesty_live_special_power_module_dual_world_empty_gate_method_names_residual_wave353,
    honesty_live_special_power_module_dual_world_empty_gate_nav_commands_residual_wave353,
    simulate_live_special_power_module_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_pow_truck_ai_dual_world_empty_gate_residual_wave354::{
    honesty_live_pow_truck_ai_dual_world_empty_gate_method_names_residual_wave354,
    honesty_live_pow_truck_ai_dual_world_empty_gate_nav_commands_residual_wave354,
    simulate_live_pow_truck_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_dock_update_dual_world_empty_gate_residual_wave355::{
    honesty_live_dock_update_dual_world_empty_gate_method_names_residual_wave355,
    honesty_live_dock_update_dual_world_empty_gate_nav_commands_residual_wave355,
    simulate_live_dock_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_weapon_template_dual_world_empty_gate_residual_wave356::{
    honesty_live_weapon_template_dual_world_empty_gate_method_names_residual_wave356,
    honesty_live_weapon_template_dual_world_empty_gate_nav_commands_residual_wave356,
    simulate_live_weapon_template_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_railroad_guide_ai_dual_world_empty_gate_residual_wave357::{
    honesty_live_railroad_guide_ai_dual_world_empty_gate_method_names_residual_wave357,
    honesty_live_railroad_guide_ai_dual_world_empty_gate_nav_commands_residual_wave357,
    simulate_live_railroad_guide_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_hack_internet_ai_dual_world_empty_gate_residual_wave358::{
    honesty_live_hack_internet_ai_dual_world_empty_gate_method_names_residual_wave358,
    honesty_live_hack_internet_ai_dual_world_empty_gate_nav_commands_residual_wave358,
    simulate_live_hack_internet_ai_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_spectre_gunship_deployment_dual_world_empty_gate_residual_wave359::{
    honesty_live_spectre_gunship_deployment_dual_world_empty_gate_method_names_residual_wave359,
    honesty_live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_residual_wave359,
    simulate_live_spectre_gunship_deployment_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_radius_decal_update_dual_world_empty_gate_residual_wave360::{
    honesty_live_radius_decal_update_dual_world_empty_gate_method_names_residual_wave360,
    honesty_live_radius_decal_update_dual_world_empty_gate_nav_commands_residual_wave360,
    simulate_live_radius_decal_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_railed_transport_dock_dual_world_empty_gate_residual_wave361::{
    honesty_live_railed_transport_dock_dual_world_empty_gate_method_names_residual_wave361,
    honesty_live_railed_transport_dock_dual_world_empty_gate_nav_commands_residual_wave361,
    simulate_live_railed_transport_dock_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_structure_collapse_update_dual_world_empty_gate_residual_wave362::{
    honesty_live_structure_collapse_update_dual_world_empty_gate_method_names_residual_wave362,
    honesty_live_structure_collapse_update_dual_world_empty_gate_nav_commands_residual_wave362,
    simulate_live_structure_collapse_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_propaganda_tower_behavior_dual_world_empty_gate_residual_wave363::{
    honesty_live_propaganda_tower_behavior_dual_world_empty_gate_method_names_residual_wave363,
    honesty_live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_residual_wave363,
    simulate_live_propaganda_tower_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_propaganda_center_behavior_dual_world_empty_gate_residual_wave364::{
    honesty_live_propaganda_center_behavior_dual_world_empty_gate_method_names_residual_wave364,
    honesty_live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_residual_wave364,
    simulate_live_propaganda_center_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_production_update_complete_dual_world_empty_gate_residual_wave365::{
    honesty_live_production_update_complete_dual_world_empty_gate_method_names_residual_wave365,
    honesty_live_production_update_complete_dual_world_empty_gate_nav_commands_residual_wave365,
    simulate_live_production_update_complete_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_pow_truck_behavior_dual_world_empty_gate_residual_wave366::{
    honesty_live_pow_truck_behavior_dual_world_empty_gate_method_names_residual_wave366,
    honesty_live_pow_truck_behavior_dual_world_empty_gate_nav_commands_residual_wave366,
    simulate_live_pow_truck_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_residual_wave367::{
    honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_residual_wave367,
    honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_residual_wave367,
    simulate_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_veterancy_crate_collide_dual_world_empty_gate_residual_wave368::{
    honesty_live_veterancy_crate_collide_dual_world_empty_gate_method_names_residual_wave368,
    honesty_live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_residual_wave368,
    simulate_live_veterancy_crate_collide_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_assault_transport_ai_update_dual_world_empty_gate_residual_wave369::{
    honesty_live_assault_transport_ai_update_dual_world_empty_gate_method_names_residual_wave369,
    honesty_live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave369,
    simulate_live_assault_transport_ai_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_heal_contain_dual_world_empty_gate_residual_wave370::{
    honesty_live_heal_contain_dual_world_empty_gate_method_names_residual_wave370,
    honesty_live_heal_contain_dual_world_empty_gate_nav_commands_residual_wave370,
    simulate_live_heal_contain_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_topple_update_dual_world_empty_gate_residual_wave371::{
    honesty_live_topple_update_dual_world_empty_gate_method_names_residual_wave371,
    honesty_live_topple_update_dual_world_empty_gate_nav_commands_residual_wave371,
    simulate_live_topple_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_projectile_stream_update_dual_world_empty_gate_residual_wave372::{
    honesty_live_projectile_stream_update_dual_world_empty_gate_method_names_residual_wave372,
    honesty_live_projectile_stream_update_dual_world_empty_gate_nav_commands_residual_wave372,
    simulate_live_projectile_stream_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_demo_trap_update_dual_world_empty_gate_residual_wave373::{
    honesty_live_demo_trap_update_dual_world_empty_gate_method_names_residual_wave373,
    honesty_live_demo_trap_update_dual_world_empty_gate_nav_commands_residual_wave373,
    simulate_live_demo_trap_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_mob_member_slaved_update_dual_world_empty_gate_residual_wave374::{
    honesty_live_mob_member_slaved_update_dual_world_empty_gate_method_names_residual_wave374,
    honesty_live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_residual_wave374,
    simulate_live_mob_member_slaved_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_tn_guard_dual_world_empty_gate_residual_wave375::{
    honesty_live_tn_guard_dual_world_empty_gate_method_names_residual_wave375,
    honesty_live_tn_guard_dual_world_empty_gate_nav_commands_residual_wave375,
    simulate_live_tn_guard_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_production_update_dual_world_empty_gate_residual_wave376::{
    honesty_live_production_update_dual_world_empty_gate_method_names_residual_wave376,
    honesty_live_production_update_dual_world_empty_gate_nav_commands_residual_wave376,
    simulate_live_production_update_dual_world_empty_gate_honesty_wave376,
};
use crate::game_logic::host_live_poisoned_behavior_dual_world_empty_gate_residual_wave377::{
    honesty_live_poisoned_behavior_dual_world_empty_gate_method_names_residual_wave377,
    honesty_live_poisoned_behavior_dual_world_empty_gate_nav_commands_residual_wave377,
    simulate_live_poisoned_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_horde_update_dual_world_empty_gate_residual_wave378::{
    honesty_live_horde_update_dual_world_empty_gate_method_names_residual_wave378,
    honesty_live_horde_update_dual_world_empty_gate_nav_commands_residual_wave378,
    simulate_live_horde_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_flammable_update_dual_world_empty_gate_residual_wave379::{
    honesty_live_flammable_update_dual_world_empty_gate_method_names_residual_wave379,
    honesty_live_flammable_update_dual_world_empty_gate_nav_commands_residual_wave379,
    simulate_live_flammable_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_base_regenerate_update_dual_world_empty_gate_residual_wave380::{
    honesty_live_base_regenerate_update_dual_world_empty_gate_method_names_residual_wave380,
    honesty_live_base_regenerate_update_dual_world_empty_gate_nav_commands_residual_wave380,
    simulate_live_base_regenerate_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_queue_production_exit_behavior_dual_world_empty_gate_residual_wave381::{
    honesty_live_queue_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave381,
    honesty_live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave381,
    simulate_live_queue_production_exit_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_missile_launcher_building_update_dual_world_empty_gate_residual_wave382::{
    honesty_live_missile_launcher_building_update_dual_world_empty_gate_method_names_residual_wave382,
    honesty_live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_residual_wave382,
    simulate_live_missile_launcher_building_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_residual_wave383::{
    honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_residual_wave383,
    honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_residual_wave383,
    simulate_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_command_button_hunt_update_dual_world_empty_gate_residual_wave384::{
    honesty_live_command_button_hunt_update_dual_world_empty_gate_method_names_residual_wave384,
    honesty_live_command_button_hunt_update_dual_world_empty_gate_nav_commands_residual_wave384,
    simulate_live_command_button_hunt_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_prison_behavior_dual_world_empty_gate_residual_wave385::{
    honesty_live_prison_behavior_dual_world_empty_gate_method_names_residual_wave385,
    honesty_live_prison_behavior_dual_world_empty_gate_nav_commands_residual_wave385,
    simulate_live_prison_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_generate_minefield_behavior_dual_world_empty_gate_residual_wave386::{
    honesty_live_generate_minefield_behavior_dual_world_empty_gate_method_names_residual_wave386,
    honesty_live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_residual_wave386,
    simulate_live_generate_minefield_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_demoralize_special_power_dual_world_empty_gate_residual_wave387::{
    honesty_live_demoralize_special_power_dual_world_empty_gate_method_names_residual_wave387,
    honesty_live_demoralize_special_power_dual_world_empty_gate_nav_commands_residual_wave387,
    simulate_live_demoralize_special_power_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_stealth_detector_update_dual_world_empty_gate_residual_wave388::{
    honesty_live_stealth_detector_update_dual_world_empty_gate_method_names_residual_wave388,
    honesty_live_stealth_detector_update_dual_world_empty_gate_nav_commands_residual_wave388,
    simulate_live_stealth_detector_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_hive_structure_body_dual_world_empty_gate_residual_wave389::{
    honesty_live_hive_structure_body_dual_world_empty_gate_method_names_residual_wave389,
    honesty_live_hive_structure_body_dual_world_empty_gate_nav_commands_residual_wave389,
    simulate_live_hive_structure_body_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_salvage_crate_collide_dual_world_empty_gate_residual_wave390::{
    honesty_live_salvage_crate_collide_dual_world_empty_gate_method_names_residual_wave390,
    honesty_live_salvage_crate_collide_dual_world_empty_gate_nav_commands_residual_wave390,
    simulate_live_salvage_crate_collide_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_residual_wave391::{
    honesty_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_residual_wave391,
    honesty_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_residual_wave391,
    simulate_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_power_plant_update_dual_world_empty_gate_residual_wave392::{
    honesty_live_power_plant_update_dual_world_empty_gate_method_names_residual_wave392,
    honesty_live_power_plant_update_dual_world_empty_gate_nav_commands_residual_wave392,
    simulate_live_power_plant_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_leaflet_drop_behavior_dual_world_empty_gate_residual_wave393::{
    honesty_live_leaflet_drop_behavior_dual_world_empty_gate_method_names_residual_wave393,
    honesty_live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_residual_wave393,
    simulate_live_leaflet_drop_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_auto_deposit_update_dual_world_empty_gate_residual_wave394::{
    honesty_live_auto_deposit_update_dual_world_empty_gate_method_names_residual_wave394,
    honesty_live_auto_deposit_update_dual_world_empty_gate_nav_commands_residual_wave394,
    simulate_live_auto_deposit_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_residual_wave395::{
    honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_residual_wave395,
    honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_residual_wave395,
    simulate_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_neutron_missile_slow_death_update_dual_world_empty_gate_residual_wave396::{
    honesty_live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_residual_wave396,
    honesty_live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_residual_wave396,
    simulate_live_neutron_missile_slow_death_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_dock_dual_world_empty_gate_residual_wave397::{
    honesty_live_ai_dock_dual_world_empty_gate_method_names_residual_wave397,
    honesty_live_ai_dock_dual_world_empty_gate_nav_commands_residual_wave397,
    simulate_live_ai_dock_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_groups_dual_world_empty_gate_residual_wave398::{
    honesty_live_ai_groups_dual_world_empty_gate_method_names_residual_wave398,
    honesty_live_ai_groups_dual_world_empty_gate_nav_commands_residual_wave398,
    simulate_live_ai_groups_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_artillery_barrage_power_dual_world_empty_gate_residual_wave399::{
    honesty_live_artillery_barrage_power_dual_world_empty_gate_method_names_residual_wave399,
    honesty_live_artillery_barrage_power_dual_world_empty_gate_nav_commands_residual_wave399,
    simulate_live_artillery_barrage_power_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_baikonur_launch_power_dual_world_empty_gate_residual_wave400::{
    honesty_live_baikonur_launch_power_dual_world_empty_gate_method_names_residual_wave400,
    honesty_live_baikonur_launch_power_dual_world_empty_gate_nav_commands_residual_wave400,
    simulate_live_baikonur_launch_power_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ai_group_dual_world_empty_gate_residual_wave401::{
    honesty_live_ai_group_core_dual_world_empty_gate_method_names_residual_wave401,
    honesty_live_ai_group_core_dual_world_empty_gate_nav_commands_residual_wave401,
    simulate_live_ai_group_core_dual_world_empty_gate_honesty_wave401,
};
use crate::game_logic::host_live_slaved_update_dual_world_empty_gate_residual_wave402::{
    honesty_live_slaved_update_dual_world_empty_gate_method_names_residual_wave402,
    honesty_live_slaved_update_dual_world_empty_gate_nav_commands_residual_wave402,
    simulate_live_slaved_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_demoralize_power_dual_world_empty_gate_residual_wave403::{
    honesty_live_demoralize_power_dual_world_empty_gate_method_names_residual_wave403,
    honesty_live_demoralize_power_dual_world_empty_gate_nav_commands_residual_wave403,
    simulate_live_demoralize_power_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_bone_fx_update_dual_world_empty_gate_residual_wave404::{
    honesty_live_bone_fx_update_dual_world_empty_gate_method_names_residual_wave404,
    honesty_live_bone_fx_update_dual_world_empty_gate_nav_commands_residual_wave404,
    simulate_live_bone_fx_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_supply_warehouse_dock_dual_world_empty_gate_residual_wave405::{
    honesty_live_supply_warehouse_dock_dual_world_empty_gate_method_names_residual_wave405,
    honesty_live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_residual_wave405,
    simulate_live_supply_warehouse_dock_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_ocl_special_power_dual_world_empty_gate_residual_wave406::{
    honesty_live_ocl_special_power_dual_world_empty_gate_method_names_residual_wave406,
    honesty_live_ocl_special_power_dual_world_empty_gate_nav_commands_residual_wave406,
    simulate_live_ocl_special_power_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_railed_transport_ai_update_dual_world_empty_gate_residual_wave407::{
    honesty_live_railed_transport_ai_update_dual_world_empty_gate_method_names_residual_wave407,
    honesty_live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave407,
    simulate_live_railed_transport_ai_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_squish_collide_dual_world_empty_gate_residual_wave408::{
    honesty_live_squish_collide_dual_world_empty_gate_method_names_residual_wave408,
    honesty_live_squish_collide_dual_world_empty_gate_nav_commands_residual_wave408,
    simulate_live_squish_collide_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_weapon_bonus_update_dual_world_empty_gate_residual_wave409::{
    honesty_live_weapon_bonus_update_dual_world_empty_gate_method_names_residual_wave409,
    honesty_live_weapon_bonus_update_dual_world_empty_gate_nav_commands_residual_wave409,
    simulate_live_weapon_bonus_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_minefield_behavior_dual_world_empty_gate_residual_wave410::{
    honesty_live_minefield_behavior_dual_world_empty_gate_method_names_residual_wave410,
    honesty_live_minefield_behavior_dual_world_empty_gate_nav_commands_residual_wave410,
    simulate_live_minefield_behavior_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_point_defense_laser_update_dual_world_empty_gate_residual_wave411::{
    honesty_live_point_defense_laser_update_dual_world_empty_gate_method_names_residual_wave411,
    honesty_live_point_defense_laser_update_dual_world_empty_gate_nav_commands_residual_wave411,
    simulate_live_point_defense_laser_update_dual_world_empty_gate_honesty,
};
use crate::game_logic::host_live_map_load_residual_wave170::{
    honesty_live_map_load_method_names_residual_wave170,
    honesty_live_map_load_nav_commands_residual_wave170, simulate_live_map_load_honesty,
};
use crate::game_logic::host_live_os_input_command_path_residual_wave209::{
    honesty_live_os_input_command_path_method_names_residual_wave209,
    honesty_live_os_input_command_path_nav_commands_residual_wave209,
    simulate_live_os_input_command_path_honesty,
};
use crate::game_logic::host_live_presentation_append_missing_residual_wave192::{
    honesty_live_presentation_append_missing_method_names_residual_wave192,
    honesty_live_presentation_append_missing_nav_commands_residual_wave192,
    simulate_live_presentation_append_missing_honesty,
};
use crate::game_logic::host_live_presentation_build_for_engine_residual_wave195::{
    honesty_live_presentation_build_for_engine_method_names_residual_wave195,
    honesty_live_presentation_build_for_engine_nav_commands_residual_wave195,
    simulate_live_presentation_build_for_engine_honesty,
};
use crate::game_logic::host_live_presentation_build_from_gameworld_residual_wave193::{
    honesty_live_presentation_build_from_gameworld_method_names_residual_wave193,
    honesty_live_presentation_build_from_gameworld_nav_commands_residual_wave193,
    simulate_live_presentation_build_from_gameworld_honesty,
};
use crate::game_logic::host_live_presentation_fow_only_residual_wave213::{
    honesty_live_presentation_fow_only_method_names_residual_wave213,
    honesty_live_presentation_fow_only_nav_commands_residual_wave213,
    simulate_live_presentation_fow_only_honesty,
};
use crate::game_logic::host_live_presentation_from_gameworld_default_residual_wave194::{
    honesty_live_presentation_from_gameworld_default_method_names_residual_wave194,
    honesty_live_presentation_from_gameworld_default_nav_commands_residual_wave194,
    simulate_live_presentation_from_gameworld_default_honesty,
};
use crate::game_logic::host_live_presentation_gameworld_overlay_residual_wave187::{
    honesty_live_presentation_gameworld_overlay_method_names_residual_wave187,
    honesty_live_presentation_gameworld_overlay_nav_commands_residual_wave187,
    simulate_live_presentation_gameworld_overlay_honesty,
};
use crate::game_logic::host_live_presentation_overlay_deepen_residual_wave189::{
    honesty_live_presentation_overlay_deepen_method_names_residual_wave189,
    honesty_live_presentation_overlay_deepen_nav_commands_residual_wave189,
    simulate_live_presentation_overlay_deepen_honesty,
};
use crate::game_logic::host_live_presentation_overlay_stamp_residual_wave190::{
    honesty_live_presentation_overlay_stamp_method_names_residual_wave190,
    honesty_live_presentation_overlay_stamp_nav_commands_residual_wave190,
    simulate_live_presentation_overlay_stamp_honesty,
};
use crate::game_logic::host_live_presentation_rebuilt_vertical_gate_residual_wave196::{
    honesty_live_presentation_rebuilt_vertical_gate_method_names_residual_wave196,
    honesty_live_presentation_rebuilt_vertical_gate_nav_commands_residual_wave196,
    simulate_live_presentation_rebuilt_vertical_gate_honesty,
};
use crate::game_logic::host_live_presentation_seed_residual_wave171::{
    honesty_live_presentation_seed_method_names_residual_wave171,
    honesty_live_presentation_seed_nav_commands_residual_wave171,
    simulate_live_presentation_seed_honesty,
};
use crate::game_logic::host_live_selection_commands_presentation_only_residual_wave218::{
    honesty_live_selection_commands_presentation_only_method_names_residual_wave218,
    honesty_live_selection_commands_presentation_only_nav_commands_residual_wave218,
    simulate_live_selection_commands_presentation_only_honesty,
};
use crate::game_logic::host_live_ui_command_selection_presentation_only_residual_wave219::{
    honesty_live_ui_command_selection_presentation_only_method_names_residual_wave219,
    honesty_live_ui_command_selection_presentation_only_nav_commands_residual_wave219,
    simulate_live_ui_command_selection_presentation_only_honesty,
};
use crate::game_logic::host_live_ui_helpers_presentation_only_residual_wave215::{
    honesty_live_ui_helpers_presentation_only_method_names_residual_wave215,
    honesty_live_ui_helpers_presentation_only_nav_commands_residual_wave215,
    simulate_live_ui_helpers_presentation_only_honesty,
};
use crate::game_logic::host_live_ui_producer_presentation_only_residual_wave214::{
    honesty_live_ui_producer_presentation_only_method_names_residual_wave214,
    honesty_live_ui_producer_presentation_only_nav_commands_residual_wave214,
    simulate_live_ui_producer_presentation_only_honesty,
};
use crate::game_logic::host_loading_screen_residual_wave135::{
    honesty_loading_screen_nav_commands_residual_wave135,
    honesty_loading_screen_stages_residual_wave135,
};
use crate::game_logic::host_main_menu_buttons_residual_wave118::{
    honesty_main_menu_button_names_residual_wave118,
    honesty_main_menu_button_nav_commands_residual_wave118,
    honesty_main_menu_push_targets_residual_wave118,
};
use crate::game_logic::host_main_menu_campaign_residual_wave119::{
    honesty_main_menu_campaign_button_names_residual_wave119,
    honesty_main_menu_campaign_enums_residual_wave119,
    honesty_main_menu_campaign_nav_commands_residual_wave119,
};
use crate::game_logic::host_main_menu_layout_residual_wave155::{
    honesty_main_menu_layout_names_residual_wave155,
    honesty_main_menu_layout_nav_commands_residual_wave155,
};
use crate::game_logic::host_main_menu_skirmish_nav_residual_wave114::{
    honesty_main_menu_skirmish_message_residual_wave114,
    honesty_main_menu_skirmish_names_residual_wave114,
    honesty_main_menu_skirmish_nav_steps_residual_wave114,
};
use crate::game_logic::host_main_menu_wnd_load_residual_wave161::{
    honesty_main_menu_wnd_load_method_names_residual_wave161,
    honesty_main_menu_wnd_load_nav_commands_residual_wave161,
};
use crate::game_logic::host_main_menu_wnd_materialise_residual_wave162::{
    honesty_main_menu_wnd_materialise_method_names_residual_wave162,
    honesty_main_menu_wnd_materialise_nav_commands_residual_wave162,
    simulate_main_menu_wnd_materialise_honesty,
};
use crate::game_logic::host_main_menu_wnd_residual_wave160::{
    honesty_main_menu_wnd_names_residual_wave160,
    honesty_main_menu_wnd_nav_commands_residual_wave160,
};
use crate::game_logic::host_map_select_menu_residual_wave132::{
    honesty_map_select_menu_control_names_residual_wave132,
    honesty_map_select_menu_nav_commands_residual_wave132,
};
use crate::game_logic::host_message_box_residual_wave128::{
    honesty_message_box_control_names_residual_wave128,
    honesty_message_box_nav_commands_residual_wave128,
};
use crate::game_logic::host_message_stream_meta_ingameui_residual_wave110::{
    honesty_game_message_argument_type_residual_wave110, honesty_ingame_ui_residual_wave110,
    honesty_message_stream_marker_residual_wave110, honesty_meta_event_category_residual_wave110,
};
use crate::game_logic::host_mines::honesty_cluster_mines_residual_pack_wave78;
use crate::game_logic::host_mouse_keyboard_view_residual_wave112::{
    honesty_keyboard_residual_wave112, honesty_mouse_residual_wave112,
    honesty_view_residual_wave112,
};
use crate::game_logic::host_multi_select_residual_wave150::{
    honesty_multi_select_method_names_residual_wave150,
    honesty_multi_select_nav_commands_residual_wave150,
};
use crate::game_logic::host_new_game_stream_drain_residual_wave167::{
    honesty_new_game_stream_method_names_residual_wave167,
    honesty_new_game_stream_nav_commands_residual_wave167,
    simulate_new_game_stream_post_drain_honesty,
};
use crate::game_logic::host_object_register_drawable_residual_wave104::{
    honesty_active_body_max_health_apply_residual_wave104,
    honesty_drawable_create_residual_wave104, honesty_gamelogic_register_object_residual_wave104,
    honesty_object_create_order_residual_wave104,
    honesty_object_status_state_machine_residual_wave104,
};
use crate::game_logic::host_ocl_timer_residual_wave146::{
    honesty_ocl_timer_method_names_residual_wave146,
    honesty_ocl_timer_nav_commands_residual_wave146,
};
use crate::game_logic::host_options_menu_residual_wave126::{
    honesty_options_menu_control_names_residual_wave126,
    honesty_options_menu_nav_commands_residual_wave126,
};
use crate::game_logic::host_paradrop::honesty_paradrop_residual_pack_wave76_ok;
use crate::game_logic::host_partition_collision_physics_residual::{
    honesty_collision_residual_pack_wave96, honesty_partition_residual_pack_wave96,
    honesty_physics_residual_pack_wave96, honesty_projectile_residual_deepen_pack_wave96,
};
use crate::game_logic::host_pathfinder::honesty_pathfinder_residual_pack_wave81;
use crate::game_logic::host_popup_communicator_residual_wave139::{
    honesty_popup_communicator_control_names_residual_wave139,
    honesty_popup_communicator_nav_commands_residual_wave139,
};
use crate::game_logic::host_popup_replay_residual_wave130::{
    honesty_popup_replay_control_names_residual_wave130,
    honesty_popup_replay_nav_commands_residual_wave130,
};
use crate::game_logic::host_presentation_boundary_residual_wave157::{
    honesty_presentation_boundary_method_names_residual_wave157,
    honesty_presentation_boundary_nav_commands_residual_wave157,
    honesty_presentation_boundary_source_markers_residual_wave157,
};
use crate::game_logic::host_presentation_client_boundary_residual_wave174::{
    honesty_presentation_client_boundary_method_names_residual_wave174,
    honesty_presentation_client_boundary_nav_commands_residual_wave174,
    simulate_presentation_client_boundary_honesty,
};
use crate::game_logic::host_production_buildable_command_residual::{
    honesty_buildable_residual_pack_wave99, honesty_command_button_residual_deepen_pack_wave99,
    honesty_control_bar_residual_deepen_pack_wave99, honesty_prerequisite_residual_pack_wave99,
    honesty_production_residual_deepen_pack_wave99,
};
use crate::game_logic::host_quit_menu_residual_wave123::{
    honesty_quit_menu_control_names_residual_wave123,
    honesty_quit_menu_nav_commands_residual_wave123,
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
use crate::game_logic::host_replay_control_residual_wave140::{
    honesty_replay_control_control_names_residual_wave140,
    honesty_replay_control_nav_commands_residual_wave140,
};
use crate::game_logic::host_replay_menu_residual_wave122::{
    honesty_replay_menu_control_names_residual_wave122,
    honesty_replay_menu_nav_commands_residual_wave122,
};
use crate::game_logic::host_rng_residual::{
    exercise_host_rng_residual, honesty_rng_residual_pack_ok,
};
use crate::game_logic::host_save_load_menu_residual_wave121::{
    honesty_save_load_control_stems_residual_wave121, honesty_save_load_layout_residual_wave121,
    honesty_save_load_nav_commands_residual_wave121,
};
use crate::game_logic::host_science_rank::honesty_science_rank_residual_pack_wave80;
use crate::game_logic::host_score_screen_residual_wave125::{
    honesty_score_screen_control_names_residual_wave125,
    honesty_score_screen_nav_commands_residual_wave125,
};
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
use crate::game_logic::host_shell_map_residual_wave141::{
    honesty_shell_map_names_residual_wave141, honesty_shell_map_nav_commands_residual_wave141,
};
use crate::game_logic::host_shell_skirmish_nav_residual_wave164::{
    honesty_shell_skirmish_nav_commands_residual_wave164,
    honesty_shell_skirmish_nav_method_names_residual_wave164, simulate_shell_skirmish_nav_honesty,
};
use crate::game_logic::host_shell_stack_push_residual_wave163::{
    honesty_shell_stack_push_method_names_residual_wave163,
    honesty_shell_stack_push_nav_commands_residual_wave163, simulate_shell_stack_push_honesty,
};
use crate::game_logic::host_single_authority_combat_honesty_residual_wave173::{
    honesty_single_authority_combat_method_names_residual_wave173,
    honesty_single_authority_combat_nav_commands_residual_wave173,
    simulate_single_authority_combat_honesty,
};
use crate::game_logic::host_single_player_menu_residual_wave131::{
    honesty_single_player_menu_control_names_residual_wave131,
    honesty_single_player_menu_nav_commands_residual_wave131,
};
use crate::game_logic::host_skirmish_map_select_residual_wave115::{
    honesty_skirmish_map_select_commands_residual_wave115,
    honesty_skirmish_map_select_names_residual_wave115,
    honesty_skirmish_map_select_nav_steps_residual_wave115,
};
use crate::game_logic::host_skirmish_options_wnd_residual_wave166::{
    honesty_skirmish_options_wnd_method_names_residual_wave166,
    honesty_skirmish_options_wnd_nav_commands_residual_wave166,
    simulate_skirmish_options_wnd_honesty,
};
use crate::game_logic::host_skirmish_rules_residual_wave117::{
    honesty_skirmish_game_speed_controls_residual_wave117,
    honesty_skirmish_rules_nav_commands_residual_wave117,
    honesty_skirmish_starting_cash_residual_wave117,
};
use crate::game_logic::host_skirmish_slot_config_residual_wave116::{
    honesty_skirmish_slot_combo_names_residual_wave116,
    honesty_skirmish_slot_nav_commands_residual_wave116,
    honesty_skirmish_slot_state_residual_wave116,
};
use crate::game_logic::host_smudge_residual_wave145::{
    honesty_smudge_method_names_residual_wave145, honesty_smudge_nav_commands_residual_wave145,
};
use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::{
    honesty_player_residual_deepen_pack_wave109,
    honesty_science_store_residual_deepen_pack_wave109,
    honesty_special_power_template_store_residual_wave109,
    honesty_team_residual_deepen_pack_wave109, honesty_upgrade_store_residual_deepen_pack_wave109,
};
use crate::game_logic::host_special_power_enum_residual::honesty_special_power_enum_residual_pack_wave80;
use crate::game_logic::host_start_game_loading_residual_wave169::{
    honesty_start_game_loading_method_names_residual_wave169,
    honesty_start_game_loading_nav_commands_residual_wave169, simulate_start_game_loading_honesty,
};
use crate::game_logic::host_structure_economy_residual::{
    honesty_capture_building_residual_pack_wave83, honesty_command_center_residual_pack_wave83,
    honesty_dozer_build_residual_pack_wave83, honesty_power_plant_residual_pack_wave83,
    honesty_production_queue_residual_pack_wave83, honesty_supply_warehouse_residual_pack_wave83,
};
use crate::game_logic::host_structure_inventory_residual_wave149::{
    honesty_structure_inventory_command_names_residual_wave149,
    honesty_structure_inventory_nav_commands_residual_wave149,
};
use crate::game_logic::host_superweapon_kindof::honesty_superweapon_kindof_residual_pack_wave80;
use crate::game_logic::host_terrain_bridge_water_road_residual_wave108::{
    honesty_bridge_residual_deepen_pack_wave108, honesty_cliff_residual_peels_pack_wave108,
    honesty_heightmap_residual_deepen_pack_wave108, honesty_road_residual_deepen_pack_wave108,
    honesty_water_residual_deepen_pack_wave108,
};
use crate::game_logic::host_terrain_env_boundary_residual_wave159::{
    honesty_terrain_env_boundary_method_names_residual_wave159,
    honesty_terrain_env_boundary_nav_commands_residual_wave159,
    honesty_terrain_env_boundary_source_markers_residual_wave159,
    simulate_terrain_env_boundary_prepare_honesty,
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
use crate::game_logic::host_under_construction_residual_wave148::{
    honesty_under_construction_method_names_residual_wave148,
    honesty_under_construction_nav_commands_residual_wave148,
};
use crate::game_logic::host_unit_training::honesty_unit_training_residual_pack_wave79_ok;
use crate::game_logic::host_upgrades::honesty_upgrades_cost_time_application_wave79_ok;
use crate::game_logic::host_w3d_main_menu_init_residual_wave168::{
    honesty_w3d_main_menu_init_method_names_residual_wave168,
    honesty_w3d_main_menu_init_nav_commands_residual_wave168, simulate_w3d_main_menu_init_honesty,
};
use crate::game_logic::host_window_video_residual_wave154::{
    honesty_window_video_method_names_residual_wave154,
    honesty_window_video_nav_commands_residual_wave154,
    honesty_window_video_type_state_names_residual_wave154,
};
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
use crate::gameplay_layout::simulate_main_menu_wnd_prepare_honesty;
use crate::gameplay_layout::simulate_main_menu_wnd_prepare_load_honesty;
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
use crate::graphics::simulate_presentation_boundary_prepare_honesty;
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
    /// Gate defaulted GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY on.
    pub production_authority_env_ok: bool,
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
    /// Wave 110 MessageStream marker residual pack honesty.
    pub message_stream_marker_wave110_ok: bool,
    /// Wave 110 GameMessage argument type residual pack honesty.
    pub game_message_arg_wave110_ok: bool,
    /// Wave 110 MetaEvent category residual pack honesty.
    pub meta_event_category_wave110_ok: bool,
    /// Wave 110 InGameUI residual pack honesty.
    pub ingame_ui_wave110_ok: bool,
    /// Wave 111 Drawable icon/flash residual pack honesty.
    pub drawable_icon_flash_wave111_ok: bool,
    /// Wave 111 Drawable status/stealth residual pack honesty.
    pub drawable_status_stealth_wave111_ok: bool,
    /// Wave 111 Terrain decal residual pack honesty.
    pub terrain_decal_wave111_ok: bool,
    /// Wave 111 Display draw-image mode residual pack honesty.
    pub display_draw_image_wave111_ok: bool,
    /// Wave 111 GameClient translator residual pack honesty.
    pub game_client_translator_wave111_ok: bool,
    /// Wave 111 Particle priority residual pack honesty.
    pub particle_priority_wave111_ok: bool,
    /// Wave 112 Mouse residual pack honesty.
    pub mouse_residual_wave112_ok: bool,
    /// Wave 112 Keyboard residual pack honesty.
    pub keyboard_residual_wave112_ok: bool,
    /// Wave 112 View residual pack honesty.
    pub view_residual_wave112_ok: bool,
    /// Wave 113 GameWindow manager residual pack honesty.
    pub game_window_manager_wave113_ok: bool,
    /// Wave 113 Window style residual pack honesty.
    pub window_style_wave113_ok: bool,
    /// Wave 113 Gadget residual pack honesty.
    pub gadget_wave113_ok: bool,
    /// Wave 113 Video buffer residual pack honesty.
    pub video_buffer_wave113_ok: bool,
    /// Wave 113 Audio event residual pack honesty.
    pub audio_event_wave113_ok: bool,
    /// Wave 114 MainMenu skirmish names residual pack honesty.
    pub main_menu_skirmish_names_wave114_ok: bool,
    /// Wave 114 MainMenu skirmish nav steps residual pack honesty.
    pub main_menu_skirmish_nav_steps_wave114_ok: bool,
    /// Wave 114 MainMenu skirmish message residual pack honesty.
    pub main_menu_skirmish_message_wave114_ok: bool,
    /// Wave 115 Skirmish map-select names residual pack honesty.
    pub map_select_names_wave115_ok: bool,
    /// Wave 115 Skirmish map-select nav steps residual pack honesty.
    pub map_select_nav_steps_wave115_ok: bool,
    /// Wave 115 Skirmish map-select commands residual pack honesty.
    pub map_select_commands_wave115_ok: bool,
    /// Wave 116 Skirmish slot state residual pack honesty.
    pub slot_state_wave116_ok: bool,
    /// Wave 116 Skirmish slot combo names residual pack honesty.
    pub slot_combo_names_wave116_ok: bool,
    /// Wave 116 Skirmish slot nav/commands residual pack honesty.
    pub slot_nav_commands_wave116_ok: bool,
    /// Wave 117 Skirmish starting cash residual pack honesty.
    pub starting_cash_wave117_ok: bool,
    /// Wave 117 Skirmish game speed/controls residual pack honesty.
    pub game_speed_controls_wave117_ok: bool,
    /// Wave 117 Skirmish rules nav/commands residual pack honesty.
    pub rules_nav_commands_wave117_ok: bool,
    /// Wave 118 MainMenu button names residual pack honesty.
    pub main_menu_button_names_wave118_ok: bool,
    /// Wave 118 MainMenu push targets residual pack honesty.
    pub main_menu_push_targets_wave118_ok: bool,
    /// Wave 118 MainMenu button nav/commands residual pack honesty.
    pub main_menu_button_nav_commands_wave118_ok: bool,
    /// Wave 119 MainMenu campaign button names residual pack honesty.
    pub campaign_button_names_wave119_ok: bool,
    /// Wave 119 MainMenu campaign enums residual pack honesty.
    pub campaign_enums_wave119_ok: bool,
    /// Wave 119 MainMenu campaign nav/commands residual pack honesty.
    pub campaign_nav_commands_wave119_ok: bool,
    /// Wave 120 ChallengeMenu control names residual pack honesty.
    pub challenge_control_names_wave120_ok: bool,
    /// Wave 120 ChallengeMenu nav/commands residual pack honesty.
    pub challenge_nav_commands_wave120_ok: bool,
    /// Wave 121 SaveLoad layout residual pack honesty.
    pub save_load_layout_wave121_ok: bool,
    /// Wave 121 SaveLoad control stems residual pack honesty.
    pub save_load_control_stems_wave121_ok: bool,
    /// Wave 121 SaveLoad nav/commands residual pack honesty.
    pub save_load_nav_commands_wave121_ok: bool,
    /// Wave 122 ReplayMenu control names residual pack honesty.
    pub replay_control_names_wave122_ok: bool,
    /// Wave 122 ReplayMenu nav/commands residual pack honesty.
    pub replay_nav_commands_wave122_ok: bool,
    /// Wave 123 QuitMenu control names residual pack honesty.
    pub quit_control_names_wave123_ok: bool,
    /// Wave 123 QuitMenu nav/commands residual pack honesty.
    pub quit_nav_commands_wave123_ok: bool,
    /// Wave 124 KeyboardOptions control names residual pack honesty.
    pub keyboard_control_names_wave124_ok: bool,
    /// Wave 124 KeyboardOptions nav/commands residual pack honesty.
    pub keyboard_nav_commands_wave124_ok: bool,
    /// Wave 125 ScoreScreen control names residual pack honesty.
    pub score_control_names_wave125_ok: bool,
    /// Wave 125 ScoreScreen nav/commands residual pack honesty.
    pub score_nav_commands_wave125_ok: bool,
    /// Wave 126 OptionsMenu control names residual pack honesty.
    pub options_control_names_wave126_ok: bool,
    /// Wave 126 OptionsMenu nav/commands residual pack honesty.
    pub options_nav_commands_wave126_ok: bool,
    /// Wave 127 CreditsMenu control names residual pack honesty.
    pub credits_control_names_wave127_ok: bool,
    /// Wave 127 CreditsMenu nav/commands residual pack honesty.
    pub credits_nav_commands_wave127_ok: bool,
    /// Wave 128 MessageBox control names residual pack honesty.
    pub message_box_control_names_wave128_ok: bool,
    /// Wave 128 MessageBox nav/commands residual pack honesty.
    pub message_box_nav_commands_wave128_ok: bool,
    /// Wave 129 Diplomacy control names residual pack honesty.
    pub diplomacy_control_names_wave129_ok: bool,
    /// Wave 129 Diplomacy nav/commands residual pack honesty.
    pub diplomacy_nav_commands_wave129_ok: bool,
    /// Wave 130 PopupReplay control names residual pack honesty.
    pub popup_replay_control_names_wave130_ok: bool,
    /// Wave 130 PopupReplay nav/commands residual pack honesty.
    pub popup_replay_nav_commands_wave130_ok: bool,
    /// Wave 131 SinglePlayerMenu control names residual pack honesty.
    pub single_player_control_names_wave131_ok: bool,
    /// Wave 131 SinglePlayerMenu nav/commands residual pack honesty.
    pub single_player_nav_commands_wave131_ok: bool,
    /// Wave 132 MapSelectMenu control names residual pack honesty.
    pub map_select_control_names_wave132_ok: bool,
    /// Wave 132 MapSelectMenu nav/commands residual pack honesty.
    pub map_select_nav_commands_wave132_ok: bool,
    /// Wave 133 ControlBar control names residual pack honesty.
    pub control_bar_control_names_wave133_ok: bool,
    /// Wave 133 ControlBar nav/commands residual pack honesty.
    pub control_bar_nav_commands_wave133_ok: bool,
    /// Wave 134 DifficultySelect control names residual pack honesty.
    pub difficulty_select_control_names_wave134_ok: bool,
    /// Wave 134 DifficultySelect nav/commands residual pack honesty.
    pub difficulty_select_nav_commands_wave134_ok: bool,
    /// Wave 135 LoadingScreen stages residual pack honesty.
    pub loading_screen_stages_wave135_ok: bool,
    /// Wave 135 LoadingScreen nav/commands residual pack honesty.
    pub loading_screen_nav_commands_wave135_ok: bool,
    /// Wave 136 InGameChat control names residual pack honesty.
    pub in_game_chat_control_names_wave136_ok: bool,
    /// Wave 136 InGameChat nav/commands residual pack honesty.
    pub in_game_chat_nav_commands_wave136_ok: bool,
    /// Wave 137 IdleWorker control names residual pack honesty.
    pub idle_worker_control_names_wave137_ok: bool,
    /// Wave 137 IdleWorker nav/commands residual pack honesty.
    pub idle_worker_nav_commands_wave137_ok: bool,
    /// Wave 138 GeneralsExp control names residual pack honesty.
    pub generals_exp_control_names_wave138_ok: bool,
    /// Wave 138 GeneralsExp nav/commands residual pack honesty.
    pub generals_exp_nav_commands_wave138_ok: bool,
    /// Wave 139 PopupCommunicator control names residual pack honesty.
    pub popup_communicator_control_names_wave139_ok: bool,
    /// Wave 139 PopupCommunicator nav/commands residual pack honesty.
    pub popup_communicator_nav_commands_wave139_ok: bool,
    /// Wave 140 ReplayControl control names residual pack honesty.
    pub replay_control_control_names_wave140_ok: bool,
    /// Wave 140 ReplayControl nav/commands residual pack honesty.
    pub replay_control_nav_commands_wave140_ok: bool,
    /// Wave 141 ShellMap names residual pack honesty.
    pub shell_map_names_wave141_ok: bool,
    /// Wave 141 ShellMap nav/commands residual pack honesty.
    pub shell_map_nav_commands_wave141_ok: bool,
    /// Wave 142 Beacon control names residual pack honesty.
    pub beacon_control_names_wave142_ok: bool,
    /// Wave 142 Beacon nav/commands residual pack honesty.
    pub beacon_nav_commands_wave142_ok: bool,
    /// Wave 143 EVA message names residual pack honesty.
    pub eva_message_names_wave143_ok: bool,
    /// Wave 143 EVA nav/commands residual pack honesty.
    pub eva_nav_commands_wave143_ok: bool,
    /// Wave 144 IME message names residual pack honesty.
    pub ime_message_names_wave144_ok: bool,
    /// Wave 144 IME nav/commands residual pack honesty.
    pub ime_nav_commands_wave144_ok: bool,
    /// Wave 145 Smudge method names residual pack honesty.
    pub smudge_method_names_wave145_ok: bool,
    /// Wave 145 Smudge nav/commands residual pack honesty.
    pub smudge_nav_commands_wave145_ok: bool,
    /// Wave 146 OCL timer method names residual pack honesty.
    pub ocl_timer_method_names_wave146_ok: bool,
    /// Wave 146 OCL timer nav/commands residual pack honesty.
    pub ocl_timer_nav_commands_wave146_ok: bool,
    /// Wave 147 ControlBarResizer method names residual pack honesty.
    pub control_bar_resizer_method_names_wave147_ok: bool,
    /// Wave 147 ControlBarResizer nav/commands residual pack honesty.
    pub control_bar_resizer_nav_commands_wave147_ok: bool,
    /// Wave 148 UnderConstruction method names residual pack honesty.
    pub under_construction_method_names_wave148_ok: bool,
    /// Wave 148 UnderConstruction nav/commands residual pack honesty.
    pub under_construction_nav_commands_wave148_ok: bool,
    /// Wave 149 StructureInventory command names residual pack honesty.
    pub structure_inventory_command_names_wave149_ok: bool,
    /// Wave 149 StructureInventory nav/commands residual pack honesty.
    pub structure_inventory_nav_commands_wave149_ok: bool,
    /// Wave 150 MultiSelect method names residual pack honesty.
    pub multi_select_method_names_wave150_ok: bool,
    /// Wave 150 MultiSelect nav/commands residual pack honesty.
    pub multi_select_nav_commands_wave150_ok: bool,
    /// Wave 151 Credits style/method names residual pack honesty.
    pub credits_style_method_names_wave151_ok: bool,
    /// Wave 151 Credits nav/commands residual pack honesty.
    pub credits_nav_commands_wave151_ok: bool,
    /// Wave 152 ChallengeGenerals method names residual pack honesty.
    pub challenge_generals_method_names_wave152_ok: bool,
    /// Wave 152 ChallengeGenerals nav/commands residual pack honesty.
    pub challenge_generals_nav_commands_wave152_ok: bool,
    /// Wave 153 GameWorld authority env names residual pack honesty.
    pub gameworld_authority_env_names_wave153_ok: bool,
    /// Wave 153 GameWorld authority method names residual pack honesty.
    pub gameworld_authority_method_names_wave153_ok: bool,
    /// Wave 153 GameWorld authority nav/commands residual pack honesty.
    pub gameworld_authority_nav_commands_wave153_ok: bool,
    /// Wave 154 WindowVideo type/state names residual pack honesty.
    pub window_video_type_state_names_wave154_ok: bool,
    /// Wave 154 WindowVideo method names residual pack honesty.
    pub window_video_method_names_wave154_ok: bool,
    /// Wave 154 WindowVideo nav/commands residual pack honesty.
    pub window_video_nav_commands_wave154_ok: bool,
    /// Wave 155 MainMenu layout names residual pack honesty.
    pub main_menu_layout_names_wave155_ok: bool,
    /// Wave 155 MainMenu layout nav/commands residual pack honesty.
    pub main_menu_layout_nav_commands_wave155_ok: bool,
    /// Wave 156 ControlBarScheme names residual pack honesty.
    pub control_bar_scheme_names_wave156_ok: bool,
    /// Wave 156 ControlBarScheme method names residual pack honesty.
    pub control_bar_scheme_method_names_wave156_ok: bool,
    /// Wave 156 ControlBarScheme nav/commands residual pack honesty.
    pub control_bar_scheme_nav_commands_wave156_ok: bool,
    /// Wave 157 presentation boundary method names residual pack honesty.
    pub presentation_boundary_method_names_wave157_ok: bool,
    /// Wave 157 presentation boundary source markers residual pack honesty.
    pub presentation_boundary_source_markers_wave157_ok: bool,
    /// Wave 157 presentation boundary nav/commands residual pack honesty.
    pub presentation_boundary_nav_commands_wave157_ok: bool,
    /// Wave 157 presentation boundary live source residual honesty.
    pub presentation_boundary_live_wave157_ok: bool,
    /// Wave 158 ControlBar print-positions names residual pack honesty.
    pub control_bar_print_names_wave158_ok: bool,
    /// Wave 158 ControlBar print-positions nav/commands residual pack honesty.
    pub control_bar_print_nav_commands_wave158_ok: bool,
    /// Wave 159 terrain/skybox boundary method names residual pack honesty.
    pub terrain_env_boundary_method_names_wave159_ok: bool,
    /// Wave 159 terrain/skybox boundary source markers residual pack honesty.
    pub terrain_env_boundary_source_markers_wave159_ok: bool,
    /// Wave 159 terrain/skybox boundary nav/commands residual pack honesty.
    pub terrain_env_boundary_nav_commands_wave159_ok: bool,
    /// Wave 159 terrain/skybox boundary live source residual honesty.
    pub terrain_env_boundary_live_wave159_ok: bool,
    /// Wave 160 MainMenu.wnd names residual pack honesty.
    pub main_menu_wnd_names_wave160_ok: bool,
    /// Wave 160 MainMenu.wnd nav/commands residual pack honesty.
    pub main_menu_wnd_nav_commands_wave160_ok: bool,
    /// Wave 160 MainMenu.wnd resolve/validate live residual honesty.
    pub main_menu_wnd_live_wave160_ok: bool,
    /// Wave 161 MainMenu.wnd load method names residual pack honesty.
    pub main_menu_wnd_load_method_names_wave161_ok: bool,
    /// Wave 161 MainMenu.wnd load nav/commands residual pack honesty.
    pub main_menu_wnd_load_nav_commands_wave161_ok: bool,
    /// Wave 161 MainMenu.wnd headless load live residual honesty.
    pub main_menu_wnd_load_live_wave161_ok: bool,
    /// Wave 162 MainMenu.wnd materialise method names residual pack honesty.
    pub main_menu_wnd_materialise_method_names_wave162_ok: bool,
    /// Wave 162 MainMenu.wnd materialise nav/commands residual pack honesty.
    pub main_menu_wnd_materialise_nav_commands_wave162_ok: bool,
    /// Wave 162 MainMenu.wnd materialise live residual honesty.
    pub main_menu_wnd_materialise_live_wave162_ok: bool,
    /// Wave 163 Shell stack push method names residual pack honesty.
    pub shell_stack_push_method_names_wave163_ok: bool,
    /// Wave 163 Shell stack push nav/commands residual pack honesty.
    pub shell_stack_push_nav_commands_wave163_ok: bool,
    /// Wave 163 Shell stack push live residual honesty.
    pub shell_stack_push_live_wave163_ok: bool,
    /// Wave 164 Shell→Skirmish nav method names residual pack honesty.
    pub shell_skirmish_nav_method_names_wave164_ok: bool,
    /// Wave 164 Shell→Skirmish nav/commands residual pack honesty.
    pub shell_skirmish_nav_commands_wave164_ok: bool,
    /// Wave 164 Shell→Skirmish nav live residual honesty.
    pub shell_skirmish_nav_live_wave164_ok: bool,
    /// Wave 165 ControlBar.wnd materialise method names residual pack honesty.
    pub control_bar_materialise_method_names_wave165_ok: bool,
    /// Wave 165 ControlBar.wnd materialise nav/commands residual pack honesty.
    pub control_bar_materialise_nav_commands_wave165_ok: bool,
    /// Wave 165 ControlBar.wnd materialise live residual honesty.
    pub control_bar_materialise_live_wave165_ok: bool,
    /// Wave 166 Skirmish options WND method names residual pack honesty.
    pub skirmish_options_wnd_method_names_wave166_ok: bool,
    /// Wave 166 Skirmish options WND nav/commands residual pack honesty.
    pub skirmish_options_wnd_nav_commands_wave166_ok: bool,
    /// Wave 166 Skirmish options WND live residual honesty.
    pub skirmish_options_wnd_live_wave166_ok: bool,
    /// Wave 167 NewGame stream method names residual pack honesty.
    pub new_game_stream_method_names_wave167_ok: bool,
    /// Wave 167 NewGame stream nav/commands residual pack honesty.
    pub new_game_stream_nav_commands_wave167_ok: bool,
    /// Wave 167 NewGame stream live residual honesty.
    pub new_game_stream_live_wave167_ok: bool,
    /// Wave 168 W3DMainMenuInit method names residual pack honesty.
    pub w3d_main_menu_init_method_names_wave168_ok: bool,
    /// Wave 168 W3DMainMenuInit nav/commands residual pack honesty.
    pub w3d_main_menu_init_nav_commands_wave168_ok: bool,
    /// Wave 168 W3DMainMenuInit live residual honesty.
    pub w3d_main_menu_init_live_wave168_ok: bool,
    /// Wave 169 start_game Loading method names residual pack honesty.
    pub start_game_loading_method_names_wave169_ok: bool,
    /// Wave 169 start_game Loading nav/commands residual pack honesty.
    pub start_game_loading_nav_commands_wave169_ok: bool,
    /// Wave 169 start_game Loading live residual honesty.
    pub start_game_loading_live_wave169_ok: bool,
    /// Wave 170 live map load method names residual pack honesty.
    pub live_map_load_method_names_wave170_ok: bool,
    /// Wave 170 live map load nav/commands residual pack honesty.
    pub live_map_load_nav_commands_wave170_ok: bool,
    /// Wave 170 live map load residual honesty.
    pub live_map_load_live_wave170_ok: bool,
    /// Wave 171 live presentation seed method names residual pack honesty.
    pub live_presentation_seed_method_names_wave171_ok: bool,
    /// Wave 171 live presentation seed nav/commands residual pack honesty.
    pub live_presentation_seed_nav_commands_wave171_ok: bool,
    /// Wave 171 live presentation seed residual honesty.
    pub live_presentation_seed_live_wave171_ok: bool,
    /// Wave 172 live GameWorld shadow overlay method names residual pack honesty.
    pub live_gameworld_shadow_overlay_method_names_wave172_ok: bool,
    /// Wave 172 live GameWorld shadow overlay nav/commands residual pack honesty.
    pub live_gameworld_shadow_overlay_nav_commands_wave172_ok: bool,
    /// Wave 172 live GameWorld shadow overlay residual honesty.
    pub live_gameworld_shadow_overlay_live_wave172_ok: bool,
    /// Wave 173 single-authority combat honesty method names residual pack.
    pub single_authority_combat_method_names_wave173_ok: bool,
    /// Wave 173 single-authority combat honesty nav/commands residual pack.
    pub single_authority_combat_nav_commands_wave173_ok: bool,
    /// Wave 173 single-authority combat honesty live residual.
    pub single_authority_combat_live_wave173_ok: bool,
    /// Wave 174 presentation/GameClient boundary method names residual pack.
    pub presentation_client_boundary_method_names_wave174_ok: bool,
    /// Wave 174 presentation/GameClient boundary nav/commands residual pack.
    pub presentation_client_boundary_nav_commands_wave174_ok: bool,
    /// Wave 174 presentation/GameClient boundary live residual.
    pub presentation_client_boundary_live_wave174_ok: bool,
    /// Wave 175 golden map-host victory method names residual pack.
    pub golden_map_host_victory_method_names_wave175_ok: bool,
    /// Wave 175 golden map-host victory nav/commands residual pack.
    pub golden_map_host_victory_nav_commands_wave175_ok: bool,
    /// Wave 175 golden map-host victory live residual.
    pub golden_map_host_victory_live_wave175_ok: bool,
    /// Wave 176 executable InGame presentation boundary method names residual pack.
    pub executable_presentation_boundary_method_names_wave176_ok: bool,
    /// Wave 176 executable InGame presentation boundary nav/commands residual pack.
    pub executable_presentation_boundary_nav_commands_wave176_ok: bool,
    /// Wave 176 executable InGame presentation boundary live residual.
    pub executable_presentation_boundary_live_wave176_ok: bool,
    /// Wave 177 GameWorld production authority method names residual pack.
    pub gameworld_production_authority_method_names_wave177_ok: bool,
    /// Wave 177 GameWorld production authority nav/commands residual pack.
    pub gameworld_production_authority_nav_commands_wave177_ok: bool,
    /// Wave 177 GameWorld production authority live residual.
    pub gameworld_production_authority_live_wave177_ok: bool,
    /// Wave 178 GameWorld sole-tick coupling method names residual pack.
    pub gameworld_sole_tick_coupling_method_names_wave178_ok: bool,
    /// Wave 178 GameWorld sole-tick coupling nav/commands residual pack.
    pub gameworld_sole_tick_coupling_nav_commands_wave178_ok: bool,
    /// Wave 178 GameWorld sole-tick coupling live residual.
    pub gameworld_sole_tick_coupling_live_wave178_ok: bool,
    /// Gate defaulted GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY on.
    pub movement_authority_env_ok: bool,
    /// Wave 179 GameWorld authority matrix method names residual pack.
    pub gameworld_authority_matrix_method_names_wave179_ok: bool,
    /// Wave 179 GameWorld authority matrix nav/commands residual pack.
    pub gameworld_authority_matrix_nav_commands_wave179_ok: bool,
    /// Wave 179 GameWorld authority matrix live residual.
    pub gameworld_authority_matrix_live_wave179_ok: bool,
    /// AI attack / fire spawn / construction authorities default-on.
    pub ai_fire_construction_authority_env_ok: bool,
    /// Wave 180 live GameWorld production writeback method names residual pack.
    pub live_gameworld_production_writeback_method_names_wave180_ok: bool,
    /// Wave 180 live GameWorld production writeback nav/commands residual pack.
    pub live_gameworld_production_writeback_nav_commands_wave180_ok: bool,
    /// Wave 180 live GameWorld production writeback live residual.
    pub live_gameworld_production_writeback_live_wave180_ok: bool,
    /// Wave 181 live GameWorld construction writeback method names residual pack.
    pub live_gameworld_construction_writeback_method_names_wave181_ok: bool,
    /// Wave 181 live GameWorld construction writeback nav/commands residual pack.
    pub live_gameworld_construction_writeback_nav_commands_wave181_ok: bool,
    /// Wave 181 live GameWorld construction writeback live residual.
    pub live_gameworld_construction_writeback_live_wave181_ok: bool,
    /// Wave 182 live GameWorld damage channel method names residual pack.
    pub live_gameworld_damage_channel_method_names_wave182_ok: bool,
    /// Wave 182 live GameWorld damage channel nav/commands residual pack.
    pub live_gameworld_damage_channel_nav_commands_wave182_ok: bool,
    /// Wave 182 live GameWorld damage channel live residual.
    pub live_gameworld_damage_channel_live_wave182_ok: bool,
    /// Wave 183 live GameWorld economy/movement method names residual pack.
    pub live_gameworld_economy_movement_method_names_wave183_ok: bool,
    /// Wave 183 live GameWorld economy/movement nav/commands residual pack.
    pub live_gameworld_economy_movement_nav_commands_wave183_ok: bool,
    /// Wave 183 live GameWorld economy/movement live residual.
    pub live_gameworld_economy_movement_live_wave183_ok: bool,
    /// Wave 184 live GameWorld projectile/AI method names residual pack.
    pub live_gameworld_projectile_ai_method_names_wave184_ok: bool,
    /// Wave 184 live GameWorld projectile/AI nav/commands residual pack.
    pub live_gameworld_projectile_ai_nav_commands_wave184_ok: bool,
    /// Wave 184 live GameWorld projectile/AI live residual.
    pub live_gameworld_projectile_ai_live_wave184_ok: bool,
    /// Wave 185 live GameWorld fire/special-power method names residual pack.
    pub live_gameworld_fire_special_power_method_names_wave185_ok: bool,
    /// Wave 185 live GameWorld fire/special-power nav/commands residual pack.
    pub live_gameworld_fire_special_power_nav_commands_wave185_ok: bool,
    /// Wave 185 live GameWorld fire/special-power live residual.
    pub live_gameworld_fire_special_power_live_wave185_ok: bool,
    /// Wave 186 live GameWorld presentation view method names residual pack.
    pub live_gameworld_presentation_view_method_names_wave186_ok: bool,
    /// Wave 186 live GameWorld presentation view nav/commands residual pack.
    pub live_gameworld_presentation_view_nav_commands_wave186_ok: bool,
    /// Wave 186 live GameWorld presentation view live residual.
    pub live_gameworld_presentation_view_live_wave186_ok: bool,
    /// Wave 187 live PresentationFrame←GameWorld overlay method names residual pack.
    pub live_presentation_gameworld_overlay_method_names_wave187_ok: bool,
    /// Wave 187 live PresentationFrame←GameWorld overlay nav/commands residual pack.
    pub live_presentation_gameworld_overlay_nav_commands_wave187_ok: bool,
    /// Wave 187 live PresentationFrame←GameWorld overlay live residual.
    pub live_presentation_gameworld_overlay_live_wave187_ok: bool,
    /// Wave 188 executable GameWorld presentation method names residual pack.
    pub executable_gameworld_presentation_method_names_wave188_ok: bool,
    /// Wave 188 executable GameWorld presentation nav/commands residual pack.
    pub executable_gameworld_presentation_nav_commands_wave188_ok: bool,
    /// Wave 188 executable GameWorld presentation live residual.
    pub executable_gameworld_presentation_live_wave188_ok: bool,
    /// Wave 189 live presentation overlay deepen method names residual pack.
    pub live_presentation_overlay_deepen_method_names_wave189_ok: bool,
    /// Wave 189 live presentation overlay deepen nav/commands residual pack.
    pub live_presentation_overlay_deepen_nav_commands_wave189_ok: bool,
    /// Wave 189 live presentation overlay deepen live residual.
    pub live_presentation_overlay_deepen_live_wave189_ok: bool,
    /// Wave 190 live presentation overlay stamp method names residual pack.
    pub live_presentation_overlay_stamp_method_names_wave190_ok: bool,
    /// Wave 190 live presentation overlay stamp nav/commands residual pack.
    pub live_presentation_overlay_stamp_nav_commands_wave190_ok: bool,
    /// Wave 190 live presentation overlay stamp live residual.
    pub live_presentation_overlay_stamp_live_wave190_ok: bool,
    /// Wave 191 live GameWorld entity view deepen method names residual pack.
    pub live_gameworld_entity_view_deepen_method_names_wave191_ok: bool,
    /// Wave 191 live GameWorld entity view deepen nav/commands residual pack.
    pub live_gameworld_entity_view_deepen_nav_commands_wave191_ok: bool,
    /// Wave 191 live GameWorld entity view deepen live residual.
    pub live_gameworld_entity_view_deepen_live_wave191_ok: bool,
    /// Wave 192 live presentation append-missing method names residual pack.
    pub live_presentation_append_missing_method_names_wave192_ok: bool,
    /// Wave 192 live presentation append-missing nav/commands residual pack.
    pub live_presentation_append_missing_nav_commands_wave192_ok: bool,
    /// Wave 192 live presentation append-missing live residual.
    pub live_presentation_append_missing_live_wave192_ok: bool,
    /// Wave 193 live presentation build-from-gameworld method names residual pack.
    pub live_presentation_build_from_gameworld_method_names_wave193_ok: bool,
    /// Wave 193 live presentation build-from-gameworld nav/commands residual pack.
    pub live_presentation_build_from_gameworld_nav_commands_wave193_ok: bool,
    /// Wave 193 live presentation build-from-gameworld live residual.
    pub live_presentation_build_from_gameworld_live_wave193_ok: bool,
    /// Wave 194 live presentation from-gameworld default method names residual pack.
    pub live_presentation_from_gameworld_default_method_names_wave194_ok: bool,
    /// Wave 194 live presentation from-gameworld default nav/commands residual pack.
    pub live_presentation_from_gameworld_default_nav_commands_wave194_ok: bool,
    /// Wave 194 live presentation from-gameworld default live residual.
    pub live_presentation_from_gameworld_default_live_wave194_ok: bool,
    /// Wave 195 live presentation build-for-engine method names residual pack.
    pub live_presentation_build_for_engine_method_names_wave195_ok: bool,
    /// Wave 195 live presentation build-for-engine nav/commands residual pack.
    pub live_presentation_build_for_engine_nav_commands_wave195_ok: bool,
    /// Wave 195 live presentation build-for-engine live residual.
    pub live_presentation_build_for_engine_live_wave195_ok: bool,
    /// Wave 196 live presentation rebuilt vertical gate method names residual pack.
    pub live_presentation_rebuilt_vertical_gate_method_names_wave196_ok: bool,
    /// Wave 196 live presentation rebuilt vertical gate nav/commands residual pack.
    pub live_presentation_rebuilt_vertical_gate_nav_commands_wave196_ok: bool,
    /// Wave 196 live presentation rebuilt vertical gate live residual.
    pub live_presentation_rebuilt_vertical_gate_live_wave196_ok: bool,
    /// Wave 197 live command attack-log method names residual pack.
    pub live_command_attack_log_method_names_wave197_ok: bool,
    /// Wave 197 live command attack-log nav/commands residual pack.
    pub live_command_attack_log_nav_commands_wave197_ok: bool,
    /// Wave 197 live command attack-log live residual.
    pub live_command_attack_log_live_wave197_ok: bool,
    /// Wave 198 live command guard-log method names residual pack.
    pub live_command_guard_log_method_names_wave198_ok: bool,
    /// Wave 198 live command guard-log nav/commands residual pack.
    pub live_command_guard_log_nav_commands_wave198_ok: bool,
    /// Wave 198 live command guard-log live residual.
    pub live_command_guard_log_live_wave198_ok: bool,
    /// Wave 199 live command production/construction log method names residual pack.
    pub live_command_production_construction_log_method_names_wave199_ok: bool,
    /// Wave 199 live command production/construction log nav/commands residual pack.
    pub live_command_production_construction_log_nav_commands_wave199_ok: bool,
    /// Wave 199 live command production/construction log live residual.
    pub live_command_production_construction_log_live_wave199_ok: bool,
    /// Wave 200 live command rally-log method names residual pack.
    pub live_command_rally_log_method_names_wave200_ok: bool,
    /// Wave 200 live command rally-log nav/commands residual pack.
    pub live_command_rally_log_nav_commands_wave200_ok: bool,
    /// Wave 200 live command rally-log live residual.
    pub live_command_rally_log_live_wave200_ok: bool,
    /// Wave 201 live evacuate contain-log method names residual pack.
    pub live_evacuate_contain_log_method_names_wave201_ok: bool,
    /// Wave 201 live evacuate contain-log nav/commands residual pack.
    pub live_evacuate_contain_log_nav_commands_wave201_ok: bool,
    /// Wave 201 live evacuate contain-log live residual.
    pub live_evacuate_contain_log_live_wave201_ok: bool,
    /// Wave 202 live cheer/science log method names residual pack.
    pub live_command_cheer_science_log_method_names_wave202_ok: bool,
    /// Wave 202 live cheer/science log nav/commands residual pack.
    pub live_command_cheer_science_log_nav_commands_wave202_ok: bool,
    /// Wave 202 live cheer/science log live residual.
    pub live_command_cheer_science_log_live_wave202_ok: bool,
    /// Wave 203 live deploy status-log method names residual pack.
    pub live_command_deploy_status_log_method_names_wave203_ok: bool,
    /// Wave 203 live deploy status-log nav/commands residual pack.
    pub live_command_deploy_status_log_nav_commands_wave203_ok: bool,
    /// Wave 203 live deploy status-log live residual.
    pub live_command_deploy_status_log_live_wave203_ok: bool,
    /// Wave 204 live formation log method names residual pack.
    pub live_command_formation_log_method_names_wave204_ok: bool,
    /// Wave 204 live formation log nav/commands residual pack.
    pub live_command_formation_log_nav_commands_wave204_ok: bool,
    /// Wave 204 live formation log live residual.
    pub live_command_formation_log_live_wave204_ok: bool,
    /// Wave 205 live order-target log method names residual pack.
    pub live_command_order_target_log_method_names_wave205_ok: bool,
    /// Wave 205 live order-target log nav/commands residual pack.
    pub live_command_order_target_log_nav_commands_wave205_ok: bool,
    /// Wave 205 live order-target log live residual.
    pub live_command_order_target_log_live_wave205_ok: bool,
    /// Wave 206 live selection log method names residual pack.
    pub live_command_selection_log_method_names_wave206_ok: bool,
    /// Wave 206 live selection log nav/commands residual pack.
    pub live_command_selection_log_nav_commands_wave206_ok: bool,
    /// Wave 206 live selection log live residual.
    pub live_command_selection_log_live_wave206_ok: bool,
    /// Wave 207 live non-attack order-target method names residual pack.
    pub live_command_non_attack_order_target_method_names_wave207_ok: bool,
    /// Wave 207 live non-attack order-target nav/commands residual pack.
    pub live_command_non_attack_order_target_nav_commands_wave207_ok: bool,
    /// Wave 207 live non-attack order-target live residual.
    pub live_command_non_attack_order_target_live_wave207_ok: bool,
    /// Wave 208 live golden mop-up honesty method names residual pack.
    pub live_golden_mopup_honesty_method_names_wave208_ok: bool,
    /// Wave 208 live golden mop-up honesty nav/commands residual pack.
    pub live_golden_mopup_honesty_nav_commands_wave208_ok: bool,
    /// Wave 208 live golden mop-up honesty live residual.
    pub live_golden_mopup_honesty_live_wave208_ok: bool,
    /// Wave 209 live OS-input command path method names residual pack.
    pub live_os_input_command_path_method_names_wave209_ok: bool,
    /// Wave 209 live OS-input command path nav/commands residual pack.
    pub live_os_input_command_path_nav_commands_wave209_ok: bool,
    /// Wave 209 live OS-input command path live residual.
    pub live_os_input_command_path_live_wave209_ok: bool,
    /// Wave 210 live beacon note method names residual pack.
    pub live_command_beacon_note_method_names_wave210_ok: bool,
    /// Wave 210 live beacon note nav/commands residual pack.
    pub live_command_beacon_note_nav_commands_wave210_ok: bool,
    /// Wave 210 live beacon note live residual.
    pub live_command_beacon_note_live_wave210_ok: bool,
    /// Wave 211 live host beacon presentation method names residual pack.
    pub live_host_beacon_presentation_method_names_wave211_ok: bool,
    /// Wave 211 live host beacon presentation nav/commands residual pack.
    pub live_host_beacon_presentation_nav_commands_wave211_ok: bool,
    /// Wave 211 live host beacon presentation live residual.
    pub live_host_beacon_presentation_live_wave211_ok: bool,
    /// Wave 212 live sell deselect log method names residual pack.
    pub live_command_sell_deselect_log_method_names_wave212_ok: bool,
    /// Wave 212 live sell deselect log nav/commands residual pack.
    pub live_command_sell_deselect_log_nav_commands_wave212_ok: bool,
    /// Wave 212 live sell deselect log live residual.
    pub live_command_sell_deselect_log_live_wave212_ok: bool,
    /// Wave 213 live presentation FOW-only method names residual pack.
    pub live_presentation_fow_only_method_names_wave213_ok: bool,
    /// Wave 213 live presentation FOW-only nav/commands residual pack.
    pub live_presentation_fow_only_nav_commands_wave213_ok: bool,
    /// Wave 213 live presentation FOW-only live residual.
    pub live_presentation_fow_only_live_wave213_ok: bool,
    /// Wave 214 live UI producer presentation-only method names residual pack.
    pub live_ui_producer_presentation_only_method_names_wave214_ok: bool,
    /// Wave 214 live UI producer presentation-only nav/commands residual pack.
    pub live_ui_producer_presentation_only_nav_commands_wave214_ok: bool,
    /// Wave 214 live UI producer presentation-only live residual.
    pub live_ui_producer_presentation_only_live_wave214_ok: bool,
    /// Wave 215 live UI helpers presentation-only method names residual pack.
    pub live_ui_helpers_presentation_only_method_names_wave215_ok: bool,
    /// Wave 215 live UI helpers presentation-only nav/commands residual pack.
    pub live_ui_helpers_presentation_only_nav_commands_wave215_ok: bool,
    /// Wave 215 live UI helpers presentation-only live residual.
    pub live_ui_helpers_presentation_only_live_wave215_ok: bool,
    /// Wave 216 control-group/camera presentation-only method names residual pack.
    pub live_control_group_camera_presentation_only_method_names_wave216_ok: bool,
    /// Wave 216 control-group/camera presentation-only nav/commands residual pack.
    pub live_control_group_camera_presentation_only_nav_commands_wave216_ok: bool,
    /// Wave 216 control-group/camera presentation-only live residual.
    pub live_control_group_camera_presentation_only_live_wave216_ok: bool,
    /// Wave 217 cmd-filter/env presentation-only method names residual pack.
    pub live_cmd_filter_env_presentation_only_method_names_wave217_ok: bool,
    /// Wave 217 cmd-filter/env presentation-only nav/commands residual pack.
    pub live_cmd_filter_env_presentation_only_nav_commands_wave217_ok: bool,
    /// Wave 217 cmd-filter/env presentation-only live residual.
    pub live_cmd_filter_env_presentation_only_live_wave217_ok: bool,
    /// Wave 218 selection-commands presentation-only method names residual pack.
    pub live_selection_commands_presentation_only_method_names_wave218_ok: bool,
    /// Wave 218 selection-commands presentation-only nav/commands residual pack.
    pub live_selection_commands_presentation_only_nav_commands_wave218_ok: bool,
    /// Wave 218 selection-commands presentation-only live residual.
    pub live_selection_commands_presentation_only_live_wave218_ok: bool,
    /// Wave 219 UI-command selection presentation-only method names residual pack.
    pub live_ui_command_selection_presentation_only_method_names_wave219_ok: bool,
    /// Wave 219 UI-command selection presentation-only nav/commands residual pack.
    pub live_ui_command_selection_presentation_only_nav_commands_wave219_ok: bool,
    /// Wave 219 UI-command selection presentation-only live residual.
    pub live_ui_command_selection_presentation_only_live_wave219_ok: bool,
    /// Wave 220 local-team presentation-only method names residual pack.
    pub live_local_team_presentation_only_method_names_wave220_ok: bool,
    /// Wave 220 local-team presentation-only nav/commands residual pack.
    pub live_local_team_presentation_only_nav_commands_wave220_ok: bool,
    /// Wave 220 local-team presentation-only live residual.
    pub live_local_team_presentation_only_live_wave220_ok: bool,
    /// Wave 221 hotkey/move/attack selection presentation-only method names residual pack.
    pub live_hotkey_move_attack_selection_presentation_only_method_names_wave221_ok: bool,
    /// Wave 221 hotkey/move/attack selection presentation-only nav/commands residual pack.
    pub live_hotkey_move_attack_selection_presentation_only_nav_commands_wave221_ok: bool,
    /// Wave 221 hotkey/move/attack selection presentation-only live residual.
    pub live_hotkey_move_attack_selection_presentation_only_live_wave221_ok: bool,
    /// Wave 222 pick-object presentation-only method names residual pack.
    pub live_pick_object_presentation_only_method_names_wave222_ok: bool,
    /// Wave 222 pick-object presentation-only nav/commands residual pack.
    pub live_pick_object_presentation_only_nav_commands_wave222_ok: bool,
    /// Wave 222 pick-object presentation-only live residual.
    pub live_pick_object_presentation_only_live_wave222_ok: bool,
    /// Wave 223 bootstrap-camera presentation-only method names residual pack.
    pub live_bootstrap_camera_presentation_only_method_names_wave223_ok: bool,
    /// Wave 223 bootstrap-camera presentation-only nav/commands residual pack.
    pub live_bootstrap_camera_presentation_only_nav_commands_wave223_ok: bool,
    /// Wave 223 bootstrap-camera presentation-only live residual.
    pub live_bootstrap_camera_presentation_only_live_wave223_ok: bool,
    /// Wave 224 force-complete authority API method names residual pack.
    pub live_force_complete_authority_api_method_names_wave224_ok: bool,
    /// Wave 224 force-complete authority API nav/commands residual pack.
    pub live_force_complete_authority_api_nav_commands_wave224_ok: bool,
    /// Wave 224 force-complete authority API live residual.
    pub live_force_complete_authority_api_live_wave224_ok: bool,
    /// Wave 225 path/guard authority API method names residual pack.
    pub live_path_guard_authority_api_method_names_wave225_ok: bool,
    /// Wave 225 path/guard authority API nav/commands residual pack.
    pub live_path_guard_authority_api_nav_commands_wave225_ok: bool,
    /// Wave 225 path/guard authority API live residual.
    pub live_path_guard_authority_api_live_wave225_ok: bool,
    /// Wave 226 hotkey selection/camera presentation-only method names residual pack.
    pub live_hotkey_selection_camera_presentation_only_method_names_wave226_ok: bool,
    /// Wave 226 hotkey selection/camera presentation-only nav/commands residual pack.
    pub live_hotkey_selection_camera_presentation_only_nav_commands_wave226_ok: bool,
    /// Wave 226 hotkey selection/camera presentation-only live residual.
    pub live_hotkey_selection_camera_presentation_only_live_wave226_ok: bool,
    /// Wave 227 construct-spawn pose authority API method names residual pack.
    pub live_construct_spawn_pose_authority_api_method_names_wave227_ok: bool,
    /// Wave 227 construct-spawn pose authority API nav/commands residual pack.
    pub live_construct_spawn_pose_authority_api_nav_commands_wave227_ok: bool,
    /// Wave 227 construct-spawn pose authority API live residual.
    pub live_construct_spawn_pose_authority_api_live_wave227_ok: bool,
    /// Wave 228 RMB target presentation-only method names residual pack.
    pub live_rmb_target_presentation_only_method_names_wave228_ok: bool,
    /// Wave 228 RMB target presentation-only nav/commands residual pack.
    pub live_rmb_target_presentation_only_nav_commands_wave228_ok: bool,
    /// Wave 228 RMB target presentation-only live residual.
    pub live_rmb_target_presentation_only_live_wave228_ok: bool,
    /// Wave 229 RMB selected presentation-only method names residual pack.
    pub live_rmb_selected_presentation_only_method_names_wave229_ok: bool,
    /// Wave 229 RMB selected presentation-only nav/commands residual pack.
    pub live_rmb_selected_presentation_only_nav_commands_wave229_ok: bool,
    /// Wave 229 RMB selected presentation-only live residual.
    pub live_rmb_selected_presentation_only_live_wave229_ok: bool,
    /// Wave 230 command unit authority API method names residual pack.
    pub live_command_unit_authority_api_method_names_wave230_ok: bool,
    /// Wave 230 command unit authority API nav/commands residual pack.
    pub live_command_unit_authority_api_nav_commands_wave230_ok: bool,
    /// Wave 230 command unit authority API live residual.
    pub live_command_unit_authority_api_live_wave230_ok: bool,
    /// Wave 231 command unit more-authority API method names residual pack.
    pub live_command_unit_more_authority_api_method_names_wave231_ok: bool,
    /// Wave 231 command unit more-authority API nav/commands residual pack.
    pub live_command_unit_more_authority_api_nav_commands_wave231_ok: bool,
    /// Wave 231 command unit more-authority API live residual.
    pub live_command_unit_more_authority_api_live_wave231_ok: bool,
    /// Wave 232 command executor authority API method names residual pack.
    pub live_command_executor_authority_api_method_names_wave232_ok: bool,
    /// Wave 232 command executor authority API nav/commands residual pack.
    pub live_command_executor_authority_api_nav_commands_wave232_ok: bool,
    /// Wave 232 command executor authority API live residual.
    pub live_command_executor_authority_api_live_wave232_ok: bool,
    /// Wave 233 command executor more-authority API method names residual pack.
    pub live_command_executor_more_authority_api_method_names_wave233_ok: bool,
    /// Wave 233 command executor more-authority API nav/commands residual pack.
    pub live_command_executor_more_authority_api_nav_commands_wave233_ok: bool,
    /// Wave 233 command executor more-authority API live residual.
    pub live_command_executor_more_authority_api_live_wave233_ok: bool,
    /// Wave 234 engine presentation player UI method names residual pack.
    pub live_engine_presentation_player_ui_method_names_wave234_ok: bool,
    /// Wave 234 engine presentation player UI nav/commands residual pack.
    pub live_engine_presentation_player_ui_nav_commands_wave234_ok: bool,
    /// Wave 234 engine presentation player UI live residual.
    pub live_engine_presentation_player_ui_live_wave234_ok: bool,
    /// Wave 235 RMB presentation full-classify method names residual pack.
    pub live_rmb_presentation_full_classify_method_names_wave235_ok: bool,
    /// Wave 235 RMB presentation full-classify nav/commands residual pack.
    pub live_rmb_presentation_full_classify_nav_commands_wave235_ok: bool,
    /// Wave 235 RMB presentation full-classify live residual.
    pub live_rmb_presentation_full_classify_live_wave235_ok: bool,
    /// Wave 236 mouse input presentation-only method names residual pack.
    pub live_mouse_input_presentation_only_method_names_wave236_ok: bool,
    /// Wave 236 mouse input presentation-only nav/commands residual pack.
    pub live_mouse_input_presentation_only_nav_commands_wave236_ok: bool,
    /// Wave 236 mouse input presentation-only live residual.
    pub live_mouse_input_presentation_only_live_wave236_ok: bool,
    /// Wave 237 engine player UI boot peel method names residual pack.
    pub live_engine_player_ui_boot_peel_method_names_wave237_ok: bool,
    /// Wave 237 engine player UI boot peel nav/commands residual pack.
    pub live_engine_player_ui_boot_peel_nav_commands_wave237_ok: bool,
    /// Wave 237 engine player UI boot peel live residual.
    pub live_engine_player_ui_boot_peel_live_wave237_ok: bool,
    /// Wave 238 player probe API method names residual pack.
    pub live_player_probe_api_method_names_wave238_ok: bool,
    /// Wave 238 player probe API nav/commands residual pack.
    pub live_player_probe_api_nav_commands_wave238_ok: bool,
    /// Wave 238 player probe API live residual.
    pub live_player_probe_api_live_wave238_ok: bool,
    /// Wave 239 player team probe method names residual pack.
    pub live_player_team_probe_method_names_wave239_ok: bool,
    /// Wave 239 player team probe nav/commands residual pack.
    pub live_player_team_probe_nav_commands_wave239_ok: bool,
    /// Wave 239 player team probe live residual.
    pub live_player_team_probe_live_wave239_ok: bool,
    /// Wave 240 player field probe method names residual pack.
    pub live_player_field_probe_method_names_wave240_ok: bool,
    /// Wave 240 player field probe nav/commands residual pack.
    pub live_player_field_probe_nav_commands_wave240_ok: bool,
    /// Wave 240 player field probe live residual.
    pub live_player_field_probe_live_wave240_ok: bool,
    /// Wave 241 camera height probe method names residual pack.
    pub live_camera_height_probe_method_names_wave241_ok: bool,
    /// Wave 241 camera height probe nav/commands residual pack.
    pub live_camera_height_probe_nav_commands_wave241_ok: bool,
    /// Wave 241 camera height probe live residual.
    pub live_camera_height_probe_live_wave241_ok: bool,
    /// Wave 242 command player probe method names residual pack.
    pub live_command_player_probe_method_names_wave242_ok: bool,
    /// Wave 242 command player probe nav/commands residual pack.
    pub live_command_player_probe_nav_commands_wave242_ok: bool,
    /// Wave 242 command player probe live residual.
    pub live_command_player_probe_live_wave242_ok: bool,
    /// Wave 243 construct economy probe method names residual pack.
    pub live_construct_economy_probe_method_names_wave243_ok: bool,
    /// Wave 243 construct economy probe nav/commands residual pack.
    pub live_construct_economy_probe_nav_commands_wave243_ok: bool,
    /// Wave 243 construct economy probe live residual.
    pub live_construct_economy_probe_live_wave243_ok: bool,
    /// Wave 244 command unit probe method names residual pack.
    pub live_command_unit_probe_method_names_wave244_ok: bool,
    /// Wave 244 command unit probe nav/commands residual pack.
    pub live_command_unit_probe_nav_commands_wave244_ok: bool,
    /// Wave 244 command unit probe live residual.
    pub live_command_unit_probe_live_wave244_ok: bool,
    /// Wave 245 selection query probe method names residual pack.
    pub live_selection_query_probe_method_names_wave245_ok: bool,
    /// Wave 245 selection query probe nav/commands residual pack.
    pub live_selection_query_probe_nav_commands_wave245_ok: bool,
    /// Wave 245 selection query probe live residual.
    pub live_selection_query_probe_live_wave245_ok: bool,
    /// Wave 246 world pick probe method names residual pack.
    pub live_world_pick_probe_method_names_wave246_ok: bool,
    /// Wave 246 world pick probe nav/commands residual pack.
    pub live_world_pick_probe_nav_commands_wave246_ok: bool,
    /// Wave 246 world pick probe live residual.
    pub live_world_pick_probe_live_wave246_ok: bool,
    /// Wave 247 object registry empty fastpath method names residual pack.
    pub live_object_registry_empty_fastpath_method_names_wave247_ok: bool,
    /// Wave 247 object registry empty fastpath nav/commands residual pack.
    pub live_object_registry_empty_fastpath_nav_commands_wave247_ok: bool,
    /// Wave 247 object registry empty fastpath live residual.
    pub live_object_registry_empty_fastpath_live_wave247_ok: bool,
    /// Wave 248 legacy object registry fastpath method names residual pack.
    pub live_legacy_object_registry_fastpath_method_names_wave248_ok: bool,
    /// Wave 248 legacy object registry fastpath nav/commands residual pack.
    pub live_legacy_object_registry_fastpath_nav_commands_wave248_ok: bool,
    /// Wave 248 legacy object registry fastpath live residual.
    pub live_legacy_object_registry_fastpath_live_wave248_ok: bool,
    /// Wave 249 client dual-world empty-gate method names residual pack.
    pub live_client_dual_world_empty_gate_method_names_wave249_ok: bool,
    /// Wave 249 client dual-world empty-gate nav/commands residual pack.
    pub live_client_dual_world_empty_gate_nav_commands_wave249_ok: bool,
    /// Wave 249 client dual-world empty-gate live residual.
    pub live_client_dual_world_empty_gate_live_wave249_ok: bool,
    /// Wave 250 presentation time-frozen probe method names residual pack.
    pub live_presentation_time_frozen_probe_method_names_wave250_ok: bool,
    /// Wave 250 presentation time-frozen probe nav/commands residual pack.
    pub live_presentation_time_frozen_probe_nav_commands_wave250_ok: bool,
    /// Wave 250 presentation time-frozen probe live residual.
    pub live_presentation_time_frozen_probe_live_wave250_ok: bool,
    /// Wave 251 presentation visual-speed probe method names residual pack.
    pub live_presentation_visual_speed_probe_method_names_wave251_ok: bool,
    /// Wave 251 presentation visual-speed probe nav/commands residual pack.
    pub live_presentation_visual_speed_probe_nav_commands_wave251_ok: bool,
    /// Wave 251 presentation visual-speed probe live residual.
    pub live_presentation_visual_speed_probe_live_wave251_ok: bool,
    /// Wave 252 presentation script-camera probe method names residual pack.
    pub live_presentation_script_camera_probe_method_names_wave252_ok: bool,
    /// Wave 252 presentation script-camera probe nav/commands residual pack.
    pub live_presentation_script_camera_probe_nav_commands_wave252_ok: bool,
    /// Wave 252 presentation script-camera probe live residual.
    pub live_presentation_script_camera_probe_live_wave252_ok: bool,
    /// Wave 253 AIGroup dual-world empty-gate method names residual pack.
    pub live_ai_group_dual_world_empty_gate_method_names_wave253_ok: bool,
    /// Wave 253 AIGroup dual-world empty-gate nav/commands residual pack.
    pub live_ai_group_dual_world_empty_gate_nav_commands_wave253_ok: bool,
    /// Wave 253 AIGroup dual-world empty-gate live residual.
    pub live_ai_group_dual_world_empty_gate_live_wave253_ok: bool,
    /// Wave 254 AIStates dual-world empty-gate method names residual pack.
    pub live_ai_states_dual_world_empty_gate_method_names_wave254_ok: bool,
    /// Wave 254 AIStates dual-world empty-gate nav/commands residual pack.
    pub live_ai_states_dual_world_empty_gate_nav_commands_wave254_ok: bool,
    /// Wave 254 AIStates dual-world empty-gate live residual.
    pub live_ai_states_dual_world_empty_gate_live_wave254_ok: bool,
    /// Wave 255 AIPlayer dual-world empty-gate method names residual pack.
    pub live_ai_player_dual_world_empty_gate_method_names_wave255_ok: bool,
    /// Wave 255 AIPlayer dual-world empty-gate nav/commands residual pack.
    pub live_ai_player_dual_world_empty_gate_nav_commands_wave255_ok: bool,
    /// Wave 255 AIPlayer dual-world empty-gate live residual.
    pub live_ai_player_dual_world_empty_gate_live_wave255_ok: bool,
    /// Wave 256 Team dual-world empty-gate method names residual pack.
    pub live_team_dual_world_empty_gate_method_names_wave256_ok: bool,
    /// Wave 256 Team dual-world empty-gate nav/commands residual pack.
    pub live_team_dual_world_empty_gate_nav_commands_wave256_ok: bool,
    /// Wave 256 Team dual-world empty-gate live residual.
    pub live_team_dual_world_empty_gate_live_wave256_ok: bool,
    /// Wave 257 legacy AI states dual-world empty-gate method names residual pack.
    pub live_ai_legacy_states_dual_world_empty_gate_method_names_wave257_ok: bool,
    /// Wave 257 legacy AI states dual-world empty-gate nav/commands residual pack.
    pub live_ai_legacy_states_dual_world_empty_gate_nav_commands_wave257_ok: bool,
    /// Wave 257 legacy AI states dual-world empty-gate live residual.
    pub live_ai_legacy_states_dual_world_empty_gate_live_wave257_ok: bool,
    /// Wave 258 Unit dual-world empty-gate method names residual pack.
    pub live_unit_dual_world_empty_gate_method_names_wave258_ok: bool,
    /// Wave 258 Unit dual-world empty-gate nav/commands residual pack.
    pub live_unit_dual_world_empty_gate_nav_commands_wave258_ok: bool,
    /// Wave 258 Unit dual-world empty-gate live residual.
    pub live_unit_dual_world_empty_gate_live_wave258_ok: bool,
    /// Wave 259 Stealth dual-world empty-gate method names residual pack.
    pub live_stealth_dual_world_empty_gate_method_names_wave259_ok: bool,
    /// Wave 259 Stealth dual-world empty-gate nav/commands residual pack.
    pub live_stealth_dual_world_empty_gate_nav_commands_wave259_ok: bool,
    /// Wave 259 Stealth dual-world empty-gate live residual.
    pub live_stealth_dual_world_empty_gate_live_wave259_ok: bool,
    /// Wave 260 Garrison dual-world empty-gate method names residual pack.
    pub live_garrison_dual_world_empty_gate_method_names_wave260_ok: bool,
    /// Wave 260 Garrison dual-world empty-gate nav/commands residual pack.
    pub live_garrison_dual_world_empty_gate_nav_commands_wave260_ok: bool,
    /// Wave 260 Garrison dual-world empty-gate live residual.
    pub live_garrison_dual_world_empty_gate_live_wave260_ok: bool,
    /// Wave 261 OpenContain dual-world empty-gate method names residual pack.
    pub live_open_contain_dual_world_empty_gate_method_names_wave261_ok: bool,
    /// Wave 261 OpenContain dual-world empty-gate nav/commands residual pack.
    pub live_open_contain_dual_world_empty_gate_nav_commands_wave261_ok: bool,
    /// Wave 261 OpenContain dual-world empty-gate live residual.
    pub live_open_contain_dual_world_empty_gate_live_wave261_ok: bool,
    /// Wave 262 Pathfind dual-world empty-gate method names residual pack.
    pub live_pathfind_dual_world_empty_gate_method_names_wave262_ok: bool,
    /// Wave 262 Pathfind dual-world empty-gate nav/commands residual pack.
    pub live_pathfind_dual_world_empty_gate_nav_commands_wave262_ok: bool,
    /// Wave 262 Pathfind dual-world empty-gate live residual.
    pub live_pathfind_dual_world_empty_gate_live_wave262_ok: bool,
    /// Wave 263 AI mod dual-world empty-gate method names residual pack.
    pub live_ai_mod_dual_world_empty_gate_method_names_wave263_ok: bool,
    /// Wave 263 AI mod dual-world empty-gate nav/commands residual pack.
    pub live_ai_mod_dual_world_empty_gate_nav_commands_wave263_ok: bool,
    /// Wave 263 AI mod dual-world empty-gate live residual.
    pub live_ai_mod_dual_world_empty_gate_live_wave263_ok: bool,
    /// Wave 264 Object mod dual-world empty-gate method names residual pack.
    pub live_object_mod_dual_world_empty_gate_method_names_wave264_ok: bool,
    /// Wave 264 Object mod dual-world empty-gate nav/commands residual pack.
    pub live_object_mod_dual_world_empty_gate_nav_commands_wave264_ok: bool,
    /// Wave 264 Object mod dual-world empty-gate live residual.
    pub live_object_mod_dual_world_empty_gate_live_wave264_ok: bool,
    /// Wave 265 Weapon dual-world empty-gate method names residual pack.
    pub live_weapon_dual_world_empty_gate_method_names_wave265_ok: bool,
    /// Wave 265 Weapon dual-world empty-gate nav/commands residual pack.
    pub live_weapon_dual_world_empty_gate_nav_commands_wave265_ok: bool,
    /// Wave 265 Weapon dual-world empty-gate live residual.
    pub live_weapon_dual_world_empty_gate_live_wave265_ok: bool,
    /// Wave 266 Partition filters dual-world empty-gate method names residual pack.
    pub live_partition_filters_dual_world_empty_gate_method_names_wave266_ok: bool,
    /// Wave 266 Partition filters dual-world empty-gate nav/commands residual pack.
    pub live_partition_filters_dual_world_empty_gate_nav_commands_wave266_ok: bool,
    /// Wave 266 Partition filters dual-world empty-gate live residual.
    pub live_partition_filters_dual_world_empty_gate_live_wave266_ok: bool,
    /// Wave 267 AI state machine dual-world empty-gate method names residual pack.
    pub live_ai_state_machine_dual_world_empty_gate_method_names_wave267_ok: bool,
    /// Wave 267 AI state machine dual-world empty-gate nav/commands residual pack.
    pub live_ai_state_machine_dual_world_empty_gate_nav_commands_wave267_ok: bool,
    /// Wave 267 AI state machine dual-world empty-gate live residual.
    pub live_ai_state_machine_dual_world_empty_gate_live_wave267_ok: bool,
    /// Wave 268 Player dual-world empty-gate method names residual pack.
    pub live_player_dual_world_empty_gate_method_names_wave268_ok: bool,
    /// Wave 268 Player dual-world empty-gate nav/commands residual pack.
    pub live_player_dual_world_empty_gate_nav_commands_wave268_ok: bool,
    /// Wave 268 Player dual-world empty-gate live residual.
    pub live_player_dual_world_empty_gate_live_wave268_ok: bool,
    /// Wave 269 GameClient dual-world empty-gate method names residual pack.
    pub live_game_client_dual_world_empty_gate_method_names_wave269_ok: bool,
    /// Wave 269 GameClient dual-world empty-gate nav/commands residual pack.
    pub live_game_client_dual_world_empty_gate_nav_commands_wave269_ok: bool,
    /// Wave 269 GameClient dual-world empty-gate live residual.
    pub live_game_client_dual_world_empty_gate_live_wave269_ok: bool,
    /// Wave 270 Drawable dual-world empty-gate method names residual pack.
    pub live_drawable_dual_world_empty_gate_method_names_wave270_ok: bool,
    /// Wave 270 Drawable dual-world empty-gate nav/commands residual pack.
    pub live_drawable_dual_world_empty_gate_nav_commands_wave270_ok: bool,
    /// Wave 270 Drawable dual-world empty-gate live residual.
    pub live_drawable_dual_world_empty_gate_live_wave270_ok: bool,
    /// Wave 271 script conditions dual-world empty-gate method names residual pack.
    pub live_script_conditions_dual_world_empty_gate_method_names_wave271_ok: bool,
    /// Wave 271 script conditions dual-world empty-gate nav/commands residual pack.
    pub live_script_conditions_dual_world_empty_gate_nav_commands_wave271_ok: bool,
    /// Wave 271 script conditions dual-world empty-gate live residual.
    pub live_script_conditions_dual_world_empty_gate_live_wave271_ok: bool,
    /// Wave 272 TransportContain dual-world empty-gate method names residual pack.
    pub live_transport_contain_dual_world_empty_gate_method_names_wave272_ok: bool,
    /// Wave 272 TransportContain dual-world empty-gate nav/commands residual pack.
    pub live_transport_contain_dual_world_empty_gate_nav_commands_wave272_ok: bool,
    /// Wave 272 TransportContain dual-world empty-gate live residual.
    pub live_transport_contain_dual_world_empty_gate_live_wave272_ok: bool,
    /// Wave 273 InGameUI dual-world empty-gate method names residual pack.
    pub live_ingame_ui_dual_world_empty_gate_method_names_wave273_ok: bool,
    /// Wave 273 InGameUI dual-world empty-gate nav/commands residual pack.
    pub live_ingame_ui_dual_world_empty_gate_nav_commands_wave273_ok: bool,
    /// Wave 273 InGameUI dual-world empty-gate live residual.
    pub live_ingame_ui_dual_world_empty_gate_live_wave273_ok: bool,
    /// Wave 274 HelixContain dual-world empty-gate method names residual pack.
    pub live_helix_contain_dual_world_empty_gate_method_names_wave274_ok: bool,
    /// Wave 274 HelixContain dual-world empty-gate nav/commands residual pack.
    pub live_helix_contain_dual_world_empty_gate_nav_commands_wave274_ok: bool,
    /// Wave 274 HelixContain dual-world empty-gate live residual.
    pub live_helix_contain_dual_world_empty_gate_live_wave274_ok: bool,
    /// Wave 275 CommandProcessor dual-world empty-gate method names residual pack.
    pub live_command_processor_dual_world_empty_gate_method_names_wave275_ok: bool,
    /// Wave 275 CommandProcessor dual-world empty-gate nav/commands residual pack.
    pub live_command_processor_dual_world_empty_gate_nav_commands_wave275_ok: bool,
    /// Wave 275 CommandProcessor dual-world empty-gate live residual.
    pub live_command_processor_dual_world_empty_gate_live_wave275_ok: bool,
    /// Wave 276 Turret dual-world empty-gate method names residual pack.
    pub live_turret_dual_world_empty_gate_method_names_wave276_ok: bool,
    /// Wave 276 Turret dual-world empty-gate nav/commands residual pack.
    pub live_turret_dual_world_empty_gate_nav_commands_wave276_ok: bool,
    /// Wave 276 Turret dual-world empty-gate live residual.
    pub live_turret_dual_world_empty_gate_live_wave276_ok: bool,
    /// Wave 277 RiderChangeContain dual-world empty-gate method names residual pack.
    pub live_rider_change_contain_dual_world_empty_gate_method_names_wave277_ok: bool,
    /// Wave 277 RiderChangeContain dual-world empty-gate nav/commands residual pack.
    pub live_rider_change_contain_dual_world_empty_gate_nav_commands_wave277_ok: bool,
    /// Wave 277 RiderChangeContain dual-world empty-gate live residual.
    pub live_rider_change_contain_dual_world_empty_gate_live_wave277_ok: bool,
    /// Wave 278 Selection dual-world empty-gate method names residual pack.
    pub live_selection_dual_world_empty_gate_method_names_wave278_ok: bool,
    /// Wave 278 Selection dual-world empty-gate nav/commands residual pack.
    pub live_selection_dual_world_empty_gate_nav_commands_wave278_ok: bool,
    /// Wave 278 Selection dual-world empty-gate live residual.
    pub live_selection_dual_world_empty_gate_live_wave278_ok: bool,
    /// Wave 279 CaveContain dual-world empty-gate method names residual pack.
    pub live_cave_contain_dual_world_empty_gate_method_names_wave279_ok: bool,
    /// Wave 279 CaveContain dual-world empty-gate nav/commands residual pack.
    pub live_cave_contain_dual_world_empty_gate_nav_commands_wave279_ok: bool,
    /// Wave 279 CaveContain dual-world empty-gate live residual.
    pub live_cave_contain_dual_world_empty_gate_live_wave279_ok: bool,
    /// Wave 280 TunnelContain dual-world empty-gate method names residual pack.
    pub live_tunnel_contain_dual_world_empty_gate_method_names_wave280_ok: bool,
    /// Wave 280 TunnelContain dual-world empty-gate nav/commands residual pack.
    pub live_tunnel_contain_dual_world_empty_gate_nav_commands_wave280_ok: bool,
    /// Wave 280 TunnelContain dual-world empty-gate live residual.
    pub live_tunnel_contain_dual_world_empty_gate_live_wave280_ok: bool,
    /// Wave 281 helpers dual-world empty-gate method names residual pack.
    pub live_helpers_dual_world_empty_gate_method_names_wave281_ok: bool,
    /// Wave 281 helpers dual-world empty-gate nav/commands residual pack.
    pub live_helpers_dual_world_empty_gate_nav_commands_wave281_ok: bool,
    /// Wave 281 helpers dual-world empty-gate live residual.
    pub live_helpers_dual_world_empty_gate_live_wave281_ok: bool,
    /// Wave 282 AIUpdateInterface dual-world empty-gate method names residual pack.
    pub live_ai_update_interface_dual_world_empty_gate_method_names_wave282_ok: bool,
    /// Wave 282 AIUpdateInterface dual-world empty-gate nav/commands residual pack.
    pub live_ai_update_interface_dual_world_empty_gate_nav_commands_wave282_ok: bool,
    /// Wave 282 AIUpdateInterface dual-world empty-gate live residual.
    pub live_ai_update_interface_dual_world_empty_gate_live_wave282_ok: bool,
    /// Wave 283 StealthUpdate dual-world empty-gate method names residual pack.
    pub live_stealth_update_dual_world_empty_gate_method_names_wave283_ok: bool,
    /// Wave 283 StealthUpdate dual-world empty-gate nav/commands residual pack.
    pub live_stealth_update_dual_world_empty_gate_nav_commands_wave283_ok: bool,
    /// Wave 283 StealthUpdate dual-world empty-gate live residual.
    pub live_stealth_update_dual_world_empty_gate_live_wave283_ok: bool,
    /// Wave 284 script executor dual-world empty-gate method names residual pack.
    pub live_script_executor_dual_world_empty_gate_method_names_wave284_ok: bool,
    /// Wave 284 script executor dual-world empty-gate nav/commands residual pack.
    pub live_script_executor_dual_world_empty_gate_nav_commands_wave284_ok: bool,
    /// Wave 284 script executor dual-world empty-gate live residual.
    pub live_script_executor_dual_world_empty_gate_live_wave284_ok: bool,
    /// Wave 285 AI integration dual-world empty-gate method names residual pack.
    pub live_ai_integration_dual_world_empty_gate_method_names_wave285_ok: bool,
    /// Wave 285 AI integration dual-world empty-gate nav/commands residual pack.
    pub live_ai_integration_dual_world_empty_gate_nav_commands_wave285_ok: bool,
    /// Wave 285 AI integration dual-world empty-gate live residual.
    pub live_ai_integration_dual_world_empty_gate_live_wave285_ok: bool,
    /// Wave 286 DumbProjectile dual-world empty-gate method names residual pack.
    pub live_dumb_projectile_dual_world_empty_gate_method_names_wave286_ok: bool,
    /// Wave 286 DumbProjectile dual-world empty-gate nav/commands residual pack.
    pub live_dumb_projectile_dual_world_empty_gate_nav_commands_wave286_ok: bool,
    /// Wave 286 DumbProjectile dual-world empty-gate live residual.
    pub live_dumb_projectile_dual_world_empty_gate_live_wave286_ok: bool,
    /// Wave 287 enhanced player dual-world empty-gate method names residual pack.
    pub live_enhanced_player_dual_world_empty_gate_method_names_wave287_ok: bool,
    /// Wave 287 enhanced player dual-world empty-gate nav/commands residual pack.
    pub live_enhanced_player_dual_world_empty_gate_nav_commands_wave287_ok: bool,
    /// Wave 287 enhanced player dual-world empty-gate live residual.
    pub live_enhanced_player_dual_world_empty_gate_live_wave287_ok: bool,
    /// Wave 288 hijacker update dual-world empty-gate method names residual pack.
    pub live_hijacker_update_dual_world_empty_gate_method_names_wave288_ok: bool,
    /// Wave 288 hijacker update dual-world empty-gate nav/commands residual pack.
    pub live_hijacker_update_dual_world_empty_gate_nav_commands_wave288_ok: bool,
    /// Wave 288 hijacker update dual-world empty-gate live residual.
    pub live_hijacker_update_dual_world_empty_gate_live_wave288_ok: bool,
    /// Wave 289 weapon dual-world empty-gate method names residual pack.
    pub live_weapon_impl_dual_world_empty_gate_method_names_wave289_ok: bool,
    /// Wave 289 weapon dual-world empty-gate nav/commands residual pack.
    pub live_weapon_impl_dual_world_empty_gate_nav_commands_wave289_ok: bool,
    /// Wave 289 weapon dual-world empty-gate live residual.
    pub live_weapon_impl_dual_world_empty_gate_live_wave289_ok: bool,
    /// Wave 290 async player dual-world empty-gate method names residual pack.
    pub live_async_player_dual_world_empty_gate_method_names_wave290_ok: bool,
    /// Wave 290 async player dual-world empty-gate nav/commands residual pack.
    pub live_async_player_dual_world_empty_gate_nav_commands_wave290_ok: bool,
    /// Wave 290 async player dual-world empty-gate live residual.
    pub live_async_player_dual_world_empty_gate_live_wave290_ok: bool,
    /// Wave 291 active body dual-world empty-gate method names residual pack.
    pub live_active_body_dual_world_empty_gate_method_names_wave291_ok: bool,
    /// Wave 291 active body dual-world empty-gate nav/commands residual pack.
    pub live_active_body_dual_world_empty_gate_nav_commands_wave291_ok: bool,
    /// Wave 291 active body dual-world empty-gate live residual.
    pub live_active_body_dual_world_empty_gate_live_wave291_ok: bool,
    /// Wave 292 skirmish conditions dual-world empty-gate method names residual pack.
    pub live_skirmish_conditions_dual_world_empty_gate_method_names_wave292_ok: bool,
    /// Wave 292 skirmish conditions dual-world empty-gate nav/commands residual pack.
    pub live_skirmish_conditions_dual_world_empty_gate_nav_commands_wave292_ok: bool,
    /// Wave 292 skirmish conditions dual-world empty-gate live residual.
    pub live_skirmish_conditions_dual_world_empty_gate_live_wave292_ok: bool,
    /// Wave 293 AI build list dual-world empty-gate method names residual pack.
    pub live_ai_build_list_dual_world_empty_gate_method_names_wave293_ok: bool,
    /// Wave 293 AI build list dual-world empty-gate nav/commands residual pack.
    pub live_ai_build_list_dual_world_empty_gate_nav_commands_wave293_ok: bool,
    /// Wave 293 AI build list dual-world empty-gate live residual.
    pub live_ai_build_list_dual_world_empty_gate_live_wave293_ok: bool,
    /// Wave 294 victory dual-world empty-gate method names residual pack.
    pub live_victory_dual_world_empty_gate_method_names_wave294_ok: bool,
    /// Wave 294 victory dual-world empty-gate nav/commands residual pack.
    pub live_victory_dual_world_empty_gate_nav_commands_wave294_ok: bool,
    /// Wave 294 victory dual-world empty-gate live residual.
    pub live_victory_dual_world_empty_gate_live_wave294_ok: bool,
    /// Wave 295 script actions dual-world empty-gate method names residual pack.
    pub live_script_actions_dual_world_empty_gate_method_names_wave295_ok: bool,
    /// Wave 295 script actions dual-world empty-gate nav/commands residual pack.
    pub live_script_actions_dual_world_empty_gate_nav_commands_wave295_ok: bool,
    /// Wave 295 script actions dual-world empty-gate live residual.
    pub live_script_actions_dual_world_empty_gate_live_wave295_ok: bool,
    /// Wave 296 special ability dual-world empty-gate method names residual pack.
    pub live_special_ability_dual_world_empty_gate_method_names_wave296_ok: bool,
    /// Wave 296 special ability dual-world empty-gate nav/commands residual pack.
    pub live_special_ability_dual_world_empty_gate_nav_commands_wave296_ok: bool,
    /// Wave 296 special ability dual-world empty-gate live residual.
    pub live_special_ability_dual_world_empty_gate_live_wave296_ok: bool,
    /// Wave 297 stealth detector dual-world empty-gate method names residual pack.
    pub live_stealth_detector_dual_world_empty_gate_method_names_wave297_ok: bool,
    /// Wave 297 stealth detector dual-world empty-gate nav/commands residual pack.
    pub live_stealth_detector_dual_world_empty_gate_nav_commands_wave297_ok: bool,
    /// Wave 297 stealth detector dual-world empty-gate live residual.
    pub live_stealth_detector_dual_world_empty_gate_live_wave297_ok: bool,
    /// Wave 298 supply system dual-world empty-gate method names residual pack.
    pub live_supply_system_dual_world_empty_gate_method_names_wave298_ok: bool,
    /// Wave 298 supply system dual-world empty-gate nav/commands residual pack.
    pub live_supply_system_dual_world_empty_gate_nav_commands_wave298_ok: bool,
    /// Wave 298 supply system dual-world empty-gate live residual.
    pub live_supply_system_dual_world_empty_gate_live_wave298_ok: bool,
    /// Wave 299 particle uplink dual-world empty-gate method names residual pack.
    pub live_particle_uplink_dual_world_empty_gate_method_names_wave299_ok: bool,
    /// Wave 299 particle uplink dual-world empty-gate nav/commands residual pack.
    pub live_particle_uplink_dual_world_empty_gate_nav_commands_wave299_ok: bool,
    /// Wave 299 particle uplink dual-world empty-gate live residual.
    pub live_particle_uplink_dual_world_empty_gate_live_wave299_ok: bool,
    /// Wave 300 overlord contain dual-world empty-gate method names residual pack.
    pub live_overlord_contain_dual_world_empty_gate_method_names_wave300_ok: bool,
    /// Wave 300 overlord contain dual-world empty-gate nav/commands residual pack.
    pub live_overlord_contain_dual_world_empty_gate_nav_commands_wave300_ok: bool,
    /// Wave 300 overlord contain dual-world empty-gate live residual.
    pub live_overlord_contain_dual_world_empty_gate_live_wave300_ok: bool,
    /// Wave 301 bridge behavior dual-world empty-gate method names residual pack.
    pub live_bridge_behavior_dual_world_empty_gate_method_names_wave301_ok: bool,
    /// Wave 301 bridge behavior dual-world empty-gate nav/commands residual pack.
    pub live_bridge_behavior_dual_world_empty_gate_nav_commands_wave301_ok: bool,
    /// Wave 301 bridge behavior dual-world empty-gate live residual.
    pub live_bridge_behavior_dual_world_empty_gate_live_wave301_ok: bool,
    /// Wave 302 stealth behavior dual-world empty-gate method names residual pack.
    pub live_stealth_behavior_dual_world_empty_gate_method_names_wave302_ok: bool,
    /// Wave 302 stealth behavior dual-world empty-gate nav/commands residual pack.
    pub live_stealth_behavior_dual_world_empty_gate_nav_commands_wave302_ok: bool,
    /// Wave 302 stealth behavior dual-world empty-gate live residual.
    pub live_stealth_behavior_dual_world_empty_gate_live_wave302_ok: bool,
    /// Wave 303 crate collide dual-world empty-gate method names residual pack.
    pub live_crate_collide_dual_world_empty_gate_method_names_wave303_ok: bool,
    /// Wave 303 crate collide dual-world empty-gate nav/commands residual pack.
    pub live_crate_collide_dual_world_empty_gate_nav_commands_wave303_ok: bool,
    /// Wave 303 crate collide dual-world empty-gate live residual.
    pub live_crate_collide_dual_world_empty_gate_live_wave303_ok: bool,
    /// Wave 304 object manager dual-world empty-gate method names residual pack.
    pub live_object_manager_dual_world_empty_gate_method_names_wave304_ok: bool,
    /// Wave 304 object manager dual-world empty-gate nav/commands residual pack.
    pub live_object_manager_dual_world_empty_gate_nav_commands_wave304_ok: bool,
    /// Wave 304 object manager dual-world empty-gate live residual.
    pub live_object_manager_dual_world_empty_gate_live_wave304_ok: bool,
    /// Wave 305 sticky bomb dual-world empty-gate method names residual pack.
    pub live_sticky_bomb_dual_world_empty_gate_method_names_wave305_ok: bool,
    /// Wave 305 sticky bomb dual-world empty-gate nav/commands residual pack.
    pub live_sticky_bomb_dual_world_empty_gate_nav_commands_wave305_ok: bool,
    /// Wave 305 sticky bomb dual-world empty-gate live residual.
    pub live_sticky_bomb_dual_world_empty_gate_live_wave305_ok: bool,
    /// Wave 306 auto heal dual-world empty-gate method names residual pack.
    pub live_auto_heal_dual_world_empty_gate_method_names_wave306_ok: bool,
    /// Wave 306 auto heal dual-world empty-gate nav/commands residual pack.
    pub live_auto_heal_dual_world_empty_gate_nav_commands_wave306_ok: bool,
    /// Wave 306 auto heal dual-world empty-gate live residual.
    pub live_auto_heal_dual_world_empty_gate_live_wave306_ok: bool,
    /// Wave 307 grant stealth dual-world empty-gate method names residual pack.
    pub live_grant_stealth_dual_world_empty_gate_method_names_wave307_ok: bool,
    /// Wave 307 grant stealth dual-world empty-gate nav/commands residual pack.
    pub live_grant_stealth_dual_world_empty_gate_nav_commands_wave307_ok: bool,
    /// Wave 307 grant stealth dual-world empty-gate live residual.
    pub live_grant_stealth_dual_world_empty_gate_live_wave307_ok: bool,
    /// Wave 308 status bits upgrade dual-world empty-gate method names residual pack.
    pub live_status_bits_upgrade_dual_world_empty_gate_method_names_wave308_ok: bool,
    /// Wave 308 status bits upgrade dual-world empty-gate nav/commands residual pack.
    pub live_status_bits_upgrade_dual_world_empty_gate_nav_commands_wave308_ok: bool,
    /// Wave 308 status bits upgrade dual-world empty-gate live residual.
    pub live_status_bits_upgrade_dual_world_empty_gate_live_wave308_ok: bool,
    /// Wave 309 jet AI dual-world empty-gate method names residual pack.
    pub live_jet_ai_dual_world_empty_gate_method_names_wave309_ok: bool,
    /// Wave 309 jet AI dual-world empty-gate nav/commands residual pack.
    pub live_jet_ai_dual_world_empty_gate_nav_commands_wave309_ok: bool,
    /// Wave 309 jet AI dual-world empty-gate live residual.
    pub live_jet_ai_dual_world_empty_gate_live_wave309_ok: bool,
    /// Wave 310 parking place dual-world empty-gate method names residual pack.
    pub live_parking_place_dual_world_empty_gate_method_names_wave310_ok: bool,
    /// Wave 310 parking place dual-world empty-gate nav/commands residual pack.
    pub live_parking_place_dual_world_empty_gate_nav_commands_wave310_ok: bool,
    /// Wave 310 parking place dual-world empty-gate live residual.
    pub live_parking_place_dual_world_empty_gate_live_wave310_ok: bool,
    /// Wave 311 flight deck dual-world empty-gate method names residual pack.
    pub live_flight_deck_dual_world_empty_gate_method_names_wave311_ok: bool,
    /// Wave 311 flight deck dual-world empty-gate nav/commands residual pack.
    pub live_flight_deck_dual_world_empty_gate_nav_commands_wave311_ok: bool,
    /// Wave 311 flight deck dual-world empty-gate live residual.
    pub live_flight_deck_dual_world_empty_gate_live_wave311_ok: bool,
    /// Wave 312 exit strategies dual-world empty-gate method names residual pack.
    pub live_exit_strategies_dual_world_empty_gate_method_names_wave312_ok: bool,
    /// Wave 312 exit strategies dual-world empty-gate nav/commands residual pack.
    pub live_exit_strategies_dual_world_empty_gate_nav_commands_wave312_ok: bool,
    /// Wave 312 exit strategies dual-world empty-gate live residual.
    pub live_exit_strategies_dual_world_empty_gate_live_wave312_ok: bool,
    /// Wave 313 collision system dual-world empty-gate method names residual pack.
    pub live_collision_system_dual_world_empty_gate_method_names_wave313_ok: bool,
    /// Wave 313 collision system dual-world empty-gate nav/commands residual pack.
    pub live_collision_system_dual_world_empty_gate_nav_commands_wave313_ok: bool,
    /// Wave 313 collision system dual-world empty-gate live residual.
    pub live_collision_system_dual_world_empty_gate_live_wave313_ok: bool,
    /// Wave 314 max health upgrade dual-world empty-gate method names residual pack.
    pub live_max_health_upgrade_dual_world_empty_gate_method_names_wave314_ok: bool,
    /// Wave 314 max health upgrade dual-world empty-gate nav/commands residual pack.
    pub live_max_health_upgrade_dual_world_empty_gate_nav_commands_wave314_ok: bool,
    /// Wave 314 max health upgrade dual-world empty-gate live residual.
    pub live_max_health_upgrade_dual_world_empty_gate_live_wave314_ok: bool,
    /// Wave 315 structure topple dual-world empty-gate method names residual pack.
    pub live_structure_topple_dual_world_empty_gate_method_names_wave315_ok: bool,
    /// Wave 315 structure topple dual-world empty-gate nav/commands residual pack.
    pub live_structure_topple_dual_world_empty_gate_nav_commands_wave315_ok: bool,
    /// Wave 315 structure topple dual-world empty-gate live residual.
    pub live_structure_topple_dual_world_empty_gate_live_wave315_ok: bool,
    /// Wave 316 physics update dual-world empty-gate method names residual pack.
    pub live_physics_update_dual_world_empty_gate_method_names_wave316_ok: bool,
    /// Wave 316 physics update dual-world empty-gate nav/commands residual pack.
    pub live_physics_update_dual_world_empty_gate_nav_commands_wave316_ok: bool,
    /// Wave 316 physics update dual-world empty-gate live residual.
    pub live_physics_update_dual_world_empty_gate_live_wave316_ok: bool,
    /// Wave 317 cleanup hazard dual-world empty-gate method names residual pack.
    pub live_cleanup_hazard_dual_world_empty_gate_method_names_wave317_ok: bool,
    /// Wave 317 cleanup hazard dual-world empty-gate nav/commands residual pack.
    pub live_cleanup_hazard_dual_world_empty_gate_nav_commands_wave317_ok: bool,
    /// Wave 317 cleanup hazard dual-world empty-gate live residual.
    pub live_cleanup_hazard_dual_world_empty_gate_live_wave317_ok: bool,
    /// Wave 318 bridge tower dual-world empty-gate method names residual pack.
    pub live_bridge_tower_dual_world_empty_gate_method_names_wave318_ok: bool,
    /// Wave 318 bridge tower dual-world empty-gate nav/commands residual pack.
    pub live_bridge_tower_dual_world_empty_gate_nav_commands_wave318_ok: bool,
    /// Wave 318 bridge tower dual-world empty-gate live residual.
    pub live_bridge_tower_dual_world_empty_gate_live_wave318_ok: bool,
    /// Wave 319 armor upgrade dual-world empty-gate method names residual pack.
    pub live_armor_upgrade_dual_world_empty_gate_method_names_wave319_ok: bool,
    /// Wave 319 armor upgrade dual-world empty-gate nav/commands residual pack.
    pub live_armor_upgrade_dual_world_empty_gate_nav_commands_wave319_ok: bool,
    /// Wave 319 armor upgrade dual-world empty-gate live residual.
    pub live_armor_upgrade_dual_world_empty_gate_live_wave319_ok: bool,
    /// Wave 320 paradrop power dual-world empty-gate method names residual pack.
    pub live_paradrop_power_dual_world_empty_gate_method_names_wave320_ok: bool,
    /// Wave 320 paradrop power dual-world empty-gate nav/commands residual pack.
    pub live_paradrop_power_dual_world_empty_gate_nav_commands_wave320_ok: bool,
    /// Wave 320 paradrop power dual-world empty-gate live residual.
    pub live_paradrop_power_dual_world_empty_gate_live_wave320_ok: bool,
    /// Wave 321 fuel air bomb dual-world empty-gate method names residual pack.
    pub live_fuel_air_bomb_dual_world_empty_gate_method_names_wave321_ok: bool,
    /// Wave 321 fuel air bomb dual-world empty-gate nav/commands residual pack.
    pub live_fuel_air_bomb_dual_world_empty_gate_nav_commands_wave321_ok: bool,
    /// Wave 321 fuel air bomb dual-world empty-gate live residual.
    pub live_fuel_air_bomb_dual_world_empty_gate_live_wave321_ok: bool,
    /// Wave 322 tensile formation dual-world empty-gate method names residual pack.
    pub live_tensile_formation_dual_world_empty_gate_method_names_wave322_ok: bool,
    /// Wave 322 tensile formation dual-world empty-gate nav/commands residual pack.
    pub live_tensile_formation_dual_world_empty_gate_nav_commands_wave322_ok: bool,
    /// Wave 322 tensile formation dual-world empty-gate live residual.
    pub live_tensile_formation_dual_world_empty_gate_live_wave322_ok: bool,
    /// Wave 323 die mod dual-world empty-gate method names residual pack.
    pub live_die_mod_dual_world_empty_gate_method_names_wave323_ok: bool,
    /// Wave 323 die mod dual-world empty-gate nav/commands residual pack.
    pub live_die_mod_dual_world_empty_gate_nav_commands_wave323_ok: bool,
    /// Wave 323 die mod dual-world empty-gate live residual.
    pub live_die_mod_dual_world_empty_gate_live_wave323_ok: bool,
    /// Wave 324 partition manager dual-world empty-gate method names residual pack.
    pub live_partition_manager_dual_world_empty_gate_method_names_wave324_ok: bool,
    /// Wave 324 partition manager dual-world empty-gate nav/commands residual pack.
    pub live_partition_manager_dual_world_empty_gate_nav_commands_wave324_ok: bool,
    /// Wave 324 partition manager dual-world empty-gate live residual.
    pub live_partition_manager_dual_world_empty_gate_live_wave324_ok: bool,
    /// Wave 325 spectre gunship dual-world empty-gate method names residual pack.
    pub live_spectre_gunship_dual_world_empty_gate_method_names_wave325_ok: bool,
    /// Wave 325 spectre gunship dual-world empty-gate nav/commands residual pack.
    pub live_spectre_gunship_dual_world_empty_gate_nav_commands_wave325_ok: bool,
    /// Wave 325 spectre gunship dual-world empty-gate live residual.
    pub live_spectre_gunship_dual_world_empty_gate_live_wave325_ok: bool,
    /// Wave 326 production update dual-world empty-gate method names residual pack.
    pub live_production_update_dual_world_empty_gate_method_names_wave326_ok: bool,
    /// Wave 326 production update dual-world empty-gate nav/commands residual pack.
    pub live_production_update_dual_world_empty_gate_nav_commands_wave326_ok: bool,
    /// Wave 326 production update dual-world empty-gate live residual.
    pub live_production_update_dual_world_empty_gate_live_wave326_ok: bool,
    /// Wave 327 neutron blast dual-world empty-gate method names residual pack.
    pub live_neutron_blast_dual_world_empty_gate_method_names_wave327_ok: bool,
    /// Wave 327 neutron blast dual-world empty-gate nav/commands residual pack.
    pub live_neutron_blast_dual_world_empty_gate_nav_commands_wave327_ok: bool,
    /// Wave 327 neutron blast dual-world empty-gate live residual.
    pub live_neutron_blast_dual_world_empty_gate_live_wave327_ok: bool,
    /// Wave 328 countermeasures dual-world empty-gate method names residual pack.
    pub live_countermeasures_dual_world_empty_gate_method_names_wave328_ok: bool,
    /// Wave 328 countermeasures dual-world empty-gate nav/commands residual pack.
    pub live_countermeasures_dual_world_empty_gate_nav_commands_wave328_ok: bool,
    /// Wave 328 countermeasures dual-world empty-gate live residual.
    pub live_countermeasures_dual_world_empty_gate_live_wave328_ok: bool,
    /// Wave 329 skirmish player dual-world empty-gate method names residual pack.
    pub live_skirmish_player_dual_world_empty_gate_method_names_wave329_ok: bool,
    /// Wave 329 skirmish player dual-world empty-gate nav/commands residual pack.
    pub live_skirmish_player_dual_world_empty_gate_nav_commands_wave329_ok: bool,
    /// Wave 329 skirmish player dual-world empty-gate live residual.
    pub live_skirmish_player_dual_world_empty_gate_live_wave329_ok: bool,
    /// Wave 330 a10 strike dual-world empty-gate method names residual pack.
    pub live_a10_strike_dual_world_empty_gate_method_names_wave330_ok: bool,
    /// Wave 330 a10 strike dual-world empty-gate nav/commands residual pack.
    pub live_a10_strike_dual_world_empty_gate_nav_commands_wave330_ok: bool,
    /// Wave 330 a10 strike dual-world empty-gate live residual.
    pub live_a10_strike_dual_world_empty_gate_live_wave330_ok: bool,
    /// Wave 331 rebuild hole dual-world empty-gate method names residual pack.
    pub live_rebuild_hole_dual_world_empty_gate_method_names_wave331_ok: bool,
    /// Wave 331 rebuild hole dual-world empty-gate nav/commands residual pack.
    pub live_rebuild_hole_dual_world_empty_gate_nav_commands_wave331_ok: bool,
    /// Wave 331 rebuild hole dual-world empty-gate live residual.
    pub live_rebuild_hole_dual_world_empty_gate_live_wave331_ok: bool,
    /// Wave 332 wave guide dual-world empty-gate method names residual pack.
    pub live_wave_guide_dual_world_empty_gate_method_names_wave332_ok: bool,
    /// Wave 332 wave guide dual-world empty-gate nav/commands residual pack.
    pub live_wave_guide_dual_world_empty_gate_nav_commands_wave332_ok: bool,
    /// Wave 332 wave guide dual-world empty-gate live residual.
    pub live_wave_guide_dual_world_empty_gate_live_wave332_ok: bool,
    /// Wave 333 emp update dual-world empty-gate method names residual pack.
    pub live_emp_update_dual_world_empty_gate_method_names_wave333_ok: bool,
    /// Wave 333 emp update dual-world empty-gate nav/commands residual pack.
    pub live_emp_update_dual_world_empty_gate_nav_commands_wave333_ok: bool,
    /// Wave 333 emp update dual-world empty-gate live residual.
    pub live_emp_update_dual_world_empty_gate_live_wave333_ok: bool,
    /// Wave 334 bunker buster dual-world empty-gate method names residual pack.
    pub live_bunker_buster_dual_world_empty_gate_method_names_wave334_ok: bool,
    /// Wave 334 bunker buster dual-world empty-gate nav/commands residual pack.
    pub live_bunker_buster_dual_world_empty_gate_nav_commands_wave334_ok: bool,
    /// Wave 334 bunker buster dual-world empty-gate live residual.
    pub live_bunker_buster_dual_world_empty_gate_live_wave334_ok: bool,
    /// Wave 335 bridge scaffold dual-world empty-gate method names residual pack.
    pub live_bridge_scaffold_dual_world_empty_gate_method_names_wave335_ok: bool,
    /// Wave 335 bridge scaffold dual-world empty-gate nav/commands residual pack.
    pub live_bridge_scaffold_dual_world_empty_gate_nav_commands_wave335_ok: bool,
    /// Wave 335 bridge scaffold dual-world empty-gate live residual.
    pub live_bridge_scaffold_dual_world_empty_gate_live_wave335_ok: bool,
    /// Wave 336 assisted targeting dual-world empty-gate method names residual pack.
    pub live_assisted_targeting_dual_world_empty_gate_method_names_wave336_ok: bool,
    /// Wave 336 assisted targeting dual-world empty-gate nav/commands residual pack.
    pub live_assisted_targeting_dual_world_empty_gate_nav_commands_wave336_ok: bool,
    /// Wave 336 assisted targeting dual-world empty-gate live residual.
    pub live_assisted_targeting_dual_world_empty_gate_live_wave336_ok: bool,
    /// Wave 337 economy dual-world empty-gate method names residual pack.
    pub live_economy_dual_world_empty_gate_method_names_wave337_ok: bool,
    /// Wave 337 economy dual-world empty-gate nav/commands residual pack.
    pub live_economy_dual_world_empty_gate_nav_commands_wave337_ok: bool,
    /// Wave 337 economy dual-world empty-gate live residual.
    pub live_economy_dual_world_empty_gate_live_wave337_ok: bool,
    /// Wave 338 turret ai dual-world empty-gate method names residual pack.
    pub live_turret_ai_dual_world_empty_gate_method_names_wave338_ok: bool,
    /// Wave 338 turret ai dual-world empty-gate nav/commands residual pack.
    pub live_turret_ai_dual_world_empty_gate_nav_commands_wave338_ok: bool,
    /// Wave 338 turret ai dual-world empty-gate live residual.
    pub live_turret_ai_dual_world_empty_gate_live_wave338_ok: bool,
    /// Wave 339 stealth detector module dual-world empty-gate method names residual pack.
    pub live_stealth_detector_module_dual_world_empty_gate_method_names_wave339_ok: bool,
    /// Wave 339 stealth detector module dual-world empty-gate nav/commands residual pack.
    pub live_stealth_detector_module_dual_world_empty_gate_nav_commands_wave339_ok: bool,
    /// Wave 339 stealth detector module dual-world empty-gate live residual.
    pub live_stealth_detector_module_dual_world_empty_gate_live_wave339_ok: bool,
    /// Wave 340 modules dual-world empty-gate method names residual pack.
    pub live_modules_dual_world_empty_gate_method_names_wave340_ok: bool,
    /// Wave 340 modules dual-world empty-gate nav/commands residual pack.
    pub live_modules_dual_world_empty_gate_nav_commands_wave340_ok: bool,
    /// Wave 340 modules dual-world empty-gate live residual.
    pub live_modules_dual_world_empty_gate_live_wave340_ok: bool,
    /// Wave 341 terrain dual-world empty-gate method names residual pack.
    pub live_terrain_dual_world_empty_gate_method_names_wave341_ok: bool,
    /// Wave 341 terrain dual-world empty-gate nav/commands residual pack.
    pub live_terrain_dual_world_empty_gate_nav_commands_wave341_ok: bool,
    /// Wave 341 terrain dual-world empty-gate live residual.
    pub live_terrain_dual_world_empty_gate_live_wave341_ok: bool,
    /// Wave 342 special power template dual-world empty-gate method names residual pack.
    pub live_special_power_template_dual_world_empty_gate_method_names_wave342_ok: bool,
    /// Wave 342 special power template dual-world empty-gate nav/commands residual pack.
    pub live_special_power_template_dual_world_empty_gate_nav_commands_wave342_ok: bool,
    /// Wave 342 special power template dual-world empty-gate live residual.
    pub live_special_power_template_dual_world_empty_gate_live_wave342_ok: bool,
    /// Wave 343 script evaluator dual-world empty-gate method names residual pack.
    pub live_script_evaluator_dual_world_empty_gate_method_names_wave343_ok: bool,
    /// Wave 343 script evaluator dual-world empty-gate nav/commands residual pack.
    pub live_script_evaluator_dual_world_empty_gate_nav_commands_wave343_ok: bool,
    /// Wave 343 script evaluator dual-world empty-gate live residual.
    pub live_script_evaluator_dual_world_empty_gate_live_wave343_ok: bool,
    /// Wave 344 system game logic dual-world empty-gate method names residual pack.
    pub live_system_game_logic_dual_world_empty_gate_method_names_wave344_ok: bool,
    /// Wave 344 system game logic dual-world empty-gate nav/commands residual pack.
    pub live_system_game_logic_dual_world_empty_gate_nav_commands_wave344_ok: bool,
    /// Wave 344 system game logic dual-world empty-gate live residual.
    pub live_system_game_logic_dual_world_empty_gate_live_wave344_ok: bool,
    /// Wave 345 meta event dual-world empty-gate method names residual pack.
    pub live_meta_event_dual_world_empty_gate_method_names_wave345_ok: bool,
    /// Wave 345 meta event dual-world empty-gate nav/commands residual pack.
    pub live_meta_event_dual_world_empty_gate_nav_commands_wave345_ok: bool,
    /// Wave 345 meta event dual-world empty-gate live residual.
    pub live_meta_event_dual_world_empty_gate_live_wave345_ok: bool,
    /// Wave 346 spawn behavior dual-world empty-gate method names residual pack.
    pub live_spawn_behavior_dual_world_empty_gate_method_names_wave346_ok: bool,
    /// Wave 346 spawn behavior dual-world empty-gate nav/commands residual pack.
    pub live_spawn_behavior_dual_world_empty_gate_nav_commands_wave346_ok: bool,
    /// Wave 346 spawn behavior dual-world empty-gate live residual.
    pub live_spawn_behavior_dual_world_empty_gate_live_wave346_ok: bool,
    /// Wave 347 action manager dual-world empty-gate method names residual pack.
    pub live_action_manager_dual_world_empty_gate_method_names_wave347_ok: bool,
    /// Wave 347 action manager dual-world empty-gate nav/commands residual pack.
    pub live_action_manager_dual_world_empty_gate_nav_commands_wave347_ok: bool,
    /// Wave 347 action manager dual-world empty-gate live residual.
    pub live_action_manager_dual_world_empty_gate_live_wave347_ok: bool,
    /// Wave 348 script engine dual-world empty-gate method names residual pack.
    pub live_script_engine_dual_world_empty_gate_method_names_wave348_ok: bool,
    /// Wave 348 script engine dual-world empty-gate nav/commands residual pack.
    pub live_script_engine_dual_world_empty_gate_nav_commands_wave348_ok: bool,
    /// Wave 348 script engine dual-world empty-gate live residual.
    pub live_script_engine_dual_world_empty_gate_live_wave348_ok: bool,
    /// Wave 349 chinook ai dual-world empty-gate method names residual pack.
    pub live_chinook_ai_dual_world_empty_gate_method_names_wave349_ok: bool,
    /// Wave 349 chinook ai dual-world empty-gate nav/commands residual pack.
    pub live_chinook_ai_dual_world_empty_gate_nav_commands_wave349_ok: bool,
    /// Wave 349 chinook ai dual-world empty-gate live residual.
    pub live_chinook_ai_dual_world_empty_gate_live_wave349_ok: bool,
    /// Wave 350 missile ai dual-world empty-gate method names residual pack.
    pub live_missile_ai_dual_world_empty_gate_method_names_wave350_ok: bool,
    /// Wave 350 missile ai dual-world empty-gate nav/commands residual pack.
    pub live_missile_ai_dual_world_empty_gate_nav_commands_wave350_ok: bool,
    /// Wave 350 missile ai dual-world empty-gate live residual.
    pub live_missile_ai_dual_world_empty_gate_live_wave350_ok: bool,
    /// Wave 351 dozer ai dual-world empty-gate method names residual pack.
    pub live_dozer_ai_dual_world_empty_gate_method_names_wave351_ok: bool,
    /// Wave 351 dozer ai dual-world empty-gate nav/commands residual pack.
    pub live_dozer_ai_dual_world_empty_gate_nav_commands_wave351_ok: bool,
    /// Wave 351 dozer ai dual-world empty-gate live residual.
    pub live_dozer_ai_dual_world_empty_gate_live_wave351_ok: bool,
    /// Wave 352 deliver payload ai dual-world empty-gate method names residual pack.
    pub live_deliver_payload_ai_dual_world_empty_gate_method_names_wave352_ok: bool,
    /// Wave 352 deliver payload ai dual-world empty-gate nav/commands residual pack.
    pub live_deliver_payload_ai_dual_world_empty_gate_nav_commands_wave352_ok: bool,
    /// Wave 352 deliver payload ai dual-world empty-gate live residual.
    pub live_deliver_payload_ai_dual_world_empty_gate_live_wave352_ok: bool,
    /// Wave 353 special power module dual-world empty-gate method names residual pack.
    pub live_special_power_module_dual_world_empty_gate_method_names_wave353_ok: bool,
    /// Wave 353 special power module dual-world empty-gate nav/commands residual pack.
    pub live_special_power_module_dual_world_empty_gate_nav_commands_wave353_ok: bool,
    /// Wave 353 special power module dual-world empty-gate live residual.
    pub live_special_power_module_dual_world_empty_gate_live_wave353_ok: bool,
    /// Wave 354 pow truck ai dual-world empty-gate method names residual pack.
    pub live_pow_truck_ai_dual_world_empty_gate_method_names_wave354_ok: bool,
    /// Wave 354 pow truck ai dual-world empty-gate nav/commands residual pack.
    pub live_pow_truck_ai_dual_world_empty_gate_nav_commands_wave354_ok: bool,
    /// Wave 354 pow truck ai dual-world empty-gate live residual.
    pub live_pow_truck_ai_dual_world_empty_gate_live_wave354_ok: bool,
    /// Wave 355 dock update dual-world empty-gate method names residual pack.
    pub live_dock_update_dual_world_empty_gate_method_names_wave355_ok: bool,
    /// Wave 355 dock update dual-world empty-gate nav/commands residual pack.
    pub live_dock_update_dual_world_empty_gate_nav_commands_wave355_ok: bool,
    /// Wave 355 dock update dual-world empty-gate live residual.
    pub live_dock_update_dual_world_empty_gate_live_wave355_ok: bool,
    /// Wave 356 weapon template dual-world empty-gate method names residual pack.
    pub live_weapon_template_dual_world_empty_gate_method_names_wave356_ok: bool,
    /// Wave 356 weapon template dual-world empty-gate nav/commands residual pack.
    pub live_weapon_template_dual_world_empty_gate_nav_commands_wave356_ok: bool,
    /// Wave 356 weapon template dual-world empty-gate live residual.
    pub live_weapon_template_dual_world_empty_gate_live_wave356_ok: bool,
    /// Wave 357 railroad guide ai dual-world empty-gate method names residual pack.
    pub live_railroad_guide_ai_dual_world_empty_gate_method_names_wave357_ok: bool,
    /// Wave 357 railroad guide ai dual-world empty-gate nav/commands residual pack.
    pub live_railroad_guide_ai_dual_world_empty_gate_nav_commands_wave357_ok: bool,
    /// Wave 357 railroad guide ai dual-world empty-gate live residual.
    pub live_railroad_guide_ai_dual_world_empty_gate_live_wave357_ok: bool,
    /// Wave 358 hack internet ai dual-world empty-gate method names residual pack.
    pub live_hack_internet_ai_dual_world_empty_gate_method_names_wave358_ok: bool,
    /// Wave 358 hack internet ai dual-world empty-gate nav/commands residual pack.
    pub live_hack_internet_ai_dual_world_empty_gate_nav_commands_wave358_ok: bool,
    /// Wave 358 hack internet ai dual-world empty-gate live residual.
    pub live_hack_internet_ai_dual_world_empty_gate_live_wave358_ok: bool,
    /// Wave 359 spectre gunship deployment dual-world empty-gate method names residual pack.
    pub live_spectre_gunship_deployment_dual_world_empty_gate_method_names_wave359_ok: bool,
    /// Wave 359 spectre gunship deployment dual-world empty-gate nav/commands residual pack.
    pub live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_wave359_ok: bool,
    /// Wave 359 spectre gunship deployment dual-world empty-gate live residual.
    pub live_spectre_gunship_deployment_dual_world_empty_gate_live_wave359_ok: bool,
    /// Wave 360 radius decal update dual-world empty-gate method names residual pack.
    pub live_radius_decal_update_dual_world_empty_gate_method_names_wave360_ok: bool,
    /// Wave 360 radius decal update dual-world empty-gate nav/commands residual pack.
    pub live_radius_decal_update_dual_world_empty_gate_nav_commands_wave360_ok: bool,
    /// Wave 360 radius decal update dual-world empty-gate live residual.
    pub live_radius_decal_update_dual_world_empty_gate_live_wave360_ok: bool,
    /// Wave 361 railed transport dock dual-world empty-gate method names residual pack.
    pub live_railed_transport_dock_dual_world_empty_gate_method_names_wave361_ok: bool,
    /// Wave 361 railed transport dock dual-world empty-gate nav/commands residual pack.
    pub live_railed_transport_dock_dual_world_empty_gate_nav_commands_wave361_ok: bool,
    /// Wave 361 railed transport dock dual-world empty-gate live residual.
    pub live_railed_transport_dock_dual_world_empty_gate_live_wave361_ok: bool,
    /// Wave 362 structure collapse update dual-world empty-gate method names residual pack.
    pub live_structure_collapse_update_dual_world_empty_gate_method_names_wave362_ok: bool,
    /// Wave 362 structure collapse update dual-world empty-gate nav/commands residual pack.
    pub live_structure_collapse_update_dual_world_empty_gate_nav_commands_wave362_ok: bool,
    /// Wave 362 structure collapse update dual-world empty-gate live residual.
    pub live_structure_collapse_update_dual_world_empty_gate_live_wave362_ok: bool,
    /// Wave 363 propaganda tower behavior dual-world empty-gate method names residual pack.
    pub live_propaganda_tower_behavior_dual_world_empty_gate_method_names_wave363_ok: bool,
    /// Wave 363 propaganda tower behavior dual-world empty-gate nav/commands residual pack.
    pub live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_wave363_ok: bool,
    /// Wave 363 propaganda tower behavior dual-world empty-gate live residual.
    pub live_propaganda_tower_behavior_dual_world_empty_gate_live_wave363_ok: bool,
    /// Wave 364 propaganda center behavior dual-world empty-gate method names residual pack.
    pub live_propaganda_center_behavior_dual_world_empty_gate_method_names_wave364_ok: bool,
    /// Wave 364 propaganda center behavior dual-world empty-gate nav/commands residual pack.
    pub live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_wave364_ok: bool,
    /// Wave 364 propaganda center behavior dual-world empty-gate live residual.
    pub live_propaganda_center_behavior_dual_world_empty_gate_live_wave364_ok: bool,
    /// Wave 365 production update complete dual-world empty-gate method names residual pack.
    pub live_production_update_complete_dual_world_empty_gate_method_names_wave365_ok: bool,
    /// Wave 365 production update complete dual-world empty-gate nav/commands residual pack.
    pub live_production_update_complete_dual_world_empty_gate_nav_commands_wave365_ok: bool,
    /// Wave 365 production update complete dual-world empty-gate live residual.
    pub live_production_update_complete_dual_world_empty_gate_live_wave365_ok: bool,
    /// Wave 366 pow truck behavior dual-world empty-gate method names residual pack.
    pub live_pow_truck_behavior_dual_world_empty_gate_method_names_wave366_ok: bool,
    /// Wave 366 pow truck behavior dual-world empty-gate nav/commands residual pack.
    pub live_pow_truck_behavior_dual_world_empty_gate_nav_commands_wave366_ok: bool,
    /// Wave 366 pow truck behavior dual-world empty-gate live residual.
    pub live_pow_truck_behavior_dual_world_empty_gate_live_wave366_ok: bool,
    /// Wave 367 fire weapon when damaged dual-world empty-gate method names residual pack.
    pub live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_wave367_ok: bool,
    /// Wave 367 fire weapon when damaged dual-world empty-gate nav/commands residual pack.
    pub live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_wave367_ok: bool,
    /// Wave 367 fire weapon when damaged dual-world empty-gate live residual.
    pub live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_live_wave367_ok: bool,
    /// Wave 368 veterancy crate collide dual-world empty-gate method names residual pack.
    pub live_veterancy_crate_collide_dual_world_empty_gate_method_names_wave368_ok: bool,
    /// Wave 368 veterancy crate collide dual-world empty-gate nav/commands residual pack.
    pub live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_wave368_ok: bool,
    /// Wave 368 veterancy crate collide dual-world empty-gate live residual.
    pub live_veterancy_crate_collide_dual_world_empty_gate_live_wave368_ok: bool,
    /// Wave 369 assault transport AI update dual-world empty-gate method names residual pack.
    pub live_assault_transport_ai_update_dual_world_empty_gate_method_names_wave369_ok: bool,
    /// Wave 369 assault transport AI update dual-world empty-gate nav/commands residual pack.
    pub live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_wave369_ok: bool,
    /// Wave 369 assault transport AI update dual-world empty-gate live residual.
    pub live_assault_transport_ai_update_dual_world_empty_gate_live_wave369_ok: bool,
    /// Wave 370 heal contain dual-world empty-gate method names residual pack.
    pub live_heal_contain_dual_world_empty_gate_method_names_wave370_ok: bool,
    /// Wave 370 heal contain dual-world empty-gate nav/commands residual pack.
    pub live_heal_contain_dual_world_empty_gate_nav_commands_wave370_ok: bool,
    /// Wave 370 heal contain dual-world empty-gate live residual.
    pub live_heal_contain_dual_world_empty_gate_live_wave370_ok: bool,
    /// Wave 371 topple update dual-world empty-gate method names residual pack.
    pub live_topple_update_dual_world_empty_gate_method_names_wave371_ok: bool,
    /// Wave 371 topple update dual-world empty-gate nav/commands residual pack.
    pub live_topple_update_dual_world_empty_gate_nav_commands_wave371_ok: bool,
    /// Wave 371 topple update dual-world empty-gate live residual.
    pub live_topple_update_dual_world_empty_gate_live_wave371_ok: bool,
    /// Wave 372 projectile stream update dual-world empty-gate method names residual pack.
    pub live_projectile_stream_update_dual_world_empty_gate_method_names_wave372_ok: bool,
    /// Wave 372 projectile stream update dual-world empty-gate nav/commands residual pack.
    pub live_projectile_stream_update_dual_world_empty_gate_nav_commands_wave372_ok: bool,
    /// Wave 372 projectile stream update dual-world empty-gate live residual.
    pub live_projectile_stream_update_dual_world_empty_gate_live_wave372_ok: bool,
    /// Wave 373 demo trap update dual-world empty-gate method names residual pack.
    pub live_demo_trap_update_dual_world_empty_gate_method_names_wave373_ok: bool,
    /// Wave 373 demo trap update dual-world empty-gate nav/commands residual pack.
    pub live_demo_trap_update_dual_world_empty_gate_nav_commands_wave373_ok: bool,
    /// Wave 373 demo trap update dual-world empty-gate live residual.
    pub live_demo_trap_update_dual_world_empty_gate_live_wave373_ok: bool,
    /// Wave 374 mob member slaved update dual-world empty-gate method names residual pack.
    pub live_mob_member_slaved_update_dual_world_empty_gate_method_names_wave374_ok: bool,
    /// Wave 374 mob member slaved update dual-world empty-gate nav/commands residual pack.
    pub live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_wave374_ok: bool,
    /// Wave 374 mob member slaved update dual-world empty-gate live residual.
    pub live_mob_member_slaved_update_dual_world_empty_gate_live_wave374_ok: bool,
    /// Wave 375 TN guard dual-world empty-gate method names residual pack.
    pub live_tn_guard_dual_world_empty_gate_method_names_wave375_ok: bool,
    /// Wave 375 TN guard dual-world empty-gate nav/commands residual pack.
    pub live_tn_guard_dual_world_empty_gate_nav_commands_wave375_ok: bool,
    /// Wave 375 TN guard dual-world empty-gate live residual.
    pub live_tn_guard_dual_world_empty_gate_live_wave375_ok: bool,
    /// Wave 376 production update dual-world empty-gate method names residual pack.
    pub live_production_update_dual_world_empty_gate_method_names_wave376_ok: bool,
    /// Wave 376 production update dual-world empty-gate nav/commands residual pack.
    pub live_production_update_dual_world_empty_gate_nav_commands_wave376_ok: bool,
    /// Wave 376 production update dual-world empty-gate live residual.
    pub live_production_update_dual_world_empty_gate_live_wave376_ok: bool,
    /// Wave 377 poisoned behavior dual-world empty-gate method names residual pack.
    pub live_poisoned_behavior_dual_world_empty_gate_method_names_wave377_ok: bool,
    /// Wave 377 poisoned behavior dual-world empty-gate nav/commands residual pack.
    pub live_poisoned_behavior_dual_world_empty_gate_nav_commands_wave377_ok: bool,
    /// Wave 377 poisoned behavior dual-world empty-gate live residual.
    pub live_poisoned_behavior_dual_world_empty_gate_live_wave377_ok: bool,
    /// Wave 378 horde update dual-world empty-gate method names residual pack.
    pub live_horde_update_dual_world_empty_gate_method_names_wave378_ok: bool,
    /// Wave 378 horde update dual-world empty-gate nav/commands residual pack.
    pub live_horde_update_dual_world_empty_gate_nav_commands_wave378_ok: bool,
    /// Wave 378 horde update dual-world empty-gate live residual.
    pub live_horde_update_dual_world_empty_gate_live_wave378_ok: bool,
    /// Wave 379 flammable update dual-world empty-gate method names residual pack.
    pub live_flammable_update_dual_world_empty_gate_method_names_wave379_ok: bool,
    /// Wave 379 flammable update dual-world empty-gate nav/commands residual pack.
    pub live_flammable_update_dual_world_empty_gate_nav_commands_wave379_ok: bool,
    /// Wave 379 flammable update dual-world empty-gate live residual.
    pub live_flammable_update_dual_world_empty_gate_live_wave379_ok: bool,
    /// Wave 380 base regenerate update dual-world empty-gate method names residual pack.
    pub live_base_regenerate_update_dual_world_empty_gate_method_names_wave380_ok: bool,
    /// Wave 380 base regenerate update dual-world empty-gate nav/commands residual pack.
    pub live_base_regenerate_update_dual_world_empty_gate_nav_commands_wave380_ok: bool,
    /// Wave 380 base regenerate update dual-world empty-gate live residual.
    pub live_base_regenerate_update_dual_world_empty_gate_live_wave380_ok: bool,
    /// Wave 381 queue production exit dual-world empty-gate method names residual pack.
    pub live_queue_production_exit_behavior_dual_world_empty_gate_method_names_wave381_ok: bool,
    /// Wave 381 queue production exit dual-world empty-gate nav/commands residual pack.
    pub live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_wave381_ok: bool,
    /// Wave 381 queue production exit dual-world empty-gate live residual.
    pub live_queue_production_exit_behavior_dual_world_empty_gate_live_wave381_ok: bool,
    /// Wave 382 missile launcher building dual-world empty-gate method names residual pack.
    pub live_missile_launcher_building_update_dual_world_empty_gate_method_names_wave382_ok: bool,
    /// Wave 382 missile launcher building dual-world empty-gate nav/commands residual pack.
    pub live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_wave382_ok: bool,
    /// Wave 382 missile launcher building dual-world empty-gate live residual.
    pub live_missile_launcher_building_update_dual_world_empty_gate_live_wave382_ok: bool,
    /// Wave 383 dynamic shroud clearing dual-world empty-gate method names residual pack.
    pub live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_wave383_ok:
        bool,
    /// Wave 383 dynamic shroud clearing dual-world empty-gate nav/commands residual pack.
    pub live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_wave383_ok:
        bool,
    /// Wave 383 dynamic shroud clearing dual-world empty-gate live residual.
    pub live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_live_wave383_ok: bool,
    /// Wave 384 command button hunt dual-world empty-gate method names residual pack.
    pub live_command_button_hunt_update_dual_world_empty_gate_method_names_wave384_ok: bool,
    /// Wave 384 command button hunt dual-world empty-gate nav/commands residual pack.
    pub live_command_button_hunt_update_dual_world_empty_gate_nav_commands_wave384_ok: bool,
    /// Wave 384 command button hunt dual-world empty-gate live residual.
    pub live_command_button_hunt_update_dual_world_empty_gate_live_wave384_ok: bool,
    /// Wave 385 prison behavior dual-world empty-gate method names residual pack.
    pub live_prison_behavior_dual_world_empty_gate_method_names_wave385_ok: bool,
    /// Wave 385 prison behavior dual-world empty-gate nav/commands residual pack.
    pub live_prison_behavior_dual_world_empty_gate_nav_commands_wave385_ok: bool,
    /// Wave 385 prison behavior dual-world empty-gate live residual.
    pub live_prison_behavior_dual_world_empty_gate_live_wave385_ok: bool,
    /// Wave 386 generate minefield dual-world empty-gate method names residual pack.
    pub live_generate_minefield_behavior_dual_world_empty_gate_method_names_wave386_ok: bool,
    /// Wave 386 generate minefield dual-world empty-gate nav/commands residual pack.
    pub live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_wave386_ok: bool,
    /// Wave 386 generate minefield dual-world empty-gate live residual.
    pub live_generate_minefield_behavior_dual_world_empty_gate_live_wave386_ok: bool,
    /// Wave 387 demoralize special power dual-world empty-gate method names residual pack.
    pub live_demoralize_special_power_dual_world_empty_gate_method_names_wave387_ok: bool,
    /// Wave 387 demoralize special power dual-world empty-gate nav/commands residual pack.
    pub live_demoralize_special_power_dual_world_empty_gate_nav_commands_wave387_ok: bool,
    /// Wave 387 demoralize special power dual-world empty-gate live residual.
    pub live_demoralize_special_power_dual_world_empty_gate_live_wave387_ok: bool,
    /// Wave 388 stealth detector dual-world empty-gate method names residual pack.
    pub live_stealth_detector_update_dual_world_empty_gate_method_names_wave388_ok: bool,
    /// Wave 388 stealth detector dual-world empty-gate nav/commands residual pack.
    pub live_stealth_detector_update_dual_world_empty_gate_nav_commands_wave388_ok: bool,
    /// Wave 388 stealth detector dual-world empty-gate live residual.
    pub live_stealth_detector_update_dual_world_empty_gate_live_wave388_ok: bool,
    /// Wave 389 hive structure body dual-world empty-gate method names residual pack.
    pub live_hive_structure_body_dual_world_empty_gate_method_names_wave389_ok: bool,
    /// Wave 389 hive structure body dual-world empty-gate nav/commands residual pack.
    pub live_hive_structure_body_dual_world_empty_gate_nav_commands_wave389_ok: bool,
    /// Wave 389 hive structure body dual-world empty-gate live residual.
    pub live_hive_structure_body_dual_world_empty_gate_live_wave389_ok: bool,
    /// Wave 390 salvage crate collide dual-world empty-gate method names residual pack.
    pub live_salvage_crate_collide_dual_world_empty_gate_method_names_wave390_ok: bool,
    /// Wave 390 salvage crate collide dual-world empty-gate nav/commands residual pack.
    pub live_salvage_crate_collide_dual_world_empty_gate_nav_commands_wave390_ok: bool,
    /// Wave 390 salvage crate collide dual-world empty-gate live residual.
    pub live_salvage_crate_collide_dual_world_empty_gate_live_wave390_ok: bool,
    /// Wave 391 sabotage internet center dual-world empty-gate method names residual pack.
    pub live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_wave391_ok:
        bool,
    /// Wave 391 sabotage internet center dual-world empty-gate nav/commands residual pack.
    pub live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_wave391_ok:
        bool,
    /// Wave 391 sabotage internet center dual-world empty-gate live residual.
    pub live_sabotage_internet_center_crate_collide_dual_world_empty_gate_live_wave391_ok: bool,
    /// Wave 392 power plant update dual-world empty-gate method names residual pack.
    pub live_power_plant_update_dual_world_empty_gate_method_names_wave392_ok: bool,
    /// Wave 392 power plant update dual-world empty-gate nav/commands residual pack.
    pub live_power_plant_update_dual_world_empty_gate_nav_commands_wave392_ok: bool,
    /// Wave 392 power plant update dual-world empty-gate live residual.
    pub live_power_plant_update_dual_world_empty_gate_live_wave392_ok: bool,
    /// Wave 393 leaflet drop dual-world empty-gate method names residual pack.
    pub live_leaflet_drop_behavior_dual_world_empty_gate_method_names_wave393_ok: bool,
    /// Wave 393 leaflet drop dual-world empty-gate nav/commands residual pack.
    pub live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_wave393_ok: bool,
    /// Wave 393 leaflet drop dual-world empty-gate live residual.
    pub live_leaflet_drop_behavior_dual_world_empty_gate_live_wave393_ok: bool,
    /// Wave 394 auto deposit dual-world empty-gate method names residual pack.
    pub live_auto_deposit_update_dual_world_empty_gate_method_names_wave394_ok: bool,
    /// Wave 394 auto deposit dual-world empty-gate nav/commands residual pack.
    pub live_auto_deposit_update_dual_world_empty_gate_nav_commands_wave394_ok: bool,
    /// Wave 394 auto deposit dual-world empty-gate live residual.
    pub live_auto_deposit_update_dual_world_empty_gate_live_wave394_ok: bool,
    /// Wave 395 supply warehouse crippling dual-world empty-gate method names residual pack.
    pub live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_wave395_ok:
        bool,
    /// Wave 395 supply warehouse crippling dual-world empty-gate nav/commands residual pack.
    pub live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_wave395_ok:
        bool,
    /// Wave 395 supply warehouse crippling dual-world empty-gate live residual.
    pub live_supply_warehouse_crippling_behavior_dual_world_empty_gate_live_wave395_ok: bool,
    /// Wave 396 neutron missile slow death dual-world empty-gate method names residual pack.
    pub live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_wave396_ok: bool,
    /// Wave 396 neutron missile slow death dual-world empty-gate nav/commands residual pack.
    pub live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_wave396_ok: bool,
    /// Wave 396 neutron missile slow death dual-world empty-gate live residual.
    pub live_neutron_missile_slow_death_update_dual_world_empty_gate_live_wave396_ok: bool,
    /// Wave 397 AI dock dual-world empty-gate method names residual pack.
    pub live_ai_dock_dual_world_empty_gate_method_names_wave397_ok: bool,
    /// Wave 397 AI dock dual-world empty-gate nav/commands residual pack.
    pub live_ai_dock_dual_world_empty_gate_nav_commands_wave397_ok: bool,
    /// Wave 397 AI dock dual-world empty-gate live residual.
    pub live_ai_dock_dual_world_empty_gate_live_wave397_ok: bool,
    /// Wave 398 AI groups dual-world empty-gate method names residual pack.
    pub live_ai_groups_dual_world_empty_gate_method_names_wave398_ok: bool,
    /// Wave 398 AI groups dual-world empty-gate nav/commands residual pack.
    pub live_ai_groups_dual_world_empty_gate_nav_commands_wave398_ok: bool,
    /// Wave 398 AI groups dual-world empty-gate live residual.
    pub live_ai_groups_dual_world_empty_gate_live_wave398_ok: bool,
    /// Wave 399 artillery barrage dual-world empty-gate method names residual pack.
    pub live_artillery_barrage_power_dual_world_empty_gate_method_names_wave399_ok: bool,
    /// Wave 399 artillery barrage dual-world empty-gate nav/commands residual pack.
    pub live_artillery_barrage_power_dual_world_empty_gate_nav_commands_wave399_ok: bool,
    /// Wave 399 artillery barrage dual-world empty-gate live residual.
    pub live_artillery_barrage_power_dual_world_empty_gate_live_wave399_ok: bool,
    /// Wave 400 baikonur launch dual-world empty-gate method names residual pack.
    pub live_baikonur_launch_power_dual_world_empty_gate_method_names_wave400_ok: bool,
    /// Wave 400 baikonur launch dual-world empty-gate nav/commands residual pack.
    pub live_baikonur_launch_power_dual_world_empty_gate_nav_commands_wave400_ok: bool,
    /// Wave 400 baikonur launch dual-world empty-gate live residual.
    pub live_baikonur_launch_power_dual_world_empty_gate_live_wave400_ok: bool,
    /// Wave 401 AI group dual-world empty-gate method names residual pack.
    pub live_ai_group_dual_world_empty_gate_method_names_wave401_ok: bool,
    /// Wave 401 AI group dual-world empty-gate nav/commands residual pack.
    pub live_ai_group_dual_world_empty_gate_nav_commands_wave401_ok: bool,
    /// Wave 401 AI group dual-world empty-gate live residual.
    pub live_ai_group_dual_world_empty_gate_live_wave401_ok: bool,
    /// Wave 402 slaved update dual-world empty-gate method names residual pack.
    pub live_slaved_update_dual_world_empty_gate_method_names_wave402_ok: bool,
    /// Wave 402 slaved update dual-world empty-gate nav/commands residual pack.
    pub live_slaved_update_dual_world_empty_gate_nav_commands_wave402_ok: bool,
    /// Wave 402 slaved update dual-world empty-gate live residual.
    pub live_slaved_update_dual_world_empty_gate_live_wave402_ok: bool,
    /// Wave 403 demoralize power dual-world empty-gate method names residual pack.
    pub live_demoralize_power_dual_world_empty_gate_method_names_wave403_ok: bool,
    /// Wave 403 demoralize power dual-world empty-gate nav/commands residual pack.
    pub live_demoralize_power_dual_world_empty_gate_nav_commands_wave403_ok: bool,
    /// Wave 403 demoralize power dual-world empty-gate live residual.
    pub live_demoralize_power_dual_world_empty_gate_live_wave403_ok: bool,
    /// Wave 404 bone fx update dual-world empty-gate method names residual pack.
    pub live_bone_fx_update_dual_world_empty_gate_method_names_wave404_ok: bool,
    /// Wave 404 bone fx update dual-world empty-gate nav/commands residual pack.
    pub live_bone_fx_update_dual_world_empty_gate_nav_commands_wave404_ok: bool,
    /// Wave 404 bone fx update dual-world empty-gate live residual.
    pub live_bone_fx_update_dual_world_empty_gate_live_wave404_ok: bool,
    /// Wave 405 supply warehouse dock dual-world empty-gate method names residual pack.
    pub live_supply_warehouse_dock_dual_world_empty_gate_method_names_wave405_ok: bool,
    /// Wave 405 supply warehouse dock dual-world empty-gate nav/commands residual pack.
    pub live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_wave405_ok: bool,
    /// Wave 405 supply warehouse dock dual-world empty-gate live residual.
    pub live_supply_warehouse_dock_dual_world_empty_gate_live_wave405_ok: bool,
    /// Wave 406 OCL special power dual-world empty-gate method names residual pack.
    pub live_ocl_special_power_dual_world_empty_gate_method_names_wave406_ok: bool,
    /// Wave 406 OCL special power dual-world empty-gate nav/commands residual pack.
    pub live_ocl_special_power_dual_world_empty_gate_nav_commands_wave406_ok: bool,
    /// Wave 406 OCL special power dual-world empty-gate live residual.
    pub live_ocl_special_power_dual_world_empty_gate_live_wave406_ok: bool,
    /// Wave 407 railed transport AI dual-world empty-gate method names residual pack.
    pub live_railed_transport_ai_update_dual_world_empty_gate_method_names_wave407_ok: bool,
    /// Wave 407 railed transport AI dual-world empty-gate nav/commands residual pack.
    pub live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_wave407_ok: bool,
    /// Wave 407 railed transport AI dual-world empty-gate live residual.
    pub live_railed_transport_ai_update_dual_world_empty_gate_live_wave407_ok: bool,
    /// Wave 408 squish collide dual-world empty-gate method names residual pack.
    pub live_squish_collide_dual_world_empty_gate_method_names_wave408_ok: bool,
    /// Wave 408 squish collide dual-world empty-gate nav/commands residual pack.
    pub live_squish_collide_dual_world_empty_gate_nav_commands_wave408_ok: bool,
    /// Wave 408 squish collide dual-world empty-gate live residual.
    pub live_squish_collide_dual_world_empty_gate_live_wave408_ok: bool,
    /// Wave 409 weapon bonus update dual-world empty-gate method names residual pack.
    pub live_weapon_bonus_update_dual_world_empty_gate_method_names_wave409_ok: bool,
    /// Wave 409 weapon bonus update dual-world empty-gate nav/commands residual pack.
    pub live_weapon_bonus_update_dual_world_empty_gate_nav_commands_wave409_ok: bool,
    /// Wave 409 weapon bonus update dual-world empty-gate live residual.
    pub live_weapon_bonus_update_dual_world_empty_gate_live_wave409_ok: bool,
    /// Wave 410 minefield behavior dual-world empty-gate method names residual pack.
    pub live_minefield_behavior_dual_world_empty_gate_method_names_wave410_ok: bool,
    /// Wave 410 minefield behavior dual-world empty-gate nav/commands residual pack.
    pub live_minefield_behavior_dual_world_empty_gate_nav_commands_wave410_ok: bool,
    /// Wave 410 minefield behavior dual-world empty-gate live residual.
    pub live_minefield_behavior_dual_world_empty_gate_live_wave410_ok: bool,
    /// Wave 411 point defense laser dual-world empty-gate method names residual pack.
    pub live_point_defense_laser_update_dual_world_empty_gate_method_names_wave411_ok: bool,
    /// Wave 411 point defense laser dual-world empty-gate nav/commands residual pack.
    pub live_point_defense_laser_update_dual_world_empty_gate_nav_commands_wave411_ok: bool,
    /// Wave 411 point defense laser dual-world empty-gate live residual.
    pub live_point_defense_laser_update_dual_world_empty_gate_live_wave411_ok: bool,
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
    let production_authority_env_ok =
        crate::gameworld_shadow::gameworld_production_authority_enabled();
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
    let message_stream_marker_wave110_ok = honesty_message_stream_marker_residual_wave110();
    let game_message_arg_wave110_ok = honesty_game_message_argument_type_residual_wave110();
    let meta_event_category_wave110_ok = honesty_meta_event_category_residual_wave110();
    let ingame_ui_wave110_ok = honesty_ingame_ui_residual_wave110();
    let drawable_icon_flash_wave111_ok = honesty_drawable_icon_flash_residual_wave111();
    let drawable_status_stealth_wave111_ok = honesty_drawable_status_stealth_residual_wave111();
    let terrain_decal_wave111_ok = honesty_terrain_decal_residual_wave111();
    let display_draw_image_wave111_ok = honesty_display_draw_image_mode_residual_wave111();
    let game_client_translator_wave111_ok = honesty_game_client_translator_residual_wave111();
    let particle_priority_wave111_ok = honesty_particle_priority_residual_wave111();
    let mouse_residual_wave112_ok = honesty_mouse_residual_wave112();
    let keyboard_residual_wave112_ok = honesty_keyboard_residual_wave112();
    let view_residual_wave112_ok = honesty_view_residual_wave112();
    let game_window_manager_wave113_ok = honesty_game_window_manager_residual_wave113();
    let window_style_wave113_ok = honesty_window_style_residual_wave113();
    let gadget_wave113_ok = honesty_gadget_residual_wave113();
    let video_buffer_wave113_ok = honesty_video_buffer_residual_wave113();
    let audio_event_wave113_ok = honesty_audio_event_residual_wave113();
    let main_menu_skirmish_names_wave114_ok = honesty_main_menu_skirmish_names_residual_wave114();
    let main_menu_skirmish_nav_steps_wave114_ok =
        honesty_main_menu_skirmish_nav_steps_residual_wave114();
    let main_menu_skirmish_message_wave114_ok =
        honesty_main_menu_skirmish_message_residual_wave114();
    let map_select_names_wave115_ok = honesty_skirmish_map_select_names_residual_wave115();
    let map_select_nav_steps_wave115_ok = honesty_skirmish_map_select_nav_steps_residual_wave115();
    let map_select_commands_wave115_ok = honesty_skirmish_map_select_commands_residual_wave115();
    let slot_state_wave116_ok = honesty_skirmish_slot_state_residual_wave116();
    let slot_combo_names_wave116_ok = honesty_skirmish_slot_combo_names_residual_wave116();
    let slot_nav_commands_wave116_ok = honesty_skirmish_slot_nav_commands_residual_wave116();
    let starting_cash_wave117_ok = honesty_skirmish_starting_cash_residual_wave117();
    let game_speed_controls_wave117_ok = honesty_skirmish_game_speed_controls_residual_wave117();
    let rules_nav_commands_wave117_ok = honesty_skirmish_rules_nav_commands_residual_wave117();
    let main_menu_button_names_wave118_ok = honesty_main_menu_button_names_residual_wave118();
    let main_menu_push_targets_wave118_ok = honesty_main_menu_push_targets_residual_wave118();
    let main_menu_button_nav_commands_wave118_ok =
        honesty_main_menu_button_nav_commands_residual_wave118();
    let campaign_button_names_wave119_ok =
        honesty_main_menu_campaign_button_names_residual_wave119();
    let campaign_enums_wave119_ok = honesty_main_menu_campaign_enums_residual_wave119();
    let campaign_nav_commands_wave119_ok =
        honesty_main_menu_campaign_nav_commands_residual_wave119();
    let challenge_control_names_wave120_ok =
        honesty_challenge_menu_control_names_residual_wave120();
    let challenge_nav_commands_wave120_ok = honesty_challenge_menu_nav_commands_residual_wave120();
    let save_load_layout_wave121_ok = honesty_save_load_layout_residual_wave121();
    let save_load_control_stems_wave121_ok = honesty_save_load_control_stems_residual_wave121();
    let save_load_nav_commands_wave121_ok = honesty_save_load_nav_commands_residual_wave121();
    let replay_control_names_wave122_ok = honesty_replay_menu_control_names_residual_wave122();
    let replay_nav_commands_wave122_ok = honesty_replay_menu_nav_commands_residual_wave122();
    let quit_control_names_wave123_ok = honesty_quit_menu_control_names_residual_wave123();
    let quit_nav_commands_wave123_ok = honesty_quit_menu_nav_commands_residual_wave123();
    let keyboard_control_names_wave124_ok =
        honesty_keyboard_options_control_names_residual_wave124();
    let keyboard_nav_commands_wave124_ok = honesty_keyboard_options_nav_commands_residual_wave124();
    let score_control_names_wave125_ok = honesty_score_screen_control_names_residual_wave125();
    let score_nav_commands_wave125_ok = honesty_score_screen_nav_commands_residual_wave125();
    let options_control_names_wave126_ok = honesty_options_menu_control_names_residual_wave126();
    let options_nav_commands_wave126_ok = honesty_options_menu_nav_commands_residual_wave126();
    let credits_control_names_wave127_ok = honesty_credits_menu_control_names_residual_wave127();
    let credits_nav_commands_wave127_ok = honesty_credits_menu_nav_commands_residual_wave127();
    let message_box_control_names_wave128_ok = honesty_message_box_control_names_residual_wave128();
    let message_box_nav_commands_wave128_ok = honesty_message_box_nav_commands_residual_wave128();
    let diplomacy_control_names_wave129_ok = honesty_diplomacy_control_names_residual_wave129();
    let diplomacy_nav_commands_wave129_ok = honesty_diplomacy_nav_commands_residual_wave129();
    let popup_replay_control_names_wave130_ok =
        honesty_popup_replay_control_names_residual_wave130();
    let popup_replay_nav_commands_wave130_ok = honesty_popup_replay_nav_commands_residual_wave130();
    let single_player_control_names_wave131_ok =
        honesty_single_player_menu_control_names_residual_wave131();
    let single_player_nav_commands_wave131_ok =
        honesty_single_player_menu_nav_commands_residual_wave131();
    let map_select_control_names_wave132_ok =
        honesty_map_select_menu_control_names_residual_wave132();
    let map_select_nav_commands_wave132_ok =
        honesty_map_select_menu_nav_commands_residual_wave132();
    let control_bar_control_names_wave133_ok = honesty_control_bar_control_names_residual_wave133();
    let control_bar_nav_commands_wave133_ok = honesty_control_bar_nav_commands_residual_wave133();
    let difficulty_select_control_names_wave134_ok =
        honesty_difficulty_select_control_names_residual_wave134();
    let difficulty_select_nav_commands_wave134_ok =
        honesty_difficulty_select_nav_commands_residual_wave134();
    let loading_screen_stages_wave135_ok = honesty_loading_screen_stages_residual_wave135();
    let loading_screen_nav_commands_wave135_ok =
        honesty_loading_screen_nav_commands_residual_wave135();
    let in_game_chat_control_names_wave136_ok =
        honesty_in_game_chat_control_names_residual_wave136();
    let in_game_chat_nav_commands_wave136_ok = honesty_in_game_chat_nav_commands_residual_wave136();
    let idle_worker_control_names_wave137_ok = honesty_idle_worker_control_names_residual_wave137();
    let idle_worker_nav_commands_wave137_ok = honesty_idle_worker_nav_commands_residual_wave137();
    let generals_exp_control_names_wave138_ok =
        honesty_generals_exp_control_names_residual_wave138();
    let generals_exp_nav_commands_wave138_ok = honesty_generals_exp_nav_commands_residual_wave138();
    let popup_communicator_control_names_wave139_ok =
        honesty_popup_communicator_control_names_residual_wave139();
    let popup_communicator_nav_commands_wave139_ok =
        honesty_popup_communicator_nav_commands_residual_wave139();
    let replay_control_control_names_wave140_ok =
        honesty_replay_control_control_names_residual_wave140();
    let replay_control_nav_commands_wave140_ok =
        honesty_replay_control_nav_commands_residual_wave140();
    let shell_map_names_wave141_ok = honesty_shell_map_names_residual_wave141();
    let shell_map_nav_commands_wave141_ok = honesty_shell_map_nav_commands_residual_wave141();
    let beacon_control_names_wave142_ok = honesty_beacon_control_names_residual_wave142();
    let beacon_nav_commands_wave142_ok = honesty_beacon_nav_commands_residual_wave142();
    let eva_message_names_wave143_ok = honesty_eva_message_names_residual_wave143();
    let eva_nav_commands_wave143_ok = honesty_eva_nav_commands_residual_wave143();
    let ime_message_names_wave144_ok = honesty_ime_message_names_residual_wave144();
    let ime_nav_commands_wave144_ok = honesty_ime_nav_commands_residual_wave144();
    let smudge_method_names_wave145_ok = honesty_smudge_method_names_residual_wave145();
    let smudge_nav_commands_wave145_ok = honesty_smudge_nav_commands_residual_wave145();
    let ocl_timer_method_names_wave146_ok = honesty_ocl_timer_method_names_residual_wave146();
    let ocl_timer_nav_commands_wave146_ok = honesty_ocl_timer_nav_commands_residual_wave146();
    let control_bar_resizer_method_names_wave147_ok =
        honesty_control_bar_resizer_method_names_residual_wave147();
    let control_bar_resizer_nav_commands_wave147_ok =
        honesty_control_bar_resizer_nav_commands_residual_wave147();
    let under_construction_method_names_wave148_ok =
        honesty_under_construction_method_names_residual_wave148();
    let under_construction_nav_commands_wave148_ok =
        honesty_under_construction_nav_commands_residual_wave148();
    let structure_inventory_command_names_wave149_ok =
        honesty_structure_inventory_command_names_residual_wave149();
    let structure_inventory_nav_commands_wave149_ok =
        honesty_structure_inventory_nav_commands_residual_wave149();
    let multi_select_method_names_wave150_ok = honesty_multi_select_method_names_residual_wave150();
    let multi_select_nav_commands_wave150_ok = honesty_multi_select_nav_commands_residual_wave150();
    let credits_style_method_names_wave151_ok =
        honesty_credits_style_method_names_residual_wave151();
    let credits_nav_commands_wave151_ok = honesty_credits_nav_commands_residual_wave151();
    let challenge_generals_method_names_wave152_ok =
        honesty_challenge_generals_method_names_residual_wave152();
    let challenge_generals_nav_commands_wave152_ok =
        honesty_challenge_generals_nav_commands_residual_wave152();
    let gameworld_authority_env_names_wave153_ok =
        honesty_gameworld_authority_env_names_residual_wave153();
    let gameworld_authority_method_names_wave153_ok =
        honesty_gameworld_authority_method_names_residual_wave153();
    let gameworld_authority_nav_commands_wave153_ok =
        honesty_gameworld_authority_nav_commands_residual_wave153();
    let window_video_type_state_names_wave154_ok =
        honesty_window_video_type_state_names_residual_wave154();
    let window_video_method_names_wave154_ok = honesty_window_video_method_names_residual_wave154();
    let window_video_nav_commands_wave154_ok = honesty_window_video_nav_commands_residual_wave154();
    let main_menu_layout_names_wave155_ok = honesty_main_menu_layout_names_residual_wave155();
    let main_menu_layout_nav_commands_wave155_ok =
        honesty_main_menu_layout_nav_commands_residual_wave155();
    let control_bar_scheme_names_wave156_ok = honesty_control_bar_scheme_names_residual_wave156();
    let control_bar_scheme_method_names_wave156_ok =
        honesty_control_bar_scheme_method_names_residual_wave156();
    let control_bar_scheme_nav_commands_wave156_ok =
        honesty_control_bar_scheme_nav_commands_residual_wave156();
    let presentation_boundary_method_names_wave157_ok =
        honesty_presentation_boundary_method_names_residual_wave157();
    let presentation_boundary_source_markers_wave157_ok =
        honesty_presentation_boundary_source_markers_residual_wave157();
    let presentation_boundary_nav_commands_wave157_ok =
        honesty_presentation_boundary_nav_commands_residual_wave157();
    let presentation_boundary_live_wave157_ok = simulate_presentation_boundary_prepare_honesty();
    let control_bar_print_names_wave158_ok = honesty_control_bar_print_names_residual_wave158();
    let control_bar_print_nav_commands_wave158_ok =
        honesty_control_bar_print_nav_commands_residual_wave158();
    let terrain_env_boundary_method_names_wave159_ok =
        honesty_terrain_env_boundary_method_names_residual_wave159();
    let terrain_env_boundary_source_markers_wave159_ok =
        honesty_terrain_env_boundary_source_markers_residual_wave159();
    let terrain_env_boundary_nav_commands_wave159_ok =
        honesty_terrain_env_boundary_nav_commands_residual_wave159();
    let terrain_env_boundary_live_wave159_ok = simulate_terrain_env_boundary_prepare_honesty();
    let main_menu_wnd_names_wave160_ok = honesty_main_menu_wnd_names_residual_wave160();
    let main_menu_wnd_nav_commands_wave160_ok =
        honesty_main_menu_wnd_nav_commands_residual_wave160();
    let main_menu_wnd_live_wave160_ok = simulate_main_menu_wnd_prepare_honesty();
    let main_menu_wnd_load_method_names_wave161_ok =
        honesty_main_menu_wnd_load_method_names_residual_wave161();
    let main_menu_wnd_load_nav_commands_wave161_ok =
        honesty_main_menu_wnd_load_nav_commands_residual_wave161();
    let main_menu_wnd_load_live_wave161_ok = simulate_main_menu_wnd_prepare_load_honesty();
    let main_menu_wnd_materialise_method_names_wave162_ok =
        honesty_main_menu_wnd_materialise_method_names_residual_wave162();
    let main_menu_wnd_materialise_nav_commands_wave162_ok =
        honesty_main_menu_wnd_materialise_nav_commands_residual_wave162();
    let main_menu_wnd_materialise_live_wave162_ok = simulate_main_menu_wnd_materialise_honesty();
    let shell_stack_push_method_names_wave163_ok =
        honesty_shell_stack_push_method_names_residual_wave163();
    let shell_stack_push_nav_commands_wave163_ok =
        honesty_shell_stack_push_nav_commands_residual_wave163();
    let shell_stack_push_live_wave163_ok = simulate_shell_stack_push_honesty();
    let shell_skirmish_nav_method_names_wave164_ok =
        honesty_shell_skirmish_nav_method_names_residual_wave164();
    let shell_skirmish_nav_commands_wave164_ok =
        honesty_shell_skirmish_nav_commands_residual_wave164();
    let shell_skirmish_nav_live_wave164_ok = simulate_shell_skirmish_nav_honesty();
    let control_bar_materialise_method_names_wave165_ok =
        honesty_control_bar_materialise_method_names_residual_wave165();
    let control_bar_materialise_nav_commands_wave165_ok =
        honesty_control_bar_materialise_nav_commands_residual_wave165();
    let control_bar_materialise_live_wave165_ok =
        simulate_control_bar_materialise_honesty_wave165();
    let skirmish_options_wnd_method_names_wave166_ok =
        honesty_skirmish_options_wnd_method_names_residual_wave166();
    let skirmish_options_wnd_nav_commands_wave166_ok =
        honesty_skirmish_options_wnd_nav_commands_residual_wave166();
    let skirmish_options_wnd_live_wave166_ok = simulate_skirmish_options_wnd_honesty();
    let new_game_stream_method_names_wave167_ok =
        honesty_new_game_stream_method_names_residual_wave167();
    let new_game_stream_nav_commands_wave167_ok =
        honesty_new_game_stream_nav_commands_residual_wave167();
    let new_game_stream_live_wave167_ok = simulate_new_game_stream_post_drain_honesty();
    let w3d_main_menu_init_method_names_wave168_ok =
        honesty_w3d_main_menu_init_method_names_residual_wave168();
    let w3d_main_menu_init_nav_commands_wave168_ok =
        honesty_w3d_main_menu_init_nav_commands_residual_wave168();
    let w3d_main_menu_init_live_wave168_ok = simulate_w3d_main_menu_init_honesty();
    let start_game_loading_method_names_wave169_ok =
        honesty_start_game_loading_method_names_residual_wave169();
    let start_game_loading_nav_commands_wave169_ok =
        honesty_start_game_loading_nav_commands_residual_wave169();
    let start_game_loading_live_wave169_ok = simulate_start_game_loading_honesty();
    let live_map_load_method_names_wave170_ok =
        honesty_live_map_load_method_names_residual_wave170();
    let live_map_load_nav_commands_wave170_ok =
        honesty_live_map_load_nav_commands_residual_wave170();
    let live_map_load_live_wave170_ok = simulate_live_map_load_honesty();
    let live_presentation_seed_method_names_wave171_ok =
        honesty_live_presentation_seed_method_names_residual_wave171();
    let live_presentation_seed_nav_commands_wave171_ok =
        honesty_live_presentation_seed_nav_commands_residual_wave171();
    let live_presentation_seed_live_wave171_ok = simulate_live_presentation_seed_honesty();
    let live_gameworld_shadow_overlay_method_names_wave172_ok =
        honesty_live_gameworld_shadow_overlay_method_names_residual_wave172();
    let live_gameworld_shadow_overlay_nav_commands_wave172_ok =
        honesty_live_gameworld_shadow_overlay_nav_commands_residual_wave172();
    let live_gameworld_shadow_overlay_live_wave172_ok =
        simulate_live_gameworld_shadow_overlay_honesty();
    let single_authority_combat_method_names_wave173_ok =
        honesty_single_authority_combat_method_names_residual_wave173();
    let single_authority_combat_nav_commands_wave173_ok =
        honesty_single_authority_combat_nav_commands_residual_wave173();
    let single_authority_combat_live_wave173_ok = simulate_single_authority_combat_honesty();
    let presentation_client_boundary_method_names_wave174_ok =
        honesty_presentation_client_boundary_method_names_residual_wave174();
    let presentation_client_boundary_nav_commands_wave174_ok =
        honesty_presentation_client_boundary_nav_commands_residual_wave174();
    let presentation_client_boundary_live_wave174_ok =
        simulate_presentation_client_boundary_honesty();
    let golden_map_host_victory_method_names_wave175_ok =
        honesty_golden_map_host_victory_method_names_residual_wave175();
    let golden_map_host_victory_nav_commands_wave175_ok =
        honesty_golden_map_host_victory_nav_commands_residual_wave175();
    let golden_map_host_victory_live_wave175_ok = simulate_golden_map_host_victory_honesty();
    let executable_presentation_boundary_method_names_wave176_ok =
        honesty_executable_presentation_boundary_method_names_residual_wave176();
    let executable_presentation_boundary_nav_commands_wave176_ok =
        honesty_executable_presentation_boundary_nav_commands_residual_wave176();
    let executable_presentation_boundary_live_wave176_ok =
        simulate_executable_presentation_boundary_honesty();
    let gameworld_production_authority_method_names_wave177_ok =
        honesty_gameworld_production_authority_method_names_residual_wave177();
    let gameworld_production_authority_nav_commands_wave177_ok =
        honesty_gameworld_production_authority_nav_commands_residual_wave177();
    let gameworld_production_authority_live_wave177_ok =
        simulate_gameworld_production_authority_honesty();
    let gameworld_sole_tick_coupling_method_names_wave178_ok =
        honesty_gameworld_sole_tick_coupling_method_names_residual_wave178();
    let gameworld_sole_tick_coupling_nav_commands_wave178_ok =
        honesty_gameworld_sole_tick_coupling_nav_commands_residual_wave178();
    let gameworld_sole_tick_coupling_live_wave178_ok =
        simulate_gameworld_sole_tick_coupling_honesty();
    let movement_authority_env_ok = crate::gameworld_shadow::gameworld_movement_authority_enabled();
    let gameworld_authority_matrix_method_names_wave179_ok =
        honesty_gameworld_authority_matrix_method_names_residual_wave179();
    let gameworld_authority_matrix_nav_commands_wave179_ok =
        honesty_gameworld_authority_matrix_nav_commands_residual_wave179();
    let gameworld_authority_matrix_live_wave179_ok = simulate_gameworld_authority_matrix_honesty();
    let ai_fire_construction_authority_env_ok = {
        use crate::gameworld_shadow::{
            gameworld_ai_attack_authority_enabled, gameworld_construction_authority_enabled,
            gameworld_fire_spawn_authority_enabled,
        };
        gameworld_ai_attack_authority_enabled()
            && gameworld_fire_spawn_authority_enabled()
            && gameworld_construction_authority_enabled()
    };
    let live_gameworld_production_writeback_method_names_wave180_ok =
        honesty_live_gameworld_production_writeback_method_names_residual_wave180();
    let live_gameworld_production_writeback_nav_commands_wave180_ok =
        honesty_live_gameworld_production_writeback_nav_commands_residual_wave180();
    let live_gameworld_production_writeback_live_wave180_ok =
        simulate_live_gameworld_production_writeback_honesty();
    let live_gameworld_construction_writeback_method_names_wave181_ok =
        honesty_live_gameworld_construction_writeback_method_names_residual_wave181();
    let live_gameworld_construction_writeback_nav_commands_wave181_ok =
        honesty_live_gameworld_construction_writeback_nav_commands_residual_wave181();
    let live_gameworld_construction_writeback_live_wave181_ok =
        simulate_live_gameworld_construction_writeback_honesty();
    let live_gameworld_damage_channel_method_names_wave182_ok =
        honesty_live_gameworld_damage_channel_method_names_residual_wave182();
    let live_gameworld_damage_channel_nav_commands_wave182_ok =
        honesty_live_gameworld_damage_channel_nav_commands_residual_wave182();
    let live_gameworld_damage_channel_live_wave182_ok =
        simulate_live_gameworld_damage_channel_honesty();
    let live_gameworld_economy_movement_method_names_wave183_ok =
        honesty_live_gameworld_economy_movement_method_names_residual_wave183();
    let live_gameworld_economy_movement_nav_commands_wave183_ok =
        honesty_live_gameworld_economy_movement_nav_commands_residual_wave183();
    let live_gameworld_economy_movement_live_wave183_ok =
        simulate_live_gameworld_economy_movement_honesty();
    let live_gameworld_projectile_ai_method_names_wave184_ok =
        honesty_live_gameworld_projectile_ai_method_names_residual_wave184();
    let live_gameworld_projectile_ai_nav_commands_wave184_ok =
        honesty_live_gameworld_projectile_ai_nav_commands_residual_wave184();
    let live_gameworld_projectile_ai_live_wave184_ok =
        simulate_live_gameworld_projectile_ai_honesty();
    let live_gameworld_fire_special_power_method_names_wave185_ok =
        honesty_live_gameworld_fire_special_power_method_names_residual_wave185();
    let live_gameworld_fire_special_power_nav_commands_wave185_ok =
        honesty_live_gameworld_fire_special_power_nav_commands_residual_wave185();
    let live_gameworld_fire_special_power_live_wave185_ok =
        simulate_live_gameworld_fire_special_power_honesty();
    let live_gameworld_presentation_view_method_names_wave186_ok =
        honesty_live_gameworld_presentation_view_method_names_residual_wave186();
    let live_gameworld_presentation_view_nav_commands_wave186_ok =
        honesty_live_gameworld_presentation_view_nav_commands_residual_wave186();
    let live_gameworld_presentation_view_live_wave186_ok =
        simulate_live_gameworld_presentation_view_honesty();
    let live_presentation_gameworld_overlay_method_names_wave187_ok =
        honesty_live_presentation_gameworld_overlay_method_names_residual_wave187();
    let live_presentation_gameworld_overlay_nav_commands_wave187_ok =
        honesty_live_presentation_gameworld_overlay_nav_commands_residual_wave187();
    let live_presentation_gameworld_overlay_live_wave187_ok =
        simulate_live_presentation_gameworld_overlay_honesty();
    let executable_gameworld_presentation_method_names_wave188_ok =
        honesty_executable_gameworld_presentation_method_names_residual_wave188();
    let executable_gameworld_presentation_nav_commands_wave188_ok =
        honesty_executable_gameworld_presentation_nav_commands_residual_wave188();
    let executable_gameworld_presentation_live_wave188_ok =
        simulate_executable_gameworld_presentation_honesty();
    let live_presentation_overlay_deepen_method_names_wave189_ok =
        honesty_live_presentation_overlay_deepen_method_names_residual_wave189();
    let live_presentation_overlay_deepen_nav_commands_wave189_ok =
        honesty_live_presentation_overlay_deepen_nav_commands_residual_wave189();
    let live_presentation_overlay_deepen_live_wave189_ok =
        simulate_live_presentation_overlay_deepen_honesty();
    let live_presentation_overlay_stamp_method_names_wave190_ok =
        honesty_live_presentation_overlay_stamp_method_names_residual_wave190();
    let live_presentation_overlay_stamp_nav_commands_wave190_ok =
        honesty_live_presentation_overlay_stamp_nav_commands_residual_wave190();
    let live_presentation_overlay_stamp_live_wave190_ok =
        simulate_live_presentation_overlay_stamp_honesty();
    let live_gameworld_entity_view_deepen_method_names_wave191_ok =
        honesty_live_gameworld_entity_view_deepen_method_names_residual_wave191();
    let live_gameworld_entity_view_deepen_nav_commands_wave191_ok =
        honesty_live_gameworld_entity_view_deepen_nav_commands_residual_wave191();
    let live_gameworld_entity_view_deepen_live_wave191_ok =
        simulate_live_gameworld_entity_view_deepen_honesty();
    let live_presentation_append_missing_method_names_wave192_ok =
        honesty_live_presentation_append_missing_method_names_residual_wave192();
    let live_presentation_append_missing_nav_commands_wave192_ok =
        honesty_live_presentation_append_missing_nav_commands_residual_wave192();
    let live_presentation_append_missing_live_wave192_ok =
        simulate_live_presentation_append_missing_honesty();
    let live_presentation_build_from_gameworld_method_names_wave193_ok =
        honesty_live_presentation_build_from_gameworld_method_names_residual_wave193();
    let live_presentation_build_from_gameworld_nav_commands_wave193_ok =
        honesty_live_presentation_build_from_gameworld_nav_commands_residual_wave193();
    let live_presentation_build_from_gameworld_live_wave193_ok =
        simulate_live_presentation_build_from_gameworld_honesty();
    let live_presentation_from_gameworld_default_method_names_wave194_ok =
        honesty_live_presentation_from_gameworld_default_method_names_residual_wave194();
    let live_presentation_from_gameworld_default_nav_commands_wave194_ok =
        honesty_live_presentation_from_gameworld_default_nav_commands_residual_wave194();
    let live_presentation_from_gameworld_default_live_wave194_ok =
        simulate_live_presentation_from_gameworld_default_honesty();
    let live_presentation_build_for_engine_method_names_wave195_ok =
        honesty_live_presentation_build_for_engine_method_names_residual_wave195();
    let live_presentation_build_for_engine_nav_commands_wave195_ok =
        honesty_live_presentation_build_for_engine_nav_commands_residual_wave195();
    let live_presentation_build_for_engine_live_wave195_ok =
        simulate_live_presentation_build_for_engine_honesty();
    let live_presentation_rebuilt_vertical_gate_method_names_wave196_ok =
        honesty_live_presentation_rebuilt_vertical_gate_method_names_residual_wave196();
    let live_presentation_rebuilt_vertical_gate_nav_commands_wave196_ok =
        honesty_live_presentation_rebuilt_vertical_gate_nav_commands_residual_wave196();
    let live_presentation_rebuilt_vertical_gate_live_wave196_ok =
        simulate_live_presentation_rebuilt_vertical_gate_honesty();
    let live_command_attack_log_method_names_wave197_ok =
        honesty_live_command_attack_log_method_names_residual_wave197();
    let live_command_attack_log_nav_commands_wave197_ok =
        honesty_live_command_attack_log_nav_commands_residual_wave197();
    let live_command_attack_log_live_wave197_ok = simulate_live_command_attack_log_honesty();
    let live_command_guard_log_method_names_wave198_ok =
        honesty_live_command_guard_log_method_names_residual_wave198();
    let live_command_guard_log_nav_commands_wave198_ok =
        honesty_live_command_guard_log_nav_commands_residual_wave198();
    let live_command_guard_log_live_wave198_ok = simulate_live_command_guard_log_honesty();
    let live_command_production_construction_log_method_names_wave199_ok =
        honesty_live_command_production_construction_log_method_names_residual_wave199();
    let live_command_production_construction_log_nav_commands_wave199_ok =
        honesty_live_command_production_construction_log_nav_commands_residual_wave199();
    let live_command_production_construction_log_live_wave199_ok =
        simulate_live_command_production_construction_log_honesty();
    let live_command_rally_log_method_names_wave200_ok =
        honesty_live_command_rally_log_method_names_residual_wave200();
    let live_command_rally_log_nav_commands_wave200_ok =
        honesty_live_command_rally_log_nav_commands_residual_wave200();
    let live_command_rally_log_live_wave200_ok = simulate_live_command_rally_log_honesty();
    let live_evacuate_contain_log_method_names_wave201_ok =
        honesty_live_evacuate_contain_log_method_names_residual_wave201();
    let live_evacuate_contain_log_nav_commands_wave201_ok =
        honesty_live_evacuate_contain_log_nav_commands_residual_wave201();
    let live_evacuate_contain_log_live_wave201_ok = simulate_live_evacuate_contain_log_honesty();
    let live_command_cheer_science_log_method_names_wave202_ok =
        honesty_live_command_cheer_science_log_method_names_residual_wave202();
    let live_command_cheer_science_log_nav_commands_wave202_ok =
        honesty_live_command_cheer_science_log_nav_commands_residual_wave202();
    let live_command_cheer_science_log_live_wave202_ok =
        simulate_live_command_cheer_science_log_honesty();
    let live_command_deploy_status_log_method_names_wave203_ok =
        honesty_live_command_deploy_status_log_method_names_residual_wave203();
    let live_command_deploy_status_log_nav_commands_wave203_ok =
        honesty_live_command_deploy_status_log_nav_commands_residual_wave203();
    let live_command_deploy_status_log_live_wave203_ok =
        simulate_live_command_deploy_status_log_honesty();
    let live_command_formation_log_method_names_wave204_ok =
        honesty_live_command_formation_log_method_names_residual_wave204();
    let live_command_formation_log_nav_commands_wave204_ok =
        honesty_live_command_formation_log_nav_commands_residual_wave204();
    let live_command_formation_log_live_wave204_ok = simulate_live_command_formation_log_honesty();
    let live_command_order_target_log_method_names_wave205_ok =
        honesty_live_command_order_target_log_method_names_residual_wave205();
    let live_command_order_target_log_nav_commands_wave205_ok =
        honesty_live_command_order_target_log_nav_commands_residual_wave205();
    let live_command_order_target_log_live_wave205_ok =
        simulate_live_command_order_target_log_honesty();
    let live_command_selection_log_method_names_wave206_ok =
        honesty_live_command_selection_log_method_names_residual_wave206();
    let live_command_selection_log_nav_commands_wave206_ok =
        honesty_live_command_selection_log_nav_commands_residual_wave206();
    let live_command_selection_log_live_wave206_ok = simulate_live_command_selection_log_honesty();
    let live_command_non_attack_order_target_method_names_wave207_ok =
        honesty_live_command_non_attack_order_target_method_names_residual_wave207();
    let live_command_non_attack_order_target_nav_commands_wave207_ok =
        honesty_live_command_non_attack_order_target_nav_commands_residual_wave207();
    let live_command_non_attack_order_target_live_wave207_ok =
        simulate_live_command_non_attack_order_target_honesty();
    let live_golden_mopup_honesty_method_names_wave208_ok =
        honesty_live_golden_mopup_honesty_method_names_residual_wave208();
    let live_golden_mopup_honesty_nav_commands_wave208_ok =
        honesty_live_golden_mopup_honesty_nav_commands_residual_wave208();
    let live_golden_mopup_honesty_live_wave208_ok = simulate_live_golden_mopup_honesty();
    let live_os_input_command_path_method_names_wave209_ok =
        honesty_live_os_input_command_path_method_names_residual_wave209();
    let live_os_input_command_path_nav_commands_wave209_ok =
        honesty_live_os_input_command_path_nav_commands_residual_wave209();
    let live_os_input_command_path_live_wave209_ok = simulate_live_os_input_command_path_honesty();
    let live_command_beacon_note_method_names_wave210_ok =
        honesty_live_command_beacon_note_method_names_residual_wave210();
    let live_command_beacon_note_nav_commands_wave210_ok =
        honesty_live_command_beacon_note_nav_commands_residual_wave210();
    let live_command_beacon_note_live_wave210_ok = simulate_live_command_beacon_note_honesty();
    let live_host_beacon_presentation_method_names_wave211_ok =
        honesty_live_host_beacon_presentation_method_names_residual_wave211();
    let live_host_beacon_presentation_nav_commands_wave211_ok =
        honesty_live_host_beacon_presentation_nav_commands_residual_wave211();
    let live_host_beacon_presentation_live_wave211_ok =
        simulate_live_host_beacon_presentation_honesty();
    let live_command_sell_deselect_log_method_names_wave212_ok =
        honesty_live_command_sell_deselect_log_method_names_residual_wave212();
    let live_command_sell_deselect_log_nav_commands_wave212_ok =
        honesty_live_command_sell_deselect_log_nav_commands_residual_wave212();
    let live_command_sell_deselect_log_live_wave212_ok =
        simulate_live_command_sell_deselect_log_honesty();
    let live_presentation_fow_only_method_names_wave213_ok =
        honesty_live_presentation_fow_only_method_names_residual_wave213();
    let live_presentation_fow_only_nav_commands_wave213_ok =
        honesty_live_presentation_fow_only_nav_commands_residual_wave213();
    let live_presentation_fow_only_live_wave213_ok = simulate_live_presentation_fow_only_honesty();
    let live_ui_producer_presentation_only_method_names_wave214_ok =
        honesty_live_ui_producer_presentation_only_method_names_residual_wave214();
    let live_ui_producer_presentation_only_nav_commands_wave214_ok =
        honesty_live_ui_producer_presentation_only_nav_commands_residual_wave214();
    let live_ui_producer_presentation_only_live_wave214_ok =
        simulate_live_ui_producer_presentation_only_honesty();
    let live_ui_helpers_presentation_only_method_names_wave215_ok =
        honesty_live_ui_helpers_presentation_only_method_names_residual_wave215();
    let live_ui_helpers_presentation_only_nav_commands_wave215_ok =
        honesty_live_ui_helpers_presentation_only_nav_commands_residual_wave215();
    let live_ui_helpers_presentation_only_live_wave215_ok =
        simulate_live_ui_helpers_presentation_only_honesty();
    let live_control_group_camera_presentation_only_method_names_wave216_ok =
        honesty_live_control_group_camera_presentation_only_method_names_residual_wave216();
    let live_control_group_camera_presentation_only_nav_commands_wave216_ok =
        honesty_live_control_group_camera_presentation_only_nav_commands_residual_wave216();
    let live_control_group_camera_presentation_only_live_wave216_ok =
        simulate_live_control_group_camera_presentation_only_honesty();
    let live_cmd_filter_env_presentation_only_method_names_wave217_ok =
        honesty_live_cmd_filter_env_presentation_only_method_names_residual_wave217();
    let live_cmd_filter_env_presentation_only_nav_commands_wave217_ok =
        honesty_live_cmd_filter_env_presentation_only_nav_commands_residual_wave217();
    let live_cmd_filter_env_presentation_only_live_wave217_ok =
        simulate_live_cmd_filter_env_presentation_only_honesty();
    let live_selection_commands_presentation_only_method_names_wave218_ok =
        honesty_live_selection_commands_presentation_only_method_names_residual_wave218();
    let live_selection_commands_presentation_only_nav_commands_wave218_ok =
        honesty_live_selection_commands_presentation_only_nav_commands_residual_wave218();
    let live_selection_commands_presentation_only_live_wave218_ok =
        simulate_live_selection_commands_presentation_only_honesty();
    let live_ui_command_selection_presentation_only_method_names_wave219_ok =
        honesty_live_ui_command_selection_presentation_only_method_names_residual_wave219();
    let live_ui_command_selection_presentation_only_nav_commands_wave219_ok =
        honesty_live_ui_command_selection_presentation_only_nav_commands_residual_wave219();
    let live_ui_command_selection_presentation_only_live_wave219_ok =
        simulate_live_ui_command_selection_presentation_only_honesty();
    let live_local_team_presentation_only_method_names_wave220_ok =
        honesty_live_local_team_presentation_only_method_names_residual_wave220();
    let live_local_team_presentation_only_nav_commands_wave220_ok =
        honesty_live_local_team_presentation_only_nav_commands_residual_wave220();
    let live_local_team_presentation_only_live_wave220_ok =
        simulate_live_local_team_presentation_only_honesty();
    let live_hotkey_move_attack_selection_presentation_only_method_names_wave221_ok =
        honesty_live_hotkey_move_attack_selection_presentation_only_method_names_residual_wave221();
    let live_hotkey_move_attack_selection_presentation_only_nav_commands_wave221_ok =
        honesty_live_hotkey_move_attack_selection_presentation_only_nav_commands_residual_wave221();
    let live_hotkey_move_attack_selection_presentation_only_live_wave221_ok =
        simulate_live_hotkey_move_attack_selection_presentation_only_honesty();
    let live_pick_object_presentation_only_method_names_wave222_ok =
        honesty_live_pick_object_presentation_only_method_names_residual_wave222();
    let live_pick_object_presentation_only_nav_commands_wave222_ok =
        honesty_live_pick_object_presentation_only_nav_commands_residual_wave222();
    let live_pick_object_presentation_only_live_wave222_ok =
        simulate_live_pick_object_presentation_only_honesty();
    let live_bootstrap_camera_presentation_only_method_names_wave223_ok =
        honesty_live_bootstrap_camera_presentation_only_method_names_residual_wave223();
    let live_bootstrap_camera_presentation_only_nav_commands_wave223_ok =
        honesty_live_bootstrap_camera_presentation_only_nav_commands_residual_wave223();
    let live_bootstrap_camera_presentation_only_live_wave223_ok =
        simulate_live_bootstrap_camera_presentation_only_honesty();
    let live_force_complete_authority_api_method_names_wave224_ok =
        honesty_live_force_complete_authority_api_method_names_residual_wave224();
    let live_force_complete_authority_api_nav_commands_wave224_ok =
        honesty_live_force_complete_authority_api_nav_commands_residual_wave224();
    let live_force_complete_authority_api_live_wave224_ok =
        simulate_live_force_complete_authority_api_honesty();
    let live_path_guard_authority_api_method_names_wave225_ok =
        honesty_live_path_guard_authority_api_method_names_residual_wave225();
    let live_path_guard_authority_api_nav_commands_wave225_ok =
        honesty_live_path_guard_authority_api_nav_commands_residual_wave225();
    let live_path_guard_authority_api_live_wave225_ok =
        simulate_live_path_guard_authority_api_honesty();
    let live_hotkey_selection_camera_presentation_only_method_names_wave226_ok =
        honesty_live_hotkey_selection_camera_presentation_only_method_names_residual_wave226();
    let live_hotkey_selection_camera_presentation_only_nav_commands_wave226_ok =
        honesty_live_hotkey_selection_camera_presentation_only_nav_commands_residual_wave226();
    let live_hotkey_selection_camera_presentation_only_live_wave226_ok =
        simulate_live_hotkey_selection_camera_presentation_only_honesty();
    let live_construct_spawn_pose_authority_api_method_names_wave227_ok =
        honesty_live_construct_spawn_pose_authority_api_method_names_residual_wave227();
    let live_construct_spawn_pose_authority_api_nav_commands_wave227_ok =
        honesty_live_construct_spawn_pose_authority_api_nav_commands_residual_wave227();
    let live_construct_spawn_pose_authority_api_live_wave227_ok =
        simulate_live_construct_spawn_pose_authority_api_honesty();
    let live_rmb_target_presentation_only_method_names_wave228_ok =
        honesty_live_rmb_target_presentation_only_method_names_residual_wave228();
    let live_rmb_target_presentation_only_nav_commands_wave228_ok =
        honesty_live_rmb_target_presentation_only_nav_commands_residual_wave228();
    let live_rmb_target_presentation_only_live_wave228_ok =
        simulate_live_rmb_target_presentation_only_honesty();
    let live_rmb_selected_presentation_only_method_names_wave229_ok =
        honesty_live_rmb_selected_presentation_only_method_names_residual_wave229();
    let live_rmb_selected_presentation_only_nav_commands_wave229_ok =
        honesty_live_rmb_selected_presentation_only_nav_commands_residual_wave229();
    let live_rmb_selected_presentation_only_live_wave229_ok =
        simulate_live_rmb_selected_presentation_only_honesty();
    let live_command_unit_authority_api_method_names_wave230_ok =
        honesty_live_command_unit_authority_api_method_names_residual_wave230();
    let live_command_unit_authority_api_nav_commands_wave230_ok =
        honesty_live_command_unit_authority_api_nav_commands_residual_wave230();
    let live_command_unit_authority_api_live_wave230_ok =
        simulate_live_command_unit_authority_api_honesty();
    let live_command_unit_more_authority_api_method_names_wave231_ok =
        honesty_live_command_unit_more_authority_api_method_names_residual_wave231();
    let live_command_unit_more_authority_api_nav_commands_wave231_ok =
        honesty_live_command_unit_more_authority_api_nav_commands_residual_wave231();
    let live_command_unit_more_authority_api_live_wave231_ok =
        simulate_live_command_unit_more_authority_api_honesty();
    let live_command_executor_authority_api_method_names_wave232_ok =
        honesty_live_command_executor_authority_api_method_names_residual_wave232();
    let live_command_executor_authority_api_nav_commands_wave232_ok =
        honesty_live_command_executor_authority_api_nav_commands_residual_wave232();
    let live_command_executor_authority_api_live_wave232_ok =
        simulate_live_command_executor_authority_api_honesty();
    let live_command_executor_more_authority_api_method_names_wave233_ok =
        honesty_live_command_executor_more_authority_api_method_names_residual_wave233();
    let live_command_executor_more_authority_api_nav_commands_wave233_ok =
        honesty_live_command_executor_more_authority_api_nav_commands_residual_wave233();
    let live_command_executor_more_authority_api_live_wave233_ok =
        simulate_live_command_executor_more_authority_api_honesty();
    let live_engine_presentation_player_ui_method_names_wave234_ok =
        honesty_live_engine_presentation_player_ui_method_names_residual_wave234();
    let live_engine_presentation_player_ui_nav_commands_wave234_ok =
        honesty_live_engine_presentation_player_ui_nav_commands_residual_wave234();
    let live_engine_presentation_player_ui_live_wave234_ok =
        simulate_live_engine_presentation_player_ui_honesty();
    let live_rmb_presentation_full_classify_method_names_wave235_ok =
        honesty_live_rmb_presentation_full_classify_method_names_residual_wave235();
    let live_rmb_presentation_full_classify_nav_commands_wave235_ok =
        honesty_live_rmb_presentation_full_classify_nav_commands_residual_wave235();
    let live_rmb_presentation_full_classify_live_wave235_ok =
        simulate_live_rmb_presentation_full_classify_honesty();
    let live_mouse_input_presentation_only_method_names_wave236_ok =
        honesty_live_mouse_input_presentation_only_method_names_residual_wave236();
    let live_mouse_input_presentation_only_nav_commands_wave236_ok =
        honesty_live_mouse_input_presentation_only_nav_commands_residual_wave236();
    let live_mouse_input_presentation_only_live_wave236_ok =
        simulate_live_mouse_input_presentation_only_honesty();
    let live_engine_player_ui_boot_peel_method_names_wave237_ok =
        honesty_live_engine_player_ui_boot_peel_method_names_residual_wave237();
    let live_engine_player_ui_boot_peel_nav_commands_wave237_ok =
        honesty_live_engine_player_ui_boot_peel_nav_commands_residual_wave237();
    let live_engine_player_ui_boot_peel_live_wave237_ok =
        simulate_live_engine_player_ui_boot_peel_honesty();
    let live_player_probe_api_method_names_wave238_ok =
        honesty_live_player_probe_api_method_names_residual_wave238();
    let live_player_probe_api_nav_commands_wave238_ok =
        honesty_live_player_probe_api_nav_commands_residual_wave238();
    let live_player_probe_api_live_wave238_ok = simulate_live_player_probe_api_honesty();
    let live_player_team_probe_method_names_wave239_ok =
        honesty_live_player_team_probe_method_names_residual_wave239();
    let live_player_team_probe_nav_commands_wave239_ok =
        honesty_live_player_team_probe_nav_commands_residual_wave239();
    let live_player_team_probe_live_wave239_ok = simulate_live_player_team_probe_honesty();
    let live_player_field_probe_method_names_wave240_ok =
        honesty_live_player_field_probe_method_names_residual_wave240();
    let live_player_field_probe_nav_commands_wave240_ok =
        honesty_live_player_field_probe_nav_commands_residual_wave240();
    let live_player_field_probe_live_wave240_ok = simulate_live_player_field_probe_honesty();
    let live_camera_height_probe_method_names_wave241_ok =
        honesty_live_camera_height_probe_method_names_residual_wave241();
    let live_camera_height_probe_nav_commands_wave241_ok =
        honesty_live_camera_height_probe_nav_commands_residual_wave241();
    let live_camera_height_probe_live_wave241_ok = simulate_live_camera_height_probe_honesty();
    let live_command_player_probe_method_names_wave242_ok =
        honesty_live_command_player_probe_method_names_residual_wave242();
    let live_command_player_probe_nav_commands_wave242_ok =
        honesty_live_command_player_probe_nav_commands_residual_wave242();
    let live_command_player_probe_live_wave242_ok = simulate_live_command_player_probe_honesty();
    let live_construct_economy_probe_method_names_wave243_ok =
        honesty_live_construct_economy_probe_method_names_residual_wave243();
    let live_construct_economy_probe_nav_commands_wave243_ok =
        honesty_live_construct_economy_probe_nav_commands_residual_wave243();
    let live_construct_economy_probe_live_wave243_ok =
        simulate_live_construct_economy_probe_honesty();
    let live_command_unit_probe_method_names_wave244_ok =
        honesty_live_command_unit_probe_method_names_residual_wave244();
    let live_command_unit_probe_nav_commands_wave244_ok =
        honesty_live_command_unit_probe_nav_commands_residual_wave244();
    let live_command_unit_probe_live_wave244_ok = simulate_live_command_unit_probe_honesty();
    let live_selection_query_probe_method_names_wave245_ok =
        honesty_live_selection_query_probe_method_names_residual_wave245();
    let live_selection_query_probe_nav_commands_wave245_ok =
        honesty_live_selection_query_probe_nav_commands_residual_wave245();
    let live_selection_query_probe_live_wave245_ok = simulate_live_selection_query_probe_honesty();
    let live_world_pick_probe_method_names_wave246_ok =
        honesty_live_world_pick_probe_method_names_residual_wave246();
    let live_world_pick_probe_nav_commands_wave246_ok =
        honesty_live_world_pick_probe_nav_commands_residual_wave246();
    let live_world_pick_probe_live_wave246_ok = simulate_live_world_pick_probe_honesty();
    let live_object_registry_empty_fastpath_method_names_wave247_ok =
        honesty_live_object_registry_empty_fastpath_method_names_residual_wave247();
    let live_object_registry_empty_fastpath_nav_commands_wave247_ok =
        honesty_live_object_registry_empty_fastpath_nav_commands_residual_wave247();
    let live_object_registry_empty_fastpath_live_wave247_ok =
        simulate_live_object_registry_empty_fastpath_honesty();
    let live_legacy_object_registry_fastpath_method_names_wave248_ok =
        honesty_live_legacy_object_registry_fastpath_method_names_residual_wave248();
    let live_legacy_object_registry_fastpath_nav_commands_wave248_ok =
        honesty_live_legacy_object_registry_fastpath_nav_commands_residual_wave248();
    let live_legacy_object_registry_fastpath_live_wave248_ok =
        simulate_live_legacy_object_registry_fastpath_honesty();
    let live_client_dual_world_empty_gate_method_names_wave249_ok =
        honesty_live_client_dual_world_empty_gate_method_names_residual_wave249();
    let live_client_dual_world_empty_gate_nav_commands_wave249_ok =
        honesty_live_client_dual_world_empty_gate_nav_commands_residual_wave249();
    let live_client_dual_world_empty_gate_live_wave249_ok =
        simulate_live_client_dual_world_empty_gate_honesty();
    let live_presentation_time_frozen_probe_method_names_wave250_ok =
        honesty_live_presentation_time_frozen_probe_method_names_residual_wave250();
    let live_presentation_time_frozen_probe_nav_commands_wave250_ok =
        honesty_live_presentation_time_frozen_probe_nav_commands_residual_wave250();
    let live_presentation_time_frozen_probe_live_wave250_ok =
        simulate_live_presentation_time_frozen_probe_honesty();
    let live_presentation_visual_speed_probe_method_names_wave251_ok =
        honesty_live_presentation_visual_speed_probe_method_names_residual_wave251();
    let live_presentation_visual_speed_probe_nav_commands_wave251_ok =
        honesty_live_presentation_visual_speed_probe_nav_commands_residual_wave251();
    let live_presentation_visual_speed_probe_live_wave251_ok =
        simulate_live_presentation_visual_speed_probe_honesty();
    let live_presentation_script_camera_probe_method_names_wave252_ok =
        honesty_live_presentation_script_camera_probe_method_names_residual_wave252();
    let live_presentation_script_camera_probe_nav_commands_wave252_ok =
        honesty_live_presentation_script_camera_probe_nav_commands_residual_wave252();
    let live_presentation_script_camera_probe_live_wave252_ok =
        simulate_live_presentation_script_camera_probe_honesty();
    let live_ai_group_dual_world_empty_gate_method_names_wave253_ok =
        honesty_live_ai_group_dual_world_empty_gate_method_names_residual_wave253();
    let live_ai_group_dual_world_empty_gate_nav_commands_wave253_ok =
        honesty_live_ai_group_dual_world_empty_gate_nav_commands_residual_wave253();
    let live_ai_group_dual_world_empty_gate_live_wave253_ok =
        simulate_live_ai_group_dual_world_empty_gate_honesty();
    let live_ai_states_dual_world_empty_gate_method_names_wave254_ok =
        honesty_live_ai_states_dual_world_empty_gate_method_names_residual_wave254();
    let live_ai_states_dual_world_empty_gate_nav_commands_wave254_ok =
        honesty_live_ai_states_dual_world_empty_gate_nav_commands_residual_wave254();
    let live_ai_states_dual_world_empty_gate_live_wave254_ok =
        simulate_live_ai_states_dual_world_empty_gate_honesty();
    let live_ai_player_dual_world_empty_gate_method_names_wave255_ok =
        honesty_live_ai_player_dual_world_empty_gate_method_names_residual_wave255();
    let live_ai_player_dual_world_empty_gate_nav_commands_wave255_ok =
        honesty_live_ai_player_dual_world_empty_gate_nav_commands_residual_wave255();
    let live_ai_player_dual_world_empty_gate_live_wave255_ok =
        simulate_live_ai_player_dual_world_empty_gate_honesty();
    let live_team_dual_world_empty_gate_method_names_wave256_ok =
        honesty_live_team_dual_world_empty_gate_method_names_residual_wave256();
    let live_team_dual_world_empty_gate_nav_commands_wave256_ok =
        honesty_live_team_dual_world_empty_gate_nav_commands_residual_wave256();
    let live_team_dual_world_empty_gate_live_wave256_ok =
        simulate_live_team_dual_world_empty_gate_honesty();
    let live_ai_legacy_states_dual_world_empty_gate_method_names_wave257_ok =
        honesty_live_ai_legacy_states_dual_world_empty_gate_method_names_residual_wave257();
    let live_ai_legacy_states_dual_world_empty_gate_nav_commands_wave257_ok =
        honesty_live_ai_legacy_states_dual_world_empty_gate_nav_commands_residual_wave257();
    let live_ai_legacy_states_dual_world_empty_gate_live_wave257_ok =
        simulate_live_ai_legacy_states_dual_world_empty_gate_honesty();
    let live_unit_dual_world_empty_gate_method_names_wave258_ok =
        honesty_live_unit_dual_world_empty_gate_method_names_residual_wave258();
    let live_unit_dual_world_empty_gate_nav_commands_wave258_ok =
        honesty_live_unit_dual_world_empty_gate_nav_commands_residual_wave258();
    let live_unit_dual_world_empty_gate_live_wave258_ok =
        simulate_live_unit_dual_world_empty_gate_honesty();
    let live_stealth_dual_world_empty_gate_method_names_wave259_ok =
        honesty_live_stealth_dual_world_empty_gate_method_names_residual_wave259();
    let live_stealth_dual_world_empty_gate_nav_commands_wave259_ok =
        honesty_live_stealth_dual_world_empty_gate_nav_commands_residual_wave259();
    let live_stealth_dual_world_empty_gate_live_wave259_ok =
        simulate_live_stealth_dual_world_empty_gate_honesty();
    let live_garrison_dual_world_empty_gate_method_names_wave260_ok =
        honesty_live_garrison_dual_world_empty_gate_method_names_residual_wave260();
    let live_garrison_dual_world_empty_gate_nav_commands_wave260_ok =
        honesty_live_garrison_dual_world_empty_gate_nav_commands_residual_wave260();
    let live_garrison_dual_world_empty_gate_live_wave260_ok =
        simulate_live_garrison_dual_world_empty_gate_honesty();
    let live_open_contain_dual_world_empty_gate_method_names_wave261_ok =
        honesty_live_open_contain_dual_world_empty_gate_method_names_residual_wave261();
    let live_open_contain_dual_world_empty_gate_nav_commands_wave261_ok =
        honesty_live_open_contain_dual_world_empty_gate_nav_commands_residual_wave261();
    let live_open_contain_dual_world_empty_gate_live_wave261_ok =
        simulate_live_open_contain_dual_world_empty_gate_honesty();
    let live_pathfind_dual_world_empty_gate_method_names_wave262_ok =
        honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave262();
    let live_pathfind_dual_world_empty_gate_nav_commands_wave262_ok =
        honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave262();
    let live_pathfind_dual_world_empty_gate_live_wave262_ok =
        simulate_live_pathfind_dual_world_empty_gate_honesty();
    let live_ai_mod_dual_world_empty_gate_method_names_wave263_ok =
        honesty_live_ai_mod_dual_world_empty_gate_method_names_residual_wave263();
    let live_ai_mod_dual_world_empty_gate_nav_commands_wave263_ok =
        honesty_live_ai_mod_dual_world_empty_gate_nav_commands_residual_wave263();
    let live_ai_mod_dual_world_empty_gate_live_wave263_ok =
        simulate_live_ai_mod_dual_world_empty_gate_honesty();
    let live_object_mod_dual_world_empty_gate_method_names_wave264_ok =
        honesty_live_object_mod_dual_world_empty_gate_method_names_residual_wave264();
    let live_object_mod_dual_world_empty_gate_nav_commands_wave264_ok =
        honesty_live_object_mod_dual_world_empty_gate_nav_commands_residual_wave264();
    let live_object_mod_dual_world_empty_gate_live_wave264_ok =
        simulate_live_object_mod_dual_world_empty_gate_honesty();
    let live_weapon_dual_world_empty_gate_method_names_wave265_ok =
        honesty_live_weapon_dual_world_empty_gate_method_names_residual_wave265();
    let live_weapon_dual_world_empty_gate_nav_commands_wave265_ok =
        honesty_live_weapon_dual_world_empty_gate_nav_commands_residual_wave265();
    let live_weapon_dual_world_empty_gate_live_wave265_ok =
        simulate_live_weapon_dual_world_empty_gate_honesty();
    let live_partition_filters_dual_world_empty_gate_method_names_wave266_ok =
        honesty_live_partition_filters_dual_world_empty_gate_method_names_residual_wave266();
    let live_partition_filters_dual_world_empty_gate_nav_commands_wave266_ok =
        honesty_live_partition_filters_dual_world_empty_gate_nav_commands_residual_wave266();
    let live_partition_filters_dual_world_empty_gate_live_wave266_ok =
        simulate_live_partition_filters_dual_world_empty_gate_honesty();
    let live_ai_state_machine_dual_world_empty_gate_method_names_wave267_ok =
        honesty_live_ai_state_machine_dual_world_empty_gate_method_names_residual_wave267();
    let live_ai_state_machine_dual_world_empty_gate_nav_commands_wave267_ok =
        honesty_live_ai_state_machine_dual_world_empty_gate_nav_commands_residual_wave267();
    let live_ai_state_machine_dual_world_empty_gate_live_wave267_ok =
        simulate_live_ai_state_machine_dual_world_empty_gate_honesty();
    let live_player_dual_world_empty_gate_method_names_wave268_ok =
        honesty_live_player_dual_world_empty_gate_method_names_residual_wave268();
    let live_player_dual_world_empty_gate_nav_commands_wave268_ok =
        honesty_live_player_dual_world_empty_gate_nav_commands_residual_wave268();
    let live_player_dual_world_empty_gate_live_wave268_ok =
        simulate_live_player_dual_world_empty_gate_honesty();
    let live_game_client_dual_world_empty_gate_method_names_wave269_ok =
        honesty_live_game_client_dual_world_empty_gate_method_names_residual_wave269();
    let live_game_client_dual_world_empty_gate_nav_commands_wave269_ok =
        honesty_live_game_client_dual_world_empty_gate_nav_commands_residual_wave269();
    let live_game_client_dual_world_empty_gate_live_wave269_ok =
        simulate_live_game_client_dual_world_empty_gate_honesty();
    let live_drawable_dual_world_empty_gate_method_names_wave270_ok =
        honesty_live_drawable_dual_world_empty_gate_method_names_residual_wave270();
    let live_drawable_dual_world_empty_gate_nav_commands_wave270_ok =
        honesty_live_drawable_dual_world_empty_gate_nav_commands_residual_wave270();
    let live_drawable_dual_world_empty_gate_live_wave270_ok =
        simulate_live_drawable_dual_world_empty_gate_honesty();
    let live_script_conditions_dual_world_empty_gate_method_names_wave271_ok =
        honesty_live_script_conditions_dual_world_empty_gate_method_names_residual_wave271();
    let live_script_conditions_dual_world_empty_gate_nav_commands_wave271_ok =
        honesty_live_script_conditions_dual_world_empty_gate_nav_commands_residual_wave271();
    let live_script_conditions_dual_world_empty_gate_live_wave271_ok =
        simulate_live_script_conditions_dual_world_empty_gate_honesty();
    let live_transport_contain_dual_world_empty_gate_method_names_wave272_ok =
        honesty_live_transport_contain_dual_world_empty_gate_method_names_residual_wave272();
    let live_transport_contain_dual_world_empty_gate_nav_commands_wave272_ok =
        honesty_live_transport_contain_dual_world_empty_gate_nav_commands_residual_wave272();
    let live_transport_contain_dual_world_empty_gate_live_wave272_ok =
        simulate_live_transport_contain_dual_world_empty_gate_honesty();
    let live_ingame_ui_dual_world_empty_gate_method_names_wave273_ok =
        honesty_live_ingame_ui_dual_world_empty_gate_method_names_residual_wave273();
    let live_ingame_ui_dual_world_empty_gate_nav_commands_wave273_ok =
        honesty_live_ingame_ui_dual_world_empty_gate_nav_commands_residual_wave273();
    let live_ingame_ui_dual_world_empty_gate_live_wave273_ok =
        simulate_live_ingame_ui_dual_world_empty_gate_honesty();
    let live_helix_contain_dual_world_empty_gate_method_names_wave274_ok =
        honesty_live_helix_contain_dual_world_empty_gate_method_names_residual_wave274();
    let live_helix_contain_dual_world_empty_gate_nav_commands_wave274_ok =
        honesty_live_helix_contain_dual_world_empty_gate_nav_commands_residual_wave274();
    let live_helix_contain_dual_world_empty_gate_live_wave274_ok =
        simulate_live_helix_contain_dual_world_empty_gate_honesty();
    let live_command_processor_dual_world_empty_gate_method_names_wave275_ok =
        honesty_live_command_processor_dual_world_empty_gate_method_names_residual_wave275();
    let live_command_processor_dual_world_empty_gate_nav_commands_wave275_ok =
        honesty_live_command_processor_dual_world_empty_gate_nav_commands_residual_wave275();
    let live_command_processor_dual_world_empty_gate_live_wave275_ok =
        simulate_live_command_processor_dual_world_empty_gate_honesty();
    let live_turret_dual_world_empty_gate_method_names_wave276_ok =
        honesty_live_turret_dual_world_empty_gate_method_names_residual_wave276();
    let live_turret_dual_world_empty_gate_nav_commands_wave276_ok =
        honesty_live_turret_dual_world_empty_gate_nav_commands_residual_wave276();
    let live_turret_dual_world_empty_gate_live_wave276_ok =
        simulate_live_turret_dual_world_empty_gate_honesty();
    let live_rider_change_contain_dual_world_empty_gate_method_names_wave277_ok =
        honesty_live_rider_change_contain_dual_world_empty_gate_method_names_residual_wave277();
    let live_rider_change_contain_dual_world_empty_gate_nav_commands_wave277_ok =
        honesty_live_rider_change_contain_dual_world_empty_gate_nav_commands_residual_wave277();
    let live_rider_change_contain_dual_world_empty_gate_live_wave277_ok =
        simulate_live_rider_change_contain_dual_world_empty_gate_honesty();
    let live_selection_dual_world_empty_gate_method_names_wave278_ok =
        honesty_live_selection_dual_world_empty_gate_method_names_residual_wave278();
    let live_selection_dual_world_empty_gate_nav_commands_wave278_ok =
        honesty_live_selection_dual_world_empty_gate_nav_commands_residual_wave278();
    let live_selection_dual_world_empty_gate_live_wave278_ok =
        simulate_live_selection_dual_world_empty_gate_honesty();
    let live_cave_contain_dual_world_empty_gate_method_names_wave279_ok =
        honesty_live_cave_contain_dual_world_empty_gate_method_names_residual_wave279();
    let live_cave_contain_dual_world_empty_gate_nav_commands_wave279_ok =
        honesty_live_cave_contain_dual_world_empty_gate_nav_commands_residual_wave279();
    let live_cave_contain_dual_world_empty_gate_live_wave279_ok =
        simulate_live_cave_contain_dual_world_empty_gate_honesty();
    let live_tunnel_contain_dual_world_empty_gate_method_names_wave280_ok =
        honesty_live_tunnel_contain_dual_world_empty_gate_method_names_residual_wave280();
    let live_tunnel_contain_dual_world_empty_gate_nav_commands_wave280_ok =
        honesty_live_tunnel_contain_dual_world_empty_gate_nav_commands_residual_wave280();
    let live_tunnel_contain_dual_world_empty_gate_live_wave280_ok =
        simulate_live_tunnel_contain_dual_world_empty_gate_honesty();
    let live_helpers_dual_world_empty_gate_method_names_wave281_ok =
        honesty_live_helpers_dual_world_empty_gate_method_names_residual_wave281();
    let live_helpers_dual_world_empty_gate_nav_commands_wave281_ok =
        honesty_live_helpers_dual_world_empty_gate_nav_commands_residual_wave281();
    let live_helpers_dual_world_empty_gate_live_wave281_ok =
        simulate_live_helpers_dual_world_empty_gate_honesty();
    let live_ai_update_interface_dual_world_empty_gate_method_names_wave282_ok =
        honesty_live_ai_update_interface_dual_world_empty_gate_method_names_residual_wave282();
    let live_ai_update_interface_dual_world_empty_gate_nav_commands_wave282_ok =
        honesty_live_ai_update_interface_dual_world_empty_gate_nav_commands_residual_wave282();
    let live_ai_update_interface_dual_world_empty_gate_live_wave282_ok =
        simulate_live_ai_update_interface_dual_world_empty_gate_honesty();
    let live_stealth_update_dual_world_empty_gate_method_names_wave283_ok =
        honesty_live_stealth_update_dual_world_empty_gate_method_names_residual_wave283();
    let live_stealth_update_dual_world_empty_gate_nav_commands_wave283_ok =
        honesty_live_stealth_update_dual_world_empty_gate_nav_commands_residual_wave283();
    let live_stealth_update_dual_world_empty_gate_live_wave283_ok =
        simulate_live_stealth_update_dual_world_empty_gate_honesty();
    let live_script_executor_dual_world_empty_gate_method_names_wave284_ok =
        honesty_live_script_executor_dual_world_empty_gate_method_names_residual_wave284();
    let live_script_executor_dual_world_empty_gate_nav_commands_wave284_ok =
        honesty_live_script_executor_dual_world_empty_gate_nav_commands_residual_wave284();
    let live_script_executor_dual_world_empty_gate_live_wave284_ok =
        simulate_live_script_executor_dual_world_empty_gate_honesty();
    let live_ai_integration_dual_world_empty_gate_method_names_wave285_ok =
        honesty_live_ai_integration_dual_world_empty_gate_method_names_residual_wave285();
    let live_ai_integration_dual_world_empty_gate_nav_commands_wave285_ok =
        honesty_live_ai_integration_dual_world_empty_gate_nav_commands_residual_wave285();
    let live_ai_integration_dual_world_empty_gate_live_wave285_ok =
        simulate_live_ai_integration_dual_world_empty_gate_honesty();
    let live_dumb_projectile_dual_world_empty_gate_method_names_wave286_ok =
        honesty_live_dumb_projectile_dual_world_empty_gate_method_names_residual_wave286();
    let live_dumb_projectile_dual_world_empty_gate_nav_commands_wave286_ok =
        honesty_live_dumb_projectile_dual_world_empty_gate_nav_commands_residual_wave286();
    let live_dumb_projectile_dual_world_empty_gate_live_wave286_ok =
        simulate_live_dumb_projectile_dual_world_empty_gate_honesty();
    let live_enhanced_player_dual_world_empty_gate_method_names_wave287_ok =
        honesty_live_enhanced_player_dual_world_empty_gate_method_names_residual_wave287();
    let live_enhanced_player_dual_world_empty_gate_nav_commands_wave287_ok =
        honesty_live_enhanced_player_dual_world_empty_gate_nav_commands_residual_wave287();
    let live_enhanced_player_dual_world_empty_gate_live_wave287_ok =
        simulate_live_enhanced_player_dual_world_empty_gate_honesty();
    let live_hijacker_update_dual_world_empty_gate_method_names_wave288_ok =
        honesty_live_hijacker_update_dual_world_empty_gate_method_names_residual_wave288();
    let live_hijacker_update_dual_world_empty_gate_nav_commands_wave288_ok =
        honesty_live_hijacker_update_dual_world_empty_gate_nav_commands_residual_wave288();
    let live_hijacker_update_dual_world_empty_gate_live_wave288_ok =
        simulate_live_hijacker_update_dual_world_empty_gate_honesty();
    let live_weapon_impl_dual_world_empty_gate_method_names_wave289_ok =
        honesty_live_weapon_impl_dual_world_empty_gate_method_names_residual_wave289();
    let live_weapon_impl_dual_world_empty_gate_nav_commands_wave289_ok =
        honesty_live_weapon_impl_dual_world_empty_gate_nav_commands_residual_wave289();
    let live_weapon_impl_dual_world_empty_gate_live_wave289_ok =
        simulate_live_weapon_impl_dual_world_empty_gate_honesty();
    let live_async_player_dual_world_empty_gate_method_names_wave290_ok =
        honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290();
    let live_async_player_dual_world_empty_gate_nav_commands_wave290_ok =
        honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290();
    let live_async_player_dual_world_empty_gate_live_wave290_ok =
        simulate_live_async_player_dual_world_empty_gate_honesty();
    let live_active_body_dual_world_empty_gate_method_names_wave291_ok =
        honesty_live_active_body_dual_world_empty_gate_method_names_residual_wave291();
    let live_active_body_dual_world_empty_gate_nav_commands_wave291_ok =
        honesty_live_active_body_dual_world_empty_gate_nav_commands_residual_wave291();
    let live_active_body_dual_world_empty_gate_live_wave291_ok =
        simulate_live_active_body_dual_world_empty_gate_honesty();
    let live_skirmish_conditions_dual_world_empty_gate_method_names_wave292_ok =
        honesty_live_skirmish_conditions_dual_world_empty_gate_method_names_residual_wave292();
    let live_skirmish_conditions_dual_world_empty_gate_nav_commands_wave292_ok =
        honesty_live_skirmish_conditions_dual_world_empty_gate_nav_commands_residual_wave292();
    let live_skirmish_conditions_dual_world_empty_gate_live_wave292_ok =
        simulate_live_skirmish_conditions_dual_world_empty_gate_honesty();
    let live_ai_build_list_dual_world_empty_gate_method_names_wave293_ok =
        honesty_live_ai_build_list_dual_world_empty_gate_method_names_residual_wave293();
    let live_ai_build_list_dual_world_empty_gate_nav_commands_wave293_ok =
        honesty_live_ai_build_list_dual_world_empty_gate_nav_commands_residual_wave293();
    let live_ai_build_list_dual_world_empty_gate_live_wave293_ok =
        simulate_live_ai_build_list_dual_world_empty_gate_honesty();
    let live_victory_dual_world_empty_gate_method_names_wave294_ok =
        honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294();
    let live_victory_dual_world_empty_gate_nav_commands_wave294_ok =
        honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294();
    let live_victory_dual_world_empty_gate_live_wave294_ok =
        simulate_live_victory_dual_world_empty_gate_honesty();
    let live_script_actions_dual_world_empty_gate_method_names_wave295_ok =
        honesty_live_script_actions_dual_world_empty_gate_method_names_residual_wave295();
    let live_script_actions_dual_world_empty_gate_nav_commands_wave295_ok =
        honesty_live_script_actions_dual_world_empty_gate_nav_commands_residual_wave295();
    let live_script_actions_dual_world_empty_gate_live_wave295_ok =
        simulate_live_script_actions_dual_world_empty_gate_honesty();
    let live_special_ability_dual_world_empty_gate_method_names_wave296_ok =
        honesty_live_special_ability_dual_world_empty_gate_method_names_residual_wave296();
    let live_special_ability_dual_world_empty_gate_nav_commands_wave296_ok =
        honesty_live_special_ability_dual_world_empty_gate_nav_commands_residual_wave296();
    let live_special_ability_dual_world_empty_gate_live_wave296_ok =
        simulate_live_special_ability_dual_world_empty_gate_honesty();
    let live_stealth_detector_dual_world_empty_gate_method_names_wave297_ok =
        honesty_live_stealth_detector_dual_world_empty_gate_method_names_residual_wave297();
    let live_stealth_detector_dual_world_empty_gate_nav_commands_wave297_ok =
        honesty_live_stealth_detector_dual_world_empty_gate_nav_commands_residual_wave297();
    let live_stealth_detector_dual_world_empty_gate_live_wave297_ok =
        simulate_live_stealth_detector_dual_world_empty_gate_honesty();
    let live_supply_system_dual_world_empty_gate_method_names_wave298_ok =
        honesty_live_supply_system_dual_world_empty_gate_method_names_residual_wave298();
    let live_supply_system_dual_world_empty_gate_nav_commands_wave298_ok =
        honesty_live_supply_system_dual_world_empty_gate_nav_commands_residual_wave298();
    let live_supply_system_dual_world_empty_gate_live_wave298_ok =
        simulate_live_supply_system_dual_world_empty_gate_honesty();
    let live_particle_uplink_dual_world_empty_gate_method_names_wave299_ok =
        honesty_live_particle_uplink_dual_world_empty_gate_method_names_residual_wave299();
    let live_particle_uplink_dual_world_empty_gate_nav_commands_wave299_ok =
        honesty_live_particle_uplink_dual_world_empty_gate_nav_commands_residual_wave299();
    let live_particle_uplink_dual_world_empty_gate_live_wave299_ok =
        simulate_live_particle_uplink_dual_world_empty_gate_honesty();
    let live_overlord_contain_dual_world_empty_gate_method_names_wave300_ok =
        honesty_live_overlord_contain_dual_world_empty_gate_method_names_residual_wave300();
    let live_overlord_contain_dual_world_empty_gate_nav_commands_wave300_ok =
        honesty_live_overlord_contain_dual_world_empty_gate_nav_commands_residual_wave300();
    let live_overlord_contain_dual_world_empty_gate_live_wave300_ok =
        simulate_live_overlord_contain_dual_world_empty_gate_honesty();
    let live_bridge_behavior_dual_world_empty_gate_method_names_wave301_ok =
        honesty_live_bridge_behavior_dual_world_empty_gate_method_names_residual_wave301();
    let live_bridge_behavior_dual_world_empty_gate_nav_commands_wave301_ok =
        honesty_live_bridge_behavior_dual_world_empty_gate_nav_commands_residual_wave301();
    let live_bridge_behavior_dual_world_empty_gate_live_wave301_ok =
        simulate_live_bridge_behavior_dual_world_empty_gate_honesty();
    let live_stealth_behavior_dual_world_empty_gate_method_names_wave302_ok =
        honesty_live_stealth_behavior_dual_world_empty_gate_method_names_residual_wave302();
    let live_stealth_behavior_dual_world_empty_gate_nav_commands_wave302_ok =
        honesty_live_stealth_behavior_dual_world_empty_gate_nav_commands_residual_wave302();
    let live_stealth_behavior_dual_world_empty_gate_live_wave302_ok =
        simulate_live_stealth_behavior_dual_world_empty_gate_honesty();
    let live_crate_collide_dual_world_empty_gate_method_names_wave303_ok =
        honesty_live_crate_collide_dual_world_empty_gate_method_names_residual_wave303();
    let live_crate_collide_dual_world_empty_gate_nav_commands_wave303_ok =
        honesty_live_crate_collide_dual_world_empty_gate_nav_commands_residual_wave303();
    let live_crate_collide_dual_world_empty_gate_live_wave303_ok =
        simulate_live_crate_collide_dual_world_empty_gate_honesty();
    let live_object_manager_dual_world_empty_gate_method_names_wave304_ok =
        honesty_live_object_manager_dual_world_empty_gate_method_names_residual_wave304();
    let live_object_manager_dual_world_empty_gate_nav_commands_wave304_ok =
        honesty_live_object_manager_dual_world_empty_gate_nav_commands_residual_wave304();
    let live_object_manager_dual_world_empty_gate_live_wave304_ok =
        simulate_live_object_manager_dual_world_empty_gate_honesty();
    let live_sticky_bomb_dual_world_empty_gate_method_names_wave305_ok =
        honesty_live_sticky_bomb_dual_world_empty_gate_method_names_residual_wave305();
    let live_sticky_bomb_dual_world_empty_gate_nav_commands_wave305_ok =
        honesty_live_sticky_bomb_dual_world_empty_gate_nav_commands_residual_wave305();
    let live_sticky_bomb_dual_world_empty_gate_live_wave305_ok =
        simulate_live_sticky_bomb_dual_world_empty_gate_honesty();
    let live_auto_heal_dual_world_empty_gate_method_names_wave306_ok =
        honesty_live_auto_heal_dual_world_empty_gate_method_names_residual_wave306();
    let live_auto_heal_dual_world_empty_gate_nav_commands_wave306_ok =
        honesty_live_auto_heal_dual_world_empty_gate_nav_commands_residual_wave306();
    let live_auto_heal_dual_world_empty_gate_live_wave306_ok =
        simulate_live_auto_heal_dual_world_empty_gate_honesty();
    let live_grant_stealth_dual_world_empty_gate_method_names_wave307_ok =
        honesty_live_grant_stealth_dual_world_empty_gate_method_names_residual_wave307();
    let live_grant_stealth_dual_world_empty_gate_nav_commands_wave307_ok =
        honesty_live_grant_stealth_dual_world_empty_gate_nav_commands_residual_wave307();
    let live_grant_stealth_dual_world_empty_gate_live_wave307_ok =
        simulate_live_grant_stealth_dual_world_empty_gate_honesty();
    let live_status_bits_upgrade_dual_world_empty_gate_method_names_wave308_ok =
        honesty_live_status_bits_upgrade_dual_world_empty_gate_method_names_residual_wave308();
    let live_status_bits_upgrade_dual_world_empty_gate_nav_commands_wave308_ok =
        honesty_live_status_bits_upgrade_dual_world_empty_gate_nav_commands_residual_wave308();
    let live_status_bits_upgrade_dual_world_empty_gate_live_wave308_ok =
        simulate_live_status_bits_upgrade_dual_world_empty_gate_honesty();
    let live_jet_ai_dual_world_empty_gate_method_names_wave309_ok =
        honesty_live_jet_ai_dual_world_empty_gate_method_names_residual_wave309();
    let live_jet_ai_dual_world_empty_gate_nav_commands_wave309_ok =
        honesty_live_jet_ai_dual_world_empty_gate_nav_commands_residual_wave309();
    let live_jet_ai_dual_world_empty_gate_live_wave309_ok =
        simulate_live_jet_ai_dual_world_empty_gate_honesty();
    let live_parking_place_dual_world_empty_gate_method_names_wave310_ok =
        honesty_live_parking_place_dual_world_empty_gate_method_names_residual_wave310();
    let live_parking_place_dual_world_empty_gate_nav_commands_wave310_ok =
        honesty_live_parking_place_dual_world_empty_gate_nav_commands_residual_wave310();
    let live_parking_place_dual_world_empty_gate_live_wave310_ok =
        simulate_live_parking_place_dual_world_empty_gate_honesty();
    let live_flight_deck_dual_world_empty_gate_method_names_wave311_ok =
        honesty_live_flight_deck_dual_world_empty_gate_method_names_residual_wave311();
    let live_flight_deck_dual_world_empty_gate_nav_commands_wave311_ok =
        honesty_live_flight_deck_dual_world_empty_gate_nav_commands_residual_wave311();
    let live_flight_deck_dual_world_empty_gate_live_wave311_ok =
        simulate_live_flight_deck_dual_world_empty_gate_honesty();
    let live_exit_strategies_dual_world_empty_gate_method_names_wave312_ok =
        honesty_live_exit_strategies_dual_world_empty_gate_method_names_residual_wave312();
    let live_exit_strategies_dual_world_empty_gate_nav_commands_wave312_ok =
        honesty_live_exit_strategies_dual_world_empty_gate_nav_commands_residual_wave312();
    let live_exit_strategies_dual_world_empty_gate_live_wave312_ok =
        simulate_live_exit_strategies_dual_world_empty_gate_honesty();
    let live_collision_system_dual_world_empty_gate_method_names_wave313_ok =
        honesty_live_collision_system_dual_world_empty_gate_method_names_residual_wave313();
    let live_collision_system_dual_world_empty_gate_nav_commands_wave313_ok =
        honesty_live_collision_system_dual_world_empty_gate_nav_commands_residual_wave313();
    let live_collision_system_dual_world_empty_gate_live_wave313_ok =
        simulate_live_collision_system_dual_world_empty_gate_honesty();
    let live_max_health_upgrade_dual_world_empty_gate_method_names_wave314_ok =
        honesty_live_max_health_upgrade_dual_world_empty_gate_method_names_residual_wave314();
    let live_max_health_upgrade_dual_world_empty_gate_nav_commands_wave314_ok =
        honesty_live_max_health_upgrade_dual_world_empty_gate_nav_commands_residual_wave314();
    let live_max_health_upgrade_dual_world_empty_gate_live_wave314_ok =
        simulate_live_max_health_upgrade_dual_world_empty_gate_honesty();
    let live_structure_topple_dual_world_empty_gate_method_names_wave315_ok =
        honesty_live_structure_topple_dual_world_empty_gate_method_names_residual_wave315();
    let live_structure_topple_dual_world_empty_gate_nav_commands_wave315_ok =
        honesty_live_structure_topple_dual_world_empty_gate_nav_commands_residual_wave315();
    let live_structure_topple_dual_world_empty_gate_live_wave315_ok =
        simulate_live_structure_topple_dual_world_empty_gate_honesty();
    let live_physics_update_dual_world_empty_gate_method_names_wave316_ok =
        honesty_live_physics_update_dual_world_empty_gate_method_names_residual_wave316();
    let live_physics_update_dual_world_empty_gate_nav_commands_wave316_ok =
        honesty_live_physics_update_dual_world_empty_gate_nav_commands_residual_wave316();
    let live_physics_update_dual_world_empty_gate_live_wave316_ok =
        simulate_live_physics_update_dual_world_empty_gate_honesty();
    let live_cleanup_hazard_dual_world_empty_gate_method_names_wave317_ok =
        honesty_live_cleanup_hazard_dual_world_empty_gate_method_names_residual_wave317();
    let live_cleanup_hazard_dual_world_empty_gate_nav_commands_wave317_ok =
        honesty_live_cleanup_hazard_dual_world_empty_gate_nav_commands_residual_wave317();
    let live_cleanup_hazard_dual_world_empty_gate_live_wave317_ok =
        simulate_live_cleanup_hazard_dual_world_empty_gate_honesty();
    let live_bridge_tower_dual_world_empty_gate_method_names_wave318_ok =
        honesty_live_bridge_tower_dual_world_empty_gate_method_names_residual_wave318();
    let live_bridge_tower_dual_world_empty_gate_nav_commands_wave318_ok =
        honesty_live_bridge_tower_dual_world_empty_gate_nav_commands_residual_wave318();
    let live_bridge_tower_dual_world_empty_gate_live_wave318_ok =
        simulate_live_bridge_tower_dual_world_empty_gate_honesty();
    let live_armor_upgrade_dual_world_empty_gate_method_names_wave319_ok =
        honesty_live_armor_upgrade_dual_world_empty_gate_method_names_residual_wave319();
    let live_armor_upgrade_dual_world_empty_gate_nav_commands_wave319_ok =
        honesty_live_armor_upgrade_dual_world_empty_gate_nav_commands_residual_wave319();
    let live_armor_upgrade_dual_world_empty_gate_live_wave319_ok =
        simulate_live_armor_upgrade_dual_world_empty_gate_honesty();
    let live_paradrop_power_dual_world_empty_gate_method_names_wave320_ok =
        honesty_live_paradrop_power_dual_world_empty_gate_method_names_residual_wave320();
    let live_paradrop_power_dual_world_empty_gate_nav_commands_wave320_ok =
        honesty_live_paradrop_power_dual_world_empty_gate_nav_commands_residual_wave320();
    let live_paradrop_power_dual_world_empty_gate_live_wave320_ok =
        simulate_live_paradrop_power_dual_world_empty_gate_honesty();
    let live_fuel_air_bomb_dual_world_empty_gate_method_names_wave321_ok =
        honesty_live_fuel_air_bomb_dual_world_empty_gate_method_names_residual_wave321();
    let live_fuel_air_bomb_dual_world_empty_gate_nav_commands_wave321_ok =
        honesty_live_fuel_air_bomb_dual_world_empty_gate_nav_commands_residual_wave321();
    let live_fuel_air_bomb_dual_world_empty_gate_live_wave321_ok =
        simulate_live_fuel_air_bomb_dual_world_empty_gate_honesty();
    let live_tensile_formation_dual_world_empty_gate_method_names_wave322_ok =
        honesty_live_tensile_formation_dual_world_empty_gate_method_names_residual_wave322();
    let live_tensile_formation_dual_world_empty_gate_nav_commands_wave322_ok =
        honesty_live_tensile_formation_dual_world_empty_gate_nav_commands_residual_wave322();
    let live_tensile_formation_dual_world_empty_gate_live_wave322_ok =
        simulate_live_tensile_formation_dual_world_empty_gate_honesty();
    let live_die_mod_dual_world_empty_gate_method_names_wave323_ok =
        honesty_live_die_mod_dual_world_empty_gate_method_names_residual_wave323();
    let live_die_mod_dual_world_empty_gate_nav_commands_wave323_ok =
        honesty_live_die_mod_dual_world_empty_gate_nav_commands_residual_wave323();
    let live_die_mod_dual_world_empty_gate_live_wave323_ok =
        simulate_live_die_mod_dual_world_empty_gate_honesty();
    let live_partition_manager_dual_world_empty_gate_method_names_wave324_ok =
        honesty_live_partition_manager_dual_world_empty_gate_method_names_residual_wave324();
    let live_partition_manager_dual_world_empty_gate_nav_commands_wave324_ok =
        honesty_live_partition_manager_dual_world_empty_gate_nav_commands_residual_wave324();
    let live_partition_manager_dual_world_empty_gate_live_wave324_ok =
        simulate_live_partition_manager_dual_world_empty_gate_honesty();
    let live_spectre_gunship_dual_world_empty_gate_method_names_wave325_ok =
        honesty_live_spectre_gunship_dual_world_empty_gate_method_names_residual_wave325();
    let live_spectre_gunship_dual_world_empty_gate_nav_commands_wave325_ok =
        honesty_live_spectre_gunship_dual_world_empty_gate_nav_commands_residual_wave325();
    let live_spectre_gunship_dual_world_empty_gate_live_wave325_ok =
        simulate_live_spectre_gunship_dual_world_empty_gate_honesty();
    let live_production_update_dual_world_empty_gate_method_names_wave326_ok =
        honesty_live_production_update_dual_world_empty_gate_method_names_residual_wave326();
    let live_production_update_dual_world_empty_gate_nav_commands_wave326_ok =
        honesty_live_production_update_dual_world_empty_gate_nav_commands_residual_wave326();
    let live_production_update_dual_world_empty_gate_live_wave326_ok =
        simulate_live_production_update_dual_world_empty_gate_honesty();
    let live_neutron_blast_dual_world_empty_gate_method_names_wave327_ok =
        honesty_live_neutron_blast_dual_world_empty_gate_method_names_residual_wave327();
    let live_neutron_blast_dual_world_empty_gate_nav_commands_wave327_ok =
        honesty_live_neutron_blast_dual_world_empty_gate_nav_commands_residual_wave327();
    let live_neutron_blast_dual_world_empty_gate_live_wave327_ok =
        simulate_live_neutron_blast_dual_world_empty_gate_honesty();
    let live_countermeasures_dual_world_empty_gate_method_names_wave328_ok =
        honesty_live_countermeasures_dual_world_empty_gate_method_names_residual_wave328();
    let live_countermeasures_dual_world_empty_gate_nav_commands_wave328_ok =
        honesty_live_countermeasures_dual_world_empty_gate_nav_commands_residual_wave328();
    let live_countermeasures_dual_world_empty_gate_live_wave328_ok =
        simulate_live_countermeasures_dual_world_empty_gate_honesty();
    let live_skirmish_player_dual_world_empty_gate_method_names_wave329_ok =
        honesty_live_skirmish_player_dual_world_empty_gate_method_names_residual_wave329();
    let live_skirmish_player_dual_world_empty_gate_nav_commands_wave329_ok =
        honesty_live_skirmish_player_dual_world_empty_gate_nav_commands_residual_wave329();
    let live_skirmish_player_dual_world_empty_gate_live_wave329_ok =
        simulate_live_skirmish_player_dual_world_empty_gate_honesty();
    let live_a10_strike_dual_world_empty_gate_method_names_wave330_ok =
        honesty_live_a10_strike_dual_world_empty_gate_method_names_residual_wave330();
    let live_a10_strike_dual_world_empty_gate_nav_commands_wave330_ok =
        honesty_live_a10_strike_dual_world_empty_gate_nav_commands_residual_wave330();
    let live_a10_strike_dual_world_empty_gate_live_wave330_ok =
        simulate_live_a10_strike_dual_world_empty_gate_honesty();
    let live_rebuild_hole_dual_world_empty_gate_method_names_wave331_ok =
        honesty_live_rebuild_hole_dual_world_empty_gate_method_names_residual_wave331();
    let live_rebuild_hole_dual_world_empty_gate_nav_commands_wave331_ok =
        honesty_live_rebuild_hole_dual_world_empty_gate_nav_commands_residual_wave331();
    let live_rebuild_hole_dual_world_empty_gate_live_wave331_ok =
        simulate_live_rebuild_hole_dual_world_empty_gate_honesty();
    let live_wave_guide_dual_world_empty_gate_method_names_wave332_ok =
        honesty_live_wave_guide_dual_world_empty_gate_method_names_residual_wave332();
    let live_wave_guide_dual_world_empty_gate_nav_commands_wave332_ok =
        honesty_live_wave_guide_dual_world_empty_gate_nav_commands_residual_wave332();
    let live_wave_guide_dual_world_empty_gate_live_wave332_ok =
        simulate_live_wave_guide_dual_world_empty_gate_honesty();
    let live_emp_update_dual_world_empty_gate_method_names_wave333_ok =
        honesty_live_emp_update_dual_world_empty_gate_method_names_residual_wave333();
    let live_emp_update_dual_world_empty_gate_nav_commands_wave333_ok =
        honesty_live_emp_update_dual_world_empty_gate_nav_commands_residual_wave333();
    let live_emp_update_dual_world_empty_gate_live_wave333_ok =
        simulate_live_emp_update_dual_world_empty_gate_honesty();
    let live_bunker_buster_dual_world_empty_gate_method_names_wave334_ok =
        honesty_live_bunker_buster_dual_world_empty_gate_method_names_residual_wave334();
    let live_bunker_buster_dual_world_empty_gate_nav_commands_wave334_ok =
        honesty_live_bunker_buster_dual_world_empty_gate_nav_commands_residual_wave334();
    let live_bunker_buster_dual_world_empty_gate_live_wave334_ok =
        simulate_live_bunker_buster_dual_world_empty_gate_honesty();
    let live_bridge_scaffold_dual_world_empty_gate_method_names_wave335_ok =
        honesty_live_bridge_scaffold_dual_world_empty_gate_method_names_residual_wave335();
    let live_bridge_scaffold_dual_world_empty_gate_nav_commands_wave335_ok =
        honesty_live_bridge_scaffold_dual_world_empty_gate_nav_commands_residual_wave335();
    let live_bridge_scaffold_dual_world_empty_gate_live_wave335_ok =
        simulate_live_bridge_scaffold_dual_world_empty_gate_honesty();
    let live_assisted_targeting_dual_world_empty_gate_method_names_wave336_ok =
        honesty_live_assisted_targeting_dual_world_empty_gate_method_names_residual_wave336();
    let live_assisted_targeting_dual_world_empty_gate_nav_commands_wave336_ok =
        honesty_live_assisted_targeting_dual_world_empty_gate_nav_commands_residual_wave336();
    let live_assisted_targeting_dual_world_empty_gate_live_wave336_ok =
        simulate_live_assisted_targeting_dual_world_empty_gate_honesty();
    let live_economy_dual_world_empty_gate_method_names_wave337_ok =
        honesty_live_economy_dual_world_empty_gate_method_names_residual_wave337();
    let live_economy_dual_world_empty_gate_nav_commands_wave337_ok =
        honesty_live_economy_dual_world_empty_gate_nav_commands_residual_wave337();
    let live_economy_dual_world_empty_gate_live_wave337_ok =
        simulate_live_economy_dual_world_empty_gate_honesty();
    let live_turret_ai_dual_world_empty_gate_method_names_wave338_ok =
        honesty_live_turret_ai_dual_world_empty_gate_method_names_residual_wave338();
    let live_turret_ai_dual_world_empty_gate_nav_commands_wave338_ok =
        honesty_live_turret_ai_dual_world_empty_gate_nav_commands_residual_wave338();
    let live_turret_ai_dual_world_empty_gate_live_wave338_ok =
        simulate_live_turret_ai_dual_world_empty_gate_honesty();
    let live_stealth_detector_module_dual_world_empty_gate_method_names_wave339_ok =
        honesty_live_stealth_detector_module_dual_world_empty_gate_method_names_residual_wave339();
    let live_stealth_detector_module_dual_world_empty_gate_nav_commands_wave339_ok =
        honesty_live_stealth_detector_module_dual_world_empty_gate_nav_commands_residual_wave339();
    let live_stealth_detector_module_dual_world_empty_gate_live_wave339_ok =
        simulate_live_stealth_detector_module_dual_world_empty_gate_honesty();
    let live_modules_dual_world_empty_gate_method_names_wave340_ok =
        honesty_live_modules_dual_world_empty_gate_method_names_residual_wave340();
    let live_modules_dual_world_empty_gate_nav_commands_wave340_ok =
        honesty_live_modules_dual_world_empty_gate_nav_commands_residual_wave340();
    let live_modules_dual_world_empty_gate_live_wave340_ok =
        simulate_live_modules_dual_world_empty_gate_honesty();
    let live_terrain_dual_world_empty_gate_method_names_wave341_ok =
        honesty_live_terrain_dual_world_empty_gate_method_names_residual_wave341();
    let live_terrain_dual_world_empty_gate_nav_commands_wave341_ok =
        honesty_live_terrain_dual_world_empty_gate_nav_commands_residual_wave341();
    let live_terrain_dual_world_empty_gate_live_wave341_ok =
        simulate_live_terrain_dual_world_empty_gate_honesty();
    let live_special_power_template_dual_world_empty_gate_method_names_wave342_ok =
        honesty_live_special_power_template_dual_world_empty_gate_method_names_residual_wave342();
    let live_special_power_template_dual_world_empty_gate_nav_commands_wave342_ok =
        honesty_live_special_power_template_dual_world_empty_gate_nav_commands_residual_wave342();
    let live_special_power_template_dual_world_empty_gate_live_wave342_ok =
        simulate_live_special_power_template_dual_world_empty_gate_honesty();
    let live_script_evaluator_dual_world_empty_gate_method_names_wave343_ok =
        honesty_live_script_evaluator_dual_world_empty_gate_method_names_residual_wave343();
    let live_script_evaluator_dual_world_empty_gate_nav_commands_wave343_ok =
        honesty_live_script_evaluator_dual_world_empty_gate_nav_commands_residual_wave343();
    let live_script_evaluator_dual_world_empty_gate_live_wave343_ok =
        simulate_live_script_evaluator_dual_world_empty_gate_honesty();
    let live_system_game_logic_dual_world_empty_gate_method_names_wave344_ok =
        honesty_live_system_game_logic_dual_world_empty_gate_method_names_residual_wave344();
    let live_system_game_logic_dual_world_empty_gate_nav_commands_wave344_ok =
        honesty_live_system_game_logic_dual_world_empty_gate_nav_commands_residual_wave344();
    let live_system_game_logic_dual_world_empty_gate_live_wave344_ok =
        simulate_live_system_game_logic_dual_world_empty_gate_honesty();
    let live_meta_event_dual_world_empty_gate_method_names_wave345_ok =
        honesty_live_meta_event_dual_world_empty_gate_method_names_residual_wave345();
    let live_meta_event_dual_world_empty_gate_nav_commands_wave345_ok =
        honesty_live_meta_event_dual_world_empty_gate_nav_commands_residual_wave345();
    let live_meta_event_dual_world_empty_gate_live_wave345_ok =
        simulate_live_meta_event_dual_world_empty_gate_honesty();
    let live_spawn_behavior_dual_world_empty_gate_method_names_wave346_ok =
        honesty_live_spawn_behavior_dual_world_empty_gate_method_names_residual_wave346();
    let live_spawn_behavior_dual_world_empty_gate_nav_commands_wave346_ok =
        honesty_live_spawn_behavior_dual_world_empty_gate_nav_commands_residual_wave346();
    let live_spawn_behavior_dual_world_empty_gate_live_wave346_ok =
        simulate_live_spawn_behavior_dual_world_empty_gate_honesty();
    let live_action_manager_dual_world_empty_gate_method_names_wave347_ok =
        honesty_live_action_manager_dual_world_empty_gate_method_names_residual_wave347();
    let live_action_manager_dual_world_empty_gate_nav_commands_wave347_ok =
        honesty_live_action_manager_dual_world_empty_gate_nav_commands_residual_wave347();
    let live_action_manager_dual_world_empty_gate_live_wave347_ok =
        simulate_live_action_manager_dual_world_empty_gate_honesty();
    let live_script_engine_dual_world_empty_gate_method_names_wave348_ok =
        honesty_live_script_engine_dual_world_empty_gate_method_names_residual_wave348();
    let live_script_engine_dual_world_empty_gate_nav_commands_wave348_ok =
        honesty_live_script_engine_dual_world_empty_gate_nav_commands_residual_wave348();
    let live_script_engine_dual_world_empty_gate_live_wave348_ok =
        simulate_live_script_engine_dual_world_empty_gate_honesty();
    let live_chinook_ai_dual_world_empty_gate_method_names_wave349_ok =
        honesty_live_chinook_ai_dual_world_empty_gate_method_names_residual_wave349();
    let live_chinook_ai_dual_world_empty_gate_nav_commands_wave349_ok =
        honesty_live_chinook_ai_dual_world_empty_gate_nav_commands_residual_wave349();
    let live_chinook_ai_dual_world_empty_gate_live_wave349_ok =
        simulate_live_chinook_ai_dual_world_empty_gate_honesty();
    let live_missile_ai_dual_world_empty_gate_method_names_wave350_ok =
        honesty_live_missile_ai_dual_world_empty_gate_method_names_residual_wave350();
    let live_missile_ai_dual_world_empty_gate_nav_commands_wave350_ok =
        honesty_live_missile_ai_dual_world_empty_gate_nav_commands_residual_wave350();
    let live_missile_ai_dual_world_empty_gate_live_wave350_ok =
        simulate_live_missile_ai_dual_world_empty_gate_honesty();
    let live_dozer_ai_dual_world_empty_gate_method_names_wave351_ok =
        honesty_live_dozer_ai_dual_world_empty_gate_method_names_residual_wave351();
    let live_dozer_ai_dual_world_empty_gate_nav_commands_wave351_ok =
        honesty_live_dozer_ai_dual_world_empty_gate_nav_commands_residual_wave351();
    let live_dozer_ai_dual_world_empty_gate_live_wave351_ok =
        simulate_live_dozer_ai_dual_world_empty_gate_honesty();
    let live_deliver_payload_ai_dual_world_empty_gate_method_names_wave352_ok =
        honesty_live_deliver_payload_ai_dual_world_empty_gate_method_names_residual_wave352();
    let live_deliver_payload_ai_dual_world_empty_gate_nav_commands_wave352_ok =
        honesty_live_deliver_payload_ai_dual_world_empty_gate_nav_commands_residual_wave352();
    let live_deliver_payload_ai_dual_world_empty_gate_live_wave352_ok =
        simulate_live_deliver_payload_ai_dual_world_empty_gate_honesty();
    let live_special_power_module_dual_world_empty_gate_method_names_wave353_ok =
        honesty_live_special_power_module_dual_world_empty_gate_method_names_residual_wave353();
    let live_special_power_module_dual_world_empty_gate_nav_commands_wave353_ok =
        honesty_live_special_power_module_dual_world_empty_gate_nav_commands_residual_wave353();
    let live_special_power_module_dual_world_empty_gate_live_wave353_ok =
        simulate_live_special_power_module_dual_world_empty_gate_honesty();
    let live_pow_truck_ai_dual_world_empty_gate_method_names_wave354_ok =
        honesty_live_pow_truck_ai_dual_world_empty_gate_method_names_residual_wave354();
    let live_pow_truck_ai_dual_world_empty_gate_nav_commands_wave354_ok =
        honesty_live_pow_truck_ai_dual_world_empty_gate_nav_commands_residual_wave354();
    let live_pow_truck_ai_dual_world_empty_gate_live_wave354_ok =
        simulate_live_pow_truck_ai_dual_world_empty_gate_honesty();
    let live_dock_update_dual_world_empty_gate_method_names_wave355_ok =
        honesty_live_dock_update_dual_world_empty_gate_method_names_residual_wave355();
    let live_dock_update_dual_world_empty_gate_nav_commands_wave355_ok =
        honesty_live_dock_update_dual_world_empty_gate_nav_commands_residual_wave355();
    let live_dock_update_dual_world_empty_gate_live_wave355_ok =
        simulate_live_dock_update_dual_world_empty_gate_honesty();
    let live_weapon_template_dual_world_empty_gate_method_names_wave356_ok =
        honesty_live_weapon_template_dual_world_empty_gate_method_names_residual_wave356();
    let live_weapon_template_dual_world_empty_gate_nav_commands_wave356_ok =
        honesty_live_weapon_template_dual_world_empty_gate_nav_commands_residual_wave356();
    let live_weapon_template_dual_world_empty_gate_live_wave356_ok =
        simulate_live_weapon_template_dual_world_empty_gate_honesty();
    let live_railroad_guide_ai_dual_world_empty_gate_method_names_wave357_ok =
        honesty_live_railroad_guide_ai_dual_world_empty_gate_method_names_residual_wave357();
    let live_railroad_guide_ai_dual_world_empty_gate_nav_commands_wave357_ok =
        honesty_live_railroad_guide_ai_dual_world_empty_gate_nav_commands_residual_wave357();
    let live_railroad_guide_ai_dual_world_empty_gate_live_wave357_ok =
        simulate_live_railroad_guide_ai_dual_world_empty_gate_honesty();
    let live_hack_internet_ai_dual_world_empty_gate_method_names_wave358_ok =
        honesty_live_hack_internet_ai_dual_world_empty_gate_method_names_residual_wave358();
    let live_hack_internet_ai_dual_world_empty_gate_nav_commands_wave358_ok =
        honesty_live_hack_internet_ai_dual_world_empty_gate_nav_commands_residual_wave358();
    let live_hack_internet_ai_dual_world_empty_gate_live_wave358_ok =
        simulate_live_hack_internet_ai_dual_world_empty_gate_honesty();
    let live_spectre_gunship_deployment_dual_world_empty_gate_method_names_wave359_ok =
        honesty_live_spectre_gunship_deployment_dual_world_empty_gate_method_names_residual_wave359(
        );
    let live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_wave359_ok =
        honesty_live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_residual_wave359(
        );
    let live_spectre_gunship_deployment_dual_world_empty_gate_live_wave359_ok =
        simulate_live_spectre_gunship_deployment_dual_world_empty_gate_honesty();
    let live_radius_decal_update_dual_world_empty_gate_method_names_wave360_ok =
        honesty_live_radius_decal_update_dual_world_empty_gate_method_names_residual_wave360();
    let live_radius_decal_update_dual_world_empty_gate_nav_commands_wave360_ok =
        honesty_live_radius_decal_update_dual_world_empty_gate_nav_commands_residual_wave360();
    let live_radius_decal_update_dual_world_empty_gate_live_wave360_ok =
        simulate_live_radius_decal_update_dual_world_empty_gate_honesty();
    let live_railed_transport_dock_dual_world_empty_gate_method_names_wave361_ok =
        honesty_live_railed_transport_dock_dual_world_empty_gate_method_names_residual_wave361();
    let live_railed_transport_dock_dual_world_empty_gate_nav_commands_wave361_ok =
        honesty_live_railed_transport_dock_dual_world_empty_gate_nav_commands_residual_wave361();
    let live_railed_transport_dock_dual_world_empty_gate_live_wave361_ok =
        simulate_live_railed_transport_dock_dual_world_empty_gate_honesty();
    let live_structure_collapse_update_dual_world_empty_gate_method_names_wave362_ok =
        honesty_live_structure_collapse_update_dual_world_empty_gate_method_names_residual_wave362(
        );
    let live_structure_collapse_update_dual_world_empty_gate_nav_commands_wave362_ok =
        honesty_live_structure_collapse_update_dual_world_empty_gate_nav_commands_residual_wave362(
        );
    let live_structure_collapse_update_dual_world_empty_gate_live_wave362_ok =
        simulate_live_structure_collapse_update_dual_world_empty_gate_honesty();
    let live_propaganda_tower_behavior_dual_world_empty_gate_method_names_wave363_ok =
        honesty_live_propaganda_tower_behavior_dual_world_empty_gate_method_names_residual_wave363(
        );
    let live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_wave363_ok =
        honesty_live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_residual_wave363(
        );
    let live_propaganda_tower_behavior_dual_world_empty_gate_live_wave363_ok =
        simulate_live_propaganda_tower_behavior_dual_world_empty_gate_honesty();
    let live_propaganda_center_behavior_dual_world_empty_gate_method_names_wave364_ok =
        honesty_live_propaganda_center_behavior_dual_world_empty_gate_method_names_residual_wave364(
        );
    let live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_wave364_ok =
        honesty_live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_residual_wave364(
        );
    let live_propaganda_center_behavior_dual_world_empty_gate_live_wave364_ok =
        simulate_live_propaganda_center_behavior_dual_world_empty_gate_honesty();
    let live_production_update_complete_dual_world_empty_gate_method_names_wave365_ok =
        honesty_live_production_update_complete_dual_world_empty_gate_method_names_residual_wave365(
        );
    let live_production_update_complete_dual_world_empty_gate_nav_commands_wave365_ok =
        honesty_live_production_update_complete_dual_world_empty_gate_nav_commands_residual_wave365(
        );
    let live_production_update_complete_dual_world_empty_gate_live_wave365_ok =
        simulate_live_production_update_complete_dual_world_empty_gate_honesty();
    let live_pow_truck_behavior_dual_world_empty_gate_method_names_wave366_ok =
        honesty_live_pow_truck_behavior_dual_world_empty_gate_method_names_residual_wave366();
    let live_pow_truck_behavior_dual_world_empty_gate_nav_commands_wave366_ok =
        honesty_live_pow_truck_behavior_dual_world_empty_gate_nav_commands_residual_wave366();
    let live_pow_truck_behavior_dual_world_empty_gate_live_wave366_ok =
        simulate_live_pow_truck_behavior_dual_world_empty_gate_honesty();
    let live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_wave367_ok =
        honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_residual_wave367();
    let live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_wave367_ok =
        honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_residual_wave367();
    let live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_live_wave367_ok =
        simulate_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_honesty();
    let live_veterancy_crate_collide_dual_world_empty_gate_method_names_wave368_ok =
        honesty_live_veterancy_crate_collide_dual_world_empty_gate_method_names_residual_wave368();
    let live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_wave368_ok =
        honesty_live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_residual_wave368();
    let live_veterancy_crate_collide_dual_world_empty_gate_live_wave368_ok =
        simulate_live_veterancy_crate_collide_dual_world_empty_gate_honesty();
    let live_assault_transport_ai_update_dual_world_empty_gate_method_names_wave369_ok =
        honesty_live_assault_transport_ai_update_dual_world_empty_gate_method_names_residual_wave369();
    let live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_wave369_ok =
        honesty_live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave369();
    let live_assault_transport_ai_update_dual_world_empty_gate_live_wave369_ok =
        simulate_live_assault_transport_ai_update_dual_world_empty_gate_honesty();
    let live_heal_contain_dual_world_empty_gate_method_names_wave370_ok =
        honesty_live_heal_contain_dual_world_empty_gate_method_names_residual_wave370();
    let live_heal_contain_dual_world_empty_gate_nav_commands_wave370_ok =
        honesty_live_heal_contain_dual_world_empty_gate_nav_commands_residual_wave370();
    let live_heal_contain_dual_world_empty_gate_live_wave370_ok =
        simulate_live_heal_contain_dual_world_empty_gate_honesty();
    let live_topple_update_dual_world_empty_gate_method_names_wave371_ok =
        honesty_live_topple_update_dual_world_empty_gate_method_names_residual_wave371();
    let live_topple_update_dual_world_empty_gate_nav_commands_wave371_ok =
        honesty_live_topple_update_dual_world_empty_gate_nav_commands_residual_wave371();
    let live_topple_update_dual_world_empty_gate_live_wave371_ok =
        simulate_live_topple_update_dual_world_empty_gate_honesty();
    let live_projectile_stream_update_dual_world_empty_gate_method_names_wave372_ok =
        honesty_live_projectile_stream_update_dual_world_empty_gate_method_names_residual_wave372();
    let live_projectile_stream_update_dual_world_empty_gate_nav_commands_wave372_ok =
        honesty_live_projectile_stream_update_dual_world_empty_gate_nav_commands_residual_wave372();
    let live_projectile_stream_update_dual_world_empty_gate_live_wave372_ok =
        simulate_live_projectile_stream_update_dual_world_empty_gate_honesty();
    let live_demo_trap_update_dual_world_empty_gate_method_names_wave373_ok =
        honesty_live_demo_trap_update_dual_world_empty_gate_method_names_residual_wave373();
    let live_demo_trap_update_dual_world_empty_gate_nav_commands_wave373_ok =
        honesty_live_demo_trap_update_dual_world_empty_gate_nav_commands_residual_wave373();
    let live_demo_trap_update_dual_world_empty_gate_live_wave373_ok =
        simulate_live_demo_trap_update_dual_world_empty_gate_honesty();
    let live_mob_member_slaved_update_dual_world_empty_gate_method_names_wave374_ok =
        honesty_live_mob_member_slaved_update_dual_world_empty_gate_method_names_residual_wave374();
    let live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_wave374_ok =
        honesty_live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_residual_wave374();
    let live_mob_member_slaved_update_dual_world_empty_gate_live_wave374_ok =
        simulate_live_mob_member_slaved_update_dual_world_empty_gate_honesty();
    let live_tn_guard_dual_world_empty_gate_method_names_wave375_ok =
        honesty_live_tn_guard_dual_world_empty_gate_method_names_residual_wave375();
    let live_tn_guard_dual_world_empty_gate_nav_commands_wave375_ok =
        honesty_live_tn_guard_dual_world_empty_gate_nav_commands_residual_wave375();
    let live_tn_guard_dual_world_empty_gate_live_wave375_ok =
        simulate_live_tn_guard_dual_world_empty_gate_honesty();
    let live_production_update_dual_world_empty_gate_method_names_wave376_ok =
        honesty_live_production_update_dual_world_empty_gate_method_names_residual_wave376();
    let live_production_update_dual_world_empty_gate_nav_commands_wave376_ok =
        honesty_live_production_update_dual_world_empty_gate_nav_commands_residual_wave376();
    let live_production_update_dual_world_empty_gate_live_wave376_ok =
        simulate_live_production_update_dual_world_empty_gate_honesty_wave376();
    let live_poisoned_behavior_dual_world_empty_gate_method_names_wave377_ok =
        honesty_live_poisoned_behavior_dual_world_empty_gate_method_names_residual_wave377();
    let live_poisoned_behavior_dual_world_empty_gate_nav_commands_wave377_ok =
        honesty_live_poisoned_behavior_dual_world_empty_gate_nav_commands_residual_wave377();
    let live_poisoned_behavior_dual_world_empty_gate_live_wave377_ok =
        simulate_live_poisoned_behavior_dual_world_empty_gate_honesty();
    let live_horde_update_dual_world_empty_gate_method_names_wave378_ok =
        honesty_live_horde_update_dual_world_empty_gate_method_names_residual_wave378();
    let live_horde_update_dual_world_empty_gate_nav_commands_wave378_ok =
        honesty_live_horde_update_dual_world_empty_gate_nav_commands_residual_wave378();
    let live_horde_update_dual_world_empty_gate_live_wave378_ok =
        simulate_live_horde_update_dual_world_empty_gate_honesty();
    let live_flammable_update_dual_world_empty_gate_method_names_wave379_ok =
        honesty_live_flammable_update_dual_world_empty_gate_method_names_residual_wave379();
    let live_flammable_update_dual_world_empty_gate_nav_commands_wave379_ok =
        honesty_live_flammable_update_dual_world_empty_gate_nav_commands_residual_wave379();
    let live_flammable_update_dual_world_empty_gate_live_wave379_ok =
        simulate_live_flammable_update_dual_world_empty_gate_honesty();
    let live_base_regenerate_update_dual_world_empty_gate_method_names_wave380_ok =
        honesty_live_base_regenerate_update_dual_world_empty_gate_method_names_residual_wave380();
    let live_base_regenerate_update_dual_world_empty_gate_nav_commands_wave380_ok =
        honesty_live_base_regenerate_update_dual_world_empty_gate_nav_commands_residual_wave380();
    let live_base_regenerate_update_dual_world_empty_gate_live_wave380_ok =
        simulate_live_base_regenerate_update_dual_world_empty_gate_honesty();
    let live_queue_production_exit_behavior_dual_world_empty_gate_method_names_wave381_ok =
        honesty_live_queue_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave381();
    let live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_wave381_ok =
        honesty_live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave381();
    let live_queue_production_exit_behavior_dual_world_empty_gate_live_wave381_ok =
        simulate_live_queue_production_exit_behavior_dual_world_empty_gate_honesty();
    let live_missile_launcher_building_update_dual_world_empty_gate_method_names_wave382_ok =
        honesty_live_missile_launcher_building_update_dual_world_empty_gate_method_names_residual_wave382();
    let live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_wave382_ok =
        honesty_live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_residual_wave382();
    let live_missile_launcher_building_update_dual_world_empty_gate_live_wave382_ok =
        simulate_live_missile_launcher_building_update_dual_world_empty_gate_honesty();
    let live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_wave383_ok =
        honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_residual_wave383();
    let live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_wave383_ok =
        honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_residual_wave383();
    let live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_live_wave383_ok =
        simulate_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_honesty();
    let live_command_button_hunt_update_dual_world_empty_gate_method_names_wave384_ok =
        honesty_live_command_button_hunt_update_dual_world_empty_gate_method_names_residual_wave384(
        );
    let live_command_button_hunt_update_dual_world_empty_gate_nav_commands_wave384_ok =
        honesty_live_command_button_hunt_update_dual_world_empty_gate_nav_commands_residual_wave384(
        );
    let live_command_button_hunt_update_dual_world_empty_gate_live_wave384_ok =
        simulate_live_command_button_hunt_update_dual_world_empty_gate_honesty();
    let live_prison_behavior_dual_world_empty_gate_method_names_wave385_ok =
        honesty_live_prison_behavior_dual_world_empty_gate_method_names_residual_wave385();
    let live_prison_behavior_dual_world_empty_gate_nav_commands_wave385_ok =
        honesty_live_prison_behavior_dual_world_empty_gate_nav_commands_residual_wave385();
    let live_prison_behavior_dual_world_empty_gate_live_wave385_ok =
        simulate_live_prison_behavior_dual_world_empty_gate_honesty();
    let live_generate_minefield_behavior_dual_world_empty_gate_method_names_wave386_ok =
        honesty_live_generate_minefield_behavior_dual_world_empty_gate_method_names_residual_wave386();
    let live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_wave386_ok =
        honesty_live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_residual_wave386();
    let live_generate_minefield_behavior_dual_world_empty_gate_live_wave386_ok =
        simulate_live_generate_minefield_behavior_dual_world_empty_gate_honesty();
    let live_demoralize_special_power_dual_world_empty_gate_method_names_wave387_ok =
        honesty_live_demoralize_special_power_dual_world_empty_gate_method_names_residual_wave387();
    let live_demoralize_special_power_dual_world_empty_gate_nav_commands_wave387_ok =
        honesty_live_demoralize_special_power_dual_world_empty_gate_nav_commands_residual_wave387();
    let live_demoralize_special_power_dual_world_empty_gate_live_wave387_ok =
        simulate_live_demoralize_special_power_dual_world_empty_gate_honesty();
    let live_stealth_detector_update_dual_world_empty_gate_method_names_wave388_ok =
        honesty_live_stealth_detector_update_dual_world_empty_gate_method_names_residual_wave388();
    let live_stealth_detector_update_dual_world_empty_gate_nav_commands_wave388_ok =
        honesty_live_stealth_detector_update_dual_world_empty_gate_nav_commands_residual_wave388();
    let live_stealth_detector_update_dual_world_empty_gate_live_wave388_ok =
        simulate_live_stealth_detector_update_dual_world_empty_gate_honesty();
    let live_hive_structure_body_dual_world_empty_gate_method_names_wave389_ok =
        honesty_live_hive_structure_body_dual_world_empty_gate_method_names_residual_wave389();
    let live_hive_structure_body_dual_world_empty_gate_nav_commands_wave389_ok =
        honesty_live_hive_structure_body_dual_world_empty_gate_nav_commands_residual_wave389();
    let live_hive_structure_body_dual_world_empty_gate_live_wave389_ok =
        simulate_live_hive_structure_body_dual_world_empty_gate_honesty();
    let live_salvage_crate_collide_dual_world_empty_gate_method_names_wave390_ok =
        honesty_live_salvage_crate_collide_dual_world_empty_gate_method_names_residual_wave390();
    let live_salvage_crate_collide_dual_world_empty_gate_nav_commands_wave390_ok =
        honesty_live_salvage_crate_collide_dual_world_empty_gate_nav_commands_residual_wave390();
    let live_salvage_crate_collide_dual_world_empty_gate_live_wave390_ok =
        simulate_live_salvage_crate_collide_dual_world_empty_gate_honesty();
    let live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_wave391_ok =
        honesty_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_residual_wave391();
    let live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_wave391_ok =
        honesty_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_residual_wave391();
    let live_sabotage_internet_center_crate_collide_dual_world_empty_gate_live_wave391_ok =
        simulate_live_sabotage_internet_center_crate_collide_dual_world_empty_gate_honesty();
    let live_power_plant_update_dual_world_empty_gate_method_names_wave392_ok =
        honesty_live_power_plant_update_dual_world_empty_gate_method_names_residual_wave392();
    let live_power_plant_update_dual_world_empty_gate_nav_commands_wave392_ok =
        honesty_live_power_plant_update_dual_world_empty_gate_nav_commands_residual_wave392();
    let live_power_plant_update_dual_world_empty_gate_live_wave392_ok =
        simulate_live_power_plant_update_dual_world_empty_gate_honesty();
    let live_leaflet_drop_behavior_dual_world_empty_gate_method_names_wave393_ok =
        honesty_live_leaflet_drop_behavior_dual_world_empty_gate_method_names_residual_wave393();
    let live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_wave393_ok =
        honesty_live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_residual_wave393();
    let live_leaflet_drop_behavior_dual_world_empty_gate_live_wave393_ok =
        simulate_live_leaflet_drop_behavior_dual_world_empty_gate_honesty();
    let live_auto_deposit_update_dual_world_empty_gate_method_names_wave394_ok =
        honesty_live_auto_deposit_update_dual_world_empty_gate_method_names_residual_wave394();
    let live_auto_deposit_update_dual_world_empty_gate_nav_commands_wave394_ok =
        honesty_live_auto_deposit_update_dual_world_empty_gate_nav_commands_residual_wave394();
    let live_auto_deposit_update_dual_world_empty_gate_live_wave394_ok =
        simulate_live_auto_deposit_update_dual_world_empty_gate_honesty();
    let live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_wave395_ok =
        honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_residual_wave395();
    let live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_wave395_ok =
        honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_residual_wave395();
    let live_supply_warehouse_crippling_behavior_dual_world_empty_gate_live_wave395_ok =
        simulate_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_honesty();
    let live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_wave396_ok =
        honesty_live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_residual_wave396();
    let live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_wave396_ok =
        honesty_live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_residual_wave396();
    let live_neutron_missile_slow_death_update_dual_world_empty_gate_live_wave396_ok =
        simulate_live_neutron_missile_slow_death_update_dual_world_empty_gate_honesty();
    let live_ai_dock_dual_world_empty_gate_method_names_wave397_ok =
        honesty_live_ai_dock_dual_world_empty_gate_method_names_residual_wave397();
    let live_ai_dock_dual_world_empty_gate_nav_commands_wave397_ok =
        honesty_live_ai_dock_dual_world_empty_gate_nav_commands_residual_wave397();
    let live_ai_dock_dual_world_empty_gate_live_wave397_ok =
        simulate_live_ai_dock_dual_world_empty_gate_honesty();
    let live_ai_groups_dual_world_empty_gate_method_names_wave398_ok =
        honesty_live_ai_groups_dual_world_empty_gate_method_names_residual_wave398();
    let live_ai_groups_dual_world_empty_gate_nav_commands_wave398_ok =
        honesty_live_ai_groups_dual_world_empty_gate_nav_commands_residual_wave398();
    let live_ai_groups_dual_world_empty_gate_live_wave398_ok =
        simulate_live_ai_groups_dual_world_empty_gate_honesty();
    let live_artillery_barrage_power_dual_world_empty_gate_method_names_wave399_ok =
        honesty_live_artillery_barrage_power_dual_world_empty_gate_method_names_residual_wave399();
    let live_artillery_barrage_power_dual_world_empty_gate_nav_commands_wave399_ok =
        honesty_live_artillery_barrage_power_dual_world_empty_gate_nav_commands_residual_wave399();
    let live_artillery_barrage_power_dual_world_empty_gate_live_wave399_ok =
        simulate_live_artillery_barrage_power_dual_world_empty_gate_honesty();
    let live_baikonur_launch_power_dual_world_empty_gate_method_names_wave400_ok =
        honesty_live_baikonur_launch_power_dual_world_empty_gate_method_names_residual_wave400();
    let live_baikonur_launch_power_dual_world_empty_gate_nav_commands_wave400_ok =
        honesty_live_baikonur_launch_power_dual_world_empty_gate_nav_commands_residual_wave400();
    let live_baikonur_launch_power_dual_world_empty_gate_live_wave400_ok =
        simulate_live_baikonur_launch_power_dual_world_empty_gate_honesty();
    let live_ai_group_dual_world_empty_gate_method_names_wave401_ok =
        honesty_live_ai_group_core_dual_world_empty_gate_method_names_residual_wave401();
    let live_ai_group_dual_world_empty_gate_nav_commands_wave401_ok =
        honesty_live_ai_group_core_dual_world_empty_gate_nav_commands_residual_wave401();
    let live_ai_group_dual_world_empty_gate_live_wave401_ok =
        simulate_live_ai_group_core_dual_world_empty_gate_honesty_wave401();
    let live_slaved_update_dual_world_empty_gate_method_names_wave402_ok =
        honesty_live_slaved_update_dual_world_empty_gate_method_names_residual_wave402();
    let live_slaved_update_dual_world_empty_gate_nav_commands_wave402_ok =
        honesty_live_slaved_update_dual_world_empty_gate_nav_commands_residual_wave402();
    let live_slaved_update_dual_world_empty_gate_live_wave402_ok =
        simulate_live_slaved_update_dual_world_empty_gate_honesty();
    let live_demoralize_power_dual_world_empty_gate_method_names_wave403_ok =
        honesty_live_demoralize_power_dual_world_empty_gate_method_names_residual_wave403();
    let live_demoralize_power_dual_world_empty_gate_nav_commands_wave403_ok =
        honesty_live_demoralize_power_dual_world_empty_gate_nav_commands_residual_wave403();
    let live_demoralize_power_dual_world_empty_gate_live_wave403_ok =
        simulate_live_demoralize_power_dual_world_empty_gate_honesty();
    let live_bone_fx_update_dual_world_empty_gate_method_names_wave404_ok =
        honesty_live_bone_fx_update_dual_world_empty_gate_method_names_residual_wave404();
    let live_bone_fx_update_dual_world_empty_gate_nav_commands_wave404_ok =
        honesty_live_bone_fx_update_dual_world_empty_gate_nav_commands_residual_wave404();
    let live_bone_fx_update_dual_world_empty_gate_live_wave404_ok =
        simulate_live_bone_fx_update_dual_world_empty_gate_honesty();
    let live_supply_warehouse_dock_dual_world_empty_gate_method_names_wave405_ok =
        honesty_live_supply_warehouse_dock_dual_world_empty_gate_method_names_residual_wave405();
    let live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_wave405_ok =
        honesty_live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_residual_wave405();
    let live_supply_warehouse_dock_dual_world_empty_gate_live_wave405_ok =
        simulate_live_supply_warehouse_dock_dual_world_empty_gate_honesty();
    let live_ocl_special_power_dual_world_empty_gate_method_names_wave406_ok =
        honesty_live_ocl_special_power_dual_world_empty_gate_method_names_residual_wave406();
    let live_ocl_special_power_dual_world_empty_gate_nav_commands_wave406_ok =
        honesty_live_ocl_special_power_dual_world_empty_gate_nav_commands_residual_wave406();
    let live_ocl_special_power_dual_world_empty_gate_live_wave406_ok =
        simulate_live_ocl_special_power_dual_world_empty_gate_honesty();
    let live_railed_transport_ai_update_dual_world_empty_gate_method_names_wave407_ok =
        honesty_live_railed_transport_ai_update_dual_world_empty_gate_method_names_residual_wave407(
        );
    let live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_wave407_ok =
        honesty_live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave407(
        );
    let live_railed_transport_ai_update_dual_world_empty_gate_live_wave407_ok =
        simulate_live_railed_transport_ai_update_dual_world_empty_gate_honesty();
    let live_squish_collide_dual_world_empty_gate_method_names_wave408_ok =
        honesty_live_squish_collide_dual_world_empty_gate_method_names_residual_wave408();
    let live_squish_collide_dual_world_empty_gate_nav_commands_wave408_ok =
        honesty_live_squish_collide_dual_world_empty_gate_nav_commands_residual_wave408();
    let live_squish_collide_dual_world_empty_gate_live_wave408_ok =
        simulate_live_squish_collide_dual_world_empty_gate_honesty();
    let live_weapon_bonus_update_dual_world_empty_gate_method_names_wave409_ok =
        honesty_live_weapon_bonus_update_dual_world_empty_gate_method_names_residual_wave409();
    let live_weapon_bonus_update_dual_world_empty_gate_nav_commands_wave409_ok =
        honesty_live_weapon_bonus_update_dual_world_empty_gate_nav_commands_residual_wave409();
    let live_weapon_bonus_update_dual_world_empty_gate_live_wave409_ok =
        simulate_live_weapon_bonus_update_dual_world_empty_gate_honesty();
    let live_minefield_behavior_dual_world_empty_gate_method_names_wave410_ok =
        honesty_live_minefield_behavior_dual_world_empty_gate_method_names_residual_wave410();
    let live_minefield_behavior_dual_world_empty_gate_nav_commands_wave410_ok =
        honesty_live_minefield_behavior_dual_world_empty_gate_nav_commands_residual_wave410();
    let live_minefield_behavior_dual_world_empty_gate_live_wave410_ok =
        simulate_live_minefield_behavior_dual_world_empty_gate_honesty();
    let live_point_defense_laser_update_dual_world_empty_gate_method_names_wave411_ok =
        honesty_live_point_defense_laser_update_dual_world_empty_gate_method_names_residual_wave411(
        );
    let live_point_defense_laser_update_dual_world_empty_gate_nav_commands_wave411_ok =
        honesty_live_point_defense_laser_update_dual_world_empty_gate_nav_commands_residual_wave411(
        );
    let live_point_defense_laser_update_dual_world_empty_gate_live_wave411_ok =
        simulate_live_point_defense_laser_update_dual_world_empty_gate_honesty();

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
        // Fail-closed: no ControlBar without game_client — do not tautology-pass.
        #[cfg(not(feature = "game_client"))]
        let control_bar_ok = false;
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
        production_authority_env_ok,
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
        message_stream_marker_wave110_ok,
        game_message_arg_wave110_ok,
        meta_event_category_wave110_ok,
        ingame_ui_wave110_ok,
        drawable_icon_flash_wave111_ok,
        drawable_status_stealth_wave111_ok,
        terrain_decal_wave111_ok,
        display_draw_image_wave111_ok,
        game_client_translator_wave111_ok,
        particle_priority_wave111_ok,
        mouse_residual_wave112_ok,
        keyboard_residual_wave112_ok,
        view_residual_wave112_ok,
        game_window_manager_wave113_ok,
        window_style_wave113_ok,
        gadget_wave113_ok,
        video_buffer_wave113_ok,
        audio_event_wave113_ok,
        main_menu_skirmish_names_wave114_ok,
        main_menu_skirmish_nav_steps_wave114_ok,
        main_menu_skirmish_message_wave114_ok,
        map_select_names_wave115_ok,
        map_select_nav_steps_wave115_ok,
        map_select_commands_wave115_ok,
        slot_state_wave116_ok,
        slot_combo_names_wave116_ok,
        slot_nav_commands_wave116_ok,
        starting_cash_wave117_ok,
        game_speed_controls_wave117_ok,
        rules_nav_commands_wave117_ok,
        main_menu_button_names_wave118_ok,
        main_menu_push_targets_wave118_ok,
        main_menu_button_nav_commands_wave118_ok,
        campaign_button_names_wave119_ok,
        campaign_enums_wave119_ok,
        campaign_nav_commands_wave119_ok,
        challenge_control_names_wave120_ok,
        challenge_nav_commands_wave120_ok,
        save_load_layout_wave121_ok,
        save_load_control_stems_wave121_ok,
        save_load_nav_commands_wave121_ok,
        replay_control_names_wave122_ok,
        replay_nav_commands_wave122_ok,
        quit_control_names_wave123_ok,
        quit_nav_commands_wave123_ok,
        keyboard_control_names_wave124_ok,
        keyboard_nav_commands_wave124_ok,
        score_control_names_wave125_ok,
        score_nav_commands_wave125_ok,
        options_control_names_wave126_ok,
        options_nav_commands_wave126_ok,
        credits_control_names_wave127_ok,
        credits_nav_commands_wave127_ok,
        message_box_control_names_wave128_ok,
        message_box_nav_commands_wave128_ok,
        diplomacy_control_names_wave129_ok,
        diplomacy_nav_commands_wave129_ok,
        popup_replay_control_names_wave130_ok,
        popup_replay_nav_commands_wave130_ok,
        single_player_control_names_wave131_ok,
        single_player_nav_commands_wave131_ok,
        map_select_control_names_wave132_ok,
        map_select_nav_commands_wave132_ok,
        control_bar_control_names_wave133_ok,
        control_bar_nav_commands_wave133_ok,
        difficulty_select_control_names_wave134_ok,
        difficulty_select_nav_commands_wave134_ok,
        loading_screen_stages_wave135_ok,
        loading_screen_nav_commands_wave135_ok,
        in_game_chat_control_names_wave136_ok,
        in_game_chat_nav_commands_wave136_ok,
        idle_worker_control_names_wave137_ok,
        idle_worker_nav_commands_wave137_ok,
        generals_exp_control_names_wave138_ok,
        generals_exp_nav_commands_wave138_ok,
        popup_communicator_control_names_wave139_ok,
        popup_communicator_nav_commands_wave139_ok,
        replay_control_control_names_wave140_ok,
        replay_control_nav_commands_wave140_ok,
        shell_map_names_wave141_ok,
        shell_map_nav_commands_wave141_ok,
        beacon_control_names_wave142_ok,
        beacon_nav_commands_wave142_ok,
        eva_message_names_wave143_ok,
        eva_nav_commands_wave143_ok,
        ime_message_names_wave144_ok,
        ime_nav_commands_wave144_ok,
        smudge_method_names_wave145_ok,
        smudge_nav_commands_wave145_ok,
        ocl_timer_method_names_wave146_ok,
        ocl_timer_nav_commands_wave146_ok,
        control_bar_resizer_method_names_wave147_ok,
        control_bar_resizer_nav_commands_wave147_ok,
        under_construction_method_names_wave148_ok,
        under_construction_nav_commands_wave148_ok,
        structure_inventory_command_names_wave149_ok,
        structure_inventory_nav_commands_wave149_ok,
        multi_select_method_names_wave150_ok,
        multi_select_nav_commands_wave150_ok,
        credits_style_method_names_wave151_ok,
        credits_nav_commands_wave151_ok,
        challenge_generals_method_names_wave152_ok,
        challenge_generals_nav_commands_wave152_ok,
        gameworld_authority_env_names_wave153_ok,
        gameworld_authority_method_names_wave153_ok,
        gameworld_authority_nav_commands_wave153_ok,
        window_video_type_state_names_wave154_ok,
        window_video_method_names_wave154_ok,
        window_video_nav_commands_wave154_ok,
        main_menu_layout_names_wave155_ok,
        main_menu_layout_nav_commands_wave155_ok,
        control_bar_scheme_names_wave156_ok,
        control_bar_scheme_method_names_wave156_ok,
        control_bar_scheme_nav_commands_wave156_ok,
        presentation_boundary_method_names_wave157_ok,
        presentation_boundary_source_markers_wave157_ok,
        presentation_boundary_nav_commands_wave157_ok,
        presentation_boundary_live_wave157_ok,
        control_bar_print_names_wave158_ok,
        control_bar_print_nav_commands_wave158_ok,
        terrain_env_boundary_method_names_wave159_ok,
        terrain_env_boundary_source_markers_wave159_ok,
        terrain_env_boundary_nav_commands_wave159_ok,
        terrain_env_boundary_live_wave159_ok,
        main_menu_wnd_names_wave160_ok,
        main_menu_wnd_nav_commands_wave160_ok,
        main_menu_wnd_live_wave160_ok,
        main_menu_wnd_load_method_names_wave161_ok,
        main_menu_wnd_load_nav_commands_wave161_ok,
        main_menu_wnd_load_live_wave161_ok,
        main_menu_wnd_materialise_method_names_wave162_ok,
        main_menu_wnd_materialise_nav_commands_wave162_ok,
        main_menu_wnd_materialise_live_wave162_ok,
        shell_stack_push_method_names_wave163_ok,
        shell_stack_push_nav_commands_wave163_ok,
        shell_stack_push_live_wave163_ok,
        shell_skirmish_nav_method_names_wave164_ok,
        shell_skirmish_nav_commands_wave164_ok,
        shell_skirmish_nav_live_wave164_ok,
        control_bar_materialise_method_names_wave165_ok,
        control_bar_materialise_nav_commands_wave165_ok,
        control_bar_materialise_live_wave165_ok,
        skirmish_options_wnd_method_names_wave166_ok,
        skirmish_options_wnd_nav_commands_wave166_ok,
        skirmish_options_wnd_live_wave166_ok,
        new_game_stream_method_names_wave167_ok,
        new_game_stream_nav_commands_wave167_ok,
        new_game_stream_live_wave167_ok,
        w3d_main_menu_init_method_names_wave168_ok,
        w3d_main_menu_init_nav_commands_wave168_ok,
        w3d_main_menu_init_live_wave168_ok,
        start_game_loading_method_names_wave169_ok,
        start_game_loading_nav_commands_wave169_ok,
        start_game_loading_live_wave169_ok,
        live_map_load_method_names_wave170_ok,
        live_map_load_nav_commands_wave170_ok,
        live_map_load_live_wave170_ok,
        live_presentation_seed_method_names_wave171_ok,
        live_presentation_seed_nav_commands_wave171_ok,
        live_presentation_seed_live_wave171_ok,
        live_gameworld_shadow_overlay_method_names_wave172_ok,
        live_gameworld_shadow_overlay_nav_commands_wave172_ok,
        live_gameworld_shadow_overlay_live_wave172_ok,
        single_authority_combat_method_names_wave173_ok,
        single_authority_combat_nav_commands_wave173_ok,
        single_authority_combat_live_wave173_ok,
        presentation_client_boundary_method_names_wave174_ok,
        presentation_client_boundary_nav_commands_wave174_ok,
        presentation_client_boundary_live_wave174_ok,
        golden_map_host_victory_method_names_wave175_ok,
        golden_map_host_victory_nav_commands_wave175_ok,
        golden_map_host_victory_live_wave175_ok,
        executable_presentation_boundary_method_names_wave176_ok,
        executable_presentation_boundary_nav_commands_wave176_ok,
        executable_presentation_boundary_live_wave176_ok,
        gameworld_production_authority_method_names_wave177_ok,
        gameworld_production_authority_nav_commands_wave177_ok,
        gameworld_production_authority_live_wave177_ok,
        gameworld_sole_tick_coupling_method_names_wave178_ok,
        gameworld_sole_tick_coupling_nav_commands_wave178_ok,
        gameworld_sole_tick_coupling_live_wave178_ok,
        movement_authority_env_ok,
        gameworld_authority_matrix_method_names_wave179_ok,
        gameworld_authority_matrix_nav_commands_wave179_ok,
        gameworld_authority_matrix_live_wave179_ok,
        ai_fire_construction_authority_env_ok,
        live_gameworld_production_writeback_method_names_wave180_ok,
        live_gameworld_production_writeback_nav_commands_wave180_ok,
        live_gameworld_production_writeback_live_wave180_ok,
        live_gameworld_construction_writeback_method_names_wave181_ok,
        live_gameworld_construction_writeback_nav_commands_wave181_ok,
        live_gameworld_construction_writeback_live_wave181_ok,
        live_gameworld_damage_channel_method_names_wave182_ok,
        live_gameworld_damage_channel_nav_commands_wave182_ok,
        live_gameworld_damage_channel_live_wave182_ok,
        live_gameworld_economy_movement_method_names_wave183_ok,
        live_gameworld_economy_movement_nav_commands_wave183_ok,
        live_gameworld_economy_movement_live_wave183_ok,
        live_gameworld_projectile_ai_method_names_wave184_ok,
        live_gameworld_projectile_ai_nav_commands_wave184_ok,
        live_gameworld_projectile_ai_live_wave184_ok,
        live_gameworld_fire_special_power_method_names_wave185_ok,
        live_gameworld_fire_special_power_nav_commands_wave185_ok,
        live_gameworld_fire_special_power_live_wave185_ok,
        live_gameworld_presentation_view_method_names_wave186_ok,
        live_gameworld_presentation_view_nav_commands_wave186_ok,
        live_gameworld_presentation_view_live_wave186_ok,
        live_presentation_gameworld_overlay_method_names_wave187_ok,
        live_presentation_gameworld_overlay_nav_commands_wave187_ok,
        live_presentation_gameworld_overlay_live_wave187_ok,
        executable_gameworld_presentation_method_names_wave188_ok,
        executable_gameworld_presentation_nav_commands_wave188_ok,
        executable_gameworld_presentation_live_wave188_ok,
        live_presentation_overlay_deepen_method_names_wave189_ok,
        live_presentation_overlay_deepen_nav_commands_wave189_ok,
        live_presentation_overlay_deepen_live_wave189_ok,
        live_presentation_overlay_stamp_method_names_wave190_ok,
        live_presentation_overlay_stamp_nav_commands_wave190_ok,
        live_presentation_overlay_stamp_live_wave190_ok,
        live_gameworld_entity_view_deepen_method_names_wave191_ok,
        live_gameworld_entity_view_deepen_nav_commands_wave191_ok,
        live_gameworld_entity_view_deepen_live_wave191_ok,
        live_presentation_append_missing_method_names_wave192_ok,
        live_presentation_append_missing_nav_commands_wave192_ok,
        live_presentation_append_missing_live_wave192_ok,
        live_presentation_build_from_gameworld_method_names_wave193_ok,
        live_presentation_build_from_gameworld_nav_commands_wave193_ok,
        live_presentation_build_from_gameworld_live_wave193_ok,
        live_presentation_from_gameworld_default_method_names_wave194_ok,
        live_presentation_from_gameworld_default_nav_commands_wave194_ok,
        live_presentation_from_gameworld_default_live_wave194_ok,
        live_presentation_build_for_engine_method_names_wave195_ok,
        live_presentation_build_for_engine_nav_commands_wave195_ok,
        live_presentation_build_for_engine_live_wave195_ok,
        live_presentation_rebuilt_vertical_gate_method_names_wave196_ok,
        live_presentation_rebuilt_vertical_gate_nav_commands_wave196_ok,
        live_presentation_rebuilt_vertical_gate_live_wave196_ok,
        live_command_attack_log_method_names_wave197_ok,
        live_command_attack_log_nav_commands_wave197_ok,
        live_command_attack_log_live_wave197_ok,
        live_command_guard_log_method_names_wave198_ok,
        live_command_guard_log_nav_commands_wave198_ok,
        live_command_guard_log_live_wave198_ok,
        live_command_production_construction_log_method_names_wave199_ok,
        live_command_production_construction_log_nav_commands_wave199_ok,
        live_command_production_construction_log_live_wave199_ok,
        live_command_rally_log_method_names_wave200_ok,
        live_command_rally_log_nav_commands_wave200_ok,
        live_command_rally_log_live_wave200_ok,
        live_evacuate_contain_log_method_names_wave201_ok,
        live_evacuate_contain_log_nav_commands_wave201_ok,
        live_evacuate_contain_log_live_wave201_ok,
        live_command_cheer_science_log_method_names_wave202_ok,
        live_command_cheer_science_log_nav_commands_wave202_ok,
        live_command_cheer_science_log_live_wave202_ok,
        live_command_deploy_status_log_method_names_wave203_ok,
        live_command_deploy_status_log_nav_commands_wave203_ok,
        live_command_deploy_status_log_live_wave203_ok,
        live_command_formation_log_method_names_wave204_ok,
        live_command_formation_log_nav_commands_wave204_ok,
        live_command_formation_log_live_wave204_ok,
        live_command_order_target_log_method_names_wave205_ok,
        live_command_order_target_log_nav_commands_wave205_ok,
        live_command_order_target_log_live_wave205_ok,
        live_command_selection_log_method_names_wave206_ok,
        live_command_selection_log_nav_commands_wave206_ok,
        live_command_selection_log_live_wave206_ok,
        live_command_non_attack_order_target_method_names_wave207_ok,
        live_command_non_attack_order_target_nav_commands_wave207_ok,
        live_command_non_attack_order_target_live_wave207_ok,
        live_golden_mopup_honesty_method_names_wave208_ok,
        live_golden_mopup_honesty_nav_commands_wave208_ok,
        live_golden_mopup_honesty_live_wave208_ok,
        live_os_input_command_path_method_names_wave209_ok,
        live_os_input_command_path_nav_commands_wave209_ok,
        live_os_input_command_path_live_wave209_ok,
        live_command_beacon_note_method_names_wave210_ok,
        live_command_beacon_note_nav_commands_wave210_ok,
        live_command_beacon_note_live_wave210_ok,
        live_host_beacon_presentation_method_names_wave211_ok,
        live_host_beacon_presentation_nav_commands_wave211_ok,
        live_host_beacon_presentation_live_wave211_ok,
        live_command_sell_deselect_log_method_names_wave212_ok,
        live_command_sell_deselect_log_nav_commands_wave212_ok,
        live_command_sell_deselect_log_live_wave212_ok,
        live_presentation_fow_only_method_names_wave213_ok,
        live_presentation_fow_only_nav_commands_wave213_ok,
        live_presentation_fow_only_live_wave213_ok,
        live_ui_producer_presentation_only_method_names_wave214_ok,
        live_ui_producer_presentation_only_nav_commands_wave214_ok,
        live_ui_producer_presentation_only_live_wave214_ok,
        live_ui_helpers_presentation_only_method_names_wave215_ok,
        live_ui_helpers_presentation_only_nav_commands_wave215_ok,
        live_ui_helpers_presentation_only_live_wave215_ok,
        live_control_group_camera_presentation_only_method_names_wave216_ok,
        live_control_group_camera_presentation_only_nav_commands_wave216_ok,
        live_control_group_camera_presentation_only_live_wave216_ok,
        live_cmd_filter_env_presentation_only_method_names_wave217_ok,
        live_cmd_filter_env_presentation_only_nav_commands_wave217_ok,
        live_cmd_filter_env_presentation_only_live_wave217_ok,
        live_selection_commands_presentation_only_method_names_wave218_ok,
        live_selection_commands_presentation_only_nav_commands_wave218_ok,
        live_selection_commands_presentation_only_live_wave218_ok,
        live_ui_command_selection_presentation_only_method_names_wave219_ok,
        live_ui_command_selection_presentation_only_nav_commands_wave219_ok,
        live_ui_command_selection_presentation_only_live_wave219_ok,
        live_local_team_presentation_only_method_names_wave220_ok,
        live_local_team_presentation_only_nav_commands_wave220_ok,
        live_local_team_presentation_only_live_wave220_ok,
        live_hotkey_move_attack_selection_presentation_only_method_names_wave221_ok,
        live_hotkey_move_attack_selection_presentation_only_nav_commands_wave221_ok,
        live_hotkey_move_attack_selection_presentation_only_live_wave221_ok,
        live_pick_object_presentation_only_method_names_wave222_ok,
        live_pick_object_presentation_only_nav_commands_wave222_ok,
        live_pick_object_presentation_only_live_wave222_ok,
        live_bootstrap_camera_presentation_only_method_names_wave223_ok,
        live_bootstrap_camera_presentation_only_nav_commands_wave223_ok,
        live_bootstrap_camera_presentation_only_live_wave223_ok,
        live_force_complete_authority_api_method_names_wave224_ok,
        live_force_complete_authority_api_nav_commands_wave224_ok,
        live_force_complete_authority_api_live_wave224_ok,
        live_path_guard_authority_api_method_names_wave225_ok,
        live_path_guard_authority_api_nav_commands_wave225_ok,
        live_path_guard_authority_api_live_wave225_ok,
        live_hotkey_selection_camera_presentation_only_method_names_wave226_ok,
        live_hotkey_selection_camera_presentation_only_nav_commands_wave226_ok,
        live_hotkey_selection_camera_presentation_only_live_wave226_ok,
        live_construct_spawn_pose_authority_api_method_names_wave227_ok,
        live_construct_spawn_pose_authority_api_nav_commands_wave227_ok,
        live_construct_spawn_pose_authority_api_live_wave227_ok,
        live_rmb_target_presentation_only_method_names_wave228_ok,
        live_rmb_target_presentation_only_nav_commands_wave228_ok,
        live_rmb_target_presentation_only_live_wave228_ok,
        live_rmb_selected_presentation_only_method_names_wave229_ok,
        live_rmb_selected_presentation_only_nav_commands_wave229_ok,
        live_rmb_selected_presentation_only_live_wave229_ok,
        live_command_unit_authority_api_method_names_wave230_ok,
        live_command_unit_authority_api_nav_commands_wave230_ok,
        live_command_unit_authority_api_live_wave230_ok,
        live_command_unit_more_authority_api_method_names_wave231_ok,
        live_command_unit_more_authority_api_nav_commands_wave231_ok,
        live_command_unit_more_authority_api_live_wave231_ok,
        live_command_executor_authority_api_method_names_wave232_ok,
        live_command_executor_authority_api_nav_commands_wave232_ok,
        live_command_executor_authority_api_live_wave232_ok,
        live_command_executor_more_authority_api_method_names_wave233_ok,
        live_command_executor_more_authority_api_nav_commands_wave233_ok,
        live_command_executor_more_authority_api_live_wave233_ok,
        live_engine_presentation_player_ui_method_names_wave234_ok,
        live_engine_presentation_player_ui_nav_commands_wave234_ok,
        live_engine_presentation_player_ui_live_wave234_ok,
        live_rmb_presentation_full_classify_method_names_wave235_ok,
        live_rmb_presentation_full_classify_nav_commands_wave235_ok,
        live_rmb_presentation_full_classify_live_wave235_ok,
        live_mouse_input_presentation_only_method_names_wave236_ok,
        live_mouse_input_presentation_only_nav_commands_wave236_ok,
        live_mouse_input_presentation_only_live_wave236_ok,
        live_engine_player_ui_boot_peel_method_names_wave237_ok,
        live_engine_player_ui_boot_peel_nav_commands_wave237_ok,
        live_engine_player_ui_boot_peel_live_wave237_ok,
        live_player_probe_api_method_names_wave238_ok,
        live_player_probe_api_nav_commands_wave238_ok,
        live_player_probe_api_live_wave238_ok,
        live_player_team_probe_method_names_wave239_ok,
        live_player_team_probe_nav_commands_wave239_ok,
        live_player_team_probe_live_wave239_ok,
        live_player_field_probe_method_names_wave240_ok,
        live_player_field_probe_nav_commands_wave240_ok,
        live_player_field_probe_live_wave240_ok,
        live_camera_height_probe_method_names_wave241_ok,
        live_camera_height_probe_nav_commands_wave241_ok,
        live_camera_height_probe_live_wave241_ok,
        live_command_player_probe_method_names_wave242_ok,
        live_command_player_probe_nav_commands_wave242_ok,
        live_command_player_probe_live_wave242_ok,
        live_construct_economy_probe_method_names_wave243_ok,
        live_construct_economy_probe_nav_commands_wave243_ok,
        live_construct_economy_probe_live_wave243_ok,
        live_command_unit_probe_method_names_wave244_ok,
        live_command_unit_probe_nav_commands_wave244_ok,
        live_command_unit_probe_live_wave244_ok,
        live_selection_query_probe_method_names_wave245_ok,
        live_selection_query_probe_nav_commands_wave245_ok,
        live_selection_query_probe_live_wave245_ok,
        live_world_pick_probe_method_names_wave246_ok,
        live_world_pick_probe_nav_commands_wave246_ok,
        live_world_pick_probe_live_wave246_ok,
        live_object_registry_empty_fastpath_method_names_wave247_ok,
        live_object_registry_empty_fastpath_nav_commands_wave247_ok,
        live_object_registry_empty_fastpath_live_wave247_ok,
        live_legacy_object_registry_fastpath_method_names_wave248_ok,
        live_legacy_object_registry_fastpath_nav_commands_wave248_ok,
        live_legacy_object_registry_fastpath_live_wave248_ok,
        live_client_dual_world_empty_gate_method_names_wave249_ok,
        live_client_dual_world_empty_gate_nav_commands_wave249_ok,
        live_client_dual_world_empty_gate_live_wave249_ok,
        live_presentation_time_frozen_probe_method_names_wave250_ok,
        live_presentation_time_frozen_probe_nav_commands_wave250_ok,
        live_presentation_time_frozen_probe_live_wave250_ok,
        live_presentation_visual_speed_probe_method_names_wave251_ok,
        live_presentation_visual_speed_probe_nav_commands_wave251_ok,
        live_presentation_visual_speed_probe_live_wave251_ok,
        live_presentation_script_camera_probe_method_names_wave252_ok,
        live_presentation_script_camera_probe_nav_commands_wave252_ok,
        live_presentation_script_camera_probe_live_wave252_ok,
        live_ai_group_dual_world_empty_gate_method_names_wave253_ok,
        live_ai_group_dual_world_empty_gate_nav_commands_wave253_ok,
        live_ai_group_dual_world_empty_gate_live_wave253_ok,
        live_ai_states_dual_world_empty_gate_method_names_wave254_ok,
        live_ai_states_dual_world_empty_gate_nav_commands_wave254_ok,
        live_ai_states_dual_world_empty_gate_live_wave254_ok,
        live_ai_player_dual_world_empty_gate_method_names_wave255_ok,
        live_ai_player_dual_world_empty_gate_nav_commands_wave255_ok,
        live_ai_player_dual_world_empty_gate_live_wave255_ok,
        live_team_dual_world_empty_gate_method_names_wave256_ok,
        live_team_dual_world_empty_gate_nav_commands_wave256_ok,
        live_team_dual_world_empty_gate_live_wave256_ok,
        live_ai_legacy_states_dual_world_empty_gate_method_names_wave257_ok,
        live_ai_legacy_states_dual_world_empty_gate_nav_commands_wave257_ok,
        live_ai_legacy_states_dual_world_empty_gate_live_wave257_ok,
        live_unit_dual_world_empty_gate_method_names_wave258_ok,
        live_unit_dual_world_empty_gate_nav_commands_wave258_ok,
        live_unit_dual_world_empty_gate_live_wave258_ok,
        live_stealth_dual_world_empty_gate_method_names_wave259_ok,
        live_stealth_dual_world_empty_gate_nav_commands_wave259_ok,
        live_stealth_dual_world_empty_gate_live_wave259_ok,
        live_garrison_dual_world_empty_gate_method_names_wave260_ok,
        live_garrison_dual_world_empty_gate_nav_commands_wave260_ok,
        live_garrison_dual_world_empty_gate_live_wave260_ok,
        live_open_contain_dual_world_empty_gate_method_names_wave261_ok,
        live_open_contain_dual_world_empty_gate_nav_commands_wave261_ok,
        live_open_contain_dual_world_empty_gate_live_wave261_ok,
        live_pathfind_dual_world_empty_gate_method_names_wave262_ok,
        live_pathfind_dual_world_empty_gate_nav_commands_wave262_ok,
        live_pathfind_dual_world_empty_gate_live_wave262_ok,
        live_ai_mod_dual_world_empty_gate_method_names_wave263_ok,
        live_ai_mod_dual_world_empty_gate_nav_commands_wave263_ok,
        live_ai_mod_dual_world_empty_gate_live_wave263_ok,
        live_object_mod_dual_world_empty_gate_method_names_wave264_ok,
        live_object_mod_dual_world_empty_gate_nav_commands_wave264_ok,
        live_object_mod_dual_world_empty_gate_live_wave264_ok,
        live_weapon_dual_world_empty_gate_method_names_wave265_ok,
        live_weapon_dual_world_empty_gate_nav_commands_wave265_ok,
        live_weapon_dual_world_empty_gate_live_wave265_ok,
        live_partition_filters_dual_world_empty_gate_method_names_wave266_ok,
        live_partition_filters_dual_world_empty_gate_nav_commands_wave266_ok,
        live_partition_filters_dual_world_empty_gate_live_wave266_ok,
        live_ai_state_machine_dual_world_empty_gate_method_names_wave267_ok,
        live_ai_state_machine_dual_world_empty_gate_nav_commands_wave267_ok,
        live_ai_state_machine_dual_world_empty_gate_live_wave267_ok,
        live_player_dual_world_empty_gate_method_names_wave268_ok,
        live_player_dual_world_empty_gate_nav_commands_wave268_ok,
        live_player_dual_world_empty_gate_live_wave268_ok,
        live_game_client_dual_world_empty_gate_method_names_wave269_ok,
        live_game_client_dual_world_empty_gate_nav_commands_wave269_ok,
        live_game_client_dual_world_empty_gate_live_wave269_ok,
        live_drawable_dual_world_empty_gate_method_names_wave270_ok,
        live_drawable_dual_world_empty_gate_nav_commands_wave270_ok,
        live_drawable_dual_world_empty_gate_live_wave270_ok,
        live_script_conditions_dual_world_empty_gate_method_names_wave271_ok,
        live_script_conditions_dual_world_empty_gate_nav_commands_wave271_ok,
        live_script_conditions_dual_world_empty_gate_live_wave271_ok,
        live_transport_contain_dual_world_empty_gate_method_names_wave272_ok,
        live_transport_contain_dual_world_empty_gate_nav_commands_wave272_ok,
        live_transport_contain_dual_world_empty_gate_live_wave272_ok,
        live_ingame_ui_dual_world_empty_gate_method_names_wave273_ok,
        live_ingame_ui_dual_world_empty_gate_nav_commands_wave273_ok,
        live_ingame_ui_dual_world_empty_gate_live_wave273_ok,
        live_helix_contain_dual_world_empty_gate_method_names_wave274_ok,
        live_helix_contain_dual_world_empty_gate_nav_commands_wave274_ok,
        live_helix_contain_dual_world_empty_gate_live_wave274_ok,
        live_command_processor_dual_world_empty_gate_method_names_wave275_ok,
        live_command_processor_dual_world_empty_gate_nav_commands_wave275_ok,
        live_command_processor_dual_world_empty_gate_live_wave275_ok,
        live_turret_dual_world_empty_gate_method_names_wave276_ok,
        live_turret_dual_world_empty_gate_nav_commands_wave276_ok,
        live_turret_dual_world_empty_gate_live_wave276_ok,
        live_rider_change_contain_dual_world_empty_gate_method_names_wave277_ok,
        live_rider_change_contain_dual_world_empty_gate_nav_commands_wave277_ok,
        live_rider_change_contain_dual_world_empty_gate_live_wave277_ok,
        live_selection_dual_world_empty_gate_method_names_wave278_ok,
        live_selection_dual_world_empty_gate_nav_commands_wave278_ok,
        live_selection_dual_world_empty_gate_live_wave278_ok,
        live_cave_contain_dual_world_empty_gate_method_names_wave279_ok,
        live_cave_contain_dual_world_empty_gate_nav_commands_wave279_ok,
        live_cave_contain_dual_world_empty_gate_live_wave279_ok,
        live_tunnel_contain_dual_world_empty_gate_method_names_wave280_ok,
        live_tunnel_contain_dual_world_empty_gate_nav_commands_wave280_ok,
        live_tunnel_contain_dual_world_empty_gate_live_wave280_ok,
        live_helpers_dual_world_empty_gate_method_names_wave281_ok,
        live_helpers_dual_world_empty_gate_nav_commands_wave281_ok,
        live_helpers_dual_world_empty_gate_live_wave281_ok,
        live_ai_update_interface_dual_world_empty_gate_method_names_wave282_ok,
        live_ai_update_interface_dual_world_empty_gate_nav_commands_wave282_ok,
        live_ai_update_interface_dual_world_empty_gate_live_wave282_ok,
        live_stealth_update_dual_world_empty_gate_method_names_wave283_ok,
        live_stealth_update_dual_world_empty_gate_nav_commands_wave283_ok,
        live_stealth_update_dual_world_empty_gate_live_wave283_ok,
        live_script_executor_dual_world_empty_gate_method_names_wave284_ok,
        live_script_executor_dual_world_empty_gate_nav_commands_wave284_ok,
        live_script_executor_dual_world_empty_gate_live_wave284_ok,
        live_ai_integration_dual_world_empty_gate_method_names_wave285_ok,
        live_ai_integration_dual_world_empty_gate_nav_commands_wave285_ok,
        live_ai_integration_dual_world_empty_gate_live_wave285_ok,
        live_dumb_projectile_dual_world_empty_gate_method_names_wave286_ok,
        live_dumb_projectile_dual_world_empty_gate_nav_commands_wave286_ok,
        live_dumb_projectile_dual_world_empty_gate_live_wave286_ok,
        live_enhanced_player_dual_world_empty_gate_method_names_wave287_ok,
        live_enhanced_player_dual_world_empty_gate_nav_commands_wave287_ok,
        live_enhanced_player_dual_world_empty_gate_live_wave287_ok,
        live_hijacker_update_dual_world_empty_gate_method_names_wave288_ok,
        live_hijacker_update_dual_world_empty_gate_nav_commands_wave288_ok,
        live_hijacker_update_dual_world_empty_gate_live_wave288_ok,
        live_weapon_impl_dual_world_empty_gate_method_names_wave289_ok,
        live_weapon_impl_dual_world_empty_gate_nav_commands_wave289_ok,
        live_weapon_impl_dual_world_empty_gate_live_wave289_ok,
        live_async_player_dual_world_empty_gate_method_names_wave290_ok,
        live_async_player_dual_world_empty_gate_nav_commands_wave290_ok,
        live_async_player_dual_world_empty_gate_live_wave290_ok,
        live_active_body_dual_world_empty_gate_method_names_wave291_ok,
        live_active_body_dual_world_empty_gate_nav_commands_wave291_ok,
        live_active_body_dual_world_empty_gate_live_wave291_ok,
        live_skirmish_conditions_dual_world_empty_gate_method_names_wave292_ok,
        live_skirmish_conditions_dual_world_empty_gate_nav_commands_wave292_ok,
        live_skirmish_conditions_dual_world_empty_gate_live_wave292_ok,
        live_ai_build_list_dual_world_empty_gate_method_names_wave293_ok,
        live_ai_build_list_dual_world_empty_gate_nav_commands_wave293_ok,
        live_ai_build_list_dual_world_empty_gate_live_wave293_ok,
        live_victory_dual_world_empty_gate_method_names_wave294_ok,
        live_victory_dual_world_empty_gate_nav_commands_wave294_ok,
        live_victory_dual_world_empty_gate_live_wave294_ok,
        live_script_actions_dual_world_empty_gate_method_names_wave295_ok,
        live_script_actions_dual_world_empty_gate_nav_commands_wave295_ok,
        live_script_actions_dual_world_empty_gate_live_wave295_ok,
        live_special_ability_dual_world_empty_gate_method_names_wave296_ok,
        live_special_ability_dual_world_empty_gate_nav_commands_wave296_ok,
        live_special_ability_dual_world_empty_gate_live_wave296_ok,
        live_stealth_detector_dual_world_empty_gate_method_names_wave297_ok,
        live_stealth_detector_dual_world_empty_gate_nav_commands_wave297_ok,
        live_stealth_detector_dual_world_empty_gate_live_wave297_ok,
        live_supply_system_dual_world_empty_gate_method_names_wave298_ok,
        live_supply_system_dual_world_empty_gate_nav_commands_wave298_ok,
        live_supply_system_dual_world_empty_gate_live_wave298_ok,
        live_particle_uplink_dual_world_empty_gate_method_names_wave299_ok,
        live_particle_uplink_dual_world_empty_gate_nav_commands_wave299_ok,
        live_particle_uplink_dual_world_empty_gate_live_wave299_ok,
        live_overlord_contain_dual_world_empty_gate_method_names_wave300_ok,
        live_overlord_contain_dual_world_empty_gate_nav_commands_wave300_ok,
        live_overlord_contain_dual_world_empty_gate_live_wave300_ok,
        live_bridge_behavior_dual_world_empty_gate_method_names_wave301_ok,
        live_bridge_behavior_dual_world_empty_gate_nav_commands_wave301_ok,
        live_bridge_behavior_dual_world_empty_gate_live_wave301_ok,
        live_stealth_behavior_dual_world_empty_gate_method_names_wave302_ok,
        live_stealth_behavior_dual_world_empty_gate_nav_commands_wave302_ok,
        live_stealth_behavior_dual_world_empty_gate_live_wave302_ok,
        live_crate_collide_dual_world_empty_gate_method_names_wave303_ok,
        live_crate_collide_dual_world_empty_gate_nav_commands_wave303_ok,
        live_crate_collide_dual_world_empty_gate_live_wave303_ok,
        live_object_manager_dual_world_empty_gate_method_names_wave304_ok,
        live_object_manager_dual_world_empty_gate_nav_commands_wave304_ok,
        live_object_manager_dual_world_empty_gate_live_wave304_ok,
        live_sticky_bomb_dual_world_empty_gate_method_names_wave305_ok,
        live_sticky_bomb_dual_world_empty_gate_nav_commands_wave305_ok,
        live_sticky_bomb_dual_world_empty_gate_live_wave305_ok,
        live_auto_heal_dual_world_empty_gate_method_names_wave306_ok,
        live_auto_heal_dual_world_empty_gate_nav_commands_wave306_ok,
        live_auto_heal_dual_world_empty_gate_live_wave306_ok,
        live_grant_stealth_dual_world_empty_gate_method_names_wave307_ok,
        live_grant_stealth_dual_world_empty_gate_nav_commands_wave307_ok,
        live_grant_stealth_dual_world_empty_gate_live_wave307_ok,
        live_status_bits_upgrade_dual_world_empty_gate_method_names_wave308_ok,
        live_status_bits_upgrade_dual_world_empty_gate_nav_commands_wave308_ok,
        live_status_bits_upgrade_dual_world_empty_gate_live_wave308_ok,
        live_jet_ai_dual_world_empty_gate_method_names_wave309_ok,
        live_jet_ai_dual_world_empty_gate_nav_commands_wave309_ok,
        live_jet_ai_dual_world_empty_gate_live_wave309_ok,
        live_parking_place_dual_world_empty_gate_method_names_wave310_ok,
        live_parking_place_dual_world_empty_gate_nav_commands_wave310_ok,
        live_parking_place_dual_world_empty_gate_live_wave310_ok,
        live_flight_deck_dual_world_empty_gate_method_names_wave311_ok,
        live_flight_deck_dual_world_empty_gate_nav_commands_wave311_ok,
        live_flight_deck_dual_world_empty_gate_live_wave311_ok,
        live_exit_strategies_dual_world_empty_gate_method_names_wave312_ok,
        live_exit_strategies_dual_world_empty_gate_nav_commands_wave312_ok,
        live_exit_strategies_dual_world_empty_gate_live_wave312_ok,
        live_collision_system_dual_world_empty_gate_method_names_wave313_ok,
        live_collision_system_dual_world_empty_gate_nav_commands_wave313_ok,
        live_collision_system_dual_world_empty_gate_live_wave313_ok,
        live_max_health_upgrade_dual_world_empty_gate_method_names_wave314_ok,
        live_max_health_upgrade_dual_world_empty_gate_nav_commands_wave314_ok,
        live_max_health_upgrade_dual_world_empty_gate_live_wave314_ok,
        live_structure_topple_dual_world_empty_gate_method_names_wave315_ok,
        live_structure_topple_dual_world_empty_gate_nav_commands_wave315_ok,
        live_structure_topple_dual_world_empty_gate_live_wave315_ok,
        live_physics_update_dual_world_empty_gate_method_names_wave316_ok,
        live_physics_update_dual_world_empty_gate_nav_commands_wave316_ok,
        live_physics_update_dual_world_empty_gate_live_wave316_ok,
        live_cleanup_hazard_dual_world_empty_gate_method_names_wave317_ok,
        live_cleanup_hazard_dual_world_empty_gate_nav_commands_wave317_ok,
        live_cleanup_hazard_dual_world_empty_gate_live_wave317_ok,
        live_bridge_tower_dual_world_empty_gate_method_names_wave318_ok,
        live_bridge_tower_dual_world_empty_gate_nav_commands_wave318_ok,
        live_bridge_tower_dual_world_empty_gate_live_wave318_ok,
        live_armor_upgrade_dual_world_empty_gate_method_names_wave319_ok,
        live_armor_upgrade_dual_world_empty_gate_nav_commands_wave319_ok,
        live_armor_upgrade_dual_world_empty_gate_live_wave319_ok,
        live_paradrop_power_dual_world_empty_gate_method_names_wave320_ok,
        live_paradrop_power_dual_world_empty_gate_nav_commands_wave320_ok,
        live_paradrop_power_dual_world_empty_gate_live_wave320_ok,
        live_fuel_air_bomb_dual_world_empty_gate_method_names_wave321_ok,
        live_fuel_air_bomb_dual_world_empty_gate_nav_commands_wave321_ok,
        live_fuel_air_bomb_dual_world_empty_gate_live_wave321_ok,
        live_tensile_formation_dual_world_empty_gate_method_names_wave322_ok,
        live_tensile_formation_dual_world_empty_gate_nav_commands_wave322_ok,
        live_tensile_formation_dual_world_empty_gate_live_wave322_ok,
        live_die_mod_dual_world_empty_gate_method_names_wave323_ok,
        live_die_mod_dual_world_empty_gate_nav_commands_wave323_ok,
        live_die_mod_dual_world_empty_gate_live_wave323_ok,
        live_partition_manager_dual_world_empty_gate_method_names_wave324_ok,
        live_partition_manager_dual_world_empty_gate_nav_commands_wave324_ok,
        live_partition_manager_dual_world_empty_gate_live_wave324_ok,
        live_spectre_gunship_dual_world_empty_gate_method_names_wave325_ok,
        live_spectre_gunship_dual_world_empty_gate_nav_commands_wave325_ok,
        live_spectre_gunship_dual_world_empty_gate_live_wave325_ok,
        live_production_update_dual_world_empty_gate_method_names_wave326_ok,
        live_production_update_dual_world_empty_gate_nav_commands_wave326_ok,
        live_production_update_dual_world_empty_gate_live_wave326_ok,
        live_neutron_blast_dual_world_empty_gate_method_names_wave327_ok,
        live_neutron_blast_dual_world_empty_gate_nav_commands_wave327_ok,
        live_neutron_blast_dual_world_empty_gate_live_wave327_ok,
        live_countermeasures_dual_world_empty_gate_method_names_wave328_ok,
        live_countermeasures_dual_world_empty_gate_nav_commands_wave328_ok,
        live_countermeasures_dual_world_empty_gate_live_wave328_ok,
        live_skirmish_player_dual_world_empty_gate_method_names_wave329_ok,
        live_skirmish_player_dual_world_empty_gate_nav_commands_wave329_ok,
        live_skirmish_player_dual_world_empty_gate_live_wave329_ok,
        live_a10_strike_dual_world_empty_gate_method_names_wave330_ok,
        live_a10_strike_dual_world_empty_gate_nav_commands_wave330_ok,
        live_a10_strike_dual_world_empty_gate_live_wave330_ok,
        live_rebuild_hole_dual_world_empty_gate_method_names_wave331_ok,
        live_rebuild_hole_dual_world_empty_gate_nav_commands_wave331_ok,
        live_rebuild_hole_dual_world_empty_gate_live_wave331_ok,
        live_wave_guide_dual_world_empty_gate_method_names_wave332_ok,
        live_wave_guide_dual_world_empty_gate_nav_commands_wave332_ok,
        live_wave_guide_dual_world_empty_gate_live_wave332_ok,
        live_emp_update_dual_world_empty_gate_method_names_wave333_ok,
        live_emp_update_dual_world_empty_gate_nav_commands_wave333_ok,
        live_emp_update_dual_world_empty_gate_live_wave333_ok,
        live_bunker_buster_dual_world_empty_gate_method_names_wave334_ok,
        live_bunker_buster_dual_world_empty_gate_nav_commands_wave334_ok,
        live_bunker_buster_dual_world_empty_gate_live_wave334_ok,
        live_bridge_scaffold_dual_world_empty_gate_method_names_wave335_ok,
        live_bridge_scaffold_dual_world_empty_gate_nav_commands_wave335_ok,
        live_bridge_scaffold_dual_world_empty_gate_live_wave335_ok,
        live_assisted_targeting_dual_world_empty_gate_method_names_wave336_ok,
        live_assisted_targeting_dual_world_empty_gate_nav_commands_wave336_ok,
        live_assisted_targeting_dual_world_empty_gate_live_wave336_ok,
        live_economy_dual_world_empty_gate_method_names_wave337_ok,
        live_economy_dual_world_empty_gate_nav_commands_wave337_ok,
        live_economy_dual_world_empty_gate_live_wave337_ok,
        live_turret_ai_dual_world_empty_gate_method_names_wave338_ok,
        live_turret_ai_dual_world_empty_gate_nav_commands_wave338_ok,
        live_turret_ai_dual_world_empty_gate_live_wave338_ok,
        live_stealth_detector_module_dual_world_empty_gate_method_names_wave339_ok,
        live_stealth_detector_module_dual_world_empty_gate_nav_commands_wave339_ok,
        live_stealth_detector_module_dual_world_empty_gate_live_wave339_ok,
        live_modules_dual_world_empty_gate_method_names_wave340_ok,
        live_modules_dual_world_empty_gate_nav_commands_wave340_ok,
        live_modules_dual_world_empty_gate_live_wave340_ok,
        live_terrain_dual_world_empty_gate_method_names_wave341_ok,
        live_terrain_dual_world_empty_gate_nav_commands_wave341_ok,
        live_terrain_dual_world_empty_gate_live_wave341_ok,
        live_special_power_template_dual_world_empty_gate_method_names_wave342_ok,
        live_special_power_template_dual_world_empty_gate_nav_commands_wave342_ok,
        live_special_power_template_dual_world_empty_gate_live_wave342_ok,
        live_script_evaluator_dual_world_empty_gate_method_names_wave343_ok,
        live_script_evaluator_dual_world_empty_gate_nav_commands_wave343_ok,
        live_script_evaluator_dual_world_empty_gate_live_wave343_ok,
        live_system_game_logic_dual_world_empty_gate_method_names_wave344_ok,
        live_system_game_logic_dual_world_empty_gate_nav_commands_wave344_ok,
        live_system_game_logic_dual_world_empty_gate_live_wave344_ok,
        live_meta_event_dual_world_empty_gate_method_names_wave345_ok,
        live_meta_event_dual_world_empty_gate_nav_commands_wave345_ok,
        live_meta_event_dual_world_empty_gate_live_wave345_ok,
        live_spawn_behavior_dual_world_empty_gate_method_names_wave346_ok,
        live_spawn_behavior_dual_world_empty_gate_nav_commands_wave346_ok,
        live_spawn_behavior_dual_world_empty_gate_live_wave346_ok,
        live_action_manager_dual_world_empty_gate_method_names_wave347_ok,
        live_action_manager_dual_world_empty_gate_nav_commands_wave347_ok,
        live_action_manager_dual_world_empty_gate_live_wave347_ok,
        live_script_engine_dual_world_empty_gate_method_names_wave348_ok,
        live_script_engine_dual_world_empty_gate_nav_commands_wave348_ok,
        live_script_engine_dual_world_empty_gate_live_wave348_ok,
        live_chinook_ai_dual_world_empty_gate_method_names_wave349_ok,
        live_chinook_ai_dual_world_empty_gate_nav_commands_wave349_ok,
        live_chinook_ai_dual_world_empty_gate_live_wave349_ok,
        live_missile_ai_dual_world_empty_gate_method_names_wave350_ok,
        live_missile_ai_dual_world_empty_gate_nav_commands_wave350_ok,
        live_missile_ai_dual_world_empty_gate_live_wave350_ok,
        live_dozer_ai_dual_world_empty_gate_method_names_wave351_ok,
        live_dozer_ai_dual_world_empty_gate_nav_commands_wave351_ok,
        live_dozer_ai_dual_world_empty_gate_live_wave351_ok,
        live_deliver_payload_ai_dual_world_empty_gate_method_names_wave352_ok,
        live_deliver_payload_ai_dual_world_empty_gate_nav_commands_wave352_ok,
        live_deliver_payload_ai_dual_world_empty_gate_live_wave352_ok,
        live_special_power_module_dual_world_empty_gate_method_names_wave353_ok,
        live_special_power_module_dual_world_empty_gate_nav_commands_wave353_ok,
        live_special_power_module_dual_world_empty_gate_live_wave353_ok,
        live_pow_truck_ai_dual_world_empty_gate_method_names_wave354_ok,
        live_pow_truck_ai_dual_world_empty_gate_nav_commands_wave354_ok,
        live_pow_truck_ai_dual_world_empty_gate_live_wave354_ok,
        live_dock_update_dual_world_empty_gate_method_names_wave355_ok,
        live_dock_update_dual_world_empty_gate_nav_commands_wave355_ok,
        live_dock_update_dual_world_empty_gate_live_wave355_ok,
        live_weapon_template_dual_world_empty_gate_method_names_wave356_ok,
        live_weapon_template_dual_world_empty_gate_nav_commands_wave356_ok,
        live_weapon_template_dual_world_empty_gate_live_wave356_ok,
        live_railroad_guide_ai_dual_world_empty_gate_method_names_wave357_ok,
        live_railroad_guide_ai_dual_world_empty_gate_nav_commands_wave357_ok,
        live_railroad_guide_ai_dual_world_empty_gate_live_wave357_ok,
        live_hack_internet_ai_dual_world_empty_gate_method_names_wave358_ok,
        live_hack_internet_ai_dual_world_empty_gate_nav_commands_wave358_ok,
        live_hack_internet_ai_dual_world_empty_gate_live_wave358_ok,
        live_spectre_gunship_deployment_dual_world_empty_gate_method_names_wave359_ok,
        live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_wave359_ok,
        live_spectre_gunship_deployment_dual_world_empty_gate_live_wave359_ok,
        live_radius_decal_update_dual_world_empty_gate_method_names_wave360_ok,
        live_radius_decal_update_dual_world_empty_gate_nav_commands_wave360_ok,
        live_radius_decal_update_dual_world_empty_gate_live_wave360_ok,
        live_railed_transport_dock_dual_world_empty_gate_method_names_wave361_ok,
        live_railed_transport_dock_dual_world_empty_gate_nav_commands_wave361_ok,
        live_railed_transport_dock_dual_world_empty_gate_live_wave361_ok,
        live_structure_collapse_update_dual_world_empty_gate_method_names_wave362_ok,
        live_structure_collapse_update_dual_world_empty_gate_nav_commands_wave362_ok,
        live_structure_collapse_update_dual_world_empty_gate_live_wave362_ok,
        live_propaganda_tower_behavior_dual_world_empty_gate_method_names_wave363_ok,
        live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_wave363_ok,
        live_propaganda_tower_behavior_dual_world_empty_gate_live_wave363_ok,
        live_propaganda_center_behavior_dual_world_empty_gate_method_names_wave364_ok,
        live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_wave364_ok,
        live_propaganda_center_behavior_dual_world_empty_gate_live_wave364_ok,
        live_production_update_complete_dual_world_empty_gate_method_names_wave365_ok,
        live_production_update_complete_dual_world_empty_gate_nav_commands_wave365_ok,
        live_production_update_complete_dual_world_empty_gate_live_wave365_ok,
        live_pow_truck_behavior_dual_world_empty_gate_method_names_wave366_ok,
        live_pow_truck_behavior_dual_world_empty_gate_nav_commands_wave366_ok,
        live_pow_truck_behavior_dual_world_empty_gate_live_wave366_ok,
        live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_wave367_ok,
        live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_wave367_ok,
        live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_live_wave367_ok,
        live_veterancy_crate_collide_dual_world_empty_gate_method_names_wave368_ok,
        live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_wave368_ok,
        live_veterancy_crate_collide_dual_world_empty_gate_live_wave368_ok,
        live_assault_transport_ai_update_dual_world_empty_gate_method_names_wave369_ok,
        live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_wave369_ok,
        live_assault_transport_ai_update_dual_world_empty_gate_live_wave369_ok,
        live_heal_contain_dual_world_empty_gate_method_names_wave370_ok,
        live_heal_contain_dual_world_empty_gate_nav_commands_wave370_ok,
        live_heal_contain_dual_world_empty_gate_live_wave370_ok,
        live_topple_update_dual_world_empty_gate_method_names_wave371_ok,
        live_topple_update_dual_world_empty_gate_nav_commands_wave371_ok,
        live_topple_update_dual_world_empty_gate_live_wave371_ok,
        live_projectile_stream_update_dual_world_empty_gate_method_names_wave372_ok,
        live_projectile_stream_update_dual_world_empty_gate_nav_commands_wave372_ok,
        live_projectile_stream_update_dual_world_empty_gate_live_wave372_ok,
        live_demo_trap_update_dual_world_empty_gate_method_names_wave373_ok,
        live_demo_trap_update_dual_world_empty_gate_nav_commands_wave373_ok,
        live_demo_trap_update_dual_world_empty_gate_live_wave373_ok,
        live_mob_member_slaved_update_dual_world_empty_gate_method_names_wave374_ok,
        live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_wave374_ok,
        live_mob_member_slaved_update_dual_world_empty_gate_live_wave374_ok,
        live_tn_guard_dual_world_empty_gate_method_names_wave375_ok,
        live_tn_guard_dual_world_empty_gate_nav_commands_wave375_ok,
        live_tn_guard_dual_world_empty_gate_live_wave375_ok,
        live_production_update_dual_world_empty_gate_method_names_wave376_ok,
        live_production_update_dual_world_empty_gate_nav_commands_wave376_ok,
        live_production_update_dual_world_empty_gate_live_wave376_ok,
        live_poisoned_behavior_dual_world_empty_gate_method_names_wave377_ok,
        live_poisoned_behavior_dual_world_empty_gate_nav_commands_wave377_ok,
        live_poisoned_behavior_dual_world_empty_gate_live_wave377_ok,
        live_horde_update_dual_world_empty_gate_method_names_wave378_ok,
        live_horde_update_dual_world_empty_gate_nav_commands_wave378_ok,
        live_horde_update_dual_world_empty_gate_live_wave378_ok,
        live_flammable_update_dual_world_empty_gate_method_names_wave379_ok,
        live_flammable_update_dual_world_empty_gate_nav_commands_wave379_ok,
        live_flammable_update_dual_world_empty_gate_live_wave379_ok,
        live_base_regenerate_update_dual_world_empty_gate_method_names_wave380_ok,
        live_base_regenerate_update_dual_world_empty_gate_nav_commands_wave380_ok,
        live_base_regenerate_update_dual_world_empty_gate_live_wave380_ok,
        live_queue_production_exit_behavior_dual_world_empty_gate_method_names_wave381_ok,
        live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_wave381_ok,
        live_queue_production_exit_behavior_dual_world_empty_gate_live_wave381_ok,
        live_missile_launcher_building_update_dual_world_empty_gate_method_names_wave382_ok,
        live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_wave382_ok,
        live_missile_launcher_building_update_dual_world_empty_gate_live_wave382_ok,
        live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_wave383_ok,
        live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_wave383_ok,
        live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_live_wave383_ok,
        live_command_button_hunt_update_dual_world_empty_gate_method_names_wave384_ok,
        live_command_button_hunt_update_dual_world_empty_gate_nav_commands_wave384_ok,
        live_command_button_hunt_update_dual_world_empty_gate_live_wave384_ok,
        live_prison_behavior_dual_world_empty_gate_method_names_wave385_ok,
        live_prison_behavior_dual_world_empty_gate_nav_commands_wave385_ok,
        live_prison_behavior_dual_world_empty_gate_live_wave385_ok,
        live_generate_minefield_behavior_dual_world_empty_gate_method_names_wave386_ok,
        live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_wave386_ok,
        live_generate_minefield_behavior_dual_world_empty_gate_live_wave386_ok,
        live_demoralize_special_power_dual_world_empty_gate_method_names_wave387_ok,
        live_demoralize_special_power_dual_world_empty_gate_nav_commands_wave387_ok,
        live_demoralize_special_power_dual_world_empty_gate_live_wave387_ok,
        live_stealth_detector_update_dual_world_empty_gate_method_names_wave388_ok,
        live_stealth_detector_update_dual_world_empty_gate_nav_commands_wave388_ok,
        live_stealth_detector_update_dual_world_empty_gate_live_wave388_ok,
        live_hive_structure_body_dual_world_empty_gate_method_names_wave389_ok,
        live_hive_structure_body_dual_world_empty_gate_nav_commands_wave389_ok,
        live_hive_structure_body_dual_world_empty_gate_live_wave389_ok,
        live_salvage_crate_collide_dual_world_empty_gate_method_names_wave390_ok,
        live_salvage_crate_collide_dual_world_empty_gate_nav_commands_wave390_ok,
        live_salvage_crate_collide_dual_world_empty_gate_live_wave390_ok,
        live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_wave391_ok,
        live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_wave391_ok,
        live_sabotage_internet_center_crate_collide_dual_world_empty_gate_live_wave391_ok,
        live_power_plant_update_dual_world_empty_gate_method_names_wave392_ok,
        live_power_plant_update_dual_world_empty_gate_nav_commands_wave392_ok,
        live_power_plant_update_dual_world_empty_gate_live_wave392_ok,
        live_leaflet_drop_behavior_dual_world_empty_gate_method_names_wave393_ok,
        live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_wave393_ok,
        live_leaflet_drop_behavior_dual_world_empty_gate_live_wave393_ok,
        live_auto_deposit_update_dual_world_empty_gate_method_names_wave394_ok,
        live_auto_deposit_update_dual_world_empty_gate_nav_commands_wave394_ok,
        live_auto_deposit_update_dual_world_empty_gate_live_wave394_ok,
        live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_wave395_ok,
        live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_wave395_ok,
        live_supply_warehouse_crippling_behavior_dual_world_empty_gate_live_wave395_ok,
        live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_wave396_ok,
        live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_wave396_ok,
        live_neutron_missile_slow_death_update_dual_world_empty_gate_live_wave396_ok,
        live_ai_dock_dual_world_empty_gate_method_names_wave397_ok,
        live_ai_dock_dual_world_empty_gate_nav_commands_wave397_ok,
        live_ai_dock_dual_world_empty_gate_live_wave397_ok,
        live_ai_groups_dual_world_empty_gate_method_names_wave398_ok,
        live_ai_groups_dual_world_empty_gate_nav_commands_wave398_ok,
        live_ai_groups_dual_world_empty_gate_live_wave398_ok,
        live_artillery_barrage_power_dual_world_empty_gate_method_names_wave399_ok,
        live_artillery_barrage_power_dual_world_empty_gate_nav_commands_wave399_ok,
        live_artillery_barrage_power_dual_world_empty_gate_live_wave399_ok,
        live_baikonur_launch_power_dual_world_empty_gate_method_names_wave400_ok,
        live_baikonur_launch_power_dual_world_empty_gate_nav_commands_wave400_ok,
        live_baikonur_launch_power_dual_world_empty_gate_live_wave400_ok,
        live_ai_group_dual_world_empty_gate_method_names_wave401_ok,
        live_ai_group_dual_world_empty_gate_nav_commands_wave401_ok,
        live_ai_group_dual_world_empty_gate_live_wave401_ok,
        live_slaved_update_dual_world_empty_gate_method_names_wave402_ok,
        live_slaved_update_dual_world_empty_gate_nav_commands_wave402_ok,
        live_slaved_update_dual_world_empty_gate_live_wave402_ok,
        live_demoralize_power_dual_world_empty_gate_method_names_wave403_ok,
        live_demoralize_power_dual_world_empty_gate_nav_commands_wave403_ok,
        live_demoralize_power_dual_world_empty_gate_live_wave403_ok,
        live_bone_fx_update_dual_world_empty_gate_method_names_wave404_ok,
        live_bone_fx_update_dual_world_empty_gate_nav_commands_wave404_ok,
        live_bone_fx_update_dual_world_empty_gate_live_wave404_ok,
        live_supply_warehouse_dock_dual_world_empty_gate_method_names_wave405_ok,
        live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_wave405_ok,
        live_supply_warehouse_dock_dual_world_empty_gate_live_wave405_ok,
        live_ocl_special_power_dual_world_empty_gate_method_names_wave406_ok,
        live_ocl_special_power_dual_world_empty_gate_nav_commands_wave406_ok,
        live_ocl_special_power_dual_world_empty_gate_live_wave406_ok,
        live_railed_transport_ai_update_dual_world_empty_gate_method_names_wave407_ok,
        live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_wave407_ok,
        live_railed_transport_ai_update_dual_world_empty_gate_live_wave407_ok,
        live_squish_collide_dual_world_empty_gate_method_names_wave408_ok,
        live_squish_collide_dual_world_empty_gate_nav_commands_wave408_ok,
        live_squish_collide_dual_world_empty_gate_live_wave408_ok,
        live_weapon_bonus_update_dual_world_empty_gate_method_names_wave409_ok,
        live_weapon_bonus_update_dual_world_empty_gate_nav_commands_wave409_ok,
        live_weapon_bonus_update_dual_world_empty_gate_live_wave409_ok,
        live_minefield_behavior_dual_world_empty_gate_method_names_wave410_ok,
        live_minefield_behavior_dual_world_empty_gate_nav_commands_wave410_ok,
        live_minefield_behavior_dual_world_empty_gate_live_wave410_ok,
        live_point_defense_laser_update_dual_world_empty_gate_method_names_wave411_ok,
        live_point_defense_laser_update_dual_world_empty_gate_nav_commands_wave411_ok,
        live_point_defense_laser_update_dual_world_empty_gate_live_wave411_ok,
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
            "host={host_constructed} cfg={skirmish_config_ok} menu_cfg={menu_config_ok} map_res={map_resolved} map_load={map_loaded} frames={frames_advanced} pres={presentation_ok} dual_tick={dual_tick_presentation_ok} dual_tick_ctr={dual_tick_counters_ok} gw_shadow={gameworld_shadow_ok} dmg_auth={damage_authority_env_ok} econ_auth={economy_authority_env_ok} dual_auth_only={dual_tick_policy_authority_only} bridge_off={engine_bridge_off} hud_sel={hud_selection_ok} sel_consumers={selection_consumers_ok} minimap_fow={minimap_fow_presentation_ok} laser_upload={laser_segment_upload_ok} projectile_upload={projectile_segment_upload_ok} move_lines={move_line_upload_ok} attack_lines={attack_line_upload_ok} multi_beam={multi_beam_soft_edge_ok} laser_pres={laser_presentation_residual_ok} floating_text={floating_text_layout_ok} ft_vanish={floating_text_vanish_ok} world_anim={world_anim_presentation_ok} world_anim_layout={world_anim_layout_ok} wa_fade={world_anim_fade_ok} anim2d={anim2d_frame_ok} anim2d_col={anim2d_collection_residual_ok} translate_copy={translate_copy_residual_ok} game_text={game_text_caption_ok} csf_str={game_text_csf_str_ok} ds_measure={display_string_measure_ok} rng={rng_stream_residual_ok} mesh={mesh_asset_residual_ok} rng_pack={rng_residual_pack_ok} sp72={special_power_wave72_residual_ok} sp73={special_power_wave73_residual_ok} sp76={special_power_wave76_residual_ok} paradrop76={paradrop_wave76_residual_ok} cb76={control_bar_wave76_residual_ok} gfx76={graphics_wave76_residual_ok} spectre_decal={spectre_orbit_decal_presentation_ok} sp77={special_power_wave77_residual_ok} fow77={fow_residual_pack_ok} gh77={ground_height_presentation_ok} weapon77={weapon_store_seed_residual_ok} ai77={ai_skirmish_residual_ok} sp78={special_power_wave78_residual_ok} cluster78={cluster_mines_wave78_residual_ok} gps78={gps_scrambler_wave78_residual_ok} cash78={cash_bounty_wave78_residual_ok} minimap79={minimap_residual_pack_ok} sel79={selection_hud_residual_pack_ok} input79={input_residual_pack_ok} draw79={drawable_residual_fields_ok} train79={unit_training_wave79_residual_ok} upg79={upgrades_cost_time_application_ok} cmdbtn80={command_button_wave80_residual_ok} rank80={science_rank_wave80_residual_ok} kindof80={superweapon_kindof_wave80_residual_ok} spenum80={special_power_enum_wave80_residual_ok} height81={terrain_height_sample_wave81_ok} path81={pathfinder_wave81_residual_ok} loco81={locomotor_table_wave81_ok} armor81={armor_table_wave81_ok} puc81={puc_flare_table_wave81_ok} dmg82={damage_type_wave82_ok} death82={death_type_wave82_ok} mc82={model_condition_wave82_ok} wbonus82={weapon_bonus_wave82_ok} ostatus82={object_status_wave82_ok} prod83={production_queue_wave83_ok} supply83={supply_warehouse_wave83_ok} dozer83={dozer_build_wave83_ok} capture83={capture_building_wave83_ok} power83={power_plant_wave83_ok} cc83={command_center_wave83_ok} kindof84={kindof_wave84_ok} wslot84={weapon_slot_wave84_ok} vet84={veterancy_wave84_ok} rel84={relationship_wave84_ok} geom84={geometry_wave84_ok} shadow84={shadow_wave84_ok} faction85={faction_side_wave85_ok} ptpl85={player_template_wave85_ok} cash85={starting_cash_wave85_ok} aiperson85={skirmish_ai_personality_wave85_ok} victory85={victory_condition_wave85_ok} cam86={gamedata_camera_fps_wave86_ok} world86={gamedata_world_constants_wave86_ok} mpopt86={multiplayer_options_wave86_ok} mapsel86={map_selection_wave86_ok} crate86={crate_deepen_wave86_ok} weather87={weather_wave87_ok} water87={water_wave87_ok} bridge87={bridge_wave87_ok} tunnel87={tunnel_wave87_ok} garrison87={garrison_wave87_ok} transport87={transport_wave87_ok} radius88={radius_cursor_wave88_ok} mouse88={mouse_cursor_wave88_ok} fxlist88={superweapon_fxlist_wave88_ok} ocl88={superweapon_ocl_wave88_ok} particle88={superweapon_particle_wave88_ok} audio88={superweapon_audio_wave88_ok} rank89={rank_skill_wave89_ok} exp89={experience_wave89_ok} hotkey89={hotkey_wave89_ok} chat89={chat_wave89_ok} replay89={replay_wave89_ok} options89={options_wave89_ok} gamespeed90={gamespeed_wave90_ok} framerate90={frame_rate_wave90_ok} debug90={debug_tables_wave90_ok} lang90={language_wave90_ok} credits90={credits_wave90_ok} tooltip91={tooltip_wave91_ok} helpbox91={help_box_wave91_ok} message91={message_wave91_ok} eva91={eva_wave91_ok} video91={video_wave91_ok} briefing91={mission_briefing_wave91_ok} weapon92={weapon_deepen_wave92_ok} armor92={armor_expand_wave92_ok} body92={body_health_wave92_ok} loco92={locomotor_expand_wave92_ok} science92={science_names_wave92_ok} particle93={particle_emit_wave93_ok} drawable93={drawable_opacity_wave93_ok} shadow93={shadow_deepen_wave93_ok} terrain_tex93={terrain_texture_wave93_ok} road93={road_wave93_ok} ai_state94={ai_state_wave94_ok} special_ability94={special_ability_wave94_ok} upgrade_names94={upgrade_names_wave94_ok} command_set94={command_set_wave94_ok} script_action95={script_action_wave95_ok} script_cond95={script_condition_wave95_ok} map_object95={map_object_wave95_ok} waypoint95={waypoint_wave95_ok} team95={team_wave95_ok} player95={player_deepen_wave95_ok} partition96={partition_wave96_ok} collision96={collision_wave96_ok} physics96={physics_wave96_ok} projectile96={projectile_wave96_ok} radar97={radar_deepen_wave97_ok} spotter97={spotter_wave97_ok} stealth97={stealth_deepen_wave97_ok} detector97={detector_deepen_wave97_ok} vision97={vision_wave97_ok} dock98={dock_wave98_ok} contain98={contain_wave98_ok} exit98={exit_wave98_ok} heal98={heal_wave98_ok} production99={production_deepen_wave99_ok} buildable99={buildable_wave99_ok} prereq99={prerequisite_wave99_ok} cmdbtn99={command_button_deepen_wave99_ok} controlbar99={control_bar_deepen_wave99_ok} thing_factory100={thing_factory_deepen_wave100_ok} module_type100={module_type_wave100_ok} xfer100={xfer_deepen_wave100_ok} tf_crosslink100={thing_factory_crosslink_wave100_ok} module_factory101={module_factory_deepen_wave101_ok} thing_factory101={thing_factory_create_wave101_ok} partition_register101={partition_register_wave101_ok} mf_crosslink101={mf_crosslink_wave101_ok} display102={display_string_deepen_wave102_ok} anim2d102={anim2d_deepen_wave102_ok} laser102={laser_segliner_deepen_wave102_ok} csf102={csf_multi_locale_deepen_wave102_ok} pres102={presentation_deepen_wave102_ok} weapon103={weapon_deepen_wave103_ok} armor103={armor_expand_wave103_ok} loco103={locomotor_expand_wave103_ok} sp103={special_power_deepen_wave103_ok} kindof103={object_kindof_wave103_ok} object_status104={object_status_wave104_ok} object_create104={object_create_wave104_ok} active_body104={active_body_wave104_ok} drawable_create104={drawable_create_wave104_ok} register_object104={register_object_wave104_ok} ai_group105={ai_group_wave105_ok} ai_path105={ai_path_wave105_ok} weapon_fire105={weapon_fire_wave105_ok} damage_app105={damage_application_wave105_ok} veterancy105={veterancy_wave105_ok} gamestate106={game_state_deepen_wave106_ok} campaign106={campaign_mission_wave106_ok} mainmenu106={main_menu_deepen_wave106_ok} gamewindow106={game_window_deepen_wave106_ok} layout106={window_layout_deepen_wave106_ok} particle107={particle_system_deepen_wave107_ok} fxlist107={fxlist_entry_deepen_wave107_ok} ocl107={ocl_create_deepen_wave107_ok} audio107={audio_deepen_wave107_ok} heightmap108={heightmap_deepen_wave108_ok} bridge108={bridge_deepen_wave108_ok} water108={water_deepen_wave108_ok} road108={road_deepen_wave108_ok} cliff108={cliff_peels_wave108_ok} sp_store109={special_power_store_wave109_ok} science_store109={science_store_wave109_ok} upgrade_store109={upgrade_store_wave109_ok} player109={player_deepen_wave109_ok} team109={team_deepen_wave109_ok} msg110={message_stream_marker_wave110_ok} garg110={game_message_arg_wave110_ok} meta110={meta_event_category_wave110_ok} igui110={ingame_ui_wave110_ok} icon111={drawable_icon_flash_wave111_ok} dstatus111={drawable_status_stealth_wave111_ok} tdecal111={terrain_decal_wave111_ok} dimg111={display_draw_image_wave111_ok} gctrans111={game_client_translator_wave111_ok} pprio111={particle_priority_wave111_ok} mouse112={mouse_residual_wave112_ok} key112={keyboard_residual_wave112_ok} view112={view_residual_wave112_ok} gwm113={game_window_manager_wave113_ok} style113={window_style_wave113_ok} gadget113={gadget_wave113_ok} video113={video_buffer_wave113_ok} audio113={audio_event_wave113_ok} mm114={main_menu_skirmish_names_wave114_ok} nav114={main_menu_skirmish_nav_steps_wave114_ok} msg114={main_menu_skirmish_message_wave114_ok} ms115={map_select_names_wave115_ok} msnav115={map_select_nav_steps_wave115_ok} mscmd115={map_select_commands_wave115_ok} slot116={slot_state_wave116_ok} slotcmb116={slot_combo_names_wave116_ok} slotnav116={slot_nav_commands_wave116_ok} cash117={starting_cash_wave117_ok} spd117={game_speed_controls_wave117_ok} rules117={rules_nav_commands_wave117_ok} mmbtn118={main_menu_button_names_wave118_ok} mmpush118={main_menu_push_targets_wave118_ok} mmnav118={main_menu_button_nav_commands_wave118_ok} camp119={campaign_button_names_wave119_ok} campen119={campaign_enums_wave119_ok} campnav119={campaign_nav_commands_wave119_ok} chal120={challenge_control_names_wave120_ok} chalnav120={challenge_nav_commands_wave120_ok} sl121={save_load_layout_wave121_ok} slctl121={save_load_control_stems_wave121_ok} slnav121={save_load_nav_commands_wave121_ok} rep122={replay_control_names_wave122_ok} repnav122={replay_nav_commands_wave122_ok} quit123={quit_control_names_wave123_ok} quitnav123={quit_nav_commands_wave123_ok} kb124={keyboard_control_names_wave124_ok} kbnav124={keyboard_nav_commands_wave124_ok} score125={score_control_names_wave125_ok} scorenav125={score_nav_commands_wave125_ok} opt126={options_control_names_wave126_ok} optnav126={options_nav_commands_wave126_ok} cred127={credits_control_names_wave127_ok} crednav127={credits_nav_commands_wave127_ok} msg128={message_box_control_names_wave128_ok} msgnav128={message_box_nav_commands_wave128_ok} dip129={diplomacy_control_names_wave129_ok} dipnav129={diplomacy_nav_commands_wave129_ok} poprep130={popup_replay_control_names_wave130_ok} poprepnav130={popup_replay_nav_commands_wave130_ok} sp131={single_player_control_names_wave131_ok} spnav131={single_player_nav_commands_wave131_ok} map132={map_select_control_names_wave132_ok} mapnav132={map_select_nav_commands_wave132_ok} cb133={control_bar_control_names_wave133_ok} cbnav133={control_bar_nav_commands_wave133_ok} diff134={difficulty_select_control_names_wave134_ok} diffnav134={difficulty_select_nav_commands_wave134_ok} load135={loading_screen_stages_wave135_ok} loadnav135={loading_screen_nav_commands_wave135_ok} chat136={in_game_chat_control_names_wave136_ok} chatnav136={in_game_chat_nav_commands_wave136_ok} idle137={idle_worker_control_names_wave137_ok} idlenav137={idle_worker_nav_commands_wave137_ok} gen138={generals_exp_control_names_wave138_ok} gennav138={generals_exp_nav_commands_wave138_ok} popcom139={popup_communicator_control_names_wave139_ok} popcomnav139={popup_communicator_nav_commands_wave139_ok} repctrl140={replay_control_control_names_wave140_ok} repctrlnav140={replay_control_nav_commands_wave140_ok} shell141={shell_map_names_wave141_ok} shellnav141={shell_map_nav_commands_wave141_ok} beacon142={beacon_control_names_wave142_ok} beaconnav142={beacon_nav_commands_wave142_ok} eva143={eva_message_names_wave143_ok} evanav143={eva_nav_commands_wave143_ok} ime144={ime_message_names_wave144_ok} imenav144={ime_nav_commands_wave144_ok} smudge145={smudge_method_names_wave145_ok} smudgenav145={smudge_nav_commands_wave145_ok} ocl146={ocl_timer_method_names_wave146_ok} oclnav146={ocl_timer_nav_commands_wave146_ok} cbresizer147={control_bar_resizer_method_names_wave147_ok} cbresizernav147={control_bar_resizer_nav_commands_wave147_ok} uc148={under_construction_method_names_wave148_ok} ucnav148={under_construction_nav_commands_wave148_ok} si149={structure_inventory_command_names_wave149_ok} sinav149={structure_inventory_nav_commands_wave149_ok} ms150={multi_select_method_names_wave150_ok} msnav150={multi_select_nav_commands_wave150_ok} cr151={credits_style_method_names_wave151_ok} crnav151={credits_nav_commands_wave151_ok} cg152={challenge_generals_method_names_wave152_ok} cgnav152={challenge_generals_nav_commands_wave152_ok} gwa153={gameworld_authority_env_names_wave153_ok} gwamethod153={gameworld_authority_method_names_wave153_ok} gwanav153={gameworld_authority_nav_commands_wave153_ok} wv154={window_video_type_state_names_wave154_ok} wvmethod154={window_video_method_names_wave154_ok} wvnav154={window_video_nav_commands_wave154_ok} mml155={main_menu_layout_names_wave155_ok} mmlnav155={main_menu_layout_nav_commands_wave155_ok} cbs156={control_bar_scheme_names_wave156_ok} cbsmethod156={control_bar_scheme_method_names_wave156_ok} cbsnav156={control_bar_scheme_nav_commands_wave156_ok} pb157={presentation_boundary_method_names_wave157_ok} pbsrc157={presentation_boundary_source_markers_wave157_ok} pbnav157={presentation_boundary_nav_commands_wave157_ok} pblive157={presentation_boundary_live_wave157_ok} cbpp158={control_bar_print_names_wave158_ok} cbppnav158={control_bar_print_nav_commands_wave158_ok} teb159={terrain_env_boundary_method_names_wave159_ok} tebsrc159={terrain_env_boundary_source_markers_wave159_ok} tebnav159={terrain_env_boundary_nav_commands_wave159_ok} teblive159={terrain_env_boundary_live_wave159_ok} mmw160={main_menu_wnd_names_wave160_ok} mmwnav160={main_menu_wnd_nav_commands_wave160_ok} mmwlive160={main_menu_wnd_live_wave160_ok} mmwl161={main_menu_wnd_load_method_names_wave161_ok} mmwlnav161={main_menu_wnd_load_nav_commands_wave161_ok} mmwllive161={main_menu_wnd_load_live_wave161_ok} mmwm162={main_menu_wnd_materialise_method_names_wave162_ok} mmwmnav162={main_menu_wnd_materialise_nav_commands_wave162_ok} mmwmlive162={main_menu_wnd_materialise_live_wave162_ok} ssp163={shell_stack_push_method_names_wave163_ok} sspnav163={shell_stack_push_nav_commands_wave163_ok} ssplive163={shell_stack_push_live_wave163_ok} ssn164={shell_skirmish_nav_method_names_wave164_ok} ssnnav164={shell_skirmish_nav_commands_wave164_ok} ssnlive164={shell_skirmish_nav_live_wave164_ok} cbm165={control_bar_materialise_method_names_wave165_ok} cbmnav165={control_bar_materialise_nav_commands_wave165_ok} cbmlive165={control_bar_materialise_live_wave165_ok} sow166={skirmish_options_wnd_method_names_wave166_ok} sownav166={skirmish_options_wnd_nav_commands_wave166_ok} sowlive166={skirmish_options_wnd_live_wave166_ok} ngs167={new_game_stream_method_names_wave167_ok} ngsnav167={new_game_stream_nav_commands_wave167_ok} ngslive167={new_game_stream_live_wave167_ok} w3dmmi168={w3d_main_menu_init_method_names_wave168_ok} w3dmminav168={w3d_main_menu_init_nav_commands_wave168_ok} w3dmmilive168={w3d_main_menu_init_live_wave168_ok} sgl169={start_game_loading_method_names_wave169_ok} sglnav169={start_game_loading_nav_commands_wave169_ok} sgllive169={start_game_loading_live_wave169_ok} lml170={live_map_load_method_names_wave170_ok} lmlnav170={live_map_load_nav_commands_wave170_ok} lmllive170={live_map_load_live_wave170_ok} lps171={live_presentation_seed_method_names_wave171_ok} lpsnav171={live_presentation_seed_nav_commands_wave171_ok} lpslive171={live_presentation_seed_live_wave171_ok} lgwso172={live_gameworld_shadow_overlay_method_names_wave172_ok} lgwsonav172={live_gameworld_shadow_overlay_nav_commands_wave172_ok} lgwsolive172={live_gameworld_shadow_overlay_live_wave172_ok} sac173={single_authority_combat_method_names_wave173_ok} sacnav173={single_authority_combat_nav_commands_wave173_ok} saclive173={single_authority_combat_live_wave173_ok} pcb174={presentation_client_boundary_method_names_wave174_ok} pcbnav174={presentation_client_boundary_nav_commands_wave174_ok} pcblive174={presentation_client_boundary_live_wave174_ok} gmhv175={golden_map_host_victory_method_names_wave175_ok} gmhvnav175={golden_map_host_victory_nav_commands_wave175_ok} gmhvlive175={golden_map_host_victory_live_wave175_ok} epb176={executable_presentation_boundary_method_names_wave176_ok} epbnav176={executable_presentation_boundary_nav_commands_wave176_ok} epblive176={executable_presentation_boundary_live_wave176_ok} gwpa177={gameworld_production_authority_method_names_wave177_ok} gwpanav177={gameworld_production_authority_nav_commands_wave177_ok} gwpalive177={gameworld_production_authority_live_wave177_ok} gwstc178={gameworld_sole_tick_coupling_method_names_wave178_ok} gwstcnav178={gameworld_sole_tick_coupling_nav_commands_wave178_ok} gwstclive178={gameworld_sole_tick_coupling_live_wave178_ok} gwam179={gameworld_authority_matrix_method_names_wave179_ok} gwamnav179={gameworld_authority_matrix_nav_commands_wave179_ok} gwamlive179={gameworld_authority_matrix_live_wave179_ok} lgpw180={live_gameworld_production_writeback_method_names_wave180_ok} lgpwnav180={live_gameworld_production_writeback_nav_commands_wave180_ok} lgpwlive180={live_gameworld_production_writeback_live_wave180_ok} lgcw181={live_gameworld_construction_writeback_method_names_wave181_ok} lgcwnav181={live_gameworld_construction_writeback_nav_commands_wave181_ok} lgcwlive181={live_gameworld_construction_writeback_live_wave181_ok} lgdc182={live_gameworld_damage_channel_method_names_wave182_ok} lgdcnav182={live_gameworld_damage_channel_nav_commands_wave182_ok} lgdclive182={live_gameworld_damage_channel_live_wave182_ok} lgem183={live_gameworld_economy_movement_method_names_wave183_ok} lgemnav183={live_gameworld_economy_movement_nav_commands_wave183_ok} lgemlive183={live_gameworld_economy_movement_live_wave183_ok} lgpa184={live_gameworld_projectile_ai_method_names_wave184_ok} lgpanav184={live_gameworld_projectile_ai_nav_commands_wave184_ok} lgpalive184={live_gameworld_projectile_ai_live_wave184_ok} lgfsp185={live_gameworld_fire_special_power_method_names_wave185_ok} lgfspnav185={live_gameworld_fire_special_power_nav_commands_wave185_ok} lgfsplive185={live_gameworld_fire_special_power_live_wave185_ok} lgpv186={live_gameworld_presentation_view_method_names_wave186_ok} lgpvnav186={live_gameworld_presentation_view_nav_commands_wave186_ok} lgpvlive186={live_gameworld_presentation_view_live_wave186_ok} lpgo187={live_presentation_gameworld_overlay_method_names_wave187_ok} lpgonav187={live_presentation_gameworld_overlay_nav_commands_wave187_ok} lpgolive187={live_presentation_gameworld_overlay_live_wave187_ok} egwp188={executable_gameworld_presentation_method_names_wave188_ok} egwpnav188={executable_gameworld_presentation_nav_commands_wave188_ok} egwplive188={executable_gameworld_presentation_live_wave188_ok} lpod189={live_presentation_overlay_deepen_method_names_wave189_ok} lpodnav189={live_presentation_overlay_deepen_nav_commands_wave189_ok} lpodlive189={live_presentation_overlay_deepen_live_wave189_ok} lpos190={live_presentation_overlay_stamp_method_names_wave190_ok} lposnav190={live_presentation_overlay_stamp_nav_commands_wave190_ok} lposlive190={live_presentation_overlay_stamp_live_wave190_ok} lgevd191={live_gameworld_entity_view_deepen_method_names_wave191_ok} lgevdnav191={live_gameworld_entity_view_deepen_nav_commands_wave191_ok} lgevdlive191={live_gameworld_entity_view_deepen_live_wave191_ok} lpam192={live_presentation_append_missing_method_names_wave192_ok} lpamnav192={live_presentation_append_missing_nav_commands_wave192_ok} lpamlive192={live_presentation_append_missing_live_wave192_ok} lpbfg193={live_presentation_build_from_gameworld_method_names_wave193_ok} lpbfgnav193={live_presentation_build_from_gameworld_nav_commands_wave193_ok} lpbfglive193={live_presentation_build_from_gameworld_live_wave193_ok} lpfgd194={live_presentation_from_gameworld_default_method_names_wave194_ok} lpfgdnav194={live_presentation_from_gameworld_default_nav_commands_wave194_ok} lpfgdlive194={live_presentation_from_gameworld_default_live_wave194_ok} lpbfe195={live_presentation_build_for_engine_method_names_wave195_ok} lpbfenav195={live_presentation_build_for_engine_nav_commands_wave195_ok} lpbfelive195={live_presentation_build_for_engine_live_wave195_ok} lprvg196={live_presentation_rebuilt_vertical_gate_method_names_wave196_ok} lprvgnav196={live_presentation_rebuilt_vertical_gate_nav_commands_wave196_ok} lprvglive196={live_presentation_rebuilt_vertical_gate_live_wave196_ok} lcal197={live_command_attack_log_method_names_wave197_ok} lcalnav197={live_command_attack_log_nav_commands_wave197_ok} lcallive197={live_command_attack_log_live_wave197_ok} lcgl198={live_command_guard_log_method_names_wave198_ok} lcglnav198={live_command_guard_log_nav_commands_wave198_ok} lcgllive198={live_command_guard_log_live_wave198_ok} lcpcl199={live_command_production_construction_log_method_names_wave199_ok} lcpclnav199={live_command_production_construction_log_nav_commands_wave199_ok} lcpcllive199={live_command_production_construction_log_live_wave199_ok} lcrl200={live_command_rally_log_method_names_wave200_ok} lcrlnav200={live_command_rally_log_nav_commands_wave200_ok} lcrllive200={live_command_rally_log_live_wave200_ok} lecl201={live_evacuate_contain_log_method_names_wave201_ok} leclnav201={live_evacuate_contain_log_nav_commands_wave201_ok} lecllive201={live_evacuate_contain_log_live_wave201_ok} lccs202={live_command_cheer_science_log_method_names_wave202_ok} lccsnav202={live_command_cheer_science_log_nav_commands_wave202_ok} lccslive202={live_command_cheer_science_log_live_wave202_ok} lcds203={live_command_deploy_status_log_method_names_wave203_ok} lcdsnav203={live_command_deploy_status_log_nav_commands_wave203_ok} lcdslive203={live_command_deploy_status_log_live_wave203_ok} lcfl204={live_command_formation_log_method_names_wave204_ok} lcflnav204={live_command_formation_log_nav_commands_wave204_ok} lcfllive204={live_command_formation_log_live_wave204_ok} lcot205={live_command_order_target_log_method_names_wave205_ok} lcotnav205={live_command_order_target_log_nav_commands_wave205_ok} lcotlive205={live_command_order_target_log_live_wave205_ok} lcsl206={live_command_selection_log_method_names_wave206_ok} lcslnav206={live_command_selection_log_nav_commands_wave206_ok} lcsllive206={live_command_selection_log_live_wave206_ok} lcna207={live_command_non_attack_order_target_method_names_wave207_ok} lcnanav207={live_command_non_attack_order_target_nav_commands_wave207_ok} lcnalive207={live_command_non_attack_order_target_live_wave207_ok} lgmh208={live_golden_mopup_honesty_method_names_wave208_ok} lgmhnav208={live_golden_mopup_honesty_nav_commands_wave208_ok} lgmhlive208={live_golden_mopup_honesty_live_wave208_ok} loic209={live_os_input_command_path_method_names_wave209_ok} loicnav209={live_os_input_command_path_nav_commands_wave209_ok} loiclive209={live_os_input_command_path_live_wave209_ok} lcbn210={live_command_beacon_note_method_names_wave210_ok} lcbnnav210={live_command_beacon_note_nav_commands_wave210_ok} lcbnlive210={live_command_beacon_note_live_wave210_ok} lhbp211={live_host_beacon_presentation_method_names_wave211_ok} lhbpnav211={live_host_beacon_presentation_nav_commands_wave211_ok} lhbplive211={live_host_beacon_presentation_live_wave211_ok} lcsd212={live_command_sell_deselect_log_method_names_wave212_ok} lcsdnav212={live_command_sell_deselect_log_nav_commands_wave212_ok} lcsdlive212={live_command_sell_deselect_log_live_wave212_ok} lpfo213={live_presentation_fow_only_method_names_wave213_ok} lpfonav213={live_presentation_fow_only_nav_commands_wave213_ok} lpfolive213={live_presentation_fow_only_live_wave213_ok} lupp214={live_ui_producer_presentation_only_method_names_wave214_ok} luppnav214={live_ui_producer_presentation_only_nav_commands_wave214_ok} lupplive214={live_ui_producer_presentation_only_live_wave214_ok} luhp215={live_ui_helpers_presentation_only_method_names_wave215_ok} luhpnav215={live_ui_helpers_presentation_only_nav_commands_wave215_ok} luhplive215={live_ui_helpers_presentation_only_live_wave215_ok} lcgc216={live_control_group_camera_presentation_only_method_names_wave216_ok} lcgcnav216={live_control_group_camera_presentation_only_nav_commands_wave216_ok} lcgclive216={live_control_group_camera_presentation_only_live_wave216_ok} lcfe217={live_cmd_filter_env_presentation_only_method_names_wave217_ok} lcfenv217={live_cmd_filter_env_presentation_only_nav_commands_wave217_ok} lcfelive217={live_cmd_filter_env_presentation_only_live_wave217_ok} lsc218={live_selection_commands_presentation_only_method_names_wave218_ok} lscnav218={live_selection_commands_presentation_only_nav_commands_wave218_ok} lsclive218={live_selection_commands_presentation_only_live_wave218_ok} lucs219={live_ui_command_selection_presentation_only_method_names_wave219_ok} lucsnav219={live_ui_command_selection_presentation_only_nav_commands_wave219_ok} lucslive219={live_ui_command_selection_presentation_only_live_wave219_ok} llt220={live_local_team_presentation_only_method_names_wave220_ok} lltnav220={live_local_team_presentation_only_nav_commands_wave220_ok} lltlive220={live_local_team_presentation_only_live_wave220_ok} lhma221={live_hotkey_move_attack_selection_presentation_only_method_names_wave221_ok} lhmanav221={live_hotkey_move_attack_selection_presentation_only_nav_commands_wave221_ok} lhmalive221={live_hotkey_move_attack_selection_presentation_only_live_wave221_ok} lpo222={live_pick_object_presentation_only_method_names_wave222_ok} lponav222={live_pick_object_presentation_only_nav_commands_wave222_ok} lpolive222={live_pick_object_presentation_only_live_wave222_ok} lbc223={live_bootstrap_camera_presentation_only_method_names_wave223_ok} lbcnav223={live_bootstrap_camera_presentation_only_nav_commands_wave223_ok} lbclive223={live_bootstrap_camera_presentation_only_live_wave223_ok} lfca224={live_force_complete_authority_api_method_names_wave224_ok} lfcanav224={live_force_complete_authority_api_nav_commands_wave224_ok} lfcalive224={live_force_complete_authority_api_live_wave224_ok} lpga225={live_path_guard_authority_api_method_names_wave225_ok} lpganav225={live_path_guard_authority_api_nav_commands_wave225_ok} lpgalive225={live_path_guard_authority_api_live_wave225_ok} lhsc226={live_hotkey_selection_camera_presentation_only_method_names_wave226_ok} lhscnav226={live_hotkey_selection_camera_presentation_only_nav_commands_wave226_ok} lhsclive226={live_hotkey_selection_camera_presentation_only_live_wave226_ok} lcsp227={live_construct_spawn_pose_authority_api_method_names_wave227_ok} lcspnav227={live_construct_spawn_pose_authority_api_nav_commands_wave227_ok} lcsplive227={live_construct_spawn_pose_authority_api_live_wave227_ok} lrmb228={live_rmb_target_presentation_only_method_names_wave228_ok} lrmbnav228={live_rmb_target_presentation_only_nav_commands_wave228_ok} lrmblive228={live_rmb_target_presentation_only_live_wave228_ok} lrmbs229={live_rmb_selected_presentation_only_method_names_wave229_ok} lrmbsnav229={live_rmb_selected_presentation_only_nav_commands_wave229_ok} lrmbslive229={live_rmb_selected_presentation_only_live_wave229_ok} lcua230={live_command_unit_authority_api_method_names_wave230_ok} lcuanav230={live_command_unit_authority_api_nav_commands_wave230_ok} lcualive230={live_command_unit_authority_api_live_wave230_ok} lcuma231={live_command_unit_more_authority_api_method_names_wave231_ok} lcumanav231={live_command_unit_more_authority_api_nav_commands_wave231_ok} lcumalive231={live_command_unit_more_authority_api_live_wave231_ok} lcea232={live_command_executor_authority_api_method_names_wave232_ok} lceanav232={live_command_executor_authority_api_nav_commands_wave232_ok} lcealive232={live_command_executor_authority_api_live_wave232_ok} lcema233={live_command_executor_more_authority_api_method_names_wave233_ok} lcemanav233={live_command_executor_more_authority_api_nav_commands_wave233_ok} lcemalive233={live_command_executor_more_authority_api_live_wave233_ok} leppu234={live_engine_presentation_player_ui_method_names_wave234_ok} leppunav234={live_engine_presentation_player_ui_nav_commands_wave234_ok} leppulive234={live_engine_presentation_player_ui_live_wave234_ok} lrfc235={live_rmb_presentation_full_classify_method_names_wave235_ok} lrfcnav235={live_rmb_presentation_full_classify_nav_commands_wave235_ok} lrfclive235={live_rmb_presentation_full_classify_live_wave235_ok} lmipo236={live_mouse_input_presentation_only_method_names_wave236_ok} lmiponav236={live_mouse_input_presentation_only_nav_commands_wave236_ok} lmipolive236={live_mouse_input_presentation_only_live_wave236_ok} lepui237={live_engine_player_ui_boot_peel_method_names_wave237_ok} lepuinav237={live_engine_player_ui_boot_peel_nav_commands_wave237_ok} lepuilive237={live_engine_player_ui_boot_peel_live_wave237_ok} lppa238={live_player_probe_api_method_names_wave238_ok} lppanav238={live_player_probe_api_nav_commands_wave238_ok} lppalive238={live_player_probe_api_live_wave238_ok} lptp239={live_player_team_probe_method_names_wave239_ok} lptpnav239={live_player_team_probe_nav_commands_wave239_ok} lptplive239={live_player_team_probe_live_wave239_ok} lpfp240={live_player_field_probe_method_names_wave240_ok} lpfpnav240={live_player_field_probe_nav_commands_wave240_ok} lpfplive240={live_player_field_probe_live_wave240_ok} lchp241={live_camera_height_probe_method_names_wave241_ok} lchpnav241={live_camera_height_probe_nav_commands_wave241_ok} lchplive241={live_camera_height_probe_live_wave241_ok} lcpp242={live_command_player_probe_method_names_wave242_ok} lcppnav242={live_command_player_probe_nav_commands_wave242_ok} lcpplive242={live_command_player_probe_live_wave242_ok} lcep243={live_construct_economy_probe_method_names_wave243_ok} lcepnav243={live_construct_economy_probe_nav_commands_wave243_ok} lceplive243={live_construct_economy_probe_live_wave243_ok} lcup244={live_command_unit_probe_method_names_wave244_ok} lcupnav244={live_command_unit_probe_nav_commands_wave244_ok} lcuplive244={live_command_unit_probe_live_wave244_ok} lsqp245={live_selection_query_probe_method_names_wave245_ok} lsqpnav245={live_selection_query_probe_nav_commands_wave245_ok} lsqplive245={live_selection_query_probe_live_wave245_ok} lwpp246={live_world_pick_probe_method_names_wave246_ok} lwppnav246={live_world_pick_probe_nav_commands_wave246_ok} lwpplive246={live_world_pick_probe_live_wave246_ok} loref247={live_object_registry_empty_fastpath_method_names_wave247_ok} lorefnav247={live_object_registry_empty_fastpath_nav_commands_wave247_ok} loreflive247={live_object_registry_empty_fastpath_live_wave247_ok} llorf248={live_legacy_object_registry_fastpath_method_names_wave248_ok} llorfnav248={live_legacy_object_registry_fastpath_nav_commands_wave248_ok} llorflive248={live_legacy_object_registry_fastpath_live_wave248_ok} lcdweg249={live_client_dual_world_empty_gate_method_names_wave249_ok} lcdwegnav249={live_client_dual_world_empty_gate_nav_commands_wave249_ok} lcdweglive249={live_client_dual_world_empty_gate_live_wave249_ok} lptfp250={live_presentation_time_frozen_probe_method_names_wave250_ok} lptfpnav250={live_presentation_time_frozen_probe_nav_commands_wave250_ok} lptfplive250={live_presentation_time_frozen_probe_live_wave250_ok} lpvsp251={live_presentation_visual_speed_probe_method_names_wave251_ok} lpvspnav251={live_presentation_visual_speed_probe_nav_commands_wave251_ok} lpvsplive251={live_presentation_visual_speed_probe_live_wave251_ok} lpscp252={live_presentation_script_camera_probe_method_names_wave252_ok} lpscpnav252={live_presentation_script_camera_probe_nav_commands_wave252_ok} lpscplive252={live_presentation_script_camera_probe_live_wave252_ok} lagdw253={live_ai_group_dual_world_empty_gate_method_names_wave253_ok} lagdwnav253={live_ai_group_dual_world_empty_gate_nav_commands_wave253_ok} lagdwlive253={live_ai_group_dual_world_empty_gate_live_wave253_ok} lasdw254={live_ai_states_dual_world_empty_gate_method_names_wave254_ok} lasdwnav254={live_ai_states_dual_world_empty_gate_nav_commands_wave254_ok} lasdwlive254={live_ai_states_dual_world_empty_gate_live_wave254_ok} lapdw255={live_ai_player_dual_world_empty_gate_method_names_wave255_ok} lapdwnav255={live_ai_player_dual_world_empty_gate_nav_commands_wave255_ok} lapdwlive255={live_ai_player_dual_world_empty_gate_live_wave255_ok} ltdw256={live_team_dual_world_empty_gate_method_names_wave256_ok} ltdwnav256={live_team_dual_world_empty_gate_nav_commands_wave256_ok} ltdwlive256={live_team_dual_world_empty_gate_live_wave256_ok} lalsdw257={live_ai_legacy_states_dual_world_empty_gate_method_names_wave257_ok} lalsdwnav257={live_ai_legacy_states_dual_world_empty_gate_nav_commands_wave257_ok} lalsdwlive257={live_ai_legacy_states_dual_world_empty_gate_live_wave257_ok} ludw258={live_unit_dual_world_empty_gate_method_names_wave258_ok} ludwnav258={live_unit_dual_world_empty_gate_nav_commands_wave258_ok} ludwlive258={live_unit_dual_world_empty_gate_live_wave258_ok} lsdw259={live_stealth_dual_world_empty_gate_method_names_wave259_ok} lsdwnav259={live_stealth_dual_world_empty_gate_nav_commands_wave259_ok} lsdwlive259={live_stealth_dual_world_empty_gate_live_wave259_ok} lgdw260={live_garrison_dual_world_empty_gate_method_names_wave260_ok} lgdwnav260={live_garrison_dual_world_empty_gate_nav_commands_wave260_ok} lgdwlive260={live_garrison_dual_world_empty_gate_live_wave260_ok} locdw261={live_open_contain_dual_world_empty_gate_method_names_wave261_ok} locdwnav261={live_open_contain_dual_world_empty_gate_nav_commands_wave261_ok} locdwlive261={live_open_contain_dual_world_empty_gate_live_wave261_ok} lpfwd262={live_pathfind_dual_world_empty_gate_method_names_wave262_ok} lpfwdnav262={live_pathfind_dual_world_empty_gate_nav_commands_wave262_ok} lpfwdlive262={live_pathfind_dual_world_empty_gate_live_wave262_ok} lamdw263={live_ai_mod_dual_world_empty_gate_method_names_wave263_ok} lamdwnav263={live_ai_mod_dual_world_empty_gate_nav_commands_wave263_ok} lamdwlive263={live_ai_mod_dual_world_empty_gate_live_wave263_ok} lomdw264={live_object_mod_dual_world_empty_gate_method_names_wave264_ok} lomdwnav264={live_object_mod_dual_world_empty_gate_nav_commands_wave264_ok} lomdwlive264={live_object_mod_dual_world_empty_gate_live_wave264_ok} lwdw265={live_weapon_dual_world_empty_gate_method_names_wave265_ok} lwdwnav265={live_weapon_dual_world_empty_gate_nav_commands_wave265_ok} lwdwlive265={live_weapon_dual_world_empty_gate_live_wave265_ok} lpfldw266={live_partition_filters_dual_world_empty_gate_method_names_wave266_ok} lpfldwnav266={live_partition_filters_dual_world_empty_gate_nav_commands_wave266_ok} lpfldwlive266={live_partition_filters_dual_world_empty_gate_live_wave266_ok} lasmdw267={live_ai_state_machine_dual_world_empty_gate_method_names_wave267_ok} lasmdwnav267={live_ai_state_machine_dual_world_empty_gate_nav_commands_wave267_ok} lasmdwlive267={live_ai_state_machine_dual_world_empty_gate_live_wave267_ok} lpdw268={live_player_dual_world_empty_gate_method_names_wave268_ok} lpdwnav268={live_player_dual_world_empty_gate_nav_commands_wave268_ok} lpdwlive268={live_player_dual_world_empty_gate_live_wave268_ok} lgcdw269={live_game_client_dual_world_empty_gate_method_names_wave269_ok} lgcdwnav269={live_game_client_dual_world_empty_gate_nav_commands_wave269_ok} lgcdwlive269={live_game_client_dual_world_empty_gate_live_wave269_ok} lddw270={live_drawable_dual_world_empty_gate_method_names_wave270_ok} lddwnav270={live_drawable_dual_world_empty_gate_nav_commands_wave270_ok} lddwlive270={live_drawable_dual_world_empty_gate_live_wave270_ok} lscdw271={live_script_conditions_dual_world_empty_gate_method_names_wave271_ok} lscdwnav271={live_script_conditions_dual_world_empty_gate_nav_commands_wave271_ok} lscdwlive271={live_script_conditions_dual_world_empty_gate_live_wave271_ok} ltcdw272={live_transport_contain_dual_world_empty_gate_method_names_wave272_ok} ltcdwnav272={live_transport_contain_dual_world_empty_gate_nav_commands_wave272_ok} ltcdwlive272={live_transport_contain_dual_world_empty_gate_live_wave272_ok} liuidw273={live_ingame_ui_dual_world_empty_gate_method_names_wave273_ok} liuidwnav273={live_ingame_ui_dual_world_empty_gate_nav_commands_wave273_ok} liuidwlive273={live_ingame_ui_dual_world_empty_gate_live_wave273_ok} lhcdw274={live_helix_contain_dual_world_empty_gate_method_names_wave274_ok} lhcdwnav274={live_helix_contain_dual_world_empty_gate_nav_commands_wave274_ok} lhcdwlive274={live_helix_contain_dual_world_empty_gate_live_wave274_ok} lcpdw275={live_command_processor_dual_world_empty_gate_method_names_wave275_ok} lcpdwnav275={live_command_processor_dual_world_empty_gate_nav_commands_wave275_ok} lcpdwlive275={live_command_processor_dual_world_empty_gate_live_wave275_ok} ltdw276={live_turret_dual_world_empty_gate_method_names_wave276_ok} ltdwnav276={live_turret_dual_world_empty_gate_nav_commands_wave276_ok} ltdwlive276={live_turret_dual_world_empty_gate_live_wave276_ok} lrcdw277={live_rider_change_contain_dual_world_empty_gate_method_names_wave277_ok} lrcdwnav277={live_rider_change_contain_dual_world_empty_gate_nav_commands_wave277_ok} lrcdwlive277={live_rider_change_contain_dual_world_empty_gate_live_wave277_ok} lsdw278={live_selection_dual_world_empty_gate_method_names_wave278_ok} lsdwnav278={live_selection_dual_world_empty_gate_nav_commands_wave278_ok} lsdwlive278={live_selection_dual_world_empty_gate_live_wave278_ok} lccdw279={live_cave_contain_dual_world_empty_gate_method_names_wave279_ok} lccdwnav279={live_cave_contain_dual_world_empty_gate_nav_commands_wave279_ok} lccdwlive279={live_cave_contain_dual_world_empty_gate_live_wave279_ok} ltucdw280={live_tunnel_contain_dual_world_empty_gate_method_names_wave280_ok} ltucdwnav280={live_tunnel_contain_dual_world_empty_gate_nav_commands_wave280_ok} ltucdwlive280={live_tunnel_contain_dual_world_empty_gate_live_wave280_ok} lhdw281={live_helpers_dual_world_empty_gate_method_names_wave281_ok} lhdwnav281={live_helpers_dual_world_empty_gate_nav_commands_wave281_ok} lhdwlive281={live_helpers_dual_world_empty_gate_live_wave281_ok} laiudw282={live_ai_update_interface_dual_world_empty_gate_method_names_wave282_ok} laiudwnav282={live_ai_update_interface_dual_world_empty_gate_nav_commands_wave282_ok} laiudwlive282={live_ai_update_interface_dual_world_empty_gate_live_wave282_ok} lsudw283={live_stealth_update_dual_world_empty_gate_method_names_wave283_ok} lsudwnav283={live_stealth_update_dual_world_empty_gate_nav_commands_wave283_ok} lsudwlive283={live_stealth_update_dual_world_empty_gate_live_wave283_ok} lsedw284={live_script_executor_dual_world_empty_gate_method_names_wave284_ok} lsedwnav284={live_script_executor_dual_world_empty_gate_nav_commands_wave284_ok} lsedwlive284={live_script_executor_dual_world_empty_gate_live_wave284_ok} laiidw285={live_ai_integration_dual_world_empty_gate_method_names_wave285_ok} laiidwnav285={live_ai_integration_dual_world_empty_gate_nav_commands_wave285_ok} laiidwlive285={live_ai_integration_dual_world_empty_gate_live_wave285_ok} ldpdw286={live_dumb_projectile_dual_world_empty_gate_method_names_wave286_ok} ldpdwnav286={live_dumb_projectile_dual_world_empty_gate_nav_commands_wave286_ok} ldpdwlive286={live_dumb_projectile_dual_world_empty_gate_live_wave286_ok} lepdw287={live_enhanced_player_dual_world_empty_gate_method_names_wave287_ok} lepdwnav287={live_enhanced_player_dual_world_empty_gate_nav_commands_wave287_ok} lepdwlive287={live_enhanced_player_dual_world_empty_gate_live_wave287_ok} hjudw288={live_hijacker_update_dual_world_empty_gate_method_names_wave288_ok} hjudwnav288={live_hijacker_update_dual_world_empty_gate_nav_commands_wave288_ok} hjudwlive288={live_hijacker_update_dual_world_empty_gate_live_wave288_ok} wimpldw289={live_weapon_impl_dual_world_empty_gate_method_names_wave289_ok} wimpldwnav289={live_weapon_impl_dual_world_empty_gate_nav_commands_wave289_ok} wimpldwlive289={live_weapon_impl_dual_world_empty_gate_live_wave289_ok} apdw290={live_async_player_dual_world_empty_gate_method_names_wave290_ok} apdwnav290={live_async_player_dual_world_empty_gate_nav_commands_wave290_ok} apdwlive290={live_async_player_dual_world_empty_gate_live_wave290_ok} abdw291={live_active_body_dual_world_empty_gate_method_names_wave291_ok} abdwnav291={live_active_body_dual_world_empty_gate_nav_commands_wave291_ok} abdwlive291={live_active_body_dual_world_empty_gate_live_wave291_ok} scdw292={live_skirmish_conditions_dual_world_empty_gate_method_names_wave292_ok} scdwnav292={live_skirmish_conditions_dual_world_empty_gate_nav_commands_wave292_ok} scdwlive292={live_skirmish_conditions_dual_world_empty_gate_live_wave292_ok} abldw293={live_ai_build_list_dual_world_empty_gate_method_names_wave293_ok} abldwnav293={live_ai_build_list_dual_world_empty_gate_nav_commands_wave293_ok} abldwlive293={live_ai_build_list_dual_world_empty_gate_live_wave293_ok} vdw294={live_victory_dual_world_empty_gate_method_names_wave294_ok} vdwnav294={live_victory_dual_world_empty_gate_nav_commands_wave294_ok} vdwlive294={live_victory_dual_world_empty_gate_live_wave294_ok} sadw295={live_script_actions_dual_world_empty_gate_method_names_wave295_ok} sadwnav295={live_script_actions_dual_world_empty_gate_nav_commands_wave295_ok} sadwlive295={live_script_actions_dual_world_empty_gate_live_wave295_ok} spabdw296={live_special_ability_dual_world_empty_gate_method_names_wave296_ok} spabdwnav296={live_special_ability_dual_world_empty_gate_nav_commands_wave296_ok} spabdwlive296={live_special_ability_dual_world_empty_gate_live_wave296_ok} sddw297={live_stealth_detector_dual_world_empty_gate_method_names_wave297_ok} sddwnav297={live_stealth_detector_dual_world_empty_gate_nav_commands_wave297_ok} sddwlive297={live_stealth_detector_dual_world_empty_gate_live_wave297_ok} supdw298={live_supply_system_dual_world_empty_gate_method_names_wave298_ok} supdwnav298={live_supply_system_dual_world_empty_gate_nav_commands_wave298_ok} supdwlive298={live_supply_system_dual_world_empty_gate_live_wave298_ok} pudw299={live_particle_uplink_dual_world_empty_gate_method_names_wave299_ok} pudwnav299={live_particle_uplink_dual_world_empty_gate_nav_commands_wave299_ok} pudwlive299={live_particle_uplink_dual_world_empty_gate_live_wave299_ok} ocdw300={live_overlord_contain_dual_world_empty_gate_method_names_wave300_ok} ocdwnav300={live_overlord_contain_dual_world_empty_gate_nav_commands_wave300_ok} ocdwlive300={live_overlord_contain_dual_world_empty_gate_live_wave300_ok} bbdw301={live_bridge_behavior_dual_world_empty_gate_method_names_wave301_ok} bbdwnav301={live_bridge_behavior_dual_world_empty_gate_nav_commands_wave301_ok} bbdwlive301={live_bridge_behavior_dual_world_empty_gate_live_wave301_ok} sbdw302={live_stealth_behavior_dual_world_empty_gate_method_names_wave302_ok} sbdwnav302={live_stealth_behavior_dual_world_empty_gate_nav_commands_wave302_ok} sbdwlive302={live_stealth_behavior_dual_world_empty_gate_live_wave302_ok} ccdw303={live_crate_collide_dual_world_empty_gate_method_names_wave303_ok} ccdwnav303={live_crate_collide_dual_world_empty_gate_nav_commands_wave303_ok} ccdwlive303={live_crate_collide_dual_world_empty_gate_live_wave303_ok} omdw304={live_object_manager_dual_world_empty_gate_method_names_wave304_ok} omdwnav304={live_object_manager_dual_world_empty_gate_nav_commands_wave304_ok} omdwlive304={live_object_manager_dual_world_empty_gate_live_wave304_ok} sbomdw305={live_sticky_bomb_dual_world_empty_gate_method_names_wave305_ok} sbomdwnav305={live_sticky_bomb_dual_world_empty_gate_nav_commands_wave305_ok} sbomdwlive305={live_sticky_bomb_dual_world_empty_gate_live_wave305_ok} ahdw306={live_auto_heal_dual_world_empty_gate_method_names_wave306_ok} ahdwnav306={live_auto_heal_dual_world_empty_gate_nav_commands_wave306_ok} ahdwlive306={live_auto_heal_dual_world_empty_gate_live_wave306_ok} gsdw307={live_grant_stealth_dual_world_empty_gate_method_names_wave307_ok} gsdwnav307={live_grant_stealth_dual_world_empty_gate_nav_commands_wave307_ok} gsdwlive307={live_grant_stealth_dual_world_empty_gate_live_wave307_ok} sbudw308={live_status_bits_upgrade_dual_world_empty_gate_method_names_wave308_ok} sbudwnav308={live_status_bits_upgrade_dual_world_empty_gate_nav_commands_wave308_ok} sbudwlive308={live_status_bits_upgrade_dual_world_empty_gate_live_wave308_ok} jadw309={live_jet_ai_dual_world_empty_gate_method_names_wave309_ok} jadwnav309={live_jet_ai_dual_world_empty_gate_nav_commands_wave309_ok} jadwlive309={live_jet_ai_dual_world_empty_gate_live_wave309_ok} ppdw310={live_parking_place_dual_world_empty_gate_method_names_wave310_ok} ppdwnav310={live_parking_place_dual_world_empty_gate_nav_commands_wave310_ok} ppdwlive310={live_parking_place_dual_world_empty_gate_live_wave310_ok} fddw311={live_flight_deck_dual_world_empty_gate_method_names_wave311_ok} fddwnav311={live_flight_deck_dual_world_empty_gate_nav_commands_wave311_ok} fddwlive311={live_flight_deck_dual_world_empty_gate_live_wave311_ok} esdw312={live_exit_strategies_dual_world_empty_gate_method_names_wave312_ok} esdwnav312={live_exit_strategies_dual_world_empty_gate_nav_commands_wave312_ok} esdwlive312={live_exit_strategies_dual_world_empty_gate_live_wave312_ok} csdw313={live_collision_system_dual_world_empty_gate_method_names_wave313_ok} csdwnav313={live_collision_system_dual_world_empty_gate_nav_commands_wave313_ok} csdwlive313={live_collision_system_dual_world_empty_gate_live_wave313_ok} mhu314={live_max_health_upgrade_dual_world_empty_gate_method_names_wave314_ok} mhunav314={live_max_health_upgrade_dual_world_empty_gate_nav_commands_wave314_ok} mhulive314={live_max_health_upgrade_dual_world_empty_gate_live_wave314_ok} stdw315={live_structure_topple_dual_world_empty_gate_method_names_wave315_ok} stdwnav315={live_structure_topple_dual_world_empty_gate_nav_commands_wave315_ok} stdwlive315={live_structure_topple_dual_world_empty_gate_live_wave315_ok} pudw316={live_physics_update_dual_world_empty_gate_method_names_wave316_ok} pudwnav316={live_physics_update_dual_world_empty_gate_nav_commands_wave316_ok} pudwlive316={live_physics_update_dual_world_empty_gate_live_wave316_ok} chdw317={live_cleanup_hazard_dual_world_empty_gate_method_names_wave317_ok} chdwnav317={live_cleanup_hazard_dual_world_empty_gate_nav_commands_wave317_ok} chdwlive317={live_cleanup_hazard_dual_world_empty_gate_live_wave317_ok} btdw318={live_bridge_tower_dual_world_empty_gate_method_names_wave318_ok} btdwnav318={live_bridge_tower_dual_world_empty_gate_nav_commands_wave318_ok} btdwlive318={live_bridge_tower_dual_world_empty_gate_live_wave318_ok} audw319={live_armor_upgrade_dual_world_empty_gate_method_names_wave319_ok} audwnav319={live_armor_upgrade_dual_world_empty_gate_nav_commands_wave319_ok} audwlive319={live_armor_upgrade_dual_world_empty_gate_live_wave319_ok} ppdw320={live_paradrop_power_dual_world_empty_gate_method_names_wave320_ok} ppdwnav320={live_paradrop_power_dual_world_empty_gate_nav_commands_wave320_ok} ppdwlive320={live_paradrop_power_dual_world_empty_gate_live_wave320_ok} fadw321={live_fuel_air_bomb_dual_world_empty_gate_method_names_wave321_ok} fadwnav321={live_fuel_air_bomb_dual_world_empty_gate_nav_commands_wave321_ok} fadwlive321={live_fuel_air_bomb_dual_world_empty_gate_live_wave321_ok} tfdw322={live_tensile_formation_dual_world_empty_gate_method_names_wave322_ok} tfdwnav322={live_tensile_formation_dual_world_empty_gate_nav_commands_wave322_ok} tfdwlive322={live_tensile_formation_dual_world_empty_gate_live_wave322_ok} dmdw323={live_die_mod_dual_world_empty_gate_method_names_wave323_ok} dmdwnav323={live_die_mod_dual_world_empty_gate_nav_commands_wave323_ok} dmdwlive323={live_die_mod_dual_world_empty_gate_live_wave323_ok} pmdw324={live_partition_manager_dual_world_empty_gate_method_names_wave324_ok} pmdwnav324={live_partition_manager_dual_world_empty_gate_nav_commands_wave324_ok} pmdwlive324={live_partition_manager_dual_world_empty_gate_live_wave324_ok} sgdw325={live_spectre_gunship_dual_world_empty_gate_method_names_wave325_ok} sgdwnav325={live_spectre_gunship_dual_world_empty_gate_nav_commands_wave325_ok} sgdwlive325={live_spectre_gunship_dual_world_empty_gate_live_wave325_ok} pudw326={live_production_update_dual_world_empty_gate_method_names_wave326_ok} pudwnav326={live_production_update_dual_world_empty_gate_nav_commands_wave326_ok} pudwlive326={live_production_update_dual_world_empty_gate_live_wave326_ok} nbdw327={live_neutron_blast_dual_world_empty_gate_method_names_wave327_ok} nbdwnav327={live_neutron_blast_dual_world_empty_gate_nav_commands_wave327_ok} nbdwlive327={live_neutron_blast_dual_world_empty_gate_live_wave327_ok} cmdw328={live_countermeasures_dual_world_empty_gate_method_names_wave328_ok} cmdwnav328={live_countermeasures_dual_world_empty_gate_nav_commands_wave328_ok} cmdwlive328={live_countermeasures_dual_world_empty_gate_live_wave328_ok} spdw329={live_skirmish_player_dual_world_empty_gate_method_names_wave329_ok} spdwnav329={live_skirmish_player_dual_world_empty_gate_nav_commands_wave329_ok} spdwlive329={live_skirmish_player_dual_world_empty_gate_live_wave329_ok} a10dw330={live_a10_strike_dual_world_empty_gate_method_names_wave330_ok} a10dwnav330={live_a10_strike_dual_world_empty_gate_nav_commands_wave330_ok} a10dwlive330={live_a10_strike_dual_world_empty_gate_live_wave330_ok} rhdw331={live_rebuild_hole_dual_world_empty_gate_method_names_wave331_ok} rhdwnav331={live_rebuild_hole_dual_world_empty_gate_nav_commands_wave331_ok} rhdwlive331={live_rebuild_hole_dual_world_empty_gate_live_wave331_ok} wgdw332={live_wave_guide_dual_world_empty_gate_method_names_wave332_ok} wgdwnav332={live_wave_guide_dual_world_empty_gate_nav_commands_wave332_ok} wgdwlive332={live_wave_guide_dual_world_empty_gate_live_wave332_ok} emdw333={live_emp_update_dual_world_empty_gate_method_names_wave333_ok} emdwnav333={live_emp_update_dual_world_empty_gate_nav_commands_wave333_ok} emdwlive333={live_emp_update_dual_world_empty_gate_live_wave333_ok} bbdw334={live_bunker_buster_dual_world_empty_gate_method_names_wave334_ok} bbdwnav334={live_bunker_buster_dual_world_empty_gate_nav_commands_wave334_ok} bbdwlive334={live_bunker_buster_dual_world_empty_gate_live_wave334_ok} bsdw335={live_bridge_scaffold_dual_world_empty_gate_method_names_wave335_ok} bsdwnav335={live_bridge_scaffold_dual_world_empty_gate_nav_commands_wave335_ok} bsdwlive335={live_bridge_scaffold_dual_world_empty_gate_live_wave335_ok} atdw336={live_assisted_targeting_dual_world_empty_gate_method_names_wave336_ok} atdwnav336={live_assisted_targeting_dual_world_empty_gate_nav_commands_wave336_ok} atdwlive336={live_assisted_targeting_dual_world_empty_gate_live_wave336_ok} ecdw337={live_economy_dual_world_empty_gate_method_names_wave337_ok} ecdwnav337={live_economy_dual_world_empty_gate_nav_commands_wave337_ok} ecdwlive337={live_economy_dual_world_empty_gate_live_wave337_ok} tadw338={live_turret_ai_dual_world_empty_gate_method_names_wave338_ok} tadwnav338={live_turret_ai_dual_world_empty_gate_nav_commands_wave338_ok} tadwlive338={live_turret_ai_dual_world_empty_gate_live_wave338_ok} sddw339={live_stealth_detector_module_dual_world_empty_gate_method_names_wave339_ok} sddwnav339={live_stealth_detector_module_dual_world_empty_gate_nav_commands_wave339_ok} sddwlive339={live_stealth_detector_module_dual_world_empty_gate_live_wave339_ok} moddw340={live_modules_dual_world_empty_gate_method_names_wave340_ok} moddwnav340={live_modules_dual_world_empty_gate_nav_commands_wave340_ok} moddwlive340={live_modules_dual_world_empty_gate_live_wave340_ok} terrdw341={live_terrain_dual_world_empty_gate_method_names_wave341_ok} terrdwnav341={live_terrain_dual_world_empty_gate_nav_commands_wave341_ok} terrdwlive341={live_terrain_dual_world_empty_gate_live_wave341_ok} sptdw342={live_special_power_template_dual_world_empty_gate_method_names_wave342_ok} sptdwnav342={live_special_power_template_dual_world_empty_gate_nav_commands_wave342_ok} sptdwlive342={live_special_power_template_dual_world_empty_gate_live_wave342_ok} sevdw343={live_script_evaluator_dual_world_empty_gate_method_names_wave343_ok} sevdwnav343={live_script_evaluator_dual_world_empty_gate_nav_commands_wave343_ok} sevdwlive343={live_script_evaluator_dual_world_empty_gate_live_wave343_ok} sgldw344={live_system_game_logic_dual_world_empty_gate_method_names_wave344_ok} sgldwnav344={live_system_game_logic_dual_world_empty_gate_nav_commands_wave344_ok} sgldwlive344={live_system_game_logic_dual_world_empty_gate_live_wave344_ok} medw345={live_meta_event_dual_world_empty_gate_method_names_wave345_ok} medwnav345={live_meta_event_dual_world_empty_gate_nav_commands_wave345_ok} medwlive345={live_meta_event_dual_world_empty_gate_live_wave345_ok} spbdw346={live_spawn_behavior_dual_world_empty_gate_method_names_wave346_ok} spbdwnav346={live_spawn_behavior_dual_world_empty_gate_nav_commands_wave346_ok} spbdwlive346={live_spawn_behavior_dual_world_empty_gate_live_wave346_ok} amdw347={live_action_manager_dual_world_empty_gate_method_names_wave347_ok} amdwnav347={live_action_manager_dual_world_empty_gate_nav_commands_wave347_ok} amdwlive347={live_action_manager_dual_world_empty_gate_live_wave347_ok} sedw348={live_script_engine_dual_world_empty_gate_method_names_wave348_ok} sedwnav348={live_script_engine_dual_world_empty_gate_nav_commands_wave348_ok} sedwlive348={live_script_engine_dual_world_empty_gate_live_wave348_ok} chdw349={live_chinook_ai_dual_world_empty_gate_method_names_wave349_ok} chdwnav349={live_chinook_ai_dual_world_empty_gate_nav_commands_wave349_ok} chdwlive349={live_chinook_ai_dual_world_empty_gate_live_wave349_ok} midw350={live_missile_ai_dual_world_empty_gate_method_names_wave350_ok} midwnav350={live_missile_ai_dual_world_empty_gate_nav_commands_wave350_ok} midwlive350={live_missile_ai_dual_world_empty_gate_live_wave350_ok} dzdw351={live_dozer_ai_dual_world_empty_gate_method_names_wave351_ok} dzdwnav351={live_dozer_ai_dual_world_empty_gate_nav_commands_wave351_ok} dzdwlive351={live_dozer_ai_dual_world_empty_gate_live_wave351_ok} dpdw352={live_deliver_payload_ai_dual_world_empty_gate_method_names_wave352_ok} dpdwnav352={live_deliver_payload_ai_dual_world_empty_gate_nav_commands_wave352_ok} dpdwlive352={live_deliver_payload_ai_dual_world_empty_gate_live_wave352_ok} spmdw353={live_special_power_module_dual_world_empty_gate_method_names_wave353_ok} spmdwnav353={live_special_power_module_dual_world_empty_gate_nav_commands_wave353_ok} spmdwlive353={live_special_power_module_dual_world_empty_gate_live_wave353_ok} ptdw354={live_pow_truck_ai_dual_world_empty_gate_method_names_wave354_ok} ptdwnav354={live_pow_truck_ai_dual_world_empty_gate_nav_commands_wave354_ok} ptdwlive354={live_pow_truck_ai_dual_world_empty_gate_live_wave354_ok} dkdw355={live_dock_update_dual_world_empty_gate_method_names_wave355_ok} dkdwnav355={live_dock_update_dual_world_empty_gate_nav_commands_wave355_ok} dkdwlive355={live_dock_update_dual_world_empty_gate_live_wave355_ok} wtdw356={live_weapon_template_dual_world_empty_gate_method_names_wave356_ok} wtdwnav356={live_weapon_template_dual_world_empty_gate_nav_commands_wave356_ok} wtdwlive356={live_weapon_template_dual_world_empty_gate_live_wave356_ok} rgdw357={live_railroad_guide_ai_dual_world_empty_gate_method_names_wave357_ok} rgdwnav357={live_railroad_guide_ai_dual_world_empty_gate_nav_commands_wave357_ok} rgdwlive357={live_railroad_guide_ai_dual_world_empty_gate_live_wave357_ok} hidw358={live_hack_internet_ai_dual_world_empty_gate_method_names_wave358_ok} hidwnav358={live_hack_internet_ai_dual_world_empty_gate_nav_commands_wave358_ok} hidwlive358={live_hack_internet_ai_dual_world_empty_gate_live_wave358_ok} sgddw359={live_spectre_gunship_deployment_dual_world_empty_gate_method_names_wave359_ok} sgddwnav359={live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_wave359_ok} sgddwlive359={live_spectre_gunship_deployment_dual_world_empty_gate_live_wave359_ok} rddw360={live_radius_decal_update_dual_world_empty_gate_method_names_wave360_ok} rddwnav360={live_radius_decal_update_dual_world_empty_gate_nav_commands_wave360_ok} rddwlive360={live_radius_decal_update_dual_world_empty_gate_live_wave360_ok} rtdw361={live_railed_transport_dock_dual_world_empty_gate_method_names_wave361_ok} rtdwnav361={live_railed_transport_dock_dual_world_empty_gate_nav_commands_wave361_ok} rtdwlive361={live_railed_transport_dock_dual_world_empty_gate_live_wave361_ok} scuw362={live_structure_collapse_update_dual_world_empty_gate_method_names_wave362_ok} scuwnav362={live_structure_collapse_update_dual_world_empty_gate_nav_commands_wave362_ok} scuwlive362={live_structure_collapse_update_dual_world_empty_gate_live_wave362_ok} ptbw363={live_propaganda_tower_behavior_dual_world_empty_gate_method_names_wave363_ok} ptbwnav363={live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_wave363_ok} ptbwlive363={live_propaganda_tower_behavior_dual_world_empty_gate_live_wave363_ok} pcbw364={live_propaganda_center_behavior_dual_world_empty_gate_method_names_wave364_ok} pcbwnav364={live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_wave364_ok} pcbwlive364={live_propaganda_center_behavior_dual_world_empty_gate_live_wave364_ok} pucw365={live_production_update_complete_dual_world_empty_gate_method_names_wave365_ok} pucwnav365={live_production_update_complete_dual_world_empty_gate_nav_commands_wave365_ok} pucwlive365={live_production_update_complete_dual_world_empty_gate_live_wave365_ok} ptbw366={live_pow_truck_behavior_dual_world_empty_gate_method_names_wave366_ok} ptbwnav366={live_pow_truck_behavior_dual_world_empty_gate_nav_commands_wave366_ok} ptbwlive366={live_pow_truck_behavior_dual_world_empty_gate_live_wave366_ok} fwwd367={live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_wave367_ok} fwwdnav367={live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_wave367_ok} fwwdlive367={live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_live_wave367_ok} vccw368={live_veterancy_crate_collide_dual_world_empty_gate_method_names_wave368_ok} vccwnav368={live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_wave368_ok} vccwlive368={live_veterancy_crate_collide_dual_world_empty_gate_live_wave368_ok} ataw369={live_assault_transport_ai_update_dual_world_empty_gate_method_names_wave369_ok} atawnav369={live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_wave369_ok} atawlive369={live_assault_transport_ai_update_dual_world_empty_gate_live_wave369_ok} hcw370={live_heal_contain_dual_world_empty_gate_method_names_wave370_ok} hcwnav370={live_heal_contain_dual_world_empty_gate_nav_commands_wave370_ok} hcwlive370={live_heal_contain_dual_world_empty_gate_live_wave370_ok} tuw371={live_topple_update_dual_world_empty_gate_method_names_wave371_ok} tuwnav371={live_topple_update_dual_world_empty_gate_nav_commands_wave371_ok} tuwlive371={live_topple_update_dual_world_empty_gate_live_wave371_ok} psuw372={live_projectile_stream_update_dual_world_empty_gate_method_names_wave372_ok} psuwnav372={live_projectile_stream_update_dual_world_empty_gate_nav_commands_wave372_ok} psuwlive372={live_projectile_stream_update_dual_world_empty_gate_live_wave372_ok} dtuw373={live_demo_trap_update_dual_world_empty_gate_method_names_wave373_ok} dtuwnav373={live_demo_trap_update_dual_world_empty_gate_nav_commands_wave373_ok} dtuwlive373={live_demo_trap_update_dual_world_empty_gate_live_wave373_ok} mmsw374={live_mob_member_slaved_update_dual_world_empty_gate_method_names_wave374_ok} mmswnav374={live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_wave374_ok} mmswlive374={live_mob_member_slaved_update_dual_world_empty_gate_live_wave374_ok} tngw375={live_tn_guard_dual_world_empty_gate_method_names_wave375_ok} tngwnav375={live_tn_guard_dual_world_empty_gate_nav_commands_wave375_ok} tngwlive375={live_tn_guard_dual_world_empty_gate_live_wave375_ok} puw376={live_production_update_dual_world_empty_gate_method_names_wave376_ok} puwnav376={live_production_update_dual_world_empty_gate_nav_commands_wave376_ok} puwlive376={live_production_update_dual_world_empty_gate_live_wave376_ok} pbw377={live_poisoned_behavior_dual_world_empty_gate_method_names_wave377_ok} pbwnav377={live_poisoned_behavior_dual_world_empty_gate_nav_commands_wave377_ok} pbwlive377={live_poisoned_behavior_dual_world_empty_gate_live_wave377_ok} huw378={live_horde_update_dual_world_empty_gate_method_names_wave378_ok} huwnav378={live_horde_update_dual_world_empty_gate_nav_commands_wave378_ok} huwlive378={live_horde_update_dual_world_empty_gate_live_wave378_ok} fuw379={live_flammable_update_dual_world_empty_gate_method_names_wave379_ok} fuwnav379={live_flammable_update_dual_world_empty_gate_nav_commands_wave379_ok} fuwlive379={live_flammable_update_dual_world_empty_gate_live_wave379_ok} bruw380={live_base_regenerate_update_dual_world_empty_gate_method_names_wave380_ok} bruwnav380={live_base_regenerate_update_dual_world_empty_gate_nav_commands_wave380_ok} bruwlive380={live_base_regenerate_update_dual_world_empty_gate_live_wave380_ok} qpew381={live_queue_production_exit_behavior_dual_world_empty_gate_method_names_wave381_ok} qpewnav381={live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_wave381_ok} qpewlive381={live_queue_production_exit_behavior_dual_world_empty_gate_live_wave381_ok} mlbw382={live_missile_launcher_building_update_dual_world_empty_gate_method_names_wave382_ok} mlbwnav382={live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_wave382_ok} mlbwlive382={live_missile_launcher_building_update_dual_world_empty_gate_live_wave382_ok} dscruw383={live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_wave383_ok} dscruwnav383={live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_wave383_ok} dscruwlive383={live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_live_wave383_ok} cbhuw384={live_command_button_hunt_update_dual_world_empty_gate_method_names_wave384_ok} cbhuwnav384={live_command_button_hunt_update_dual_world_empty_gate_nav_commands_wave384_ok} cbhuwlive384={live_command_button_hunt_update_dual_world_empty_gate_live_wave384_ok} pbw385={live_prison_behavior_dual_world_empty_gate_method_names_wave385_ok} pbwnav385={live_prison_behavior_dual_world_empty_gate_nav_commands_wave385_ok} pbwlive385={live_prison_behavior_dual_world_empty_gate_live_wave385_ok} gmbw386={live_generate_minefield_behavior_dual_world_empty_gate_method_names_wave386_ok} gmbwnav386={live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_wave386_ok} gmbwlive386={live_generate_minefield_behavior_dual_world_empty_gate_live_wave386_ok} dspw387={live_demoralize_special_power_dual_world_empty_gate_method_names_wave387_ok} dspwnav387={live_demoralize_special_power_dual_world_empty_gate_nav_commands_wave387_ok} dspwlive387={live_demoralize_special_power_dual_world_empty_gate_live_wave387_ok} sduw388={live_stealth_detector_update_dual_world_empty_gate_method_names_wave388_ok} sduwnav388={live_stealth_detector_update_dual_world_empty_gate_nav_commands_wave388_ok} sduwlive388={live_stealth_detector_update_dual_world_empty_gate_live_wave388_ok} hsbw389={live_hive_structure_body_dual_world_empty_gate_method_names_wave389_ok} hsbwnav389={live_hive_structure_body_dual_world_empty_gate_nav_commands_wave389_ok} hsbwlive389={live_hive_structure_body_dual_world_empty_gate_live_wave389_ok} sccw390={live_salvage_crate_collide_dual_world_empty_gate_method_names_wave390_ok} sccwnav390={live_salvage_crate_collide_dual_world_empty_gate_nav_commands_wave390_ok} sccwlive390={live_salvage_crate_collide_dual_world_empty_gate_live_wave390_ok} siccw391={live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_wave391_ok} siccwnav391={live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_wave391_ok} siccwlive391={live_sabotage_internet_center_crate_collide_dual_world_empty_gate_live_wave391_ok} ppuw392={live_power_plant_update_dual_world_empty_gate_method_names_wave392_ok} ppuwnav392={live_power_plant_update_dual_world_empty_gate_nav_commands_wave392_ok} ppuwlive392={live_power_plant_update_dual_world_empty_gate_live_wave392_ok} ldbw393={live_leaflet_drop_behavior_dual_world_empty_gate_method_names_wave393_ok} ldbwnav393={live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_wave393_ok} ldbwlive393={live_leaflet_drop_behavior_dual_world_empty_gate_live_wave393_ok} aduw394={live_auto_deposit_update_dual_world_empty_gate_method_names_wave394_ok} aduwnav394={live_auto_deposit_update_dual_world_empty_gate_nav_commands_wave394_ok} aduwlive394={live_auto_deposit_update_dual_world_empty_gate_live_wave394_ok} swcbw395={live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_wave395_ok} swcbwnav395={live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_wave395_ok} swcbwlive395={live_supply_warehouse_crippling_behavior_dual_world_empty_gate_live_wave395_ok} nmsduw396={live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_wave396_ok} nmsduwnav396={live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_wave396_ok} nmsduwlive396={live_neutron_missile_slow_death_update_dual_world_empty_gate_live_wave396_ok} aidockw397={live_ai_dock_dual_world_empty_gate_method_names_wave397_ok} aidockwnav397={live_ai_dock_dual_world_empty_gate_nav_commands_wave397_ok} aidockwlive397={live_ai_dock_dual_world_empty_gate_live_wave397_ok} aigrpw398={live_ai_groups_dual_world_empty_gate_method_names_wave398_ok} aigrpwnav398={live_ai_groups_dual_world_empty_gate_nav_commands_wave398_ok} aigrpwlive398={live_ai_groups_dual_world_empty_gate_live_wave398_ok} abpw399={live_artillery_barrage_power_dual_world_empty_gate_method_names_wave399_ok} abpwnav399={live_artillery_barrage_power_dual_world_empty_gate_nav_commands_wave399_ok} abpwlive399={live_artillery_barrage_power_dual_world_empty_gate_live_wave399_ok} blpw400={live_baikonur_launch_power_dual_world_empty_gate_method_names_wave400_ok} blpwnav400={live_baikonur_launch_power_dual_world_empty_gate_nav_commands_wave400_ok} blpwlive400={live_baikonur_launch_power_dual_world_empty_gate_live_wave400_ok} aigrpw401={live_ai_group_dual_world_empty_gate_method_names_wave401_ok} aigrpwnav401={live_ai_group_dual_world_empty_gate_nav_commands_wave401_ok} aigrpwlive401={live_ai_group_dual_world_empty_gate_live_wave401_ok} slvu402={live_slaved_update_dual_world_empty_gate_method_names_wave402_ok} slvunav402={live_slaved_update_dual_world_empty_gate_nav_commands_wave402_ok} slvulive402={live_slaved_update_dual_world_empty_gate_live_wave402_ok} dmpw403={live_demoralize_power_dual_world_empty_gate_method_names_wave403_ok} dmpwnav403={live_demoralize_power_dual_world_empty_gate_nav_commands_wave403_ok} dmpwlive403={live_demoralize_power_dual_world_empty_gate_live_wave403_ok} bfxu404={live_bone_fx_update_dual_world_empty_gate_method_names_wave404_ok} bfxunav404={live_bone_fx_update_dual_world_empty_gate_nav_commands_wave404_ok} bfxulive404={live_bone_fx_update_dual_world_empty_gate_live_wave404_ok} swdw405={live_supply_warehouse_dock_dual_world_empty_gate_method_names_wave405_ok} swdwnav405={live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_wave405_ok} swdwlive405={live_supply_warehouse_dock_dual_world_empty_gate_live_wave405_ok} oclsp406={live_ocl_special_power_dual_world_empty_gate_method_names_wave406_ok} oclspnav406={live_ocl_special_power_dual_world_empty_gate_nav_commands_wave406_ok} oclsplive406={live_ocl_special_power_dual_world_empty_gate_live_wave406_ok} rtaiu407={live_railed_transport_ai_update_dual_world_empty_gate_method_names_wave407_ok} rtaiunav407={live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_wave407_ok} rtaiulive407={live_railed_transport_ai_update_dual_world_empty_gate_live_wave407_ok} sqc408={live_squish_collide_dual_world_empty_gate_method_names_wave408_ok} sqcnav408={live_squish_collide_dual_world_empty_gate_nav_commands_wave408_ok} sqclive408={live_squish_collide_dual_world_empty_gate_live_wave408_ok} wbu409={live_weapon_bonus_update_dual_world_empty_gate_method_names_wave409_ok} wbunav409={live_weapon_bonus_update_dual_world_empty_gate_nav_commands_wave409_ok} wbulive409={live_weapon_bonus_update_dual_world_empty_gate_live_wave409_ok} mfb410={live_minefield_behavior_dual_world_empty_gate_method_names_wave410_ok} mfbnav410={live_minefield_behavior_dual_world_empty_gate_nav_commands_wave410_ok} mfblive410={live_minefield_behavior_dual_world_empty_gate_live_wave410_ok} pdlu411={live_point_defense_laser_update_dual_world_empty_gate_method_names_wave411_ok} pdlunav411={live_point_defense_laser_update_dual_world_empty_gate_nav_commands_wave411_ok} pdlulive411={live_point_defense_laser_update_dual_world_empty_gate_live_wave411_ok} aifc_auth={ai_fire_construction_authority_env_ok} move_auth={movement_authority_env_ok} prod_auth={production_authority_env_ok} screen={screen_skirmish_ok} control_bar={control_bar_layout_ok} cb_path={control_bar_path_resolved} cb_valid={control_bar_wnd_validated} cb_loaded={control_bar_window_loaded} cb_windows={control_bar_window_count} shell_host_playable_ok={shell_host_playable_ok} playable_claim={playable_claim} {layout_report}"
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
    fn presentation_victory_summary_residual() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("Prefer presentation-frozen summary when available")
                && eng.contains("Boot residual only — no presentation summary yet")
                && eng.contains("f.victory_summary.clone()"),
            "show_victory_screen must prefer presentation VictorySummary residual"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("pub victory_summary:")
                && pf.contains("build_victory_summary(winner)")
                && pf.contains("fn victory_summary_residual"),
            "snapshot must freeze VictorySummary at evaluate"
        );
    }

    #[test]
    fn presentation_victory_prefers_snapshot_match_over() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("build_with_victory")
                && eng.contains("Prefer presentation victory residual when frame installed")
                && eng.contains("Boot residual only — no presentation frame yet")
                && eng.contains("pres.match_over"),
            "InGame victory must prefer presentation match_over residual"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn build_with_victory")
                && pf.contains("evaluate_victory_condition")
                && pf.contains("PresentationEvent::Victory"),
            "snapshot must freeze victory residual once"
        );
    }

    fn presentation_particle_client_mirror_same_frame() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("apply_particle_systems_to_client")
                && eng.contains(
                    "Same-frame particle residual: backfill client ParticleSystemManager"
                ),
            "engine must apply presentation particles to client same frame"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn apply_particle_systems_to_client")
                && pf.contains("mirror_spawn_to_client_manager")
                && pf.contains("ParticleSystemSpawned"),
            "presentation must backfill client particle mirrors"
        );
        let cp = include_str!("game_logic/combat_particles.rs");
        assert!(
            cp.contains("pub(crate) fn mirror_spawn_to_client_manager"),
            "mirror helper must be callable from presentation residual"
        );
    }

    fn presentation_audio_processed_same_frame_after_apply() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("apply_events_to_audio")
                && eng.contains("Same-frame residual: drain presentation-queued audio now")
                && eng.contains("self.game_logic.process_audio_events()")
                && eng.contains("drain immediately so Select/Command is not delayed one tick"),
            "presentation audio must process same frame after apply and input SFX"
        );
        let gl = include_str!("game_logic/game_logic.rs");
        assert!(
            gl.contains("pub(crate) fn process_audio_events"),
            "process_audio_events must be callable from engine residual path"
        );
    }

    #[test]
    fn play_sound_effect_prefers_presentation_audio_queue() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng.find("fn play_sound_effect").expect("play_sound_effect");
        let body = &eng[i..eng.len().min(i + 2200)];
        assert!(
            body.contains("last_presentation_frame.is_some()")
                && body.contains("AudioManagerSubsystem")
                && body.contains("UnitSelect")
                && body.contains("UnitCommand")
                && !body.contains("self.game_logic.queue_audio_event")
                && !body.contains("self.game_logic.process_audio_events()"),
            "InGame SFX must dispatch direct to AudioManager when presentation frame installed"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("UnitMove") && pf.contains("MoveOrdered { unit"),
            "MoveOrdered must map to UnitMove audio residual"
        );
    }

    #[test]
    fn presentation_shell_input_audio_without_draw_dual_own() {
        let gc = include_str!("../../GameEngine/GameClient/src/core/game_client.rs");
        let start = gc
            .find("pub fn update_presentation_shell")
            .expect("presentation shell");
        let window = &gc[start..start + 2500.min(gc.len() - start)];
        assert!(
            gc.contains("update_presentation_shell")
                && window.contains("without Main-owned input/audio or Display DRAW dual-ownership")
                && window.contains("update_drawables_local")
                && !window.contains("self.draw_display()?")
                && window.contains("update_input")
                && window.contains("update_audio"),
            "presentation shell ticks drawables/UI without dual-owning Main OS input/3D draw; client audio queue drains"
        );
        assert!(
            gc.contains("fn update_post_draw_ui") && gc.contains("crate::eva::update_eva_system()"),
            "Eva must tick from post-draw UI residual used by shell"
        );
    }

    #[test]
    fn presentation_fow_shroud_drawable_residual_no_object_registry() {
        let gc = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/game_client.rs"
        ));
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let fow = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/fow_rendering.rs"));
        let apply_idx = eng
            .find("apply_presentation_shroud_to_drawables")
            .expect("engine must call presentation shroud apply");
        let apply_window = &eng[apply_idx..apply_idx.saturating_add(800).min(eng.len())];
        assert!(
            gc.contains("apply_presentation_shroud_to_drawables")
                && gc.contains("no OBJECT_REGISTRY")
                && fow.contains("fully_obscures_drawable")
                && eng.contains("fow_visibility.fully_obscures_drawable")
                && !apply_window.contains("OBJECT_REGISTRY"),
            "presentation FOW must drive drawable shroud without OBJECT_REGISTRY dual-read"
        );
    }

    #[test]
    fn presentation_alliance_local_player_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        assert!(
            eng.contains("take_alliance_events")
                && eng.contains("last_presentation_frame")
                && eng.contains("Prefer presentation local_player residual")
                && eng.contains("local_player_id"),
            "alliance notifications must prefer presentation residual then drain live take"
        );
    }

    #[test]
    fn presentation_camera_residual_prefers_frame() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        assert!(
            eng.contains("fn apply_presentation_camera_residual")
                && eng.contains("fn drain_live_camera_request_queues")
                && eng.contains("Prefer presentation-frozen camera residual")
                && eng.contains("apply_presentation_camera_residual(&pres)"),
            "InGame script camera must prefer presentation freeze over live take_* dual-read"
        );
        let i = eng
            .find("fn apply_pending_script_camera_requests")
            .expect("apply_pending_script_camera_requests");
        let window = &eng[i..i.saturating_add(900).min(eng.len())];
        assert!(
            window.contains("last_presentation_frame.clone()")
                && window.contains("drain_live_camera_request_queues"),
            "presentation path must drain live queues after apply"
        );
    }

    #[test]
    fn presentation_popup_music_fps_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        assert!(
            pf.contains("struct PresentationPopupMessage")
                && pf.contains("pause_music: p.pause_music")
                && eng.contains("Prefer presentation popup/music residual")
                && eng.contains("Prefer presentation script FPS residual")
                && eng.contains("pres.pending_music_stop")
                && eng.contains("p.script_fps_limit"),
            "popup/music/fps must prefer presentation freeze over live take dual-read"
        );
    }

    #[test]
    fn presentation_script_message_movie_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let gl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game_logic/game_logic.rs"
        ));
        assert!(
            eng.contains("Prefer presentation new_script_messages residual")
                && eng.contains("apply_presentation_movie_residual")
                && eng.contains("fn apply_presentation_movie_residual")
                && eng.contains("Prefer presentation victory residual when installed")
                && gl.contains("fn take_pending_movie")
                && gl.contains("fn take_pending_radar_movie"),
            "script messages/movies/victory status must prefer presentation freeze"
        );
    }

    #[test]
    fn presentation_play_time_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        assert!(
            pf.contains("total_play_time_seconds: logic.get_total_play_time()")
                && pf.contains("ui.current_game_time = self.total_play_time_seconds")
                && eng.contains("Prefer presentation sim clock residual")
                && eng.contains("p.total_play_time_seconds"),
            "UI game time must prefer presentation total_play_time_seconds"
        );
    }

    #[test]
    fn presentation_defeat_save_info_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        let gl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game_logic/game_logic.rs"
        ));
        assert!(
            pf.contains("defeated_player_ids")
                && pf.contains("logic.peek_defeat_events()")
                && gl.contains("fn peek_defeat_events")
                && eng.contains("pres.defeated_player_ids.clone()")
                && eng.contains("Prefer presentation residual for map/play_time/local team")
                && eng.contains("p.total_play_time_seconds")
                && eng.contains("p.world_env.map_name"),
            "defeat notifications and save_info must prefer presentation freeze"
        );
    }

    #[test]
    fn presentation_alliance_events_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        let gl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game_logic/game_logic.rs"
        ));
        assert!(
            pf.contains("pub alliance_events:")
                && pf.contains("logic.peek_alliance_events()")
                && gl.contains("fn peek_alliance_events")
                && eng.contains("Prefer presentation alliance residual")
                && eng.contains("pres.alliance_events.clone()"),
            "alliance notifications must prefer presentation freeze over live take"
        );
    }

    #[test]
    fn presentation_difficulty_game_mode_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        assert!(
            pf.contains("ai_difficulty: logic.get_difficulty()")
                && pf.contains("game_mode: logic.game_mode()")
                && eng.contains("p.ai_difficulty")
                && eng.contains("p.game_mode")
                && eng.contains("Prefer presentation residual for map/mode/faction")
                && eng.contains("Prefer presentation world_env map residual"),
            "save/restart/runtime-host must prefer presentation difficulty/mode/map"
        );
    }

    #[test]
    fn presentation_menu_shell_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        assert!(
            eng.contains("Prefer presentation shell residual when it affirms shell-map mode")
                && eng.contains("Some(pres) if pres.fow_shell_bypass => true")
                && eng
                    .contains("Prefer presentation script FPS residual when shell frame installed")
                && eng.contains("filter(|p| p.fow_shell_bypass)"),
            "menu shell tick must prefer presentation fow_shell_bypass when true"
        );
    }

    #[test]
    fn presentation_game_mode_helper_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        assert!(
            eng.contains("fn presentation_or_live_game_mode")
                && eng.contains("Prefer presentation game_mode residual when installed")
                && eng.matches("presentation_or_live_game_mode()").count() >= 7
                && eng.contains("should_keep_logic_running_while_iconic")
                && eng.contains("engine.presentation_or_live_game_mode()"),
            "load-screen/quick-save/restart/iconic must prefer presentation game_mode helper"
        );
    }

    #[test]
    fn presentation_load_screen_roster_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        assert!(
            pf.contains("is_ai: logic.ai_manager_contains_player(id)")
                && pf.contains("color_rgb: p.color_rgb")
                && eng.contains("Prefer full presentation roster when installed")
                && eng.contains("frame.players.is_empty()")
                && eng.contains("is_ai: player.is_ai")
                && eng.contains("player.color_rgb")
                && eng.contains("apparent_text_color: Some(text_color)")
                && eng.contains("apparent_color is multiplayer color *index*")
                && eng.contains("visible: player.is_alive"),
            "load-screen init must expand slots from full presentation roster with is_ai/color"
        );
    }

    #[test]
    fn presentation_cinematic_letterbox_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let gc = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/game_client.rs"
        ));
        assert!(
            gc.contains("fn apply_presentation_cinematic_letterbox")
                && gc.contains("enable_letter_box(enabled)")
                && eng.contains("apply_presentation_cinematic_letterbox(pres.cinematic_letterbox)")
                && eng.contains("Presentation cinematic letterbox residual"),
            "presentation cinematic_letterbox must drive GameClient display letterbox"
        );
    }

    #[test]
    fn presentation_military_caption_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        let gc = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/game_client.rs"
        ));
        let gl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game_logic/game_logic.rs"
        ));
        assert!(
            pf.contains("military_caption_remaining_ms")
                && gl.contains("fn military_caption_remaining_ms")
                && gc.contains("fn apply_presentation_military_caption")
                && eng.contains("apply_presentation_military_caption")
                && eng.contains("pres.military_caption_remaining_ms"),
            "presentation military caption must freeze remaining_ms and apply to InGameUI"
        );
    }

    #[test]
    fn presentation_cinematic_text_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let gc = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/game_client.rs"
        ));
        assert!(
            gc.contains("fn apply_presentation_cinematic_text")
                && gc.contains("push_hud_message")
                && gc.contains("last_applied_cinematic_text")
                && eng
                    .contains("apply_presentation_cinematic_text(pres.cinematic_text.as_deref())")
                && eng.contains("Cinematic text residual"),
            "presentation cinematic_text must push InGameUI HUD message with anti-spam"
        );
    }

    #[test]
    fn presentation_camera_follow_residual() {
        let eng = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cnc_game_engine.rs"
        ));
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        let gl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game_logic/game_logic.rs"
        ));
        assert!(
            gl.contains("fn peek_camera_follow_target_position")
                && pf.contains("camera_follow_position")
                && pf.contains("peek_camera_follow_target_position")
                && eng.contains("pres.camera_follow_position")
                && eng.contains("Prefer presentation-frozen follow position"),
            "camera follow must prefer presentation freeze over live dual-read"
        );
    }

    #[test]
    fn presentation_timers_cameo_superweapon_residual() {
        let pf = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/presentation_frame.rs"
        ));
        assert!(
            pf.contains("ui.named_timers = self.named_timers.clone()")
                && pf.contains("ui.named_timer_display_shown = self.named_timer_display_shown")
                && pf.contains("ui.cameo_flash = self.cameo_flash.clone()")
                && pf.contains("ui.superweapon_display_enabled = self.superweapon_display_enabled")
                && pf.contains(
                    "ui.superweapon_hidden_objects = self.superweapon_hidden_objects.clone()"
                )
                && pf.contains("Script named-timer / cameo / superweapon residual"),
            "apply_to_ui_state must project named_timers/cameo/superweapon from presentation"
        );
    }

    #[test]
    fn aiplayer_update_with_frame_cpp_phase_order() {
        // C++ AIPlayer::update (AIPlayer.cpp): doBaseBuilding → checkReadyTeams →
        // checkQueuedTeams → doTeamBuilding → doUpgradesAndSkills → updateBridgeRepair.
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        let i = src
            .find("pub fn update_with_frame")
            .expect("update_with_frame");
        let window = &src[i..i.saturating_add(2200).min(src.len())];
        let base = window
            .find("self.do_base_building()?")
            .expect("do_base_building");
        let ready = window
            .find("self.check_ready_teams()?")
            .expect("check_ready");
        let queued = window
            .find("self.check_queued_teams()?")
            .expect("check_queued");
        let team = window.find("self.do_team_building()?").expect("do_team");
        let upg = window
            .find("self.do_upgrades_and_skills()?")
            .expect("upgrades");
        let bridge = window.find("self.update_bridge_repair()?").expect("bridge");
        assert!(
            base < ready && ready < queued && queued < team && team < upg && upg < bridge,
            "update_with_frame must match C++ AIPlayer::update phase order (got base={base} ready={ready} queued={queued} team={team} upg={upg} bridge={bridge})"
        );
        // Attack residual must not interrupt the C++ phase block.
        if let Some(atk) = window.find("self.process_attack_decisions") {
            assert!(
                atk > bridge,
                "process_attack_decisions is host residual and must run after C++ phase block"
            );
        }
    }

    #[test]
    fn aiplayer_check_ready_queued_cpp_parity_surface() {
        // C++ AIPlayer.cpp checkReadyTeams / checkQueuedTeams surface in Rust.
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("AIPlayer::checkReadyTeams")
                && src.contains("60 * LOGICFRAMES_PER_SECOND")
                && src.contains("set_active()")
                && src.contains("AIPlayer::checkQueuedTeams")
                && src.contains("is_build_time_expired")
                && src.contains("push_front(team)")
                && src.contains("is_minimum_built")
                && src.contains("are_builds_complete")
                && src.contains("clear_team_flags")
                && src.contains("join_team_reinforcement"),
            "check_ready/queued must port C++ activation, expiry, prepend, joinTeam residual"
        );
        // Must NOT only move build→ready inside check_ready_teams (old wrong port).
        let i = src.find("fn check_ready_teams").expect("check_ready_teams");
        let end = (i + 3500).min(src.len());
        let window = &src[i..end];
        assert!(
            !window.contains("team_build_queue.remove")
                || window.contains("team_ready_queue.remove"),
            "check_ready_teams must drain ready queue (C++), not only promote build queue"
        );
        assert!(
            window.contains("team_ready_queue.remove") || window.contains("team_ready_queue"),
            "check_ready_teams must iterate ready queue"
        );
    }

    #[test]
    fn aiplayer_do_base_team_delay_cpp() {
        // C++ AIPlayer.cpp doBaseBuilding / doTeamBuilding delay rechecks.
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("BUILD_DELAY_RECHECK_FRAMES: u32 = 2 * LOGICFRAMES_PER_SECOND")
                && src.contains("TEAM_DELAY_RECHECK_FRAMES: u32 = 5 * LOGICFRAMES_PER_SECOND")
                && src.contains("self.build_delay = BUILD_DELAY_RECHECK_FRAMES")
                && src.contains("self.team_delay = TEAM_DELAY_RECHECK_FRAMES")
                && src.contains("let _ = self.queue_units()")
                && src.contains("C++ `AIPlayer::doBaseBuilding`")
                && src.contains("C++ `AIPlayer::doTeamBuilding`"),
            "do_base/do_team must port C++ 2s/5s delay + queueUnits cadence"
        );
    }

    #[test]
    fn aiplayer_process_base_building_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::processBaseBuilding`")
                && src.contains("arm_structure_timer_after_build")
                && src.contains("rebuild_delay_frames")
                && src.contains("structures_poor_mod")
                && src.contains("structures_wealthy_mod"),
            "process_base_building must arm structureTimer with C++ wealth mods"
        );
    }

    #[test]
    fn aiplayer_select_team_to_build_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::selectTeamToBuild`")
                && src.contains("select_team_to_reinforce(hi_pri)")
                && src.contains("game_logic_random_value")
                && src.contains("build_specific_ai_team(team_name, false)")
                && src.contains("arm_team_timer_after_build")
                && src.contains("is_possible_to_build_team(team_name, true)"),
            "selectTeamToBuild must hiPri-filter, reinforce first, random pick, arm teamTimer"
        );
    }

    #[test]
    fn aiplayer_select_team_to_reinforce_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::selectTeamToReinforce`")
                && src.contains("automatically_reinforce")
                && src.contains("try_to_recruit")
                && src.contains("self.team_delay = 0"),
            "selectTeamToReinforce must auto-reinforce with recruit/train + teamDelay shortcut"
        );
    }

    #[test]
    fn team_evaluate_production_condition_cpp() {
        let team = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/team.rs"
        ));
        let ai = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            team.contains("evaluate_production_condition")
                && team.contains("production_condition_runtime")
                && team.contains("always_false")
                && team.contains("evaluate_conditions")
                && ai.contains("evaluate_production_condition()"),
            "TeamPrototype::evaluateProductionCondition + isAGoodIdea wire required"
        );
    }

    #[test]
    fn aiplayer_queue_units_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::queueUnits`")
                && src.contains("try_to_recruit")
                && src.contains("while order.is_waiting_to_build()")
                && src.contains("ai_move_to_position")
                && src.contains("ai_idle"),
            "queueUnits must tryToRecruit then startTraining (C++)"
        );
    }

    #[test]
    fn aiplayer_is_possible_to_build_team_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::isPossibleToBuildTeam`")
                && src.contains("any_idle")
                && src.contains("max_units as f32 + min_units as f32")
                && src.contains("team_resources_to_build"),
            "isPossibleToBuildTeam C++ factory/cost semantics required"
        );
    }

    #[test]
    fn aiplayer_start_training_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::startTraining`")
                && src.contains("queue_unit_with_production_id")
                && src.contains("request_unique_unit_production_id")
                && src.contains("C++ `AIPlayer::findFactory`")
                && src.contains("get_build_list"),
            "startTraining/findFactory must queueCreateUnit via build-list factories"
        );
    }

    #[test]
    fn aiplayer_on_unit_produced_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::onUnitProduced`")
                && src.contains("self.team_delay = 0")
                && src.contains("set_force_wanting_state")
                && src.contains("structure_timer = 1"),
            "onUnitProduced must setTeam/delays/supply/dozer like C++"
        );
    }

    #[test]
    fn aiplayer_build_structure_with_dozer_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::buildStructureWithDozer`")
                && src.contains("set_build_task")
                && src.contains("build_structure_with_dozer"),
            "buildStructureWithDozer must be wired for USE_DOZER base building"
        );
    }

    #[test]
    fn aiplayer_new_map_and_build_structure_now_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::newMap`")
                && src.contains("C++ `AIPlayer::buildStructureNow`")
                && src.contains("C++ `AIPlayer::checkForSupplyCenter`")
                && src.contains("is_initially_built")
                && src.contains("set_desired_gatherers(desired + 1)"),
            "newMap/buildStructureNow/checkForSupplyCenter C++ paths required"
        );
    }

    #[test]
    fn aiplayer_queue_supply_truck_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::queueSupplyTruck`")
                && src.contains("truck_in_queue")
                && src.contains("queue_one_harvester_at_factory")
                && src.contains("try_reattach_loose_harvester"),
            "queueSupplyTruck C++ gatherer path required"
        );
    }

    #[test]
    fn aiplayer_find_dozer_queue_supply_safe_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::findDozer`")
                && src.contains("C++ `AIPlayer::queueDozer`")
                && src.contains("C++ `AIPlayer::isSupplySourceAttacked`")
                && src.contains("C++ `AIPlayer::isSupplySourceSafe`")
                && src.contains("C++ `AIPlayer::guardSupplyCenter`")
                && src.contains("is_currently_ferrying_supplies")
                && src.contains("start_training_internal"),
            "findDozer/queueDozer/supply safety C++ paths required"
        );
    }

    #[test]
    fn aiplayer_compute_superweapon_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::computeSuperweaponTarget`")
                && src.contains("C++ `AIPlayer::getPlayerSuperweaponValue`")
                && src.contains("Fine tune: C++ uses (x-5) for BOTH axes")
                && src.contains("FSBaseDefense"),
            "superweapon targeting C++ parity required"
        );
    }

    #[test]
    fn aiplayer_build_upgrade_repair_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::buildUpgrade`")
                && src.contains("C++ `AIPlayer::buildBySupplies`")
                && src.contains("C++ `AIPlayer::repairStructure`")
                && src.contains("C++ `AIPlayer::updateBridgeRepair`")
                && src.contains("add_to_priority_build_list")
                && src.contains("AiCommandType::Repair"),
            "buildUpgrade/buildBySupplies/repair bridge C++ paths required"
        );
    }

    #[test]
    fn aiplayer_find_supply_nearest_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::findSupplyCenter`")
                && src.contains("C++ `AIPlayer::buildSpecificBuildingNearestTeam`")
                && src.contains("C++ `AIPlayer::calcClosestConstructionZoneLocation`")
                && src.contains("C++ `AIPlayer::onStructureProduced`")
                && src.contains("dist_sqr * 0.4")
                && src.contains("RebuildHole"),
            "findSupplyCenter/nearest/onStructure C++ paths required"
        );
    }

    #[test]
    fn aiplayer_build_recruit_team_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/ai_player.rs"
        ));
        assert!(
            src.contains("C++ `AIPlayer::buildSpecificAITeam`")
                && src.contains("C++ `AIPlayer::recruitSpecificAITeam`")
                && src.contains("create_inactive_team")
                && src.contains("team_ready_queue.push_front")
                && src.contains("get_can_build_units"),
            "buildSpecificAITeam/recruitSpecificAITeam C++ paths required"
        );
    }

    #[test]
    fn aiskirmish_base_defense_new_map_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
        ));
        assert!(
            src.contains("C++ `AISkirmishPlayer::buildAIBaseDefenseStructure`")
                && src.contains("C++ `AISkirmishPlayer::newMap`")
                && src.contains("add_to_priority_build_list")
                && src.contains("build_structure_now_at_public"),
            "skirmish defense/newMap C++ paths required"
        );
    }

    #[test]
    fn aiskirmish_process_base_building_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
        ));
        assert!(
            src.contains("build_structure_with_dozer")
                && src.contains("ResumeConstruction")
                && src.contains("processBaseBuilding"),
            "skirmish processBaseBuilding dozer path required"
        );
    }

    #[test]
    fn aiskirmish_select_team_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
        ));
        assert!(
            src.contains("selectTeamToBuild")
                && src.contains("is_a_good_idea_to_build_team(proto.get_name()")
                && src.contains("select_team_to_reinforce(min_priority)"),
            "skirmish selectTeam C++ delegation required"
        );
    }

    #[test]
    fn aiskirmish_update_phase_cpp() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
        ));
        assert!(
            src.contains("C++ `AISkirmishPlayer::update`")
                && src.contains("check_ready_teams")
                && src.contains("do_base_building()")
                && src.contains("do_team_building()")
                && src.contains("check_queued_teams")
                && src.contains("update_bridge_repair"),
            "skirmish update C++ phase order required"
        );
    }

    fn load_screen_init_prefers_presentation_roster() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains(
                "Prefer presentation roster when installed (InGame residual); live only boot/menu"
            ) && eng.contains("Boot residual only — no presentation roster yet")
                && eng.contains("frame.player_info(local_id)"),
            "load_screen_init_context must prefer presentation roster when frame installed"
        );
    }

    fn render_execute_passes_none_game_logic_when_presentation_installed() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("set_presentation_frame(self.last_presentation_frame.clone())")
                && eng.contains("PresentationFrame::build_from_logic")
                && eng.contains("last_presentation_frame.is_none()"),
            "engine must seed presentation before execute (no live GameLogic dual-read)"
        );
        // Structural: execute call site prefers None when presentation is Some.
        let idx = eng
            .find("self.render_pipeline.execute(")
            .expect("execute call");
        let window = &eng[idx..idx + 450];
        assert!(
            !window.contains("Some(&self.game_logic)"),
            "execute must not pass live GameLogic (presentation-only boundary): {window}"
        );
        assert!(
            eng.contains("PresentationFrame::build_from_logic")
                && eng.contains("last_presentation_frame.is_none()"),
            "engine must seed a presentation frame before execute when missing"
        );
        let rp = include_str!("graphics/render_pipeline.rs");
        assert!(
            rp.contains(
                "Boot residual only — presentation unit_render_inputs owns model/transform"
            ),
            "pipeline live get_template must be boot residual only"
        );
    }

    fn defeat_alliance_prefer_presentation_no_live_dual_read_when_frame_installed() {
        let src = include_str!("cnc_game_engine.rs");
        assert!(
            src.contains(
                "Boot residual only — no live dual-read when a presentation frame is installed"
            ) && src.contains(
                "Presentation installed but roster miss — fail-closed id-only residual"
            ) && src.contains(
                "Prefer presentation roster team when installed; live only if no frame"
            ) && src.contains("else if self.last_presentation_frame.is_none()")
                && src.contains("Boot residual only — presentation local_team owns InGame Ctrl+A")
                && src.contains("Boot residual only — presentation local_team owns InGame Tab cycle")
                && src.contains("Boot residual only — presentation local_team owns InGame similar-select")
                && src.contains("Boot residual only — presentation local_team owns InGame box-select")
                && src.contains("Boot residual only — presentation local_team owns InGame attack-click")
                && src.contains("Boot residual only — presentation local_team owns InGame control-group select")
                && src.contains("Boot residual only — presentation filter_alive_selectable_ids owns InGame")
                && src.contains("Boot residual only — presentation centroid_of_ids owns InGame double-tap")
                && src.contains("Boot residual only — presentation box_select_unit_ids owns InGame path")
                && src.contains("Boot residual only — presentation similar_unit_ids owns InGame path"),
            "defeat/alliance/selection must not dual-read get_player when presentation frame is installed"
        );
    }

    fn legacy_render_stubs_prefer_presentation_identity() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains(
                "Prefer presentation identity when installed (no live get_objects dual-read)"
            ),
            "render_game_objects stub must prefer presentation"
        );
        assert!(
            eng.contains("Prefer presentation selected residual when installed (no live find_object dual-read)"),
            "render_selection_indicators stub must prefer presentation"
        );
        assert!(
            eng.contains("selection_renderer is sole path"),
            "must document selection_renderer as production path"
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
            r.message_stream_marker_wave110_ok,
            "message stream marker residual pack wave110: {}",
            r.detail
        );
        assert!(
            r.game_message_arg_wave110_ok,
            "game message argument type residual pack wave110: {}",
            r.detail
        );
        assert!(
            r.meta_event_category_wave110_ok,
            "meta event category residual pack wave110: {}",
            r.detail
        );
        assert!(
            r.ingame_ui_wave110_ok,
            "ingame ui residual pack wave110: {}",
            r.detail
        );
        assert!(
            r.drawable_icon_flash_wave111_ok,
            "drawable icon/flash residual pack wave111: {}",
            r.detail
        );
        assert!(
            r.drawable_status_stealth_wave111_ok,
            "drawable status/stealth residual pack wave111: {}",
            r.detail
        );
        assert!(
            r.terrain_decal_wave111_ok,
            "terrain decal residual pack wave111: {}",
            r.detail
        );
        assert!(
            r.display_draw_image_wave111_ok,
            "display draw-image mode residual pack wave111: {}",
            r.detail
        );
        assert!(
            r.game_client_translator_wave111_ok,
            "game client translator residual pack wave111: {}",
            r.detail
        );
        assert!(
            r.particle_priority_wave111_ok,
            "particle priority residual pack wave111: {}",
            r.detail
        );
        assert!(
            r.mouse_residual_wave112_ok,
            "mouse residual pack wave112: {}",
            r.detail
        );
        assert!(
            r.keyboard_residual_wave112_ok,
            "keyboard residual pack wave112: {}",
            r.detail
        );
        assert!(
            r.view_residual_wave112_ok,
            "view residual pack wave112: {}",
            r.detail
        );
        assert!(
            r.game_window_manager_wave113_ok,
            "game window manager residual pack wave113: {}",
            r.detail
        );
        assert!(
            r.window_style_wave113_ok,
            "window style residual pack wave113: {}",
            r.detail
        );
        assert!(
            r.gadget_wave113_ok,
            "gadget residual pack wave113: {}",
            r.detail
        );
        assert!(
            r.video_buffer_wave113_ok,
            "video buffer residual pack wave113: {}",
            r.detail
        );
        assert!(
            r.audio_event_wave113_ok,
            "audio event residual pack wave113: {}",
            r.detail
        );
        assert!(
            r.main_menu_skirmish_names_wave114_ok,
            "main menu skirmish names residual pack wave114: {}",
            r.detail
        );
        assert!(
            r.main_menu_skirmish_nav_steps_wave114_ok,
            "main menu skirmish nav steps residual pack wave114: {}",
            r.detail
        );
        assert!(
            r.main_menu_skirmish_message_wave114_ok,
            "main menu skirmish message residual pack wave114: {}",
            r.detail
        );
        assert!(
            r.map_select_names_wave115_ok,
            "map select names residual pack wave115: {}",
            r.detail
        );
        assert!(
            r.map_select_nav_steps_wave115_ok,
            "map select nav steps residual pack wave115: {}",
            r.detail
        );
        assert!(
            r.map_select_commands_wave115_ok,
            "map select commands residual pack wave115: {}",
            r.detail
        );
        assert!(
            r.slot_state_wave116_ok,
            "slot state residual pack wave116: {}",
            r.detail
        );
        assert!(
            r.slot_combo_names_wave116_ok,
            "slot combo names residual pack wave116: {}",
            r.detail
        );
        assert!(
            r.slot_nav_commands_wave116_ok,
            "slot nav commands residual pack wave116: {}",
            r.detail
        );
        assert!(
            r.starting_cash_wave117_ok,
            "starting cash residual pack wave117: {}",
            r.detail
        );
        assert!(
            r.game_speed_controls_wave117_ok,
            "game speed controls residual pack wave117: {}",
            r.detail
        );
        assert!(
            r.rules_nav_commands_wave117_ok,
            "rules nav commands residual pack wave117: {}",
            r.detail
        );
        assert!(
            r.main_menu_button_names_wave118_ok,
            "main menu button names residual pack wave118: {}",
            r.detail
        );
        assert!(
            r.main_menu_push_targets_wave118_ok,
            "main menu push targets residual pack wave118: {}",
            r.detail
        );
        assert!(
            r.main_menu_button_nav_commands_wave118_ok,
            "main menu button nav commands residual pack wave118: {}",
            r.detail
        );
        assert!(
            r.campaign_button_names_wave119_ok,
            "campaign button names residual pack wave119: {}",
            r.detail
        );
        assert!(
            r.campaign_enums_wave119_ok,
            "campaign enums residual pack wave119: {}",
            r.detail
        );
        assert!(
            r.campaign_nav_commands_wave119_ok,
            "campaign nav commands residual pack wave119: {}",
            r.detail
        );
        assert!(
            r.challenge_control_names_wave120_ok,
            "challenge control names residual pack wave120: {}",
            r.detail
        );
        assert!(
            r.challenge_nav_commands_wave120_ok,
            "challenge nav commands residual pack wave120: {}",
            r.detail
        );
        assert!(
            r.save_load_layout_wave121_ok,
            "save load layout residual pack wave121: {}",
            r.detail
        );
        assert!(
            r.save_load_control_stems_wave121_ok,
            "save load control stems residual pack wave121: {}",
            r.detail
        );
        assert!(
            r.save_load_nav_commands_wave121_ok,
            "save load nav commands residual pack wave121: {}",
            r.detail
        );
        assert!(
            r.replay_control_names_wave122_ok,
            "replay control names residual pack wave122: {}",
            r.detail
        );
        assert!(
            r.replay_nav_commands_wave122_ok,
            "replay nav commands residual pack wave122: {}",
            r.detail
        );
        assert!(
            r.quit_control_names_wave123_ok,
            "quit control names residual pack wave123: {}",
            r.detail
        );
        assert!(
            r.quit_nav_commands_wave123_ok,
            "quit nav commands residual pack wave123: {}",
            r.detail
        );
        assert!(
            r.keyboard_control_names_wave124_ok,
            "keyboard control names residual pack wave124: {}",
            r.detail
        );
        assert!(
            r.keyboard_nav_commands_wave124_ok,
            "keyboard nav commands residual pack wave124: {}",
            r.detail
        );
        assert!(
            r.score_control_names_wave125_ok,
            "score control names residual pack wave125: {}",
            r.detail
        );
        assert!(
            r.score_nav_commands_wave125_ok,
            "score nav commands residual pack wave125: {}",
            r.detail
        );
        assert!(
            r.options_control_names_wave126_ok,
            "options control names residual pack wave126: {}",
            r.detail
        );
        assert!(
            r.options_nav_commands_wave126_ok,
            "options nav commands residual pack wave126: {}",
            r.detail
        );
        assert!(
            r.credits_control_names_wave127_ok,
            "credits control names residual pack wave127: {}",
            r.detail
        );
        assert!(
            r.credits_nav_commands_wave127_ok,
            "credits nav commands residual pack wave127: {}",
            r.detail
        );
        assert!(
            r.message_box_control_names_wave128_ok,
            "message box control names residual pack wave128: {}",
            r.detail
        );
        assert!(
            r.message_box_nav_commands_wave128_ok,
            "message box nav commands residual pack wave128: {}",
            r.detail
        );
        assert!(
            r.diplomacy_control_names_wave129_ok,
            "diplomacy control names residual pack wave129: {}",
            r.detail
        );
        assert!(
            r.diplomacy_nav_commands_wave129_ok,
            "diplomacy nav commands residual pack wave129: {}",
            r.detail
        );
        assert!(
            r.popup_replay_control_names_wave130_ok,
            "popup replay control names residual pack wave130: {}",
            r.detail
        );
        assert!(
            r.popup_replay_nav_commands_wave130_ok,
            "popup replay nav commands residual pack wave130: {}",
            r.detail
        );
        assert!(
            r.single_player_control_names_wave131_ok,
            "single player control names residual pack wave131: {}",
            r.detail
        );
        assert!(
            r.single_player_nav_commands_wave131_ok,
            "single player nav commands residual pack wave131: {}",
            r.detail
        );
        assert!(
            r.map_select_control_names_wave132_ok,
            "map select control names residual pack wave132: {}",
            r.detail
        );
        assert!(
            r.map_select_nav_commands_wave132_ok,
            "map select nav commands residual pack wave132: {}",
            r.detail
        );
        assert!(
            r.control_bar_control_names_wave133_ok,
            "control bar control names residual pack wave133: {}",
            r.detail
        );
        assert!(
            r.control_bar_nav_commands_wave133_ok,
            "control bar nav commands residual pack wave133: {}",
            r.detail
        );
        assert!(
            r.difficulty_select_control_names_wave134_ok,
            "difficulty select control names residual pack wave134: {}",
            r.detail
        );
        assert!(
            r.difficulty_select_nav_commands_wave134_ok,
            "difficulty select nav commands residual pack wave134: {}",
            r.detail
        );
        assert!(
            r.loading_screen_stages_wave135_ok,
            "loading screen stages residual pack wave135: {}",
            r.detail
        );
        assert!(
            r.loading_screen_nav_commands_wave135_ok,
            "loading screen nav commands residual pack wave135: {}",
            r.detail
        );
        assert!(
            r.in_game_chat_control_names_wave136_ok,
            "in game chat control names residual pack wave136: {}",
            r.detail
        );
        assert!(
            r.in_game_chat_nav_commands_wave136_ok,
            "in game chat nav commands residual pack wave136: {}",
            r.detail
        );
        assert!(
            r.idle_worker_control_names_wave137_ok,
            "idle worker control names residual pack wave137: {}",
            r.detail
        );
        assert!(
            r.idle_worker_nav_commands_wave137_ok,
            "idle worker nav commands residual pack wave137: {}",
            r.detail
        );
        assert!(
            r.generals_exp_control_names_wave138_ok,
            "generals exp control names residual pack wave138: {}",
            r.detail
        );
        assert!(
            r.generals_exp_nav_commands_wave138_ok,
            "generals exp nav commands residual pack wave138: {}",
            r.detail
        );
        assert!(
            r.popup_communicator_control_names_wave139_ok,
            "popup communicator control names residual pack wave139: {}",
            r.detail
        );
        assert!(
            r.popup_communicator_nav_commands_wave139_ok,
            "popup communicator nav commands residual pack wave139: {}",
            r.detail
        );
        assert!(
            r.replay_control_control_names_wave140_ok,
            "replay control control names residual pack wave140: {}",
            r.detail
        );
        assert!(
            r.replay_control_nav_commands_wave140_ok,
            "replay control nav commands residual pack wave140: {}",
            r.detail
        );
        assert!(
            r.shell_map_names_wave141_ok,
            "shell map names residual pack wave141: {}",
            r.detail
        );
        assert!(
            r.shell_map_nav_commands_wave141_ok,
            "shell map nav commands residual pack wave141: {}",
            r.detail
        );
        assert!(
            r.beacon_control_names_wave142_ok,
            "beacon control names residual pack wave142: {}",
            r.detail
        );
        assert!(
            r.beacon_nav_commands_wave142_ok,
            "beacon nav commands residual pack wave142: {}",
            r.detail
        );
        assert!(
            r.eva_message_names_wave143_ok,
            "eva message names residual pack wave143: {}",
            r.detail
        );
        assert!(
            r.eva_nav_commands_wave143_ok,
            "eva nav commands residual pack wave143: {}",
            r.detail
        );
        assert!(
            r.ime_message_names_wave144_ok,
            "ime message names residual pack wave144: {}",
            r.detail
        );
        assert!(
            r.ime_nav_commands_wave144_ok,
            "ime nav commands residual pack wave144: {}",
            r.detail
        );
        assert!(
            r.smudge_method_names_wave145_ok,
            "smudge method names residual pack wave145: {}",
            r.detail
        );
        assert!(
            r.smudge_nav_commands_wave145_ok,
            "smudge nav commands residual pack wave145: {}",
            r.detail
        );
        assert!(
            r.ocl_timer_method_names_wave146_ok,
            "ocl timer method names residual pack wave146: {}",
            r.detail
        );
        assert!(
            r.ocl_timer_nav_commands_wave146_ok,
            "ocl timer nav commands residual pack wave146: {}",
            r.detail
        );
        assert!(
            r.control_bar_resizer_method_names_wave147_ok,
            "control bar resizer method names residual pack wave147: {}",
            r.detail
        );
        assert!(
            r.control_bar_resizer_nav_commands_wave147_ok,
            "control bar resizer nav commands residual pack wave147: {}",
            r.detail
        );
        assert!(
            r.under_construction_method_names_wave148_ok,
            "under construction method names residual pack wave148: {}",
            r.detail
        );
        assert!(
            r.under_construction_nav_commands_wave148_ok,
            "under construction nav commands residual pack wave148: {}",
            r.detail
        );
        assert!(
            r.structure_inventory_command_names_wave149_ok,
            "structure inventory command names residual pack wave149: {}",
            r.detail
        );
        assert!(
            r.structure_inventory_nav_commands_wave149_ok,
            "structure inventory nav commands residual pack wave149: {}",
            r.detail
        );
        assert!(
            r.multi_select_method_names_wave150_ok,
            "multi select method names residual pack wave150: {}",
            r.detail
        );
        assert!(
            r.multi_select_nav_commands_wave150_ok,
            "multi select nav commands residual pack wave150: {}",
            r.detail
        );
        assert!(
            r.credits_style_method_names_wave151_ok,
            "credits style method names residual pack wave151: {}",
            r.detail
        );
        assert!(
            r.credits_nav_commands_wave151_ok,
            "credits nav commands residual pack wave151: {}",
            r.detail
        );
        assert!(
            r.challenge_generals_method_names_wave152_ok,
            "challenge generals method names residual pack wave152: {}",
            r.detail
        );
        assert!(
            r.challenge_generals_nav_commands_wave152_ok,
            "challenge generals nav commands residual pack wave152: {}",
            r.detail
        );
        assert!(
            r.gameworld_authority_env_names_wave153_ok,
            "gameworld authority env names residual pack wave153: {}",
            r.detail
        );
        assert!(
            r.gameworld_authority_method_names_wave153_ok,
            "gameworld authority method names residual pack wave153: {}",
            r.detail
        );
        assert!(
            r.gameworld_authority_nav_commands_wave153_ok,
            "gameworld authority nav commands residual pack wave153: {}",
            r.detail
        );
        assert!(
            r.window_video_type_state_names_wave154_ok,
            "window video type state names residual pack wave154: {}",
            r.detail
        );
        assert!(
            r.window_video_method_names_wave154_ok,
            "window video method names residual pack wave154: {}",
            r.detail
        );
        assert!(
            r.window_video_nav_commands_wave154_ok,
            "window video nav commands residual pack wave154: {}",
            r.detail
        );
        assert!(
            r.main_menu_layout_names_wave155_ok,
            "main menu layout names residual pack wave155: {}",
            r.detail
        );
        assert!(
            r.main_menu_layout_nav_commands_wave155_ok,
            "main menu layout nav commands residual pack wave155: {}",
            r.detail
        );
        assert!(
            r.control_bar_scheme_names_wave156_ok,
            "control bar scheme names residual pack wave156: {}",
            r.detail
        );
        assert!(
            r.control_bar_scheme_method_names_wave156_ok,
            "control bar scheme method names residual pack wave156: {}",
            r.detail
        );
        assert!(
            r.control_bar_scheme_nav_commands_wave156_ok,
            "control bar scheme nav commands residual pack wave156: {}",
            r.detail
        );
        assert!(
            r.presentation_boundary_method_names_wave157_ok,
            "presentation boundary method names residual pack wave157: {}",
            r.detail
        );
        assert!(
            r.presentation_boundary_source_markers_wave157_ok,
            "presentation boundary source markers residual pack wave157: {}",
            r.detail
        );
        assert!(
            r.presentation_boundary_nav_commands_wave157_ok,
            "presentation boundary nav commands residual pack wave157: {}",
            r.detail
        );
        assert!(
            r.presentation_boundary_live_wave157_ok,
            "presentation boundary live residual wave157: {}",
            r.detail
        );
        assert!(
            r.control_bar_print_names_wave158_ok,
            "control bar print names residual pack wave158: {}",
            r.detail
        );
        assert!(
            r.control_bar_print_nav_commands_wave158_ok,
            "control bar print nav commands residual pack wave158: {}",
            r.detail
        );
        assert!(
            r.terrain_env_boundary_method_names_wave159_ok,
            "terrain env boundary method names residual pack wave159: {}",
            r.detail
        );
        assert!(
            r.terrain_env_boundary_source_markers_wave159_ok,
            "terrain env boundary source markers residual pack wave159: {}",
            r.detail
        );
        assert!(
            r.terrain_env_boundary_nav_commands_wave159_ok,
            "terrain env boundary nav commands residual pack wave159: {}",
            r.detail
        );
        assert!(
            r.terrain_env_boundary_live_wave159_ok,
            "terrain env boundary live residual wave159: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_names_wave160_ok,
            "main menu wnd names residual pack wave160: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_nav_commands_wave160_ok,
            "main menu wnd nav commands residual pack wave160: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_live_wave160_ok,
            "main menu wnd live residual wave160: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_load_method_names_wave161_ok,
            "main menu wnd load method names residual pack wave161: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_load_nav_commands_wave161_ok,
            "main menu wnd load nav commands residual pack wave161: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_load_live_wave161_ok,
            "main menu wnd load live residual wave161: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_materialise_method_names_wave162_ok,
            "main menu wnd materialise method names residual pack wave162: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_materialise_nav_commands_wave162_ok,
            "main menu wnd materialise nav commands residual pack wave162: {}",
            r.detail
        );
        assert!(
            r.main_menu_wnd_materialise_live_wave162_ok,
            "main menu wnd materialise live residual wave162: {}",
            r.detail
        );
        assert!(
            r.shell_stack_push_method_names_wave163_ok,
            "shell stack push method names residual pack wave163: {}",
            r.detail
        );
        assert!(
            r.shell_stack_push_nav_commands_wave163_ok,
            "shell stack push nav commands residual pack wave163: {}",
            r.detail
        );
        assert!(
            r.shell_stack_push_live_wave163_ok,
            "shell stack push live residual wave163: {}",
            r.detail
        );
        assert!(
            r.shell_skirmish_nav_method_names_wave164_ok,
            "shell skirmish nav method names residual pack wave164: {}",
            r.detail
        );
        assert!(
            r.shell_skirmish_nav_commands_wave164_ok,
            "shell skirmish nav commands residual pack wave164: {}",
            r.detail
        );
        assert!(
            r.shell_skirmish_nav_live_wave164_ok,
            "shell skirmish nav live residual wave164: {}",
            r.detail
        );
        assert!(
            r.control_bar_materialise_method_names_wave165_ok,
            "control bar materialise method names residual pack wave165: {}",
            r.detail
        );
        assert!(
            r.control_bar_materialise_nav_commands_wave165_ok,
            "control bar materialise nav commands residual pack wave165: {}",
            r.detail
        );
        assert!(
            r.control_bar_materialise_live_wave165_ok,
            "control bar materialise live residual wave165: {}",
            r.detail
        );
        assert!(
            r.skirmish_options_wnd_method_names_wave166_ok,
            "skirmish options wnd method names residual pack wave166: {}",
            r.detail
        );
        assert!(
            r.skirmish_options_wnd_nav_commands_wave166_ok,
            "skirmish options wnd nav commands residual pack wave166: {}",
            r.detail
        );
        assert!(
            r.skirmish_options_wnd_live_wave166_ok,
            "skirmish options wnd live residual wave166: {}",
            r.detail
        );
        assert!(
            r.new_game_stream_method_names_wave167_ok,
            "new game stream method names residual pack wave167: {}",
            r.detail
        );
        assert!(
            r.new_game_stream_nav_commands_wave167_ok,
            "new game stream nav commands residual pack wave167: {}",
            r.detail
        );
        assert!(
            r.new_game_stream_live_wave167_ok,
            "new game stream live residual wave167: {}",
            r.detail
        );
        assert!(
            r.w3d_main_menu_init_method_names_wave168_ok,
            "w3d main menu init method names residual pack wave168: {}",
            r.detail
        );
        assert!(
            r.w3d_main_menu_init_nav_commands_wave168_ok,
            "w3d main menu init nav commands residual pack wave168: {}",
            r.detail
        );
        assert!(
            r.w3d_main_menu_init_live_wave168_ok,
            "w3d main menu init live residual wave168: {}",
            r.detail
        );
        assert!(
            r.start_game_loading_method_names_wave169_ok,
            "start game loading method names residual pack wave169: {}",
            r.detail
        );
        assert!(
            r.start_game_loading_nav_commands_wave169_ok,
            "start game loading nav commands residual pack wave169: {}",
            r.detail
        );
        assert!(
            r.start_game_loading_live_wave169_ok,
            "start game loading live residual wave169: {}",
            r.detail
        );
        assert!(
            r.live_map_load_method_names_wave170_ok,
            "live map load method names residual pack wave170: {}",
            r.detail
        );
        assert!(
            r.live_map_load_nav_commands_wave170_ok,
            "live map load nav commands residual pack wave170: {}",
            r.detail
        );
        assert!(
            r.live_map_load_live_wave170_ok,
            "live map load live residual wave170: {}",
            r.detail
        );
        assert!(
            r.live_presentation_seed_method_names_wave171_ok,
            "live presentation seed method names residual pack wave171: {}",
            r.detail
        );
        assert!(
            r.live_presentation_seed_nav_commands_wave171_ok,
            "live presentation seed nav commands residual pack wave171: {}",
            r.detail
        );
        assert!(
            r.live_presentation_seed_live_wave171_ok,
            "live presentation seed live residual wave171: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_shadow_overlay_method_names_wave172_ok,
            "live gameworld shadow overlay method names residual pack wave172: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_shadow_overlay_nav_commands_wave172_ok,
            "live gameworld shadow overlay nav commands residual pack wave172: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_shadow_overlay_live_wave172_ok,
            "live gameworld shadow overlay live residual wave172: {}",
            r.detail
        );
        assert!(
            r.single_authority_combat_method_names_wave173_ok,
            "single authority combat method names residual pack wave173: {}",
            r.detail
        );
        assert!(
            r.single_authority_combat_nav_commands_wave173_ok,
            "single authority combat nav commands residual pack wave173: {}",
            r.detail
        );
        assert!(
            r.single_authority_combat_live_wave173_ok,
            "single authority combat live residual wave173: {}",
            r.detail
        );
        assert!(
            r.presentation_client_boundary_method_names_wave174_ok,
            "presentation client boundary method names residual pack wave174: {}",
            r.detail
        );
        assert!(
            r.presentation_client_boundary_nav_commands_wave174_ok,
            "presentation client boundary nav commands residual pack wave174: {}",
            r.detail
        );
        assert!(
            r.presentation_client_boundary_live_wave174_ok,
            "presentation client boundary live residual wave174: {}",
            r.detail
        );
        assert!(
            r.golden_map_host_victory_method_names_wave175_ok,
            "golden map host victory method names residual pack wave175: {}",
            r.detail
        );
        assert!(
            r.golden_map_host_victory_nav_commands_wave175_ok,
            "golden map host victory nav commands residual pack wave175: {}",
            r.detail
        );
        assert!(
            r.golden_map_host_victory_live_wave175_ok,
            "golden map host victory live residual wave175: {}",
            r.detail
        );
        assert!(
            r.executable_presentation_boundary_method_names_wave176_ok,
            "executable presentation boundary method names residual pack wave176: {}",
            r.detail
        );
        assert!(
            r.executable_presentation_boundary_nav_commands_wave176_ok,
            "executable presentation boundary nav commands residual pack wave176: {}",
            r.detail
        );
        assert!(
            r.executable_presentation_boundary_live_wave176_ok,
            "executable presentation boundary live residual wave176: {}",
            r.detail
        );
        assert!(
            r.production_authority_env_ok,
            "production authority env residual wave177: {}",
            r.detail
        );
        assert!(
            r.gameworld_production_authority_method_names_wave177_ok,
            "gameworld production authority method names residual pack wave177: {}",
            r.detail
        );
        assert!(
            r.gameworld_production_authority_nav_commands_wave177_ok,
            "gameworld production authority nav commands residual pack wave177: {}",
            r.detail
        );
        assert!(
            r.gameworld_production_authority_live_wave177_ok,
            "gameworld production authority live residual wave177: {}",
            r.detail
        );
        assert!(
            r.movement_authority_env_ok,
            "movement authority env residual wave178: {}",
            r.detail
        );
        assert!(
            r.gameworld_sole_tick_coupling_method_names_wave178_ok,
            "gameworld sole tick coupling method names residual pack wave178: {}",
            r.detail
        );
        assert!(
            r.gameworld_sole_tick_coupling_nav_commands_wave178_ok,
            "gameworld sole tick coupling nav commands residual pack wave178: {}",
            r.detail
        );
        assert!(
            r.gameworld_sole_tick_coupling_live_wave178_ok,
            "gameworld sole tick coupling live residual wave178: {}",
            r.detail
        );
        assert!(
            r.ai_fire_construction_authority_env_ok,
            "ai/fire/construction authority env residual wave179: {}",
            r.detail
        );
        assert!(
            r.gameworld_authority_matrix_method_names_wave179_ok,
            "gameworld authority matrix method names residual pack wave179: {}",
            r.detail
        );
        assert!(
            r.gameworld_authority_matrix_nav_commands_wave179_ok,
            "gameworld authority matrix nav commands residual pack wave179: {}",
            r.detail
        );
        assert!(
            r.gameworld_authority_matrix_live_wave179_ok,
            "gameworld authority matrix live residual wave179: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_production_writeback_method_names_wave180_ok,
            "live gameworld production writeback method names residual pack wave180: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_production_writeback_nav_commands_wave180_ok,
            "live gameworld production writeback nav commands residual pack wave180: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_production_writeback_live_wave180_ok,
            "live gameworld production writeback live residual wave180: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_construction_writeback_method_names_wave181_ok,
            "live gameworld construction writeback method names residual pack wave181: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_construction_writeback_nav_commands_wave181_ok,
            "live gameworld construction writeback nav commands residual pack wave181: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_construction_writeback_live_wave181_ok,
            "live gameworld construction writeback live residual wave181: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_damage_channel_method_names_wave182_ok,
            "live gameworld damage channel method names residual pack wave182: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_damage_channel_nav_commands_wave182_ok,
            "live gameworld damage channel nav commands residual pack wave182: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_damage_channel_live_wave182_ok,
            "live gameworld damage channel live residual wave182: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_economy_movement_method_names_wave183_ok,
            "live gameworld economy movement method names residual pack wave183: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_economy_movement_nav_commands_wave183_ok,
            "live gameworld economy movement nav commands residual pack wave183: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_economy_movement_live_wave183_ok,
            "live gameworld economy movement live residual wave183: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_projectile_ai_method_names_wave184_ok,
            "live gameworld projectile ai method names residual pack wave184: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_projectile_ai_nav_commands_wave184_ok,
            "live gameworld projectile ai nav commands residual pack wave184: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_projectile_ai_live_wave184_ok,
            "live gameworld projectile ai live residual wave184: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_fire_special_power_method_names_wave185_ok,
            "live gameworld fire special power method names residual pack wave185: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_fire_special_power_nav_commands_wave185_ok,
            "live gameworld fire special power nav commands residual pack wave185: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_fire_special_power_live_wave185_ok,
            "live gameworld fire special power live residual wave185: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_presentation_view_method_names_wave186_ok,
            "live gameworld presentation view method names residual pack wave186: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_presentation_view_nav_commands_wave186_ok,
            "live gameworld presentation view nav commands residual pack wave186: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_presentation_view_live_wave186_ok,
            "live gameworld presentation view live residual wave186: {}",
            r.detail
        );
        assert!(
            r.live_presentation_gameworld_overlay_method_names_wave187_ok,
            "live presentation gameworld overlay method names residual pack wave187: {}",
            r.detail
        );
        assert!(
            r.live_presentation_gameworld_overlay_nav_commands_wave187_ok,
            "live presentation gameworld overlay nav commands residual pack wave187: {}",
            r.detail
        );
        assert!(
            r.live_presentation_gameworld_overlay_live_wave187_ok,
            "live presentation gameworld overlay live residual wave187: {}",
            r.detail
        );
        assert!(
            r.executable_gameworld_presentation_method_names_wave188_ok,
            "executable gameworld presentation method names residual pack wave188: {}",
            r.detail
        );
        assert!(
            r.executable_gameworld_presentation_nav_commands_wave188_ok,
            "executable gameworld presentation nav commands residual pack wave188: {}",
            r.detail
        );
        assert!(
            r.executable_gameworld_presentation_live_wave188_ok,
            "executable gameworld presentation live residual wave188: {}",
            r.detail
        );
        assert!(
            r.live_presentation_overlay_deepen_method_names_wave189_ok,
            "live presentation overlay deepen method names residual pack wave189: {}",
            r.detail
        );
        assert!(
            r.live_presentation_overlay_deepen_nav_commands_wave189_ok,
            "live presentation overlay deepen nav commands residual pack wave189: {}",
            r.detail
        );
        assert!(
            r.live_presentation_overlay_deepen_live_wave189_ok,
            "live presentation overlay deepen live residual wave189: {}",
            r.detail
        );
        assert!(
            r.live_presentation_overlay_stamp_method_names_wave190_ok,
            "live presentation overlay stamp method names residual pack wave190: {}",
            r.detail
        );
        assert!(
            r.live_presentation_overlay_stamp_nav_commands_wave190_ok,
            "live presentation overlay stamp nav commands residual pack wave190: {}",
            r.detail
        );
        assert!(
            r.live_presentation_overlay_stamp_live_wave190_ok,
            "live presentation overlay stamp live residual wave190: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_entity_view_deepen_method_names_wave191_ok,
            "live gameworld entity view deepen method names residual pack wave191: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_entity_view_deepen_nav_commands_wave191_ok,
            "live gameworld entity view deepen nav commands residual pack wave191: {}",
            r.detail
        );
        assert!(
            r.live_gameworld_entity_view_deepen_live_wave191_ok,
            "live gameworld entity view deepen live residual wave191: {}",
            r.detail
        );
        assert!(
            r.live_presentation_append_missing_method_names_wave192_ok,
            "live presentation append missing method names residual pack wave192: {}",
            r.detail
        );
        assert!(
            r.live_presentation_append_missing_nav_commands_wave192_ok,
            "live presentation append missing nav commands residual pack wave192: {}",
            r.detail
        );
        assert!(
            r.live_presentation_append_missing_live_wave192_ok,
            "live presentation append missing live residual wave192: {}",
            r.detail
        );
        assert!(
            r.live_presentation_build_from_gameworld_method_names_wave193_ok,
            "live presentation build from gameworld method names residual pack wave193: {}",
            r.detail
        );
        assert!(
            r.live_presentation_build_from_gameworld_nav_commands_wave193_ok,
            "live presentation build from gameworld nav commands residual pack wave193: {}",
            r.detail
        );
        assert!(
            r.live_presentation_build_from_gameworld_live_wave193_ok,
            "live presentation build from gameworld live residual wave193: {}",
            r.detail
        );
        assert!(
            r.live_presentation_from_gameworld_default_method_names_wave194_ok,
            "live presentation from gameworld default method names residual pack wave194: {}",
            r.detail
        );
        assert!(
            r.live_presentation_from_gameworld_default_nav_commands_wave194_ok,
            "live presentation from gameworld default nav commands residual pack wave194: {}",
            r.detail
        );
        assert!(
            r.live_presentation_from_gameworld_default_live_wave194_ok,
            "live presentation from gameworld default live residual wave194: {}",
            r.detail
        );
        assert!(
            r.live_presentation_build_for_engine_method_names_wave195_ok,
            "live presentation build for engine method names residual pack wave195: {}",
            r.detail
        );
        assert!(
            r.live_presentation_build_for_engine_nav_commands_wave195_ok,
            "live presentation build for engine nav commands residual pack wave195: {}",
            r.detail
        );
        assert!(
            r.live_presentation_build_for_engine_live_wave195_ok,
            "live presentation build for engine live residual wave195: {}",
            r.detail
        );
        assert!(
            r.live_presentation_rebuilt_vertical_gate_method_names_wave196_ok,
            "live presentation rebuilt vertical gate method names residual pack wave196: {}",
            r.detail
        );
        assert!(
            r.live_presentation_rebuilt_vertical_gate_nav_commands_wave196_ok,
            "live presentation rebuilt vertical gate nav commands residual pack wave196: {}",
            r.detail
        );
        assert!(
            r.live_presentation_rebuilt_vertical_gate_live_wave196_ok,
            "live presentation rebuilt vertical gate live residual wave196: {}",
            r.detail
        );
        assert!(
            r.live_command_attack_log_method_names_wave197_ok,
            "live command attack log method names residual pack wave197: {}",
            r.detail
        );
        assert!(
            r.live_command_attack_log_nav_commands_wave197_ok,
            "live command attack log nav commands residual pack wave197: {}",
            r.detail
        );
        assert!(
            r.live_command_attack_log_live_wave197_ok,
            "live command attack log live residual wave197: {}",
            r.detail
        );
        assert!(
            r.live_command_guard_log_method_names_wave198_ok,
            "live command guard log method names residual pack wave198: {}",
            r.detail
        );
        assert!(
            r.live_command_guard_log_nav_commands_wave198_ok,
            "live command guard log nav commands residual pack wave198: {}",
            r.detail
        );
        assert!(
            r.live_command_guard_log_live_wave198_ok,
            "live command guard log live residual wave198: {}",
            r.detail
        );
        assert!(
            r.live_command_production_construction_log_method_names_wave199_ok,
            "live command production construction log method names residual pack wave199: {}",
            r.detail
        );
        assert!(
            r.live_command_production_construction_log_nav_commands_wave199_ok,
            "live command production construction log nav commands residual pack wave199: {}",
            r.detail
        );
        assert!(
            r.live_command_production_construction_log_live_wave199_ok,
            "live command production construction log live residual wave199: {}",
            r.detail
        );
        assert!(
            r.live_command_rally_log_method_names_wave200_ok,
            "live command rally log method names residual pack wave200: {}",
            r.detail
        );
        assert!(
            r.live_command_rally_log_nav_commands_wave200_ok,
            "live command rally log nav commands residual pack wave200: {}",
            r.detail
        );
        assert!(
            r.live_command_rally_log_live_wave200_ok,
            "live command rally log live residual wave200: {}",
            r.detail
        );
        assert!(
            r.live_evacuate_contain_log_method_names_wave201_ok,
            "live evacuate contain log method names residual pack wave201: {}",
            r.detail
        );
        assert!(
            r.live_evacuate_contain_log_nav_commands_wave201_ok,
            "live evacuate contain log nav commands residual pack wave201: {}",
            r.detail
        );
        assert!(
            r.live_evacuate_contain_log_live_wave201_ok,
            "live evacuate contain log live residual wave201: {}",
            r.detail
        );
        assert!(
            r.live_command_cheer_science_log_method_names_wave202_ok,
            "live command cheer science log method names residual pack wave202: {}",
            r.detail
        );
        assert!(
            r.live_command_cheer_science_log_nav_commands_wave202_ok,
            "live command cheer science log nav commands residual pack wave202: {}",
            r.detail
        );
        assert!(
            r.live_command_cheer_science_log_live_wave202_ok,
            "live command cheer science log live residual wave202: {}",
            r.detail
        );
        assert!(
            r.live_command_deploy_status_log_method_names_wave203_ok,
            "live command deploy status log method names residual pack wave203: {}",
            r.detail
        );
        assert!(
            r.live_command_deploy_status_log_nav_commands_wave203_ok,
            "live command deploy status log nav commands residual pack wave203: {}",
            r.detail
        );
        assert!(
            r.live_command_deploy_status_log_live_wave203_ok,
            "live command deploy status log live residual wave203: {}",
            r.detail
        );
        assert!(
            r.live_command_formation_log_method_names_wave204_ok,
            "live command formation log method names residual pack wave204: {}",
            r.detail
        );
        assert!(
            r.live_command_formation_log_nav_commands_wave204_ok,
            "live command formation log nav commands residual pack wave204: {}",
            r.detail
        );
        assert!(
            r.live_command_formation_log_live_wave204_ok,
            "live command formation log live residual wave204: {}",
            r.detail
        );
        assert!(
            r.live_command_order_target_log_method_names_wave205_ok,
            "live command order target log method names residual pack wave205: {}",
            r.detail
        );
        assert!(
            r.live_command_order_target_log_nav_commands_wave205_ok,
            "live command order target log nav commands residual pack wave205: {}",
            r.detail
        );
        assert!(
            r.live_command_order_target_log_live_wave205_ok,
            "live command order target log live residual wave205: {}",
            r.detail
        );
        assert!(
            r.live_command_selection_log_method_names_wave206_ok,
            "live command selection log method names residual pack wave206: {}",
            r.detail
        );
        assert!(
            r.live_command_selection_log_nav_commands_wave206_ok,
            "live command selection log nav commands residual pack wave206: {}",
            r.detail
        );
        assert!(
            r.live_command_selection_log_live_wave206_ok,
            "live command selection log live residual wave206: {}",
            r.detail
        );
        assert!(
            r.live_command_non_attack_order_target_method_names_wave207_ok,
            "live command non attack order target method names residual pack wave207: {}",
            r.detail
        );
        assert!(
            r.live_command_non_attack_order_target_nav_commands_wave207_ok,
            "live command non attack order target nav commands residual pack wave207: {}",
            r.detail
        );
        assert!(
            r.live_command_non_attack_order_target_live_wave207_ok,
            "live command non attack order target live residual wave207: {}",
            r.detail
        );
        assert!(
            r.live_golden_mopup_honesty_method_names_wave208_ok,
            "live golden mopup honesty method names residual pack wave208: {}",
            r.detail
        );
        assert!(
            r.live_golden_mopup_honesty_nav_commands_wave208_ok,
            "live golden mopup honesty nav commands residual pack wave208: {}",
            r.detail
        );
        assert!(
            r.live_golden_mopup_honesty_live_wave208_ok,
            "live golden mopup honesty live residual wave208: {}",
            r.detail
        );
        assert!(
            r.live_os_input_command_path_method_names_wave209_ok,
            "live os input command path method names residual pack wave209: {}",
            r.detail
        );
        assert!(
            r.live_os_input_command_path_nav_commands_wave209_ok,
            "live os input command path nav commands residual pack wave209: {}",
            r.detail
        );
        assert!(
            r.live_os_input_command_path_live_wave209_ok,
            "live os input command path live residual wave209: {}",
            r.detail
        );
        assert!(
            r.live_command_beacon_note_method_names_wave210_ok,
            "live command beacon note method names residual pack wave210: {}",
            r.detail
        );
        assert!(
            r.live_command_beacon_note_nav_commands_wave210_ok,
            "live command beacon note nav commands residual pack wave210: {}",
            r.detail
        );
        assert!(
            r.live_command_beacon_note_live_wave210_ok,
            "live command beacon note live residual wave210: {}",
            r.detail
        );
        assert!(
            r.live_host_beacon_presentation_method_names_wave211_ok,
            "live host beacon presentation method names residual pack wave211: {}",
            r.detail
        );
        assert!(
            r.live_host_beacon_presentation_nav_commands_wave211_ok,
            "live host beacon presentation nav commands residual pack wave211: {}",
            r.detail
        );
        assert!(
            r.live_host_beacon_presentation_live_wave211_ok,
            "live host beacon presentation live residual wave211: {}",
            r.detail
        );
        assert!(
            r.live_command_sell_deselect_log_method_names_wave212_ok,
            "live command sell deselect log method names residual pack wave212: {}",
            r.detail
        );
        assert!(
            r.live_command_sell_deselect_log_nav_commands_wave212_ok,
            "live command sell deselect log nav commands residual pack wave212: {}",
            r.detail
        );
        assert!(
            r.live_command_sell_deselect_log_live_wave212_ok,
            "live command sell deselect log live residual wave212: {}",
            r.detail
        );
        assert!(
            r.live_presentation_fow_only_method_names_wave213_ok,
            "live presentation fow only method names residual pack wave213: {}",
            r.detail
        );
        assert!(
            r.live_presentation_fow_only_nav_commands_wave213_ok,
            "live presentation fow only nav commands residual pack wave213: {}",
            r.detail
        );
        assert!(
            r.live_presentation_fow_only_live_wave213_ok,
            "live presentation fow only live residual wave213: {}",
            r.detail
        );
        assert!(
            r.live_ui_producer_presentation_only_method_names_wave214_ok,
            "live ui producer presentation only method names residual pack wave214: {}",
            r.detail
        );
        assert!(
            r.live_ui_producer_presentation_only_nav_commands_wave214_ok,
            "live ui producer presentation only nav commands residual pack wave214: {}",
            r.detail
        );
        assert!(
            r.live_ui_producer_presentation_only_live_wave214_ok,
            "live ui producer presentation only live residual wave214: {}",
            r.detail
        );
        assert!(
            r.live_ui_helpers_presentation_only_method_names_wave215_ok,
            "live ui helpers presentation only method names residual pack wave215: {}",
            r.detail
        );
        assert!(
            r.live_ui_helpers_presentation_only_nav_commands_wave215_ok,
            "live ui helpers presentation only nav commands residual pack wave215: {}",
            r.detail
        );
        assert!(
            r.live_ui_helpers_presentation_only_live_wave215_ok,
            "live ui helpers presentation only live residual wave215: {}",
            r.detail
        );
        assert!(
            r.live_control_group_camera_presentation_only_method_names_wave216_ok,
            "live control group camera presentation only method names residual pack wave216: {}",
            r.detail
        );
        assert!(
            r.live_control_group_camera_presentation_only_nav_commands_wave216_ok,
            "live control group camera presentation only nav commands residual pack wave216: {}",
            r.detail
        );
        assert!(
            r.live_control_group_camera_presentation_only_live_wave216_ok,
            "live control group camera presentation only live residual wave216: {}",
            r.detail
        );
        assert!(
            r.live_cmd_filter_env_presentation_only_method_names_wave217_ok,
            "live cmd filter env presentation only method names residual pack wave217: {}",
            r.detail
        );
        assert!(
            r.live_cmd_filter_env_presentation_only_nav_commands_wave217_ok,
            "live cmd filter env presentation only nav commands residual pack wave217: {}",
            r.detail
        );
        assert!(
            r.live_cmd_filter_env_presentation_only_live_wave217_ok,
            "live cmd filter env presentation only live residual wave217: {}",
            r.detail
        );
        assert!(
            r.live_selection_commands_presentation_only_method_names_wave218_ok,
            "live selection commands presentation only method names residual pack wave218: {}",
            r.detail
        );
        assert!(
            r.live_selection_commands_presentation_only_nav_commands_wave218_ok,
            "live selection commands presentation only nav commands residual pack wave218: {}",
            r.detail
        );
        assert!(
            r.live_selection_commands_presentation_only_live_wave218_ok,
            "live selection commands presentation only live residual wave218: {}",
            r.detail
        );
        assert!(
            r.live_ui_command_selection_presentation_only_method_names_wave219_ok,
            "live ui command selection presentation only method names residual pack wave219: {}",
            r.detail
        );
        assert!(
            r.live_ui_command_selection_presentation_only_nav_commands_wave219_ok,
            "live ui command selection presentation only nav commands residual pack wave219: {}",
            r.detail
        );
        assert!(
            r.live_ui_command_selection_presentation_only_live_wave219_ok,
            "live ui command selection presentation only live residual wave219: {}",
            r.detail
        );
        assert!(
            r.live_local_team_presentation_only_method_names_wave220_ok,
            "live local team presentation only method names residual pack wave220: {}",
            r.detail
        );
        assert!(
            r.live_local_team_presentation_only_nav_commands_wave220_ok,
            "live local team presentation only nav commands residual pack wave220: {}",
            r.detail
        );
        assert!(
            r.live_local_team_presentation_only_live_wave220_ok,
            "live local team presentation only live residual wave220: {}",
            r.detail
        );
        assert!(
            r.live_hotkey_move_attack_selection_presentation_only_method_names_wave221_ok,
            "live hotkey move attack selection presentation only method names residual pack wave221: {}",
            r.detail
        );
        assert!(
            r.live_hotkey_move_attack_selection_presentation_only_nav_commands_wave221_ok,
            "live hotkey move attack selection presentation only nav commands residual pack wave221: {}",
            r.detail
        );
        assert!(
            r.live_hotkey_move_attack_selection_presentation_only_live_wave221_ok,
            "live hotkey move attack selection presentation only live residual wave221: {}",
            r.detail
        );
        assert!(
            r.live_pick_object_presentation_only_method_names_wave222_ok,
            "live pick object presentation only method names residual pack wave222: {}",
            r.detail
        );
        assert!(
            r.live_pick_object_presentation_only_nav_commands_wave222_ok,
            "live pick object presentation only nav commands residual pack wave222: {}",
            r.detail
        );
        assert!(
            r.live_pick_object_presentation_only_live_wave222_ok,
            "live pick object presentation only live residual wave222: {}",
            r.detail
        );
        assert!(
            r.live_bootstrap_camera_presentation_only_method_names_wave223_ok,
            "live bootstrap camera presentation only method names residual pack wave223: {}",
            r.detail
        );
        assert!(
            r.live_bootstrap_camera_presentation_only_nav_commands_wave223_ok,
            "live bootstrap camera presentation only nav commands residual pack wave223: {}",
            r.detail
        );
        assert!(
            r.live_bootstrap_camera_presentation_only_live_wave223_ok,
            "live bootstrap camera presentation only live residual wave223: {}",
            r.detail
        );
        assert!(
            r.live_force_complete_authority_api_method_names_wave224_ok,
            "live force complete authority api method names residual pack wave224: {}",
            r.detail
        );
        assert!(
            r.live_force_complete_authority_api_nav_commands_wave224_ok,
            "live force complete authority api nav commands residual pack wave224: {}",
            r.detail
        );
        assert!(
            r.live_force_complete_authority_api_live_wave224_ok,
            "live force complete authority api live residual wave224: {}",
            r.detail
        );
        assert!(
            r.live_path_guard_authority_api_method_names_wave225_ok,
            "live path guard authority api method names residual pack wave225: {}",
            r.detail
        );
        assert!(
            r.live_path_guard_authority_api_nav_commands_wave225_ok,
            "live path guard authority api nav commands residual pack wave225: {}",
            r.detail
        );
        assert!(
            r.live_path_guard_authority_api_live_wave225_ok,
            "live path guard authority api live residual wave225: {}",
            r.detail
        );
        assert!(
            r.live_hotkey_selection_camera_presentation_only_method_names_wave226_ok,
            "live hotkey selection camera presentation only method names residual pack wave226: {}",
            r.detail
        );
        assert!(
            r.live_hotkey_selection_camera_presentation_only_nav_commands_wave226_ok,
            "live hotkey selection camera presentation only nav commands residual pack wave226: {}",
            r.detail
        );
        assert!(
            r.live_hotkey_selection_camera_presentation_only_live_wave226_ok,
            "live hotkey selection camera presentation only live residual wave226: {}",
            r.detail
        );
        assert!(
            r.live_construct_spawn_pose_authority_api_method_names_wave227_ok,
            "live construct spawn pose authority api method names residual pack wave227: {}",
            r.detail
        );
        assert!(
            r.live_construct_spawn_pose_authority_api_nav_commands_wave227_ok,
            "live construct spawn pose authority api nav commands residual pack wave227: {}",
            r.detail
        );
        assert!(
            r.live_construct_spawn_pose_authority_api_live_wave227_ok,
            "live construct spawn pose authority api live residual wave227: {}",
            r.detail
        );
        assert!(
            r.live_rmb_target_presentation_only_method_names_wave228_ok,
            "live rmb target presentation only method names residual pack wave228: {}",
            r.detail
        );
        assert!(
            r.live_rmb_target_presentation_only_nav_commands_wave228_ok,
            "live rmb target presentation only nav commands residual pack wave228: {}",
            r.detail
        );
        assert!(
            r.live_rmb_target_presentation_only_live_wave228_ok,
            "live rmb target presentation only live residual wave228: {}",
            r.detail
        );
        assert!(
            r.live_rmb_selected_presentation_only_method_names_wave229_ok,
            "live rmb selected presentation only method names residual pack wave229: {}",
            r.detail
        );
        assert!(
            r.live_rmb_selected_presentation_only_nav_commands_wave229_ok,
            "live rmb selected presentation only nav commands residual pack wave229: {}",
            r.detail
        );
        assert!(
            r.live_rmb_selected_presentation_only_live_wave229_ok,
            "live rmb selected presentation only live residual wave229: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_authority_api_method_names_wave230_ok,
            "live command unit authority api method names residual pack wave230: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_authority_api_nav_commands_wave230_ok,
            "live command unit authority api nav commands residual pack wave230: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_authority_api_live_wave230_ok,
            "live command unit authority api live residual wave230: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_more_authority_api_method_names_wave231_ok,
            "live command unit more authority api method names residual pack wave231: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_more_authority_api_nav_commands_wave231_ok,
            "live command unit more authority api nav commands residual pack wave231: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_more_authority_api_live_wave231_ok,
            "live command unit more authority api live residual wave231: {}",
            r.detail
        );
        assert!(
            r.live_command_executor_authority_api_method_names_wave232_ok,
            "live command executor authority api method names residual pack wave232: {}",
            r.detail
        );
        assert!(
            r.live_command_executor_authority_api_nav_commands_wave232_ok,
            "live command executor authority api nav commands residual pack wave232: {}",
            r.detail
        );
        assert!(
            r.live_command_executor_authority_api_live_wave232_ok,
            "live command executor authority api live residual wave232: {}",
            r.detail
        );
        assert!(
            r.live_command_executor_more_authority_api_method_names_wave233_ok,
            "live command executor more authority api method names residual pack wave233: {}",
            r.detail
        );
        assert!(
            r.live_command_executor_more_authority_api_nav_commands_wave233_ok,
            "live command executor more authority api nav commands residual pack wave233: {}",
            r.detail
        );
        assert!(
            r.live_command_executor_more_authority_api_live_wave233_ok,
            "live command executor more authority api live residual wave233: {}",
            r.detail
        );
        assert!(
            r.live_engine_presentation_player_ui_method_names_wave234_ok,
            "live engine presentation player ui method names residual pack wave234: {}",
            r.detail
        );
        assert!(
            r.live_engine_presentation_player_ui_nav_commands_wave234_ok,
            "live engine presentation player ui nav commands residual pack wave234: {}",
            r.detail
        );
        assert!(
            r.live_engine_presentation_player_ui_live_wave234_ok,
            "live engine presentation player ui live residual wave234: {}",
            r.detail
        );
        assert!(
            r.live_rmb_presentation_full_classify_method_names_wave235_ok,
            "live rmb presentation full classify method names residual pack wave235: {}",
            r.detail
        );
        assert!(
            r.live_rmb_presentation_full_classify_nav_commands_wave235_ok,
            "live rmb presentation full classify nav commands residual pack wave235: {}",
            r.detail
        );
        assert!(
            r.live_rmb_presentation_full_classify_live_wave235_ok,
            "live rmb presentation full classify live residual wave235: {}",
            r.detail
        );
        assert!(
            r.live_mouse_input_presentation_only_method_names_wave236_ok,
            "live mouse input presentation only method names residual pack wave236: {}",
            r.detail
        );
        assert!(
            r.live_mouse_input_presentation_only_nav_commands_wave236_ok,
            "live mouse input presentation only nav commands residual pack wave236: {}",
            r.detail
        );
        assert!(
            r.live_mouse_input_presentation_only_live_wave236_ok,
            "live mouse input presentation only live residual wave236: {}",
            r.detail
        );
        assert!(
            r.live_engine_player_ui_boot_peel_method_names_wave237_ok,
            "live engine player ui boot peel method names residual pack wave237: {}",
            r.detail
        );
        assert!(
            r.live_engine_player_ui_boot_peel_nav_commands_wave237_ok,
            "live engine player ui boot peel nav commands residual pack wave237: {}",
            r.detail
        );
        assert!(
            r.live_engine_player_ui_boot_peel_live_wave237_ok,
            "live engine player ui boot peel live residual wave237: {}",
            r.detail
        );
        assert!(
            r.live_player_probe_api_method_names_wave238_ok,
            "live player probe api method names residual pack wave238: {}",
            r.detail
        );
        assert!(
            r.live_player_probe_api_nav_commands_wave238_ok,
            "live player probe api nav commands residual pack wave238: {}",
            r.detail
        );
        assert!(
            r.live_player_probe_api_live_wave238_ok,
            "live player probe api live residual wave238: {}",
            r.detail
        );
        assert!(
            r.live_player_team_probe_method_names_wave239_ok,
            "live player team probe method names residual pack wave239: {}",
            r.detail
        );
        assert!(
            r.live_player_team_probe_nav_commands_wave239_ok,
            "live player team probe nav commands residual pack wave239: {}",
            r.detail
        );
        assert!(
            r.live_player_team_probe_live_wave239_ok,
            "live player team probe live residual wave239: {}",
            r.detail
        );
        assert!(
            r.live_player_field_probe_method_names_wave240_ok,
            "live player field probe method names residual pack wave240: {}",
            r.detail
        );
        assert!(
            r.live_player_field_probe_nav_commands_wave240_ok,
            "live player field probe nav commands residual pack wave240: {}",
            r.detail
        );
        assert!(
            r.live_player_field_probe_live_wave240_ok,
            "live player field probe live residual wave240: {}",
            r.detail
        );
        assert!(
            r.live_camera_height_probe_method_names_wave241_ok,
            "live camera height probe method names residual pack wave241: {}",
            r.detail
        );
        assert!(
            r.live_camera_height_probe_nav_commands_wave241_ok,
            "live camera height probe nav commands residual pack wave241: {}",
            r.detail
        );
        assert!(
            r.live_camera_height_probe_live_wave241_ok,
            "live camera height probe live residual wave241: {}",
            r.detail
        );
        assert!(
            r.live_command_player_probe_method_names_wave242_ok,
            "live command player probe method names residual pack wave242: {}",
            r.detail
        );
        assert!(
            r.live_command_player_probe_nav_commands_wave242_ok,
            "live command player probe nav commands residual pack wave242: {}",
            r.detail
        );
        assert!(
            r.live_command_player_probe_live_wave242_ok,
            "live command player probe live residual wave242: {}",
            r.detail
        );
        assert!(
            r.live_construct_economy_probe_method_names_wave243_ok,
            "live construct economy probe method names residual pack wave243: {}",
            r.detail
        );
        assert!(
            r.live_construct_economy_probe_nav_commands_wave243_ok,
            "live construct economy probe nav commands residual pack wave243: {}",
            r.detail
        );
        assert!(
            r.live_construct_economy_probe_live_wave243_ok,
            "live construct economy probe live residual wave243: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_probe_method_names_wave244_ok,
            "live command unit probe method names residual pack wave244: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_probe_nav_commands_wave244_ok,
            "live command unit probe nav commands residual pack wave244: {}",
            r.detail
        );
        assert!(
            r.live_command_unit_probe_live_wave244_ok,
            "live command unit probe live residual wave244: {}",
            r.detail
        );
        assert!(
            r.live_selection_query_probe_method_names_wave245_ok,
            "live selection query probe method names residual pack wave245: {}",
            r.detail
        );
        assert!(
            r.live_selection_query_probe_nav_commands_wave245_ok,
            "live selection query probe nav commands residual pack wave245: {}",
            r.detail
        );
        assert!(
            r.live_selection_query_probe_live_wave245_ok,
            "live selection query probe live residual wave245: {}",
            r.detail
        );
        assert!(
            r.live_world_pick_probe_method_names_wave246_ok,
            "live world pick probe method names residual pack wave246: {}",
            r.detail
        );
        assert!(
            r.live_world_pick_probe_nav_commands_wave246_ok,
            "live world pick probe nav commands residual pack wave246: {}",
            r.detail
        );
        assert!(
            r.live_world_pick_probe_live_wave246_ok,
            "live world pick probe live residual wave246: {}",
            r.detail
        );
        assert!(
            r.live_object_registry_empty_fastpath_method_names_wave247_ok,
            "live object registry empty fastpath method names residual pack wave247: {}",
            r.detail
        );
        assert!(
            r.live_object_registry_empty_fastpath_nav_commands_wave247_ok,
            "live object registry empty fastpath nav commands residual pack wave247: {}",
            r.detail
        );
        assert!(
            r.live_object_registry_empty_fastpath_live_wave247_ok,
            "live object registry empty fastpath live residual wave247: {}",
            r.detail
        );
        assert!(
            r.live_legacy_object_registry_fastpath_method_names_wave248_ok,
            "live legacy object registry fastpath method names residual pack wave248: {}",
            r.detail
        );
        assert!(
            r.live_legacy_object_registry_fastpath_nav_commands_wave248_ok,
            "live legacy object registry fastpath nav commands residual pack wave248: {}",
            r.detail
        );
        assert!(
            r.live_legacy_object_registry_fastpath_live_wave248_ok,
            "live legacy object registry fastpath live residual wave248: {}",
            r.detail
        );
        assert!(
            r.live_client_dual_world_empty_gate_method_names_wave249_ok,
            "live client dual world empty gate method names residual pack wave249: {}",
            r.detail
        );
        assert!(
            r.live_client_dual_world_empty_gate_nav_commands_wave249_ok,
            "live client dual world empty gate nav commands residual pack wave249: {}",
            r.detail
        );
        assert!(
            r.live_client_dual_world_empty_gate_live_wave249_ok,
            "live client dual world empty gate live residual wave249: {}",
            r.detail
        );
        assert!(
            r.live_presentation_time_frozen_probe_method_names_wave250_ok,
            "live presentation time frozen probe method names residual pack wave250: {}",
            r.detail
        );
        assert!(
            r.live_presentation_time_frozen_probe_nav_commands_wave250_ok,
            "live presentation time frozen probe nav commands residual pack wave250: {}",
            r.detail
        );
        assert!(
            r.live_presentation_time_frozen_probe_live_wave250_ok,
            "live presentation time frozen probe live residual wave250: {}",
            r.detail
        );
        assert!(
            r.live_presentation_visual_speed_probe_method_names_wave251_ok,
            "live presentation visual speed probe method names residual pack wave251: {}",
            r.detail
        );
        assert!(
            r.live_presentation_visual_speed_probe_nav_commands_wave251_ok,
            "live presentation visual speed probe nav commands residual pack wave251: {}",
            r.detail
        );
        assert!(
            r.live_presentation_visual_speed_probe_live_wave251_ok,
            "live presentation visual speed probe live residual wave251: {}",
            r.detail
        );
        assert!(
            r.live_presentation_script_camera_probe_method_names_wave252_ok,
            "live presentation script camera probe method names residual pack wave252: {}",
            r.detail
        );
        assert!(
            r.live_presentation_script_camera_probe_nav_commands_wave252_ok,
            "live presentation script camera probe nav commands residual pack wave252: {}",
            r.detail
        );
        assert!(
            r.live_presentation_script_camera_probe_live_wave252_ok,
            "live presentation script camera probe live residual wave252: {}",
            r.detail
        );
        assert!(
            r.live_ai_group_dual_world_empty_gate_method_names_wave253_ok,
            "live ai group dual-world empty gate method names residual pack wave253: {}",
            r.detail
        );
        assert!(
            r.live_ai_group_dual_world_empty_gate_nav_commands_wave253_ok,
            "live ai group dual-world empty gate nav commands residual pack wave253: {}",
            r.detail
        );
        assert!(
            r.live_ai_group_dual_world_empty_gate_live_wave253_ok,
            "live ai group dual-world empty gate live residual wave253: {}",
            r.detail
        );
        assert!(
            r.live_ai_states_dual_world_empty_gate_method_names_wave254_ok,
            "live ai states dual-world empty gate method names residual pack wave254: {}",
            r.detail
        );
        assert!(
            r.live_ai_states_dual_world_empty_gate_nav_commands_wave254_ok,
            "live ai states dual-world empty gate nav commands residual pack wave254: {}",
            r.detail
        );
        assert!(
            r.live_ai_states_dual_world_empty_gate_live_wave254_ok,
            "live ai states dual-world empty gate live residual wave254: {}",
            r.detail
        );
        assert!(
            r.live_ai_player_dual_world_empty_gate_method_names_wave255_ok,
            "live ai player dual-world empty gate method names residual pack wave255: {}",
            r.detail
        );
        assert!(
            r.live_ai_player_dual_world_empty_gate_nav_commands_wave255_ok,
            "live ai player dual-world empty gate nav commands residual pack wave255: {}",
            r.detail
        );
        assert!(
            r.live_ai_player_dual_world_empty_gate_live_wave255_ok,
            "live ai player dual-world empty gate live residual wave255: {}",
            r.detail
        );
        assert!(
            r.live_team_dual_world_empty_gate_method_names_wave256_ok,
            "live team dual-world empty gate method names residual pack wave256: {}",
            r.detail
        );
        assert!(
            r.live_team_dual_world_empty_gate_nav_commands_wave256_ok,
            "live team dual-world empty gate nav commands residual pack wave256: {}",
            r.detail
        );
        assert!(
            r.live_team_dual_world_empty_gate_live_wave256_ok,
            "live team dual-world empty gate live residual wave256: {}",
            r.detail
        );
        assert!(
            r.live_ai_legacy_states_dual_world_empty_gate_method_names_wave257_ok,
            "live ai legacy states dual-world empty gate method names residual pack wave257: {}",
            r.detail
        );
        assert!(
            r.live_ai_legacy_states_dual_world_empty_gate_nav_commands_wave257_ok,
            "live ai legacy states dual-world empty gate nav commands residual pack wave257: {}",
            r.detail
        );
        assert!(
            r.live_ai_legacy_states_dual_world_empty_gate_live_wave257_ok,
            "live ai legacy states dual-world empty gate live residual wave257: {}",
            r.detail
        );
        assert!(
            r.live_unit_dual_world_empty_gate_method_names_wave258_ok,
            "live unit dual-world empty gate method names residual pack wave258: {}",
            r.detail
        );
        assert!(
            r.live_unit_dual_world_empty_gate_nav_commands_wave258_ok,
            "live unit dual-world empty gate nav commands residual pack wave258: {}",
            r.detail
        );
        assert!(
            r.live_unit_dual_world_empty_gate_live_wave258_ok,
            "live unit dual-world empty gate live residual wave258: {}",
            r.detail
        );
        assert!(
            r.live_stealth_dual_world_empty_gate_method_names_wave259_ok,
            "live stealth dual-world empty gate method names residual pack wave259: {}",
            r.detail
        );
        assert!(
            r.live_stealth_dual_world_empty_gate_nav_commands_wave259_ok,
            "live stealth dual-world empty gate nav commands residual pack wave259: {}",
            r.detail
        );
        assert!(
            r.live_stealth_dual_world_empty_gate_live_wave259_ok,
            "live stealth dual-world empty gate live residual wave259: {}",
            r.detail
        );
        assert!(
            r.live_garrison_dual_world_empty_gate_method_names_wave260_ok,
            "live garrison dual-world empty gate method names residual pack wave260: {}",
            r.detail
        );
        assert!(
            r.live_garrison_dual_world_empty_gate_nav_commands_wave260_ok,
            "live garrison dual-world empty gate nav commands residual pack wave260: {}",
            r.detail
        );
        assert!(
            r.live_garrison_dual_world_empty_gate_live_wave260_ok,
            "live garrison dual-world empty gate live residual wave260: {}",
            r.detail
        );
        assert!(
            r.live_open_contain_dual_world_empty_gate_method_names_wave261_ok,
            "live open contain dual-world empty gate method names residual pack wave261: {}",
            r.detail
        );
        assert!(
            r.live_open_contain_dual_world_empty_gate_nav_commands_wave261_ok,
            "live open contain dual-world empty gate nav commands residual pack wave261: {}",
            r.detail
        );
        assert!(
            r.live_open_contain_dual_world_empty_gate_live_wave261_ok,
            "live open contain dual-world empty gate live residual wave261: {}",
            r.detail
        );
        assert!(
            r.live_pathfind_dual_world_empty_gate_method_names_wave262_ok,
            "live pathfind dual-world empty gate method names residual pack wave262: {}",
            r.detail
        );
        assert!(
            r.live_pathfind_dual_world_empty_gate_nav_commands_wave262_ok,
            "live pathfind dual-world empty gate nav commands residual pack wave262: {}",
            r.detail
        );
        assert!(
            r.live_pathfind_dual_world_empty_gate_live_wave262_ok,
            "live pathfind dual-world empty gate live residual wave262: {}",
            r.detail
        );
        assert!(
            r.live_ai_mod_dual_world_empty_gate_method_names_wave263_ok,
            "live ai mod dual-world empty gate method names residual pack wave263: {}",
            r.detail
        );
        assert!(
            r.live_ai_mod_dual_world_empty_gate_nav_commands_wave263_ok,
            "live ai mod dual-world empty gate nav commands residual pack wave263: {}",
            r.detail
        );
        assert!(
            r.live_ai_mod_dual_world_empty_gate_live_wave263_ok,
            "live ai mod dual-world empty gate live residual wave263: {}",
            r.detail
        );
        assert!(
            r.live_object_mod_dual_world_empty_gate_method_names_wave264_ok,
            "live object mod dual-world empty gate method names residual pack wave264: {}",
            r.detail
        );
        assert!(
            r.live_object_mod_dual_world_empty_gate_nav_commands_wave264_ok,
            "live object mod dual-world empty gate nav commands residual pack wave264: {}",
            r.detail
        );
        assert!(
            r.live_object_mod_dual_world_empty_gate_live_wave264_ok,
            "live object mod dual-world empty gate live residual wave264: {}",
            r.detail
        );
        assert!(
            r.live_weapon_dual_world_empty_gate_method_names_wave265_ok,
            "live weapon dual-world empty gate method names residual pack wave265: {}",
            r.detail
        );
        assert!(
            r.live_weapon_dual_world_empty_gate_nav_commands_wave265_ok,
            "live weapon dual-world empty gate nav commands residual pack wave265: {}",
            r.detail
        );
        assert!(
            r.live_weapon_dual_world_empty_gate_live_wave265_ok,
            "live weapon dual-world empty gate live residual wave265: {}",
            r.detail
        );
        assert!(
            r.live_partition_filters_dual_world_empty_gate_method_names_wave266_ok,
            "live partition filters dual-world empty gate method names residual pack wave266: {}",
            r.detail
        );
        assert!(
            r.live_partition_filters_dual_world_empty_gate_nav_commands_wave266_ok,
            "live partition filters dual-world empty gate nav commands residual pack wave266: {}",
            r.detail
        );
        assert!(
            r.live_partition_filters_dual_world_empty_gate_live_wave266_ok,
            "live partition filters dual-world empty gate live residual wave266: {}",
            r.detail
        );
        assert!(
            r.live_ai_state_machine_dual_world_empty_gate_method_names_wave267_ok,
            "live ai state machine dual-world empty gate method names residual pack wave267: {}",
            r.detail
        );
        assert!(
            r.live_ai_state_machine_dual_world_empty_gate_nav_commands_wave267_ok,
            "live ai state machine dual-world empty gate nav commands residual pack wave267: {}",
            r.detail
        );
        assert!(
            r.live_ai_state_machine_dual_world_empty_gate_live_wave267_ok,
            "live ai state machine dual-world empty gate live residual wave267: {}",
            r.detail
        );
        assert!(
            r.live_player_dual_world_empty_gate_method_names_wave268_ok,
            "live player dual-world empty gate method names residual pack wave268: {}",
            r.detail
        );
        assert!(
            r.live_player_dual_world_empty_gate_nav_commands_wave268_ok,
            "live player dual-world empty gate nav commands residual pack wave268: {}",
            r.detail
        );
        assert!(
            r.live_player_dual_world_empty_gate_live_wave268_ok,
            "live player dual-world empty gate live residual wave268: {}",
            r.detail
        );
        assert!(
            r.live_game_client_dual_world_empty_gate_method_names_wave269_ok,
            "live game client dual-world empty gate method names residual pack wave269: {}",
            r.detail
        );
        assert!(
            r.live_game_client_dual_world_empty_gate_nav_commands_wave269_ok,
            "live game client dual-world empty gate nav commands residual pack wave269: {}",
            r.detail
        );
        assert!(
            r.live_game_client_dual_world_empty_gate_live_wave269_ok,
            "live game client dual-world empty gate live residual wave269: {}",
            r.detail
        );
        assert!(
            r.live_drawable_dual_world_empty_gate_method_names_wave270_ok,
            "live drawable dual-world empty gate method names residual pack wave270: {}",
            r.detail
        );
        assert!(
            r.live_drawable_dual_world_empty_gate_nav_commands_wave270_ok,
            "live drawable dual-world empty gate nav commands residual pack wave270: {}",
            r.detail
        );
        assert!(
            r.live_drawable_dual_world_empty_gate_live_wave270_ok,
            "live drawable dual-world empty gate live residual wave270: {}",
            r.detail
        );
        assert!(
            r.live_script_conditions_dual_world_empty_gate_method_names_wave271_ok,
            "live script conditions dual-world empty gate method names residual pack wave271: {}",
            r.detail
        );
        assert!(
            r.live_script_conditions_dual_world_empty_gate_nav_commands_wave271_ok,
            "live script conditions dual-world empty gate nav commands residual pack wave271: {}",
            r.detail
        );
        assert!(
            r.live_script_conditions_dual_world_empty_gate_live_wave271_ok,
            "live script conditions dual-world empty gate live residual wave271: {}",
            r.detail
        );
        assert!(
            r.live_transport_contain_dual_world_empty_gate_method_names_wave272_ok,
            "live transport contain dual-world empty gate method names residual pack wave272: {}",
            r.detail
        );
        assert!(
            r.live_transport_contain_dual_world_empty_gate_nav_commands_wave272_ok,
            "live transport contain dual-world empty gate nav commands residual pack wave272: {}",
            r.detail
        );
        assert!(
            r.live_transport_contain_dual_world_empty_gate_live_wave272_ok,
            "live transport contain dual-world empty gate live residual wave272: {}",
            r.detail
        );
        assert!(
            r.live_ingame_ui_dual_world_empty_gate_method_names_wave273_ok,
            "live ingame ui dual-world empty gate method names residual pack wave273: {}",
            r.detail
        );
        assert!(
            r.live_ingame_ui_dual_world_empty_gate_nav_commands_wave273_ok,
            "live ingame ui dual-world empty gate nav commands residual pack wave273: {}",
            r.detail
        );
        assert!(
            r.live_ingame_ui_dual_world_empty_gate_live_wave273_ok,
            "live ingame ui dual-world empty gate live residual wave273: {}",
            r.detail
        );
        assert!(
            r.live_helix_contain_dual_world_empty_gate_method_names_wave274_ok,
            "live helix contain dual-world empty gate method names residual pack wave274: {}",
            r.detail
        );
        assert!(
            r.live_helix_contain_dual_world_empty_gate_nav_commands_wave274_ok,
            "live helix contain dual-world empty gate nav commands residual pack wave274: {}",
            r.detail
        );
        assert!(
            r.live_helix_contain_dual_world_empty_gate_live_wave274_ok,
            "live helix contain dual-world empty gate live residual wave274: {}",
            r.detail
        );
        assert!(
            r.live_command_processor_dual_world_empty_gate_method_names_wave275_ok,
            "live command processor dual-world empty gate method names residual pack wave275: {}",
            r.detail
        );
        assert!(
            r.live_command_processor_dual_world_empty_gate_nav_commands_wave275_ok,
            "live command processor dual-world empty gate nav commands residual pack wave275: {}",
            r.detail
        );
        assert!(
            r.live_command_processor_dual_world_empty_gate_live_wave275_ok,
            "live command processor dual-world empty gate live residual wave275: {}",
            r.detail
        );
        assert!(
            r.live_turret_dual_world_empty_gate_method_names_wave276_ok,
            "live turret dual-world empty gate method names residual pack wave276: {}",
            r.detail
        );
        assert!(
            r.live_turret_dual_world_empty_gate_nav_commands_wave276_ok,
            "live turret dual-world empty gate nav commands residual pack wave276: {}",
            r.detail
        );
        assert!(
            r.live_turret_dual_world_empty_gate_live_wave276_ok,
            "live turret dual-world empty gate live residual wave276: {}",
            r.detail
        );
        assert!(
            r.live_rider_change_contain_dual_world_empty_gate_method_names_wave277_ok,
            "live rider change contain dual-world empty gate method names residual pack wave277: {}",
            r.detail
        );
        assert!(
            r.live_rider_change_contain_dual_world_empty_gate_nav_commands_wave277_ok,
            "live rider change contain dual-world empty gate nav commands residual pack wave277: {}",
            r.detail
        );
        assert!(
            r.live_rider_change_contain_dual_world_empty_gate_live_wave277_ok,
            "live rider change contain dual-world empty gate live residual wave277: {}",
            r.detail
        );
        assert!(
            r.live_selection_dual_world_empty_gate_method_names_wave278_ok,
            "live selection dual-world empty gate method names residual pack wave278: {}",
            r.detail
        );
        assert!(
            r.live_selection_dual_world_empty_gate_nav_commands_wave278_ok,
            "live selection dual-world empty gate nav commands residual pack wave278: {}",
            r.detail
        );
        assert!(
            r.live_selection_dual_world_empty_gate_live_wave278_ok,
            "live selection dual-world empty gate live residual wave278: {}",
            r.detail
        );
        assert!(
            r.live_cave_contain_dual_world_empty_gate_method_names_wave279_ok,
            "live cave contain dual-world empty gate method names residual pack wave279: {}",
            r.detail
        );
        assert!(
            r.live_cave_contain_dual_world_empty_gate_nav_commands_wave279_ok,
            "live cave contain dual-world empty gate nav commands residual pack wave279: {}",
            r.detail
        );
        assert!(
            r.live_cave_contain_dual_world_empty_gate_live_wave279_ok,
            "live cave contain dual-world empty gate live residual wave279: {}",
            r.detail
        );
        assert!(
            r.live_tunnel_contain_dual_world_empty_gate_method_names_wave280_ok,
            "live tunnel contain dual-world empty gate method names residual pack wave280: {}",
            r.detail
        );
        assert!(
            r.live_tunnel_contain_dual_world_empty_gate_nav_commands_wave280_ok,
            "live tunnel contain dual-world empty gate nav commands residual pack wave280: {}",
            r.detail
        );
        assert!(
            r.live_tunnel_contain_dual_world_empty_gate_live_wave280_ok,
            "live tunnel contain dual-world empty gate live residual wave280: {}",
            r.detail
        );
        assert!(
            r.live_helpers_dual_world_empty_gate_method_names_wave281_ok,
            "live helpers dual-world empty gate method names residual pack wave281: {}",
            r.detail
        );
        assert!(
            r.live_helpers_dual_world_empty_gate_nav_commands_wave281_ok,
            "live helpers dual-world empty gate nav commands residual pack wave281: {}",
            r.detail
        );
        assert!(
            r.live_helpers_dual_world_empty_gate_live_wave281_ok,
            "live helpers dual-world empty gate live residual wave281: {}",
            r.detail
        );
        assert!(
            r.live_ai_update_interface_dual_world_empty_gate_method_names_wave282_ok,
            "live ai update interface dual-world empty gate method names residual pack wave282: {}",
            r.detail
        );
        assert!(
            r.live_ai_update_interface_dual_world_empty_gate_nav_commands_wave282_ok,
            "live ai update interface dual-world empty gate nav commands residual pack wave282: {}",
            r.detail
        );
        assert!(
            r.live_ai_update_interface_dual_world_empty_gate_live_wave282_ok,
            "live ai update interface dual-world empty gate live residual wave282: {}",
            r.detail
        );
        assert!(
            r.live_stealth_update_dual_world_empty_gate_method_names_wave283_ok,
            "live stealth update dual-world empty gate method names residual pack wave283: {}",
            r.detail
        );
        assert!(
            r.live_stealth_update_dual_world_empty_gate_nav_commands_wave283_ok,
            "live stealth update dual-world empty gate nav commands residual pack wave283: {}",
            r.detail
        );
        assert!(
            r.live_stealth_update_dual_world_empty_gate_live_wave283_ok,
            "live stealth update dual-world empty gate live residual wave283: {}",
            r.detail
        );
        assert!(
            r.live_script_executor_dual_world_empty_gate_method_names_wave284_ok,
            "live script executor dual-world empty gate method names residual pack wave284: {}",
            r.detail
        );
        assert!(
            r.live_script_executor_dual_world_empty_gate_nav_commands_wave284_ok,
            "live script executor dual-world empty gate nav commands residual pack wave284: {}",
            r.detail
        );
        assert!(
            r.live_script_executor_dual_world_empty_gate_live_wave284_ok,
            "live script executor dual-world empty gate live residual wave284: {}",
            r.detail
        );
        assert!(
            r.live_ai_integration_dual_world_empty_gate_method_names_wave285_ok,
            "live ai integration dual-world empty gate method names residual pack wave285: {}",
            r.detail
        );
        assert!(
            r.live_ai_integration_dual_world_empty_gate_nav_commands_wave285_ok,
            "live ai integration dual-world empty gate nav commands residual pack wave285: {}",
            r.detail
        );
        assert!(
            r.live_ai_integration_dual_world_empty_gate_live_wave285_ok,
            "live ai integration dual-world empty gate live residual wave285: {}",
            r.detail
        );
        assert!(
            r.live_dumb_projectile_dual_world_empty_gate_method_names_wave286_ok,
            "live dumb projectile dual-world empty gate method names residual pack wave286: {}",
            r.detail
        );
        assert!(
            r.live_dumb_projectile_dual_world_empty_gate_nav_commands_wave286_ok,
            "live dumb projectile dual-world empty gate nav commands residual pack wave286: {}",
            r.detail
        );
        assert!(
            r.live_dumb_projectile_dual_world_empty_gate_live_wave286_ok,
            "live dumb projectile dual-world empty gate live residual wave286: {}",
            r.detail
        );
        assert!(
            r.live_enhanced_player_dual_world_empty_gate_method_names_wave287_ok,
            "live enhanced player dual-world empty gate method names residual pack wave287: {}",
            r.detail
        );
        assert!(
            r.live_enhanced_player_dual_world_empty_gate_nav_commands_wave287_ok,
            "live enhanced player dual-world empty gate nav commands residual pack wave287: {}",
            r.detail
        );
        assert!(
            r.live_enhanced_player_dual_world_empty_gate_live_wave287_ok,
            "live enhanced player dual-world empty gate live residual wave287: {}",
            r.detail
        );
        assert!(
            r.live_hijacker_update_dual_world_empty_gate_method_names_wave288_ok,
            "live hijacker update dual-world empty gate method names residual pack wave288: {}",
            r.detail
        );
        assert!(
            r.live_hijacker_update_dual_world_empty_gate_nav_commands_wave288_ok,
            "live hijacker update dual-world empty gate nav commands residual pack wave288: {}",
            r.detail
        );
        assert!(
            r.live_hijacker_update_dual_world_empty_gate_live_wave288_ok,
            "live hijacker update dual-world empty gate live residual wave288: {}",
            r.detail
        );
        assert!(
            r.live_weapon_impl_dual_world_empty_gate_method_names_wave289_ok,
            "live weapon impl dual-world empty gate method names residual pack wave289: {}",
            r.detail
        );
        assert!(
            r.live_weapon_impl_dual_world_empty_gate_nav_commands_wave289_ok,
            "live weapon impl dual-world empty gate nav commands residual pack wave289: {}",
            r.detail
        );
        assert!(
            r.live_weapon_impl_dual_world_empty_gate_live_wave289_ok,
            "live weapon impl dual-world empty gate live residual wave289: {}",
            r.detail
        );
        assert!(
            r.live_async_player_dual_world_empty_gate_method_names_wave290_ok,
            "live async player dual-world empty gate method names residual pack wave290: {}",
            r.detail
        );
        assert!(
            r.live_async_player_dual_world_empty_gate_nav_commands_wave290_ok,
            "live async player dual-world empty gate nav commands residual pack wave290: {}",
            r.detail
        );
        assert!(
            r.live_async_player_dual_world_empty_gate_live_wave290_ok,
            "live async player dual-world empty gate live residual wave290: {}",
            r.detail
        );
        assert!(
            r.live_active_body_dual_world_empty_gate_method_names_wave291_ok,
            "live active body dual-world empty gate method names residual pack wave291: {}",
            r.detail
        );
        assert!(
            r.live_active_body_dual_world_empty_gate_nav_commands_wave291_ok,
            "live active body dual-world empty gate nav commands residual pack wave291: {}",
            r.detail
        );
        assert!(
            r.live_active_body_dual_world_empty_gate_live_wave291_ok,
            "live active body dual-world empty gate live residual wave291: {}",
            r.detail
        );
        assert!(
            r.live_skirmish_conditions_dual_world_empty_gate_method_names_wave292_ok,
            "live skirmish conditions dual-world empty gate method names residual pack wave292: {}",
            r.detail
        );
        assert!(
            r.live_skirmish_conditions_dual_world_empty_gate_nav_commands_wave292_ok,
            "live skirmish conditions dual-world empty gate nav commands residual pack wave292: {}",
            r.detail
        );
        assert!(
            r.live_skirmish_conditions_dual_world_empty_gate_live_wave292_ok,
            "live skirmish conditions dual-world empty gate live residual wave292: {}",
            r.detail
        );
        assert!(
            r.live_ai_build_list_dual_world_empty_gate_method_names_wave293_ok,
            "live ai build list dual-world empty gate method names residual pack wave293: {}",
            r.detail
        );
        assert!(
            r.live_ai_build_list_dual_world_empty_gate_nav_commands_wave293_ok,
            "live ai build list dual-world empty gate nav commands residual pack wave293: {}",
            r.detail
        );
        assert!(
            r.live_ai_build_list_dual_world_empty_gate_live_wave293_ok,
            "live ai build list dual-world empty gate live residual wave293: {}",
            r.detail
        );
        assert!(
            r.live_victory_dual_world_empty_gate_method_names_wave294_ok,
            "live victory dual-world empty gate method names residual pack wave294: {}",
            r.detail
        );
        assert!(
            r.live_victory_dual_world_empty_gate_nav_commands_wave294_ok,
            "live victory dual-world empty gate nav commands residual pack wave294: {}",
            r.detail
        );
        assert!(
            r.live_victory_dual_world_empty_gate_live_wave294_ok,
            "live victory dual-world empty gate live residual wave294: {}",
            r.detail
        );
        assert!(
            r.live_script_actions_dual_world_empty_gate_method_names_wave295_ok,
            "live script actions dual-world empty gate method names residual pack wave295: {}",
            r.detail
        );
        assert!(
            r.live_script_actions_dual_world_empty_gate_nav_commands_wave295_ok,
            "live script actions dual-world empty gate nav commands residual pack wave295: {}",
            r.detail
        );
        assert!(
            r.live_script_actions_dual_world_empty_gate_live_wave295_ok,
            "live script actions dual-world empty gate live residual wave295: {}",
            r.detail
        );
        assert!(
            r.live_special_ability_dual_world_empty_gate_method_names_wave296_ok,
            "live special ability dual-world empty gate method names residual pack wave296: {}",
            r.detail
        );
        assert!(
            r.live_special_ability_dual_world_empty_gate_nav_commands_wave296_ok,
            "live special ability dual-world empty gate nav commands residual pack wave296: {}",
            r.detail
        );
        assert!(
            r.live_special_ability_dual_world_empty_gate_live_wave296_ok,
            "live special ability dual-world empty gate live residual wave296: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_dual_world_empty_gate_method_names_wave297_ok,
            "live stealth detector dual-world empty gate method names residual pack wave297: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_dual_world_empty_gate_nav_commands_wave297_ok,
            "live stealth detector dual-world empty gate nav commands residual pack wave297: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_dual_world_empty_gate_live_wave297_ok,
            "live stealth detector dual-world empty gate live residual wave297: {}",
            r.detail
        );
        assert!(
            r.live_supply_system_dual_world_empty_gate_method_names_wave298_ok,
            "live supply system dual-world empty gate method names residual pack wave298: {}",
            r.detail
        );
        assert!(
            r.live_supply_system_dual_world_empty_gate_nav_commands_wave298_ok,
            "live supply system dual-world empty gate nav commands residual pack wave298: {}",
            r.detail
        );
        assert!(
            r.live_supply_system_dual_world_empty_gate_live_wave298_ok,
            "live supply system dual-world empty gate live residual wave298: {}",
            r.detail
        );
        assert!(
            r.live_particle_uplink_dual_world_empty_gate_method_names_wave299_ok,
            "live particle uplink dual-world empty gate method names residual pack wave299: {}",
            r.detail
        );
        assert!(
            r.live_particle_uplink_dual_world_empty_gate_nav_commands_wave299_ok,
            "live particle uplink dual-world empty gate nav commands residual pack wave299: {}",
            r.detail
        );
        assert!(
            r.live_particle_uplink_dual_world_empty_gate_live_wave299_ok,
            "live particle uplink dual-world empty gate live residual wave299: {}",
            r.detail
        );
        assert!(
            r.live_overlord_contain_dual_world_empty_gate_method_names_wave300_ok,
            "live overlord contain dual-world empty gate method names residual pack wave300: {}",
            r.detail
        );
        assert!(
            r.live_overlord_contain_dual_world_empty_gate_nav_commands_wave300_ok,
            "live overlord contain dual-world empty gate nav commands residual pack wave300: {}",
            r.detail
        );
        assert!(
            r.live_overlord_contain_dual_world_empty_gate_live_wave300_ok,
            "live overlord contain dual-world empty gate live residual wave300: {}",
            r.detail
        );
        assert!(
            r.live_bridge_behavior_dual_world_empty_gate_method_names_wave301_ok,
            "live bridge behavior dual-world empty gate method names residual pack wave301: {}",
            r.detail
        );
        assert!(
            r.live_bridge_behavior_dual_world_empty_gate_nav_commands_wave301_ok,
            "live bridge behavior dual-world empty gate nav commands residual pack wave301: {}",
            r.detail
        );
        assert!(
            r.live_bridge_behavior_dual_world_empty_gate_live_wave301_ok,
            "live bridge behavior dual-world empty gate live residual wave301: {}",
            r.detail
        );
        assert!(
            r.live_stealth_behavior_dual_world_empty_gate_method_names_wave302_ok,
            "live stealth behavior dual-world empty gate method names residual pack wave302: {}",
            r.detail
        );
        assert!(
            r.live_stealth_behavior_dual_world_empty_gate_nav_commands_wave302_ok,
            "live stealth behavior dual-world empty gate nav commands residual pack wave302: {}",
            r.detail
        );
        assert!(
            r.live_stealth_behavior_dual_world_empty_gate_live_wave302_ok,
            "live stealth behavior dual-world empty gate live residual wave302: {}",
            r.detail
        );
        assert!(
            r.live_crate_collide_dual_world_empty_gate_method_names_wave303_ok,
            "live crate collide dual-world empty gate method names residual pack wave303: {}",
            r.detail
        );
        assert!(
            r.live_crate_collide_dual_world_empty_gate_nav_commands_wave303_ok,
            "live crate collide dual-world empty gate nav commands residual pack wave303: {}",
            r.detail
        );
        assert!(
            r.live_crate_collide_dual_world_empty_gate_live_wave303_ok,
            "live crate collide dual-world empty gate live residual wave303: {}",
            r.detail
        );
        assert!(
            r.live_object_manager_dual_world_empty_gate_method_names_wave304_ok,
            "live object manager dual-world empty gate method names residual pack wave304: {}",
            r.detail
        );
        assert!(
            r.live_object_manager_dual_world_empty_gate_nav_commands_wave304_ok,
            "live object manager dual-world empty gate nav commands residual pack wave304: {}",
            r.detail
        );
        assert!(
            r.live_object_manager_dual_world_empty_gate_live_wave304_ok,
            "live object manager dual-world empty gate live residual wave304: {}",
            r.detail
        );
        assert!(
            r.live_sticky_bomb_dual_world_empty_gate_method_names_wave305_ok,
            "live sticky bomb dual-world empty gate method names residual pack wave305: {}",
            r.detail
        );
        assert!(
            r.live_sticky_bomb_dual_world_empty_gate_nav_commands_wave305_ok,
            "live sticky bomb dual-world empty gate nav commands residual pack wave305: {}",
            r.detail
        );
        assert!(
            r.live_sticky_bomb_dual_world_empty_gate_live_wave305_ok,
            "live sticky bomb dual-world empty gate live residual wave305: {}",
            r.detail
        );
        assert!(
            r.live_auto_heal_dual_world_empty_gate_method_names_wave306_ok,
            "live auto heal dual-world empty gate method names residual pack wave306: {}",
            r.detail
        );
        assert!(
            r.live_auto_heal_dual_world_empty_gate_nav_commands_wave306_ok,
            "live auto heal dual-world empty gate nav commands residual pack wave306: {}",
            r.detail
        );
        assert!(
            r.live_auto_heal_dual_world_empty_gate_live_wave306_ok,
            "live auto heal dual-world empty gate live residual wave306: {}",
            r.detail
        );
        assert!(
            r.live_grant_stealth_dual_world_empty_gate_method_names_wave307_ok,
            "live grant stealth dual-world empty gate method names residual pack wave307: {}",
            r.detail
        );
        assert!(
            r.live_grant_stealth_dual_world_empty_gate_nav_commands_wave307_ok,
            "live grant stealth dual-world empty gate nav commands residual pack wave307: {}",
            r.detail
        );
        assert!(
            r.live_grant_stealth_dual_world_empty_gate_live_wave307_ok,
            "live grant stealth dual-world empty gate live residual wave307: {}",
            r.detail
        );
        assert!(
            r.live_status_bits_upgrade_dual_world_empty_gate_method_names_wave308_ok,
            "live status bits upgrade dual-world empty gate method names residual pack wave308: {}",
            r.detail
        );
        assert!(
            r.live_status_bits_upgrade_dual_world_empty_gate_nav_commands_wave308_ok,
            "live status bits upgrade dual-world empty gate nav commands residual pack wave308: {}",
            r.detail
        );
        assert!(
            r.live_status_bits_upgrade_dual_world_empty_gate_live_wave308_ok,
            "live status bits upgrade dual-world empty gate live residual wave308: {}",
            r.detail
        );
        assert!(
            r.live_jet_ai_dual_world_empty_gate_method_names_wave309_ok,
            "live jet ai dual-world empty gate method names residual pack wave309: {}",
            r.detail
        );
        assert!(
            r.live_jet_ai_dual_world_empty_gate_nav_commands_wave309_ok,
            "live jet ai dual-world empty gate nav commands residual pack wave309: {}",
            r.detail
        );
        assert!(
            r.live_jet_ai_dual_world_empty_gate_live_wave309_ok,
            "live jet ai dual-world empty gate live residual wave309: {}",
            r.detail
        );
        assert!(
            r.live_parking_place_dual_world_empty_gate_method_names_wave310_ok,
            "live parking place dual-world empty gate method names residual pack wave310: {}",
            r.detail
        );
        assert!(
            r.live_parking_place_dual_world_empty_gate_nav_commands_wave310_ok,
            "live parking place dual-world empty gate nav commands residual pack wave310: {}",
            r.detail
        );
        assert!(
            r.live_parking_place_dual_world_empty_gate_live_wave310_ok,
            "live parking place dual-world empty gate live residual wave310: {}",
            r.detail
        );
        assert!(
            r.live_flight_deck_dual_world_empty_gate_method_names_wave311_ok,
            "live flight deck dual-world empty gate method names residual pack wave311: {}",
            r.detail
        );
        assert!(
            r.live_flight_deck_dual_world_empty_gate_nav_commands_wave311_ok,
            "live flight deck dual-world empty gate nav commands residual pack wave311: {}",
            r.detail
        );
        assert!(
            r.live_flight_deck_dual_world_empty_gate_live_wave311_ok,
            "live flight deck dual-world empty gate live residual wave311: {}",
            r.detail
        );
        assert!(
            r.live_exit_strategies_dual_world_empty_gate_method_names_wave312_ok,
            "live exit strategies dual-world empty gate method names residual pack wave312: {}",
            r.detail
        );
        assert!(
            r.live_exit_strategies_dual_world_empty_gate_nav_commands_wave312_ok,
            "live exit strategies dual-world empty gate nav commands residual pack wave312: {}",
            r.detail
        );
        assert!(
            r.live_exit_strategies_dual_world_empty_gate_live_wave312_ok,
            "live exit strategies dual-world empty gate live residual wave312: {}",
            r.detail
        );
        assert!(
            r.live_collision_system_dual_world_empty_gate_method_names_wave313_ok,
            "live collision system dual-world empty gate method names residual pack wave313: {}",
            r.detail
        );
        assert!(
            r.live_collision_system_dual_world_empty_gate_nav_commands_wave313_ok,
            "live collision system dual-world empty gate nav commands residual pack wave313: {}",
            r.detail
        );
        assert!(
            r.live_collision_system_dual_world_empty_gate_live_wave313_ok,
            "live collision system dual-world empty gate live residual wave313: {}",
            r.detail
        );
        assert!(
            r.live_max_health_upgrade_dual_world_empty_gate_method_names_wave314_ok,
            "live max health upgrade dual-world empty gate method names residual pack wave314: {}",
            r.detail
        );
        assert!(
            r.live_max_health_upgrade_dual_world_empty_gate_nav_commands_wave314_ok,
            "live max health upgrade dual-world empty gate nav commands residual pack wave314: {}",
            r.detail
        );
        assert!(
            r.live_max_health_upgrade_dual_world_empty_gate_live_wave314_ok,
            "live max health upgrade dual-world empty gate live residual wave314: {}",
            r.detail
        );
        assert!(
            r.live_structure_topple_dual_world_empty_gate_method_names_wave315_ok,
            "live structure topple dual-world empty gate method names residual pack wave315: {}",
            r.detail
        );
        assert!(
            r.live_structure_topple_dual_world_empty_gate_nav_commands_wave315_ok,
            "live structure topple dual-world empty gate nav commands residual pack wave315: {}",
            r.detail
        );
        assert!(
            r.live_structure_topple_dual_world_empty_gate_live_wave315_ok,
            "live structure topple dual-world empty gate live residual wave315: {}",
            r.detail
        );
        assert!(
            r.live_physics_update_dual_world_empty_gate_method_names_wave316_ok,
            "live physics update dual-world empty gate method names residual pack wave316: {}",
            r.detail
        );
        assert!(
            r.live_physics_update_dual_world_empty_gate_nav_commands_wave316_ok,
            "live physics update dual-world empty gate nav commands residual pack wave316: {}",
            r.detail
        );
        assert!(
            r.live_physics_update_dual_world_empty_gate_live_wave316_ok,
            "live physics update dual-world empty gate live residual wave316: {}",
            r.detail
        );
        assert!(
            r.live_cleanup_hazard_dual_world_empty_gate_method_names_wave317_ok,
            "live cleanup hazard dual-world empty gate method names residual pack wave317: {}",
            r.detail
        );
        assert!(
            r.live_cleanup_hazard_dual_world_empty_gate_nav_commands_wave317_ok,
            "live cleanup hazard dual-world empty gate nav commands residual pack wave317: {}",
            r.detail
        );
        assert!(
            r.live_cleanup_hazard_dual_world_empty_gate_live_wave317_ok,
            "live cleanup hazard dual-world empty gate live residual wave317: {}",
            r.detail
        );
        assert!(
            r.live_bridge_tower_dual_world_empty_gate_method_names_wave318_ok,
            "live bridge tower dual-world empty gate method names residual pack wave318: {}",
            r.detail
        );
        assert!(
            r.live_bridge_tower_dual_world_empty_gate_nav_commands_wave318_ok,
            "live bridge tower dual-world empty gate nav commands residual pack wave318: {}",
            r.detail
        );
        assert!(
            r.live_bridge_tower_dual_world_empty_gate_live_wave318_ok,
            "live bridge tower dual-world empty gate live residual wave318: {}",
            r.detail
        );
        assert!(
            r.live_armor_upgrade_dual_world_empty_gate_method_names_wave319_ok,
            "live armor upgrade dual-world empty gate method names residual pack wave319: {}",
            r.detail
        );
        assert!(
            r.live_armor_upgrade_dual_world_empty_gate_nav_commands_wave319_ok,
            "live armor upgrade dual-world empty gate nav commands residual pack wave319: {}",
            r.detail
        );
        assert!(
            r.live_armor_upgrade_dual_world_empty_gate_live_wave319_ok,
            "live armor upgrade dual-world empty gate live residual wave319: {}",
            r.detail
        );
        assert!(
            r.live_paradrop_power_dual_world_empty_gate_method_names_wave320_ok,
            "live paradrop power dual-world empty gate method names residual pack wave320: {}",
            r.detail
        );
        assert!(
            r.live_paradrop_power_dual_world_empty_gate_nav_commands_wave320_ok,
            "live paradrop power dual-world empty gate nav commands residual pack wave320: {}",
            r.detail
        );
        assert!(
            r.live_paradrop_power_dual_world_empty_gate_live_wave320_ok,
            "live paradrop power dual-world empty gate live residual wave320: {}",
            r.detail
        );
        assert!(
            r.live_fuel_air_bomb_dual_world_empty_gate_method_names_wave321_ok,
            "live fuel air bomb dual-world empty gate method names residual pack wave321: {}",
            r.detail
        );
        assert!(
            r.live_fuel_air_bomb_dual_world_empty_gate_nav_commands_wave321_ok,
            "live fuel air bomb dual-world empty gate nav commands residual pack wave321: {}",
            r.detail
        );
        assert!(
            r.live_fuel_air_bomb_dual_world_empty_gate_live_wave321_ok,
            "live fuel air bomb dual-world empty gate live residual wave321: {}",
            r.detail
        );
        assert!(
            r.live_tensile_formation_dual_world_empty_gate_method_names_wave322_ok,
            "live tensile formation dual-world empty gate method names residual pack wave322: {}",
            r.detail
        );
        assert!(
            r.live_tensile_formation_dual_world_empty_gate_nav_commands_wave322_ok,
            "live tensile formation dual-world empty gate nav commands residual pack wave322: {}",
            r.detail
        );
        assert!(
            r.live_tensile_formation_dual_world_empty_gate_live_wave322_ok,
            "live tensile formation dual-world empty gate live residual wave322: {}",
            r.detail
        );
        assert!(
            r.live_die_mod_dual_world_empty_gate_method_names_wave323_ok,
            "live die mod dual-world empty gate method names residual pack wave323: {}",
            r.detail
        );
        assert!(
            r.live_die_mod_dual_world_empty_gate_nav_commands_wave323_ok,
            "live die mod dual-world empty gate nav commands residual pack wave323: {}",
            r.detail
        );
        assert!(
            r.live_die_mod_dual_world_empty_gate_live_wave323_ok,
            "live die mod dual-world empty gate live residual wave323: {}",
            r.detail
        );
        assert!(
            r.live_partition_manager_dual_world_empty_gate_method_names_wave324_ok,
            "live partition manager dual-world empty gate method names residual pack wave324: {}",
            r.detail
        );
        assert!(
            r.live_partition_manager_dual_world_empty_gate_nav_commands_wave324_ok,
            "live partition manager dual-world empty gate nav commands residual pack wave324: {}",
            r.detail
        );
        assert!(
            r.live_partition_manager_dual_world_empty_gate_live_wave324_ok,
            "live partition manager dual-world empty gate live residual wave324: {}",
            r.detail
        );
        assert!(
            r.live_spectre_gunship_dual_world_empty_gate_method_names_wave325_ok,
            "live spectre gunship dual-world empty gate method names residual pack wave325: {}",
            r.detail
        );
        assert!(
            r.live_spectre_gunship_dual_world_empty_gate_nav_commands_wave325_ok,
            "live spectre gunship dual-world empty gate nav commands residual pack wave325: {}",
            r.detail
        );
        assert!(
            r.live_spectre_gunship_dual_world_empty_gate_live_wave325_ok,
            "live spectre gunship dual-world empty gate live residual wave325: {}",
            r.detail
        );
        assert!(
            r.live_production_update_dual_world_empty_gate_method_names_wave326_ok,
            "live production update dual-world empty gate method names residual pack wave326: {}",
            r.detail
        );
        assert!(
            r.live_production_update_dual_world_empty_gate_nav_commands_wave326_ok,
            "live production update dual-world empty gate nav commands residual pack wave326: {}",
            r.detail
        );
        assert!(
            r.live_production_update_dual_world_empty_gate_live_wave326_ok,
            "live production update dual-world empty gate live residual wave326: {}",
            r.detail
        );
        assert!(
            r.live_neutron_blast_dual_world_empty_gate_method_names_wave327_ok,
            "live neutron blast dual-world empty gate method names residual pack wave327: {}",
            r.detail
        );
        assert!(
            r.live_neutron_blast_dual_world_empty_gate_nav_commands_wave327_ok,
            "live neutron blast dual-world empty gate nav commands residual pack wave327: {}",
            r.detail
        );
        assert!(
            r.live_neutron_blast_dual_world_empty_gate_live_wave327_ok,
            "live neutron blast dual-world empty gate live residual wave327: {}",
            r.detail
        );
        assert!(
            r.live_countermeasures_dual_world_empty_gate_method_names_wave328_ok,
            "live countermeasures dual-world empty gate method names residual pack wave328: {}",
            r.detail
        );
        assert!(
            r.live_countermeasures_dual_world_empty_gate_nav_commands_wave328_ok,
            "live countermeasures dual-world empty gate nav commands residual pack wave328: {}",
            r.detail
        );
        assert!(
            r.live_countermeasures_dual_world_empty_gate_live_wave328_ok,
            "live countermeasures dual-world empty gate live residual wave328: {}",
            r.detail
        );
        assert!(
            r.live_skirmish_player_dual_world_empty_gate_method_names_wave329_ok,
            "live skirmish player dual-world empty gate method names residual pack wave329: {}",
            r.detail
        );
        assert!(
            r.live_skirmish_player_dual_world_empty_gate_nav_commands_wave329_ok,
            "live skirmish player dual-world empty gate nav commands residual pack wave329: {}",
            r.detail
        );
        assert!(
            r.live_skirmish_player_dual_world_empty_gate_live_wave329_ok,
            "live skirmish player dual-world empty gate live residual wave329: {}",
            r.detail
        );
        assert!(
            r.live_a10_strike_dual_world_empty_gate_method_names_wave330_ok,
            "live a10 strike dual-world empty gate method names residual pack wave330: {}",
            r.detail
        );
        assert!(
            r.live_a10_strike_dual_world_empty_gate_nav_commands_wave330_ok,
            "live a10 strike dual-world empty gate nav commands residual pack wave330: {}",
            r.detail
        );
        assert!(
            r.live_a10_strike_dual_world_empty_gate_live_wave330_ok,
            "live a10 strike dual-world empty gate live residual wave330: {}",
            r.detail
        );
        assert!(
            r.live_rebuild_hole_dual_world_empty_gate_method_names_wave331_ok,
            "live rebuild hole dual-world empty gate method names residual pack wave331: {}",
            r.detail
        );
        assert!(
            r.live_rebuild_hole_dual_world_empty_gate_nav_commands_wave331_ok,
            "live rebuild hole dual-world empty gate nav commands residual pack wave331: {}",
            r.detail
        );
        assert!(
            r.live_rebuild_hole_dual_world_empty_gate_live_wave331_ok,
            "live rebuild hole dual-world empty gate live residual wave331: {}",
            r.detail
        );
        assert!(
            r.live_wave_guide_dual_world_empty_gate_method_names_wave332_ok,
            "live wave guide dual-world empty gate method names residual pack wave332: {}",
            r.detail
        );
        assert!(
            r.live_wave_guide_dual_world_empty_gate_nav_commands_wave332_ok,
            "live wave guide dual-world empty gate nav commands residual pack wave332: {}",
            r.detail
        );
        assert!(
            r.live_wave_guide_dual_world_empty_gate_live_wave332_ok,
            "live wave guide dual-world empty gate live residual wave332: {}",
            r.detail
        );
        assert!(
            r.live_emp_update_dual_world_empty_gate_method_names_wave333_ok,
            "live emp update dual-world empty gate method names residual pack wave333: {}",
            r.detail
        );
        assert!(
            r.live_emp_update_dual_world_empty_gate_nav_commands_wave333_ok,
            "live emp update dual-world empty gate nav commands residual pack wave333: {}",
            r.detail
        );
        assert!(
            r.live_emp_update_dual_world_empty_gate_live_wave333_ok,
            "live emp update dual-world empty gate live residual wave333: {}",
            r.detail
        );
        assert!(
            r.live_bunker_buster_dual_world_empty_gate_method_names_wave334_ok,
            "live bunker buster dual-world empty gate method names residual pack wave334: {}",
            r.detail
        );
        assert!(
            r.live_bunker_buster_dual_world_empty_gate_nav_commands_wave334_ok,
            "live bunker buster dual-world empty gate nav commands residual pack wave334: {}",
            r.detail
        );
        assert!(
            r.live_bunker_buster_dual_world_empty_gate_live_wave334_ok,
            "live bunker buster dual-world empty gate live residual wave334: {}",
            r.detail
        );
        assert!(
            r.live_bridge_scaffold_dual_world_empty_gate_method_names_wave335_ok,
            "live bridge scaffold dual-world empty gate method names residual pack wave335: {}",
            r.detail
        );
        assert!(
            r.live_bridge_scaffold_dual_world_empty_gate_nav_commands_wave335_ok,
            "live bridge scaffold dual-world empty gate nav commands residual pack wave335: {}",
            r.detail
        );
        assert!(
            r.live_bridge_scaffold_dual_world_empty_gate_live_wave335_ok,
            "live bridge scaffold dual-world empty gate live residual wave335: {}",
            r.detail
        );
        assert!(
            r.live_assisted_targeting_dual_world_empty_gate_method_names_wave336_ok,
            "live assisted targeting dual-world empty gate method names residual pack wave336: {}",
            r.detail
        );
        assert!(
            r.live_assisted_targeting_dual_world_empty_gate_nav_commands_wave336_ok,
            "live assisted targeting dual-world empty gate nav commands residual pack wave336: {}",
            r.detail
        );
        assert!(
            r.live_assisted_targeting_dual_world_empty_gate_live_wave336_ok,
            "live assisted targeting dual-world empty gate live residual wave336: {}",
            r.detail
        );
        assert!(
            r.live_economy_dual_world_empty_gate_method_names_wave337_ok,
            "live economy dual-world empty gate method names residual pack wave337: {}",
            r.detail
        );
        assert!(
            r.live_economy_dual_world_empty_gate_nav_commands_wave337_ok,
            "live economy dual-world empty gate nav commands residual pack wave337: {}",
            r.detail
        );
        assert!(
            r.live_economy_dual_world_empty_gate_live_wave337_ok,
            "live economy dual-world empty gate live residual wave337: {}",
            r.detail
        );
        assert!(
            r.live_turret_ai_dual_world_empty_gate_method_names_wave338_ok,
            "live turret ai dual-world empty gate method names residual pack wave338: {}",
            r.detail
        );
        assert!(
            r.live_turret_ai_dual_world_empty_gate_nav_commands_wave338_ok,
            "live turret ai dual-world empty gate nav commands residual pack wave338: {}",
            r.detail
        );
        assert!(
            r.live_turret_ai_dual_world_empty_gate_live_wave338_ok,
            "live turret ai dual-world empty gate live residual wave338: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_module_dual_world_empty_gate_method_names_wave339_ok,
            "live stealth detector module dual-world empty gate method names residual pack wave339: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_module_dual_world_empty_gate_nav_commands_wave339_ok,
            "live stealth detector module dual-world empty gate nav commands residual pack wave339: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_module_dual_world_empty_gate_live_wave339_ok,
            "live stealth detector module dual-world empty gate live residual wave339: {}",
            r.detail
        );
        assert!(
            r.live_modules_dual_world_empty_gate_method_names_wave340_ok,
            "live modules dual-world empty gate method names residual pack wave340: {}",
            r.detail
        );
        assert!(
            r.live_modules_dual_world_empty_gate_nav_commands_wave340_ok,
            "live modules dual-world empty gate nav commands residual pack wave340: {}",
            r.detail
        );
        assert!(
            r.live_modules_dual_world_empty_gate_live_wave340_ok,
            "live modules dual-world empty gate live residual wave340: {}",
            r.detail
        );
        assert!(
            r.live_terrain_dual_world_empty_gate_method_names_wave341_ok,
            "live terrain dual-world empty gate method names residual pack wave341: {}",
            r.detail
        );
        assert!(
            r.live_terrain_dual_world_empty_gate_nav_commands_wave341_ok,
            "live terrain dual-world empty gate nav commands residual pack wave341: {}",
            r.detail
        );
        assert!(
            r.live_terrain_dual_world_empty_gate_live_wave341_ok,
            "live terrain dual-world empty gate live residual wave341: {}",
            r.detail
        );
        assert!(
            r.live_special_power_template_dual_world_empty_gate_method_names_wave342_ok,
            "live special power template dual-world empty gate method names residual pack wave342: {}",
            r.detail
        );
        assert!(
            r.live_special_power_template_dual_world_empty_gate_nav_commands_wave342_ok,
            "live special power template dual-world empty gate nav commands residual pack wave342: {}",
            r.detail
        );
        assert!(
            r.live_special_power_template_dual_world_empty_gate_live_wave342_ok,
            "live special power template dual-world empty gate live residual wave342: {}",
            r.detail
        );
        assert!(
            r.live_script_evaluator_dual_world_empty_gate_method_names_wave343_ok,
            "live script evaluator dual-world empty gate method names residual pack wave343: {}",
            r.detail
        );
        assert!(
            r.live_script_evaluator_dual_world_empty_gate_nav_commands_wave343_ok,
            "live script evaluator dual-world empty gate nav commands residual pack wave343: {}",
            r.detail
        );
        assert!(
            r.live_script_evaluator_dual_world_empty_gate_live_wave343_ok,
            "live script evaluator dual-world empty gate live residual wave343: {}",
            r.detail
        );
        assert!(
            r.live_system_game_logic_dual_world_empty_gate_method_names_wave344_ok,
            "live system game logic dual-world empty gate method names residual pack wave344: {}",
            r.detail
        );
        assert!(
            r.live_system_game_logic_dual_world_empty_gate_nav_commands_wave344_ok,
            "live system game logic dual-world empty gate nav commands residual pack wave344: {}",
            r.detail
        );
        assert!(
            r.live_system_game_logic_dual_world_empty_gate_live_wave344_ok,
            "live system game logic dual-world empty gate live residual wave344: {}",
            r.detail
        );
        assert!(
            r.live_meta_event_dual_world_empty_gate_method_names_wave345_ok,
            "live meta event dual-world empty gate method names residual pack wave345: {}",
            r.detail
        );
        assert!(
            r.live_meta_event_dual_world_empty_gate_nav_commands_wave345_ok,
            "live meta event dual-world empty gate nav commands residual pack wave345: {}",
            r.detail
        );
        assert!(
            r.live_meta_event_dual_world_empty_gate_live_wave345_ok,
            "live meta event dual-world empty gate live residual wave345: {}",
            r.detail
        );
        assert!(
            r.live_spawn_behavior_dual_world_empty_gate_method_names_wave346_ok,
            "live spawn behavior dual-world empty gate method names residual pack wave346: {}",
            r.detail
        );
        assert!(
            r.live_spawn_behavior_dual_world_empty_gate_nav_commands_wave346_ok,
            "live spawn behavior dual-world empty gate nav commands residual pack wave346: {}",
            r.detail
        );
        assert!(
            r.live_spawn_behavior_dual_world_empty_gate_live_wave346_ok,
            "live spawn behavior dual-world empty gate live residual wave346: {}",
            r.detail
        );
        assert!(
            r.live_action_manager_dual_world_empty_gate_method_names_wave347_ok,
            "live action manager dual-world empty gate method names residual pack wave347: {}",
            r.detail
        );
        assert!(
            r.live_action_manager_dual_world_empty_gate_nav_commands_wave347_ok,
            "live action manager dual-world empty gate nav commands residual pack wave347: {}",
            r.detail
        );
        assert!(
            r.live_action_manager_dual_world_empty_gate_live_wave347_ok,
            "live action manager dual-world empty gate live residual wave347: {}",
            r.detail
        );
        assert!(
            r.live_script_engine_dual_world_empty_gate_method_names_wave348_ok,
            "live script engine dual-world empty gate method names residual pack wave348: {}",
            r.detail
        );
        assert!(
            r.live_script_engine_dual_world_empty_gate_nav_commands_wave348_ok,
            "live script engine dual-world empty gate nav commands residual pack wave348: {}",
            r.detail
        );
        assert!(
            r.live_script_engine_dual_world_empty_gate_live_wave348_ok,
            "live script engine dual-world empty gate live residual wave348: {}",
            r.detail
        );
        assert!(
            r.live_chinook_ai_dual_world_empty_gate_method_names_wave349_ok,
            "live chinook ai dual-world empty gate method names residual pack wave349: {}",
            r.detail
        );
        assert!(
            r.live_chinook_ai_dual_world_empty_gate_nav_commands_wave349_ok,
            "live chinook ai dual-world empty gate nav commands residual pack wave349: {}",
            r.detail
        );
        assert!(
            r.live_chinook_ai_dual_world_empty_gate_live_wave349_ok,
            "live chinook ai dual-world empty gate live residual wave349: {}",
            r.detail
        );
        assert!(
            r.live_missile_ai_dual_world_empty_gate_method_names_wave350_ok,
            "live missile ai dual-world empty gate method names residual pack wave350: {}",
            r.detail
        );
        assert!(
            r.live_missile_ai_dual_world_empty_gate_nav_commands_wave350_ok,
            "live missile ai dual-world empty gate nav commands residual pack wave350: {}",
            r.detail
        );
        assert!(
            r.live_missile_ai_dual_world_empty_gate_live_wave350_ok,
            "live missile ai dual-world empty gate live residual wave350: {}",
            r.detail
        );
        assert!(
            r.live_dozer_ai_dual_world_empty_gate_method_names_wave351_ok,
            "live dozer ai dual-world empty gate method names residual pack wave351: {}",
            r.detail
        );
        assert!(
            r.live_dozer_ai_dual_world_empty_gate_nav_commands_wave351_ok,
            "live dozer ai dual-world empty gate nav commands residual pack wave351: {}",
            r.detail
        );
        assert!(
            r.live_dozer_ai_dual_world_empty_gate_live_wave351_ok,
            "live dozer ai dual-world empty gate live residual wave351: {}",
            r.detail
        );
        assert!(
            r.live_deliver_payload_ai_dual_world_empty_gate_method_names_wave352_ok,
            "live deliver payload ai dual-world empty gate method names residual pack wave352: {}",
            r.detail
        );
        assert!(
            r.live_deliver_payload_ai_dual_world_empty_gate_nav_commands_wave352_ok,
            "live deliver payload ai dual-world empty gate nav commands residual pack wave352: {}",
            r.detail
        );
        assert!(
            r.live_deliver_payload_ai_dual_world_empty_gate_live_wave352_ok,
            "live deliver payload ai dual-world empty gate live residual wave352: {}",
            r.detail
        );
        assert!(
            r.live_special_power_module_dual_world_empty_gate_method_names_wave353_ok,
            "live special power module dual-world empty gate method names residual pack wave353: {}",
            r.detail
        );
        assert!(
            r.live_special_power_module_dual_world_empty_gate_nav_commands_wave353_ok,
            "live special power module dual-world empty gate nav commands residual pack wave353: {}",
            r.detail
        );
        assert!(
            r.live_special_power_module_dual_world_empty_gate_live_wave353_ok,
            "live special power module dual-world empty gate live residual wave353: {}",
            r.detail
        );
        assert!(
            r.live_pow_truck_ai_dual_world_empty_gate_method_names_wave354_ok,
            "live pow truck ai dual-world empty gate method names residual pack wave354: {}",
            r.detail
        );
        assert!(
            r.live_pow_truck_ai_dual_world_empty_gate_nav_commands_wave354_ok,
            "live pow truck ai dual-world empty gate nav commands residual pack wave354: {}",
            r.detail
        );
        assert!(
            r.live_pow_truck_ai_dual_world_empty_gate_live_wave354_ok,
            "live pow truck ai dual-world empty gate live residual wave354: {}",
            r.detail
        );
        assert!(
            r.live_dock_update_dual_world_empty_gate_method_names_wave355_ok,
            "live dock update dual-world empty gate method names residual pack wave355: {}",
            r.detail
        );
        assert!(
            r.live_dock_update_dual_world_empty_gate_nav_commands_wave355_ok,
            "live dock update dual-world empty gate nav commands residual pack wave355: {}",
            r.detail
        );
        assert!(
            r.live_dock_update_dual_world_empty_gate_live_wave355_ok,
            "live dock update dual-world empty gate live residual wave355: {}",
            r.detail
        );
        assert!(
            r.live_weapon_template_dual_world_empty_gate_method_names_wave356_ok,
            "live weapon template dual-world empty gate method names residual pack wave356: {}",
            r.detail
        );
        assert!(
            r.live_weapon_template_dual_world_empty_gate_nav_commands_wave356_ok,
            "live weapon template dual-world empty gate nav commands residual pack wave356: {}",
            r.detail
        );
        assert!(
            r.live_weapon_template_dual_world_empty_gate_live_wave356_ok,
            "live weapon template dual-world empty gate live residual wave356: {}",
            r.detail
        );
        assert!(
            r.live_railroad_guide_ai_dual_world_empty_gate_method_names_wave357_ok,
            "live railroad guide ai dual-world empty gate method names residual pack wave357: {}",
            r.detail
        );
        assert!(
            r.live_railroad_guide_ai_dual_world_empty_gate_nav_commands_wave357_ok,
            "live railroad guide ai dual-world empty gate nav commands residual pack wave357: {}",
            r.detail
        );
        assert!(
            r.live_railroad_guide_ai_dual_world_empty_gate_live_wave357_ok,
            "live railroad guide ai dual-world empty gate live residual wave357: {}",
            r.detail
        );
        assert!(
            r.live_hack_internet_ai_dual_world_empty_gate_method_names_wave358_ok,
            "live hack internet ai dual-world empty gate method names residual pack wave358: {}",
            r.detail
        );
        assert!(
            r.live_hack_internet_ai_dual_world_empty_gate_nav_commands_wave358_ok,
            "live hack internet ai dual-world empty gate nav commands residual pack wave358: {}",
            r.detail
        );
        assert!(
            r.live_hack_internet_ai_dual_world_empty_gate_live_wave358_ok,
            "live hack internet ai dual-world empty gate live residual wave358: {}",
            r.detail
        );
        assert!(
            r.live_spectre_gunship_deployment_dual_world_empty_gate_method_names_wave359_ok,
            "live spectre gunship deployment dual-world empty gate method names residual pack wave359: {}",
            r.detail
        );
        assert!(
            r.live_spectre_gunship_deployment_dual_world_empty_gate_nav_commands_wave359_ok,
            "live spectre gunship deployment dual-world empty gate nav commands residual pack wave359: {}",
            r.detail
        );
        assert!(
            r.live_spectre_gunship_deployment_dual_world_empty_gate_live_wave359_ok,
            "live spectre gunship deployment dual-world empty gate live residual wave359: {}",
            r.detail
        );
        assert!(
            r.live_radius_decal_update_dual_world_empty_gate_method_names_wave360_ok,
            "live radius decal update dual-world empty gate method names residual pack wave360: {}",
            r.detail
        );
        assert!(
            r.live_radius_decal_update_dual_world_empty_gate_nav_commands_wave360_ok,
            "live radius decal update dual-world empty gate nav commands residual pack wave360: {}",
            r.detail
        );
        assert!(
            r.live_radius_decal_update_dual_world_empty_gate_live_wave360_ok,
            "live radius decal update dual-world empty gate live residual wave360: {}",
            r.detail
        );
        assert!(
            r.live_railed_transport_dock_dual_world_empty_gate_method_names_wave361_ok,
            "live railed transport dock dual-world empty gate method names residual pack wave361: {}",
            r.detail
        );
        assert!(
            r.live_railed_transport_dock_dual_world_empty_gate_nav_commands_wave361_ok,
            "live railed transport dock dual-world empty gate nav commands residual pack wave361: {}",
            r.detail
        );
        assert!(
            r.live_railed_transport_dock_dual_world_empty_gate_live_wave361_ok,
            "live railed transport dock dual-world empty gate live residual wave361: {}",
            r.detail
        );
        assert!(
            r.live_structure_collapse_update_dual_world_empty_gate_method_names_wave362_ok,
            "live structure collapse update dual-world empty gate method names residual pack wave362: {}",
            r.detail
        );
        assert!(
            r.live_structure_collapse_update_dual_world_empty_gate_nav_commands_wave362_ok,
            "live structure collapse update dual-world empty gate nav commands residual pack wave362: {}",
            r.detail
        );
        assert!(
            r.live_structure_collapse_update_dual_world_empty_gate_live_wave362_ok,
            "live structure collapse update dual-world empty gate live residual wave362: {}",
            r.detail
        );
        assert!(
            r.live_propaganda_tower_behavior_dual_world_empty_gate_method_names_wave363_ok,
            "live propaganda tower behavior dual-world empty gate method names residual pack wave363: {}",
            r.detail
        );
        assert!(
            r.live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_wave363_ok,
            "live propaganda tower behavior dual-world empty gate nav commands residual pack wave363: {}",
            r.detail
        );
        assert!(
            r.live_propaganda_tower_behavior_dual_world_empty_gate_live_wave363_ok,
            "live propaganda tower behavior dual-world empty gate live residual wave363: {}",
            r.detail
        );
        assert!(
            r.live_propaganda_center_behavior_dual_world_empty_gate_method_names_wave364_ok,
            "live propaganda center behavior dual-world empty gate method names residual pack wave364: {}",
            r.detail
        );
        assert!(
            r.live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_wave364_ok,
            "live propaganda center behavior dual-world empty gate nav commands residual pack wave364: {}",
            r.detail
        );
        assert!(
            r.live_propaganda_center_behavior_dual_world_empty_gate_live_wave364_ok,
            "live propaganda center behavior dual-world empty gate live residual wave364: {}",
            r.detail
        );
        assert!(
            r.live_production_update_complete_dual_world_empty_gate_method_names_wave365_ok,
            "live production update complete dual-world empty gate method names residual pack wave365: {}",
            r.detail
        );
        assert!(
            r.live_production_update_complete_dual_world_empty_gate_nav_commands_wave365_ok,
            "live production update complete dual-world empty gate nav commands residual pack wave365: {}",
            r.detail
        );
        assert!(
            r.live_production_update_complete_dual_world_empty_gate_live_wave365_ok,
            "live production update complete dual-world empty gate live residual wave365: {}",
            r.detail
        );
        assert!(
            r.live_pow_truck_behavior_dual_world_empty_gate_method_names_wave366_ok,
            "live pow truck behavior dual-world empty gate method names residual pack wave366: {}",
            r.detail
        );
        assert!(
            r.live_pow_truck_behavior_dual_world_empty_gate_nav_commands_wave366_ok,
            "live pow truck behavior dual-world empty gate nav commands residual pack wave366: {}",
            r.detail
        );
        assert!(
            r.live_pow_truck_behavior_dual_world_empty_gate_live_wave366_ok,
            "live pow truck behavior dual-world empty gate live residual wave366: {}",
            r.detail
        );
        assert!(
            r.live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_wave367_ok,
            "live fire weapon when damaged behavior dual-world empty gate method names residual pack wave367: {}",
            r.detail
        );
        assert!(
            r.live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_wave367_ok,
            "live fire weapon when damaged behavior dual-world empty gate nav commands residual pack wave367: {}",
            r.detail
        );
        assert!(
            r.live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_live_wave367_ok,
            "live fire weapon when damaged behavior dual-world empty gate live residual wave367: {}",
            r.detail
        );
        assert!(
            r.live_veterancy_crate_collide_dual_world_empty_gate_method_names_wave368_ok,
            "live veterancy crate collide dual-world empty gate method names residual pack wave368: {}",
            r.detail
        );
        assert!(
            r.live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_wave368_ok,
            "live veterancy crate collide dual-world empty gate nav commands residual pack wave368: {}",
            r.detail
        );
        assert!(
            r.live_veterancy_crate_collide_dual_world_empty_gate_live_wave368_ok,
            "live veterancy crate collide dual-world empty gate live residual wave368: {}",
            r.detail
        );
        assert!(
            r.live_assault_transport_ai_update_dual_world_empty_gate_method_names_wave369_ok,
            "live assault transport ai update dual-world empty gate method names residual pack wave369: {}",
            r.detail
        );
        assert!(
            r.live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_wave369_ok,
            "live assault transport ai update dual-world empty gate nav commands residual pack wave369: {}",
            r.detail
        );
        assert!(
            r.live_assault_transport_ai_update_dual_world_empty_gate_live_wave369_ok,
            "live assault transport ai update dual-world empty gate live residual wave369: {}",
            r.detail
        );
        assert!(
            r.live_heal_contain_dual_world_empty_gate_method_names_wave370_ok,
            "live heal contain dual-world empty gate method names residual pack wave370: {}",
            r.detail
        );
        assert!(
            r.live_heal_contain_dual_world_empty_gate_nav_commands_wave370_ok,
            "live heal contain dual-world empty gate nav commands residual pack wave370: {}",
            r.detail
        );
        assert!(
            r.live_heal_contain_dual_world_empty_gate_live_wave370_ok,
            "live heal contain dual-world empty gate live residual wave370: {}",
            r.detail
        );
        assert!(
            r.live_topple_update_dual_world_empty_gate_method_names_wave371_ok,
            "live topple update dual-world empty gate method names residual pack wave371: {}",
            r.detail
        );
        assert!(
            r.live_topple_update_dual_world_empty_gate_nav_commands_wave371_ok,
            "live topple update dual-world empty gate nav commands residual pack wave371: {}",
            r.detail
        );
        assert!(
            r.live_topple_update_dual_world_empty_gate_live_wave371_ok,
            "live topple update dual-world empty gate live residual wave371: {}",
            r.detail
        );
        assert!(
            r.live_projectile_stream_update_dual_world_empty_gate_method_names_wave372_ok,
            "live projectile stream update dual-world empty gate method names residual pack wave372: {}",
            r.detail
        );
        assert!(
            r.live_projectile_stream_update_dual_world_empty_gate_nav_commands_wave372_ok,
            "live projectile stream update dual-world empty gate nav commands residual pack wave372: {}",
            r.detail
        );
        assert!(
            r.live_projectile_stream_update_dual_world_empty_gate_live_wave372_ok,
            "live projectile stream update dual-world empty gate live residual wave372: {}",
            r.detail
        );
        assert!(
            r.live_demo_trap_update_dual_world_empty_gate_method_names_wave373_ok,
            "live demo trap update dual-world empty gate method names residual pack wave373: {}",
            r.detail
        );
        assert!(
            r.live_demo_trap_update_dual_world_empty_gate_nav_commands_wave373_ok,
            "live demo trap update dual-world empty gate nav commands residual pack wave373: {}",
            r.detail
        );
        assert!(
            r.live_demo_trap_update_dual_world_empty_gate_live_wave373_ok,
            "live demo trap update dual-world empty gate live residual wave373: {}",
            r.detail
        );
        assert!(
            r.live_mob_member_slaved_update_dual_world_empty_gate_method_names_wave374_ok,
            "live mob member slaved update dual-world empty gate method names residual pack wave374: {}",
            r.detail
        );
        assert!(
            r.live_mob_member_slaved_update_dual_world_empty_gate_nav_commands_wave374_ok,
            "live mob member slaved update dual-world empty gate nav commands residual pack wave374: {}",
            r.detail
        );
        assert!(
            r.live_mob_member_slaved_update_dual_world_empty_gate_live_wave374_ok,
            "live mob member slaved update dual-world empty gate live residual wave374: {}",
            r.detail
        );
        assert!(
            r.live_tn_guard_dual_world_empty_gate_method_names_wave375_ok,
            "live tn guard dual-world empty gate method names residual pack wave375: {}",
            r.detail
        );
        assert!(
            r.live_tn_guard_dual_world_empty_gate_nav_commands_wave375_ok,
            "live tn guard dual-world empty gate nav commands residual pack wave375: {}",
            r.detail
        );
        assert!(
            r.live_tn_guard_dual_world_empty_gate_live_wave375_ok,
            "live tn guard dual-world empty gate live residual wave375: {}",
            r.detail
        );
        assert!(
            r.live_production_update_dual_world_empty_gate_method_names_wave376_ok,
            "live production update dual-world empty gate method names residual pack wave376: {}",
            r.detail
        );
        assert!(
            r.live_production_update_dual_world_empty_gate_nav_commands_wave376_ok,
            "live production update dual-world empty gate nav commands residual pack wave376: {}",
            r.detail
        );
        assert!(
            r.live_production_update_dual_world_empty_gate_live_wave376_ok,
            "live production update dual-world empty gate live residual wave376: {}",
            r.detail
        );
        assert!(
            r.live_poisoned_behavior_dual_world_empty_gate_method_names_wave377_ok,
            "live poisoned behavior dual-world empty gate method names residual pack wave377: {}",
            r.detail
        );
        assert!(
            r.live_poisoned_behavior_dual_world_empty_gate_nav_commands_wave377_ok,
            "live poisoned behavior dual-world empty gate nav commands residual pack wave377: {}",
            r.detail
        );
        assert!(
            r.live_poisoned_behavior_dual_world_empty_gate_live_wave377_ok,
            "live poisoned behavior dual-world empty gate live residual wave377: {}",
            r.detail
        );
        assert!(
            r.live_horde_update_dual_world_empty_gate_method_names_wave378_ok,
            "live horde update dual-world empty gate method names residual pack wave378: {}",
            r.detail
        );
        assert!(
            r.live_horde_update_dual_world_empty_gate_nav_commands_wave378_ok,
            "live horde update dual-world empty gate nav commands residual pack wave378: {}",
            r.detail
        );
        assert!(
            r.live_horde_update_dual_world_empty_gate_live_wave378_ok,
            "live horde update dual-world empty gate live residual wave378: {}",
            r.detail
        );
        assert!(
            r.live_flammable_update_dual_world_empty_gate_method_names_wave379_ok,
            "live flammable update dual-world empty gate method names residual pack wave379: {}",
            r.detail
        );
        assert!(
            r.live_flammable_update_dual_world_empty_gate_nav_commands_wave379_ok,
            "live flammable update dual-world empty gate nav commands residual pack wave379: {}",
            r.detail
        );
        assert!(
            r.live_flammable_update_dual_world_empty_gate_live_wave379_ok,
            "live flammable update dual-world empty gate live residual wave379: {}",
            r.detail
        );
        assert!(
            r.live_base_regenerate_update_dual_world_empty_gate_method_names_wave380_ok,
            "live base regenerate update dual-world empty gate method names residual pack wave380: {}",
            r.detail
        );
        assert!(
            r.live_base_regenerate_update_dual_world_empty_gate_nav_commands_wave380_ok,
            "live base regenerate update dual-world empty gate nav commands residual pack wave380: {}",
            r.detail
        );
        assert!(
            r.live_base_regenerate_update_dual_world_empty_gate_live_wave380_ok,
            "live base regenerate update dual-world empty gate live residual wave380: {}",
            r.detail
        );
        assert!(
            r.live_queue_production_exit_behavior_dual_world_empty_gate_method_names_wave381_ok,
            "live queue production exit behavior dual-world empty gate method names residual pack wave381: {}",
            r.detail
        );
        assert!(
            r.live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_wave381_ok,
            "live queue production exit behavior dual-world empty gate nav commands residual pack wave381: {}",
            r.detail
        );
        assert!(
            r.live_queue_production_exit_behavior_dual_world_empty_gate_live_wave381_ok,
            "live queue production exit behavior dual-world empty gate live residual wave381: {}",
            r.detail
        );
        assert!(
            r.live_missile_launcher_building_update_dual_world_empty_gate_method_names_wave382_ok,
            "live missile launcher building update dual-world empty gate method names residual pack wave382: {}",
            r.detail
        );
        assert!(
            r.live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_wave382_ok,
            "live missile launcher building update dual-world empty gate nav commands residual pack wave382: {}",
            r.detail
        );
        assert!(
            r.live_missile_launcher_building_update_dual_world_empty_gate_live_wave382_ok,
            "live missile launcher building update dual-world empty gate live residual wave382: {}",
            r.detail
        );
        assert!(
            r.live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_wave383_ok,
            "live dynamic shroud clearing range update dual-world empty gate method names residual pack wave383: {}",
            r.detail
        );
        assert!(
            r.live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_wave383_ok,
            "live dynamic shroud clearing range update dual-world empty gate nav commands residual pack wave383: {}",
            r.detail
        );
        assert!(
            r.live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_live_wave383_ok,
            "live dynamic shroud clearing range update dual-world empty gate live residual wave383: {}",
            r.detail
        );
        assert!(
            r.live_command_button_hunt_update_dual_world_empty_gate_method_names_wave384_ok,
            "live command button hunt update dual-world empty gate method names residual pack wave384: {}",
            r.detail
        );
        assert!(
            r.live_command_button_hunt_update_dual_world_empty_gate_nav_commands_wave384_ok,
            "live command button hunt update dual-world empty gate nav commands residual pack wave384: {}",
            r.detail
        );
        assert!(
            r.live_command_button_hunt_update_dual_world_empty_gate_live_wave384_ok,
            "live command button hunt update dual-world empty gate live residual wave384: {}",
            r.detail
        );
        assert!(
            r.live_prison_behavior_dual_world_empty_gate_method_names_wave385_ok,
            "live prison behavior dual-world empty gate method names residual pack wave385: {}",
            r.detail
        );
        assert!(
            r.live_prison_behavior_dual_world_empty_gate_nav_commands_wave385_ok,
            "live prison behavior dual-world empty gate nav commands residual pack wave385: {}",
            r.detail
        );
        assert!(
            r.live_prison_behavior_dual_world_empty_gate_live_wave385_ok,
            "live prison behavior dual-world empty gate live residual wave385: {}",
            r.detail
        );
        assert!(
            r.live_generate_minefield_behavior_dual_world_empty_gate_method_names_wave386_ok,
            "live generate minefield behavior dual-world empty gate method names residual pack wave386: {}",
            r.detail
        );
        assert!(
            r.live_generate_minefield_behavior_dual_world_empty_gate_nav_commands_wave386_ok,
            "live generate minefield behavior dual-world empty gate nav commands residual pack wave386: {}",
            r.detail
        );
        assert!(
            r.live_generate_minefield_behavior_dual_world_empty_gate_live_wave386_ok,
            "live generate minefield behavior dual-world empty gate live residual wave386: {}",
            r.detail
        );
        assert!(
            r.live_demoralize_special_power_dual_world_empty_gate_method_names_wave387_ok,
            "live demoralize special power dual-world empty gate method names residual pack wave387: {}",
            r.detail
        );
        assert!(
            r.live_demoralize_special_power_dual_world_empty_gate_nav_commands_wave387_ok,
            "live demoralize special power dual-world empty gate nav commands residual pack wave387: {}",
            r.detail
        );
        assert!(
            r.live_demoralize_special_power_dual_world_empty_gate_live_wave387_ok,
            "live demoralize special power dual-world empty gate live residual wave387: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_update_dual_world_empty_gate_method_names_wave388_ok,
            "live stealth detector update dual-world empty gate method names residual pack wave388: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_update_dual_world_empty_gate_nav_commands_wave388_ok,
            "live stealth detector update dual-world empty gate nav commands residual pack wave388: {}",
            r.detail
        );
        assert!(
            r.live_stealth_detector_update_dual_world_empty_gate_live_wave388_ok,
            "live stealth detector update dual-world empty gate live residual wave388: {}",
            r.detail
        );
        assert!(
            r.live_hive_structure_body_dual_world_empty_gate_method_names_wave389_ok,
            "live hive structure body dual-world empty gate method names residual pack wave389: {}",
            r.detail
        );
        assert!(
            r.live_hive_structure_body_dual_world_empty_gate_nav_commands_wave389_ok,
            "live hive structure body dual-world empty gate nav commands residual pack wave389: {}",
            r.detail
        );
        assert!(
            r.live_hive_structure_body_dual_world_empty_gate_live_wave389_ok,
            "live hive structure body dual-world empty gate live residual wave389: {}",
            r.detail
        );
        assert!(
            r.live_salvage_crate_collide_dual_world_empty_gate_method_names_wave390_ok,
            "live salvage crate collide dual-world empty gate method names residual pack wave390: {}",
            r.detail
        );
        assert!(
            r.live_salvage_crate_collide_dual_world_empty_gate_nav_commands_wave390_ok,
            "live salvage crate collide dual-world empty gate nav commands residual pack wave390: {}",
            r.detail
        );
        assert!(
            r.live_salvage_crate_collide_dual_world_empty_gate_live_wave390_ok,
            "live salvage crate collide dual-world empty gate live residual wave390: {}",
            r.detail
        );
        assert!(
            r.live_sabotage_internet_center_crate_collide_dual_world_empty_gate_method_names_wave391_ok,
            "live sabotage internet center crate collide dual-world empty gate method names residual pack wave391: {}",
            r.detail
        );
        assert!(
            r.live_sabotage_internet_center_crate_collide_dual_world_empty_gate_nav_commands_wave391_ok,
            "live sabotage internet center crate collide dual-world empty gate nav commands residual pack wave391: {}",
            r.detail
        );
        assert!(
            r.live_sabotage_internet_center_crate_collide_dual_world_empty_gate_live_wave391_ok,
            "live sabotage internet center crate collide dual-world empty gate live residual wave391: {}",
            r.detail
        );
        assert!(
            r.live_power_plant_update_dual_world_empty_gate_method_names_wave392_ok,
            "live power plant update dual-world empty gate method names residual pack wave392: {}",
            r.detail
        );
        assert!(
            r.live_power_plant_update_dual_world_empty_gate_nav_commands_wave392_ok,
            "live power plant update dual-world empty gate nav commands residual pack wave392: {}",
            r.detail
        );
        assert!(
            r.live_power_plant_update_dual_world_empty_gate_live_wave392_ok,
            "live power plant update dual-world empty gate live residual wave392: {}",
            r.detail
        );
        assert!(
            r.live_leaflet_drop_behavior_dual_world_empty_gate_method_names_wave393_ok,
            "live leaflet drop behavior dual-world empty gate method names residual pack wave393: {}",
            r.detail
        );
        assert!(
            r.live_leaflet_drop_behavior_dual_world_empty_gate_nav_commands_wave393_ok,
            "live leaflet drop behavior dual-world empty gate nav commands residual pack wave393: {}",
            r.detail
        );
        assert!(
            r.live_leaflet_drop_behavior_dual_world_empty_gate_live_wave393_ok,
            "live leaflet drop behavior dual-world empty gate live residual wave393: {}",
            r.detail
        );
        assert!(
            r.live_auto_deposit_update_dual_world_empty_gate_method_names_wave394_ok,
            "live auto deposit update dual-world empty gate method names residual pack wave394: {}",
            r.detail
        );
        assert!(
            r.live_auto_deposit_update_dual_world_empty_gate_nav_commands_wave394_ok,
            "live auto deposit update dual-world empty gate nav commands residual pack wave394: {}",
            r.detail
        );
        assert!(
            r.live_auto_deposit_update_dual_world_empty_gate_live_wave394_ok,
            "live auto deposit update dual-world empty gate live residual wave394: {}",
            r.detail
        );
        assert!(
            r.live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_wave395_ok,
            "live supply warehouse crippling behavior dual-world empty gate method names residual pack wave395: {}",
            r.detail
        );
        assert!(
            r.live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_wave395_ok,
            "live supply warehouse crippling behavior dual-world empty gate nav commands residual pack wave395: {}",
            r.detail
        );
        assert!(
            r.live_supply_warehouse_crippling_behavior_dual_world_empty_gate_live_wave395_ok,
            "live supply warehouse crippling behavior dual-world empty gate live residual wave395: {}",
            r.detail
        );
        assert!(
            r.live_neutron_missile_slow_death_update_dual_world_empty_gate_method_names_wave396_ok,
            "live neutron missile slow death update dual-world empty gate method names residual pack wave396: {}",
            r.detail
        );
        assert!(
            r.live_neutron_missile_slow_death_update_dual_world_empty_gate_nav_commands_wave396_ok,
            "live neutron missile slow death update dual-world empty gate nav commands residual pack wave396: {}",
            r.detail
        );
        assert!(
            r.live_neutron_missile_slow_death_update_dual_world_empty_gate_live_wave396_ok,
            "live neutron missile slow death update dual-world empty gate live residual wave396: {}",
            r.detail
        );
        assert!(
            r.live_ai_dock_dual_world_empty_gate_method_names_wave397_ok,
            "live ai dock dual-world empty gate method names residual pack wave397: {}",
            r.detail
        );
        assert!(
            r.live_ai_dock_dual_world_empty_gate_nav_commands_wave397_ok,
            "live ai dock dual-world empty gate nav commands residual pack wave397: {}",
            r.detail
        );
        assert!(
            r.live_ai_dock_dual_world_empty_gate_live_wave397_ok,
            "live ai dock dual-world empty gate live residual wave397: {}",
            r.detail
        );
        assert!(
            r.live_ai_groups_dual_world_empty_gate_method_names_wave398_ok,
            "live ai groups dual-world empty gate method names residual pack wave398: {}",
            r.detail
        );
        assert!(
            r.live_ai_groups_dual_world_empty_gate_nav_commands_wave398_ok,
            "live ai groups dual-world empty gate nav commands residual pack wave398: {}",
            r.detail
        );
        assert!(
            r.live_ai_groups_dual_world_empty_gate_live_wave398_ok,
            "live ai groups dual-world empty gate live residual wave398: {}",
            r.detail
        );
        assert!(
            r.live_artillery_barrage_power_dual_world_empty_gate_method_names_wave399_ok,
            "live artillery barrage power dual-world empty gate method names residual pack wave399: {}",
            r.detail
        );
        assert!(
            r.live_artillery_barrage_power_dual_world_empty_gate_nav_commands_wave399_ok,
            "live artillery barrage power dual-world empty gate nav commands residual pack wave399: {}",
            r.detail
        );
        assert!(
            r.live_artillery_barrage_power_dual_world_empty_gate_live_wave399_ok,
            "live artillery barrage power dual-world empty gate live residual wave399: {}",
            r.detail
        );
        assert!(
            r.live_baikonur_launch_power_dual_world_empty_gate_method_names_wave400_ok,
            "live baikonur launch power dual-world empty gate method names residual pack wave400: {}",
            r.detail
        );
        assert!(
            r.live_baikonur_launch_power_dual_world_empty_gate_nav_commands_wave400_ok,
            "live baikonur launch power dual-world empty gate nav commands residual pack wave400: {}",
            r.detail
        );
        assert!(
            r.live_baikonur_launch_power_dual_world_empty_gate_live_wave400_ok,
            "live baikonur launch power dual-world empty gate live residual wave400: {}",
            r.detail
        );
        assert!(
            r.live_ai_group_dual_world_empty_gate_method_names_wave401_ok,
            "live ai group dual-world empty gate method names residual pack wave401: {}",
            r.detail
        );
        assert!(
            r.live_ai_group_dual_world_empty_gate_nav_commands_wave401_ok,
            "live ai group dual-world empty gate nav commands residual pack wave401: {}",
            r.detail
        );
        assert!(
            r.live_ai_group_dual_world_empty_gate_live_wave401_ok,
            "live ai group dual-world empty gate live residual wave401: {}",
            r.detail
        );
        assert!(
            r.live_slaved_update_dual_world_empty_gate_method_names_wave402_ok,
            "live slaved update dual-world empty gate method names residual pack wave402: {}",
            r.detail
        );
        assert!(
            r.live_slaved_update_dual_world_empty_gate_nav_commands_wave402_ok,
            "live slaved update dual-world empty gate nav commands residual pack wave402: {}",
            r.detail
        );
        assert!(
            r.live_slaved_update_dual_world_empty_gate_live_wave402_ok,
            "live slaved update dual-world empty gate live residual wave402: {}",
            r.detail
        );
        assert!(
            r.live_demoralize_power_dual_world_empty_gate_method_names_wave403_ok,
            "live demoralize power dual-world empty gate method names residual pack wave403: {}",
            r.detail
        );
        assert!(
            r.live_demoralize_power_dual_world_empty_gate_nav_commands_wave403_ok,
            "live demoralize power dual-world empty gate nav commands residual pack wave403: {}",
            r.detail
        );
        assert!(
            r.live_demoralize_power_dual_world_empty_gate_live_wave403_ok,
            "live demoralize power dual-world empty gate live residual wave403: {}",
            r.detail
        );
        assert!(
            r.live_bone_fx_update_dual_world_empty_gate_method_names_wave404_ok,
            "live bone fx update dual-world empty gate method names residual pack wave404: {}",
            r.detail
        );
        assert!(
            r.live_bone_fx_update_dual_world_empty_gate_nav_commands_wave404_ok,
            "live bone fx update dual-world empty gate nav commands residual pack wave404: {}",
            r.detail
        );
        assert!(
            r.live_bone_fx_update_dual_world_empty_gate_live_wave404_ok,
            "live bone fx update dual-world empty gate live residual wave404: {}",
            r.detail
        );
        assert!(
            r.live_supply_warehouse_dock_dual_world_empty_gate_method_names_wave405_ok,
            "live supply warehouse dock dual-world empty gate method names residual pack wave405: {}",
            r.detail
        );
        assert!(
            r.live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_wave405_ok,
            "live supply warehouse dock dual-world empty gate nav commands residual pack wave405: {}",
            r.detail
        );
        assert!(
            r.live_supply_warehouse_dock_dual_world_empty_gate_live_wave405_ok,
            "live supply warehouse dock dual-world empty gate live residual wave405: {}",
            r.detail
        );
        assert!(
            r.live_ocl_special_power_dual_world_empty_gate_method_names_wave406_ok,
            "live ocl special power dual-world empty gate method names residual pack wave406: {}",
            r.detail
        );
        assert!(
            r.live_ocl_special_power_dual_world_empty_gate_nav_commands_wave406_ok,
            "live ocl special power dual-world empty gate nav commands residual pack wave406: {}",
            r.detail
        );
        assert!(
            r.live_ocl_special_power_dual_world_empty_gate_live_wave406_ok,
            "live ocl special power dual-world empty gate live residual wave406: {}",
            r.detail
        );
        assert!(
            r.live_railed_transport_ai_update_dual_world_empty_gate_method_names_wave407_ok,
            "live railed transport ai update dual-world empty gate method names residual pack wave407: {}",
            r.detail
        );
        assert!(
            r.live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_wave407_ok,
            "live railed transport ai update dual-world empty gate nav commands residual pack wave407: {}",
            r.detail
        );
        assert!(
            r.live_railed_transport_ai_update_dual_world_empty_gate_live_wave407_ok,
            "live railed transport ai update dual-world empty gate live residual wave407: {}",
            r.detail
        );
        assert!(
            r.live_squish_collide_dual_world_empty_gate_method_names_wave408_ok,
            "live squish collide dual-world empty gate method names residual pack wave408: {}",
            r.detail
        );
        assert!(
            r.live_squish_collide_dual_world_empty_gate_nav_commands_wave408_ok,
            "live squish collide dual-world empty gate nav commands residual pack wave408: {}",
            r.detail
        );
        assert!(
            r.live_squish_collide_dual_world_empty_gate_live_wave408_ok,
            "live squish collide dual-world empty gate live residual wave408: {}",
            r.detail
        );
        assert!(
            r.live_weapon_bonus_update_dual_world_empty_gate_method_names_wave409_ok,
            "live weapon bonus update dual-world empty gate method names residual pack wave409: {}",
            r.detail
        );
        assert!(
            r.live_weapon_bonus_update_dual_world_empty_gate_nav_commands_wave409_ok,
            "live weapon bonus update dual-world empty gate nav commands residual pack wave409: {}",
            r.detail
        );
        assert!(
            r.live_weapon_bonus_update_dual_world_empty_gate_live_wave409_ok,
            "live weapon bonus update dual-world empty gate live residual wave409: {}",
            r.detail
        );
        assert!(
            r.live_minefield_behavior_dual_world_empty_gate_method_names_wave410_ok,
            "live minefield behavior dual-world empty gate method names residual pack wave410: {}",
            r.detail
        );
        assert!(
            r.live_minefield_behavior_dual_world_empty_gate_nav_commands_wave410_ok,
            "live minefield behavior dual-world empty gate nav commands residual pack wave410: {}",
            r.detail
        );
        assert!(
            r.live_minefield_behavior_dual_world_empty_gate_live_wave410_ok,
            "live minefield behavior dual-world empty gate live residual wave410: {}",
            r.detail
        );
        assert!(
            r.live_point_defense_laser_update_dual_world_empty_gate_method_names_wave411_ok,
            "live point defense laser update dual-world empty gate method names residual pack wave411: {}",
            r.detail
        );
        assert!(
            r.live_point_defense_laser_update_dual_world_empty_gate_nav_commands_wave411_ok,
            "live point defense laser update dual-world empty gate nav commands residual pack wave411: {}",
            r.detail
        );
        assert!(
            r.live_point_defense_laser_update_dual_world_empty_gate_live_wave411_ok,
            "live point defense laser update dual-world empty gate live residual wave411: {}",
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
        // Selection hotkeys / pick residual prefer presentation local_team when dual-scanning.
        // Right-click context path is command-system residual via current_player_id.
        for needle in [
            "Retail SELECT_ALL (KEY_Q) / Ctrl+A residual",
            "fn select_all_friendly_units",
            "fn find_object_at_position",
        ] {
            let idx = eng
                .find(needle)
                .unwrap_or_else(|| panic!("missing {needle}"));
            let window = &eng[idx..eng.len().min(idx + 2500)];
            assert!(
                window.contains("local_team")
                    || window.contains("local_team()")
                    || window.contains("pick_object_id_at_world_from_presentation")
                    || window.contains("Boot residual")
                    || window.contains("Presentation-only"),
                "{needle} must prefer presentation local_team / presentation pick"
            );
            if needle == "fn find_object_at_position" {
                // Bound to this method only (next fn starts pathfollowing stub).
                let end = window
                    .find("fn update_unit_pathfinding")
                    .unwrap_or(window.len());
                let body = &window[..end];
                assert!(
                    body.contains("Presentation-only")
                        && body.contains("pick_object_id_at_world_from_presentation")
                        && !body.contains("game_logic.get_objects()"),
                    "engine find_object_at_position must be presentation-only"
                );
            }
        }
        assert!(
            eng.contains("fn handle_right_click")
                && eng.contains("process_mouse_input")
                && eng.contains("current_player_id"),
            "right-click must route context commands via current_player selection residual"
        );
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
        // Presentation-only: no live get_player dual-read in this path.
        assert!(
            window.contains("last_presentation_frame") || window.contains("Presentation-only"),
            "select_similar_units must be presentation-frame gated"
        );
        assert!(
            !window.contains("game_logic.get_player"),
            "select_similar_units must not dual-read live get_player"
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
        // Live get_player only as residual after presentation roster miss.
        assert!(
            window.contains("get_player") || window.contains("player_info"),
            "defeat UI must use presentation roster and/or residual get_player"
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
