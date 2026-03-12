//! Base Special Power Module Implementation

use super::cooldown::{CooldownManager, CooldownState};
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::modules::SpecialPowerModuleInterface as ModuleInterface;
use crate::player::{PlayerIndex, ThePlayerList};
use std::sync::{Arc, Mutex};

/// Base special power module data (configuration)
#[derive(Debug, Clone)]
pub struct SpecialPowerModuleData {
    /// Power identifier
    pub power_id: SpecialPowerID,
    /// Power type/kind
    pub power_kind: SpecialPowerKind,
    /// Display name
    pub name: AsciiString,
    /// Description for UI
    pub description: AsciiString,
    /// Cooldown time in seconds
    pub recharge_time: Real,
    /// Initial charge time in seconds (first use)
    pub init_charge_time: Real,
    /// Money cost to activate
    pub cost: Int,
    /// Range (0 = infinite)
    pub range: Real,
    /// Minimum range
    pub min_range: Real,
    /// Effect radius
    pub radius: Real,
    /// Special power flags
    pub flags: SpecialPowerFlags,
    /// Required science/tech
    pub required_science: Vec<AsciiString>,
    /// Shared sync group for shared cooldowns
    pub shared_sync_group: Option<AsciiString>,
    /// Icon name for UI
    pub icon_name: AsciiString,
    /// Sound effect name
    pub sound_effect: AsciiString,
    /// Maximum altitude for targeting
    pub max_altitude: Real,
}

impl SpecialPowerModuleData {
    pub fn new(name: AsciiString, power_kind: SpecialPowerKind) -> Self {
        Self {
            power_id: 0, // Set by registry
            power_kind,
            name,
            description: AsciiString::new(),
            recharge_time: 30.0,
            init_charge_time: 0.0,
            cost: 0,
            range: 0.0,
            min_range: 0.0,
            radius: 0.0,
            flags: SpecialPowerFlags::empty(),
            required_science: Vec::new(),
            shared_sync_group: None,
            icon_name: AsciiString::new(),
            sound_effect: AsciiString::new(),
            max_altitude: 1000.0,
        }
    }

    /// Check if prerequisites are met
    /// Matches C++ SpecialPowerTemplate::checkPrerequisites
    pub fn check_prerequisites(&self, player_id: ObjectID) -> Bool {
        if self.required_science.is_empty() {
            return true;
        }

        let Some(manager) = super::player_science::get_player_science_manager() else {
            return false;
        };
        let Ok(mgr) = manager.read() else {
            return false;
        };
        let Some(player_science) = mgr.get_player(player_id) else {
            return false;
        };

        player_science.has_all_sciences(&self.required_science)
    }

    /// Check if power requires targeting
    pub fn requires_targeting(&self) -> Bool {
        self.flags.contains(SpecialPowerFlags::REQUIRES_TARGETING)
    }

    /// Check if power is instant (no targeting)
    pub fn is_instant(&self) -> Bool {
        self.flags.contains(SpecialPowerFlags::INSTANT)
    }

    /// Check if power is a superweapon
    pub fn is_superweapon(&self) -> Bool {
        self.flags.contains(SpecialPowerFlags::SUPERWEAPON)
    }
}

/// Base special power module interface trait
pub trait SpecialPowerModuleInterface: Send + Sync {
    /// Get module data
    fn get_data(&self) -> &SpecialPowerModuleData;

    /// Get mutable module data
    fn get_data_mut(&mut self) -> &mut SpecialPowerModuleData;

    /// Get cooldown state
    fn get_cooldown_state(&self) -> &CooldownState;

    /// Get mutable cooldown state
    fn get_cooldown_state_mut(&mut self) -> &mut CooldownState;

    /// Get statistics
    fn get_stats(&self) -> &SpecialPowerStats;

    /// Get mutable statistics
    fn get_stats_mut(&mut self) -> &mut SpecialPowerStats;

    /// Check if power is ready to activate
    fn is_ready(&self) -> Bool {
        self.get_cooldown_state().is_ready()
    }

    /// Check if power is on cooldown
    fn is_on_cooldown(&self) -> Bool {
        self.get_cooldown_state().is_on_cooldown()
    }

    /// Get activation state
    fn get_activation_state(&self) -> ActivationState {
        if !self.is_ready() {
            if self.is_on_cooldown() {
                ActivationState::Cooldown
            } else {
                ActivationState::Unavailable
            }
        } else {
            ActivationState::Ready
        }
    }

    /// Demoralize-specific execution hook (default: unsupported).
    fn execute_demoralize(
        &mut self,
        _player_id: ObjectID,
        _targeting: &TargetingInfo,
    ) -> Result<(), String> {
        Err("execute_demoralize not supported".to_string())
    }

