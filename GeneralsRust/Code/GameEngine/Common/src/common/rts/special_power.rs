//! Special Power System - Full implementation
//!
//! Manages special powers/superweapons in the game, including:
//! - Power activation and cooldowns
//! - Shared/synced powers across command centers
//! - Targeting and execution
//! - Science prerequisites
//!
//! Reference: /GeneralsMD/Code/GameEngine/Source/Common/RTS/SpecialPower.cpp
//! Reference: /GeneralsMD/Code/GameEngine/Include/Common/SpecialPower.h

use std::collections::HashMap;

/// Special power types - matches C++ SpecialPowerType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SpecialPowerType {
    Invalid = 0,

    // Superweapons
    DaisyCutter,
    ParadropAmerica,
    CarpetBomb,
    ClusterMines,
    EmpPulse,
    NapalmStrike,
    CashHack,
    NeutronMissile,
    SpySatellite,
    Defector,
    TerrorCell,
    Ambush,
    BlackMarketNuke,
    AnthraxBomb,
    ScudStorm,
    Demoralize,
    CrateDrop,
    A10ThunderboltStrike,
    DetonateDirtyNuke,
    ArtilleryBarrage,

    // Special abilities
    MissileDefenderLaserGuidedMissiles,
    RemoteCharges,
    TimedCharges,
    HelixNapalmBomb,
    HackerDisableBuilding,
    TankHunterTntAttack,
    BlackLotusCaptureBuilding,
    BlackLotusDisableVehicleHack,
    BlackLotusStealCashHack,
    InfantryCaptureBuilding,
    RadarVanScan,
    SpyDrone,
    DisguiseAsVehicle,
    BoobyTrap,
    RepairVehicles,
    ParticleUplinkCannon,
    CashBounty,
    ChangeBattlePlans,
    CiaIntelligence,
    CleanupArea,
    LaunchBaikonurRocket,
    SpectreGunship,
    GpsScrambler,
    Frenzy,
    SneakAttack,

    // Faction variants
    ChinaCarpetBomb,
    EarlyChinaCarpetBomb,
    LeafletDrop,
    EarlyLeafletDrop,
    EarlyFrenzy,
    CommunicationsDownload,
    EarlyRepairVehicles,
    TankParadrop,
    SupwParticleUplinkCannon,
    AirfDaisyCutter,
    NukeClusterMines,
    NukeNeutronMissile,
    AirfA10ThunderboltStrike,
    AirfSpectreGunship,
    InfaParadropAmerica,
    SlthGpsScrambler,
    AirfCarpetBomb,
    SuprCruiseMissile,
    LazrParticleUplinkCannon,
    SupwNeutronMissile,
    BattleshipBombardment,
}

impl Default for SpecialPowerType {
    fn default() -> Self {
        Self::Invalid
    }
}

/// Special power template - defines a type of special power
/// Reference: C++ SpecialPowerTemplate class
#[derive(Debug, Clone)]
pub struct SpecialPowerTemplate {
    pub name: String,
    pub id: u32,
    pub power_type: SpecialPowerType,

    // Timing - matches C++ m_reloadTime (in frames)
    pub reload_time: u32,

    // Science requirements
    pub required_science: Option<String>,

    // Audio
    pub initiate_sound: String,
    pub initiate_at_location_sound: String,

    // Flags - matches C++ boolean fields
    pub public_timer: bool,
    pub shared_n_sync: bool, // Shared between command centers
    pub shortcut_power: bool,

    // Detection and viewing
    pub detection_time: u32, // Frames for infiltration powers
    pub view_object_duration: u32,
    pub view_object_range: f32,
    pub radius_cursor_radius: f32,

    // Cost
    pub cost: i32,
}

