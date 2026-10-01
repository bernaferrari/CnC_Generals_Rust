use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::states::{AIAttackObjectState, AIEnterState, AIPickUpCrateState};
use crate::ai::{GuardMode, object_registry::get_legacy_object, the_ai, vision_factors};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::coord::*;
use crate::common::vector_ext::Vector3Ext;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::*;
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
use crate::game_logic::ai_internal_move_to_state::AIInternalMoveToState;
use crate::helpers::{TheGameLogic, ThePartitionManager, game_logic_random_value};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::polygon_trigger::PolygonTrigger;
use crate::state_machine::*;
use crate::terrain::get_terrain_logic;
use std::sync::{Arc, Mutex};

/// Wave 428: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

fn guard_attack_should_exit(
    exit_conditions: &Arc<Mutex<ExitConditions>>,
    goal_id: ObjectID,
) -> bool {
    let Ok(exit_guard) = exit_conditions.lock() else {
        return false;
    };
    exit_guard.should_exit_for_goal(goal_id, TheGameLogic::get_frame())
}

fn start_guard_attack_object(
    owner_id: ObjectID,
    nemesis_id: ObjectID,
    follow: bool,
    force: bool,
) -> Result<(AIAttackObjectState, StateReturnType), String> {
    // Snapshot ids before on_enter re-enters the registry.
    let scratch = StateMachine::new_with_owner_id(owner_id, "AIAttackObject");
    let mut attack_state = AIAttackObjectState::new(&scratch, force, follow);
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
    let Ok(data_guard) = data.read() else {
        return 30;
    };
    data_guard.guard_enemy_scan_rate
}

fn get_guard_chase_unit_frames() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 0;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 0;
    };
    data_guard.guard_chase_unit_frames
}

fn get_guard_enemy_return_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 60;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 60;
    };
    data_guard.guard_enemy_return_scan_rate
}

fn scan_guard_inner_target(
    owner_id: ObjectID,
    pos: &Coord3D,
    guard_mode: GuardMode,
    area: Option<&PolygonTrigger>,
) -> Option<ObjectID> {
    crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
    if !owner_guard.is_able_to_attack() {
        return None;
    }

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

#[derive(Debug)]
pub struct GuardSharedState {
    owner_id: ObjectID,
    // One cell keeps the machine configuration and deferred transition request coherent across
    // the contained state views that hold an Arc to this record.
    fields: Mutex<GuardSharedFields>,
}

#[derive(Debug)]
struct GuardSharedFields {
    target_to_guard: ObjectID,
    nemesis_to_attack: ObjectID,
    position_to_guard: Coord3D,
    area_to_guard: Option<Arc<PolygonTrigger>>,
    guard_mode: GuardMode,
    pending_state: Option<u32>,
}

impl GuardSharedState {
    fn new(owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            fields: Mutex::new(GuardSharedFields {
                target_to_guard: crate::common::INVALID_ID,
                nemesis_to_attack: crate::common::INVALID_ID,
                position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
                area_to_guard: None,
                guard_mode: GuardMode::Normal,
                pending_state: None,
            }),
        }
    }

    fn owner_id(&self) -> ObjectID {
        self.owner_id
    }

    fn request_state(&self, state: u32) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.pending_state = Some(state);
        }
    }

    fn take_pending_state(&self) -> Option<u32> {
        self.fields
            .lock()
            .ok()
            .and_then(|mut fields| fields.pending_state.take())
    }

    /// Request a transition. The owning machine applies it after the child
    /// update returns (see `update_machine_then_apply_pending_state`), which
    /// preserves the legacy callback order without re-entering the machine.
    fn request_state(&self, state: GuardStateType) {
        self.request_state_id(state as u32);
    }

    fn request_state_id(&self, state: u32) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.pending_state = Some(state);
        }
    }

    fn get_target_to_guard(&self) -> ObjectID {
        self.fields
            .lock()
            .map(|fields| fields.target_to_guard)
            .unwrap_or(crate::common::INVALID_ID)
    }

    fn set_target_to_guard(&self, id: ObjectID) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.target_to_guard = id;
        }
    }

    fn get_nemesis_to_attack(&self) -> ObjectID {
        self.fields
            .lock()
            .map(|fields| fields.nemesis_to_attack)
            .unwrap_or(crate::common::INVALID_ID)
    }

    fn set_nemesis_to_attack(&self, id: ObjectID) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.nemesis_to_attack = id;
        }
    }

    fn get_position_to_guard(&self) -> Coord3D {
        self.fields
            .lock()
            .map(|fields| fields.position_to_guard)
            .unwrap_or_else(|_| Coord3D::new(0.0, 0.0, 0.0))
    }

    fn set_position_to_guard(&self, pos: Coord3D) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.position_to_guard = pos;
        }
    }

    fn get_guard_mode(&self) -> GuardMode {
        self.fields
            .lock()
            .map(|fields| fields.guard_mode)
            .unwrap_or(GuardMode::Normal)
    }

    fn get_area_to_guard(&self) -> Option<Arc<PolygonTrigger>> {
        self.fields
            .lock()
            .ok()
            .and_then(|fields| fields.area_to_guard.as_ref().map(Arc::clone))
    }

    fn set_area_to_guard(&self, area: Option<Arc<PolygonTrigger>>) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.area_to_guard = area;
        }
    }

    fn set_guard_mode(&self, guard_mode: GuardMode) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.guard_mode = guard_mode;
        }
    }

    fn sync_from_machine(
        &self,
        target_to_guard: ObjectID,
        nemesis_to_attack: ObjectID,
        position_to_guard: Coord3D,
        area_to_guard: Option<Arc<PolygonTrigger>>,
        guard_mode: GuardMode,
    ) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.target_to_guard = target_to_guard;
            fields.nemesis_to_attack = nemesis_to_attack;
            fields.position_to_guard = position_to_guard;
            fields.area_to_guard = area_to_guard;
            fields.guard_mode = guard_mode;
        }
    }

    fn notify_state_change(&self) -> Result<(), String> {
        // Guard state notifications are not wired to external systems yet.
        Ok(())
    }
}

