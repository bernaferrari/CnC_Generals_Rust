//! Host CreateObjectDie residual (spawn OCL template(s) on death).
//!
//! C++: `CreateObjectDie::onDie` → `ObjectCreationList::create(ocl, dying, killer)`.
//! Residual: spawn one or more named templates at the dying object's pose/team.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostCreateObjectDieData {
    /// OCL residual name (for honesty / presentation).
    pub ocl_name: String,
    /// Templates to spawn (OCL peel residual).
    pub spawn_templates: Vec<String>,
    /// C++ TransferPreviousHealth residual.
    pub transfer_previous_health: bool,
    pub fired: bool,
}

impl HostCreateObjectDieData {
    pub fn single(ocl: &str, template: &str) -> Self {
        Self {
            ocl_name: ocl.into(),
            spawn_templates: vec![template.into()],
            transfer_previous_health: false,
            fired: false,
        }
    }

    /// Fire once. Returns spawn template list.
    pub fn on_die(&mut self) -> Option<Vec<String>> {
        if self.fired || self.spawn_templates.is_empty() {
            return None;
        }
        self.fired = true;
        Some(self.spawn_templates.clone())
    }
}

/// Map common OCL / CreateObjectDie peels to spawn template names.
pub fn create_object_die_config_for_template(name: &str) -> Option<HostCreateObjectDieData> {
    let n = name.to_ascii_lowercase();

    // Sneak attack start → tunnel network (retail CreateObjectDie + TransferPreviousHealth).
    if n.contains("sneakattack") && n.contains("start") {
        return Some(HostCreateObjectDieData {
            ocl_name: "OCL_CreateSneakAttackTunnel".into(),
            spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
            transfer_previous_health: true,
            fired: false,
        });
    }
    if n.contains("sneakattack") && n.contains("tunnel") && n.contains("start") {
        return Some(HostCreateObjectDieData {
            ocl_name: "OCL_CreateSneakAttackTunnel".into(),
            spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
            transfer_previous_health: true,
            fired: false,
        });
    }

    // Aurora bomb → gas cloud residual.
    if n.contains("aurorabomb") || n.contains("aurora_bomb") {
        return Some(HostCreateObjectDieData {
            ocl_name: "AirF_OCL_AuroraBombExplode".into(),
            spawn_templates: vec![
                "AirF_AuroraBombGas".into(),
                "GenericDebris".into(),
            ],
            transfer_previous_health: false,
            fired: false,
        });
    }

    // Superweapon / Daisy FuelAir bomb → gas + shell debris residual
    // (SupW_OCL_FuelAirBomb CreateObject + CreateDebris).
    if n.contains("daisycutterbomb")
        || n.contains("daisy_cutter_bomb")
        || n.contains("fuelairbomb")
        || n.contains("fuel_air_bomb")
        || (n.contains("aurora") && n.contains("fuelair"))
        || n == "moab"
        || n.ends_with("moab")
        || n.contains("moabbomb")
    {
        let gas = if n.contains("supw") || n.contains("superweapon") {
            "SupW_AuroraFuelAirGas"
        } else if n.contains("airf") || n.contains("aurora") {
            "AirF_AuroraBombGas"
        } else {
            // Default Daisy / MOAB residual gas.
            "SupW_AuroraFuelAirGas"
        };
        return Some(HostCreateObjectDieData {
            ocl_name: "SupW_OCL_FuelAirBomb".into(),
            spawn_templates: vec![gas.into(), "GenericDebris".into()],
            transfer_previous_health: false,
            fired: false,
        });
    }

    // Demo truck / high explosive → crater debris residual peel.
    if n.contains("demotrap") {
        return Some(HostCreateObjectDieData::single(
            "OCL_GenericTankDebris",
            "GenericDebris",
        ));
    }

    // Poison field generators often leave nothing; skip.

    // Pilot eject residual is separate (host_usa_pilot).
    None
}

/// Resolve OCL name string to spawn templates when config carries only OCL.
pub fn peel_ocl_spawn_templates(ocl_name: &str) -> Vec<String> {
    let n = ocl_name.to_ascii_lowercase();
    if n.contains("sneakattack") && n.contains("tunnel") && !n.contains("start") {
        return vec!["GLASneakAttackTunnelNetwork".into()];
    }
    if n.contains("sneakattack") && n.contains("start") {
        return vec!["GLASneakAttackTunnelNetworkStart".into()];
    }
    if n.contains("poisonfieldmedium") {
        return vec!["PoisonFieldMedium".into()];
    }
    if n.contains("poisonfieldsmall") {
        return vec!["PoisonFieldSmall".into()];
    }
    if n.contains("poisonfieldlarge") {
        return vec!["PoisonFieldLarge".into()];
    }
    if n.contains("firestorm") {
        return vec!["FirestormSmall".into()];
    }
    if n.contains("aurorabomb") {
        return vec!["AirF_AuroraBombGas".into(), "GenericDebris".into()];
    }
    if n.contains("fuelairbomb") || n.contains("fuel_air") {
        return vec!["SupW_AuroraFuelAirGas".into(), "GenericDebris".into()];
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_object_die_fires_once() {
        let mut d = HostCreateObjectDieData::single("OCL_X", "ThingA");
        assert_eq!(d.on_die().unwrap(), vec!["ThingA".to_string()]);
        assert!(d.on_die().is_none());
    }

    #[test]
    fn fuel_air_bomb_peel() {
        let d = create_object_die_config_for_template("DaisyCutterBomb").unwrap();
        assert_eq!(d.ocl_name, "SupW_OCL_FuelAirBomb");
        assert!(d.spawn_templates.iter().any(|s| s.contains("FuelAirGas") || s.contains("Gas")));
        assert!(d.spawn_templates.iter().any(|s| s.contains("Debris")));
        let m = create_object_die_config_for_template("MOAB").unwrap();
        assert_eq!(m.spawn_templates.len(), 2);
        assert!(m.spawn_templates[0].contains("Gas"));
    }

    #[test]
    fn sneak_start_peel() {
        let d = create_object_die_config_for_template("GLASneakAttackTunnelNetworkStart").unwrap();
        assert!(d.transfer_previous_health);
        assert!(d.spawn_templates[0].contains("TunnelNetwork"));
    }
}
