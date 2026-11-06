//! Authored upgrade modules applied through the UpgradeMux trigger rules.
//!
//! Mirrors C++ `UpgradeMux::attemptUpgrade` / `testUpgradeConditions`
//! (GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UpgradeModule.cpp)
//! and `MaxHealthUpgrade::upgradeImplementation` (MaxHealthUpgrade.cpp): an
//! Object raises its body's maximum health only because its template authors
//! a `MaxHealthUpgrade` whose `TriggeredBy` names the completed upgrade. No
//! unit or template name selects a recipient.
use super::super::*;
use super::weapon_upgrades::UpgradeRecipients;

/// C++ `MaxHealthChangeType` (BodyModule.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MaxHealthChangeType {
    SameCurrentHealth,
    PreserveRatio,
    AddCurrentHealthToo,
    FullyHeal,
}

impl MaxHealthChangeType {
    /// INI token; C++ defaults to SAME_CURRENTHEALTH.
    fn parse(token: Option<&str>) -> Self {
        match token.map(str::trim) {
            Some(t) if t.eq_ignore_ascii_case("PRESERVE_RATIO") => Self::PreserveRatio,
            Some(t) if t.eq_ignore_ascii_case("ADD_CURRENT_HEALTH_TOO") => {
                Self::AddCurrentHealthToo
            }
            Some(t) if t.eq_ignore_ascii_case("FULLY_HEAL") => Self::FullyHeal,
            _ => Self::SameCurrentHealth,
        }
    }
}

impl From<MaxHealthChangeType> for crate::game_logic::object::MaxHealthChange {
    fn from(kind: MaxHealthChangeType) -> Self {
        match kind {
            MaxHealthChangeType::SameCurrentHealth => Self::KeepCurrent,
            MaxHealthChangeType::PreserveRatio => Self::PreserveRatio,
            MaxHealthChangeType::AddCurrentHealthToo => Self::AddToCurrent,
            MaxHealthChangeType::FullyHeal => Self::FullyHeal,
        }
    }
}

/// One authored `MaxHealthUpgrade` module.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AuthoredMaxHealthUpgrade {
    /// Module tag; identifies the module's executed flag on the Object.
    pub tag: String,
    pub triggered_by: Vec<String>,
    pub add_max_health: f32,
    pub change_type: MaxHealthChangeType,
}

impl AuthoredMaxHealthUpgrade {
    /// Every `MaxHealthUpgrade` the template authors, in declaration order.
    pub(crate) fn authored_by(template: &ThingTemplate) -> Vec<Self> {
        let Some(definition) = template.receiving_object_definition.as_deref() else {
            return Vec::new();
        };
        definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("MaxHealthUpgrade"))
            .enumerate()
            .map(|(index, module)| Self {
                tag: module
                    .module_tag
                    .clone()
                    .unwrap_or_else(|| format!("MaxHealthUpgrade{index}")),
                triggered_by: module
                    .attribute("TriggeredBy")
                    .map(|v| v.split_whitespace().map(str::to_owned).collect())
                    .unwrap_or_default(),
                add_max_health: module
                    .attribute("AddMaxHealth")
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .unwrap_or(0.0),
                change_type: MaxHealthChangeType::parse(module.attribute("ChangeType")),
            })
            .collect()
    }

    /// Per-Object executed flag (C++ `UpgradeMux::m_upgradeExecuted`).
    fn executed_key(&self) -> String {
        format!("MaxHealthUpgrade#{}", self.tag)
    }
}

