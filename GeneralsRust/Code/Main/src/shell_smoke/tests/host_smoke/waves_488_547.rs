//! Host smoke residual assertions: waves 488–547.

use super::ShellSmokeResult;

pub(super) fn assert_waves_488_547(r: &ShellSmokeResult) {
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
}
