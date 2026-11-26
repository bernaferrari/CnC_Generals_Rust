//! Chinook flight state and transition rules from ChinookAIUpdate.cpp.

use serde::{Deserialize, Serialize};

/// C++ ChinookTakeoffOrLandingState and ChinookMoveToBldgState arrival threshold.
pub const ARRIVAL_DISTANCE: f32 = 3.0;
pub const ARRIVAL_DISTANCE_SQUARED: f32 = ARRIVAL_DISTANCE * ARRIVAL_DISTANCE;

// Retail fallback values for a transport without admitted INI overrides.
pub const DEFAULT_PREFERRED_HEIGHT: f32 = 100.0;
pub const DEFAULT_MIN_DROP_HEIGHT: f32 = 40.0;
pub const DEFAULT_RAPPEL_SPEED: f32 = 30.0;
pub const DEFAULT_ROPE_RELEASE_DELAY_FRAMES: u32 = 27;

/// C++ `ChinookAIUpdate.cpp:1408` — KindOf CAN_ATTACK, never garrison ObjectStatus.
pub fn attack_allowed_by_kind_of(kind_of_can_attack: bool) -> bool {
    kind_of_can_attack
}

/// C++ `ChinookAIUpdate.cpp:1070-1082` idle + want enter/exit + not landed → LANDING.
pub fn should_auto_land(parent_idle: bool, waiting: bool, landed: bool) -> bool {
    parent_idle && waiting && !landed
}

/// C++ `ChinookAIUpdate.cpp:1084-1086` idle + empty want + landed + no heal pad → TAKING_OFF.
pub fn should_auto_takeoff(
    parent_idle: bool,
    waiting: bool,
    landed: bool,
    healing_airfield: bool,
) -> bool {
    parent_idle && !waiting && landed && !healing_airfield
}

/// C++ `ChinookAIUpdate.cpp:1022-1028` `getAiFreeToExit`.
pub fn free_to_exit(landed: bool, doing_combat_drop: bool, exiter_can_rappel: bool) -> bool {
    landed || (doing_combat_drop && exiter_can_rappel)
}

/// C++ `ChinookMoveToBldgState::onEnter` preferred-height raise.
pub fn move_to_bldg_preferred_height(
    old_preferred: f32,
    live_structure: bool,
    geom_max_height_above: f32,
    min_drop_height: f32,
) -> f32 {
    if !live_structure {
        return old_preferred;
    }
    let raised = geom_max_height_above + min_drop_height;
    if raised < old_preferred {
        old_preferred
    } else {
        raised
    }
}

/// C++ `ChinookMoveToBldgState::update` — 2D arrival **and** `|z-destZ|<=3`.
pub fn move_to_bldg_arrived(arrived_2d: bool, z: f32, dest_z: f32) -> bool {
    arrived_2d && (z - dest_z).abs() <= ARRIVAL_DISTANCE
}

/// C++ `aiDoCommand` evac: take off first when landed and dest farther than 3.
pub fn evac_needs_takeoff_first(landed: bool, dist_sqr: f32) -> bool {
    landed && dist_sqr > ARRIVAL_DISTANCE_SQUARED
}

/// C++ `ChinookFlightStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChinookFlightStatus {
    TakingOff = 0,
    #[default]
    Flying = 1,
    DoingCombatDrop = 2,
    Landing = 3,
    Landed = 4,
}

/// C++ `ChinookAIStateType` with the live idle state at index zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChinookAiState {
    #[default]
    Idle,
    TakingOff,
    Landing,
    MoveToAndLand,
    MoveToAndEvac,
    LandAndEvac,
    EvacAndTakeoff,
    MoveToAndEvacAndExitInit,
    MoveToAndEvacAndExit,
    LandAndEvacAndExit,
    EvacAndExit,
    TakeoffAndExit,
    HeadOffMap,
    MoveToCombatDrop,
    DoCombatDrop,
}