/// Parse the C++ `UpgradeMuxData` trigger fields of one authored module.
pub(crate) fn upgrade_mux_data(
    module: &crate::assets::ini_parser::BehaviorModuleDefinition,
) -> crate::game_logic::UpgradeMuxTriggers {
    let tokens = |key: &str| -> Vec<String> {
        module
            .attribute(key)
            .map(|v| v.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    crate::game_logic::UpgradeMuxTriggers {
        triggered_by: tokens("TriggeredBy"),
        conflicts_with: tokens("ConflictsWith"),
        requires_all: module
            .attribute("RequiresAllTriggers")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("yes") || v.trim() == "1"),
    }
}

fn upgrade_mux_triggers(
    module: &crate::assets::ini_parser::BehaviorModuleDefinition,
    has: &impl Fn(&str) -> bool,
) -> bool {
    upgrade_mux_data(module).satisfied_by(has)
}

/// The flag-setting upgrade modules an object's template authors that are
/// triggered by its current upgrade mask (C++ `upgradeImplementation` of
/// WeaponSetUpgrade, ArmorUpgrade, WeaponBonusUpgrade, CommandSetUpgrade).
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct AuthoredFlagUpgrades {
    /// WeaponSetUpgrade -> `setWeaponSetFlag(WEAPONSET_PLAYER_UPGRADE)`.
    pub weapon_set: bool,
    /// ArmorUpgrade -> `setArmorSetFlag(ARMORSET_PLAYER_UPGRADE)`.
    pub armor_set: bool,
    /// WeaponBonusUpgrade -> `setWeaponBonusCondition(WEAPONBONUSCONDITION_PLAYER_UPGRADE)`.
    pub weapon_bonus: bool,
    /// CommandSetUpgrade -> `setCommandSetStringOverride(CommandSet)`.
    pub command_set: Option<String>,
}

impl AuthoredFlagUpgrades {
    /// Evaluate `template`'s authored modules against `has(upgrade)`.
    pub(crate) fn triggered(template: &ThingTemplate, has: impl Fn(&str) -> bool) -> Self {
        let mut out = Self::default();
        let Some(definition) = template.receiving_object_definition.as_deref() else {
            return out;
        };
        for module in &definition.behavior_modules {
            let class = module.class_name.as_str();
            let is = |name: &str| class.eq_ignore_ascii_case(name);
            if !(is("WeaponSetUpgrade")
                || is("ArmorUpgrade")
                || is("WeaponBonusUpgrade")
                || is("CommandSetUpgrade"))
                || !upgrade_mux_triggers(module, &has)
            {
                continue;
            }
            if is("WeaponSetUpgrade") {
                out.weapon_set = true;
            } else if is("ArmorUpgrade") {
                out.armor_set = true;
            } else if is("WeaponBonusUpgrade") {
                out.weapon_bonus = true;
            } else {
                // CommandSetUpgrade.cpp: TriggerAlt selects CommandSetAlt.
                let alt = module
                    .attribute("TriggerAlt")
                    .map(str::trim)
                    .filter(|name| !name.is_empty() && has(name))
                    .and_then(|_| module.attribute("CommandSetAlt"));
                if let Some(set) = alt.or_else(|| module.attribute("CommandSet")) {
                    out.command_set = Some(set.trim().to_owned());
                }
            }
        }
        out
    }
}

/// `UnpauseSpecialPowerUpgrade` modules a template authors that are triggered
/// by `has` (UnpauseSpecialPowerUpgrade.cpp: `upgradeImplementation` calls
/// `pauseCountdown(FALSE)` on the module's `SpecialPowerTemplate`). Capture,
/// Radar Van Scan, Booby Trap and Helix bombs all arrive through this module;
/// no unit or upgrade name selects the power.
pub(crate) fn authored_unpause_powers(
    template: &ThingTemplate,
    has: impl Fn(&str) -> bool,
) -> Vec<crate::command_system::SpecialPowerType> {
    let Some(definition) = template.receiving_object_definition.as_deref() else {
        return Vec::new();
    };
    definition
        .behavior_modules
        .iter()
        .filter(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("UnpauseSpecialPowerUpgrade")
                && upgrade_mux_triggers(module, &has)
        })
        .filter_map(|module| {
            crate::command_system::special_power_type_from_template_name(
                module.attribute("SpecialPowerTemplate")?.trim(),
            )
        })
        .collect()
}

/// Whether the template authors a `StealthUpgrade` triggered by `has`
/// (StealthUpgrade.cpp: `upgradeImplementation` sets OBJECT_STATUS_CAN_STEALTH
/// for the module's owner). Camouflage and Camo Netting both arrive through
/// this module; no unit or structure name selects the recipient.
pub(crate) fn authors_triggered_stealth_upgrade(
    template: &ThingTemplate,
    has: impl Fn(&str) -> bool,
) -> bool {
    template.stealth_upgrades.iter().any(|rules| rules.satisfied_by(&has))
}

