//! Debug/internal GlobalData fields actually present in GameDataDebug.ini.
//! Original GlobalData.cpp:467-493. Existing typed fields remain authoritative;
//! unsupported keys and malformed values continue to fail the shared parser.
use super::{GlobalData, INIResult, parse_bool, parse_int, parse_real, parse_rgb_color, parse_u32};
pub(super) fn parse_field(key: &str, args: &[&str], data: &mut GlobalData) -> INIResult<bool> {
    match key.to_ascii_uppercase().as_str() {
        "WIREFRAME" => data.wireframe = parse_bool(args)?,
        "STATEMACHINEDEBUG" => data.state_machine_debug = parse_bool(args)?,
        "USECAMERACONSTRAINTS" => data.use_camera_constraints = parse_bool(args)?,
        "SHROUDON" => data.shroud_on = parse_bool(args)?,
        "FOGOFWARON" => data.fog_of_war_on = parse_bool(args)?,
        "SHOWCOLLISIONEXTENTS" => data.show_collision_extents = parse_bool(args)?,
        "DEBUGPROJECTILETILEWIDTH" => data.debug_projectile_tile_width = parse_real(args)?,
        "DEBUGPROJECTILETILEDURATION" => data.debug_projectile_tile_duration = parse_int(args)?,
        "DEBUGPROJECTILETILECOLOR" => data.debug_projectile_tile_color = parse_rgb_color(args)?,
        "DEBUGVISIBILITYTILECOUNT" => data.debug_visibility_tile_count = parse_int(args)?,
        "DEBUGVISIBILITYTILEWIDTH" => data.debug_visibility_tile_width = parse_real(args)?,
        "DEBUGVISIBILITYTILEDURATION" => data.debug_visibility_tile_duration = parse_int(args)?,
        "DEBUGVISIBILITYTILETARGETTABLECOLOR" => {
            data.debug_visibility_targettable_color = parse_rgb_color(args)?
        }
        "DEBUGVISIBILITYTILEDESHROUDCOLOR" => {
            data.debug_visibility_deshroud_color = parse_rgb_color(args)?
        }
        "DEBUGVISIBILITYTILEGAPCOLOR" => data.debug_visibility_gap_color = parse_rgb_color(args)?,
        "DEBUGTHREATMAPTILEDURATION" => data.debug_threat_map_tile_duration = parse_int(args)?,
        "MAXDEBUGTHREATMAPVALUE" => data.max_debug_threat = parse_u32(args)?,
        "DEBUGCASHVALUEMAPTILEDURATION" => {
            data.debug_cash_value_map_tile_duration = parse_int(args)?
        }
        "MAXDEBUGCASHVALUEMAPVALUE" => data.max_debug_value = parse_u32(args)?,
        "VTUNE" => data.v_tune = parse_bool(args)?,
        _ => return Ok(false),
    }
    Ok(true)
}
#[cfg(test)]
#[path = "debug_fields_tests.rs"]
mod tests;
