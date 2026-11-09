//! Borrowed CommandButton / CommandSet catalog of one game instance.
//!
//! C++ keeps the parsed buttons and sets on the engine's own ControlBar
//! (`ControlBar::m_commandButtons` / `m_commandSets`, ControlBar.cpp:1060-1120,
//! `findCommandButton` / `findCommandSet` ControlBar.cpp:1690-1730). Map INI
//! overrides change them per map, so each game instance owns its catalog.
//! Callers receive this owner explicitly from the instance they act for; there
//! is no ambient lookup.

use super::ini_command_button::{CommandButton, ControlBar};
use super::ini_command_set::{CommandSet, CommandSetManager};
use crate::common::thing::receiving_module_content::ReceivingActionDefinitions;
use parking_lot::RwLock;
use std::sync::Arc;

/// One instance's CommandButton / CommandSet catalog.
#[derive(Clone)]
pub enum CommandDefinitions {
    /// Immutable admitted catalog of the receiving session's module content.
    Admitted(Arc<ReceivingActionDefinitions>),
    /// INI catalog owned by the instance's `CommonServices`.
    Loaded {
        buttons: Arc<RwLock<ControlBar>>,
        sets: Arc<RwLock<CommandSetManager>>,
    },
}

impl CommandDefinitions {
    /// The INI catalog owned by exactly this `CommonServices`.
    pub fn from_common(common: &crate::common::common_services::CommonServices) -> Self {
        Self::Loaded {
            buttons: Arc::clone(&common.ini.control_bar),
            sets: Arc::clone(&common.ini.command_set),
        }
    }

    pub fn with_buttons<R>(&self, f: impl FnOnce(&ControlBar) -> R) -> R {
        match self {
            Self::Admitted(defs) => f(&defs.buttons),
            Self::Loaded { buttons, .. } => f(&buttons.read()),
        }
    }

    pub fn with_sets<R>(&self, f: impl FnOnce(&CommandSetManager) -> R) -> R {
        match self {
            Self::Admitted(defs) => f(&defs.sets),
            Self::Loaded { sets, .. } => f(&sets.read()),
        }
    }

    /// Borrow the CommandButton catalog for a bounded multi-lookup.
    pub fn buttons(&self) -> ButtonsRef<'_> {
        match self {
            Self::Admitted(defs) => ButtonsRef::Admitted(&defs.buttons),
            Self::Loaded { buttons, .. } => ButtonsRef::Loaded(buttons.read()),
        }
    }

    /// C++ `ControlBar::findCommandButton` (final override entry).
    pub fn find_command_button(&self, name: &str) -> Option<CommandButton> {
        self.with_buttons(|bar| bar.find_command_button_resolved(name).cloned())
    }

    /// C++ `ControlBar::findCommandSet` (final override entry).
    pub fn find_command_set(&self, name: &str) -> Option<CommandSet> {
        self.with_sets(|sets| sets.find_command_set_resolved(name).cloned())
    }

    /// Exact lookup, then the case-insensitive NameKey match C++ gets from
    /// `NAMEKEY` (NameKeyGenerator is case-insensitive).
    pub fn find_command_set_nocase(&self, name: &str) -> Option<CommandSet> {
        self.with_sets(|sets| {
            sets.find_command_set_resolved(name)
                .or_else(|| {
                    sets.iter_resolved_sets()
                        .into_iter()
                        .find(|(set_name, _)| set_name.eq_ignore_ascii_case(name))
                        .map(|(_, set)| set)
                })
                .cloned()
        })
    }

    /// Same catalog identity (both views share one owner).
    pub fn same_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Admitted(a), Self::Admitted(b)) => Arc::ptr_eq(a, b),
            (
                Self::Loaded { buttons: a, sets: c },
                Self::Loaded { buttons: b, sets: d },
            ) => Arc::ptr_eq(a, b) && Arc::ptr_eq(c, d),
            _ => false,
        }
    }
}

/// Read view of one instance's CommandButton catalog.
pub enum ButtonsRef<'a> {
    Admitted(&'a ControlBar),
    Loaded(parking_lot::RwLockReadGuard<'a, ControlBar>),
}

impl std::ops::Deref for ButtonsRef<'_> {
    type Target = ControlBar;
    fn deref(&self) -> &ControlBar {
        match self {
            Self::Admitted(bar) => bar,
            Self::Loaded(guard) => guard,
        }
    }
}

impl std::fmt::Debug for CommandDefinitions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admitted(_) => f.write_str("CommandDefinitions::Admitted"),
            Self::Loaded { .. } => f.write_str("CommandDefinitions::Loaded"),
        }
    }
}

impl PartialEq for CommandDefinitions {
    fn eq(&self, other: &Self) -> bool {
        self.same_owner(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::common_services::CommonServices;

    #[test]
    fn two_instances_resolve_only_their_own_command_sets() {
        let a = CommonServices::new();
        let b = CommonServices::new();
        for (services, button) in [(&a, "Command_A"), (&b, "Command_B")] {
            services.ini.control_bar.write().new_command_button(button.to_string());
            let mut sets = services.ini.command_set.write();
            let set = sets.new_command_set("SharedSetName".to_string());
            set.set_button_at_position(0, button.to_string()).unwrap();
        }
        let da = CommandDefinitions::from_common(&a);
        let db = CommandDefinitions::from_common(&b);
        assert_eq!(
            da.find_command_set("SharedSetName").unwrap().get_button_at_position(0).map(String::as_str),
            Some("Command_A")
        );
        assert_eq!(
            db.find_command_set_nocase("sharedsetname").unwrap().get_button_at_position(0).map(String::as_str),
            Some("Command_B")
        );
        assert!(da.find_command_button("Command_B").is_none());
        assert!(db.find_command_button("Command_A").is_none());
        assert!(!da.same_owner(&db));
        assert!(da.same_owner(&CommandDefinitions::from_common(&a)));
    }
}
