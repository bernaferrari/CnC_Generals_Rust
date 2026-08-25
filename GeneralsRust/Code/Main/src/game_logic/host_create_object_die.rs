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

/// C++ CreateObjectDie.cpp:42 `CreationList` → ObjectCreationList::create.
/// Prefer an authored OCL name over template-name whitelist peels.
pub fn create_object_die_config_from_creation_list(
    ocl_name: &str,
    transfer_previous_health: bool,
) -> Option<HostCreateObjectDieData> {
    let ocl_name = ocl_name.trim();
    if ocl_name.is_empty() {
        return None;
    }
    let mut spawn_templates = peel_ocl_spawn_templates(ocl_name);
    if spawn_templates.is_empty() {
        spawn_templates = ocl_store_spawn_templates(ocl_name);
    }
    if spawn_templates.is_empty() {
        return None;
    }
    Some(HostCreateObjectDieData {
        ocl_name: ocl_name.to_string(),
        spawn_templates,
        transfer_previous_health,
        fired: false,
    })
}

/// C++ CreateObjectDieModuleData::buildFieldParse CreationList + TransferPreviousHealth.
pub fn create_object_die_config_from_modules<'a>(
    modules: impl IntoIterator<Item = (&'a str, Option<&'a str>, Option<&'a str>)>,
) -> Option<HostCreateObjectDieData> {
    for (class_name, creation_list, transfer) in modules {
        if !class_name.eq_ignore_ascii_case("CreateObjectDie") {
            continue;
        }
        let Some(ocl) = creation_list.map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let transfer_previous_health = transfer
            .map(|s| s.eq_ignore_ascii_case("yes") || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if let Some(cfg) =
            create_object_die_config_from_creation_list(ocl, transfer_previous_health)
        {
            return Some(cfg);
        }
    }
    None
}

fn authored_create_object_die_config(name: &str) -> Option<HostCreateObjectDieData> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
    create_object_die_config_from_modules(definition.behavior_modules.iter().map(|module| {
        (
            module.class_name.as_str(),
            module.attribute("CreationList"),
            module.attribute("TransferPreviousHealth"),
        )
    }))
}

fn ocl_store_spawn_templates(ocl_name: &str) -> Vec<String> {
    use gamelogic::object_creation_list::GenericObjectCreationNugget;
    let Some(ocl) =
        gamelogic::helpers::TheObjectCreationListStore::lookup_object_creation_list(ocl_name)
    else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for nugget in ocl.nuggets() {
        if let Some(generic) = nugget
            .as_any()
            .downcast_ref::<GenericObjectCreationNugget>()
        {
            if generic.name_are_objects {
                let copies = generic.debris_to_generate.max(1) as usize;
                for _ in 0..copies {
                    names.extend(generic.names.iter().cloned());
                }
            }
        }
    }
    names
}

/// Map common OCL / CreateObjectDie peels to spawn template names.
/// Authored `CreationList` wins when Object INI is loaded.
pub fn create_object_die_config_for_template(name: &str) -> Option<HostCreateObjectDieData> {
    if let Some(cfg) = authored_create_object_die_config(name) {
        return Some(cfg);
    }
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
            spawn_templates: vec!["AirF_AuroraBombGas".into(), "GenericDebris".into()],
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
        assert!(
            d.spawn_templates
                .iter()
                .any(|s| s.contains("FuelAirGas") || s.contains("Gas"))
        );
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

    #[test]
    fn authored_creation_list_wins_over_template_whitelist() {
        // C++ CreateObjectDie.cpp:42-43 / 78: CreationList is the authored OCL,
        // not a template-name whitelist. DemoTrap peel would be GenericDebris.
        let authored = create_object_die_config_from_creation_list("OCL_PoisonFieldSmall", false)
            .expect("authored OCL must resolve");
        assert_eq!(authored.ocl_name, "OCL_PoisonFieldSmall");
        assert!(
            authored
                .spawn_templates
                .iter()
                .any(|s| s.contains("PoisonFieldSmall")),
            "authored CreationList must spawn OCL ObjectNames, got {:?}",
            authored.spawn_templates
        );
        assert!(!authored.transfer_previous_health);

        let from_modules = create_object_die_config_from_modules([(
            "CreateObjectDie",
            Some("OCL_CreateSneakAttackTunnel"),
            Some("Yes"),
        )])
        .expect("CreateObjectDie module CreationList");
        assert_eq!(from_modules.ocl_name, "OCL_CreateSneakAttackTunnel");
        assert!(from_modules.transfer_previous_health);
        assert!(from_modules.spawn_templates[0].contains("TunnelNetwork"));
    }
}
