//! Host smoke residual assertions: waves 183–243.

use super::ShellSmokeResult;

pub(super) fn assert_waves_183_243(r: &ShellSmokeResult) {
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
}