    /// Attempt to activate the power
    fn try_activate(
        &mut self,
        player_id: ObjectID,
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult;

    /// Execute the power's effect
    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String>;

    /// Update the power (called every frame)
    fn update(&mut self, delta_time: Real) {
        self.get_cooldown_state_mut().update(delta_time);
    }

    /// Reset the power (clear cooldown)
    fn reset(&mut self) {
        self.get_cooldown_state_mut().reset();
    }

    /// Get display name
    fn get_name(&self) -> &AsciiString {
        &self.get_data().name
    }

    /// Get cooldown progress (0.0 to 1.0)
    fn get_cooldown_progress(&self) -> Real {
        self.get_cooldown_state().get_progress()
    }

    /// Get remaining cooldown time
    fn get_remaining_cooldown(&self) -> Real {
        self.get_cooldown_state().time_remaining
    }
}

/// Base special power module implementation
#[derive(Debug)]
pub struct SpecialPowerModule {
    /// Module configuration data
    data: SpecialPowerModuleData,
    /// Cooldown state
    cooldown: CooldownState,
    /// Statistics
    stats: SpecialPowerStats,
    /// Current activation state
    activation_state: ActivationState,
    /// Owner player ID
    owner_player_id: Option<ObjectID>,
}

impl SpecialPowerModule {
    pub fn new(data: SpecialPowerModuleData) -> Self {
        let cooldown = CooldownState::new(data.recharge_time, data.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            activation_state: ActivationState::Ready,
            owner_player_id: None,
        }
    }

    /// Set owner player
    pub fn set_owner(&mut self, player_id: ObjectID) {
        self.owner_player_id = Some(player_id);
    }

    /// Get owner player
    pub fn get_owner(&self) -> Option<ObjectID> {
        self.owner_player_id
    }

    /// Check if player can afford the power
    /// Matches C++ SpecialPower::canAfford
    fn can_afford(&self, player_id: ObjectID) -> Bool {
        if self.data.cost <= 0 {
            return true;
        }

        self.get_player_money(player_id)
            .map(|money| money >= self.data.cost)
            .unwrap_or(false)
    }

    /// Deduct cost from player
    /// Matches C++ SpecialPower::deductCost
    fn deduct_cost(&mut self, player_id: ObjectID, _current_frame: UnsignedInt) -> Bool {
        if self.data.cost <= 0 {
            return true;
        }

        let player_list = ThePlayerList();
        let Ok(list_guard) = player_list.read() else {
            return false;
        };
        let Some(player_arc) = list_guard.get_player(player_id as PlayerIndex) else {
            return false;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return false;
        };

        if !player_guard.get_money_mut().subtract_money(self.data.cost) {
            return false;
        }

        if self.data.cost > 0 {
            player_guard
                .get_score_keeper_mut()
                .add_money_spent(self.data.cost as u32);
        }

        true
    }

    fn get_player_money(&self, player_id: ObjectID) -> Option<Int> {
        let player_list = ThePlayerList();
        let list_guard = player_list.read().ok()?;
        let player_arc = list_guard.get_player(player_id as PlayerIndex)?;
        let player_guard = player_arc.read().ok()?;
        Some(player_guard.get_money().get_money())
    }

    /// Check prerequisites
    fn check_prerequisites(&self, player_id: ObjectID) -> Bool {
        self.data.check_prerequisites(player_id)
    }

    /// Validate targeting
    fn validate_targeting(&self, targeting: Option<&TargetingInfo>) -> Result<(), String> {
        if self.data.requires_targeting() && targeting.is_none() {
            return Err("Power requires targeting but no target provided".to_string());
        }

        if self.data.is_instant() && targeting.is_some() {
            return Err("Instant power does not accept targeting".to_string());
        }

        Ok(())
    }

    /// Play activation sound
    fn play_sound(&self) {
        // Play sound effect (matches C++ SpecialPower activation sound playing)
        // In C++: TheAudio->addAudioEvent(&soundEvent)
        // Deferred: requires Audio system integration
        if !self.data.sound_effect.is_empty() {
            log::debug!("Playing sound: {}", self.data.sound_effect);
        }
    }

    /// Show visual effects
    fn show_effects(&self, _targeting: &TargetingInfo) {
        // Show visual FX (matches C++ SpecialPower FX display)
        // In C++: FXList::doFXPos(power->getFXList(), location, ...)
        // Deferred: requires FX system integration for particle effects
        log::debug!("Showing visual effects for power: {}", self.data.name);
    }
}

impl SpecialPowerModuleInterface for SpecialPowerModule {
    fn get_data(&self) -> &SpecialPowerModuleData {
        &self.data
    }

