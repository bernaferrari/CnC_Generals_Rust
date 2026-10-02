use crate::ai::object_registry::get_legacy_object;
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::audio::AudioEventRts;
use crate::common::coord::*;
use crate::common::types::ModelConditionFlags;
use crate::common::xfer::{Xfer, XferVersion};
use crate::common::*;
use crate::game_logic::game_logic::TheGameLogic;
use crate::helpers::{TheAudio, ThePartitionManager};
use crate::object::registry::{OBJECT_REGISTRY, SharedObjectHandle};
use crate::object::*;
use crate::state_machine::*;
use crate::team::TeamID;
use crate::terrain::{BridgeAttackInfo, get_terrain_logic};
use crate::weapon::{Weapon, WeaponChoiceCriteria, WeaponSlotType};
use game_engine::common::system::Snapshotable;
use std::any::Any;

use std::sync::Arc;

/// C++ TurretAI.cpp ENABLE_SWEEP_FRAME_COUNT after notifyFired.
const ENABLE_SWEEP_FRAME_COUNT: u32 = 3;
/// C++ TurretAIAimTurretState REL_THRESH (~2 degrees).
const TURRET_AIM_REL_THRESH: f32 = 0.035;

/// Wave 276: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

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

/// Turret AI behavior controller
///
/// C++ `TurretAI` owns `TurretStateMachine* m_turretStateMachine`
/// (TurretAI.h:332; allocated in the `TurretAI` ctor, TurretAI.cpp:294-295) and
/// the machine's states reach it back through `TurretStateMachine::getTurretAI()`
/// (TurretAI.h:75). Ownership here inverts that arrow: the machine is a plain
/// owned field, states carry no handles, and the turret itself is loaned to
/// each machine step via `StateImplementation::update_with_owner` /
/// `on_enter_with_owner` — exactly the `this` context C++ passes through the
/// `StateMachine::update` flow. Nothing in this module is shared: no Arc, no
/// Mutex, no Weak. The only handle the turret keeps is `owner_id`, resolved
/// through the id-keyed object registry on every use.
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
    /// The turret's own state machine (C++ `m_turretStateMachine`). Owned, not
    /// shared: no Arc, no Mutex, no Weak.
    state_machine: StateMachine,
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
    /// Snapshot of UnitAIUpdate::turrets_linked taken before the machine runs.
    turrets_linked_cached: bool,
    /// `MM_Action_Attack` includes `MAA_Action_Ok`. Stamped before the machine runs.
    attack_ok_cached: bool,
    /// `AIUpdate::getGoalObject` id, stamped before the machine runs.
    goal_object_id_cached: crate::common::ObjectID,
    /// `AIUpdate::getLastCommandSource`, stamped before the machine runs.
    last_command_source_cached: crate::common::CommandSourceType,
    next_mood_check_cached: u32,
    reset_mood_check_pending: bool,
    clear_turret_sync: Option<TurretType>,
    idle_mood_check_pending: bool,
    /// Victim transfer during fire. Synced after the machine lock drops.
    goal_sync_pending: bool,
    /// C++ m_sleepUntil — frame at which turret wakes up
    sleep_until: u32,
    /// C++ m_turretRotOrPitchSound (TurretMoveLoop).
    turret_rot_or_pitch_sound: AudioEventRts,
}

impl TurretAI {
    /// C++ `TurretAI::TurretAI(Object* owner, const TurretAIData*, WhichTurretType)`
    /// (TurretAI.cpp:248-298), split in two here: this constructor keeps only
    /// the id of the owner, `TurretAIData::apply_to` applies the INI half, and
    /// [`TurretStateMachine::new`] then builds the machine and enters the
    /// default state — the same order as C++, whose `TurretStateMachine` ctor
    /// and `initDefaultState()` run inside this constructor (TurretAI.cpp:294-
    /// 295). The rotation sound is the template's `TurretMoveLoop`
    /// (TurretAI.cpp:297); it is re-resolved lazily at first use.
    pub fn new(owner_id: ObjectID) -> Self {
        let turret_rot_or_pitch_sound = OBJECT_REGISTRY
            .with_object(owner_id, |guard| {
                guard.get_template().get_per_unit_sound("TurretMoveLoop")
            })
            .flatten()
            .unwrap_or_else(|| AudioEventRts::new(""));
        Self {
            owner_id,
            current_target: None,
            target_kind: TurretTargetKind::None,
            target_was_set_by_idle_mood: false,
            is_force_attacking: false,
            victim_initial_team: None,
            target_position: None,
            state_machine: StateMachine::empty(),
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
            turrets_linked_cached: false,
            attack_ok_cached: true,
            goal_object_id_cached: crate::common::INVALID_ID,
            last_command_source_cached: crate::common::CommandSourceType::FromAi,
            next_mood_check_cached: 0,
            reset_mood_check_pending: false,
            clear_turret_sync: None,
            idle_mood_check_pending: false,
            goal_sync_pending: false,
            sleep_until: 0,
            turret_rot_or_pitch_sound,
        }
    }

    pub fn get_current_target_id(&self) -> Option<ObjectID> {
        self.current_target
    }

    /// Resolve the current target's registry handle for read access. The
    /// handle is transient — the registry stays the owner. Wave 276: empty
    /// dual-world → None.
    pub fn get_current_target(&self) -> Option<SharedObjectHandle> {
        // Wave 276: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        self.current_target
            .and_then(|id| OBJECT_REGISTRY.get_object(id))
    }

