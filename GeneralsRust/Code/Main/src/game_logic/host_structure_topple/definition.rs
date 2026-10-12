//! Immutable retained INI data. Runtime state remains on the actual Object.
use super::*;

#[cfg(test)]
#[path = "definition_tests.rs"]
mod tests;

#[derive(Debug, Clone)]
pub(crate) struct StructureToppleDefinition {
    pub min_delay: u32,
    pub max_delay: u32,
    pub integrity: f32,
    pub decay: f32,
    pub fx: AuthoredStructureTopplePeel,
    pub die_mux: gamelogic::object::die::DieMuxData,
    pub damage_fx_types: gamelogic::damage::DamageTypeFlags,
}
impl Default for StructureToppleDefinition {
    fn default() -> Self {
        Self {
            min_delay: 0,
            max_delay: 0,
            integrity: 0.1,
            decay: 0.0,
            fx: AuthoredStructureTopplePeel::default(),
            die_mux: gamelogic::object::die::DieMuxData::default(),
            damage_fx_types: gamelogic::damage::DamageTypeFlags::all_flags(),
        }
    }
}
impl StructureToppleDefinition {
    pub(crate) fn from_module(
        module: &crate::assets::BehaviorModuleDefinition,
    ) -> Result<Self, String> {
        fn invalid(key: &str, value: &str) -> String {
            format!("invalid StructureToppleUpdate {key}: {value:?}")
        }
        fn duration(
            module: &crate::assets::BehaviorModuleDefinition,
            key: &str,
        ) -> Result<u32, String> {
            match module.attribute(key) {
                None => Ok(0),
                Some(value) => {
                    // CPP parses unsigned milliseconds, then rounds its Real
                    // frame conversion up. Invalid authored data is retained
                    // as a rejection, never silently replaced by defaults.
                    let ms = value
                        .trim()
                        .parse::<u32>()
                        .map_err(|_| invalid(key, value))?;
                    let frames = (ms as f32 * (30.0_f32 / 1000.0_f32)).ceil();
                    if !frames.is_finite() || frames as f64 > u32::MAX as f64 {
                        return Err(invalid(key, value));
                    }
                    Ok(frames as u32)
                }
            }
        }
        fn real(
            module: &crate::assets::BehaviorModuleDefinition,
            key: &str,
            default: f32,
        ) -> Result<f32, String> {
            let value = module
                .attribute(key)
                .map(str::parse::<f32>)
                .transpose()
                .map_err(|_| invalid(key, module.attribute(key).unwrap_or("")))?
                .unwrap_or(default);
            if !value.is_finite() {
                return Err(invalid(key, module.attribute(key).unwrap_or("")));
            }
            Ok(value)
        }
        fn name(module: &crate::assets::BehaviorModuleDefinition, key: &str) -> String {
            module.attribute(key).unwrap_or("").trim().to_string()
        }
        fn flags<T>(
            module: &crate::assets::BehaviorModuleDefinition,
            key: &str,
            default: T,
            parse: impl FnOnce(&[&str]) -> Result<T, game_engine::common::ini::INIError>,
        ) -> Result<T, String> {
            let Some(value) = module.attribute(key) else {
                return Ok(default);
            };
            let tokens: Vec<_> = value.split_whitespace().collect();
            parse(&tokens).map_err(|_| invalid(key, value))
        }
        let mut die_mux = gamelogic::object::die::DieMuxData::default();
        die_mux.death_types = flags(
            module,
            "DeathTypes",
            die_mux.death_types,
            gamelogic::object::die::parse_death_type_flags_tokens,
        )?;
        die_mux.veterancy_levels = flags(
            module,
            "VeterancyLevels",
            die_mux.veterancy_levels,
            gamelogic::object::die::parse_veterancy_level_flags_tokens,
        )?;
        die_mux.exempt_status = flags(
            module,
            "ExemptStatus",
            die_mux.exempt_status,
            gamelogic::object::die::parse_object_status_mask_tokens,
        )?;
        die_mux.required_status = flags(
            module,
            "RequiredStatus",
            die_mux.required_status,
            gamelogic::object::die::parse_object_status_mask_tokens,
        )?;
        let damage_fx_types = flags(
            module,
            "DamageFXTypes",
            gamelogic::damage::DamageTypeFlags::all_flags(),
            gamelogic::damage::flag_parsing::parse_damage_type_flags_tokens,
        )?;
        let angle_fx = match module.attribute("AngleFX") {
            None => Vec::new(),
            Some(value) => {
                let mut angles = Vec::new();
                for line in value.lines() {
                    let pair: Vec<_> = line.split_whitespace().collect();
                    if pair.len() != 2 {
                        return Err(invalid("AngleFX", value));
                    }
                    let degrees = pair[0]
                        .parse::<f32>()
                        .map_err(|_| invalid("AngleFX", value))?;
                    let angle = degrees * std::f32::consts::PI / 180.0;
                    if !angle.is_finite() {
                        return Err(invalid("AngleFX", value));
                    }
                    if !pair[1].eq_ignore_ascii_case("None") {
                        angles.push((angle, pair[1].to_owned()));
                    }
                }
                if value.is_empty() {
                    return Err(invalid("AngleFX", value));
                }
                angles
            }
        };
        Ok(Self {
            min_delay: duration(module, "MinToppleDelay")?,
            max_delay: duration(module, "MaxToppleDelay")?,
            integrity: real(module, "StructuralIntegrity", 0.1)?,
            decay: real(module, "StructuralDecay", 0.0)?,
            fx: AuthoredStructureTopplePeel {
                weapon: name(module, "CrushingWeaponName"),
                fx: name(module, "CrushingFX"),
                start_fx: name(module, "ToppleStartFX"),
                delay_fx: name(module, "ToppleDelayFX"),
                done_fx: name(module, "ToppleDoneFX"),
                angle_fx,
                min_burst_delay: duration(module, "MinToppleBurstDelay")?,
                max_burst_delay: duration(module, "MaxToppleBurstDelay")?,
            },
            die_mux,
            damage_fx_types,
        })
    }
}
