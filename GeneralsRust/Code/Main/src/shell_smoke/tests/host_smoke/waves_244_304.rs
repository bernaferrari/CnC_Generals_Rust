//! Host smoke residual assertions: waves 244–304.

use super::ShellSmokeResult;

pub(super) fn assert_waves_244_304(r: &ShellSmokeResult) {
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
}
