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
