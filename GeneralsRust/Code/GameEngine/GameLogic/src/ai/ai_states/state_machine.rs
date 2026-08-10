/// Main AI State Machine
#[derive(Debug)]
pub struct AIStateMachine {
    owner_id: ObjectID,
    name: String,
    current_state: Option<Box<dyn AIState>>,
    context: AIStateMachineContext,
    temporary_state: Option<Box<dyn AIState>>,
    temporary_state_frame_end: Option<u32>,
}

impl AIStateMachine {
    /// Create new AI state machine
    pub fn new(owner_id: ObjectID, name: String) -> Self {
        let mut machine = Self {
            owner_id,
            name,
            current_state: None,
            context: AIStateMachineContext::default(),
            temporary_state: None,
            temporary_state_frame_end: None,
        };

        machine.context.owner_id = owner_id;

        // Start in idle state
        machine.set_state(AIStateType::Idle);

        machine
    }

    /// Clear state machine
    pub fn clear(&mut self) {
        self.current_state = None;
        self.temporary_state = None;
        self.temporary_state_frame_end = None;
        self.context = AIStateMachineContext::default();
        self.context.owner_id = self.owner_id;
    }

    /// Reset to default state
    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        self.set_state(AIStateType::Idle)
    }

    /// Set new state
    pub fn set_state(&mut self, state_type: AIStateType) -> StateReturnType {
        // Exit current state
        if let Some(mut current) = self.current_state.take() {
            current.on_exit(&mut self.context, StateExitType::Normal);
        }

        // Create new state
        let new_state: Box<dyn AIState> = match state_type {
            AIStateType::Idle => Box::new(AIIdleState::new(true)),
            AIStateType::MoveTo => Box::new(AIMoveToState::new()),
            AIStateType::MoveOutOfTheWay => Box::new(AIMoveOutOfTheWayState::new()),
            AIStateType::AttackObject => Box::new(AIAttackState::new(false, true, false, false)),
            AIStateType::AttackPosition => Box::new(AIAttackState::new(false, false, false, false)),
            AIStateType::ForceAttackObject => {
                Box::new(AIAttackState::new(false, true, true, false))
            }
            AIStateType::AttackAndFollowObject => {
                Box::new(AIAttackState::new(true, true, false, false))
            }
            AIStateType::AttackMoveTo => Box::new(AIAttackState::new(true, false, false, false)),
            AIStateType::AttackArea => Box::new(AIAttackState::new(false, false, false, true)),
            AIStateType::Guard => Box::new(AIGuardState::new()),
            AIStateType::GuardTunnelNetwork => Box::new(AIGuardTunnelNetworkState::new()),
            AIStateType::GuardRetaliate => Box::new(AIGuardRetaliateState::new()),
            AIStateType::Hunt => Box::new(AIHuntState::new()),
            AIStateType::FollowWaypointPathAsTeam => Box::new(AIFollowWaypointPathState::new(true)),
            AIStateType::FollowWaypointPathAsIndividuals => {
                Box::new(AIFollowWaypointPathState::new(false))
            }
            AIStateType::FollowWaypointPathAsTeamExact => {
                Box::new(AIFollowWaypointPathState::new_with_exact(true, true))
            }
            AIStateType::FollowWaypointPathAsIndividualsExact => {
                Box::new(AIFollowWaypointPathState::new_with_exact(false, true))
            }
            AIStateType::AttackFollowWaypointPathAsTeam => Box::new(
                AIFollowWaypointPathState::new_with_exact_and_attack(true, false, true),
            ),
            AIStateType::AttackFollowWaypointPathAsIndividuals => Box::new(
                AIFollowWaypointPathState::new_with_exact_and_attack(false, false, true),
            ),
            AIStateType::MoveAndTighten => Box::new(AIMoveAndTightenState::new()),
            AIStateType::MoveAndEvacuate => Box::new(AIMoveAndEvacuateState::new(false)),
            AIStateType::MoveAndEvacuateAndExit => Box::new(AIMoveAndEvacuateState::new(true)),
            AIStateType::MoveAndDelete => Box::new(AIMoveAndDeleteState::new()),
            AIStateType::ExitInstantly => Box::new(AIExitInstantlyState::new()),
            AIStateType::GetRepaired => Box::new(AIGetRepairedState::new()),
            AIStateType::MoveAwayFromRepulsors => Box::new(AIMoveAwayFromRepulsorsState::new()),
            AIStateType::Wander => Box::new(AIWanderState::new()),
            AIStateType::WanderInPlace => Box::new(AIWanderInPlaceState::new()),
            AIStateType::Panic => Box::new(AIPanicState::new()),
            AIStateType::FollowPath => Box::new(AIFollowPathState::new()),
            AIStateType::FollowExitProductionPath => {
                Box::new(AIFollowExitProductionPathState::new())
            }
            AIStateType::Wait => Box::new(AIWaitState::new()),
            AIStateType::Dead => Box::new(AIDeadState::new()),
            AIStateType::Dock => Box::new(AIDockState::new()),
            AIStateType::Enter => Box::new(AIEnterState::new()),
            AIStateType::Exit => Box::new(AIExitState::new()),
            AIStateType::PickUpCrate => Box::new(AIPickUpCrateState::new()),
            AIStateType::AttackSquad => Box::new(AIAttackSquadState::new()),
            AIStateType::HackInternet => Box::new(AIHackInternetState::new()),
            AIStateType::FaceObject => Box::new(AIFaceObjectState::new()),
            AIStateType::FacePosition => Box::new(AIFacePositionState::new()),
            AIStateType::RappelInto => Box::new(AIRappelIntoState::new()),
            AIStateType::CombatDrop => Box::new(AICombatDropState::new()),
            AIStateType::Busy => Box::new(AIBusyState::new()),
        };

        // Enter new state
        let mut new_state = new_state;
        let result = new_state.on_enter(&mut self.context);
        self.current_state = Some(new_state);

        result
    }

    /// Set temporary state
    pub fn set_temporary_state(
        &mut self,
        state_type: AIStateType,
        frame_limit: u32,
    ) -> StateReturnType {
        // Create temporary state
        let temp_state: Box<dyn AIState> = match state_type {
            AIStateType::MoveOutOfTheWay => Box::new(AIMoveOutOfTheWayState::new()),
            AIStateType::MoveAndTighten => Box::new(AIMoveAndTightenState::new()),
            _ => return StateReturnType::Failed, // Only certain states can be temporary
        };

        let mut temp_state = temp_state;
        let result = temp_state.on_enter(&mut self.context);

        self.temporary_state = Some(temp_state);
        self.temporary_state_frame_end = Some(frame_limit);

        result
    }

    /// Update state machine
    pub fn update_state_machine(&mut self) -> StateReturnType {
        let current_frame = TheGameLogic::get_frame();

        // Handle temporary state first
        if let Some(ref mut temp_state) = self.temporary_state {
            if let Some(end_frame) = self.temporary_state_frame_end {
                if current_frame >= end_frame {
                    // Temporary state expired
                    temp_state.on_exit(&mut self.context, StateExitType::Normal);
                    self.temporary_state = None;
                    self.temporary_state_frame_end = None;
                } else {
                    // Update temporary state
                    return temp_state.update(&mut self.context);
                }
            }
        }

        // Update main state
        if let Some(ref mut current) = self.current_state {
            current.update(&mut self.context)
        } else {
            // No current state, reset to default
            self.reset_to_default_state()
        }
    }

    /// Set goal path
    pub fn set_goal_path(&mut self, path: &[Coord3D]) {
        self.context.goal_path = path.to_vec();
    }

    /// Add to goal path
    pub fn add_to_goal_path(&mut self, path_point: &Coord3D) {
        self.context.goal_path.push(*path_point);
    }

    /// Set goal waypoint
    pub fn set_goal_waypoint(&mut self, waypoint_id: u32) {
        self.context.goal_waypoint = Some(waypoint_id);
    }

    /// Set goal object
    pub fn set_goal_object(&mut self, object_id: ObjectID) {
        self.context.goal_object = Some(object_id);
    }

    /// Set goal position
    pub fn set_goal_position(&mut self, position: Coord3D) {
        self.context.goal_position = Some(position);
    }

    /// Get current state type
    pub fn get_current_state_type(&self) -> Option<AIStateType> {
        self.current_state.as_ref().map(|s| s.get_state_type())
    }

    /// Check if in attack state
    pub fn is_in_attack_state(&self) -> bool {
        self.current_state.as_ref().map_or(false, |s| s.is_attack())
    }

    /// Check if idle
    pub fn is_idle(&self) -> bool {
        self.current_state.as_ref().map_or(false, |s| s.is_idle())
    }

    /// Check if busy
    pub fn is_busy(&self) -> bool {
        self.current_state.as_ref().map_or(false, |s| s.is_busy())
    }

    /// Check if guard idle
    pub fn is_guard_idle(&self) -> bool {
        self.current_state
            .as_ref()
            .map_or(false, |s| s.is_guard_idle())
    }
}

