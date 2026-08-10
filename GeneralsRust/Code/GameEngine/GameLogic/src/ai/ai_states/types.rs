const CRATE_PICKUP_RANGE_SQR: f32 = 100.0;

/// AI state context for carrying parameters between states
#[derive(Debug, Clone)]
pub struct AIStateContext {
    pub goal_object: Option<ObjectID>,
    pub goal_position: Option<Coord3D>,
    pub damage_info: DamageInfo,
    pub int_value: i32,
    pub real_value: Real,
    pub bool_value: bool,
}

impl Default for AIStateContext {
    fn default() -> Self {
        Self {
            goal_object: None,
            goal_position: None,
            damage_info: DamageInfo::new(),
            int_value: 0,
            real_value: 0.0,
            bool_value: false,
        }
    }
}

/// AI State IDs matching the C++ original exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AIStateType {
    Idle,
    MoveTo,                               // Move to GoalObject or GoalPosition
    FollowWaypointPathAsTeam,             // Follow waypoint path as team
    FollowWaypointPathAsIndividuals,      // Follow waypoint path individually
    FollowWaypointPathAsTeamExact,        // Follow waypoint path as team (exact)
    FollowWaypointPathAsIndividualsExact, // Follow waypoint path individually (exact)
    FollowPath,                           // Follow simple list of points
    FollowExitProductionPath,             // Same as FollowPath but only when exiting production
    Wait,
    AttackPosition,        // Attack GoalPosition
    AttackObject,          // Attack GoalObject
    ForceAttackObject,     // Force attack GoalObject
    AttackAndFollowObject, // Attack GoalObject, follow if necessary
    Dead,
    Dock,                                  // Dock with GoalObject with DockUpdate
    Enter,                                 // Move to GoalObject and enter when close
    Guard,                                 // Guard current location
    Hunt,                                  // Seek and destroy behavior
    Wander,                                // Wander following waypoint path
    Panic,                                 // Run around screaming following waypoint
    AttackSquad,                           // Attack all objects in goalSquad
    GuardTunnelNetwork,                    // Guard from inside tunnel network
    GetRepaired,                           // Get repaired at repair depot
    MoveOutOfTheWay,                       // Move out of way of another unit
    MoveAndTighten,                        // Move to tighten up formation
    MoveAndEvacuate,                       // Move to then empty transport
    MoveAndEvacuateAndExit,                // Move to then empty transport and exit
    MoveAndDelete,                         // Move to then delete self
    AttackArea,                            // Attack units in an area
    HackInternet,                          // Hack internet for money
    AttackMoveTo,                          // Attack-move to location
    AttackFollowWaypointPathAsIndividuals, // Attack-follow waypoint path individually
    AttackFollowWaypointPathAsTeam,        // Attack-follow waypoint path as team
    FaceObject,                            // Face towards object
    FacePosition,                          // Face towards position
    RappelInto,                            // Rappel from current pos to target
    CombatDrop,                            // Send AI_RAPPEL_INTO to contents
    Exit,                                  // Exit the object, wait if necessary
    PickUpCrate,                           // Pick up crate created by kill
    MoveAwayFromRepulsors,                 // Civilians running from repulsors
    WanderInPlace,                         // Wander around a spot
    Busy,                                  // Busy doing stuff that doesn't require AI
    ExitInstantly,                         // Exit object without waiting
    GuardRetaliate,                        // Attack attacker with restrictions
}

// StateExitType is now imported from crate::state_machine

/// AI State machine context
#[derive(Debug, Clone)]
pub struct AIStateMachineContext {
    pub owner_id: ObjectID,
    pub goal_object: Option<ObjectID>,
    pub goal_position: Option<Coord3D>,
    pub goal_waypoint: Option<u32>, // Waypoint ID
    pub goal_squad: Option<u32>,    // Squad ID
    pub goal_path: Vec<Coord3D>,
    pub damage_info: DamageInfo,
    pub int_value: i32,
    pub command_button: Option<u32>, // Command button ID
    pub current_path: Option<u32>,   // Path ID
}

impl Default for AIStateMachineContext {
    fn default() -> Self {
        Self {
            owner_id: 0,
            goal_object: None,
            goal_position: None,
            goal_waypoint: None,
            goal_squad: None,
            goal_path: Vec::new(),
            damage_info: DamageInfo::default(),
            int_value: 0,
            command_button: None,
            current_path: None,
        }
    }
}


