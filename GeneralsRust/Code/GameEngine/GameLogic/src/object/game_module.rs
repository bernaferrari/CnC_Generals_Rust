//! Gamelogic-side module surface with C++ `BehaviorModule` virtuals.
//!
//! The engine `Module` trait (game_engine) cannot name `DieModuleInterface`,
//! `UpgradeModuleInterface`, or `SpecialPowerModuleInterface` because those
//! traits (and `DamageInfo`) live in this crate. C++ dispatches die/upgrade/
//! special-power through virtuals on the module itself; `GameModule` is that
//! virtual surface. Modules stored in the [`ModuleSlot::Game`] variant are
//! dispatched virtually — no `as_any` recovery, no per-type ladder.
//!
//! Engine-factory modules without a gamelogic surface stay in
//! [`ModuleSlot::Engine`] and keep the legacy typed-recovery ladders until
//! their factories register here.

use crate::modules::{DieModuleInterface, UpgradeModuleInterface};
use crate::object::ModuleEntry;
use crate::object::Object;
use crate::damage::DamageInfo;
use crate::common::UpgradeMaskType;
use game_engine::common::thing::module::Module;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

/// C++ `BehaviorModule::getDie/getUpgrade/getSpecialPower` virtuals.
pub trait GameModule: Module {
    /// C++ `BehaviorModule::getDie()`.
    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        None
    }

    /// C++ `BehaviorModule::getUpgrade()`.
    fn get_upgrade(&mut self) -> Option<&mut dyn UpgradeModuleInterface> {
        None
    }

    /// C++ `BehaviorModule::getSpecialPower()`.
    fn get_special_power(&mut self) -> Option<&mut dyn crate::modules::SpecialPowerModuleInterface> {
        None
    }
}

/// How a constructed module is stored on its [`ModuleEntry`].
pub enum ModuleSlot {
    /// Constructed by gamelogic with its interface surface intact.
    Game(Box<dyn GameModule>),
    /// Produced by the engine module factory, surface erased.
    Engine(Box<dyn Module>),
}

impl ModuleSlot {
    pub(crate) fn as_module(&mut self) -> &mut dyn Module {
        match self {
            Self::Game(game) => game.as_mut(),
            Self::Engine(module) => module.as_mut(),
        }
    }

