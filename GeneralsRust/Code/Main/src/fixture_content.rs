//! Authored fixture content for Main lib tests.
//!
//! C++ GameEngine::init (GameEngine.cpp:428-458) loads Locomotor and Object
//! INI before any NewGame, and every Object is built from its authored
//! modules. Fixtures therefore declare the Objects they create here, as real
//! INI blocks, instead of inserting programmatic ThingTemplates afterwards
//! (an admitted world builds bodies, AI and locomotors from the INI only).

use crate::game_logic::GameLogic;

/// Upgrade granted at creation to fixture radar providers.
pub(crate) const FIXTURE_RADAR_UPGRADE: &str = "Upgrade_FixtureRadar";

/// Two-legged ground locomotor shared by fixture walkers.
pub(crate) const FIXTURE_WALK_LOCOMOTOR: &str = "FixtureWalk";

/// One authored fixture Object definition.
#[derive(Clone, Debug)]
pub(crate) struct FixtureObject {
    name: String,
    kind_of: String,
    health: Option<f32>,
    fields: Vec<String>,
    walker: bool,
    modules: Vec<String>,
    weapon_slots: Vec<(String, String)>,
}

impl FixtureObject {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind_of: String::new(),
            health: None,
            fields: Vec::new(),
            walker: false,
            modules: Vec::new(),
            weapon_slots: Vec::new(),
        }
    }

    /// INI KindOf tokens, e.g. `"STRUCTURE POWERED"`.
    pub(crate) fn kind_of(mut self, kinds: &str) -> Self {
        if !self.kind_of.is_empty() {
            self.kind_of.push(' ');
        }
        self.kind_of.push_str(kinds);
        self
    }

    /// ActiveBody MaxHealth and InitialHealth.
    pub(crate) fn health(mut self, health: f32) -> Self {
        self.health = Some(health);
        self
    }

    pub(crate) fn energy(self, production: i32) -> Self {
        self.field(format!("EnergyProduction = {production}"))
    }

    pub(crate) fn build_cost(self, cost: u32) -> Self {
        self.field(format!("BuildCost = {cost}"))
    }

    pub(crate) fn build_time(self, seconds: f32) -> Self {
        self.field(format!("BuildTime = {seconds}"))
    }

    /// Any other top-level Object field line.
    pub(crate) fn field(mut self, line: impl Into<String>) -> Self {
        self.fields.push(line.into());
        self
    }

    /// AIUpdateInterface plus the shared fixture walking locomotor.
    pub(crate) fn walker(mut self) -> Self {
        self.walker = true;
        self
    }

    /// A raw module block, e.g. `"Behavior = InstantDeathBehavior Death\nEnd"`.
    pub(crate) fn module(mut self, block: impl Into<String>) -> Self {
        self.modules.push(block.into());
        self
    }

    /// Radar comes only from an authored RadarUpgrade (RadarUpgrade.cpp:72-96),
    /// granted at creation like a completed command center.
    pub(crate) fn radar_provider(self) -> Self {
        self.module(format!(
            "Behavior = RadarUpgrade FixtureRadar\n TriggeredBy = {FIXTURE_RADAR_UPGRADE}\nEnd\nBehavior = GrantUpgradeCreate FixtureRadarGrant\n UpgradeToGrant = {FIXTURE_RADAR_UPGRADE}\nEnd"
        ))
    }

    /// DozerAIUpdate: the only builder that advances construction.
    pub(crate) fn dozer(self) -> Self {
        self.module("Behavior = DozerAIUpdate FixtureDozerAI\nEnd")
    }

    /// One slot of the default (`Conditions = None`) WeaponSet, e.g.
    /// `.weapon("SECONDARY", "FixtureRocket")`. C++ Objects fire only weapons
    /// their authored WeaponSet names (WeaponSet.cpp); kind never implies a gun.
    pub(crate) fn weapon(mut self, slot: &str, weapon: &str) -> Self {
        self.weapon_slots.push((slot.into(), weapon.into()));
        self
    }

    /// The default WeaponSet's PRIMARY weapon.
    pub(crate) fn armed(self, weapon: &str) -> Self {
        self.weapon("PRIMARY", weapon)
    }

    pub(crate) fn instant_death(self) -> Self {
        self.module("Behavior = InstantDeathBehavior FixtureDeath\nEnd")
    }

    pub(crate) fn ini(&self) -> String {
        let mut text = format!("Object {}\n", self.name);
        if !self.kind_of.is_empty() {
            text.push_str(&format!(" KindOf = {}\n", self.kind_of));
        }
        for field in &self.fields {
            text.push_str(&format!(" {field}\n"));
        }
        if self.walker {
            text.push_str(&format!(" Locomotor = SET_NORMAL {FIXTURE_WALK_LOCOMOTOR}\n"));
        }
        if let Some(health) = self.health {
            text.push_str(&format!(
                " Body = ActiveBody FixtureBody\n  MaxHealth = {health}\n  InitialHealth = {health}\n End\n"
            ));
        }
        if !self.weapon_slots.is_empty() {
            text.push_str(" WeaponSet\n  Conditions = None\n");
            for (slot, weapon) in &self.weapon_slots {
                text.push_str(&format!("  Weapon = {slot} {weapon}\n"));
            }
            text.push_str(" End\n");
        }
        if self.walker {
            text.push_str(" Behavior = AIUpdateInterface FixtureBrain\n End\n");
        }
        for module in &self.modules {
            text.push_str(module);
            text.push('\n');
        }
        text.push_str("End\n");
        text
    }
}