impl Object {
    /// Apply the authored flag upgrades triggered by this object's mask plus
    /// `extra` (C++ Object::updateUpgradeModules). Flags only ever turn on
    /// here; removal follows UpgradeMux::removeUpgrade paths.
    pub(crate) fn apply_authored_flag_upgrades(
        &mut self,
        extra: impl Fn(&str) -> bool,
    ) -> AuthoredFlagUpgrades {
        let applied = AuthoredFlagUpgrades::triggered(&self.thing().template, |name| {
            extra(name) || self.has_upgrade_tag(name)
        });
        if applied.weapon_set {
            let _ = self.set_weapon_set_flag(0, true);
        }
        if applied.armor_set {
            self.set_armor_set_player_upgrade(true);
        }
        if applied.weapon_bonus {
            self.set_weapon_bonus_player_upgrade(true);
        }
        if let Some(set) = applied.command_set.clone() {
            self.set_command_set_override(Some(set));
        }
        applied
    }
    /// C++ `Object::updateUpgradeModules` for `LocomotorSetUpgrade`
    /// (LocomotorSetUpgrade.cpp): run the template's authored module when its
    /// UpgradeMux triggers are satisfied by `has` (the owning player's
    /// completed upgrades for a new object). Returns whether a module
    /// executed. No unit or upgrade name selects the recipient.
    pub(crate) fn apply_authored_locomotor_set_upgrade_for_mask(
        &mut self,
        has: impl Fn(&str) -> bool,
        locomotors: Option<&game_engine::common::ini::ini_locomotor::LocomotorStore>,
    ) -> bool {
        let triggered = self
            .thing()
            .template
            .receiving_object_definition
            .as_deref()
            .is_some_and(|definition| {
                definition.behavior_modules.iter().any(|module| {
                    module.class_name.eq_ignore_ascii_case("LocomotorSetUpgrade")
                        && upgrade_mux_triggers(module, &has)
                })
            });
        if triggered {
            self.set_locomotor_upgrade_in(true, locomotors);
        }
        triggered
    }

    /// Run this Object's authored `MaxHealthUpgrade` modules whose trigger is
    /// satisfied (`is_completed(upgrade)`), each at most once. Returns whether
    /// any module executed. Used both at upgrade completion and by
    /// `Object::initObject -> updateUpgradeModules` for new objects.
    pub(crate) fn run_authored_max_health_upgrades(
        &mut self,
        is_completed: impl Fn(&str) -> bool,
    ) -> bool {
        let mut applied = false;
        for module in AuthoredMaxHealthUpgrade::authored_by(&self.thing().template) {
            if !module.triggered_by.iter().any(|name| is_completed(name))
                || !self.applied_upgrades.insert(module.executed_key())
            {
                continue;
            }
            // ActiveBody::getMaxHealth (the live body maximum).
            let current_max = if self.health.maximum > 0.0 {
                self.health.maximum
            } else {
                self.max_health
            };
            self.set_body_max_health_with_mode(
                current_max + module.add_max_health,
                module.change_type.into(),
            );
            applied = true;
        }
        applied
    }
}

/// Whether the template authors a `SpecialAbilityUpdate` for the named
/// special power and an `UnpauseSpecialPowerUpgrade` for it that `has`
/// triggers (the StartsPaused ability is usable only after the upgrade).
pub(crate) fn authors_unpaused_ability(
    template: &ThingTemplate,
    power_template: &str,
    has: impl Fn(&str) -> bool,
) -> bool {
    let Some(definition) = template.receiving_object_definition.as_deref() else {
        return false;
    };
    let names_power = |module: &crate::assets::ini_parser::BehaviorModuleDefinition| {
        module
            .attribute("SpecialPowerTemplate")
            .is_some_and(|name| name.trim().eq_ignore_ascii_case(power_template))
    };
    definition.behavior_modules.iter().any(|module| {
        module.class_name.eq_ignore_ascii_case("SpecialAbilityUpdate") && names_power(module)
    }) && definition.behavior_modules.iter().any(|module| {
        module
            .class_name
            .eq_ignore_ascii_case("UnpauseSpecialPowerUpgrade")
            && names_power(module)
            && upgrade_mux_triggers(module, &has)
    })
}

