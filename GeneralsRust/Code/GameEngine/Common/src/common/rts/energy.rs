//! Energy management system for RTS game
//!
//! This module manages energy production and consumption for players.
//! Energy is used to power buildings and various systems.
//!
//! # C++ Reference
//! Based on `/GeneralsMD/Code/GameEngine/Source/Common/RTS/Energy.cpp`
//! and `/GeneralsMD/Code/GameEngine/Include/Common/Energy.h`

use crate::common::system::{Xfer, XferMode};
use crate::common::time;

use super::handles::{FrameNumber, ObjectHandle, PlayerHandle};
use std::sync::{Arc, OnceLock};

/// Energy management system
///
/// This class encapsulates the Player's energy use and production.
/// For consistent nomenclature, energy units are measured in "kilowatts"
/// (though that may have no bearing on reality).
///
/// # C++ Reference
/// Matches C++ class `Energy` from Energy.h lines 32-92
#[derive(Debug, Clone)]
pub struct Energy {
    /// Level of energy production, in kw
    /// C++ Reference: Energy.h line 88
    energy_production: i32,

    /// Level of energy consumption, in kw
    /// C++ Reference: Energy.h line 89
    energy_consumption: i32,

    /// If power is sabotaged, the frame will be greater than now
    /// C++ Reference: Energy.h line 90 - GLA power sabotage ability
    power_sabotaged_till_frame: FrameNumber,

    /// Handle to the owning player
    /// C++ Reference: Energy.h line 91
    owner: PlayerHandle,
}

impl Energy {
    /// Create a new Energy system
    ///
    /// # C++ Reference
    /// Energy.cpp lines 31-37
    pub fn new() -> Self {
        Self {
            energy_production: 0,
            energy_consumption: 0,
            power_sabotaged_till_frame: 0,
            owner: PlayerHandle::INVALID,
        }
    }

    /// Reset energy information to base values
    ///
    /// # C++ Reference
    /// Energy.h lines 40-45 (inline init method)
    pub fn init(&mut self, owner: PlayerHandle) {
        self.energy_production = 0;
        self.energy_consumption = 0;
        self.owner = owner;
    }

    /// Return current energy production in kilowatts
    /// Takes sabotage into account
    ///
    /// # C++ Reference
    /// Energy.cpp lines 40-48
    pub fn get_production(&self) -> i32 {
        let current_frame = time::frame();

        // C++ Energy.cpp line 42-45: Check for power sabotage
        if current_frame < self.power_sabotaged_till_frame {
            // Power sabotaged, therefore no power
            0
        } else {
            self.energy_production
        }
    }

    /// Return current energy consumption in kilowatts
    ///
    /// # C++ Reference
    /// Energy.h line 51 (inline getter)
    pub fn get_consumption(&self) -> i32 {
        self.energy_consumption
    }

    /// Check if we have sufficient power
    ///
    /// # C++ Reference
    /// Energy.cpp lines 68-76
    pub fn has_sufficient_power(&self) -> bool {
        let current_frame = time::frame();

        // C++ Energy.cpp line 70-73: Check for power sabotage
        if current_frame < self.power_sabotaged_till_frame {
            // Power sabotaged, therefore no power
            false
        } else {
            // C++ Energy.cpp line 75
            self.energy_production >= self.energy_consumption
        }
    }

    /// Return the percentage of energy needed that we actually produce, as a 0.0 ... 1.0+ fraction
    ///
    /// # C++ Reference
    /// Energy.cpp lines 51-65
    pub fn get_energy_supply_ratio(&self) -> f32 {
        // C++ Energy.cpp line 53: Debug assertion
        debug_assert!(
            self.energy_production >= 0 && self.energy_consumption >= 0,
            "neg Energy numbers"
        );

        let current_frame = time::frame();

        // C++ Energy.cpp lines 55-59: Check for power sabotage
        if current_frame < self.power_sabotaged_till_frame {
            // Power sabotaged, therefore no power, no ratio
            return 0.0;
        }

        // C++ Energy.cpp lines 61-62: Handle zero consumption
        if self.energy_consumption == 0 {
            return self.energy_production as f32;
        }

        // C++ Energy.cpp line 64: Calculate ratio
        self.energy_production as f32 / self.energy_consumption as f32
    }

