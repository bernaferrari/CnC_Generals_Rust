use crate::ai::object_registry::get_legacy_object;
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::coord::*;
use crate::common::xfer::{Xfer, XferVersion};
use crate::common::*;
use crate::compat::{legacy_transition, register_classic_state, ClassicState};
use crate::game_logic::game_logic::TheGameLogic;
use crate::helpers::ThePartitionManager;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::state_machine::*;
use crate::team::TeamID;
use crate::weapon::{Weapon, WeaponChoiceCriteria, WeaponSlotType};
use game_engine::common::system::Snapshotable;
use log::warn;

use std::sync::{Arc, Mutex, MutexGuard, RwLock, Weak};

/// Default turn rate for turrets
pub const DEFAULT_TURN_RATE: f32 = 0.01;
/// Default pitch rate for turrets
pub const DEFAULT_PITCH_RATE: f32 = 0.01;

/// Wait indefinitely constant
const WAIT_INDEFINITELY: u32 = 0xffffffff;

/// Turret AI state types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretStateType {
    Idle,
    IdleScan,
    Aim,      // Aim turret at GoalObject
    Fire,     // Fire turret at GoalObject
    Recenter, // Rotate turret back to default position
    Hold,     // Hold turret position for a bit before recenter
}

impl From<TurretStateType> for u32 {
    fn from(state: TurretStateType) -> Self {
        state as u32
    }
}

#[derive(Debug, Default)]
pub struct TurretSharedState {
    machine: Weak<Mutex<StateMachine>>,
    turret_ai: Option<Weak<Mutex<TurretAI>>>,
}

impl TurretSharedState {
    fn new(machine: &Arc<Mutex<StateMachine>>, turret_ai: Option<Arc<Mutex<TurretAI>>>) -> Self {
        Self {
            machine: Arc::downgrade(machine),
            turret_ai: turret_ai.map(|ai| Arc::downgrade(&ai)),
        }
    }

    fn with_machine<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut StateMachine) -> R,
    {
        let machine = self
            .machine
            .upgrade()
            .ok_or_else(|| "turret state machine context lost".to_string())?;
        let mut guard = machine
            .lock()
            .map_err(|_| "turret state machine lock poisoned".to_string())?;
        Ok(f(&mut guard))
    }

    fn turret_ai(&self) -> Option<Arc<Mutex<TurretAI>>> {
        self.turret_ai.as_ref().and_then(|weak| weak.upgrade())
    }

    fn notify_state_change(&self) -> Result<(), String> {
        if let Some(ai) = self.turret_ai() {
            let mut guard = ai
                .lock()
                .map_err(|_| "turret AI lock poisoned".to_string())?;
            guard.friend_notify_state_machine_changed();
        }
        Ok(())
    }

    fn change_state(&self, state: TurretStateType) -> Result<(), String> {
        let old_state = self.with_machine(|machine| machine.get_current_state_id())?;
        self.with_machine(|machine| {
            let _ = machine.set_current_state(state.into());
        })?;
        if old_state != Some(state.into()) {
            self.notify_state_change()?;
        }
        Ok(())
    }
}

/// Turret AI behavior controller
pub struct TurretAI {
    /// Owner object
    owner_id: ObjectID,
    /// Current target
    current_target: Option<ObjectID>,
    /// Target kind (none/object/position)
    target_kind: TurretTargetKind,
    /// Whether current target was set by idle mood targeting
    target_was_set_by_idle_mood: bool,
    /// Force-attacking flag
    is_force_attacking: bool,
    /// Victim's initial team (for validation)
    victim_initial_team: Option<TeamID>,
    /// Target position (if aiming at position)
    target_position: Option<Coord3D>,
    /// State machine back-reference
    state_machine: Option<Weak<Mutex<StateMachine>>>,
    /// Turret's natural/default angle
    natural_angle: f32,
    /// Turret's natural/default pitch
    natural_pitch: f32,
    /// Turret's current angle
    current_angle: f32,
    /// Turret's current pitch
    current_pitch: f32,
    /// Turn rate (radians per frame)
    turn_rate: f32,
    /// Pitch rate (radians per frame)
    pitch_rate: f32,
    /// Weapon slot being controlled
    weapon_slot: WeaponSlotType,
    /// Turret weapon slot mask (matches C++ m_turretWeaponSlots)
    turret_weapon_slots_mask: u32,
    /// Whether turret can scan for targets
    can_scan: bool,
    /// Scan angle range (from natural angle)
    scan_range: f32,
    /// Idle scan angle minimum
    min_idle_scan_angle: f32,
    /// Idle scan angle maximum
    max_idle_scan_angle: f32,
    /// Turret fire angle sweep per weapon slot
    turret_fire_angle_sweep: [f32; 3],
    /// Turret sweep speed modifier per weapon slot
    turret_sweep_speed_modifier: [f32; 3],
    /// Minimum physical pitch angle
    min_pitch: f32,
    /// Default ground unit pitch
    ground_unit_pitch: f32,
    /// Whether turret is currently enabled
    enabled: bool,
    /// Whether turret starts disabled
    initially_disabled: bool,
    /// Whether turret can fire while turning
    fires_while_turning: bool,
    /// Inter-turret delay (legacy field)
    inter_turret_delay: u32,
    /// Sweep direction flag
    positive_sweep: bool,
    /// Sweep enabled until this frame
    enable_sweep_until: u32,
    /// Whether turret allows pitch aiming
    allows_pitch: bool,
    /// Fixed fire pitch (if > 0, use instead of aiming at target)
    fire_pitch: f32,
    /// Time to hold position before recentering
    hold_time: u32,
    /// Recenter time (C++ TurretAIData::m_recenterTime)
    recenter_time: u32,
    /// Idle scan interval range (frames)
    min_idle_scan_interval: u32,
    max_idle_scan_interval: u32,
    /// C++ m_continuousFireExpirationFrame — controls when continuous fire stops
    continuous_fire_expiration_frame: u32,
    /// C++ m_playRotSound — rotation sound trigger
    play_rot_sound: bool,
    /// C++ m_playPitchSound — pitch sound trigger
    play_pitch_sound: bool,
    /// C++ m_didFire — fire event tracking
    did_fire: bool,
    /// C++ m_sleepUntil — frame at which turret wakes up
    sleep_until: u32,
}

impl TurretAI {
    pub fn new(owner: Weak<RwLock<Object>>) -> Self {
        let owner_id = owner
            .upgrade()
            .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
            .unwrap_or(crate::common::INVALID_ID);
        Self {
            owner_id,
            current_target: None,
            target_kind: TurretTargetKind::None,
            target_was_set_by_idle_mood: false,
            is_force_attacking: false,
            victim_initial_team: None,
            target_position: None,
            state_machine: None,
            natural_angle: 0.0,
            natural_pitch: 0.0,
            current_angle: 0.0,
            current_pitch: 0.0,
            turn_rate: DEFAULT_TURN_RATE,
            pitch_rate: DEFAULT_PITCH_RATE,
            weapon_slot: WeaponSlotType::Primary,
            turret_weapon_slots_mask: 1 << 0,
            can_scan: true,
            scan_range: std::f32::consts::PI, // 180 degrees
            min_idle_scan_angle: 0.0,
            max_idle_scan_angle: 0.0,
            turret_fire_angle_sweep: [0.0; 3],
            turret_sweep_speed_modifier: [1.0; 3],
            min_pitch: 0.0,
            ground_unit_pitch: 0.0,
            enabled: true,
            initially_disabled: false,
            fires_while_turning: false,
            inter_turret_delay: 0,
            positive_sweep: true,
            enable_sweep_until: 0,
            allows_pitch: false,
            fire_pitch: 0.0,
            hold_time: LOGICFRAMES_PER_SECOND * 2,
            recenter_time: LOGICFRAMES_PER_SECOND * 2,
            min_idle_scan_interval: 9_999_999,
            max_idle_scan_interval: 9_999_999,
            continuous_fire_expiration_frame: u32::MAX,
            play_rot_sound: false,
            play_pitch_sound: false,
            did_fire: false,
            sleep_until: 0,
        }
    }