fn update_machine_then_apply_pending_state(
    machine: &mut StateMachine,
    shared: &GuardSharedState,
) -> StateReturnType {
    // Child states request transitions while this machine is already borrowed. Apply the request
    // only after the child update returns, preserving the legacy guard callback order.
    let result = machine.update();
    if let Some(state_id) = shared.take_pending_state() {
        let _ = machine.set_current_state(state_id);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug)]
    struct DummyState;

    impl StateImplementation for DummyState {
        fn update(&mut self) -> StateReturnType {
            StateReturnType::Continue
        }
    }

    #[derive(Debug)]
    struct QueueGuardTransition {
        shared: Arc<GuardSharedState>,
        update_finished: Arc<AtomicBool>,
        destination: u32,
    }

    impl StateImplementation for QueueGuardTransition {
        fn update(&mut self) -> StateReturnType {
            self.shared.request_state(self.destination);
            self.update_finished.store(true, Ordering::SeqCst);
            StateReturnType::Continue
        }
    }

    #[derive(Debug)]
    struct CheckGuardTransitionOrder(Arc<AtomicBool>);

    impl StateImplementation for CheckGuardTransitionOrder {
        fn on_enter(&mut self) -> StateReturnType {
            assert!(self.0.load(Ordering::SeqCst));
            StateReturnType::Continue
        }

        fn update(&mut self) -> StateReturnType {
            StateReturnType::Continue
        }
    }

    #[test]
    fn guard_shared_state_applies_pending_state_transition() {
        let mut machine = StateMachine::new_with_owner_id(crate::common::INVALID_ID, "test_guard");
        machine.define_state(
            GuardStateType::Inner as u32,
            Box::new(DummyState),
            None,
            None,
            None,
        );
        machine.define_state(
            GuardStateType::Outer as u32,
            Box::new(DummyState),
            None,
            None,
            None,
        );

        let shared = GuardSharedState::new(crate::common::INVALID_ID);
        shared.request_state(GuardStateType::Inner);
        let _ = machine.set_current_state(shared.take_pending_state().unwrap());
        shared.request_state(GuardStateType::Outer);
        let _ = machine.set_current_state(shared.take_pending_state().unwrap());

        let current = machine.get_current_state_id();
        assert_eq!(current, Some(GuardStateType::Outer as u32));
    }

    #[test]
    fn guard_shared_fields_are_visible_across_state_handles_but_isolated_per_machine() {
        let shared_a = Arc::new(GuardSharedState::new(77));
        let shared_b = Arc::new(GuardSharedState::new(78));
        let child_view_a = Arc::clone(&shared_a);

        shared_a.set_nemesis_to_attack(101);
        shared_b.set_nemesis_to_attack(202);
        child_view_a.set_target_to_guard(303);

        assert_eq!(child_view_a.get_nemesis_to_attack(), 101);
        assert_eq!(shared_b.get_nemesis_to_attack(), 202);
        assert_eq!(shared_a.get_target_to_guard(), 303);
        assert_eq!(shared_b.get_target_to_guard(), crate::common::INVALID_ID);
    }

    #[test]
    fn guard_pending_state_applies_only_after_child_update_returns() {
        let mut locked = StateMachine::new_with_owner_id(crate::common::INVALID_ID, "test_guard_pending_order");
        let shared = Arc::new(GuardSharedState::new(crate::common::INVALID_ID));
        let update_finished = Arc::new(AtomicBool::new(false));
        let destination = GuardStateType::Outer as u32;
        locked.define_state(
            GuardStateType::Inner as u32,
            Box::new(QueueGuardTransition {
                shared: Arc::clone(&shared),
                update_finished: Arc::clone(&update_finished),
                destination,
            }),
            None,
            None,
            None,
        );
        locked.define_state(
            destination,
            Box::new(CheckGuardTransitionOrder(Arc::clone(&update_finished))),
            None,
            None,
            None,
        );
        locked.init_default_state();
        assert_eq!(
            update_machine_then_apply_pending_state(&mut locked, &shared),
            StateReturnType::Continue
        );
        assert_eq!(locked.get_current_state_id(), Some(destination));
        assert!(update_finished.load(Ordering::SeqCst));
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
        let machine = StateMachine::new_with_owner_id(crate::common::INVALID_ID, "test_guard_return");
        let shared = Arc::new(GuardSharedState::new(crate::common::INVALID_ID));
        let ret = AIGuardReturnState::new(&machine, shared.clone());
        let _ = ret.move_helper.get_adjusts_destination();
        let crate_state = AIGuardPickUpCrateState::new(&machine, shared);
        assert!(crate_state.crate_state.is_none());
    }

    #[test]
    fn inner_outer_aggressor_wrap_ai_attack_object_state() {
        // C++ AIGuard.cpp:397/520/815 construct AIAttackState, not a bare AttackStateMachine.
        let machine = StateMachine::new_with_owner_id(crate::common::INVALID_ID, "test_guard_attack");
        let shared = Arc::new(GuardSharedState::new(crate::common::INVALID_ID));
        let inner = AIGuardInnerState::new(&machine, shared.clone());
        let outer = AIGuardOuterState::new(&machine, shared.clone());
        let agg = AIGuardAttackAggressorState::new(&machine, shared);
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
#[derive(Debug)]
pub struct AIGuardMachine {
    /// Base state machine
    base: StateMachine,
    /// Shared state used by guard states
    shared: Arc<GuardSharedState>,
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
}

impl AIGuardMachine {
    pub fn new(owner_id: ObjectID) -> Self {
        let base = StateMachine::new_with_owner_id(owner_id, "AIGuardMachine");
        let shared = Arc::new(GuardSharedState::new(owner_id));

        let mut machine = Self {
            base,
            shared,
            target_to_guard: crate::common::INVALID_ID,
            area_to_guard: None,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
            guard_mode: GuardMode::Normal,
        };

        // Define states - order matters: first state is default
        machine.define_guard_states();
        let _ = machine.base.init_default_state();
        machine
    }

    fn define_guard_states(&mut self) {
        let shared = self.shared.clone();

        let attack_aggressor_conditions_inner = vec![legacy_transition(
            guard_attack_aggressor_inner,
            GuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];
        let attack_aggressor_conditions_return = vec![legacy_transition(
            guard_attack_aggressor_return,
            GuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];
        let attack_aggressor_conditions_idle = vec![legacy_transition(
            guard_attack_aggressor_idle,
            GuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];

        // Build each state first (it only copies the machine's owner id), then
        // register — avoids overlapping borrows of `self.base`.
        let inner_state = AIGuardInnerState::new(&self.base, shared.clone());
        register_classic_state(
            &mut self.base,
            GuardStateType::Inner as u32,
            inner_state,
            Some(GuardStateType::Outer as u32),
            Some(GuardStateType::Outer as u32),
            &attack_aggressor_conditions_inner,
        );

        let return_state = AIGuardReturnState::new(&self.base, shared.clone());
        register_classic_state(
            &mut self.base,
            GuardStateType::Return as u32,
            return_state,
            Some(GuardStateType::Idle as u32),
            Some(GuardStateType::Inner as u32),
            &attack_aggressor_conditions_return,
        );

        let idle_state = AIGuardIdleState::new(&self.base, shared.clone());
        register_classic_state(
            &mut self.base,
            GuardStateType::Idle as u32,
            idle_state,
            Some(GuardStateType::Inner as u32),
            Some(GuardStateType::Return as u32),
            &attack_aggressor_conditions_idle,
        );

        let outer_state = AIGuardOuterState::new(&self.base, shared.clone());
        register_classic_state(
            &mut self.base,
            GuardStateType::Outer as u32,
            outer_state,
            Some(GuardStateType::GetCrate as u32),
            Some(GuardStateType::GetCrate as u32),
            &[],
        );

        let crate_state = AIGuardPickUpCrateState::new(&self.base, shared.clone());
        register_classic_state(
            &mut self.base,
            GuardStateType::GetCrate as u32,
            crate_state,
            Some(GuardStateType::Return as u32),
            Some(GuardStateType::Return as u32),
            &[],
        );

        let aggressor_state = AIGuardAttackAggressorState::new(&self.base, shared.clone());
        register_classic_state(
            &mut self.base,
            GuardStateType::AttackAggressor as u32,
            aggressor_state,
            Some(GuardStateType::Inner as u32),
            Some(GuardStateType::Inner as u32),
            &[],
        );
    }

    pub fn find_target_to_guard_by_id(&self) -> Option<ObjectID> {
        if self.target_to_guard == crate::common::INVALID_ID {
            return None;
        }
        get_legacy_object(self.target_to_guard)
    }

    pub fn set_target_to_guard(&mut self, object: Option<ObjectID>) {
        self.target_to_guard = object.unwrap_or(crate::common::INVALID_ID);
        self.shared.set_target_to_guard(self.target_to_guard);
    }

    pub fn get_position_to_guard(&self) -> &Coord3D {
        &self.position_to_guard
    }

    pub fn set_target_position_to_guard(&mut self, pos: &Coord3D) {
        self.position_to_guard = *pos;
        self.shared.set_position_to_guard(*pos);
    }

    pub fn get_area_to_guard(&self) -> Option<&Arc<PolygonTrigger>> {
        self.area_to_guard.as_ref()
    }

    pub fn set_area_to_guard(&mut self, area: Option<Arc<PolygonTrigger>>) {
        self.area_to_guard = area;
        self.shared.set_area_to_guard(self.area_to_guard.clone());
    }

    pub fn set_nemesis_id(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
        self.shared.set_nemesis_to_attack(id);
    }

    pub fn get_nemesis_id(&self) -> ObjectID {
        self.shared.get_nemesis_to_attack()
    }

    pub fn get_guard_mode(&self) -> GuardMode {
        self.guard_mode
    }

    pub fn set_guard_mode(&mut self, guard_mode: GuardMode) {
        self.guard_mode = guard_mode;
        self.shared.set_guard_mode(guard_mode);
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_state(&mut self, state: GuardStateType) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.base.is_in_guard_idle_state()
    }

    pub fn update(&mut self) -> StateReturnType {
        update_machine_then_apply_pending_state(&mut self.base, &self.shared)
    }

    pub fn look_for_inner_target(&mut self) -> bool {
        let Some(owner_id) = self.base.get_owner() else {
            return false;
        };
        // Snapshot team target before scan re-enters the registry.
        let team_target = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
            if !owner_guard.is_able_to_attack() {
                return Some(crate::common::INVALID_ID);
            }
            let team_id = owner_guard.get_team()?;
            crate::team::with_team(team_id, |team_guard| {
                if team_guard.attack_common_target() {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != INVALID_ID {
                        return Some(team_target);
                    }
                }
                None
            }).flatten()
        });
        match team_target {
            None => return false,
            Some(Some(id)) if id == crate::common::INVALID_ID => return false,
            Some(Some(id)) => {
                self.set_nemesis_id(id);
                return true;
            }
            Some(None) => {}
        }

        let area = self.get_area_to_guard().map(Arc::clone);
        let center = if let Some(area) = area.as_ref() {
            area.get_center_point()
        } else if let Some(target_id) = self.find_target_to_guard_by_id() {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |target| *target.get_position())
                .unwrap_or_else(|| *self.get_position_to_guard())
        } else {
            *self.get_position_to_guard()
        };

        if let Some(target_id) =
            scan_guard_inner_target(owner_id, &center, self.guard_mode, area.as_deref())
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
            self.base.xfer(xfer).map_err(|e| e.to_string())?;
        }

        if !xfer.is_loading() {
            self.nemesis_to_attack = self.shared.get_nemesis_to_attack();
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

        self.shared.sync_from_machine(
            self.target_to_guard,
            self.nemesis_to_attack,
            self.position_to_guard,
            self.area_to_guard.clone(),
            self.guard_mode,
        );

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        self.base
            .load_post_process()
            .map_err(|e| format!("guard load_post_process: {e}"))
    }
}

// State implementations

/// Shared helper for guard state implementations
#[derive(Debug)]
pub struct GuardState {
    base: State,
    shared: Arc<GuardSharedState>,
}

impl GuardState {
    fn new(machine: &StateMachine, shared: Arc<GuardSharedState>, name: &str) -> Self {
        Self {
            base: State::new(machine, name),
            shared,
        }
    }

    fn state(&self) -> &State {
        &self.base
    }

    fn state_mut(&mut self) -> &mut State {
        &mut self.base
    }


    fn get_target_to_guard(&self) -> ObjectID {
        self.shared.get_target_to_guard()
    }

    fn get_nemesis_to_attack(&self) -> ObjectID {
        self.shared.get_nemesis_to_attack()
    }

    fn set_nemesis_to_attack(&self, id: ObjectID) {
        self.shared.set_nemesis_to_attack(id);
    }

    fn get_position_to_guard(&self) -> Coord3D {
        self.shared.get_position_to_guard()
    }

    fn get_guard_mode(&self) -> GuardMode {
        self.shared.get_guard_mode()
    }

    fn get_area_to_guard(&self) -> Option<Arc<PolygonTrigger>> {
        self.shared.get_area_to_guard()
    }
}

/// Inner guard state - attack anything within area
#[derive(Debug)]
pub struct AIGuardInnerState {
    base: GuardState,
    exit_conditions: Arc<Mutex<ExitConditions>>,
    is_attacking: bool,
    attack_state: Option<AIAttackObjectState>,
    enter_state: Option<AIEnterState>,
}

impl AIGuardInnerState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardInner"),
            exit_conditions: Arc::new(Mutex::new(ExitConditions::new())),
            is_attacking: false,
            attack_state: None,
            enter_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardInnerState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardInnerState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .shared
            .owner_id();
        if owner == crate::common::INVALID_ID {
            return Err("guard inner missing owner".to_string());
        }
        let nemesis_id = self.base.get_nemesis_to_attack();
        let Some(nemesis) = (nemesis_id != crate::common::INVALID_ID)
            .then(|| get_legacy_object(nemesis_id))
            .flatten()
        else {
            self.is_attacking = false;
            self.attack_state = None;
            self.enter_state = None;
            return Ok(StateReturnType::Success);
        };

        let is_enter_guard = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner, |guard| guard.get_template().is_enter_guard())
            .unwrap_or(false);

        if is_enter_guard {
            let scratch = StateMachine::new_with_owner_id(owner, "AIEnter");
            let mut enter_state = AIEnterState::new(&scratch);
            enter_state.preset_goal_id = nemesis;
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(nemesis, |goal| *goal.get_position())
            {
                enter_state.goal_position = pos;
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

        let mut center = self.base.get_position_to_guard();
        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_to_guard, |target_guard| *target_guard.get_position())
            {
                center = pos;
            }
        }

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            let radius = AIGuardMachine::get_std_guard_range(owner);
            exit_guard.set_center(center);
            exit_guard.set_radius_sqr(radius * radius);
            exit_guard.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let (attack_state, result) = start_guard_attack_object(owner, nemesis, false, false)?;
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_state = Some(attack_state);
        self.enter_state = None;

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Some(attack_state) = self.attack_state.as_mut() {
            let target_to_guard = self.base.get_target_to_guard();
            if target_to_guard != crate::common::INVALID_ID {
                if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                    .with_object(target_to_guard, |target_guard| *target_guard.get_position())
                {
                    if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                        exit_guard.set_center(pos);
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

        let owner = self.base.shared.owner_id();
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            if let Some(team_id) = owner_guard.get_team() {
                crate::team::with_team_mut(team_id, |team_guard| {
                    team_guard.set_team_target_object(crate::common::INVALID_ID);
                });
            }
        });
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.is_attack()
    }

    fn classic_is_busy(&self) -> bool {
        true
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
    pub fn new(machine: &StateMachine, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardIdleState"),
            next_enemy_scan_time: 0,
            guardee_pos: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    pub fn is_guard_idle(&self) -> bool {
        true
    }
}

impl StateImplementation for AIGuardIdleState {
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

impl ClassicState for AIGuardIdleState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // C++ AIGuard.cpp:678-685 only arms the scan timer. m_guardeePos stays
        // at its zero default until update sees a per-axis move.
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_scan_rate();
        self.next_enemy_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return Ok(StateReturnType::Sleep(self.next_enemy_scan_time - now));
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());

        let owner = self
            .base
            .shared
            .owner_id();
        if owner == crate::common::INVALID_ID {
            return Err("guard idle missing owner".to_string());
        }
        let early = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if ai.get_crate_id() != crate::common::INVALID_ID {
                    return Some(StateReturnType::Sleep(0));
                }
            }
            if let Some(team_id) = owner_guard.get_team() {
                if crate::team::with_team(team_id, |team_guard| {
                    team_guard.attack_common_target()
                        && team_guard.get_team_target_object() != crate::common::INVALID_ID
                }).unwrap_or(false) {
                    return Some(StateReturnType::Success);
                }
            }
            None
        });
        // Re-read team target id outside? captured above only the signal.
        if let Some(StateReturnType::Sleep(_)) = early {
            self.base.shared.request_state(GuardStateType::GetCrate as u32);
            return Ok(StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now)));
        }
        if let Some(StateReturnType::Success) = early {
            // team target was seen; resolve again without holding owner.
            if let Some(team_target) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                owner_guard.get_team().and_then(|team_id| {
                    crate::team::with_team(team_id, |team_guard| {
                        if team_guard.attack_common_target() {
                            let id = team_guard.get_team_target_object();
                            if id != crate::common::INVALID_ID { Some(id) } else { None }
                        } else { None }
                    }).flatten()
                })
            }).flatten() {
                self.base.set_nemesis_to_attack(team_target);
                return Ok(StateReturnType::Success);
            }
        }

        let target_id = self.base.get_target_to_guard();
        let center = if target_id != crate::common::INVALID_ID {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |target| *target.get_position())
                .unwrap_or_else(|| self.base.get_position_to_guard())
        } else {
            self.base.get_position_to_guard()
        };

        let area = self.base.get_area_to_guard();
        if let Some(target_id) =
            scan_guard_inner_target(owner, &center, self.base.get_guard_mode(), area.as_deref())
        {
            if get_legacy_object(target_id).is_some() {
                self.base.set_nemesis_to_attack(target_id);
                return Ok(StateReturnType::Success);
            }
        }

        if target_id != crate::common::INVALID_ID {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |target_guard| *target_guard.get_position())
            {
                if guardee_moved_beyond_return_threshold(&self.guardee_pos, &pos) {
                    self.guardee_pos = pos;
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        Ok(StateReturnType::Sleep(self.next_enemy_scan_time - now))
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Cleanup when exiting idle guard state
        Ok(())
    }

    fn classic_is_guard_idle(&self) -> bool {
        true
    }
}