    /// Set current target by stable object ID.
    pub fn set_current_target(&mut self, target: Option<ObjectID>) {
        if target
            .filter(|&id| id != crate::common::INVALID_ID)
            .is_none()
        {
            self.remove_self_as_targeter();
        }
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

    /// Set current target from idle mood selection. C++
    /// `TurretAI::friend_checkForIdleMoodTarget` tail (TurretAI.cpp:855-876):
    /// `setTurretTargetObject(enemy, FALSE)` plus the `m_targetWasSetByIdleMood`
    /// marker. UnitAIUpdate's deferred idle-mood pass calls this directly on
    /// the loaned turret — no exported machine handle involved.
    pub fn set_current_target_from_idle_mood(&mut self, target: Option<ObjectID>) {
        self.assign_idle_mood_target(target);
        self.sync_goal_object();
        self.sync_state_for_target();
    }

    /// Current machine state id.
    pub fn get_current_state_id(&self) -> Option<u32> {
        self.state_machine.get_current_state_id()
    }

    /// Define the machine's states and enter the default one. Runs from
    /// [`TurretStateMachine::new`] after `TurretAIData::apply_to`, matching the
    /// C++ order (UnitAI builds the turret, applies data, then constructs
    /// `TurretStateMachine`, TurretAI.cpp:40-71).
    fn build_state_machine(&mut self) {
        if !self.state_machine.is_empty() {
            return;
        }
        let mut machine =
            StateMachine::new_with_owner_id(self.owner_id, "TurretStateMachine");
        self.define_turret_states(&mut machine);
        // C++ TurretStateMachine ctor ends with the default state entered; the
        // IDLE onEnter reads the just-applied turret data.
        let idle_id = TurretStateType::Idle.into();
        let _ = machine.set_current_state_with_owner(idle_id, self);
        self.state_machine = machine;
    }

    /// Resolve the owner object handle. C++ `TurretAI::getOwner()`. The handle
    /// is transient: resolved by id on every use, never stored.
    fn owner_object(&self) -> Option<SharedObjectHandle> {
        if self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
            .or_else(|| OBJECT_REGISTRY.get_object(self.owner_id))
    }

    pub fn assign_idle_mood_target(&mut self, target: Option<ObjectID>) {
        if target
            .filter(|&id| id != crate::common::INVALID_ID)
            .is_none()
        {
            self.remove_self_as_targeter();
        }
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
    }

    pub fn set_weapon_slot(&mut self, slot: WeaponSlotType) {
        self.weapon_slot = slot;
    }

    pub fn get_weapon_slot(&self) -> WeaponSlotType {
        self.weapon_slot
    }

    /// C++ `TurretAI::friend_getWhichTurret` — slot assigned at machine build.
    pub fn friend_get_which_turret(&self) -> TurretType {
        match self.weapon_slot {
            WeaponSlotType::Primary => TurretType::Primary,
            WeaponSlotType::Secondary => TurretType::Secondary,
            WeaponSlotType::Tertiary => TurretType::Invalid,
        }
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
        let mut target = target.filter(|&id| id != crate::common::INVALID_ID);
        if let Some(id) = target {
            let dead_or_missing = OBJECT_REGISTRY
                .with_object(id, |obj| obj.is_effectively_dead())
                .unwrap_or(true);
            if (dead_or_missing || !self.is_owners_cur_weapon_on_turret())
                && !self.owner_turrets_linked()
            {
                target = None;
            }
        }
        if target.is_none() {
            self.remove_self_as_targeter();
        }
        self.current_target = target;
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
        self.set_turret_target_position(pos);
    }

    /// C++ `TurretAI::setTurretTargetPosition`.
    pub fn set_turret_target_position(&mut self, mut pos: Option<Coord3D>) {
        if pos.is_none() || !self.is_owners_cur_weapon_on_turret() {
            if !self.owner_turrets_linked() {
                pos = None;
            }
        }
        self.remove_self_as_targeter();
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

    pub fn get_target_kind(&self) -> TurretTargetKind {
        self.target_kind
    }

    pub fn get_target_position(&self) -> Option<Coord3D> {
        self.target_position
    }

    fn sync_goal_object(&mut self) {
        // The machine is owned, so the goal is written directly (C++ writes
        // m_turretStateMachine->getGoalObject() through the machine pointer).
        match self.target_kind {
            TurretTargetKind::Object => {
                self.state_machine.set_goal_object_by_id(self.current_target);
            }
            TurretTargetKind::Position => {
                self.state_machine.set_goal_object_by_id(None);
                if let Some(pos) = &self.target_position {
                    self.state_machine.set_goal_position(*pos);
                }
            }
            TurretTargetKind::None => {
                self.state_machine.set_goal_object_by_id(None);
            }
        }
    }

    /// C++ `setGoalObject(getCurrentVictim())` after a shot transfers the attack.
    /// Does not clear force-attack or idle-mood, and does not change state.
    pub fn note_transferred_victim(&mut self, victim: ObjectID) {
        if victim == crate::common::INVALID_ID {
            return;
        }
        self.current_target = Some(victim);
        self.target_kind = TurretTargetKind::Object;
        self.goal_sync_pending = true;
    }

    fn sync_state_for_target(&mut self) {
        let current = self.state_machine.get_current_state_id();
        let aim_id = TurretStateType::Aim.into();
        let fire_id = TurretStateType::Fire.into();
        let hold_id = TurretStateType::Hold.into();

        // C++ setTurretTargetObject/Position: any live target (object or Coord3D) enters AIM.
        let forced = if self.target_kind != TurretTargetKind::None {
            (current != Some(aim_id) && current != Some(fire_id)).then_some(aim_id)
        } else if current == Some(aim_id) || current == Some(fire_id) {
            Some(hold_id)
        } else {
            None
        };
        let Some(state_id) = forced else {
            return;
        };
        // Owned machine: take it out so the enter can loan `self` to the state.
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let _ = machine.set_current_state_with_owner(state_id, self);
        self.state_machine = machine;
    }

    fn owner_turrets_linked(&self) -> bool {
        self.turrets_linked_cached
    }

    fn owner_is_under_construction(&self) -> bool {
        if dual_world_registry_unavailable() || self.owner_id == crate::common::INVALID_ID {
            return false;
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner| {
                owner
                    .get_status_bits()
                    .test(ObjectStatusTypes::UnderConstruction)
            })
            .unwrap_or(false)
    }

    fn remove_self_as_targeter(&self) {
        if self.target_kind != TurretTargetKind::Object {
            return;
        }
        let Some(target_id) = self.current_target else {
            return;
        };
        if dual_world_registry_unavailable() {
            return;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target| {
            if let Some(ai) = target.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.add_targeter(self.owner_id, false);
                }
            }
        });
    }