    /// Get current target
    fn owner_object(&self) -> Option<Arc<RwLock<Object>>> {
        if self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.owner_id))
            .or_else(|| crate::ai::object_registry::get_legacy_object(self.owner_id))
    }

    pub fn get_current_target_id(&self) -> Option<ObjectID> {
        self.current_target
    }

    /// Resolve the current target handle for call sites that still need an Arc.
    pub fn get_current_target(&self) -> Option<Arc<RwLock<Object>>> {
        self.current_target
            .and_then(|id| OBJECT_REGISTRY.get_object(id))
    }

    /// Set current target by stable object ID.
    pub fn set_current_target(&mut self, target: Option<ObjectID>) {
        self.current_target = target.filter(|&id| id != crate::common::INVALID_ID);
        self.target_kind = if self.current_target.is_some() {
            TurretTargetKind::Object
        } else {
            TurretTargetKind::None
        };
        self.target_was_set_by_idle_mood = false;
        self.is_force_attacking = false;
        self.victim_initial_team = self.current_target.and_then(|id| {
            OBJECT_REGISTRY
                .with_object(id, |guard| guard.get_team_id())
                .flatten()
        });
        self.target_position = None;
        self.sync_goal_object();
        self.sync_state_for_target();
    }

    /// Set current target from idle mood selection
    pub fn set_current_target_from_idle_mood(&mut self, target: Option<ObjectID>) {
        self.current_target = target.filter(|&id| id != crate::common::INVALID_ID);
        self.target_kind = if self.current_target.is_some() {
            TurretTargetKind::Object
        } else {
            TurretTargetKind::None
        };
        self.target_was_set_by_idle_mood = true;
        self.is_force_attacking = false;
        self.victim_initial_team = self.current_target.and_then(|id| {
            OBJECT_REGISTRY
                .with_object(id, |guard| guard.get_team_id())
                .flatten()
        });
        self.target_position = None;
        self.sync_goal_object();
        self.sync_state_for_target();
    }

    pub fn set_weapon_slot(&mut self, slot: WeaponSlotType) {
        self.weapon_slot = slot;
    }

    pub fn target_was_set_by_idle_mood(&self) -> bool {
        self.target_was_set_by_idle_mood
    }

    pub fn is_force_attacking(&self) -> bool {
        self.is_force_attacking
    }

    pub fn set_current_target_with_force(
        &mut self,
        target: Option<ObjectID>,
        force_attacking: bool,
    ) {
        self.current_target = target.filter(|&id| id != crate::common::INVALID_ID);
        self.target_kind = if self.current_target.is_some() {
            TurretTargetKind::Object
        } else {
            TurretTargetKind::None
        };
        self.target_was_set_by_idle_mood = false;
        self.is_force_attacking = force_attacking;
        self.victim_initial_team = self.current_target.and_then(|id| {
            OBJECT_REGISTRY
                .with_object(id, |guard| guard.get_team_id())
                .flatten()
        });
        self.target_position = None;
        self.sync_goal_object();
        self.sync_state_for_target();
    }

    pub fn set_target_position(&mut self, pos: Option<Coord3D>) {
        self.target_position = pos;
        self.current_target = None;
        self.target_kind = if self.target_position.is_some() {
            TurretTargetKind::Position
        } else {
            TurretTargetKind::None
        };
        self.target_was_set_by_idle_mood = false;
        self.is_force_attacking = false;
        self.victim_initial_team = None;
        self.sync_goal_object();
        self.sync_state_for_target();
    }

    pub fn set_state_machine(&mut self, machine: Weak<Mutex<StateMachine>>) {
        self.state_machine = Some(machine);
    }

    fn sync_goal_object(&self) {
        let Some(machine) = self.state_machine.as_ref().and_then(|weak| weak.upgrade()) else {
            return;
        };
        if let Ok(mut guard) = machine.lock() {
            match self.target_kind {
                TurretTargetKind::Object => {
                    guard.set_goal_object_by_id(self.current_target);
                }
                TurretTargetKind::Position => {
                    guard.set_goal_object_by_id(None);
                    if let Some(pos) = &self.target_position {
                        guard.set_goal_position(*pos);
                    }
                }
                TurretTargetKind::None => guard.set_goal_object_by_id(None),
            }
        };
    }

    fn sync_state_for_target(&self) {
        let Some(machine) = self.state_machine.as_ref().and_then(|weak| weak.upgrade()) else {
            return;
        };
        let Ok(mut guard) = machine.lock() else {
            return;
        };
        let current = guard.get_current_state_id();
        let aim_id = TurretStateType::Aim.into();
        let fire_id = TurretStateType::Fire.into();
        let hold_id = TurretStateType::Hold.into();

        if self.current_target.is_some() {
            if current != Some(aim_id) && current != Some(fire_id) {
                let _ = guard.set_current_state(aim_id);
            }
        } else if current == Some(aim_id) || current == Some(fire_id) {
            let _ = guard.set_current_state(hold_id);
        }
    }

    /// Get current angle
    pub fn get_current_angle(&self) -> f32 {
        self.current_angle
    }

    /// Set current angle
    pub fn set_current_angle(&mut self, angle: f32) {
        self.current_angle = angle;
    }

    /// Get current pitch
    pub fn get_current_pitch(&self) -> f32 {
        self.current_pitch
    }

    /// Set current pitch
    pub fn set_current_pitch(&mut self, pitch: f32) {
        self.current_pitch = pitch;
    }

    /// Get natural angle
    pub fn get_natural_angle(&self) -> f32 {
        self.natural_angle
    }

    /// Set natural angle
    pub fn set_natural_angle(&mut self, angle: f32) {
        self.natural_angle = angle;
    }

    /// Get natural pitch
    pub fn get_natural_pitch(&self) -> f32 {
        self.natural_pitch
    }

    /// Set natural pitch
    pub fn set_natural_pitch(&mut self, pitch: f32) {
        self.natural_pitch = pitch;
    }

    /// Get turn rate
    pub fn get_turn_rate(&self) -> f32 {
        self.turn_rate
    }

    /// Set turn rate
    pub fn set_turn_rate(&mut self, rate: f32) {
        self.turn_rate = rate;
    }

    /// Get pitch rate
    pub fn get_pitch_rate(&self) -> f32 {
        self.pitch_rate
    }

    /// Set pitch rate
    pub fn set_pitch_rate(&mut self, rate: f32) {
        self.pitch_rate = rate;
    }

    /// Calculate angle to target
    pub fn calculate_angle_to_target(&self, target_id: ObjectID) -> Option<f32> {
        if self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        let owner_pos = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_ref| *owner_ref.get_position())?;
        let target_pos = crate::object::registry::OBJECT_REGISTRY
            .with_object(target_id, |target_ref| *target_ref.get_position())?;
        let dx = target_pos.x - owner_pos.x;
        let dy = target_pos.y - owner_pos.y;
        Some(dy.atan2(dx))
    }

    /// Calculate pitch to target
    pub fn calculate_pitch_to_target(&self, target_id: ObjectID) -> Option<f32> {
        if self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        let is_aircraft = crate::object::registry::OBJECT_REGISTRY
            .with_object(target_id, |target_ref| {
                target_ref.is_kind_of(KindOf::Aircraft)
            })
            .unwrap_or(false);
        if !is_aircraft && self.ground_unit_pitch != 0.0 {
            return Some(self.ground_unit_pitch);
        }
        let owner_pos = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_ref| *owner_ref.get_position())?;
        let target_pos = crate::object::registry::OBJECT_REGISTRY
            .with_object(target_id, |target_ref| *target_ref.get_position())?;
        let dx = target_pos.x - owner_pos.x;
        let dy = target_pos.y - owner_pos.y;
        let dz = target_pos.z - owner_pos.z;
        let horizontal_dist = (dx * dx + dy * dy).sqrt();
        if horizontal_dist > 0.0 {
            Some(dz.atan2(horizontal_dist))
        } else {
            None
        }
    }

    /// Rotate turret towards desired angle
    pub fn rotate_towards_angle(&mut self, desired_angle: f32) -> bool {
        let angle_diff = Self::normalize_angle(desired_angle - self.current_angle);

        if angle_diff.abs() < self.turn_rate {
            // Close enough, snap to target
            self.current_angle = desired_angle;
            true
        } else {
            // Rotate towards target
            if angle_diff > 0.0 {
                self.current_angle += self.turn_rate;
            } else {
                self.current_angle -= self.turn_rate;
            }
            self.current_angle = Self::normalize_angle(self.current_angle);
            self.play_rot_sound = true;
            false
        }
    }

    /// Rotate turret towards desired angle with speed modifier and threshold.
    pub fn rotate_towards_angle_with_speed(
        &mut self,
        desired_angle: f32,
        speed_modifier: f32,
        threshold: f32,
    ) -> bool {
        let angle_diff = Self::normalize_angle(desired_angle - self.current_angle);
        let rate = self.turn_rate * speed_modifier.max(0.0);

        if angle_diff.abs() < threshold {
            self.current_angle = desired_angle;
            true
        } else if angle_diff.abs() < rate {
            self.current_angle = desired_angle;
            true
        } else {
            if angle_diff > 0.0 {
                self.current_angle += rate;
            } else {
                self.current_angle -= rate;
            }
            self.current_angle = Self::normalize_angle(self.current_angle);
            self.play_rot_sound = true;
            false
        }
    }

    /// Pitch turret towards desired pitch
    pub fn pitch_towards_angle(&mut self, desired_pitch: f32) -> bool {
        let desired_pitch = desired_pitch.max(self.min_pitch);
        let pitch_diff = desired_pitch - self.current_pitch;

        if pitch_diff.abs() < self.pitch_rate {
            // Close enough, snap to target
            self.current_pitch = desired_pitch;
            true
        } else {
            // Pitch towards target
            if pitch_diff > 0.0 {
                self.current_pitch += self.pitch_rate;
            } else {
                self.current_pitch -= self.pitch_rate;
            }
            self.play_pitch_sound = true;
            false
        }
    }

    /// Check if turret is aimed at target
    pub fn is_aimed_at_target(&self, target_id: ObjectID) -> bool {
        if let Some(desired_angle) = self.calculate_angle_to_target(target_id) {
            let angle_diff = Self::normalize_angle(desired_angle - self.current_angle);
            return angle_diff.abs() < self.turn_rate * 2.0; // Allow some tolerance
        }
        false
    }

    /// Check if turret can fire at target
    pub fn can_fire_at_target(&self, target_id: ObjectID) -> bool {
        if self.fires_while_turning {
            return self.is_target_in_weapon_range(target_id);
        }
        self.is_aimed_at_target(target_id) && self.is_target_in_weapon_range(target_id)
    }

    /// Check if target is in weapon range
    pub fn is_target_in_weapon_range(&self, target_id: ObjectID) -> bool {
        if self.owner_id == crate::common::INVALID_ID {
            return false;
        }
        let Some(target_pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(target_id, |guard| *guard.get_position())
        else {
            return false;
        };
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_ref| {
                let Some(weapon) = owner_ref.get_weapon_in_slot(self.weapon_slot) else {
                    return false;
                };
                let max_range = weapon.get_attack_range(owner_ref.get_id());
                let min_range = weapon.get_template().get_minimum_attack_range();
                let owner_pos = *owner_ref.get_position();
                let dx = target_pos.x - owner_pos.x;
                let dy = target_pos.y - owner_pos.y;
                let dist = (dx * dx + dy * dy).sqrt();
                dist >= min_range && dist <= max_range
            })
            .unwrap_or(false)
    }

    /// Check if any turret weapon is within range of target (matches C++ friend_isAnyWeaponInRangeOf)
    pub fn friend_is_any_weapon_in_range_of(&self, target_id: ObjectID) -> bool {
        let Some(target) = crate::helpers::TheGameLogic::find_object_by_id(target_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(target_id))
        else {
            return false;
        };
        let owner_arc = match self.owner_object() {
            Some(owner) => owner,
            None => return false,
        };
        let owner_guard = match owner_arc.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let target_id = match target.read() {
            Ok(guard) => guard.get_id(),
            Err(_) => return false,
        };

        for slot in [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ] {
            if !self.is_weapon_slot_on_turret(slot) {
                continue;
            }
            let Some(weapon) = owner_guard.get_weapon_in_slot(slot) else {
                continue;
            };
            if weapon.is_within_attack_range(owner_guard.get_id(), Some(target_id), None) {
                return true;
            }
        }

        false
    }

    /// Scan for targets within turret's range and arc
    pub fn scan_for_targets(&self) -> Vec<Arc<RwLock<Object>>> {
        let mut targets = Vec::new();

        if !self.can_scan {
            return targets;
        }

        let owner_arc = match self.owner_object() {
            Some(owner) => owner,
            None => return targets,
        };
        let owner_guard = match owner_arc.read() {
            Ok(guard) => guard,
            Err(_) => return targets,
        };
        let Some(weapon) = owner_guard.get_weapon_in_slot(self.weapon_slot) else {
            return targets;
        };
        let range = weapon.get_attack_range(owner_guard.get_id());
        let Some(partition) = ThePartitionManager::get() else {
            return targets;
        };

        for candidate_id in partition.get_objects_in_range(owner_guard.get_position(), range) {
            if candidate_id == owner_guard.get_id() {
                continue;
            }
            let Some(candidate_arc) = get_legacy_object(candidate_id) else {
                continue;
            };
            {
                let Ok(candidate_guard) = candidate_arc.read() else {
                    continue;
                };
                if candidate_guard.is_destroyed() {
                    continue;
                }
                if owner_guard.relationship_to(&candidate_guard) != Relationship::Enemies {
                    continue;
                }
            }
            if let Some(angle_to_target) = self.calculate_angle_to_target(candidate_id) {
                let angle_diff = Self::normalize_angle(angle_to_target - self.natural_angle).abs();
                if angle_diff > self.scan_range {
                    continue;
                }
            }
            if self.is_target_in_weapon_range(candidate_id) {
                targets.push(candidate_arc);
            }
        }

        targets
    }

    /// Find best target from available targets
    pub fn find_best_target(&self, targets: &[Arc<RwLock<Object>>]) -> Option<Arc<RwLock<Object>>> {
        if targets.is_empty() {
            return None;
        }

        // Simple targeting: closest enemy
        let mut best_target: Option<Arc<RwLock<Object>>> = None;
        let mut best_distance_sqr = f32::MAX;

        if self.owner_id != crate::common::INVALID_ID {
            if let Some(owner_pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(self.owner_id, |owner_ref| *owner_ref.get_position())
            {
                for target in targets {
                    if let Ok(target_ref) = target.try_read() {
                        let target_pos = target_ref.get_position();
                        let dist_sqr = owner_pos.distance_sqr(target_pos);

                        if dist_sqr < best_distance_sqr {
                            best_distance_sqr = dist_sqr;
                            best_target = Some(target.clone());
                        }
                    }
                }
            }
        }

        best_target
    }

    /// Normalize angle to -PI to PI range
    fn normalize_angle(angle: f32) -> f32 {
        let mut normalized = angle;
        while normalized > std::f32::consts::PI {
            normalized -= 2.0 * std::f32::consts::PI;
        }
        while normalized < -std::f32::consts::PI {
            normalized += 2.0 * std::f32::consts::PI;
        }
        normalized
    }

    /// Called when state machine changes
    pub fn friend_notify_state_machine_changed(&mut self) {
        self.sleep_until = TheGameLogic::get_frame();
    }

    /// Update turret AI and maintain the C++ sleep/sweep/sound bookkeeping.
    pub fn update_turret_ai(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if self.sleep_until != 0 && now < self.sleep_until {
            return StateReturnType::Sleep(self.sleep_until - now);
        }

        if !self.fires_while_turning || self.continuous_fire_expiration_frame <= now {
            self.play_rot_sound = false;
            self.play_pitch_sound = false;
        }

        if self.enabled {
            self.did_fire = false;
        } else {
            self.sleep_until = now.saturating_add(WAIT_INDEFINITELY);
            return StateReturnType::Sleep(WAIT_INDEFINITELY);
        }

        self.sleep_until = now;
        StateReturnType::Continue
    }

    /// Safe updater for shared turret handles. This drops the turret lock before
    /// running the state machine because states may need to lock the turret.
    pub fn update_turret_ai_handle(turret: &Arc<Mutex<TurretAI>>) -> StateReturnType {
        let machine = {
            let Ok(mut guard) = turret.lock() else {
                return StateReturnType::Failure;
            };
            let now = TheGameLogic::get_frame();
            if guard.sleep_until != 0 && now < guard.sleep_until {
                return StateReturnType::Sleep(guard.sleep_until - now);
            }
            if !guard.fires_while_turning || guard.continuous_fire_expiration_frame <= now {
                guard.play_rot_sound = false;
                guard.play_pitch_sound = false;
            }
            let recentering = guard
                .state_machine
                .as_ref()
                .and_then(|weak| weak.upgrade())
                .and_then(|machine| machine.lock().ok()?.get_current_state_id())
                == Some(TurretStateType::Recenter.into());
            if !guard.enabled && !recentering {
                guard.sleep_until = now.saturating_add(WAIT_INDEFINITELY);
                return StateReturnType::Sleep(WAIT_INDEFINITELY);
            }
            guard.did_fire = false;
            guard.state_machine.as_ref().and_then(|weak| weak.upgrade())
        };

        let state_return = machine
            .map(|machine| {
                machine
                    .lock()
                    .map(|mut guard| guard.update())
                    .unwrap_or(StateReturnType::Failure)
            })
            .unwrap_or(StateReturnType::Continue);

        let Ok(mut guard) = turret.lock() else {
            return StateReturnType::Failure;
        };
        let now = TheGameLogic::get_frame();
        if guard.did_fire {
            const ENABLE_SWEEP_FRAME_COUNT: u32 = 3;
            guard.enable_sweep_until = now.saturating_add(ENABLE_SWEEP_FRAME_COUNT);
            guard.continuous_fire_expiration_frame = now.saturating_add(ENABLE_SWEEP_FRAME_COUNT);
        }
        let sleep_frames = match state_return {
            StateReturnType::Sleep(frames) => frames,
            _ => 0,
        };
        guard.sleep_until = now.saturating_add(sleep_frames);
        state_return
    }

    /// Get hold time
    pub fn get_hold_time(&self) -> u32 {
        self.hold_time
    }

    /// Set hold time
    pub fn set_hold_time(&mut self, time: u32) {
        self.hold_time = time;
    }

    pub fn get_recenter_time(&self) -> u32 {
        self.recenter_time
    }

    pub fn set_recenter_time(&mut self, time: u32) {
        self.recenter_time = time;
    }

    pub fn get_min_idle_scan_interval(&self) -> u32 {
        self.min_idle_scan_interval
    }

    pub fn get_max_idle_scan_interval(&self) -> u32 {
        self.max_idle_scan_interval
    }

    pub fn set_idle_scan_interval_range(&mut self, min: u32, max: u32) {
        self.min_idle_scan_interval = min;
        self.max_idle_scan_interval = max;
    }

    pub fn get_min_idle_scan_angle(&self) -> f32 {
        self.min_idle_scan_angle
    }

    pub fn get_max_idle_scan_angle(&self) -> f32 {
        self.max_idle_scan_angle
    }

    pub fn set_idle_scan_angle_range(&mut self, min: f32, max: f32) {
        self.min_idle_scan_angle = min;
        self.max_idle_scan_angle = max;
    }

    pub fn get_continuous_fire_expiration_frame(&self) -> u32 {
        self.continuous_fire_expiration_frame
    }

    pub fn set_continuous_fire_expiration_frame(&mut self, frame: u32) {
        self.continuous_fire_expiration_frame = frame;
    }

    pub fn get_sleep_until(&self) -> u32 {
        self.sleep_until
    }

    pub fn set_sleep_until(&mut self, frame: u32) {
        self.sleep_until = frame;
    }

    pub fn get_did_fire(&self) -> bool {
        self.did_fire
    }

    pub fn set_did_fire(&mut self, value: bool) {
        self.did_fire = value;
    }

    pub fn get_play_rot_sound(&self) -> bool {
        self.play_rot_sound
    }

    pub fn set_play_rot_sound(&mut self, value: bool) {
        self.play_rot_sound = value;
    }

    pub fn get_play_pitch_sound(&self) -> bool {
        self.play_pitch_sound
    }

    pub fn set_play_pitch_sound(&mut self, value: bool) {
        self.play_pitch_sound = value;
    }

    fn slot_index(slot: WeaponSlotType) -> usize {
        match slot {
            WeaponSlotType::Primary => 0,
            WeaponSlotType::Secondary => 1,
            WeaponSlotType::Tertiary => 2,
        }
    }

    pub fn set_turret_weapon_slots_mask(&mut self, mask: u32) {
        self.turret_weapon_slots_mask = mask;
    }

    pub fn is_weapon_slot_on_turret(&self, slot: WeaponSlotType) -> bool {
        let bit = 1u32 << Self::slot_index(slot);
        (self.turret_weapon_slots_mask & bit) != 0
    }

    pub fn is_owners_cur_weapon_on_turret(&self) -> bool {
        if self.owner_id == crate::common::INVALID_ID {
            return false;
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_guard| {
                owner_guard
                    .get_current_weapon()
                    .map(|(_, slot)| self.is_weapon_slot_on_turret(slot))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    pub fn get_turret_angle(&self) -> f32 {
        self.get_current_angle()
    }

    pub fn get_turret_pitch(&self) -> f32 {
        self.get_current_pitch()
    }

    pub fn is_trying_to_aim_at_target(&self, target: ObjectID) -> bool {
        let has_target = self.current_target == Some(target);
        if !has_target {
            return false;
        }

        let Some(machine) = self.state_machine.as_ref().and_then(|weak| weak.upgrade()) else {
            return false;
        };
        let Ok(machine_guard) = machine.lock() else {
            return false;
        };
        matches!(
            machine_guard.get_current_state_id(),
            Some(state)
                if state == TurretStateType::Aim as u32 || state == TurretStateType::Fire as u32
        )
    }

    pub fn get_turret_fire_angle_sweep_for_weapon_slot(&self, slot: WeaponSlotType) -> f32 {
        self.turret_fire_angle_sweep[Self::slot_index(slot)]
    }

    pub fn set_turret_fire_angle_sweep_for_weapon_slot(
        &mut self,
        slot: WeaponSlotType,
        sweep: f32,
    ) {
        self.turret_fire_angle_sweep[Self::slot_index(slot)] = sweep;
    }

    pub fn get_turret_sweep_speed_modifier_for_weapon_slot(&self, slot: WeaponSlotType) -> f32 {
        self.turret_sweep_speed_modifier[Self::slot_index(slot)]
    }

    pub fn set_turret_sweep_speed_modifier_for_weapon_slot(
        &mut self,
        slot: WeaponSlotType,
        modifier: f32,
    ) {
        self.turret_sweep_speed_modifier[Self::slot_index(slot)] = modifier;
    }

    pub fn set_min_pitch(&mut self, pitch: f32) {
        self.min_pitch = pitch;
    }

    pub fn set_ground_unit_pitch(&mut self, pitch: f32) {
        self.ground_unit_pitch = pitch;
    }

    pub fn set_turret_enabled(&mut self, enabled: bool) {
        if enabled && !self.enabled {
            self.sleep_until = TheGameLogic::get_frame();
        }
        self.enabled = enabled;
    }

    pub fn is_turret_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_initially_disabled(&mut self, disabled: bool) {
        self.initially_disabled = disabled;
    }

    pub fn set_fires_while_turning(&mut self, fires: bool) {
        self.fires_while_turning = fires;
    }

    pub fn set_inter_turret_delay(&mut self, delay: u32) {
        self.inter_turret_delay = delay;
    }

    pub fn get_inter_turret_delay(&self) -> u32 {
        self.inter_turret_delay
    }

    pub fn get_fires_while_turning(&self) -> bool {
        self.fires_while_turning
    }

    pub fn recenter_turret(&mut self) {
        if let Some(machine) = self.state_machine.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(mut guard) = machine.lock() {
                let _ = guard.set_current_state(TurretStateType::Recenter.into());
            }
        }
    }

    pub fn is_turret_in_natural_position(&self) -> bool {
        (self.natural_angle - self.current_angle).abs() < 0.0001
            && (self.natural_pitch - self.current_pitch).abs() < 0.0001
    }

    pub fn friend_is_sweep_enabled(&self) -> bool {
        self.enable_sweep_until != 0 && self.enable_sweep_until > TheGameLogic::get_frame()
    }

    pub fn friend_get_positive_sweep(&self) -> bool {
        self.positive_sweep
    }

    pub fn friend_set_positive_sweep(&mut self, value: bool) {
        self.positive_sweep = value;
    }

    pub fn set_allows_pitch(&mut self, value: bool) {
        self.allows_pitch = value;
    }

    pub fn is_allows_pitch(&self) -> bool {
        self.allows_pitch
    }

    pub fn set_fire_pitch(&mut self, pitch: f32) {
        self.fire_pitch = pitch;
    }

    pub fn get_fire_pitch(&self) -> f32 {
        self.fire_pitch
    }

    /// Next frame to check idle mood target (matches C++ friend_getNextIdleMoodTargetFrame)
    pub fn friend_get_next_idle_mood_target_frame(&self) -> u32 {
        if self.owner_id == crate::common::INVALID_ID {
            return TheGameLogic::get_frame();
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_guard| {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        return ai_guard.get_next_mood_check_time();
                    }
                }
                TheGameLogic::get_frame()
            })
            .unwrap_or_else(TheGameLogic::get_frame)
    }

    /// Check for idle mood target acquisition (matches C++ friend_checkForIdleMoodTarget)
    pub fn friend_check_for_idle_mood_target(&mut self) {
        let owner_arc = match self.owner_object() {
            Some(owner) => owner,
            None => return,
        };
        let owner_guard = match owner_arc.read() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let ai = match owner_guard.get_ai_update_interface() {
            Some(ai) => ai,
            None => return,
        };
        let mut ai_guard = match ai.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let adjustment =
            ai_guard.get_mood_matrix_action_adjustment(crate::ai::MoodMatrixAction::Idle);
        if (adjustment & crate::ai::mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL) != 0 {
            return;
        }
        if let Some(enemy) = ai_guard.get_next_mood_target(true, true) {
            drop(ai_guard);
            drop(owner_guard);
            if let (Ok(mut owner_write), Ok(target_guard)) = (owner_arc.write(), enemy.read()) {
                let _ = owner_write.choose_best_weapon_for_target(
                    &target_guard,
                    WeaponChoiceCriteria::PreferMostDamage,
                    crate::common::CommandSourceType::FromAi,
                );
            }
            let enemy_id = enemy.read().ok().map(|g| g.get_id());
            self.set_current_target_from_idle_mood(enemy_id);
        }
    }
}

