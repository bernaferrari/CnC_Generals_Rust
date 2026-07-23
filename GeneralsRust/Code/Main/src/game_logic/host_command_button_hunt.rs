//! Host CommandButtonHuntUpdate residual.
//!
//! C++: when a unit is ordered to "hunt" with a special command button, each
//! scan interval while idle it picks the nearest legal target and issues the
//! button at that object (`doCommandButtonAtObject`).
//!
//! Retail defaults: ScanRate **1s → 30f**, ScanRange **9999**.
//!
//! Residual playability slice (enter modes used by GLA scripts / AI):
//! - Hijack vehicle (enemies, vehicles, not already hijacked)
//! - Convert to car bomb (neutrals, vehicles)
//! - Sabotage building (enemies, structures)
//!
//! Fail-closed: not full special-power template matrix / capture-building /
//! place-explosive priority / weapon-lock hunt / AttackPriorityInfo.

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const COMMAND_BUTTON_HUNT_LOGIC_FPS: f32 = 30.0;
/// Retail ScanRate default = 1 second → 30 frames.
pub const COMMAND_BUTTON_HUNT_SCAN_FRAMES: u32 = 30;
/// Retail ScanRange default.
pub const COMMAND_BUTTON_HUNT_SCAN_RANGE: f32 = 9999.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCommandButtonHuntMode {
    HijackVehicle,
    ConvertToCarBomb,
    SabotageBuilding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCommandButtonHuntData {
    pub mode: HostCommandButtonHuntMode,
    pub next_scan_frame: u32,
    pub active: bool,
}

impl HostCommandButtonHuntData {
    pub fn new(mode: HostCommandButtonHuntMode, current_frame: u32) -> Self {
        Self {
            mode,
            next_scan_frame: current_frame,
            active: true,
        }
    }

    pub fn clear(&mut self) {
        self.active = false;
    }

    pub fn due(&self, current_frame: u32) -> bool {
        self.active && current_frame >= self.next_scan_frame
    }

    pub fn schedule_next(&mut self, current_frame: u32) {
        self.next_scan_frame = current_frame.saturating_add(COMMAND_BUTTON_HUNT_SCAN_FRAMES);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCommandButtonHuntRegistry {
    pub hunts_started: u32,
    pub scans: u32,
    pub targets_issued: u32,
    pub cancelled: u32,
}

impl HostCommandButtonHuntRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_start(&mut self) {
        self.hunts_started = self.hunts_started.saturating_add(1);
    }
    pub fn record_scan(&mut self) {
        self.scans = self.scans.saturating_add(1);
    }
    pub fn record_target(&mut self) {
        self.targets_issued = self.targets_issued.saturating_add(1);
    }
    pub fn record_cancel(&mut self) {
        self.cancelled = self.cancelled.saturating_add(1);
    }
    pub fn honesty_hunt_ok(&self) -> bool {
        self.hunts_started > 0 && self.targets_issued > 0
    }
}

/// True when unit template typically carries CommandButtonHuntUpdate residual.
pub fn is_command_button_hunt_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("hijacker")
        || n.contains("terrorist")
        || n.contains("saboteur")
        || n.contains("blacklotus")
        || n.contains("jarmenkell")
        || n.contains("redguard")
        || n.contains("minigunner")
        || n.contains("ranger")
        || n.contains("pathfinder")
        || n.contains("tankhunter")
        || n.contains("troopcrawler")
}

/// C++ relationship filter residual for enter hunt modes.
///
/// `same_team` / `target_neutral` are precomputed by the host so this module
/// stays free of the Team enum.
pub fn hunt_allows_team(
    mode: HostCommandButtonHuntMode,
    same_team: bool,
    target_neutral: bool,
) -> bool {
    match mode {
        HostCommandButtonHuntMode::ConvertToCarBomb => target_neutral,
        HostCommandButtonHuntMode::HijackVehicle | HostCommandButtonHuntMode::SabotageBuilding => {
            !same_team && !target_neutral
        }
    }
}

/// Kind residual: vehicle for hijack/car-bomb, structure for sabotage.
pub fn hunt_allows_kind(
    mode: HostCommandButtonHuntMode,
    is_vehicle: bool,
    is_structure: bool,
    is_aircraft: bool,
) -> bool {
    match mode {
        HostCommandButtonHuntMode::HijackVehicle | HostCommandButtonHuntMode::ConvertToCarBomb => {
            is_vehicle && !is_aircraft
        }
        HostCommandButtonHuntMode::SabotageBuilding => is_structure,
    }
}

pub fn honesty_command_button_hunt_residual_ok() -> bool {
    COMMAND_BUTTON_HUNT_SCAN_FRAMES == 30
        && (COMMAND_BUTTON_HUNT_SCAN_RANGE - 9999.0).abs() < 0.1
        && is_command_button_hunt_template("GLAInfantryHijacker")
        && is_command_button_hunt_template("GLAInfantryTerrorist")
        && !is_command_button_hunt_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_peels_and_filters() {
        assert!(honesty_command_button_hunt_residual_ok());
        assert!(hunt_allows_team(
            HostCommandButtonHuntMode::HijackVehicle,
            false,
            false
        ));
        assert!(!hunt_allows_team(
            HostCommandButtonHuntMode::HijackVehicle,
            true,
            false
        ));
        assert!(hunt_allows_team(
            HostCommandButtonHuntMode::ConvertToCarBomb,
            false,
            true
        ));
        assert!(hunt_allows_kind(
            HostCommandButtonHuntMode::SabotageBuilding,
            false,
            true,
            false
        ));
        assert!(!hunt_allows_kind(
            HostCommandButtonHuntMode::HijackVehicle,
            false,
            true,
            false
        ));
    }

    #[test]
    fn schedule_scan_interval() {
        let mut d = HostCommandButtonHuntData::new(HostCommandButtonHuntMode::HijackVehicle, 10);
        assert!(d.due(10));
        d.schedule_next(10);
        assert!(!d.due(39));
        assert!(d.due(40));
    }
}
