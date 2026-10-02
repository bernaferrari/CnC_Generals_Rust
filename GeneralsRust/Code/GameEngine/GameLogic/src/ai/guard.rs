use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::states::{AIAttackObjectState, AIEnterState, AIPickUpCrateState};
use crate::ai::{GuardMode, object_registry::get_legacy_object, the_ai, vision_factors};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::coord::*;
use crate::common::vector_ext::Vector3Ext;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::*;
use crate::game_logic::ai_internal_move_to_state::AIInternalMoveToState;
use crate::helpers::{TheGameLogic, ThePartitionManager, game_logic_random_value};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::polygon_trigger::PolygonTrigger;
use crate::state_machine::*;
use crate::terrain::get_terrain_logic;
use std::sync::{Arc, RwLock, Weak};

/// Wave 428: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Resolve a guard machine's owner object from its id (C++ reached the owner
/// through the machine pointer; the owned machine keeps only the id).
fn guard_owner_arc_for_id(owner_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
    if owner_id == crate::common::INVALID_ID {
        return None;
    }
    TheGameLogic::find_object_by_id(owner_id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(owner_id))
}

fn guard_attack_should_exit(exit_conditions: &ExitConditions, goal_id: ObjectID) -> bool {
    exit_conditions.should_exit_for_goal(goal_id, TheGameLogic::get_frame())
}

fn start_guard_attack_object(
    owner: &Arc<RwLock<Object>>,
    nemesis_id: ObjectID,
    follow: bool,
    force: bool,
) -> Result<(AIAttackObjectState, StateReturnType), String> {
    let scratch = StateMachine::new(Some(Arc::downgrade(owner)), "AIAttackObject");
    let mut attack_state = AIAttackObjectState::new(&scratch, force, follow);
    attack_state.preset_owner = Some(owner.clone());
    attack_state.preset_goal_id = nemesis_id;
    let result = attack_state.on_enter();
    Ok((attack_state, result))
}

/// C++ `AIGuardIdleState::update` (`AIGuard.cpp:722-730`): per-axis
/// `|d| > 2 * PATHFIND_CELL_SIZE` via `delta*delta > 4*cell*cell`.
fn guardee_moved_beyond_return_threshold(guardee_pos: &Coord3D, current_pos: &Coord3D) -> bool {
    let limit_sqr = 4.0 * PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F;
    let dx = guardee_pos.x - current_pos.x;
    if dx * dx > limit_sqr {
        return true;
    }
    let dy = guardee_pos.y - current_pos.y;
    dy * dy > limit_sqr
}

/// C++ `AIGuardAttackAggressorState::onEnter` (`AIGuard.cpp:791-796`):
/// last-damage `sourceID` always overwrites a pre-existing nemesis.
fn last_damage_overrides_nemesis(
    last_damage_source: ObjectID,
    existing_nemesis: ObjectID,
) -> ObjectID {
    if last_damage_source != crate::common::INVALID_ID {
        last_damage_source
    } else {
        existing_nemesis
    }
}

fn get_guard_enemy_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 30;
    };
    let data = ai_guard.get_ai_data();
    data.guard_enemy_scan_rate
}

fn get_guard_chase_unit_frames() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 0;
    };
    let data = ai_guard.get_ai_data();
    data.guard_chase_unit_frames
}

fn get_guard_enemy_return_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 60;
    };
    let data = ai_guard.get_ai_data();
    data.guard_enemy_return_scan_rate
}

fn scan_guard_inner_target(
    owner_arc: &Arc<RwLock<Object>>,
    pos: &Coord3D,
    guard_mode: GuardMode,
    area: Option<&PolygonTrigger>,
) -> Option<ObjectID> {
    let Ok(owner_guard) = owner_arc.read() else {
        return None;
    };

    if !owner_guard.is_able_to_attack() {
        return None;
    }

    let is_enter_guard = owner_guard.get_template().is_enter_guard();
    let is_hijack_guard = owner_guard.get_template().is_hijack_guard();

    let mut vision_range = AIGuardMachine::get_std_guard_range(owner_guard.get_id());
    let mut center = *pos;
    if let Some(area) = area {
        let scan_rate = get_guard_enemy_scan_rate();
        let changed = TheGameLogic::get_frame_objects_changed_trigger_areas();
        // C++ AIGuard.cpp:250-253: frame > changed + scanRate returns false.
        // A zero stamp is not special; after scanRate frames the area scan stops.
        let check_frame = changed.saturating_add(scan_rate);
        if TheGameLogic::get_frame() > check_frame {
            return None;
        }
        vision_range = area.get_radius();
        center = area.get_center_point();
    }
    let flying_only = matches!(guard_mode, GuardMode::GuardFlyingUnitsOnly);
    let Some(partition) = ThePartitionManager::get() else {
        return None;
    };

    partition.get_closest_object_2d(&center, vision_range, |candidate| {
        if candidate.get_id() == owner_guard.get_id() {
            return false;
        }
        if candidate.is_effectively_dead() {
            return false;
        }
        if owner_guard.is_off_map() != candidate.is_off_map() {
            return false;
        }
        if flying_only && !candidate.is_airborne_target() && !candidate.is_kind_of(KindOf::Aircraft)
        {
            return false;
        }
        if let Some(area) = area {
            let position = candidate.get_position();
            if !area.point_in_trigger(&Coord2D::new(position.x, position.y)) {
                return false;
            }
        }

        if is_enter_guard {
            if is_hijack_guard {
                if owner_guard.relationship_to(candidate) != Relationship::Enemies {
                    return false;
                }
                return TheActionManager::can_hijack_vehicle(
                    &owner_guard,
                    candidate,
                    CommandSourceType::FromAi,
                );
            }

            if owner_guard.relationship_to(candidate) != Relationship::Neutral {
                return false;
            }
            return TheActionManager::can_enter_object(
                &owner_guard,
                candidate,
                CommandSourceType::FromAi,
                CanEnterType::CheckCapacity,
            );
        }

        if owner_guard.relationship_to(candidate) != Relationship::Enemies {
            return false;
        }
        matches!(
            owner_guard.get_able_to_attack_specific_object(
                AbleToAttackType::NewTarget,
                candidate,
                CommandSourceType::FromAi,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        )
    })
}

/// Guard state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardStateType {
    /// Attack anything within this area till death
    Inner = 5000,
    /// Wait till something shows up to attack
    Idle = 5001,
    /// Attack anything within this area that has been aggressive, until the timer expires
    Outer = 5002,
    /// Restore to a position within the inner circle
    Return = 5003,
    /// Pick up a crate from an enemy we killed
    GetCrate = 5004,
    /// Attack something that attacked me (that I can attack)
    AttackAggressor = 5005,
}