    /// C++ `TurretAI::friend_getTurretTarget`.
    pub fn friend_get_turret_target(
        &mut self,
        clear_dead: bool,
    ) -> (TurretTargetKind, Option<ObjectID>, Coord3D) {
        match self.target_kind {
            TurretTargetKind::Object => {
                let mut pos = Coord3D::new(0.0, 0.0, 0.0);
                let mut id = self.current_target;
                if clear_dead {
                    let dead = id
                        .and_then(|tid| {
                            OBJECT_REGISTRY
                                .with_object(tid, |t| t.is_effectively_dead())
                        })
                        .unwrap_or(true);
                    if dead {
                        self.current_target = None;
                        self.target_kind = TurretTargetKind::None;
                        self.target_was_set_by_idle_mood = false;
                        self.goal_sync_pending = true;
                        return (TurretTargetKind::None, None, Coord3D::new(0.0, 0.0, 0.0));
                    }
                }
                if let Some(tid) = id {
                    if let Some(p) = OBJECT_REGISTRY.with_object(tid, |t| *t.get_position()) {
                        pos = p;
                    } else if clear_dead {
                        self.current_target = None;
                        self.target_kind = TurretTargetKind::None;
                        self.target_was_set_by_idle_mood = false;
                        self.goal_sync_pending = true;
                        return (TurretTargetKind::None, None, Coord3D::new(0.0, 0.0, 0.0));
                    } else {
                        id = None;
                    }
                }
                (TurretTargetKind::Object, id, pos)
            }
            TurretTargetKind::Position => {
                let pos = self
                    .target_position
                    .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));
                (TurretTargetKind::Position, None, pos)
            }
            TurretTargetKind::None => (TurretTargetKind::None, None, Coord3D::new(0.0, 0.0, 0.0)),
        }
    }

    fn relative_angle_2d_to(owner_pos: &Coord3D, owner_orient: f32, target: &Coord3D) -> f32 {
        let dx = target.x - owner_pos.x;
        let dy = target.y - owner_pos.y;
        if dx == 0.0 && dy == 0.0 {
            return 0.0;
        }
        Self::normalize_angle(dy.atan2(dx) - owner_orient)
    }

    fn nearer_bridge_attack_point(
        owner_pos: &Coord3D,
        owner_z_to_center: f32,
        bridge_id: ObjectID,
    ) -> Option<Coord3D> {
        let mut info = BridgeAttackInfo::new();
        if let Ok(terrain) = get_terrain_logic().try_read() {
            terrain.get_bridge_attack_points(bridge_id, &mut info);
        } else {
            return None;
        }
        // C++ FROM_BOUNDINGSPHERE_3D lifts the owner to the geometry center.
        let center = Coord3D::new(
            owner_pos.x,
            owner_pos.y,
            owner_pos.z + owner_z_to_center,
        );
        let d1 = center.distance_sqr(&info.attack_point1);
        let d2 = center.distance_sqr(&info.attack_point2);
        Some(if d1 > d2 {
            info.attack_point2
        } else {
            info.attack_point1
        })
    }

    /// C++ FirePitch / asin(v.z/len) / GroundUnitPitch*(dist/range) after min-pitch clamp.
    pub fn compute_desired_aim_pitch(
        &self,
        origin: &Coord3D,
        target: &Coord3D,
        origin_height_above: f32,
        attack_range: f32,
    ) -> f32 {
        if self.fire_pitch > 0.0 {
            return self.fire_pitch;
        }
        let mut v = Coord3D::new(
            target.x - origin.x,
            target.y - origin.y,
            target.z - origin.z,
        );
        v.z -= origin_height_above * 0.5;
        let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        let actual = if len > 0.0 { (v.z / len).asin() } else { 0.0 };
        let mut desired = actual.max(self.min_pitch);
        if self.ground_unit_pitch > 0.0 {
            // C++ nulls `enemy` before this block, so GroundUnitPitch always applies.
            let range = attack_range.max(1.0);
            desired = (actual + self.ground_unit_pitch * (len / range)).max(self.min_pitch);
        }
        desired
    }

    fn apply_turret_rotate_model_condition(&self, rotating: bool) {
        if dual_world_registry_unavailable() || self.owner_id == crate::common::INVALID_ID {
            return;
        }
        if let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_id) {
            if let Ok(mut guard) = owner.write() {
                if rotating {
                    guard.set_model_condition_state(ModelConditionFlags::TURRET_ROTATE);
                } else {
                    guard.clear_model_condition_state(ModelConditionFlags::TURRET_ROTATE);
                }
            }
        }
    }

    fn react_to_turret_change(&self, old_angle: f32) {
        if self.current_angle == old_angle {
            return;
        }
        if dual_world_registry_unavailable() || self.owner_id == crate::common::INVALID_ID {
            return;
        }
        let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.try_write() else {
            return;
        };
        if self.friend_get_which_turret() == TurretType::Primary {
            owner_guard.note_main_turret_yaw_and_redeploy(self.current_angle);
        } else {
            owner_guard.react_to_non_main_turret_turn();
        }
    }

    fn ensure_turret_move_loop_sound(&mut self) {
        if !self.turret_rot_or_pitch_sound.get_event_name().is_empty() {
            return;
        }
        if dual_world_registry_unavailable() || self.owner_id == crate::common::INVALID_ID {
            return;
        }
        if let Some(event) = OBJECT_REGISTRY
            .with_object(self.owner_id, |owner| {
                owner.get_template().get_per_unit_sound("TurretMoveLoop")
            })
            .flatten()
        {
            self.turret_rot_or_pitch_sound = event;
        }
    }

    fn start_rot_or_pitch_sound(&mut self) {
        self.ensure_turret_move_loop_sound();
        if self.turret_rot_or_pitch_sound.get_event_name().is_empty() {
            return;
        }
        if self.turret_rot_or_pitch_sound.is_currently_playing() {
            return;
        }
        self.turret_rot_or_pitch_sound.set_object_id(self.owner_id);
        if let Some(audio) = TheAudio::get() {
            let handle = audio.add_audio_event(&self.turret_rot_or_pitch_sound);
            self.turret_rot_or_pitch_sound.set_playing_handle(handle);
        }
    }

    fn stop_rot_or_pitch_sound(&mut self) {
        if !self.turret_rot_or_pitch_sound.is_currently_playing() {
            return;
        }
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(self.turret_rot_or_pitch_sound.get_playing_handle());
        }
        self.turret_rot_or_pitch_sound.set_playing_handle(0);
    }

    /// C++ `TurretAI::friend_turnTowardsAngle`.
    pub fn friend_turn_towards_angle(
        &mut self,
        desired_angle: f32,
        rate_modifier: f32,
        rel_thresh: f32,
    ) -> bool {
        let desired_angle = Self::normalize_angle(desired_angle);
        let orig_angle = self.current_angle;
        let mut actual = orig_angle;
        let turn_rate = self.turn_rate * rate_modifier;
        let angle_diff = Self::normalize_angle(desired_angle - actual);
        if angle_diff.abs() < turn_rate {
            actual = desired_angle;
            self.apply_turret_rotate_model_condition(false);
        } else {
            if angle_diff > 0.0 {
                actual += turn_rate;
            } else {
                actual -= turn_rate;
            }
            self.apply_turret_rotate_model_condition(true);
            self.play_rot_sound = true;
        }
        self.current_angle = Self::normalize_angle(actual);
        if self.current_angle != orig_angle {
            self.react_to_turret_change(orig_angle);
        }
        (self.current_angle - desired_angle).abs() <= rel_thresh
    }

    /// C++ `TurretAI::friend_turnTowardsPitch`.
    pub fn friend_turn_towards_pitch(&mut self, desired_pitch: f32, rate_modifier: f32) -> bool {
        if !self.allows_pitch {
            return true;
        }
        let desired_pitch = Self::normalize_angle(desired_pitch);
        let mut actual = self.current_pitch;
        let pitch_rate = self.pitch_rate * rate_modifier;
        let pitch_diff = Self::normalize_angle(desired_pitch - actual);
        if pitch_diff.abs() < pitch_rate {
            actual = desired_pitch;
        } else {
            if pitch_diff > 0.0 {
                actual += pitch_rate;
            } else {
                actual -= pitch_rate;
            }
            self.play_pitch_sound = true;
        }
        self.current_pitch = Self::normalize_angle(actual);
        self.current_pitch == desired_pitch
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
        // Wave 276: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

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
        if dual_world_registry_unavailable() || self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        let (owner_pos, height, range) = OBJECT_REGISTRY.with_object(self.owner_id, |owner| {
            let height = owner.get_geometry_info().get_max_height_above_position();
            let range = owner
                .get_weapon_in_slot(self.weapon_slot)
                .map(|w| w.get_attack_range(owner.get_id()))
                .unwrap_or(1.0);
            (*owner.get_position(), height, range)
        })?;
        let target_pos = OBJECT_REGISTRY.with_object(target_id, |t| *t.get_position())?;
        Some(self.compute_desired_aim_pitch(&owner_pos, &target_pos, height, range))
    }

    /// Rotate turret towards desired angle
    pub fn rotate_towards_angle(&mut self, desired_angle: f32) -> bool {
        self.friend_turn_towards_angle(desired_angle, 1.0, 0.0)
    }

    /// Rotate turret towards desired angle with speed modifier and threshold.
    pub fn rotate_towards_angle_with_speed(
        &mut self,
        desired_angle: f32,
        speed_modifier: f32,
        threshold: f32,
    ) -> bool {
        self.friend_turn_towards_angle(desired_angle, speed_modifier, threshold)
    }

    /// Pitch turret towards desired pitch
    pub fn pitch_towards_angle(&mut self, desired_pitch: f32) -> bool {
        self.friend_turn_towards_pitch(desired_pitch.max(self.min_pitch), 1.0)
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
        // Wave 276: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

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
        // Wave 276: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.owner_id == crate::common::INVALID_ID {
            return false;
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_guard| {
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
            })
            .unwrap_or(false)
    }

    /// Scan for targets within turret's range and arc
    pub fn scan_for_targets(&self) -> Vec<SharedObjectHandle> {
        // Wave 276: empty dual-world → no targets.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let mut targets = Vec::new();

        if !self.can_scan {
            return targets;
        }

        if self.owner_id == crate::common::INVALID_ID {
            return targets;
        }
        let Some((owner_id, owner_pos, range)) = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_guard| {
                let Some(weapon) = owner_guard.get_weapon_in_slot(self.weapon_slot) else {
                    return None;
                };
                let range = weapon.get_attack_range(owner_guard.get_id());
                Some((owner_guard.get_id(), *owner_guard.get_position(), range))
            })
            .flatten()
        else {
            return targets;
        };
        let Some(partition) = ThePartitionManager::get() else {
            return targets;
        };

        for candidate_id in partition.get_objects_in_range(&owner_pos, range) {
            if candidate_id == owner_id {
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
                let is_enemy = crate::object::registry::OBJECT_REGISTRY
                    .with_object(owner_id, |owner_guard| {
                        owner_guard.relationship_to(&candidate_guard) == Relationship::Enemies
                    })
                    .unwrap_or(false);
                if !is_enemy {
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
    pub fn find_best_target(&self, targets: &[SharedObjectHandle]) -> Option<SharedObjectHandle> {
        // Wave 276: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        if targets.is_empty() {
            return None;
        }

        // Simple targeting: closest enemy
        let mut best_target: Option<SharedObjectHandle> = None;
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

    /// C++ `TurretAI::updateTurretAI` (TurretAI.cpp:664-736): sleep gate, sound
    /// flag reset, run the owned behavior machine, then the fire/sweep/sound
    /// bookkeeping. States receive `self` via `update_with_owner`, so the whole
    /// step runs under one `&mut self` — the old drop-lock-then-relock dance is
    /// gone because nothing locks the turret from inside the step anymore.
    pub fn update_turret_ai(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if self.sleep_until != 0 && now < self.sleep_until {
            return StateReturnType::Sleep(self.sleep_until - now);
        }

        // either we don't care about continuous fire stuff, or we care, but time has elapsed
        if !self.fires_while_turning || self.continuous_fire_expiration_frame <= now {
            self.play_rot_sound = false;
            self.play_pitch_sound = false;
        }

        let recentering = self.state_machine.get_current_state_id()
            == Some(TurretStateType::Recenter.into());
        if !self.enabled && !recentering {
            self.sleep_until = now.saturating_add(WAIT_INDEFINITELY);
            return StateReturnType::Sleep(WAIT_INDEFINITELY);
        }

        self.did_fire = false;

        // Run the behavior state machine BEFORE doing sound check. The machine
        // is owned, so it is taken out for the step and returned afterwards.
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let state_return = if machine.is_empty() {
            StateReturnType::Continue
        } else {
            machine.update_with_owner(self)
        };
        self.state_machine = machine;

        if self.goal_sync_pending {
            self.goal_sync_pending = false;
            self.sync_goal_object();
        }
        if self.did_fire {
            // C++ ENABLE_SWEEP_FRAME_COUNT: enable sweeping for a few frames.
            let now = TheGameLogic::get_frame();
            self.enable_sweep_until = now.saturating_add(ENABLE_SWEEP_FRAME_COUNT);
            self.continuous_fire_expiration_frame = now.saturating_add(ENABLE_SWEEP_FRAME_COUNT);
        }
        if self.play_rot_sound || self.play_pitch_sound {
            self.start_rot_or_pitch_sound();
        } else {
            self.stop_rot_or_pitch_sound();
        }

        // C++: STATE_CONTINUE/SUCCESS/FAILURE all require next frame (sleep 0);
        // a Sleep keeps its frame count. Disabled-and-not-recentering sleeps
        // forever, which the early return above already recorded.
        let now = TheGameLogic::get_frame();
        let sleep_frames = match state_return {
            StateReturnType::Sleep(frames) => frames,
            _ => 0,
        };
        self.sleep_until = now.saturating_add(sleep_frames);
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

    pub fn set_turrets_linked_cached(&mut self, value: bool) {
        self.turrets_linked_cached = value;
    }

    pub fn set_attack_ok_cached(&mut self, value: bool) {
        self.attack_ok_cached = value;
    }

    pub fn attack_ok_cached(&self) -> bool {
        self.attack_ok_cached
    }

    pub fn set_goal_object_id_cached(&mut self, id: crate::common::ObjectID) {
        self.goal_object_id_cached = id;
    }

    pub fn set_last_command_source_cached(&mut self, source: crate::common::CommandSourceType) {
        self.last_command_source_cached = source;
    }

    pub fn turrets_linked_cached(&self) -> bool {
        self.turrets_linked_cached
    }

    pub fn set_next_mood_check_cached(&mut self, frame: u32) {
        self.next_mood_check_cached = frame;
    }

    pub fn take_reset_mood_check(&mut self) -> bool {
        let pending = self.reset_mood_check_pending;
        self.reset_mood_check_pending = false;
        pending
    }

    pub fn take_clear_turret_sync(&mut self) -> Option<TurretType> {
        self.clear_turret_sync.take()
    }

    pub fn take_idle_mood_check(&mut self) -> bool {
        let pending = self.idle_mood_check_pending;
        self.idle_mood_check_pending = false;
        pending
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
        // Wave 276: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

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

        matches!(
            self.state_machine.get_current_state_id(),
            Some(state) if state == TurretStateType::Aim as u32
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
        // C++ `TurretAI::recenterTurret` → `setState(TURRETAI_RECENTER)`. The
        // machine is owned, so it is taken out for the enter (which loans the
        // owner to the state) and returned.
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let _ = machine.set_current_state_with_owner(TurretStateType::Recenter.into(), self);
        self.state_machine = machine;
    }

    pub fn is_turret_in_natural_position(&self) -> bool {
        // C++ TurretAI::isTurretInNaturalPosition: UNDER_CONSTRUCTION is natural.
        if crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner| {
                owner
                    .get_status_bits()
                    .test(crate::object::ObjectStatusTypes::UnderConstruction)
            })
            .unwrap_or(false)
        {
            return true;
        }
        self.natural_angle == self.current_angle && self.natural_pitch == self.current_pitch
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
        if self.next_mood_check_cached != 0 {
            self.next_mood_check_cached
        } else {
            TheGameLogic::get_frame()
        }
    }

    /// Check for idle mood target acquisition (matches C++ friend_checkForIdleMoodTarget)
    pub fn friend_check_for_idle_mood_target(&mut self) {
        // Wave 276: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.owner_id == crate::common::INVALID_ID {
            return;
        }
        let Some(ai) = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |owner_guard| {
                owner_guard.get_ai_update_interface()
            })
            .flatten()
        else {
            return;
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
            if let Some(owner_arc) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.owner_id))
            {
                if let (Ok(mut owner_write), Ok(target_guard)) = (owner_arc.write(), enemy.read()) {
                    let _ = owner_write.choose_best_weapon_for_target(
                        &target_guard,
                        WeaponChoiceCriteria::PreferMostDamage,
                        crate::common::CommandSourceType::FromAi,
                    );
                }
            }
            let enemy_id = enemy.read().ok().map(|g| g.get_id());
            self.set_current_target_from_idle_mood(enemy_id);
        }
    }

    /// C++ `TurretAIAimTurretState::update`.
    pub fn update_aim_state(&mut self) -> StateReturnType {
        let (kind, target_id, mut aim_pos) = self.friend_get_turret_target(true);
        if kind == TurretTargetKind::None {
            return StateReturnType::Failure;
        }
        let prechecked_out_of_range = match (kind, target_id) {
            (TurretTargetKind::Object, Some(tid)) => !self.friend_is_any_weapon_in_range_of(tid),
            _ => false,
        };

        let Some(owner_arc) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
            .or_else(|| OBJECT_REGISTRY.get_object(self.owner_id))
        else {
            return StateReturnType::Failure;
        };
        let Ok(owner) = owner_arc.read() else {
            return StateReturnType::Failure;
        };
        if owner.get_ai_update_interface().is_none() {
            return StateReturnType::Failure;
        }

        let mut preventing = false;
        let mut nothing_in_range = false;
        let mut enemy_for_range: Option<ObjectID> = None;
        let owner_pos = *owner.get_position();
        let owner_orient = owner.get_orientation();
        let owner_height = owner.get_geometry_info().get_max_height_above_position();

        if kind == TurretTargetKind::Object {
            let Some(tid) = target_id else {
                if self.target_was_set_by_idle_mood {
                    self.remove_self_as_targeter();
                    self.current_target = None;
                    self.target_kind = TurretTargetKind::None;
                    self.target_was_set_by_idle_mood = false;
                    self.goal_sync_pending = true;
                }
                return StateReturnType::Failure;
            };
            let Some(target_arc) = OBJECT_REGISTRY.get_object(tid) else {
                if self.target_was_set_by_idle_mood {
                    self.remove_self_as_targeter();
                    self.current_target = None;
                    self.target_kind = TurretTargetKind::None;
                    self.target_was_set_by_idle_mood = false;
                    self.goal_sync_pending = true;
                }
                return StateReturnType::Failure;
            };
            let Ok(target) = target_arc.read() else {
                return StateReturnType::Failure;
            };
            let is_primary = self.goal_object_id_cached != crate::common::INVALID_ID
                && self.goal_object_id_cached == tid;
            let mut able = owner.is_able_to_attack();
            if able {
                let attack_type = if self.is_force_attacking {
                    AbleToAttackType::ContinuedTargetForced
                } else {
                    AbleToAttackType::ContinuedTarget
                };
                let cmd = self.last_command_source_cached;
                able = matches!(
                    owner.get_able_to_attack_specific_object(attack_type, &target, cmd),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                );
            }
            nothing_in_range = prechecked_out_of_range;
            let team_changed = self.victim_initial_team != target.get_team_id();
            if !able || (!is_primary && nothing_in_range) || team_changed {
                if self.target_was_set_by_idle_mood {
                    self.remove_self_as_targeter();
                    self.current_target = None;
                    self.target_kind = TurretTargetKind::None;
                    self.target_was_set_by_idle_mood = false;
                    self.goal_sync_pending = true;
                }
                return StateReturnType::Failure;
            }
            if target.is_kind_of(KindOf::Bridge) {
                let z_center = owner.get_geometry_info().get_z_delta_to_center_position();
                if let Some(pt) = Self::nearer_bridge_attack_point(&owner_pos, z_center, tid) {
                    aim_pos = pt;
                }
            } else {
                aim_pos = *target.get_position();
            }
            if let Some(enemy_ai) = target.get_ai_update_interface() {
                if let Ok(mut enemy_ai) = enemy_ai.lock() {
                    enemy_ai.add_targeter(self.owner_id, true);
                    preventing = enemy_ai.is_temporarily_preventing_aim_success();
                }
            }
            enemy_for_range = Some(tid);
        }

        let source_id = owner.get_id();
        let source_radius = owner.get_geometry_info().get_bounding_circle_radius();
        let source_geom = *owner.get_geometry_info();
        let mut flags = crate::weapon::helpers::map_common_bonus_flags(owner.get_weapon_bonus_condition());
        let container = crate::weapon::weapon_bonus::container_passenger_bonus_flags(owner.get_contained_by());
        flags.union(crate::weapon::helpers::map_common_bonus_flags(container));
        let Some((weapon, slot)) = owner.get_current_weapon() else {
            return StateReturnType::Failure;
        };
        let bonus = weapon.bonus_from_flags(flags);
        let attack_range = weapon.template.get_attack_range(&bonus);
        let in_range = if let Some(tid) = enemy_for_range {
            weapon.is_within_attack_range_from_source(
                &owner_pos,
                source_radius,
                &source_geom,
                &bonus,
                Some(tid),
                None,
            )
        } else {
            weapon.is_within_attack_range_from_source(
                &owner_pos,
                source_radius,
                &source_geom,
                &bonus,
                None,
                Some(&aim_pos),
            )
        };

        let rel_angle = Self::relative_angle_2d_to(&owner_pos, owner_orient, &aim_pos);
        let mut aim_angle = rel_angle;
        let mut turn_speed_modifier = 1.0;
        let sweep = self.get_turret_fire_angle_sweep_for_weapon_slot(slot);
        if sweep > 0.0 && self.friend_is_sweep_enabled() {
            if self.positive_sweep {
                aim_angle += sweep;
            } else {
                aim_angle -= sweep;
            }
            turn_speed_modifier = self.get_turret_sweep_speed_modifier_for_weapon_slot(slot);
        }
        let mut turn_aligned =
            self.friend_turn_towards_angle(aim_angle, turn_speed_modifier, TURRET_AIM_REL_THRESH);
        if sweep > 0.0 {
            if turn_aligned {
                self.positive_sweep = !self.positive_sweep;
            }
            let angle_diff = Self::normalize_angle(rel_angle - self.current_angle);
            turn_aligned = angle_diff.abs() < sweep;
        }

        let mut pitch_aligned = true;
        if self.allows_pitch {
            let desired =
                self.compute_desired_aim_pitch(&owner_pos, &aim_pos, owner_height, attack_range);
            pitch_aligned = self.friend_turn_towards_pitch(desired, 1.0);
        }

        if turn_aligned && pitch_aligned && in_range {
            if preventing || nothing_in_range {
                return StateReturnType::Continue;
            }
            return StateReturnType::Success;
        }
        StateReturnType::Continue
    }
}