/// Turret state machine
fn turret_out_of_weapon_range_object(
    state: &TurretAIFireWeaponState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    let owner = state
        .base_state()
        .get_machine_owner()
        .ok_or_else(|| "turret fire missing owner".to_string())?;
    let target_id = state
        .base_state()
        .get_machine_goal_object_id()
        .ok_or_else(|| "turret fire missing target".to_string())?;
    let owner_guard = owner
        .read()
        .map_err(|_| "turret fire owner lock poisoned".to_string())?;
    let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
        return Ok(false);
    };
    if weapon.has_leech_range() {
        return Ok(false);
    }
    Ok(!weapon.is_within_attack_range(owner_guard.get_id(), Some(target_id), None))
}

pub struct TurretStateMachine {
    /// Base state machine
    base: Arc<Mutex<StateMachine>>,
    /// Reference to turret AI
    turret_ai: Option<Arc<Mutex<TurretAI>>>,
    /// Shared state accessible by individual turret states
    shared: Arc<TurretSharedState>,
}

impl TurretStateMachine {
    pub fn new(
        turret_ai: Option<Arc<Mutex<TurretAI>>>,
        owner: Weak<RwLock<Object>>,
        name: &str,
    ) -> Self {
        let base = Arc::new(Mutex::new(StateMachine::new(Some(owner), name)));
        let shared = Arc::new(TurretSharedState::new(&base, turret_ai.clone()));

        if let Some(ai) = turret_ai.as_ref() {
            if let Ok(mut guard) = ai.lock() {
                guard.set_state_machine(Arc::downgrade(&base));
            }
        }

        let mut machine = Self {
            base,
            turret_ai,
            shared,
        };

        machine.define_turret_states();
        machine
    }