/// Exit conditions for attack states
#[derive(Debug, Clone)]
pub struct ExitConditions {
    /// Bitmask of conditions to consider
    conditions_to_consider: u32,
    /// Center position for radius checks
    center: Coord3D,
    /// Radius squared for distance checks
    radius_sqr: f32,
    /// Frame at which we give up attacking
    attack_give_up_frame: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Weak as StdWeak;

    #[test]
    fn guard_machine_settles_in_idle_and_stays_there() {
        // IDLE is the one guard state whose enter does not need the owner
        // object, so it lands (and stays) even with no resolvable owner.
        let mut machine = AIGuardMachine::new(StdWeak::new());
        machine.set_state(GuardStateType::Idle);
        assert_eq!(
            machine.state_machine.get_current_state_id(),
            Some(GuardStateType::Idle as u32)
        );
    }

    #[test]
    fn guard_states_copy_their_machine_owner_and_flags() {
        let machine = StateMachine::new_with_owner_id(77, "test_guard_states");
        let inner = AIGuardInnerState::new(&machine);
        let idle = AIGuardIdleState::new(&machine);
        let aggressor = AIGuardAttackAggressorState::new(&machine);

        // The owner id was copied once at define time — the states' only link
        // back to the machine that owns them.
        assert_eq!(inner.get_machine_owner_id().ok(), Some(77));
        assert_eq!(idle.get_machine_owner_id().ok(), Some(77));
        assert_eq!(aggressor.get_machine_owner_id().ok(), Some(77));

        assert!(idle.is_guard_idle());
        assert!(!idle.is_attack());
        assert!(!idle.is_busy()); // idle is the one guard state that is not busy
        assert!(inner.is_attack() == false); // not attacking until it enters
        assert_eq!(idle.get_name(), "AIGuardIdleState");
    }

    #[test]
    fn guard_machine_fields_are_isolated_per_machine() {
        let owner_a = Arc::new(RwLock::new(Object::new_test(77, 100.0)));
        let owner_b = Arc::new(RwLock::new(Object::new_test(78, 100.0)));

        let mut machine_a = AIGuardMachine::new(Arc::downgrade(&owner_a));
        let mut machine_b = AIGuardMachine::new(Arc::downgrade(&owner_b));

        machine_a.set_nemesis_id(101);
        machine_b.set_nemesis_id(202);
        machine_a.set_target_to_guard(Some(&owner_a));

        assert_eq!(machine_a.get_nemesis_id(), 101);
        assert_eq!(machine_b.get_nemesis_id(), 202);
        assert_ne!(machine_a.target_to_guard, crate::common::INVALID_ID);
        assert_eq!(machine_b.target_to_guard, crate::common::INVALID_ID);
    }

    #[test]
    fn guard_pending_state_applies_after_the_step_returns() {
        let mut machine = AIGuardMachine::new(StdWeak::new());
        machine.set_state(GuardStateType::Idle);

        // A state requests its transition mid-step through the loaned machine;
        // the request is queued here the same way and applied by `update` only
        // after the step's own transitions ran. IDLE re-entry is owner-free,
        // so the applied request lands deterministically.
        machine.pending_state = Some(GuardStateType::Idle as u32);
        machine.update();

        assert!(machine.pending_state.is_none(), "request must be consumed");
        assert_eq!(
            machine.state_machine.get_current_state_id(),
            Some(GuardStateType::Idle as u32)
        );
    }

    #[test]
    fn idle_follows_guardee_per_axis_two_cells_not_euclidean_four() {
        // C++ AIGuard.cpp:722-730 — `delta*delta > 4*PATHFIND_CELL_SIZE_F^2` per axis.
        // Pre-fix leftover used Euclidean length > 4 cells, so a 2.5-cell X-only
        // walk (25 world units) would not trip a return-to-post.
        let post = Coord3D::new(0.0, 0.0, 0.0);
        let x_only = Coord3D::new(PATHFIND_CELL_SIZE_F * 2.5, 0.0, 0.0);
        assert!(
            guardee_moved_beyond_return_threshold(&post, &x_only),
            "2.5 cells on X alone must return-to-post"
        );
        let diagonal_under =
            Coord3D::new(PATHFIND_CELL_SIZE_F * 1.5, PATHFIND_CELL_SIZE_F * 1.5, 0.0);
        assert!(
            !guardee_moved_beyond_return_threshold(&post, &diagonal_under),
            "1.5 cells on both axes stays idle (each axis < 2 cells)"
        );
        let exactly_two = Coord3D::new(PATHFIND_CELL_SIZE_F * 2.0, 0.0, 0.0);
        assert!(
            !guardee_moved_beyond_return_threshold(&post, &exactly_two),
            "exactly 2 cells is not greater than the C++ 4*cell*cell threshold"
        );
    }

    #[test]
    fn attack_aggressor_last_damage_overwrites_existing_nemesis() {
        // C++ AIGuard.cpp:791-796 — lastDamageInfo sourceID overwrites nemesis
        // even when a machine goal / prior inner nemesis is already set.
        let prior = 11;
        let attacker = 22;
        assert_eq!(last_damage_overrides_nemesis(attacker, prior), attacker);
        assert_eq!(
            last_damage_overrides_nemesis(crate::common::INVALID_ID, prior),
            prior
        );
        assert_eq!(
            last_damage_overrides_nemesis(attacker, crate::common::INVALID_ID),
            attacker
        );
    }

    #[test]
    fn return_and_crate_construct_internal_move_helpers() {
        // C++ AIGuard.h:193 AIGuardReturnState : public AIInternalMoveToState
        // C++ AIGuard.cpp:744 AIGuardPickUpCrateState : public AIPickUpCrateState
        let machine = StateMachine::new_with_owner_id(77, "test_guard_return");
        let ret = AIGuardReturnState::new(&machine);
        let _ = ret.move_helper.get_adjusts_destination();
        let crate_state = AIGuardPickUpCrateState::new(&machine);
        assert!(crate_state.crate_state.is_none());
    }

