//! Host smoke residual assertions: waves 72–112.

use super::ShellSmokeResult;

pub(super) fn assert_waves_72_112(r: &ShellSmokeResult) {
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
}
