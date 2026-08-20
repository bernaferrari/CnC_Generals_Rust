//! Host FXListDie residual (play authored DeathFX on die).
//!
//! C++: `FXListDie::onDie` — require upgrade active (`StartsActive` default TRUE
//! via ctor `giveSelfUpgrade`), DieMux, skip if object or player `ConflictsWith`
//! mask matches, then `FXList::doFXObj` (`OrientToObject` default TRUE) or
//! `doFXPos` with authored `DeathFX`.

use serde::{Deserialize, Serialize};

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
            fired: false,
        }
    }
}

impl HostFxListDieData {
    pub fn with_fx(fx: &str) -> Self {
        Self {
            death_fx: Some(fx.into()),
            death_audio: None,
            orient_to_object: true,
            starts_active: true,
            upgrade_active: true,
            conflicts_with: Vec::new(),
            fired: false,
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

    /// Fire once on die. Returns (fx, audio) names.
    pub fn on_die(
        &mut self,
        owned_upgrades: &[String],
    ) -> Option<(Option<String>, Option<String>)> {
        if self.fired {
            return None;
        }
        if !self.upgrade_active {
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
        fired: false,
    }
}

fn authored_fx_list_die(name: &str) -> Option<HostFxListDieData> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
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
            return Some(data);
        }
    }
    None
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
        let (fx, _) = d.on_die(&[]).unwrap();
        assert_eq!(fx.as_deref(), Some("FX_Test"));
        assert!(d.on_die(&[]).is_none());
    }

    #[test]
    fn starts_active_no_does_not_fire() {
        let mut d = HostFxListDieData {
            death_fx: Some("FX_Hidden".into()),
            starts_active: false,
            upgrade_active: false,
            ..Default::default()
        };
        assert!(d.on_die(&[]).is_none());
    }

    #[test]
    fn conflicts_with_owned_skips() {
        let mut d = HostFxListDieData {
            death_fx: Some("FX_Base".into()),
            conflicts_with: vec!["Upgrade_GLAAnthraxBeta".into()],
            ..Default::default()
        };
        assert!(d
            .on_die(&["Upgrade_GLAAnthraxBeta".into()])
            .is_none());
        let mut d2 = HostFxListDieData {
            death_fx: Some("FX_Base".into()),
            conflicts_with: vec!["Upgrade_GLAAnthraxBeta".into()],
            ..Default::default()
        };
        assert!(d2.on_die(&[]).is_some());
    }

    #[test]
    fn authored_attrs_default_orient_and_starts_active() {
        let d = fx_list_die_from_behavior_attrs(&[("DeathFX", "FX_AuthoredDie")]);
        assert_eq!(d.death_fx.as_deref(), Some("FX_AuthoredDie"));
        assert!(d.orient_to_object);
        assert!(d.starts_active);
        assert!(d.upgrade_active);
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
}