    #[test]
    fn inner_outer_aggressor_wrap_ai_attack_object_state() {
        // C++ AIGuard.cpp:397/520/815 construct AIAttackState, not a bare AttackStateMachine.
        let machine = StateMachine::new_with_owner_id(77, "test_guard_attack");
        let inner = AIGuardInnerState::new(&machine);
        let outer = AIGuardOuterState::new(&machine);
        let agg = AIGuardAttackAggressorState::new(&machine);
        let _: &Option<AIAttackObjectState> = &inner.attack_state;
        let _: &Option<AIAttackObjectState> = &outer.attack_state;
        let _: &Option<AIAttackObjectState> = &agg.attack_state;
        assert!(inner.attack_state.is_none());
        assert!(outer.attack_state.is_none());
        assert!(agg.attack_state.is_none());
    }
}

/// Exit condition flags
pub mod exit_conditions {
    pub const ATTACK_EXIT_IF_OUTSIDE_RADIUS: u32 = 0x01;
    pub const ATTACK_EXIT_IF_EXPIRED_DURATION: u32 = 0x02;
    pub const ATTACK_EXIT_IF_NO_UNIT_FOUND: u32 = 0x04;
}

impl ExitConditions {
    pub fn new() -> Self {
        Self {
            conditions_to_consider: 0,
            center: Coord3D::new(0.0, 0.0, 0.0),
            radius_sqr: 0.0,
            attack_give_up_frame: 0,
        }
    }

    pub fn should_exit(&self, machine: &StateMachine) -> bool {
        // Wave 428: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        let goal_object_id = machine.get_goal_object_id();

        if goal_object_id == crate::common::INVALID_ID {
            return (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND)
                != 0;
        }

        if (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION) != 0 {
            if machine.get_current_frame() >= self.attack_give_up_frame {
                return true;
            }
        }

        if (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS) != 0 {
            if let Some(outside) =
                crate::object::registry::OBJECT_REGISTRY.with_object(goal_object_id, |obj_ref| {
                    let obj_pos = obj_ref.get_position();
                    let delta = Coord3D::new(
                        obj_pos.x - self.center.x,
                        obj_pos.y - self.center.y,
                        0.0, // Don't account for Z in distance calculation
                    );
                    Vector3Ext::length_sqr(&delta) > self.radius_sqr
                })
            {
                if outside {
                    return true;
                }
            }
        }

        false
    }

    pub fn should_exit_for_goal(&self, goal_object_id: ObjectID, frame: u32) -> bool {
        if dual_world_registry_unavailable() {
            return false;
        }
        if goal_object_id == crate::common::INVALID_ID {
            return (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND)
                != 0;
        }
        if (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION) != 0
            && frame >= self.attack_give_up_frame
        {
            return true;
        }
        if (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS) != 0 {
            if let Some(true) =
                crate::object::registry::OBJECT_REGISTRY.with_object(goal_object_id, |obj_ref| {
                    let obj_pos = obj_ref.get_position();
                    let delta =
                        Coord3D::new(obj_pos.x - self.center.x, obj_pos.y - self.center.y, 0.0);
                    Vector3Ext::length_sqr(&delta) > self.radius_sqr
                })
            {
                return true;
            }
        }
        false
    }

    pub fn set_conditions(&mut self, conditions: u32) {
        self.conditions_to_consider = conditions;
    }

    pub fn set_center(&mut self, center: Coord3D) {
        self.center = center;
    }

    pub fn set_radius_sqr(&mut self, radius_sqr: f32) {
        self.radius_sqr = radius_sqr;
    }

    pub fn set_attack_give_up_frame(&mut self, frame: u32) {
        self.attack_give_up_frame = frame;
    }
}

/// Main guard state machine
///
/// C++ `AIGuardMachine` owns `StateMachine* m_stateMachine` and the guard
/// configuration (`m_targetToGuard`, `m_nemesisToAttack`, ...) as plain
/// members; the states reached it through the machine pointer
/// (`getMachine()`). Ownership is inverted here: the machine is a plain owned
/// field, states carry no handles, and the machine itself is loaned to each
/// state hook via `StateImplementation::update_with_owner` /
/// `on_enter_with_owner` — exactly the `this` context C++ passed through the
/// `StateMachine::update` flow. Nothing in this module is shared for the
/// machine: no Arc, no Mutex, no Weak. The only handle kept is `owner_id`,
/// resolved through the id-keyed registries on every use.
#[derive(Debug)]
pub struct AIGuardMachine {
    /// The guard's own state machine (C++ `m_stateMachine`). Owned, not
    /// shared: no Arc, no Mutex, no Weak. Taken out for each step so the
    /// states can be loaned this whole machine.
    state_machine: StateMachine,
    /// Owner object id (C++ `getOwner()`), resolved per use.
    owner_id: ObjectID,
    /// Object to guard by ID
    target_to_guard: ObjectID,
    /// Area to guard
    area_to_guard: Option<Arc<PolygonTrigger>>,
    /// Position to guard
    position_to_guard: Coord3D,
    /// Nemesis to attack
    nemesis_to_attack: ObjectID,
    /// Guard mode
    guard_mode: GuardMode,
    /// Transition a state requested while the machine was loaned out.
    /// Applied by [`AIGuardMachine::update`] after the step returns,
    /// preserving the legacy guard callback order.
    pending_state: Option<u32>,
}

impl AIGuardMachine {
    /// C++ `AIGuardMachine::AIGuardMachine(Object* owner)` split in two: this
    /// constructor keeps only the owner's id, then builds the machine and
    /// enters the default state — the same order as before, whose state
    /// definition + `initDefaultState()` ran inside the constructor.
    pub fn new(owner: Weak<RwLock<Object>>) -> Self {
        let owner_id = owner
            .upgrade()
            .and_then(|arc| arc.read().ok().map(|owner_ref| owner_ref.get_id()))
            .unwrap_or(crate::common::INVALID_ID);

        let mut machine = Self {
            state_machine: StateMachine::empty(),
            owner_id,
            target_to_guard: crate::common::INVALID_ID,
            area_to_guard: None,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
            guard_mode: GuardMode::Normal,
            pending_state: None,
        };
        machine.build_state_machine();
        machine
    }