/// The receiving Object consumes this result at the state entry boundary.
/// Landing can complete in the same call, so flight status cannot report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChinookFlightEntry {
    #[default]
    None,
    Landing,
    CombatDrop,
}

/// Immediate C++ ChinookEvacuateState entry action, performed by the owner.
/// This value is not persisted; an unfinished action is represented by the
/// existing EvacAndTakeoff/EvacAndExit state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChinookEvacuation {
    Takeoff,
    TakeoffAndExit,
}

/// Result of the Chinook load/unload eligibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChinookFreeToExit {
    FreeToExit,
    WaitToExit,
}

/// Flight state for one Chinook. The world supplies poses and performs the returned entry actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChinookAi {
    pub flight_status: ChinookFlightStatus,
    pub state: ChinookAiState,
    pub pos: [f32; 3],
    pub dest: [f32; 3],
    pub original_pos: [f32; 3],
    pub preferred_height: f32,
    pub min_drop_height: f32,
    pub rappel_speed: f32,
    pub layer_is_ground: bool,
    pub pad_search_applied: bool,
    pub rider8: bool,
    pub destroyed: bool,
    pub healee: bool,
    pub airfield_id: Option<u32>,
    pub parent_idle: bool,
    pub wanting_enter_or_exit: bool,
    /// KindOf CAN_ATTACK (Combat Chinook). Not OBJECT_STATUS_CAN_ATTACK.
    pub kind_of_can_attack: bool,
    pub passengers_allowed_to_fire: bool,
    pub combat_drop_dest_z: f32,
    pub move_to_bldg_old_preferred: f32,
    pub contained_count: u32,
    pub map_lo: [f32; 2],
    pub map_hi: [f32; 2],
    /// Next permitted rappel release (logic frames).
    #[serde(default)]
    pub combat_drop_next_release_frame: u32,
    /// Rappellers released this drop (rope stagger).
    #[serde(default)]
    pub combat_drop_releases: u32,
    /// C++ `privateCombatDrop` goal object (garrison / structure).
    #[serde(default)]
    pub combat_drop_target: Option<u32>,
    /// C++ `m_pendingCommand` evac dest (landed takeoff reconstitution).
    #[serde(default)]
    pub pending_evac_dest: Option<[f32; 3]>,
    #[serde(default)]
    pub pending_evac_and_exit: bool,
    /// C++ stored AICMD_IDLE / CMD_FROM_AI; mutually exclusive with pending evac.
    #[serde(default)]
    pub pending_ai_idle: bool,
    /// In-flight `aiRappelInto` jobs (rappeller, building, dest Y).
    #[serde(default)]
    pub rappel_into_jobs: Vec<RappelJob>,
}

/// Pending roof approach and garrison target for one rappeller.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RappelJob {
    pub rappeller: u32,
    pub building: Option<u32>,
    pub dest_y: f32,
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

impl ChinookAi {
    /// Create a Combat Chinook with KindOf CAN_ATTACK.
    pub fn new_combat(pos: [f32; 3]) -> Self {
        Self::new(pos, true)
    }

    /// Create a transport Chinook.
    pub fn new_vanilla(pos: [f32; 3]) -> Self {
        Self::new(pos, false)
    }

    fn new(pos: [f32; 3], kind_of_can_attack: bool) -> Self {
        Self {
            flight_status: ChinookFlightStatus::Flying,
            state: ChinookAiState::Idle,
            pos,
            dest: pos,
            original_pos: pos,
            preferred_height: DEFAULT_PREFERRED_HEIGHT,
            min_drop_height: DEFAULT_MIN_DROP_HEIGHT,
            rappel_speed: DEFAULT_RAPPEL_SPEED,
            layer_is_ground: true,
            pad_search_applied: false,
            rider8: false,
            destroyed: false,
            healee: false,
            airfield_id: None,
            parent_idle: true,
            wanting_enter_or_exit: false,
            kind_of_can_attack,
            combat_drop_next_release_frame: 0,
            combat_drop_releases: 0,
            passengers_allowed_to_fire: kind_of_can_attack,
            combat_drop_dest_z: pos[2],
            move_to_bldg_old_preferred: DEFAULT_PREFERRED_HEIGHT,
            contained_count: 0,
            map_lo: [0.0, 0.0],
            map_hi: [500.0, 500.0],
            combat_drop_target: None,
            pending_evac_dest: None,
            pending_evac_and_exit: false,
            pending_ai_idle: false,
            rappel_into_jobs: Vec::new(),
        }
    }