impl SpecialPowerTemplate {
    /// Create a new special power template
    /// Reference: C++ SpecialPowerTemplate::SpecialPowerTemplate()
    pub fn new(name: String, id: u32) -> Self {
        // Default defection detection protection time limit
        // Reference: C++ DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT
        const DEFAULT_DETECTION_TIME: u32 = 30 * 10; // 10 seconds at 30 FPS

        Self {
            name,
            id,
            power_type: SpecialPowerType::Invalid,
            reload_time: 0,
            required_science: None,
            initiate_sound: String::new(),
            initiate_at_location_sound: String::new(),
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            detection_time: DEFAULT_DETECTION_TIME,
            view_object_duration: 0,
            view_object_range: 0.0,
            radius_cursor_radius: 0.0,
            cost: 0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_special_power_type(&self) -> SpecialPowerType {
        self.power_type
    }

    pub fn get_reload_time(&self) -> u32 {
        self.reload_time
    }

    pub fn has_public_timer(&self) -> bool {
        self.public_timer
    }

    pub fn is_shared_n_sync(&self) -> bool {
        self.shared_n_sync
    }

    pub fn is_shortcut_power(&self) -> bool {
        self.shortcut_power
    }

    pub fn get_detection_time(&self) -> u32 {
        self.detection_time
    }
}

/// Special power store - manages all available special powers
/// Reference: C++ SpecialPowerStore class
#[derive(Debug)]
pub struct SpecialPowerStore {
    templates: Vec<SpecialPowerTemplate>,
    next_special_power_id: u32,
}

impl SpecialPowerStore {
    /// Create a new special power store
    /// Reference: C++ SpecialPowerStore::SpecialPowerStore()
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
            next_special_power_id: 0,
        }
    }

    /// Find a special power template by name
    /// Reference: C++ SpecialPowerStore::findSpecialPowerTemplatePrivate()
    pub fn find_template(&self, name: &str) -> Option<&SpecialPowerTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    /// Find a special power template by ID
    /// Reference: C++ SpecialPowerStore::findSpecialPowerTemplateByID()
    pub fn find_template_by_id(&self, id: u32) -> Option<&SpecialPowerTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// Get a special power template by index (for WorldBuilder)
    /// Reference: C++ SpecialPowerStore::getSpecialPowerTemplateByIndex()
    pub fn get_template_by_index(&self, index: usize) -> Option<&SpecialPowerTemplate> {
        self.templates.get(index)
    }

    /// Get the number of special powers
    /// Reference: C++ SpecialPowerStore::getNumSpecialPowers()
    pub fn get_num_special_powers(&self) -> usize {
        self.templates.len()
    }

    /// Add a new template
    pub fn add_template(&mut self, template: SpecialPowerTemplate) {
        self.templates.push(template);
    }

    /// Reset the store
    /// Reference: C++ SpecialPowerStore::reset()
    pub fn reset(&mut self) {
        self.templates.clear();
        self.next_special_power_id = 0;
    }
}

impl Default for SpecialPowerStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Special power module - runtime instance of a special power
/// Reference: C++ SpecialPowerModule class
#[derive(Debug, Clone)]
pub struct SpecialPowerModule {
    /// Template this module uses
    template: SpecialPowerTemplate,

    /// Frame when this power becomes available
    /// Reference: C++ m_availableOnFrame
    available_on_frame: u32,

    /// Pause reference count
    /// Reference: C++ m_pausedCount
    paused_count: i32,

    /// Frame when paused
    /// Reference: C++ m_pausedOnFrame
    paused_on_frame: u32,

    /// Percent ready when paused
    /// Reference: C++ m_pausedPercent
    paused_percent: f32,

    /// Update module starts attack flag
    update_module_starts_attack: bool,

    /// Starts paused flag
    starts_paused: bool,

    /// Script only flag
    scripted_special_power_only: bool,
}

impl SpecialPowerModule {
    /// Create a new special power module
    /// Reference: C++ SpecialPowerModule::SpecialPowerModule()
    pub fn new(template: SpecialPowerTemplate) -> Self {
        Self {
            template,
            available_on_frame: 0,
            paused_count: 0,
            paused_on_frame: 0,
            paused_percent: 0.0,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        }
    }

    /// Check if this module is for a specific power
    /// Reference: C++ SpecialPowerModule::isModuleForPower()
    pub fn is_module_for_power(&self, template: &SpecialPowerTemplate) -> bool {
        self.template.id == template.id
    }

    /// Check if the power is ready
    /// Reference: C++ SpecialPowerModule::isReady()
    pub fn is_ready(&self, current_frame: u32) -> bool {
        self.paused_count == 0 && current_frame >= self.available_on_frame
    }

