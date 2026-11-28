//! Values exchanged synchronously with the match running an authored script.

use crate::kind_of::classic::KindOf;
use game_engine::common::game_common::Relationship;
use std::collections::{HashMap, HashSet};

/// Live owner query supplied by the world driving this execution.
/// Unavailable is reserved for the standalone canonical adapter; Missing is
/// authoritative absence and must never fall back to a frozen host snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptOwnerQuery<T> {
    Unavailable,
    Missing,
    Present(T),
}

pub struct ScriptCommandButton {
    pub special_power: Option<String>,
    pub upgrade: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptTeamAreaTransition {
    AllEntered,
    PartiallyEntered,
    AllExited,
    PartiallyExited,
}

/// The three CPP player-area census operations share geometry and ownership,
/// while retaining their distinct liveness, crate and cache rules.
pub enum ScriptPlayerAreaQuery<'a> {
    Templates(&'a [String]),
    Kind(KindOf),
    Value,
    Units,
}

/// The scalar result and SIDE lookup state from an owned polygon census.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPlayerAreaCensus {
    MissingPlayer,
    MissingArea {
        player_mask: u32,
    },
    Measured {
        player_mask: u32,
        value: i32,
        membership_changed: bool,
    },
}

/// The two supply conditions make distinct AI queries. A safety check is
/// given the authored cash threshold while an attack check consumes a scan.
#[derive(Clone, Copy)]
pub enum ScriptSupplyQuery {
    Safe { minimum_cash: i32 },
    Attacked,
}

/// Data needed by the current authored player-census condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPlayerCensusScope {
    MoneyAndPower,
    ObjectCounts,
    TemplateCounts,
}

#[derive(Clone, Copy)]
pub enum ScriptSkirmishCensus {
    NeutralUnmanned,
    Garrisoned(u32),
    Captured(u32),
}

pub enum ScriptSpecialPowerReadiness {
    Ready,
    NotReady { earliest_ready_frame: Option<u32> },
}

/// CPP named unit pointer lifetime, distinct from alive/dead state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptNamedLifecycle {
    pub exists: bool,
    pub effectively_dead: bool,
    pub ever_existed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptObjectStatus {
    pub has_ai: bool,
    pub idle: bool,
    pub effectively_dead: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptTeamStatus {
    pub has_group: bool,
    pub idle: bool,
    pub dead: bool,
}

/// The exact Team selected by the script's calling and condition context.
pub struct ScriptConditionTeam {
    pub controller: Option<u32>,
    pub members: Vec<u32>,
}

pub struct ScriptDamageSource<'a> {
    pub recorded_template: Option<&'a str>,
    pub live_attacker: Option<ScriptAttacker<'a>>,
}

pub struct ScriptAttacker<'a> {
    pub template: &'a str,
    pub controller: Option<u32>,
}

/// The actual contain interface's counts, independent of Enter admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptContainmentStatus {
    pub count: u32,
    pub max: u32,
}

/// CPP ScriptConditions1994: consider installed roster members in visitation order.
/// Physically free AI_EXIT objects remain free in the original expression.
pub fn script_team_is_contained(
    members: impl IntoIterator<Item = Option<bool>>,
    all: bool,
) -> bool {
    let mut considered = false;
    for contained in members.into_iter().flatten() {
        if contained && !all {
            return true;
        }
        if !contained && all {
            return false;
        }
        considered = true;
    }
    considered && all
}

/// CPP getTeamNamed selection and Team::hasAnyObjects, not AIGroup death.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptTeamPresence {
    pub created: bool,
    pub has_objects: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptTeamAreaMembership {
    pub any_considered: bool,
    pub any_inside: bool,
    pub any_outside: bool,
}

/// A named bridge's terrain edges after the interpreter checks the aggregate edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptBridgeStatus {
    pub broken: bool,
    pub repaired: bool,
}

/// CPP ScriptConditions1029/1086: only the enemy family filters relationships.
#[derive(Debug, Clone, Copy)]
pub enum ScriptSightingFilter<'a> {
    Relationship(Relationship),
    Types(&'a [String]),
}

#[derive(Debug, Clone, Copy)]
pub struct ScriptSightingObserver {
    pub position: glam::Vec3,
    pub vision_range: f32,
    pub off_map: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ScriptSightingCandidate<'a> {
    pub position: glam::Vec3,
    pub off_map: bool,
    pub effectively_stealthed: bool,
    pub owner: Option<u32>,
    pub template: &'a str,
}

