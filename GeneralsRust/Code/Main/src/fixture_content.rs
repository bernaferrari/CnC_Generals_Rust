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
    world
        .admit_object_ini_sources([("fixture-objects.ini".into(), text)])
        .expect("fixture Object content admitted before NewGame");
}

/// Declare named Objects with no authored modules (content-only witnesses).
pub(crate) fn admit_declared_objects(world: &mut GameLogic, names: &[&str]) {
    let objects: Vec<_> = names.iter().map(|name| FixtureObject::new(*name)).collect();
    admit_fixture_objects(world, &objects);
}
