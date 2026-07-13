//! Host USA Pilot residual (eject recrew of unmanned vehicles).
//!
//! Residual slice (playability):
//! - `AmericaInfantryPilot` / AirF_ / CINE_ / TestPilot enter unmanned ground
//!   vehicles (DISABLED_UNMANNED residual from Jarmen Kell snipe / neutron) →
//!   recrew: clear unmanned, transfer team to pilot's team, transfer pilot
//!   veterancy (retail `VeterancyCrateCollide` IsPilot + AddsOwnerVeterancy),
//!   consume pilot.
//! - Pilots spawn residual at least VETERAN (VeterancyGainCreate StartingLevel).
//!
//! Fail-closed honesty:
//! - Not full EjectPilotDie air/ground OCL parachute spawn matrix
//! - Not full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
//! - Not full AutoFindHealingUpdate hospital path residual
//! - Not network recrew / pilot-eject replication (network deferred)

use super::VeterancyLevel;
use serde::{Deserialize, Serialize};

/// Retail pilot template family residual.
pub const PILOT_RECREW_AUDIO: &str = "PilotEnterVehicle";

/// Whether template is a residual USA Pilot infantry.
///
/// Fail-closed: name residual. Excludes weapons / science / debris / pathfinder.
pub fn is_pilot_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("pathfinder")
        || n.contains("ranger")
        || n.contains("colonel")
        || n.contains("burton")
        || n.contains("commandset")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testpilot" || n == "usa_pilot" || n == "americainfantrypilot" {
        return true;
    }
    // AmericaInfantryPilot / AirF_AmericaInfantryPilot / CINE_AmericaInfantryPilot
    n.contains("infantrypilot") || n.ends_with("pilot") && n.contains("america")
}

/// Residual pilot starting veterancy (VeterancyGainCreate StartingLevel = VETERAN).
pub fn pilot_default_veterancy() -> VeterancyLevel {
    VeterancyLevel::Veteran
}

/// Rank for residual veterancy transfer (higher wins).
pub fn veterancy_rank(level: VeterancyLevel) -> u8 {
    match level {
        VeterancyLevel::Rookie => 0,
        VeterancyLevel::Veteran => 1,
        VeterancyLevel::Elite => 2,
        VeterancyLevel::Heroic => 3,
    }
}

/// Whether residual vehicle can be recrewed by a pilot.
///
/// Fail-closed: live ground vehicle, unmanned, not under construction, not aircraft.
pub fn is_recrewable_unmanned_vehicle(
    is_alive: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_unmanned: bool,
    under_construction: bool,
    is_dozer: bool,
) -> bool {
    // Retail VeterancyCrateCollide ForbiddenKindOf = DOZER residual.
    is_alive && is_vehicle && !is_aircraft && is_unmanned && !under_construction && !is_dozer
}

/// Whether an enter command should take the pilot recrew residual path.
pub fn should_recrew_on_enter(is_pilot: bool, vehicle_recrewable: bool) -> bool {
    is_pilot && vehicle_recrewable
}

/// Merged veterancy after recrew: max(vehicle, pilot).
pub fn merged_recrew_veterancy(
    vehicle_level: VeterancyLevel,
    pilot_level: VeterancyLevel,
) -> VeterancyLevel {
    if veterancy_rank(pilot_level) >= veterancy_rank(vehicle_level) {
        pilot_level
    } else {
        vehicle_level
    }
}

/// Host residual honesty counters for USA Pilot recrew.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostUsaPilotRegistry {
    /// Successful unmanned-vehicle recrews (pilot consumed).
    pub recrews: u32,
    /// Veterancy promotions applied onto recrewed vehicles.
    pub veterancy_transfers: u32,
}

impl HostUsaPilotRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_recrew(&mut self, transferred_veterancy: bool) {
        self.recrews = self.recrews.saturating_add(1);
        if transferred_veterancy {
            self.veterancy_transfers = self.veterancy_transfers.saturating_add(1);
        }
    }

    /// Residual honesty: at least one recrew completed.
    pub fn honesty_recrew_ok(&self) -> bool {
        self.recrews > 0
    }

    /// Residual honesty: recrew path with veterancy transfer observed.
    pub fn honesty_veterancy_transfer_ok(&self) -> bool {
        self.recrews > 0 && self.veterancy_transfers > 0
    }

    /// Combined pilot residual honesty.
    pub fn honesty_pilot_ok(&self) -> bool {
        self.honesty_recrew_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_name_matrix() {
        assert!(is_pilot_template("AmericaInfantryPilot"));
        assert!(is_pilot_template("AirF_AmericaInfantryPilot"));
        assert!(is_pilot_template("CINE_AmericaInfantryPilot"));
        assert!(is_pilot_template("TestPilot"));
        assert!(is_pilot_template("USA_Pilot"));
        assert!(!is_pilot_template("AmericaInfantryRanger"));
        assert!(!is_pilot_template("AmericaInfantryPathfinder"));
        assert!(!is_pilot_template("AmericaInfantryColonelBurton"));
        assert!(!is_pilot_template("Upgrade_AmericaPilot"));
        assert!(!is_pilot_template("GLAInfantryWorker"));
    }

    #[test]
    fn recrewable_gate() {
        assert!(is_recrewable_unmanned_vehicle(
            true, true, false, true, false, false
        ));
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, false, false, false
        )); // manned
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, true, true, false, false
        )); // aircraft
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, true, false, true
        )); // dozer forbidden
        assert!(!is_recrewable_unmanned_vehicle(
            false, true, false, true, false, false
        )); // dead
    }

    #[test]
    fn veterancy_merge() {
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Rookie, VeterancyLevel::Veteran),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Heroic, VeterancyLevel::Veteran),
            VeterancyLevel::Heroic
        );
        assert_eq!(pilot_default_veterancy(), VeterancyLevel::Veteran);
    }

    #[test]
    fn honesty_flags() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_pilot_ok());
        reg.record_recrew(true);
        assert!(reg.honesty_recrew_ok());
        assert!(reg.honesty_veterancy_transfer_ok());
        assert!(reg.honesty_pilot_ok());
        assert_eq!(reg.recrews, 1);
        assert_eq!(reg.veterancy_transfers, 1);
    }
}