    fn define_turret_states(&mut self) {
        let shared = self.shared.clone();
        let base_arc = self.base.clone();

        let mut base = self
            .base
            .lock()
            .expect("turret state machine lock poisoned");

        register_classic_state(
            &mut *base,
            TurretStateType::Idle.into(),
            TurretAIIdleState::new(&base_arc, shared.clone()),
            Some(TurretStateType::Idle.into()),
            Some(TurretStateType::IdleScan.into()),
            &[],
        );

        register_classic_state(
            &mut *base,
            TurretStateType::IdleScan.into(),
            TurretAIIdleScanState::new(&base_arc, shared.clone()),
            Some(TurretStateType::Hold.into()),
            Some(TurretStateType::Hold.into()),
            &[],
        );

        register_classic_state(
            &mut *base,
            TurretStateType::Aim.into(),
            TurretAIAimTurretState::new(&base_arc, shared.clone()),
            Some(TurretStateType::Fire.into()),
            Some(TurretStateType::Hold.into()),
            &[],
        );

        let fire_conditions = vec![legacy_transition::<TurretAIFireWeaponState>(
            turret_out_of_weapon_range_object,
            TurretStateType::Aim.into(),
            StateTransitionUserData::new(),
            "out_of_weapon_range_object",
        )];

        register_classic_state(
            &mut *base,
            TurretStateType::Fire.into(),
            TurretAIFireWeaponState::new(&base_arc, shared.clone()),
            Some(TurretStateType::Aim.into()),
            Some(TurretStateType::Aim.into()),
            &fire_conditions,
        );

        register_classic_state(
            &mut *base,
            TurretStateType::Recenter.into(),
            TurretAIRecenterTurretState::new(&base_arc, shared.clone()),
            Some(TurretStateType::Idle.into()),
            Some(TurretStateType::Idle.into()),
            &[],
        );

        register_classic_state(
            &mut *base,
            TurretStateType::Hold.into(),
            TurretAIHoldTurretState::new(&base_arc, shared.clone()),
            Some(TurretStateType::Recenter.into()),
            Some(TurretStateType::Recenter.into()),
            &[],
        );

        let _ = base.set_current_state(TurretStateType::Idle.into());
    }

