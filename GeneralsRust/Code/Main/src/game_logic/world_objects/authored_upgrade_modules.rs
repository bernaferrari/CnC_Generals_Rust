//! Authored upgrade modules applied through the UpgradeMux trigger rules.
//!
//! Mirrors C++ `UpgradeMux::attemptUpgrade` / `testUpgradeConditions`
//! (GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UpgradeModule.cpp)
//! and `MaxHealthUpgrade::upgradeImplementation` (MaxHealthUpgrade.cpp): an
//! Object raises its body's maximum health only because its template authors
//! a `MaxHealthUpgrade` whose `TriggeredBy` names the completed upgrade. No
//! unit or template name selects a recipient.
use super::super::*;

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

/// Armor/command-set residuals. WeaponSetUpgrade and WeaponBonusUpgrade run
/// only in the world's typed source-order traversal, including at birth.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct AuthoredFlagUpgrades {
    /// ArmorUpgrade -> `setArmorSetFlag(ARMORSET_PLAYER_UPGRADE)`.
    pub armor_set: bool,
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
            if !(is("ArmorUpgrade") || is("CommandSetUpgrade"))
                || !upgrade_mux_triggers(module, &has)
            {
                continue;
            }
            if is("ArmorUpgrade") {
                out.armor_set = true;
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
        if applied.armor_set {
            self.set_armor_set_player_upgrade(true);
        }
        if let Some(set) = applied.command_set.clone() {
            self.set_command_set_override(Some(set));
        }
        applied
    }

    /// C++ `StealthUpgrade::upgradeImplementation` (StealthUpgrade.cpp:27-42)
    /// for this Object's authored StealthUpgrade modules triggered by its
    /// upgrade mask plus `extra`: `setStatus(OBJECT_STATUS_CAN_STEALTH)`; the
    /// StealthUpdate then cloaks once its StealthDelay has passed. Returns
    /// whether a module executed.
    pub(crate) fn run_authored_stealth_upgrades(&mut self, extra: impl Fn(&str) -> bool) -> bool {
        if !authors_triggered_stealth_upgrade(&self.thing().template, |name| {
            extra(name) || self.has_upgrade_tag(name)
        }) {
            return false;
        }
        self.innate_stealth = true;
        true
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

    /// C++ MaxHealthUpgrade.cpp:56-68. The ordered world callback owns mux
    /// admission, FX/removals and the execution latch; this is only the body
    /// operation, shared by object grants, player research and object birth.
    pub(crate) fn apply_authored_max_health_upgrade(
        &mut self,
        data: &gamelogic::object::upgrade::max_health_upgrade::MaxHealthUpgradeModuleData,
    ) {
        use gamelogic::object::body::body_module::MaxHealthChangeType;
        use crate::game_logic::object::MaxHealthChange;
        let change = match data.change_type() {
            MaxHealthChangeType::SameCurrentHealth => MaxHealthChange::KeepCurrent,
            MaxHealthChangeType::PreserveRatio => MaxHealthChange::PreserveRatio,
            MaxHealthChangeType::AddCurrentHealthToo => MaxHealthChange::AddToCurrent,
            MaxHealthChangeType::FullyHeal => MaxHealthChange::FullyHeal,
        };
        self.set_body_max_health_with_mode(self.health.maximum + data.add_max_health(), change);
    }

    /// `ModelConditionUpgrade` / `SubObjectsUpgrade::upgradeImplementation`
    /// (ModelConditionUpgrade.cpp:60-67, SubObjectsUpgrade.cpp:75-117) for
    /// every authored module whose UpgradeMux `has` satisfies, each once
    /// (`UpgradeMux::m_upgradeExecuted`). Returns the shown sub-object names.
    pub(crate) fn run_authored_visual_upgrades(
        &mut self,
        has: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        let model_condition = self.thing().template.model_condition_upgrades.clone();
        let sub_objects = self.thing().template.sub_objects_upgrades.clone();
        for module in &model_condition {
            if module.mux.satisfied_by(&has)
                && self
                    .applied_upgrades
                    .insert(format!("ModelConditionUpgrade#{}", module.tag))
            {
                if let Some(bit) = module.condition_bit {
                    self.model_condition_bits |= 1u128 << bit;
                }
            }
        }
        let mut shown = Vec::new();
        for module in &sub_objects {
            if !module.mux.satisfied_by(&has)
                || !self
                    .applied_upgrades
                    .insert(format!("SubObjectsUpgrade#{}", module.tag))
            {
                continue;
            }
            let show: Vec<&str> = module.show.iter().map(String::as_str).collect();
            let hide: Vec<&str> = module.hide.iter().map(String::as_str).collect();
            self.sub_object_visibility.apply_show_hide(&show, &hide);
            shown.extend(module.show.iter().cloned());
        }
        shown
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

}

#[cfg(test)]
mod tests {
    use crate::game_logic::{GameLogic, Player, Team};

    fn admit(world: &mut GameLogic, name: &str, modules: &str) {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world.add_player(Player::new(1, Team::USA, "Other owner", false));
        for (upgrade, kind) in [
            ("Upgrade_TestPlating", "PLAYER"),
            ("Upgrade_TestObjectPlating", "OBJECT"),
            ("Upgrade_TestBlockPlating", "OBJECT"),
            ("Upgrade_Other", "OBJECT"),
            ("Upgrade_AmericaCompositeArmor", "PLAYER"),
        ] {
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                world, upgrade, kind, 0, 1,
            );
        }
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
        let id = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        // Object::giveUpgrade accepts even a Type=PLAYER template.
        world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
        let object = world.host_object(id).unwrap();
        assert_eq!((object.health.maximum, object.health.current), (300.0, 300.0));
        assert!(object.completed_object_upgrades.contains("Upgrade_TestPlating"));
        assert!(!world.players[&0].has_unlocked_upgrade("Upgrade_TestPlating"));
        world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
        assert_eq!(world.host_object(id).unwrap().health.maximum, 300.0);
    }

    #[test]
    fn composite_armor_named_template_without_module_is_unchanged() {
        let mut world = GameLogic::new();
        admit(&mut world, "AmericaTankCrusader", "");
        let id = world
            .create_object_for_player("AmericaTankCrusader", 0, glam::Vec3::ZERO)
            .unwrap();
        world.apply_upgrade_to_object(id, "Upgrade_AmericaCompositeArmor");
        assert_eq!(world.host_object(id).unwrap().health.maximum, 200.0);
    }

    #[test]
    fn untriggered_module_and_default_change_type_keep_current_health() {
        let mut world = GameLogic::new();
        admit(
            &mut world,
            "PlainHauler",
            "Behavior = MaxHealthUpgrade ArmorTag\nTriggeredBy = Upgrade_TestPlating\nAddMaxHealth = 50.0\nEnd\n",
        );
        let id = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        world.apply_upgrade_to_object(id, "Upgrade_Other");
        assert_eq!(world.host_object(id).unwrap().health.maximum, 200.0);
        world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
        let object = world.host_object(id).unwrap();
        // SAME_CURRENTHEALTH: maximum grows, current stays.
        assert_eq!((object.health.maximum, object.health.current), (250.0, 200.0));
    }

    #[test]
    fn max_health_live_research_and_birth_use_only_the_controlling_player() {
        let mut world = GameLogic::new();
        admit(&mut world, "PlainHauler", ARMOR);
        let owned = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        let other = world.create_object_for_player("PlainHauler", 1, glam::Vec3::ZERO).unwrap();
        world.players.get_mut(&0).unwrap().complete_researched_upgrade(
            "Upgrade_TestPlating", gamelogic::upgrade::UpgradeType::Player,
        );
        world.apply_host_upgrade_complete(Team::USA, 0, "Upgrade_TestPlating");
        assert_eq!(world.host_object(owned).unwrap().health.maximum, 300.0);
        assert_eq!(world.host_object(other).unwrap().health.maximum, 200.0);
        assert!(world.host_object(owned).unwrap().completed_object_upgrades.is_empty());
        let born = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        assert_eq!(world.host_object(born).unwrap().health.maximum, 300.0);
        assert!(world.host_object(born).unwrap().completed_object_upgrades.is_empty());
        for id in [owned, born, other] {
            world.objects.get_mut(&id).unwrap().take_damage_from_immediate(
                250.0, None, world.frame, &mut world.health_events,
            );
        }
        for id in [owned, born] {
            let object = world.host_object(id).unwrap();
            assert!(object.is_alive());
            assert_eq!(object.health.current, 50.0,
                "combat must consume the upgraded live body, not just a display maximum");
        }
        assert!(!world.host_object(other).unwrap().is_alive());
    }

    #[test]
    fn max_health_object_completion_combines_masks_and_honors_conflicts() {
        let mut world = GameLogic::new();
        admit(&mut world, "PlainHauler",
            "Behavior = MaxHealthUpgrade ArmorTag\nTriggeredBy = Upgrade_TestPlating Upgrade_TestObjectPlating\nRequiresAllTriggers = Yes\nConflictsWith = Upgrade_TestBlockPlating\nAddMaxHealth = 100\nChangeType = ADD_CURRENT_HEALTH_TOO\nEnd\n");
        let owned = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        let peer = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        world.players.get_mut(&0).unwrap().complete_researched_upgrade(
            "Upgrade_TestPlating", gamelogic::upgrade::UpgradeType::Player,
        );
        world.apply_host_upgrade_complete(Team::USA, 0, "Upgrade_TestPlating");
        assert_eq!(world.host_object(owned).unwrap().health.maximum, 200.0);
        world.apply_upgrade_to_object(owned, "Upgrade_TestBlockPlating");
        world.record_host_upgrade_queued(0, Team::USA, "Upgrade_TestObjectPlating", Some(owned));
        world.apply_host_upgrade_complete(Team::USA, 0, "Upgrade_TestObjectPlating");
        assert_eq!(world.host_object(owned).unwrap().health.maximum, 200.0);
        assert!(world.remove_object_upgrade_with_receiving_rules(owned, "Upgrade_TestBlockPlating"));
        world.apply_upgrade_to_object(owned, "Upgrade_TestObjectPlating");
        assert_eq!(world.host_object(owned).unwrap().health.maximum, 300.0);
        assert_eq!(world.host_object(peer).unwrap().health.maximum, 200.0);
        assert!(!world.players[&0].has_unlocked_upgrade("Upgrade_TestObjectPlating"));
        assert!(world.remove_object_upgrade_with_receiving_rules(owned, "Upgrade_TestObjectPlating"));
        assert_eq!(world.host_object(owned).unwrap().health.maximum, 300.0,
            "resetUpgrade must not undo the health addition");
        world.apply_upgrade_to_object(owned, "Upgrade_TestObjectPlating");
        assert_eq!(world.host_object(owned).unwrap().health.maximum, 400.0);
    }

    #[test]
    fn max_health_waits_for_construction_and_ignores_destroyed_objects() {
        let mut world = GameLogic::new();
        admit(&mut world, "PlainHauler", ARMOR);
        let id = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        world.host_object_mut(id).unwrap().status.under_construction = true;
        world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
        assert_eq!(world.host_object(id).unwrap().health.maximum, 200.0);
        world.host_object_mut(id).unwrap().status.under_construction = false;
        world.apply_researched_player_upgrades_to_object_with_interpreter(
            id, None, &mut Default::default(),
        );
        assert_eq!(world.host_object(id).unwrap().health.maximum, 300.0);
        world.remove_object_upgrade_with_receiving_rules(id, "Upgrade_TestPlating");
        world.host_object_mut(id).unwrap().status.destroyed = true;
        world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
        assert_eq!(world.host_object(id).unwrap().health.maximum, 300.0);
    }

    #[test]
    fn max_health_removal_uses_source_order_and_the_captured_mask() {
        // Object.cpp:2421-2437 snapshots once; UpgradeModule.cpp:140-146
        // removes before implementation, then latches. Re-execution changes
        // the real body, so this catches family batching without a mock log.
        for health_first in [true, false] {
            let mut world = GameLogic::new();
            let reset = "Behavior = StatusBitsUpgrade ResetHealth\nTriggeredBy = Upgrade_TestPlating\nRemovesUpgrades = Upgrade_TestPlating\nStatusToSet = BOOBY_TRAPPED\nEnd\n";
            let modules = if health_first {
                format!("{ARMOR}{reset}")
            } else {
                format!("{reset}{ARMOR}")
            };
            admit(&mut world, "PlainHauler", &modules);
            let id = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
            world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
            assert_eq!(world.host_object(id).unwrap().health.maximum, 300.0);
            assert!(!world.host_object(id).unwrap().completed_object_upgrades.contains("Upgrade_TestPlating"));
            world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
            assert_eq!(world.host_object(id).unwrap().health.maximum,
                if health_first { 400.0 } else { 300.0 });
        }
    }

    #[test]
    fn max_health_birth_traverses_mux_once_even_when_a_later_module_resets_it() {
        let mut world = GameLogic::new();
        let reset = "Behavior = StatusBitsUpgrade ResetHealth\nTriggeredBy = Upgrade_TestPlating\nRemovesUpgrades = Upgrade_TestPlating\nStatusToSet = BOOBY_TRAPPED\nEnd\n";
        admit(&mut world, "PlainHauler", &format!("{ARMOR}{reset}"));
        world.players.get_mut(&0).unwrap().complete_researched_upgrade(
            "Upgrade_TestPlating", gamelogic::upgrade::UpgradeType::Player,
        );
        let id = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        let object = world.host_object(id).unwrap();
        assert_eq!((object.health.maximum, object.health.current), (300.0, 300.0));
        assert!(object.has_object_status_bit("BOOBY_TRAPPED"));
        assert!(object.completed_object_upgrades.is_empty());
        assert!(world.players[&0].has_unlocked_upgrade("Upgrade_TestPlating"),
            "Object::removeUpgrade never clears the player's mask");
    }

    #[test]
    fn max_health_uses_each_receiving_world_definition_and_object_latch() {
        let mut first = GameLogic::new();
        let mut second = GameLogic::new();
        admit(&mut first, "PlainHauler", ARMOR);
        admit(&mut second, "PlainHauler", &ARMOR.replace("100.0", "700.0"));
        let a = first.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        let b = second.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
        assert_eq!(a, b);
        first.apply_upgrade_to_object(a, "Upgrade_TestPlating");
        assert_eq!(first.host_object(a).unwrap().health.current, 300.0);
        assert_eq!(second.host_object(b).unwrap().health.current, 200.0);
        second.apply_upgrade_to_object(b, "Upgrade_TestPlating");
        assert_eq!(second.host_object(b).unwrap().health.current, 900.0);
        assert_eq!(first.host_object(a).unwrap().health.current, 300.0);
    }

    #[test]
    fn max_health_preserve_ratio_and_fully_heal_reach_the_live_body() {
        for (change, expected) in [("PRESERVE_RATIO", 150.0), ("FULLY_HEAL", 300.0)] {
            let mut world = GameLogic::new();
            admit(&mut world, "PlainHauler", &ARMOR.replace("ADD_CURRENT_HEALTH_TOO", change));
            let id = world.create_object_for_player("PlainHauler", 0, glam::Vec3::ZERO).unwrap();
            world.objects.get_mut(&id).unwrap().take_damage_from_immediate(
                100.0, None, world.frame, &mut world.health_events,
            );
            world.apply_upgrade_to_object(id, "Upgrade_TestPlating");
            let object = world.host_object(id).unwrap();
            assert_eq!((object.health.maximum, object.health.current), (300.0, expected));
        }
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
    use crate::game_logic::{GameLogic, Player, Team};

    fn world(name: &str, modules: &str) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        for upgrade in ["Upgrade_TestGun", "Upgrade_TestPlate", "Upgrade_TestBlock",
            "Upgrade_TestA", "Upgrade_TestB", "Upgrade_Unrelated", "Upgrade_AmericaTOWMissile"] {
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &mut world, upgrade, "OBJECT", 0, 1,
            );
        }
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
        let id = world.create_object_for_player(name, 0, glam::Vec3::ZERO).unwrap();
        (world, id)
    }

    const MODULES: &str = "Behavior = WeaponSetUpgrade WsTag\nTriggeredBy = Upgrade_TestGun\nEnd\n\
        Behavior = ArmorUpgrade ArTag\nTriggeredBy = Upgrade_TestPlate\nConflictsWith = Upgrade_TestBlock\nEnd\n\
        Behavior = WeaponBonusUpgrade WbTag\nTriggeredBy = Upgrade_TestA Upgrade_TestB\nRequiresAllTriggers = Yes\nEnd\n\
        Behavior = CommandSetUpgrade CsTag\nTriggeredBy = Upgrade_TestGun\nCommandSet = TestUpgradedSet\nEnd\n";

    #[test]
    fn authored_modules_set_flags_only_when_their_mux_triggers() {
        let (mut world, id) = world("PlainBuggy", MODULES);
        world.apply_upgrade_to_object(id, "Upgrade_Unrelated");
        assert!(!world.host_object(id).unwrap().test_weapon_condition(3));
        assert!(!world.host_object(id).unwrap().weapon_bonus_player_upgrade);
        world.apply_upgrade_to_object(id, "Upgrade_TestGun");
        assert!(world.host_object(id).unwrap().test_weapon_condition(3));
        let object = world.host_object_mut(id).unwrap();
        let flags = object.apply_authored_flag_upgrades(|n| n == "Upgrade_TestGun");
        assert_eq!(flags.command_set.as_deref(), Some("TestUpgradedSet"));
        assert!(!object.apply_authored_flag_upgrades(
            |n| n == "Upgrade_TestPlate" || n == "Upgrade_TestBlock",
        ).armor_set);
        world.apply_upgrade_to_object(id, "Upgrade_TestA");
        assert!(!world.host_object(id).unwrap().weapon_bonus_player_upgrade);
        world.apply_upgrade_to_object(id, "Upgrade_TestB");
        assert!(world.host_object(id).unwrap().weapon_bonus_player_upgrade);
    }

    #[test]
    fn weapon_upgrade_rebinds_selected_row_and_preserves_shared_lock() {
        use crate::game_logic::object::WeaponLockType;

        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &mut world, "Upgrade_TestGun", "OBJECT", 0, 1,
        );
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "upgrade-row.ini".into(),
                "Object UpgradeRow\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nWeaponSet\nConditions = None\nWeapon = PRIMARY UpgradeBase\nWeapon = SECONDARY UpgradeBase\nEnd\nWeaponSet\nConditions = PLAYER_UPGRADE\nWeapon = PRIMARY UpgradeNew\nWeapon = SECONDARY UpgradeNew\nWeaponLockSharedAcrossSets = Yes\nEnd\nBehavior = WeaponSetUpgrade UpgradeTag\nTriggeredBy = Upgrade_TestGun\nEnd\nEnd\n".into(),
            )]).unwrap(),
        ).unwrap();
        let mut definitions = std::collections::HashMap::new();
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon UpgradeBase\nPrimaryDamage = 10\nClipSize = 3\nEnd\nWeapon UpgradeNew\nPrimaryDamage = 20\nClipSize = 4\nSuspendFXDelay = 1000\nEnd\n",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        );
        world.admit_weapon_content_snapshot(
            definitions.into_values().map(|rule| (rule.name.clone(), rule)).collect(),
            &[],
        ).unwrap();
        let id = world.create_object_for_player("UpgradeRow", 0, glam::Vec3::ZERO).unwrap();
        assert!(world.host_object_mut(id).unwrap().set_weapon_lock(1, WeaponLockType::LockedPermanently));
        world.frame = 80;
        world.apply_upgrade_to_object(id, "Upgrade_TestGun");
        let object = world.host_object_mut(id).unwrap();
        assert!(object.is_weapon_locked());
        assert_eq!(object.active_weapon_slot, 1);
        assert_eq!(object.weapon_slot(0).unwrap().damage, 20.0);
        assert_eq!(object.weapon_slot(0).unwrap().ammo, Some(4));
        assert_eq!(object.weapon_slot(0).unwrap().suspend_fx_frame, 110);
        object.weapon.as_mut().unwrap().ammo = Some(1);
        world.frame = 81;
        world.apply_upgrade_to_object(id, "Upgrade_TestGun");
        let object = world.host_object(id).unwrap();
        assert_eq!(object.weapon_slot(0).unwrap().ammo, Some(1));
        assert!(object.is_weapon_locked());
    }

    #[test]
    fn unit_named_template_without_modules_gets_no_flags() {
        let (mut world, id) = world("AmericaVehicleHumvee", "");
        world.apply_upgrade_to_object(id, "Upgrade_AmericaTOWMissile");
        let object = world.host_object_mut(id).unwrap();
        let flags = object.apply_authored_flag_upgrades(|n| n == "Upgrade_AmericaTOWMissile");
        assert_eq!(flags, AuthoredFlagUpgrades::default());
        assert!(!object.test_weapon_condition(3));
    }

    fn weapon_clock_world(modules: &str) -> (GameLogic, crate::game_logic::ObjectId, crate::game_logic::ObjectId) {
        use gamelogic::weapon::{WeaponBonus, WeaponBonusConditionType, WeaponBonusField, WeaponBonusSet, WeaponTemplate};
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world.add_player(Player::new(1, Team::China, "Target", false));
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &mut world, "Upgrade_OrderedWeapon", "PLAYER", 0, 1,
        );
        let mut base = WeaponTemplate::new("OrderedBase".into());
        base.primary_damage = 1.0;
        base.attack_range = 100.0;
        base.min_delay_between_shots = 30;
        base.max_delay_between_shots = 90;
        base.projectile_name = "NONE".into();
        let mut bonus = WeaponBonus::new();
        bonus.set_field(WeaponBonusField::RateOfFire, 2.0);
        let mut bonuses = WeaponBonusSet::new();
        bonuses.set_bonus(WeaponBonusConditionType::PlayerUpgrade, bonus);
        base.extra_bonus = Some(bonuses);
        let mut upgraded = base.clone();
        upgraded.name = "OrderedNew".into();
        upgraded.primary_damage = 7.0;
        world.admit_receiving_weapon_snapshot(vec![
            ("OrderedBase".into(), base), ("OrderedNew".into(), upgraded),
        ]).unwrap();
        world.admit_object_ini_sources([(
            "ordered-weapon-upgrades.ini".into(),
            format!("Object OrderedActor\nKindOf = INFANTRY CAN_ATTACK\nBody = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nWeaponSet\nConditions = None\nWeapon = PRIMARY OrderedBase\nEnd\nWeaponSet\nConditions = PLAYER_UPGRADE\nWeapon = PRIMARY OrderedNew\nEnd\nBehavior = AIUpdateInterface AI\nEnd\n{modules}End\n"),
        )]).unwrap();
        let source = world.create_object_for_player("OrderedActor", 0, glam::Vec3::ZERO).unwrap();
        let target = world.create_object_for_player("OrderedActor", 1, glam::Vec3::new(20.0, 0.0, 0.0)).unwrap();
        world.logic_random.seed_random(412);
        (world, source, target)
    }

    const BONUS: &str = "Behavior = WeaponBonusUpgrade Bonus\nTriggeredBy = Upgrade_OrderedWeapon\nEnd\n";
    const SET: &str = "Behavior = WeaponSetUpgrade Set\nTriggeredBy = Upgrade_OrderedWeapon\nEnd\n";

    #[test]
    fn weapon_bonus_upgrade_restarts_real_waiting_weapon_once_with_receiving_rates() {
        let (mut world, source, target) = weapon_clock_world(BONUS);
        world.frame = 15;
        assert!(world.fire_owned_passive_weapon(source, target, 0, glam::Vec3::ZERO));
        let expected = game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot());
        let deadline = 20 + (expected.next_int(30, 90) as f32 / 2.0).floor() as u32;
        world.frame = 20;
        world.apply_upgrade_to_object(source, "Upgrade_OrderedWeapon");
        assert_eq!(world.host_object(source).unwrap().weapon.as_ref().unwrap().next_fire_frame, deadline);
        assert_eq!(world.logic_random.seed_words(), expected.seed_words());
        // Removing resets the module, not the already-set condition. A fresh
        // module execution must not restart the timer without a changed bit.
        world.remove_object_upgrade_with_receiving_rules(source, "Upgrade_OrderedWeapon");
        world.apply_upgrade_to_object(source, "Upgrade_OrderedWeapon");
        assert_eq!(world.host_object(source).unwrap().weapon.as_ref().unwrap().next_fire_frame, deadline);
        assert_eq!(world.logic_random.seed_words(), expected.seed_words());
        world.frame = deadline - 1;
        assert!(!world.fire_owned_passive_weapon(source, target, 0, glam::Vec3::ZERO));
        world.frame = deadline;
        assert!(world.fire_owned_passive_weapon(source, target, 0, glam::Vec3::ZERO));
    }

    #[test]
    fn weapon_bonus_and_set_callbacks_preserve_source_order_and_rng() {
        for bonus_first in [true, false] {
            let modules = if bonus_first { format!("{BONUS}{SET}") } else { format!("{SET}{BONUS}") };
            let (mut world, source, target) = weapon_clock_world(&modules);
            world.frame = 15;
            assert!(world.fire_owned_passive_weapon(source, target, 0, glam::Vec3::ZERO));
            let expected = game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot());
            if bonus_first {
                expected.next_int(30, 90);
            }
            world.frame = 20;
            world.apply_upgrade_to_object(source, "Upgrade_OrderedWeapon");
            assert_eq!(world.logic_random.seed_words(), expected.seed_words(),
                "bonus-before-set restarts the old Weapon; set-before-bonus sees the new READY Weapon");
            assert_eq!(world.host_object(source).unwrap().weapon.as_ref().unwrap().damage, 7.0);
            let before = world.host_object(target).unwrap().health.current;
            assert!(world.fire_owned_passive_weapon(source, target, 0, glam::Vec3::ZERO));
            assert_eq!(world.host_object(target).unwrap().health.current, before - 7.0);
            world.players.get_mut(&0).unwrap().complete_researched_upgrade(
                "Upgrade_OrderedWeapon", gamelogic::upgrade::UpgradeType::Player,
            );
            let born = world.create_object_for_player("OrderedActor", 0, glam::Vec3::ZERO).unwrap();
            assert_eq!(world.host_object(born).unwrap().weapon.as_ref().unwrap().damage, 7.0);
            assert!(world.host_object(born).unwrap().weapon_bonus_player_upgrade);
            assert!(world.host_object(born).unwrap().completed_object_upgrades.is_empty());
        }
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

        assert!(world.host_object_mut(sneaker).unwrap().run_authored_stealth_upgrades(|n| n == UPGRADE_GLA_CAMOUFLAGE));
        assert!(!world.host_object_mut(bunker).unwrap().run_authored_stealth_upgrades(|n| n == UPGRADE_GLA_CAMOUFLAGE));
        assert!(!world.host_object_mut(rebel).unwrap().run_authored_stealth_upgrades(|n| n == UPGRADE_GLA_CAMOUFLAGE));
        assert!(world.host_object(sneaker).unwrap().innate_stealth);
        assert!(!world.host_object(bunker).unwrap().innate_stealth);
        assert!(!world.host_object(rebel).unwrap().innate_stealth, "the name authors no StealthUpgrade");

        assert!(world.host_object_mut(bunker).unwrap().run_authored_stealth_upgrades(|n| n == UPGRADE_GLA_CAMO_NETTING));
        assert!(world.host_object(bunker).unwrap().innate_stealth);
    }
}

