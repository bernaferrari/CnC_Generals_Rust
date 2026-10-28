//! Borrowed receiving-world commands for the enter / garrison / exit actions.
//!
//! C++ `ScriptActions::doNamedEnterNamed` / `doTeamEnterNamed` /
//! `doNamedExitAll` / `doTeamExitAll` / `doTeamGarrisonSpecificBuilding` /
//! `doExitSpecificBuilding` / `doTeamGarrisonNearestBuilding` /
//! `doTeamExitAllBuildings` / `doUnitGarrisonSpecificBuilding` /
//! `doUnitGarrisonNearestBuilding` / `doUnitExitBuilding` /
//! `doPlayerGarrisonAllBuildings` / `doPlayerExitAllBuildings`.

/// Synchronous commands for the receiving ScriptExecutionDriver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptGarrisonEnterExitRequest {
    TeamEnter { team: String, dest: String },
    TeamGarrisonSpecific { team: String, building: String },
    ExitSpecificBuilding { building: String },
    TeamGarrisonNearest { team: String },
    TeamExitAllBuildings { team: String },
    NamedGarrisonSpecific { unit: String, building: String },
    NamedGarrisonNearest { unit: String },
    NamedExitBuilding { unit: String },
    PlayerGarrisonAll { player: String },
    PlayerExitAll { player: String },
}
