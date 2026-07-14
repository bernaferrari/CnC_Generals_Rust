use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() {
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
        && r.object_kindof_wave103_ok;
    if pass {
        println!(
            "shell_smoke_gate: PASS (playable_claim={} shell_host_playable_ok={} control_bar={} cb_valid={} cb_loaded={} cb_windows={} dual_tick={} hud_sel={} sel_consumers={} minimap_fow={} laser_upload={} mesh={} sp72={} sp73={} sp76={} paradrop76={} cb76={} gfx76={} spectre_decal={} sp77={} fow77={} gh77={} weapon77={} ai77={} sp78={} cluster78={} gps78={} cash78={} minimap79={} sel79={} input79={} draw79={} train79={} upg79={} cmdbtn80={} rank80={} kindof80={} spenum80={} height81={} path81={} loco81={} armor81={} puc81={} dmg82={} death82={} mc82={} wbonus82={} ostatus82={} prod83={} supply83={} dozer83={} capture83={} power83={} cc83={} kindof84={} wslot84={} vet84={} rel84={} geom84={} shadow84={} faction85={} ptpl85={} cash85={} aiperson85={} victory85={} cam86={} world86={} mpopt86={} mapsel86={} crate86={} weather87={} water87={} bridge87={} tunnel87={} garrison87={} transport87={} radius88={} mouse88={} fxlist88={} ocl88={} particle88={} audio88={} rank89={} exp89={} hotkey89={} chat89={} replay89={} options89={} gamespeed90={} framerate90={} debug90={} lang90={} credits90={} tooltip91={} helpbox91={} message91={} eva91={} video91={} briefing91={} weapon92={} armor92={} body92={} loco92={} science92={} particle93={} drawable93={} shadow93={} terrain_tex93={} road93={} ai_state94={} special_ability94={} upgrade_names94={} command_set94={} script_action95={} script_cond95={} map_object95={} waypoint95={} team95={} player95={} partition96={} collision96={} physics96={} projectile96={} radar97={} spotter97={} stealth97={} detector97={} vision97={} dock98={} contain98={} exit98={} heal98={} production99={} buildable99={} prereq99={} cmdbtn99={} controlbar99={} thing_factory100={} module_type100={} xfer100={} tf_crosslink100={} module_factory101={} thing_factory101={} partition_register101={} mf_crosslink101={} display102={} anim2d102={} laser102={} csf102={} pres102={} weapon103={} armor103={} loco103={} sp103={} kindof103={} screen={} map_loaded={})",
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
