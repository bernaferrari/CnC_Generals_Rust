//! Host InstantDeathBehavior residual (FX / OCL / Weapon then destroyObject).
//!
//! C++: `InstantDeathBehavior::onDie` — DieMux, mark AI dead, pick one authored
//! FX / OCL / Weapon via `GameLogicRandomValue`, then `destroyObject`.
//!
//! Live path attaches from Object INI `Behavior = InstantDeathBehavior` (or
//! peel fallbacks) and runs from `mark_object_for_destruction`.

use crate::game_logic::host_usa_pilot::HostDeathType;
use serde::{Deserialize, Serialize};

/// C++ InstantDeath DeathTypes residual (SpectreHowitzerShell table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HostInstantDeathTypes {
    #[default]
    All,
    DetonatedOnly,
    LaseredOnly,
    GenericNotLaseredDetonated,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostInstantDeathIni {
    pub fx: Vec<String>,
    pub ocls: Vec<String>,
    pub weapons: Vec<String>,
    pub required_under_construction: bool,
    pub death_types: HostInstantDeathTypes,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostInstantDeathBurst {
    pub fx: Option<String>,
    pub ocl: Option<String>,
    pub weapon: Option<String>,
}

impl HostInstantDeathIni {
    pub fn is_applicable(&self, under_construction: bool, death_type: HostDeathType) -> bool {
        if self.required_under_construction && !under_construction {
            return false;
        }
        match self.death_types {
            HostInstantDeathTypes::All => true,
            HostInstantDeathTypes::DetonatedOnly => {
                matches!(death_type, HostDeathType::Detonated)
            }
            HostInstantDeathTypes::LaseredOnly => matches!(death_type, HostDeathType::Lasered),
            HostInstantDeathTypes::GenericNotLaseredDetonated => !matches!(
                death_type,
                HostDeathType::Lasered | HostDeathType::Detonated
            ),
        }
    }

    /// C++ `GameLogicRandomValue(0, listSize-1)` residual — seed from object id.
    pub fn pick(&self, seed: u32) -> HostInstantDeathBurst {
        HostInstantDeathBurst {
            fx: pick_one(&self.fx, seed),
            ocl: pick_one(&self.ocls, seed.wrapping_add(1)),
            weapon: pick_one(&self.weapons, seed.wrapping_add(2)),
        }
    }
}

fn pick_one(items: &[String], seed: u32) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    Some(items[(seed as usize) % items.len()].clone())
}

fn split_names(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("None"))
        .map(|s| s.to_string())
        .collect()
}

pub fn parse_instant_death_types(raw: &str) -> HostInstantDeathTypes {
    let u = raw.to_ascii_uppercase();
    let plus_det = u.contains("+DETONATED");
    let plus_las = u.contains("+LASERED");
    let minus_det = u.contains("-DETONATED");
    let minus_las = u.contains("-LASERED");
    if plus_det && !plus_las {
        HostInstantDeathTypes::DetonatedOnly
    } else if plus_las && !plus_det {
        HostInstantDeathTypes::LaseredOnly
    } else if minus_las && minus_det {
        HostInstantDeathTypes::GenericNotLaseredDetonated
    } else {
        HostInstantDeathTypes::All
    }
}

pub fn instant_death_ini_from_behavior_attrs(attrs: &[(&str, &str)]) -> HostInstantDeathIni {
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(key))
            .map(|(_, v)| *v)
    };
    let required = get("RequiredStatus")
        .map(|s| s.to_ascii_uppercase().contains("UNDER_CONSTRUCTION"))
        .unwrap_or(false);
    HostInstantDeathIni {
        fx: get("FX").map(split_names).unwrap_or_default(),
        ocls: get("OCL").map(split_names).unwrap_or_default(),
        weapons: get("Weapon").map(split_names).unwrap_or_default(),
        required_under_construction: required,
        death_types: get("DeathTypes")
            .map(parse_instant_death_types)
            .unwrap_or_default(),
    }
}

fn authored_instant_death_modules(name: &str) -> Vec<HostInstantDeathIni> {
    let Some(manager) = crate::assets::get_asset_manager() else {
        return Vec::new();
    };
    let Ok(manager) = manager.lock() else {
        return Vec::new();
    };
    let Some(definition) = manager.get_object_definition(name) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for module in &definition.behavior_modules {
        if !module
            .class_name
            .eq_ignore_ascii_case("InstantDeathBehavior")
        {
            continue;
        }
        let attrs: Vec<(&str, &str)> = module
            .attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let ini = instant_death_ini_from_behavior_attrs(&attrs);
        if !ini.fx.is_empty() || !ini.ocls.is_empty() || !ini.weapons.is_empty() {
            out.push(ini);
        }
    }
    out
}

