use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() {
    // Gate default: GameWorld damage last-writer for HP (opt out DAMAGE_AUTHORITY=0).
    generals_main::gameworld_shadow::ensure_gate_damage_authority();
    let r = run_shell_smoke(10);
    println!("{}", format_shell_smoke_report(&r));
    // Fail-closed retail: headless smoke must never claim full W3D playability.
    // Limited host claim (shell_host_playable_ok) is required for success and is
    // independent of playable_claim — see shell_smoke module docs.
    let pass = r.status == "success"
        && !r.playable_claim
        && r.shell_host_playable_ok
        && r.control_bar_layout_ok
        && r.hud_selection_ok
        && r.selection_consumers_ok
        && r.screen_skirmish_ok
        && r.dual_tick_presentation_ok
        && r.minimap_fow_presentation_ok
        && r.laser_segment_upload_ok
        && r.projectile_segment_upload_ok
        && r.move_line_upload_ok
        && r.attack_line_upload_ok
        // Wave 75 residual honesty (host-testable; never flips playable_claim).
        && r.mesh_asset_residual_ok
        && r.rng_residual_pack_ok
        && r.special_power_wave72_residual_ok
        && r.special_power_wave73_residual_ok
        // Wave 76 residual honesty (host-testable; never flips playable_claim).
        && r.special_power_wave76_residual_ok
        && r.paradrop_wave76_residual_ok
        && r.control_bar_wave76_residual_ok
        && r.graphics_wave76_residual_ok
        && r.spectre_orbit_decal_presentation_ok
        // Wave 77 residual honesty (orthogonal to ControlBar/script; never flips playable_claim).
        && r.special_power_wave77_residual_ok
        && r.fow_residual_pack_ok
        && r.ground_height_presentation_ok
        && r.weapon_store_seed_residual_ok
        && r.ai_skirmish_residual_ok
        // Wave 78 residual honesty (reload table + science tiers; never flips playable_claim).
        && r.special_power_wave78_residual_ok
        && r.cluster_mines_wave78_residual_ok
        && r.gps_scrambler_wave78_residual_ok
        && r.cash_bounty_wave78_residual_ok
        // Wave 79 residual honesty (orthogonal to special powers; never flips playable_claim).
        && r.minimap_residual_pack_ok
        && r.selection_hud_residual_pack_ok
        && r.input_residual_pack_ok
        && r.drawable_residual_fields_ok
        && r.unit_training_wave79_residual_ok
        && r.upgrades_cost_time_application_ok
        // Wave 80 residual honesty (INI-backed superweapon/science residual; never flips playable_claim).
        && r.command_button_wave80_residual_ok
        && r.science_rank_wave80_residual_ok
        && r.superweapon_kindof_wave80_residual_ok
        && r.special_power_enum_wave80_residual_ok
        // Wave 81 residual honesty (terrain/pathfinder/locomotor/armor/PUC; never flips playable_claim).
        && r.terrain_height_sample_wave81_ok
        && r.pathfinder_wave81_residual_ok
        && r.locomotor_table_wave81_ok
        && r.armor_table_wave81_ok
        && r.puc_flare_table_wave81_ok
        // Wave 82 residual honesty (enum/bit-name tables; never flips playable_claim).
        && r.damage_type_wave82_ok
        && r.death_type_wave82_ok
        && r.model_condition_wave82_ok
        && r.weapon_bonus_wave82_ok
        && r.object_status_wave82_ok
        // Wave 83 residual honesty (structure/economy residual; never flips playable_claim).
        && r.production_queue_wave83_ok
        && r.supply_warehouse_wave83_ok
        && r.dozer_build_wave83_ok
        && r.capture_building_wave83_ok
        && r.power_plant_wave83_ok
        && r.command_center_wave83_ok
        // Wave 84 residual honesty (KindOf/WeaponSlot/Veterancy/Relationship/Geometry/Shadow tables; never flips playable_claim).
        && r.kindof_wave84_ok
        && r.weapon_slot_wave84_ok
        && r.veterancy_wave84_ok
        && r.relationship_wave84_ok
        && r.geometry_wave84_ok
        && r.shadow_wave84_ok
        // Wave 85 residual honesty (faction/skirmish residual; never flips playable_claim).
        && r.faction_side_wave85_ok
        && r.player_template_wave85_ok
        && r.starting_cash_wave85_ok
        && r.skirmish_ai_personality_wave85_ok
        && r.victory_condition_wave85_ok
        // Wave 86 residual honesty (GameData/lobby/map/crate residual; never flips playable_claim).
        && r.gamedata_camera_fps_wave86_ok
        && r.gamedata_world_constants_wave86_ok
        && r.multiplayer_options_wave86_ok
        && r.map_selection_wave86_ok
        && r.crate_deepen_wave86_ok
        // Wave 87 residual honesty (weather/water/bridge/tunnel/garrison/transport; never flips playable_claim).
        && r.weather_wave87_ok
        && r.water_wave87_ok
        && r.bridge_wave87_ok
        && r.tunnel_wave87_ok
        && r.garrison_wave87_ok
        && r.transport_wave87_ok
        // Wave 88 residual honesty (FX/OCL/particle/audio/cursor name tables; never flips playable_claim).
        && r.radius_cursor_wave88_ok
        && r.mouse_cursor_wave88_ok
        && r.superweapon_fxlist_wave88_ok
        && r.superweapon_ocl_wave88_ok
        && r.superweapon_particle_wave88_ok
        && r.superweapon_audio_wave88_ok
        // Wave 89 residual honesty (rank/exp/hotkey/chat/replay/options; never flips playable_claim).
        && r.rank_skill_wave89_ok
        && r.experience_wave89_ok
        && r.hotkey_wave89_ok
        && r.chat_wave89_ok
        && r.replay_wave89_ok
        && r.options_wave89_ok
        // Wave 90 residual honesty (gamespeed/framerate/debug/language/credits; never flips playable_claim).
        && r.gamespeed_wave90_ok
        && r.frame_rate_wave90_ok
        && r.debug_tables_wave90_ok
        && r.language_wave90_ok
        && r.credits_wave90_ok
        // Wave 91 residual honesty (tooltip/helpbox/message/eva/video/briefing; never flips playable_claim).
        && r.tooltip_wave91_ok
        && r.help_box_wave91_ok
        && r.message_wave91_ok
        && r.eva_wave91_ok
        && r.video_wave91_ok
        && r.mission_briefing_wave91_ok
        // Wave 92 residual honesty (weapon/armor/body/locomotor/science; never flips playable_claim).
        && r.weapon_deepen_wave92_ok
        && r.armor_expand_wave92_ok
        && r.body_health_wave92_ok
        && r.locomotor_expand_wave92_ok
        && r.science_names_wave92_ok
        // Wave 93 residual honesty (particle/drawable/shadow/terrain/road; never flips playable_claim).
        && r.particle_emit_wave93_ok
        && r.drawable_opacity_wave93_ok
        && r.shadow_deepen_wave93_ok
        && r.terrain_texture_wave93_ok
        && r.road_wave93_ok
        // Wave 94 residual honesty (AI/special ability/upgrade/CommandSet; never flips playable_claim).
        && r.ai_state_wave94_ok
        && r.special_ability_wave94_ok
        && r.upgrade_names_wave94_ok
        && r.command_set_wave94_ok
        // Wave 95 residual honesty (script/map/waypoint/team/player; never flips playable_claim).
        && r.script_action_wave95_ok
        && r.script_condition_wave95_ok
        && r.map_object_wave95_ok
        && r.waypoint_wave95_ok
        && r.team_wave95_ok
        && r.player_deepen_wave95_ok
        // Wave 96 residual honesty (partition/collision/physics/projectile; never flips playable_claim).
        && r.partition_wave96_ok
        && r.collision_wave96_ok
        && r.physics_wave96_ok
        && r.projectile_wave96_ok
        // Wave 97 residual honesty (radar/spotter/stealth/detector/vision; never flips playable_claim).
        && r.radar_deepen_wave97_ok
        && r.spotter_wave97_ok
        && r.stealth_deepen_wave97_ok
        && r.detector_deepen_wave97_ok
        && r.vision_wave97_ok
        // Wave 98 residual honesty (dock/contain/exit/heal; never flips playable_claim).
        && r.dock_wave98_ok
        && r.contain_wave98_ok
        && r.exit_wave98_ok
        && r.heal_wave98_ok
        // Wave 99 residual honesty (production/buildable/prereq/command-button/control-bar; never flips playable_claim).
        && r.production_deepen_wave99_ok
        && r.buildable_wave99_ok
        && r.prerequisite_wave99_ok
        && r.command_button_deepen_wave99_ok
        && r.control_bar_deepen_wave99_ok
        // Wave 100 residual honesty (ThingFactory/module/xfer; never flips playable_claim).
        && r.thing_factory_deepen_wave100_ok
        && r.module_type_wave100_ok
        && r.xfer_deepen_wave100_ok
        && r.thing_factory_crosslink_wave100_ok
        // Wave 101 residual honesty (ModuleFactory/ThingFactory create/Partition register; never flips playable_claim).
        && r.module_factory_deepen_wave101_ok
        && r.thing_factory_create_wave101_ok
        && r.partition_register_wave101_ok
        && r.mf_crosslink_wave101_ok
        // Wave 102 residual honesty (DisplayString/Anim2D/laser/CSF/presentation; never flips playable_claim).
        && r.display_string_deepen_wave102_ok
        && r.anim2d_deepen_wave102_ok
        && r.laser_segliner_deepen_wave102_ok
        && r.csf_multi_locale_deepen_wave102_ok
        && r.presentation_deepen_wave102_ok
        // Wave 103 residual honesty (weapon/armor/loco/special-power/KindOf; never flips playable_claim).
        && r.weapon_deepen_wave103_ok
        && r.armor_expand_wave103_ok
        && r.locomotor_expand_wave103_ok
        && r.special_power_deepen_wave103_ok
        && r.object_kindof_wave103_ok
        // Wave 104 residual honesty (Object status/create, ActiveBody, Drawable create, registerObject; never flips playable_claim).
        && r.object_status_wave104_ok
        && r.object_create_wave104_ok
        && r.active_body_wave104_ok
        && r.drawable_create_wave104_ok
        && r.register_object_wave104_ok
        // Wave 105 residual honesty (AI group/path/weapon fire/damage/veterancy; never flips playable_claim).
        && r.ai_group_wave105_ok
        && r.ai_path_wave105_ok
        && r.weapon_fire_wave105_ok
        && r.damage_application_wave105_ok
        && r.veterancy_wave105_ok
        // Wave 106 residual honesty (GameState/campaign/MainMenu/GameWindow/layout; never flips playable_claim).
        && r.game_state_deepen_wave106_ok
        && r.campaign_mission_wave106_ok
        && r.main_menu_deepen_wave106_ok
        && r.game_window_deepen_wave106_ok
        && r.window_layout_deepen_wave106_ok
        // Wave 107 residual honesty (particle/FXList entry/OCL create/audio; never flips playable_claim).
        && r.particle_system_deepen_wave107_ok
        && r.fxlist_entry_deepen_wave107_ok
        && r.ocl_create_deepen_wave107_ok
        && r.audio_deepen_wave107_ok
        // Wave 108 residual honesty (HeightMap/bridge/water/road/cliff; never flips playable_claim).
        && r.heightmap_deepen_wave108_ok
        && r.bridge_deepen_wave108_ok
        && r.water_deepen_wave108_ok
        && r.road_deepen_wave108_ok
        && r.cliff_peels_wave108_ok
        // Wave 109 residual honesty (SP/Science/Upgrade store + Player/Team; never flips playable_claim).
        && r.special_power_store_wave109_ok
        && r.science_store_wave109_ok
        && r.upgrade_store_wave109_ok
        && r.player_deepen_wave109_ok
        && r.team_deepen_wave109_ok
        // Wave 110 residual honesty (MessageStream/MetaEvent/InGameUI; never flips playable_claim).
        && r.message_stream_marker_wave110_ok
        && r.game_message_arg_wave110_ok
        && r.meta_event_category_wave110_ok
        && r.ingame_ui_wave110_ok
        // Wave 111 residual honesty (Drawable/Display/Client/Particle; never flips playable_claim).
        && r.drawable_icon_flash_wave111_ok
        && r.drawable_status_stealth_wave111_ok
        && r.terrain_decal_wave111_ok
        && r.display_draw_image_wave111_ok
        && r.game_client_translator_wave111_ok
        && r.particle_priority_wave111_ok
        // Wave 112 residual honesty (Mouse/Keyboard/View; never flips playable_claim).
        && r.mouse_residual_wave112_ok
        && r.keyboard_residual_wave112_ok
        && r.view_residual_wave112_ok
        // Wave 113 residual honesty (Gadget/Video/Audio; never flips playable_claim).
        && r.game_window_manager_wave113_ok
        && r.window_style_wave113_ok
        && r.gadget_wave113_ok
        && r.video_buffer_wave113_ok
        && r.audio_event_wave113_ok
        // Wave 114 residual honesty (MainMenu→Skirmish WND nav; never flips playable_claim).
        && r.main_menu_skirmish_names_wave114_ok
        && r.main_menu_skirmish_nav_steps_wave114_ok
        && r.main_menu_skirmish_message_wave114_ok
        // Wave 115 residual honesty (Skirmish map-select WND; never flips playable_claim).
        && r.map_select_names_wave115_ok
        && r.map_select_nav_steps_wave115_ok
        && r.map_select_commands_wave115_ok
        // Wave 116 residual honesty (Skirmish slot/AI config; never flips playable_claim).
        && r.slot_state_wave116_ok
        && r.slot_combo_names_wave116_ok
        && r.slot_nav_commands_wave116_ok
        // Wave 117 residual honesty (Skirmish rules cash/SW/speed; never flips playable_claim).
        && r.starting_cash_wave117_ok
        && r.game_speed_controls_wave117_ok
        && r.rules_nav_commands_wave117_ok
        // Wave 118 residual honesty (MainMenu buttons WND; never flips playable_claim).
        && r.main_menu_button_names_wave118_ok
        && r.main_menu_push_targets_wave118_ok
        && r.main_menu_button_nav_commands_wave118_ok
        // Wave 119 residual honesty (MainMenu campaign/difficulty; never flips playable_claim).
        && r.campaign_button_names_wave119_ok
        && r.campaign_enums_wave119_ok
        && r.campaign_nav_commands_wave119_ok
        // Wave 120 residual honesty (ChallengeMenu; never flips playable_claim).
        && r.challenge_control_names_wave120_ok
        && r.challenge_nav_commands_wave120_ok
        // Wave 121 residual honesty (SaveLoad menu; never flips playable_claim).
        && r.save_load_layout_wave121_ok
        && r.save_load_control_stems_wave121_ok
        && r.save_load_nav_commands_wave121_ok
        // Wave 122 residual honesty (ReplayMenu; never flips playable_claim).
        && r.replay_control_names_wave122_ok
        && r.replay_nav_commands_wave122_ok
        // Wave 123 residual honesty (QuitMenu; never flips playable_claim).
        && r.quit_control_names_wave123_ok
        && r.quit_nav_commands_wave123_ok
        // Wave 124 residual honesty (KeyboardOptions; never flips playable_claim).
        && r.keyboard_control_names_wave124_ok
        && r.keyboard_nav_commands_wave124_ok
        // Wave 125 residual honesty (ScoreScreen; never flips playable_claim).
        && r.score_control_names_wave125_ok
        && r.score_nav_commands_wave125_ok
        // Wave 126 residual honesty (OptionsMenu; never flips playable_claim).
        && r.options_control_names_wave126_ok
        && r.options_nav_commands_wave126_ok
        // Wave 127 residual honesty (CreditsMenu; never flips playable_claim).
        && r.credits_control_names_wave127_ok
        && r.credits_nav_commands_wave127_ok
        // Wave 128 residual honesty (MessageBox; never flips playable_claim).
        && r.message_box_control_names_wave128_ok
        && r.message_box_nav_commands_wave128_ok
        // Wave 129 residual honesty (Diplomacy; never flips playable_claim).
        && r.diplomacy_control_names_wave129_ok
        && r.diplomacy_nav_commands_wave129_ok
        // Wave 130 residual honesty (PopupReplay; never flips playable_claim).
        && r.popup_replay_control_names_wave130_ok
        && r.popup_replay_nav_commands_wave130_ok
        // Wave 131 residual honesty (SinglePlayerMenu; never flips playable_claim).
        && r.single_player_control_names_wave131_ok
        && r.single_player_nav_commands_wave131_ok
        // Wave 132 residual honesty (MapSelectMenu; never flips playable_claim).
        && r.map_select_control_names_wave132_ok
        && r.map_select_nav_commands_wave132_ok
        // Wave 133 residual honesty (ControlBar; never flips playable_claim).
        && r.control_bar_control_names_wave133_ok
        && r.control_bar_nav_commands_wave133_ok
        // Wave 134 residual honesty (DifficultySelect; never flips playable_claim).
        && r.difficulty_select_control_names_wave134_ok
        && r.difficulty_select_nav_commands_wave134_ok
        // Wave 135 residual honesty (LoadingScreen; never flips playable_claim).
        && r.loading_screen_stages_wave135_ok
        && r.loading_screen_nav_commands_wave135_ok
        // Wave 136/137 residual honesty (InGameChat + IdleWorker; never flips playable_claim).
        && r.in_game_chat_control_names_wave136_ok
        && r.in_game_chat_nav_commands_wave136_ok
        && r.idle_worker_control_names_wave137_ok
        && r.idle_worker_nav_commands_wave137_ok
        // Wave 138/139 residual honesty (GeneralsExp + PopupCommunicator; never flips playable_claim).
        && r.generals_exp_control_names_wave138_ok
        && r.generals_exp_nav_commands_wave138_ok
        && r.popup_communicator_control_names_wave139_ok
        && r.popup_communicator_nav_commands_wave139_ok
        // Wave 140 residual honesty (ReplayControl; never flips playable_claim).
        && r.replay_control_control_names_wave140_ok
        && r.replay_control_nav_commands_wave140_ok
        // Wave 141 residual honesty (ShellMap; never flips playable_claim).
        && r.shell_map_names_wave141_ok
        && r.shell_map_nav_commands_wave141_ok
        // Wave 142 residual honesty (Beacon; never flips playable_claim).
        && r.beacon_control_names_wave142_ok
        && r.beacon_nav_commands_wave142_ok
        // Wave 143 residual honesty (EVA; never flips playable_claim).
        && r.eva_message_names_wave143_ok
        && r.eva_nav_commands_wave143_ok
        // Wave 144 residual honesty (IME; never flips playable_claim).
        && r.ime_message_names_wave144_ok
        && r.ime_nav_commands_wave144_ok
        // Wave 145 residual honesty (Smudge; never flips playable_claim).
        && r.smudge_method_names_wave145_ok
        && r.smudge_nav_commands_wave145_ok
        // Wave 146 residual honesty (OCL timer; never flips playable_claim).
        && r.ocl_timer_method_names_wave146_ok
        && r.ocl_timer_nav_commands_wave146_ok
        // Wave 147 residual honesty (ControlBarResizer; never flips playable_claim).
        && r.control_bar_resizer_method_names_wave147_ok
        && r.control_bar_resizer_nav_commands_wave147_ok
        // Wave 148 residual honesty (UnderConstruction; never flips playable_claim).
        && r.under_construction_method_names_wave148_ok
        && r.under_construction_nav_commands_wave148_ok
        // Wave 149 residual honesty (StructureInventory; never flips playable_claim).
        && r.structure_inventory_command_names_wave149_ok
        && r.structure_inventory_nav_commands_wave149_ok
        // Wave 150 residual honesty (MultiSelect; never flips playable_claim).
        && r.multi_select_method_names_wave150_ok
        && r.multi_select_nav_commands_wave150_ok
        // Wave 151 residual honesty (Credits roll; never flips playable_claim).
        && r.credits_style_method_names_wave151_ok
        && r.credits_nav_commands_wave151_ok
        // Wave 152 residual honesty (ChallengeGenerals; never flips playable_claim).
        && r.challenge_generals_method_names_wave152_ok
        && r.challenge_generals_nav_commands_wave152_ok
        // Wave 153 residual honesty (GameWorld authority env; never flips playable_claim).
        && r.gameworld_authority_env_names_wave153_ok
        && r.gameworld_authority_method_names_wave153_ok
        && r.gameworld_authority_nav_commands_wave153_ok
        // Wave 154 residual honesty (WindowVideo; never flips playable_claim).
        && r.window_video_type_state_names_wave154_ok
        && r.window_video_method_names_wave154_ok
        && r.window_video_nav_commands_wave154_ok
        // Wave 155 residual honesty (MainMenu layout; never flips playable_claim).
        && r.main_menu_layout_names_wave155_ok
        && r.main_menu_layout_nav_commands_wave155_ok
        // Wave 156 residual honesty (ControlBarScheme; never flips playable_claim).
        && r.control_bar_scheme_names_wave156_ok
        && r.control_bar_scheme_method_names_wave156_ok
        && r.control_bar_scheme_nav_commands_wave156_ok
        // Wave 157 residual honesty (presentation boundary; never flips playable_claim).
        && r.presentation_boundary_method_names_wave157_ok
        && r.presentation_boundary_source_markers_wave157_ok
        && r.presentation_boundary_nav_commands_wave157_ok
        && r.presentation_boundary_live_wave157_ok
        // Wave 158 residual honesty (ControlBar print positions; never flips playable_claim).
        && r.control_bar_print_names_wave158_ok
        && r.control_bar_print_nav_commands_wave158_ok
        // Wave 159 residual honesty (terrain/skybox presentation-first; never flips playable_claim).
        && r.terrain_env_boundary_method_names_wave159_ok
        && r.terrain_env_boundary_source_markers_wave159_ok
        && r.terrain_env_boundary_nav_commands_wave159_ok
        && r.terrain_env_boundary_live_wave159_ok
        // Wave 160 residual honesty (MainMenu.wnd resolve/validate; never flips playable_claim).
        && r.main_menu_wnd_names_wave160_ok
        && r.main_menu_wnd_nav_commands_wave160_ok
        && r.main_menu_wnd_live_wave160_ok
        // Wave 161 residual honesty (MainMenu.wnd headless load; never flips playable_claim).
        && r.main_menu_wnd_load_method_names_wave161_ok
        && r.main_menu_wnd_load_nav_commands_wave161_ok
        && r.main_menu_wnd_load_live_wave161_ok
        // Wave 162 residual honesty (MainMenu.wnd materialise; never flips playable_claim).
        && r.main_menu_wnd_materialise_method_names_wave162_ok
        && r.main_menu_wnd_materialise_nav_commands_wave162_ok
        && r.main_menu_wnd_materialise_live_wave162_ok
        // Wave 163 residual honesty (Shell stack push; never flips playable_claim).
        && r.shell_stack_push_method_names_wave163_ok
        && r.shell_stack_push_nav_commands_wave163_ok
        && r.shell_stack_push_live_wave163_ok
        // Wave 164 residual honesty (Shell→Skirmish nav; never flips playable_claim).
        && r.shell_skirmish_nav_method_names_wave164_ok
        && r.shell_skirmish_nav_commands_wave164_ok
        && r.shell_skirmish_nav_live_wave164_ok
        // Wave 165 residual honesty (ControlBar.wnd materialise; never flips playable_claim).
        && r.control_bar_materialise_method_names_wave165_ok
        && r.control_bar_materialise_nav_commands_wave165_ok
        && r.control_bar_materialise_live_wave165_ok
        // Wave 166 residual honesty (Skirmish options WND; never flips playable_claim).
        && r.skirmish_options_wnd_method_names_wave166_ok
        && r.skirmish_options_wnd_nav_commands_wave166_ok
        && r.skirmish_options_wnd_live_wave166_ok
        // Wave 167 residual honesty (NewGame stream drain; never flips playable_claim).
        && r.new_game_stream_method_names_wave167_ok
        && r.new_game_stream_nav_commands_wave167_ok
        && r.new_game_stream_live_wave167_ok
        // Wave 168 residual honesty (W3DMainMenuInit; never flips playable_claim).
        && r.w3d_main_menu_init_method_names_wave168_ok
        && r.w3d_main_menu_init_nav_commands_wave168_ok
        && r.w3d_main_menu_init_live_wave168_ok
        // Wave 169 residual honesty (start_game Loading; never flips playable_claim).
        && r.start_game_loading_method_names_wave169_ok
        && r.start_game_loading_nav_commands_wave169_ok
        && r.start_game_loading_live_wave169_ok
        // Wave 170 residual honesty (live map load; never flips playable_claim).
        && r.live_map_load_method_names_wave170_ok
        && r.live_map_load_nav_commands_wave170_ok
        && r.live_map_load_live_wave170_ok
        // Wave 171 residual honesty (live presentation seed; never flips playable_claim).
        && r.live_presentation_seed_method_names_wave171_ok
        && r.live_presentation_seed_nav_commands_wave171_ok
        && r.live_presentation_seed_live_wave171_ok
        // Wave 172 residual honesty (GameWorld shadow overlay; never flips playable_claim).
        && r.live_gameworld_shadow_overlay_method_names_wave172_ok
        && r.live_gameworld_shadow_overlay_nav_commands_wave172_ok
        && r.live_gameworld_shadow_overlay_live_wave172_ok
        // Wave 173 residual honesty (single-authority + golden teleport opt-in).
        && r.single_authority_combat_method_names_wave173_ok
        && r.single_authority_combat_nav_commands_wave173_ok
        && r.single_authority_combat_live_wave173_ok
        // Wave 174 residual honesty (presentation-only execute + GameClient OS-input disconnect).
        && r.presentation_client_boundary_method_names_wave174_ok
        && r.presentation_client_boundary_nav_commands_wave174_ok
        && r.presentation_client_boundary_live_wave174_ok
        // Wave 175 residual honesty (golden map-host victory mop-up + formula).
        && r.golden_map_host_victory_method_names_wave175_ok
        && r.golden_map_host_victory_nav_commands_wave175_ok
        && r.golden_map_host_victory_live_wave175_ok
        // Wave 176 residual honesty (executable InGame presentation boundary).
        && r.executable_presentation_boundary_method_names_wave176_ok
        && r.executable_presentation_boundary_nav_commands_wave176_ok
        && r.executable_presentation_boundary_live_wave176_ok
        // Wave 177 residual honesty (GameWorld production authority default-on + sole-tick).
        && r.production_authority_env_ok
        && r.gameworld_production_authority_method_names_wave177_ok
        && r.gameworld_production_authority_nav_commands_wave177_ok
        && r.gameworld_production_authority_live_wave177_ok
        // Wave 178 residual honesty (sole-tick coupling + movement authority).
        && r.movement_authority_env_ok
        && r.gameworld_sole_tick_coupling_method_names_wave178_ok
        && r.gameworld_sole_tick_coupling_nav_commands_wave178_ok
        && r.gameworld_sole_tick_coupling_live_wave178_ok
        // Wave 179 residual honesty (full GameWorld authority matrix + engine couple).
        && r.ai_fire_construction_authority_env_ok
        && r.gameworld_authority_matrix_method_names_wave179_ok
        && r.gameworld_authority_matrix_nav_commands_wave179_ok
        && r.gameworld_authority_matrix_live_wave179_ok
        // Wave 180 residual honesty (live GameWorld production writeback + presentation shell).
        && r.live_gameworld_production_writeback_method_names_wave180_ok
        && r.live_gameworld_production_writeback_nav_commands_wave180_ok
        && r.live_gameworld_production_writeback_live_wave180_ok
        // Wave 181 residual honesty (live GameWorld construction writeback).
        && r.live_gameworld_construction_writeback_method_names_wave181_ok
        && r.live_gameworld_construction_writeback_nav_commands_wave181_ok
        && r.live_gameworld_construction_writeback_live_wave181_ok
        // Wave 182 residual honesty (live GameWorld damage channel parity).
        && r.live_gameworld_damage_channel_method_names_wave182_ok
        && r.live_gameworld_damage_channel_nav_commands_wave182_ok
        && r.live_gameworld_damage_channel_live_wave182_ok
        // Wave 183 residual honesty (live GameWorld economy writeback + movement channel).
        && r.live_gameworld_economy_movement_method_names_wave183_ok
        && r.live_gameworld_economy_movement_nav_commands_wave183_ok
        && r.live_gameworld_economy_movement_live_wave183_ok
        // Wave 184 residual honesty (live GameWorld projectile + AI decision channels).
        && r.live_gameworld_projectile_ai_method_names_wave184_ok
        && r.live_gameworld_projectile_ai_nav_commands_wave184_ok
        && r.live_gameworld_projectile_ai_live_wave184_ok
        // Wave 185 residual honesty (live GameWorld fire-spawn + special-power).
        && r.live_gameworld_fire_special_power_method_names_wave185_ok
        && r.live_gameworld_fire_special_power_nav_commands_wave185_ok
        && r.live_gameworld_fire_special_power_live_wave185_ok
        // Wave 186 residual honesty (live GameWorld presentation view after shadow tick).
        && r.live_gameworld_presentation_view_method_names_wave186_ok
        && r.live_gameworld_presentation_view_nav_commands_wave186_ok
        && r.live_gameworld_presentation_view_live_wave186_ok
        // Wave 187 residual honesty (PresentationFrame overlay from GameWorld shadow).
        && r.live_presentation_gameworld_overlay_method_names_wave187_ok
        && r.live_presentation_gameworld_overlay_nav_commands_wave187_ok
        && r.live_presentation_gameworld_overlay_live_wave187_ok
        // Wave 188 residual honesty (executable GameWorld presentation entities gate).
        && r.executable_gameworld_presentation_method_names_wave188_ok
        && r.executable_gameworld_presentation_nav_commands_wave188_ok
        && r.executable_gameworld_presentation_live_wave188_ok
        // Wave 189 residual honesty (deepened PresentationFrame←GameWorld overlay).
        && r.live_presentation_overlay_deepen_method_names_wave189_ok
        && r.live_presentation_overlay_deepen_nav_commands_wave189_ok
        && r.live_presentation_overlay_deepen_live_wave189_ok
        // Wave 190 residual honesty (PresentationFrame gameworld_overlay_stamped).
        && r.live_presentation_overlay_stamp_method_names_wave190_ok
        && r.live_presentation_overlay_stamp_nav_commands_wave190_ok
        && r.live_presentation_overlay_stamp_live_wave190_ok
        // Wave 191 residual honesty (enriched GameWorldEntityView + exec overlay stamp gate).
        && r.live_gameworld_entity_view_deepen_method_names_wave191_ok
        && r.live_gameworld_entity_view_deepen_nav_commands_wave191_ok
        && r.live_gameworld_entity_view_deepen_live_wave191_ok
        // Wave 192 residual honesty (append_missing_from_gameworld sparse objects).
        && r.live_presentation_append_missing_method_names_wave192_ok
        && r.live_presentation_append_missing_nav_commands_wave192_ok
        && r.live_presentation_append_missing_live_wave192_ok
        // Wave 193 residual honesty (build_from_gameworld / rebuild_objects).
        && r.live_presentation_build_from_gameworld_method_names_wave193_ok
        && r.live_presentation_build_from_gameworld_nav_commands_wave193_ok
        && r.live_presentation_build_from_gameworld_live_wave193_ok
        // Wave 194 residual honesty (GameWorld presentation default ON).
        && r.live_presentation_from_gameworld_default_method_names_wave194_ok
        && r.live_presentation_from_gameworld_default_nav_commands_wave194_ok
        && r.live_presentation_from_gameworld_default_live_wave194_ok
        // Wave 195 residual honesty (build_for_engine single presentation path).
        && r.live_presentation_build_for_engine_method_names_wave195_ok
        && r.live_presentation_build_for_engine_nav_commands_wave195_ok
        && r.live_presentation_build_for_engine_live_wave195_ok
        // Wave 196 residual honesty (rebuilt vertical gate + primary objects flag).
        && r.live_presentation_rebuilt_vertical_gate_method_names_wave196_ok
        && r.live_presentation_rebuilt_vertical_gate_nav_commands_wave196_ok
        && r.live_presentation_rebuilt_vertical_gate_live_wave196_ok
        // Wave 197 residual honesty (command attack log → GameWorld shadow).
        && r.live_command_attack_log_method_names_wave197_ok
        && r.live_command_attack_log_nav_commands_wave197_ok
        && r.live_command_attack_log_live_wave197_ok
        // Wave 198 residual honesty (command guard log → GameWorld SetGuard).
        && r.live_command_guard_log_method_names_wave198_ok
        && r.live_command_guard_log_nav_commands_wave198_ok
        && r.live_command_guard_log_live_wave198_ok
        // Wave 199 residual honesty (production cancel + construction start logs).
        && r.live_command_production_construction_log_method_names_wave199_ok
        && r.live_command_production_construction_log_nav_commands_wave199_ok
        && r.live_command_production_construction_log_live_wave199_ok
        // Wave 200 residual honesty (rally point log → GameWorld SetRallyPoint).
        && r.live_command_rally_log_method_names_wave200_ok
        && r.live_command_rally_log_nav_commands_wave200_ok
        && r.live_command_rally_log_live_wave200_ok
        // Wave 201 residual honesty (evacuate contain log + build_for_engine-only eng).
        && r.live_evacuate_contain_log_method_names_wave201_ok
        && r.live_evacuate_contain_log_nav_commands_wave201_ok
        && r.live_evacuate_contain_log_live_wave201_ok
        // Wave 202 residual honesty (cheer + science purchase progress logs).
        && r.live_command_cheer_science_log_method_names_wave202_ok
        && r.live_command_cheer_science_log_nav_commands_wave202_ok
        && r.live_command_cheer_science_log_live_wave202_ok
        // Wave 203 residual honesty (deploy → status log).
        && r.live_command_deploy_status_log_method_names_wave203_ok
        && r.live_command_deploy_status_log_nav_commands_wave203_ok
        && r.live_command_deploy_status_log_live_wave203_ok
        // Wave 204 residual honesty (CreateFormation → SetFormation).
        && r.live_command_formation_log_method_names_wave204_ok
        && r.live_command_formation_log_nav_commands_wave204_ok
        && r.live_command_formation_log_live_wave204_ok
        // Wave 205 residual honesty (ability pathing → set_order_target).
        && r.live_command_order_target_log_method_names_wave205_ok
        && r.live_command_order_target_log_nav_commands_wave205_ok
        && r.live_command_order_target_log_live_wave205_ok
        // Wave 206 residual honesty (selection → select/deselect status log).
        && r.live_command_selection_log_method_names_wave206_ok
        && r.live_command_selection_log_nav_commands_wave206_ok
        && r.live_command_selection_log_live_wave206_ok
        // Wave 207 residual honesty (enter/dock/repair → set_order_target).
        && r.live_command_non_attack_order_target_method_names_wave207_ok
        && r.live_command_non_attack_order_target_nav_commands_wave207_ok
        && r.live_command_non_attack_order_target_live_wave207_ok
        // Wave 208 residual honesty (golden mop-up holes/dead only).
        && r.live_golden_mopup_honesty_method_names_wave208_ok
        && r.live_golden_mopup_honesty_nav_commands_wave208_ok
        && r.live_golden_mopup_honesty_live_wave208_ok
        // Wave 209 residual honesty (OS mouse → handle_*_click command intake).
        && r.live_os_input_command_path_method_names_wave209_ok
        && r.live_os_input_command_path_nav_commands_wave209_ok
        && r.live_os_input_command_path_live_wave209_ok
        // Wave 210 residual honesty (PlaceBeacon → note_beacon_placed).
        && r.live_command_beacon_note_method_names_wave210_ok
        && r.live_command_beacon_note_nav_commands_wave210_ok
        && r.live_command_beacon_note_live_wave210_ok
        // Wave 211 residual honesty (host_beacons presentation freeze).
        && r.live_host_beacon_presentation_method_names_wave211_ok
        && r.live_host_beacon_presentation_nav_commands_wave211_ok
        && r.live_host_beacon_presentation_live_wave211_ok
        // Wave 212 residual honesty (sell → deselect status log).
        && r.live_command_sell_deselect_log_method_names_wave212_ok
        && r.live_command_sell_deselect_log_nav_commands_wave212_ok
        && r.live_command_sell_deselect_log_live_wave212_ok
        // Wave 213 residual honesty (presentation FOW snapshot only).
        && r.live_presentation_fow_only_method_names_wave213_ok
        && r.live_presentation_fow_only_nav_commands_wave213_ok
        && r.live_presentation_fow_only_live_wave213_ok
        // Wave 214 residual honesty (UI producer pick presentation-only).
        && r.live_ui_producer_presentation_only_method_names_wave214_ok
        && r.live_ui_producer_presentation_only_nav_commands_wave214_ok
        && r.live_ui_producer_presentation_only_live_wave214_ok
        // Wave 215 residual honesty (UI helpers presentation-only).
        && r.live_ui_helpers_presentation_only_method_names_wave215_ok
        && r.live_ui_helpers_presentation_only_nav_commands_wave215_ok
        && r.live_ui_helpers_presentation_only_live_wave215_ok
        // Wave 216 residual honesty (control-group/camera presentation-only).
        && r.live_control_group_camera_presentation_only_method_names_wave216_ok
        && r.live_control_group_camera_presentation_only_nav_commands_wave216_ok
        && r.live_control_group_camera_presentation_only_live_wave216_ok
        // Wave 217 residual honesty (cmd-filter/env presentation-only).
        && r.live_cmd_filter_env_presentation_only_method_names_wave217_ok
        && r.live_cmd_filter_env_presentation_only_nav_commands_wave217_ok
        && r.live_cmd_filter_env_presentation_only_live_wave217_ok
        // Wave 218 residual honesty (selection-commands presentation-only).
        && r.live_selection_commands_presentation_only_method_names_wave218_ok
        && r.live_selection_commands_presentation_only_nav_commands_wave218_ok
        && r.live_selection_commands_presentation_only_live_wave218_ok
        // Wave 219 residual honesty (UI-command selection presentation-only).
        && r.live_ui_command_selection_presentation_only_method_names_wave219_ok
        && r.live_ui_command_selection_presentation_only_nav_commands_wave219_ok
        && r.live_ui_command_selection_presentation_only_live_wave219_ok
        // Wave 220 residual honesty (local-team presentation-only).
        && r.live_local_team_presentation_only_method_names_wave220_ok
        && r.live_local_team_presentation_only_nav_commands_wave220_ok
        && r.live_local_team_presentation_only_live_wave220_ok
        // Wave 221 residual honesty (hotkey/move/attack selection presentation-only).
        && r.live_hotkey_move_attack_selection_presentation_only_method_names_wave221_ok
        && r.live_hotkey_move_attack_selection_presentation_only_nav_commands_wave221_ok
        && r.live_hotkey_move_attack_selection_presentation_only_live_wave221_ok
        // Wave 222 residual honesty (pick-object presentation-only).
        && r.live_pick_object_presentation_only_method_names_wave222_ok
        && r.live_pick_object_presentation_only_nav_commands_wave222_ok
        && r.live_pick_object_presentation_only_live_wave222_ok
        // Wave 223 residual honesty (bootstrap-camera presentation-only).
        && r.live_bootstrap_camera_presentation_only_method_names_wave223_ok
        && r.live_bootstrap_camera_presentation_only_nav_commands_wave223_ok
        && r.live_bootstrap_camera_presentation_only_live_wave223_ok
        // Wave 224 residual honesty (force-complete authority API).
        && r.live_force_complete_authority_api_method_names_wave224_ok
        && r.live_force_complete_authority_api_nav_commands_wave224_ok
        && r.live_force_complete_authority_api_live_wave224_ok
        // Wave 225 residual honesty (path/guard authority API).
        && r.live_path_guard_authority_api_method_names_wave225_ok
        && r.live_path_guard_authority_api_nav_commands_wave225_ok
        && r.live_path_guard_authority_api_live_wave225_ok
        // Wave 226 residual honesty (hotkey selection/camera presentation-only).
        && r.live_hotkey_selection_camera_presentation_only_method_names_wave226_ok
        && r.live_hotkey_selection_camera_presentation_only_nav_commands_wave226_ok
        && r.live_hotkey_selection_camera_presentation_only_live_wave226_ok
        // Wave 227 residual honesty (construct spawn pose authority API).
        && r.live_construct_spawn_pose_authority_api_method_names_wave227_ok
        && r.live_construct_spawn_pose_authority_api_nav_commands_wave227_ok
        && r.live_construct_spawn_pose_authority_api_live_wave227_ok
        // Wave 228 residual honesty (RMB target presentation-only).
        && r.live_rmb_target_presentation_only_method_names_wave228_ok
        && r.live_rmb_target_presentation_only_nav_commands_wave228_ok
        && r.live_rmb_target_presentation_only_live_wave228_ok
        // Wave 229 residual honesty (RMB selected presentation-only).
        && r.live_rmb_selected_presentation_only_method_names_wave229_ok
        && r.live_rmb_selected_presentation_only_nav_commands_wave229_ok
        && r.live_rmb_selected_presentation_only_live_wave229_ok
        // Wave 230 residual honesty (command unit authority API).
        && r.live_command_unit_authority_api_method_names_wave230_ok
        && r.live_command_unit_authority_api_nav_commands_wave230_ok
        && r.live_command_unit_authority_api_live_wave230_ok
        // Wave 231 residual honesty (command unit more-authority API).
        && r.live_command_unit_more_authority_api_method_names_wave231_ok
        && r.live_command_unit_more_authority_api_nav_commands_wave231_ok
        && r.live_command_unit_more_authority_api_live_wave231_ok
        // Wave 232 residual honesty (command executor authority API).
        && r.live_command_executor_authority_api_method_names_wave232_ok
        && r.live_command_executor_authority_api_nav_commands_wave232_ok
        && r.live_command_executor_authority_api_live_wave232_ok
        // Wave 233 residual honesty (command executor more-authority API).
        && r.live_command_executor_more_authority_api_method_names_wave233_ok
        && r.live_command_executor_more_authority_api_nav_commands_wave233_ok
        && r.live_command_executor_more_authority_api_live_wave233_ok
        // Wave 234 residual honesty (engine presentation player UI).
        && r.live_engine_presentation_player_ui_method_names_wave234_ok
        && r.live_engine_presentation_player_ui_nav_commands_wave234_ok
        && r.live_engine_presentation_player_ui_live_wave234_ok
        // Wave 235 residual honesty (RMB presentation full classify).
        && r.live_rmb_presentation_full_classify_method_names_wave235_ok
        && r.live_rmb_presentation_full_classify_nav_commands_wave235_ok
        && r.live_rmb_presentation_full_classify_live_wave235_ok
        // Wave 236 residual honesty (mouse input presentation-only).
        && r.live_mouse_input_presentation_only_method_names_wave236_ok
        && r.live_mouse_input_presentation_only_nav_commands_wave236_ok
        && r.live_mouse_input_presentation_only_live_wave236_ok
        // Wave 237 residual honesty (engine player UI boot peel).
        && r.live_engine_player_ui_boot_peel_method_names_wave237_ok
        && r.live_engine_player_ui_boot_peel_nav_commands_wave237_ok
        && r.live_engine_player_ui_boot_peel_live_wave237_ok
        // Wave 238 residual honesty (player probe API).
        && r.live_player_probe_api_method_names_wave238_ok
        && r.live_player_probe_api_nav_commands_wave238_ok
        && r.live_player_probe_api_live_wave238_ok
        // Wave 239 residual honesty (player team probe).
        && r.live_player_team_probe_method_names_wave239_ok
        && r.live_player_team_probe_nav_commands_wave239_ok
        && r.live_player_team_probe_live_wave239_ok
        // Wave 240 residual honesty (player field probe).
        && r.live_player_field_probe_method_names_wave240_ok
        && r.live_player_field_probe_nav_commands_wave240_ok
        && r.live_player_field_probe_live_wave240_ok
        // Wave 241 residual honesty (camera height probe).
        && r.live_camera_height_probe_method_names_wave241_ok
        && r.live_camera_height_probe_nav_commands_wave241_ok
        && r.live_camera_height_probe_live_wave241_ok
        // Wave 242 residual honesty (command player probe).
        && r.live_command_player_probe_method_names_wave242_ok
        && r.live_command_player_probe_nav_commands_wave242_ok
        && r.live_command_player_probe_live_wave242_ok
        // Wave 243 residual honesty (construct economy probe).
        && r.live_construct_economy_probe_method_names_wave243_ok
        && r.live_construct_economy_probe_nav_commands_wave243_ok
        && r.live_construct_economy_probe_live_wave243_ok
        // Wave 244 residual honesty (command unit probe).
        && r.live_command_unit_probe_method_names_wave244_ok
        && r.live_command_unit_probe_nav_commands_wave244_ok
        && r.live_command_unit_probe_live_wave244_ok
        // Wave 245 residual honesty (selection query probe).
        && r.live_selection_query_probe_method_names_wave245_ok
        && r.live_selection_query_probe_nav_commands_wave245_ok
        && r.live_selection_query_probe_live_wave245_ok
        // Wave 246 residual honesty (world pick probe).
        && r.live_world_pick_probe_method_names_wave246_ok
        && r.live_world_pick_probe_nav_commands_wave246_ok
        && r.live_world_pick_probe_live_wave246_ok
        // Wave 247 residual honesty (object registry empty fastpath).
        && r.live_object_registry_empty_fastpath_method_names_wave247_ok
        && r.live_object_registry_empty_fastpath_nav_commands_wave247_ok
        && r.live_object_registry_empty_fastpath_live_wave247_ok
        // Wave 248 residual honesty (legacy object registry fastpath).
        && r.live_legacy_object_registry_fastpath_method_names_wave248_ok
        && r.live_legacy_object_registry_fastpath_nav_commands_wave248_ok
        && r.live_legacy_object_registry_fastpath_live_wave248_ok
        // Wave 249 residual honesty (client dual-world empty gate).
        && r.live_client_dual_world_empty_gate_method_names_wave249_ok
        && r.live_client_dual_world_empty_gate_nav_commands_wave249_ok
        && r.live_client_dual_world_empty_gate_live_wave249_ok
        // Wave 250 residual honesty (presentation time frozen probe).
        && r.live_presentation_time_frozen_probe_method_names_wave250_ok
        && r.live_presentation_time_frozen_probe_nav_commands_wave250_ok
        && r.live_presentation_time_frozen_probe_live_wave250_ok
        // Wave 251 residual honesty (presentation visual speed probe).
        && r.live_presentation_visual_speed_probe_method_names_wave251_ok
        && r.live_presentation_visual_speed_probe_nav_commands_wave251_ok
        && r.live_presentation_visual_speed_probe_live_wave251_ok
        // Wave 252 residual honesty (presentation script camera probe).
        && r.live_presentation_script_camera_probe_method_names_wave252_ok
        && r.live_presentation_script_camera_probe_nav_commands_wave252_ok
        && r.live_presentation_script_camera_probe_live_wave252_ok
        // Wave 253 residual honesty (AIGroup dual-world empty gates).
        && r.live_ai_group_dual_world_empty_gate_method_names_wave253_ok
        && r.live_ai_group_dual_world_empty_gate_nav_commands_wave253_ok
        && r.live_ai_group_dual_world_empty_gate_live_wave253_ok
        // Wave 254 residual honesty (AIStates dual-world empty gates).
        && r.live_ai_states_dual_world_empty_gate_method_names_wave254_ok
        && r.live_ai_states_dual_world_empty_gate_nav_commands_wave254_ok
        && r.live_ai_states_dual_world_empty_gate_live_wave254_ok
        // Wave 255 residual honesty (AIPlayer dual-world empty gates).
        && r.live_ai_player_dual_world_empty_gate_method_names_wave255_ok
        && r.live_ai_player_dual_world_empty_gate_nav_commands_wave255_ok
        && r.live_ai_player_dual_world_empty_gate_live_wave255_ok
        // Wave 256 residual honesty (Team dual-world empty gates).
        && r.live_team_dual_world_empty_gate_method_names_wave256_ok
        && r.live_team_dual_world_empty_gate_nav_commands_wave256_ok
        && r.live_team_dual_world_empty_gate_live_wave256_ok
        // Wave 257 residual honesty (legacy AI states dual-world empty gates).
        && r.live_ai_legacy_states_dual_world_empty_gate_method_names_wave257_ok
        && r.live_ai_legacy_states_dual_world_empty_gate_nav_commands_wave257_ok
        && r.live_ai_legacy_states_dual_world_empty_gate_live_wave257_ok
        // Wave 258 residual honesty (Unit dual-world empty gates).
        && r.live_unit_dual_world_empty_gate_method_names_wave258_ok
        && r.live_unit_dual_world_empty_gate_nav_commands_wave258_ok
        && r.live_unit_dual_world_empty_gate_live_wave258_ok
        // Wave 259 residual honesty (Stealth dual-world empty gates).
        && r.live_stealth_dual_world_empty_gate_method_names_wave259_ok
        && r.live_stealth_dual_world_empty_gate_nav_commands_wave259_ok
        && r.live_stealth_dual_world_empty_gate_live_wave259_ok
        // Wave 260 residual honesty (Garrison dual-world empty gates).
        && r.live_garrison_dual_world_empty_gate_method_names_wave260_ok
        && r.live_garrison_dual_world_empty_gate_nav_commands_wave260_ok
        && r.live_garrison_dual_world_empty_gate_live_wave260_ok
        // Wave 261 residual honesty (OpenContain dual-world empty gates).
        && r.live_open_contain_dual_world_empty_gate_method_names_wave261_ok
        && r.live_open_contain_dual_world_empty_gate_nav_commands_wave261_ok
        && r.live_open_contain_dual_world_empty_gate_live_wave261_ok
        // Wave 262 residual honesty (Pathfind dual-world empty gates).
        && r.live_pathfind_dual_world_empty_gate_method_names_wave262_ok
        && r.live_pathfind_dual_world_empty_gate_nav_commands_wave262_ok
        && r.live_pathfind_dual_world_empty_gate_live_wave262_ok
        // Wave 263 residual honesty (AI mod dual-world empty gates).
        && r.live_ai_mod_dual_world_empty_gate_method_names_wave263_ok
        && r.live_ai_mod_dual_world_empty_gate_nav_commands_wave263_ok
        && r.live_ai_mod_dual_world_empty_gate_live_wave263_ok
        // Wave 264 residual honesty (Object mod dual-world empty gates).
        && r.live_object_mod_dual_world_empty_gate_method_names_wave264_ok
        && r.live_object_mod_dual_world_empty_gate_nav_commands_wave264_ok
        && r.live_object_mod_dual_world_empty_gate_live_wave264_ok
        // Wave 265 residual honesty (Weapon dual-world empty gates).
        && r.live_weapon_dual_world_empty_gate_method_names_wave265_ok
        && r.live_weapon_dual_world_empty_gate_nav_commands_wave265_ok
        && r.live_weapon_dual_world_empty_gate_live_wave265_ok
        // Wave 266 residual honesty (Partition filters dual-world empty gates).
        && r.live_partition_filters_dual_world_empty_gate_method_names_wave266_ok
        && r.live_partition_filters_dual_world_empty_gate_nav_commands_wave266_ok
        && r.live_partition_filters_dual_world_empty_gate_live_wave266_ok
        // Wave 267 residual honesty (AI state machine dual-world empty gates).
        && r.live_ai_state_machine_dual_world_empty_gate_method_names_wave267_ok
        && r.live_ai_state_machine_dual_world_empty_gate_nav_commands_wave267_ok
        && r.live_ai_state_machine_dual_world_empty_gate_live_wave267_ok
        // Wave 268 residual honesty (Player dual-world empty gates).
        && r.live_player_dual_world_empty_gate_method_names_wave268_ok
        && r.live_player_dual_world_empty_gate_nav_commands_wave268_ok
        && r.live_player_dual_world_empty_gate_live_wave268_ok
        // Wave 269 residual honesty (GameClient dual-world empty gates).
        && r.live_game_client_dual_world_empty_gate_method_names_wave269_ok
        && r.live_game_client_dual_world_empty_gate_nav_commands_wave269_ok
        && r.live_game_client_dual_world_empty_gate_live_wave269_ok
        // Wave 270 residual honesty (Drawable dual-world empty gates).
        && r.live_drawable_dual_world_empty_gate_method_names_wave270_ok
        && r.live_drawable_dual_world_empty_gate_nav_commands_wave270_ok
        && r.live_drawable_dual_world_empty_gate_live_wave270_ok
        // Wave 271 residual honesty (script conditions dual-world empty gates).
        && r.live_script_conditions_dual_world_empty_gate_method_names_wave271_ok
        && r.live_script_conditions_dual_world_empty_gate_nav_commands_wave271_ok
        && r.live_script_conditions_dual_world_empty_gate_live_wave271_ok
        // Wave 272 residual honesty (TransportContain dual-world empty gates).
        && r.live_transport_contain_dual_world_empty_gate_method_names_wave272_ok
        && r.live_transport_contain_dual_world_empty_gate_nav_commands_wave272_ok
        && r.live_transport_contain_dual_world_empty_gate_live_wave272_ok
        // Wave 273 residual honesty (InGameUI dual-world empty gates).
        && r.live_ingame_ui_dual_world_empty_gate_method_names_wave273_ok
        && r.live_ingame_ui_dual_world_empty_gate_nav_commands_wave273_ok
        && r.live_ingame_ui_dual_world_empty_gate_live_wave273_ok
        // Wave 274 residual honesty (HelixContain dual-world empty gates).
        && r.live_helix_contain_dual_world_empty_gate_method_names_wave274_ok
        && r.live_helix_contain_dual_world_empty_gate_nav_commands_wave274_ok
        && r.live_helix_contain_dual_world_empty_gate_live_wave274_ok
        // Wave 275 residual honesty (CommandProcessor dual-world empty gates).
        && r.live_command_processor_dual_world_empty_gate_method_names_wave275_ok
        && r.live_command_processor_dual_world_empty_gate_nav_commands_wave275_ok
        && r.live_command_processor_dual_world_empty_gate_live_wave275_ok
        // Wave 276 residual honesty (Turret dual-world empty gates).
        && r.live_turret_dual_world_empty_gate_method_names_wave276_ok
        && r.live_turret_dual_world_empty_gate_nav_commands_wave276_ok
        && r.live_turret_dual_world_empty_gate_live_wave276_ok
        // Wave 277 residual honesty (RiderChangeContain dual-world empty gates).
        && r.live_rider_change_contain_dual_world_empty_gate_method_names_wave277_ok
        && r.live_rider_change_contain_dual_world_empty_gate_nav_commands_wave277_ok
        && r.live_rider_change_contain_dual_world_empty_gate_live_wave277_ok
        // Wave 278 residual honesty (Selection dual-world empty gates).
        && r.live_selection_dual_world_empty_gate_method_names_wave278_ok
        && r.live_selection_dual_world_empty_gate_nav_commands_wave278_ok
        && r.live_selection_dual_world_empty_gate_live_wave278_ok
        // Wave 279 residual honesty (CaveContain dual-world empty gates).
        && r.live_cave_contain_dual_world_empty_gate_method_names_wave279_ok
        && r.live_cave_contain_dual_world_empty_gate_nav_commands_wave279_ok
        && r.live_cave_contain_dual_world_empty_gate_live_wave279_ok
        // Wave 280 residual honesty (TunnelContain dual-world empty gates).
        && r.live_tunnel_contain_dual_world_empty_gate_method_names_wave280_ok
        && r.live_tunnel_contain_dual_world_empty_gate_nav_commands_wave280_ok
        && r.live_tunnel_contain_dual_world_empty_gate_live_wave280_ok
        // Wave 281 residual honesty (helpers dual-world empty gates).
        && r.live_helpers_dual_world_empty_gate_method_names_wave281_ok
        && r.live_helpers_dual_world_empty_gate_nav_commands_wave281_ok
        && r.live_helpers_dual_world_empty_gate_live_wave281_ok
        // Wave 282 residual honesty (AIUpdateInterface dual-world empty gates).
        && r.live_ai_update_interface_dual_world_empty_gate_method_names_wave282_ok
        && r.live_ai_update_interface_dual_world_empty_gate_nav_commands_wave282_ok
        && r.live_ai_update_interface_dual_world_empty_gate_live_wave282_ok
        // Wave 283 residual honesty (StealthUpdate dual-world empty gates).
        && r.live_stealth_update_dual_world_empty_gate_method_names_wave283_ok
        && r.live_stealth_update_dual_world_empty_gate_nav_commands_wave283_ok
        && r.live_stealth_update_dual_world_empty_gate_live_wave283_ok
        // Wave 284 residual honesty (script executor dual-world empty gates).
        && r.live_script_executor_dual_world_empty_gate_method_names_wave284_ok
        && r.live_script_executor_dual_world_empty_gate_nav_commands_wave284_ok
        && r.live_script_executor_dual_world_empty_gate_live_wave284_ok
        // Wave 285 residual honesty (AI integration dual-world empty gates).
        && r.live_ai_integration_dual_world_empty_gate_method_names_wave285_ok
        && r.live_ai_integration_dual_world_empty_gate_nav_commands_wave285_ok
        && r.live_ai_integration_dual_world_empty_gate_live_wave285_ok
        // Wave 286 residual honesty (DumbProjectile dual-world empty gates).
        && r.live_dumb_projectile_dual_world_empty_gate_method_names_wave286_ok
        && r.live_dumb_projectile_dual_world_empty_gate_nav_commands_wave286_ok
        && r.live_dumb_projectile_dual_world_empty_gate_live_wave286_ok
        // Wave 287 residual honesty (enhanced player dual-world empty gates).
        && r.live_enhanced_player_dual_world_empty_gate_method_names_wave287_ok
        && r.live_enhanced_player_dual_world_empty_gate_nav_commands_wave287_ok
        && r.live_enhanced_player_dual_world_empty_gate_live_wave287_ok
        // Wave 288 residual honesty (hijacker update dual-world empty gates).
        && r.live_hijacker_update_dual_world_empty_gate_method_names_wave288_ok
        && r.live_hijacker_update_dual_world_empty_gate_nav_commands_wave288_ok
        && r.live_hijacker_update_dual_world_empty_gate_live_wave288_ok
        // Wave 289 residual honesty (weapon dual-world empty gates).
        && r.live_weapon_impl_dual_world_empty_gate_method_names_wave289_ok
        && r.live_weapon_impl_dual_world_empty_gate_nav_commands_wave289_ok
        && r.live_weapon_impl_dual_world_empty_gate_live_wave289_ok
        // Wave 290 residual honesty (async player dual-world empty gates).
        && r.live_async_player_dual_world_empty_gate_method_names_wave290_ok
        && r.live_async_player_dual_world_empty_gate_nav_commands_wave290_ok
        && r.live_async_player_dual_world_empty_gate_live_wave290_ok
        // Wave 291 residual honesty (active body dual-world empty gates).
        && r.live_active_body_dual_world_empty_gate_method_names_wave291_ok
        && r.live_active_body_dual_world_empty_gate_nav_commands_wave291_ok
        && r.live_active_body_dual_world_empty_gate_live_wave291_ok
        // Wave 292 residual honesty (skirmish conditions dual-world empty gates).
        && r.live_skirmish_conditions_dual_world_empty_gate_method_names_wave292_ok
        && r.live_skirmish_conditions_dual_world_empty_gate_nav_commands_wave292_ok
        && r.live_skirmish_conditions_dual_world_empty_gate_live_wave292_ok
        // Wave 293 residual honesty (AI build list dual-world empty gates).
        && r.live_ai_build_list_dual_world_empty_gate_method_names_wave293_ok
        && r.live_ai_build_list_dual_world_empty_gate_nav_commands_wave293_ok
        && r.live_ai_build_list_dual_world_empty_gate_live_wave293_ok
        // Wave 294 residual honesty (victory dual-world empty gates).
        && r.live_victory_dual_world_empty_gate_method_names_wave294_ok
        && r.live_victory_dual_world_empty_gate_nav_commands_wave294_ok
        && r.live_victory_dual_world_empty_gate_live_wave294_ok
        // Wave 295 residual honesty (script actions dual-world empty gates).
        && r.live_script_actions_dual_world_empty_gate_method_names_wave295_ok
        && r.live_script_actions_dual_world_empty_gate_nav_commands_wave295_ok
        && r.live_script_actions_dual_world_empty_gate_live_wave295_ok
        // Wave 296 residual honesty (special ability dual-world empty gates).
        && r.live_special_ability_dual_world_empty_gate_method_names_wave296_ok
        && r.live_special_ability_dual_world_empty_gate_nav_commands_wave296_ok
        && r.live_special_ability_dual_world_empty_gate_live_wave296_ok
        // Wave 297 residual honesty (stealth detector dual-world empty gates).
        && r.live_stealth_detector_dual_world_empty_gate_method_names_wave297_ok
        && r.live_stealth_detector_dual_world_empty_gate_nav_commands_wave297_ok
        && r.live_stealth_detector_dual_world_empty_gate_live_wave297_ok
        // Wave 298 residual honesty (supply system dual-world empty gates).
        && r.live_supply_system_dual_world_empty_gate_method_names_wave298_ok
        && r.live_supply_system_dual_world_empty_gate_nav_commands_wave298_ok
        && r.live_supply_system_dual_world_empty_gate_live_wave298_ok
        // Wave 299 residual honesty (particle uplink dual-world empty gates).
        && r.live_particle_uplink_dual_world_empty_gate_method_names_wave299_ok
        && r.live_particle_uplink_dual_world_empty_gate_nav_commands_wave299_ok
        && r.live_particle_uplink_dual_world_empty_gate_live_wave299_ok
        // Wave 300 residual honesty (overlord contain dual-world empty gates).
        && r.live_overlord_contain_dual_world_empty_gate_method_names_wave300_ok
        && r.live_overlord_contain_dual_world_empty_gate_nav_commands_wave300_ok
        && r.live_overlord_contain_dual_world_empty_gate_live_wave300_ok
        // Wave 301 residual honesty (bridge behavior dual-world empty gates).
        && r.live_bridge_behavior_dual_world_empty_gate_method_names_wave301_ok
        && r.live_bridge_behavior_dual_world_empty_gate_nav_commands_wave301_ok
        && r.live_bridge_behavior_dual_world_empty_gate_live_wave301_ok
        // Wave 302 residual honesty (stealth behavior dual-world empty gates).
        && r.live_stealth_behavior_dual_world_empty_gate_method_names_wave302_ok
        && r.live_stealth_behavior_dual_world_empty_gate_nav_commands_wave302_ok
        && r.live_stealth_behavior_dual_world_empty_gate_live_wave302_ok
        // Wave 303 residual honesty (crate collide dual-world empty gates).
        && r.live_crate_collide_dual_world_empty_gate_method_names_wave303_ok
        && r.live_crate_collide_dual_world_empty_gate_nav_commands_wave303_ok
        && r.live_crate_collide_dual_world_empty_gate_live_wave303_ok
        // Wave 304 residual honesty (object manager dual-world empty gates).
        && r.live_object_manager_dual_world_empty_gate_method_names_wave304_ok
        && r.live_object_manager_dual_world_empty_gate_nav_commands_wave304_ok
        && r.live_object_manager_dual_world_empty_gate_live_wave304_ok
        // Wave 305 residual honesty (sticky bomb dual-world empty gates).
        && r.live_sticky_bomb_dual_world_empty_gate_method_names_wave305_ok
        && r.live_sticky_bomb_dual_world_empty_gate_nav_commands_wave305_ok
        && r.live_sticky_bomb_dual_world_empty_gate_live_wave305_ok
        // Wave 306 residual honesty (auto heal dual-world empty gates).
        && r.live_auto_heal_dual_world_empty_gate_method_names_wave306_ok
        && r.live_auto_heal_dual_world_empty_gate_nav_commands_wave306_ok
        && r.live_auto_heal_dual_world_empty_gate_live_wave306_ok
        // Wave 307 residual honesty (grant stealth dual-world empty gates).
        && r.live_grant_stealth_dual_world_empty_gate_method_names_wave307_ok
        && r.live_grant_stealth_dual_world_empty_gate_nav_commands_wave307_ok
        && r.live_grant_stealth_dual_world_empty_gate_live_wave307_ok
        // Wave 308 residual honesty (status bits upgrade dual-world empty gates).
        && r.live_status_bits_upgrade_dual_world_empty_gate_method_names_wave308_ok
        && r.live_status_bits_upgrade_dual_world_empty_gate_nav_commands_wave308_ok
        && r.live_status_bits_upgrade_dual_world_empty_gate_live_wave308_ok
        // Wave 309 residual honesty (jet AI dual-world empty gates).
        && r.live_jet_ai_dual_world_empty_gate_method_names_wave309_ok
        && r.live_jet_ai_dual_world_empty_gate_nav_commands_wave309_ok
        && r.live_jet_ai_dual_world_empty_gate_live_wave309_ok
        // Wave 310 residual honesty (parking place dual-world empty gates).
        && r.live_parking_place_dual_world_empty_gate_method_names_wave310_ok
        && r.live_parking_place_dual_world_empty_gate_nav_commands_wave310_ok
        && r.live_parking_place_dual_world_empty_gate_live_wave310_ok
        // Wave 311 residual honesty (flight deck dual-world empty gates).
        && r.live_flight_deck_dual_world_empty_gate_method_names_wave311_ok
        && r.live_flight_deck_dual_world_empty_gate_nav_commands_wave311_ok
        && r.live_flight_deck_dual_world_empty_gate_live_wave311_ok
        // Wave 312 residual honesty (exit strategies dual-world empty gates).
        && r.live_exit_strategies_dual_world_empty_gate_method_names_wave312_ok
        && r.live_exit_strategies_dual_world_empty_gate_nav_commands_wave312_ok
        && r.live_exit_strategies_dual_world_empty_gate_live_wave312_ok
        // Wave 313 residual honesty (collision system dual-world empty gates).
        && r.live_collision_system_dual_world_empty_gate_method_names_wave313_ok
        && r.live_collision_system_dual_world_empty_gate_nav_commands_wave313_ok
        && r.live_collision_system_dual_world_empty_gate_live_wave313_ok
        // Wave 314 residual honesty (max health upgrade dual-world empty gates).
        && r.live_max_health_upgrade_dual_world_empty_gate_method_names_wave314_ok
        && r.live_max_health_upgrade_dual_world_empty_gate_nav_commands_wave314_ok
        && r.live_max_health_upgrade_dual_world_empty_gate_live_wave314_ok
        // Wave 315 residual honesty (structure topple dual-world empty gates).
        && r.live_structure_topple_dual_world_empty_gate_method_names_wave315_ok
        && r.live_structure_topple_dual_world_empty_gate_nav_commands_wave315_ok
        && r.live_structure_topple_dual_world_empty_gate_live_wave315_ok
        // Wave 316 residual honesty (physics update dual-world empty gates).
        && r.live_physics_update_dual_world_empty_gate_method_names_wave316_ok
        && r.live_physics_update_dual_world_empty_gate_nav_commands_wave316_ok
        && r.live_physics_update_dual_world_empty_gate_live_wave316_ok
        // Wave 317 residual honesty (cleanup hazard dual-world empty gates).
        && r.live_cleanup_hazard_dual_world_empty_gate_method_names_wave317_ok
        && r.live_cleanup_hazard_dual_world_empty_gate_nav_commands_wave317_ok
        && r.live_cleanup_hazard_dual_world_empty_gate_live_wave317_ok
        // Wave 318 residual honesty (bridge tower dual-world empty gates).
        && r.live_bridge_tower_dual_world_empty_gate_method_names_wave318_ok
        && r.live_bridge_tower_dual_world_empty_gate_nav_commands_wave318_ok
        && r.live_bridge_tower_dual_world_empty_gate_live_wave318_ok
        // Wave 319 residual honesty (armor upgrade dual-world empty gates).
        && r.live_armor_upgrade_dual_world_empty_gate_method_names_wave319_ok
        && r.live_armor_upgrade_dual_world_empty_gate_nav_commands_wave319_ok
        && r.live_armor_upgrade_dual_world_empty_gate_live_wave319_ok
        // Wave 320 residual honesty (paradrop power dual-world empty gates).
        && r.live_paradrop_power_dual_world_empty_gate_method_names_wave320_ok
        && r.live_paradrop_power_dual_world_empty_gate_nav_commands_wave320_ok
        && r.live_paradrop_power_dual_world_empty_gate_live_wave320_ok
        // Wave 321 residual honesty (fuel air bomb dual-world empty gates).
        && r.live_fuel_air_bomb_dual_world_empty_gate_method_names_wave321_ok
        && r.live_fuel_air_bomb_dual_world_empty_gate_nav_commands_wave321_ok
        && r.live_fuel_air_bomb_dual_world_empty_gate_live_wave321_ok
        // Wave 322 residual honesty (tensile formation dual-world empty gates).
        && r.live_tensile_formation_dual_world_empty_gate_method_names_wave322_ok
        && r.live_tensile_formation_dual_world_empty_gate_nav_commands_wave322_ok
        && r.live_tensile_formation_dual_world_empty_gate_live_wave322_ok
        // Wave 323 residual honesty (die mod dual-world empty gates).
        && r.live_die_mod_dual_world_empty_gate_method_names_wave323_ok
        && r.live_die_mod_dual_world_empty_gate_nav_commands_wave323_ok
        && r.live_die_mod_dual_world_empty_gate_live_wave323_ok
        // Wave 324 residual honesty (partition manager dual-world empty gates).
        && r.live_partition_manager_dual_world_empty_gate_method_names_wave324_ok
        && r.live_partition_manager_dual_world_empty_gate_nav_commands_wave324_ok
        && r.live_partition_manager_dual_world_empty_gate_live_wave324_ok
        // Wave 325 residual honesty (spectre gunship dual-world empty gates).
        && r.live_spectre_gunship_dual_world_empty_gate_method_names_wave325_ok
        && r.live_spectre_gunship_dual_world_empty_gate_nav_commands_wave325_ok
        && r.live_spectre_gunship_dual_world_empty_gate_live_wave325_ok
        // Wave 326 residual honesty (production update dual-world empty gates).
        && r.live_production_update_dual_world_empty_gate_method_names_wave326_ok
        && r.live_production_update_dual_world_empty_gate_nav_commands_wave326_ok
        && r.live_production_update_dual_world_empty_gate_live_wave326_ok
        // Wave 327 residual honesty (neutron blast dual-world empty gates).
        && r.live_neutron_blast_dual_world_empty_gate_method_names_wave327_ok
        && r.live_neutron_blast_dual_world_empty_gate_nav_commands_wave327_ok
        && r.live_neutron_blast_dual_world_empty_gate_live_wave327_ok
        // Wave 328 residual honesty (countermeasures dual-world empty gates).
        && r.live_countermeasures_dual_world_empty_gate_method_names_wave328_ok
        && r.live_countermeasures_dual_world_empty_gate_nav_commands_wave328_ok
        && r.live_countermeasures_dual_world_empty_gate_live_wave328_ok
        // Wave 329 residual honesty (skirmish player dual-world empty gates).
        && r.live_skirmish_player_dual_world_empty_gate_method_names_wave329_ok
        && r.live_skirmish_player_dual_world_empty_gate_nav_commands_wave329_ok
        && r.live_skirmish_player_dual_world_empty_gate_live_wave329_ok
        // Wave 330 residual honesty (a10 strike dual-world empty gates).
        && r.live_a10_strike_dual_world_empty_gate_method_names_wave330_ok
        && r.live_a10_strike_dual_world_empty_gate_nav_commands_wave330_ok
        && r.live_a10_strike_dual_world_empty_gate_live_wave330_ok
        // Wave 331 residual honesty (rebuild hole dual-world empty gates).
        && r.live_rebuild_hole_dual_world_empty_gate_method_names_wave331_ok
        && r.live_rebuild_hole_dual_world_empty_gate_nav_commands_wave331_ok
        && r.live_rebuild_hole_dual_world_empty_gate_live_wave331_ok
        // Wave 332 residual honesty (wave guide dual-world empty gates).
        && r.live_wave_guide_dual_world_empty_gate_method_names_wave332_ok
        && r.live_wave_guide_dual_world_empty_gate_nav_commands_wave332_ok
        && r.live_wave_guide_dual_world_empty_gate_live_wave332_ok
        // Wave 333 residual honesty (emp update dual-world empty gates).
        && r.live_emp_update_dual_world_empty_gate_method_names_wave333_ok
        && r.live_emp_update_dual_world_empty_gate_nav_commands_wave333_ok
        && r.live_emp_update_dual_world_empty_gate_live_wave333_ok
        // Wave 334 residual honesty (bunker buster dual-world empty gates).
        && r.live_bunker_buster_dual_world_empty_gate_method_names_wave334_ok
        && r.live_bunker_buster_dual_world_empty_gate_nav_commands_wave334_ok
        && r.live_bunker_buster_dual_world_empty_gate_live_wave334_ok
        // Wave 335 residual honesty (bridge scaffold dual-world empty gates).
        && r.live_bridge_scaffold_dual_world_empty_gate_method_names_wave335_ok
        && r.live_bridge_scaffold_dual_world_empty_gate_nav_commands_wave335_ok
        && r.live_bridge_scaffold_dual_world_empty_gate_live_wave335_ok
        // Wave 336 residual honesty (assisted targeting dual-world empty gates).
        && r.live_assisted_targeting_dual_world_empty_gate_method_names_wave336_ok
        && r.live_assisted_targeting_dual_world_empty_gate_nav_commands_wave336_ok
        && r.live_assisted_targeting_dual_world_empty_gate_live_wave336_ok
        // Wave 337 residual honesty (economy dual-world empty gates).
        && r.live_economy_dual_world_empty_gate_method_names_wave337_ok
        && r.live_economy_dual_world_empty_gate_nav_commands_wave337_ok
        && r.live_economy_dual_world_empty_gate_live_wave337_ok
        // Wave 338 residual honesty (turret ai dual-world empty gates).
        && r.live_turret_ai_dual_world_empty_gate_method_names_wave338_ok
        && r.live_turret_ai_dual_world_empty_gate_nav_commands_wave338_ok
        && r.live_turret_ai_dual_world_empty_gate_live_wave338_ok
        // Wave 339 residual honesty (stealth detector module dual-world empty gates).
        && r.live_stealth_detector_module_dual_world_empty_gate_method_names_wave339_ok
        && r.live_stealth_detector_module_dual_world_empty_gate_nav_commands_wave339_ok
        && r.live_stealth_detector_module_dual_world_empty_gate_live_wave339_ok
        // Wave 340 residual honesty (modules dual-world empty gates).
        && r.live_modules_dual_world_empty_gate_method_names_wave340_ok
        && r.live_modules_dual_world_empty_gate_nav_commands_wave340_ok
        && r.live_modules_dual_world_empty_gate_live_wave340_ok
        // Wave 341 residual honesty (terrain dual-world empty gates).
        && r.live_terrain_dual_world_empty_gate_method_names_wave341_ok
        && r.live_terrain_dual_world_empty_gate_nav_commands_wave341_ok
        && r.live_terrain_dual_world_empty_gate_live_wave341_ok
        // Wave 342 residual honesty (special power template dual-world empty gates).
        && r.live_special_power_template_dual_world_empty_gate_method_names_wave342_ok
        && r.live_special_power_template_dual_world_empty_gate_nav_commands_wave342_ok
        && r.live_special_power_template_dual_world_empty_gate_live_wave342_ok
        // Wave 343 residual honesty (script evaluator dual-world empty gates).
        && r.live_script_evaluator_dual_world_empty_gate_method_names_wave343_ok
        && r.live_script_evaluator_dual_world_empty_gate_nav_commands_wave343_ok
        && r.live_script_evaluator_dual_world_empty_gate_live_wave343_ok
        // Wave 344 residual honesty (system game logic dual-world empty gates).
        && r.live_system_game_logic_dual_world_empty_gate_method_names_wave344_ok
        && r.live_system_game_logic_dual_world_empty_gate_nav_commands_wave344_ok
        && r.live_system_game_logic_dual_world_empty_gate_live_wave344_ok
        // Wave 345 residual honesty (meta event dual-world empty gates).
        && r.live_meta_event_dual_world_empty_gate_method_names_wave345_ok
        && r.live_meta_event_dual_world_empty_gate_nav_commands_wave345_ok
        && r.live_meta_event_dual_world_empty_gate_live_wave345_ok
        // Wave 346 residual honesty (spawn behavior dual-world empty gates).
        && r.live_spawn_behavior_dual_world_empty_gate_method_names_wave346_ok
        && r.live_spawn_behavior_dual_world_empty_gate_nav_commands_wave346_ok
        && r.live_spawn_behavior_dual_world_empty_gate_live_wave346_ok
        // Wave 347 residual honesty (action manager dual-world empty gates).
        && r.live_action_manager_dual_world_empty_gate_method_names_wave347_ok
        && r.live_action_manager_dual_world_empty_gate_nav_commands_wave347_ok
        && r.live_action_manager_dual_world_empty_gate_live_wave347_ok
        // Wave 348 residual honesty (script engine dual-world empty gates).
        && r.live_script_engine_dual_world_empty_gate_method_names_wave348_ok
        && r.live_script_engine_dual_world_empty_gate_nav_commands_wave348_ok
        && r.live_script_engine_dual_world_empty_gate_live_wave348_ok
        // Wave 349 residual honesty (chinook ai dual-world empty gates).
        && r.live_chinook_ai_dual_world_empty_gate_method_names_wave349_ok
        && r.live_chinook_ai_dual_world_empty_gate_nav_commands_wave349_ok
        && r.live_chinook_ai_dual_world_empty_gate_live_wave349_ok
        // Wave 350 residual honesty (missile ai dual-world empty gates).
        && r.live_missile_ai_dual_world_empty_gate_method_names_wave350_ok
        && r.live_missile_ai_dual_world_empty_gate_nav_commands_wave350_ok
        && r.live_missile_ai_dual_world_empty_gate_live_wave350_ok
        // Wave 351 residual honesty (dozer ai dual-world empty gates).
        && r.live_dozer_ai_dual_world_empty_gate_method_names_wave351_ok
        && r.live_dozer_ai_dual_world_empty_gate_nav_commands_wave351_ok
        && r.live_dozer_ai_dual_world_empty_gate_live_wave351_ok
        // Wave 352 residual honesty (deliver payload ai dual-world empty gates).
        && r.live_deliver_payload_ai_dual_world_empty_gate_method_names_wave352_ok
        && r.live_deliver_payload_ai_dual_world_empty_gate_nav_commands_wave352_ok
        && r.live_deliver_payload_ai_dual_world_empty_gate_live_wave352_ok
        // Wave 353 residual honesty (special power module dual-world empty gates).
        && r.live_special_power_module_dual_world_empty_gate_method_names_wave353_ok
        && r.live_special_power_module_dual_world_empty_gate_nav_commands_wave353_ok
        && r.live_special_power_module_dual_world_empty_gate_live_wave353_ok
        // Wave 354 residual honesty (pow truck ai dual-world empty gates).
        && r.live_pow_truck_ai_dual_world_empty_gate_method_names_wave354_ok
        && r.live_pow_truck_ai_dual_world_empty_gate_nav_commands_wave354_ok
        && r.live_pow_truck_ai_dual_world_empty_gate_live_wave354_ok
        // Wave 355 residual honesty (dock update dual-world empty gates).
        && r.live_dock_update_dual_world_empty_gate_method_names_wave355_ok
        && r.live_dock_update_dual_world_empty_gate_nav_commands_wave355_ok
        && r.live_dock_update_dual_world_empty_gate_live_wave355_ok
        // Wave 356 residual honesty (weapon template dual-world empty gates).
        && r.live_weapon_template_dual_world_empty_gate_method_names_wave356_ok
        && r.live_weapon_template_dual_world_empty_gate_nav_commands_wave356_ok
        && r.live_weapon_template_dual_world_empty_gate_live_wave356_ok;
    if pass {
        println!(
            "shell_smoke_gate: PASS (playable_claim={} shell_host_playable_ok={} control_bar={} cb_valid={} cb_loaded={} cb_windows={} dual_tick={} hud_sel={} sel_consumers={} minimap_fow={} laser_upload={} mesh={} sp72={} sp73={} sp76={} paradrop76={} cb76={} gfx76={} spectre_decal={} sp77={} fow77={} gh77={} weapon77={} ai77={} sp78={} cluster78={} gps78={} cash78={} minimap79={} sel79={} input79={} draw79={} train79={} upg79={} cmdbtn80={} rank80={} kindof80={} spenum80={} height81={} path81={} loco81={} armor81={} puc81={} dmg82={} death82={} mc82={} wbonus82={} ostatus82={} prod83={} supply83={} dozer83={} capture83={} power83={} cc83={} kindof84={} wslot84={} vet84={} rel84={} geom84={} shadow84={} faction85={} ptpl85={} cash85={} aiperson85={} victory85={} cam86={} world86={} mpopt86={} mapsel86={} crate86={} weather87={} water87={} bridge87={} tunnel87={} garrison87={} transport87={} radius88={} mouse88={} fxlist88={} ocl88={} particle88={} audio88={} rank89={} exp89={} hotkey89={} chat89={} replay89={} options89={} gamespeed90={} framerate90={} debug90={} lang90={} credits90={} tooltip91={} helpbox91={} message91={} eva91={} video91={} briefing91={} weapon92={} armor92={} body92={} loco92={} science92={} particle93={} drawable93={} shadow93={} terrain_tex93={} road93={} ai_state94={} special_ability94={} upgrade_names94={} command_set94={} script_action95={} script_cond95={} map_object95={} waypoint95={} team95={} player95={} partition96={} collision96={} physics96={} projectile96={} radar97={} spotter97={} stealth97={} detector97={} vision97={} dock98={} contain98={} exit98={} heal98={} production99={} buildable99={} prereq99={} cmdbtn99={} controlbar99={} thing_factory100={} module_type100={} xfer100={} tf_crosslink100={} module_factory101={} thing_factory101={} partition_register101={} mf_crosslink101={} display102={} anim2d102={} laser102={} csf102={} pres102={} weapon103={} armor103={} loco103={} sp103={} kindof103={} object_status104={} object_create104={} active_body104={} drawable_create104={} register_object104={} ai_group105={} ai_path105={} weapon_fire105={} damage_app105={} veterancy105={} gamestate106={} campaign106={} mainmenu106={} gamewindow106={} layout106={} particle107={} fxlist107={} ocl107={} audio107={} heightmap108={} bridge108={} water108={} road108={} cliff108={} sp_store109={} science_store109={} upgrade_store109={} player109={} team109={} screen={} map_loaded={})",
            r.playable_claim,
            r.shell_host_playable_ok,
            r.control_bar_layout_ok,
            r.control_bar_wnd_validated,
            r.control_bar_window_loaded,
            r.control_bar_window_count,
            r.dual_tick_presentation_ok,
            r.hud_selection_ok,
            r.selection_consumers_ok,
            r.minimap_fow_presentation_ok,
            r.laser_segment_upload_ok,
            r.mesh_asset_residual_ok,
            r.special_power_wave72_residual_ok,
            r.special_power_wave73_residual_ok,
            r.special_power_wave76_residual_ok,
            r.paradrop_wave76_residual_ok,
            r.control_bar_wave76_residual_ok,
            r.graphics_wave76_residual_ok,
            r.spectre_orbit_decal_presentation_ok,
            r.special_power_wave77_residual_ok,
            r.fow_residual_pack_ok,
            r.ground_height_presentation_ok,
            r.weapon_store_seed_residual_ok,
            r.ai_skirmish_residual_ok,
            r.special_power_wave78_residual_ok,
            r.cluster_mines_wave78_residual_ok,
            r.gps_scrambler_wave78_residual_ok,
            r.cash_bounty_wave78_residual_ok,
            r.minimap_residual_pack_ok,
            r.selection_hud_residual_pack_ok,
            r.input_residual_pack_ok,
            r.drawable_residual_fields_ok,
            r.unit_training_wave79_residual_ok,
            r.upgrades_cost_time_application_ok,
            r.command_button_wave80_residual_ok,
            r.science_rank_wave80_residual_ok,
            r.superweapon_kindof_wave80_residual_ok,
            r.special_power_enum_wave80_residual_ok,
            r.terrain_height_sample_wave81_ok,
            r.pathfinder_wave81_residual_ok,
            r.locomotor_table_wave81_ok,
            r.armor_table_wave81_ok,
            r.puc_flare_table_wave81_ok,
            r.damage_type_wave82_ok,
            r.death_type_wave82_ok,
            r.model_condition_wave82_ok,
            r.weapon_bonus_wave82_ok,
            r.object_status_wave82_ok,
            r.production_queue_wave83_ok,
            r.supply_warehouse_wave83_ok,
            r.dozer_build_wave83_ok,
            r.capture_building_wave83_ok,
            r.power_plant_wave83_ok,
            r.command_center_wave83_ok,
            r.kindof_wave84_ok,
            r.weapon_slot_wave84_ok,
            r.veterancy_wave84_ok,
            r.relationship_wave84_ok,
            r.geometry_wave84_ok,
            r.shadow_wave84_ok,
            r.faction_side_wave85_ok,
            r.player_template_wave85_ok,
            r.starting_cash_wave85_ok,
            r.skirmish_ai_personality_wave85_ok,
            r.victory_condition_wave85_ok,
            r.gamedata_camera_fps_wave86_ok,
            r.gamedata_world_constants_wave86_ok,
            r.multiplayer_options_wave86_ok,
            r.map_selection_wave86_ok,
            r.crate_deepen_wave86_ok,
            r.weather_wave87_ok,
            r.water_wave87_ok,
            r.bridge_wave87_ok,
            r.tunnel_wave87_ok,
            r.garrison_wave87_ok,
            r.transport_wave87_ok,
            r.radius_cursor_wave88_ok,
            r.mouse_cursor_wave88_ok,
            r.superweapon_fxlist_wave88_ok,
            r.superweapon_ocl_wave88_ok,
            r.superweapon_particle_wave88_ok,
            r.superweapon_audio_wave88_ok,
            r.rank_skill_wave89_ok,
            r.experience_wave89_ok,
            r.hotkey_wave89_ok,
            r.chat_wave89_ok,
            r.replay_wave89_ok,
            r.options_wave89_ok,
            r.gamespeed_wave90_ok,
            r.frame_rate_wave90_ok,
            r.debug_tables_wave90_ok,
            r.language_wave90_ok,
            r.credits_wave90_ok,
            r.tooltip_wave91_ok,
            r.help_box_wave91_ok,
            r.message_wave91_ok,
            r.eva_wave91_ok,
            r.video_wave91_ok,
            r.mission_briefing_wave91_ok,
            r.weapon_deepen_wave92_ok,
            r.armor_expand_wave92_ok,
            r.body_health_wave92_ok,
            r.locomotor_expand_wave92_ok,
            r.science_names_wave92_ok,
            r.particle_emit_wave93_ok,
            r.drawable_opacity_wave93_ok,
            r.shadow_deepen_wave93_ok,
            r.terrain_texture_wave93_ok,
            r.road_wave93_ok,
            r.ai_state_wave94_ok,
            r.special_ability_wave94_ok,
            r.upgrade_names_wave94_ok,
            r.command_set_wave94_ok,
            r.script_action_wave95_ok,
            r.script_condition_wave95_ok,
            r.map_object_wave95_ok,
            r.waypoint_wave95_ok,
            r.team_wave95_ok,
            r.player_deepen_wave95_ok,
            r.partition_wave96_ok,
            r.collision_wave96_ok,
            r.physics_wave96_ok,
            r.projectile_wave96_ok,
            r.radar_deepen_wave97_ok,
            r.spotter_wave97_ok,
            r.stealth_deepen_wave97_ok,
            r.detector_deepen_wave97_ok,
            r.vision_wave97_ok,
            r.dock_wave98_ok,
            r.contain_wave98_ok,
            r.exit_wave98_ok,
            r.heal_wave98_ok,
            r.production_deepen_wave99_ok,
            r.buildable_wave99_ok,
            r.prerequisite_wave99_ok,
            r.command_button_deepen_wave99_ok,
            r.control_bar_deepen_wave99_ok,
            r.thing_factory_deepen_wave100_ok,
            r.module_type_wave100_ok,
            r.xfer_deepen_wave100_ok,
            r.thing_factory_crosslink_wave100_ok,
            r.module_factory_deepen_wave101_ok,
            r.thing_factory_create_wave101_ok,
            r.partition_register_wave101_ok,
            r.mf_crosslink_wave101_ok,
            r.display_string_deepen_wave102_ok,
            r.anim2d_deepen_wave102_ok,
            r.laser_segliner_deepen_wave102_ok,
            r.csf_multi_locale_deepen_wave102_ok,
            r.presentation_deepen_wave102_ok,
            r.weapon_deepen_wave103_ok,
            r.armor_expand_wave103_ok,
            r.locomotor_expand_wave103_ok,
            r.special_power_deepen_wave103_ok,
            r.object_kindof_wave103_ok,
            r.object_status_wave104_ok,
            r.object_create_wave104_ok,
            r.active_body_wave104_ok,
            r.drawable_create_wave104_ok,
            r.register_object_wave104_ok,
            r.ai_group_wave105_ok,
            r.ai_path_wave105_ok,
            r.weapon_fire_wave105_ok,
            r.damage_application_wave105_ok,
            r.veterancy_wave105_ok,
            r.game_state_deepen_wave106_ok,
            r.campaign_mission_wave106_ok,
            r.main_menu_deepen_wave106_ok,
            r.game_window_deepen_wave106_ok,
            r.window_layout_deepen_wave106_ok,
            r.particle_system_deepen_wave107_ok,
            r.fxlist_entry_deepen_wave107_ok,
            r.ocl_create_deepen_wave107_ok,
            r.audio_deepen_wave107_ok,
            r.heightmap_deepen_wave108_ok,
            r.bridge_deepen_wave108_ok,
            r.water_deepen_wave108_ok,
            r.road_deepen_wave108_ok,
            r.cliff_peels_wave108_ok,
            r.special_power_store_wave109_ok,
            r.science_store_wave109_ok,
            r.upgrade_store_wave109_ok,
            r.player_deepen_wave109_ok,
            r.team_deepen_wave109_ok,
            r.screen_skirmish_ok,
            r.map_loaded
        );
        std::process::exit(0);
    }
    eprintln!(
        "shell_smoke_gate: FAIL status={} playable_claim={} shell_host_playable_ok={} control_bar={} cb_loaded={} dual_tick={} sel_consumers={} laser={} {}",
        r.status,
        r.playable_claim,
        r.shell_host_playable_ok,
        r.control_bar_layout_ok,
        r.control_bar_window_loaded,
        r.dual_tick_presentation_ok,
        r.selection_consumers_ok,
        r.laser_segment_upload_ok,
        r.detail
    );
    std::process::exit(1);
}