    /// C++ `getAiFreeToExit`: landed, or combat-drop + `KINDOF_CAN_RAPPEL`.
    pub fn ai_free_to_exit(&self, exiter_can_rappel: bool) -> ChinookFreeToExit {
        if free_to_exit(
            self.flight_status == ChinookFlightStatus::Landed,
            self.flight_status == ChinookFlightStatus::DoingCombatDrop,
            exiter_can_rappel,
        ) {
            ChinookFreeToExit::FreeToExit
        } else {
            ChinookFreeToExit::WaitToExit
        }
    }

    /// C++ `isKindOf(KINDOF_CAN_ATTACK)` gate for privateAttack*.
    pub fn can_issue_attack(&self) -> bool {
        attack_allowed_by_kind_of(self.kind_of_can_attack)
    }

    /// C++ passenger follow: only riders with `getCurrentVictim()==NULL`.
    pub fn passenger_should_follow_attack(&self, passenger_has_victim: bool) -> bool {
        self.passengers_allowed_to_fire && !passenger_has_victim
    }

    /// C++ `ChinookCombatDropState` `setDesiredSpeed(m_rappelSpeed)`.
    pub fn apply_rappel_speed(&self) -> f32 {
        self.rappel_speed
    }
    /// Current runtime eligibility for a rappelling passenger.
    pub fn passenger_kind_can_rappel(is_infantry: bool) -> bool {
        is_infantry
    }

    /// Hover arrived: enter `DO_COMBAT_DROP` (C++ ChinookMoveToBldg → CombatDrop).
    pub fn arrive_for_combat_drop(&mut self) -> ChinookFlightEntry {
        if self.state == ChinookAiState::MoveToCombatDrop {
            return self.enter_state(ChinookAiState::DoCombatDrop);
        }
        ChinookFlightEntry::None
    }

    /// C++ `now >= nextDropTime` on a rope.
    pub fn can_release_rappeller(&self, now: u32) -> bool {
        self.flight_status == ChinookFlightStatus::DoingCombatDrop
            && now >= self.combat_drop_next_release_frame
    }

    /// Schedule the next release using the existing fixed minimum delay.
    pub fn on_rappeller_released(&mut self, now: u32) {
        self.combat_drop_releases = self.combat_drop_releases.saturating_add(1);
        self.combat_drop_next_release_frame = now.saturating_add(DEFAULT_ROPE_RELEASE_DELAY_FRAMES);
    }

    /// C++ idle + `hasObjectsWantingToEnterOrExit` + not landed → `LANDING`.
    /// C++ uses the base AI state's idleness, so active flight states cannot
    /// be replaced by this idle contain policy merely because their path is empty.
    pub fn tick_idle_auto_land(&mut self) -> ChinookFlightEntry {
        if self.state != ChinookAiState::Idle {
            return ChinookFlightEntry::None;
        }
        let landed = self.flight_status == ChinookFlightStatus::Landed;
        if should_auto_land(self.parent_idle, self.wanting_enter_or_exit, landed) {
            return self.enter_state(ChinookAiState::Landing);
        }
        if should_auto_takeoff(
            self.parent_idle,
            self.wanting_enter_or_exit,
            landed,
            self.airfield_id.is_some(),
        ) {
            self.enter_state(ChinookAiState::TakingOff);
        }
        ChinookFlightEntry::None
    }

