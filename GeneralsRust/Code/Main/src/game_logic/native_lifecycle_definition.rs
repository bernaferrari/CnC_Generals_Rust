//! Immutable authored definitions for C++ LifetimeUpdate, DeletionUpdate,
//! HeightDieUpdate, FireWeaponUpdate and EMPUpdate. Parsing does not publish a simulation owner.
use crate::assets::ini_parser::BehaviorModuleDefinition;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct NativeLifecycleDefinition {
    pub(crate) module_tag: Option<String>,
    pub(crate) kind: NativeLifecycleKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum NativeLifecycleKind {
    Lifetime(LifetimeFrameRange),
    Deletion(LifetimeFrameRange),
    HeightDie(HeightDieDefinition),
    FireWeapon(FireWeaponUpdateDefinition),
    Emp(EmpUpdateDefinition),
    FireOclAfterCooldown(FireOclAfterCooldownDefinition),
}

impl NativeLifecycleDefinition {
    /// A versioned snapshot decoder must validate decoded configuration before
    /// admitting it, just as the INI parser validates authored configuration.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let valid = match &self.kind {
            NativeLifecycleKind::Lifetime(range) | NativeLifecycleKind::Deletion(range) => {
                range.minimum <= range.maximum
            }
            NativeLifecycleKind::HeightDie(height) => {
                height.target_height.is_finite()
                    && height.destroy_attached_particles_at_height.is_finite()
            }
            NativeLifecycleKind::FireWeapon(_) => true,
            NativeLifecycleKind::Emp(emp) => emp.is_valid(),
            // C++ PRIMARY/SECONDARY/TERTIARY are the only authored slots.
            NativeLifecycleKind::FireOclAfterCooldown(definition) => definition.slot < 3,
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "invalid lifecycle configuration for {}",
                self.module_tag.as_deref().unwrap_or("<untagged>"),
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct LifetimeFrameRange {
    pub(crate) minimum: u32,
    pub(crate) maximum: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct HeightDieDefinition {
    pub(crate) target_height: f32,
    pub(crate) target_height_includes_structures: bool,
    pub(crate) only_when_moving_down: bool,
    pub(crate) destroy_attached_particles_at_height: f32,
    pub(crate) snap_to_ground_on_death: bool,
    pub(crate) initial_delay: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FireWeaponUpdateDefinition {
    pub(crate) weapon_name: String,
    pub(crate) initial_delay: u32,
    pub(crate) exclusive_delay: u32,
}

/// C++ FireOCLAfterWeaponCooldownUpdateModuleData authored fields
/// (FireOCLAfterWeaponCooldownUpdate.cpp:37-63).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FireOclAfterCooldownDefinition {
    /// Authored `WeaponSlot`; 0=PRIMARY, 1=SECONDARY, 2=TERTIARY.
    pub(crate) slot: u8,
    /// `OCL` nugget name; `None` when authored as `None` or omitted.
    pub(crate) ocl: Option<String>,
    /// `MinShotsToCreateOCL` (default 1).
    pub(crate) min_shots: u32,
    /// `OCLLifetimePerSecond` in milliseconds (default 1000).
    pub(crate) lifetime_per_second: u32,
    /// `OCLLifetimeMaxCap` in logic frames (default 1000).
    pub(crate) max_frames: u32,
    /// UpgradeMuxData `TriggeredBy`/`ConflictsWith`/`RequiresAllTriggers`.
    pub(crate) upgrades: super::thing::UpgradeMuxTriggers,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EmpUpdateDefinition {
    pub(super) lifetime_frames: u32,
    pub(super) start_fade_frames: u32,
    pub(super) disabled_duration_frames: u32,
    pub(super) start_scale: f32,
    pub(super) target_scale_min: f32,
    pub(super) target_scale_max: f32,
    pub(super) radius: f32,
    pub(super) start_color: [f32; 3],
    pub(super) end_color: [f32; 3],
    pub(super) sparks: Option<Arc<str>>,
    pub(super) sparks_per_cubic_foot: f32,
    pub(super) exempts_own_buildings: bool,
    pub(super) exempts_non_structure_allies: bool,
}

impl EmpUpdateDefinition {
    fn is_valid(&self) -> bool {
        self.start_fade_frames < self.lifetime_frames
            && [
                self.start_scale,
                self.target_scale_min,
                self.target_scale_max,
                self.radius,
                self.sparks_per_cubic_foot,
            ]
            .into_iter()
            .all(f32::is_finite)
            && self
                .start_color
                .into_iter()
                .chain(self.end_color)
                .all(|value| (0.0..=1.0).contains(&value))
    }

    fn parse(module: &BehaviorModuleDefinition) -> Result<Self, String> {
        let color = |field, default| {
            let Some(source) = module.attribute(field) else {
                return Ok(default);
            };
            let tokens: Vec<_> = source.split_whitespace().collect();
            game_engine::common::ini::INI::parse_rgb_color(&tokens)
                .map(|(r, g, b)| [r, g, b])
                .map_err(|_| invalid(module, field))
        };
        let lifetime_frames = if module.attribute("Lifetime").is_none() {
            1
        } else {
            duration(module, "Lifetime")?
        };
        let mut exempts_non_structure_allies = false;
        for flag in module
            .attribute("DoesNotAffect")
            .unwrap_or("")
            .split_whitespace()
        {
            if flag.eq_ignore_ascii_case("NONE") || flag.eq_ignore_ascii_case("-ALLIES") {
                exempts_non_structure_allies = false;
            } else if flag.trim_start_matches('+').eq_ignore_ascii_case("ALLIES") {
                exempts_non_structure_allies = true;
            }
        }
        let definition = Self {
            lifetime_frames,
            start_fade_frames: duration(module, "StartFadeTime")?,
            disabled_duration_frames: duration(module, "DisabledDuration")?,
            start_scale: real(module, "StartScale", 1.0)?,
            target_scale_min: real(module, "TargetScaleMin", 1.0)?,
            target_scale_max: real(module, "TargetScaleMax", 1.0)?,
            radius: real(module, "EffectRadius", 200.0)?,
            start_color: color("StartColor", [1.0; 3])?,
            end_color: color("EndColor", [0.0; 3])?,
            sparks: module
                .attribute("DisableFXParticleSystem")
                .and_then(|value| value.split_whitespace().next())
                .filter(|name| !name.eq_ignore_ascii_case("NONE"))
                .map(Arc::from),
            sparks_per_cubic_foot: real(module, "SparksPerCubicFoot", 0.001)?,
            exempts_own_buildings: boolean(module, "DoesNotAffectMyOwnBuildings")?,
            exempts_non_structure_allies,
        };
        if !definition.is_valid() {
            return Err(invalid(module, "EMPUpdate fade/lifetime"));
        }
        Ok(definition)
    }
}

impl Default for HeightDieDefinition {
    fn default() -> Self {
        Self {
            target_height: 0.0,
            target_height_includes_structures: false,
            only_when_moving_down: false,
            destroy_attached_particles_at_height: -1.0,
            snap_to_ground_on_death: false,
            initial_delay: 0,
        }
    }
}

/// Retain every authored module, including duplicate classes, in source order.
pub(crate) fn parse_native_lifecycle_definitions(
    modules: &[BehaviorModuleDefinition],
) -> Result<Vec<NativeLifecycleDefinition>, String> {
    let mut result = Vec::new();
    for module in modules {
        let kind = match module.class_name.as_str() {
            name if name.eq_ignore_ascii_case("LifetimeUpdate") => {
                NativeLifecycleKind::Lifetime(parse_range(module)?)
            }
            name if name.eq_ignore_ascii_case("DeletionUpdate") => {
                NativeLifecycleKind::Deletion(parse_range(module)?)
            }
            name if name.eq_ignore_ascii_case("HeightDieUpdate") => {
                NativeLifecycleKind::HeightDie(HeightDieDefinition {
                    target_height: real(module, "TargetHeight", 0.0)?,
                    target_height_includes_structures: boolean(
                        module,
                        "TargetHeightIncludesStructures",
                    )?,
                    only_when_moving_down: boolean(module, "OnlyWhenMovingDown")?,
                    destroy_attached_particles_at_height: real(
                        module,
                        "DestroyAttachedParticlesAtHeight",
                        -1.0,
                    )?,
                    snap_to_ground_on_death: boolean(module, "SnapToGroundOnDeath")?,
                    initial_delay: duration(module, "InitialDelay")?,
                })
            }
            name if name.eq_ignore_ascii_case("FireWeaponUpdate") => {
                NativeLifecycleKind::FireWeapon(FireWeaponUpdateDefinition {
                    weapon_name: module.attribute("Weapon").unwrap_or("").trim().to_owned(),
                    initial_delay: duration(module, "InitialDelay")?,
                    exclusive_delay: duration(module, "ExclusiveWeaponDelay")?,
                })
            }
            name if name.eq_ignore_ascii_case("EMPUpdate") => {
                NativeLifecycleKind::Emp(EmpUpdateDefinition::parse(module)?)
            }
            name if name.eq_ignore_ascii_case("FireOCLAfterWeaponCooldownUpdate") => {
                let token = |field| {
                    module
                        .attribute(field)
                        .and_then(|value| value.split_whitespace().next())
                };
                // INI::parseLookupList over TheWeaponSlotTypeNamesLookupList.
                let slot = match token("WeaponSlot") {
                    None => 0,
                    Some(value) if value.eq_ignore_ascii_case("PRIMARY") => 0,
                    Some(value) if value.eq_ignore_ascii_case("SECONDARY") => 1,
                    Some(value) if value.eq_ignore_ascii_case("TERTIARY") => 2,
                    Some(_) => return Err(invalid(module, "WeaponSlot")),
                };
                let ocl = token("OCL")
                    .filter(|name| !name.eq_ignore_ascii_case("None"))
                    .map(str::to_owned);
                let max_frames = if module.attribute("OCLLifetimeMaxCap").is_none() {
                    1000
                } else {
                    duration(module, "OCLLifetimeMaxCap")?
                };
                NativeLifecycleKind::FireOclAfterCooldown(FireOclAfterCooldownDefinition {
                    slot,
                    ocl,
                    min_shots: unsigned(module, "MinShotsToCreateOCL", 1)?,
                    lifetime_per_second: unsigned(module, "OCLLifetimePerSecond", 1000)?,
                    max_frames,
                    upgrades: crate::game_logic::world_objects::authored_upgrade_modules::upgrade_mux_data(
                        module,
                    ),
                })
            }
            _ => continue,
        };
        result.push(NativeLifecycleDefinition {
            module_tag: module.module_tag.clone(),
            kind,
        });
    }
    Ok(result)
}

fn invalid(module: &BehaviorModuleDefinition, field: &str) -> String {
    format!(
        "{} {}: invalid {field}",
        module.class_name,
        module.module_tag.as_deref().unwrap_or("<untagged>"),
    )
}

fn parse_range(module: &BehaviorModuleDefinition) -> Result<LifetimeFrameRange, String> {
    let minimum = duration(module, "MinLifetime")?;
    let maximum = duration(module, "MaxLifetime")?;
    // Reject malformed authoring rather than silently sorting or extending it.
    if minimum > maximum {
        return Err(invalid(module, "MinLifetime > MaxLifetime"));
    }
    Ok(LifetimeFrameRange { minimum, maximum })
}

/// C++ scanUnsignedInt accepts a decimal prefix (e.g. 95ms, or 1s);
/// the suffix does not change the parsed value.
fn unsigned_prefix(
    module: &BehaviorModuleDefinition,
    field: &str,
    value: &str,
) -> Result<u32, String> {
    let token = value.trim_start();
    let token = token.strip_prefix('+').unwrap_or(token);
    let end = token.bytes().take_while(u8::is_ascii_digit).count();
    token[..end]
        .parse::<u32>()
        .map_err(|_| invalid(module, field))
}

/// INI::parseUnsignedInt with a caller default for an absent field.
fn unsigned(
    module: &BehaviorModuleDefinition,
    field: &str,
    default: u32,
) -> Result<u32, String> {
    let Some(value) = module.attribute(field) else {
        return Ok(default);
    };
    unsigned_prefix(module, field, value)
}

fn duration(module: &BehaviorModuleDefinition, field: &str) -> Result<u32, String> {
    let Some(value) = module.attribute(field) else {
        return Ok(0);
    };
    // Units in the suffix do not change the millisecond interpretation.
    let milliseconds = unsigned_prefix(module, field, value)?;
    // GameCommon.h: Real multiplication by LOGICFRAMES_PER_MSEC_REAL;
    // INI.cpp:1693 uses ceilf, retaining original f32 conversion order.
    let frames = (milliseconds as f32 * (30.0_f32 / 1000.0_f32)).ceil();
    if !frames.is_finite() || frames as f64 > u32::MAX as f64 {
        return Err(invalid(module, field));
    }
    Ok(frames as u32)
}

fn real(module: &BehaviorModuleDefinition, field: &str, default: f32) -> Result<f32, String> {
    let Some(value) = module.attribute(field) else {
        return Ok(default);
    };
    let parsed = value
        .trim()
        .parse::<f32>()
        .map_err(|_| invalid(module, field))?;
    parsed
        .is_finite()
        .then_some(parsed)
        .ok_or_else(|| invalid(module, field))
}

fn boolean(module: &BehaviorModuleDefinition, field: &str) -> Result<bool, String> {
    match module.attribute(field).map(str::trim) {
        None => Ok(false),
        Some(value) if value.eq_ignore_ascii_case("Yes") => Ok(true),
        Some(value) if value.eq_ignore_ascii_case("No") => Ok(false),
        _ => Err(invalid(module, field)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(class: &str, tag: &str, fields: &[(&str, &str)]) -> BehaviorModuleDefinition {
        BehaviorModuleDefinition {
            class_name: class.into(),
            module_tag: Some(tag.into()),
            attributes: fields
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        }
    }

    #[test]
    fn source_order_duplicates_tags_and_cpp_defaults_are_retained() {
        let definitions = parse_native_lifecycle_definitions(&[
            module("LifetimeUpdate", "First", &[]),
            module("UnrelatedBehavior", "Ignored", &[]),
            module("DeletionUpdate", "Delete", &[]),
            module("LifetimeUpdate", "Second", &[]),
            module("HeightDieUpdate", "Height", &[]),
        ])
        .unwrap();
        assert_eq!(definitions.len(), 4);
        assert_eq!(definitions[0].module_tag.as_deref(), Some("First"));
        assert_eq!(definitions[2].module_tag.as_deref(), Some("Second"));
        assert_eq!(
            definitions[0].kind,
            NativeLifecycleKind::Lifetime(LifetimeFrameRange::default())
        );
        assert_eq!(
            definitions[1].kind,
            NativeLifecycleKind::Deletion(LifetimeFrameRange::default())
        );
        assert_eq!(
            definitions[3].kind,
            NativeLifecycleKind::HeightDie(HeightDieDefinition::default())
        );
    }

    #[test]
    fn durations_use_cpp_unsigned_prefix_and_upward_rounding() {
        for (text, frames) in [
            ("0", 0),
            ("1", 1),
            ("34", 2),
            ("95ms", 3),
            ("1s", 1),
            ("+1000", 30),
        ] {
            let source = module(
                "LifetimeUpdate",
                "Timer",
                &[("MinLifetime", text), ("MaxLifetime", text)],
            );
            assert_eq!(
                parse_range(&source).unwrap(),
                LifetimeFrameRange {
                    minimum: frames,
                    maximum: frames
                }
            );
        }
    }

    #[test]
    fn height_fields_are_complete_and_case_insensitive() {
        let source = module(
            "HeightDieUpdate",
            "Height",
            &[
                ("targetheight", "12.5"),
                ("TargetHeightIncludesStructures", "yEs"),
                ("OnlyWhenMovingDown", "YES"),
                ("DestroyAttachedParticlesAtHeight", "-2"),
                ("SnapToGroundOnDeath", "Yes"),
                ("InitialDelay", "34"),
            ],
        );
        assert_eq!(
            parse_native_lifecycle_definitions(&[source]).unwrap()[0].kind,
            NativeLifecycleKind::HeightDie(HeightDieDefinition {
                target_height: 12.5,
                target_height_includes_structures: true,
                only_when_moving_down: true,
                destroy_attached_particles_at_height: -2.0,
                snap_to_ground_on_death: true,
                initial_delay: 2,
            })
        );
    }

    #[test]
    fn invalid_authoring_is_rejected_without_inventing_a_range() {
        for fields in [
            vec![("MinLifetime", "2000"), ("MaxLifetime", "1000")],
            vec![("MaxLifetime", "-1")],
            vec![("MaxLifetime", "4294967296")],
            vec![("MaxLifetime", "none")],
        ] {
            assert!(
                parse_native_lifecycle_definitions(&[module("LifetimeUpdate", "Bad", &fields)])
                    .is_err()
            );
        }
        for fields in [
            vec![("TargetHeight", "NaN")],
            vec![("DestroyAttachedParticlesAtHeight", "inf")],
            vec![("OnlyWhenMovingDown", "true")],
        ] {
            assert!(
                parse_native_lifecycle_definitions(&[module("HeightDieUpdate", "Bad", &fields)])
                    .is_err()
            );
        }
    }

    #[test]
    fn decoded_configuration_requires_the_same_range_and_finite_checks() {
        let mut definition = NativeLifecycleDefinition {
            module_tag: Some("Saved".into()),
            kind: NativeLifecycleKind::Lifetime(LifetimeFrameRange {
                minimum: 2,
                maximum: 1,
            }),
        };
        assert!(definition.validate().is_err());
        definition.kind = NativeLifecycleKind::HeightDie(HeightDieDefinition {
            target_height: f32::NAN,
            ..HeightDieDefinition::default()
        });
        assert!(definition.validate().is_err());
        definition.kind = NativeLifecycleKind::HeightDie(HeightDieDefinition::default());
        assert!(definition.validate().is_ok());
    }
}