    /// Get turret AI reference
    pub fn get_turret_ai(&self) -> Option<Arc<Mutex<TurretAI>>> {
        self.turret_ai.as_ref().cloned()
    }

    /// Clear state machine
    pub fn clear(&self) {
        if let Ok(mut base) = self.base.lock() {
            base.clear();
        }

        if let Err(err) = self.shared.notify_state_change() {
            warn!("turret state machine clear notify failed: {}", err);
        }
    }

    /// Reset to default state
    pub fn reset_to_default_state(&self) -> StateReturnType {
        let mut result = StateReturnType::Continue;
        if let Ok(mut base) = self.base.lock() {
            result = base.reset_to_default_state();
        }

        if let Err(err) = self.shared.notify_state_change() {
            warn!("turret state machine reset notify failed: {}", err);
        }

        result
    }

    /// Set state
    pub fn set_state(&self, new_state_id: u32) -> StateReturnType {
        let mut notify = false;
        let mut result = StateReturnType::Continue;

        if let Ok(mut base) = self.base.lock() {
            let old_id = base.get_current_state_id();
            result = base.set_current_state(new_state_id);
            if old_id != Some(new_state_id) {
                notify = true;
            }
        }

        if notify {
            if let Err(err) = self.shared.notify_state_change() {
                warn!("turret state machine change notify failed: {}", err);
            }
        }

        result
    }

    /// Update state machine
    pub fn update(&self) -> StateReturnType {
        if let Some(ai) = self.turret_ai.as_ref() {
            if let Ok(guard) = ai.lock() {
                if !guard.is_turret_enabled() {
                    if let Ok(base) = self.base.lock() {
                        if base.get_current_state_id() != Some(TurretStateType::Recenter.into()) {
                            return StateReturnType::Continue;
                        }
                    }
                }
            }
        }
        if let Ok(mut base) = self.base.lock() {
            return base.update();
        }
        StateReturnType::Failure
    }
}

// Turret state implementations

/// Base class for turret states
#[derive(Debug)]
pub struct TurretState {
    base: State,
    shared: Arc<TurretSharedState>,
}

impl TurretState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<TurretSharedState>,
        name: &str,
    ) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), name),
            shared,
        }
    }

    fn change_state(&self, state: TurretStateType) -> Result<(), String> {
        self.shared.change_state(state)
    }

    fn turret_ai_lock(&self) -> Result<Option<Arc<Mutex<TurretAI>>>, String> {
        Ok(self.shared.turret_ai())
    }

    fn state(&self) -> &State {
        &self.base
    }

    fn state_mut(&mut self) -> &mut State {
        &mut self.base
    }
}

/// Idle state - do nothing, wait for targets
#[derive(Debug)]
pub struct TurretAIIdleState {
    base: TurretState,
    next_idle_scan: u32,
}

impl TurretAIIdleState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TurretSharedState>) -> Self {
        Self {
            base: TurretState::new(machine, shared, "TurretAIIdleState"),
            next_idle_scan: 0,
        }
    }

    fn reset_idle_scan(&mut self) -> Result<(), String> {
        let current_frame = TheGameLogic::try_get_frame()?;
        let (min_interval, max_interval) = match self.base.turret_ai_lock()? {
            Some(turret_ai) => turret_ai
                .lock()
                .map(|t| {
                    (
                        t.get_min_idle_scan_interval(),
                        t.get_max_idle_scan_interval(),
                    )
                })
                .unwrap_or((30, 30)),
            None => (30, 30),
        };
        let max_interval = if max_interval < min_interval {
            min_interval
        } else {
            max_interval
        };
        let interval = GameLogicRandomValue(min_interval as i32, max_interval as i32) as u32;
        self.next_idle_scan = current_frame + interval;
        Ok(())
    }
}

impl StateImplementation for TurretAIIdleState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIIdleState xfer version failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_idle_scan)
            .map_err(|e| format!("TurretAIIdleState xfer next_idle_scan failed: {:?}", e))?;
        Ok(())
    }
}

impl ClassicState for TurretAIIdleState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIIdleState xfer version failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_idle_scan)
            .map_err(|e| format!("TurretAIIdleState xfer next_idle_scan failed: {:?}", e))?;
        Ok(())
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Some(owner) = self.base_state().get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.reset_next_mood_check_time();
                    }
                }
            }
        }
        self.reset_idle_scan()?;
        let mut mood_frame = None;
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(turret) = turret_ai.lock() {
                mood_frame = Some(turret.friend_get_next_idle_mood_target_frame());
            }
        }
        Ok(frame_to_sleep_time(
            mood_frame.unwrap_or(self.next_idle_scan),
            Some(self.next_idle_scan),
            None,
            None,
        ))
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let current_frame = TheGameLogic::try_get_frame()?;

        if current_frame >= self.next_idle_scan {
            return Ok(StateReturnType::Failure);
        }

        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(mut turret) = turret_ai.lock() {
                turret.friend_check_for_idle_mood_target();
                let mood_frame = turret.friend_get_next_idle_mood_target_frame();
                return Ok(frame_to_sleep_time(
                    mood_frame,
                    Some(self.next_idle_scan),
                    None,
                    None,
                ));
            }
        }

        Ok(frame_to_sleep_time(self.next_idle_scan, None, None, None))
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_is_idle(&self) -> bool {
        true
    }
}