/// Wave 254: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

fn out_of_weapon_range_object(context: &AIStateMachineContext) -> bool {
    // Wave 254: empty dual-world → no owner/target resolve.
    if dual_world_registry_unavailable() {
        return false;
    }
    let Some(target_id) = context.goal_object else {
        return false;
    };
    OBJECT_REGISTRY
        .with_object(context.owner_id, |owner| {
            let Some((weapon, _slot)) = owner.get_current_weapon() else {
                return false;
            };
            if weapon.has_leech_range() {
                return false;
            }
            if OBJECT_REGISTRY.with_object(target_id, |_| ()).is_none() {
                return false;
            }
            !weapon.is_within_attack_range(owner.get_id(), Some(target_id), None)
        })
        .unwrap_or(false)
}

fn out_of_weapon_range_position(context: &AIStateMachineContext) -> bool {
    // Wave 254: empty dual-world → no owner resolve.
    if dual_world_registry_unavailable() {
        return false;
    }
    let Some(goal_position) = context.goal_position else {
        return false;
    };
    OBJECT_REGISTRY
        .with_object(context.owner_id, |owner| {
            let Some((weapon, _slot)) = owner.get_current_weapon() else {
                return false;
            };
            !weapon.is_within_attack_range(owner.get_id(), None, Some(&goal_position))
        })
        .unwrap_or(false)
}

fn want_to_squish_target(context: &AIStateMachineContext) -> bool {
    // Wave 254: empty dual-world → no target resolve.
    if dual_world_registry_unavailable() {
        return false;
    }
    let Some(target_id) = context.goal_object else {
        return false;
    };
    let target_ok = OBJECT_REGISTRY
        .with_object(target_id, |target| {
            target.get_contained_by().is_none() && target.is_kind_of(KindOf::Infantry)
        })
        .unwrap_or(false);
    if !target_ok {
        return false;
    }

    OBJECT_REGISTRY
        .with_object(context.owner_id, |owner| {
            let turret = owner
                .get_ai_update_interface()
                .map(|ai| ai.get_which_turret_for_cur_weapon())
                .unwrap_or(TurretType::Invalid);
            if turret == TurretType::Invalid {
                return false;
            }

            let is_computer = owner
                .get_controlling_player()
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|guard| guard.get_player_type() == PlayerType::Computer)
                })
                .unwrap_or(false);
            if !is_computer {
                return false;
            }

            owner.get_crusher_level() != 0
        })
        .unwrap_or(false)
}

fn goal_reached(context: &AIStateMachineContext) -> bool {
    // Wave 254: empty dual-world → no owner position.
    if dual_world_registry_unavailable() {
        return false;
    }
    let Some(goal_pos) = context.goal_position else {
        return false;
    };
    let Some(current_pos) =
        OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
    else {
        return false;
    };
    let delta = goal_pos - current_pos;
    let dist_sqr = delta.x * delta.x + delta.y * delta.y;
    let close_enough = PATHFIND_CLOSE_ENOUGH * PATHFIND_CLOSE_ENOUGH;
    dist_sqr <= close_enough
}

fn resolve_goal_position(context: &mut AIStateMachineContext) -> Option<Coord3D> {
    // Wave 254: empty dual-world → keep existing goal_position only.
    if dual_world_registry_unavailable() {
        return context.goal_position;
    }
    if let Some(target_id) = context.goal_object {
        if let Some(pos) = OBJECT_REGISTRY.with_object(target_id, |target| *target.get_position()) {
            context.goal_position = Some(pos);
            return Some(pos);
        }
    }

    context.goal_position
}

/// Base AI State trait
pub trait AIState: std::fmt::Debug + Send + Sync {
    /// Called when entering the state
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType;

    /// Called every frame while in the state
    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType;

    /// Called when exiting the state
    fn on_exit(&mut self, context: &mut AIStateMachineContext, exit_type: StateExitType);

    /// Get state type ID
    fn get_state_type(&self) -> AIStateType;

    /// Check if state is idle
    fn is_idle(&self) -> bool {
        false
    }

    /// Check if state is busy
    fn is_busy(&self) -> bool {
        false
    }

    /// Check if state is attack state
    fn is_attack(&self) -> bool {
        false
    }

    /// Check if state is guard idle
    fn is_guard_idle(&self) -> bool {
        false
    }
}