/// Parsed display effect for the game driving this action. Borrowed text is
/// valid only during the synchronous callback; the owner queues its own copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptDisplayRequest<'a> {
    Text(&'a str),
    Cinematic {
        text: &'a str,
        font: &'a str,
        duration_seconds: i32,
    },
    MilitaryCaption {
        text: &'a str,
        duration_ms: i32,
    },
}

/// CPP ScriptActions:2243,6210,1250: synchronous player AI effects.
/// Skillsets remain script-numbered until the receiving owner converts them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptAiPlayerRequest<'a> {
    BuildUpgrade {
        player: &'a str,
        upgrade: &'a str,
    },
    BuildNearest {
        player: &'a str,
        building: &'a str,
        team: &'a str,
        calling: Option<u32>,
        condition: Option<u32>,
    },
    BaseDefense {
        player: &'a str,
        structure: Option<&'a str>,
        flank: bool,
    },
    FireSpecialPower {
        player: &'a str,
        power: &'a str,
    },
    BuildTeam {
        team: &'a str,
    },
    RecruitTeam {
        team: &'a str,
        radius: f32,
    },
    BuildSupplyCenter {
        player: &'a str,
        building: &'a str,
        cash: i32,
        current_player: Option<&'a str>,
    },
    RepairNamed {
        player: &'a str,
        structure: &'a str,
    },
    SelectSkillset {
        player: &'a str,
        script_skillset: i32,
    },
    SetTeamDelay {
        player: &'a str,
        seconds: i32,
    },
}

/// C++ ScriptActions player construction toggles; the raw selector is kept
/// until the receiving owner resolves it against the current interpreter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptPlayerConstructionRequest {
    Units {
        player: String,
        enabled: bool,
    },
    Base {
        player: String,
        enabled: bool,
    },
    Factories {
        player: String,
        template: String,
        enabled: bool,
    },
}

/// Rank actions retain the authored player selector until the receiving
/// interpreter resolves it against its own match.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptPlayerRankRequest {
    AddSkillPoints { player: String, delta: i32 },
    AddRankLevel { player: String, delta: i32 },
    SetRankLevel { player: String, level: i32 },
    SetRankLevelLimit { limit: i32 },
    AffectReceivingExperience { player: String, modifier: f32 },
}

/// CPP ScriptActions1042–1062/1861–1878/1966–1980/6081–6095: named AI commands.
/// The owner resolves literal names and THIS_OBJECT within its own object store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptNamedCommand<'a> {
    ForceAttack { unit: &'a str, target: &'a str },
    FaceObject { unit: &'a str, target: &'a str },
    Hunt { unit: &'a str },
    Guard { unit: &'a str },
    Stop { unit: &'a str },
    Enter { unit: &'a str, target: &'a str },
    Evacuate { unit: &'a str },
}

/// Named runtime mutations retain the action's actual receiving actor.
#[derive(Debug, Clone, Copy)]
pub enum ScriptNamedRuntimeRequest<'a> {
    FaceWaypoint {
        unit: &'a str,
        waypoint: &'a str,
    },
    TransferOwnership {
        unit: &'a str,
        player: &'a str,
    },
    FireWeaponPath {
        unit: &'a str,
        path: &'a str,
    },
}

/// CPP ScriptActions5136-5308: nearest selection and goal-object movement.
#[derive(Debug, Clone, Copy)]
pub enum ScriptNearestRecipient<'a> {
    Named(&'a str),
    Team(&'a str),
}

#[derive(Debug, Clone, Copy)]
pub struct ScriptNearestObjectRequest<'a> {
    pub recipient: ScriptNearestRecipient<'a>,
    pub object_type: &'a str,
    pub object_types: &'a [String],
    pub trigger: &'a str,
    pub current_player: Option<&'a str>,
    pub this_object: Option<u32>,
    pub calling_team: Option<u32>,
    pub condition_team: Option<u32>,
}

/// CPP ScriptActions water changes are synchronous with the next action.
#[derive(Debug, Clone, Copy)]
pub enum ScriptWaterRequest<'a> {
    SetHeight {
        name: &'a str,
        height: f32,
    },
    OverTime {
        name: &'a str,
        height: f32,
        seconds: f32,
        damage: f32,
    },
}

/// C++ `doRevealMapEntire*` / `doShroudMapEntire` operations, applied in the
/// authored script action phase to the receiving world's shroud owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptMapVisibilityOperation {
    Reveal,
    RevealPermanently,
    UndoPermanentReveal,
    Shroud,
}

