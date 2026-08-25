//! Host GameLogic tests — `production_and_mobs`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

// -----------------------------------------------------------------------
// China Overlord / Helix / Emperor portable gattling + propaganda residual
// Fail-closed: not full OverlordContain portable-structure spawn / W3D draw.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// China Nuke Cannon primary residual (area + medium radiation)
// Fail-closed: not full projectile lob / DeployStyleAIUpdate.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// GLA Battle Bus residual (capacity 8 + passenger fire + armed-riders weapon set)
// Fail-closed: not SlowDeath undeath SECOND_LIFE / multi-door exit matrix.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// GLA Tunnel Network residual (shared MaxTunnelCapacity=10, cross-tunnel exit)
// Fail-closed: not GuardTunnelNetwork AI / CaveSystem / TimeForFullHeal.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// AirF Combat Chinook residual (capacity 8 + passenger fire + armed-riders)
// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// China Listening Outpost residual (detect 300 + transport 2 + TankHunter payload)
// Fail-closed: not IR FX / multi-door / RIDERS_ATTACKING uncloak matrix.
// -----------------------------------------------------------------------

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod mobs_transports_and_flights;
mod ocl_and_special_power;
mod production_and_hunt;
