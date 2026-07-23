//! Host FXListDie residual (play DeathFX on die).
//!
//! C++: `FXListDie::onDie` → `FXList::doFXObj` / doFXPos at object.
//! Residual: queue audio + presentation FX name when object is destroyed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostFxListDieData {
    pub death_fx: Option<String>,
    pub death_audio: Option<String>,
    pub orient_to_object: bool,
    pub fired: bool,
}

impl HostFxListDieData {
    pub fn with_fx(fx: &str) -> Self {
        Self {
            death_fx: Some(fx.into()),
            death_audio: None,
            orient_to_object: false,
            fired: false,
        }
    }

    /// Fire once on die. Returns (fx, audio) names.
    pub fn on_die(&mut self) -> Option<(Option<String>, Option<String>)> {
        if self.fired {
            return None;
        }
        if self.death_fx.is_none() && self.death_audio.is_none() {
            return None;
        }
        self.fired = true;
        Some((self.death_fx.clone(), self.death_audio.clone()))
    }
}

pub fn fx_list_die_config_for_template(name: &str) -> Option<HostFxListDieData> {
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
    // Generic vehicle/structure death FX residual.
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
        let (fx, _) = d.on_die().unwrap();
        assert_eq!(fx.as_deref(), Some("FX_Test"));
        assert!(d.on_die().is_none());
    }
}