    /// Adjust power by a delta amount
    /// If adding is false, we're supposed to be removing this
    ///
    /// # C++ Reference
    /// Energy.cpp lines 79-99
    pub fn adjust_power(&mut self, power_delta: i32, adding: bool) {
        // C++ Energy.cpp lines 81-83: Early exit for zero delta
        if power_delta == 0 {
            return;
        }

        // C++ Energy.cpp lines 85-98: Handle positive/negative deltas
        if power_delta > 0 {
            if adding {
                self.add_production(power_delta);
            } else {
                self.add_production(-power_delta);
            }
        } else {
            // C++ Energy.cpp line 92: Consumption is reversed - negative power is positive consumption
            if adding {
                self.add_consumption(-power_delta);
            } else {
                self.add_consumption(power_delta);
            }
        }
    }

    /// New 'obj' will now add/subtract from this energy construct
    ///
    /// # C++ Reference
    /// Energy.cpp lines 104-125
    pub fn object_entering_influence(&mut self, obj: ObjectHandle) {
        // C++ Energy.cpp lines 108-109: Sanity check
        if !obj.is_valid() {
            return;
        }

        // C++ Energy.cpp line 112: Get energy from template
        // Template energy lookup is delegated via `EnergyObjectLookup` to keep
        // the Common RTS layer decoupled from concrete object/template storage.
        let energy = get_object_energy_production(obj);

        // C++ Energy.cpp lines 115-118: Adjust energy based on sign
        if energy < 0 {
            self.add_consumption(-energy);
        } else if energy > 0 {
            self.add_production(energy);
        }

        // C++ Energy.cpp lines 121-123: Sanity check
        debug_assert!(
            self.energy_production >= 0 && self.energy_consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.energy_production,
            self.energy_consumption
        );
    }

    /// 'obj' will now no longer add/subtract from this energy construct
    ///
    /// # C++ Reference
    /// Energy.cpp lines 130-151
    pub fn object_leaving_influence(&mut self, obj: ObjectHandle) {
        // C++ Energy.cpp lines 134-135: Sanity check
        if !obj.is_valid() {
            return;
        }

        // C++ Energy.cpp line 138: Get energy from template
        let energy = get_object_energy_production(obj);

        // C++ Energy.cpp lines 141-144: Adjust energy (note reversed signs from entering)
        if energy < 0 {
            self.add_consumption(energy);
        } else if energy > 0 {
            self.add_production(-energy);
        }

        // C++ Energy.cpp lines 147-149: Sanity check
        debug_assert!(
            self.energy_production >= 0 && self.energy_consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.energy_production,
            self.energy_consumption
        );
    }

    /// Adds an energy bonus to the player's pool of energy when the "Control Rods" upgrade
    /// is made to the American Cold Fusion Plant
    ///
    /// # C++ Reference
    /// Energy.cpp lines 157-171
    pub fn add_power_bonus(&mut self, obj: ObjectHandle) {
        // C++ Energy.cpp lines 161-162: Sanity check
        if !obj.is_valid() {
            return;
        }

        // C++ Energy.cpp line 164: Get energy bonus from template
        let bonus = get_object_energy_bonus(obj);
        self.add_production(bonus);

        // C++ Energy.cpp lines 167-169: Sanity check
        debug_assert!(
            self.energy_production >= 0 && self.energy_consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.energy_production,
            self.energy_consumption
        );
    }

