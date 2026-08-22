//! Host FXListDie residual (play authored DeathFX on die).
//!
//! C++: `FXListDie::onDie` — require upgrade active (`StartsActive` default TRUE
//! via ctor `giveSelfUpgrade`), DieMux, skip if object or player `ConflictsWith`
//! mask matches, then `FXList::doFXObj` (`OrientToObject` default TRUE) or
//! `doFXPos` with authored `DeathFX`.
use serde::{Deserialize, Serialize};

use crate::game_logic::host_usa_pilot::HostDeathType;

fn default_death_types() -> u32 {
    gamelogic::damage::DEATH_TYPE_FLAGS_ALL
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFxListDieData {
    pub death_fx: Option<String>,
    pub death_audio: Option<String>,
    /// C++ `m_orientToObject` default TRUE.
    pub orient_to_object: bool,
    /// C++ `m_initiallyActive` / StartsActive default TRUE.
    pub starts_active: bool,
    /// C++ `UpgradeMux::isAlreadyUpgraded` after ctor `giveSelfUpgrade`.
    pub upgrade_active: bool,
    /// C++ `ConflictsWith` upgrade names.
    pub conflicts_with: Vec<String>,
    /// C++ `DieMuxData::m_deathTypes` (`DEATH_TYPE_FLAGS_ALL` default).
    #[serde(default = "default_death_types")]
    pub death_types: u32,
    /// Additional authored `FXListDie` modules on the same template.
    #[serde(default)]
    pub more: Vec<HostFxListDieData>,
    pub fired: bool,
}

impl Default for HostFxListDieData {
    fn default() -> Self {
        Self {
            death_fx: None,
            death_audio: None,
            orient_to_object: true,
            starts_active: true,
            upgrade_active: true,
            conflicts_with: Vec::new(),
            death_types: default_death_types(),
            more: Vec::new(),
            fired: false,
        }
    }
}

impl HostFxListDieData {
    pub fn with_fx(fx: &str) -> Self {
        Self {
            death_fx: Some(fx.into()),
            ..Self::default()
        }
    }

    pub fn conflicts_with_owned(&self, owned: &[String]) -> bool {
        !self.conflicts_with.is_empty()
            && self.conflicts_with.iter().any(|c| {
                owned
                    .iter()
                    .any(|o| o.eq_ignore_ascii_case(c))
            })
    }

    /// C++ `getDeathTypeFlag` — `1UL << (dt - 1)` (NORMAL wraps to bit 31).
    pub fn death_type_allowed(&self, death_type: HostDeathType) -> bool {
        let shift = (death_type.ordinal() as u32).wrapping_sub(1) & 31;
        (self.death_types & (1u32 << shift)) != 0
    }

    fn try_fire_self(
        &mut self,
        owned_upgrades: &[String],
        death_type: HostDeathType,
    ) -> Option<(Option<String>, Option<String>)> {
        if self.fired {
            return None;
        }
        if !self.upgrade_active {
            return None;
        }
        if !self.death_type_allowed(death_type) {
            return None;
        }
        if self.conflicts_with_owned(owned_upgrades) {
            return None;
        }
        if self.death_fx.is_none() && self.death_audio.is_none() {
            return None;
        }
        self.fired = true;
        Some((self.death_fx.clone(), self.death_audio.clone()))
    }

    /// Fire every applicable authored `FXListDie` (C++ walks all Die modules).
    pub fn collect_applicable(
        &mut self,
        owned_upgrades: &[String],
        death_type: HostDeathType,
    ) -> Vec<(Option<String>, Option<String>)> {
        let mut out = Vec::new();
        if let Some(hit) = self.try_fire_self(owned_upgrades, death_type) {
            out.push(hit);
        }
        for extra in &mut self.more {
            if let Some(hit) = extra.try_fire_self(owned_upgrades, death_type) {
                out.push(hit);
            }
        }
        out
    }

    /// Fire once on die. Returns the first applicable (fx, audio) pair.
    pub fn on_die(
        &mut self,
        owned_upgrades: &[String],
        death_type: HostDeathType,
    ) -> Option<(Option<String>, Option<String>)> {
        self.collect_applicable(owned_upgrades, death_type)
            .into_iter()
            .next()
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("yes") || t.eq_ignore_ascii_case("true") {
        Some(true)
    } else if t.eq_ignore_ascii_case("no") || t.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn split_names(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("None"))
        .map(|s| s.to_string())
        .collect()
}

fn parse_death_types(raw: &str) -> u32 {
    let tokens: Vec<&str> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    gamelogic::object::die::parse_death_type_flags_tokens(&tokens)
        .unwrap_or_else(|_| default_death_types())
}

pub fn fx_list_die_from_behavior_attrs(attrs: &[(&str, &str)]) -> HostFxListDieData {
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(key))
            .map(|(_, v)| *v)
    };
    let starts_active = get("StartsActive").and_then(parse_bool).unwrap_or(true);
    let death_fx = get("DeathFX")
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("None"))
        .map(|s| s.to_string());
    HostFxListDieData {
        death_fx,
        death_audio: None,
        orient_to_object: get("OrientToObject").and_then(parse_bool).unwrap_or(true),
        starts_active,
        upgrade_active: starts_active,
        conflicts_with: get("ConflictsWith").map(split_names).unwrap_or_default(),
        death_types: get("DeathTypes")
            .map(parse_death_types)
            .unwrap_or_else(default_death_types),
        more: Vec::new(),
        fired: false,
    }
}

