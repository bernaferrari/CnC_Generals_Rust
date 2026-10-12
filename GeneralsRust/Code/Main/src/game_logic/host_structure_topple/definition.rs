//! Immutable retained INI data. Runtime state remains on the actual Object.
use super::*;

#[derive(Debug, Clone)]
pub(crate) struct StructureToppleDefinition {
    pub min_delay: u32,
    pub max_delay: u32,
    pub integrity: f32,
    pub decay: f32,
    pub fx: AuthoredStructureTopplePeel,
}
impl Default for StructureToppleDefinition {
    fn default() -> Self {
        Self {
            min_delay: 0,
            max_delay: 0,
            integrity: 0.1,
            decay: 0.0,
            fx: AuthoredStructureTopplePeel::default(),
        }
    }
}
impl StructureToppleDefinition {
    pub(crate) fn from_module(module: &crate::assets::BehaviorModuleDefinition) -> Option<Self> {
        fn duration(module: &crate::assets::BehaviorModuleDefinition, key: &str) -> Option<u32> {
            match module.attribute(key) {
                None => Some(0),
                Some(value) => {
                    let ms = value.trim().parse::<f64>().ok()?;
                    let frames = (ms * 30.0 / 1000.0).ceil();
                    (frames.is_finite() && frames >= 0.0 && frames <= u32::MAX as f64)
                        .then_some(frames as u32)
                }
            }
        }
        fn real(
            module: &crate::assets::BehaviorModuleDefinition,
            key: &str,
            default: f32,
        ) -> Option<f32> {
            let value = module
                .attribute(key)
                .map(str::parse::<f32>)
                .transpose()
                .ok()?
                .unwrap_or(default);
            value.is_finite().then_some(value)
        }
        fn name(module: &crate::assets::BehaviorModuleDefinition, key: &str) -> String {
            module.attribute(key).unwrap_or("").trim().to_string()
        }
        Some(Self {
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
                angle_fx: module
                    .attribute("AngleFX")
                    .map(parse_ini_angle_fx_field)
                    .unwrap_or_default(),
                min_burst_delay: duration(module, "MinToppleBurstDelay")?,
                max_burst_delay: duration(module, "MaxToppleBurstDelay")?,
            },
        })
    }
}
