//! Host CommandButtonHuntUpdate residual.
//!
//! C++: when a unit is ordered to "hunt" with a special command button, each
//! scan interval while idle it picks the nearest legal target and issues the
//! button at that object (`doCommandButtonAtObject`).
//!
//! Retail defaults: ScanRate **1s → 30f**, ScanRange **9999**.
//!
//! Live host arms all three C++ hunt families:
//! - Special power (capture / Lotus hack / snipe / TNT / flashbang-adjacent)
//! - Fire/switch weapon (lock slot + `aiHunt`)
//! - Enter modes (hijack / car-bomb / sabotage)

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const COMMAND_BUTTON_HUNT_LOGIC_FPS: f32 = 30.0;
/// Retail ScanRate default = 1 second → 30 frames.
pub const COMMAND_BUTTON_HUNT_SCAN_FRAMES: u32 = 30;
/// Retail ScanRange default.
pub const COMMAND_BUTTON_HUNT_SCAN_RANGE: f32 = 9999.0;
/// C++ `CMD_FROM_PLAYER` — player order ends CommandButtonHunt.
pub const HUNT_CMD_FROM_PLAYER: u32 = 0;
/// C++ `CMD_FROM_SCRIPT` — script order ends CommandButtonHunt.
pub const HUNT_CMD_FROM_SCRIPT: u32 = 1;
/// C++ `CMD_FROM_AI` — only this source keeps CommandButtonHunt armed.
pub const HUNT_CMD_FROM_AI: u32 = 2;

/// serde default: newly spawned units have no player/script order yet.
pub fn default_last_command_source() -> u32 {
    HUNT_CMD_FROM_AI
}

/// C++ `getLastCommandSource() == CMD_FROM_AI`.
pub fn hunt_last_command_is_from_ai(source: u32) -> bool {
    source == HUNT_CMD_FROM_AI
}

/// C++ PartitionFilterSameMapStatus: both on-map or both off-map.
pub fn hunt_same_map_status(hunter_off_map: bool, target_off_map: bool) -> bool {
    hunter_off_map == target_off_map
}

/// C++ PartitionFilterStealthedAndUndetected (undetected stealth is illegal).
pub fn hunt_stealthed_undetected(stealthed: bool, detected: bool) -> bool {
    stealthed && !detected
}

