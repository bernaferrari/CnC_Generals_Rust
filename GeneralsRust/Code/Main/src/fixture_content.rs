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

    /// W3DModelDraw DefaultConditionState Model (W3DModelDraw.cpp
    /// parseConditionState): the Object's authored presentation model.
    pub(crate) fn model(self, model: &str) -> Self {
        self.module(format!(
            "Draw = W3DModelDraw FixtureDraw\n DefaultConditionState\n  Model = {model}\n End\nEnd"
        ))
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
            text.push_str(&format!(
                " Locomotor = SET_NORMAL {FIXTURE_WALK_LOCOMOTOR}\n"
            ));
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
    admit_fixture_content_with_actions(world, objects, weapons, "");
}

/// [`admit_fixture_content`] plus authored `CommandButton`, `CommandSet` and
/// `SpecialPower` blocks. C++ loads CommandButton.ini and SpecialPower.ini with
/// the other startup INI (GameEngine.cpp:428-458), so a fixture's buttons and
/// powers exist before any Object template binds them.
pub(crate) fn admit_fixture_content_with_actions(
    world: &mut GameLogic,
    objects: &[FixtureObject],
    weapons: &[FixtureWeapon],
    actions: &str,
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
    if objects.iter().any(|object| {
        object
            .modules
            .iter()
            .any(|m| m.contains(FIXTURE_RADAR_UPGRADE))
    }) {
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
    if !actions.is_empty() {
        let sciences =
            std::sync::Arc::new(game_engine::common::rts::science::ScienceNameFacts::default());
        let mut definitions =
            game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions::default();
        definitions
            .apply_source(actions, &sciences)
            .expect("fixture action definitions parse");
        let content = crate::assets::ReceivingObjectContent::from_sources_with_catalogs(
            sources,
            sciences,
            std::sync::Arc::new(definitions),
        )
        .expect("fixture Object content parses");
        let weapons = (!weapons.is_empty()).then(|| fixture_weapon_rules(weapons));
        world
            .admit_receiving_module_content_with_weapons(content, weapons)
            .expect("fixture Object, Weapon and action content admitted before NewGame");
        return;
    }
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

    /// Author the stat fields a test states as a host `Weapon` as Weapon.ini
    /// lines (seconds become msec, the C++ INI unit); runtime fire state is
    /// not authored content and stays the caller's to set on the instance.
    pub(crate) fn from_stats(name: impl Into<String>, stats: &crate::game_logic::Weapon) -> Self {
        fn yes(flag: bool) -> &'static str {
            if flag { "Yes" } else { "No" }
        }
        fn msec(seconds: f32) -> u32 {
            (seconds.max(0.0) * 1000.0).round() as u32
        }
        let mut weapon = Self::new(name)
            .damage(stats.damage)
            .range(stats.range)
            .delay_ms(msec(stats.reload_time))
            .field(format!("AntiGround = {}", yes(stats.can_target_ground)))
            .field(format!(
                "AntiAirborneVehicle = {}",
                yes(stats.can_target_air)
            ))
            .field(format!(
                "AntiAirborneInfantry = {}",
                yes(stats.can_target_air)
            ));
        // C++ WeaponSpeed 999999 is the instant-hit sentinel.
        weapon = weapon.field(if stats.projectile_speed > 0.0 {
            format!("WeaponSpeed = {}", stats.projectile_speed)
        } else {
            "WeaponSpeed = 999999".to_string()
        });
        if stats.min_range > 0.0 {
            weapon = weapon.min_range(stats.min_range);
        }
        if stats.splash_radius > 0.0 {
            weapon = weapon.radius(stats.splash_radius);
        }
        if stats.pre_attack_delay > 0.0 {
            weapon = weapon.field(format!("PreAttackDelay = {}", msec(stats.pre_attack_delay)));
        }
        if stats.clip_size > 0 {
            weapon = weapon.clip(stats.clip_size, msec(stats.clip_reload_time));
        }
        weapon
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
    assert_eq!(
        parsed,
        weapons.len(),
        "every fixture Weapon parses:\n{text}"
    );
    let mut rules: Vec<_> = rules
        .into_values()
        .map(|rule| (rule.name.clone(), rule))
        .collect();
    rules.sort_by(|a, b| a.0.cmp(&b.0));
    rules
}

/// Author a hand-built template's default WeaponSet (`Weapon = PRIMARY/
/// SECONDARY/TERTIARY` from `weapons` in slot order) bound to the fixture
/// Weapons parsed as Weapon.ini, as C++ ThingTemplate WeaponSets name
/// TheWeaponStore templates.
pub(crate) fn arm_template(
    template: &mut crate::game_logic::ThingTemplate,
    slots: [Option<FixtureWeapon>; 3],
) {
    let names = slots.each_ref().map(|slot| slot.as_ref().map(|weapon| weapon.name.clone()));
    assert!(names.iter().any(Option::is_some), "a WeaponSet authors a slot");
    template.primary_weapon_name = names[0].clone();
    template.secondary_weapon_name = names[1].clone();
    template.tertiary_weapon_name = names[2].clone();
    let weapons: Vec<_> = slots.into_iter().flatten().collect();
    let catalog = fixture_weapon_rules(&weapons)
        .into_iter()
        .map(|(name, rule)| (name.to_ascii_lowercase(), std::sync::Arc::new(rule)))
        .collect();
    template.bind_receiving_weapon_sets(&catalog);
}

/// Give an Object a fixture-authored default WeaponSet (PRIMARY, SECONDARY,
/// TERTIARY) and instantiate it, as C++ WeaponSet::updateWeaponSet does when
/// an Object's set is (re)selected.
pub(crate) fn arm_slots(object: &mut crate::game_logic::Object, slots: [Option<FixtureWeapon>; 3]) {
    arm_template(object.template_mut(), slots);
    assert!(
        object.rebind_receiving_weapon_row_if_changed(None, 0),
        "authored WeaponSet row binds"
    );
}

/// [`arm_slots`] filling PRIMARY.. in order.
pub(crate) fn arm(object: &mut crate::game_logic::Object, weapons: &[FixtureWeapon]) {
    assert!(weapons.len() <= 3, "a WeaponSet has three slots");
    arm_slots(object, std::array::from_fn(|slot| weapons.get(slot).cloned()));
}

/// [`arm`] with one PRIMARY authored from a test's stated stats.
pub(crate) fn arm_with_stats(object: &mut crate::game_logic::Object, stats: crate::game_logic::Weapon) {
    arm_with_slot_stats(object, [Some(stats), None, None]);
}

/// [`arm_slots`] authored from a test's stated stats (plus its stated fire
/// state on the instance). A slot keeps the Weapon
/// name its template already authors (name-keyed rules such as Jarmen Kell's
/// rifle read it), else gets a per-slot fixture name.
pub(crate) fn arm_with_slot_stats(
    object: &mut crate::game_logic::Object,
    slots: [Option<crate::game_logic::Weapon>; 3],
) {
    let template = &object.thing().template;
    let authored = [
        template.primary_weapon_name.clone(),
        template.secondary_weapon_name.clone(),
        template.tertiary_weapon_name.clone(),
    ];
    let mut slot = 0;
    let weapons = slots.each_ref().map(|stats| {
        let name = authored[slot]
            .clone()
            .unwrap_or_else(|| format!("FixtureWeapon{slot}"));
        slot += 1;
        stats.as_ref().map(|stats| FixtureWeapon::from_stats(name, stats))
    });
    arm_slots(object, weapons);
    // Fire state the test states is instance state, not authored content.
    for (slot, stats) in (0u8..).zip(slots) {
        if let (Some(stats), Some(weapon)) = (stats, object.weapon_slot_mut(slot)) {
            weapon.last_fire_time = stats.last_fire_time;
            weapon.last_fire_frame = stats.last_fire_frame;
            if stats.ammo.is_some() {
                weapon.ammo = stats.ammo;
            }
            weapon.reloading_clip |= stats.reloading_clip;
            if stats.last_bonus_rof != 0.0 {
                weapon.last_bonus_rof = stats.last_bonus_rof;
            }
            if stats.next_fire_frame != 0 {
                weapon.next_fire_frame = stats.next_fire_frame;
            }
        }
    }
}

/// Re-author one WeaponSet slot from a test's stated stats, keeping the
/// Object's other current slots, as a test that swaps one weapon intends.
pub(crate) fn arm_slot_with_stats(
    object: &mut crate::game_logic::Object,
    slot: u8,
    stats: crate::game_logic::Weapon,
) {
    let mut slots: [Option<crate::game_logic::Weapon>; 3] =
        std::array::from_fn(|index| object.weapon_slot(index as u8).cloned());
    slots[usize::from(slot)] = Some(stats);
    arm_with_slot_stats(object, slots);
}

/// Give a hand-built Object an authored current Locomotor parsed from
/// Locomotor.ini `fields` (C++ AIUpdateInterface `m_curLocomotor`): movement,
/// turning and lift run only `if (m_curLocomotor)` (AIUpdate.cpp doLocomotor).
/// Fields the test then assigns on the instance override the authored ones.
pub(crate) fn locomote(object: &mut crate::game_logic::Object, fields: &[&str]) {
    let mut text = String::from("Locomotor FixtureHandLocomotor\n");
    for field in fields {
        text.push_str(&format!(" {field}\n"));
    }
    text.push_str("End\n");
    let mut store = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    store.apply_source(&text).expect("fixture Locomotor parses");
    let binding = crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding_in(
        Some(&store),
        "FixtureHandLocomotor",
    )
    .expect("fixture Locomotor resolves");
    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(object, &binding);
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

    #[test]
    fn arm_instantiates_its_authored_weapon_set() {
        use crate::game_logic::{ObjectId, ThingTemplate};
        let mut object =
            crate::game_logic::Object::new(ThingTemplate::new("FixtureHandGunner"), ObjectId(7), Team::USA);
        arm(
            &mut object,
            &[
                FixtureWeapon::new("FixtureHandGun").damage(9.0).range(80.0),
                FixtureWeapon::new("FixtureHandRocket")
                    .damage(30.0)
                    .anti_air(),
            ],
        );
        let primary = object.weapon_slot(0).expect("PRIMARY");
        assert_eq!((primary.damage, primary.range), (9.0, 80.0));
        assert_eq!(object.weapon_slot(1).expect("SECONDARY").damage, 30.0);
        assert!(object.weapon_slot(2).is_none());
    }
}
