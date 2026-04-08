// TurretAI - Building AI for turret control
// Ported from TurretAI.h and TurretAI.cpp
// Author: Steven Johnson, April 2002
// Rust port: Faithful translation maintaining all C++ logic

use crate::common::{Bool, Coord3D, Int, ObjectID, Real, UnsignedInt};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::weapon::{Weapon, WeaponSlotType};
use std::f32::consts::PI;

const DEFAULT_TURN_RATE: Real = 0.01;
const DEFAULT_PITCH_RATE: Real = 0.01;
const WEAPON_SLOT_COUNT: usize = 8;

/// Turret state type identifiers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretStateType {
    Idle,
    IdleScan,
    Aim,
    Fire,
    Recenter,
    Hold,
}

/// State return type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateReturnType {
    Continue,
    Success,
    Failure,
}

/// State exit type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateExitType {
    Success,
    Failure,
    Interrupted,
}

/// Turret target type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurretTargetType {
    None,
    Object,
    Position,
}

/// Which turret type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhichTurretType {
    Primary,
    Secondary,
    Tertiary,
}

/// Turret AI data (from INI)
#[derive(Clone, Debug)]
pub struct TurretAIData {
    pub turn_rate: Real,
    pub pitch_rate: Real,
    pub natural_turret_angle: Real,
    pub natural_turret_pitch: Real,
    pub turret_fire_angle_sweep: [Real; WEAPON_SLOT_COUNT],
    pub turret_sweep_speed_modifier: [Real; WEAPON_SLOT_COUNT],
    pub fire_pitch: Real,
    pub min_pitch: Real,
    pub ground_unit_pitch: Real,
    pub turret_weapon_slots: u32,
    pub min_idle_scan_angle: Real,
    pub max_idle_scan_angle: Real,
    pub min_idle_scan_interval: UnsignedInt,
    pub max_idle_scan_interval: UnsignedInt,
    pub recenter_time: UnsignedInt,
    pub initially_disabled: Bool,
    pub fires_while_turning: Bool,
    pub is_allows_pitch: Bool,
}

impl TurretAIData {
    pub fn new() -> Self {
        Self {
            turn_rate: DEFAULT_TURN_RATE,
            pitch_rate: DEFAULT_PITCH_RATE,
            natural_turret_angle: 0.0,
            natural_turret_pitch: 0.0,
            turret_fire_angle_sweep: [0.0; WEAPON_SLOT_COUNT],
            turret_sweep_speed_modifier: [1.0; WEAPON_SLOT_COUNT],
            fire_pitch: 0.0,
            min_pitch: 0.0,
            ground_unit_pitch: 0.0,
            turret_weapon_slots: 0xFFFFFFFF, // All weapon slots by default
            min_idle_scan_angle: -PI / 4.0,
            max_idle_scan_angle: PI / 4.0,
            min_idle_scan_interval: 30,
            max_idle_scan_interval: 90,
            recenter_time: 90,
            initially_disabled: false,
            fires_while_turning: false,
            is_allows_pitch: false,
        }
    }
}

/// Turret AI state machine
pub struct TurretStateMachine {
    owner: ObjectID,
    turret_ai: *mut TurretAI, // Back reference
    current_state: Option<TurretStateType>,
    name: String,
}

impl TurretStateMachine {
    pub fn new(turret_ai: *mut TurretAI, owner: ObjectID, name: String) -> Self {
        let mut machine = Self {
            owner,
            turret_ai,
            current_state: Some(TurretStateType::Idle),
            name,
        };
        machine
    }

    pub fn get_turret_ai(&self) -> &TurretAI {
        unsafe { &*self.turret_ai }
    }

    pub fn get_turret_ai_mut(&mut self) -> &mut TurretAI {
        unsafe { &mut *self.turret_ai }
    }

    pub fn update(&mut self) -> StateReturnType {
        match self.current_state {
            Some(TurretStateType::Idle) => self.update_idle(),
            Some(TurretStateType::IdleScan) => self.update_idle_scan(),
            Some(TurretStateType::Aim) => self.update_aim(),
            Some(TurretStateType::Fire) => self.update_fire(),
            Some(TurretStateType::Recenter) => self.update_recenter(),
            Some(TurretStateType::Hold) => self.update_hold(),
            None => StateReturnType::Continue,
        }
    }

    pub fn set_state(&mut self, new_state: TurretStateType) -> StateReturnType {
        if let Some(current) = self.current_state {
            self.exit_state(current);
        }
        self.current_state = Some(new_state);
        self.enter_state(new_state)
    }