    /// Removes an energy bonus
    ///
    /// # C++ Reference
    /// Energy.cpp lines 176-190
    pub fn remove_power_bonus(&mut self, obj: ObjectHandle) {
        // C++ Energy.cpp lines 180-181: Sanity check
        if !obj.is_valid() {
            return;
        }

        // C++ Energy.cpp line 183: Remove energy bonus from template
        let bonus = get_object_energy_bonus(obj);
        self.add_production(-bonus);

        // C++ Energy.cpp lines 186-188: Sanity check
        debug_assert!(
            self.energy_production >= 0 && self.energy_consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.energy_production,
            self.energy_consumption
        );
    }

    /// Set the frame until which power is sabotaged (GLA power sabotage special ability)
    ///
    /// # C++ Reference
    /// Energy.h line 68
    pub fn set_power_sabotaged_till_frame(&mut self, frame: FrameNumber) {
        self.power_sabotaged_till_frame = frame;
    }

    /// Get the frame until which power is sabotaged
    ///
    /// # C++ Reference
    /// Energy.h line 69
    pub fn get_power_sabotaged_till_frame(&self) -> FrameNumber {
        self.power_sabotaged_till_frame
    }

    // Private helper methods

    /// Add to production and notify owner of power change
    ///
    /// # C++ Reference
    /// Energy.cpp lines 196-206
    fn add_production(&mut self, amt: i32) {
        // C++ Energy.cpp line 198
        self.energy_production += amt;

        // C++ Energy.cpp lines 200-201
        if !self.owner.is_valid() {
            return;
        }

        // C++ Energy.cpp lines 203-205: Notify player of brownout state change.
        // A repeated brownout signal is safe; the player-side handler can refresh disables.
        notify_player_brownout_change(self.owner, !self.has_sufficient_power());
    }

    /// Add to consumption and notify owner of power change
    ///
    /// # C++ Reference
    /// Energy.cpp lines 209-217
    fn add_consumption(&mut self, amt: i32) {
        // C++ Energy.cpp line 211
        self.energy_consumption += amt;

        // C++ Energy.cpp lines 213-214
        if !self.owner.is_valid() {
            return;
        }

        // C++ Energy.cpp line 216: Notify player of brownout state change.
        notify_player_brownout_change(self.owner, !self.has_sufficient_power());
    }
}

impl Default for Energy {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// Serialization Support
// ================================================================================================

/// Serialization support for Energy system
///
/// # C++ Reference
/// Energy.cpp lines 220-272 (crc, xfer, loadPostProcess methods)
impl Energy {
    /// Compute CRC for save game validation
    ///
    /// # C++ Reference
    /// Energy.cpp lines 222-225
    ///
    /// Note: The C++ implementation has an empty CRC method, which suggests
    /// that energy state is reconstructed from buildings during load rather
    /// than being directly serialized.
    pub fn crc(&self, _xfer: &mut dyn Xfer) {
        // C++ Energy.cpp line 224: Empty implementation
        // Energy values are reconstructed when buildings are loaded
    }

    /// Transfer data for save/load
    ///
    /// # C++ Reference
    /// Energy.cpp lines 232-264
    ///
    /// Version History:
    /// - Version 1: Initial version (deprecated)
    /// - Version 2: Removed direct serialization of production/consumption (line 244-249)
    ///              These are now reconstructed from buildings
    /// - Version 3: Added power sabotage frame (line 259-262)
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        // C++ Energy.cpp lines 236-238: Version management
        const CURRENT_VERSION: u8 = 3;
        let mut version = CURRENT_VERSION;

        // C++ Energy.cpp line 238: xferVersion
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("Energy::xfer version failed: {}", e))
            .ok();

        // C++ Energy.cpp lines 243-249: Production and consumption
        // NOTE: As of version 2, these are NOT saved because they are reconstructed
        // when buildings are loaded. The C++ comment says:
        // "It is actually incorrect to save these, as they are reconstructed when the buildings are loaded"
        if version < 2 {
            xfer.xfer_int(&mut self.energy_production)
                .map_err(|e| format!("Energy::xfer production failed: {}", e))
                .ok();
            xfer.xfer_int(&mut self.energy_consumption)
                .map_err(|e| format!("Energy::xfer consumption failed: {}", e))
                .ok();
        }