/// Outer guard state - attack aggressive targets with timer
#[derive(Debug)]
pub struct AIGuardOuterState {
    base: GuardState,
    exit_conditions: Arc<Mutex<ExitConditions>>,
    is_attacking: bool,
    attack_state: Option<AIAttackObjectState>,
}

impl AIGuardOuterState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardOuter"),
            exit_conditions: Arc::new(Mutex::new(ExitConditions::new())),
            is_attacking: false,
            attack_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardOuterState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardOuterState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        if matches!(self.base.get_guard_mode(), GuardMode::GuardWithoutPursuit) {
            return Ok(StateReturnType::Success);
        }

        let owner = self
            .base
            .shared
            .owner_id();
        if owner == crate::common::INVALID_ID {
            return Err("guard outer missing owner".to_string());
        }
        // C++ AIGuard.cpp:496 — getNemesisID only. No goal-object fallback.
        let nemesis_id = self.base.get_nemesis_to_attack();
        let Some(nemesis) = (if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        }) else {
            self.is_attacking = false;
            self.attack_state = None;
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = self.base.get_target_to_guard();
        let mut center = self.base.get_position_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_to_guard, |target_guard| *target_guard.get_position())
            {
                center = pos;
            }
        }

        let mut range = {
            let ai_store = the_ai();
            let ai = ai_store
                .read()
                .map_err(|_| "guard outer AI lock poisoned".to_string())?;
            ai.get_adjusted_vision_range_for_object(
                owner,
                vision_factors::OWNER_TYPE | vision_factors::MOOD,
            )
            .unwrap_or_else(|_| AIGuardMachine::get_std_guard_range(owner))
        };

        if let Some(area) = self.base.get_area_to_guard() {
            if range < area.get_radius() {
                range = area.get_radius();
            }
            center = area.get_center_point();
        }

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_center(center);
            exit_guard.set_radius_sqr(range * range);
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            exit_guard.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let (attack_state, result) = start_guard_attack_object(owner, nemesis, false, false)?;
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_state = Some(attack_state);

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 428: empty dual-world → Ok(Continue).
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Continue);
        }

        let Some(attack_state) = self.attack_state.as_mut() else {
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_to_guard, |target_guard| *target_guard.get_position())
            {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    exit_guard.set_center(pos);
                }
            }
        }

        let goal_id = attack_state.target_id;
        if goal_id != crate::common::INVALID_ID {
            if let Some(goal_pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(goal_id, |goal_guard| *goal_guard.get_position())
            {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    let delta = Coord3D::new(
                        exit_guard.center.x - goal_pos.x,
                        exit_guard.center.y - goal_pos.y,
                        exit_guard.center.z - goal_pos.z,
                    );
                    let owner = self.base.shared.owner_id();
                    let vision = AIGuardMachine::get_std_guard_range(owner);
                    if Vector3Ext::length_sqr(&delta) <= vision * vision {
                        exit_guard.set_attack_give_up_frame(
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

    fn classic_is_attack(&self) -> bool {
        self.is_attack()
    }

    fn classic_is_busy(&self) -> bool {
        true
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
    pub fn new(machine: &StateMachine, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardReturn"),
            next_return_scan_time: 0,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            move_helper: AIInternalMoveToState::new("AIGuardReturn".to_string()),
        }
    }
}

impl StateImplementation for AIGuardReturnState {
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

impl ClassicState for AIGuardReturnState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        self.goal_position = self.base.get_position_to_guard();
        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_to_guard, |g| *g.get_position())
            {
                self.goal_position = pos;
            }
        }
        if let Some(area) = self.base.get_area_to_guard() {
            self.goal_position = area.get_center_point();
        }

        let owner = self
            .base
            .shared
            .owner_id();
        if owner == crate::common::INVALID_ID {
            return Err("guard return missing owner".to_string());
        }
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
            if let Some(ai) = owner_guard.get_ai_update_interface_mut() {
                if ai.is_doing_ground_movement() {
                    let _ = ai.adjust_destination(&mut self.goal_position);
                }
            }
        });
        // C++ AIGuard.cpp:624-625 — setAdjustsDestination(true); AIInternalMoveToState::onEnter()
        self.move_helper.set_adjusts_destination(true);
        self.move_helper.set_goal_position(self.goal_position);
        self.move_helper.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now >= self.next_return_scan_time {
            self.next_return_scan_time = now.saturating_add(get_guard_enemy_return_scan_rate());

            let owner = self
                .base
                .shared
                .owner_id();
        if owner == crate::common::INVALID_ID {
            return Err("guard return missing owner".to_string());
        }
            let team_target = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                let team_id = owner_guard.get_team()?;
                crate::team::with_team(team_id, |team_guard| {
                    if team_guard.attack_common_target() {
                        let team_target = team_guard.get_team_target_object();
                        if team_target != crate::common::INVALID_ID {
                            return Some(team_target);
                        }
                    }
                    None
                }).flatten()
            }).flatten();
            if let Some(team_target) = team_target {
                self.base.set_nemesis_to_attack(team_target);
                return Ok(StateReturnType::Failure);
            }
            let target_id = self.base.get_target_to_guard();
            let center = if target_id != crate::common::INVALID_ID {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(target_id, |target| *target.get_position())
                    .unwrap_or_else(|| self.base.get_position_to_guard())
            } else {
                self.base.get_position_to_guard()
            };
            let area = self.base.get_area_to_guard();

            if let Some(target) = scan_guard_inner_target(
                owner,
                &center,
                self.base.get_guard_mode(),
                area.as_deref(),
            ) {
                self.base.set_nemesis_to_attack(target);
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

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Pick up crate state
#[derive(Debug)]
pub struct AIGuardPickUpCrateState {
    base: GuardState,
    crate_state: Option<AIPickUpCrateState>,
}

impl AIGuardPickUpCrateState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardPickUpCrate"),
            crate_state: None,
        }
    }
}