    /// Defines the guard states and enters the default one. Runs once, the
    /// first time the machine is needed (C++ `AIGuardMachine` ctor tail +
    /// `initDefaultState()`).
    fn build_state_machine(&mut self) {
        if !self.state_machine.is_empty() {
            return;
        }
        let mut machine = StateMachine::new_with_owner_id(self.owner_id, "AIGuardMachine");
        self.define_guard_states(&mut machine);
        // Order matters: the first defined state (INNER) is the default, and
        // C++ enters it with `initDefaultState()` right in the ctor.
        let _ = machine.init_default_state_with_owner(self);
        self.state_machine = machine;
    }

    /// C++ `AIGuardMachine` ctor state definitions (`AIGuard.cpp:101-127`):
    /// define the six states, their success/failure links and the
    /// ATTACK_AGGRESSOR condition. States register directly (no legacy
    /// adapter) so their hooks can receive the loaned machine via
    /// `update_with_owner` / `on_enter_with_owner`.
    fn define_guard_states(&self, machine: &mut StateMachine) {
        let inner_id = GuardStateType::Inner as u32;
        let idle_id = GuardStateType::Idle as u32;
        let outer_id = GuardStateType::Outer as u32;
        let return_id = GuardStateType::Return as u32;
        let crate_id = GuardStateType::GetCrate as u32;
        let aggressor_id = GuardStateType::AttackAggressor as u32;

        let attack_aggressor_conditions = vec![StateConditionInfo::new(
            guard_attack_aggressor_condition,
            aggressor_id,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];

        machine.define_state(
            inner_id,
            Box::new(AIGuardInnerState::new(machine)),
            Some(outer_id),
            Some(outer_id),
            Some(&attack_aggressor_conditions),
        );

        machine.define_state(
            return_id,
            Box::new(AIGuardReturnState::new(machine)),
            Some(idle_id),
            Some(inner_id),
            Some(&attack_aggressor_conditions),
        );

        machine.define_state(
            idle_id,
            Box::new(AIGuardIdleState::new(machine)),
            Some(inner_id),
            Some(return_id),
            Some(&attack_aggressor_conditions),
        );

        machine.define_state(
            outer_id,
            Box::new(AIGuardOuterState::new(machine)),
            Some(crate_id),
            Some(crate_id),
            None,
        );

        machine.define_state(
            crate_id,
            Box::new(AIGuardPickUpCrateState::new(machine)),
            Some(return_id),
            Some(return_id),
            None,
        );

        machine.define_state(
            aggressor_id,
            Box::new(AIGuardAttackAggressorState::new(machine)),
            Some(inner_id),
            Some(inner_id),
            None,
        );
    }

    // ---- loan-safe accessors -------------------------------------------------
    //
    // `friend_*` methods run while a step has loaned `self` to a state, so
    // they must never touch `state_machine` (it is an empty placeholder during
    // the loan). Transitions are queued through
    // [`AIGuardMachine::friend_request_state`] and applied by `update`.

    fn friend_request_state(&mut self, state: u32) {
        self.pending_state = Some(state);
    }

    fn friend_owner_arc(&self) -> Option<Arc<RwLock<Object>>> {
        guard_owner_arc_for_id(self.owner_id)
    }

    fn friend_target_to_guard(&self) -> ObjectID {
        self.target_to_guard
    }

    fn friend_position_to_guard(&self) -> Coord3D {
        self.position_to_guard
    }

    fn friend_area_to_guard(&self) -> Option<Arc<PolygonTrigger>> {
        self.area_to_guard.clone()
    }

    fn friend_guard_mode(&self) -> GuardMode {
        self.guard_mode
    }

    fn friend_nemesis_to_attack(&self) -> ObjectID {
        self.nemesis_to_attack
    }

    fn friend_set_nemesis_to_attack(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
    }

    // ---- public API ----------------------------------------------------------

    pub fn find_target_to_guard_by_id(&self) -> Option<Arc<RwLock<Object>>> {
        if self.target_to_guard == crate::common::INVALID_ID {
            return None;
        }
        get_legacy_object(self.target_to_guard)
    }

    pub fn set_target_to_guard(&mut self, object: Option<&Arc<RwLock<Object>>>) {
        self.target_to_guard = if let Some(obj) = object {
            if let Ok(obj_ref) = obj.try_read() {
                obj_ref.get_id()
            } else {
                crate::common::INVALID_ID
            }
        } else {
            crate::common::INVALID_ID
        };
    }

    pub fn get_position_to_guard(&self) -> &Coord3D {
        &self.position_to_guard
    }

    pub fn set_target_position_to_guard(&mut self, pos: &Coord3D) {
        self.position_to_guard = *pos;
    }

    pub fn get_area_to_guard(&self) -> Option<&Arc<PolygonTrigger>> {
        self.area_to_guard.as_ref()
    }

    pub fn set_area_to_guard(&mut self, area: Option<Arc<PolygonTrigger>>) {
        self.area_to_guard = area;
    }

    pub fn set_nemesis_id(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
    }

    pub fn get_nemesis_id(&self) -> ObjectID {
        self.nemesis_to_attack
    }

    pub fn get_guard_mode(&self) -> GuardMode {
        self.guard_mode
    }

    pub fn set_guard_mode(&mut self, guard_mode: GuardMode) {
        self.guard_mode = guard_mode;
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        if self.state_machine.is_empty() {
            return StateReturnType::Failure;
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine.init_default_state_with_owner(self);
        self.state_machine = machine;
        result
    }

    pub fn set_state(&mut self, state: GuardStateType) -> StateReturnType {
        if self.state_machine.is_empty() {
            return StateReturnType::Failure;
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine.set_current_state_with_owner(state as u32, self);
        self.state_machine = machine;
        result
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.state_machine.halt()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.state_machine.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.state_machine.is_in_guard_idle_state()
    }

    /// One machine step. Child states request transitions through
    /// [`AIGuardMachine::friend_request_state`] while the machine is already
    /// loaned to them; the request is applied only after the child update
    /// returns, preserving the legacy guard callback order.
    pub fn update(&mut self) -> StateReturnType {
        if self.state_machine.is_empty() {
            return StateReturnType::Failure;
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine.update_with_owner(self);
        if let Some(state_id) = self.pending_state.take() {
            let _ = machine.set_current_state_with_owner(state_id, self);
        }
        self.state_machine = machine;
        result
    }

    pub fn look_for_inner_target(&mut self) -> bool {
        let Some(owner_arc) = self.friend_owner_arc() else {
            return false;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return false;
        };

        if !owner_guard.is_able_to_attack() {
            return false;
        }

        if let Some(team_arc) = owner_guard.get_team() {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.attack_common_target() {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != INVALID_ID {
                        self.set_nemesis_id(team_target);
                        return true;
                    }
                }
            }
        }

        let area = self.area_to_guard.clone();
        let center = if let Some(area) = area.as_ref() {
            area.get_center_point()
        } else if let Some(target_arc) = self.find_target_to_guard_by_id() {
            target_arc
                .read()
                .ok()
                .map(|target| *target.get_position())
                .unwrap_or_else(|| *self.get_position_to_guard())
        } else {
            *self.get_position_to_guard()
        };

        if let Some(target_id) =
            scan_guard_inner_target(&owner_arc, &center, self.guard_mode, area.as_deref())
        {
            self.set_nemesis_id(target_id);
            return true;
        }

        false
    }

    pub fn get_std_guard_range(obj_id: ObjectID) -> f32 {
        let ai_store = the_ai();
        let ai = ai_store.read().ok();
        ai.and_then(|ai| {
            ai.get_adjusted_vision_range_for_object(
                obj_id,
                vision_factors::OWNER_TYPE | vision_factors::MOOD | vision_factors::GUARD_INNER,
            )
            .ok()
        })
        .unwrap_or(100.0)
    }

    pub fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ AIGuardMachine::crc is empty.
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        if version >= 2 {
            self.state_machine
                .xfer(xfer)
                .map_err(|e| e.to_string())?;
        }

        xfer.xfer_object_id(&mut self.target_to_guard)
            .map_err(|e| format!("Failed to xfer target_to_guard: {:?}", e))?;
        xfer.xfer_object_id(&mut self.nemesis_to_attack)
            .map_err(|e| format!("Failed to xfer nemesis_to_attack: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.x)
            .map_err(|e| format!("Failed to xfer position_to_guard.x: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.y)
            .map_err(|e| format!("Failed to xfer position_to_guard.y: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.z)
            .map_err(|e| format!("Failed to xfer position_to_guard.z: {:?}", e))?;

        let mut trigger_name = self
            .area_to_guard
            .as_ref()
            .map(|area| area.get_trigger_name().str().to_string())
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut trigger_name)
            .map_err(|e| format!("Failed to xfer guard trigger name: {:?}", e))?;
        if xfer.is_loading() {
            self.area_to_guard = None;
            if !trigger_name.is_empty() {
                if let Ok(terrain) = get_terrain_logic().read() {
                    if let Some(trigger) = terrain.get_trigger_area_by_name(&trigger_name) {
                        self.area_to_guard = Some(Arc::new(trigger.clone()));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        if self.state_machine.is_empty() {
            return Ok(());
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine
            .load_post_process_with_owner(self)
            .map_err(|e| format!("guard load_post_process: {e}"));
        self.state_machine = machine;
        result
    }
}

// State implementations
//
// C++ guard states reached the machine through the machine pointer
// (`getMachine()`); the machine is owned by `AIGuardMachine` here, so states
// embed only the legacy `State` bookkeeping (id/name plus the machine's owner
// id copied once at define time) and receive the loaned machine through
// `update_with_owner` / `on_enter_with_owner` — the loaned-machine equivalent
// of C++'s `getMachine()`.

/// Shared helper for guard state implementations
#[derive(Debug)]
pub struct GuardState {
    base: State,
}

impl GuardState {
    fn new(machine: &StateMachine, name: &str) -> Self {
        // `State::new` copies the machine's owner id and attaches NO machine
        // handle: the machine owns its states, never the other way round.
        Self {
            base: State::new(machine, name),
        }
    }

    fn state(&self) -> &State {
        &self.base
    }

    fn state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn owner_arc(&self) -> Option<Arc<RwLock<Object>>> {
        guard_owner_arc_for_id(self.base.owner_id)
    }

    fn downcast_machine(owner: &mut dyn std::any::Any) -> Option<&mut AIGuardMachine> {
        owner.downcast_mut::<AIGuardMachine>()
    }
}

/// Owner of the state currently being tested for a conditional transition
/// (C++ `getMachine()->getOwner()`), resolved through the copied owner id.
fn guard_state_owner_arc(state: &dyn StateImplementation) -> Option<Arc<RwLock<Object>>> {
    guard_owner_arc_for_id(state.get_machine_owner_id().ok()?)
}

/// Inner guard state - attack anything within area
#[derive(Debug)]
pub struct AIGuardInnerState {
    base: GuardState,
    exit_conditions: ExitConditions,
    is_attacking: bool,
    attack_state: Option<AIAttackObjectState>,
    enter_state: Option<AIEnterState>,
}

impl AIGuardInnerState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardState::new(machine, "AIGuardInner"),
            exit_conditions: ExitConditions::new(),
            is_attacking: false,
            attack_state: None,
            enter_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }

    fn classic_on_enter(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        let owner = machine
            .friend_owner_arc()
            .ok_or_else(|| "guard inner missing owner".to_string())?;
        let nemesis_id = machine.friend_nemesis_to_attack();
        let Some(nemesis) = (nemesis_id != crate::common::INVALID_ID)
            .then(|| get_legacy_object(nemesis_id))
            .flatten()
        else {
            self.is_attacking = false;
            self.attack_state = None;
            self.enter_state = None;
            return Ok(StateReturnType::Success);
        };

        let is_enter_guard = owner
            .read()
            .map(|guard| guard.get_template().is_enter_guard())
            .unwrap_or(false);

        if is_enter_guard {
            let scratch = StateMachine::new(Some(Arc::downgrade(&owner)), "AIEnter");
            let mut enter_state = AIEnterState::new(&scratch);
            enter_state.preset_owner = Some(owner.clone());
            enter_state.preset_goal_id = nemesis
                .read()
                .ok()
                .map(|goal| goal.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            if let Ok(goal) = nemesis.read() {
                enter_state.goal_position = *goal.get_position();
            }
            self.is_attacking = false;
            self.attack_state = None;
            self.enter_state = Some(enter_state);
            if let Some(enter_state) = self.enter_state.as_mut() {
                let result = enter_state.on_enter();
                if result == StateReturnType::Continue {
                    return Ok(StateReturnType::Continue);
                }
            }
            return Ok(StateReturnType::Success);
        }

        let mut center = machine.friend_position_to_guard();
        let target_to_guard = machine.friend_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    center = *target_guard.get_position();
                }
            }
        }

        {
            let radius = owner
                .read()
                .ok()
                .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                .unwrap_or(100.0);
            self.exit_conditions.set_center(center);
            self.exit_conditions.set_radius_sqr(radius * radius);
            self.exit_conditions.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let nemesis_id = nemesis
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(nemesis_id);
        let (attack_state, result) = start_guard_attack_object(&owner, nemesis_id, false, false)?;
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_state = Some(attack_state);
        self.enter_state = None;

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        if let Some(attack_state) = self.attack_state.as_mut() {
            let target_to_guard = machine.friend_target_to_guard();
            if target_to_guard != crate::common::INVALID_ID {
                if let Some(target_arc) = get_legacy_object(target_to_guard) {
                    if let Ok(target_guard) = target_arc.read() {
                        self.exit_conditions.set_center(*target_guard.get_position());
                    }
                }
            }
            if guard_attack_should_exit(&self.exit_conditions, attack_state.target_id) {
                return Ok(StateReturnType::Success);
            }
            return Ok(attack_state.update());
        }

        if let Some(enter_state) = self.enter_state.as_mut() {
            return Ok(enter_state.update());
        }

        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut attack_state) = self.attack_state.take() {
            attack_state.on_exit(_exit);
        }
        if let Some(mut enter_state) = self.enter_state.take() {
            enter_state.on_exit(_exit);
        }
        self.is_attacking = false;

        if let Some(owner) = self.base.owner_arc() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            }
        }
        Ok(())
    }
}

impl StateImplementation for AIGuardInnerState {
    /// Guard states are only stepped through their owner's machine, which
    /// always loans the machine via `update_with_owner`; there is no owner-free
    /// step to run here.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_update(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    /// C++ `AIGuardInnerState::loadPostProcess` re-runs `onEnter` to rebuild
    /// the attack child against the owner's live fields.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        match GuardState::downcast_machine(owner) {
            Some(machine) => {
                let _ = self.classic_on_enter(machine);
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_attack(&self) -> bool {
        self.is_attack()
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Idle guard state - wait for targets to appear
#[derive(Debug)]
pub struct AIGuardIdleState {
    base: GuardState,
    next_enemy_scan_time: u32,
    guardee_pos: Coord3D,
}

impl AIGuardIdleState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardState::new(machine, "AIGuardIdleState"),
            next_enemy_scan_time: 0,
            guardee_pos: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    pub fn is_guard_idle(&self) -> bool {
        true
    }

    fn classic_on_enter(
        &mut self,
        _machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        // C++ AIGuard.cpp:678-685 only arms the scan timer. m_guardeePos stays
        // at its zero default until update sees a per-axis move.
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_scan_rate();
        self.next_enemy_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return Ok(StateReturnType::Sleep(self.next_enemy_scan_time - now));
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());

        let owner = machine
            .friend_owner_arc()
            .ok_or_else(|| "guard idle missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "guard idle owner lock poisoned".to_string())?;

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                if ai_guard.get_crate_id() != crate::common::INVALID_ID {
                    machine.friend_request_state(GuardStateType::GetCrate as u32);
                    return Ok(StateReturnType::Sleep(
                        self.next_enemy_scan_time.saturating_sub(now),
                    ));
                }
            }
        }

        if let Some(team_arc) = owner_guard.get_team() {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.attack_common_target() {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != crate::common::INVALID_ID {
                        machine.friend_set_nemesis_to_attack(team_target);
                        return Ok(StateReturnType::Success);
                    }
                }
            }
        }

        drop(owner_guard);

        let target_id = machine.friend_target_to_guard();
        let center = if target_id != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_id) {
                target_arc
                    .read()
                    .ok()
                    .map(|target| *target.get_position())
                    .unwrap_or_else(|| machine.friend_position_to_guard())
            } else {
                machine.friend_position_to_guard()
            }
        } else {
            machine.friend_position_to_guard()
        };

        let area = machine.friend_area_to_guard();
        if let Some(target_id) =
            scan_guard_inner_target(&owner, &center, machine.friend_guard_mode(), area.as_deref())
        {
            if let Some(target_arc) = get_legacy_object(target_id) {
                machine.friend_set_nemesis_to_attack(target_id);
                return Ok(StateReturnType::Success);
            }
        }

        if target_id != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    let pos = *target_guard.get_position();
                    if guardee_moved_beyond_return_threshold(&self.guardee_pos, &pos) {
                        self.guardee_pos = pos;
                        return Ok(StateReturnType::Failure);
                    }
                }
            }
        }

        Ok(StateReturnType::Sleep(self.next_enemy_scan_time - now))
    }
}