    /// Get the percentage ready (0.0 to 1.0)
    /// Reference: C++ SpecialPowerModule::getPercentReady()
    pub fn get_percent_ready(&self, current_frame: u32) -> f32 {
        // Don't consider it ready if paused
        if self.paused_count > 0 && self.paused_percent == 1.0 {
            return 0.99999;
        }

        // Easy case - is ready
        if self.is_ready(current_frame) {
            return 1.0;
        }

        if self.paused_count > 0 {
            return self.paused_percent;
        }

        // Sanity check
        if self.template.reload_time == 0 {
            return 0.0;
        }

        // Calculate the percent
        let ready_frame = self.available_on_frame;
        if ready_frame <= current_frame {
            return 1.0;
        }

        let percent =
            1.0 - ((ready_frame - current_frame) as f32 / self.template.reload_time as f32);
        percent.max(0.0).min(1.0)
    }

    /// Get the ready frame
    /// Reference: C++ SpecialPowerModule::getReadyFrame()
    pub fn get_ready_frame(&self) -> u32 {
        self.available_on_frame
    }

    /// Set the ready frame
    /// Reference: C++ SpecialPowerModule::setReadyFrame()
    pub fn set_ready_frame(&mut self, frame: u32, current_frame: u32) {
        self.available_on_frame = frame;
        // Update paused frame if changed
        self.paused_on_frame = current_frame;
    }

    /// Start power recharge
    /// Reference: C++ SpecialPowerModule::startPowerRecharge()
    pub fn start_power_recharge(&mut self, current_frame: u32) {
        // Set the frame we will be 100% available on
        self.available_on_frame = current_frame + self.template.reload_time;
    }

    /// Pause or unpause the countdown
    /// Reference: C++ SpecialPowerModule::pauseCountdown()
    pub fn pause_countdown(&mut self, pause: bool, current_frame: u32) {
        if pause {
            if self.paused_count == 0 {
                // First pause - save current state
                self.paused_on_frame = current_frame;
                self.paused_percent = self.get_percent_ready(current_frame);
            }
            self.paused_count += 1;
        } else {
            self.paused_count -= 1;
            if self.paused_count == 0 {
                // Last unpause - restore state
                let elapsed = current_frame - self.paused_on_frame;
                self.available_on_frame += elapsed;
            }
        }
    }

    /// Check if this is a script-only power
    /// Reference: C++ SpecialPowerModule::isScriptOnly()
    pub fn is_script_only(&self) -> bool {
        self.scripted_special_power_only
    }

    /// Get the power name
    /// Reference: C++ SpecialPowerModule::getPowerName()
    pub fn get_power_name(&self) -> &str {
        &self.template.name
    }

    /// Get the template
    pub fn get_template(&self) -> &SpecialPowerTemplate {
        &self.template
    }
}

/// Player power state - tracks power availability for a player
/// Reference: C++ Player class special power methods
#[derive(Debug, Clone)]
pub struct PlayerPowerState {
    /// Shared power ready frames - for powers with shared_n_sync=true
    shared_power_frames: HashMap<u32, u32>,
}

impl PlayerPowerState {
    pub fn new() -> Self {
        Self {
            shared_power_frames: HashMap::new(),
        }
    }

    /// Get or start the ready frame for a shared power
    /// Reference: C++ Player::getOrStartSpecialPowerReadyFrame()
    pub fn get_or_start_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        current_frame: u32,
    ) -> u32 {
        *self
            .shared_power_frames
            .entry(template.id)
            .or_insert_with(|| current_frame + template.reload_time)
    }

    /// Reset or start the ready frame for a shared power
    /// Reference: C++ Player::resetOrStartSpecialPowerReadyFrame()
    pub fn reset_or_start_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        current_frame: u32,
    ) {
        self.shared_power_frames
            .insert(template.id, current_frame + template.reload_time);
    }

    /// Express the ready frame for a shared power
    /// Reference: C++ Player::expressSpecialPowerReadyFrame()
    pub fn express_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        frame: u32,
    ) {
        self.shared_power_frames.insert(template.id, frame);
    }
}

impl Default for PlayerPowerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Special power execution parameters
#[derive(Debug, Clone)]
pub struct SpecialPowerExecution {
    pub power_id: u32,
    pub target_position: Option<(f32, f32, f32)>,
    pub target_object_id: Option<u32>,
    pub angle: f32,
    pub command_options: u32,
}

impl SpecialPowerExecution {
    pub fn new(power_id: u32) -> Self {
        Self {
            power_id,
            target_position: None,
            target_object_id: None,
            angle: 0.0,
            command_options: 0,
        }
    }