    pub(crate) fn as_game(&mut self) -> Option<&mut dyn GameModule> {
        match self {
            Self::Game(game) => Some(game.as_mut()),
            Self::Engine(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Interface registrations for modules that already expose BehaviorModuleInterface
// ---------------------------------------------------------------------------

impl<T> GameModule for crate::contain_module_overrides::ActiveBehaviorModule<T>
where
    T: crate::modules::BehaviorModuleInterface + game_engine::common::system::Snapshotable + 'static,
{
    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        self.behavior_mut().get_die()
    }

    fn get_upgrade(&mut self) -> Option<&mut dyn UpgradeModuleInterface> {
        self.behavior_mut().get_upgrade()
    }

    fn get_special_power(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SpecialPowerModuleInterface> {
        self.behavior_mut().get_special_power()
    }
}

impl GameModule for crate::contain_module_overrides::MissingOwnerModule {}

impl GameModule for crate::object::behavior::fire_weapon_when_dead_behavior::FireWeaponWhenDeadBehaviorModule {
    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self.behavior_mut())
    }

    fn get_upgrade(&mut self) -> Option<&mut dyn UpgradeModuleInterface> {
        Some(self.behavior_mut())
    }
}

impl GameModule for crate::object::behavior::SlowDeathBehavior {
    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        crate::modules::BehaviorModuleInterface::get_die(self)
    }
}

// ---------------------------------------------------------------------------
// Gamelogic module registry
// ---------------------------------------------------------------------------

/// Constructor registered by gamelogic factories that produce modules with a
/// gamelogic interface surface. Mirrors the engine `NewModuleProc` shape so
/// the install loop can consult this registry first and never erase the
/// surface of a module gamelogic constructed itself.
pub type GameModuleCtor = fn(
    Arc<dyn game_engine::common::thing::module::Thing>,
    Arc<dyn game_engine::common::thing::module::ModuleData>,
) -> Box<dyn GameModule>;

type RegistryKey = String;

static GAME_MODULE_FACTORIES: LazyLock<RwLock<HashMap<RegistryKey, GameModuleCtor>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Same key derivation as the engine's `decorated_name_key`: type byte then
/// name, so `(name, ModuleType)` pairs cannot collide.
fn registry_key(
    name: &str,
    module_type: game_engine::common::thing::module::ModuleType,
) -> String {
    format!("{}{}", module_type as u8, name)
}

/// Register a gamelogic module constructor under `(name, module_type)`.
/// Called from factory registration at INI/system init time, before any
/// object construction consults the registry.
pub fn register_game_module_factory(
    name: &str,
    module_type: game_engine::common::thing::module::ModuleType,
    ctor: GameModuleCtor,
) {
    if let Ok(mut guard) = GAME_MODULE_FACTORIES.write() {
        guard.insert(registry_key(name, module_type), ctor);
    }
}

/// Take the gamelogic constructor for `(name, module_type)`, if registered.
pub fn take_game_module_factory(
    name: &str,
    module_type: game_engine::common::thing::module::ModuleType,
) -> Option<GameModuleCtor> {
    GAME_MODULE_FACTORIES
        .read()
        .ok()
        .and_then(|guard| guard.get(&registry_key(name, module_type)).copied())
}

// ---------------------------------------------------------------------------
// Object-aware dispatch (C++ Object::onDie / applyUpgradeMask walk m_behaviors)
// ---------------------------------------------------------------------------

impl ModuleEntry {
    pub(crate) fn with_slot<R>(&self, f: impl FnOnce(&mut ModuleSlot) -> R) -> R {
        let mut guard = self.module.lock().expect("behavior module lock poisoned");
        f(&mut *guard)
    }
}

/// C++ `Object::onDie` die-module walk for one entry. The dying object is
/// passed explicitly: it is already owned (or checked out) by the caller, and
/// a same-id registry re-entry from inside a module would silently miss.
pub(crate) fn dispatch_on_die(entry: &ModuleEntry, object: &mut Object, damage: &DamageInfo) {
    entry.with_slot(|slot| {
        if let Some(die) = slot.as_game().and_then(|game| game.get_die()) {
            let _ = die.on_die_with_object(object, damage);
        } else if let Some(kind) = crate::object::module_die_kind(slot.as_module()) {
            let _ = kind.into_interface().on_die_with_object(object, damage);
        }
    });
}

/// One upgrade entry under an object the caller holds. Returns
/// `(matched_any, applied)`; `matched_any` mirrors the ladder's "an upgrade
/// module existed" signal used by the caller's fallback paths.
pub(crate) fn dispatch_apply_upgrade(
    entry: &ModuleEntry,
    object: &mut Object,
    mask: UpgradeMaskType,
) -> (bool, bool) {
    entry.with_slot(|slot| {
        if let Some(upgrade) = slot.as_game().and_then(|game| game.get_upgrade()) {
            let applied = if upgrade.can_upgrade(mask) {
                upgrade.apply_upgrade_with_object(object, mask)
            } else {
                false
            };
            (true, applied)
        } else if let Some(kind) = crate::object::module_upgrade_kind(slot.as_module()) {
            let upgrade = kind.into_interface();
            let applied = if upgrade.can_upgrade(mask) {
                upgrade.apply_upgrade_with_object(object, mask)
            } else {
                false
            };
            (true, applied)
        } else {
            (false, false)
        }
    })
}

/// `can_upgrade` probe for one entry; `None` when the entry has no upgrade
/// surface at all.
pub(crate) fn dispatch_can_upgrade(entry: &ModuleEntry, mask: UpgradeMaskType) -> Option<bool> {
    entry.with_slot(|slot| {
        if let Some(upgrade) = slot.as_game().and_then(|game| game.get_upgrade()) {
            Some(upgrade.can_upgrade(mask))
        } else {
            crate::object::module_upgrade_kind(slot.as_module())
                .map(|kind| kind.into_interface().can_upgrade(mask))
        }
    })
}

/// `remove_upgrade` for one entry; returns whether an upgrade surface existed.
pub(crate) fn dispatch_remove_upgrade(entry: &ModuleEntry, mask: UpgradeMaskType) -> bool {
    entry.with_slot(|slot| {
        if let Some(upgrade) = slot.as_game().and_then(|game| game.get_upgrade()) {
            upgrade.remove_upgrade(mask);
            true
        } else if let Some(kind) = crate::object::module_upgrade_kind(slot.as_module()) {
            kind.into_interface().remove_upgrade(mask);
            true
        } else {
            false
        }
    })
}