impl GameLogic {
    /// Booby Trap planting (SpecialAbilityBoobyTrap): the Object's authored
    /// modules plus the upgrades it or its controlling Player completed.
    pub(crate) fn object_may_plant_booby_trap(&self, id: ObjectId) -> bool {
        let Some(object) = self.objects.get(&id) else {
            return false;
        };
        let owner = object.owner_player_id.and_then(|pid| self.players.get(&pid));
        authors_unpaused_ability(&object.thing().template, "SpecialAbilityBoobyTrap", |name| {
            object.has_upgrade_tag(name) || owner.is_some_and(|p| p.has_unlocked_upgrade(name))
        })
    }
}

impl Object {
    /// Run this Object's authored `PowerPlantUpgrade` modules whose UpgradeMux
    /// triggers `has` satisfies (PowerPlantUpgrade.cpp `upgradeImplementation`),
    /// each at most once. Returns the template's `EnergyBonus` (what
    /// `Player::addPowerBonus` adds) when any module executed.
    pub(crate) fn run_authored_power_plant_upgrades(
        &mut self,
        has: impl Fn(&str) -> bool,
    ) -> Option<i32> {
        let definition = self.thing().template.receiving_object_definition.clone()?;
        let mut executed = false;
        for (index, module) in definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("PowerPlantUpgrade"))
            .enumerate()
        {
            let key = format!(
                "PowerPlantUpgrade#{}",
                module
                    .module_tag
                    .clone()
                    .unwrap_or_else(|| format!("PowerPlantUpgrade{index}"))
            );
            if upgrade_mux_triggers(module, &has) && self.applied_upgrades.insert(key) {
                executed = true;
            }
        }
        executed.then(|| self.thing().template.energy_bonus.unwrap_or(0))
    }
}

impl GameLogic {
    /// `PowerPlantUpgrade::upgradeImplementation` for one Object: run its
    /// authored modules, then add the EnergyBonus and `extendRods(TRUE)`.
    pub(in crate::game_logic) fn apply_authored_power_plant_upgrades(
        &mut self,
        id: ObjectId,
        has: impl Fn(&str) -> bool,
    ) -> bool {
        let Some(bonus) = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.run_authored_power_plant_upgrades(has))
        else {
            return false;
        };
        self.complete_power_plant_upgrade(id, bonus);
        true
    }

    /// Player::addPowerBonus + PowerPlantUpdate::extendRods(TRUE).
    pub(in crate::game_logic) fn complete_power_plant_upgrade(&mut self, id: ObjectId, bonus: i32) {
        if let Some(object) = self.objects.get_mut(&id) {
            object.power_provided = object.power_provided.saturating_add(bonus);
        }
        let _ = self.begin_power_plant_rods_extend(id);
    }

    /// Run every recipient's authored `MaxHealthUpgrade` triggered by
    /// `upgrade_name`. Returns the number of objects changed.
    pub(in crate::game_logic) fn apply_authored_max_health_upgrades(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        let mut changed = 0u32;
        for obj in self.objects.registered_values_mut() {
            if !recipients.matches_object(obj) || !obj.is_alive() {
                continue;
            }
            if obj.run_authored_max_health_upgrades(|name| name.eq_ignore_ascii_case(upgrade_name))
            {
                self.health_events
                    .record_applied_heal(obj.id, obj.health.current);
                changed = changed.saturating_add(1);
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use crate::game_logic::{GameLogic, Team};

    fn admit(world: &mut GameLogic, name: &str, modules: &str) {
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-max-health.ini".into(),
                    format!(
                        "Object {name}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 200\nInitialHealth = 200\nEnd\n{modules}End\n"
                    ),
                )])
                .unwrap(),
            )
            .unwrap();
    }

    const ARMOR: &str = "Behavior = MaxHealthUpgrade ArmorTag\nTriggeredBy = Upgrade_TestPlating\nAddMaxHealth = 100.0\nChangeType = ADD_CURRENT_HEALTH_TOO\nEnd\n";

    #[test]
    fn authored_max_health_upgrade_applies_once_to_any_template_name() {
        let mut world = GameLogic::new();
        admit(&mut world, "PlainHauler", ARMOR);
        let id = world.create_object("PlainHauler", Team::USA, glam::Vec3::ZERO).unwrap();
        let object = world.host_object_mut(id).unwrap();
        assert!(object.run_authored_max_health_upgrades(|n| n == "Upgrade_TestPlating"));
        assert_eq!((object.health.maximum, object.health.current), (300.0, 300.0));
        // UpgradeMux executes a module once.
        assert!(!object.run_authored_max_health_upgrades(|n| n == "Upgrade_TestPlating"));
        assert_eq!(object.health.maximum, 300.0);
    }

    #[test]
    fn composite_armor_named_template_without_module_is_unchanged() {
        let mut world = GameLogic::new();
        admit(&mut world, "AmericaTankCrusader", "");
        let id = world
            .create_object("AmericaTankCrusader", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let object = world.host_object_mut(id).unwrap();
        assert!(!object.run_authored_max_health_upgrades(|n| n == "Upgrade_AmericaCompositeArmor"));
        assert_eq!(object.health.maximum, 200.0);
    }

    #[test]
    fn untriggered_module_and_default_change_type_keep_current_health() {
        let mut world = GameLogic::new();
        admit(
            &mut world,
            "PlainHauler",
            "Behavior = MaxHealthUpgrade ArmorTag\nTriggeredBy = Upgrade_TestPlating\nAddMaxHealth = 50.0\nEnd\n",
        );
        let id = world.create_object("PlainHauler", Team::USA, glam::Vec3::ZERO).unwrap();
        let object = world.host_object_mut(id).unwrap();
        assert!(!object.run_authored_max_health_upgrades(|n| n == "Upgrade_Other"));
        assert!(object.run_authored_max_health_upgrades(|n| n == "Upgrade_TestPlating"));
        // SAME_CURRENTHEALTH: maximum grows, current stays.
        assert_eq!((object.health.maximum, object.health.current), (250.0, 200.0));
    }
}