/// Idle scan state - slowly rotate turret looking for targets
#[derive(Debug)]
pub struct TurretAIIdleScanState {
    base: TurretState,
    desired_angle: f32,
}

impl TurretAIIdleScanState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TurretSharedState>) -> Self {
        Self {
            base: TurretState::new(machine, shared, "TurretAIIdleScanState"),
            desired_angle: 0.0,
        }
    }
}

impl StateImplementation for TurretAIIdleScanState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIIdleScanState xfer version failed: {:?}", e))?;
        xfer.xfer_real(&mut self.desired_angle)
            .map_err(|e| format!("TurretAIIdleScanState xfer desired_angle failed: {:?}", e))?;
        Ok(())
    }
}

impl ClassicState for TurretAIIdleScanState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIIdleScanState xfer version failed: {:?}", e))?;
        xfer.xfer_real(&mut self.desired_angle)
            .map_err(|e| format!("TurretAIIdleScanState xfer desired_angle failed: {:?}", e))?;
        Ok(())
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(turret) = turret_ai.lock() {
                let min_angle = turret.get_min_idle_scan_angle();
                let max_angle = turret.get_max_idle_scan_angle();
                if min_angle == 0.0 && max_angle == 0.0 {
                    return Ok(StateReturnType::Success);
                }
                let mut offset =
                    min_angle + GameLogicRandomValueReal(0.0, (max_angle - min_angle).max(0.0));
                if GameLogicRandomValue(0, 1) == 0 {
                    offset = -offset;
                }
                self.desired_angle = turret.get_natural_angle() + offset;
            }
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Some(owner) = self.base_state().get_machine_owner() {
            if let Ok(owner) = owner.read() {
                if owner
                    .get_status_bits()
                    .test(ObjectStatusTypes::UnderConstruction)
                {
                    return Ok(StateReturnType::Continue);
                }
            }
        }
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(mut turret) = turret_ai.lock() {
                let angle_aligned = turret.rotate_towards_angle(self.desired_angle);
                let natural_pitch = turret.get_natural_pitch();
                let pitch_aligned = turret.pitch_towards_angle(natural_pitch);
                if angle_aligned && pitch_aligned {
                    return Ok(StateReturnType::Success);
                }
            }
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Aim turret state - rotate turret to aim at target
#[derive(Debug)]
pub struct TurretAIAimTurretState {
    base: TurretState,
    delay_until: u32,
}

impl TurretAIAimTurretState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TurretSharedState>) -> Self {
        Self {
            base: TurretState::new(machine, shared, "TurretAIAimTurretState"),
            delay_until: 0,
        }
    }
}

impl StateImplementation for TurretAIAimTurretState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for TurretAIAimTurretState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.delay_until = 0;
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(turret) = turret_ai.lock() {
                let delay = turret.get_inter_turret_delay();
                if delay > 0 {
                    self.delay_until = TheGameLogic::get_frame().saturating_add(delay);
                }
            }
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.delay_until > 0 {
            let now = TheGameLogic::get_frame();
            if now < self.delay_until {
                return Ok(StateReturnType::Sleep(self.delay_until - now));
            }
            self.delay_until = 0;
        }
        let mut next_state = None;
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(mut turret) = turret_ai.lock() {
                if let Some(target) = turret.get_current_target() {
                    let target_dead = target
                        .try_read()
                        .map(|guard| guard.is_effectively_dead())
                        .unwrap_or(false);

                    if target_dead {
                        turret.set_current_target(None);
                        next_state = Some(TurretStateType::Hold);
                    } else if let Some(owner_arc) = turret.owner_object() {
                        let (rel, is_primary_enemy, can_attack, can_attack_target, team_changed) =
                            match (owner_arc.read(), target.read()) {
                                (Ok(owner_guard), Ok(target_guard)) => {
                                    let rel = owner_guard.relationship_to(&target_guard);
                                    let can_attack = owner_guard.is_able_to_attack();
                                    let team_changed = match turret.victim_initial_team {
                                        Some(team_id) => {
                                            Some(team_id) != target_guard.get_team_id()
                                        }
                                        None => false,
                                    };
                                    let can_attack_target = {
                                        let attack_type = if turret.is_force_attacking() {
                                            AbleToAttackType::ContinuedTargetForced
                                        } else if turret.target_was_set_by_idle_mood() {
                                            AbleToAttackType::ContinuedTarget
                                        } else {
                                            AbleToAttackType::NewTarget
                                        };
                                        matches!(
                                            owner_guard.get_able_to_attack_specific_object(
                                                attack_type,
                                                &target_guard,
                                                CommandSourceType::FromAi,
                                            ),
                                            CanAttackResult::Possible
                                                | CanAttackResult::PossibleAfterMoving
                                        )
                                    };
                                    let mut primary = false;
                                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                                        if let Ok(ai_guard) = ai.lock() {
                                            primary = ai_guard.get_goal_object_id()
                                                == target_guard.get_id();
                                        }
                                    }
                                    (rel, primary, can_attack, can_attack_target, team_changed)
                                }
                                _ => (Relationship::Neutral, false, false, false, false),
                            };
                        if rel != Relationship::Enemies
                            || !can_attack
                            || !can_attack_target
                            || team_changed
                        {
                            turret.set_current_target(None);
                            next_state = Some(TurretStateType::Hold);
                        } else if !is_primary_enemy
                            && !turret.friend_is_any_weapon_in_range_of(
                                target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                            )
                        {
                            turret.set_current_target(None);
                            next_state = Some(TurretStateType::Hold);
                        }
                    } else if let Some(rel_angle) = turret.calculate_angle_to_target(
                        target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                    ) {
                        let can_fire = turret.can_fire_at_target(
                            target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                        );
                        let mut aim_angle = rel_angle;
                        let mut turn_speed_modifier = 1.0f32;
                        let sweep =
                            turret.get_turret_fire_angle_sweep_for_weapon_slot(turret.weapon_slot);

                        if sweep > 0.0 && turret.friend_is_sweep_enabled() {
                            if turret.friend_get_positive_sweep() {
                                aim_angle += sweep;
                            } else {
                                aim_angle -= sweep;
                            }
                            turn_speed_modifier = turret
                                .get_turret_sweep_speed_modifier_for_weapon_slot(
                                    turret.weapon_slot,
                                );
                        }

                        const REL_THRESH: f32 = 0.035;
                        let mut turn_aligned = turret.rotate_towards_angle_with_speed(
                            aim_angle,
                            turn_speed_modifier,
                            REL_THRESH,
                        );

                        if sweep > 0.0 {
                            if turn_aligned && turret.friend_is_sweep_enabled() {
                                let next = !turret.friend_get_positive_sweep();
                                turret.friend_set_positive_sweep(next);
                            }
                            let angle_diff =
                                TurretAI::normalize_angle(rel_angle - turret.get_current_angle());
                            turn_aligned = angle_diff.abs() < sweep;
                        }

                        let mut pitch_aligned = true;
                        if turret.is_allows_pitch() {
                            let desired_pitch = if turret.get_fire_pitch() > 0.0 {
                                turret.get_fire_pitch()
                            } else {
                                turret
                                    .calculate_pitch_to_target(
                                        target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                                    )
                                    .unwrap_or(turret.get_natural_pitch())
                            };
                            pitch_aligned = turret.pitch_towards_angle(desired_pitch);
                        }

                        let fires_while_turning = turret.get_fires_while_turning();
                        if (turn_aligned && pitch_aligned && can_fire)
                            || (fires_while_turning && can_fire)
                        {
                            next_state = Some(TurretStateType::Fire);
                        } else if !can_fire && turret.target_was_set_by_idle_mood() {
                            turret.set_current_target(None);
                            next_state = Some(TurretStateType::Hold);
                        }
                    }
                } else {
                    next_state = Some(TurretStateType::Hold);
                }
            }
        }

        if let Some(state) = next_state {
            self.base.change_state(state)?;
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.delay_until = 0;
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Fire weapon state - fire at target
#[derive(Debug)]
pub struct TurretAIFireWeaponState {
    base: TurretState,
}

impl TurretAIFireWeaponState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TurretSharedState>) -> Self {
        Self {
            base: TurretState::new(machine, shared, "TurretAIFireWeaponState"),
        }
    }
}