    pub fn with_location(mut self, x: f32, y: f32, z: f32) -> Self {
        self.target_position = Some((x, y, z));
        self
    }

    pub fn with_object(mut self, object_id: u32) -> Self {
        self.target_object_id = Some(object_id);
        self
    }

    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    pub fn with_options(mut self, options: u32) -> Self {
        self.command_options = options;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_template_creation() {
        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        assert_eq!(template.get_name(), "TestPower");
        assert_eq!(template.get_id(), 1);
        assert_eq!(template.get_special_power_type(), SpecialPowerType::Invalid);
        assert!(!template.has_public_timer());
        assert!(!template.is_shared_n_sync());
    }

    #[test]
    fn test_special_power_store() {
        let mut store = SpecialPowerStore::new();

        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        store.add_template(template);

        assert_eq!(store.get_num_special_powers(), 1);

        let found = store.find_template("TestPower");
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_id(), 1);

        let found_by_id = store.find_template_by_id(1);
        assert!(found_by_id.is_some());
        assert_eq!(found_by_id.unwrap().get_name(), "TestPower");
    }

    #[test]
    fn test_special_power_module_ready_state() {
        let mut template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        template.reload_time = 100; // 100 frames

        let mut module = SpecialPowerModule::new(template);

        // Start recharge at frame 0
        module.start_power_recharge(0);

        // At frame 0, should not be ready
        assert!(!module.is_ready(0));
        assert_eq!(module.get_percent_ready(0), 0.0);

        // At frame 50, should be 50% ready
        assert!(!module.is_ready(50));
        assert!((module.get_percent_ready(50) - 0.5).abs() < 0.01);

        // At frame 100, should be ready
        assert!(module.is_ready(100));
        assert_eq!(module.get_percent_ready(100), 1.0);

        // After frame 100, should still be ready
        assert!(module.is_ready(150));
        assert_eq!(module.get_percent_ready(150), 1.0);
    }

    #[test]
    fn test_special_power_module_pause() {
        let mut template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        template.reload_time = 100;

        let mut module = SpecialPowerModule::new(template);
        module.start_power_recharge(0);

        // Pause at frame 50 (50% ready)
        module.pause_countdown(true, 50);
        assert!(!module.is_ready(50));

        // At frame 100, still paused so not ready
        assert!(!module.is_ready(100));

        // Unpause at frame 100
        module.pause_countdown(false, 100);

        // Should now be ready at frame 150 (50 frames paused + 100 original)
        assert!(!module.is_ready(100));
        assert!(module.is_ready(150));
    }

    #[test]
    fn test_player_power_state_shared_powers() {
        let mut state = PlayerPowerState::new();

        let mut template = SpecialPowerTemplate::new("SharedPower".to_string(), 1);
        template.reload_time = 100;
        template.shared_n_sync = true;

        // First call should start the timer
        let ready_frame = state.get_or_start_special_power_ready_frame(&template, 0);
        assert_eq!(ready_frame, 100);

        // Second call should return the same frame
        let ready_frame2 = state.get_or_start_special_power_ready_frame(&template, 50);
        assert_eq!(ready_frame2, 100);

        // Reset should update the frame
        state.reset_or_start_special_power_ready_frame(&template, 50);
        let ready_frame3 = state.get_or_start_special_power_ready_frame(&template, 60);
        assert_eq!(ready_frame3, 150);
    }

    #[test]
    fn test_special_power_execution() {
        let exec = SpecialPowerExecution::new(1)
            .with_location(100.0, 200.0, 0.0)
            .with_angle(45.0)
            .with_options(0x1);

        assert_eq!(exec.power_id, 1);
        assert_eq!(exec.target_position, Some((100.0, 200.0, 0.0)));
        assert_eq!(exec.angle, 45.0);
        assert_eq!(exec.command_options, 0x1);
    }

    #[test]
    fn test_multiple_pause_unpause() {
        let mut template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        template.reload_time = 100;

        let mut module = SpecialPowerModule::new(template);
        module.start_power_recharge(0);

        // Multiple pauses
        module.pause_countdown(true, 50);
        module.pause_countdown(true, 50);
        assert!(!module.is_ready(100));

        // First unpause - still paused
        module.pause_countdown(false, 100);
        assert!(!module.is_ready(100));

        // Second unpause - now unpaused
        module.pause_countdown(false, 100);
        assert!(module.is_ready(150));
    }
}