#[cfg(test)]
mod unpause_tests {
    use super::authored_unpause_powers;
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{GameLogic, Team};

    #[test]
    fn unpause_module_trigger_selects_its_power_for_any_template_name() {
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-unpause.ini".into(),
                    "Object PlainScanner\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                     Behavior = UnpauseSpecialPowerUpgrade UnpauseTag\nSpecialPowerTemplate = SpecialPowerRadarVanScan\nTriggeredBy = Upgrade_GLARadarVanScan\nEnd\nEnd\n\
                     Object GLAVehicleRadarVan\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
                        .into(),
                )])
                .unwrap(),
            )
            .unwrap();
        let plain = world.create_object("PlainScanner", Team::GLA, glam::Vec3::ZERO).unwrap();
        let named = world.create_object("GLAVehicleRadarVan", Team::GLA, glam::Vec3::ZERO).unwrap();
        let powers = |id, upgrade: &'static str| {
            authored_unpause_powers(&world.host_object(id).unwrap().thing().template, |n| {
                n == upgrade
            })
        };
        assert_eq!(powers(plain, "Upgrade_GLARadarVanScan"), vec![SpecialPowerType::RadarScan]);
        assert!(powers(plain, "Upgrade_Unrelated").is_empty());
        // The retail name alone authors nothing.
        assert!(powers(named, "Upgrade_GLARadarVanScan").is_empty());
    }
}

#[cfg(test)]
mod flag_tests {
    use super::AuthoredFlagUpgrades;
    use crate::game_logic::{GameLogic, Team};

