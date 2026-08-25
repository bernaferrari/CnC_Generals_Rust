//! Host GameLogic tests — `combat_particles_and_economy`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

// -----------------------------------------------------------------------
// Transport residual (infantry enter vehicle capacity; unload all; evacuate)
// Fail-closed: not multi-door / Chinook air-transport path parity.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// China Overlord BattleBunker residual
// Fail-closed: not full OverlordContain redirect / portable-structure spawn /
// GattlingCannon / PropagandaTower payload matrix.
// -----------------------------------------------------------------------

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod capture_and_containment;
mod supply_and_superweapons;
