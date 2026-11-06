//! Typed access to a template's authored behavior modules.
//!
//! Mirrors C++ `ThingTemplate::getBehaviorModuleInfo()` as consumed by
//! `Object::Object` / `ModuleFactory::newModule`
//! (GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingTemplate.cpp,
//! GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp): every
//! per-object behavior is selected by the module declarations the template
//! authors, never by the template's name.
//!
//! The data comes only from this session's admitted receiving definition
//! (`ThingTemplate::receiving_client_template`, falling back to the raw
//! receiving `ObjectDefinition` for class presence). A template without
//! admitted content authors no modules.

use super::ThingTemplate;

impl ThingTemplate {
    /// True when this template authors a behavior module of `class`
    /// (case-insensitive, as C++ INI module names are).
    pub(crate) fn authors_behavior_module(&self, class: &str) -> bool {
        if let Some(template) = self.receiving_client_template.as_ref() {
            if template
                .behavior_modules()
                .iter()
                .any(|entry| entry.name.as_str().eq_ignore_ascii_case(class))
            {
                return true;
            }
        }
        self.receiving_object_definition
            .as_deref()
            .is_some_and(|definition| {
                definition
                    .behavior_modules
                    .iter()
                    .any(|module| module.class_name.eq_ignore_ascii_case(class))
            })
    }

    /// C++ `WorkerAIUpdate::getUpgradedSupplyBoost` (WorkerAIUpdate.cpp:1376):
    /// the authored `UpgradedSupplyBoost` of this template's `WorkerAIUpdate`
    /// while the controlling player has completed `Upgrade_GLAWorkerShoes`,
    /// otherwise 0. A template without that module never boosts.
    pub(crate) fn worker_upgraded_supply_boost(&self, player_has_worker_shoes: bool) -> u32 {
        if player_has_worker_shoes && self.worker_ai_update {
            self.supply_truck_metadata
                .map_or(0, |metadata| metadata.upgraded_supply_boost)
        } else {
            0
        }
    }

    /// First authored behavior module of `class` whose typed module data is `T`.
    pub(crate) fn authored_module_data<T: 'static>(&self, class: &str) -> Option<&T> {
        self.receiving_client_template
            .as_ref()?
            .behavior_modules()
            .iter()
            .filter(|entry| entry.name.as_str().eq_ignore_ascii_case(class))
            .find_map(|entry| entry.data.downcast_ref::<T>())
    }

    /// Every authored behavior module of `class` whose typed data is `T`,
    /// in declaration order.
    pub(crate) fn authored_modules_data<T: 'static>(&self, class: &str) -> Vec<&T> {
        let Some(template) = self.receiving_client_template.as_ref() else {
            return Vec::new();
        };
        template
            .behavior_modules()
            .iter()
            .filter(|entry| entry.name.as_str().eq_ignore_ascii_case(class))
            .filter_map(|entry| entry.data.downcast_ref::<T>())
            .collect()
    }
}

#[cfg(test)]
mod worker_boost_tests {
    use crate::game_logic::{GameLogic, Team};

    fn world() -> GameLogic {
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-worker-boost.ini".into(),
                    "Object PlainHauler\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = WorkerAIUpdate WorkerTag\nUpgradedSupplyBoost = 8\nMaxBoxes = 3\nEnd\nEnd\n\
                     Object GLAInfantryWorker\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
                        .into(),
                )])
                .unwrap(),
            )
            .unwrap();
        world
    }

    #[test]
    fn upgraded_boost_follows_the_authored_worker_module_not_the_name() {
        let mut world = world();
        let hauler = world.create_object("PlainHauler", Team::GLA, glam::Vec3::ZERO).unwrap();
        let named = world
            .create_object("GLAInfantryWorker", Team::GLA, glam::Vec3::ZERO)
            .unwrap();
        let boost = |world: &GameLogic, id, shoes| {
            world
                .host_object(id)
                .unwrap()
                .thing()
                .template
                .worker_upgraded_supply_boost(shoes)
        };
        assert_eq!(boost(&world, hauler, true), 8);
        assert_eq!(boost(&world, hauler, false), 0);
        assert_eq!(boost(&world, named, true), 0);
    }
}
