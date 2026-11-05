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

impl Object {
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

impl GameLogic {
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