impl StateImplementation for TurretAIFireWeaponState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for TurretAIFireWeaponState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let mut next_state = None;

        // Get turret AI and current target
        if let Some(turret_ai_arc) = self.base.turret_ai_lock()? {
            let turret_guard = turret_ai_arc
                .lock()
                .map_err(|_| "turret AI lock poisoned")?;
            let target_opt = turret_guard.get_current_target();
            let weapon_slot = turret_guard.weapon_slot;
            drop(turret_guard);

            if let Some(target) = target_opt {
                // Check if target is still valid
                let target_dead = target
                    .try_read()
                    .map(|guard| guard.is_effectively_dead())
                    .unwrap_or(false);

                if target_dead {
                    // Target is dead, clear it and transition to Hold
                    if let Ok(mut turret) = turret_ai_arc.lock() {
                        turret.set_current_target(None);
                    }
                    next_state = Some(TurretStateType::Hold);
                } else {
                    // Target is alive, try to fire
                    // Get owner object from state machine
                    if let Some(owner_arc) = self.base.state().get_machine_owner() {
                        // Check if we can fire at target
                        let can_fire = turret_ai_arc
                            .lock()
                            .map(|t| {
                                t.can_fire_at_target(
                                    target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                                )
                            })
                            .unwrap_or(false);

                        if can_fire {
                            // Fire weapon - matches C++ AIAttackFireWeaponState::update() from AIStates.cpp:5169
                            if let Ok(mut owner_guard) = owner_arc.try_write() {
                                // Temporarily take weapon_set to avoid aliasing issues
                                let mut weapon_set = std::mem::take(&mut owner_guard.weapon_set);

                                // Get weapon from slot
                                // Convert turret::WeaponSlotType to weapon::WeaponSlotType
                                let weapon_slot_converted = match weapon_slot {
                                    WeaponSlotType::Primary => {
                                        crate::weapon::WeaponSlotType::Primary
                                    }
                                    WeaponSlotType::Secondary => {
                                        crate::weapon::WeaponSlotType::Secondary
                                    }
                                    WeaponSlotType::Tertiary => {
                                        crate::weapon::WeaponSlotType::Tertiary
                                    }
                                };
                                if let Some(weapon) =
                                    weapon_set.get_weapon_in_slot_mut(weapon_slot_converted)
                                {
                                    // Check weapon status - matches C++ line 5189-5197
                                    let weapon_status = weapon.get_status();

                                    if weapon_status == crate::weapon::WeaponStatus::PreAttack {
                                        // Still in pre-attack delay, continue waiting
                                        // Restore weapon_set before returning
                                        owner_guard.weapon_set = weapon_set;
                                        return Ok(StateReturnType::Continue);
                                    } else if weapon_status
                                        == crate::weapon::WeaponStatus::ReadyToFire
                                    {
                                        // Weapon is ready, fire it - matches C++ line 5221
                                        let target_id =
                                            target.try_read().map(|t| t.get_id()).unwrap_or(0);
                                        let source_id = owner_guard.get_id();

                                        // Fire weapon at target
                                        match weapon.fire_weapon_at_object(source_id, target_id) {
                                            Ok(_) => {
                                                // Weapon fired successfully
                                                // Restore weapon_set
                                                owner_guard.weapon_set = weapon_set;

                                                // Notify turret AI that we fired
                                                // This matches C++ TurretAI::notifyFired() from TurretAI.cpp:462
                                                if let Ok(mut turret_guard) = turret_ai_arc.lock() {
                                                    turret_guard.set_did_fire(true);
                                                }
                                                drop(owner_guard);

                                                // Transition back to Aim state to continue tracking
                                                // Matches C++ state transition in AIAttackFireWeaponState
                                                next_state = Some(TurretStateType::Aim);
                                            }
                                            Err(e) => {
                                                // Fire failed, restore weapon_set and transition to Aim
                                                owner_guard.weapon_set = weapon_set;
                                                warn!("Turret weapon fire failed: {}", e);
                                                next_state = Some(TurretStateType::Aim);
                                            }
                                        }
                                    } else {
                                        // Weapon not ready (reloading, out of ammo, etc.)
                                        // Restore weapon_set and transition to Aim
                                        owner_guard.weapon_set = weapon_set;
                                        next_state = Some(TurretStateType::Aim);
                                    }
                                } else {
                                    // No weapon in slot, restore weapon_set and transition to Hold
                                    owner_guard.weapon_set = weapon_set;
                                    next_state = Some(TurretStateType::Hold);
                                }
                            } else {
                                // Could not lock owner, transition to Aim to retry
                                next_state = Some(TurretStateType::Aim);
                            }
                        } else {
                            // Can't fire (out of range, not aimed, etc.), transition to Aim
                            next_state = Some(TurretStateType::Aim);
                        }
                    } else {
                        // No owner object, transition to Hold
                        next_state = Some(TurretStateType::Hold);
                    }
                }
            } else {
                // No target, transition to Hold
                next_state = Some(TurretStateType::Hold);
            }
        }

        if let Some(state) = next_state {
            self.base.change_state(state)?;
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Recenter turret state - rotate back to natural position
#[derive(Debug)]
pub struct TurretAIRecenterTurretState {
    base: TurretState,
}

impl TurretAIRecenterTurretState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TurretSharedState>) -> Self {
        Self {
            base: TurretState::new(machine, shared, "TurretAIRecenterTurretState"),
        }
    }
}

impl StateImplementation for TurretAIRecenterTurretState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for TurretAIRecenterTurretState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let mut next_state = None;
        if let Some(owner) = self.base_state().get_machine_owner() {
            if let Ok(owner) = owner.read() {
                if owner
                    .get_status_bits()
                    .test(ObjectStatusTypes::UnderConstruction)
                {
                    return Ok(StateReturnType::Continue);
                }
            }
        }
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(mut turret) = turret_ai.lock() {
                let natural_angle = turret.get_natural_angle();
                let angle_aligned = turret.rotate_towards_angle(natural_angle);
                let natural_pitch = turret.get_natural_pitch();
                let pitch_aligned = turret.pitch_towards_angle(natural_pitch);
                if angle_aligned && pitch_aligned {
                    next_state = Some(TurretStateType::Idle);
                }
            }
        }

        if let Some(state) = next_state {
            self.base.change_state(state)?;
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Hold turret state - hold position before recentering
#[derive(Debug)]
pub struct TurretAIHoldTurretState {
    base: TurretState,
    timestamp: u32,
}

impl TurretAIHoldTurretState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TurretSharedState>) -> Self {
        Self {
            base: TurretState::new(machine, shared, "TurretAIHoldTurretState"),
            timestamp: 0,
        }
    }
}

impl StateImplementation for TurretAIHoldTurretState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIHoldTurretState xfer version failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.timestamp)
            .map_err(|e| format!("TurretAIHoldTurretState xfer timestamp failed: {:?}", e))?;
        Ok(())
    }
}