/// Directed overrides owned by an existing Team in the executing session.
#[derive(Debug, Clone, Copy)]
pub enum ScriptTeamRelationship<'a> {
    SetTeam {
        source: &'a str,
        target: &'a str,
        relationship: Relationship,
    },
    RemoveTeam {
        source: &'a str,
        target: &'a str,
    },
    SetPlayer {
        source: &'a str,
        target: &'a str,
        relationship: Relationship,
    },
    RemovePlayer {
        source: &'a str,
        target: &'a str,
    },
    RemoveAll {
        source: &'a str,
    },
}

/// An immediate Player-to-Team override in the executing session.
#[derive(Debug, Clone, Copy)]
pub enum ScriptPlayerTeamRelationship<'a> {
    Set {
        player: &'a str,
        team: &'a str,
        relationship: Relationship,
    },
    Remove {
        player: &'a str,
        team: &'a str,
    },
}

/// Synchronous effects and live queries of the actual execution owner.
/// The driver is borrowed for execution only; it is never installed in the
/// engine, dispatcher context, or a process-wide active slot.
/// CPP ScriptActions952: the driving interpreter admits an object onto its own team.
#[derive(Debug, Clone, Copy)]
pub struct ScriptObjectCreation<'a> {
    pub template: &'a str,
    pub team: &'a str,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub angle: f32,
}

/// Synchronous presentation/native effects of the driving script owner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptWorldRequest<'a> {
    /// Switch the driving match's authored terrain boundary synchronously.
    MapSwitchBoundary(i32),
    Input {
        enabled: bool,
    },
    MovieFullscreen(&'a str),
    RadarForced(bool),
    /// C++ `TheRadar->refreshTerrain` on the receiving match radar.
    RadarRefreshTerrain,
    Timer {
        name: &'a str,
        text: &'a str,
        countdown: bool,
    },
    Freeze(bool),
    VisualSpeed(i32),
    Weather(bool),
    Music {
        track: &'a str,
        fade_out: bool,
        fade_in: bool,
    },
}

/// Actual UI selection observation; the stamp is produced by selection mutation.
#[derive(Clone, Copy, Debug)]
pub struct ScriptSelectionStatus {
    pub multiplayer: bool,
    pub changed_frame: u32,
    pub selected: bool,
}

/// Synchronous script match-audio effects, independent of application sliders.
#[derive(Debug, Clone, Copy)]
pub enum ScriptAudioRequest<'a> {
    StopMusic {
        fade: bool,
    },
    SetVolume {
        volume: f32,
        affect: game_engine::common::audio::AudioAffect,
    },
    NamedVolume {
        name: &'a str,
        volume: f32,
    },
    RemoveNamed(&'a str),
    RemoveDisabled,
    Pause(game_engine::common::audio::AudioAffect),
    Resume(game_engine::common::audio::AudioAffect),
}
/// Parsed camera movement, view effect, or animation modifier for the driving game.
/// Coordinates retain engine Coord3D axes; the host translates at its boundary.
/// A request is synchronous and must be flushed before the next instruction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptCameraRequest<'a> {
    MoveTo {
        x: f32,
        y: f32,
        z: f32,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    },
    WaypointPath {
        waypoint_path: &'a str,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    },
    Rotate {
        rotations: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    },
    MoveToSelection,
    MoveHome,
    Zoom {
        zoom: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    },
    Pitch {
        pitch: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    },
    FreezeTime,
    FreezeAngle,
    FinalZoom {
        zoom: f32,
        ease_in: f32,
        ease_out: f32,
    },
    FinalPitch {
        pitch: f32,
        ease_in: f32,
        ease_out: f32,
    },
    FinalSpeedMultiplier {
        multiplier: i32,
    },
    RollingAverage {
        frames: i32,
    },
    FollowObject {
        object_id: u32,
        snap_to_unit: bool,
    },
    TetherObject {
        object_id: u32,
        snap_to_unit: bool,
        play: f32,
    },
    StopFollow,
    ResetTo {
        x: f32,
        y: f32,
        z: f32,
        duration_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    },
    Setup {
        x: f32,
        y: f32,
        z: f32,
        zoom: f32,
        pitch: f32,
        look_toward_x: f32,
        look_toward_y: f32,
        look_toward_z: f32,
    },
    LookTowardObject {
        object_id: u32,
        seconds: f32,
        hold_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    },
    LookTowardWaypoint {
        x: f32,
        y: f32,
        z: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
        reverse_rotation: bool,
    },
    LookToward {
        x: f32,
        y: f32,
        z: f32,
    },
    FinalLookToward {
        x: f32,
        y: f32,
        z: f32,
    },
    LetterboxBegin,
    LetterboxEnd,
    SetDefault {
        pitch: f32,
        angle: f32,
        max_height: f32,
    },
    EnableSlaveMode {
        thing_template_name: &'a str,
        bone_name: &'a str,
    },
    DisableSlaveMode,
    ScreenShake {
        intensity: i32,
    },
    AddShakerAt {
        x: f32,
        y: f32,
        z: f32,
        amplitude: f32,
        duration_seconds: f32,
        radius: f32,
    },
    ViewGuardband {
        gbx: f32,
        gby: f32,
    },
    BwMode {
        enabled: bool,
        frames: i32,
    },
    SkyboxEnabled {
        enabled: bool,
    },
    MotionBlur {
        zoom_in: bool,
        saturate: bool,
    },
    MotionBlurJump {
        x: f32,
        y: f32,
        z: f32,
        saturate: bool,
    },
    MotionBlurFollow {
        amount: i32,
    },
    MotionBlurEndFollow,
}