/// Admit fixture Objects (and the walker locomotor when any walks) as the
/// world's ordered startup content, before NewGame.
pub(crate) fn admit_fixture_objects(world: &mut GameLogic, objects: &[FixtureObject]) {
    admit_fixture_content(world, objects, &[]);
}

/// Admit fixture Objects together with the authored Weapons their WeaponSets
/// name, as one ordered startup admission (C++ loads Weapon.ini with the
/// other INI before Object templates are built, GameEngine.cpp:428-458).
pub(crate) fn admit_fixture_content(
    world: &mut GameLogic,
    objects: &[FixtureObject],
    weapons: &[FixtureWeapon],
) {
    if objects.iter().any(|object| object.walker) {
        let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
        locomotors
            .apply_source(&format!(
                "Locomotor {FIXTURE_WALK_LOCOMOTOR}\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 360\n Appearance = TWO_LEGS\nEnd\n"
            ))
            .expect("fixture locomotor");
        world
            .admit_locomotor_definitions(locomotors)
            .expect("fixture locomotor admitted before Object content");
    }
    if objects.iter().any(|object| object.modules.iter().any(|m| m.contains(FIXTURE_RADAR_UPGRADE))) {
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            world,
            FIXTURE_RADAR_UPGRADE,
            "OBJECT",
            0,
            1,
        );
    }
    let text: String = objects.iter().map(FixtureObject::ini).collect();
    let sources = [("fixture-objects.ini".to_string(), text)];
    if weapons.is_empty() {
        world
            .admit_object_ini_sources(sources)
            .expect("fixture Object content admitted before NewGame");
        return;
    }
    let content = crate::assets::ReceivingObjectContent::from_sources(sources)
        .expect("fixture Object content parses");
    world
        .admit_receiving_module_content_with_weapons(content, Some(fixture_weapon_rules(weapons)))
        .expect("fixture Object and Weapon content admitted before NewGame");
}

/// One authored fixture Weapon definition (Weapon.ini block). Unset fields
/// keep the C++ WeaponTemplate defaults (Weapon.cpp WeaponTemplate ctor).
#[derive(Clone, Debug)]
pub(crate) struct FixtureWeapon {
    name: String,
    fields: Vec<String>,
}

