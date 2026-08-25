use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandType {
    // Selection commands
    CreateSelectedGroup {
        create_new: bool,
        units: Vec<ObjectId>,
    },
    DestroySelectedGroup {
        team_id: u32,
    },
    RemoveFromSelectedGroup {
        units: Vec<ObjectId>,
    },

    // Movement commands
    Move {
        destination: Vec3,
    }, // Basic move command
    MoveTo {
        destination: Vec3,
        waypoints: Vec<Vec3>,
    },
    AttackMoveTo {
        destination: Vec3,
        /// C++ maxShotsToFire residual (-1 / NO_MAX = unlimited).
        #[serde(default = "default_max_shots_cmd")]
        max_shots: i32,
    },
    ForceMoveTo {
        destination: Vec3,
    },
    AddWaypoint {
        destination: Vec3,
    },

    // Combat commands
    Attack {
        target_id: ObjectId,
    }, // Basic attack command
    AttackObject {
        target_id: ObjectId,
    },
    ForceAttackObject {
        target_id: ObjectId,
    },
    ForceAttackGround {
        location: Vec3,
    },
    /// C++ AIGroup::groupAttackPosition residual (None loc = attack own position).
    AttackPosition {
        location: Option<Vec3>,
        max_shots: i32,
    },
    Stop,
    Guard {
        target: GuardTarget,
        /// C++ GuardMode residual (default Normal).
        #[serde(default)]
        mode: crate::game_logic::GuardMode,
    },
    /// Patrol residual: units wander and auto-engage nearby enemies.
    Patrol,
    /// C++ AttitudeType residual hotkeys / strip.
    AttitudeSleep,
    AttitudePassive,
    AttitudeNormal,
    AttitudeAggressive,
    Scatter,
    /// C++ AIGroup::groupTightenToPosition residual — all units path to same point.
    TightenToPosition {
        destination: glam::Vec3,
    },
    /// C++ AIGroup::groupAttackTeam residual.
    AttackTeam {
        /// Team discriminant: 0=GLA, 1=USA, 2=China (Neutral ignored).
        team: u8,
        max_shots: i32,
    },
    /// C++ AIGroup::groupOverrideSpecialPowerDestination residual.
    OverrideSpecialPowerDestination {
        location: glam::Vec3,
    },
    /// C++ AIGroup::setWeaponSetFlag residual.
    /// flag: 0=PLAYER_UPGRADE, 1=MINE_CLEARING, 2=CARBOMB, 3=VEHICLE_HIJACK.
    SetWeaponSetFlag {
        flag: u8,
        enabled: bool,
    },
    /// C++ AIGroup::groupFollowWaypointPath residual (explicit path points).
    FollowWaypointPath {
        waypoints: Vec<glam::Vec3>,
        exact: bool,
        /// C++ groupFollowWaypointPathAsTeam residual.
        #[serde(default)]
        as_team: bool,
    },
    /// C++ AIAttackFollowWaypointPathState residual.
    AttackFollowWaypointPath {
        waypoints: Vec<glam::Vec3>,
        exact: bool,
        #[serde(default)]
        as_team: bool,
    },
    /// C++ AIGroup::groupDoCommandButtonUsingWaypoints residual.
    DoCommandButtonUsingWaypoints {
        button: String,
        waypoints: Vec<glam::Vec3>,
    },
    /// C++ AIGroup::groupSurrender residual (test-key path).
    Surrender {
        surrendered: bool,
    },
    /// C++ AIGroup::groupDoCommandButton residual — dispatch by button name.
    DoCommandButton {
        button: String,
    },
    /// C++ AIGroup::groupDoCommandButtonAtPosition residual.
    DoCommandButtonAtPosition {
        button: String,
        location: glam::Vec3,
    },
    /// C++ AIGroup::groupDoCommandButtonAtObject residual.
    DoCommandButtonAtObject {
        button: String,
        target: crate::game_logic::ObjectId,
    },
    /// C++ AIGroup::groupExecuteRailedTransport residual.
    ExecuteRailedTransport,

    Deploy,
    Gather {
        target_id: ObjectId,
    },

    // Building and construction
    Build {
        template_name: String,
        location: Vec3,
    }, // Basic build command
    DozerConstruct {
        template_name: String,
        location: Vec3,
        /// Build facing residual (radians about Y).
        orientation: f32,
    },
    DozerConstructLine {
        template_name: String,
        start: Vec3,
        end: Vec3,
    },
    DozerCancelConstruct {
        object_id: ObjectId,
    },
    ResumeConstruction {
        target_id: ObjectId,
    },
    Sell {
        object_id: ObjectId,
    },

    // Unit production
    QueueUnitCreate {
        template_name: String,
        quantity: u32,
    },
    CancelUnitCreate {
        template_name: String,
    },

    // Special abilities
    DoSpecialPower {
        power_type: SpecialPowerType,
        target: PowerTarget,
    },
    DoWeapon {
        weapon_slot: WeaponSlot,
        /// Exact C++ `CommandButton::getMaxShotsToFire()` payload.
        ///
        /// Older saved host commands lacked this field, where the safe legacy
        /// meaning is an unlimited order.
        #[serde(default = "default_weapon_max_shots_cmd")]
        max_shots_to_fire: i32,
        target: WeaponTarget,
    },

    // Transport and container
    Enter {
        target_id: ObjectId,
    },
    Exit,
    Evacuate,
    /// C++ AIGroup::groupMoveToAndEvacuate — path container then unload.
    MoveToAndEvacuate {
        destination: glam::Vec3,
        /// When true: AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT (transport self-destruct residual).
        and_exit: bool,
    },
    /// China Hacker field HackInternet residual (start cash interval).
    HackInternet,
    /// Aircraft return-to-base / dock nearest airfield residual.
    ReturnToBase,
    /// Harvester return cargo to nearest SupplyCenter residual.
    ReturnSupplies,
    /// Dozer/Worker path to nearest enemy mine and clear residual.
    ClearMines,
    /// C++ AIGroup::setMineClearingDetail residual.
    SetMineClearingDetail {
        enabled: bool,
    },
    /// C++ AIGroup::groupGoProne residual.
    GoProne,
    /// C++ AIGroup::setWeaponLockForGroup residual.
    SetWeaponLock {
        slot: u8,
        /// 0=NotLocked, 1=Temporary, 2=Permanent (WeaponLockType).
        lock_type: u8,
    },
    /// C++ AIGroup::releaseWeaponLockForGroup residual.
    ReleaseWeaponLock {
        /// 1=Temporary, 2=Permanent.
        lock_type: u8,
    },
    /// C++ AIGroup::groupSetEmoticon residual.
    SetEmoticon {
        name: String,
        /// Duration in logic frames.
        duration_frames: i32,
    },
    /// C++ AIGroup::groupAttackArea — polygon AIAttackAreaState or circle residual.
    AttackArea {
        center: glam::Vec3,
        radius: f32,
        polygon_name: Option<String>,
    },
    Dock {
        target_id: ObjectId,
    },
    CombatDrop {
        target: DropTarget,
    },

    // Utility commands
    Repair {
        target_id: ObjectId,
    },
    GetRepaired {
        target_id: ObjectId,
    },
    GetHealed {
        target_id: ObjectId,
    },
    SetRallyPoint {
        location: Vec3,
    },

    // Economy and resources
    PurchaseScience {
        science_name: String,
    },
    QueueUpgrade {
        upgrade_name: String,
    },
    CancelUpgrade {
        upgrade_name: String,
    },

    // Special unit abilities
    Hijack {
        target_id: ObjectId,
    },
    Sabotage {
        target_id: ObjectId,
    },
    ConvertToCarbomb {
        target_id: ObjectId,
    },
    CaptureBuilding {
        target_id: ObjectId,
    },
    SnipeVehicle {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: plant timed demo charge on structure/vehicle.
    PlantTimedDemoCharge {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: plant remote demo charge on structure/vehicle
    /// (SPECIAL_REMOTE_CHARGES — no auto-timer until DetonateRemoteDemoCharges).
    PlantRemoteDemoCharge {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: detonate all remote demo charges planted by
    /// the selected unit(s) (SPECIAL_REMOTE_CHARGES no-target path).
    DetonateRemoteDemoCharges,
    /// Demo General residual: intentional SUICIDED detonation
    /// (`Demo_Command_TertiarySuicide` FIRE_WEAPON tertiary after
    /// `Demo_Upgrade_SuicideBomb` CommandSetUpgrade residual).
    DemoTertiarySuicide,
    /// Black Lotus residual: steal cash from enemy supply/cash building.
    StealCashHack {
        target_id: ObjectId,
    },
    /// Black Lotus residual: disable enemy ground vehicle (DISABLED_HACKED).
    DisableVehicleHack {
        target_id: ObjectId,
    },
    /// China Hacker residual: disable enemy structure (DISABLED_HACKED).
    /// SpecialAbilityHackerDisableBuilding.
    HackerDisableBuilding {
        target_id: ObjectId,
    },
    /// GLA Bomb Truck residual: disguise as target vehicle
    /// (SpecialAbilityDisguiseAsVehicle / StealthUpdate::disguiseAsTemplate).
    DisguiseAsVehicle {
        target_id: ObjectId,
    },
    /// GLA Rebel residual: plant BoobyTrap on structure (SpecialAbilityBoobyTrap).
    PlantBoobyTrap {
        target_id: ObjectId,
    },
    /// C++ `MSG_SWITCH_WEAPONS` — lock the button's weapon slot permanently.
    /// `ControlBarCommandProcessing.cpp:786-794` / `GameLogicDispatch.cpp:583-590`.
    SwitchWeapons {
        slot: u8,
    },
    ToggleOvercharge,

    // Formation and group commands
    CreateFormation,
    Cheer,

    // Network/multiplayer commands
    PlaceBeacon {
        location: Vec3,
        text: String,
    },
    RemoveBeacon,
    /// C++ `MSG_SET_BEACON_TEXT` — caption on the selected beacon drawable.
    SetBeaconText {
        text: String,
    },
    /// C++ `MSG_ENABLE_RETALIATION_MODE` (`GameLogicDispatch.cpp:603-614`).
    EnableRetaliationMode {
        player_index: u32,
        enabled: bool,
    },
    /// C++ `MSG_SELF_DESTRUCT` (`GameLogicDispatch.cpp:1762-1797`).
    /// `transfer_to_ally` is argument 0: true = give units to a living mutual
    /// ally then `killPlayer`; false = `killPlayer` immediately.
    SelfDestruct {
        transfer_to_ally: bool,
    },
    ViewLastRadarEvent,
    ViewRadarAt {
        position: Vec3,
    },
    ViewCommandCenter,
    /// C++ `MSG_DO_SALVAGE` — salvage crate click. Location-only, like MoveTo.
    DoSalvage {
        destination: Vec3,
    },

    // Invalid command placeholder
    Invalid,
}

/// Target types for guard command
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardTarget {
    Position(Vec3),
    Object(ObjectId),
}

/// Target types for special powers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PowerTarget {
    Location(Vec3),
    /// C++ `MSG_DO_SPECIAL_POWER_AT_LOCATION` location + placement angle
    /// (`PlaceEventTranslator` sneak-attack construct).
    LocationFacing {
        pos: Vec3,
        angle: f32,
    },
    Object(ObjectId),
    None,
}

impl PowerTarget {
    pub fn location_pos(&self) -> Option<Vec3> {
        match self {
            Self::Location(pos) => Some(*pos),
            Self::LocationFacing { pos, .. } => Some(*pos),
            _ => None,
        }
    }

    /// C++ `appendRealArgument(angle)` residual. Zero when no facing was stored.
    pub fn location_angle(&self) -> f32 {
        match self {
            Self::LocationFacing { angle, .. } => *angle,
            _ => 0.0,
        }
    }

    pub fn from_location_and_angle(pos: Vec3, angle: f32) -> Self {
        if angle.abs() <= f32::EPSILON {
            Self::Location(pos)
        } else {
            Self::LocationFacing { pos, angle }
        }
    }
}

/// Target types for weapon commands
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WeaponTarget {
    Location(Vec3),
    Object(ObjectId),
}

/// Target types for combat drops
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DropTarget {
    Location(Vec3),
    Object(ObjectId),
}