/// Retail InstantDeath peels when Object INI is not loaded.
fn peel_instant_death_modules(name: &str) -> Vec<HostInstantDeathIni> {
    let n = name.to_ascii_lowercase();
    if n.contains("spectrehowitzer") || n.contains("spectre_howitzer") {
        return vec![
            HostInstantDeathIni {
                fx: vec!["FX_NukeGLA".into()],
                death_types: HostInstantDeathTypes::DetonatedOnly,
                ..Default::default()
            },
            HostInstantDeathIni {
                fx: vec!["FX_GenericMissileDisintegrate".into()],
                ocls: vec!["OCL_GenericMissileDisintegrate".into()],
                death_types: HostInstantDeathTypes::LaseredOnly,
                ..Default::default()
            },
            HostInstantDeathIni {
                fx: vec!["FX_GenericMissileDeath".into()],
                death_types: HostInstantDeathTypes::GenericNotLaseredDetonated,
                ..Default::default()
            },
        ];
    }
    if n.contains("missile")
        || n.contains("rocket")
        || (n.contains("shell") && !n.contains("artillery"))
    {
        return vec![HostInstantDeathIni {
            fx: vec!["FX_GenericMissileDeath".into()],
            death_types: HostInstantDeathTypes::All,
            ..Default::default()
        }];
    }
    Vec::new()
}

/// Under-construction buildings: InstantDeath RequiredStatus UNDER_CONSTRUCTION.
fn peel_under_construction_instant_death(name: &str) -> Option<HostInstantDeathIni> {
    let n = name.to_ascii_lowercase();
    if n.contains("building")
        || n.contains("factory")
        || n.contains("barracks")
        || n.contains("center")
        || n.contains("plant")
        || n.contains("cannon")
        || n.contains("uplink")
        || n.contains("command")
        || n.contains("warfactory")
        || n.contains("airfield")
        || n.contains("power")
        || n.contains("supply")
        || n.contains("tunnel")
        || n.contains("palace")
        || n.contains("scudstorm")
        || n.contains("particle")
    {
        return Some(HostInstantDeathIni {
            fx: vec!["FX_StructureMediumDeath".into()],
            ocls: vec!["OCL_ABPowerPlantExplode".into()],
            required_under_construction: true,
            death_types: HostInstantDeathTypes::All,
            ..Default::default()
        });
    }
    None
}

pub fn instant_death_modules_for_template(name: &str) -> Vec<HostInstantDeathIni> {
    let authored = authored_instant_death_modules(name);
    if !authored.is_empty() {
        return authored;
    }
    let mut peels = peel_instant_death_modules(name);
    if let Some(uc) = peel_under_construction_instant_death(name) {
        peels.push(uc);
    }
    peels
}

pub fn first_applicable_instant_death(
    name: &str,
    under_construction: bool,
    death_type: HostDeathType,
) -> Option<HostInstantDeathIni> {
    instant_death_modules_for_template(name)
        .into_iter()
        .find(|ini| ini.is_applicable(under_construction, death_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectre_table_picks_by_death_type() {
        let mods = peel_instant_death_modules("SpectreHowitzerShell");
        assert_eq!(mods.len(), 3);
        let det = mods
            .iter()
            .find(|m| m.is_applicable(false, HostDeathType::Detonated))
            .unwrap();
        assert_eq!(det.pick(0).fx.as_deref(), Some("FX_NukeGLA"));
        let las = mods
            .iter()
            .find(|m| m.is_applicable(false, HostDeathType::Lasered))
            .unwrap();
        assert_eq!(
            las.pick(0).ocl.as_deref(),
            Some("OCL_GenericMissileDisintegrate")
        );
        let generic = mods
            .iter()
            .find(|m| m.is_applicable(false, HostDeathType::Normal))
            .unwrap();
        assert_eq!(
            generic.pick(0).fx.as_deref(),
            Some("FX_GenericMissileDeath")
        );
    }

    #[test]
    fn under_construction_required_status() {
        let uc = peel_under_construction_instant_death("AmericaParticleUplinkCannon").unwrap();
        assert!(uc.required_under_construction);
        assert!(uc.is_applicable(true, HostDeathType::Normal));
        assert!(!uc.is_applicable(false, HostDeathType::Normal));
        assert_eq!(uc.ocls[0], "OCL_ABPowerPlantExplode");
    }

    #[test]
    fn parse_death_types_tokens() {
        assert_eq!(
            parse_instant_death_types("NONE +DETONATED"),
            HostInstantDeathTypes::DetonatedOnly
        );
        assert_eq!(
            parse_instant_death_types("NONE +LASERED"),
            HostInstantDeathTypes::LaseredOnly
        );
        assert_eq!(
            parse_instant_death_types("ALL -LASERED -DETONATED"),
            HostInstantDeathTypes::GenericNotLaseredDetonated
        );
    }

    #[test]
    fn from_modules_attrs() {
        let ini = instant_death_ini_from_behavior_attrs(&[
            ("FX", "FX_A FX_B"),
            ("OCL", "OCL_X"),
            ("Weapon", "DeathWeapon"),
            ("RequiredStatus", "UNDER_CONSTRUCTION"),
            ("DeathTypes", "NONE +DETONATED"),
        ]);
        assert_eq!(ini.fx.len(), 2);
        assert_eq!(ini.ocls[0], "OCL_X");
        assert_eq!(ini.weapons[0], "DeathWeapon");
        assert!(ini.required_under_construction);
        assert_eq!(ini.death_types, HostInstantDeathTypes::DetonatedOnly);
    }
}