    /// C++ `AICMD_MOVE_TO_POSITION_AND_EVACUATE[_AND_EXIT]`.
    pub fn command_evac(&mut self, dest: [f32; 3], and_exit: bool) {
        self.pending_ai_idle = false;
        if evac_needs_takeoff_first(
            self.flight_status == ChinookFlightStatus::Landed,
            distance_squared(self.pos, dest),
        ) {
            // C++ stores `m_pendingCommand` then TAKING_OFF; dest is not dropped.
            self.pending_evac_dest = Some(dest);
            self.pending_evac_and_exit = and_exit;
            self.enter_state(ChinookAiState::TakingOff);
            return;
        }
        self.dest = dest;
        self.enter_state(if and_exit {
            ChinookAiState::MoveToAndEvacAndExitInit
        } else {
            ChinookAiState::MoveToAndEvac
        });
    }

    /// C++ `ChinookCombatDropState` success: clear Held, `CHINOOK_FLYING`, idle.
    pub fn finish_combat_drop(&mut self) {
        if self.state == ChinookAiState::DoCombatDrop
            || self.flight_status == ChinookFlightStatus::DoingCombatDrop
        {
            self.succeed();
        }
    }

    /// After a real dump, continue evac-and-exit without re-recording spawn at LZ.
    pub fn begin_takeoff_and_exit(&mut self) {
        self.contained_count = 0;
        self.wanting_enter_or_exit = false;
        self.enter_state(ChinookAiState::TakeoffAndExit);
    }

    /// C++ `privateGetRepaired` → `MOVE_TO_AND_LAND` (not immediate LANDING).
    pub fn command_repair(&mut self, depot: [f32; 3], depot_id: u32) {
        self.pending_ai_idle = false;
        if matches!(
            self.flight_status,
            ChinookFlightStatus::Landing | ChinookFlightStatus::Landed
        ) {
            return;
        }
        self.airfield_id = Some(depot_id);
        self.dest = depot;
        self.enter_state(ChinookAiState::MoveToAndLand);
    }

    /// C++ `privateCombatDrop` → `MOVE_TO_COMBAT_DROP` (MoveToBldg height).
    pub fn command_combat_drop(&mut self, dest: [f32; 3], building_height: Option<f32>) {
        self.pending_ai_idle = false;
        self.dest = dest;
        self.move_to_bldg_old_preferred = self.preferred_height;
        let new_pref = move_to_bldg_preferred_height(
            self.preferred_height,
            building_height.is_some(),
            building_height.unwrap_or(0.0),
            self.min_drop_height,
        );
        self.combat_drop_dest_z = dest[2] + new_pref;
        self.preferred_height = new_pref;
        self.enter_state(ChinookAiState::MoveToCombatDrop);
    }

    fn enter_state(&mut self, state: ChinookAiState) -> ChinookFlightEntry {
        self.state = state;
        match state {
            ChinookAiState::Idle => {}
            ChinookAiState::TakingOff | ChinookAiState::TakeoffAndExit => {
                self.enter_takeoff_or_landing(false);
            }
            ChinookAiState::Landing
            | ChinookAiState::LandAndEvac
            | ChinookAiState::LandAndEvacAndExit => {
                self.enter_takeoff_or_landing(true);
                return ChinookFlightEntry::Landing;
            }
            ChinookAiState::MoveToAndLand
            | ChinookAiState::MoveToAndEvac
            | ChinookAiState::MoveToAndEvacAndExit => {}
            ChinookAiState::MoveToAndEvacAndExitInit => {
                self.original_pos = self.pos;
                return self.enter_state(ChinookAiState::MoveToAndEvacAndExit);
            }
            ChinookAiState::EvacAndTakeoff | ChinookAiState::EvacAndExit => {
                // C++ removes the actual contain list while still landed,
                // before entering the succeeding takeoff state. The callback
                // performs that domain operation after releasing its owner borrow.
            }
            ChinookAiState::HeadOffMap => {
                self.rider8 = true;
                self.dest = self.original_pos;
            }
            ChinookAiState::MoveToCombatDrop => {}
            ChinookAiState::DoCombatDrop => {
                self.flight_status = ChinookFlightStatus::DoingCombatDrop;
                self.preferred_height = self.move_to_bldg_old_preferred;
                // C++ ChinookCombatDropState::onEnter while(loseOneBox()).
                // The receiving Object owns and clears the carrier's cargo.
                self.combat_drop_next_release_frame = 0;
                self.combat_drop_releases = 0;
                return ChinookFlightEntry::CombatDrop;
            }
        }
        ChinookFlightEntry::None
    }

