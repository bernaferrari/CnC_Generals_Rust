//! Live control owned separately from the state bodies it drives.
//!
//! These are the machine's authoritative fields, not a callback snapshot.
//! Keeping them disjoint lets a state borrow control during a synchronous
//! callback without reacquiring its enclosing machine.

use super::*;

#[derive(Debug)]
pub struct StateMachineControl {
    pub(super) owner_id: crate::common::ObjectID,
    pub(super) owner: Option<Weak<RwLock<Object>>>,
    pub(super) sleep_till: u32,
    pub(super) default_state_id: StateId,
    pub(super) current_state_id: Option<StateId>,
    pub(super) goal_object_id: crate::common::ObjectID,
    pub(super) goal_squad: Option<Weak<Squad>>,
    pub(super) goal_polygon: Option<Weak<PolygonTrigger>>,
    pub(super) goal_waypoint: Option<WaypointId>,
    pub(super) guard_mode_raw: i32,
    pub(super) goal_position: Coord3D,
    pub(super) locked: bool,
    pub(super) default_state_inited: bool,
    pub(super) name: String,
    pub(super) debug_output: bool,
    pub(super) transition_depth: u32,
    pub(super) sleep_transition_depth: u32,
}

impl StateMachineControl {
    pub(super) fn new(owner_id: crate::common::ObjectID, name: &str) -> Self {
        Self {
            owner_id,
            owner: None,
            sleep_till: 0,
            default_state_id: INVALID_STATE_ID,
            current_state_id: None,
            goal_object_id: crate::common::INVALID_ID,
            goal_squad: None,
            goal_polygon: None,
            goal_waypoint: None,
            guard_mode_raw: 0,
            goal_position: Coord3D::origin(),
            locked: false,
            default_state_inited: false,
            name: name.to_string(),
            debug_output: false,
            transition_depth: 0,
            sleep_transition_depth: 0,
        }
    }

    pub(super) fn internal_clear(&mut self) {
        self.goal_object_id = crate::common::INVALID_ID;
        self.goal_squad = None;
        self.goal_polygon = None;
        self.goal_waypoint = None;
        self.guard_mode_raw = 0;
        self.goal_position = Coord3D::origin();
    }

    pub(super) fn internal_set_goal_object(&mut self, obj: Option<Weak<RwLock<Object>>>) {
        if let Some(weak) = obj {
            if let Some(strong) = weak.upgrade() {
                if let Ok(guard) = strong.read() {
                    self.goal_object_id = guard.get_id();
                    self.internal_set_goal_position(guard.get_position().clone());
                    return;
                }
            }
        }

        self.goal_object_id = crate::common::INVALID_ID;
    }

    pub(super) fn internal_set_goal_position(&mut self, pos: Coord3D) {
        self.goal_position = pos;
    }

    /// Lock/unlock this state machine
    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Get the owner object
    pub fn get_owner(&self) -> Option<Arc<RwLock<Object>>> {
        if let Some(owner) = &self.owner {
            return owner.upgrade();
        }
        if self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.owner_id))
    }

    pub(crate) fn owner_reference(&self) -> Option<Weak<RwLock<Object>>> {
        self.owner.clone()
    }

    pub fn get_owner_id(&self) -> crate::common::ObjectID {
        self.owner_id
    }

    pub fn set_owner_id(&mut self, owner_id: crate::common::ObjectID) {
        if self.owner_id != owner_id {
            self.owner = None;
        }
        self.owner_id = owner_id;
    }

    /// Set goal object
    pub fn set_goal_object(&mut self, obj: Option<Weak<RwLock<Object>>>) {
        if self.locked {
            return;
        }

        self.internal_set_goal_object(obj);
    }

    /// ID-first goal object setter (no Arc/Weak required at call site).
    pub fn set_goal_object_by_id(&mut self, object_id: Option<crate::common::ObjectID>) {
        if self.locked {
            return;
        }
        match object_id {
            Some(id) if id != crate::common::INVALID_ID => {
                self.goal_object_id = id;
                if let Some(arc) = crate::helpers::TheGameLogic::find_object_by_id(id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
                {
                    if let Ok(guard) = arc.read() {
                        self.internal_set_goal_position(guard.get_position().clone());
                    }
                }
            }
            _ => {
                self.goal_object_id = crate::common::INVALID_ID;
            }
        }
    }

    /// Get goal object
    pub fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        if self.goal_object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.goal_object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.goal_object_id))
    }

    pub fn get_goal_object_id(&self) -> crate::common::ObjectID {
        self.goal_object_id
    }

    /// Set goal squad
    pub fn set_goal_squad(&mut self, squad: Option<Weak<Squad>>) {
        self.goal_squad = squad;
    }

    /// Get goal squad
    pub fn get_goal_squad(&self) -> Option<Arc<Squad>> {
        self.goal_squad.as_ref()?.upgrade()
    }

    /// Set goal polygon trigger
    pub fn set_goal_polygon(&mut self, polygon: Option<Weak<PolygonTrigger>>) {
        self.goal_polygon = polygon;
    }

    /// Set guard mode (raw int value).
    pub fn set_guard_mode_raw(&mut self, guard_mode: i32) {
        self.guard_mode_raw = guard_mode;
    }

    /// Get guard mode (raw int value).
    pub fn get_guard_mode_raw(&self) -> i32 {
        self.guard_mode_raw
    }

    /// Get goal polygon trigger
    pub fn get_goal_polygon(&self) -> Option<Arc<PolygonTrigger>> {
        self.goal_polygon.as_ref()?.upgrade()
    }

    pub fn set_goal_waypoint(&mut self, waypoint: Option<WaypointId>) {
        self.goal_waypoint = waypoint;
    }

    pub fn get_goal_waypoint(&self) -> Option<WaypointId> {
        self.goal_waypoint
    }

    /// Set goal position
    pub fn set_goal_position(&mut self, pos: Coord3D) {
        if self.locked {
            return;
        }

        self.internal_set_goal_position(pos);
    }

    /// Get goal position
    pub fn get_goal_position(&self) -> Coord3D {
        self.goal_position
    }

    /// Identity of the active body. Classification stays with that body.
    pub fn get_current_state_id(&self) -> Option<StateId> {
        self.current_state_id
    }
}