#[cfg(test)]
mod replay_tests {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{GameLogic, Player, Team};

    #[test]
    fn completed_rocket_pod_upgrade_selects_authored_row_at_factory() {
        let mut world = GameLogic::new();
        let mut owner = Player::new(0, Team::USA, "Owner", true);
        owner.add_completed_upgrade("Upgrade_AmericaComancheRocketPods");
        world.players.insert(0, owner);
        world.players.insert(1, Player::new(1, Team::USA, "Other", false));
        // The completed name only contributes a mask through its template.
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &mut world, "Upgrade_AmericaComancheRocketPods", "PLAYER", 0, 1,
        );
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "upgrade-factory-row.ini".into(),
                "Object FactoryAircraft\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nWeaponSet\nConditions = None\nWeapon = PRIMARY Comanche20mmCannonWeapon\nEnd\nWeaponSet\nConditions = PLAYER_UPGRADE\nWeapon = PRIMARY FactoryUpgradedGun\nWeapon = TERTIARY FactoryPod\nEnd\nBehavior = WeaponSetUpgrade UpgradeTag\nTriggeredBy = Upgrade_AmericaComancheRocketPods\nEnd\nEnd\n".into(),
            )]).unwrap(),
        ).unwrap();
        let mut definitions = std::collections::HashMap::new();
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon Comanche20mmCannonWeapon\nPrimaryDamage = 11\nEnd\nWeapon FactoryUpgradedGun\nPrimaryDamage = 22\nEnd\nWeapon FactoryPod\nPrimaryDamage = 33\nEnd\n",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        );
        world.admit_weapon_content_snapshot(
            definitions.into_values().map(|rule| (rule.name.clone(), rule)).collect(),
            &[],
        ).unwrap();
        let upgraded = world.create_object_for_player("FactoryAircraft", 0, glam::Vec3::ZERO).unwrap();
        let plain = world.create_object_for_player("FactoryAircraft", 1, glam::Vec3::X).unwrap();
        assert_eq!(world.host_object(upgraded).unwrap().weapon_slot(0).unwrap().damage, 22.0);
        assert_eq!(world.host_object(upgraded).unwrap().weapon_slot(2).unwrap().damage, 33.0);
        assert_eq!(world.host_object(plain).unwrap().weapon_slot(0).unwrap().damage, 11.0);
        assert!(world.host_object(plain).unwrap().weapon_slot(2).is_none());
    }

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