    fn world(name: &str, modules: &str) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-flag-upgrades.ini".into(),
                    format!(
                        "Object {name}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{modules}End\n"
                    ),
                )])
                .unwrap(),
            )
            .unwrap();
        let id = world.create_object(name, Team::USA, glam::Vec3::ZERO).unwrap();
        (world, id)
    }

    const MODULES: &str = "Behavior = WeaponSetUpgrade WsTag\nTriggeredBy = Upgrade_TestGun\nEnd\n\
        Behavior = ArmorUpgrade ArTag\nTriggeredBy = Upgrade_TestPlate\nConflictsWith = Upgrade_TestBlock\nEnd\n\
        Behavior = WeaponBonusUpgrade WbTag\nTriggeredBy = Upgrade_TestA Upgrade_TestB\nRequiresAllTriggers = Yes\nEnd\n\
        Behavior = CommandSetUpgrade CsTag\nTriggeredBy = Upgrade_TestGun\nCommandSet = TestUpgradedSet\nEnd\n";

    #[test]
    fn authored_modules_set_flags_only_when_their_mux_triggers() {
        let (mut world, id) = world("PlainBuggy", MODULES);
        let object = world.host_object_mut(id).unwrap();
        let none = object.apply_authored_flag_upgrades(|n| n == "Upgrade_Unrelated");
        assert_eq!(none, AuthoredFlagUpgrades::default());
        let gun = object.apply_authored_flag_upgrades(|n| n == "Upgrade_TestGun");
        assert!(gun.weapon_set && !gun.armor_set && !gun.weapon_bonus);
        assert_eq!(gun.command_set.as_deref(), Some("TestUpgradedSet"));
        assert!(object.weapon_set_player_upgrade);
        // ConflictsWith blocks; RequiresAllTriggers needs both.
        let blocked = object
            .apply_authored_flag_upgrades(|n| n == "Upgrade_TestPlate" || n == "Upgrade_TestBlock");
        assert!(!blocked.armor_set);
        assert!(!object.apply_authored_flag_upgrades(|n| n == "Upgrade_TestA").weapon_bonus);
        assert!(object
            .apply_authored_flag_upgrades(|n| n == "Upgrade_TestA" || n == "Upgrade_TestB")
            .weapon_bonus);
    }

    #[test]
    fn unit_named_template_without_modules_gets_no_flags() {
        let (mut world, id) = world("AmericaVehicleHumvee", "");
        let object = world.host_object_mut(id).unwrap();
        let flags = object.apply_authored_flag_upgrades(|n| n == "Upgrade_AmericaTOWMissile");
        assert_eq!(flags, AuthoredFlagUpgrades::default());
        assert!(!object.weapon_set_player_upgrade);
    }
}

#[cfg(test)]
mod stealth_upgrade_tests {
    use crate::game_logic::host_upgrades::{UPGRADE_GLA_CAMOUFLAGE, UPGRADE_GLA_CAMO_NETTING};
    use crate::game_logic::{GameLogic, Team};

    #[test]
    fn stealth_upgrade_module_selects_camo_recipients_not_the_name() {
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-stealth-upgrade.ini".into(),
                    "Object PlainSneaker\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                     Behavior = StealthUpgrade CamoTag\nTriggeredBy = Upgrade_GLACamouflage\nEnd\nEnd\n\
                     Object PlainBunker\nKindOf = STRUCTURE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                     Behavior = StealthUpgrade NetTag\nTriggeredBy = Upgrade_GLACamoNetting\nEnd\nEnd\n\
                     Object GLAInfantryRebel\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
                        .into(),
                )])
                .unwrap(),
            )
            .unwrap();
        let sneaker = world.create_object("PlainSneaker", Team::GLA, glam::Vec3::ZERO).unwrap();
        let bunker = world.create_object("PlainBunker", Team::GLA, glam::Vec3::ZERO).unwrap();
        let rebel = world.create_object("GLAInfantryRebel", Team::GLA, glam::Vec3::ZERO).unwrap();

        assert_eq!(world.apply_camouflage_unlock_to_team(Team::GLA.into(), UPGRADE_GLA_CAMOUFLAGE), 1);
        assert!(world.host_object(sneaker).unwrap().innate_stealth);
        assert!(!world.host_object(bunker).unwrap().innate_stealth);
        assert!(!world.host_object(rebel).unwrap().innate_stealth, "the name authors no StealthUpgrade");

        assert_eq!(world.apply_camo_netting_unlock_to_team(Team::GLA.into(), UPGRADE_GLA_CAMO_NETTING), 1);
        assert!(world.host_object(bunker).unwrap().innate_stealth);
    }
}

#[cfg(test)]
mod replay_tests {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{GameLogic, Player, Team};

