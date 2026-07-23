//! Host KeepObjectDie residual (leave rubble in the world).
//!
//! C++: `KeepObjectDie::onDie` is intentionally empty. Its presence on a template
//! means the object has a die module, so the engine does **not** default to
//! `DestroyDie` (which calls `destroyObject` and removes it). Civilian /
//! tech buildings use `ModuleTag_IWantRubble` so rubble stays after death.
//!
//! Residual playability slice:
//! - Templates matching KeepObjectDie peels do not enqueue destroy/remove
//! - Death marks effectively-dead rubble (HP 0, rubble body state, unselectable)
//! - Object remains in the world store for presentation (RUBBLE mesh)
//!
//! Fail-closed: not full DieModule masks, garrison-contain built-in die, or
//! pathfind rubble cell registration.

use serde::{Deserialize, Serialize};

/// C++ KeepObjectDie residual runtime flag on Object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostKeepObjectDieData {
    /// True once onDie residual has converted this object to lasting rubble.
    pub is_rubble: bool,
    /// Frame when rubble conversion happened (0 = never).
    pub rubble_frame: u32,
}

impl HostKeepObjectDieData {
    pub fn mark_rubble(&mut self, frame: u32) {
        self.is_rubble = true;
        self.rubble_frame = frame;
    }
}

/// True if template uses KeepObjectDie / IWantRubble residual peels.
pub fn wants_keep_object_die(template_name: &str, is_structure: bool) -> bool {
    if !is_structure {
        // Cine / unit peels that still leave a husk.
        let n = template_name.to_ascii_lowercase();
        return n.contains("cine") && (n.contains("rubble") || n.contains("wreck"));
    }
    let n = template_name.to_ascii_lowercase();
    // Tech buildings + civilian buildings + dam body.
    if n.contains("tech")
        || n.contains("civilian")
        || n.contains("civ")
        || n.contains("barn")
        || n.contains("house")
        || n.contains("shack")
        || n.contains("store")
        || n.contains("church")
        || n.contains("oil")
        || n.contains("hospital")
        || n.contains("artillery")
        || n.contains("convention")
        || n.contains("dam")
        || n.contains("rubble")
        || n.contains("prop")
        || n.contains("fence")
        || n.contains("wall")
        || n.contains("tent")
        || n.contains("hut")
        || n.contains("village")
        || n.contains("city")
        || n.contains("building")
    {
        // Military faction bases still destroy via other modules / topple.
        if n.starts_with("america")
            || n.starts_with("china")
            || n.starts_with("gla")
            || n.contains("commandcenter")
            || n.contains("warfactory")
            || n.contains("barracks")
            || n.contains("airfield")
            || n.contains("supply")
            || n.contains("powerplant")
            || n.contains("reactor")
            || n.contains("tunnel")
            || n.contains("stinger")
            || n.contains("palace")
            || n.contains("scudstorm")
            || n.contains("nuclearmissile")
            || n.contains("particlecannon")
        {
            return false;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tech_and_civilian_want_keep() {
        assert!(wants_keep_object_die("TechHospital", true));
        assert!(wants_keep_object_die("CivilianBuilding01", true));
        assert!(wants_keep_object_die("Dam", true));
    }

    #[test]
    fn faction_structures_do_not_keep_by_default() {
        assert!(!wants_keep_object_die("AmericaCommandCenter", true));
        assert!(!wants_keep_object_die("GLABarracks", true));
        assert!(!wants_keep_object_die("ChinaWarFactory", true));
    }
}