impl StateImplementation for AIGuardIdleState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_update(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {}

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_guard_idle(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Outer guard state - attack aggressive targets with timer
#[derive(Debug)]
pub struct AIGuardOuterState {
    base: GuardState,
    exit_conditions: ExitConditions,
    is_attacking: bool,
    attack_state: Option<AIAttackObjectState>,
}

impl AIGuardOuterState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardState::new(machine, "AIGuardOuter"),
            exit_conditions: ExitConditions::new(),
            is_attacking: false,
            attack_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }

    fn classic_on_enter(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        if matches!(machine.friend_guard_mode(), GuardMode::GuardWithoutPursuit) {
            return Ok(StateReturnType::Success);
        }

        let owner = machine
            .friend_owner_arc()
            .ok_or_else(|| "guard outer missing owner".to_string())?;
        // C++ AIGuard.cpp:496 — getNemesisID only. No goal-object fallback.
        let nemesis_id = machine.friend_nemesis_to_attack();
        let Some(nemesis) = (if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        }) else {
            self.is_attacking = false;
            self.attack_state = None;
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = machine.friend_target_to_guard();
        let mut center = machine.friend_position_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    center = *target_guard.get_position();
                }
            }
        }

        let mut range = {
            let ai_store = the_ai();
            let ai = ai_store
                .read()
                .map_err(|_| "guard outer AI lock poisoned".to_string())?;
            ai.get_adjusted_vision_range_for_object(
                owner
                    .read()
                    .map_err(|_| "guard outer owner lock poisoned".to_string())?
                    .get_id(),
                vision_factors::OWNER_TYPE | vision_factors::MOOD,
            )
            .unwrap_or_else(|_| {
                owner
                    .read()
                    .ok()
                    .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                    .unwrap_or(100.0)
            })
        };

        if let Some(area) = machine.friend_area_to_guard() {
            if range < area.get_radius() {
                range = area.get_radius();
            }
            center = area.get_center_point();
        }

        {
            self.exit_conditions.set_center(center);
            self.exit_conditions.set_radius_sqr(range * range);
            self.exit_conditions.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            self.exit_conditions.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let nemesis_id = nemesis
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(nemesis_id);
        let (attack_state, result) = start_guard_attack_object(&owner, nemesis_id, false, false)?;
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_state = Some(attack_state);

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        // Wave 428: empty dual-world → Ok(Continue).
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Continue);
        }

        let Some(attack_state) = self.attack_state.as_mut() else {
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = machine.friend_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    self.exit_conditions.set_center(*target_guard.get_position());
                }
            }
        }

        let goal_id = attack_state.target_id;
        if goal_id != crate::common::INVALID_ID {
            if let Some(goal_pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(goal_id, |goal_guard| *goal_guard.get_position())
            {
                {
                    let delta = Coord3D::new(
                        self.exit_conditions.center.x - goal_pos.x,
                        self.exit_conditions.center.y - goal_pos.y,
                        self.exit_conditions.center.z - goal_pos.z,
                    );
                    let vision = machine
                        .friend_owner_arc()
                        .and_then(|owner| {
                            owner
                                .read()
                                .ok()
                                .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                        })
                        .unwrap_or(100.0);
                    if Vector3Ext::length_sqr(&delta) <= vision * vision {
                        self.exit_conditions.set_attack_give_up_frame(
                            TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
                        );
                    }
                }
            }
        }

        if guard_attack_should_exit(&self.exit_conditions, attack_state.target_id) {
            return Ok(StateReturnType::Success);
        }
        Ok(attack_state.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut attack_state) = self.attack_state.take() {
            attack_state.on_exit(_exit);
        }
        self.is_attacking = false;
        Ok(())
    }
}

