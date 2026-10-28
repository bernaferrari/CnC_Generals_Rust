use super::*;
use serde::{Deserialize, Serialize};

/// Unit-specific data and behaviors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitData {
    pub unit_type: UnitType,
    pub squad_size: usize,
    pub formation_offset: Vec2,
    pub resource_gatherer: Option<ResourceGatherer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitType {
    Infantry,
    Vehicle,
    Aircraft,
}

impl UnitData {
    pub fn new_infantry() -> Self {
        Self {
            unit_type: UnitType::Infantry,
            squad_size: 1,
            formation_offset: Vec2::ZERO,
            resource_gatherer: None,
        }
    }

    pub fn new_vehicle() -> Self {
        Self {
            unit_type: UnitType::Vehicle,
            squad_size: 1,
            formation_offset: Vec2::ZERO,
            resource_gatherer: None,
        }
    }

    pub fn new_worker() -> Self {
        Self {
            unit_type: UnitType::Infantry,
            squad_size: 1,
            formation_offset: Vec2::ZERO,
            resource_gatherer: Some(ResourceGatherer::default()),
        }
    }

    pub fn can_gather_resources(&self) -> bool {
        self.resource_gatherer.is_some()
    }
}

/// Unit factory functions
pub fn create_unit_templates() -> HashMap<String, ThingTemplate> {
    let mut templates = HashMap::new();

    // GLA Units
    let mut gla_soldier = ThingTemplate::new("GLA_Soldier");
    gla_soldier
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(50.0)
        .set_cost(60, 0)
        // GLAInfantryRebel → UIRGrd_SKN (retail W3DZH.big basename).
        .set_model("uirgrd_skn")
        .set_primary_weapon_name(super::weapon_bootstrap::GLA_REBEL_PRIMARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
    templates.insert("GLA_Soldier".to_string(), gla_soldier);

    let mut gla_worker = ThingTemplate::new("GLA_Worker");
    gla_worker
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Selectable)
        .set_health(40.0)
        .set_cost(200, 0)
        // GLAInfantryWorker → UIWRKR_SKN (retail W3DZH.big basename).
        .set_model("uiwrkr_skn");
    templates.insert("GLA_Worker".to_string(), gla_worker);

    let mut gla_technical = ThingTemplate::new("GLA_Technical");
    gla_technical
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_cost(400, 0)
        // GLAVehicleTechnical → UVTechTrck (retail W3DZH.big basename).
        .set_model("uvtechtrck")
        .set_primary_weapon_name(super::weapon_bootstrap::TECHNICAL_MACHINE_GUN)
        .set_locomotor_name(super::locomotor_bootstrap::TECHNICAL_LOCOMOTOR);
    templates.insert("GLA_Technical".to_string(), gla_technical);

    let mut gla_scorpion = ThingTemplate::new("GLA_Scorpion");
    gla_scorpion
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_cost(800, 0)
        // GLATankScorpion → UVLiteTank (retail W3DZH.big basename).
        .set_model("uvlitetank")
        .set_locomotor_name(super::locomotor_bootstrap::SCORPION_LOCOMOTOR)
        .set_primary_weapon_name(super::weapon_bootstrap::SCORPION_TANK_GUN);
    templates.insert("GLA_Scorpion".to_string(), gla_scorpion);

    let mut gla_rpg = ThingTemplate::new("GLA_RPGTrooper");
    gla_rpg
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(60.0)
        .set_cost(100, 0)
        // GLAInfantryTunnelDefender → UITunF_SKN, never the guard proxy.
        .set_model("uitunf_skn")
        .set_primary_weapon_name(super::weapon_bootstrap::TUNNEL_DEFENDER_ROCKET_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
    templates.insert("GLA_RPGTrooper".to_string(), gla_rpg);

    let mut gla_marauder = ThingTemplate::new("GLA_MarauderTank");
    gla_marauder
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(450.0)
        .set_cost(1400, 0)
        // GLATankMarauder → UVMarauder, never the Scorpion proxy.
        .set_model("uvmarauder")
        .set_primary_weapon_name(super::weapon_bootstrap::MARAUDER_TANK_GUN)
        .set_locomotor_name(super::locomotor_bootstrap::SCORPION_LOCOMOTOR);
    templates.insert("GLA_MarauderTank".to_string(), gla_marauder);

    // USA Units
    let mut usa_ranger = ThingTemplate::new("USA_Ranger");
    usa_ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(60.0)
        .set_cost(80, 0)
        // AmericaInfantryRanger → AIRngr_SKN (retail W3DZH.big basename).
        .set_model("airngr_skn")
        .set_primary_weapon_name(super::weapon_bootstrap::RANGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::RANGER_SECONDARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
    templates.insert("USA_Ranger".to_string(), usa_ranger);

    let mut usa_dozer = ThingTemplate::new("USA_Dozer");
    usa_dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(1000, 0)
        // AmericaVehicleDozer → AVCONSTDOZ_A (retail W3DZH.big basename).
        .set_model("avconstdoz_a");
    templates.insert("USA_Dozer".to_string(), usa_dozer);

    let mut usa_humvee = ThingTemplate::new("USA_Humvee");
    usa_humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0)
        .set_cost(600, 0)
        .set_model("avhummer") // USA Humvee
        .set_primary_weapon_name(super::weapon_bootstrap::HUMVEE_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::HUMVEE_SECONDARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::HUMVEE_LOCOMOTOR);
    templates.insert("USA_Humvee".to_string(), usa_humvee);

    let mut usa_crusader = ThingTemplate::new("USA_Crusader");
    usa_crusader
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(600.0)
        .set_cost(1200, 0)
        // AmericaTankCrusader → AVLeopard (retail W3DZH.big basename).
        .set_model("avleopard")
        .set_primary_weapon_name(super::weapon_bootstrap::CRUSADER_TANK_GUN)
        .set_locomotor_name(super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
    templates.insert("USA_Crusader".to_string(), usa_crusader);

    let mut usa_paladin = ThingTemplate::new("USA_Paladin");
    usa_paladin
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(700.0)
        .set_cost(1800, 0)
        // AmericaTankPaladin → AVPaladin, never the Crusader proxy.
        .set_model("avpaladin")
        .set_primary_weapon_name(super::weapon_bootstrap::PALADIN_TANK_GUN)
        .set_locomotor_name(super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
    templates.insert("USA_Paladin".to_string(), usa_paladin);

    let mut usa_tomahawk = ThingTemplate::new("USA_Tomahawk");
    usa_tomahawk
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_cost(1200, 0)
        .set_model("avtomahawk")
        .set_primary_weapon_name(super::weapon_bootstrap::TOMAHAWK_MISSILE_WEAPON);
    templates.insert("USA_Tomahawk".to_string(), usa_tomahawk);

    // China Units
    let mut china_soldier = ThingTemplate::new("China_Soldier");
    china_soldier
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(55.0)
        .set_cost(75, 0)
        // ChinaInfantryRedguard → NICNSC_SKN (retail W3DZH.big basename).
        .set_model("nicnsc_skn")
        .set_primary_weapon_name(super::weapon_bootstrap::REDGUARD_PRIMARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::REDGUARD_LOCOMOTOR);
    templates.insert("China_Soldier".to_string(), china_soldier);

    let mut china_dozer = ThingTemplate::new("China_Dozer");
    china_dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Selectable)
        .set_health(350.0)
        .set_cost(1000, 0)
        // ChinaVehicleDozer → NVCONSTDOZ_A (retail W3DZH.big basename).
        .set_model("nvconstdoz_a");
    templates.insert("China_Dozer".to_string(), china_dozer);

    let mut china_battletank = ThingTemplate::new("China_BattleTank");
    china_battletank
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_cost(1000, 0)
        // ChinaTankBattleMaster → NVBtMstr (retail W3DZH.big basename).
        .set_model("nvbtmstr")
        .set_primary_weapon_name(super::weapon_bootstrap::BATTLE_MASTER_TANK_GUN)
        .set_locomotor_name(super::locomotor_bootstrap::BATTLE_MASTER_LOCOMOTOR);
    templates.insert("China_BattleTank".to_string(), china_battletank);

    let mut china_dragon = ThingTemplate::new("China_DragonTank");
    china_dragon
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(280.0)
        .set_cost(800, 0)
        // ChinaTankDragon → NVDragon (retail W3DZH.big basename).
        .set_model("nvdragon")
        .set_primary_weapon_name(super::weapon_bootstrap::DRAGON_TANK_FLAME_WEAPON);
    templates.insert("China_DragonTank".to_string(), china_dragon);

    let mut china_gattling = ThingTemplate::new("China_GattlingTank");
    china_gattling
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_cost(800, 0)
        // ChinaTankGattling → NVGattTank (retail W3DZH.big basename).
        .set_model("nvgatttank")
        .set_primary_weapon_name(super::weapon_bootstrap::GATTLING_TANK_GUN)
        .set_secondary_weapon_name(super::weapon_bootstrap::GATTLING_TANK_GUN_AIR);
    templates.insert("China_GattlingTank".to_string(), china_gattling);

    let mut china_tank_hunter = ThingTemplate::new("China_TankHunter");
    china_tank_hunter
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_cost(110, 0)
        // ChinaInfantryTankHunter → NIMSST_SKN, never the guard proxy.
        .set_model("nimsst_skn")
        .set_primary_weapon_name(super::weapon_bootstrap::TANK_HUNTER_PRIMARY_WEAPON)
        .set_locomotor_name(super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
    templates.insert("China_TankHunter".to_string(), china_tank_hunter);

    templates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_factory_uses_exact_retail_models_instead_of_legacy_proxies() {
        let templates = create_unit_templates();
        let expected_models = [
            ("GLA_Soldier", "uirgrd_skn"),
            ("GLA_Worker", "uiwrkr_skn"),
            ("GLA_Technical", "uvtechtrck"),
            ("GLA_Scorpion", "uvlitetank"),
            ("GLA_RPGTrooper", "uitunf_skn"),
            ("GLA_MarauderTank", "uvmarauder"),
            ("USA_Ranger", "airngr_skn"),
            ("USA_Dozer", "avconstdoz_a"),
            ("USA_Crusader", "avleopard"),
            ("USA_Paladin", "avpaladin"),
            ("China_Soldier", "nicnsc_skn"),
            ("China_Dozer", "nvconstdoz_a"),
            ("China_BattleTank", "nvbtmstr"),
            ("China_DragonTank", "nvdragon"),
            ("China_GattlingTank", "nvgatttank"),
            ("China_TankHunter", "nimsst_skn"),
        ];

        for (template_name, expected_model) in expected_models {
            let template = templates
                .get(template_name)
                .unwrap_or_else(|| panic!("missing unit factory template {template_name}"));
            assert_eq!(
                template.model_name.as_deref(),
                Some(expected_model),
                "{template_name} must not borrow another retail unit's W3D"
            );
        }
    }
}