    /// C++ `ChinookTakeoffOrLandingState::onEnter`: dump boxes, pad search, layer.
    fn enter_takeoff_or_landing(&mut self, landing: bool) {
        self.flight_status = if landing {
            ChinookFlightStatus::Landing
        } else {
            ChinookFlightStatus::TakingOff
        };
        if landing {
            // C++ ChinookTakeoffOrLandingState::onEnter while(loseOneBox()).
            // The receiving Object owns and clears the carrier's cargo.
            self.pad_search_applied = true;
            self.dest = [self.pos[0], self.pos[1], 0.0];
        } else {
            self.dest = [
                self.pos[0],
                self.pos[1],
                self.pos[2].max(0.0) + self.preferred_height,
            ];
            self.layer_is_ground = true;
        }
    }

    fn arrived_3d(&self, dest: [f32; 3]) -> bool {
        distance_squared(self.pos, dest) <= ARRIVAL_DISTANCE_SQUARED
    }

    fn arrived_2d(&self, dest: [f32; 3]) -> bool {
        let dx = self.pos[0] - dest[0];
        let dy = self.pos[1] - dest[1];
        dx * dx + dy * dy <= ARRIVAL_DISTANCE_SQUARED
    }

    fn succeed(&mut self) -> ChinookFlightEntry {
        let next = match self.state {
            ChinookAiState::TakingOff => {
                self.flight_status = ChinookFlightStatus::Flying;
                ChinookAiState::Idle
            }
            ChinookAiState::Landing => {
                self.flight_status = ChinookFlightStatus::Landed;
                ChinookAiState::Idle
            }
            ChinookAiState::MoveToAndLand => ChinookAiState::Landing,
            ChinookAiState::MoveToAndEvac => ChinookAiState::LandAndEvac,
            ChinookAiState::LandAndEvac => {
                self.flight_status = ChinookFlightStatus::Landed;
                ChinookAiState::EvacAndTakeoff
            }
            ChinookAiState::EvacAndTakeoff => ChinookAiState::TakingOff,
            ChinookAiState::MoveToAndEvacAndExit => ChinookAiState::LandAndEvacAndExit,
            ChinookAiState::LandAndEvacAndExit => {
                self.flight_status = ChinookFlightStatus::Landed;
                ChinookAiState::EvacAndExit
            }
            ChinookAiState::EvacAndExit => ChinookAiState::TakeoffAndExit,
            ChinookAiState::TakeoffAndExit => {
                self.flight_status = ChinookFlightStatus::Flying;
                ChinookAiState::HeadOffMap
            }
            ChinookAiState::HeadOffMap => {
                self.rider8 = false;
                ChinookAiState::Idle
            }
            ChinookAiState::MoveToCombatDrop => ChinookAiState::DoCombatDrop,
            ChinookAiState::DoCombatDrop => {
                self.flight_status = ChinookFlightStatus::Flying;
                ChinookAiState::Idle
            }
            ChinookAiState::MoveToAndEvacAndExitInit | ChinookAiState::Idle => ChinookAiState::Idle,
        };
        if next != self.state {
            return self.enter_state(next);
        }
        self.state = ChinookAiState::Idle;
        ChinookFlightEntry::None
    }

    /// Evaluate flight transitions at the pose sampled from the owning Object.
    /// C++ state updates issue locomotor goals; the locomotor alone integrates motion.
    pub fn update_from_observed_pose(&mut self) -> Option<ChinookEvacuation> {
        self.prepare_observed_update(false);
        self.advance_observed_state().1
    }