#[cfg(test)]
mod visual_upgrade_tests {
    use crate::game_logic::{GameLogic, Team};

    const MODULES: &str = "Behavior = ModelConditionUpgrade McTag\nTriggeredBy = Upgrade_TestRadar\nConditionFlag = RADAR_UPGRADED\nEnd\n\
        Behavior = SubObjectsUpgrade SoTag\nTriggeredBy = Upgrade_TestBio\nShowSubObjects = Bombload02\nHideSubObjects = Bombload01\nEnd\n";

    /// ModelConditionUpgrade.cpp / SubObjectsUpgrade.cpp: the authored
    /// ConditionFlag and Show/HideSubObjects run once when the module's
    /// UpgradeMux triggers; a retail-named template without modules is inert.
    #[test]
    fn authored_visual_upgrades_follow_their_modules_not_names() {
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-visual-upgrades.ini".into(),
                    format!(
                        "Object PlainTruck\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{MODULES}End\n\
                         Object GLAVehicleBombTruck\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
                    ),
                )])
                .unwrap(),
            )
            .unwrap();
        let radar_bit = 1u128
            << crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                "RADAR_UPGRADED",
            )
            .unwrap();
        let plain = world.create_object("PlainTruck", Team::GLA, glam::Vec3::ZERO).unwrap();
        let named = world
            .create_object("GLAVehicleBombTruck", Team::GLA, glam::Vec3::X)
            .unwrap();

        let object = world.host_object_mut(plain).unwrap();
        assert!(object.run_authored_visual_upgrades(|n| n == "Upgrade_Other").is_empty());
        assert_eq!(object.model_condition_bits & radar_bit, 0);
        assert!(object.run_authored_visual_upgrades(|n| n == "Upgrade_TestRadar").is_empty());
        assert_ne!(object.model_condition_bits & radar_bit, 0);
        let shown = object.run_authored_visual_upgrades(|n| n == "Upgrade_TestBio");
        assert_eq!(shown, vec!["Bombload02".to_string()]);
        assert!(object.sub_object_visibility.is_hidden("Bombload01"));
        // UpgradeMux::isAlreadyUpgraded: each module executes once.
        assert!(object.run_authored_visual_upgrades(|n| n == "Upgrade_TestBio").is_empty());

        let named = world.host_object_mut(named).unwrap();
        assert!(
            named
                .run_authored_visual_upgrades(|n| n.starts_with("Upgrade_"))
                .is_empty()
        );
        assert_eq!(named.model_condition_bits & radar_bit, 0);
    }
}