fn authored_fx_list_die(name: &str) -> Option<HostFxListDieData> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
    let mut found = Vec::new();
    for module in &definition.behavior_modules {
        if !module.class_name.eq_ignore_ascii_case("FXListDie") {
            continue;
        }
        let attrs: Vec<(&str, &str)> = module
            .attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let data = fx_list_die_from_behavior_attrs(&attrs);
        if data.death_fx.is_some() {
            found.push(data);
        }
    }
    let mut first = found.drain(..).next()?;
    first.more = found;
    Some(first)
}

pub fn fx_list_die_config_for_template(name: &str) -> Option<HostFxListDieData> {
    if let Some(authored) = authored_fx_list_die(name) {
        return Some(authored);
    }
    let n = name.to_ascii_lowercase();
    if n.contains("bombtruck") || n.contains("demotruck") {
        return Some(HostFxListDieData {
            death_fx: Some("WeaponFX_BombTruckHighExplosiveBombDetonation".into()),
            death_audio: Some("ExplosionBombTruck".into()),
            ..Default::default()
        });
    }
    if n.contains("terrorist") {
        return Some(HostFxListDieData {
            death_fx: Some("WeaponFX_TerroristDynamitePackDetonation".into()),
            death_audio: Some("ExplosionTerrorist".into()),
            ..Default::default()
        });
    }
    if n.contains("scud") && n.contains("missile") {
        return Some(HostFxListDieData {
            death_fx: Some("FX_ScudMissileDie".into()),
            death_audio: Some("ExplosionScud".into()),
            ..Default::default()
        });
    }
    if n.contains("nuke") && n.contains("missile") {
        return Some(HostFxListDieData {
            death_fx: Some("FX_NukeMissileDie".into()),
            death_audio: Some("ExplosionNuke".into()),
            ..Default::default()
        });
    }
    if n.contains("tank") || n.contains("vehicle") || n.contains("truck") {
        return Some(HostFxListDieData {
            death_fx: Some("FX_VehicleDie".into()),
            death_audio: Some("VehicleDestroyed".into()),
            ..Default::default()
        });
    }
    if n.contains("building")
        || n.contains("factory")
        || n.contains("barracks")
        || n.contains("center")
        || n.contains("plant")
    {
        return Some(HostFxListDieData {
            death_fx: Some("FX_StructureDie".into()),
            death_audio: Some("BuildingCollapse".into()),
            ..Default::default()
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx_list_die_fires_once() {
        let mut d = HostFxListDieData::with_fx("FX_Test");
        let (fx, _) = d.on_die(&[], HostDeathType::Normal).unwrap();
        assert_eq!(fx.as_deref(), Some("FX_Test"));
        assert!(d.on_die(&[], HostDeathType::Normal).is_none());
    }

    #[test]
    fn starts_active_no_does_not_fire() {
        let mut d = HostFxListDieData {
            death_fx: Some("FX_Hidden".into()),
            starts_active: false,
            upgrade_active: false,
            ..Default::default()
        };
        assert!(d.on_die(&[], HostDeathType::Normal).is_none());
    }

    #[test]
    fn conflicts_with_owned_skips() {
        let mut d = HostFxListDieData {
            death_fx: Some("FX_Base".into()),
            conflicts_with: vec!["Upgrade_GLAAnthraxBeta".into()],
            ..Default::default()
        };
        assert!(d
            .on_die(&["Upgrade_GLAAnthraxBeta".into()], HostDeathType::Normal)
            .is_none());
        let mut d2 = HostFxListDieData {
            death_fx: Some("FX_Base".into()),
            conflicts_with: vec!["Upgrade_GLAAnthraxBeta".into()],
            ..Default::default()
        };
        assert!(d2.on_die(&[], HostDeathType::Normal).is_some());
    }

    #[test]
    fn authored_attrs_default_orient_and_starts_active() {
        let d = fx_list_die_from_behavior_attrs(&[("DeathFX", "FX_AuthoredDie")]);
        assert_eq!(d.death_fx.as_deref(), Some("FX_AuthoredDie"));
        assert!(d.orient_to_object);
        assert!(d.starts_active);
        assert!(d.upgrade_active);
        assert_eq!(d.death_types, default_death_types());
    }

    #[test]
    fn authored_starts_active_no() {
        let d = fx_list_die_from_behavior_attrs(&[
            ("DeathFX", "FX_UpgradeDie"),
            ("StartsActive", "No"),
            ("ConflictsWith", "Upgrade_A"),
        ]);
        assert!(!d.starts_active);
        assert!(!d.upgrade_active);
        assert_eq!(d.conflicts_with, vec!["Upgrade_A".to_string()]);
    }

    #[test]
    fn crush_only_death_types_skip_normal() {
        let mut d = fx_list_die_from_behavior_attrs(&[
            ("DeathFX", "FX_CrushInfantry"),
            ("DeathTypes", "NONE +CRUSHED"),
        ]);
        assert!(d.on_die(&[], HostDeathType::Normal).is_none());
        assert!(d.on_die(&[], HostDeathType::Burned).is_none());
        let (fx, _) = d.on_die(&[], HostDeathType::Crushed).unwrap();
        assert_eq!(fx.as_deref(), Some("FX_CrushInfantry"));
    }
}