impl StateImplementation for AIGuardOuterState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_update(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    /// C++ `AIGuardOuterState::loadPostProcess` re-runs `onEnter`.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        match GuardState::downcast_machine(owner) {
            Some(machine) => {
                let _ = self.classic_on_enter(machine);
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_attack(&self) -> bool {
        self.is_attack()
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Return guard state - move back to guard position
#[derive(Debug)]
pub struct AIGuardReturnState {
    base: GuardState,
    next_return_scan_time: u32,
    goal_position: Coord3D,
    move_helper: AIInternalMoveToState,
}

impl AIGuardReturnState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardState::new(machine, "AIGuardReturn"),
            next_return_scan_time: 0,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            move_helper: AIInternalMoveToState::new_with_owner_id(
                machine.get_owner_id(),
                "AIGuardReturn".to_string(),
            ),
        }
    }

    fn classic_on_enter(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        self.goal_position = machine.friend_position_to_guard();
        let target_to_guard = machine.friend_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    self.goal_position = *target_guard.get_position();
                }
            }
        }
        if let Some(area) = machine.friend_area_to_guard() {
            self.goal_position = area.get_center_point();
        }

        if let Some(owner) = machine.friend_owner_arc() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if ai_guard.is_doing_ground_movement() {
                            let _ = ai_guard.adjust_destination(&mut self.goal_position);
                        }
                    }
                }
            }
        }
        // C++ AIGuard.cpp:624-625 — setAdjustsDestination(true); AIInternalMoveToState::onEnter()
        self.move_helper.set_adjusts_destination(true);
        self.move_helper.set_goal_position(self.goal_position);
        self.move_helper.on_enter()
    }

    fn classic_on_update(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now >= self.next_return_scan_time {
            self.next_return_scan_time = now.saturating_add(get_guard_enemy_return_scan_rate());

            let owner = machine
                .friend_owner_arc()
                .ok_or_else(|| "guard return missing owner".to_string())?;
            if let Ok(owner_guard) = owner.read() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(team_guard) = team_arc.read() {
                        if team_guard.attack_common_target() {
                            let team_target = team_guard.get_team_target_object();
                            if team_target != crate::common::INVALID_ID {
                                machine.friend_set_nemesis_to_attack(team_target);
                                return Ok(StateReturnType::Failure);
                            }
                        }
                    }
                }
            }
            let target_id = machine.friend_target_to_guard();
            let center = if target_id != crate::common::INVALID_ID {
                if let Some(target_arc) = get_legacy_object(target_id) {
                    target_arc
                        .read()
                        .ok()
                        .map(|target| *target.get_position())
                        .unwrap_or_else(|| machine.friend_position_to_guard())
                } else {
                    machine.friend_position_to_guard()
                }
            } else {
                machine.friend_position_to_guard()
            };
            let area = machine.friend_area_to_guard();

            if let Some(target) =
                scan_guard_inner_target(&owner, &center, machine.friend_guard_mode(), area.as_deref())
            {
                machine.friend_set_nemesis_to_attack(target);
                return Ok(StateReturnType::Failure);
            }
        }

        // C++ AIGuard.cpp:640 — return AIInternalMoveToState::update()
        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // C++ AIGuard.cpp:646 — AIInternalMoveToState::onExit(status)
        self.move_helper.on_exit(_exit)
    }
}

