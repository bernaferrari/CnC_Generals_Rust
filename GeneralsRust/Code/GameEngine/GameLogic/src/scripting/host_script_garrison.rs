//! Borrowed receiving-world commands for the enter / garrison / exit actions.
//!
//! C++ `ScriptActions::doNamedEnterNamed` / `doTeamEnterNamed` /
//! `doNamedExitAll` / `doTeamExitAll` / `doTeamGarrisonSpecificBuilding` /
//! `doExitSpecificBuilding` / `doTeamGarrisonNearestBuilding` /
//! `doTeamExitAllBuildings` / `doUnitGarrisonSpecificBuilding` /
//! `doUnitGarrisonNearestBuilding` / `doUnitExitBuilding` /
//! `doPlayerGarrisonAllBuildings` / `doPlayerExitAllBuildings`.

pub use generals_sim::scripting::world::HostScriptGarrisonEnterExitRequest;
