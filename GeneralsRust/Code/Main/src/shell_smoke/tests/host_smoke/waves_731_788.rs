//! Host smoke residual assertions: waves 731–788 (plus leftover wave80/453 tail asserts).

use super::ShellSmokeResult;

pub(super) fn assert_waves_731_788(r: &ShellSmokeResult) {
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
}