impl StateImplementation for AIGuardReturnState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_update(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Pick up crate state
#[derive(Debug)]
pub struct AIGuardPickUpCrateState {
    base: GuardState,
    crate_state: Option<AIPickUpCrateState>,
}

impl AIGuardPickUpCrateState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardState::new(machine, "AIGuardPickUpCrate"),
            crate_state: None,
        }
    }

    fn classic_on_enter(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        let owner = machine
            .friend_owner_arc()
            .ok_or_else(|| "pick up crate missing owner".to_string())?;

        let owner_guard = owner
            .read()
            .map_err(|_| "pick up crate owner lock poisoned".to_string())?;
        let crate_id = owner_guard.ai_fire_crate_id;
        if crate_id == crate::common::INVALID_ID {
            return Ok(StateReturnType::Success);
        }
        let crate_pos = TheGameLogic::find_object_by_id(crate_id)
            .and_then(|crate_obj| crate_obj.read().ok().map(|goal| *goal.get_position()));
        drop(owner_guard);

        let scratch = StateMachine::new(Some(Arc::downgrade(&owner)), "AIPickUpCrate");
        let mut crate_state = AIPickUpCrateState::new(&scratch);
        crate_state.preset_goal_id = crate_id;
        crate_state.base.preset_owner = Some(owner);
        if let Some(pos) = crate_pos {
            crate_state.goal_position = pos;
            crate_state.base.goal_position = pos;
        }
        let result = crate_state.on_enter();
        self.crate_state = Some(crate_state);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(crate_state) = self.crate_state.as_mut() else {
            return Ok(StateReturnType::Success);
        };
        Ok(crate_state.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // C++ AIGuardPickUpCrateState::onExit is empty. It does not call
        // AIPickUpCrateState::onExit.
        self.crate_state = None;
        Ok(())
    }
}