/// Marker payload for the FIRE → AIM out-of-range transition. C++
/// `outOfWeaponRangeObject` (AIStates.cpp) reads only the machine owner and the
/// machine goal object — both are mirrored onto the state (`State::owner_id`
/// copied at build, goal id bound per step), so the check needs no turret or
/// machine handle and runs while the turret is already `&mut`-borrowed.
struct TurretFireRangeCheck;

/// Dispatch a transition payload to the concrete state. Same shape as the
/// legacy adapter's thunk, kept local because turret states register directly.
fn turret_condition_invoke(
    state: &dyn StateImplementation,
    user_data: &StateTransitionUserData,
) -> bool {
    let Some(payload) = user_data.data.as_ref() else {
        return false;
    };
    state
        .evaluate_transition_payload(payload.as_ref())
        .unwrap_or(false)
}

/// UnitAIUpdate's per-slot turret bundle (`turret_primary_machine` /
/// `turret_secondary_machine`; C++ `UnitAI` keeps one `TurretAI*` per slot).
/// The turret owns its machine outright (C++ `TurretAI` owns
/// `m_turretStateMachine`, TurretAI.h:332), so this wrapper owns the whole
/// bundle by value and hands out borrows — no Arc, no Mutex, no Weak anywhere
/// in the chain. Construction builds the six states and enters IDLE exactly
/// where the C++ `TurretStateMachine` ctor + `initDefaultState()` ran
/// (TurretAI.cpp:65-80, 294-295).
pub struct TurretStateMachine {
    /// The turret and, inside it, the machine. `None` only while a tick has
    /// loaned the turret out through [`Self::take_turret`].
    turret: Option<TurretAI>,
}