    /// Object::initObject -> updateUpgradeModules: a unit created after its
    /// Player completed the triggering upgrade has StartsPaused lifted.
    #[test]
    fn unit_created_after_upgrade_has_authored_power_unpaused() {
        let mut world = GameLogic::new();
        let mut owner = Player::new(0, Team::China, "Owner", true);
        owner.add_completed_upgrade("Upgrade_TestUnpause");
        world.players.insert(0, owner);
        world.players.insert(1, Player::new(1, Team::China, "Other", false));
        use game_engine::common::{
            rts::science::{ScienceNameFacts, ScienceStore},
            thing::receiving_module_content::ReceivingActionDefinitions,
        };
        let sciences = std::sync::Arc::new(ScienceNameFacts::from_store(&ScienceStore::new()));
        let mut actions = ReceivingActionDefinitions::default();
        actions
            .apply_source(
                "SpecialPower SpecialAbilityHelixNapalmBomb\nReloadTime = 6000\nEnd\n",
                &sciences,
            )
            .unwrap();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources_with_catalogs(
                    [(
                        "receiving-unpause-replay.ini".into(),
                        "Object PlainBomber\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                         Behavior = SpecialAbility AbilityTag\nSpecialPowerTemplate = SpecialAbilityHelixNapalmBomb\nStartsPaused = Yes\nEnd\n\
                         Behavior = UnpauseSpecialPowerUpgrade UnpauseTag\nSpecialPowerTemplate = SpecialAbilityHelixNapalmBomb\nTriggeredBy = Upgrade_TestUnpause\nEnd\nEnd\n"
                            .into(),
                    )],
                    sciences,
                    std::sync::Arc::new(actions),
                )
                .unwrap(),
            )
            .unwrap();
        let upgraded = world.create_object_for_player("PlainBomber", 0, glam::Vec3::ZERO).unwrap();
        let plain = world.create_object_for_player("PlainBomber", 1, glam::Vec3::X).unwrap();
        assert_eq!(
            world.host_object(plain).unwrap().thing().template.special_power_modules.len(),
            1,
            "authored SpecialAbility module admitted"
        );
        let paused = |id| {
            world
                .host_object(id)
                .unwrap()
                .is_special_power_countdown_paused(&SpecialPowerType::HelixNapalmBomb)
        };
        assert!(paused(plain), "the constructor pause stays without the upgrade");
        assert!(!paused(upgraded), "the completed upgrade lifts the pause on a new unit");
    }

    /// SpecialAbilityBoobyTrap is usable only by an Object whose authored
    /// modules carry the ability and an UnpauseSpecialPowerUpgrade the
    /// controlling Player's completed upgrade triggers; the name selects nothing.
    #[test]
    fn booby_trap_planting_follows_authored_modules_not_the_name() {
        let mut world = GameLogic::new();
        let mut owner = Player::new(0, Team::GLA, "Owner", true);
        owner.add_completed_upgrade("Upgrade_TestBoobyTrap");
        world.players.insert(0, owner);
        world.players.insert(1, Player::new(1, Team::GLA, "Other", false));
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-booby-trap.ini".into(),
                    "Object PlainPlanter\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                     Behavior = SpecialAbilityUpdate AbilityTag\nSpecialPowerTemplate = SpecialAbilityBoobyTrap\nEnd\n\
                     Behavior = UnpauseSpecialPowerUpgrade UnpauseTag\nSpecialPowerTemplate = SpecialAbilityBoobyTrap\nTriggeredBy = Upgrade_TestBoobyTrap\nEnd\nEnd\n\
                     Object GLAInfantryRebel\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
                        .into(),
                )])
                .unwrap(),
            )
            .unwrap();
        let upgraded = world.create_object_for_player("PlainPlanter", 0, glam::Vec3::ZERO).unwrap();
        let plain = world.create_object_for_player("PlainPlanter", 1, glam::Vec3::X).unwrap();
        let named = world.create_object_for_player("GLAInfantryRebel", 0, glam::Vec3::Y).unwrap();
        assert!(world.object_may_plant_booby_trap(upgraded));
        assert!(!world.object_may_plant_booby_trap(plain));
        assert!(!world.object_may_plant_booby_trap(named), "the retail name authors nothing");
    }
}