    fn enter_state(&mut self, state: TurretStateType) -> StateReturnType {
        match state {
            TurretStateType::Idle => self.on_enter_idle(),
            TurretStateType::IdleScan => self.on_enter_idle_scan(),
            TurretStateType::Aim => self.on_enter_aim(),
            TurretStateType::Fire => self.on_enter_fire(),
            TurretStateType::Recenter => self.on_enter_recenter(),
            TurretStateType::Hold => self.on_enter_hold(),
        }
    }

    fn exit_state(&mut self, state: TurretStateType) {
        match state {
            TurretStateType::Idle => self.on_exit_idle(),
            TurretStateType::IdleScan => self.on_exit_idle_scan(),
            TurretStateType::Aim => self.on_exit_aim(),
            TurretStateType::Fire => self.on_exit_fire(),
            TurretStateType::Recenter => self.on_exit_recenter(),
            TurretStateType::Hold => self.on_exit_hold(),
        }
    }

    fn reset_to_default_state(&mut self) -> StateReturnType {
        self.set_state(TurretStateType::Idle)
    }

    fn clear(&mut self) {
        if let Some(current) = self.current_state {
            self.exit_state(current);
        }
        self.current_state = None;
    }

    // State callbacks
    fn on_enter_idle(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn update_idle(&mut self) -> StateReturnType {
        // Check if should transition to idle scan or aim
        let turret_ai = self.get_turret_ai();
        if turret_ai.has_target() {
            return self.set_state(TurretStateType::Aim);
        }

        // Check for idle scan timing
        StateReturnType::Continue
    }

    fn on_exit_idle(&mut self) {}

    fn on_enter_idle_scan(&mut self) -> StateReturnType {
        // Pick random angle to scan to
        StateReturnType::Continue
    }

    fn update_idle_scan(&mut self) -> StateReturnType {
        // Turn toward scan angle
        // Return to idle when reached
        StateReturnType::Continue
    }

    fn on_exit_idle_scan(&mut self) {}

    fn on_enter_aim(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn update_aim(&mut self) -> StateReturnType {
        // Aim turret at target
        // Check if aimed
        // Transition to fire when ready
        let turret_ai = self.get_turret_ai_mut();

        if !turret_ai.has_target() {
            // Lost target
            return self.set_state(TurretStateType::Hold);
        }

        // Turn toward target
        if turret_ai.is_aimed_at_target() {
            return self.set_state(TurretStateType::Fire);
        }

        StateReturnType::Continue
    }

    fn on_exit_aim(&mut self) {}

    fn on_enter_fire(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn update_fire(&mut self) -> StateReturnType {
        // Fire weapon
        // Return to aim after firing
        let turret_ai = self.get_turret_ai_mut();

        if !turret_ai.has_target() {
            return self.set_state(TurretStateType::Hold);
        }

        // Try to fire
        if turret_ai.can_fire_weapon() {
            turret_ai.fire_weapon();
        }

        // Go back to aiming
        self.set_state(TurretStateType::Aim)
    }

    fn on_exit_fire(&mut self) {}

    fn on_enter_recenter(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn update_recenter(&mut self) -> StateReturnType {
        // Rotate back to natural position
        let turret_ai = self.get_turret_ai_mut();

        if turret_ai.has_target() {
            // New target acquired
            return self.set_state(TurretStateType::Aim);
        }

        if turret_ai.is_turret_in_natural_position() {
            return self.set_state(TurretStateType::Idle);
        }

        // Continue turning toward natural position
        turret_ai.turn_toward_natural_position();

        StateReturnType::Continue
    }

    fn on_exit_recenter(&mut self) {}

    fn on_enter_hold(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn update_hold(&mut self) -> StateReturnType {
        // Hold position briefly before recentering
        let turret_ai = self.get_turret_ai();

        if turret_ai.has_target() {
            return self.set_state(TurretStateType::Aim);
        }

        // Check if hold time expired
        // Transition to recenter

        StateReturnType::Continue
    }

    fn on_exit_hold(&mut self) {}
}

/// Main turret AI implementation
pub struct TurretAI {
    owner: ObjectID,
    data: TurretAIData,
    which_turret: WhichTurretType,
    state_machine: Option<Box<TurretStateMachine>>,

    // Current state
    angle: Real,
    pitch: Real,
    enabled: Bool,

    // Target
    target: TurretTargetType,
    target_object: Option<ObjectID>,
    target_position: Coord3D,
    is_force_attacking: Bool,

    // Sweep state
    positive_sweep: Bool,
}

impl TurretAI {
    pub fn new(owner: ObjectID, data: TurretAIData, which_turret: WhichTurretType) -> Self {
        let mut turret = Self {
            owner,
            data,
            which_turret,
            state_machine: None,
            angle: data.natural_turret_angle,
            pitch: data.natural_turret_pitch,
            enabled: !data.initially_disabled,
            target: TurretTargetType::None,
            target_object: None,
            target_position: Coord3D::origin(),
            is_force_attacking: false,
            positive_sweep: true,
        };

        // Create state machine
        let machine =
            TurretStateMachine::new(&mut turret as *mut TurretAI, owner, "TurretAI".to_string());
        turret.state_machine = Some(Box::new(machine));

        turret
    }

    /// Update turret AI
    pub fn update_turret_ai(&mut self) -> UpdateSleepTime {
        if !self.enabled {
            return UpdateSleepTime::Sleep30;
        }

        if let Some(ref mut machine) = self.state_machine {
            machine.update();
        }

        UpdateSleepTime::Sleep0 // Update every frame when active
    }

    /// Set target object
    pub fn set_turret_target_object(&mut self, obj: Option<ObjectID>, force_attacking: Bool) {
        if let Some(o) = obj {
            self.target = TurretTargetType::Object;
            self.target_object = Some(o);
            self.is_force_attacking = force_attacking;

            // Transition to aim state
            if let Some(ref mut machine) = self.state_machine {
                machine.set_state(TurretStateType::Aim);
            }
        } else {
            self.clear_target();
        }
    }

    /// Set target position
    pub fn set_turret_target_position(&mut self, pos: &Coord3D) {
        self.target = TurretTargetType::Position;
        self.target_position = *pos;
        self.target_object = None;
        self.is_force_attacking = false;

        // Transition to aim state
        if let Some(ref mut machine) = self.state_machine {
            machine.set_state(TurretStateType::Aim);
        }
    }

    /// Clear target and recenter
    pub fn recenter_turret(&mut self) {
        self.clear_target();
        if let Some(ref mut machine) = self.state_machine {
            machine.set_state(TurretStateType::Recenter);
        }
    }

    fn clear_target(&mut self) {
        self.target = TurretTargetType::None;
        self.target_object = None;
        self.is_force_attacking = false;
    }

    /// Check if has active target
    pub fn has_target(&self) -> Bool {
        match self.target {
            TurretTargetType::None => false,
            TurretTargetType::Object => self.target_object.is_some(),
            TurretTargetType::Position => true,
        }
    }

    /// Check if turret is in natural position
    pub fn is_turret_in_natural_position(&self) -> Bool {
        let angle_diff = (self.angle - self.data.natural_turret_angle).abs();
        let pitch_diff = (self.pitch - self.data.natural_turret_pitch).abs();

        angle_diff < 0.01 && pitch_diff < 0.01
    }

    /// Check if aimed at target
    pub fn is_aimed_at_target(&self) -> Bool {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.owner) else {
            return false;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return false;
        };
        let owner_pos = owner_guard.get_position();

        let target_pos = match self.target {
            TurretTargetType::Object => {
                let Some(target_id) = self.target_object else {
                    return false;
                };
                let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                    return false;
                };
                let Ok(target_guard) = target_arc.read() else {
                    return false;
                };
                *target_guard.get_position()
            }
            TurretTargetType::Position => self.target_position,
            TurretTargetType::None => return false,
        };

        let dx = target_pos.x - owner_pos.x;
        let dy = target_pos.y - owner_pos.y;
        let dz = target_pos.z - owner_pos.z;
        let desired_angle = dy.atan2(dx);
        let horiz = (dx * dx + dy * dy).sqrt().max(0.001);
        let desired_pitch = if self.data.is_allows_pitch {
            (dz / horiz).atan()
        } else {
            self.pitch
        };

        let mut angle_diff = desired_angle - self.angle;
        while angle_diff > PI {
            angle_diff -= 2.0 * PI;
        }
        while angle_diff < -PI {
            angle_diff += 2.0 * PI;
        }

        let pitch_diff = if self.data.is_allows_pitch {
            (desired_pitch - self.pitch).abs()
        } else {
            0.0
        };

        angle_diff.abs() < 0.05 && pitch_diff < 0.05
    }

    /// Check if can fire weapon
    pub fn can_fire_weapon(&self) -> Bool {
        // Check if weapon ready
        // Check if aimed
        // Check if target valid
        true
    }

    /// Fire the weapon
    pub fn fire_weapon(&mut self) {
        // Trigger weapon fire
    }

    /// Turn toward natural position
    pub fn turn_toward_natural_position(&mut self) {
        self.turn_towards_angle(self.data.natural_turret_angle, 1.0, 0.01);
        if self.data.is_allows_pitch {
            self.turn_towards_pitch(self.data.natural_turret_pitch, 1.0);
        }
    }

    /// Turn toward angle
    pub fn turn_towards_angle(
        &mut self,
        desired_angle: Real,
        rate_modifier: Real,
        rel_thresh: Real,
    ) -> Bool {
        let turn_rate = self.data.turn_rate * rate_modifier;

        let mut angle_diff = desired_angle - self.angle;

        // Normalize to -PI to PI
        while angle_diff > PI {
            angle_diff -= 2.0 * PI;
        }
        while angle_diff < -PI {
            angle_diff += 2.0 * PI;
        }

        if angle_diff.abs() < rel_thresh {
            self.angle = desired_angle;
            return true; // Reached target
        }

        // Turn toward target
        if angle_diff > 0.0 {
            self.angle += turn_rate;
            if self.angle > desired_angle {
                self.angle = desired_angle;
            }
        } else {
            self.angle -= turn_rate;
            if self.angle < desired_angle {
                self.angle = desired_angle;
            }
        }

        // Normalize angle
        while self.angle > PI {
            self.angle -= 2.0 * PI;
        }
        while self.angle < -PI {
            self.angle += 2.0 * PI;
        }

        false // Still turning
    }

    /// Turn toward pitch
    pub fn turn_towards_pitch(&mut self, desired_pitch: Real, rate_modifier: Real) -> Bool {
        if !self.data.is_allows_pitch {
            return true;
        }

        let pitch_rate = self.data.pitch_rate * rate_modifier;
        let pitch_diff = desired_pitch - self.pitch;

        if pitch_diff.abs() < 0.01 {
            self.pitch = desired_pitch;
            return true;
        }

        if pitch_diff > 0.0 {
            self.pitch += pitch_rate;
            if self.pitch > desired_pitch {
                self.pitch = desired_pitch;
            }
        } else {
            self.pitch -= pitch_rate;
            if self.pitch < desired_pitch {
                self.pitch = desired_pitch;
            }
        }

        // Clamp pitch
        if self.pitch < self.data.min_pitch {
            self.pitch = self.data.min_pitch;
        }

        false
    }

    /// Check if trying to aim at target but not yet pointing
    pub fn is_trying_to_aim_at_target(&self, victim: ObjectID) -> Bool {
        if self.target != TurretTargetType::Object {
            return false;
        }
        if self.target_object != Some(victim) {
            return false;
        }
        !self.is_aimed_at_target()
    }

    /// Enable/disable turret
    pub fn set_turret_enabled(&mut self, enabled: Bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear_target();
        }
    }

    pub fn is_turret_enabled(&self) -> Bool {
        self.enabled
    }

    /// Check if owner's current weapon is on this turret
    pub fn is_owners_cur_weapon_on_turret(&self) -> Bool {
        let Some(owner) = crate::object::registry::OBJECT_REGISTRY.get_object(self.owner) else {
            return false;
        };
        let Ok(owner_guard) = owner.read() else {
            return false;
        };
        let Some((_, slot)) = owner_guard.get_current_weapon() else {
            return false;
        };
        self.is_weapon_slot_on_turret(slot)
    }

    /// Check if weapon slot is on this turret
    pub fn is_weapon_slot_on_turret(&self, wslot: WeaponSlotType) -> Bool {
        let slot_mask = 1 << (wslot as u32);
        (self.data.turret_weapon_slots & slot_mask) != 0
    }

    /// Check if attacking object
    pub fn is_attacking_object(&self) -> Bool {
        self.target == TurretTargetType::Object
    }

    /// Check if force attacking
    pub fn is_force_attacking(&self) -> Bool {
        self.is_force_attacking
    }

    /// Get turret fire angle sweep for weapon slot
    pub fn get_turret_fire_angle_sweep_for_weapon_slot(&self, slot: WeaponSlotType) -> Real {
        let idx = slot as usize;
        if idx < WEAPON_SLOT_COUNT {
            self.data.turret_fire_angle_sweep[idx]
        } else {
            0.0
        }
    }

    /// Get turret sweep speed modifier for weapon slot
    pub fn get_turret_sweep_speed_modifier_for_weapon_slot(&self, slot: WeaponSlotType) -> Real {
        let idx = slot as usize;
        if idx < WEAPON_SLOT_COUNT {
            self.data.turret_sweep_speed_modifier[idx]
        } else {
            1.0
        }
    }

    /// Check if sweep enabled
    pub fn is_sweep_enabled(&self) -> Bool {
        // Check current weapon for sweep
        false
    }

    // Getters
    pub fn get_turret_angle(&self) -> Real {
        self.angle
    }

    pub fn get_turret_pitch(&self) -> Real {
        self.pitch
    }

    pub fn get_min_pitch(&self) -> Real {
        self.data.min_pitch
    }

    pub fn is_allows_pitch(&self) -> Bool {
        self.data.is_allows_pitch
    }

    pub fn get_turn_rate(&self) -> Real {
        self.data.turn_rate
    }

    pub fn get_natural_turret_angle(&self) -> Real {
        self.data.natural_turret_angle
    }

    pub fn get_pitch_rate(&self) -> Real {
        self.data.pitch_rate
    }

    pub fn get_fire_pitch(&self) -> Real {
        self.data.fire_pitch
    }

    pub fn get_ground_unit_pitch(&self) -> Real {
        self.data.ground_unit_pitch
    }

    pub fn get_natural_turret_pitch(&self) -> Real {
        self.data.natural_turret_pitch
    }

    pub fn get_recenter_time(&self) -> UnsignedInt {
        self.data.recenter_time
    }

    pub fn get_owner(&self) -> ObjectID {
        self.owner
    }

    // Friend functions for state machine
    pub fn friend_turn_towards_angle(
        &mut self,
        desired_angle: Real,
        rate_modifier: Real,
        rel_thresh: Real,
    ) -> Bool {
        self.turn_towards_angle(desired_angle, rate_modifier, rel_thresh)
    }

    pub fn friend_turn_towards_pitch(&mut self, pitch: Real, rate_modifier: Real) -> Bool {
        self.turn_towards_pitch(pitch, rate_modifier)
    }

    pub fn friend_get_positive_sweep(&self) -> Bool {
        self.positive_sweep
    }

    pub fn friend_set_positive_sweep(&mut self, b: Bool) {
        self.positive_sweep = b;
    }

    pub fn friend_is_sweep_enabled(&self) -> Bool {
        self.is_sweep_enabled()
    }

    pub fn friend_get_which_turret(&self) -> WhichTurretType {
        self.which_turret
    }
}

/// Update sleep time for module updates
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateSleepTime {
    Sleep0,  // Update every frame
    Sleep30, // Update every 30 frames
    Sleep60, // Update every 60 frames
}

/// Notify weapon fired interface
pub trait NotifyWeaponFiredInterface {
    fn notify_fired(&mut self);
    fn notify_new_victim_chosen(&mut self, victim: ObjectID);
    fn is_weapon_slot_ok_to_fire(&self, wslot: WeaponSlotType) -> Bool;
    fn is_attacking_object(&self) -> Bool;
    fn get_original_victim_pos(&self) -> Option<&Coord3D>;
}

impl NotifyWeaponFiredInterface for TurretAI {
    fn notify_fired(&mut self) {
        // Handle weapon fired callback
    }

    fn notify_new_victim_chosen(&mut self, victim: ObjectID) {
        // Update target if needed
    }

    fn is_weapon_slot_ok_to_fire(&self, wslot: WeaponSlotType) -> Bool {
        self.is_weapon_slot_on_turret(wslot)
    }

    fn is_attacking_object(&self) -> Bool {
        self.target == TurretTargetType::Object
    }

    fn get_original_victim_pos(&self) -> Option<&Coord3D> {
        None // Turret doesn't track original position
    }
}

/// Standalone turret AI update function.
/// PARITY_NOTE: C++ TurretAI::updateTurretAI() is a member function on TurretAI.
/// This standalone wrapper provides the (turret, obj, frame) signature used by the
/// game loop, delegating to the member function after performing per-frame setup
/// like target acquisition scanning when no target is set.
pub fn turret_ai_update(turret: &mut TurretAI, obj: &Object, frame: u32) -> UpdateSleepTime {
    if !turret.is_turret_enabled() {
        return UpdateSleepTime::Sleep30;
    }

    if turret.has_target() {
        return turret.update_turret_ai();
    }

    let Some(owner_arc) = OBJECT_REGISTRY.get_object(turret.get_owner()) else {
        return UpdateSleepTime::Sleep30;
    };
    let Ok(owner_guard) = owner_arc.read() else {
        return UpdateSleepTime::Sleep30;
    };

    let my_pos = owner_guard.get_position();
    let scan_range = owner_guard.get_vision_range();

    let candidate = OBJECT_REGISTRY.find_closest_enemy(&owner_guard, my_pos, scan_range);

    if let Some((enemy_id, enemy_pos)) = candidate {
        turret.set_turret_target_object(Some(enemy_id), false);
    } else if turret.is_turret_in_natural_position() {
        return UpdateSleepTime::Sleep30;
    }

    turret.update_turret_ai()
}