        // C++ Energy.cpp lines 252-256: Owning player index
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                let mut owning_player_index = self.owner.value() as i32;
                xfer.xfer_int(&mut owning_player_index)
                    .map_err(|e| format!("Energy::xfer owner failed: {}", e))
                    .ok();
            }
            XferMode::Load => {
                let mut owning_player_index: i32 = 0;
                xfer.xfer_int(&mut owning_player_index)
                    .map_err(|e| format!("Energy::xfer owner failed: {}", e))
                    .ok();
                self.owner = PlayerHandle::new(owning_player_index.max(0) as u32);
            }
            XferMode::Crc | XferMode::Invalid => {}
        }

        // C++ Energy.cpp lines 259-262: Power sabotage (version 3+)
        if version >= 3 {
            xfer.xfer_unsigned_int(&mut self.power_sabotaged_till_frame)
                .map_err(|e| format!("Energy::xfer sabotage failed: {}", e))
                .ok();
        }
    }

    /// Post-process after loading from save game
    ///
    /// # C++ Reference
    /// Energy.cpp lines 269-272
    ///
    /// Note: The C++ implementation is empty because energy production and consumption
    /// are reconstructed from buildings during the load process rather than being
    /// directly loaded from save data.
    pub fn load_post_process(&mut self) {
        // C++ Energy.cpp line 271: Empty implementation
        // Energy values are reconstructed from loaded buildings
    }
}

// ================================================================================================
// Object Template Registry Integration
// ================================================================================================

/// Provides energy-related template data for `ObjectHandle` lookups.
///
/// The original C++ code accesses `obj->getTemplate()` directly. In Rust, this is provided by
/// a pluggable integration point so the Common RTS layer can stay decoupled from whichever
/// object/template registry the runtime uses.
pub trait EnergyObjectLookup: Send + Sync {
    fn energy_production(&self, obj: ObjectHandle) -> i32;
    fn energy_bonus(&self, obj: ObjectHandle) -> i32;
}

static ENERGY_OBJECT_LOOKUP: OnceLock<Arc<dyn EnergyObjectLookup>> = OnceLock::new();

pub fn set_energy_object_lookup(
    lookup: Arc<dyn EnergyObjectLookup>,
) -> Result<(), Arc<dyn EnergyObjectLookup>> {
    ENERGY_OBJECT_LOOKUP.set(lookup)
}

/// Get energy production/consumption from an object's template.
///
/// # C++ Reference
/// Energy.cpp line 112: `obj->getTemplate()->getEnergyProduction()`
///
/// The energy production value can be:
/// - Positive: Building produces power (e.g., power plant = +5)
/// - Negative: Building consumes power (e.g., barracks = -1)
/// - Zero: Building is power-neutral
fn get_object_energy_production(obj: ObjectHandle) -> i32 {
    let lookup = ENERGY_OBJECT_LOOKUP
        .get()
        .expect("EnergyObjectLookup must be registered before energy queries");
    lookup.energy_production(obj)
}

/// Get energy bonus from an object's template.
///
/// # C++ Reference
/// Energy.cpp line 164: `obj->getTemplate()->getEnergyBonus()`
///
/// The energy bonus is used for upgrades like the American "Control Rods" upgrade
/// to the Cold Fusion Reactor, which grants +3 bonus energy production.
fn get_object_energy_bonus(obj: ObjectHandle) -> i32 {
    let lookup = ENERGY_OBJECT_LOOKUP
        .get()
        .expect("EnergyObjectLookup must be registered before energy queries");
    lookup.energy_bonus(obj)
}

// ================================================================================================
// Power Brownout Notification System
// ================================================================================================

/// Receives power brownout notifications for a `PlayerHandle`.
pub trait EnergyOwnerCallbacks: Send + Sync {
    fn on_power_brown_out_change(&self, player: PlayerHandle, brown_out: bool);
}