    /// ChinookAIUpdate.cpp:1067–1088 runs before the parent state update.
    pub fn prepare_observed_update(&mut self, replayed_idle: bool) -> ChinookFlightEntry {
        if self.destroyed {
            return ChinookFlightEntry::None;
        }
        // C++ checks base idleness and reconstitutes pending commands before
        // auto-land, then advances the state machine in the parent update.
        // A takeoff completed below is therefore replayed on the next tick.
        if !replayed_idle && self.state == ChinookAiState::Idle && self.parent_idle {
            if let Some(dest) = self.pending_evac_dest.take() {
                let and_exit = self.pending_evac_and_exit;
                self.pending_evac_and_exit = false;
                self.command_evac(dest, and_exit);
            } else {
                return self.tick_idle_auto_land();
            }
        }
        ChinookFlightEntry::None
    }

    pub fn advance_observed_state(&mut self) -> (ChinookFlightEntry, Option<ChinookEvacuation>) {
        let mut entry = ChinookFlightEntry::None;
        if self.destroyed {
            return (entry, None);
        }
        if self.flight_status == ChinookFlightStatus::Landed && self.airfield_id.is_some() {
            self.healee = true;
        }
        match self.state {
            ChinookAiState::Idle | ChinookAiState::DoCombatDrop => {}
            ChinookAiState::TakingOff
            | ChinookAiState::Landing
            | ChinookAiState::LandAndEvac
            | ChinookAiState::LandAndEvacAndExit
            | ChinookAiState::TakeoffAndExit => {
                if self.arrived_3d(self.dest) {
                    entry = self.succeed();
                }
            }
            ChinookAiState::MoveToAndLand
            | ChinookAiState::MoveToAndEvac
            | ChinookAiState::MoveToAndEvacAndExit => {
                if self.arrived_2d(self.dest) {
                    entry = self.succeed();
                }
            }
            ChinookAiState::MoveToCombatDrop => {
                let hover = [self.dest[0], self.dest[1], self.combat_drop_dest_z];
                if move_to_bldg_arrived(
                    self.arrived_2d(hover),
                    self.pos[2],
                    self.combat_drop_dest_z,
                ) {
                    entry = self.succeed();
                }
            }
            ChinookAiState::HeadOffMap => {
                // C++ ChinookHeadOffMapState::update (ChinookAIUpdate.cpp:152-162)
                // destroys the owner the moment its CURRENT position leaves
                // getExtentIncludingBorder — the bounds check observes the
                // pre-move position each frame; an already-outside owner never
                // gets another locomotor step back toward the map.
                if self.pos[0] < self.map_lo[0]
                    || self.pos[0] > self.map_hi[0]
                    || self.pos[1] < self.map_lo[1]
                    || self.pos[1] > self.map_hi[1]
                {
                    self.destroyed = true;
                    entry = self.succeed();
                }
            }
            ChinookAiState::EvacAndTakeoff
            | ChinookAiState::EvacAndExit
            | ChinookAiState::MoveToAndEvacAndExitInit => {}
        }
        let evacuation = match self.state {
            ChinookAiState::EvacAndTakeoff => Some(ChinookEvacuation::Takeoff),
            ChinookAiState::EvacAndExit => Some(ChinookEvacuation::TakeoffAndExit),
            _ => None,
        };
        (entry, evacuation)
    }

    /// Commit the entry action only after the owner reports its real contain
    /// list. A failed or mismatched operation leaves the transport landed.
    pub fn complete_evacuation(
        &mut self,
        effect: ChinookEvacuation,
        observed_contained_count: u32,
    ) -> bool {
        let expected = match effect {
            ChinookEvacuation::Takeoff => ChinookAiState::EvacAndTakeoff,
            ChinookEvacuation::TakeoffAndExit => ChinookAiState::EvacAndExit,
        };
        if self.state != expected {
            return false;
        }
        self.contained_count = observed_contained_count;
        if observed_contained_count != 0 {
            return false;
        }
        self.wanting_enter_or_exit = false;
        self.succeed();
        true
    }
}

#[cfg(test)]
mod state_transition_tests;