    fn get_data_mut(&mut self) -> &mut SpecialPowerModuleData {
        &mut self.data
    }

    fn get_cooldown_state(&self) -> &CooldownState {
        &self.cooldown
    }

    fn get_cooldown_state_mut(&mut self) -> &mut CooldownState {
        &mut self.cooldown
    }

    fn get_stats(&self) -> &SpecialPowerStats {
        &self.stats
    }

    fn get_stats_mut(&mut self) -> &mut SpecialPowerStats {
        &mut self.stats
    }

    fn try_activate(
        &mut self,
        player_id: ObjectID,
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        // Check if on cooldown
        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        // Check if can afford
        if !self.can_afford(player_id) {
            let available = self.get_player_money(player_id).unwrap_or(0);
            return ActivationResult::InsufficientFunds {
                cost: self.data.cost,
                available,
            };
        }

        // Check prerequisites
        if !self.check_prerequisites(player_id) {
            return ActivationResult::MissingPrerequisites {
                required: self.data.required_science.clone(),
            };
        }

        // Validate targeting
        if let Err(reason) = self.validate_targeting(targeting) {
            return ActivationResult::InvalidTarget { reason };
        }

        // Deduct cost
        if !self.deduct_cost(player_id, current_frame) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        // Execute power
        if let Some(target_info) = targeting {
            if let Err(reason) = self.execute(target_info) {
                return ActivationResult::Failed { reason };
            }
        }

        // Play sound and effects
        self.play_sound();
        if let Some(target_info) = targeting {
            self.show_effects(target_info);
        }

        // Start cooldown
        self.cooldown.start_cooldown(current_frame);

        // Update stats
        self.stats.record_activation(current_frame, self.data.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, _targeting: &TargetingInfo) -> Result<(), String> {
        // Base implementation - should be overridden by specific power types
        log::warn!("Base execute called for power: {}", self.data.name);
        Ok(())
    }
}

/// Shared power module type
pub type SharedSpecialPowerModule = Arc<Mutex<Box<dyn SpecialPowerModuleInterface>>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use std::sync::{Arc, Mutex, OnceLock, RwLock};

    static TEST_PLAYER_LIST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct PlayerListTestGuard {
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl PlayerListTestGuard {
        fn lock() -> Self {
            let lock = TEST_PLAYER_LIST_LOCK.get_or_init(|| Mutex::new(()));
            let _guard = lock.lock().unwrap_or_else(|err| err.into_inner());
            Self { _guard }
        }
    }

    fn setup_player(player_id: PlayerIndex, money: Int) -> PlayerListTestGuard {
        let guard = PlayerListTestGuard::lock();
        let player_list = ThePlayerList();
        if let Ok(mut list_guard) = player_list.write() {
            list_guard.clear();
            let player = Arc::new(RwLock::new(Player::new(player_id)));
            if let Ok(mut player_guard) = player.write() {
                player_guard.get_money_mut().set_money(money);
            }
            list_guard.add_player(player);
            list_guard.set_local_player_index(player_id);
        }
        guard
    }

    #[test]
    fn test_power_module_creation() {
        let _player_guard = setup_player(0, 0);
        let data = SpecialPowerModuleData::new("TestPower".into(), SpecialPowerKind::OCL);
        let module = SpecialPowerModule::new(data);

        assert_eq!(module.get_name(), "TestPower");
        assert!(module.is_ready());
    }

    #[test]
    fn test_power_activation() {
        let _player_guard = setup_player(0, 2000);
        let mut data = SpecialPowerModuleData::new("TestPower".into(), SpecialPowerKind::OCL);
        data.cost = 1000;
        data.recharge_time = 30.0;

        let mut module = SpecialPowerModule::new(data);

        // First activation should succeed
        let result = module.try_activate(0, None, 0);
        assert!(result.is_success());

        // Should now be on cooldown
        assert!(module.is_on_cooldown());

        // Second activation should fail
        let result = module.try_activate(0, None, 30);
        assert!(!result.is_success());
    }

    #[test]
    fn test_cooldown_update() {
        let _player_guard = setup_player(0, 0);
        let data = SpecialPowerModuleData::new("TestPower".into(), SpecialPowerKind::OCL);
        let mut module = SpecialPowerModule::new(data);

        // Activate and start cooldown
        module.try_activate(0, None, 0);
        assert!(module.is_on_cooldown());

        // Update cooldown
        module.update(15.0);
        assert!(module.is_on_cooldown());

        module.update(15.0);
        assert!(!module.is_on_cooldown());
        assert!(module.is_ready());
    }
}