static ENERGY_OWNER_CALLBACKS: OnceLock<Arc<dyn EnergyOwnerCallbacks>> = OnceLock::new();

pub fn set_energy_owner_callbacks(
    callbacks: Arc<dyn EnergyOwnerCallbacks>,
) -> Result<(), Arc<dyn EnergyOwnerCallbacks>> {
    ENERGY_OWNER_CALLBACKS.set(callbacks)
}

/// Notify player of power brownout state change
///
/// # C++ Reference
/// Energy.cpp lines 205, 216: `m_owner->onPowerBrownOutChange(brownOut)`
/// Player.cpp lines 3232-3241: Implementation of Player::onPowerBrownOutChange
///
/// When called, this function should:
/// 1. If brownOut is true: disable radar
/// 2. If brownOut is false: enable radar (removes restriction, doesn't force on)
/// 3. Iterate all player objects and call doPowerDisable on each
///
/// This is commented out because it requires the Player callback system to be
/// fully integrated into the Rust architecture.
#[allow(dead_code)] // C++ parity: awaiting Player callback system integration
fn notify_player_brownout_change(_player: PlayerHandle, _brown_out: bool) {
    if let Some(callbacks) = ENERGY_OWNER_CALLBACKS.get() {
        callbacks.on_power_brown_out_change(_player, _brown_out);
    }
}

// ================================================================================================
// Tests
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_creation() {
        let energy = Energy::new();
        assert_eq!(energy.get_production(), 0);
        assert_eq!(energy.get_consumption(), 0);
        assert!(energy.has_sufficient_power());
    }

    #[test]
    fn test_energy_init() {
        let mut energy = Energy::new();
        let player = PlayerHandle::new(1);
        energy.init(player);
        assert_eq!(energy.get_production(), 0);
        assert_eq!(energy.get_consumption(), 0);
    }

    #[test]
    fn test_sufficient_power() {
        let mut energy = Energy::new();
        energy.adjust_power(100, true); // Add 100 production
        energy.adjust_power(-50, true); // Add 50 consumption
        assert!(energy.has_sufficient_power());
        assert_eq!(energy.get_production(), 100);
        assert_eq!(energy.get_consumption(), 50);
    }

    #[test]
    fn test_insufficient_power() {
        let mut energy = Energy::new();
        energy.adjust_power(50, true); // Add 50 production
        energy.adjust_power(-100, true); // Add 100 consumption
        assert!(!energy.has_sufficient_power());
    }

    #[test]
    fn test_energy_supply_ratio() {
        let mut energy = Energy::new();
        energy.adjust_power(100, true);
        energy.adjust_power(-50, true);
        assert_eq!(energy.get_energy_supply_ratio(), 2.0); // 100 / 50 = 2.0
    }

    #[test]
    fn test_zero_consumption_ratio() {
        let mut energy = Energy::new();
        energy.adjust_power(100, true);
        assert_eq!(energy.get_energy_supply_ratio(), 100.0);
    }

    #[test]
    fn test_power_sabotage() {
        let mut energy = Energy::new();
        energy.adjust_power(100, true);

        // Sabotage power until frame 1000
        energy.set_power_sabotaged_till_frame(1000);

        // Production should be 0 while sabotaged (assuming current frame < 1000)
        assert_eq!(energy.get_production(), 0);
        assert!(!energy.has_sufficient_power());
        assert_eq!(energy.get_energy_supply_ratio(), 0.0);
    }

    #[test]
    fn test_adjust_power_positive() {
        let mut energy = Energy::new();
        energy.adjust_power(50, true);
        assert_eq!(energy.get_production(), 50);

        energy.adjust_power(50, false);
        assert_eq!(energy.get_production(), 0);
    }

    #[test]
    fn test_adjust_power_negative() {
        let mut energy = Energy::new();
        energy.adjust_power(-50, true);
        assert_eq!(energy.get_consumption(), 50);

        energy.adjust_power(-50, false);
        assert_eq!(energy.get_consumption(), 0);
    }
}