#[derive(Debug, Clone, Default)]
pub struct HostScriptQueryObject {
    pub id: u32,
    pub name: String,
    pub team: u32,
    pub x: f32,
    /// Host Y-up height. C++ AudioEventRTS Z-up uses this as `z`.
    pub y: f32,
    pub z: f32,

    pub alive: bool,
    /// C++ Object::isEffectivelyDead while the pointer still exists.
    pub effectively_dead: bool,
    pub health: f32,
    pub initial_health: f32,
    pub owner_player: String,
    pub template_name: String,
    pub has_contain: bool,
    pub contain_count: u32,
    pub contain_max: u32,
    pub last_damage_source_id: u32,
    pub last_damage_template: String,
    pub last_damage_player: String,
    pub kind_structure: bool,
    pub kind_projectile: bool,
    pub kind_inert: bool,
    pub kind_mine: bool,
    pub held: bool,
    pub stealthed_hidden: bool,
    pub discovered_by: Vec<String>,
    pub waypoint_labels: Vec<String>,
    /// C++ Drawable selected / Object::isSelected for NAMED_SELECTED.
    pub selected: bool,
    /// C++ AIUpdateInterface::isIdle for sequential UNIT/TEAM progress.
    pub idle: bool,
    /// C++ Object::getVisionRange for ENEMY/TYPE_SIGHTED.
    pub vision_range: f32,
    /// Host KindOf Debug names (Infantry, Vehicle, Structure, …).
    /// Shared per KindOf set (the host snapshot rebuilds every object each frame).
    pub kind_names: std::sync::Arc<Vec<String>>,
    /// C++ SpecialPowerModule::getPercentReady() == 1 for host isReady.
    pub special_power_ready: bool,
    /// Canonical SpecialPowerTemplate names this object owns.
    pub special_power_templates: Vec<String>,
    /// C++ LocomotorSet::getValidSurfaces; 0 means no AI → GROUND.
    pub locomotor_surfaces: u32,
    /// C++ Object::isCaptured (private CAPTURED status).
    pub captured: bool,
    /// C++ DISABLED_UNMANNED (unowned faction vehicle).
    pub unmanned: bool,
    /// C++ ContainModuleInterface::isGarrisonable.
    pub garrisonable: bool,
    /// C++ ThingTemplate::friend_getBuildCost.
    pub build_cost: i32,
    /// C++ Object::getStatusBits packed mask (OBJECT_STATUS_*).
    pub status_bits: u64,
    /// C++ OpenContain::getPlayerWhoEntered — last enterer's SIDE name.
    pub player_who_entered: String,
    /// C++ findUpdateModule("SupplyWarehouseDockUpdate") present.
    pub is_supply_warehouse: bool,
    /// C++ SupplyWarehouseDockUpdate::getBoxesStored.
    pub warehouse_boxes: i32,
    /// C++ Object::isOffMap — PartitionFilterOnMap reject.
    pub off_map: bool,
    /// C++ Object::getContainedBy id (0 = none) for TEAM_WAIT_FOR_NOT_CONTAINED.
    pub contained_by: u32,
    /// C++ AI_EXIT pretend-contained while evacuating (leftover Exit state).
    pub ai_exiting: bool,
}