impl ClassicState for TurretAIHoldTurretState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIHoldTurretState xfer version failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.timestamp)
            .map_err(|e| format!("TurretAIHoldTurretState xfer timestamp failed: {:?}", e))?;
        Ok(())
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let current_frame = TheGameLogic::try_get_frame()?;
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(turret) = turret_ai.lock() {
                self.timestamp = current_frame.saturating_add(turret.get_recenter_time());
                return Ok(frame_to_sleep_time(
                    turret.friend_get_next_idle_mood_target_frame(),
                    Some(self.timestamp),
                    None,
                    None,
                ));
            }
        }
        self.timestamp = current_frame;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let current_frame = TheGameLogic::try_get_frame()?;

        let mut next_state = None;
        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(mut turret) = turret_ai.lock() {
                turret.friend_check_for_idle_mood_target();
                if turret.get_current_target().is_some() {
                    next_state = Some(TurretStateType::Aim);
                } else if current_frame >= self.timestamp {
                    next_state = Some(TurretStateType::Recenter);
                }
            }
        }

        if let Some(state) = next_state {
            self.base.change_state(state)?;
        }

        if let Some(turret_ai) = self.base.turret_ai_lock()? {
            if let Ok(turret) = turret_ai.lock() {
                return Ok(frame_to_sleep_time(
                    turret.friend_get_next_idle_mood_target_frame(),
                    Some(self.timestamp),
                    None,
                    None,
                ));
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

impl Snapshotable for TurretAI {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("TurretAI version crc failed: {:?}", e))?;
        if let Some(machine_weak) = &self.state_machine {
            if let Some(machine) = machine_weak.upgrade() {
                let mut guard = machine
                    .lock()
                    .map_err(|_| "TurretAI state machine lock poisoned".to_string())?;
                guard.crc(xfer).map_err(|e| e.to_string())?;
            }
        }
        let mut current_angle = self.current_angle;
        xfer.xfer_real(&mut current_angle)
            .map_err(|e| format!("TurretAI current_angle crc failed: {:?}", e))?;
        let mut current_pitch = self.current_pitch;
        xfer.xfer_real(&mut current_pitch)
            .map_err(|e| format!("TurretAI current_pitch crc failed: {:?}", e))?;
        let mut enable_sweep_until = self.enable_sweep_until;
        xfer.xfer_unsigned_int(&mut enable_sweep_until)
            .map_err(|e| format!("TurretAI enable_sweep_until crc failed: {:?}", e))?;
        let mut target_kind_val = match self.target_kind {
            TurretTargetKind::None => 0u32,
            TurretTargetKind::Object => 1u32,
            TurretTargetKind::Position => 2u32,
        };
        xfer.xfer_unsigned_int(&mut target_kind_val)
            .map_err(|e| format!("TurretAI target_kind crc failed: {:?}", e))?;
        let mut continuous_fire_expiration_frame = self.continuous_fire_expiration_frame;
        xfer.xfer_unsigned_int(&mut continuous_fire_expiration_frame)
            .map_err(|e| format!("TurretAI continuous_fire_expiration crc failed: {:?}", e))?;
        let mut play_rot_sound = self.play_rot_sound;
        xfer.xfer_bool(&mut play_rot_sound)
            .map_err(|e| format!("TurretAI play_rot_sound crc failed: {:?}", e))?;
        let mut play_pitch_sound = self.play_pitch_sound;
        xfer.xfer_bool(&mut play_pitch_sound)
            .map_err(|e| format!("TurretAI play_pitch_sound crc failed: {:?}", e))?;
        let mut positive_sweep = self.positive_sweep;
        xfer.xfer_bool(&mut positive_sweep)
            .map_err(|e| format!("TurretAI positive_sweep crc failed: {:?}", e))?;
        let mut did_fire = self.did_fire;
        xfer.xfer_bool(&mut did_fire)
            .map_err(|e| format!("TurretAI did_fire crc failed: {:?}", e))?;
        Ok(())
    }

    /// Serialize/deserialize TurretAI state
    /// Matches C++ TurretAI::xfer (version 2)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("TurretAI version xfer failed: {:?}", e))?;

        // C++ line 332: xferSnapshot(m_turretStateMachine)
        if let Some(machine_weak) = &self.state_machine {
            if let Some(machine) = machine_weak.upgrade() {
                let mut guard = machine
                    .lock()
                    .map_err(|_| "TurretAI state machine lock poisoned".to_string())?;
                guard.xfer(xfer).map_err(|e| e.to_string())?;
            }
        }

        // C++ line 334: xferReal(&m_angle)
        xfer.xfer_real(&mut self.current_angle)
            .map_err(|e| format!("TurretAI current_angle xfer failed: {:?}", e))?;

        // C++ line 335: xferReal(&m_pitch)
        xfer.xfer_real(&mut self.current_pitch)
            .map_err(|e| format!("TurretAI current_pitch xfer failed: {:?}", e))?;

        // C++ line 336: xferUnsignedInt(&m_enableSweepUntil)
        xfer.xfer_unsigned_int(&mut self.enable_sweep_until)
            .map_err(|e| format!("TurretAI enable_sweep_until xfer failed: {:?}", e))?;

        // C++ line 338: xferUser(&m_target, sizeof(m_target))
        let mut target_kind_val = match self.target_kind {
            TurretTargetKind::None => 0u32,
            TurretTargetKind::Object => 1u32,
            TurretTargetKind::Position => 2u32,
        };
        xfer.xfer_unsigned_int(&mut target_kind_val)
            .map_err(|e| format!("TurretAI target_kind xfer failed: {:?}", e))?;
        if xfer.is_loading() {
            self.target_kind = match target_kind_val {
                0 => TurretTargetKind::None,
                1 => TurretTargetKind::Object,
                2 => TurretTargetKind::Position,
                _ => TurretTargetKind::None,
            };
        }

        // C++ line 339: xferUnsignedInt(&m_continuousFireExpirationFrame)
        xfer.xfer_unsigned_int(&mut self.continuous_fire_expiration_frame)
            .map_err(|e| format!("TurretAI continuous_fire_expiration xfer failed: {:?}", e))?;

        // C++ lines 341-348: 7 Bool fields via UNPACK_AND_XFER macro
        // m_playRotSound
        xfer.xfer_bool(&mut self.play_rot_sound)
            .map_err(|e| format!("TurretAI play_rot_sound xfer failed: {:?}", e))?;

        // m_playPitchSound
        xfer.xfer_bool(&mut self.play_pitch_sound)
            .map_err(|e| format!("TurretAI play_pitch_sound xfer failed: {:?}", e))?;

        // m_positiveSweep
        xfer.xfer_bool(&mut self.positive_sweep)
            .map_err(|e| format!("TurretAI positive_sweep xfer failed: {:?}", e))?;

        // m_didFire
        xfer.xfer_bool(&mut self.did_fire)
            .map_err(|e| format!("TurretAI did_fire xfer failed: {:?}", e))?;

        // m_enabled
        xfer.xfer_bool(&mut self.enabled)
            .map_err(|e| format!("TurretAI enabled xfer failed: {:?}", e))?;

        // m_firesWhileTurning
        xfer.xfer_bool(&mut self.fires_while_turning)
            .map_err(|e| format!("TurretAI fires_while_turning xfer failed: {:?}", e))?;

        // m_targetWasSetByIdleMood
        xfer.xfer_bool(&mut self.target_was_set_by_idle_mood)
            .map_err(|e| format!("TurretAI target_was_set_by_idle_mood xfer failed: {:?}", e))?;

        // C++ line 351-352: version >= 2: xferUnsignedInt(&m_sleepUntil)
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.sleep_until)
                .map_err(|e| format!("TurretAI sleep_until xfer failed: {:?}", e))?;
        }

        Ok(())
    }

    /// Post-load processing
    /// Matches C++ TurretAI::loadPostProcess
    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ TurretAI.cpp line 359-364: captures victim initial team
        // The turret state machine's goal object is the victim
        if self.target_kind == TurretTargetKind::Object {
            if let Some(target_id) = self.current_target {
                self.victim_initial_team = OBJECT_REGISTRY
                    .with_object(target_id, |guard| guard.get_team_id())
                    .flatten();
            }
        }
        Ok(())
    }
}

/// Helper function to calculate frame sleep time
pub fn frame_to_sleep_time(
    frame1: u32,
    frame2: Option<u32>,
    frame3: Option<u32>,
    frame4: Option<u32>,
) -> StateReturnType {
    let mut min_frame = frame1;

    if let Some(f2) = frame2 {
        min_frame = min_frame.min(f2);
    }
    if let Some(f3) = frame3 {
        min_frame = min_frame.min(f3);
    }
    if let Some(f4) = frame4 {
        min_frame = min_frame.min(f4);
    }

    let current_frame = TheGameLogic::get_frame();

    if min_frame > current_frame {
        StateReturnType::Sleep(min_frame - current_frame)
    } else {
        StateReturnType::Continue
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretTargetKind {
    None,
    Object,
    Position,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_turret() -> TurretAI {
        TurretAI::new(Weak::new())
    }

    #[test]
    fn turret_defaults_match_cpp_runtime_fields() {
        let turret = test_turret();

        assert_eq!(turret.get_continuous_fire_expiration_frame(), u32::MAX);
        assert_eq!(turret.get_sleep_until(), 0);
        assert!(!turret.get_play_rot_sound());
        assert!(!turret.get_play_pitch_sound());
        assert!(!turret.get_did_fire());
    }

    #[test]
    fn turret_rotation_and_pitch_set_sound_flags_when_moving() {
        let mut turret = test_turret();
        turret.set_turn_rate(0.1);
        turret.set_pitch_rate(0.1);
        turret.set_allows_pitch(true);

        assert!(!turret.rotate_towards_angle(1.0));
        assert!(turret.get_play_rot_sound());

        assert!(!turret.pitch_towards_angle(1.0));
        assert!(turret.get_play_pitch_sound());
    }

    #[test]
    fn turret_wake_and_sleep_bookkeeping_matches_cpp_fields() {
        let mut turret = test_turret();
        let now = TheGameLogic::get_frame();

        turret.set_sleep_until(now.saturating_add(5));
        assert_eq!(turret.update_turret_ai(), StateReturnType::Sleep(5));

        turret.friend_notify_state_machine_changed();
        assert_eq!(turret.get_sleep_until(), now);

        turret.set_turret_enabled(false);
        turret.set_turret_enabled(true);
        assert_eq!(turret.get_sleep_until(), now);
    }
}
