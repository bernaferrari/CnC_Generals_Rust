// Fill ShellSmokeResult fields from fields_core.rs (no include! in struct-literal position).
#[rustfmt::skip]
fn fill_core(
    out: &mut ShellSmokeResult,
    host: &super::host::HostSession,
    hud_selection_ok: bool,
) {
    out.host_constructed = host.host_constructed;
    out.skirmish_config_ok = host.skirmish_config_ok;
    out.menu_config_ok = host.menu_config_ok;
    out.map_resolved = host.map_resolved;
    out.map_loaded = host.map_loaded;
    out.frames_advanced = host.frames_advanced;
    out.presentation_ok = host.presentation_ok;
    out.dual_tick_presentation_ok = host.dual_tick_presentation_ok;
    out.dual_tick_counters_ok = host.dual_tick_counters_ok;
    out.gameworld_shadow_ok = host.gameworld_shadow_ok;
    out.damage_authority_env_ok = host.damage_authority_env_ok;
    out.economy_authority_env_ok = host.economy_authority_env_ok;
    out.production_authority_env_ok = host.production_authority_env_ok;
    out.dual_tick_policy_authority_only = host.dual_tick_policy_authority_only;
    out.engine_bridge_off = host.engine_bridge_off;
    out.hud_selection_ok = hud_selection_ok;
}