/// AI Command Interface implementation for state machine
impl AiCommandInterface for AIStateMachine {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), AiError> {
        // Update context with command parameters
        self.context.goal_object = params.obj;
        self.context.goal_position = if params.pos != Coord3D::new(0.0, 0.0, 0.0) {
            Some(params.pos)
        } else {
            None
        };
        if self.context.goal_position.is_none() {
            if let Some(trigger_id) = params.polygon {
                if let Ok(terrain_guard) = get_terrain_logic().read() {
                    if let Some(trigger) = terrain_guard.get_trigger_areas().get_by_id(trigger_id) {
                        self.context.goal_position = Some(trigger.get_center_point());
                    }
                }
            }
        }
        self.context.goal_path = params.coords.clone();
        self.context.command_button = params.command_button;
        self.context.int_value = params.int_value;
        // Convert ai::DamageInfo to damage::DamageInfo (ai params currently carry no damage fields)
        self.context.damage_info = crate::damage::DamageInfo::new();

        // Set appropriate state based on command
        let state_type = match params.cmd {
            AiCommandType::Idle => AIStateType::Idle,
            AiCommandType::MoveToPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveTo
            }
            AiCommandType::MoveToObject => {
                self.context.goal_object = params.obj;
                AIStateType::MoveTo
            }
            AiCommandType::TightenToPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveAndTighten
            }
            AiCommandType::MoveToPositionAndEvacuate => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveAndEvacuate
            }
            AiCommandType::MoveToPositionAndEvacuateAndExit => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveAndEvacuateAndExit
            }
            AiCommandType::AttackObject => AIStateType::AttackObject,
            AiCommandType::ForceAttackObject => AIStateType::ForceAttackObject,
            AiCommandType::AttackTeam => AIStateType::AttackSquad,
            AiCommandType::AttackPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::AttackPosition
            }
            AiCommandType::AttackMoveToPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::AttackMoveTo
            }
            AiCommandType::AttackArea => {
                self.context.goal_position = Some(params.pos);
                AIStateType::AttackArea
            }
            AiCommandType::FollowPath => AIStateType::FollowPath,
            AiCommandType::FollowExitProductionPath => AIStateType::FollowExitProductionPath,
            AiCommandType::FollowUserPath => AIStateType::FollowPath,
            AiCommandType::FollowPathAppend => AIStateType::FollowPath,
            AiCommandType::GuardPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::Guard
            }
            AiCommandType::GuardObject => {
                self.context.goal_object = params.obj;
                AIStateType::Guard
            }
            AiCommandType::GuardArea => AIStateType::Guard,
            AiCommandType::Hunt => AIStateType::Hunt,
            AiCommandType::Repair => AIStateType::GetRepaired,
            AiCommandType::GetHealed => AIStateType::GetRepaired,
            AiCommandType::Enter => AIStateType::Enter,
            AiCommandType::Dock => AIStateType::Dock,
            AiCommandType::Exit => AIStateType::Exit,
            AiCommandType::Evacuate => AIStateType::Exit,
            AiCommandType::FollowWaypointPath => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsIndividuals
            }
            AiCommandType::FollowWaypointPathAsTeam => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsTeam
            }
            AiCommandType::FollowWaypointPathExact => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsIndividualsExact
            }
            AiCommandType::FollowWaypointPathAsTeamExact => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsTeamExact
            }
            AiCommandType::AttackFollowWaypointPath => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::AttackFollowWaypointPathAsIndividuals
            }
            AiCommandType::AttackFollowWaypointPathAsTeam => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::AttackFollowWaypointPathAsTeam
            }
            AiCommandType::FaceObject => {
                self.context.goal_object = params.obj;
                AIStateType::FaceObject
            }
            AiCommandType::FacePosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::FacePosition
            }
            AiCommandType::RappelInto => AIStateType::RappelInto,
            AiCommandType::CombatDrop => AIStateType::CombatDrop,
            AiCommandType::Wander => AIStateType::Wander,
            AiCommandType::WanderInPlace => AIStateType::WanderInPlace,
            AiCommandType::Panic => AIStateType::Panic,
            AiCommandType::Busy => AIStateType::Busy,
            AiCommandType::MoveAwayFromUnit => AIStateType::MoveOutOfTheWay,
            AiCommandType::HackInternet => AIStateType::HackInternet,
            AiCommandType::NoCommand => AIStateType::Idle,
            AiCommandType::MoveToPositionEvenIfSleeping => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveTo
            }
            AiCommandType::PickUpPrisoner => AIStateType::PickUpCrate,
            AiCommandType::ReturnPrisoners => AIStateType::Busy,
            AiCommandType::ResumeConstruction => AIStateType::Busy,
            AiCommandType::GetRepaired => AIStateType::GetRepaired,
            AiCommandType::ExecuteRailedTransport => AIStateType::Busy,
            AiCommandType::GoProne => AIStateType::Busy,
            AiCommandType::DeployAssaultReturn => AIStateType::Busy,
            AiCommandType::CommandButton => AIStateType::Busy,
            AiCommandType::CommandButtonObj => AIStateType::Busy,
            AiCommandType::CommandButtonPos => AIStateType::Busy,
            AiCommandType::GuardTunnelNetwork => AIStateType::GuardTunnelNetwork,
            AiCommandType::EvacuateInstantly => AIStateType::ExitInstantly,
            AiCommandType::ExitInstantly => AIStateType::ExitInstantly,
            AiCommandType::GuardRetaliate => AIStateType::GuardRetaliate,
            AiCommandType::DoSpecialPower
            | AiCommandType::DoSpecialPowerAtObject
            | AiCommandType::DoSpecialPowerAtLocation
            | AiCommandType::Sell
            | AiCommandType::ToggleOvercharge
            | AiCommandType::Surrender
            | AiCommandType::Cheer => AIStateType::Busy,
        };

        self.set_state(state_type);
        Ok(())
    }
}