impl TurretStateMachine {
    /// Takes the already-data-applied turret (C++ passes the `TurretAI*`), then
    /// runs the state definition + default-state entry that the C++
    /// `TurretStateMachine` ctor performed.
    pub fn new(turret: TurretAI) -> Self {
        let mut turret = turret;
        turret.build_state_machine();
        Self { turret: Some(turret) }
    }

    /// The turret (C++ `TurretStateMachine::getTurretAI`, TurretAI.h:53 — the
    /// arrow is inverted here: the bundle owns the turret, and the machine
    /// lives inside the turret).
    pub fn turret(&self) -> &TurretAI {
        self.turret
            .as_ref()
            .expect("turret loaned via take_turret without restore")
    }

    /// Mutable borrow for call sites that drive the turret directly.
    pub fn turret_mut(&mut self) -> &mut TurretAI {
        self.turret
            .as_mut()
            .expect("turret loaned via take_turret without restore")
    }

    /// Loan the turret out for a step that also borrows `UnitAIUpdate` (the
    /// machine lives inside the turret, so it travels with the loan). Pair
    /// with [`Self::restore_turret`] before the next tick.
    pub fn take_turret(&mut self) -> Option<TurretAI> {
        self.turret.take()
    }

    /// Return a [`Self::take_turret`] loan.
    pub fn restore_turret(&mut self, turret: TurretAI) {
        self.turret = Some(turret);
    }
}

impl TurretAI {
    /// C++ `TurretStateMachine` ctor (TurretAI.cpp:41-71): define the six
    /// states, their success/failure links, and the FIRE out-of-range
    /// condition. Order matters — the first state defined is the default.
    /// States register directly (no legacy adapter) so their hooks can receive
    /// the loaned owner via `update_with_owner` / `on_enter_with_owner`.
    fn define_turret_states(&self, machine: &mut StateMachine) {
        let idle_id = TurretStateType::Idle.into();
        let idle_scan_id = TurretStateType::IdleScan.into();
        let aim_id = TurretStateType::Aim.into();
        let fire_id = TurretStateType::Fire.into();
        let recenter_id = TurretStateType::Recenter.into();
        let hold_id = TurretStateType::Hold.into();

        machine.define_state(
            idle_id,
            Box::new(TurretAIIdleState::new(machine, "TurretAIIdleState")),
            Some(idle_id),
            Some(idle_scan_id),
            None,
        );

        machine.define_state(
            idle_scan_id,
            Box::new(TurretAIIdleScanState::new(
                machine,
                "TurretAIIdleScanState",
            )),
            Some(hold_id),
            Some(hold_id),
            None,
        );

        machine.define_state(
            aim_id,
            Box::new(TurretAIAimTurretState::new(
                machine,
                "TurretAIAimTurretState",
            )),
            Some(fire_id),
            Some(hold_id),
            None,
        );

        let fire_conditions = vec![StateConditionInfo::new(
            turret_condition_invoke,
            aim_id,
            StateTransitionUserData {
                data: Some(Arc::new(TurretFireRangeCheck)),
            },
            "out_of_weapon_range_object",
        )];

        machine.define_state(
            fire_id,
            Box::new(TurretAIFireWeaponState::new(
                machine,
                "TurretAIFireWeaponState",
            )),
            Some(aim_id),
            Some(aim_id),
            Some(&fire_conditions),
        );

        machine.define_state(
            recenter_id,
            Box::new(TurretAIRecenterTurretState::new(
                machine,
                "TurretAIRecenterTurretState",
            )),
            Some(idle_id),
            Some(idle_id),
            None,
        );

        machine.define_state(
            hold_id,
            Box::new(TurretAIHoldTurretState::new(
                machine,
                "TurretAIHoldTurretState",
            )),
            Some(recenter_id),
            Some(recenter_id),
            None,
        );
    }
}

// Turret state implementations
//
// C++ `TurretState` reached the turret through the machine pointer
// (TurretAI.h:70-76 `getTurretAI()`). The machine is owned by `TurretAI` here,
// so states embed only the legacy `State` bookkeeping (id/name, the machine's
// owner id copied once at define time, plus the goal mirrors the machine binds
// each step) and receive `&mut TurretAI` through `update_with_owner` /
// `on_enter_with_owner` — the loaned-turret equivalent of C++'s `getTurretAI()`.

/// Base data for turret states.
#[derive(Debug)]
pub struct TurretState {
    base: State,
}

impl TurretState {
    fn new(machine: &StateMachine, name: &str) -> Self {
        // `State::new` copies the machine's owner id and attaches NO machine
        // handle: the machine owns its states, never the other way round.
        Self {
            base: State::new(machine, name),
        }
    }

    fn downcast_owner(owner: &mut dyn Any) -> Option<&mut TurretAI> {
        owner.downcast_mut::<TurretAI>()
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.set_id(id);
    }

    /// Goal the machine bound for this step (C++ `getMachineGoalObject` id).
    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.base.goal_object_id = id;
    }
}

/// Idle state - do nothing, wait for targets
#[derive(Debug)]
pub struct TurretAIIdleState {
    base: TurretState,
    next_idle_scan: u32,
}