/// C++ TimedCharges / TankHunterTNT ViewObjectRange residual (Burton 100).
pub const HUNT_PLACE_EXPLOSIVE_VIEW_RANGE: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCommandButtonHuntMode {
    HijackVehicle,
    ConvertToCarBomb,
    SabotageBuilding,
    /// GUI_COMMAND_SPECIAL_POWER — capture / Lotus / snipe / TNT / hack.
    SpecialPower,
    /// GUI_COMMAND_FIRE_WEAPON / SWITCH_WEAPON — lock slot then hunt.
    FireWeapon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCommandButtonHuntData {
    pub mode: HostCommandButtonHuntMode,
    pub next_scan_frame: u32,
    pub active: bool,
    /// Scripted button name (Command_CaptureBuilding, flashbang, …).
    #[serde(default)]
    pub button_name: String,
    /// Weapon slot for FireWeapon (PRIMARY=0, SECONDARY=1, TERTIARY=2).
    #[serde(default)]
    pub weapon_slot: u8,
}

impl HostCommandButtonHuntData {
    pub fn new(mode: HostCommandButtonHuntMode, current_frame: u32) -> Self {
        Self {
            mode,
            next_scan_frame: current_frame,
            active: true,
            button_name: String::new(),
            weapon_slot: 0,
        }
    }

    pub fn with_button(mut self, button: &str) -> Self {
        self.button_name = button.to_string();
        self.weapon_slot = weapon_slot_from_button_name(button);
        self
    }

    pub fn clear(&mut self) {
        self.active = false;
    }

    pub fn due(&self, current_frame: u32) -> bool {
        if !self.active {
            return false;
        }
        // C++ huntWeapon returns UPDATE_SLEEP_NONE — wake every frame so a
        // temp lock released on clip auto-reload is re-armed next tick.
        if matches!(self.mode, HostCommandButtonHuntMode::FireWeapon) {
            return true;
        }
        current_frame >= self.next_scan_frame
    }

    pub fn schedule_next(&mut self, current_frame: u32) {
        if matches!(self.mode, HostCommandButtonHuntMode::FireWeapon) {
            self.next_scan_frame = current_frame;
            return;
        }
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

/// Map a TEAM_HUNT_WITH_COMMAND_BUTTON ability name to the live enter-hunt residual.
/// C++ `doTeamHuntWithCommandButton` arms CommandButtonHuntUpdate with this button.
pub fn hunt_mode_from_button_name(name: &str) -> Option<HostCommandButtonHuntMode> {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    if n.contains("carbomb") || n.contains("makecarbomb") {
        Some(HostCommandButtonHuntMode::ConvertToCarBomb)
    } else if n.contains("hijack") {
        Some(HostCommandButtonHuntMode::HijackVehicle)
    } else if n.contains("sabotage") {
        Some(HostCommandButtonHuntMode::SabotageBuilding)
    } else if n.contains("flashbang")
        || n.contains("flashbanggrenade")
        || n.contains("switchweapon")
        || n.contains("fireweapon")
        || n.contains("pathfinder")
    {
        Some(HostCommandButtonHuntMode::FireWeapon)
    } else if n.contains("capture")
        || n.contains("blacklotus")
        || n.contains("snipe")
        || n.contains("jarmenkell")
        || n.contains("tnt")
        || n.contains("tankhunter")
        || n.contains("stealcash")
        || n.contains("disablevehicle")
        || n.contains("hackbuilding")
        || n.contains("disablebuilding")
        || n.contains("booby")
        || n.contains("timeddemo")
        || n.contains("remotedemo")
        || n.contains("timedcharge")
        || n.contains("remotecharge")
        || n.contains("colonelburton")
        || n.contains("specialpower")
        || n.contains("specialability")
    {
        Some(HostCommandButtonHuntMode::SpecialPower)
    } else {
        None
    }
}

/// Fallback when the button name is missing: terrorist/hijacker/saboteur/heroes.
pub fn hunt_mode_from_template(name: &str) -> Option<HostCommandButtonHuntMode> {
    let n = name.to_ascii_lowercase();
    if n.contains("terrorist") {
        Some(HostCommandButtonHuntMode::ConvertToCarBomb)
    } else if n.contains("hijacker") {
        Some(HostCommandButtonHuntMode::HijackVehicle)
    } else if n.contains("saboteur") {
        Some(HostCommandButtonHuntMode::SabotageBuilding)
    } else if n.contains("pathfinder") {
        Some(HostCommandButtonHuntMode::FireWeapon)
    } else if n.contains("blacklotus")
        || n.contains("jarmenkell")
        || n.contains("tankhunter")
        || n.contains("ranger")
        || n.contains("redguard")
        || n.contains("minigunner")
        || n.contains("colonelburton")
        || n.contains("hacker")
        || n.contains("rebel")
    {
        Some(HostCommandButtonHuntMode::SpecialPower)
    } else {
        None
    }
}

pub fn weapon_slot_from_button_name(name: &str) -> u8 {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    if n.contains("tertiary") {
        2
    } else if n.contains("flashbang") || n.contains("secondary") || n.contains("grenade") {
        1
    } else {
        0
    }
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
        HostCommandButtonHuntMode::SpecialPower | HostCommandButtonHuntMode::FireWeapon => {
            !same_team
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
        HostCommandButtonHuntMode::SpecialPower | HostCommandButtonHuntMode::FireWeapon => true,
    }
}

/// C++ ActionManager enter-hunt gates (canHijack / canConvert / canSabotage).
/// Relationship and KindOf are precomputed so this module stays Team-free.
pub fn hunt_enter_action_ok(
    mode: HostCommandButtonHuntMode,
    relationship_enemies: bool,
    relationship_neutral: bool,
    is_vehicle: bool,
    is_structure: bool,
    is_aircraft: bool,
    is_drone: bool,
    hijack_rejected: bool,
    already_carbomb: bool,
) -> bool {
    match mode {
        HostCommandButtonHuntMode::HijackVehicle => {
            relationship_enemies && is_vehicle && !is_aircraft && !is_drone && !hijack_rejected
        }
        HostCommandButtonHuntMode::ConvertToCarBomb => {
            relationship_neutral && is_vehicle && !is_aircraft && !already_carbomb
        }
        HostCommandButtonHuntMode::SabotageBuilding => relationship_enemies && is_structure,
        HostCommandButtonHuntMode::SpecialPower | HostCommandButtonHuntMode::FireWeapon => false,
    }
}

/// Capture hunt: C++ skips same controlling player and ALLIES.
pub fn hunt_special_capture_skips(same_player: bool, relationship_allies: bool) -> bool {
    same_player || relationship_allies
}

/// TNT / timed / remote charges: skip if an owned mine is in ViewObjectRange.
pub fn hunt_special_is_place_explosive(button: &str) -> bool {
    let n: String = button
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    n.contains("tnt")
        || n.contains("tankhunter")
        || n.contains("timedcharge")
        || n.contains("timeddemo")
        || n.contains("remotecharge")
        || n.contains("remotedemo")
        || n.contains("burton")
}

/// C++ `curPriority - dist/AttackPriorityDistanceModifier`, min 1.
pub fn hunt_effective_priority(raw_priority: i32, dist: f32, distance_modifier: f32) -> i32 {
    if raw_priority == 0 {
        return 0;
    }
    let modifier = if distance_modifier > 0.0 {
        (dist / distance_modifier) as i32
    } else {
        0
    };
    (raw_priority - modifier).max(1)
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
        assert!(hunt_enter_action_ok(
            HostCommandButtonHuntMode::HijackVehicle,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false
        ));
        assert!(
            !hunt_enter_action_ok(
                HostCommandButtonHuntMode::HijackVehicle,
                false,
                false,
                true,
                false,
                false,
                false,
                false,
                false
            ),
            "2v2 ally is not Enemies"
        );
        assert!(
            !hunt_enter_action_ok(
                HostCommandButtonHuntMode::HijackVehicle,
                true,
                false,
                true,
                false,
                false,
                true,
                false,
                false
            ),
            "drone rejected"
        );
        assert!(hunt_enter_action_ok(
            HostCommandButtonHuntMode::ConvertToCarBomb,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
            false
        ));
        assert!(hunt_special_capture_skips(false, true));
        assert!(hunt_special_is_place_explosive(
            "Command_ChinaTankHunterTNT"
        ));
        assert_eq!(hunt_effective_priority(80, 100.0, 50.0), 78);
        assert!(hunt_last_command_is_from_ai(HUNT_CMD_FROM_AI));
        assert!(!hunt_last_command_is_from_ai(HUNT_CMD_FROM_PLAYER));
    }

    #[test]
    fn hunt_mode_arms_special_power_and_weapon_buttons() {
        assert_eq!(
            hunt_mode_from_button_name("Command_CaptureBuilding"),
            Some(HostCommandButtonHuntMode::SpecialPower)
        );
        assert_eq!(
            hunt_mode_from_button_name("Command_BlackLotusCaptureBuilding"),
            Some(HostCommandButtonHuntMode::SpecialPower)
        );
        assert_eq!(
            hunt_mode_from_button_name("Command_JarmenKellSnipeVehicle"),
            Some(HostCommandButtonHuntMode::SpecialPower)
        );
        assert_eq!(
            hunt_mode_from_button_name("Command_ChinaTankHunterTNT"),
            Some(HostCommandButtonHuntMode::SpecialPower)
        );
        assert_eq!(
            hunt_mode_from_button_name("Command_AmericaRangerFlashBangGrenade"),
            Some(HostCommandButtonHuntMode::FireWeapon)
        );
        assert_eq!(
            weapon_slot_from_button_name("Command_AmericaRangerFlashBangGrenade"),
            1
        );
        assert_eq!(
            hunt_mode_from_template("AmericaInfantryRanger"),
            Some(HostCommandButtonHuntMode::SpecialPower)
        );
    }

    #[test]
    fn fire_weapon_hunt_stays_due_every_frame() {
        let mut d = HostCommandButtonHuntData::new(HostCommandButtonHuntMode::FireWeapon, 10);
        assert!(d.due(10));
        d.schedule_next(10);
        assert!(
            d.due(11),
            "C++ huntWeapon UPDATE_SLEEP_NONE re-arms lock every frame"
        );
        assert!(d.due(39));
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
