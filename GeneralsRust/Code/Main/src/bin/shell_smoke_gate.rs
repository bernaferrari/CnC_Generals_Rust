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
        && r.transport_wave87_ok;
    if pass {
        println!(
            "shell_smoke_gate: PASS (playable_claim={} shell_host_playable_ok={} control_bar={} cb_valid={} cb_loaded={} cb_windows={} dual_tick={} hud_sel={} sel_consumers={} minimap_fow={} laser_upload={} mesh={} sp72={} sp73={} sp76={} paradrop76={} cb76={} gfx76={} spectre_decal={} sp77={} fow77={} gh77={} weapon77={} ai77={} sp78={} cluster78={} gps78={} cash78={} minimap79={} sel79={} input79={} draw79={} train79={} upg79={} cmdbtn80={} rank80={} kindof80={} spenum80={} height81={} path81={} loco81={} armor81={} puc81={} dmg82={} death82={} mc82={} wbonus82={} ostatus82={} prod83={} supply83={} dozer83={} capture83={} power83={} cc83={} kindof84={} wslot84={} vet84={} rel84={} geom84={} shadow84={} faction85={} ptpl85={} cash85={} aiperson85={} victory85={} cam86={} world86={} mpopt86={} mapsel86={} crate86={} weather87={} water87={} bridge87={} tunnel87={} garrison87={} transport87={} screen={} map_loaded={})",
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