impl FixtureWeapon {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
        }
    }

    /// A raw Weapon field line, e.g. `"AntiAirborneVehicle = Yes"`.
    pub(crate) fn field(mut self, line: impl Into<String>) -> Self {
        self.fields.push(line.into());
        self
    }

    pub(crate) fn damage(self, damage: f32) -> Self {
        self.field(format!("PrimaryDamage = {damage}"))
    }

    pub(crate) fn radius(self, radius: f32) -> Self {
        self.field(format!("PrimaryDamageRadius = {radius}"))
    }

    pub(crate) fn range(self, range: f32) -> Self {
        self.field(format!("AttackRange = {range}"))
    }

    pub(crate) fn min_range(self, range: f32) -> Self {
        self.field(format!("MinimumAttackRange = {range}"))
    }

    pub(crate) fn delay_ms(self, delay: u32) -> Self {
        self.field(format!("DelayBetweenShots = {delay}"))
    }

    pub(crate) fn damage_type(self, damage_type: &str) -> Self {
        self.field(format!("DamageType = {damage_type}"))
    }

    pub(crate) fn clip(self, size: u32, reload_ms: u32) -> Self {
        self.field(format!("ClipSize = {size}"))
            .field(format!("ClipReloadTime = {reload_ms}"))
    }

    pub(crate) fn anti_air(self) -> Self {
        self.field("AntiAirborneVehicle = Yes")
            .field("AntiAirborneInfantry = Yes")
    }

    pub(crate) fn ini(&self) -> String {
        let mut text = format!("Weapon {}\n", self.name);
        for field in &self.fields {
            text.push_str(&format!(" {field}\n"));
        }
        text.push_str("End\n");
        text
    }
}

/// Parse fixture Weapons through the ordinary Weapon.ini loader.
pub(crate) fn fixture_weapon_rules(
    weapons: &[FixtureWeapon],
) -> Vec<(String, gamelogic::weapon::WeaponTemplate)> {
    let text: String = weapons.iter().map(FixtureWeapon::ini).collect();
    let mut rules = std::collections::HashMap::new();
    let parsed = crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut rules,
        &text,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    assert_eq!(parsed, weapons.len(), "every fixture Weapon parses:\n{text}");
    let mut rules: Vec<_> = rules
        .into_values()
        .map(|rule| (rule.name.clone(), rule))
        .collect();
    rules.sort_by(|a, b| a.0.cmp(&b.0));
    rules
}

/// Declare named Objects with no authored modules (content-only witnesses).
pub(crate) fn admit_declared_objects(world: &mut GameLogic, names: &[&str]) {
    let objects: Vec<_> = names.iter().map(|name| FixtureObject::new(*name)).collect();
    admit_fixture_objects(world, &objects);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team};

    #[test]
    fn armed_fixture_object_fires_only_its_authored_weapon_set() {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Gunner", true));
        admit_fixture_content(
            &mut world,
            &[
                FixtureObject::new("FixtureGunner")
                    .kind_of("INFANTRY CAN_ATTACK")
                    .health(100.0)
                    .walker()
                    .armed("FixtureGun"),
                FixtureObject::new("FixtureUnarmed")
                    .kind_of("INFANTRY")
                    .health(100.0),
            ],
            &[FixtureWeapon::new("FixtureGun").damage(12.0).range(150.0)],
        );
        let gunner = world
            .create_object_for_player("FixtureGunner", 0, glam::Vec3::ZERO)
            .expect("armed fixture object");
        let weapon = world
            .host_object(gunner)
            .unwrap()
            .weapon
            .as_ref()
            .expect("authored WeaponSet instantiates its PRIMARY");
        assert_eq!(weapon.damage, 12.0);
        assert_eq!(weapon.range, 150.0);
        let unarmed = world
            .create_object_for_player("FixtureUnarmed", 0, glam::Vec3::ZERO)
            .expect("unarmed fixture object");
        assert!(world.host_object(unarmed).unwrap().weapon.is_none());
    }
}