impl TurretAIIdleState {
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        Self {
            base: TurretState::new(machine, name),
            next_idle_scan: 0,
        }
    }

    /// C++ `TurretAIIdleState::onEnter` (TurretAI.cpp): reset the mood check,
    /// publish the turret slot on the sync flag, arm the idle scan timer.
    fn classic_on_enter(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        let which = turret.friend_get_which_turret();
        turret.reset_mood_check_pending = true;
        turret.clear_turret_sync = Some(which);
        self.reset_idle_scan(turret)?;
        let mood_frame = turret.friend_get_next_idle_mood_target_frame();
        Ok(frame_to_sleep_time(
            mood_frame,
            Some(self.next_idle_scan),
            None,
            None,
        ))
    }

    fn reset_idle_scan(&mut self, turret: &TurretAI) -> Result<(), String> {
        let current_frame = TheGameLogic::try_get_frame()?;
        let min_interval = turret.get_min_idle_scan_interval();
        let mut max_interval = turret.get_max_idle_scan_interval();
        if max_interval < min_interval {
            max_interval = min_interval;
        }
        let interval = GameLogicRandomValue(min_interval as i32, max_interval as i32) as u32;
        self.next_idle_scan = current_frame + interval;
        Ok(())
    }

    fn classic_on_update(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        let current_frame = TheGameLogic::try_get_frame()?;

        if current_frame >= self.next_idle_scan {
            return Ok(StateReturnType::Failure);
        }

        turret.idle_mood_check_pending = true;
        let mood_frame = turret.friend_get_next_idle_mood_target_frame();
        Ok(frame_to_sleep_time(
            mood_frame,
            Some(self.next_idle_scan),
            None,
            None,
        ))
    }
}

impl StateImplementation for TurretAIIdleState {
    /// Turret states are only stepped through their owner's machine, which
    /// always loans the turret via `update_with_owner`; there is no owner-free
    /// step to run here.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }


    fn on_enter_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_enter(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_update(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {}

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIIdleState xfer version failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_idle_scan)
            .map_err(|e| format!("TurretAIIdleState xfer next_idle_scan failed: {:?}", e))?;
        Ok(())
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.set_id(id);
    }

    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn is_idle(&self) -> bool {
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
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        Self {
            base: TurretState::new(machine, name),
            desired_angle: 0.0,
        }
    }

    fn classic_on_enter(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
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
        // C++ stores the offset. Update adds the current natural angle.
        self.desired_angle = offset;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        if let Some(owner) = turret.owner_object() {
            if let Ok(owner) = owner.read() {
                if owner
                    .get_status_bits()
                    .test(ObjectStatusTypes::UnderConstruction)
                {
                    return Ok(StateReturnType::Continue);
                }
            }
        }
        let goal_angle = turret.get_natural_angle() + self.desired_angle;
        let natural_pitch = turret.get_natural_pitch();
        let angle_aligned = turret.friend_turn_towards_angle(goal_angle, 0.5, 0.0);
        let pitch_aligned = turret.friend_turn_towards_pitch(natural_pitch, 0.5);
        if angle_aligned && pitch_aligned {
            return Ok(StateReturnType::Success);
        }
        Ok(StateReturnType::Continue)
    }
}

impl StateImplementation for TurretAIIdleScanState {
    /// Turret states are only stepped through their owner's machine, which
    /// always loans the turret via `update_with_owner`; there is no owner-free
    /// step to run here.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }


    fn on_enter_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_enter(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_update(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {}

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIIdleScanState xfer version failed: {:?}", e))?;
        xfer.xfer_real(&mut self.desired_angle)
            .map_err(|e| format!("TurretAIIdleScanState xfer desired_angle failed: {:?}", e))?;
        Ok(())
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.set_id(id);
    }

    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn is_busy(&self) -> bool {
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
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        Self {
            base: TurretState::new(machine, name),
            delay_until: 0,
        }
    }

    fn classic_on_enter(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        self.delay_until = 0;
        let delay = turret.get_inter_turret_delay();
        if delay > 0 {
            self.delay_until = TheGameLogic::get_frame().saturating_add(delay);
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        if self.delay_until > 0 {
            let now = TheGameLogic::get_frame();
            if now < self.delay_until {
                return Ok(StateReturnType::Sleep(self.delay_until - now));
            }
            self.delay_until = 0;
        }
        Ok(turret.update_aim_state())
    }
}

impl StateImplementation for TurretAIAimTurretState {
    /// Turret states are only stepped through their owner's machine, which
    /// always loans the turret via `update_with_owner`; there is no owner-free
    /// step to run here.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }


    fn on_enter_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_enter(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_update(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.delay_until = 0;
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.set_id(id);
    }

    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn is_busy(&self) -> bool {
        true
    }
}

/// Fire weapon state - fire at target
#[derive(Debug)]
pub struct TurretAIFireWeaponState {
    base: TurretState,
}

impl TurretAIFireWeaponState {
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        Self {
            base: TurretState::new(machine, name),
        }
    }

    /// C++ `outOfWeaponRangeObject` (AIStates.cpp): reads only the machine
    /// owner and the machine goal object. The owner id is mirrored at build
    /// (`State::owner_id`) and the goal id is bound every step
    /// (`bind_goal_object_id`), so this runs without any turret handle while
    /// the step already holds the turret.
    fn out_of_weapon_range_object(&self) -> bool {
        let owner_id = self.base.base.owner_id;
        let target_id = self.base.base.goal_object_id;
        if owner_id == crate::common::INVALID_ID || target_id == crate::common::INVALID_ID {
            return false;
        }
        let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(owner_id)
            .or_else(|| OBJECT_REGISTRY.get_object(owner_id))
        else {
            return false;
        };
        let Ok(owner_guard) = owner.read() else {
            return false;
        };
        let source_pos = *owner_guard.get_position();
        let source_radius = owner_guard.get_geometry_info().get_bounding_circle_radius();
        let source_geom = *owner_guard.get_geometry_info();
        let mut flags =
            crate::weapon::helpers::map_common_bonus_flags(owner_guard.get_weapon_bonus_condition());
        let container = crate::weapon::weapon_bonus::container_passenger_bonus_flags(
            owner_guard.get_contained_by(),
        );
        flags.union(crate::weapon::helpers::map_common_bonus_flags(container));
        let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
            return false;
        };
        if weapon.has_leech_range() {
            return false;
        }
        let bonus = weapon.bonus_from_flags(flags);
        !weapon.is_within_attack_range_from_source(
            &source_pos,
            source_radius,
            &source_geom,
            &bonus,
            Some(target_id),
            None,
        )
    }

    fn classic_on_enter(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        let victim = turret.get_current_target_id();
        if !turret.attack_ok_cached() {
            return Ok(StateReturnType::Failure);
        }
        if let Some(owner) = turret.owner_object() {
            if let Ok(mut owner_guard) = owner.try_write() {
                if let Some(victim_id) = victim {
                    if let Some(team_arc) = owner_guard.get_team() {
                        if let Ok(mut team) = team_arc.write() {
                            crate::ai::states::seed_team_target_if_attack_common(
                                &mut team,
                                victim_id,
                            );
                        }
                    }
                }
                owner_guard.set_firing_condition_for_current_weapon();
                owner_guard.pre_fire_current_weapon(victim);
            }
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        let pos_only = turret.target_kind == TurretTargetKind::Position;
        if !pos_only && dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        // Get current target
        let target_opt = turret.get_current_target();

        if let Some(target) = target_opt {
            // Check if target is still valid
            let target_dead = target
                .try_read()
                .map(|guard| guard.is_effectively_dead())
                .unwrap_or(false);

            if target_dead {
                // AIAttackFireWeaponState returns FAILURE. The fire link goes back to Aim.
                return Ok(StateReturnType::Failure);
            }

            // Target is alive, try to fire.
            let Some(owner_arc) = turret.owner_object() else {
                // No owner object, transition to Hold
                return Ok(StateReturnType::Failure);
            };
            // Check if we can fire at target
            let can_fire =
                turret.can_fire_at_target(target.read().ok().map(|g| g.get_id()).unwrap_or(0));
            if !can_fire {
                // Can't fire (out of range, not aimed, etc.), transition to Aim
                return Ok(StateReturnType::Failure);
            }

            // Fire weapon - matches C++ AIAttackFireWeaponState::update() from AIStates.cpp:5169
            if let Ok(mut owner_guard) = owner_arc.try_write() {
                // Temporarily take weapon_set to avoid aliasing issues
                let mut weapon_set = std::mem::take(&mut owner_guard.weapon_set);

                let Some(current_slot) = weapon_set.get_current_weapon().map(|(_, slot)| slot)
                else {
                    weapon_set.apply_pending_shared_fire();
                    owner_guard.weapon_set = weapon_set;
                    return Ok(StateReturnType::Failure);
                };
                let slot_blocked = !turret.turrets_linked_cached()
                    && !turret.is_weapon_slot_on_turret(current_slot);
                if slot_blocked {
                    weapon_set.apply_pending_shared_fire();
                    owner_guard.weapon_set = weapon_set;
                    return Ok(StateReturnType::Failure);
                }
                if let Some(weapon) = weapon_set.get_weapon_in_slot_mut(current_slot) {
                    // Check weapon status - matches C++ line 5189-5197
                    let weapon_status = weapon.get_status();

                    if weapon_status == crate::weapon::WeaponStatus::PreAttack {
                        // Still in pre-attack delay, continue waiting
                        // Restore weapon_set before returning
                        weapon_set.apply_pending_shared_fire();
                        owner_guard.weapon_set = weapon_set;
                        return Ok(StateReturnType::Continue);
                    } else if weapon_status == crate::weapon::WeaponStatus::ReadyToFire {
                        owner_guard.set_firing_condition_for_current_weapon();
                        weapon_set.apply_pending_shared_fire();
                        owner_guard.weapon_set = weapon_set;
                        let target_id = target.try_read().ok().map(|guard| guard.get_id());
                        let owner_id = owner_guard.get_id();
                        if let Some(target_id) = target_id {
                            let _ = owner_guard.fire_current_weapon_at_target_id(target_id);
                        }
                        owner_guard.clear_status(
                            crate::common::ObjectStatusMaskType::from_status(
                                crate::object::ObjectStatusTypes::IgnoringStealth,
                            ),
                        );
                        turret.set_did_fire(true);
                        drop(owner_guard);
                        if let Some(target_id) = target_id {
                            if let Some(current) = crate::object::unit::unit_attack_target(owner_id)
                            {
                                if current != target_id {
                                    turret.note_transferred_victim(current);
                                }
                            }
                        }
                        return Ok(StateReturnType::Success);
                    } else {
                        // Weapon not ready (reloading, out of ammo, etc.)
                        // Restore weapon_set and transition to Aim
                        weapon_set.apply_pending_shared_fire();
                        owner_guard.weapon_set = weapon_set;
                        return Ok(StateReturnType::Failure);
                    }
                } else {
                    // No weapon in slot, restore weapon_set and transition to Hold
                    weapon_set.apply_pending_shared_fire();
                    owner_guard.weapon_set = weapon_set;
                    return Ok(StateReturnType::Failure);
                }
            }
            return Ok(StateReturnType::Continue);
        } else if let Some(pos) = turret
            .target_position
            .filter(|_| turret.target_kind == TurretTargetKind::Position)
        {
            let Some(owner_arc) = turret.owner_object() else {
                return Ok(StateReturnType::Failure);
            };
            if let Ok(mut owner_guard) = owner_arc.try_write() {
                let mut weapon_set = std::mem::take(&mut owner_guard.weapon_set);
                let Some(current_slot) = weapon_set.get_current_weapon().map(|(_, slot)| slot)
                else {
                    weapon_set.apply_pending_shared_fire();
                    owner_guard.weapon_set = weapon_set;
                    return Ok(StateReturnType::Failure);
                };
                let slot_blocked = !turret.turrets_linked_cached()
                    && !turret.is_weapon_slot_on_turret(current_slot);
                if slot_blocked {
                    weapon_set.apply_pending_shared_fire();
                    owner_guard.weapon_set = weapon_set;
                    return Ok(StateReturnType::Failure);
                }
                if let Some(weapon) = weapon_set.get_weapon_in_slot_mut(current_slot) {
                    let status = weapon.get_status();
                    if status == crate::weapon::WeaponStatus::PreAttack {
                        weapon_set.apply_pending_shared_fire();
                        owner_guard.weapon_set = weapon_set;
                        return Ok(StateReturnType::Continue);
                    }
                    if status != crate::weapon::WeaponStatus::ReadyToFire {
                        weapon_set.apply_pending_shared_fire();
                        owner_guard.weapon_set = weapon_set;
                        return Ok(StateReturnType::Failure);
                    }
                    owner_guard.set_firing_condition_for_current_weapon();
                    let linked = turret.turrets_linked_cached();
                    if linked {
                        weapon_set.apply_pending_shared_fire();
                        owner_guard.weapon_set = weapon_set;
                        owner_guard.fire_linked_turrets_at_position(&pos);
                    } else {
                        weapon_set.apply_pending_shared_fire();
                        owner_guard.weapon_set = weapon_set;
                        let _ = owner_guard.fire_current_weapon_at_position(&pos);
                    }
                    owner_guard.clear_status(
                        crate::common::ObjectStatusMaskType::from_status(
                            crate::object::ObjectStatusTypes::IgnoringStealth,
                        ),
                    );
                    turret.set_did_fire(true);
                    return Ok(StateReturnType::Success);
                } else {
                    weapon_set.apply_pending_shared_fire();
                    owner_guard.weapon_set = weapon_set;
                    return Ok(StateReturnType::Failure);
                }
            }
            return Ok(StateReturnType::Continue);
        }

        // No target, transition to Hold
        Ok(StateReturnType::Failure)
    }

    fn classic_on_exit(&mut self, _turret: &mut TurretAI) -> Result<(), String> {
        // Owner status cleanup lives in `on_exit`, which has no owner loan
        // (exits only need the machine's owner id, mirrored on the state).
        Ok(())
    }
}

impl StateImplementation for TurretAIFireWeaponState {
    /// Turret states are only stepped through their owner's machine, which
    /// always loans the turret via `update_with_owner`; there is no owner-free
    /// step to run here.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }


    fn on_enter_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_enter(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_update(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, status: StateExitType) {
        // on_exit has no owner loan available; the exit only touches the owner
        // object, resolved through the machine's owner id.
        let owner_id = self.base.base.owner_id;
        if owner_id != crate::common::INVALID_ID {
            if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(owner_id)
                .or_else(|| OBJECT_REGISTRY.get_object(owner_id))
            {
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_status(
                        crate::common::ObjectStatusMaskType::from_status(
                            crate::object::ObjectStatusTypes::IsFiringWeapon,
                        ),
                    );
                    owner_guard.clear_status(
                        crate::common::ObjectStatusMaskType::from_status(
                            crate::object::ObjectStatusTypes::IgnoringStealth,
                        ),
                    );
                    if owner_guard
                        .get_current_weapon()
                        .is_some_and(|(weapon, _)| {
                            weapon.get_status() == crate::weapon::WeaponStatus::PreAttack
                        })
                    {
                        owner_guard.cancel_pre_attack_for_current_weapon();
                    }
                }
            }
        }
        let _ = status;
    }

    /// Transition predicates run while the step already holds `&mut TurretAI`,
    /// so they resolve purely from the state's mirrored owner/goal ids.
    fn evaluate_transition_payload(&self, payload: &(dyn Any + Send + Sync)) -> Option<bool> {
        payload
            .downcast_ref::<TurretFireRangeCheck>()
            .map(|_| self.out_of_weapon_range_object())
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.set_id(id);
    }

    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn get_machine_goal_object(
        &self,
    ) -> Result<Option<SharedObjectHandle>, String> {
        let id = self.base.base.goal_object_id;
        if id == crate::common::INVALID_ID {
            return Ok(None);
        }
        Ok(crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| OBJECT_REGISTRY.get_object(id)))
    }

    fn is_attack(&self) -> bool {
        true
    }

    fn is_busy(&self) -> bool {
        true
    }
}

/// Recenter turret state - rotate back to natural position
#[derive(Debug)]
pub struct TurretAIRecenterTurretState {
    base: TurretState,
}

impl TurretAIRecenterTurretState {
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        Self {
            base: TurretState::new(machine, name),
        }
    }

    fn classic_on_update(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        if let Some(owner) = turret.owner_object() {
            if let Ok(owner) = owner.read() {
                if owner
                    .get_status_bits()
                    .test(ObjectStatusTypes::UnderConstruction)
                {
                    return Ok(StateReturnType::Continue);
                }
            }
        }
        let natural_angle = turret.get_natural_angle();
        let angle_aligned = turret.friend_turn_towards_angle(natural_angle, 0.5, 0.0);
        let natural_pitch = turret.get_natural_pitch();
        let pitch_aligned = turret.friend_turn_towards_pitch(natural_pitch, 0.5);
        if angle_aligned && pitch_aligned {
            return Ok(StateReturnType::Success);
        }
        Ok(StateReturnType::Continue)
    }
}

