// AUTO-GENERATED source-scan dump. Not compiled.
// Live test lives in host_smoke/ via #[path] in tests/mod.rs.

//! Host smoke residual assertions (full ShellSmokeResult pack).

pub use super::*;

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
            r.live_lifetime_update_dual_world_empty_gate_method_names_wave412_ok,
            "live lifetime update dual-world empty gate method names residual pack wave412: {}",
            r.detail
        );
        assert!(
            r.live_lifetime_update_dual_world_empty_gate_nav_commands_wave412_ok,
            "live lifetime update dual-world empty gate nav commands residual pack wave412: {}",
            r.detail
        );
        assert!(
            r.live_lifetime_update_dual_world_empty_gate_live_wave412_ok,
            "live lifetime update dual-world empty gate live residual wave412: {}",
            r.detail
        );
        assert!(
            r.live_slow_death_behavior_dual_world_empty_gate_method_names_wave413_ok,
            "live slow death behavior dual-world empty gate method names residual pack wave413: {}",
            r.detail
        );
        assert!(
            r.live_slow_death_behavior_dual_world_empty_gate_nav_commands_wave413_ok,
            "live slow death behavior dual-world empty gate nav commands residual pack wave413: {}",
            r.detail
        );
        assert!(
            r.live_slow_death_behavior_dual_world_empty_gate_live_wave413_ok,
            "live slow death behavior dual-world empty gate live residual wave413: {}",
            r.detail
        );
        assert!(
            r.live_battle_bus_slow_death_behavior_dual_world_empty_gate_method_names_wave414_ok,
            "live battle bus slow death behavior dual-world empty gate method names residual pack wave414: {}",
            r.detail
        );
        assert!(
            r.live_battle_bus_slow_death_behavior_dual_world_empty_gate_nav_commands_wave414_ok,
            "live battle bus slow death behavior dual-world empty gate nav commands residual pack wave414: {}",
            r.detail
        );
        assert!(
            r.live_battle_bus_slow_death_behavior_dual_world_empty_gate_live_wave414_ok,
            "live battle bus slow death behavior dual-world empty gate live residual wave414: {}",
            r.detail
        );
        assert!(
            r.live_damage_module_dual_world_empty_gate_method_names_wave415_ok,
            "live damage module dual-world empty gate method names residual pack wave415: {}",
            r.detail
        );
        assert!(
            r.live_damage_module_dual_world_empty_gate_nav_commands_wave415_ok,
            "live damage module dual-world empty gate nav commands residual pack wave415: {}",
            r.detail
        );
        assert!(
            r.live_damage_module_dual_world_empty_gate_live_wave415_ok,
            "live damage module dual-world empty gate live residual wave415: {}",
            r.detail
        );
        assert!(
            r.live_transition_damage_fx_dual_world_empty_gate_method_names_wave416_ok,
            "live transition damage fx dual-world empty gate method names residual pack wave416: {}",
            r.detail
        );
        assert!(
            r.live_transition_damage_fx_dual_world_empty_gate_nav_commands_wave416_ok,
            "live transition damage fx dual-world empty gate nav commands residual pack wave416: {}",
            r.detail
        );
        assert!(
            r.live_transition_damage_fx_dual_world_empty_gate_live_wave416_ok,
            "live transition damage fx dual-world empty gate live residual wave416: {}",
            r.detail
        );
        assert!(
            r.live_spawn_point_production_exit_behavior_dual_world_empty_gate_method_names_wave417_ok,
            "live spawn point production exit behavior dual-world empty gate method names residual pack wave417: {}",
            r.detail
        );
        assert!(
            r.live_spawn_point_production_exit_behavior_dual_world_empty_gate_nav_commands_wave417_ok,
            "live spawn point production exit behavior dual-world empty gate nav commands residual pack wave417: {}",
            r.detail
        );
        assert!(
            r.live_spawn_point_production_exit_behavior_dual_world_empty_gate_live_wave417_ok,
            "live spawn point production exit behavior dual-world empty gate live residual wave417: {}",
            r.detail
        );
        assert!(
            r.live_build_placement_dual_world_empty_gate_method_names_wave418_ok,
            "live build placement dual-world empty gate method names residual pack wave418: {}",
            r.detail
        );
        assert!(
            r.live_build_placement_dual_world_empty_gate_nav_commands_wave418_ok,
            "live build placement dual-world empty gate nav commands residual pack wave418: {}",
            r.detail
        );
        assert!(
            r.live_build_placement_dual_world_empty_gate_live_wave418_ok,
            "live build placement dual-world empty gate live residual wave418: {}",
            r.detail
        );
        assert!(
            r.live_weapon_set_dual_world_empty_gate_method_names_wave419_ok,
            "live weapon set dual-world empty gate method names residual pack wave419: {}",
            r.detail
        );
        assert!(
            r.live_weapon_set_dual_world_empty_gate_nav_commands_wave419_ok,
            "live weapon set dual-world empty gate nav commands residual pack wave419: {}",
            r.detail
        );
        assert!(
            r.live_weapon_set_dual_world_empty_gate_live_wave419_ok,
            "live weapon set dual-world empty gate live residual wave419: {}",
            r.detail
        );
        assert!(
            r.live_experience_tracker_dual_world_empty_gate_method_names_wave420_ok,
            "live experience tracker dual-world empty gate method names residual pack wave420: {}",
            r.detail
        );
        assert!(
            r.live_experience_tracker_dual_world_empty_gate_nav_commands_wave420_ok,
            "live experience tracker dual-world empty gate nav commands residual pack wave420: {}",
            r.detail
        );
        assert!(
            r.live_experience_tracker_dual_world_empty_gate_live_wave420_ok,
            "live experience tracker dual-world empty gate live residual wave420: {}",
            r.detail
        );
        assert!(
            r.live_ai_targeting_dual_world_empty_gate_method_names_wave421_ok,
            "live ai targeting dual-world empty gate method names residual pack wave421: {}",
            r.detail
        );
        assert!(
            r.live_ai_targeting_dual_world_empty_gate_nav_commands_wave421_ok,
            "live ai targeting dual-world empty gate nav commands residual pack wave421: {}",
            r.detail
        );
        assert!(
            r.live_ai_targeting_dual_world_empty_gate_live_wave421_ok,
            "live ai targeting dual-world empty gate live residual wave421: {}",
            r.detail
        );
        assert!(
            r.live_move_to_state_dual_world_empty_gate_method_names_wave422_ok,
            "live move to state dual-world empty gate method names residual pack wave422: {}",
            r.detail
        );
        assert!(
            r.live_move_to_state_dual_world_empty_gate_nav_commands_wave422_ok,
            "live move to state dual-world empty gate nav commands residual pack wave422: {}",
            r.detail
        );
        assert!(
            r.live_move_to_state_dual_world_empty_gate_live_wave422_ok,
            "live move to state dual-world empty gate live residual wave422: {}",
            r.detail
        );
        assert!(
            r.live_locomotor_core_dual_world_empty_gate_method_names_wave423_ok,
            "live locomotor core dual-world empty gate method names residual pack wave423: {}",
            r.detail
        );
        assert!(
            r.live_locomotor_core_dual_world_empty_gate_nav_commands_wave423_ok,
            "live locomotor core dual-world empty gate nav commands residual pack wave423: {}",
            r.detail
        );
        assert!(
            r.live_locomotor_core_dual_world_empty_gate_live_wave423_ok,
            "live locomotor core dual-world empty gate live residual wave423: {}",
            r.detail
        );
        assert!(
            r.live_path_following_dual_world_empty_gate_method_names_wave424_ok,
            "live path following dual-world empty gate method names residual pack wave424: {}",
            r.detail
        );
        assert!(
            r.live_path_following_dual_world_empty_gate_nav_commands_wave424_ok,
            "live path following dual-world empty gate nav commands residual pack wave424: {}",
            r.detail
        );
        assert!(
            r.live_path_following_dual_world_empty_gate_live_wave424_ok,
            "live path following dual-world empty gate live residual wave424: {}",
            r.detail
        );
        assert!(
            r.live_ai_manager_dual_world_empty_gate_method_names_wave425_ok,
            "live ai manager dual-world empty gate method names residual pack wave425: {}",
            r.detail
        );
        assert!(
            r.live_ai_manager_dual_world_empty_gate_nav_commands_wave425_ok,
            "live ai manager dual-world empty gate nav commands residual pack wave425: {}",
            r.detail
        );
        assert!(
            r.live_ai_manager_dual_world_empty_gate_live_wave425_ok,
            "live ai manager dual-world empty gate live residual wave425: {}",
            r.detail
        );
        assert!(
            r.live_pathfind_dual_world_empty_gate_method_names_wave426_ok,
            "live pathfind dual-world empty gate method names residual pack wave426: {}",
            r.detail
        );
        assert!(
            r.live_pathfind_dual_world_empty_gate_nav_commands_wave426_ok,
            "live pathfind dual-world empty gate nav commands residual pack wave426: {}",
            r.detail
        );
        assert!(
            r.live_pathfind_dual_world_empty_gate_live_wave426_ok,
            "live pathfind dual-world empty gate live residual wave426: {}",
            r.detail
        );
        assert!(
            r.live_fire_weapon_when_dead_behavior_dual_world_empty_gate_method_names_wave427_ok,
            "live fire weapon when dead behavior dual-world empty gate method names residual pack wave427: {}",
            r.detail
        );
        assert!(
            r.live_fire_weapon_when_dead_behavior_dual_world_empty_gate_nav_commands_wave427_ok,
            "live fire weapon when dead behavior dual-world empty gate nav commands residual pack wave427: {}",
            r.detail
        );
        assert!(
            r.live_fire_weapon_when_dead_behavior_dual_world_empty_gate_live_wave427_ok,
            "live fire weapon when dead behavior dual-world empty gate live residual wave427: {}",
            r.detail
        );
        assert!(
            r.live_guard_dual_world_empty_gate_method_names_wave428_ok,
            "live guard dual-world empty gate method names residual pack wave428: {}",
            r.detail
        );
        assert!(
            r.live_guard_dual_world_empty_gate_nav_commands_wave428_ok,
            "live guard dual-world empty gate nav commands residual pack wave428: {}",
            r.detail
        );
        assert!(
            r.live_guard_dual_world_empty_gate_live_wave428_ok,
            "live guard dual-world empty gate live residual wave428: {}",
            r.detail
        );
        assert!(
            r.live_guard_retaliate_dual_world_empty_gate_method_names_wave429_ok,
            "live guard retaliate dual-world empty gate method names residual pack wave429: {}",
            r.detail
        );
        assert!(
            r.live_guard_retaliate_dual_world_empty_gate_nav_commands_wave429_ok,
            "live guard retaliate dual-world empty gate nav commands residual pack wave429: {}",
            r.detail
        );
        assert!(
            r.live_guard_retaliate_dual_world_empty_gate_live_wave429_ok,
            "live guard retaliate dual-world empty gate live residual wave429: {}",
            r.detail
        );
        assert!(
            r.live_wander_ai_dual_world_empty_gate_method_names_wave430_ok,
            "live wander ai dual-world empty gate method names residual pack wave430: {}",
            r.detail
        );
        assert!(
            r.live_wander_ai_dual_world_empty_gate_nav_commands_wave430_ok,
            "live wander ai dual-world empty gate nav commands residual pack wave430: {}",
            r.detail
        );
        assert!(
            r.live_wander_ai_dual_world_empty_gate_live_wave430_ok,
            "live wander ai dual-world empty gate live residual wave430: {}",
            r.detail
        );
        assert!(
            r.live_subobjects_upgrade_dual_world_empty_gate_method_names_wave431_ok,
            "live subobjects upgrade dual-world empty gate method names residual pack wave431: {}",
            r.detail
        );
        assert!(
            r.live_subobjects_upgrade_dual_world_empty_gate_nav_commands_wave431_ok,
            "live subobjects upgrade dual-world empty gate nav commands residual pack wave431: {}",
            r.detail
        );
        assert!(
            r.live_subobjects_upgrade_dual_world_empty_gate_live_wave431_ok,
            "live subobjects upgrade dual-world empty gate live residual wave431: {}",
            r.detail
        );
        assert!(
            r.live_unit_exit_dual_world_empty_gate_method_names_wave432_ok,
            "live unit exit dual-world empty gate method names residual pack wave432: {}",
            r.detail
        );
        assert!(
            r.live_unit_exit_dual_world_empty_gate_nav_commands_wave432_ok,
            "live unit exit dual-world empty gate nav commands residual pack wave432: {}",
            r.detail
        );
        assert!(
            r.live_unit_exit_dual_world_empty_gate_live_wave432_ok,
            "live unit exit dual-world empty gate live residual wave432: {}",
            r.detail
        );
        assert!(
            r.live_owner_resolve_dual_world_empty_gate_method_names_wave433_ok,
            "live owner resolve dual-world empty gate method names residual pack wave433: {}",
            r.detail
        );
        assert!(
            r.live_owner_resolve_dual_world_empty_gate_nav_commands_wave433_ok,
            "live owner resolve dual-world empty gate nav commands residual pack wave433: {}",
            r.detail
        );
        assert!(
            r.live_owner_resolve_dual_world_empty_gate_live_wave433_ok,
            "live owner resolve dual-world empty gate live residual wave433: {}",
            r.detail
        );
        assert!(
            r.live_spy_vision_update_dual_world_empty_gate_method_names_wave434_ok,
            "live spy vision update dual-world empty gate method names residual pack wave434: {}",
            r.detail
        );
        assert!(
            r.live_spy_vision_update_dual_world_empty_gate_nav_commands_wave434_ok,
            "live spy vision update dual-world empty gate nav commands residual pack wave434: {}",
            r.detail
        );
        assert!(
            r.live_spy_vision_update_dual_world_empty_gate_live_wave434_ok,
            "live spy vision update dual-world empty gate live residual wave434: {}",
            r.detail
        );
        assert!(
            r.live_overcharge_behavior_dual_world_empty_gate_method_names_wave435_ok,
            "live overcharge behavior dual-world empty gate method names residual pack wave435: {}",
            r.detail
        );
        assert!(
            r.live_overcharge_behavior_dual_world_empty_gate_nav_commands_wave435_ok,
            "live overcharge behavior dual-world empty gate nav commands residual pack wave435: {}",
            r.detail
        );
        assert!(
            r.live_overcharge_behavior_dual_world_empty_gate_live_wave435_ok,
            "live overcharge behavior dual-world empty gate live residual wave435: {}",
            r.detail
        );
        assert!(
            r.live_tech_building_behavior_dual_world_empty_gate_method_names_wave436_ok,
            "live tech building behavior dual-world empty gate method names residual pack wave436: {}",
            r.detail
        );
        assert!(
            r.live_tech_building_behavior_dual_world_empty_gate_nav_commands_wave436_ok,
            "live tech building behavior dual-world empty gate nav commands residual pack wave436: {}",
            r.detail
        );
        assert!(
            r.live_tech_building_behavior_dual_world_empty_gate_live_wave436_ok,
            "live tech building behavior dual-world empty gate live residual wave436: {}",
            r.detail
        );
        assert!(
            r.live_power_plant_upgrade_dual_world_empty_gate_method_names_wave437_ok,
            "live power plant upgrade dual-world empty gate method names residual pack wave437: {}",
            r.detail
        );
        assert!(
            r.live_power_plant_upgrade_dual_world_empty_gate_nav_commands_wave437_ok,
            "live power plant upgrade dual-world empty gate nav commands residual pack wave437: {}",
            r.detail
        );
        assert!(
            r.live_power_plant_upgrade_dual_world_empty_gate_live_wave437_ok,
            "live power plant upgrade dual-world empty gate live residual wave437: {}",
            r.detail
        );
        assert!(
            r.live_stealth_upgrade_dual_world_empty_gate_method_names_wave438_ok,
            "live stealth upgrade dual-world empty gate method names residual pack wave438: {}",
            r.detail
        );
        assert!(
            r.live_stealth_upgrade_dual_world_empty_gate_nav_commands_wave438_ok,
            "live stealth upgrade dual-world empty gate nav commands residual pack wave438: {}",
            r.detail
        );
        assert!(
            r.live_stealth_upgrade_dual_world_empty_gate_live_wave438_ok,
            "live stealth upgrade dual-world empty gate live residual wave438: {}",
            r.detail
        );
        assert!(
            r.live_aurora_strike_power_dual_world_empty_gate_method_names_wave439_ok,
            "live aurora strike power dual-world empty gate method names residual pack wave439: {}",
            r.detail
        );
        assert!(
            r.live_aurora_strike_power_dual_world_empty_gate_nav_commands_wave439_ok,
            "live aurora strike power dual-world empty gate nav commands residual pack wave439: {}",
            r.detail
        );
        assert!(
            r.live_aurora_strike_power_dual_world_empty_gate_live_wave439_ok,
            "live aurora strike power dual-world empty gate live residual wave439: {}",
            r.detail
        );
        assert!(
            r.live_carpet_bomb_power_dual_world_empty_gate_method_names_wave440_ok,
            "live carpet bomb power dual-world empty gate method names residual pack wave440: {}",
            r.detail
        );
        assert!(
            r.live_carpet_bomb_power_dual_world_empty_gate_nav_commands_wave440_ok,
            "live carpet bomb power dual-world empty gate nav commands residual pack wave440: {}",
            r.detail
        );
        assert!(
            r.live_carpet_bomb_power_dual_world_empty_gate_live_wave440_ok,
            "live carpet bomb power dual-world empty gate live residual wave440: {}",
            r.detail
        );
        assert!(
            r.live_nuclear_missile_power_dual_world_empty_gate_method_names_wave441_ok,
            "live nuclear missile power dual-world empty gate method names residual pack wave441: {}",
            r.detail
        );
        assert!(
            r.live_nuclear_missile_power_dual_world_empty_gate_nav_commands_wave441_ok,
            "live nuclear missile power dual-world empty gate nav commands residual pack wave441: {}",
            r.detail
        );
        assert!(
            r.live_nuclear_missile_power_dual_world_empty_gate_live_wave441_ok,
            "live nuclear missile power dual-world empty gate live residual wave441: {}",
            r.detail
        );
        assert!(
            r.live_overlord_draw_dual_world_empty_gate_method_names_wave442_ok,
            "live overlord draw dual-world empty gate method names residual pack wave442: {}",
            r.detail
        );
        assert!(
            r.live_overlord_draw_dual_world_empty_gate_nav_commands_wave442_ok,
            "live overlord draw dual-world empty gate nav commands residual pack wave442: {}",
            r.detail
        );
        assert!(
            r.live_overlord_draw_dual_world_empty_gate_live_wave442_ok,
            "live overlord draw dual-world empty gate live residual wave442: {}",
            r.detail
        );
        assert!(
            r.live_stealth_integration_dual_world_empty_gate_method_names_wave443_ok,
            "live stealth integration dual-world empty gate method names residual pack wave443: {}",
            r.detail
        );
        assert!(
            r.live_stealth_integration_dual_world_empty_gate_nav_commands_wave443_ok,
            "live stealth integration dual-world empty gate nav commands residual pack wave443: {}",
            r.detail
        );
        assert!(
            r.live_stealth_integration_dual_world_empty_gate_live_wave443_ok,
            "live stealth integration dual-world empty gate live residual wave443: {}",
            r.detail
        );
        assert!(
            r.live_player_upgrade_manager_dual_world_empty_gate_method_names_wave444_ok,
            "live player upgrade manager dual-world empty gate method names residual pack wave444: {}",
            r.detail
        );
        assert!(
            r.live_player_upgrade_manager_dual_world_empty_gate_nav_commands_wave444_ok,
            "live player upgrade manager dual-world empty gate nav commands residual pack wave444: {}",
            r.detail
        );
        assert!(
            r.live_player_upgrade_manager_dual_world_empty_gate_live_wave444_ok,
            "live player upgrade manager dual-world empty gate live residual wave444: {}",
            r.detail
        );
        assert!(
            r.live_advanced_nuggets_dual_world_empty_gate_method_names_wave445_ok,
            "live advanced nuggets dual-world empty gate method names residual pack wave445: {}",
            r.detail
        );
        assert!(
            r.live_advanced_nuggets_dual_world_empty_gate_nav_commands_wave445_ok,
            "live advanced nuggets dual-world empty gate nav commands residual pack wave445: {}",
            r.detail
        );
        assert!(
            r.live_advanced_nuggets_dual_world_empty_gate_live_wave445_ok,
            "live advanced nuggets dual-world empty gate live residual wave445: {}",
            r.detail
        );
        assert!(
            r.live_replace_object_upgrade_dual_world_empty_gate_method_names_wave446_ok,
            "live replace object upgrade dual-world empty gate method names residual pack wave446: {}",
            r.detail
        );
        assert!(
            r.live_replace_object_upgrade_dual_world_empty_gate_nav_commands_wave446_ok,
            "live replace object upgrade dual-world empty gate nav commands residual pack wave446: {}",
            r.detail
        );
        assert!(
            r.live_replace_object_upgrade_dual_world_empty_gate_live_wave446_ok,
            "live replace object upgrade dual-world empty gate live residual wave446: {}",
            r.detail
        );
        assert!(
            r.live_fire_spread_update_dual_world_empty_gate_method_names_wave447_ok,
            "live fire spread update dual-world empty gate method names residual pack wave447: {}",
            r.detail
        );
        assert!(
            r.live_fire_spread_update_dual_world_empty_gate_nav_commands_wave447_ok,
            "live fire spread update dual-world empty gate nav commands residual pack wave447: {}",
            r.detail
        );
        assert!(
            r.live_fire_spread_update_dual_world_empty_gate_live_wave447_ok,
            "live fire spread update dual-world empty gate live residual wave447: {}",
            r.detail
        );
        assert!(
            r.live_object_upgrade_batch_dual_world_empty_gate_method_names_wave448_ok,
            "live object upgrade batch dual-world empty gate method names residual pack wave448: {}",
            r.detail
        );
        assert!(
            r.live_object_upgrade_batch_dual_world_empty_gate_nav_commands_wave448_ok,
            "live object upgrade batch dual-world empty gate nav commands residual pack wave448: {}",
            r.detail
        );
        assert!(
            r.live_object_upgrade_batch_dual_world_empty_gate_live_wave448_ok,
            "live object upgrade batch dual-world empty gate live residual wave448: {}",
            r.detail
        );
        assert!(
            r.live_contain_module_overrides_fail_closed_method_names_wave449_ok,
            "live contain module overrides fail-closed method names residual pack wave449: {}",
            r.detail
        );
        assert!(
            r.live_contain_module_overrides_fail_closed_nav_commands_wave449_ok,
            "live contain module overrides fail-closed nav commands residual pack wave449: {}",
            r.detail
        );
        assert!(
            r.live_contain_module_overrides_fail_closed_live_wave449_ok,
            "live contain module overrides fail-closed live residual wave449: {}",
            r.detail
        );
        assert!(
            r.live_core_sim_dual_world_empty_gate_method_names_wave450_ok,
            "live core sim dual-world empty gate method names residual pack wave450: {}",
            r.detail
        );
        assert!(
            r.live_core_sim_dual_world_empty_gate_nav_commands_wave450_ok,
            "live core sim dual-world empty gate nav commands residual pack wave450: {}",
            r.detail
        );
        assert!(
            r.live_core_sim_dual_world_empty_gate_live_wave450_ok,
            "live core sim dual-world empty gate live residual wave450: {}",
            r.detail
        );
        assert!(
            r.live_golden_mopup_honesty_method_names_wave451_ok,
            "live golden mop-up honesty method names residual pack wave451: {}",
            r.detail
        );
        assert!(
            r.live_die_command_dual_world_empty_gate_method_names_wave452_ok,
            "live die command dual-world empty gate method names residual pack wave452: {}",
            r.detail
        );
        assert!(
            r.live_die_command_dual_world_empty_gate_nav_commands_wave452_ok,
            "live die command dual-world empty gate nav commands residual pack wave452: {}",
            r.detail
        );
        assert!(
            r.live_die_command_dual_world_empty_gate_live_wave452_ok,
            "live die command dual-world empty gate live residual wave452: {}",
            r.detail
        );
        assert!(
            r.live_die_command_dual_world_empty_gate_live_wave452_ok,
            "live die command dual-world empty gate live residual wave452: {}",
            r.detail
        );
        assert!(
            r.live_upgrade_behavior_dual_world_empty_gate_method_names_wave453_ok,
            "live upgrade behavior dual-world empty gate method names residual pack wave453: {}",
            r.detail
        );
        assert!(
            r.live_upgrade_behavior_dual_world_empty_gate_nav_commands_wave453_ok,
            "live upgrade behavior dual-world empty gate nav commands residual pack wave453: {}",
            r.detail
        );
        assert!(
            r.live_upgrade_behavior_dual_world_empty_gate_live_wave453_ok,
            "live upgrade behavior dual-world empty gate live residual wave453: {}",
            r.detail
        );
        assert!(
            r.live_construction_placement_dual_world_empty_gate_method_names_wave454_ok,
            "live construction placement dual-world empty gate method names residual pack wave454: {}",
            r.detail
        );
        assert!(
            r.live_construction_placement_dual_world_empty_gate_nav_commands_wave454_ok,
            "live construction placement dual-world empty gate nav commands residual pack wave454: {}",
            r.detail
        );
        assert!(
            r.live_construction_placement_dual_world_empty_gate_live_wave454_ok,
            "live construction placement dual-world empty gate live residual wave454: {}",
            r.detail
        );
        assert!(
            r.live_presentation_env_only_method_names_wave455_ok,
            "live presentation env-only method names residual pack wave455: {}",
            r.detail
        );
        assert!(
            r.live_presentation_env_only_nav_commands_wave455_ok,
            "live presentation env-only nav commands residual pack wave455: {}",
            r.detail
        );
        assert!(
            r.live_presentation_env_only_live_wave455_ok,
            "live presentation env-only live residual wave455: {}",
            r.detail
        );
        assert!(
            r.map_lighting_presentation_only_method_names_wave456_ok,
            "map lighting presentation only method names residual pack wave456: {}",
            r.detail
        );
        assert!(
            r.map_lighting_presentation_only_nav_commands_wave456_ok,
            "map lighting presentation only nav commands residual pack wave456: {}",
            r.detail
        );
        assert!(
            r.map_lighting_presentation_only_live_wave456_ok,
            "map lighting presentation only live residual wave456: {}",
            r.detail
        );
        assert!(
            r.minimap_bounds_presentation_first_method_names_wave457_ok,
            "minimap bounds presentation first method names residual pack wave457: {}",
            r.detail
        );
        assert!(
            r.minimap_bounds_presentation_first_nav_commands_wave457_ok,
            "minimap bounds presentation first nav commands residual pack wave457: {}",
            r.detail
        );
        assert!(
            r.minimap_bounds_presentation_first_live_wave457_ok,
            "minimap bounds presentation first live residual wave457: {}",
            r.detail
        );
        assert!(
            r.bootstrap_camera_no_live_dual_read_method_names_wave458_ok,
            "bootstrap camera no live dual read method names residual pack wave458: {}",
            r.detail
        );
        assert!(
            r.bootstrap_camera_no_live_dual_read_nav_commands_wave458_ok,
            "bootstrap camera no live dual read nav commands residual pack wave458: {}",
            r.detail
        );
        assert!(
            r.bootstrap_camera_no_live_dual_read_live_wave458_ok,
            "bootstrap camera no live dual read live residual wave458: {}",
            r.detail
        );
        assert!(
            r.terrain_visual_presentation_only_method_names_wave459_ok,
            "terrain visual presentation only method names residual pack wave459: {}",
            r.detail
        );
        assert!(
            r.terrain_visual_presentation_only_nav_commands_wave459_ok,
            "terrain visual presentation only nav commands residual pack wave459: {}",
            r.detail
        );
        assert!(
            r.terrain_visual_presentation_only_live_wave459_ok,
            "terrain visual presentation only live residual wave459: {}",
            r.detail
        );
        assert!(
            r.camera_center_presentation_height_method_names_wave460_ok,
            "camera center presentation height method names residual pack wave460: {}",
            r.detail
        );
        assert!(
            r.camera_center_presentation_height_nav_commands_wave460_ok,
            "camera center presentation height nav commands residual pack wave460: {}",
            r.detail
        );
        assert!(
            r.camera_center_presentation_height_live_wave460_ok,
            "camera center presentation height live residual wave460: {}",
            r.detail
        );
        assert!(
            r.presentation_world_bounds_probe_method_names_wave461_ok,
            "presentation world bounds probe method names residual pack wave461: {}",
            r.detail
        );
        assert!(
            r.presentation_world_bounds_probe_nav_commands_wave461_ok,
            "presentation world bounds probe nav commands residual pack wave461: {}",
            r.detail
        );
        assert!(
            r.presentation_world_bounds_probe_live_wave461_ok,
            "presentation world bounds probe live residual wave461: {}",
            r.detail
        );
        assert!(
            r.render_ui_pipeline_presentation_method_names_wave462_ok,
            "render ui pipeline presentation method names residual pack wave462: {}",
            r.detail
        );
        assert!(
            r.render_ui_pipeline_presentation_nav_commands_wave462_ok,
            "render ui pipeline presentation nav commands residual pack wave462: {}",
            r.detail
        );
        assert!(
            r.render_ui_pipeline_presentation_live_wave462_ok,
            "render ui pipeline presentation live residual wave462: {}",
            r.detail
        );
        assert!(
            r.production_quantity_writeback_method_names_wave463_ok,
            "production quantity writeback method names residual pack wave463: {}",
            r.detail
        );
        assert!(
            r.production_quantity_writeback_nav_commands_wave463_ok,
            "production quantity writeback nav commands residual pack wave463: {}",
            r.detail
        );
        assert!(
            r.production_quantity_writeback_live_wave463_ok,
            "production quantity writeback live residual wave463: {}",
            r.detail
        );
        assert!(
            r.production_exit_delay_sole_tick_method_names_wave464_ok,
            "production exit delay sole tick method names residual pack wave464: {}",
            r.detail
        );
        assert!(
            r.production_exit_delay_sole_tick_nav_commands_wave464_ok,
            "production exit delay sole tick nav commands residual pack wave464: {}",
            r.detail
        );
        assert!(
            r.production_exit_delay_sole_tick_live_wave464_ok,
            "production exit delay sole tick live residual wave464: {}",
            r.detail
        );
        assert!(
            r.minimap_heightmap_repair_presentation_first_method_names_wave465_ok,
            "minimap heightmap repair presentation first method names residual pack wave465: {}",
            r.detail
        );
        assert!(
            r.minimap_heightmap_repair_presentation_first_nav_commands_wave465_ok,
            "minimap heightmap repair presentation first nav commands residual pack wave465: {}",
            r.detail
        );
        assert!(
            r.minimap_heightmap_repair_presentation_first_live_wave465_ok,
            "minimap heightmap repair presentation first live residual wave465: {}",
            r.detail
        );
        assert!(
            r.presentation_env_seed_gameworld_method_names_wave466_ok,
            "presentation env seed gameworld method names residual pack wave466: {}",
            r.detail
        );
        assert!(
            r.presentation_env_seed_gameworld_nav_commands_wave466_ok,
            "presentation env seed gameworld nav commands residual pack wave466: {}",
            r.detail
        );
        assert!(
            r.presentation_env_seed_gameworld_live_wave466_ok,
            "presentation env seed gameworld live residual wave466: {}",
            r.detail
        );
        assert!(
            r.presentation_env_seed_mirror_last_method_names_wave467_ok,
            "presentation env seed mirror last method names residual pack wave467: {}",
            r.detail
        );
        assert!(
            r.presentation_env_seed_mirror_last_nav_commands_wave467_ok,
            "presentation env seed mirror last nav commands residual pack wave467: {}",
            r.detail
        );
        assert!(
            r.presentation_env_seed_mirror_last_live_wave467_ok,
            "presentation env seed mirror last live residual wave467: {}",
            r.detail
        );
        assert!(
            r.minimap_reinit_instance_presentation_method_names_wave468_ok,
            "minimap reinit instance presentation method names residual pack wave468: {}",
            r.detail
        );
        assert!(
            r.minimap_reinit_instance_presentation_nav_commands_wave468_ok,
            "minimap reinit instance presentation nav commands residual pack wave468: {}",
            r.detail
        );
        assert!(
            r.minimap_reinit_instance_presentation_live_wave468_ok,
            "minimap reinit instance presentation live residual wave468: {}",
            r.detail
        );
        assert!(
            r.pathfind_midframe_stub_removed_method_names_wave469_ok,
            "pathfind midframe stub removed method names residual pack wave469: {}",
            r.detail
        );
        assert!(
            r.pathfind_midframe_stub_removed_nav_commands_wave469_ok,
            "pathfind midframe stub removed nav commands residual pack wave469: {}",
            r.detail
        );
        assert!(
            r.pathfind_midframe_stub_removed_live_wave469_ok,
            "pathfind midframe stub removed live residual wave469: {}",
            r.detail
        );
        assert!(
            r.projectile_authority_flare_host_method_names_wave470_ok,
            "projectile authority flare host method names residual pack wave470: {}",
            r.detail
        );
        assert!(
            r.projectile_authority_flare_host_nav_commands_wave470_ok,
            "projectile authority flare host nav commands residual pack wave470: {}",
            r.detail
        );
        assert!(
            r.projectile_authority_flare_host_live_wave470_ok,
            "projectile authority flare host live residual wave470: {}",
            r.detail
        );
        assert!(
            r.engine_env_free_fn_game_logic_only_seed_method_names_wave471_ok,
            "engine env free fn game logic only seed method names residual pack wave471: {}",
            r.detail
        );
        assert!(
            r.engine_env_free_fn_game_logic_only_seed_nav_commands_wave471_ok,
            "engine env free fn game logic only seed nav commands residual pack wave471: {}",
            r.detail
        );
        assert!(
            r.engine_env_free_fn_game_logic_only_seed_live_wave471_ok,
            "engine env free fn game logic only seed live residual wave471: {}",
            r.detail
        );
        assert!(
            r.dead_model_preload_removed_method_names_wave472_ok,
            "dead model preload removed method names residual pack wave472: {}",
            r.detail
        );
        assert!(
            r.dead_model_preload_removed_nav_commands_wave472_ok,
            "dead model preload removed nav commands residual pack wave472: {}",
            r.detail
        );
        assert!(
            r.dead_model_preload_removed_live_wave472_ok,
            "dead model preload removed live residual wave472: {}",
            r.detail
        );
        assert!(
            r.camera_bootstrap_presentation_only_method_names_wave473_ok,
            "camera bootstrap presentation only method names residual pack wave473: {}",
            r.detail
        );
        assert!(
            r.camera_bootstrap_presentation_only_nav_commands_wave473_ok,
            "camera bootstrap presentation only nav commands residual pack wave473: {}",
            r.detail
        );
        assert!(
            r.camera_bootstrap_presentation_only_live_wave473_ok,
            "camera bootstrap presentation only live residual wave473: {}",
            r.detail
        );
        assert!(
            r.ensure_presentation_env_instance_method_names_wave474_ok,
            "ensure presentation env instance method names residual pack wave474: {}",
            r.detail
        );
        assert!(
            r.ensure_presentation_env_instance_nav_commands_wave474_ok,
            "ensure presentation env instance nav commands residual pack wave474: {}",
            r.detail
        );
        assert!(
            r.ensure_presentation_env_instance_live_wave474_ok,
            "ensure presentation env instance live residual wave474: {}",
            r.detail
        );
        assert!(
            r.map_ground_no_registry_pose_dual_write_method_names_wave475_ok,
            "map ground no registry pose dual write method names residual pack wave475: {}",
            r.detail
        );
        assert!(
            r.map_ground_no_registry_pose_dual_write_nav_commands_wave475_ok,
            "map ground no registry pose dual write nav commands residual pack wave475: {}",
            r.detail
        );
        assert!(
            r.map_ground_no_registry_pose_dual_write_live_wave475_ok,
            "map ground no registry pose dual write live residual wave475: {}",
            r.detail
        );
        assert!(
            r.named_shell_host_only_tracker_method_names_wave476_ok,
            "named shell host only tracker method names residual pack wave476: {}",
            r.detail
        );
        assert!(
            r.named_shell_host_only_tracker_nav_commands_wave476_ok,
            "named shell host only tracker nav commands residual pack wave476: {}",
            r.detail
        );
        assert!(
            r.named_shell_host_only_tracker_live_wave476_ok,
            "named shell host only tracker live residual wave476: {}",
            r.detail
        );
        assert!(
            r.production_sole_tick_no_progress_stomp_method_names_wave477_ok,
            "production sole tick no progress stomp method names residual pack wave477: {}",
            r.detail
        );
        assert!(
            r.production_sole_tick_no_progress_stomp_nav_commands_wave477_ok,
            "production sole tick no progress stomp nav commands residual pack wave477: {}",
            r.detail
        );
        assert!(
            r.production_sole_tick_no_progress_stomp_live_wave477_ok,
            "production sole tick no progress stomp live residual wave477: {}",
            r.detail
        );
        assert!(
            r.construction_sole_tick_no_progress_stomp_method_names_wave478_ok,
            "construction sole tick no progress stomp method names residual pack wave478: {}",
            r.detail
        );
        assert!(
            r.construction_sole_tick_no_progress_stomp_nav_commands_wave478_ok,
            "construction sole tick no progress stomp nav commands residual pack wave478: {}",
            r.detail
        );
        assert!(
            r.construction_sole_tick_no_progress_stomp_live_wave478_ok,
            "construction sole tick no progress stomp live residual wave478: {}",
            r.detail
        );
        assert!(
            r.special_power_sole_tick_no_cooldown_stomp_method_names_wave479_ok,
            "special power sole tick no cooldown stomp method names residual pack wave479: {}",
            r.detail
        );
        assert!(
            r.special_power_sole_tick_no_cooldown_stomp_nav_commands_wave479_ok,
            "special power sole tick no cooldown stomp nav commands residual pack wave479: {}",
            r.detail
        );
        assert!(
            r.special_power_sole_tick_no_cooldown_stomp_live_wave479_ok,
            "special power sole tick no cooldown stomp live residual wave479: {}",
            r.detail
        );
        assert!(
            r.production_sole_tick_exit_delay_arm_method_names_wave480_ok,
            "production sole tick exit delay arm method names residual pack wave480: {}",
            r.detail
        );
        assert!(
            r.production_sole_tick_exit_delay_arm_nav_commands_wave480_ok,
            "production sole tick exit delay arm nav commands residual pack wave480: {}",
            r.detail
        );
        assert!(
            r.production_sole_tick_exit_delay_arm_live_wave480_ok,
            "production sole tick exit delay arm live residual wave480: {}",
            r.detail
        );
        assert!(
            r.sell_deconstruction_sole_tick_no_stomp_method_names_wave481_ok,
            "sell deconstruction sole tick no stomp method names residual pack wave481: {}",
            r.detail
        );
        assert!(
            r.sell_deconstruction_sole_tick_no_stomp_nav_commands_wave481_ok,
            "sell deconstruction sole tick no stomp nav commands residual pack wave481: {}",
            r.detail
        );
        assert!(
            r.sell_deconstruction_sole_tick_no_stomp_live_wave481_ok,
            "sell deconstruction sole tick no stomp live residual wave481: {}",
            r.detail
        );
        assert!(
            r.sell_finish_skips_topple_destroy_method_names_wave482_ok,
            "sell finish skips topple destroy method names residual pack wave482: {}",
            r.detail
        );
        assert!(
            r.sell_finish_skips_topple_destroy_nav_commands_wave482_ok,
            "sell finish skips topple destroy nav commands residual pack wave482: {}",
            r.detail
        );
        assert!(
            r.sell_finish_skips_topple_destroy_live_wave482_ok,
            "sell finish skips topple destroy live residual wave482: {}",
            r.detail
        );
        assert!(
            r.production_upgrade_complete_queue_refresh_method_names_wave483_ok,
            "production upgrade complete queue refresh method names residual pack wave483: {}",
            r.detail
        );
        assert!(
            r.production_upgrade_complete_queue_refresh_nav_commands_wave483_ok,
            "production upgrade complete queue refresh nav commands residual pack wave483: {}",
            r.detail
        );
        assert!(
            r.production_upgrade_complete_queue_refresh_live_wave483_ok,
            "production upgrade complete queue refresh live residual wave483: {}",
            r.detail
        );
        assert!(
            r.cancel_all_production_queue_refresh_method_names_wave484_ok,
            "cancel_all production queue refresh method names residual pack wave484: {}",
            r.detail
        );
        assert!(
            r.cancel_all_production_queue_refresh_nav_commands_wave484_ok,
            "cancel_all production queue refresh nav commands residual pack wave484: {}",
            r.detail
        );
        assert!(
            r.cancel_all_production_queue_refresh_live_wave484_ok,
            "cancel_all production queue refresh live residual wave484: {}",
            r.detail
        );
        assert!(
            r.cancel_clears_exit_delay_method_names_wave485_ok,
            "cancel clears exit delay method names residual pack wave485: {}",
            r.detail
        );
        assert!(
            r.cancel_clears_exit_delay_nav_commands_wave485_ok,
            "cancel clears exit delay nav commands residual pack wave485: {}",
            r.detail
        );
        assert!(
            r.cancel_clears_exit_delay_live_wave485_ok,
            "cancel clears exit delay live residual wave485: {}",
            r.detail
        );
        assert!(
            r.production_door_model_condition_log_method_names_wave486_ok,
            "production door model condition log method names residual pack wave486: {}",
            r.detail
        );
        assert!(
            r.production_door_model_condition_log_nav_commands_wave486_ok,
            "production door model condition log nav commands residual pack wave486: {}",
            r.detail
        );
        assert!(
            r.production_door_model_condition_log_live_wave486_ok,
            "production door model condition log live residual wave486: {}",
            r.detail
        );
        assert!(
            r.combat_model_condition_channel_method_names_wave487_ok,
            "combat model condition channel method names residual pack wave487: {}",
            r.detail
        );
        assert!(
            r.combat_model_condition_channel_nav_commands_wave487_ok,
            "combat model condition channel nav commands residual pack wave487: {}",
            r.detail
        );
        assert!(
            r.combat_model_condition_channel_live_wave487_ok,
            "combat model condition channel live residual wave487: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_model_condition_method_names_wave488_ok,
            "entity presentation model condition method names residual pack wave488: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_model_condition_nav_commands_wave488_ok,
            "entity presentation model condition nav commands residual pack wave488: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_model_condition_live_wave488_ok,
            "entity presentation model condition live residual wave488: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_combat_ui_method_names_wave489_ok,
            "entity presentation combat ui method names residual pack wave489: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_combat_ui_nav_commands_wave489_ok,
            "entity presentation combat ui nav commands residual pack wave489: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_combat_ui_live_wave489_ok,
            "entity presentation combat ui live residual wave489: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_structure_ui_method_names_wave490_ok,
            "entity presentation structure ui method names residual pack wave490: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_structure_ui_nav_commands_wave490_ok,
            "entity presentation structure ui nav commands residual pack wave490: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_structure_ui_live_wave490_ok,
            "entity presentation structure ui live residual wave490: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_sold_condition_method_names_wave491_ok,
            "presentation mesh sold condition method names residual pack wave491: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_sold_condition_nav_commands_wave491_ok,
            "presentation mesh sold condition nav commands residual pack wave491: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_sold_condition_live_wave491_ok,
            "presentation mesh sold condition live residual wave491: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_mesh_fow_method_names_wave492_ok,
            "entity presentation mesh fow method names residual pack wave492: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_mesh_fow_nav_commands_wave492_ok,
            "entity presentation mesh fow nav commands residual pack wave492: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_mesh_fow_live_wave492_ok,
            "entity presentation mesh fow live residual wave492: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_ground_bridge_method_names_wave493_ok,
            "entity presentation ground bridge method names residual pack wave493: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_ground_bridge_nav_commands_wave493_ok,
            "entity presentation ground bridge nav commands residual pack wave493: {}",
            r.detail
        );
        assert!(
            r.entity_presentation_ground_bridge_live_wave493_ok,
            "entity presentation ground bridge live residual wave493: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_turret_method_names_wave494_ok,
            "presentation mesh turret method names residual pack wave494: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_turret_nav_commands_wave494_ok,
            "presentation mesh turret nav commands residual pack wave494: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_turret_live_wave494_ok,
            "presentation mesh turret live residual wave494: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_combat_flags_method_names_wave495_ok,
            "presentation mesh combat flags method names residual pack wave495: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_combat_flags_nav_commands_wave495_ok,
            "presentation mesh combat flags nav commands residual pack wave495: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_combat_flags_live_wave495_ok,
            "presentation mesh combat flags live residual wave495: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_door_phase_method_names_wave496_ok,
            "presentation mesh door phase method names residual pack wave496: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_door_phase_nav_commands_wave496_ok,
            "presentation mesh door phase nav commands residual pack wave496: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_door_phase_live_wave496_ok,
            "presentation mesh door phase live residual wave496: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_condition_resolve_method_names_wave497_ok,
            "presentation mesh condition resolve method names residual pack wave497: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_condition_resolve_nav_commands_wave497_ok,
            "presentation mesh condition resolve nav commands residual pack wave497: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_condition_resolve_live_wave497_ok,
            "presentation mesh condition resolve live residual wave497: {}",
            r.detail
        );
        assert!(
            r.presentation_host_fx_overlay_method_names_wave498_ok,
            "presentation host FX overlay method names residual pack wave498: {}",
            r.detail
        );
        assert!(
            r.presentation_host_fx_overlay_nav_commands_wave498_ok,
            "presentation host FX overlay nav commands residual pack wave498: {}",
            r.detail
        );
        assert!(
            r.presentation_host_fx_overlay_live_wave498_ok,
            "presentation host FX overlay live residual wave498: {}",
            r.detail
        );
        assert!(
            r.presentation_poison_defector_tint_method_names_wave499_ok,
            "presentation poison/defector tint method names residual pack wave499: {}",
            r.detail
        );
        assert!(
            r.presentation_poison_defector_tint_nav_commands_wave499_ok,
            "presentation poison/defector tint nav commands residual pack wave499: {}",
            r.detail
        );
        assert!(
            r.presentation_poison_defector_tint_live_wave499_ok,
            "presentation poison/defector tint live residual wave499: {}",
            r.detail
        );
        assert!(
            r.presentation_object_fx_particles_method_names_wave500_ok,
            "presentation object FX particles method names residual pack wave500: {}",
            r.detail
        );
        assert!(
            r.presentation_object_fx_particles_nav_commands_wave500_ok,
            "presentation object FX particles nav commands residual pack wave500: {}",
            r.detail
        );
        assert!(
            r.presentation_object_fx_particles_live_wave500_ok,
            "presentation object FX particles live residual wave500: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_deploy_radar_method_names_wave501_ok,
            "presentation mesh deploy/radar method names residual pack wave501: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_deploy_radar_nav_commands_wave501_ok,
            "presentation mesh deploy/radar nav commands residual pack wave501: {}",
            r.detail
        );
        assert!(
            r.presentation_mesh_deploy_radar_live_wave501_ok,
            "presentation mesh deploy/radar live residual wave501: {}",
            r.detail
        );
        assert!(
            r.presentation_stealth_mesh_method_names_wave502_ok,
            "presentation stealth mesh method names residual pack wave502: {}",
            r.detail
        );
        assert!(
            r.presentation_stealth_mesh_nav_commands_wave502_ok,
            "presentation stealth mesh nav commands residual pack wave502: {}",
            r.detail
        );
        assert!(
            r.presentation_stealth_mesh_live_wave502_ok,
            "presentation stealth mesh live residual wave502: {}",
            r.detail
        );
        assert!(
            r.presentation_construction_disguise_method_names_wave503_ok,
            "presentation construction/disguise method names residual pack wave503: {}",
            r.detail
        );
        assert!(
            r.presentation_construction_disguise_nav_commands_wave503_ok,
            "presentation construction/disguise nav commands residual pack wave503: {}",
            r.detail
        );
        assert!(
            r.presentation_construction_disguise_live_wave503_ok,
            "presentation construction/disguise live residual wave503: {}",
            r.detail
        );
        assert!(
            r.presentation_garrison_contain_method_names_wave504_ok,
            "presentation garrison/contain method names residual pack wave504: {}",
            r.detail
        );
        assert!(
            r.presentation_garrison_contain_nav_commands_wave504_ok,
            "presentation garrison/contain nav commands residual pack wave504: {}",
            r.detail
        );
        assert!(
            r.presentation_garrison_contain_live_wave504_ok,
            "presentation garrison/contain live residual wave504: {}",
            r.detail
        );
        assert!(
            r.presentation_air_parachute_method_names_wave505_ok,
            "presentation air/parachute method names residual pack wave505: {}",
            r.detail
        );
        assert!(
            r.presentation_air_parachute_nav_commands_wave505_ok,
            "presentation air/parachute nav commands residual pack wave505: {}",
            r.detail
        );
        assert!(
            r.presentation_air_parachute_live_wave505_ok,
            "presentation air/parachute live residual wave505: {}",
            r.detail
        );
        assert!(
            r.presentation_weaponset_veterancy_method_names_wave506_ok,
            "presentation weaponset veterancy method names residual pack wave506: {}",
            r.detail
        );
        assert!(
            r.presentation_weaponset_veterancy_nav_commands_wave506_ok,
            "presentation weaponset veterancy nav commands residual pack wave506: {}",
            r.detail
        );
        assert!(
            r.presentation_weaponset_veterancy_live_wave506_ok,
            "presentation weaponset veterancy live residual wave506: {}",
            r.detail
        );
        assert!(
            r.presentation_water_rider_method_names_wave507_ok,
            "presentation water/rider method names residual pack wave507: {}",
            r.detail
        );
        assert!(
            r.presentation_water_rider_nav_commands_wave507_ok,
            "presentation water/rider nav commands residual pack wave507: {}",
            r.detail
        );
        assert!(
            r.presentation_water_rider_live_wave507_ok,
            "presentation water/rider live residual wave507: {}",
            r.detail
        );
        assert!(
            r.presentation_body_disguise_stun_method_names_wave508_ok,
            "presentation body/disguise/stun method names residual pack wave508: {}",
            r.detail
        );
        assert!(
            r.presentation_body_disguise_stun_nav_commands_wave508_ok,
            "presentation body/disguise/stun nav commands residual pack wave508: {}",
            r.detail
        );
        assert!(
            r.presentation_body_disguise_stun_live_wave508_ok,
            "presentation body/disguise/stun live residual wave508: {}",
            r.detail
        );
        assert!(
            r.presentation_topple_freefall_weather_method_names_wave509_ok,
            "presentation topple/freefall/weather method names residual pack wave509: {}",
            r.detail
        );
        assert!(
            r.presentation_topple_freefall_weather_nav_commands_wave509_ok,
            "presentation topple/freefall/weather nav commands residual pack wave509: {}",
            r.detail
        );
        assert!(
            r.presentation_topple_freefall_weather_live_wave509_ok,
            "presentation topple/freefall/weather live residual wave509: {}",
            r.detail
        );
        assert!(
            r.presentation_capture_load_overcharge_method_names_wave510_ok,
            "presentation capture/load/overcharge method names residual pack wave510: {}",
            r.detail
        );
        assert!(
            r.presentation_capture_load_overcharge_nav_commands_wave510_ok,
            "presentation capture/load/overcharge nav commands residual pack wave510: {}",
            r.detail
        );
        assert!(
            r.presentation_capture_load_overcharge_live_wave510_ok,
            "presentation capture/load/overcharge live residual wave510: {}",
            r.detail
        );
        assert!(
            r.presentation_burn_cheer_carry_method_names_wave511_ok,
            "presentation burn/cheer/carry method names residual pack wave511: {}",
            r.detail
        );
        assert!(
            r.presentation_burn_cheer_carry_nav_commands_wave511_ok,
            "presentation burn/cheer/carry nav commands residual pack wave511: {}",
            r.detail
        );
        assert!(
            r.presentation_burn_cheer_carry_live_wave511_ok,
            "presentation burn/cheer/carry live residual wave511: {}",
            r.detail
        );
        assert!(
            r.presentation_fire_prone_turret_method_names_wave512_ok,
            "presentation fire/prone/turret method names residual pack wave512: {}",
            r.detail
        );
        assert!(
            r.presentation_fire_prone_turret_nav_commands_wave512_ok,
            "presentation fire/prone/turret nav commands residual pack wave512: {}",
            r.detail
        );
        assert!(
            r.presentation_fire_prone_turret_live_wave512_ok,
            "presentation fire/prone/turret live residual wave512: {}",
            r.detail
        );
        assert!(
            r.presentation_jam_die_reload_pack_method_names_wave513_ok,
            "presentation jam/die/reload/pack method names residual pack wave513: {}",
            r.detail
        );
        assert!(
            r.presentation_jam_die_reload_pack_nav_commands_wave513_ok,
            "presentation jam/die/reload/pack nav commands residual pack wave513: {}",
            r.detail
        );
        assert!(
            r.presentation_jam_die_reload_pack_live_wave513_ok,
            "presentation jam/die/reload/pack live residual wave513: {}",
            r.detail
        );
        assert!(
            r.presentation_emoticon_float_method_names_wave514_ok,
            "presentation emoticon float method names residual pack wave514: {}",
            r.detail
        );
        assert!(
            r.presentation_emoticon_float_nav_commands_wave514_ok,
            "presentation emoticon float nav commands residual pack wave514: {}",
            r.detail
        );
        assert!(
            r.presentation_emoticon_float_live_wave514_ok,
            "presentation emoticon float live residual wave514: {}",
            r.detail
        );
        assert!(
            r.presentation_surrender_formation_method_names_wave515_ok,
            "presentation surrender/formation method names residual pack wave515: {}",
            r.detail
        );

        assert!(
            r.presentation_surrender_formation_nav_commands_wave515_ok,
            "presentation surrender/formation nav commands residual pack wave515: {}",
            r.detail
        );
        assert!(
            r.presentation_surrender_formation_live_wave515_ok,
            "presentation surrender/formation live residual wave515: {}",
            r.detail
        );
        assert!(
            r.presentation_formation_link_method_names_wave516_ok,
            "presentation formation link method names residual pack wave516: {}",
            r.detail
        );
        assert!(
            r.presentation_formation_link_nav_commands_wave516_ok,
            "presentation formation link nav commands residual pack wave516: {}",
            r.detail
        );
        assert!(
            r.presentation_formation_link_live_wave516_ok,
            "presentation formation link live residual wave516: {}",
            r.detail
        );
        assert!(
            r.presentation_weapon_fire_slot_method_names_wave517_ok,
            "presentation weapon fire slot method names residual pack wave517: {}",
            r.detail
        );
        assert!(
            r.presentation_weapon_fire_slot_nav_commands_wave517_ok,
            "presentation weapon fire slot nav commands residual pack wave517: {}",
            r.detail
        );
        assert!(
            r.presentation_weapon_fire_slot_live_wave517_ok,
            "presentation weapon fire slot live residual wave517: {}",
            r.detail
        );
        assert!(
            r.presentation_weaponset_enemy_near_method_names_wave518_ok,
            "presentation weaponset/enemy-near method names residual pack wave518: {}",
            r.detail
        );
        assert!(
            r.presentation_weaponset_enemy_near_nav_commands_wave518_ok,
            "presentation weaponset/enemy-near nav commands residual pack wave518: {}",
            r.detail
        );
        assert!(
            r.presentation_weaponset_enemy_near_live_wave518_ok,
            "presentation weaponset/enemy-near live residual wave518: {}",
            r.detail
        );
        assert!(
            r.presentation_shock_power_jet_method_names_wave519_ok,
            "presentation shock/power/jet method names residual pack wave519: {}",
            r.detail
        );
        assert!(
            r.presentation_shock_power_jet_nav_commands_wave519_ok,
            "presentation shock/power/jet nav commands residual pack wave519: {}",
            r.detail
        );
        assert!(
            r.presentation_shock_power_jet_live_wave519_ok,
            "presentation shock/power/jet live residual wave519: {}",
            r.detail
        );
        assert!(
            r.presentation_anim_steer_method_names_wave520_ok,
            "presentation anim steer method names residual pack wave520: {}",
            r.detail
        );
        assert!(
            r.presentation_anim_steer_nav_commands_wave520_ok,
            "presentation anim steer nav commands residual pack wave520: {}",
            r.detail
        );
        assert!(
            r.presentation_anim_steer_live_wave520_ok,
            "presentation anim steer live residual wave520: {}",
            r.detail
        );
        assert!(
            r.presentation_dock_rider_method_names_wave521_ok,
            "presentation dock/rider method names residual pack wave521: {}",
            r.detail
        );
        assert!(
            r.presentation_dock_rider_nav_commands_wave521_ok,
            "presentation dock/rider nav commands residual pack wave521: {}",
            r.detail
        );
        assert!(
            r.presentation_dock_rider_live_wave521_ok,
            "presentation dock/rider live residual wave521: {}",
            r.detail
        );
        assert!(
            r.presentation_cliff_flood_method_names_wave522_ok,
            "presentation cliff/flood method names residual pack wave522: {}",
            r.detail
        );
        assert!(
            r.presentation_cliff_flood_nav_commands_wave522_ok,
            "presentation cliff/flood nav commands residual pack wave522: {}",
            r.detail
        );
        assert!(
            r.presentation_cliff_flood_live_wave522_ok,
            "presentation cliff/flood live residual wave522: {}",
            r.detail
        );
        assert!(
            r.presentation_second_life_stun_method_names_wave523_ok,
            "presentation second-life/stun method names residual pack wave523: {}",
            r.detail
        );
        assert!(
            r.presentation_second_life_stun_nav_commands_wave523_ok,
            "presentation second-life/stun nav commands residual pack wave523: {}",
            r.detail
        );
        assert!(
            r.presentation_second_life_stun_live_wave523_ok,
            "presentation second-life/stun live residual wave523: {}",
            r.detail
        );
        assert!(
            r.presentation_multi_door_smolder_method_names_wave524_ok,
            "presentation multi-door/smolder method names residual pack wave524: {}",
            r.detail
        );
        assert!(
            r.presentation_multi_door_smolder_nav_commands_wave524_ok,
            "presentation multi-door/smolder nav commands residual pack wave524: {}",
            r.detail
        );
        assert!(
            r.presentation_multi_door_smolder_live_wave524_ok,
            "presentation multi-door/smolder live residual wave524: {}",
            r.detail
        );
        assert!(
            r.presentation_crush_user_method_names_wave525_ok,
            "presentation crush/user method names residual pack wave525: {}",
            r.detail
        );
        assert!(
            r.presentation_crush_user_nav_commands_wave525_ok,
            "presentation crush/user nav commands residual pack wave525: {}",
            r.detail
        );
        assert!(
            r.presentation_crush_user_live_wave525_ok,
            "presentation crush/user live residual wave525: {}",
            r.detail
        );
        assert!(
            r.presentation_move_attack_helper_method_names_wave526_ok,
            "presentation move/attack helper method names residual pack wave526: {}",
            r.detail
        );
        assert!(
            r.presentation_move_attack_helper_nav_commands_wave526_ok,
            "presentation move/attack helper nav commands residual pack wave526: {}",
            r.detail
        );
        assert!(
            r.presentation_move_attack_helper_live_wave526_ok,
            "presentation move/attack helper live residual wave526: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_audio_method_names_wave527_ok,
            "presentation firesound audio method names residual pack wave527: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_audio_nav_commands_wave527_ok,
            "presentation firesound audio nav commands residual pack wave527: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_audio_live_wave527_ok,
            "presentation firesound audio live residual wave527: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_stop_method_names_wave528_ok,
            "presentation firesound stop method names residual pack wave528: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_stop_nav_commands_wave528_ok,
            "presentation firesound stop nav commands residual pack wave528: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_stop_live_wave528_ok,
            "presentation firesound stop live residual wave528: {}",
            r.detail
        );
        assert!(
            r.presentation_radar_eva_audio_method_names_wave529_ok,
            "presentation radar/EVA audio method names residual pack wave529: {}",
            r.detail
        );
        assert!(
            r.presentation_radar_eva_audio_nav_commands_wave529_ok,
            "presentation radar/EVA audio nav commands residual pack wave529: {}",
            r.detail
        );
        assert!(
            r.presentation_radar_eva_audio_live_wave529_ok,
            "presentation radar/EVA audio live residual wave529: {}",
            r.detail
        );
        assert!(
            r.presentation_capture_audio_method_names_wave530_ok,
            "presentation capture audio method names residual pack wave530: {}",
            r.detail
        );
        assert!(
            r.presentation_capture_audio_nav_commands_wave530_ok,
            "presentation capture audio nav commands residual pack wave530: {}",
            r.detail
        );
        assert!(
            r.presentation_capture_audio_live_wave530_ok,
            "presentation capture audio live residual wave530: {}",
            r.detail
        );
        assert!(
            r.command_integration_presentation_fill_method_names_wave531_ok,
            "command_integration presentation fill method names residual pack wave531: {}",
            r.detail
        );
        assert!(
            r.command_integration_presentation_fill_nav_commands_wave531_ok,
            "command_integration presentation fill nav commands residual pack wave531: {}",
            r.detail
        );
        assert!(
            r.command_integration_presentation_fill_live_wave531_ok,
            "command_integration presentation fill live residual wave531: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_drain_sibling_method_names_wave532_ok,
            "presentation firesound drain sibling method names residual pack wave532: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_drain_sibling_nav_commands_wave532_ok,
            "presentation firesound drain sibling nav commands residual pack wave532: {}",
            r.detail
        );
        assert!(
            r.presentation_firesound_drain_sibling_live_wave532_ok,
            "presentation firesound drain sibling live residual wave532: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_pulse_audio_method_names_wave533_ok,
            "presentation eva pulse audio method names residual pack wave533: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_pulse_audio_nav_commands_wave533_ok,
            "presentation eva pulse audio nav commands residual pack wave533: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_pulse_audio_live_wave533_ok,
            "presentation eva pulse audio live residual wave533: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_full_matrix_method_names_wave534_ok,
            "presentation eva full matrix method names residual pack wave534: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_full_matrix_nav_commands_wave534_ok,
            "presentation eva full matrix nav commands residual pack wave534: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_full_matrix_live_wave534_ok,
            "presentation eva full matrix live residual wave534: {}",
            r.detail
        );
        assert!(
            r.presentation_particle_spawn_audio_method_names_wave535_ok,
            "presentation particle spawn audio method names residual pack wave535: {}",
            r.detail
        );
        assert!(
            r.presentation_particle_spawn_audio_nav_commands_wave535_ok,
            "presentation particle spawn audio nav commands residual pack wave535: {}",
            r.detail
        );
        assert!(
            r.presentation_particle_spawn_audio_live_wave535_ok,
            "presentation particle spawn audio live residual wave535: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_client_dispatch_method_names_wave536_ok,
            "presentation eva client dispatch method names residual pack wave536: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_client_dispatch_nav_commands_wave536_ok,
            "presentation eva client dispatch nav commands residual pack wave536: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_client_dispatch_live_wave536_ok,
            "presentation eva client dispatch live residual wave536: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_alert_counter_dedupe_method_names_wave537_ok,
            "presentation eva alert counter dedupe method names residual pack wave537: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_alert_counter_dedupe_nav_commands_wave537_ok,
            "presentation eva alert counter dedupe nav commands residual pack wave537: {}",
            r.detail
        );
        assert!(
            r.presentation_eva_alert_counter_dedupe_live_wave537_ok,
            "presentation eva alert counter dedupe live residual wave537: {}",
            r.detail
        );
        assert!(
            r.presentation_alliance_notify_method_names_wave538_ok,
            "presentation alliance notify method names residual pack wave538: {}",
            r.detail
        );
        assert!(
            r.presentation_alliance_notify_nav_commands_wave538_ok,
            "presentation alliance notify nav commands residual pack wave538: {}",
            r.detail
        );
        assert!(
            r.presentation_alliance_notify_live_wave538_ok,
            "presentation alliance notify live residual wave538: {}",
            r.detail
        );
        assert!(
            r.presentation_defeat_notify_method_names_wave539_ok,
            "presentation defeat notify method names residual pack wave539: {}",
            r.detail
        );
        assert!(
            r.presentation_defeat_notify_nav_commands_wave539_ok,
            "presentation defeat notify nav commands residual pack wave539: {}",
            r.detail
        );
        assert!(
            r.presentation_defeat_notify_live_wave539_ok,
            "presentation defeat notify live residual wave539: {}",
            r.detail
        );
        assert!(
            r.presentation_camera_shell_flag_method_names_wave540_ok,
            "presentation camera shell flag method names residual pack wave540: {}",
            r.detail
        );
        assert!(
            r.presentation_camera_shell_flag_nav_commands_wave540_ok,
            "presentation camera shell flag nav commands residual pack wave540: {}",
            r.detail
        );
        assert!(
            r.presentation_camera_shell_flag_live_wave540_ok,
            "presentation camera shell flag live residual wave540: {}",
            r.detail
        );
        assert!(
            r.rmb_presentation_no_dual_read_method_names_wave541_ok,
            "rmb presentation no dual-read method names residual pack wave541: {}",
            r.detail
        );
        assert!(
            r.rmb_presentation_no_dual_read_nav_commands_wave541_ok,
            "rmb presentation no dual-read nav commands residual pack wave541: {}",
            r.detail
        );
        assert!(
            r.rmb_presentation_no_dual_read_live_wave541_ok,
            "rmb presentation no dual-read live residual wave541: {}",
            r.detail
        );
        assert!(
            r.presentation_mouse_and_defeat_gate_method_names_wave542_ok,
            "presentation mouse/defeat gate method names residual pack wave542: {}",
            r.detail
        );
        assert!(
            r.presentation_mouse_and_defeat_gate_nav_commands_wave542_ok,
            "presentation mouse/defeat gate nav commands residual pack wave542: {}",
            r.detail
        );
        assert!(
            r.presentation_mouse_and_defeat_gate_live_wave542_ok,
            "presentation mouse/defeat gate live residual wave542: {}",
            r.detail
        );
        assert!(
            r.ui_selected_presentation_fail_closed_method_names_wave543_ok,
            "ui_selected presentation fail-closed method names residual pack wave543: {}",
            r.detail
        );
        assert!(
            r.ui_selected_presentation_fail_closed_nav_commands_wave543_ok,
            "ui_selected presentation fail-closed nav commands residual pack wave543: {}",
            r.detail
        );
        assert!(
            r.ui_selected_presentation_fail_closed_live_wave543_ok,
            "ui_selected presentation fail-closed live residual wave543: {}",
            r.detail
        );
        assert!(
            r.ui_selection_seed_presentation_fail_closed_method_names_wave544_ok,
            "ui_selection_seed presentation fail-closed method names residual pack wave544: {}",
            r.detail
        );
        assert!(
            r.ui_selection_seed_presentation_fail_closed_nav_commands_wave544_ok,
            "ui_selection_seed presentation fail-closed nav commands residual pack wave544: {}",
            r.detail
        );
        assert!(
            r.ui_selection_seed_presentation_fail_closed_live_wave544_ok,
            "ui_selection_seed presentation fail-closed live residual wave544: {}",
            r.detail
        );
        assert!(
            r.save_restart_presentation_fail_closed_method_names_wave545_ok,
            "save/restart presentation fail-closed method names residual pack wave545: {}",
            r.detail
        );
        assert!(
            r.save_restart_presentation_fail_closed_nav_commands_wave545_ok,
            "save/restart presentation fail-closed nav commands residual pack wave545: {}",
            r.detail
        );
        assert!(
            r.save_restart_presentation_fail_closed_live_wave545_ok,
            "save/restart presentation fail-closed live residual wave545: {}",
            r.detail
        );
        assert!(
            r.host_status_map_presentation_fail_closed_method_names_wave546_ok,
            "host status map presentation fail-closed method names residual pack wave546: {}",
            r.detail
        );
        assert!(
            r.host_status_map_presentation_fail_closed_nav_commands_wave546_ok,
            "host status map presentation fail-closed nav commands residual pack wave546: {}",
            r.detail
        );
        assert!(
            r.host_status_map_presentation_fail_closed_live_wave546_ok,
            "host status map presentation fail-closed live residual wave546: {}",
            r.detail
        );
        assert!(
            r.host_status_selected_presentation_fail_closed_method_names_wave547_ok,
            "host status selected presentation fail-closed method names residual pack wave547: {}",
            r.detail
        );
        assert!(
            r.host_status_selected_presentation_fail_closed_nav_commands_wave547_ok,
            "host status selected presentation fail-closed nav commands residual pack wave547: {}",
            r.detail
        );
        assert!(
            r.host_status_selected_presentation_fail_closed_live_wave547_ok,
            "host status selected presentation fail-closed live residual wave547: {}",
            r.detail
        );
        assert!(
            r.camera_follow_presentation_fail_closed_method_names_wave548_ok,
            "camera follow presentation fail-closed method names residual pack wave548: {}",
            r.detail
        );
        assert!(
            r.camera_follow_presentation_fail_closed_nav_commands_wave548_ok,
            "camera follow presentation fail-closed nav commands residual pack wave548: {}",
            r.detail
        );
        assert!(
            r.camera_follow_presentation_fail_closed_live_wave548_ok,
            "camera follow presentation fail-closed live residual wave548: {}",
            r.detail
        );
        assert!(
            r.ui_player_info_presentation_fail_closed_method_names_wave549_ok,
            "ui_player_info presentation fail-closed method names residual pack wave549: {}",
            r.detail
        );
        assert!(
            r.ui_player_info_presentation_fail_closed_nav_commands_wave549_ok,
            "ui_player_info presentation fail-closed nav commands residual pack wave549: {}",
            r.detail
        );
        assert!(
            r.ui_player_info_presentation_fail_closed_live_wave549_ok,
            "ui_player_info presentation fail-closed live residual wave549: {}",
            r.detail
        );
        assert!(
            r.visual_speed_presentation_helper_method_names_wave550_ok,
            "visual speed presentation helper method names residual pack wave550: {}",
            r.detail
        );
        assert!(
            r.visual_speed_presentation_helper_nav_commands_wave550_ok,
            "visual speed presentation helper nav commands residual pack wave550: {}",
            r.detail
        );
        assert!(
            r.visual_speed_presentation_helper_live_wave550_ok,
            "visual speed presentation helper live residual wave550: {}",
            r.detail
        );
        assert!(
            r.time_frozen_presentation_helper_method_names_wave551_ok,
            "time frozen presentation helper method names residual pack wave551: {}",
            r.detail
        );
        assert!(
            r.time_frozen_presentation_helper_nav_commands_wave551_ok,
            "time frozen presentation helper nav commands residual pack wave551: {}",
            r.detail
        );
        assert!(
            r.time_frozen_presentation_helper_live_wave551_ok,
            "time frozen presentation helper live residual wave551: {}",
            r.detail
        );
        assert!(
            r.shell_bypass_presentation_helper_method_names_wave552_ok,
            "shell bypass presentation helper method names residual pack wave552: {}",
            r.detail
        );
        assert!(
            r.shell_bypass_presentation_helper_nav_commands_wave552_ok,
            "shell bypass presentation helper nav commands residual pack wave552: {}",
            r.detail
        );
        assert!(
            r.shell_bypass_presentation_helper_live_wave552_ok,
            "shell bypass presentation helper live residual wave552: {}",
            r.detail
        );
        assert!(
            r.play_time_local_player_presentation_helper_method_names_wave553_ok,
            "play-time/local-player presentation helper method names residual pack wave553: {}",
            r.detail
        );
        assert!(
            r.play_time_local_player_presentation_helper_nav_commands_wave553_ok,
            "play-time/local-player presentation helper nav commands residual pack wave553: {}",
            r.detail
        );
        assert!(
            r.play_time_local_player_presentation_helper_live_wave553_ok,
            "play-time/local-player presentation helper live residual wave553: {}",
            r.detail
        );
        assert!(
            r.map_difficulty_presentation_helper_method_names_wave554_ok,
            "map/difficulty presentation helper method names residual pack wave554: {}",
            r.detail
        );
        assert!(
            r.map_difficulty_presentation_helper_nav_commands_wave554_ok,
            "map/difficulty presentation helper nav commands residual pack wave554: {}",
            r.detail
        );
        assert!(
            r.map_difficulty_presentation_helper_live_wave554_ok,
            "map/difficulty presentation helper live residual wave554: {}",
            r.detail
        );
        assert!(
            r.science_team_presentation_helper_method_names_wave555_ok,
            "science/team presentation helper method names residual pack wave555: {}",
            r.detail
        );
        assert!(
            r.science_team_presentation_helper_nav_commands_wave555_ok,
            "science/team presentation helper nav commands residual pack wave555: {}",
            r.detail
        );
        assert!(
            r.science_team_presentation_helper_live_wave555_ok,
            "science/team presentation helper live residual wave555: {}",
            r.detail
        );
        assert!(
            r.victory_presentation_helper_method_names_wave556_ok,
            "victory presentation helper method names residual pack wave556: {}",
            r.detail
        );
        assert!(
            r.victory_presentation_helper_nav_commands_wave556_ok,
            "victory presentation helper nav commands residual pack wave556: {}",
            r.detail
        );
        assert!(
            r.victory_presentation_helper_live_wave556_ok,
            "victory presentation helper live residual wave556: {}",
            r.detail
        );
        assert!(
            r.replay_presentation_helper_method_names_wave557_ok,
            "replay presentation helper method names residual pack wave557: {}",
            r.detail
        );
        assert!(
            r.replay_presentation_helper_nav_commands_wave557_ok,
            "replay presentation helper nav commands residual pack wave557: {}",
            r.detail
        );
        assert!(
            r.replay_presentation_helper_live_wave557_ok,
            "replay presentation helper live residual wave557: {}",
            r.detail
        );
        assert!(
            r.diplomacy_presentation_helper_method_names_wave558_ok,
            "diplomacy presentation helper method names residual pack wave558: {}",
            r.detail
        );
        assert!(
            r.diplomacy_presentation_helper_nav_commands_wave558_ok,
            "diplomacy presentation helper nav commands residual pack wave558: {}",
            r.detail
        );
        assert!(
            r.diplomacy_presentation_helper_live_wave558_ok,
            "diplomacy presentation helper live residual wave558: {}",
            r.detail
        );
        assert!(
            r.presentation_honesty_align_method_names_wave559_ok,
            "presentation honesty align method names residual pack wave559: {}",
            r.detail
        );
        assert!(
            r.presentation_honesty_align_nav_commands_wave559_ok,
            "presentation honesty align nav commands residual pack wave559: {}",
            r.detail
        );
        assert!(
            r.presentation_honesty_align_live_wave559_ok,
            "presentation honesty align live residual wave559: {}",
            r.detail
        );
        assert!(
            r.logic_frame_presentation_helper_method_names_wave560_ok,
            "logic frame presentation helper method names residual pack wave560: {}",
            r.detail
        );
        assert!(
            r.logic_frame_presentation_helper_nav_commands_wave560_ok,
            "logic frame presentation helper nav commands residual pack wave560: {}",
            r.detail
        );
        assert!(
            r.logic_frame_presentation_helper_live_wave560_ok,
            "logic frame presentation helper live residual wave560: {}",
            r.detail
        );
        assert!(
            r.logic_steps_presentation_helper_method_names_wave561_ok,
            "logic steps presentation helper method names residual pack wave561: {}",
            r.detail
        );
        assert!(
            r.logic_steps_presentation_helper_nav_commands_wave561_ok,
            "logic steps presentation helper nav commands residual pack wave561: {}",
            r.detail
        );
        assert!(
            r.logic_steps_presentation_helper_live_wave561_ok,
            "logic steps presentation helper live residual wave561: {}",
            r.detail
        );
        assert!(
            r.combat_kill_particle_observe_method_names_wave562_ok,
            "combat kill particle observe method names residual pack wave562: {}",
            r.detail
        );
        assert!(
            r.combat_kill_particle_observe_nav_commands_wave562_ok,
            "combat kill particle observe nav commands residual pack wave562: {}",
            r.detail
        );
        assert!(
            r.combat_kill_particle_observe_live_wave562_ok,
            "combat kill particle observe live residual wave562: {}",
            r.detail
        );
        assert!(
            r.template_name_presentation_helper_method_names_wave563_ok,
            "template name presentation helper method names residual pack wave563: {}",
            r.detail
        );
        assert!(
            r.template_name_presentation_helper_nav_commands_wave563_ok,
            "template name presentation helper nav commands residual pack wave563: {}",
            r.detail
        );
        assert!(
            r.template_name_presentation_helper_live_wave563_ok,
            "template name presentation helper live residual wave563: {}",
            r.detail
        );
        assert!(
            r.fixed_step_diag_presentation_helper_method_names_wave564_ok,
            "fixed-step diag presentation helper method names residual pack wave564: {}",
            r.detail
        );
        assert!(
            r.fixed_step_diag_presentation_helper_nav_commands_wave564_ok,
            "fixed-step diag presentation helper nav commands residual pack wave564: {}",
            r.detail
        );
        assert!(
            r.fixed_step_diag_presentation_helper_live_wave564_ok,
            "fixed-step diag presentation helper live residual wave564: {}",
            r.detail
        );
        assert!(
            r.construct_template_presentation_helper_method_names_wave565_ok,
            "construct template presentation helper method names residual pack wave565: {}",
            r.detail
        );
        assert!(
            r.construct_template_presentation_helper_nav_commands_wave565_ok,
            "construct template presentation helper nav commands residual pack wave565: {}",
            r.detail
        );
        assert!(
            r.construct_template_presentation_helper_live_wave565_ok,
            "construct template presentation helper live residual wave565: {}",
            r.detail
        );
        assert!(
            r.boot_ui_message_helper_method_names_wave566_ok,
            "boot UI message helper method names residual pack wave566: {}",
            r.detail
        );
        assert!(
            r.boot_ui_message_helper_nav_commands_wave566_ok,
            "boot UI message helper nav commands residual pack wave566: {}",
            r.detail
        );
        assert!(
            r.boot_ui_message_helper_live_wave566_ok,
            "boot UI message helper live residual wave566: {}",
            r.detail
        );
        assert!(
            r.boot_movie_helper_method_names_wave567_ok,
            "boot movie helper method names residual pack wave567: {}",
            r.detail
        );
        assert!(
            r.boot_movie_helper_nav_commands_wave567_ok,
            "boot movie helper nav commands residual pack wave567: {}",
            r.detail
        );
        assert!(
            r.boot_movie_helper_live_wave567_ok,
            "boot movie helper live residual wave567: {}",
            r.detail
        );
        assert!(
            r.script_fps_helper_method_names_wave568_ok,
            "script FPS helper method names residual pack wave568: {}",
            r.detail
        );
        assert!(
            r.script_fps_helper_nav_commands_wave568_ok,
            "script FPS helper nav commands residual pack wave568: {}",
            r.detail
        );
        assert!(
            r.script_fps_helper_live_wave568_ok,
            "script FPS helper live residual wave568: {}",
            r.detail
        );
        assert!(
            r.defeat_alliance_helper_method_names_wave569_ok,
            "defeat/alliance helper method names residual pack wave569: {}",
            r.detail
        );
        assert!(
            r.defeat_alliance_helper_nav_commands_wave569_ok,
            "defeat/alliance helper nav commands residual pack wave569: {}",
            r.detail
        );
        assert!(
            r.defeat_alliance_helper_live_wave569_ok,
            "defeat/alliance helper live residual wave569: {}",
            r.detail
        );
        assert!(
            r.script_msg_helper_method_names_wave570_ok,
            "script msg helper method names residual pack wave570: {}",
            r.detail
        );
        assert!(
            r.script_msg_helper_nav_commands_wave570_ok,
            "script msg helper nav commands residual pack wave570: {}",
            r.detail
        );
        assert!(
            r.script_msg_helper_live_wave570_ok,
            "script msg helper live residual wave570: {}",
            r.detail
        );
        assert!(
            r.popup_music_helper_method_names_wave571_ok,
            "popup/music helper method names residual pack wave571: {}",
            r.detail
        );
        assert!(
            r.popup_music_helper_nav_commands_wave571_ok,
            "popup/music helper nav commands residual pack wave571: {}",
            r.detail
        );
        assert!(
            r.popup_music_helper_live_wave571_ok,
            "popup/music helper live residual wave571: {}",
            r.detail
        );
        assert!(
            r.boot_camera_helper_method_names_wave572_ok,
            "boot camera helper method names residual pack wave572: {}",
            r.detail
        );
        assert!(
            r.boot_camera_helper_nav_commands_wave572_ok,
            "boot camera helper nav commands residual pack wave572: {}",
            r.detail
        );
        assert!(
            r.boot_camera_helper_live_wave572_ok,
            "boot camera helper live residual wave572: {}",
            r.detail
        );
        assert!(
            r.boot_player_info_helper_method_names_wave573_ok,
            "boot player info helper method names residual pack wave573: {}",
            r.detail
        );
        assert!(
            r.boot_player_info_helper_nav_commands_wave573_ok,
            "boot player info helper nav commands residual pack wave573: {}",
            r.detail
        );
        assert!(
            r.boot_player_info_helper_live_wave573_ok,
            "boot player info helper live residual wave573: {}",
            r.detail
        );
        assert!(
            r.boot_local_player_helper_method_names_wave574_ok,
            "boot local player helper method names residual pack wave574: {}",
            r.detail
        );
        assert!(
            r.boot_local_player_helper_nav_commands_wave574_ok,
            "boot local player helper nav commands residual pack wave574: {}",
            r.detail
        );
        assert!(
            r.boot_local_player_helper_live_wave574_ok,
            "boot local player helper live residual wave574: {}",
            r.detail
        );
        assert!(
            r.host_pause_team_helper_method_names_wave575_ok,
            "host pause/team helper method names residual pack wave575: {}",
            r.detail
        );
        assert!(
            r.host_pause_team_helper_nav_commands_wave575_ok,
            "host pause/team helper nav commands residual pack wave575: {}",
            r.detail
        );
        assert!(
            r.host_pause_team_helper_live_wave575_ok,
            "host pause/team helper live residual wave575: {}",
            r.detail
        );
        assert!(
            r.host_command_flush_helper_method_names_wave576_ok,
            "host command flush helper method names residual pack wave576: {}",
            r.detail
        );
        assert!(
            r.host_command_flush_helper_nav_commands_wave576_ok,
            "host command flush helper nav commands residual pack wave576: {}",
            r.detail
        );
        assert!(
            r.host_command_flush_helper_live_wave576_ok,
            "host command flush helper live residual wave576: {}",
            r.detail
        );
        assert!(
            r.host_camera_start_helper_method_names_wave577_ok,
            "host camera/start helper method names residual pack wave577: {}",
            r.detail
        );
        assert!(
            r.host_camera_start_helper_nav_commands_wave577_ok,
            "host camera/start helper nav commands residual pack wave577: {}",
            r.detail
        );
        assert!(
            r.host_camera_start_helper_live_wave577_ok,
            "host camera/start helper live residual wave577: {}",
            r.detail
        );
        assert!(
            r.host_silent_command_peel_method_names_wave578_ok,
            "host silent command peel method names residual pack wave578: {}",
            r.detail
        );
        assert!(
            r.host_silent_command_peel_nav_commands_wave578_ok,
            "host silent command peel nav commands residual pack wave578: {}",
            r.detail
        );
        assert!(
            r.host_silent_command_peel_live_wave578_ok,
            "host silent command peel live residual wave578: {}",
            r.detail
        );
        assert!(
            r.host_selection_map_helper_method_names_wave579_ok,
            "host selection/map helper method names residual pack wave579: {}",
            r.detail
        );
        assert!(
            r.host_selection_map_helper_nav_commands_wave579_ok,
            "host selection/map helper nav commands residual pack wave579: {}",
            r.detail
        );
        assert!(
            r.host_selection_map_helper_live_wave579_ok,
            "host selection/map helper live residual wave579: {}",
            r.detail
        );
        assert!(
            r.host_cancel_selection_helper_method_names_wave580_ok,
            "host cancel/selection helper method names residual pack wave580: {}",
            r.detail
        );
        assert!(
            r.host_cancel_selection_helper_nav_commands_wave580_ok,
            "host cancel/selection helper nav commands residual pack wave580: {}",
            r.detail
        );
        assert!(
            r.host_cancel_selection_helper_live_wave580_ok,
            "host cancel/selection helper live residual wave580: {}",
            r.detail
        );
        assert!(
            r.host_template_spawn_helper_method_names_wave581_ok,
            "host template/spawn helper method names residual pack wave581: {}",
            r.detail
        );
        assert!(
            r.host_template_spawn_helper_nav_commands_wave581_ok,
            "host template/spawn helper nav commands residual pack wave581: {}",
            r.detail
        );
        assert!(
            r.host_template_spawn_helper_live_wave581_ok,
            "host template/spawn helper live residual wave581: {}",
            r.detail
        );
        assert!(
            r.host_enqueue_shell_cmd_helper_method_names_wave582_ok,
            "host enqueue/shell cmd helper method names residual pack wave582: {}",
            r.detail
        );
        assert!(
            r.host_enqueue_shell_cmd_helper_nav_commands_wave582_ok,
            "host enqueue/shell cmd helper nav commands residual pack wave582: {}",
            r.detail
        );
        assert!(
            r.host_enqueue_shell_cmd_helper_live_wave582_ok,
            "host enqueue/shell cmd helper live residual wave582: {}",
            r.detail
        );
        assert!(
            r.host_runtime_cmd_helper_method_names_wave583_ok,
            "host runtime cmd helper method names residual pack wave583: {}",
            r.detail
        );
        assert!(
            r.host_runtime_cmd_helper_nav_commands_wave583_ok,
            "host runtime cmd helper nav commands residual pack wave583: {}",
            r.detail
        );
        assert!(
            r.host_runtime_cmd_helper_live_wave583_ok,
            "host runtime cmd helper live residual wave583: {}",
            r.detail
        );
        assert!(
            r.host_tick_mutation_helper_method_names_wave584_ok,
            "host tick/mutation helper method names residual pack wave584: {}",
            r.detail
        );
        assert!(
            r.host_tick_mutation_helper_nav_commands_wave584_ok,
            "host tick/mutation helper nav commands residual pack wave584: {}",
            r.detail
        );
        assert!(
            r.host_tick_mutation_helper_live_wave584_ok,
            "host tick/mutation helper live residual wave584: {}",
            r.detail
        );
        assert!(
            r.host_ui_shell_world_helper_method_names_wave585_ok,
            "host UI/shell/world helper method names residual pack wave585: {}",
            r.detail
        );
        assert!(
            r.host_ui_shell_world_helper_nav_commands_wave585_ok,
            "host UI/shell/world helper nav commands residual pack wave585: {}",
            r.detail
        );
        assert!(
            r.host_ui_shell_world_helper_live_wave585_ok,
            "host UI/shell/world helper live residual wave585: {}",
            r.detail
        );
        assert!(
            r.host_game_client_shell_tick_helper_method_names_wave586_ok,
            "host GameClient shell tick helper method names residual pack wave586: {}",
            r.detail
        );
        assert!(
            r.host_game_client_shell_tick_helper_nav_commands_wave586_ok,
            "host GameClient shell tick helper nav commands residual pack wave586: {}",
            r.detail
        );
        assert!(
            r.host_game_client_shell_tick_helper_live_wave586_ok,
            "host GameClient shell tick helper live residual wave586: {}",
            r.detail
        );
        assert!(
            r.host_game_client_device_tick_helper_method_names_wave587_ok,
            "host GameClient device tick helper method names residual pack wave587: {}",
            r.detail
        );
        assert!(
            r.host_game_client_device_tick_helper_nav_commands_wave587_ok,
            "host GameClient device tick helper nav commands residual pack wave587: {}",
            r.detail
        );
        assert!(
            r.host_game_client_device_tick_helper_live_wave587_ok,
            "host GameClient device tick helper live residual wave587: {}",
            r.detail
        );
        assert!(
            r.host_game_client_menu_shell_helper_method_names_wave588_ok,
            "host GameClient menu shell helper method names residual pack wave588: {}",
            r.detail
        );
        assert!(
            r.host_game_client_menu_shell_helper_nav_commands_wave588_ok,
            "host GameClient menu shell helper nav commands residual pack wave588: {}",
            r.detail
        );
        assert!(
            r.host_game_client_menu_shell_helper_live_wave588_ok,
            "host GameClient menu shell helper live residual wave588: {}",
            r.detail
        );
        assert!(
            r.host_presentation_finalize_helper_method_names_wave589_ok,
            "host presentation finalize helper method names residual pack wave589: {}",
            r.detail
        );
        assert!(
            r.host_presentation_finalize_helper_nav_commands_wave589_ok,
            "host presentation finalize helper nav commands residual pack wave589: {}",
            r.detail
        );
        assert!(
            r.host_presentation_finalize_helper_live_wave589_ok,
            "host presentation finalize helper live residual wave589: {}",
            r.detail
        );
        assert!(
            r.host_presentation_seed_helper_method_names_wave590_ok,
            "host presentation seed helper method names residual pack wave590: {}",
            r.detail
        );
        assert!(
            r.host_presentation_seed_helper_nav_commands_wave590_ok,
            "host presentation seed helper nav commands residual pack wave590: {}",
            r.detail
        );
        assert!(
            r.host_presentation_seed_helper_live_wave590_ok,
            "host presentation seed helper live residual wave590: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_presentation_helper_method_names_wave591_ok,
            "host render UI presentation helper method names residual pack wave591: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_presentation_helper_nav_commands_wave591_ok,
            "host render UI presentation helper nav commands residual pack wave591: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_presentation_helper_live_wave591_ok,
            "host render UI presentation helper live residual wave591: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_overlays_helper_method_names_wave592_ok,
            "host render UI overlays helper method names residual pack wave592: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_overlays_helper_nav_commands_wave592_ok,
            "host render UI overlays helper nav commands residual pack wave592: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_overlays_helper_live_wave592_ok,
            "host render UI overlays helper live residual wave592: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_finalize_helper_method_names_wave593_ok,
            "host render UI finalize helper method names residual pack wave593: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_finalize_helper_nav_commands_wave593_ok,
            "host render UI finalize helper nav commands residual pack wave593: {}",
            r.detail
        );
        assert!(
            r.host_render_ui_finalize_helper_live_wave593_ok,
            "host render UI finalize helper live residual wave593: {}",
            r.detail
        );
        assert!(
            r.host_minimap_bounds_repair_helper_method_names_wave594_ok,
            "host minimap bounds repair helper method names residual pack wave594: {}",
            r.detail
        );
        assert!(
            r.host_minimap_bounds_repair_helper_nav_commands_wave594_ok,
            "host minimap bounds repair helper nav commands residual pack wave594: {}",
            r.detail
        );
        assert!(
            r.host_minimap_bounds_repair_helper_live_wave594_ok,
            "host minimap bounds repair helper live residual wave594: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_apply_helper_method_names_wave595_ok,
            "host production complete apply helper method names residual pack wave595: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_apply_helper_nav_commands_wave595_ok,
            "host production complete apply helper nav commands residual pack wave595: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_apply_helper_live_wave595_ok,
            "host production complete apply helper live residual wave595: {}",
            r.detail
        );
        assert!(
            r.host_camera_queue_drain_helper_method_names_wave596_ok,
            "host camera queue drain helper method names residual pack wave596: {}",
            r.detail
        );
        assert!(
            r.host_camera_queue_drain_helper_nav_commands_wave596_ok,
            "host camera queue drain helper nav commands residual pack wave596: {}",
            r.detail
        );
        assert!(
            r.host_camera_queue_drain_helper_live_wave596_ok,
            "host camera queue drain helper live residual wave596: {}",
            r.detail
        );
        assert!(
            r.host_gameworld_shadow_session_helper_method_names_wave597_ok,
            "host gameworld shadow session helper method names residual pack wave597: {}",
            r.detail
        );
        assert!(
            r.host_gameworld_shadow_session_helper_nav_commands_wave597_ok,
            "host gameworld shadow session helper nav commands residual pack wave597: {}",
            r.detail
        );
        assert!(
            r.host_gameworld_shadow_session_helper_live_wave597_ok,
            "host gameworld shadow session helper live residual wave597: {}",
            r.detail
        );
        assert!(
            r.host_ingame_hud_helper_method_names_wave598_ok,
            "host ingame hud helper method names residual pack wave598: {}",
            r.detail
        );
        assert!(
            r.host_ingame_hud_helper_nav_commands_wave598_ok,
            "host ingame hud helper nav commands residual pack wave598: {}",
            r.detail
        );
        assert!(
            r.host_ingame_hud_helper_live_wave598_ok,
            "host ingame hud helper live residual wave598: {}",
            r.detail
        );
        assert!(
            r.host_match_outcome_helper_method_names_wave599_ok,
            "host match outcome helper method names residual pack wave599: {}",
            r.detail
        );
        assert!(
            r.host_match_outcome_helper_nav_commands_wave599_ok,
            "host match outcome helper nav commands residual pack wave599: {}",
            r.detail
        );
        assert!(
            r.host_match_outcome_helper_live_wave599_ok,
            "host match outcome helper live residual wave599: {}",
            r.detail
        );
        assert!(
            r.host_post_presentation_client_helper_method_names_wave600_ok,
            "host post presentation client helper method names residual pack wave600: {}",
            r.detail
        );
        assert!(
            r.host_post_presentation_client_helper_nav_commands_wave600_ok,
            "host post presentation client helper nav commands residual pack wave600: {}",
            r.detail
        );
        assert!(
            r.host_post_presentation_client_helper_live_wave600_ok,
            "host post presentation client helper live residual wave600: {}",
            r.detail
        );
        assert!(
            r.host_restart_pause_helper_method_names_wave601_ok,
            "host restart pause helper method names residual pack wave601: {}",
            r.detail
        );
        assert!(
            r.host_restart_pause_helper_nav_commands_wave601_ok,
            "host restart pause helper nav commands residual pack wave601: {}",
            r.detail
        );
        assert!(
            r.host_restart_pause_helper_live_wave601_ok,
            "host restart pause helper live residual wave601: {}",
            r.detail
        );
        assert!(
            r.host_ingame_logic_shell_helper_method_names_wave602_ok,
            "host ingame logic shell helper method names residual pack wave602: {}",
            r.detail
        );
        assert!(
            r.host_ingame_logic_shell_helper_nav_commands_wave602_ok,
            "host ingame logic shell helper nav commands residual pack wave602: {}",
            r.detail
        );
        assert!(
            r.host_ingame_logic_shell_helper_live_wave602_ok,
            "host ingame logic shell helper live residual wave602: {}",
            r.detail
        );
        assert!(
            r.host_paused_endgame_boot_ui_helper_method_names_wave603_ok,
            "host paused endgame boot ui helper method names residual pack wave603: {}",
            r.detail
        );
        assert!(
            r.host_paused_endgame_boot_ui_helper_nav_commands_wave603_ok,
            "host paused endgame boot ui helper nav commands residual pack wave603: {}",
            r.detail
        );
        assert!(
            r.host_paused_endgame_boot_ui_helper_live_wave603_ok,
            "host paused endgame boot ui helper live residual wave603: {}",
            r.detail
        );
        assert!(
            r.host_loading_sfx_helper_method_names_wave604_ok,
            "host loading sfx helper method names residual pack wave604: {}",
            r.detail
        );
        assert!(
            r.host_loading_sfx_helper_nav_commands_wave604_ok,
            "host loading sfx helper nav commands residual pack wave604: {}",
            r.detail
        );
        assert!(
            r.host_loading_sfx_helper_live_wave604_ok,
            "host loading sfx helper live residual wave604: {}",
            r.detail
        );
        assert!(
            r.host_menu_client_helper_method_names_wave605_ok,
            "host menu client helper method names residual pack wave605: {}",
            r.detail
        );
        assert!(
            r.host_menu_client_helper_nav_commands_wave605_ok,
            "host menu client helper nav commands residual pack wave605: {}",
            r.detail
        );
        assert!(
            r.host_menu_client_helper_live_wave605_ok,
            "host menu client helper live residual wave605: {}",
            r.detail
        );
        assert!(
            r.host_os_inject_presentation_notify_helper_method_names_wave606_ok,
            "host os inject presentation notify helper method names residual pack wave606: {}",
            r.detail
        );
        assert!(
            r.host_os_inject_presentation_notify_helper_nav_commands_wave606_ok,
            "host os inject presentation notify helper nav commands residual pack wave606: {}",
            r.detail
        );
        assert!(
            r.host_os_inject_presentation_notify_helper_live_wave606_ok,
            "host os inject presentation notify helper live residual wave606: {}",
            r.detail
        );
        assert!(
            r.host_ui_presentation_drain_helper_method_names_wave607_ok,
            "host ui presentation drain helper method names residual pack wave607: {}",
            r.detail
        );
        assert!(
            r.host_ui_presentation_drain_helper_nav_commands_wave607_ok,
            "host ui presentation drain helper nav commands residual pack wave607: {}",
            r.detail
        );
        assert!(
            r.host_ui_presentation_drain_helper_live_wave607_ok,
            "host ui presentation drain helper live residual wave607: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_host_apply_helper_method_names_wave608_ok,
            "host production complete host apply helper method names residual pack wave608: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_host_apply_helper_nav_commands_wave608_ok,
            "host production complete host apply helper nav commands residual pack wave608: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_host_apply_helper_live_wave608_ok,
            "host production complete host apply helper live residual wave608: {}",
            r.detail
        );
        assert!(
            r.host_ui_economy_mouse_mode_helper_method_names_wave609_ok,
            "host ui economy mouse mode helper method names residual pack wave609: {}",
            r.detail
        );
        assert!(
            r.host_ui_economy_mouse_mode_helper_nav_commands_wave609_ok,
            "host ui economy mouse mode helper nav commands residual pack wave609: {}",
            r.detail
        );
        assert!(
            r.host_ui_economy_mouse_mode_helper_live_wave609_ok,
            "host ui economy mouse mode helper live residual wave609: {}",
            r.detail
        );
        assert!(
            r.host_ui_selection_startup_helper_method_names_wave610_ok,
            "host ui selection startup helper method names residual pack wave610: {}",
            r.detail
        );
        assert!(
            r.host_ui_selection_startup_helper_nav_commands_wave610_ok,
            "host ui selection startup helper nav commands residual pack wave610: {}",
            r.detail
        );
        assert!(
            r.host_ui_selection_startup_helper_live_wave610_ok,
            "host ui selection startup helper live residual wave610: {}",
            r.detail
        );
        assert!(
            r.host_start_save_load_helper_method_names_wave611_ok,
            "host start save load helper method names residual pack wave611: {}",
            r.detail
        );
        assert!(
            r.host_start_save_load_helper_nav_commands_wave611_ok,
            "host start save load helper nav commands residual pack wave611: {}",
            r.detail
        );
        assert!(
            r.host_start_save_load_helper_live_wave611_ok,
            "host start save load helper live residual wave611: {}",
            r.detail
        );
        assert!(
            r.host_combat_cursor_transition_helper_method_names_wave612_ok,
            "host combat cursor transition helper method names residual pack wave612: {}",
            r.detail
        );
        assert!(
            r.host_combat_cursor_transition_helper_nav_commands_wave612_ok,
            "host combat cursor transition helper nav commands residual pack wave612: {}",
            r.detail
        );
        assert!(
            r.host_combat_cursor_transition_helper_live_wave612_ok,
            "host combat cursor transition helper live residual wave612: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_collect_helper_method_names_wave613_ok,
            "host production complete collect helper method names residual pack wave613: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_collect_helper_nav_commands_wave613_ok,
            "host production complete collect helper nav commands residual pack wave613: {}",
            r.detail
        );
        assert!(
            r.host_production_complete_collect_helper_live_wave613_ok,
            "host production complete collect helper live residual wave613: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_log_helper_method_names_wave614_ok,
            "host production ready log helper method names residual pack wave614: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_log_helper_nav_commands_wave614_ok,
            "host production ready log helper nav commands residual pack wave614: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_log_helper_live_wave614_ok,
            "host production ready log helper live residual wave614: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_helper_method_names_wave615_ok,
            "host production spawn helper method names residual pack wave615: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_helper_nav_commands_wave615_ok,
            "host production spawn helper nav commands residual pack wave615: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_helper_live_wave615_ok,
            "host production spawn helper live residual wave615: {}",
            r.detail
        );
        assert!(
            r.ai_attack_recheck_production_authority_chain_method_names_wave616_ok,
            "ai attack recheck production authority chain method names residual pack wave616: {}",
            r.detail
        );
        assert!(
            r.ai_attack_recheck_production_authority_chain_nav_commands_wave616_ok,
            "ai attack recheck production authority chain nav commands residual pack wave616: {}",
            r.detail
        );
        assert!(
            r.ai_attack_recheck_production_authority_chain_live_wave616_ok,
            "ai attack recheck production authority chain live residual wave616: {}",
            r.detail
        );
        assert!(
            r.host_construction_ready_log_helper_method_names_wave617_ok,
            "host construction ready log helper method names residual pack wave617: {}",
            r.detail
        );
        assert!(
            r.host_construction_ready_log_helper_nav_commands_wave617_ok,
            "host construction ready log helper nav commands residual pack wave617: {}",
            r.detail
        );
        assert!(
            r.host_construction_ready_log_helper_live_wave617_ok,
            "host construction ready log helper live residual wave617: {}",
            r.detail
        );
        assert!(
            r.host_special_power_ready_log_helper_method_names_wave618_ok,
            "host special power ready log helper method names residual pack wave618: {}",
            r.detail
        );
        assert!(
            r.host_special_power_ready_log_helper_nav_commands_wave618_ok,
            "host special power ready log helper nav commands residual pack wave618: {}",
            r.detail
        );
        assert!(
            r.host_special_power_ready_log_helper_live_wave618_ok,
            "host special power ready log helper live residual wave618: {}",
            r.detail
        );
        assert!(
            r.host_sell_ready_log_helper_method_names_wave619_ok,
            "host sell ready log helper method names residual pack wave619: {}",
            r.detail
        );
        assert!(
            r.host_sell_ready_log_helper_nav_commands_wave619_ok,
            "host sell ready log helper nav commands residual pack wave619: {}",
            r.detail
        );
        assert!(
            r.host_sell_ready_log_helper_live_wave619_ok,
            "host sell ready log helper live residual wave619: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_ready_log_helper_method_names_wave620_ok,
            "host rebuild ready log helper method names residual pack wave620: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_ready_log_helper_nav_commands_wave620_ok,
            "host rebuild ready log helper nav commands residual pack wave620: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_ready_log_helper_live_wave620_ok,
            "host rebuild ready log helper live residual wave620: {}",
            r.detail
        );
        assert!(
            r.host_destroy_ready_log_helper_method_names_wave621_ok,
            "host destroy ready log helper method names residual pack wave621: {}",
            r.detail
        );
        assert!(
            r.host_destroy_ready_log_helper_nav_commands_wave621_ok,
            "host destroy ready log helper nav commands residual pack wave621: {}",
            r.detail
        );
        assert!(
            r.host_destroy_ready_log_helper_live_wave621_ok,
            "host destroy ready log helper live residual wave621: {}",
            r.detail
        );
        assert!(
            r.host_veterancy_ready_log_helper_method_names_wave622_ok,
            "host veterancy ready log helper method names residual pack wave622: {}",
            r.detail
        );
        assert!(
            r.host_veterancy_ready_log_helper_nav_commands_wave622_ok,
            "host veterancy ready log helper nav commands residual pack wave622: {}",
            r.detail
        );
        assert!(
            r.host_veterancy_ready_log_helper_live_wave622_ok,
            "host veterancy ready log helper live residual wave622: {}",
            r.detail
        );
        assert!(
            r.host_body_damage_ready_log_helper_method_names_wave623_ok,
            "host body damage ready log helper method names residual pack wave623: {}",
            r.detail
        );
        assert!(
            r.host_body_damage_ready_log_helper_nav_commands_wave623_ok,
            "host body damage ready log helper nav commands residual pack wave623: {}",
            r.detail
        );
        assert!(
            r.host_body_damage_ready_log_helper_live_wave623_ok,
            "host body damage ready log helper live residual wave623: {}",
            r.detail
        );
        assert!(
            r.host_upgrade_ready_log_helper_method_names_wave624_ok,
            "host upgrade ready log helper method names residual pack wave624: {}",
            r.detail
        );
        assert!(
            r.host_upgrade_ready_log_helper_nav_commands_wave624_ok,
            "host upgrade ready log helper nav commands residual pack wave624: {}",
            r.detail
        );
        assert!(
            r.host_upgrade_ready_log_helper_live_wave624_ok,
            "host upgrade ready log helper live residual wave624: {}",
            r.detail
        );
        assert!(
            r.host_radar_extend_ready_log_helper_method_names_wave625_ok,
            "host radar extend ready log helper method names residual pack wave625: {}",
            r.detail
        );
        assert!(
            r.host_radar_extend_ready_log_helper_nav_commands_wave625_ok,
            "host radar extend ready log helper nav commands residual pack wave625: {}",
            r.detail
        );
        assert!(
            r.host_radar_extend_ready_log_helper_live_wave625_ok,
            "host radar extend ready log helper live residual wave625: {}",
            r.detail
        );
        assert!(
            r.host_construction_complete_clear_ready_log_helper_method_names_wave626_ok,
            "host construction complete clear ready log helper method names residual pack wave626: {}",
            r.detail
        );
        assert!(
            r.host_construction_complete_clear_ready_log_helper_nav_commands_wave626_ok,
            "host construction complete clear ready log helper nav commands residual pack wave626: {}",
            r.detail
        );
        assert!(
            r.host_construction_complete_clear_ready_log_helper_live_wave626_ok,
            "host construction complete clear ready log helper live residual wave626: {}",
            r.detail
        );
        assert!(
            r.host_production_door_ready_log_helper_method_names_wave627_ok,
            "host production door ready log helper method names residual pack wave627: {}",
            r.detail
        );
        assert!(
            r.host_production_door_ready_log_helper_nav_commands_wave627_ok,
            "host production door ready log helper nav commands residual pack wave627: {}",
            r.detail
        );
        assert!(
            r.host_production_door_ready_log_helper_live_wave627_ok,
            "host production door ready log helper live residual wave627: {}",
            r.detail
        );
        assert!(
            r.host_contain_ready_log_helper_method_names_wave628_ok,
            "host contain ready log helper method names residual pack wave628: {}",
            r.detail
        );
        assert!(
            r.host_contain_ready_log_helper_nav_commands_wave628_ok,
            "host contain ready log helper nav commands residual pack wave628: {}",
            r.detail
        );
        assert!(
            r.host_contain_ready_log_helper_live_wave628_ok,
            "host contain ready log helper live residual wave628: {}",
            r.detail
        );
        assert!(
            r.host_owner_ready_log_helper_method_names_wave629_ok,
            "host owner ready log helper method names residual pack wave629: {}",
            r.detail
        );
        assert!(
            r.host_owner_ready_log_helper_nav_commands_wave629_ok,
            "host owner ready log helper nav commands residual pack wave629: {}",
            r.detail
        );
        assert!(
            r.host_owner_ready_log_helper_live_wave629_ok,
            "host owner ready log helper live residual wave629: {}",
            r.detail
        );
        assert!(
            r.host_ai_state_ready_log_helper_method_names_wave630_ok,
            "host ai state ready log helper method names residual pack wave630: {}",
            r.detail
        );
        assert!(
            r.host_ai_state_ready_log_helper_nav_commands_wave630_ok,
            "host ai state ready log helper nav commands residual pack wave630: {}",
            r.detail
        );
        assert!(
            r.host_ai_state_ready_log_helper_live_wave630_ok,
            "host ai state ready log helper live residual wave630: {}",
            r.detail
        );
        assert!(
            r.host_economy_ready_log_helper_method_names_wave631_ok,
            "host economy ready log helper method names residual pack wave631: {}",
            r.detail
        );
        assert!(
            r.host_economy_ready_log_helper_nav_commands_wave631_ok,
            "host economy ready log helper nav commands residual pack wave631: {}",
            r.detail
        );
        assert!(
            r.host_economy_ready_log_helper_live_wave631_ok,
            "host economy ready log helper live residual wave631: {}",
            r.detail
        );
        assert!(
            r.host_death_type_ready_log_helper_method_names_wave632_ok,
            "host death type ready log helper method names residual pack wave632: {}",
            r.detail
        );
        assert!(
            r.host_death_type_ready_log_helper_nav_commands_wave632_ok,
            "host death type ready log helper nav commands residual pack wave632: {}",
            r.detail
        );
        assert!(
            r.host_death_type_ready_log_helper_live_wave632_ok,
            "host death type ready log helper live residual wave632: {}",
            r.detail
        );
        assert!(
            r.host_model_condition_ready_log_helper_method_names_wave633_ok,
            "host model condition ready log helper method names residual pack wave633: {}",
            r.detail
        );
        assert!(
            r.host_model_condition_ready_log_helper_nav_commands_wave633_ok,
            "host model condition ready log helper nav commands residual pack wave633: {}",
            r.detail
        );
        assert!(
            r.host_model_condition_ready_log_helper_live_wave633_ok,
            "host model condition ready log helper live residual wave633: {}",
            r.detail
        );
        assert!(
            r.host_combat_status_ready_log_helper_method_names_wave634_ok,
            "host combat status ready log helper method names residual pack wave634: {}",
            r.detail
        );
        assert!(
            r.host_combat_status_ready_log_helper_nav_commands_wave634_ok,
            "host combat status ready log helper nav commands residual pack wave634: {}",
            r.detail
        );
        assert!(
            r.host_combat_status_ready_log_helper_live_wave634_ok,
            "host combat status ready log helper live residual wave634: {}",
            r.detail
        );
        assert!(
            r.host_weapon_stats_ready_log_helper_method_names_wave635_ok,
            "host weapon stats ready log helper method names residual pack wave635: {}",
            r.detail
        );
        assert!(
            r.host_weapon_stats_ready_log_helper_nav_commands_wave635_ok,
            "host weapon stats ready log helper nav commands residual pack wave635: {}",
            r.detail
        );
        assert!(
            r.host_weapon_stats_ready_log_helper_live_wave635_ok,
            "host weapon stats ready log helper live residual wave635: {}",
            r.detail
        );
        assert!(
            r.host_transform_ready_log_helper_method_names_wave636_ok,
            "host transform ready log helper method names residual pack wave636: {}",
            r.detail
        );
        assert!(
            r.host_transform_ready_log_helper_nav_commands_wave636_ok,
            "host transform ready log helper nav commands residual pack wave636: {}",
            r.detail
        );
        assert!(
            r.host_transform_ready_log_helper_live_wave636_ok,
            "host transform ready log helper live residual wave636: {}",
            r.detail
        );
        assert!(
            r.host_movement_ready_log_helper_method_names_wave637_ok,
            "host movement ready log helper method names residual pack wave637: {}",
            r.detail
        );
        assert!(
            r.host_movement_ready_log_helper_nav_commands_wave637_ok,
            "host movement ready log helper nav commands residual pack wave637: {}",
            r.detail
        );
        assert!(
            r.host_movement_ready_log_helper_live_wave637_ok,
            "host movement ready log helper live residual wave637: {}",
            r.detail
        );
        assert!(
            r.host_attack_target_ready_log_helper_method_names_wave638_ok,
            "host attack target ready log helper method names residual pack wave638: {}",
            r.detail
        );
        assert!(
            r.host_attack_target_ready_log_helper_nav_commands_wave638_ok,
            "host attack target ready log helper nav commands residual pack wave638: {}",
            r.detail
        );
        assert!(
            r.host_attack_target_ready_log_helper_live_wave638_ok,
            "host attack target ready log helper live residual wave638: {}",
            r.detail
        );
        assert!(
            r.host_move_target_ready_log_helper_method_names_wave639_ok,
            "host move target ready log helper method names residual pack wave639: {}",
            r.detail
        );
        assert!(
            r.host_move_target_ready_log_helper_nav_commands_wave639_ok,
            "host move target ready log helper nav commands residual pack wave639: {}",
            r.detail
        );
        assert!(
            r.host_move_target_ready_log_helper_live_wave639_ok,
            "host move target ready log helper live residual wave639: {}",
            r.detail
        );
        assert!(
            r.host_fire_intent_ready_log_helper_method_names_wave640_ok,
            "host fire intent ready log helper method names residual pack wave640: {}",
            r.detail
        );
        assert!(
            r.host_fire_intent_ready_log_helper_nav_commands_wave640_ok,
            "host fire intent ready log helper nav commands residual pack wave640: {}",
            r.detail
        );
        assert!(
            r.host_fire_intent_ready_log_helper_live_wave640_ok,
            "host fire intent ready log helper live residual wave640: {}",
            r.detail
        );
        assert!(
            r.host_stored_supplies_ready_log_helper_method_names_wave641_ok,
            "host stored supplies ready log helper method names residual pack wave641: {}",
            r.detail
        );
        assert!(
            r.host_stored_supplies_ready_log_helper_nav_commands_wave641_ok,
            "host stored supplies ready log helper nav commands residual pack wave641: {}",
            r.detail
        );
        assert!(
            r.host_stored_supplies_ready_log_helper_live_wave641_ok,
            "host stored supplies ready log helper live residual wave641: {}",
            r.detail
        );
        assert!(
            r.host_weapon_set_ready_log_helper_method_names_wave642_ok,
            "host weapon set ready log helper method names residual pack wave642: {}",
            r.detail
        );
        assert!(
            r.host_weapon_set_ready_log_helper_nav_commands_wave642_ok,
            "host weapon set ready log helper nav commands residual pack wave642: {}",
            r.detail
        );
        assert!(
            r.host_weapon_set_ready_log_helper_live_wave642_ok,
            "host weapon set ready log helper live residual wave642: {}",
            r.detail
        );
        assert!(
            r.host_combat_attack_ready_log_helper_method_names_wave643_ok,
            "host combat attack ready log helper method names residual pack wave643: {}",
            r.detail
        );
        assert!(
            r.host_combat_attack_ready_log_helper_nav_commands_wave643_ok,
            "host combat attack ready log helper nav commands residual pack wave643: {}",
            r.detail
        );
        assert!(
            r.host_combat_attack_ready_log_helper_live_wave643_ok,
            "host combat attack ready log helper live residual wave643: {}",
            r.detail
        );
        assert!(
            r.host_command_set_ready_log_helper_method_names_wave644_ok,
            "host command set ready log helper method names residual pack wave644: {}",
            r.detail
        );
        assert!(
            r.host_command_set_ready_log_helper_nav_commands_wave644_ok,
            "host command set ready log helper nav commands residual pack wave644: {}",
            r.detail
        );
        assert!(
            r.host_command_set_ready_log_helper_live_wave644_ok,
            "host command set ready log helper live residual wave644: {}",
            r.detail
        );
        assert!(
            r.host_ai_mood_ready_log_helper_method_names_wave645_ok,
            "host ai mood ready log helper method names residual pack wave645: {}",
            r.detail
        );
        assert!(
            r.host_ai_mood_ready_log_helper_nav_commands_wave645_ok,
            "host ai mood ready log helper nav commands residual pack wave645: {}",
            r.detail
        );
        assert!(
            r.host_ai_mood_ready_log_helper_live_wave645_ok,
            "host ai mood ready log helper live residual wave645: {}",
            r.detail
        );
        assert!(
            r.host_locomotor_ready_log_helper_method_names_wave646_ok,
            "host locomotor ready log helper method names residual pack wave646: {}",
            r.detail
        );
        assert!(
            r.host_locomotor_ready_log_helper_nav_commands_wave646_ok,
            "host locomotor ready log helper nav commands residual pack wave646: {}",
            r.detail
        );
        assert!(
            r.host_locomotor_ready_log_helper_live_wave646_ok,
            "host locomotor ready log helper live residual wave646: {}",
            r.detail
        );
        assert!(
            r.host_hijacker_ready_log_helper_method_names_wave647_ok,
            "host hijacker ready log helper method names residual pack wave647: {}",
            r.detail
        );
        assert!(
            r.host_hijacker_ready_log_helper_nav_commands_wave647_ok,
            "host hijacker ready log helper nav commands residual pack wave647: {}",
            r.detail
        );
        assert!(
            r.host_hijacker_ready_log_helper_live_wave647_ok,
            "host hijacker ready log helper live residual wave647: {}",
            r.detail
        );
        assert!(
            r.host_ai_request_ready_log_helper_method_names_wave648_ok,
            "host ai request ready log helper method names residual pack wave648: {}",
            r.detail
        );
        assert!(
            r.host_ai_request_ready_log_helper_nav_commands_wave648_ok,
            "host ai request ready log helper nav commands residual pack wave648: {}",
            r.detail
        );
        assert!(
            r.host_ai_request_ready_log_helper_live_wave648_ok,
            "host ai request ready log helper live residual wave648: {}",
            r.detail
        );
        assert!(
            r.host_physics_motive_ready_log_helper_method_names_wave649_ok,
            "host physics_motive ready log helper method names residual pack wave649: {}",
            r.detail
        );
        assert!(
            r.host_physics_motive_ready_log_helper_nav_commands_wave649_ok,
            "host physics_motive ready log helper nav commands residual pack wave649: {}",
            r.detail
        );
        assert!(
            r.host_physics_motive_ready_log_helper_live_wave649_ok,
            "host physics_motive ready log helper live residual wave649: {}",
            r.detail
        );
        assert!(
            r.host_bounce_land_ready_log_helper_method_names_wave650_ok,
            "host bounce_land ready log helper method names residual pack wave650: {}",
            r.detail
        );
        assert!(
            r.host_bounce_land_ready_log_helper_nav_commands_wave650_ok,
            "host bounce_land ready log helper nav commands residual pack wave650: {}",
            r.detail
        );
        assert!(
            r.host_bounce_land_ready_log_helper_live_wave650_ok,
            "host bounce_land ready log helper live residual wave650: {}",
            r.detail
        );
        assert!(
            r.host_stealth_delay_ready_log_helper_method_names_wave651_ok,
            "host stealth_delay ready log helper method names residual pack wave651: {}",
            r.detail
        );
        assert!(
            r.host_stealth_delay_ready_log_helper_nav_commands_wave651_ok,
            "host stealth_delay ready log helper nav commands residual pack wave651: {}",
            r.detail
        );
        assert!(
            r.host_stealth_delay_ready_log_helper_live_wave651_ok,
            "host stealth_delay ready log helper live residual wave651: {}",
            r.detail
        );
        assert!(
            r.host_stealth_flags_ready_log_helper_method_names_wave652_ok,
            "host stealth_flags ready log helper method names residual pack wave652: {}",
            r.detail
        );
        assert!(
            r.host_stealth_flags_ready_log_helper_nav_commands_wave652_ok,
            "host stealth_flags ready log helper nav commands residual pack wave652: {}",
            r.detail
        );
        assert!(
            r.host_stealth_flags_ready_log_helper_live_wave652_ok,
            "host stealth_flags ready log helper live residual wave652: {}",
            r.detail
        );
        assert!(
            r.host_disguise_ready_log_helper_method_names_wave653_ok,
            "host disguise ready log helper method names residual pack wave653: {}",
            r.detail
        );
        assert!(
            r.host_disguise_ready_log_helper_nav_commands_wave653_ok,
            "host disguise ready log helper nav commands residual pack wave653: {}",
            r.detail
        );
        assert!(
            r.host_disguise_ready_log_helper_live_wave653_ok,
            "host disguise ready log helper live residual wave653: {}",
            r.detail
        );
        assert!(
            r.host_vision_camo_ready_log_helper_method_names_wave654_ok,
            "host vision_camo ready log helper method names residual pack wave654: {}",
            r.detail
        );
        assert!(
            r.host_vision_camo_ready_log_helper_nav_commands_wave654_ok,
            "host vision_camo ready log helper nav commands residual pack wave654: {}",
            r.detail
        );
        assert!(
            r.host_vision_camo_ready_log_helper_live_wave654_ok,
            "host vision_camo ready log helper live residual wave654: {}",
            r.detail
        );
        assert!(
            r.host_selection_radius_ready_log_helper_method_names_wave655_ok,
            "host selection_radius ready log helper method names residual pack wave655: {}",
            r.detail
        );
        assert!(
            r.host_selection_radius_ready_log_helper_nav_commands_wave655_ok,
            "host selection_radius ready log helper nav commands residual pack wave655: {}",
            r.detail
        );
        assert!(
            r.host_selection_radius_ready_log_helper_live_wave655_ok,
            "host selection_radius ready log helper live residual wave655: {}",
            r.detail
        );
        assert!(
            r.host_ground_height_ready_log_helper_method_names_wave656_ok,
            "host ground_height ready log helper method names residual pack wave656: {}",
            r.detail
        );
        assert!(
            r.host_ground_height_ready_log_helper_nav_commands_wave656_ok,
            "host ground_height ready log helper nav commands residual pack wave656: {}",
            r.detail
        );
        assert!(
            r.host_ground_height_ready_log_helper_live_wave656_ok,
            "host ground_height ready log helper live residual wave656: {}",
            r.detail
        );
        assert!(
            r.host_weapon_slot_ready_log_helper_method_names_wave657_ok,
            "host weapon_slot ready log helper method names residual pack wave657: {}",
            r.detail
        );
        assert!(
            r.host_weapon_slot_ready_log_helper_nav_commands_wave657_ok,
            "host weapon_slot ready log helper nav commands residual pack wave657: {}",
            r.detail
        );
        assert!(
            r.host_weapon_slot_ready_log_helper_live_wave657_ok,
            "host weapon_slot ready log helper live residual wave657: {}",
            r.detail
        );
        assert!(
            r.host_weapon_bonus_ready_log_helper_method_names_wave658_ok,
            "host weapon_bonus ready log helper method names residual pack wave658: {}",
            r.detail
        );
        assert!(
            r.host_weapon_bonus_ready_log_helper_nav_commands_wave658_ok,
            "host weapon_bonus ready log helper nav commands residual pack wave658: {}",
            r.detail
        );
        assert!(
            r.host_weapon_bonus_ready_log_helper_live_wave658_ok,
            "host weapon_bonus ready log helper live residual wave658: {}",
            r.detail
        );
        assert!(
            r.host_ai_attitude_ready_log_helper_method_names_wave659_ok,
            "host ai_attitude ready log helper method names residual pack wave659: {}",
            r.detail
        );
        assert!(
            r.host_ai_attitude_ready_log_helper_nav_commands_wave659_ok,
            "host ai_attitude ready log helper nav commands residual pack wave659: {}",
            r.detail
        );
        assert!(
            r.host_ai_attitude_ready_log_helper_live_wave659_ok,
            "host ai_attitude ready log helper live residual wave659: {}",
            r.detail
        );
        assert!(
            r.host_identity_ready_log_helper_method_names_wave660_ok,
            "host identity ready log helper method names residual pack wave660: {}",
            r.detail
        );
        assert!(
            r.host_identity_ready_log_helper_nav_commands_wave660_ok,
            "host identity ready log helper nav commands residual pack wave660: {}",
            r.detail
        );
        assert!(
            r.host_identity_ready_log_helper_live_wave660_ok,
            "host identity ready log helper live residual wave660: {}",
            r.detail
        );
        assert!(
            r.host_repulsor_ready_log_helper_method_names_wave661_ok,
            "host repulsor ready log helper method names residual pack wave661: {}",
            r.detail
        );
        assert!(
            r.host_repulsor_ready_log_helper_nav_commands_wave661_ok,
            "host repulsor ready log helper nav commands residual pack wave661: {}",
            r.detail
        );
        assert!(
            r.host_repulsor_ready_log_helper_live_wave661_ok,
            "host repulsor ready log helper live residual wave661: {}",
            r.detail
        );
        assert!(
            r.host_shock_stun_ready_log_helper_method_names_wave662_ok,
            "host shock_stun ready log helper method names residual pack wave662: {}",
            r.detail
        );
        assert!(
            r.host_shock_stun_ready_log_helper_nav_commands_wave662_ok,
            "host shock_stun ready log helper nav commands residual pack wave662: {}",
            r.detail
        );
        assert!(
            r.host_shock_stun_ready_log_helper_live_wave662_ok,
            "host shock_stun ready log helper live residual wave662: {}",
            r.detail
        );
        assert!(
            r.host_sole_healing_ready_log_helper_method_names_wave663_ok,
            "host sole_healing ready log helper method names residual pack wave663: {}",
            r.detail
        );
        assert!(
            r.host_sole_healing_ready_log_helper_nav_commands_wave663_ok,
            "host sole_healing ready log helper nav commands residual pack wave663: {}",
            r.detail
        );
        assert!(
            r.host_sole_healing_ready_log_helper_live_wave663_ok,
            "host sole_healing ready log helper live residual wave663: {}",
            r.detail
        );
        assert!(
            r.host_crush_vision_ready_log_helper_method_names_wave664_ok,
            "host crush_vision ready log helper method names residual pack wave664: {}",
            r.detail
        );
        assert!(
            r.host_crush_vision_ready_log_helper_nav_commands_wave664_ok,
            "host crush_vision ready log helper nav commands residual pack wave664: {}",
            r.detail
        );
        assert!(
            r.host_crush_vision_ready_log_helper_live_wave664_ok,
            "host crush_vision ready log helper live residual wave664: {}",
            r.detail
        );
        assert!(
            r.host_demo_mine_cheer_ready_log_helper_method_names_wave665_ok,
            "host demo_mine_cheer ready log helper method names residual pack wave665: {}",
            r.detail
        );
        assert!(
            r.host_demo_mine_cheer_ready_log_helper_nav_commands_wave665_ok,
            "host demo_mine_cheer ready log helper nav commands residual pack wave665: {}",
            r.detail
        );
        assert!(
            r.host_demo_mine_cheer_ready_log_helper_live_wave665_ok,
            "host demo_mine_cheer ready log helper live residual wave665: {}",
            r.detail
        );
        assert!(
            r.host_overlord_ready_log_helper_method_names_wave666_ok,
            "host overlord ready log helper method names residual pack wave666: {}",
            r.detail
        );
        assert!(
            r.host_overlord_ready_log_helper_nav_commands_wave666_ok,
            "host overlord ready log helper nav commands residual pack wave666: {}",
            r.detail
        );
        assert!(
            r.host_overlord_ready_log_helper_live_wave666_ok,
            "host overlord ready log helper live residual wave666: {}",
            r.detail
        );
        assert!(
            r.host_hive_ready_log_helper_method_names_wave667_ok,
            "host hive ready log helper method names residual pack wave667: {}",
            r.detail
        );
        assert!(
            r.host_hive_ready_log_helper_nav_commands_wave667_ok,
            "host hive ready log helper nav commands residual pack wave667: {}",
            r.detail
        );
        assert!(
            r.host_hive_ready_log_helper_live_wave667_ok,
            "host hive ready log helper live residual wave667: {}",
            r.detail
        );
        assert!(
            r.host_overcharge_ready_log_helper_method_names_wave668_ok,
            "host overcharge ready log helper method names residual pack wave668: {}",
            r.detail
        );
        assert!(
            r.host_overcharge_ready_log_helper_nav_commands_wave668_ok,
            "host overcharge ready log helper nav commands residual pack wave668: {}",
            r.detail
        );
        assert!(
            r.host_overcharge_ready_log_helper_live_wave668_ok,
            "host overcharge ready log helper live residual wave668: {}",
            r.detail
        );
        assert!(
            r.host_guard_ready_log_helper_method_names_wave669_ok,
            "host guard ready log helper method names residual pack wave669: {}",
            r.detail
        );
        assert!(
            r.host_guard_ready_log_helper_nav_commands_wave669_ok,
            "host guard ready log helper nav commands residual pack wave669: {}",
            r.detail
        );
        assert!(
            r.host_guard_ready_log_helper_live_wave669_ok,
            "host guard ready log helper live residual wave669: {}",
            r.detail
        );
        assert!(
            r.host_continuous_fire_ready_log_helper_method_names_wave670_ok,
            "host continuous_fire ready log helper method names residual pack wave670: {}",
            r.detail
        );
        assert!(
            r.host_continuous_fire_ready_log_helper_nav_commands_wave670_ok,
            "host continuous_fire ready log helper nav commands residual pack wave670: {}",
            r.detail
        );
        assert!(
            r.host_continuous_fire_ready_log_helper_live_wave670_ok,
            "host continuous_fire ready log helper live residual wave670: {}",
            r.detail
        );
        assert!(
            r.host_detector_ready_log_helper_method_names_wave671_ok,
            "host detector ready log helper method names residual pack wave671: {}",
            r.detail
        );
        assert!(
            r.host_detector_ready_log_helper_nav_commands_wave671_ok,
            "host detector ready log helper nav commands residual pack wave671: {}",
            r.detail
        );
        assert!(
            r.host_detector_ready_log_helper_live_wave671_ok,
            "host detector ready log helper live residual wave671: {}",
            r.detail
        );
        assert!(
            r.host_target_location_ready_log_helper_method_names_wave672_ok,
            "host target_location ready log helper method names residual pack wave672: {}",
            r.detail
        );
        assert!(
            r.host_target_location_ready_log_helper_nav_commands_wave672_ok,
            "host target_location ready log helper nav commands residual pack wave672: {}",
            r.detail
        );
        assert!(
            r.host_target_location_ready_log_helper_live_wave672_ok,
            "host target_location ready log helper live residual wave672: {}",
            r.detail
        );
        assert!(
            r.host_turret_ready_log_helper_method_names_wave673_ok,
            "host turret ready log helper method names residual pack wave673: {}",
            r.detail
        );
        assert!(
            r.host_turret_ready_log_helper_nav_commands_wave673_ok,
            "host turret ready log helper nav commands residual pack wave673: {}",
            r.detail
        );
        assert!(
            r.host_turret_ready_log_helper_live_wave673_ok,
            "host turret ready log helper live residual wave673: {}",
            r.detail
        );
        assert!(
            r.host_entity_power_ready_log_helper_method_names_wave674_ok,
            "host entity_power ready log helper method names residual pack wave674: {}",
            r.detail
        );
        assert!(
            r.host_entity_power_ready_log_helper_nav_commands_wave674_ok,
            "host entity_power ready log helper nav commands residual pack wave674: {}",
            r.detail
        );
        assert!(
            r.host_entity_power_ready_log_helper_live_wave674_ok,
            "host entity_power ready log helper live residual wave674: {}",
            r.detail
        );
        assert!(
            r.host_building_type_ready_log_helper_method_names_wave675_ok,
            "host building_type ready log helper method names residual pack wave675: {}",
            r.detail
        );
        assert!(
            r.host_building_type_ready_log_helper_nav_commands_wave675_ok,
            "host building_type ready log helper nav commands residual pack wave675: {}",
            r.detail
        );
        assert!(
            r.host_building_type_ready_log_helper_live_wave675_ok,
            "host building_type ready log helper live residual wave675: {}",
            r.detail
        );
        assert!(
            r.host_faerie_fire_ready_log_helper_method_names_wave676_ok,
            "host faerie_fire ready log helper method names residual pack wave676: {}",
            r.detail
        );
        assert!(
            r.host_faerie_fire_ready_log_helper_nav_commands_wave676_ok,
            "host faerie_fire ready log helper nav commands residual pack wave676: {}",
            r.detail
        );
        assert!(
            r.host_faerie_fire_ready_log_helper_live_wave676_ok,
            "host faerie_fire ready log helper live residual wave676: {}",
            r.detail
        );
        assert!(
            r.host_disable_timers_ready_log_helper_method_names_wave677_ok,
            "host disable_timers ready log helper method names residual pack wave677: {}",
            r.detail
        );
        assert!(
            r.host_disable_timers_ready_log_helper_nav_commands_wave677_ok,
            "host disable_timers ready log helper nav commands residual pack wave677: {}",
            r.detail
        );
        assert!(
            r.host_disable_timers_ready_log_helper_live_wave677_ok,
            "host disable_timers ready log helper live residual wave677: {}",
            r.detail
        );
        assert!(
            r.host_projectiles_ready_log_helper_method_names_wave678_ok,
            "host projectiles ready log helper method names residual pack wave678: {}",
            r.detail
        );
        assert!(
            r.host_projectiles_ready_log_helper_nav_commands_wave678_ok,
            "host projectiles ready log helper nav commands residual pack wave678: {}",
            r.detail
        );
        assert!(
            r.host_projectiles_ready_log_helper_live_wave678_ok,
            "host projectiles ready log helper live residual wave678: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_ready_log_helper_method_names_wave679_ok,
            "host production_spawn ready log helper method names residual pack wave679: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_ready_log_helper_nav_commands_wave679_ok,
            "host production_spawn ready log helper nav commands residual pack wave679: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_ready_log_helper_live_wave679_ok,
            "host production_spawn ready log helper live residual wave679: {}",
            r.detail
        );
        assert!(
            r.host_eager_spawn_map_helper_method_names_wave680_ok,
            "host eager_spawn_map helper method names residual pack wave680: {}",
            r.detail
        );
        assert!(
            r.host_eager_spawn_map_helper_nav_commands_wave680_ok,
            "host eager_spawn_map helper nav commands residual pack wave680: {}",
            r.detail
        );
        assert!(
            r.host_eager_spawn_map_helper_live_wave680_ok,
            "host eager_spawn_map helper live residual wave680: {}",
            r.detail
        );
        assert!(
            r.host_eager_destroy_unmap_helper_method_names_wave681_ok,
            "host eager_destroy_unmap helper method names residual pack wave681: {}",
            r.detail
        );
        assert!(
            r.host_eager_destroy_unmap_helper_nav_commands_wave681_ok,
            "host eager_destroy_unmap helper nav commands residual pack wave681: {}",
            r.detail
        );
        assert!(
            r.host_eager_destroy_unmap_helper_live_wave681_ok,
            "host eager_destroy_unmap helper live residual wave681: {}",
            r.detail
        );
        assert!(
            r.host_eager_fire_spawn_helper_method_names_wave682_ok,
            "host eager_fire_spawn helper method names residual pack wave682: {}",
            r.detail
        );
        assert!(
            r.host_eager_fire_spawn_helper_nav_commands_wave682_ok,
            "host eager_fire_spawn helper nav commands residual pack wave682: {}",
            r.detail
        );
        assert!(
            r.host_eager_fire_spawn_helper_live_wave682_ok,
            "host eager_fire_spawn helper live residual wave682: {}",
            r.detail
        );
        assert!(
            r.host_eager_move_attack_helper_method_names_wave683_ok,
            "host eager_move_attack helper method names residual pack wave683: {}",
            r.detail
        );
        assert!(
            r.host_eager_move_attack_helper_nav_commands_wave683_ok,
            "host eager_move_attack helper nav commands residual pack wave683: {}",
            r.detail
        );
        assert!(
            r.host_eager_move_attack_helper_live_wave683_ok,
            "host eager_move_attack helper live residual wave683: {}",
            r.detail
        );
        assert!(
            r.host_eager_damage_helper_method_names_wave684_ok,
            "host eager_damage helper method names residual pack wave684: {}",
            r.detail
        );
        assert!(
            r.host_eager_damage_helper_nav_commands_wave684_ok,
            "host eager_damage helper nav commands residual pack wave684: {}",
            r.detail
        );
        assert!(
            r.host_eager_damage_helper_live_wave684_ok,
            "host eager_damage helper live residual wave684: {}",
            r.detail
        );
        assert!(
            r.host_eager_heal_helper_method_names_wave685_ok,
            "host eager_heal helper method names residual pack wave685: {}",
            r.detail
        );
        assert!(
            r.host_eager_heal_helper_nav_commands_wave685_ok,
            "host eager_heal helper nav commands residual pack wave685: {}",
            r.detail
        );
        assert!(
            r.host_eager_heal_helper_live_wave685_ok,
            "host eager_heal helper live residual wave685: {}",
            r.detail
        );
        assert!(
            r.host_eager_max_health_xp_helper_method_names_wave686_ok,
            "host eager_max_health_xp helper method names residual pack wave686: {}",
            r.detail
        );
        assert!(
            r.host_eager_max_health_xp_helper_nav_commands_wave686_ok,
            "host eager_max_health_xp helper nav commands residual pack wave686: {}",
            r.detail
        );
        assert!(
            r.host_eager_max_health_xp_helper_live_wave686_ok,
            "host eager_max_health_xp helper live residual wave686: {}",
            r.detail
        );
        assert!(
            r.host_eager_ai_fire_intent_helper_method_names_wave687_ok,
            "host eager_ai_fire_intent helper method names residual pack wave687: {}",
            r.detail
        );
        assert!(
            r.host_eager_ai_fire_intent_helper_nav_commands_wave687_ok,
            "host eager_ai_fire_intent helper nav commands residual pack wave687: {}",
            r.detail
        );
        assert!(
            r.host_eager_ai_fire_intent_helper_live_wave687_ok,
            "host eager_ai_fire_intent helper live residual wave687: {}",
            r.detail
        );
        assert!(
            r.host_eager_owner_movement_helper_method_names_wave688_ok,
            "host eager_owner_movement helper method names residual pack wave688: {}",
            r.detail
        );
        assert!(
            r.host_eager_owner_movement_helper_nav_commands_wave688_ok,
            "host eager_owner_movement helper nav commands residual pack wave688: {}",
            r.detail
        );
        assert!(
            r.host_eager_owner_movement_helper_live_wave688_ok,
            "host eager_owner_movement helper live residual wave688: {}",
            r.detail
        );
        assert!(
            r.host_eager_status_veterancy_helper_method_names_wave689_ok,
            "host eager_status_veterancy helper method names residual pack wave689: {}",
            r.detail
        );
        assert!(
            r.host_eager_status_veterancy_helper_nav_commands_wave689_ok,
            "host eager_status_veterancy helper nav commands residual pack wave689: {}",
            r.detail
        );
        assert!(
            r.host_eager_status_veterancy_helper_live_wave689_ok,
            "host eager_status_veterancy helper live residual wave689: {}",
            r.detail
        );
        assert!(
            r.host_eager_weapon_bonus_slot_helper_method_names_wave690_ok,
            "host eager_weapon_bonus_slot helper method names residual pack wave690: {}",
            r.detail
        );
        assert!(
            r.host_eager_weapon_bonus_slot_helper_nav_commands_wave690_ok,
            "host eager_weapon_bonus_slot helper nav commands residual pack wave690: {}",
            r.detail
        );
        assert!(
            r.host_eager_weapon_bonus_slot_helper_live_wave690_ok,
            "host eager_weapon_bonus_slot helper live residual wave690: {}",
            r.detail
        );
        assert!(
            r.host_eager_weapon_set_power_helper_method_names_wave691_ok,
            "host eager_weapon_set_power helper method names residual pack wave691: {}",
            r.detail
        );
        assert!(
            r.host_eager_weapon_set_power_helper_nav_commands_wave691_ok,
            "host eager_weapon_set_power helper nav commands residual pack wave691: {}",
            r.detail
        );
        assert!(
            r.host_eager_weapon_set_power_helper_live_wave691_ok,
            "host eager_weapon_set_power helper live residual wave691: {}",
            r.detail
        );
        assert!(
            r.host_eager_turret_guard_rally_helper_method_names_wave692_ok,
            "host eager_turret_guard_rally helper method names residual pack wave692: {}",
            r.detail
        );
        assert!(
            r.host_eager_turret_guard_rally_helper_nav_commands_wave692_ok,
            "host eager_turret_guard_rally helper nav commands residual pack wave692: {}",
            r.detail
        );
        assert!(
            r.host_eager_turret_guard_rally_helper_live_wave692_ok,
            "host eager_turret_guard_rally helper live residual wave692: {}",
            r.detail
        );
        assert!(
            r.host_eager_tloc_detector_cf_helper_method_names_wave693_ok,
            "host eager_tloc_detector_cf helper method names residual pack wave693: {}",
            r.detail
        );
        assert!(
            r.host_eager_tloc_detector_cf_helper_nav_commands_wave693_ok,
            "host eager_tloc_detector_cf helper nav commands residual pack wave693: {}",
            r.detail
        );
        assert!(
            r.host_eager_tloc_detector_cf_helper_live_wave693_ok,
            "host eager_tloc_detector_cf helper live residual wave693: {}",
            r.detail
        );
        assert!(
            r.host_eager_attitude_overcharge_stealth_helper_method_names_wave694_ok,
            "host eager_attitude_overcharge_stealth helper method names residual pack wave694: {}",
            r.detail
        );
        assert!(
            r.host_eager_attitude_overcharge_stealth_helper_nav_commands_wave694_ok,
            "host eager_attitude_overcharge_stealth helper nav commands residual pack wave694: {}",
            r.detail
        );
        assert!(
            r.host_eager_attitude_overcharge_stealth_helper_live_wave694_ok,
            "host eager_attitude_overcharge_stealth helper live residual wave694: {}",
            r.detail
        );
        assert!(
            r.host_eager_contain_hive_overlord_helper_method_names_wave695_ok,
            "host eager_contain_hive_overlord helper method names residual pack wave695: {}",
            r.detail
        );
        assert!(
            r.host_eager_contain_hive_overlord_helper_nav_commands_wave695_ok,
            "host eager_contain_hive_overlord helper nav commands residual pack wave695: {}",
            r.detail
        );
        assert!(
            r.host_eager_contain_hive_overlord_helper_live_wave695_ok,
            "host eager_contain_hive_overlord helper live residual wave695: {}",
            r.detail
        );
        assert!(
            r.host_eager_cmdset_disguise_camo_helper_method_names_wave696_ok,
            "host eager_cmdset_disguise_camo helper method names residual pack wave696: {}",
            r.detail
        );
        assert!(
            r.host_eager_cmdset_disguise_camo_helper_nav_commands_wave696_ok,
            "host eager_cmdset_disguise_camo helper nav commands residual pack wave696: {}",
            r.detail
        );
        assert!(
            r.host_eager_cmdset_disguise_camo_helper_live_wave696_ok,
            "host eager_cmdset_disguise_camo helper live residual wave696: {}",
            r.detail
        );
        assert!(
            r.host_eager_wstats_sel_model_helper_method_names_wave697_ok,
            "host eager_wstats_sel_model helper method names residual pack wave697: {}",
            r.detail
        );
        assert!(
            r.host_eager_wstats_sel_model_helper_nav_commands_wave697_ok,
            "host eager_wstats_sel_model helper nav commands residual pack wave697: {}",
            r.detail
        );
        assert!(
            r.host_eager_wstats_sel_model_helper_live_wave697_ok,
            "host eager_wstats_sel_model helper live residual wave697: {}",
            r.detail
        );
        assert!(
            r.host_eager_demo_form_crush_helper_method_names_wave698_ok,
            "host eager_demo_form_crush helper method names residual pack wave698: {}",
            r.detail
        );
        assert!(
            r.host_eager_demo_form_crush_helper_nav_commands_wave698_ok,
            "host eager_demo_form_crush helper nav commands residual pack wave698: {}",
            r.detail
        );
        assert!(
            r.host_eager_demo_form_crush_helper_live_wave698_ok,
            "host eager_demo_form_crush helper live residual wave698: {}",
            r.detail
        );
        assert!(
            r.host_eager_btype_identity_ground_helper_method_names_wave699_ok,
            "host eager_btype_identity_ground helper method names residual pack wave699: {}",
            r.detail
        );
        assert!(
            r.host_eager_btype_identity_ground_helper_nav_commands_wave699_ok,
            "host eager_btype_identity_ground helper nav commands residual pack wave699: {}",
            r.detail
        );
        assert!(
            r.host_eager_btype_identity_ground_helper_live_wave699_ok,
            "host eager_btype_identity_ground helper live residual wave699: {}",
            r.detail
        );
        assert!(
            r.host_eager_mesh_fow_kindof_helper_method_names_wave700_ok,
            "host eager_mesh_fow_kindof helper method names residual pack wave700: {}",
            r.detail
        );
        assert!(
            r.host_eager_mesh_fow_kindof_helper_nav_commands_wave700_ok,
            "host eager_mesh_fow_kindof helper nav commands residual pack wave700: {}",
            r.detail
        );
        assert!(
            r.host_eager_mesh_fow_kindof_helper_live_wave700_ok,
            "host eager_mesh_fow_kindof helper live residual wave700: {}",
            r.detail
        );
        assert!(
            r.host_eager_faerie_repulsor_disable_helper_method_names_wave701_ok,
            "host eager_faerie_repulsor_disable helper method names residual pack wave701: {}",
            r.detail
        );
        assert!(
            r.host_eager_faerie_repulsor_disable_helper_nav_commands_wave701_ok,
            "host eager_faerie_repulsor_disable helper nav commands residual pack wave701: {}",
            r.detail
        );
        assert!(
            r.host_eager_faerie_repulsor_disable_helper_live_wave701_ok,
            "host eager_faerie_repulsor_disable helper live residual wave701: {}",
            r.detail
        );
        assert!(
            r.host_eager_body_death_physics_helper_method_names_wave702_ok,
            "host eager_body_death_physics helper method names residual pack wave702: {}",
            r.detail
        );
        assert!(
            r.host_eager_body_death_physics_helper_nav_commands_wave702_ok,
            "host eager_body_death_physics helper nav commands residual pack wave702: {}",
            r.detail
        );
        assert!(
            r.host_eager_body_death_physics_helper_live_wave702_ok,
            "host eager_body_death_physics helper live residual wave702: {}",
            r.detail
        );
        assert!(
            r.host_eager_loco_bounce_helper_method_names_wave703_ok,
            "host eager_loco_bounce helper method names residual pack wave703: {}",
            r.detail
        );
        assert!(
            r.host_eager_loco_bounce_helper_nav_commands_wave703_ok,
            "host eager_loco_bounce helper nav commands residual pack wave703: {}",
            r.detail
        );
        assert!(
            r.host_eager_loco_bounce_helper_live_wave703_ok,
            "host eager_loco_bounce helper live residual wave703: {}",
            r.detail
        );
        assert!(
            r.host_eager_aimood_request_shock_helper_method_names_wave704_ok,
            "host eager_aimood_request_shock helper method names residual pack wave704: {}",
            r.detail
        );
        assert!(
            r.host_eager_aimood_request_shock_helper_nav_commands_wave704_ok,
            "host eager_aimood_request_shock helper nav commands residual pack wave704: {}",
            r.detail
        );
        assert!(
            r.host_eager_aimood_request_shock_helper_live_wave704_ok,
            "host eager_aimood_request_shock helper live residual wave704: {}",
            r.detail
        );
        assert!(
            r.host_eager_stealth_sole_radar_helper_method_names_wave705_ok,
            "host eager_stealth_sole_radar helper method names residual pack wave705: {}",
            r.detail
        );
        assert!(
            r.host_eager_stealth_sole_radar_helper_nav_commands_wave705_ok,
            "host eager_stealth_sole_radar helper nav commands residual pack wave705: {}",
            r.detail
        );
        assert!(
            r.host_eager_stealth_sole_radar_helper_live_wave705_ok,
            "host eager_stealth_sole_radar helper live residual wave705: {}",
            r.detail
        );
        assert!(
            r.host_eager_hijack_rebuild_supplies_helper_method_names_wave706_ok,
            "host eager_hijack_rebuild_supplies helper method names residual pack wave706: {}",
            r.detail
        );
        assert!(
            r.host_eager_hijack_rebuild_supplies_helper_nav_commands_wave706_ok,
            "host eager_hijack_rebuild_supplies helper nav commands residual pack wave706: {}",
            r.detail
        );
        assert!(
            r.host_eager_hijack_rebuild_supplies_helper_live_wave706_ok,
            "host eager_hijack_rebuild_supplies helper live residual wave706: {}",
            r.detail
        );
        assert!(
            r.host_eager_sp_radar_progress_helper_method_names_wave707_ok,
            "host eager_sp_radar_progress helper method names residual pack wave707: {}",
            r.detail
        );
        assert!(
            r.host_eager_sp_radar_progress_helper_nav_commands_wave707_ok,
            "host eager_sp_radar_progress helper nav commands residual pack wave707: {}",
            r.detail
        );
        assert!(
            r.host_eager_sp_radar_progress_helper_live_wave707_ok,
            "host eager_sp_radar_progress helper live residual wave707: {}",
            r.detail
        );
        assert!(
            r.host_eager_meta_cooldown_door_helper_method_names_wave708_ok,
            "host eager_meta_cooldown_door helper method names residual pack wave708: {}",
            r.detail
        );
        assert!(
            r.host_eager_meta_cooldown_door_helper_nav_commands_wave708_ok,
            "host eager_meta_cooldown_door helper nav commands residual pack wave708: {}",
            r.detail
        );
        assert!(
            r.host_eager_meta_cooldown_door_helper_live_wave708_ok,
            "host eager_meta_cooldown_door helper live residual wave708: {}",
            r.detail
        );
        assert!(
            r.host_eager_prod_construction_helper_method_names_wave709_ok,
            "host eager_prod_construction helper method names residual pack wave709: {}",
            r.detail
        );
        assert!(
            r.host_eager_prod_construction_helper_nav_commands_wave709_ok,
            "host eager_prod_construction helper nav commands residual pack wave709: {}",
            r.detail
        );
        assert!(
            r.host_eager_prod_construction_helper_live_wave709_ok,
            "host eager_prod_construction helper live residual wave709: {}",
            r.detail
        );
        assert!(
            r.host_eager_combat_projectile_helper_method_names_wave710_ok,
            "host eager_combat_projectile helper method names residual pack wave710: {}",
            r.detail
        );
        assert!(
            r.host_eager_combat_projectile_helper_nav_commands_wave710_ok,
            "host eager_combat_projectile helper nav commands residual pack wave710: {}",
            r.detail
        );
        assert!(
            r.host_eager_combat_projectile_helper_live_wave710_ok,
            "host eager_combat_projectile helper live residual wave710: {}",
            r.detail
        );
        assert!(
            r.host_eager_destroy_contain_ai_helper_method_names_wave711_ok,
            "host eager_destroy_contain_ai helper method names residual pack wave711: {}",
            r.detail
        );
        assert!(
            r.host_eager_destroy_contain_ai_helper_nav_commands_wave711_ok,
            "host eager_destroy_contain_ai helper nav commands residual pack wave711: {}",
            r.detail
        );
        assert!(
            r.host_eager_destroy_contain_ai_helper_live_wave711_ok,
            "host eager_destroy_contain_ai helper live residual wave711: {}",
            r.detail
        );
        assert!(
            r.host_eager_spawn_move_attack_helper_method_names_wave712_ok,
            "host eager_spawn_move_attack helper method names residual pack wave712: {}",
            r.detail
        );
        assert!(
            r.host_eager_spawn_move_attack_helper_nav_commands_wave712_ok,
            "host eager_spawn_move_attack helper nav commands residual pack wave712: {}",
            r.detail
        );
        assert!(
            r.host_eager_spawn_move_attack_helper_live_wave712_ok,
            "host eager_spawn_move_attack helper live residual wave712: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_no_empty_scan_method_names_wave713_ok,
            "host production_ready_no_empty_scan method names residual pack wave713: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_no_empty_scan_nav_commands_wave713_ok,
            "host production_ready_no_empty_scan nav commands residual pack wave713: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_no_empty_scan_live_wave713_ok,
            "host production_ready_no_empty_scan live residual wave713: {}",
            r.detail
        );
        assert!(
            r.host_production_same_frame_ready_complete_method_names_wave714_ok,
            "host production_same_frame_ready_complete method names residual pack wave714: {}",
            r.detail
        );
        assert!(
            r.host_production_same_frame_ready_complete_nav_commands_wave714_ok,
            "host production_same_frame_ready_complete nav commands residual pack wave714: {}",
            r.detail
        );
        assert!(
            r.host_production_same_frame_ready_complete_live_wave714_ok,
            "host production_same_frame_ready_complete live residual wave714: {}",
            r.detail
        );
        assert!(
            r.host_construction_same_frame_ready_complete_method_names_wave715_ok,
            "host construction_same_frame_ready_complete method names residual pack wave715: {}",
            r.detail
        );
        assert!(
            r.host_construction_same_frame_ready_complete_nav_commands_wave715_ok,
            "host construction_same_frame_ready_complete nav commands residual pack wave715: {}",
            r.detail
        );
        assert!(
            r.host_construction_same_frame_ready_complete_live_wave715_ok,
            "host construction_same_frame_ready_complete live residual wave715: {}",
            r.detail
        );
        assert!(
            r.host_sell_same_frame_ready_complete_method_names_wave716_ok,
            "host sell_same_frame_ready_complete method names residual pack wave716: {}",
            r.detail
        );
        assert!(
            r.host_sell_same_frame_ready_complete_nav_commands_wave716_ok,
            "host sell_same_frame_ready_complete nav commands residual pack wave716: {}",
            r.detail
        );
        assert!(
            r.host_sell_same_frame_ready_complete_live_wave716_ok,
            "host sell_same_frame_ready_complete live residual wave716: {}",
            r.detail
        );
        assert!(
            r.host_special_power_same_frame_ready_eva_method_names_wave717_ok,
            "host special_power_same_frame_ready_eva method names residual pack wave717: {}",
            r.detail
        );
        assert!(
            r.host_special_power_same_frame_ready_eva_nav_commands_wave717_ok,
            "host special_power_same_frame_ready_eva nav commands residual pack wave717: {}",
            r.detail
        );
        assert!(
            r.host_special_power_same_frame_ready_eva_live_wave717_ok,
            "host special_power_same_frame_ready_eva live residual wave717: {}",
            r.detail
        );
        assert!(
            r.host_train_force_complete_opt_in_method_names_wave718_ok,
            "host train_force_complete_opt_in method names residual pack wave718: {}",
            r.detail
        );
        assert!(
            r.host_train_force_complete_opt_in_nav_commands_wave718_ok,
            "host train_force_complete_opt_in nav commands residual pack wave718: {}",
            r.detail
        );
        assert!(
            r.host_train_force_complete_opt_in_live_wave718_ok,
            "host train_force_complete_opt_in live residual wave718: {}",
            r.detail
        );
        assert!(
            r.host_construct_spawn_dozer_opt_in_method_names_wave719_ok,
            "host construct_spawn_dozer_opt_in method names residual pack wave719: {}",
            r.detail
        );
        assert!(
            r.host_construct_spawn_dozer_opt_in_nav_commands_wave719_ok,
            "host construct_spawn_dozer_opt_in nav commands residual pack wave719: {}",
            r.detail
        );
        assert!(
            r.host_construct_spawn_dozer_opt_in_live_wave719_ok,
            "host construct_spawn_dozer_opt_in live residual wave719: {}",
            r.detail
        );
        assert!(
            r.host_formation_spawn_buddy_opt_in_method_names_wave720_ok,
            "host formation_spawn_buddy_opt_in method names residual pack wave720: {}",
            r.detail
        );
        assert!(
            r.host_formation_spawn_buddy_opt_in_nav_commands_wave720_ok,
            "host formation_spawn_buddy_opt_in nav commands residual pack wave720: {}",
            r.detail
        );
        assert!(
            r.host_formation_spawn_buddy_opt_in_live_wave720_ok,
            "host formation_spawn_buddy_opt_in live residual wave720: {}",
            r.detail
        );
        assert!(
            r.host_grant_min_supplies_opt_in_method_names_wave721_ok,
            "host grant_min_supplies_opt_in method names residual pack wave721: {}",
            r.detail
        );
        assert!(
            r.host_grant_min_supplies_opt_in_nav_commands_wave721_ok,
            "host grant_min_supplies_opt_in nav commands residual pack wave721: {}",
            r.detail
        );
        assert!(
            r.host_grant_min_supplies_opt_in_live_wave721_ok,
            "host grant_min_supplies_opt_in live residual wave721: {}",
            r.detail
        );
        assert!(
            r.host_golden_ranger_template_opt_in_method_names_wave722_ok,
            "host golden_ranger_template_opt_in method names residual pack wave722: {}",
            r.detail
        );
        assert!(
            r.host_golden_ranger_template_opt_in_nav_commands_wave722_ok,
            "host golden_ranger_template_opt_in nav commands residual pack wave722: {}",
            r.detail
        );
        assert!(
            r.host_golden_ranger_template_opt_in_live_wave722_ok,
            "host golden_ranger_template_opt_in live residual wave722: {}",
            r.detail
        );
        assert!(
            r.host_ensure_barracks_opt_in_method_names_wave723_ok,
            "host ensure_barracks_opt_in method names residual pack wave723: {}",
            r.detail
        );
        assert!(
            r.host_ensure_barracks_opt_in_nav_commands_wave723_ok,
            "host ensure_barracks_opt_in nav commands residual pack wave723: {}",
            r.detail
        );
        assert!(
            r.host_ensure_barracks_opt_in_live_wave723_ok,
            "host ensure_barracks_opt_in live residual wave723: {}",
            r.detail
        );
        assert!(
            r.host_train_try_names_golden_opt_in_method_names_wave724_ok,
            "host train_try_names_golden_opt_in method names residual pack wave724: {}",
            r.detail
        );
        assert!(
            r.host_train_try_names_golden_opt_in_nav_commands_wave724_ok,
            "host train_try_names_golden_opt_in nav commands residual pack wave724: {}",
            r.detail
        );
        assert!(
            r.host_train_try_names_golden_opt_in_live_wave724_ok,
            "host train_try_names_golden_opt_in live residual wave724: {}",
            r.detail
        );
        assert!(
            r.host_alias_fallback_opt_in_method_names_wave725_ok,
            "host alias_fallback_opt_in method names residual pack wave725: {}",
            r.detail
        );
        assert!(
            r.host_alias_fallback_opt_in_nav_commands_wave725_ok,
            "host alias_fallback_opt_in nav commands residual pack wave725: {}",
            r.detail
        );
        assert!(
            r.host_alias_fallback_opt_in_live_wave725_ok,
            "host alias_fallback_opt_in live residual wave725: {}",
            r.detail
        );
        assert!(
            r.host_auto_select_mobile_opt_in_method_names_wave726_ok,
            "host auto_select_mobile_opt_in method names residual pack wave726: {}",
            r.detail
        );
        assert!(
            r.host_auto_select_mobile_opt_in_nav_commands_wave726_ok,
            "host auto_select_mobile_opt_in nav commands residual pack wave726: {}",
            r.detail
        );
        assert!(
            r.host_auto_select_mobile_opt_in_live_wave726_ok,
            "host auto_select_mobile_opt_in live residual wave726: {}",
            r.detail
        );
        assert!(
            r.host_default_template_opt_in_method_names_wave727_ok,
            "host default_template_opt_in method names residual pack wave727: {}",
            r.detail
        );
        assert!(
            r.host_default_template_opt_in_nav_commands_wave727_ok,
            "host default_template_opt_in nav commands residual pack wave727: {}",
            r.detail
        );
        assert!(
            r.host_default_template_opt_in_live_wave727_ok,
            "host default_template_opt_in live residual wave727: {}",
            r.detail
        );
        assert!(
            r.host_sell_auto_target_opt_in_method_names_wave728_ok,
            "host sell_auto_target_opt_in method names residual pack wave728: {}",
            r.detail
        );
        assert!(
            r.host_sell_auto_target_opt_in_nav_commands_wave728_ok,
            "host sell_auto_target_opt_in nav commands residual pack wave728: {}",
            r.detail
        );
        assert!(
            r.host_sell_auto_target_opt_in_live_wave728_ok,
            "host sell_auto_target_opt_in live residual wave728: {}",
            r.detail
        );
        assert!(
            r.host_auto_target_opt_in_method_names_wave729_ok,
            "host auto_target_opt_in method names residual pack wave729: {}",
            r.detail
        );
        assert!(
            r.host_auto_target_opt_in_nav_commands_wave729_ok,
            "host auto_target_opt_in nav commands residual pack wave729: {}",
            r.detail
        );
        assert!(
            r.host_auto_target_opt_in_live_wave729_ok,
            "host auto_target_opt_in live residual wave729: {}",
            r.detail
        );
        assert!(
            r.host_cmd_auto_select_opt_in_method_names_wave730_ok,
            "host cmd_auto_select_opt_in method names residual pack wave730: {}",
            r.detail
        );
        assert!(
            r.host_cmd_auto_select_opt_in_nav_commands_wave730_ok,
            "host cmd_auto_select_opt_in nav commands residual pack wave730: {}",
            r.detail
        );
        assert!(
            r.host_cmd_auto_select_opt_in_live_wave730_ok,
            "host cmd_auto_select_opt_in live residual wave730: {}",
            r.detail
        );
        assert!(
            r.host_cmd_auto_pick_opt_in_method_names_wave731_ok,
            "host cmd_auto_pick_opt_in method names residual pack wave731: {}",
            r.detail
        );
        assert!(
            r.host_cmd_auto_pick_opt_in_nav_commands_wave731_ok,
            "host cmd_auto_pick_opt_in nav commands residual pack wave731: {}",
            r.detail
        );
        assert!(
            r.host_cmd_auto_pick_opt_in_live_wave731_ok,
            "host cmd_auto_pick_opt_in live residual wave731: {}",
            r.detail
        );
        assert!(
            r.host_seed_start_presence_opt_in_method_names_wave732_ok,
            "host seed_start_presence_opt_in method names residual pack wave732: {}",
            r.detail
        );
        assert!(
            r.host_seed_start_presence_opt_in_nav_commands_wave732_ok,
            "host seed_start_presence_opt_in nav commands residual pack wave732: {}",
            r.detail
        );
        assert!(
            r.host_seed_start_presence_opt_in_live_wave732_ok,
            "host seed_start_presence_opt_in live residual wave732: {}",
            r.detail
        );
        assert!(
            r.host_spawn_faction_base_opt_in_method_names_wave733_ok,
            "host spawn_faction_base_opt_in method names residual pack wave733: {}",
            r.detail
        );
        assert!(
            r.host_spawn_faction_base_opt_in_nav_commands_wave733_ok,
            "host spawn_faction_base_opt_in nav commands residual pack wave733: {}",
            r.detail
        );
        assert!(
            r.host_spawn_faction_base_opt_in_live_wave733_ok,
            "host spawn_faction_base_opt_in live residual wave733: {}",
            r.detail
        );
        assert!(
            r.host_seed_starting_building_opt_in_method_names_wave734_ok,
            "host seed_starting_building_opt_in method names residual pack wave734: {}",
            r.detail
        );
        assert!(
            r.host_seed_starting_building_opt_in_nav_commands_wave734_ok,
            "host seed_starting_building_opt_in nav commands residual pack wave734: {}",
            r.detail
        );
        assert!(
            r.host_seed_starting_building_opt_in_live_wave734_ok,
            "host seed_starting_building_opt_in live residual wave734: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_pose_authority_method_names_wave735_ok,
            "host production_ready_pose_authority method names residual pack wave735: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_pose_authority_nav_commands_wave735_ok,
            "host production_ready_pose_authority nav commands residual pack wave735: {}",
            r.detail
        );
        assert!(
            r.host_production_ready_pose_authority_live_wave735_ok,
            "host production_ready_pose_authority live residual wave735: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_entity_first_method_names_wave736_ok,
            "host production_spawn_entity_first method names residual pack wave736: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_entity_first_nav_commands_wave736_ok,
            "host production_spawn_entity_first nav commands residual pack wave736: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_entity_first_live_wave736_ok,
            "host production_spawn_entity_first live residual wave736: {}",
            r.detail
        );
        assert!(
            r.host_production_object_id_prefers_gw_entity_method_names_wave737_ok,
            "host production_object_id_prefers_gw_entity method names residual pack wave737: {}",
            r.detail
        );
        assert!(
            r.host_production_object_id_prefers_gw_entity_nav_commands_wave737_ok,
            "host production_object_id_prefers_gw_entity nav commands residual pack wave737: {}",
            r.detail
        );
        assert!(
            r.host_production_object_id_prefers_gw_entity_live_wave737_ok,
            "host production_object_id_prefers_gw_entity live residual wave737: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_requires_gw_bind_method_names_wave738_ok,
            "host production_spawn_requires_gw_bind method names residual pack wave738: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_requires_gw_bind_nav_commands_wave738_ok,
            "host production_spawn_requires_gw_bind nav commands residual pack wave738: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_requires_gw_bind_live_wave738_ok,
            "host production_spawn_requires_gw_bind live residual wave738: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_pose_no_rejitter_method_names_wave739_ok,
            "host production_spawn_pose_no_rejitter method names residual pack wave739: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_pose_no_rejitter_nav_commands_wave739_ok,
            "host production_spawn_pose_no_rejitter nav commands residual pack wave739: {}",
            r.detail
        );
        assert!(
            r.host_production_spawn_pose_no_rejitter_live_wave739_ok,
            "host production_spawn_pose_no_rejitter live residual wave739: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_spawn_entity_first_method_names_wave740_ok,
            "host rebuild_spawn_entity_first method names residual pack wave740: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_spawn_entity_first_nav_commands_wave740_ok,
            "host rebuild_spawn_entity_first nav commands residual pack wave740: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_spawn_entity_first_live_wave740_ok,
            "host rebuild_spawn_entity_first live residual wave740: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_spawn_requires_gw_bind_method_names_wave741_ok,
            "host rebuild_spawn_requires_gw_bind method names residual pack wave741: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_spawn_requires_gw_bind_nav_commands_wave741_ok,
            "host rebuild_spawn_requires_gw_bind nav commands residual pack wave741: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_spawn_requires_gw_bind_live_wave741_ok,
            "host rebuild_spawn_requires_gw_bind live residual wave741: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_hole_expose_entity_first_method_names_wave742_ok,
            "host rebuild_hole_expose_entity_first method names residual pack wave742: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_hole_expose_entity_first_nav_commands_wave742_ok,
            "host rebuild_hole_expose_entity_first nav commands residual pack wave742: {}",
            r.detail
        );
        assert!(
            r.host_rebuild_hole_expose_entity_first_live_wave742_ok,
            "host rebuild_hole_expose_entity_first live residual wave742: {}",
            r.detail
        );
        assert!(
            r.host_production_door_sole_no_dual_tick_method_names_wave743_ok,
            "host production_door_sole_no_dual_tick method names residual pack wave743: {}",
            r.detail
        );
        assert!(
            r.host_production_door_sole_no_dual_tick_nav_commands_wave743_ok,
            "host production_door_sole_no_dual_tick nav commands residual pack wave743: {}",
            r.detail
        );
        assert!(
            r.host_production_door_sole_no_dual_tick_live_wave743_ok,
            "host production_door_sole_no_dual_tick live residual wave743: {}",
            r.detail
        );
        assert!(
            r.host_radar_extend_no_dual_complete_method_names_wave744_ok,
            "host radar_extend_no_dual_complete method names residual pack wave744: {}",
            r.detail
        );
        assert!(
            r.host_radar_extend_no_dual_complete_nav_commands_wave744_ok,
            "host radar_extend_no_dual_complete nav commands residual pack wave744: {}",
            r.detail
        );
        assert!(
            r.host_radar_extend_no_dual_complete_live_wave744_ok,
            "host radar_extend_no_dual_complete live residual wave744: {}",
            r.detail
        );
        assert!(
            r.host_lifetime_kill_no_damage_auth_hp_stomp_method_names_wave745_ok,
            "host lifetime_kill_no_damage_auth_hp_stomp method names residual pack wave745: {}",
            r.detail
        );
        assert!(
            r.host_lifetime_kill_no_damage_auth_hp_stomp_nav_commands_wave745_ok,
            "host lifetime_kill_no_damage_auth_hp_stomp nav commands residual pack wave745: {}",
            r.detail
        );
        assert!(
            r.host_lifetime_kill_no_damage_auth_hp_stomp_live_wave745_ok,
            "host lifetime_kill_no_damage_auth_hp_stomp live residual wave745: {}",
            r.detail
        );
        assert!(
            r.host_crush_failclosed_no_damage_auth_hp_stomp_method_names_wave746_ok,
            "host crush_failclosed_no_damage_auth_hp_stomp method names residual pack wave746: {}",
            r.detail
        );
        assert!(
            r.host_crush_failclosed_no_damage_auth_hp_stomp_nav_commands_wave746_ok,
            "host crush_failclosed_no_damage_auth_hp_stomp nav commands residual pack wave746: {}",
            r.detail
        );
        assert!(
            r.host_crush_failclosed_no_damage_auth_hp_stomp_live_wave746_ok,
            "host crush_failclosed_no_damage_auth_hp_stomp live residual wave746: {}",
            r.detail
        );
        assert!(
            r.host_evacuate_exit_no_damage_auth_hp_stomp_method_names_wave747_ok,
            "host evacuate_exit_no_damage_auth_hp_stomp method names residual pack wave747: {}",
            r.detail
        );
        assert!(
            r.host_evacuate_exit_no_damage_auth_hp_stomp_nav_commands_wave747_ok,
            "host evacuate_exit_no_damage_auth_hp_stomp nav commands residual pack wave747: {}",
            r.detail
        );
        assert!(
            r.host_evacuate_exit_no_damage_auth_hp_stomp_live_wave747_ok,
            "host evacuate_exit_no_damage_auth_hp_stomp live residual wave747: {}",
            r.detail
        );
        assert!(
            r.host_hive_struct_damage_no_damage_auth_hp_stomp_method_names_wave748_ok,
            "host hive_struct_damage_no_damage_auth_hp_stomp method names residual pack wave748: {}",
            r.detail
        );
        assert!(
            r.host_hive_struct_damage_no_damage_auth_hp_stomp_nav_commands_wave748_ok,
            "host hive_struct_damage_no_damage_auth_hp_stomp nav commands residual pack wave748: {}",
            r.detail
        );
        assert!(
            r.host_hive_struct_damage_no_damage_auth_hp_stomp_live_wave748_ok,
            "host hive_struct_damage_no_damage_auth_hp_stomp live residual wave748: {}",
            r.detail
        );
        assert!(
            r.host_tensile_rubble_no_damage_auth_hp_stomp_method_names_wave749_ok,
            "host tensile_rubble_no_damage_auth_hp_stomp method names residual pack wave749: {}",
            r.detail
        );
        assert!(
            r.host_tensile_rubble_no_damage_auth_hp_stomp_nav_commands_wave749_ok,
            "host tensile_rubble_no_damage_auth_hp_stomp nav commands residual pack wave749: {}",
            r.detail
        );
        assert!(
            r.host_tensile_rubble_no_damage_auth_hp_stomp_live_wave749_ok,
            "host tensile_rubble_no_damage_auth_hp_stomp live residual wave749: {}",
            r.detail
        );
        assert!(
            r.host_spectre_prior_clear_no_damage_auth_hp_stomp_method_names_wave750_ok,
            "host spectre_prior_clear_no_damage_auth_hp_stomp method names residual pack wave750: {}",
            r.detail
        );
        assert!(
            r.host_spectre_prior_clear_no_damage_auth_hp_stomp_nav_commands_wave750_ok,
            "host spectre_prior_clear_no_damage_auth_hp_stomp nav commands residual pack wave750: {}",
            r.detail
        );
        assert!(
            r.host_spectre_prior_clear_no_damage_auth_hp_stomp_live_wave750_ok,
            "host spectre_prior_clear_no_damage_auth_hp_stomp live residual wave750: {}",
            r.detail
        );
        assert!(
            r.host_booby_trap_destroy_no_damage_auth_hp_stomp_method_names_wave751_ok,
            "host booby_trap_destroy_no_damage_auth_hp_stomp method names residual pack wave751: {}",
            r.detail
        );
        assert!(
            r.host_booby_trap_destroy_no_damage_auth_hp_stomp_nav_commands_wave751_ok,
            "host booby_trap_destroy_no_damage_auth_hp_stomp nav commands residual pack wave751: {}",
            r.detail
        );
        assert!(
            r.host_booby_trap_destroy_no_damage_auth_hp_stomp_live_wave751_ok,
            "host booby_trap_destroy_no_damage_auth_hp_stomp live residual wave751: {}",
            r.detail
        );
        assert!(
            r.host_lethal_finish_bulk_no_damage_auth_hp_stomp_method_names_wave752_ok,
            "host lethal_finish_bulk_no_damage_auth_hp_stomp method names residual pack wave752: {}",
            r.detail
        );
        assert!(
            r.host_lethal_finish_bulk_no_damage_auth_hp_stomp_nav_commands_wave752_ok,
            "host lethal_finish_bulk_no_damage_auth_hp_stomp nav commands residual pack wave752: {}",
            r.detail
        );
        assert!(
            r.host_lethal_finish_bulk_no_damage_auth_hp_stomp_live_wave752_ok,
            "host lethal_finish_bulk_no_damage_auth_hp_stomp live residual wave752: {}",
            r.detail
        );
        assert!(
            r.host_dual_line_lethal_no_damage_auth_hp_stomp_method_names_wave753_ok,
            "host dual_line_lethal_no_damage_auth_hp_stomp method names residual pack wave753: {}",
            r.detail
        );
        assert!(
            r.host_dual_line_lethal_no_damage_auth_hp_stomp_nav_commands_wave753_ok,
            "host dual_line_lethal_no_damage_auth_hp_stomp nav commands residual pack wave753: {}",
            r.detail
        );
        assert!(
            r.host_dual_line_lethal_no_damage_auth_hp_stomp_live_wave753_ok,
            "host dual_line_lethal_no_damage_auth_hp_stomp live residual wave753: {}",
            r.detail
        );
        assert!(
            r.host_eject_pilot_die_death_start_method_names_wave754_ok,
            "host eject_pilot_die_death_start method names residual pack wave754: {}",
            r.detail
        );
        assert!(
            r.host_eject_pilot_die_death_start_nav_commands_wave754_ok,
            "host eject_pilot_die_death_start nav commands residual pack wave754: {}",
            r.detail
        );
        assert!(
            r.host_eject_pilot_die_death_start_live_wave754_ok,
            "host eject_pilot_die_death_start live residual wave754: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_host_logs_method_names_wave755_ok,
            "host writeback_skip_pending_host_logs method names residual pack wave755: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_host_logs_nav_commands_wave755_ok,
            "host writeback_skip_pending_host_logs nav commands residual pack wave755: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_host_logs_live_wave755_ok,
            "host writeback_skip_pending_host_logs live residual wave755: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_shock_disable_repulsor_method_names_wave756_ok,
            "host writeback_skip_pending_shock_disable_repulsor method names residual pack wave756: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_shock_disable_repulsor_nav_commands_wave756_ok,
            "host writeback_skip_pending_shock_disable_repulsor nav commands residual pack wave756: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_shock_disable_repulsor_live_wave756_ok,
            "host writeback_skip_pending_shock_disable_repulsor live residual wave756: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_combat_movement_logs_method_names_wave757_ok,
            "host writeback_skip_pending_combat_movement_logs method names residual pack wave757: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_combat_movement_logs_nav_commands_wave757_ok,
            "host writeback_skip_pending_combat_movement_logs nav commands residual pack wave757: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_combat_movement_logs_live_wave757_ok,
            "host writeback_skip_pending_combat_movement_logs live residual wave757: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_remaining_logs_method_names_wave758_ok,
            "host writeback_skip_pending_remaining_logs method names residual pack wave758: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_remaining_logs_nav_commands_wave758_ok,
            "host writeback_skip_pending_remaining_logs nav commands residual pack wave758: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_remaining_logs_live_wave758_ok,
            "host writeback_skip_pending_remaining_logs live residual wave758: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_move_transform_logs_method_names_wave759_ok,
            "host writeback_skip_pending_move_transform_logs method names residual pack wave759: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_move_transform_logs_nav_commands_wave759_ok,
            "host writeback_skip_pending_move_transform_logs nav commands residual pack wave759: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_move_transform_logs_live_wave759_ok,
            "host writeback_skip_pending_move_transform_logs live residual wave759: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_player_projectile_logs_method_names_wave760_ok,
            "host writeback_skip_pending_player_projectile_logs method names residual pack wave760: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_player_projectile_logs_nav_commands_wave760_ok,
            "host writeback_skip_pending_player_projectile_logs nav commands residual pack wave760: {}",
            r.detail
        );
        assert!(
            r.host_writeback_skip_pending_player_projectile_logs_live_wave760_ok,
            "host writeback_skip_pending_player_projectile_logs live residual wave760: {}",
            r.detail
        );
        assert!(
            r.host_status_timer_dual_peel_method_names_wave761_ok,
            "host status_timer_dual_peel method names residual pack wave761: {}",
            r.detail
        );
        assert!(
            r.host_status_timer_dual_peel_nav_commands_wave761_ok,
            "host status_timer_dual_peel nav commands residual pack wave761: {}",
            r.detail
        );
        assert!(
            r.host_status_timer_dual_peel_live_wave761_ok,
            "host status_timer_dual_peel live residual wave761: {}",
            r.detail
        );
        assert!(
            r.host_eject_invuln_dual_peel_method_names_wave762_ok,
            "host eject_invuln_dual_peel method names residual pack wave762: {}",
            r.detail
        );
        assert!(
            r.host_eject_invuln_dual_peel_nav_commands_wave762_ok,
            "host eject_invuln_dual_peel nav commands residual pack wave762: {}",
            r.detail
        );
        assert!(
            r.host_eject_invuln_dual_peel_live_wave762_ok,
            "host eject_invuln_dual_peel live residual wave762: {}",
            r.detail
        );
        assert!(
            r.host_force_reload_dual_peel_method_names_wave763_ok,
            "host force_reload_dual_peel method names residual pack wave763: {}",
            r.detail
        );
        assert!(
            r.host_force_reload_dual_peel_nav_commands_wave763_ok,
            "host force_reload_dual_peel nav commands residual pack wave763: {}",
            r.detail
        );
        assert!(
            r.host_force_reload_dual_peel_live_wave763_ok,
            "host force_reload_dual_peel live residual wave763: {}",
            r.detail
        );
        assert!(
            r.host_shock_stun_dual_peel_method_names_wave764_ok,
            "host shock_stun_dual_peel method names residual pack wave764: {}",
            r.detail
        );
        assert!(
            r.host_shock_stun_dual_peel_nav_commands_wave764_ok,
            "host shock_stun_dual_peel nav commands residual pack wave764: {}",
            r.detail
        );
        assert!(
            r.host_shock_stun_dual_peel_live_wave764_ok,
            "host shock_stun_dual_peel live residual wave764: {}",
            r.detail
        );
        assert!(
            r.host_subdual_heal_dual_peel_method_names_wave765_ok,
            "host subdual_heal_dual_peel method names residual pack wave765: {}",
            r.detail
        );
        assert!(
            r.host_subdual_heal_dual_peel_nav_commands_wave765_ok,
            "host subdual_heal_dual_peel nav commands residual pack wave765: {}",
            r.detail
        );
        assert!(
            r.host_subdual_heal_dual_peel_live_wave765_ok,
            "host subdual_heal_dual_peel live residual wave765: {}",
            r.detail
        );
        assert!(
            r.host_defection_timer_dual_peel_method_names_wave766_ok,
            "host defection_timer_dual_peel method names residual pack wave766: {}",
            r.detail
        );
        assert!(
            r.host_defection_timer_dual_peel_nav_commands_wave766_ok,
            "host defection_timer_dual_peel nav commands residual pack wave766: {}",
            r.detail
        );
        assert!(
            r.host_defection_timer_dual_peel_live_wave766_ok,
            "host defection_timer_dual_peel live residual wave766: {}",
            r.detail
        );
        assert!(
            r.host_fire_sound_loop_dual_peel_method_names_wave767_ok,
            "host fire_sound_loop_dual_peel method names residual pack wave767: {}",
            r.detail
        );
        assert!(
            r.host_fire_sound_loop_dual_peel_nav_commands_wave767_ok,
            "host fire_sound_loop_dual_peel nav commands residual pack wave767: {}",
            r.detail
        );
        assert!(
            r.host_fire_sound_loop_dual_peel_live_wave767_ok,
            "host fire_sound_loop_dual_peel live residual wave767: {}",
            r.detail
        );
        assert!(
            r.host_lifetime_expire_dual_peel_method_names_wave768_ok,
            "host lifetime_expire_dual_peel method names residual pack wave768: {}",
            r.detail
        );
        assert!(
            r.host_lifetime_expire_dual_peel_nav_commands_wave768_ok,
            "host lifetime_expire_dual_peel nav commands residual pack wave768: {}",
            r.detail
        );
        assert!(
            r.host_lifetime_expire_dual_peel_live_wave768_ok,
            "host lifetime_expire_dual_peel live residual wave768: {}",
            r.detail
        );
        assert!(
            r.host_poison_dot_dual_peel_method_names_wave769_ok,
            "host poison_dot_dual_peel method names residual pack wave769: {}",
            r.detail
        );
        assert!(
            r.host_poison_dot_dual_peel_nav_commands_wave769_ok,
            "host poison_dot_dual_peel nav commands residual pack wave769: {}",
            r.detail
        );
        assert!(
            r.host_poison_dot_dual_peel_live_wave769_ok,
            "host poison_dot_dual_peel live residual wave769: {}",
            r.detail
        );
        assert!(
            r.host_topple_fall_dual_peel_method_names_wave770_ok,
            "host topple_fall_dual_peel method names residual pack wave770: {}",
            r.detail
        );
        assert!(
            r.host_topple_fall_dual_peel_nav_commands_wave770_ok,
            "host topple_fall_dual_peel nav commands residual pack wave770: {}",
            r.detail
        );
        assert!(
            r.host_topple_fall_dual_peel_live_wave770_ok,
            "host topple_fall_dual_peel live residual wave770: {}",
            r.detail
        );
        assert!(
            r.host_height_die_dual_peel_method_names_wave771_ok,
            "host height_die_dual_peel method names residual pack wave771: {}",
            r.detail
        );
        assert!(
            r.host_height_die_dual_peel_nav_commands_wave771_ok,
            "host height_die_dual_peel nav commands residual pack wave771: {}",
            r.detail
        );
        assert!(
            r.host_height_die_dual_peel_live_wave771_ok,
            "host height_die_dual_peel live residual wave771: {}",
            r.detail
        );
        assert!(
            r.host_jet_slow_death_dual_peel_method_names_wave772_ok,
            "host jet_slow_death_dual_peel method names residual pack wave772: {}",
            r.detail
        );
        assert!(
            r.host_jet_slow_death_dual_peel_nav_commands_wave772_ok,
            "host jet_slow_death_dual_peel nav commands residual pack wave772: {}",
            r.detail
        );
        assert!(
            r.host_jet_slow_death_dual_peel_live_wave772_ok,
            "host jet_slow_death_dual_peel live residual wave772: {}",
            r.detail
        );
        assert!(
            r.host_heli_slow_death_dual_peel_method_names_wave773_ok,
            "host heli_slow_death_dual_peel method names residual pack wave773: {}",
            r.detail
        );
        assert!(
            r.host_heli_slow_death_dual_peel_nav_commands_wave773_ok,
            "host heli_slow_death_dual_peel nav commands residual pack wave773: {}",
            r.detail
        );
        assert!(
            r.host_heli_slow_death_dual_peel_live_wave773_ok,
            "host heli_slow_death_dual_peel live residual wave773: {}",
            r.detail
        );
        assert!(
            r.host_slow_death_dual_peel_method_names_wave774_ok,
            "host slow_death_dual_peel method names residual pack wave774: {}",
            r.detail
        );
        assert!(
            r.host_slow_death_dual_peel_nav_commands_wave774_ok,
            "host slow_death_dual_peel nav commands residual pack wave774: {}",
            r.detail
        );
        assert!(
            r.host_slow_death_dual_peel_live_wave774_ok,
            "host slow_death_dual_peel live residual wave774: {}",
            r.detail
        );
        assert!(
            r.host_structure_collapse_dual_peel_method_names_wave775_ok,
            "host structure_collapse_dual_peel method names residual pack wave775: {}",
            r.detail
        );
        assert!(
            r.host_structure_collapse_dual_peel_nav_commands_wave775_ok,
            "host structure_collapse_dual_peel nav commands residual pack wave775: {}",
            r.detail
        );
        assert!(
            r.host_structure_collapse_dual_peel_live_wave775_ok,
            "host structure_collapse_dual_peel live residual wave775: {}",
            r.detail
        );
        assert!(
            r.host_structure_topple_dual_peel_method_names_wave776_ok,
            "host structure_topple_dual_peel method names residual pack wave776: {}",
            r.detail
        );
        assert!(
            r.host_structure_topple_dual_peel_nav_commands_wave776_ok,
            "host structure_topple_dual_peel nav commands residual pack wave776: {}",
            r.detail
        );
        assert!(
            r.host_structure_topple_dual_peel_live_wave776_ok,
            "host structure_topple_dual_peel live residual wave776: {}",
            r.detail
        );
        assert!(
            r.host_structure_topple_crush_dual_peel_method_names_wave777_ok,
            "host structure_topple_crush_dual_peel method names residual pack wave777: {}",
            r.detail
        );
        assert!(
            r.host_structure_topple_crush_dual_peel_nav_commands_wave777_ok,
            "host structure_topple_crush_dual_peel nav commands residual pack wave777: {}",
            r.detail
        );
        assert!(
            r.host_structure_topple_crush_dual_peel_live_wave777_ok,
            "host structure_topple_crush_dual_peel live residual wave777: {}",
            r.detail
        );
        assert!(
            r.host_fwwd_continuous_dual_peel_method_names_wave778_ok,
            "host fwwd_continuous_dual_peel method names residual pack wave778: {}",
            r.detail
        );
        assert!(
            r.host_fwwd_continuous_dual_peel_nav_commands_wave778_ok,
            "host fwwd_continuous_dual_peel nav commands residual pack wave778: {}",
            r.detail
        );
        assert!(
            r.host_fwwd_continuous_dual_peel_live_wave778_ok,
            "host fwwd_continuous_dual_peel live residual wave778: {}",
            r.detail
        );
        assert!(
            r.host_fwwd_reaction_dual_peel_method_names_wave779_ok,
            "host fwwd_reaction_dual_peel method names residual pack wave779: {}",
            r.detail
        );
        assert!(
            r.host_fwwd_reaction_dual_peel_nav_commands_wave779_ok,
            "host fwwd_reaction_dual_peel nav commands residual pack wave779: {}",
            r.detail
        );
        assert!(
            r.host_fwwd_reaction_dual_peel_live_wave779_ok,
            "host fwwd_reaction_dual_peel live residual wave779: {}",
            r.detail
        );
        assert!(
            r.host_base_regen_dual_peel_method_names_wave780_ok,
            "host base_regen_dual_peel method names residual pack wave780: {}",
            r.detail
        );
        assert!(
            r.host_base_regen_dual_peel_nav_commands_wave780_ok,
            "host base_regen_dual_peel nav commands residual pack wave780: {}",
            r.detail
        );
        assert!(
            r.host_base_regen_dual_peel_live_wave780_ok,
            "host base_regen_dual_peel live residual wave780: {}",
            r.detail
        );
        assert!(
            r.host_enemy_near_dual_peel_method_names_wave781_ok,
            "host enemy_near_dual_peel method names residual pack wave781: {}",
            r.detail
        );
        assert!(
            r.host_enemy_near_dual_peel_nav_commands_wave781_ok,
            "host enemy_near_dual_peel nav commands residual pack wave781: {}",
            r.detail
        );
        assert!(
            r.host_enemy_near_dual_peel_live_wave781_ok,
            "host enemy_near_dual_peel live residual wave781: {}",
            r.detail
        );
        assert!(
            r.host_prone_update_dual_peel_method_names_wave782_ok,
            "host prone_update_dual_peel method names residual pack wave782: {}",
            r.detail
        );
        assert!(
            r.host_prone_update_dual_peel_nav_commands_wave782_ok,
            "host prone_update_dual_peel nav commands residual pack wave782: {}",
            r.detail
        );
        assert!(
            r.host_prone_update_dual_peel_live_wave782_ok,
            "host prone_update_dual_peel live residual wave782: {}",
            r.detail
        );
        assert!(
            r.host_float_update_dual_peel_method_names_wave783_ok,
            "host float_update_dual_peel method names residual pack wave783: {}",
            r.detail
        );
        assert!(
            r.host_float_update_dual_peel_nav_commands_wave783_ok,
            "host float_update_dual_peel nav commands residual pack wave783: {}",
            r.detail
        );
        assert!(
            r.host_float_update_dual_peel_live_wave783_ok,
            "host float_update_dual_peel live residual wave783: {}",
            r.detail
        );
        assert!(
            r.host_anim_steer_dual_peel_method_names_wave784_ok,
            "host anim_steer_dual_peel method names residual pack wave784: {}",
            r.detail
        );
        assert!(
            r.host_anim_steer_dual_peel_nav_commands_wave784_ok,
            "host anim_steer_dual_peel nav commands residual pack wave784: {}",
            r.detail
        );
        assert!(
            r.host_anim_steer_dual_peel_live_wave784_ok,
            "host anim_steer_dual_peel live residual wave784: {}",
            r.detail
        );
        assert!(
            r.host_radius_decal_dual_peel_method_names_wave785_ok,
            "host radius_decal_dual_peel method names residual pack wave785: {}",
            r.detail
        );
        assert!(
            r.host_radius_decal_dual_peel_nav_commands_wave785_ok,
            "host radius_decal_dual_peel nav commands residual pack wave785: {}",
            r.detail
        );
        assert!(
            r.host_radius_decal_dual_peel_live_wave785_ok,
            "host radius_decal_dual_peel live residual wave785: {}",
            r.detail
        );
        assert!(
            r.host_checkpoint_dual_peel_method_names_wave786_ok,
            "host checkpoint_dual_peel method names residual pack wave786: {}",
            r.detail
        );
        assert!(
            r.host_checkpoint_dual_peel_nav_commands_wave786_ok,
            "host checkpoint_dual_peel nav commands residual pack wave786: {}",
            r.detail
        );
        assert!(
            r.host_checkpoint_dual_peel_live_wave786_ok,
            "host checkpoint_dual_peel live residual wave786: {}",
            r.detail
        );
        assert!(
            r.host_smart_bomb_homing_dual_peel_method_names_wave787_ok,
            "host smart_bomb_homing_dual_peel method names residual pack wave787: {}",
            r.detail
        );
        assert!(
            r.host_smart_bomb_homing_dual_peel_nav_commands_wave787_ok,
            "host smart_bomb_homing_dual_peel nav commands residual pack wave787: {}",
            r.detail
        );
        assert!(
            r.host_smart_bomb_homing_dual_peel_live_wave787_ok,
            "host smart_bomb_homing_dual_peel live residual wave787: {}",
            r.detail
        );
        assert!(
            r.host_daisy_cutter_flight_dual_peel_method_names_wave788_ok,
            "host daisy_cutter_flight_dual_peel method names residual pack wave788: {}",
            r.detail
        );
        assert!(
            r.host_daisy_cutter_flight_dual_peel_nav_commands_wave788_ok,
            "host daisy_cutter_flight_dual_peel nav commands residual pack wave788: {}",
            r.detail
        );
        assert!(
            r.host_daisy_cutter_flight_dual_peel_live_wave788_ok,
            "host daisy_cutter_flight_dual_peel live residual wave788: {}",
            r.detail
        );
        assert!(
            r.live_upgrade_behavior_dual_world_empty_gate_nav_commands_wave453_ok,
            "live upgrade behavior dual-world empty gate nav commands residual pack wave453: {}",
            r.detail
        );
        assert!(
            r.live_upgrade_behavior_dual_world_empty_gate_live_wave453_ok,
            "live upgrade behavior dual-world empty gate live residual wave453: {}",
            r.detail
        );
        assert!(
            r.live_golden_mopup_honesty_nav_commands_wave451_ok,
            "live golden mop-up honesty nav commands residual pack wave451: {}",
            r.detail
        );
        assert!(
            r.live_golden_mopup_honesty_live_wave451_ok,
            "live golden mop-up honesty live residual wave451: {}",
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