/// Live host Player::getMoney / getEnergy / hasAnyObjects census.
/// C++ ScriptConditions player conditions read these from the same Player.
#[derive(Debug, Clone, Default)]
pub struct HostScriptPlayerCensus {
    /// C++ Player::getMoney()->countMoney().
    pub money: i32,
    /// C++ Energy production (0 while sabotaged).
    pub energy_production: i32,
    /// C++ Energy consumption.
    pub energy_consumption: i32,
    /// C++ Energy::m_powerSabotagedTillFrame still active.
    pub power_sabotaged: bool,
    /// C++ Player::hasAnyObjects().
    pub has_any_objects: bool,
    /// C++ Player::hasAnyBuildFacility().
    pub has_any_build_facility: bool,
    /// C++ Player::countBuildings().
    pub building_count: i32,
    /// C++ Player::countObjects(STRUCTURE|MP_COUNT_FOR_VICTORY).
    pub faction_building_count: i32,
    /// C++ Player::getSciencePurchasePoints().
    pub science_purchase_points: i32,
    /// C++ Player science vec names (SCIENCE_RankN / purchased).
    pub unlocked_sciences: Vec<String>,
    /// C++ countObjectsByThingTemplate(..., ignoreDead=false, ignoreUnderConstruction=true).
    /// Keyed by lowercase ThingTemplate name.
    pub template_counts: HashMap<String, i32>,
    /// C++ countObjectsByThingTemplate(..., ignoreDead=true, ignoreUnderConstruction=true).
    pub template_counts_ignore_dead: HashMap<String, i32>,
    /// C++ Player::getSupplyBoxValue (GlobalData m_baseValuePerSupplyBox).
    pub supply_box_value: i32,
}

impl HostScriptPlayerCensus {
    /// C++ Energy::hasSufficientPower.
    pub fn has_sufficient_power(&self) -> bool {
        if self.power_sabotaged {
            false
        } else {
            self.energy_production >= self.energy_consumption
        }
    }

    /// C++ Energy::getEnergySupplyRatio.
    pub fn supply_ratio(&self) -> f32 {
        if self.power_sabotaged {
            return 0.0;
        }
        if self.energy_consumption <= 0 {
            self.energy_production as f32
        } else {
            self.energy_production as f32 / self.energy_consumption as f32
        }
    }

    /// C++ production - consumption (sabotage zeros production).
    pub fn excess_power(&self) -> i32 {
        if self.power_sabotaged {
            -self.energy_consumption
        } else {
            self.energy_production - self.energy_consumption
        }
    }

    /// Sum census counts for the given template / ObjectTypes names.
    pub fn count_templates(
        &self,
        type_names: impl IntoIterator<Item = impl AsRef<str>>,
        ignore_dead: bool,
    ) -> i32 {
        let map = if ignore_dead {
            &self.template_counts_ignore_dead
        } else {
            &self.template_counts
        };
        let mut seen = HashSet::new();
        let mut sum = 0;
        for name in type_names {
            let key = name.as_ref().trim().to_ascii_lowercase();
            if key.is_empty() || !seen.insert(key.clone()) {
                continue;
            }
            sum += map.get(&key).copied().unwrap_or(0);
        }
        sum
    }
}

/// Synchronous Core-to-host script action payload for named sound actions.
/// Main consumes this only through its receiving `ScriptExecutionDriver`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptObjectSoundRequest {
    PlayNamed { sound: String, unit: String },
    Enable { unit: String, enable: bool },
}

/// Synchronous owner request: PLAYER_SET_MONEY / PLAYER_GIVE_MONEY.
/// C++ `ScriptActions::doSetMoney` / `doGiveMoney`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptMoneyRequest {
    Set { player: String, amount: i32 },
    Give { player: String, amount: i32 },
}

/// Synchronous commands for the receiving ScriptExecutionDriver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostScriptGarrisonEnterExitRequest {
    TeamEnter { team: String, dest: String },
    TeamGarrisonSpecific { team: String, building: String },
    ExitSpecificBuilding { building: String },
    TeamGarrisonNearest { team: String },
    TeamExitAllBuildings { team: String },
    NamedGarrisonSpecific { unit: String, building: String },
    NamedGarrisonNearest { unit: String },
    NamedExitBuilding { unit: String },
    PlayerGarrisonAll { player: String },
    PlayerExitAll { player: String },
}