impl StateImplementation for AIGuardPickUpCrateState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            // The crate wrapper needs no machine fields while stepping.
            Some(_) => self.classic_on_update().unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Attack aggressor state - attack something that attacked us
#[derive(Debug)]
pub struct AIGuardAttackAggressorState {
    base: GuardState,
    exit_conditions: ExitConditions,
    is_attacking: bool,
    attack_state: Option<AIAttackObjectState>,
}

impl AIGuardAttackAggressorState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardState::new(machine, "AIGuardAttackAggressor"),
            exit_conditions: ExitConditions::new(),
            is_attacking: false,
            attack_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }

    fn classic_on_enter(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        let owner = machine
            .friend_owner_arc()
            .ok_or_else(|| "guard aggressor missing owner".to_string())?;

        // C++ AIGuard.cpp:791-798 — last damage source always overwrites nemesis.
        let last_damage_source = {
            let mut source = crate::common::INVALID_ID;
            if let Ok(owner_guard) = owner.read() {
                if let Some(body) = owner_guard.get_body_module() {
                    if let Ok(body_guard) = body.lock() {
                        if let Some(info) = body_guard.get_last_damage_info() {
                            source = if info.input.source_id != crate::common::INVALID_ID {
                                info.input.source_id
                            } else {
                                info.source_id
                            };
                        }
                    }
                }
            }
            source
        };
        let existing_nemesis = machine.friend_nemesis_to_attack();
        let nemesis_id = last_damage_overrides_nemesis(last_damage_source, existing_nemesis);
        if nemesis_id != crate::common::INVALID_ID {
            machine.friend_set_nemesis_to_attack(nemesis_id);
        }

        let Some(nemesis) = (if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        }) else {
            self.is_attacking = false;
            self.attack_state = None;
            return Ok(StateReturnType::Success);
        };

        let mut center = machine.friend_position_to_guard();
        let target_to_guard = machine.friend_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    center = *target_guard.get_position();
                }
            }
        }

        {
            let radius = owner
                .read()
                .ok()
                .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                .unwrap_or(100.0);
            self.exit_conditions.set_center(center);
            self.exit_conditions.set_radius_sqr(radius * radius);
            self.exit_conditions.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            self.exit_conditions.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let nemesis_id = nemesis
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(nemesis_id);
        // C++ AIGuard.cpp:815 — AIAttackState(machine, follow=true, attackingObject, !force)
        let (attack_state, result) = start_guard_attack_object(&owner, nemesis_id, true, false)?;
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_state = Some(attack_state);

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(
        &mut self,
        machine: &mut AIGuardMachine,
    ) -> Result<StateReturnType, String> {
        let Some(attack_state) = self.attack_state.as_mut() else {
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = machine.friend_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    self.exit_conditions.set_center(*target_guard.get_position());
                }
            }
        }

        if guard_attack_should_exit(&self.exit_conditions, attack_state.target_id) {
            return Ok(StateReturnType::Success);
        }
        Ok(attack_state.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut attack_state) = self.attack_state.take() {
            attack_state.on_exit(_exit);
        }
        self.is_attacking = false;

        if let Some(owner) = self.base.owner_arc() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            }
        }
        Ok(())
    }
}

impl StateImplementation for AIGuardAttackAggressorState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_update(machine).unwrap_or(StateReturnType::Failure),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    /// C++ `AIGuardAttackAggressorState::loadPostProcess` re-runs `onEnter`.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        match GuardState::downcast_machine(owner) {
            Some(machine) => {
                let _ = self.classic_on_enter(machine);
                Ok(())
            }
            None => Ok(()),
        }
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_attack(&self) -> bool {
        self.is_attack()
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Helper function to check if an object has attacked and can be retaliated against
pub fn has_attacked_me_and_i_can_return_fire(machine: &StateMachine) -> bool {
    machine.get_owner().as_ref().is_some_and(has_attacked_me_from_owner)
}

fn has_attacked_me_from_owner(owner: &Arc<RwLock<Object>>) -> bool {
    if let Ok(owner_ref) = owner.try_read() {
        if let Some(body_module) = owner_ref.get_body_module() {
            if let Ok(mut body_guard) = body_module.lock() {
                let last_attacker = body_guard.get_clearable_last_attacker();
                if last_attacker == crate::common::INVALID_ID {
                    return false;
                }

                body_guard.clear_last_attacker();

                let Some(target_arc) = TheGameLogic::find_object_by_id(last_attacker) else {
                    return false;
                };
                let Ok(target_guard) = target_arc.read() else {
                    return false;
                };

                if owner_ref.relationship_to(&target_guard) != Relationship::Enemies {
                    return false;
                }

                if target_guard.is_effectively_dead() {
                    return false;
                }

                // C++ AIGuard.cpp:78 — isAbleToAttack before the specific-object test.
                if !owner_ref.is_able_to_attack() {
                    return false;
                }

                matches!(
                    owner_ref.get_able_to_attack_specific_object(
                        AbleToAttackType::NewTarget,
                        &target_guard,
                        CommandSourceType::FromAi,
                    ),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                )
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}

/// C++ `AIGuardState`'s per-state `ATTACK_AGGRESSOR` condition. The states no
/// longer carry a machine handle, so the owner comes from the state's copied
/// owner id — the loaned-machine equivalent of `getMachine()->getOwner()`.
fn guard_attack_aggressor_condition(
    state: &dyn StateImplementation,
    _user_data: &StateTransitionUserData,
) -> bool {
    guard_state_owner_arc(state).as_ref().is_some_and(has_attacked_me_from_owner)
}