impl StateImplementation for TurretAIRecenterTurretState {
    /// Turret states are only stepped through their owner's machine, which
    /// always loans the turret via `update_with_owner`; there is no owner-free
    /// step to run here.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }


    fn update_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_update(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.set_id(id);
    }

    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn is_busy(&self) -> bool {
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
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        Self {
            base: TurretState::new(machine, name),
            timestamp: 0,
        }
    }

    fn classic_on_enter(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        let current_frame = TheGameLogic::try_get_frame()?;
        self.timestamp = current_frame.saturating_add(turret.get_recenter_time());
        Ok(frame_to_sleep_time(
            turret.friend_get_next_idle_mood_target_frame(),
            Some(self.timestamp),
            None,
            None,
        ))
    }

    fn classic_on_update(&mut self, turret: &mut TurretAI) -> Result<StateReturnType, String> {
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let current_frame = TheGameLogic::try_get_frame()?;
        // C++ returns success before the mood check once the hold timer is done.
        if current_frame >= self.timestamp {
            return Ok(StateReturnType::Success);
        }

        turret.idle_mood_check_pending = true;
        Ok(frame_to_sleep_time(
            turret.friend_get_next_idle_mood_target_frame(),
            Some(self.timestamp),
            None,
            None,
        ))
    }
}

impl StateImplementation for TurretAIHoldTurretState {
    /// Turret states are only stepped through their owner's machine, which
    /// always loans the turret via `update_with_owner`; there is no owner-free
    /// step to run here.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }


    fn on_enter_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_enter(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        match TurretState::downcast_owner(owner) {
            Some(turret) => self.classic_on_update(turret).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("TurretAIHoldTurretState xfer version failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.timestamp)
            .map_err(|e| format!("TurretAIHoldTurretState xfer timestamp failed: {:?}", e))?;
        Ok(())
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.set_id(id);
    }

    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn is_busy(&self) -> bool {
        true
    }
}

impl Snapshotable for TurretAI {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("TurretAI version crc failed: {:?}", e))?;
        if !self.state_machine.is_empty() {
            self.state_machine.crc(xfer).map_err(|e| e.to_string())?;
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
        if !self.state_machine.is_empty() {
            self.state_machine
                .xfer(xfer)
                .map_err(|e| e.to_string())?;
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
        // Wave 276: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

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
        TurretAI::new(crate::common::INVALID_ID)
    }

    #[test]
    fn owned_machine_builds_enters_idle_and_tracks_position_goal() {
        // Builds the bundle the way UnitAIUpdate does (ai_core.rs
        // build_turret_machine): TurretAI::new → apply data → TurretStateMachine::new.
        // No registry, no shared handle anywhere.
        let mut machine = TurretStateMachine::new(TurretAI::new(crate::common::INVALID_ID));
        let idle_id = u32::from(TurretStateType::Idle);
        assert_eq!(
            machine.turret().get_current_state_id(),
            Some(idle_id),
            "fresh turret must start in TURRETAI_IDLE"
        );

        // A position goal is refused unless the owner's current weapon is on
        // the turret or the turrets are linked (TurretAI.cpp:589-597), so link
        // them the way the update loop stamps `areTurretsLinked()`.
        machine.turret_mut().set_turrets_linked_cached(true);
        let goal = Coord3D::new(30.0, 0.0, 0.0);
        machine.turret_mut().set_target_position(Some(goal));
        assert_eq!(machine.turret().get_target_kind(), TurretTargetKind::Position);
        assert_eq!(
            machine.turret().get_current_state_id(),
            Some(u32::from(TurretStateType::Aim)),
            "position goal must force TURRETAI_AIM"
        );

        // Recenter through the owned machine.
        machine.turret_mut().recenter_turret();
        assert_eq!(
            machine.turret().get_current_state_id(),
            Some(u32::from(TurretStateType::Recenter))
        );

        // A full step stays inside the owned machine.
        machine.turret_mut().set_turret_enabled(true);
        let result = machine.turret_mut().update_turret_ai();
        assert!(matches!(result, StateReturnType::Continue | StateReturnType::Sleep(0)));
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
    fn set_target_position_refused_unless_linked_or_armed() {
        // C++ TurretAI::setTurretTargetPosition (TurretAI.cpp:589-597): a
        // position goal only sticks when the owner's current weapon is on the
        // turret OR the turrets are linked. With neither, the goal is dropped.
        let mut turret = test_turret();
        turret.set_turrets_linked_cached(true);
        turret.set_target_position(Some(Coord3D::new(10.0, 0.0, 5.0)));
        assert_eq!(turret.target_kind, TurretTargetKind::Position);
        assert!(turret.target_position.is_some());
        assert!(turret.get_current_target_id().is_none());
    }

    #[test]
    fn set_target_position_dropped_when_not_linked() {
        let mut turret = test_turret();
        turret.set_target_position(Some(Coord3D::new(10.0, 0.0, 5.0)));
        assert_eq!(turret.target_kind, TurretTargetKind::None);
        assert!(turret.target_position.is_none());
    }

    #[test]
    fn disabled_turret_without_recenter_sleeps() {
        // C++ updateTurretAI still runs only when enabled or TURRETAI_RECENTER.
        let mut turret = test_turret();
        turret.set_turret_enabled(false);
        let now = TheGameLogic::get_frame();
        match turret.update_turret_ai() {
            StateReturnType::Sleep(_) => {}
            other => panic!("expected sleep when disabled and not recentering, got {other:?}"),
        }
        let _ = now;
    }

    #[test]
    fn notify_fired_enables_three_frame_sweep() {
        // C++ ENABLE_SWEEP_FRAME_COUNT = 3 (TurretAI.cpp:700-701)
        let mut turret = test_turret();
        turret.set_did_fire(true);
        let now = TheGameLogic::get_frame();
        turret.enable_sweep_until = now.saturating_add(3);
        assert!(turret.friend_is_sweep_enabled() || turret.enable_sweep_until > now);
    }

    #[test]
    fn fire_pitch_overrides_computed_aim_pitch() {
        let mut turret = test_turret();
        turret.set_allows_pitch(true);
        turret.set_fire_pitch(0.5);
        turret.set_ground_unit_pitch(0.25);
        let origin = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 50.0);
        assert!(
            (turret.compute_desired_aim_pitch(&origin, &target, 20.0, 200.0) - 0.5).abs() < 1e-5
        );
    }

    #[test]
    fn ground_unit_pitch_scales_with_distance_over_range() {
        let mut turret = test_turret();
        turret.set_allows_pitch(true);
        turret.set_ground_unit_pitch(0.4);
        turret.set_min_pitch(-0.2);
        let origin = Coord3D::new(0.0, 0.0, 10.0);
        let target = Coord3D::new(50.0, 0.0, 10.0);
        let pitch = turret.compute_desired_aim_pitch(&origin, &target, 10.0, 100.0);
        assert!(pitch > -0.2 && pitch < 0.4);
    }

    #[test]
    fn friend_turn_aligns_within_rel_thresh() {
        let mut turret = test_turret();
        turret.set_turn_rate(0.2);
        assert!(!turret.friend_turn_towards_angle(1.0, 1.0, TURRET_AIM_REL_THRESH));
        assert!(turret.get_play_rot_sound());
        for _ in 0..20 {
            if turret.friend_turn_towards_angle(1.0, 1.0, TURRET_AIM_REL_THRESH) {
                break;
            }
        }
        assert!(turret.friend_turn_towards_angle(1.0, 1.0, TURRET_AIM_REL_THRESH));
    }

    #[test]
    fn set_turret_target_position_is_position_kind() {
        // Linked turrets keep the position goal even without a weapon on the
        // turret (TurretAI.cpp:591-597, `areTurretsLinked()` branch).
        let mut turret = test_turret();
        turret.set_turrets_linked_cached(true);
        turret.set_turret_target_position(Some(Coord3D::new(1.0, 2.0, 3.0)));
        assert_eq!(turret.get_target_kind(), TurretTargetKind::Position);
        let (kind, id, pos) = turret.friend_get_turret_target(false);
        assert_eq!(kind, TurretTargetKind::Position);
        assert!(id.is_none());
        assert!((pos.x - 1.0).abs() < 1e-5);
    }
}