impl StateImplementation for AIGuardPickUpCrateState {
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

impl ClassicState for AIGuardPickUpCrateState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .shared
            .owner_id();
        if owner == crate::common::INVALID_ID {
            return Err("pick up crate missing owner".to_string());
        }

        let crate_id = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner, |owner_guard| owner_guard.ai_fire_crate_id)
            .ok_or_else(|| "pick up crate owner missing".to_string())?;
        if crate_id == crate::common::INVALID_ID {
            return Ok(StateReturnType::Success);
        }
        let crate_pos = crate::object::registry::OBJECT_REGISTRY
            .with_object(crate_id, |goal| *goal.get_position());

        let scratch = StateMachine::new_with_owner_id(owner, "AIPickUpCrate");
        let mut crate_state = AIPickUpCrateState::new(&scratch);
        crate_state.preset_goal_id = crate_id;
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

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Attack aggressor state - attack something that attacked us
#[derive(Debug)]
pub struct AIGuardAttackAggressorState {
    base: GuardState,
    exit_conditions: Arc<Mutex<ExitConditions>>,
    is_attacking: bool,
    attack_state: Option<AIAttackObjectState>,
}

impl AIGuardAttackAggressorState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardAttackAggressor"),
            exit_conditions: Arc::new(Mutex::new(ExitConditions::new())),
            is_attacking: false,
            attack_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardAttackAggressorState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardAttackAggressorState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .shared
            .owner_id();
        if owner == crate::common::INVALID_ID {
            return Err("guard aggressor missing owner".to_string());
        }

        // C++ AIGuard.cpp:791-798 — last damage source always overwrites nemesis.
        let last_damage_source = {
            let mut source = crate::common::INVALID_ID;
            if let Some(found) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                let body = owner_guard.get_body_module()?;
                let info = body.get_last_damage_info()?;
                Some(if info.input.source_id != crate::common::INVALID_ID {
                    info.input.source_id
                } else {
                    info.source_id
                })
            }).flatten() {
                source = found;
            }
            source
        };
        let existing_nemesis = self.base.get_nemesis_to_attack();
        let nemesis_id = last_damage_overrides_nemesis(last_damage_source, existing_nemesis);
        if nemesis_id != crate::common::INVALID_ID {
            self.base.set_nemesis_to_attack(nemesis_id);
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

        let mut center = self.base.get_position_to_guard();
        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_to_guard, |target_guard| *target_guard.get_position())
            {
                center = pos;
            }
        }

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            let radius = AIGuardMachine::get_std_guard_range(owner);
            exit_guard.set_center(center);
            exit_guard.set_radius_sqr(radius * radius);
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            exit_guard.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        // C++ AIGuard.cpp:815 — AIAttackState(machine, follow=true, attackingObject, !force)
        let (attack_state, result) = start_guard_attack_object(owner, nemesis, true, false)?;
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_state = Some(attack_state);

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(attack_state) = self.attack_state.as_mut() else {
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_to_guard, |target_guard| *target_guard.get_position())
            {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    exit_guard.set_center(pos);
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

        let owner = self.base.shared.owner_id();
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            if let Some(team_id) = owner_guard.get_team() {
                crate::team::with_team_mut(team_id, |team_guard| {
                    team_guard.set_team_target_object(crate::common::INVALID_ID);
                });
            }
        });
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.is_attack()
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Helper function to check if an object has attacked and can be retaliated against
pub fn has_attacked_me_and_i_can_return_fire(owner_id: ObjectID) -> bool {
    use crate::object::registry::OBJECT_REGISTRY;
    // Clear attacker under the owner borrow, then re-enter for the other id.
    let Some(last_attacker) = OBJECT_REGISTRY.with_object_mut(owner_id, |owner_ref| {
        let body_module = owner_ref.get_body_module_mut()?;
        let last_attacker = body_module.get_clearable_last_attacker();
        if last_attacker == crate::common::INVALID_ID {
            return None;
        }
        body_guard.clear_last_attacker();
        Some(last_attacker)
    }) else {
        return false;
    };
    let Some(last_attacker) = last_attacker else {
        return false;
    };
    OBJECT_REGISTRY
        .with_object(owner_id, |owner_ref| {
            OBJECT_REGISTRY.with_object(last_attacker, |target_guard| {
                if owner_ref.relationship_to(target_guard) != Relationship::Enemies {
                    return false;
                }
                if target_guard.is_effectively_dead() {
                    return false;
                }
                if !owner_ref.is_able_to_attack() {
                    return false;
                }
                matches!(
                    owner_ref.get_able_to_attack_specific_object(
                        AbleToAttackType::NewTarget,
                        target_guard,
                        CommandSourceType::FromAi,
                    ),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                )
            })
        })
        .flatten()
        .unwrap_or(false)
}

fn guard_attack_aggressor_common<T: ClassicState>(state: &T) -> Result<bool, String> {
    let owner_id = state
        .base_state()
        .get_machine_owner_id()
        .unwrap_or(crate::common::INVALID_ID);
    Ok(has_attacked_me_and_i_can_return_fire(owner_id))
}

fn guard_attack_aggressor_inner(
    state: &AIGuardInnerState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    guard_attack_aggressor_common(state)
}

fn guard_attack_aggressor_return(
    state: &AIGuardReturnState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    guard_attack_aggressor_common(state)
}

fn guard_attack_aggressor_idle(
    state: &AIGuardIdleState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    guard_attack_aggressor_common(state)
}
