use super::*;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Exact family of C++ `DockUpdateInterface` exposed by an Object INI.
///
/// This is deliberately separate from `KindOf`: a SupplyCenter, a supply
/// warehouse, and a railed transport all accept `MSG_DOCK`, but their legality
/// and execution are different.  `None` is the backwards-compatible snapshot
/// default for templates created before module metadata was retained.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockKind {
    None = 0,
    SupplyCenter = 1,
    SupplyWarehouse = 2,
    RailedTransport = 3,
}

impl Default for DockKind {
    fn default() -> Self {
        Self::None
    }
}

impl DockKind {
    #[inline]
    pub const fn from_ordinal(value: u8) -> Self {
        match value {
            1 => Self::SupplyCenter,
            2 => Self::SupplyWarehouse,
            3 => Self::RailedTransport,
            _ => Self::None,
        }
    }
}

/// Concrete containment behavior retained from an Object INI `Behavior`
/// declaration.  C++ `ActionManager::canEnterObject` asks the target for a
/// real `ContainModuleInterface`; being a VEHICLE is not itself evidence of a
/// container.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainModuleKind {
    None = 0,
    Transport = 1,
    RiderChange = 2,
    RailedTransport = 3,
    Garrison = 4,
    /// C++ `InternetHackContain`.  This is a structure-side transport
    /// interface with exact controller and `TransportSlotCount` accounting;
    /// it is deliberately separate from a generic garrison.
    InternetHack = 5,
    /// C++ `HealContain` (barracks / hospital). Not a transport; heals then
    /// auto-exits. `isHealContain() == true`.
    Heal = 6,
    /// C++ `CaveContain` (CaveSystem shared tracker).
    Cave = 7,
    /// C++ `TunnelContain` (Player::TunnelTracker shared pool).
    Tunnel = 8,
}

impl Default for ContainModuleKind {
    fn default() -> Self {
        Self::None
    }
}

impl ContainModuleKind {
    #[inline]
    pub const fn is_mobile_container(self) -> bool {
        matches!(
            self,
            Self::Transport | Self::RiderChange | Self::RailedTransport
        )
    }

    /// C++ `ContainModuleInterface::isHealContain`.
    #[inline]
    pub const fn is_heal_contain(self) -> bool {
        matches!(self, Self::Heal)
    }

    /// C++ `ContainModuleInterface::isTunnelContain`.
    #[inline]
    pub const fn is_tunnel_contain(self) -> bool {
        matches!(self, Self::Tunnel)
    }

    /// C++ CaveContain (CaveSystem index, not Player::TunnelTracker).
    #[inline]
    pub const fn is_cave_contain(self) -> bool {
        matches!(self, Self::Cave)
    }
}

/// The subset of C++ `AllowInsideKindOf`/`ForbidInsideKindOf` that the active
/// Rust object model can represent without guessing.  Leftover-known KindOf
/// bits such as `HUGE_VEHICLE` stay on the module mask and are applied by
/// leftover OpenContain algebra.  An unrepresentable name is `Unsupported`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainAdmission {
    /// No retained source module / a mask the host cannot represent.
    Unsupported = 0,
    /// C++ default: every mobile kind is admitted.
    AnyMobile = 1,
    /// `AllowInsideKindOf = INFANTRY`.
    InfantryOnly = 2,
    /// `AllowInsideKindOf = INFANTRY VEHICLE` or an equivalent aircraft ban.
    InfantryOrVehicle = 3,
    /// Exact `InternetHackContain::AllowInsideKindOf = MONEY_HACKER`.
    /// Keep this separate from Infantry: Black Lotus and arbitrary infantry
    /// cannot enter an Internet Center merely because they share that broad
    /// class.
    MoneyHackerOnly = 4,
}

impl Default for ContainAdmission {
    fn default() -> Self {
        // Older snapshots must not turn an arbitrary vehicle into a transport.
        Self::Unsupported
    }
}

/// Frozen, exact Object INI containment data used by normal Enter.  This is
/// intentionally separate from the specialized host transport flags: those
/// flags retain explicit implemented behavior, while this metadata makes newly
/// parsed retail containers usable without a template-name heuristic.
/// One authored `RiderN` record from `RiderChangeContain`.
///
/// C++ stores these as independent template/model-condition/weapon-set/status/
/// command-set/locomotor values and asks `ThingTemplate::isEquivalentTo` at
/// admission time.  The active host has no source-side reskin/build-variation
/// equivalence graph, so `template_matches` deliberately retains only the
/// exact, case-insensitive Object INI identity.  A variant without that exact
/// identity stays rejected instead of being accepted by a name heuristic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiderChangeRiderMetadata {
    /// C++ Rider1..Rider8 ordinal.
    pub slot: u8,
    /// Authored rider ThingTemplate identity.
    pub template_name: String,
    /// Authored ModelCondition flag, retained even when the host cannot apply
    /// the record physically.
    pub model_condition: String,
    /// Authored WeaponSet flag.  The active Combat Cycle bridge consumes the
    /// selected rider *slot*, never this template spelling.
    pub weapon_set: String,
    /// Authored ObjectStatus bit name.
    pub object_status: String,
    /// C++ Object::m_commandSetStringOverride while this rider is contained.
    pub command_set: String,
    /// C++ RiderChangeContain-selected locomotor set token.
    pub locomotor_set: String,
    /// Exact primary locomotor selected from the corresponding source
    /// `Locomotor = SET_* ...` row, when Main can represent that row.  The
    /// full row remains on ObjectDefinition; unsupported sets retain `None`.
    #[serde(default)]
    pub active_locomotor_name: Option<String>,
    /// Every exact source locomotor in the selected SET_* row, in authored
    /// order.  C++ chooses one by surface; Main admits the row only when its
    /// represented members share one safe active behavior profile.
    #[serde(default)]
    pub active_locomotor_names: Vec<String>,
    /// Union of the represented members' source surface masks.
    #[serde(default)]
    pub active_locomotor_surfaces: u32,
    /// Active model-condition representation, zero only when unsupported.
    #[serde(default)]
    pub model_condition_mask: u128,
    /// Active ObjectStatus representation, zero only when unsupported.
    #[serde(default)]
    pub object_status_mask: u64,
    /// True only when every effect needed by the bounded physical Combat Cycle
    /// transaction is represented.  Parsed-but-unsupported records remain in
    /// the template for save/presentation fidelity but cannot authorize RMB.
    #[serde(default)]
    pub physical_enter_supported: bool,
}

impl RiderChangeRiderMetadata {
    #[inline]
    pub fn template_matches(&self, template_name: &str) -> bool {
        !self.template_name.is_empty()
            && self
                .template_name
                .eq_ignore_ascii_case(template_name.trim())
    }
}

/// Frozen, exact Object INI containment data used by normal Enter.  This is
/// intentionally separate from the specialized host transport flags: those
/// flags retain explicit implemented behavior, while this metadata makes newly
/// parsed retail containers usable without a template-name heuristic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainModuleMetadata {
    #[serde(default)]
    pub kind: ContainModuleKind,
    /// `Slots` for Transport/RiderChange/RailedTransport, or `ContainMax` for
    /// GarrisonContain / HealContain. `Some(0)` is an authored zero-capacity
    /// module and must remain distinct from no contain module.
    #[serde(default)]
    pub slots: Option<usize>,
    #[serde(default)]
    pub admission: ContainAdmission,
    /// C++ OpenContain defaults are all true; retain authored overrides.
    #[serde(default = "default_allow_inside")]
    pub allow_allies_inside: bool,
    #[serde(default = "default_allow_inside")]
    pub allow_enemies_inside: bool,
    #[serde(default = "default_allow_inside")]
    pub allow_neutral_inside: bool,
    /// Exact authored Rider1..Rider8 table.  Empty is distinct from an empty
    /// `RiderChangeContain`: the latter must remain fail-closed for physical
    /// Enter because C++ has no generic rider fallback.
    #[serde(default)]
    pub rider_change_riders: Vec<RiderChangeRiderMetadata>,
    /// C++ `ScuttleDelay`, already converted with `parseDurationUnsignedInt`
    /// semantics to logic frames.  `None` means the module was not parsed;
    /// `Some(0)` is the C++ default and destroys on the next update.
    #[serde(default)]
    pub rider_change_scuttle_delay_frames: Option<u32>,
    /// C++ `ScuttleStatus` raw ModelCondition token (default TOPPLED).
    #[serde(default)]
    pub rider_change_scuttle_status: String,
    /// Active model-condition bit corresponding to `ScuttleStatus`.
    #[serde(default)]
    pub rider_change_scuttle_status_mask: u128,
    /// C++ HealContain / TunnelContain `TimeForFullHeal`, already converted
    /// with `parseDurationUnsignedInt` semantics to logic frames. `None`
    /// means the field was not authored. HealContain default is 0 (instant
    /// complete); TunnelContain default is 1 frame.
    #[serde(default)]
    pub frames_for_full_heal: Option<u32>,
    /// C++ GarrisonContainModuleData::m_immuneToClearBuildingAttacks (default false).
    #[serde(default)]
    pub immune_to_clear_building_attacks: bool,
    /// C++ GarrisonContainModuleData::m_isEnclosingContainer (default true).
    #[serde(default = "default_enclosing_container")]
    pub is_enclosing_container: bool,
    /// C++ CaveContainModuleData::m_caveIndexData (default 0).
    #[serde(default)]
    pub cave_index: i32,
    /// C++ GarrisonContainModuleData::m_doIHealObjects (default false).
    #[serde(default)]
    pub heal_objects: bool,
    /// C++ GarrisonContainModuleData::m_initialRoster.templateName.
    #[serde(default)]
    pub initial_roster_template: String,
    /// C++ GarrisonContainModuleData::m_initialRoster.count (0 = none).
    #[serde(default)]
    pub initial_roster_count: i32,
    /// C++ OpenContain::isWeaponBonusPassedToPassengers residual.
    #[serde(default)]
    pub weapon_bonus_passed_to_passengers: bool,
    /// C++ `OpenContainModuleData::m_enterSound` (INI `EnterSound`).
    #[serde(default)]
    pub enter_sound: String,
    /// C++ `OpenContainModuleData::m_exitSound` (INI `ExitSound`).
    #[serde(default)]
    pub exit_sound: String,
    /// Leftover `OpenContainModuleData::allow_inside_kind_of` (C++ KindOf mask).
    /// Zero means leftover OpenContain's "no allow restriction" path.
    #[serde(default)]
    pub allow_inside_kind_of: u128,
    /// Leftover `OpenContainModuleData::forbid_inside_kind_of` (C++ KindOf mask).
    #[serde(default)]
    pub forbid_inside_kind_of: u128,
    /// C++ TransportContainModuleData::m_keepContainerVelocityOnExit (default false).
    /// No retail Object INI authors this; do not invent a hull-velocity kick.
    #[serde(default)]
    pub keep_container_velocity_on_exit: bool,
    /// C++ `OpenContainModuleData::m_doorOpenTime` (default 1 frame).
    /// `0` is DeliverPayloadAIUpdate's opt-out so this module never diddles doors.
    #[serde(default = "default_door_open_time")]
    pub door_open_time: u32,
}

/// Exact `OverchargeBehaviorModuleData` retained from one Object INI behavior
/// declaration.  Presence is the authority contract: a power-plant KindOf or
/// a template spelling never fabricates this module.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OverchargeBehaviorMetadata {
    /// C++ `HealthPercentToDrainPerSecond`, already converted from its INI
    /// percentage representation (for example `3%` becomes `0.03`).
    pub health_percent_to_drain_per_second: f32,
    /// C++ `NotAllowedWhenHealthBelowPercent`, likewise a real fraction.
    pub not_allowed_when_health_below_percent: f32,
}

impl Default for OverchargeBehaviorMetadata {
    fn default() -> Self {
        // `OverchargeBehaviorModuleData` initializes both fields to zero.
        Self {
            health_percent_to_drain_per_second: 0.0,
            not_allowed_when_health_below_percent: 0.0,
        }
    }
}

/// The narrow `PowerPlantUpdate` data consumed by Overcharge's rod animation
/// hook.  This stays separate from `OverchargeBehavior`: C++ toggles power
/// without requiring a PowerPlantUpdate interface, but only that separate
/// interface owns the POWER_PLANT_UPGRADING/UPGRADED model conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerPlantUpdateMetadata {
    /// C++ `RodsExtendTime`, parsed to logic frames.
    pub rods_extend_time_frames: u32,
}

const fn default_allow_inside() -> bool {
    true
}

const fn default_enclosing_container() -> bool {
    true
}

const fn default_door_open_time() -> u32 {
    1
}

impl Default for ContainModuleMetadata {
    fn default() -> Self {
        Self {
            kind: ContainModuleKind::None,
            slots: None,
            admission: ContainAdmission::Unsupported,
            allow_allies_inside: true,
            allow_enemies_inside: true,
            allow_neutral_inside: true,
            rider_change_riders: Vec::new(),
            rider_change_scuttle_delay_frames: None,
            rider_change_scuttle_status: String::new(),
            rider_change_scuttle_status_mask: 0,
            frames_for_full_heal: None,
            immune_to_clear_building_attacks: false,
            is_enclosing_container: true,
            cave_index: 0,
            heal_objects: false,
            initial_roster_template: String::new(),
            initial_roster_count: 0,
            weapon_bonus_passed_to_passengers: false,
            enter_sound: String::new(),
            exit_sound: String::new(),
            allow_inside_kind_of: 0,
            forbid_inside_kind_of: 0,
            keep_container_velocity_on_exit: false,
            door_open_time: 1,
        }
    }
}

impl ContainModuleMetadata {
    /// The bounded live implementation models the retail one-seat Combat
    /// Cycle transaction only.  A malformed/custom multi-seat RiderChange
    /// module is retained but is not advertised as ordinary Enter.
    #[inline]
    pub fn has_supported_rider_change_roster(&self) -> bool {
        let supported = self
            .rider_change_riders
            .iter()
            .any(|rider| rider.physical_enter_supported && rider.active_locomotor_name.is_some());
        self.kind == ContainModuleKind::RiderChange
            && self.slots == Some(1)
            && self.admission != ContainAdmission::Unsupported
            && self.rider_change_scuttle_delay_frames.is_some()
            && self.rider_change_scuttle_status_mask != 0
            && supported
            // C++'s first equivalent entry would make duplicate authored
            // identities declaration-order sensitive.  The bounded host has
            // no safe way to validate a custom duplicate effect matrix, so
            // retain it but do not make the container physically enterable.
            // Check every retained row, not just the physical subset: an
            // unsupported earlier/later duplicate still changes C++'s
            // declaration-order selection and cannot be ignored safely.
            && self.rider_change_riders.iter().enumerate().all(|(index, rider)| {
                self.rider_change_riders
                    .iter()
                    .skip(index + 1)
                    .all(|other| !rider.template_name.eq_ignore_ascii_case(&other.template_name))
            })
    }

    #[inline]
    pub fn supported_rider_change_rider_for_template(
        &self,
        template_name: &str,
    ) -> Option<&RiderChangeRiderMetadata> {
        self.rider_change_riders.iter().find(|rider| {
            rider.physical_enter_supported
                && rider.active_locomotor_name.is_some()
                && rider.template_matches(template_name)
        })
    }

    /// Leftover `OpenContain::is_valid_container_for` KindOf mask algebra.
    /// C++ `isAnyKindOf(allow) == FALSE || isAnyKindOf(forbid) == TRUE`.
    #[inline]
    pub fn leftover_kind_masks_admit(&self, obj_kind: u128) -> bool {
        if self.allow_inside_kind_of != 0 && (obj_kind & self.allow_inside_kind_of) == 0 {
            return false;
        }
        if (obj_kind & self.forbid_inside_kind_of) != 0 {
            return false;
        }
        true
    }
}

/// Exact `ParkingPlaceBehavior` module data retained from an Object INI.
///
/// C++ keeps one runtime `ParkingPlaceInfo` for every `NumRows × NumCols`
/// entry, with its own reservation and exit-door state.  The Main host keeps
/// that mutable reservation state separately on `GameLogic`; this immutable
/// record is only the authored shape and flight/healing parameters needed to
/// create and validate those spaces.  `None` on [`ThingTemplate`] means no
/// `ParkingPlaceBehavior` was authored — an `FSAirfield` KindOf alone is not
/// enough to admit an aircraft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParkingPlaceMetadata {
    /// C++ `ParkingPlaceBehaviorModuleData::m_numRows`.
    pub num_rows: i32,
    /// C++ `ParkingPlaceBehaviorModuleData::m_numCols`.
    pub num_cols: i32,
    /// C++ `ParkingPlaceBehaviorModuleData::m_approachHeight`.
    pub approach_height: f32,
    /// C++ `ParkingPlaceBehaviorModuleData::m_landingDeckHeightOffset`.
    pub landing_deck_height_offset: f32,
    /// C++ `ParkingPlaceBehaviorModuleData::m_hasRunways`.
    pub has_runways: bool,
    /// C++ `ParkingPlaceBehaviorModuleData::m_parkInHangars`.
    pub park_in_hangars: bool,
    /// C++ `ParkingPlaceBehaviorModuleData::m_healAmount`.
    pub heal_amount_per_second: f32,
}

impl ParkingPlaceMetadata {
    /// Number of real reservation records created by C++ `buildInfo`.
    ///
    /// A malformed negative count, multiplication overflow, or non-finite
    /// physical parameter cannot be represented faithfully by the bounded
    /// Main path, so callers fail closed instead of inventing a generic
    /// airfield capacity.
    #[inline]
    pub fn capacity(&self) -> Option<usize> {
        if !self.is_well_formed() {
            return None;
        }
        let rows = usize::try_from(self.num_rows).ok()?;
        let cols = usize::try_from(self.num_cols).ok()?;
        rows.checked_mul(cols)
    }

    /// C++ creates one runway for each column only when `HasRunways` is set.
    #[inline]
    pub fn runway_count(&self) -> Option<usize> {
        if !self.is_well_formed() {
            return None;
        }
        if self.has_runways {
            usize::try_from(self.num_cols).ok()
        } else {
            Some(0)
        }
    }

    #[inline]
    pub fn is_well_formed(&self) -> bool {
        self.num_rows >= 0
            && self.num_cols >= 0
            && self.approach_height.is_finite()
            && self.landing_deck_height_offset.is_finite()
            && self.heal_amount_per_second.is_finite()
    }
}

/// Exact `FlightDeckBehavior` module data retained from an Object INI.
///
/// C++ `FlightDeckBehavior::buildInfo` creates `NumSpacesPerRunway × NumRunways`
/// stalls and payload-spawns `PayloadTemplate` jets.  `None` on
/// [`ThingTemplate`] means no `FlightDeckBehavior` was authored — a carrier
/// KindOf or template basename never fabricates a deck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlightDeckMetadata {
    /// C++ `m_thingTemplateName` (`PayloadTemplate`).
    pub payload_template: String,
    /// C++ `m_numRows` (`NumSpacesPerRunway`).
    pub num_rows: i32,
    /// C++ `m_numCols` (`NumRunways`).
    pub num_cols: i32,
    /// C++ `m_approachHeight`.
    pub approach_height: f32,
    /// C++ `m_landingDeckHeightOffset`.
    pub landing_deck_height_offset: f32,
    /// C++ `m_healAmount`.
    pub heal_amount_per_second: f32,
    /// C++ `m_cleanupFrames` (`ParkingCleanupPeriod`).
    pub cleanup_frames: u32,
    /// C++ `m_humanFollowFrames` (`HumanFollowPeriod`).
    pub human_follow_frames: u32,
    /// C++ `m_replacementFrames` (`ReplacementDelay`).
    pub replacement_frames: u32,
    /// C++ `m_dockAnimationFrames` (`DockAnimationDelay`).
    pub dock_animation_frames: u32,
    /// C++ `m_launchWaveFrames` (`LaunchWaveDelay`).
    pub launch_wave_frames: u32,
    /// C++ `m_launchRampFrames` (`LaunchRampDelay`).
    pub launch_ramp_frames: u32,
    /// C++ `m_lowerRampFrames` (`LowerRampDelay`).
    pub lower_ramp_frames: u32,
    /// C++ `m_catapultFireFrames` (`CatapultFireDelay`).
    pub catapult_fire_frames: u32,
    /// C++ `RunwayNCatapultSystem` names (index 0/1).
    pub catapult_system: [Option<String>; 2],
}

impl FlightDeckMetadata {
    #[inline]
    pub fn capacity(&self) -> Option<usize> {
        if !self.is_well_formed() {
            return None;
        }
        let rows = usize::try_from(self.num_rows).ok()?;
        let cols = usize::try_from(self.num_cols).ok()?;
        rows.checked_mul(cols)
    }

    #[inline]
    pub fn is_well_formed(&self) -> bool {
        self.num_rows >= 0
            && self.num_cols >= 0
            && self.num_cols <= 2
            && self.approach_height.is_finite()
            && self.landing_deck_height_offset.is_finite()
            && self.heal_amount_per_second.is_finite()
    }
}

/// Exact `DeployStyleAIUpdateModuleData` retained from one Object INI
/// `Behavior = DeployStyleAIUpdate` declaration.
///
/// C++ parses `PackTime` and `UnpackTime` with
/// `INI::parseDurationUnsignedInt`, so these values are logic frames rather
/// than source milliseconds.  Keeping the post-parser representation matches
/// the C++ module data that is serialized with an Object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployStyleMetadata {
    /// C++ `m_packTime`, in 30 Hz logic frames.
    pub pack_time_frames: u32,
    /// C++ `m_unpackTime`, in 30 Hz logic frames.
    pub unpack_time_frames: u32,
    /// C++ `m_resetTurretBeforePacking`.  Retained for snapshot parity; no
    /// guessed turret reset is performed by the bounded host state machine.
    pub reset_turret_before_packing: bool,
    /// C++ `m_turretsFunctionOnlyWhenDeployed`.  Retained separately from
    /// generic weapon availability so a missing per-turret mapping cannot
    /// silently disable a unit's non-turret weapon.
    pub turrets_function_only_when_deployed: bool,
    /// C++ `m_turretsMustCenterBeforePacking`. Host DeployStyle waits in
    /// `AligningTurrets` until `isTurretInNaturalPosition` before packing.
    pub turrets_must_center_before_packing: bool,
    /// C++ `m_manualDeployAnimations`.  The logic state is retained, but the
    /// renderer must not fabricate a manual animation-frame scrub from this
    /// boolean alone.
    pub manual_deploy_animations: bool,
}

impl Default for DeployStyleMetadata {
    fn default() -> Self {
        // Matches DeployStyleAIUpdateModuleData's constructor defaults.
        Self {
            pack_time_frames: 0,
            unpack_time_frames: 0,
            reset_turret_before_packing: false,
            turrets_function_only_when_deployed: false,
            turrets_must_center_before_packing: false,
            manual_deploy_animations: false,
        }
    }
}

/// Authored `SupplyTruckAIUpdate` / `ChinookAIUpdate` / `WorkerAIUpdate`
/// timing, capacity, and INI `UpgradedSupplyBoost`. Ordinary Harvesters
/// without one of those modules stay `None`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SupplyTruckMetadata {
    pub max_boxes: u32,
    pub warehouse_scan_distance: f32,
    pub warehouse_delay_frames: u32,
    pub center_delay_frames: u32,
    /// C++ `ChinookAIUpdateModuleData::m_upgradedSupplyBoost` /
    /// `WorkerAIUpdateModuleData::m_upgradedSupplyBoost`. Supply trucks
    /// author 0 (`SupplyTruckAIUpdate::getUpgradedSupplyBoost`).
    #[serde(default)]
    pub upgraded_supply_boost: u32,
}

/// Compact runtime mirror of the C++ supply-truck Wanting/dock state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SupplyTruckState {
    #[default]
    Idle,
    Wanting,
    DockingWarehouse,
    DockingCenter,
    /// C++ `ST_REGROUPING` — wanting failed, hang out at base.
    Regrouping,
}

/// The production-exit interfaces carried by the bounded live producer
/// path.  This is deliberately not inferred from a building kind or basename:
/// C++ `Object::getObjectExitInterface` exposes an interface only when an
/// Object INI behavior authors one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductionExitStyle {
    /// `QueueProductionExitUpdate`: a successful exit arms a delay, while the
    /// authored initial burst can keep the interface immediately available.
    Queue,
    /// `DefaultProductionExitUpdate`: every completed batch member can use
    /// `DOOR_1` in the same ProductionUpdate.
    Default,
    /// `SupplyCenterProductionExitUpdate`: exits through the authored path,
    /// then hands an eligible supply truck to its ForceWanting autopilot.
    SupplyCenter,
}

/// Exact immutable module data from either one
/// `QueueProductionExitUpdate` or `DefaultProductionExitUpdate` declaration.
///
/// The corresponding mutable Queue counters live on `BuildingData`, just as
/// the C++ update module owns `m_currentDelay` and `m_currentBurstCount` per
/// Object instance rather than per ThingTemplate.  `None` on
/// [`ThingTemplate`] remains distinct from a module with all-default fields.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProductionExitMetadata {
    /// Which source ExitInterface owns the production exit.
    pub style: ProductionExitStyle,
    /// C++ `m_unitCreatePoint`, in model-space X/Y/Z coordinates.
    pub unit_create_point: [f32; 3],
    /// C++ `m_naturalRallyPoint`, in model-space X/Y/Z coordinates.
    pub natural_rally_point: [f32; 3],
    /// C++ Queue `m_exitDelayData`, parsed to 30 Hz logic frames.  Default
    /// exit modules have no delay data and retain zero here.
    pub exit_delay_frames: u32,
    /// C++ Queue `m_allowAirborneCreationData`.  Keeps transformed spawn Y;
    /// the airborne motive/pitch kick is gated on pre-snap Y != ground, not this bit.
    pub allow_airborne_creation: bool,
    /// C++ Queue `m_initialBurst`.  The runtime counter is initialized once
    /// per producer Object from this template value.
    pub initial_burst: u32,
    /// C++ Default `m_useSpawnRallyPoint`.  This is retained for the separate
    /// spawn/parachute path; ordinary unit production always follows its
    /// authored natural/custom exit route.
    pub use_spawn_rally_point: bool,
    /// C++ SupplyCenter production-exit temporary stealth grant frames.
    #[serde(default)]
    pub grant_temporary_stealth_frames: u32,
}

impl ProductionExitMetadata {
    #[inline]
    pub const fn is_queue(self) -> bool {
        matches!(self.style, ProductionExitStyle::Queue)
    }

    #[inline]
    pub const fn is_default(self) -> bool {
        matches!(self.style, ProductionExitStyle::Default)
    }

    #[inline]
    pub const fn is_supply_center(self) -> bool {
        matches!(self.style, ProductionExitStyle::SupplyCenter)
    }

    /// `getNaturalRallyPoint(offset = TRUE)` adds two pathfinding cells along
    /// the authored model-space rally vector before the producer transform.
    /// A zero authored vector remains zero rather than acquiring an arbitrary
    /// direction.
    #[inline]
    pub fn natural_rally_point_with_path_offset(self, pathfind_cell_size: f32) -> [f32; 3] {
        let [x, y, z] = self.natural_rally_point;
        let length = (x * x + y * y + z * z).sqrt();
        if !length.is_finite() || length <= f32::EPSILON || !pathfind_cell_size.is_finite() {
            return [x, y, z];
        }
        let distance = 2.0 * pathfind_cell_size;
        [
            x + x / length * distance,
            y + y / length * distance,
            z + z / length * distance,
        ]
    }
}

/// The narrow `VeterancyCrateCollide` data slice that makes an infantry
/// object a USA Pilot re-crew source.
///
/// This is intentionally not a generic crate/experience implementation.
/// C++ `VeterancyCrateCollide` is used for many unrelated crate effects; the
/// host retains only an explicitly authored `IsPilot = Yes` module and the
/// companion `VeterancyGainCreate::StartingLevel` needed by the live pilot
/// path.  Missing or unrepresentable fields do not authorize re-crew.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VeterancyCrateCollideMetadata {
    /// Exact authored `IsPilot = Yes` marker.  This stays explicit instead of
    /// inferring pilot behavior from a template basename.
    pub is_pilot: bool,
    /// The compact host can faithfully service the retail `RequiredKindOf =
    /// VEHICLE` pilot criterion only when it was explicitly parsed.
    pub required_kind_of_vehicle: bool,
    /// The compact host can faithfully service the retail `ForbiddenKindOf =
    /// DOZER` pilot criterion only when it was explicitly parsed.
    pub forbidden_kind_of_dozer: bool,
    /// C++ `m_rangeOfEffect`.  Re-crew is a collide/Enter action only when
    /// this is exactly zero; `None` records an absent or malformed source
    /// field and fails closed.
    pub effect_range: Option<f32>,
    /// C++ `AddsOwnerVeterancy`.  The live path only carries the pilot's
    /// veterancy into the vehicle when this authored field is true.
    pub adds_owner_veterancy: bool,
    /// The `StartingLevel` from the one companion `VeterancyGainCreate`
    /// module.  It is retained only under parsed `IsPilot`, not as a generic
    /// free-veterancy fallback.
    pub starting_level: Option<VeterancyLevel>,
}

impl VeterancyCrateCollideMetadata {
    /// Whether this exact behavior is representable by the bounded physical
    /// USA Pilot re-crew path.  Do not broaden this to other crate masks:
    /// unknown/missing source fields must never grant an Enter action.
    #[inline]
    pub fn supports_pilot_recrew(&self) -> bool {
        self.is_pilot
            && self.required_kind_of_vehicle
            && self.forbidden_kind_of_dozer
            && self.effect_range == Some(0.0)
            && self.adds_owner_veterancy
    }

    /// `VeterancyGainCreate` is a separate C++ module, but the starting level
    /// is only applied by this host path when an explicit pilot module was
    /// parsed from the same template.
    #[inline]
    pub fn pilot_starting_level(&self) -> Option<VeterancyLevel> {
        self.is_pilot.then_some(self.starting_level).flatten()
    }
}

/// C++ `VeterancyGainCreateModuleData` — StartingLevel + optional ScienceRequired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VeterancyGainCreateMetadata {
    pub starting_level: VeterancyLevel,
    /// `None` means SCIENCE_INVALID (always apply when trainable).
    pub science_required: Option<String>,
}

/// C++ `GrantUpgradeCreateModuleData` — UpgradeToGrant + ExemptStatus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantUpgradeCreateMetadata {
    pub upgrade_name: String,
    /// True when INI `ExemptStatus` includes UNDER_CONSTRUCTION.
    pub exempt_under_construction: bool,
}

/// The two retail ObjectCreationLists used by `EjectPilotDie`.
///
/// C++ retains pointers to arbitrary OCLs.  The compact live bridge only
/// implements these two fully understood retail lists; an absent value is
/// therefore intentionally a no-spawn result, whether the source pointer was
/// null or named an unsupported list.  The enclosing metadata still records
/// the EjectPilotDie *interface* for the separate Hijacker path.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EjectPilotCreationList {
    OnGround = 0,
    ViaParachute = 1,
}

/// Exact subset of C++ `DieMuxData::m_deathTypes` represented by active
/// retail `EjectPilotDie` behaviors.  Unsupported masks never authorize a
/// host death spawn.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EjectPilotDeathTypes {
    All = 0,
    AllExceptCrushedAndSplatted = 1,
    Unsupported = 255,
}

/// Exact subset of C++ `DieMuxData::m_veterancyLevels` represented by active
/// retail `EjectPilotDie` behaviors.  `Regular` is Rust's `Rookie` rank.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EjectPilotVeterancyLevels {
    All = 0,
    AllExceptRegular = 1,
    Unsupported = 255,
}

/// Exact `DieMuxData::m_exemptStatus` cases retained for EjectPilotDie.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EjectPilotExemptStatus {
    None = 0,
    Hijacked = 1,
    Unsupported = 255,
}

/// Exact `DieMuxData::m_requiredStatus` cases retained for EjectPilotDie.
/// No retail EjectPilotDie block authors a required status; an unfamiliar
/// requirement must not be guessed by the compact death path.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EjectPilotRequiredStatus {
    None = 0,
    Unsupported = 255,
}

/// Typed C++ `EjectPilotDieModuleData` retained from one Object INI Behavior.
///
/// The presence of this value is the source-backed
/// `getEjectPilotDieInterface()` fact used by
/// `ConvertToHijackedVehicleCrateCollide`.  Death spawning is intentionally
/// stricter: it requires a representable DieMux filter and an exact OCL for
/// the selected ground/air branch.  That separation preserves C++'s interface
/// predicate without inventing an OCL action for unknown data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EjectPilotDieMetadata {
    /// C++ `m_oclOnGround`; `None` retains a null or unsupported OCL pointer.
    pub ground_creation_list: Option<EjectPilotCreationList>,
    /// C++ `m_oclInAir`; `None` retains a null or unsupported OCL pointer.
    pub air_creation_list: Option<EjectPilotCreationList>,
    /// C++ `m_invulnerableTime`, in source milliseconds.  It defaults to
    /// zero and is retained even though the retail ejection OCL owns the
    /// actual spawned pilot's InvulnerableTime.
    pub invulnerable_time_ms: Option<u32>,
    pub death_types: EjectPilotDeathTypes,
    pub veterancy_levels: EjectPilotVeterancyLevels,
    pub exempt_status: EjectPilotExemptStatus,
    pub required_status: EjectPilotRequiredStatus,
}

impl Default for EjectPilotDieMetadata {
    fn default() -> Self {
        // Exact EjectPilotDieModuleData / DieMuxData constructor defaults.
        Self {
            ground_creation_list: None,
            air_creation_list: None,
            invulnerable_time_ms: Some(0),
            death_types: EjectPilotDeathTypes::All,
            veterancy_levels: EjectPilotVeterancyLevels::All,
            exempt_status: EjectPilotExemptStatus::None,
            required_status: EjectPilotRequiredStatus::None,
        }
    }
}

impl EjectPilotDieMetadata {
    /// `getEjectPilotDieInterface()` is exposed solely by module presence in
    /// C++; OCL availability and DieMux applicability are not part of that
    /// query.  A parsed metadata value therefore always carries the interface.
    #[inline]
    pub const fn has_eject_pilot_die_interface(&self) -> bool {
        true
    }

    /// Return the exact OCL selected by C++ `EjectPilotDie::onDie` for the
    /// already-evaluated `isSignificantlyAboveTerrain` result.
    #[inline]
    pub const fn creation_list_for_air_path(
        &self,
        significantly_above_terrain: bool,
    ) -> Option<EjectPilotCreationList> {
        if significantly_above_terrain {
            self.air_creation_list
        } else {
            self.ground_creation_list
        }
    }

    /// Evaluate the supported portion of C++ `DieMuxData::isDieApplicable`.
    /// Unknown filters and malformed duration input stay fail-closed for the
    /// physical spawn, while the separate interface predicate remains valid.
    #[inline]
    pub fn allows_supported_death(
        &self,
        death_is_crushed_or_splatted: bool,
        veterancy_is_regular: bool,
        is_hijacked: bool,
    ) -> bool {
        self.invulnerable_time_ms.is_some()
            && matches!(self.required_status, EjectPilotRequiredStatus::None)
            && match self.death_types {
                EjectPilotDeathTypes::All => true,
                EjectPilotDeathTypes::AllExceptCrushedAndSplatted => !death_is_crushed_or_splatted,
                EjectPilotDeathTypes::Unsupported => false,
            }
            && match self.veterancy_levels {
                EjectPilotVeterancyLevels::All => true,
                EjectPilotVeterancyLevels::AllExceptRegular => !veterancy_is_regular,
                EjectPilotVeterancyLevels::Unsupported => false,
            }
            && match self.exempt_status {
                EjectPilotExemptStatus::None => true,
                EjectPilotExemptStatus::Hijacked => !is_hijacked,
                EjectPilotExemptStatus::Unsupported => false,
            }
    }
}

/// Exact `RebuildHoleExposeDie` module data. Presence is the C++ die
/// interface; a template name or GLA KindOf never fabricates a hole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RebuildHoleExposeDieMetadata {
    /// C++ `m_holeName` (`HoleName`).
    pub hole_name: String,
    /// C++ `m_holeMaxHealth` (`HoleMaxHealth`). Constructor default 0.
    pub hole_max_health: f32,
    /// C++ `m_transferAttackers` (`TransferAttackers`). Default true.
    pub transfer_attackers: bool,
}

impl Default for RebuildHoleExposeDieMetadata {
    fn default() -> Self {
        Self {
            hole_name: String::new(),
            hole_max_health: 0.0,
            transfer_attackers: true,
        }
    }
}

impl RebuildHoleExposeDieMetadata {
    pub fn authored(hole_name: impl Into<String>, hole_max_health: f32) -> Self {
        Self {
            hole_name: hole_name.into(),
            hole_max_health,
            transfer_attackers: true,
        }
    }
}

/// Exact `HackInternetAIUpdateModuleData` fields retained from Object INI.
///
/// The host uses this only for the currently implemented cash scheduler.  It
/// retains `PackTime`, `UnpackTime`, and variation as source data, but does
/// not fabricate the unported packing/model-condition state machine.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HackInternetAIUpdateMetadata {
    /// C++ `m_unpackTime`, converted by `INI::parseDurationUnsignedInt`.
    pub unpack_time_frames: u32,
    /// C++ `m_packTime`, converted by `INI::parseDurationUnsignedInt`.
    pub pack_time_frames: u32,
    /// C++ `m_cashUpdateDelay`, in logic frames.
    pub cash_update_delay_frames: u32,
    /// C++ `m_cashUpdateDelayFast`, in logic frames while contained.
    pub cash_update_delay_fast_frames: u32,
    pub regular_cash_amount: u32,
    pub veteran_cash_amount: u32,
    pub elite_cash_amount: u32,
    pub heroic_cash_amount: u32,
    pub xp_per_cash_update: f32,
    pub pack_unpack_variation_factor: f32,
}

impl HackInternetAIUpdateMetadata {
    /// C++ `HackInternetState::update` falls through to lower tiers when a
    /// higher authored amount is zero, finally yielding one credit.  That
    /// fallback applies to every successfully parsed `HackInternetAIUpdate`,
    /// including an all-zero module.  Absent or malformed modules are `None`
    /// on `ThingTemplate` and fail closed at their callers.
    #[inline]
    pub const fn cash_amount_for_level(&self, level: VeterancyLevel) -> u32 {
        let amount = match level {
            VeterancyLevel::Heroic if self.heroic_cash_amount != 0 => self.heroic_cash_amount,
            VeterancyLevel::Heroic | VeterancyLevel::Elite if self.elite_cash_amount != 0 => {
                self.elite_cash_amount
            }
            VeterancyLevel::Heroic | VeterancyLevel::Elite | VeterancyLevel::Veteran
                if self.veteran_cash_amount != 0 =>
            {
                self.veteran_cash_amount
            }
            _ if self.regular_cash_amount != 0 => self.regular_cash_amount,
            _ => 1,
        };
        amount
    }

    #[inline]
    pub const fn cash_update_delay_frames(&self, contained: bool) -> u32 {
        if contained {
            self.cash_update_delay_fast_frames
        } else {
            self.cash_update_delay_frames
        }
    }
}

/// Exact paired `SpecialAbility` + `SpecialAbilityUpdate` data for C++
/// `SPECIAL_HACKER_DISABLE_BUILDING`.
///
/// The parser exposes this only after both modules name the same, loaded
/// SpecialPower template.  This is intentionally not inferred from Hacker,
/// China, Infantry, or a CommandButton name: C++ ActionManager asks the
/// source object's SpecialPowerModule and its matching update module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HackerDisableBuildingMetadata {
    /// Source `SpecialAbility::SpecialPowerTemplate`, retained so snapshots
    /// describe the exact paired source rather than a generic enum alone.
    pub special_power_template: String,
    /// `SpecialAbility::UpdateModuleStartsAttack`; HDB's physical channel is
    /// unsupported unless the source explicitly delegates to the update.
    pub update_module_starts_attack: bool,
    /// `SpecialAbility::StartsPaused` participates in readiness just like a
    /// C++ SpecialPowerModule pause count.
    pub starts_paused: bool,
    /// Script-only abilities cannot be armed through the player command path.
    pub scripted_special_power_only: bool,
    /// `SpecialPowerTemplate::ReloadTime`, already converted to logic frames
    /// by the Common SpecialPower parser.
    pub reload_time_frames: u32,
    /// Resolved `SpecialPowerTemplate::RequiredScience`; `None` means the
    /// C++ `SCIENCE_INVALID` default, not an inferred faction prerequisite.
    pub required_science: Option<String>,
    /// `SpecialPowerTemplate::SharedSyncedTimer`.
    pub shared_n_sync: bool,
    /// `SpecialAbilityUpdate::StartAbilityRange`.
    pub start_ability_range: f32,
    /// `SpecialAbilityUpdate::AbilityAbortRange`.
    pub ability_abort_range: f32,
    /// `SpecialAbilityUpdate::ApproachRequiresLOS`; omitted modules retain
    /// the C++ module-data default of `Yes`.
    pub approach_requires_los: bool,
    /// C++ timing fields are retained in milliseconds because host channels
    /// integrate in seconds and must not round a source duration at parse.
    pub unpack_time_ms: u32,
    pub preparation_time_ms: u32,
    pub persistent_prep_time_ms: u32,
    pub effect_duration_ms: u32,
    pub pack_time_ms: u32,
    /// `SpecialAbilityUpdate::PackUnpackVariationFactor`.
    #[serde(default)]
    pub pack_unpack_variation_factor: f32,
    /// `SpecialAbilityUpdate::PersistenceRequiresRecharge`.
    pub persistence_requires_recharge: bool,
}

impl HackerDisableBuildingMetadata {
    /// Host command enum for this paired source. C++ has no distinct
    /// Microwave SpecialPowerType; both templates are
    /// `SPECIAL_HACKER_DISABLE_BUILDING`, but the live command adapter
    /// keeps the authored template identity for charge keys and buttons.
    pub fn command_power(&self) -> crate::command_system::SpecialPowerType {
        crate::command_system::special_power_type_from_template_name(&self.special_power_template)
            .unwrap_or(crate::command_system::SpecialPowerType::HackerDisableBuilding)
    }

    /// `Command_HackerDisableBuilding` is only the Hacker identity.
    /// Microwave keeps its own SpecialPower button.
    pub fn is_hacker_command(&self) -> bool {
        matches!(
            self.command_power(),
            crate::command_system::SpecialPowerType::HackerDisableBuilding
        )
    }
}

/// Exact `SpecialAbilityUpdate` data for Burton C4 / Tank Hunter TNT plants.
///
/// C++ `SpecialAbilityUpdate::startUnpacking` then `triggerAbilityEffect`
/// then `finishAbility` (`SpecialAbilityUpdate.cpp:770-794`, `1733-1818`).
/// Missing metadata fails closed to instant plant with no flee.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChargePlantAbilityMetadata {
    pub special_power_template: String,
    /// `SpecialAbilityUpdate::UnpackTime` milliseconds.
    pub unpack_time_ms: u32,
    /// `SpecialAbilityUpdate::PackTime` milliseconds.
    pub pack_time_ms: u32,
    /// `SpecialAbilityUpdate::PackUnpackVariationFactor`.
    pub pack_unpack_variation_factor: f32,
    /// `SpecialAbilityUpdate::FleeRangeAfterCompletion`.
    pub flee_range_after_completion: f32,
    /// `SpecialAbilityUpdate::FlipOwnerAfterUnpacking`.
    pub flip_object_after_unpacking: bool,
    /// `SpecialAbilityUpdate::FlipOwnerAfterPacking`.
    pub flip_object_after_packing: bool,
}

impl ChargePlantAbilityMetadata {
    pub fn is_timed_charge_power(&self) -> bool {
        let name = self.special_power_template.to_ascii_lowercase();
        name.contains("timedcharges") || name.contains("tntattack")
    }

    pub fn is_remote_charge_power(&self) -> bool {
        self.special_power_template
            .to_ascii_lowercase()
            .contains("remotecharges")
    }
}

/// C++ `SpecialAbilityUpdate.cpp:721` / `:774`:
/// `m_animFrames = time * GameLogicRandomValueReal(1-factor, 1+factor)`.
/// `unit_sample` is 0..1 along that inclusive range (0 → 1-factor).
pub fn pack_unpack_variation_multiplier(factor: f32, unit_sample: f32) -> f32 {
    let factor = if factor.is_finite() {
        factor.max(0.0)
    } else {
        0.0
    };
    let sample = if unit_sample.is_finite() {
        unit_sample.clamp(0.0, 1.0)
    } else {
        0.5
    };
    (1.0 - factor) + (2.0 * factor * sample)
}

/// Apply a C++ pack/unpack variation multiplier to a millisecond duration.
/// Unsigned conversion truncates toward zero like C++ `UnsignedInt` assign.
pub fn apply_pack_unpack_variation_ms(base_ms: u32, variation: f32) -> u32 {
    if base_ms == 0 {
        return 0;
    }
    let variation = if variation.is_finite() {
        variation.max(0.0)
    } else {
        1.0
    };
    (base_ms as f32 * variation) as u32
}

/// Live-path pack/unpack duration. Factor 0 is deterministic (C++ range is 1..1).
pub fn vary_pack_unpack_duration_ms(base_ms: u32, factor: f32) -> u32 {
    if base_ms == 0 {
        return 0;
    }
    let factor = if factor.is_finite() {
        factor.max(0.0)
    } else {
        0.0
    };
    let variation = if factor <= 0.0 {
        1.0
    } else {
        game_engine::common::random_value::get_game_logic_random_value_real(
            1.0 - factor,
            1.0 + factor,
        )
    };
    apply_pack_unpack_variation_ms(base_ms, variation)
}

/// The concrete C++ `SpecialPowerModule` subclass that owns a parsed
/// `SpecialPowerTemplate`.  The module identity stays distinct from the
/// template's Common enum: retail templates such as Hacker Disable and
/// Microwave deliberately share enum values while their module behavior is
/// different.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialPowerModuleKind {
    SpecialAbility = 0,
    OclSpecialPower = 1,
    BaikonurLaunchPower = 2,
    CashBountyPower = 3,
    CashHackSpecialPower = 4,
    CleanupAreaPower = 5,
    DefectorSpecialPower = 6,
    DemoralizeSpecialPower = 7,
    FireWeaponPower = 8,
    SpyVisionSpecialPower = 9,
}

impl SpecialPowerModuleKind {
    /// C++ subclasses that implement the `SpecialPowerModuleInterface`.
    /// Completion/update modules are intentionally absent: a
    /// `SpecialPowerCompletionDie` naming the same template must never grant
    /// player ability authority.
    pub fn from_behavior_class_name(class_name: &str) -> Option<Self> {
        if class_name.eq_ignore_ascii_case("SpecialAbility") {
            Some(Self::SpecialAbility)
        } else if class_name.eq_ignore_ascii_case("OCLSpecialPower") {
            Some(Self::OclSpecialPower)
        } else if class_name.eq_ignore_ascii_case("BaikonurLaunchPower") {
            Some(Self::BaikonurLaunchPower)
        } else if class_name.eq_ignore_ascii_case("CashBountyPower") {
            Some(Self::CashBountyPower)
        } else if class_name.eq_ignore_ascii_case("CashHackSpecialPower") {
            Some(Self::CashHackSpecialPower)
        } else if class_name.eq_ignore_ascii_case("CleanupAreaPower") {
            Some(Self::CleanupAreaPower)
        } else if class_name.eq_ignore_ascii_case("DefectorSpecialPower") {
            Some(Self::DefectorSpecialPower)
        } else if class_name.eq_ignore_ascii_case("DemoralizeSpecialPower") {
            Some(Self::DemoralizeSpecialPower)
        } else if class_name.eq_ignore_ascii_case("FireWeaponPower") {
            Some(Self::FireWeaponPower)
        } else if class_name.eq_ignore_ascii_case("SpyVisionSpecialPower") {
            Some(Self::SpyVisionSpecialPower)
        } else {
            None
        }
    }
}

/// One source-ordered C++ `SpecialPowerModule` interface.
///
/// `Object::getSpecialPowerModule` compares a loaded `SpecialPowerTemplate`
/// pointer while walking behavior modules.  Retaining both the canonical name
/// and parsed ID prevents a host command enum or an object basename from
/// becoming the authority boundary.  More than one module is legal; callers
/// preserve this order and select the first exact match like C++.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecialPowerModuleMetadata {
    /// Declaration-order index in the Object INI Behavior list.
    pub source_index: u32,
    /// Optional `ModuleTag_*` source identity.
    pub module_tag: Option<String>,
    pub module_kind: SpecialPowerModuleKind,
    /// Canonical loaded `SpecialPowerTemplate::m_name`.
    pub special_power_template: String,
    /// Stable loaded `SpecialPowerTemplate::m_id`.
    pub special_power_template_id: u32,
    /// Main command adapter only.  `None` remains a valid parsed module but
    /// cannot be driven by an unported command implementation.
    pub command_power: Option<crate::command_system::SpecialPowerType>,
    pub reload_time_frames: u32,
    /// Canonical `RequiredScience`; `None` is C++ `SCIENCE_INVALID`.
    pub required_science: Option<String>,
    pub public_timer: bool,
    pub shared_n_sync: bool,
    pub shortcut_power: bool,
    /// `SpecialAbility` flags.  Other subclasses retain C++ defaults.
    pub update_module_starts_attack: bool,
    pub starts_paused: bool,
    pub scripted_special_power_only: bool,
}

/// Exact capture special carried by an Object INI `SpecialAbility` module.
///
/// This remains separate from `KindOf::Infantry` and template spelling: C++
/// `ActionManager::canCaptureBuilding` asks whether the source owns one of
/// these SpecialPower modules and whether that module is ready.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapturePowerKind {
    None = 0,
    Ranger = 1,
    RedGuard = 2,
    Rebel = 3,
    BlackLotus = 4,
}

impl Default for CapturePowerKind {
    fn default() -> Self {
        Self::None
    }
}

impl CapturePowerKind {
    #[inline]
    pub const fn from_ordinal(value: u8) -> Self {
        match value {
            1 => Self::Ranger,
            2 => Self::RedGuard,
            3 => Self::Rebel,
            4 => Self::BlackLotus,
            _ => Self::None,
        }
    }

    /// Resolve only the four retail capture SpecialPower templates.  The
    /// normalized key tolerates Object INI case/separator differences without
    /// widening acceptance to a name heuristic.
    pub fn from_special_power_template(name: &str) -> Self {
        let key: String = name
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .map(|character| character.to_ascii_lowercase())
            .collect();
        match key.as_str() {
            "specialabilityrangercapturebuilding" => Self::Ranger,
            "specialabilityredguardcapturebuilding" => Self::RedGuard,
            "specialabilityrebelcapturebuilding" => Self::Rebel,
            "specialabilityblacklotuscapturebuilding" => Self::BlackLotus,
            _ => Self::None,
        }
    }

    pub const fn special_power_type(self) -> Option<crate::command_system::SpecialPowerType> {
        use crate::command_system::SpecialPowerType;
        match self {
            Self::Ranger => Some(SpecialPowerType::RangerCaptureBuilding),
            Self::RedGuard => Some(SpecialPowerType::RedGuardCaptureBuilding),
            Self::Rebel => Some(SpecialPowerType::RebelCaptureBuilding),
            Self::BlackLotus => Some(SpecialPowerType::BlackLotusCaptureBuilding),
            Self::None => None,
        }
    }

    pub const fn from_special_power_type(power: &crate::command_system::SpecialPowerType) -> Self {
        use crate::command_system::SpecialPowerType;
        match power {
            SpecialPowerType::RangerCaptureBuilding => Self::Ranger,
            SpecialPowerType::RedGuardCaptureBuilding => Self::RedGuard,
            SpecialPowerType::RebelCaptureBuilding => Self::Rebel,
            SpecialPowerType::BlackLotusCaptureBuilding => Self::BlackLotus,
            _ => Self::None,
        }
    }
}

/// C++ `ArmorTemplateSet` residual: one Object INI `ArmorSet` row.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct HostArmorSet {
    /// C++ `ArmorSetFlags` mask (`ArmorSetType` bits).
    #[serde(default)]
    pub conditions: u8,
    /// Armor.ini template name (`Armor = ...`).
    #[serde(default)]
    pub armor: Option<String>,
    /// DamageFX.ini name (`DamageFX = ...`).
    #[serde(default)]
    pub damage_fx: Option<String>,
}

/// C++ `GeometryType` (`Geometry.h:25-33`). SPHERE=0, CYLINDER=1, BOX=2.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HostGeometryType {
    #[default]
    Sphere = 0,
    Cylinder = 1,
    Box = 2,
}

impl HostGeometryType {
    /// C++ `GeometryNames[]` / `INI::scanIndexList` (`Geometry.cpp:26-29`).
    pub fn from_ini(token: &str) -> Option<Self> {
        match token.trim() {
            t if t.eq_ignore_ascii_case("SPHERE") => Some(Self::Sphere),
            t if t.eq_ignore_ascii_case("CYLINDER") => Some(Self::Cylinder),
            t if t.eq_ignore_ascii_case("BOX") => Some(Self::Box),
            _ => None,
        }
    }
}

/// C++ `ThingTemplate::m_geometryInfo` (`ThingTemplate.cpp:966`, `Geometry.cpp:26-88`).
///
/// INI parse writes each field independently (no `set()` copy of sphere/cylinder
/// radii). Constructor default is SPHERE / not-small / 1 / 1 / 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HostGeometryInfo {
    pub geom_type: HostGeometryType,
    pub is_small: bool,
    pub height: f32,
    pub major_radius: f32,
    pub minor_radius: f32,
    /// True when any Object INI `Geometry*` field was parsed onto this template.
    #[serde(default)]
    pub authored: bool,
}

impl Default for HostGeometryInfo {
    fn default() -> Self {
        // C++ `ThingTemplate::ThingTemplate()`: `m_geometryInfo(GEOMETRY_SPHERE, FALSE, 1, 1, 1)`.
        Self {
            geom_type: HostGeometryType::Sphere,
            is_small: false,
            height: 1.0,
            major_radius: 1.0,
            minor_radius: 1.0,
            authored: false,
        }
    }
}

impl HostGeometryInfo {
    /// C++ `GeometryInfo::calcBoundingStuff` circle (`Geometry.cpp:468-495`).
    pub fn bounding_circle_radius(&self) -> f32 {
        match self.geom_type {
            HostGeometryType::Sphere | HostGeometryType::Cylinder => self.major_radius,
            HostGeometryType::Box => (self.major_radius * self.major_radius
                + self.minor_radius * self.minor_radius)
                .sqrt(),
        }
    }

    /// C++ `GeometryInfo::calcBoundingStuff` sphere (`Geometry.cpp:468-495`).
    pub fn bounding_sphere_radius(&self) -> f32 {
        match self.geom_type {
            HostGeometryType::Sphere => self.major_radius,
            HostGeometryType::Cylinder => {
                let half_h = self.height * 0.5;
                if half_h < self.major_radius {
                    self.major_radius
                } else {
                    half_h
                }
            }
            HostGeometryType::Box => {
                let half_h = self.height * 0.5;
                (self.major_radius * self.major_radius
                    + self.minor_radius * self.minor_radius
                    + half_h * half_h)
                    .sqrt()
            }
        }
    }

    /// C++ `GeometryInfo::getMaxHeightAbovePosition` (Sphere→major; Box/Cylinder→height).
    pub fn max_height_above_position(&self) -> f32 {
        match self.geom_type {
            HostGeometryType::Sphere => self.major_radius,
            HostGeometryType::Cylinder | HostGeometryType::Box => self.height,
        }
    }

    /// Stamp host pose `GeometryInfo` (Y-up bounds) from C++ geom extents.
    pub fn to_host_geometry(&self) -> GeometryInfo {
        let (bounds_min, bounds_max, radius) = match self.geom_type {
            HostGeometryType::Sphere => {
                let r = self.major_radius;
                (Vec3::splat(-r), Vec3::splat(r), r)
            }
            HostGeometryType::Cylinder => {
                let r = self.major_radius;
                let h = self.height;
                (Vec3::new(-r, 0.0, -r), Vec3::new(r, h, r), r)
            }
            HostGeometryType::Box => {
                let a = self.major_radius;
                let b = self.minor_radius;
                let h = self.height;
                (
                    Vec3::new(-a, 0.0, -b),
                    Vec3::new(a, h, b),
                    self.bounding_circle_radius(),
                )
            }
        };
        GeometryInfo {
            position: Vec3::ZERO,
            rotation: 0.0,
            bounds_min,
            bounds_max,
            radius,
        }
    }
}

/// Thing Template - shared configuration data for Things
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThingTemplate {
    pub name: String,
    pub display_name: String,
    pub kind_of: HashSet<KindOf>,
    pub max_health: f32,
    pub armor: f32,
    /// C++ `ThingTemplate::m_visionRange` from Object INI `VisionRange`.
    /// Default 0 = reveal nothing (ThingTemplate.cpp:976).
    #[serde(default)]
    pub sight_range: f32,
    /// C++ `ThingTemplate::m_shroudClearingRange`. `-1` means use `sight_range`.
    #[serde(default = "default_template_shroud_clearing_range")]
    pub shroud_clearing_range: f32,
    /// C++ `ThingTemplate::m_shroudRevealToAllRange`. `-1` / `<= 0` means none.
    #[serde(default = "default_template_shroud_reveal_to_all_range")]
    pub shroud_reveal_to_all_range: f32,
    /// C++ `KINDOF_REVEAL_TO_ALL` — full ally-range looker for every player.
    #[serde(default)]
    pub reveal_to_all: bool,
    /// C++ `KINDOF_ALWAYS_VISIBLE` — never shrouded (UI feedback objects).
    #[serde(default)]
    pub always_visible: bool,
    pub build_cost: Resources,
    pub build_time: f32,
    /// C++ `ThingTemplate::m_buildable` (`BSTATUS_YES` = 0).
    #[serde(default)]
    pub buildable_status: u32,
    /// C++ `ThingTemplate::m_refundValue` from Object INI `RefundValue`.
    /// A zero value means "use BuildCost × GlobalData::SellPercentage";
    /// a non-zero value is an exact sale refund.
    #[serde(default)]
    pub refund_value: u16,
    /// C++ `ThingTemplate::m_threatValue` from Object INI `ThreatValue`.
    /// Object::addThreat stamps this, never BuildCost (Object.cpp:4873).
    #[serde(default)]
    pub threat_value: u16,
    pub model_name: Option<String>,
    pub texture_name: Option<String>,
    /// C++ `ThingTemplate::m_assetScale` from Object INI `Scale`.
    #[serde(default = "default_asset_scale")]
    pub asset_scale: f32,
    /// Authored DockUpdate family.  Never infer this from a template name.
    #[serde(default)]
    pub dock_kind: DockKind,
    /// `SupplyWarehouseDockUpdate::StartingBoxes`, when authored.  `Some(0)`
    /// is meaningful and must remain distinct from no warehouse module.
    #[serde(default)]
    pub dock_starting_boxes: Option<u32>,
    /// `SupplyWarehouseDockUpdate::DeleteWhenEmpty`.  It only applies to a
    /// warehouse dock; ordinary resource objects retain their own lifecycle.
    #[serde(default)]
    pub dock_delete_when_empty: bool,
    /// Exact `SupplyTruckAIUpdate` module data, when authored.
    #[serde(default)]
    pub supply_truck_metadata: Option<SupplyTruckMetadata>,
    /// C++ `SupplyTruckAIUpdateModuleData::m_suppliesDepletedVoice`.
    #[serde(default)]
    pub supplies_depleted_voice: String,

    /// `RailedTransportContain::Slots`, when that exact contain module is
    /// present.  A railed dock with no contain module never gains synthetic
    /// transport capacity.
    #[serde(default)]
    pub railed_transport_slots: Option<usize>,
    /// C++ `RailedTransportAIUpdateModuleData::m_pathPrefixName`.
    /// Empty unless that exact AI module authored `PathPrefixName`.
    #[serde(default)]
    pub railed_path_prefix_name: String,

    /// Exact source containment behavior and capacity, parsed from the Object
    /// INI module rather than inferred from VEHICLE, dimensions, or a name.
    #[serde(default)]
    pub contain_module: ContainModuleMetadata,
    /// Exact StealthUpdate FriendlyOpacityMin/Max values.  They are retained
    /// on the immutable template so presentation can select the C++ friendly
    /// look without re-reading INI or live GameLogic during WGPU collection.
    #[serde(default = "default_stealth_friendly_opacity_min")]
    pub stealth_friendly_opacity_min: f32,
    #[serde(default = "default_stealth_friendly_opacity_max")]
    pub stealth_friendly_opacity_max: f32,
    /// Exact `ParkingPlaceBehavior` data.  This remains absent when the
    /// source object has no such behavior, even if its KindOf is
    /// `FSAirfield`; physical aircraft landing then fails closed.
    #[serde(default)]
    pub parking_place: Option<ParkingPlaceMetadata>,
    /// Exact `FlightDeckBehavior` data.  Absent unless the source object
    /// declares that Behavior; a carrier KindOf never fabricates a deck.
    #[serde(default)]
    pub flight_deck: Option<FlightDeckMetadata>,
    /// Exact `DeployStyleAIUpdate` behavior.  It remains absent unless the
    /// source object actually declares that Behavior; a vehicle name or
    /// `CAN_ATTACK` KindOf never creates deploy authority.
    #[serde(default)]
    pub deploy_style_metadata: Option<DeployStyleMetadata>,
    /// Exact `QueueProductionExitUpdate` or `DefaultProductionExitUpdate`
    /// module.  Missing data never grants a named producer a synthetic Queue
    /// delay, authored exit point, or batch-release policy.
    #[serde(default)]
    pub production_exit_metadata: Option<ProductionExitMetadata>,
    /// Exact `VeterancyCrateCollide IsPilot` data.  This remains absent for a
    /// pilot-named template unless its Object INI authors and parses the
    /// corresponding behavior module.
    #[serde(default)]
    pub veterancy_crate_collide: Option<VeterancyCrateCollideMetadata>,
    /// Exact `EjectPilotDie` module data.  Presence records the C++ die
    /// interface for Hijacker behavior; active death spawning separately
    /// rejects unrepresentable filters or OCLs.
    #[serde(default)]
    pub eject_pilot_die: Option<EjectPilotDieMetadata>,
    /// Exact `RebuildHoleExposeDie` module data. Presence, not a GLA/name
    /// heuristic, is the C++ hole-expose authority.
    #[serde(default)]
    pub rebuild_hole_expose: Option<RebuildHoleExposeDieMetadata>,
    /// Exact `HackInternetAIUpdate` module data.  This remains absent when a
    /// source unit is merely named like a hacker; active command and income
    /// authority require this typed behavior.
    #[serde(default)]
    pub hack_internet_ai_update: Option<HackInternetAIUpdateMetadata>,
    /// Exact paired `SpecialAbility` + `SpecialAbilityUpdate` data for the
    /// Hacker Disable Building channel.  No generic Hacker/template-name
    /// fallback may populate this capability.
    #[serde(default)]
    pub hacker_disable_building: Option<HackerDisableBuildingMetadata>,
    /// Authored `SpecialAbilityUpdate` rows for timed/remote C4 / TNT plants.
    #[serde(default)]
    pub charge_plant_abilities: Vec<ChargePlantAbilityMetadata>,

    /// Source-ordered SpecialPowerModule interfaces.  This is generic module
    /// identity, not a replacement for the HDB paired-channel metadata above.
    #[serde(default)]
    pub special_power_modules: Vec<SpecialPowerModuleMetadata>,
    /// C++ Object INI `EnergyProduction`.  It is an Object field, not a
    /// SpecialPowerModule property: Scud/Particle/Nuke must not borrow a
    /// template-name fallback to affect team power.
    #[serde(default)]
    pub energy_production: Option<i32>,
    /// C++ Object INI `MaxSimultaneousLinkKey`; kept independently because a
    /// link-key object need not expose a player SpecialPowerModule.
    #[serde(default)]
    pub max_simultaneous_link_key: Option<String>,
    /// C++ Object INI `MaxSimultaneousOfType` numeric cap. `0` is unlimited
    /// unless `DeterminedBySuperweaponRestriction` overrides via GameLogic.
    #[serde(default)]
    pub max_simultaneous_of_type: u16,
    /// Exact `MaxSimultaneousOfType=DeterminedBySuperweaponRestriction`
    /// policy associated with this object template.
    #[serde(default)]
    pub max_simultaneous_determined_by_superweapon_restriction: bool,
    /// C++ Object INI `EnergyBonus`.  `None` is its constructor default of
    /// zero; a valid OverchargeBehavior therefore remains player-toggleable
    /// with no power delta.  The parser rejects a malformed *present* field
    /// before exposing the behavior as authority.
    #[serde(default)]
    pub energy_bonus: Option<i32>,
    /// Exact `OverchargeBehavior` data.  A missing or malformed behavior
    /// never receives a player-facing toggle merely because it is a China
    /// plant or has a PowerPlant KindOf.
    #[serde(default)]
    pub overcharge_behavior: Option<OverchargeBehaviorMetadata>,
    /// Exact `PowerPlantUpdate` data for visual rod state.  Its absence does
    /// not prevent an otherwise valid OverchargeBehavior from adding power.
    #[serde(default)]
    pub power_plant_update: Option<PowerPlantUpdateMetadata>,
    /// C++ Object INI `TransportSlotCount`: how much capacity this source
    /// consumes when boarding a normal transport.  `None` is intentionally
    /// unproven and fails closed in a player Enter command.
    #[serde(default)]
    pub transport_slot_count: Option<usize>,
    /// C++ `KINDOF_CAPTURABLE`, retained outside the packed KindOf bank so
    /// capture authorization is data-driven rather than inferred from a
    /// faction/building name.
    #[serde(default)]
    pub capturable: bool,
    /// C++ `KINDOF_IMMUNE_TO_CAPTURE`, likewise independent of targetability
    /// and ordinary structure classification.
    #[serde(default)]
    pub immune_to_capture: bool,
    /// Exact `GarrisonContain` module capacity.  `None` means this target is
    /// not garrisonable for the C++ capture legality check; `Some(0)` remains
    /// distinct and intentionally fail-closed.
    #[serde(default)]
    pub garrison_contain_max: Option<usize>,
    /// Exact `SpecialAbility` capture module on this source, if any.
    #[serde(default)]
    pub capture_power: CapturePowerKind,
    /// `SpecialAbility::StartsPaused` for the authored capture power.
    #[serde(default)]
    pub capture_starts_paused: bool,
    /// `UnpauseSpecialPowerUpgrade::TriggeredBy` for that same capture power.
    #[serde(default)]
    pub capture_upgrade_trigger: Option<String>,
    /// `SpecialAbilityUpdate::StartAbilityRange`.  The authority state uses
    /// this rather than a hero/template-name range fallback.
    #[serde(default)]
    pub capture_start_ability_range: Option<f32>,
    /// `SpecialAbilityUpdate::UnpackTime` (milliseconds).  Capture cannot
    /// begin preparation until this authored animation phase has elapsed.
    #[serde(default)]
    pub capture_unpack_time_ms: Option<u32>,
    /// `SpecialAbilityUpdate::PreparationTime` (milliseconds).  This is the
    /// live channel duration after unpacking, not a click-time delay.
    #[serde(default)]
    pub capture_preparation_time_ms: Option<u32>,
    /// `SpecialAbilityUpdate::PackTime` (milliseconds).  C++ keeps the
    /// ability active through this post-trigger phase before returning idle.
    #[serde(default)]
    pub capture_pack_time_ms: Option<u32>,
    /// `SpecialAbilityUpdate::PackUnpackVariationFactor` for the capture pair.
    #[serde(default)]
    pub capture_pack_unpack_variation_factor: f32,
    /// `SpecialAbilityUpdate::UnpackSound` for the capture module.
    #[serde(default)]
    pub capture_unpack_sound: Option<String>,
    /// `SpecialAbilityUpdate::PackSound` for the capture module.
    #[serde(default)]
    pub capture_pack_sound: Option<String>,
    /// `SpecialAbilityUpdate::TriggerSound` for the capture module.
    #[serde(default)]
    pub capture_trigger_sound: Option<String>,
    /// `SpecialAbilityUpdate::TriggerSound` for leftover steal/disable modules.
    #[serde(default)]
    pub leftover_sa_trigger_sound: Option<String>,

    pub special_power_cooldown: f32,
    /// C++ parity: XP awarded to the killer when this object is destroyed.
    /// Rookie/Regular token; prefer `experience_values` when authored.
    pub experience_value: f32,
    /// C++ `ExperienceValue` 4-int list [Regular, Veteran, Elite, Heroic].
    #[serde(default)]
    pub experience_values: [f32; 4],
    /// C++ `SkillPointValue` 4-int list. `-999` (`USE_EXP_VALUE_FOR_SKILL_VALUE`)
    /// falls back to `ExperienceValue` for that level.
    #[serde(default = "default_template_skill_point_values")]
    pub skill_point_values: [i32; 4],
    /// C++ `ExperienceRequired` mapped to [Veteran, Elite, Heroic] thresholds.
    /// Defaults to [60, 150, 300] for unparsed templates.
    pub veterancy_xp_thresholds: [f32; 3],
    /// C++ ThingTemplate `IsTrainable` (default FALSE).
    #[serde(default)]
    pub is_trainable: bool,
    /// C++ ThingTemplate `EnterGuard` (default FALSE). Guard boards instead of shooting.
    #[serde(default)]
    pub enter_guard: bool,
    /// C++ ThingTemplate `HijackGuard` (default FALSE). Guard hijacks enemy vehicles.
    #[serde(default)]
    pub hijack_guard: bool,
    /// Authored `VeterancyGainCreate` modules (StartingLevel / ScienceRequired).
    #[serde(default)]
    pub veterancy_gain_creates: Vec<VeterancyGainCreateMetadata>,
    /// Authored `GrantUpgradeCreate` modules (UpgradeToGrant / ExemptStatus).
    #[serde(default)]
    pub grant_upgrade_creates: Vec<GrantUpgradeCreateMetadata>,
    /// Authored `LockWeaponCreate` slot (PRIMARY=0, SECONDARY=1, TERTIARY=2).
    #[serde(default)]
    pub lock_weapon_slot: Option<u8>,
    /// Authored `PreorderCreate` module presence (not a template-name heuristic).
    #[serde(default)]
    pub has_preorder_create: bool,
    /// Authored `SpecialPowerCreate` module presence.
    #[serde(default)]
    pub has_special_power_create: bool,
    /// Authored `SupplyCenterCreate` module presence.
    #[serde(default)]
    pub has_supply_center_create: bool,
    /// Authored `SupplyWarehouseCreate` module presence.
    #[serde(default)]
    pub has_supply_warehouse_create: bool,
    /// Host primary weapon stats when the template defines combat capability.
    /// Prefer this over ad-hoc `Weapon::default()` injection at create time.
    pub primary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI primary weapon template name (resolved via WeaponStore).
    pub primary_weapon_name: Option<String>,
    /// An authored no-flag `WeaponSet` explicitly contained `PRIMARY None`.
    /// This is distinct from a template with no retained WeaponSet at all:
    /// the latter may use legacy host fallback while the former must remain
    /// unarmed until a supported conditional set is selected.
    #[serde(default)]
    pub primary_weapon_explicitly_none: bool,
    /// Exact `WeaponSet Conditions = MINE_CLEARING_DETAIL` PRIMARY instance.
    /// It is separate from the ordinary primary so toggling the C++ detail bit
    /// cannot overwrite cooldown/ammo state of a normal combat weapon.
    #[serde(default)]
    pub mine_clearing_primary_weapon: Option<Weapon>,
    /// Source Weapon.ini name for the bounded mine-clearing conditional slot.
    #[serde(default)]
    pub mine_clearing_primary_weapon_name: Option<String>,
    /// Host secondary weapon stats (Weapon = SECONDARY Name). Optional; no kind fallback.
    pub secondary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI secondary weapon template name (resolved via WeaponStore).
    pub secondary_weapon_name: Option<String>,
    /// Host tertiary weapon stats (`Weapon = TERTIARY Name`).
    ///
    /// Kept separate from SECONDARY because C++ WeaponSet has three concrete
    /// slots. In particular, Comanche rocket pods must not replace its
    /// anti-tank SECONDARY weapon.
    #[serde(default)]
    pub tertiary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI tertiary weapon template name (resolved via WeaponStore).
    #[serde(default)]
    pub tertiary_weapon_name: Option<String>,
    /// C++ `WeaponTemplateSet::m_preferredAgainst` per slot (0=PRIMARY).
    /// Empty means the live chooser falls back to residual damage heuristics.
    #[serde(default)]
    pub preferred_against: [Vec<KindOf>; 3],
    /// C++ `WeaponTemplateSet::m_isReloadTimeShared`.
    #[serde(default)]
    pub share_weapon_reload_time: bool,
    /// C++ `WeaponTemplateSet::m_autoChooseMask` per slot. Default all-sources.
    #[serde(default = "default_auto_choose_masks")]
    pub auto_choose_masks: [u32; 3],
    /// C++ `WeaponTemplateSet::m_isWeaponLockSharedAcrossSets`.
    #[serde(default)]
    pub weapon_lock_shared_across_sets: bool,
    /// C++ `WeaponSet` `AutoChooseSources = PRIMARY NONE`.
    ///
    /// The authored PRIMARY still resolves from Weapon.ini when present, but
    /// Object construction must not invent a kind-based `Weapon::default`
    /// after a store miss (Strategy Center artillery starts turret-disabled).
    #[serde(default)]
    pub primary_auto_choose_none: bool,
    /// C++ `FireOCLAfterWeaponCooldownUpdate` is present on the Object INI.
    /// Create installs the residual module from this flag, not a unit name.
    #[serde(default)]
    pub has_fire_ocl_after_weapon_cooldown: bool,
    /// Source-ordered `FireWeaponWhenDamagedBehavior` module data.  This is
    /// retained separately from ordinary WeaponSet slots because C++ creates
    /// up to eight independent PRIMARY `Weapon` instances per module.  Main
    /// does not activate the records until Object snapshot persistence can
    /// carry each mutable Weapon state in C++ Xfer order.
    #[serde(default)]
    pub fire_weapon_when_damaged_behaviors:
        Vec<crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDamagedMetadata>,
    /// Source-ordered `FireWeaponWhenDeadBehavior` module data.  C++ creates
    /// a fresh ephemeral PRIMARY Weapon for each qualifying death, so these
    /// records retain source gates/references only and add no object snapshot
    /// state by themselves.
    #[serde(default)]
    pub fire_weapon_when_dead_behaviors:
        Vec<crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadMetadata>,
    /// Locomotor.ini SET_NORMAL template name (resolved via Common LocomotorStore).
    /// Primary member only; the full SET_* row lives in `locomotor_set_names`.
    pub locomotor_name: Option<String>,
    /// Authored SET_NORMAL (or current SET_*) members in declaration order.
    /// C++ `chooseGoodLocomotorFromCurrentSet` picks one by cell surface.
    #[serde(default)]
    pub locomotor_set_names: Vec<String>,
    /// C++ CreateCrateDieModuleData::m_crateNameList residual (CrateData names).
    #[serde(default)]
    pub create_crate_data: Vec<String>,
    /// C++ `ThingTemplate::m_armorTemplateSets` from Object INI `ArmorSet`.
    #[serde(default)]
    pub armor_sets: Vec<HostArmorSet>,
    /// C++ `ActiveBodyModuleData::m_subdualDamageCap`. Default 0 = immune.
    #[serde(default)]
    pub subdual_damage_cap: f32,
    /// C++ `SubdualDamageHealRate` converted to logic frames.
    #[serde(default)]
    pub subdual_heal_rate_frames: u32,
    /// C++ `SubdualDamageHealAmount`.
    #[serde(default)]
    pub subdual_heal_amount: f32,
    /// C++ PhysicsBehaviorModuleData::m_mass from Object INI `Mass`.
    #[serde(default = "default_template_physics_mass")]
    pub physics_mass: f32,
    /// C++ PhysicsBehaviorModuleData::m_shockResistance from `ShockResistance`.
    #[serde(default)]
    pub shock_resistance: f32,
    /// C++ PhysicsBehaviorModuleData::m_pitchRollYawFactor (default 2.0).
    #[serde(default = "default_template_pitch_roll_yaw_factor")]
    pub pitch_roll_yaw_factor: f32,
    /// C++ PhysicsBehaviorModuleData friction (per-frame after parseFrictionPerSec).
    #[serde(default = "default_template_forward_friction")]
    pub forward_friction: f32,
    #[serde(default = "default_template_lateral_friction")]
    pub lateral_friction: f32,
    #[serde(default = "default_template_z_friction")]
    pub z_friction: f32,
    #[serde(default)]
    pub aerodynamic_friction: f32,
    /// C++ PhysicsBehaviorModuleData::m_centerOfMassOffset.
    #[serde(default)]
    pub center_of_mass_offset: f32,
    /// C++ PhysicsBehaviorModuleData::m_allowBouncing.
    #[serde(default)]
    pub allow_bouncing: bool,
    /// C++ PhysicsBehaviorModuleData::m_allowCollideForce (default true).
    #[serde(default = "default_template_allow_collide_force")]
    pub allow_collide_force: bool,
    /// C++ PhysicsBehaviorModuleData::m_killWhenRestingOnGround.
    #[serde(default)]
    pub kill_when_resting_on_ground: bool,
    /// C++ m_minFallSpeedForDamage after parseHeightToSpeed.
    #[serde(default = "default_template_min_fall_speed")]
    pub min_fall_speed_for_damage: f32,
    /// C++ PhysicsBehaviorModuleData::m_fallHeightDamageFactor (default 1).
    #[serde(default = "default_template_fall_height_damage_factor")]
    pub fall_height_damage_factor: f32,
    /// C++ `ThingTemplate::m_crusherLevel` from Object INI `CrusherLevel`.
    /// Default 0 = cannot crush anything (ThingTemplate.cpp:1023).
    #[serde(default)]
    pub crusher_level: u8,
    /// C++ `ThingTemplate::m_crushableLevel` from Object INI `CrushableLevel`.
    /// Default 255 = cannot be crushed (ThingTemplate.cpp:1024).
    #[serde(default = "default_template_crushable_level")]
    pub crushable_level: u8,
    /// C++ `ThingTemplate::m_fenceWidth` from Object INI `FenceWidth`.
    #[serde(default)]
    pub fence_width: f32,
    /// C++ `ThingTemplate::m_fenceXOffset` from Object INI `FenceXOffset`.
    #[serde(default)]
    pub fence_x_offset: f32,
    /// C++ `ThingTemplate::m_shadowSizeX` (Object INI `ShadowSizeX`).
    #[serde(default)]
    pub shadow_size_x: f32,
    /// C++ `ThingTemplate::m_shadowSizeY` (Object INI `ShadowSizeY`).
    #[serde(default)]
    pub shadow_size_y: f32,
    /// C++ `ThingTemplate::m_shadowType` (Object INI `Shadow` bitstring).
    #[serde(default)]
    pub shadow_type: u32,
    /// C++ `ThingTemplate::m_shadowOffsetX` (Object INI `ShadowOffsetX`).
    #[serde(default)]
    pub shadow_offset_x: f32,
    /// C++ `ThingTemplate::m_shadowOffsetY` (Object INI `ShadowOffsetY`).
    #[serde(default)]
    pub shadow_offset_y: f32,
    /// C++ `ThingTemplate::m_shadowTextureName` (Object INI `ShadowTexture`).
    #[serde(default)]
    pub shadow_texture: Option<String>,

    /// C++ `ThingTemplate::m_radarPriority` (Object INI `RadarPriority`).
    /// 0=INVALID, 1=NOT_ON_RADAR, 2=STRUCTURE, 3=UNIT, 4=LOCAL_UNIT_ONLY.
    #[serde(default)]
    pub radar_priority: u8,
    /// C++ `TTAUDIO_soundMoveStart`.
    #[serde(default)]
    pub sound_move_start: Option<String>,
    /// C++ `TTAUDIO_soundMoveStartDamaged`.
    #[serde(default)]
    pub sound_move_start_damaged: Option<String>,
    /// C++ `TTAUDIO_soundMoveLoop`.
    #[serde(default)]
    pub sound_move_loop: Option<String>,
    /// C++ `TTAUDIO_soundMoveLoopDamaged`.
    #[serde(default)]
    pub sound_move_loop_damaged: Option<String>,
    /// C++ `TTAUDIO_soundAmbient`.
    #[serde(default)]
    pub sound_ambient: Option<String>,
    /// C++ `TTAUDIO_soundAmbientDamaged`.
    #[serde(default)]
    pub sound_ambient_damaged: Option<String>,
    /// C++ `TTAUDIO_soundAmbientReallyDamaged`.
    #[serde(default)]
    pub sound_ambient_really_damaged: Option<String>,
    /// C++ `TTAUDIO_soundAmbientRubble`.
    #[serde(default)]
    pub sound_ambient_rubble: Option<String>,
    /// C++ `ThingTemplate::m_upgradeCameoUpgradeNames` (`UpgradeCameo1..5`).
    #[serde(default)]
    pub upgrade_cameo_names: [String; 5],

    /// C++ `ThingTemplate::m_geometryInfo` from Object INI Geometry*.
    #[serde(default)]
    pub geometry_info: HostGeometryInfo,
    /// C++ `ThingTemplate::m_structureRubbleHeight` (unsigned byte; 0 = GameData default).
    #[serde(default)]
    pub structure_rubble_height: u8,
    /// C++ `AIUpdateModuleData::m_autoAcquireEnemiesWhenIdle` from Object INI.
    #[serde(default)]
    pub auto_acquire_enemies_when_idle: u32,
    /// C++ `AIUpdateModuleData::m_forbidPlayerCommands` (Spectre gunship).
    #[serde(default)]
    pub forbid_player_commands: bool,
    /// Leftover `ThingTemplate::m_prereqInfo` from Object INI `Prerequisites`.
    /// Template data, not instance state — re-parsed from leftover factory / INI.
    #[serde(skip)]
    pub production_prerequisites: Vec<game_engine::common::rts::ProductionPrerequisite>,
}

impl ThingTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: name.to_string(),
            kind_of: HashSet::new(),
            max_health: 100.0,
            armor: 0.0,
            sight_range: 0.0,
            shroud_clearing_range: default_template_shroud_clearing_range(),
            shroud_reveal_to_all_range: default_template_shroud_reveal_to_all_range(),
            reveal_to_all: false,
            always_visible: false,
            build_cost: Resources::default(),
            build_time: 1.0,
            buildable_status: 0,
            refund_value: 0,
            threat_value: 0,
            model_name: None,
            texture_name: None,
            asset_scale: default_asset_scale(),
            dock_kind: DockKind::None,
            dock_starting_boxes: None,
            dock_delete_when_empty: false,
            supply_truck_metadata: None,
            supplies_depleted_voice: String::new(),
            railed_transport_slots: None,
            railed_path_prefix_name: String::new(),

            contain_module: ContainModuleMetadata::default(),

            stealth_friendly_opacity_min: default_stealth_friendly_opacity_min(),
            stealth_friendly_opacity_max: default_stealth_friendly_opacity_max(),
            parking_place: None,
            flight_deck: None,
            deploy_style_metadata: None,
            production_exit_metadata: None,
            veterancy_crate_collide: None,
            eject_pilot_die: None,
            rebuild_hole_expose: None,
            hack_internet_ai_update: None,
            hacker_disable_building: None,
            charge_plant_abilities: Vec::new(),
            special_power_modules: Vec::new(),
            energy_production: None,
            max_simultaneous_link_key: None,
            max_simultaneous_determined_by_superweapon_restriction: false,
            max_simultaneous_of_type: 0,
            energy_bonus: None,
            overcharge_behavior: None,
            power_plant_update: None,
            transport_slot_count: None,
            capturable: false,
            immune_to_capture: false,
            garrison_contain_max: None,
            capture_power: CapturePowerKind::None,
            capture_starts_paused: false,
            capture_upgrade_trigger: None,
            capture_start_ability_range: None,
            capture_unpack_time_ms: None,
            capture_preparation_time_ms: None,
            capture_pack_time_ms: None,
            capture_pack_unpack_variation_factor: 0.0,
            capture_unpack_sound: None,
            capture_pack_sound: None,
            capture_trigger_sound: None,
            leftover_sa_trigger_sound: None,

            special_power_cooldown: 10.0,

            experience_value: 0.0,
            experience_values: [0.0; 4],
            skill_point_values:
                [crate::game_logic::host_rank_ui_residual::USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL;
                    4],
            veterancy_xp_thresholds: [60.0, 150.0, 300.0],
            is_trainable: false,
            enter_guard: false,
            hijack_guard: false,
            veterancy_gain_creates: Vec::new(),
            grant_upgrade_creates: Vec::new(),
            lock_weapon_slot: None,
            has_preorder_create: false,
            has_special_power_create: false,
            has_supply_center_create: false,
            has_supply_warehouse_create: false,
            primary_weapon: None,
            primary_weapon_name: None,
            primary_weapon_explicitly_none: false,
            mine_clearing_primary_weapon: None,
            mine_clearing_primary_weapon_name: None,
            secondary_weapon: None,
            secondary_weapon_name: None,
            tertiary_weapon: None,
            tertiary_weapon_name: None,
            preferred_against: [Vec::new(), Vec::new(), Vec::new()],
            share_weapon_reload_time: false,
            auto_choose_masks: default_auto_choose_masks(),
            weapon_lock_shared_across_sets: false,
            primary_auto_choose_none: false,
            has_fire_ocl_after_weapon_cooldown: false,
            fire_weapon_when_damaged_behaviors: Vec::new(),
            fire_weapon_when_dead_behaviors: Vec::new(),
            locomotor_name: None,
            locomotor_set_names: Vec::new(),
            create_crate_data: Vec::new(),
            armor_sets: Vec::new(),
            subdual_damage_cap: 0.0,
            subdual_heal_rate_frames: 0,
            subdual_heal_amount: 0.0,
            physics_mass: 1.0,
            shock_resistance: 0.0,
            pitch_roll_yaw_factor: 2.0,
            forward_friction: 0.15,
            lateral_friction: 0.15,
            z_friction: 0.8,
            aerodynamic_friction: 0.0,
            center_of_mass_offset: 0.0,
            allow_bouncing: false,
            allow_collide_force: true,
            kill_when_resting_on_ground: false,
            min_fall_speed_for_damage: default_template_min_fall_speed(),
            fall_height_damage_factor: 1.0,
            crusher_level: 0,
            crushable_level: 255,
            fence_width: 0.0,
            fence_x_offset: 0.0,
            shadow_size_x: 0.0,
            shadow_size_y: 0.0,
            shadow_type: 0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_texture: None,

            radar_priority: 0,
            sound_move_start: None,
            sound_move_start_damaged: None,
            sound_move_loop: None,
            sound_move_loop_damaged: None,
            sound_ambient: None,
            sound_ambient_damaged: None,
            sound_ambient_really_damaged: None,
            sound_ambient_rubble: None,
            upgrade_cameo_names: Default::default(),

            geometry_info: HostGeometryInfo::default(),
            structure_rubble_height: 0,
            auto_acquire_enemies_when_idle: 0,
            forbid_player_commands: false,
            production_prerequisites: Vec::new(),
        }
    }
    /// C++ `ThingTemplate::getExperienceValue(level)`. Uses the authored
    /// 4-int table when any token is non-zero; otherwise the single
    /// `experience_value` field (tests / unparsed templates).
    pub fn experience_value_for_level(&self, level: VeterancyLevel) -> f32 {
        let idx = match level {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        if self.experience_values.iter().any(|v| *v != 0.0) {
            self.experience_values[idx]
        } else {
            self.experience_value
        }
    }

    /// C++ `ThingTemplate::getSkillPointValue(level)`.
    pub fn skill_point_value_for_level(&self, level: VeterancyLevel) -> i32 {
        let idx = match level {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        let value = self.skill_point_values[idx];
        if value == crate::game_logic::host_rank_ui_residual::USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL
        {
            self.experience_value_for_level(level) as i32
        } else {
            value
        }
    }

    pub fn charge_plant_ability_for_timed(&self) -> Option<&ChargePlantAbilityMetadata> {
        self.charge_plant_abilities
            .iter()
            .find(|ability| ability.is_timed_charge_power())
    }

    pub fn charge_plant_ability_for_remote(&self) -> Option<&ChargePlantAbilityMetadata> {
        self.charge_plant_abilities
            .iter()
            .find(|ability| ability.is_remote_charge_power())
    }

    /// Preserve a drawable authored C++ asset scale. Retail Object INIs use
    /// positive finite values; malformed values retain the default instead of
    /// entering a WGPU transform as NaN or infinity.
    pub fn set_asset_scale(&mut self, scale: f32) -> &mut Self {
        if scale.is_finite() && scale > 0.0 {
            self.asset_scale = scale;
        }
        self
    }

    /// Whether this template crossed the typed authority boundary for an
    /// Overcharge command.  The behavior alone is authoritative: C++ permits
    /// an authored module when ThingTemplate::EnergyBonus retains its default
    /// zero.  The parser rejects malformed present bonus fields before this
    /// can return true.
    #[inline]
    pub fn supports_overcharge(&self) -> bool {
        self.overcharge_behavior.is_some()
    }

    /// C++ `Object::getSpecialPowerModule`-style lookup by the host command
    /// adapter.  The stored module record remains keyed by exact loaded
    /// template name/id; the enum comparison merely routes an already parsed
    /// command to the first matching source module in declaration order.
    #[inline]
    pub fn special_power_module_for_command(
        &self,
        power: &crate::command_system::SpecialPowerType,
    ) -> Option<&SpecialPowerModuleMetadata> {
        self.special_power_modules
            .iter()
            .find(|module| module.command_power.as_ref() == Some(power))
    }

    /// Whether this exact Object INI template participates in the C++
    /// `DeterminedBySuperweaponRestriction` link-key quota.
    #[inline]
    pub fn has_superweapon_restriction_link_key(&self) -> bool {
        self.max_simultaneous_determined_by_superweapon_restriction
            && self
                .max_simultaneous_link_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty())
    }

    /// C++ `ThingTemplate::getMaxSimultaneousOfType`.
    #[inline]
    pub fn get_max_simultaneous_of_type(&self, superweapon_restriction: u32) -> u32 {
        if self.max_simultaneous_determined_by_superweapon_restriction {
            superweapon_restriction
        } else {
            u32::from(self.max_simultaneous_of_type)
        }
    }

    /// C++ `countExisting` template match: `isEquivalentTo` (name) or shared
    /// `MaxSimultaneousLinkKey`.
    #[inline]
    pub fn counts_toward_max_simultaneous_of(&self, wanted: &ThingTemplate) -> bool {
        if self.name.eq_ignore_ascii_case(&wanted.name) {
            return true;
        }
        match (
            wanted
                .max_simultaneous_link_key
                .as_deref()
                .map(str::trim)
                .filter(|key| !key.is_empty()),
            self.max_simultaneous_link_key
                .as_deref()
                .map(str::trim)
                .filter(|key| !key.is_empty()),
        ) {
            (Some(wanted_key), Some(candidate_key))
                if wanted_key.eq_ignore_ascii_case(candidate_key) =>
            {
                true
            }
            _ => false,
        }
    }

    /// Attach host primary weapon stats (damage/range/reload) to this template.
    /// C++ CreateCrateDie CrateData residual append.
    pub fn add_create_crate_data(&mut self, crate_data_name: &str) -> &mut Self {
        let n = crate_data_name.trim();
        if !n.is_empty() {
            self.create_crate_data.push(n.to_string());
        }
        self
    }

    pub fn set_primary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.primary_weapon = Some(weapon);
        self.primary_weapon_explicitly_none = false;
        self
    }

    /// Record the Weapon.ini template name for store lookup at create time.
    pub fn set_primary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.primary_weapon_name = Some(n.to_string());
            self.primary_weapon_explicitly_none = false;
        }
        self
    }

    /// Preserve `WeaponSet Conditions = None` / `Weapon = PRIMARY None`.
    /// This must suppress generic kind/name fallback so a dozer or worker
    /// cannot gain an invented ordinary primary before its authored mine-clear
    /// detail set is selected.
    pub fn set_primary_weapon_none(&mut self) -> &mut Self {
        self.primary_weapon = None;
        self.primary_weapon_name = None;
        self.primary_weapon_explicitly_none = true;
        self
    }

    /// Attach exact host stats for a supported mine-clearing conditional
    /// primary. This has no relation to a generic `Worker`/`Dozer` identity.
    pub fn set_mine_clearing_primary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.mine_clearing_primary_weapon = Some(weapon);
        self
    }

    /// Record the exact Weapon.ini name used by `MINE_CLEARING_DETAIL`.
    /// Empty/`None` rows remain absent and therefore fail closed at arm time.
    pub fn set_mine_clearing_primary_weapon_name(&mut self, name: &str) -> &mut Self {
        let name = name.trim();
        if !name.is_empty() && !name.eq_ignore_ascii_case("none") {
            self.mine_clearing_primary_weapon_name = Some(name.to_string());
        }
        self
    }

    /// Attach host secondary weapon stats (damage/range/reload) to this template.
    pub fn set_secondary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.secondary_weapon = Some(weapon);
        self
    }

    /// Record the Weapon.ini secondary template name for store lookup at create time.
    /// Fail-closed: "None"/empty does not register a secondary slot.
    pub fn set_secondary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.secondary_weapon_name = Some(n.to_string());
        }
        self
    }

    /// Attach host tertiary weapon stats (damage/range/reload) to this template.
    pub fn set_tertiary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.tertiary_weapon = Some(weapon);
        self
    }

    /// Record the Weapon.ini tertiary template name for store lookup at create time.
    /// Fail-closed: "None"/empty does not register a tertiary slot.
    pub fn set_tertiary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.tertiary_weapon_name = Some(n.to_string());
        }
        self
    }

    /// Record the Locomotor.ini SET_NORMAL template name for store lookup at create time.
    /// Fail-closed: empty/"None" does not register a locomotor bind.
    pub fn set_locomotor_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.locomotor_name = Some(n.to_string());
        }
        self
    }

    /// Store every authored SET_* member so live march can surface-switch.
    pub fn set_locomotor_set_names(&mut self, names: &[String]) -> &mut Self {
        self.locomotor_set_names = names
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("none"))
            .map(|n| n.to_string())
            .collect();
        if self.locomotor_name.is_none() {
            if let Some(first) = self.locomotor_set_names.first() {
                self.locomotor_name = Some(first.clone());
            }
        }
        self
    }

    /// Resolve host Movement stats from the Locomotor catalog:
    /// 1) explicit locomotor_name → LocomotorStore (seed/INI)
    /// Fail-closed: no kind-based default — units without a name keep Movement::default().
    pub fn resolve_movement(&self) -> Option<super::locomotor_bootstrap::HostMovementStats> {
        if let Some(name) = self.locomotor_name.as_deref() {
            // Host residual: unit tests / early create often have an empty store
            // (no AssetManager archive load). Bootstrap seeds known locomotors or
            // loads extracted Locomotor.ini when present — see locomotor_bootstrap.rs.
            return super::locomotor_bootstrap::resolve_host_movement(name);
        }
        None
    }

    /// Resolve weapon for a newly created combat unit:
    /// 1) explicit host stats, 2) WeaponStore by primary_weapon_name,
    /// 3) host residual map by template name (`primary_weapon_name_for_unit`),
    /// 4) kind-based default fallback (fail-open last resort for Attackable kinds).
    pub fn resolve_primary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.primary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.primary_weapon_name.as_deref() {
            // Host residual: unit tests / early create often have an empty store
            // (no AssetManager archive load). Bootstrap seeds known weapons or
            // loads extracted Weapon.ini when present — see weapon_bootstrap.rs.
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(name) {
                return Some(w);
            }
        }
        if self.primary_weapon_explicitly_none || self.primary_auto_choose_none {
            // C++ Object.cpp:160-497 arms only ThingTemplate WeaponSet data.
            // AutoChooseSources=PRIMARY NONE must not fall through to a
            // kind-based Weapon::default after a store miss.
            return None;
        }
        // Host residual map: templates often omit primary_weapon_name (units.rs /
        // setup_templates gaps) but have a known retail weapon for the unit name.
        // Prefer store residual over kind-based Weapon::default().
        if let Some(wname) = super::weapon_bootstrap::primary_weapon_name_for_unit(&self.name) {
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(wname) {
                return Some(w);
            }
        }
        if self.is_kind_of(KindOf::Infantry)
            || self.is_kind_of(KindOf::Vehicle)
            || self.is_kind_of(KindOf::Aircraft)
            || self.is_kind_of(KindOf::Attackable)
        {
            // Last-resort host combat stats when no template/store weapon is usable.
            return Some(Weapon::default());
        }
        None
    }

    /// Resolve secondary weapon for a newly created combat unit.
    /// Fail-closed (not full WeaponSet):
    /// 1) explicit host stats, 2) WeaponStore by secondary_weapon_name,
    /// 3) host residual map by template name (`secondary_weapon_name_for_unit`).
    /// No kind-based `Weapon::default()` fallback — units without SECONDARY stay unarmed there.
    pub fn resolve_secondary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.secondary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.secondary_weapon_name.as_deref() {
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(name) {
                return Some(w);
            }
        }
        // Host residual map: secondary slot by unit template name when not set.
        if let Some(wname) = super::weapon_bootstrap::secondary_weapon_name_for_unit(&self.name) {
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            if let Some(w) = Self::weapon_from_store(wname) {
                return Some(w);
            }
        }
        None
    }

    /// Resolve tertiary weapon for a newly created combat unit.
    ///
    /// TERTIARY has no template-name or KindOf fallback: it is generally a
    /// manual/conditional WeaponSet slot, so inventing one would turn an
    /// unavailable ability into a primary shot.
    pub fn resolve_tertiary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.tertiary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.tertiary_weapon_name.as_deref() {
            let _ = super::weapon_bootstrap::ensure_host_weapon_store();
            return Self::weapon_from_store(name);
        }
        None
    }

    /// Resolve the exact supported `MINE_CLEARING_DETAIL` primary. Unlike the
    /// ordinary primary there is intentionally no template-name or KindOf
    /// fallback: an untyped unit may not acquire mine-clearing authority.
    pub fn resolve_mine_clearing_primary_weapon(&self) -> Option<Weapon> {
        if let Some(weapon) = &self.mine_clearing_primary_weapon {
            return Some(weapon.clone());
        }
        let name = self.mine_clearing_primary_weapon_name.as_deref()?;
        let _ = super::weapon_bootstrap::ensure_host_weapon_store();
        Self::weapon_from_store(name)
    }

    /// Convert a gamelogic WeaponStore template into Main host Weapon stats.
    /// Returns None if store is missing or stats are unusable (0 dmg/range).
    pub fn weapon_from_store(name: &str) -> Option<Weapon> {
        use gamelogic::weapon::{WeaponAntiMask, WeaponBonus, with_weapon_store};
        const FPS: f32 = 30.0;
        let wt = with_weapon_store(|store| store.find_weapon_template(name).cloned()).ok()??;
        if wt.primary_damage <= 0.0 || wt.attack_range <= 0.0 {
            return None;
        }
        // Leftover WeaponTemplate::get_delay_between_shots (Weapon.cpp:475-490).
        // DelayBetweenShots is a Min/Max range, not clip vs between. Identity
        // RATE_OF_FIRE here — leftover applies the ROF floor at fire. Ready-checks
        // must not consume GameLogicRandomValue (C++ draws once in privateFireWeapon);
        // force leftover's min==max branch for the stored yardstick.
        let delay_frames = if wt.min_delay_between_shots == wt.max_delay_between_shots {
            wt.get_delay_between_shots(&WeaponBonus::new())
        } else {
            let mut yardstick = gamelogic::weapon::WeaponTemplate::new(wt.name.clone());
            yardstick.min_delay_between_shots = wt.min_delay_between_shots;
            yardstick.max_delay_between_shots = wt.min_delay_between_shots;
            yardstick.get_delay_between_shots(&WeaponBonus::new())
        };
        let reload_time = if delay_frames > 0 {
            delay_frames as f32 / FPS
        } else {
            1.0
        };
        let pre_attack_delay = (wt.pre_attack_delay.max(0) as f32) / FPS;
        let projectile_speed = if wt.weapon_speed >= 999_999.0 {
            0.0
        } else {
            wt.weapon_speed
        };
        let suspend_fx_frame = crate::game_logic::host_historic_bonus::logic_frame()
            .saturating_add(wt.suspend_fx_delay);
        // Leftover WeaponTemplate::get_attack_range / get_minimum_attack_range
        // (Weapon.cpp:437-462, RATIONALIZE_ATTACK_RANGE): −¼ pathfind cell.
        // Identity RANGE bonus — leftover applies RANGE at fire.
        let bonus = WeaponBonus::new();
        Some(Weapon {
            damage: wt.primary_damage,
            range: wt.get_attack_range(&bonus),
            min_range: wt.get_minimum_attack_range(),
            reload_time,
            last_fire_time: 0.0,
            ammo: if wt.clip_size > 0 {
                Some(wt.clip_size as u32)
            } else {
                None
            },
            clip_size: wt.clip_size.max(0) as u32,
            // C++ ClipReloadTime is independent of DelayBetweenShots. Store
            // already converted msec → frames; host Weapon uses seconds.
            // Absent/0 stays 0 — reloadWithBonus is ready the same frame.
            clip_reload_time: if wt.clip_size > 0 {
                (wt.clip_reload_time.max(0) as f32) / FPS
            } else {
                0.0
            },
            can_target_air: wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_VEHICLE)
                || wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_INFANTRY),
            // C++ WeaponTemplate defaults to AntiGround and accepts a ground
            // victim only when that actual anti-mask bit is set.  Treating an
            // arbitrary non-air mask (for example AntiProjectile) as ground
            // let point-defense-only weapons attack ordinary units.
            can_target_ground: wt.anti_mask.contains(WeaponAntiMask::GROUND),
            projectile_speed,
            pre_attack_delay,
            splash_radius: wt.primary_damage_radius.max(0.0),
            reloading_clip: false,
            last_bonus_rof: 0.0,
            suspend_fx_frame,
        })
    }

    /// C++ FiringTracker thresholds copied from the live WeaponStore.
    /// `weapon_from_store` only builds host `Weapon` fire stats; these fields
    /// live on the Object (ContinuousFireOne/Two/Coast, AutoReloadWhenIdle).
    pub fn weapon_tracker_from_store(name: &str) -> WeaponTrackerBind {
        use gamelogic::weapon::with_weapon_store;
        let _ = super::weapon_bootstrap::ensure_host_weapon_store();
        with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|wt| WeaponTrackerBind {
                    continuous_fire_one_shots: shots_needed_to_host(
                        wt.continuous_fire_one_shots_needed,
                    ),
                    continuous_fire_two_shots: shots_needed_to_host(
                        wt.continuous_fire_two_shots_needed,
                    ),
                    continuous_fire_coast_frames: wt.continuous_fire_coast_frames,
                    auto_reload_when_idle_frames: wt.auto_reload_when_idle_frames,
                })
        })
        .ok()
        .flatten()
        .unwrap_or_default()
    }

    /// Resolve ContinuousFire / AutoReloadWhenIdle for this template's primary.
    pub fn weapon_tracker_bind(&self) -> WeaponTrackerBind {
        let name = self
            .primary_weapon_name
            .as_deref()
            .or_else(|| super::weapon_bootstrap::primary_weapon_name_for_unit(&self.name));
        match name {
            Some(name) => Self::weapon_tracker_from_store(name),
            None => WeaponTrackerBind::default(),
        }
    }

    /// Apply one unconditional Object INI `WeaponSet` row (PreferredAgainst +
    /// ShareWeaponReloadTime + AutoChooseSources + WeaponLockSharedAcrossSets).
    /// C++ WeaponSet.cpp parsePreferredAgainst / parseAutoChoose / parseBool.
    pub fn apply_weapon_set_definition(&mut self, set: &crate::assets::WeaponSetDefinition) {
        for (key, value) in &set.attributes {
            if key.eq_ignore_ascii_case("ShareWeaponReloadTime")
                || key.eq_ignore_ascii_case("ShareReloadTime")
            {
                self.share_weapon_reload_time = parse_ini_bool(value);
                continue;
            }
            if key.eq_ignore_ascii_case("WeaponLockSharedAcrossSets")
                || key.eq_ignore_ascii_case("ShareWeaponLock")
            {
                self.weapon_lock_shared_across_sets = parse_ini_bool(value);
                continue;
            }
            let lower = key.to_ascii_lowercase();
            if lower == "preferredagainst" || lower.starts_with("preferredagainst") {
                if let Some((slot, kinds)) = parse_preferred_against_value(value) {
                    if let Some(slot_kinds) = self.preferred_against.get_mut(slot as usize) {
                        *slot_kinds = kinds;
                    }
                }
                continue;
            }
            if lower == "autochoosesources" || lower.starts_with("autochoosesources") {
                if let Some((slot, mask)) = parse_auto_choose_value(value) {
                    if let Some(slot_mask) = self.auto_choose_masks.get_mut(slot as usize) {
                        *slot_mask = mask;
                    }
                    if slot == 0 && mask == 0 {
                        self.primary_auto_choose_none = true;
                    }
                }
            }
        }
    }

    /// Fill PreferredAgainst / ShareWeaponReloadTime from the live Object INI
    /// catalog when the template has not already authored them (tests).
    pub fn bind_weapon_set_from_live_assets(&mut self) {
        if !(self.preferred_against.iter().any(|kinds| !kinds.is_empty())
            || self.share_weapon_reload_time)
        {
            if let Some(manager) = crate::assets::get_asset_manager() {
                if let Ok(guard) = manager.lock() {
                    if let Some(definition) = guard.get_object_definition(&self.name) {
                        if let Some(set) = definition
                            .weapon_sets
                            .iter()
                            .find(|set| set.is_unconditional())
                        {
                            self.apply_weapon_set_definition(set);
                        }
                    }
                }
            }
        }
        self.apply_retail_button_only_auto_choose();
    }

    /// C++ `AutoChooseSources = SECONDARY NONE` (Jarmen snipe / Missile Defender
    /// laser / Toxin Tractor sprayer). Stamp only while the slot is still the
    /// WeaponTemplateSet::clear default so authored INI bits win.
    pub fn apply_retail_button_only_auto_choose(&mut self) {
        if self.auto_choose_masks.get(1).copied() != Some(u32::MAX) {
            return;
        }
        if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(&self.name)
            || crate::game_logic::host_missile_defender::is_missile_defender_template(&self.name)
            || crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(&self.name)
        {
            self.auto_choose_masks[1] = 0;
        }
    }

    /// C++ WeaponSet.cpp:869-877 — victim matches this slot's PreferredAgainst.

    /// C++ WeaponSet.cpp:816-822 — AutoChooseSources includes FROM_PLAYER /
    /// FROM_AI / DEFAULT_SWITCH_WEAPON. NONE (mask 0) is button-only.
    pub fn slot_allows_auto_choose(&self, slot: u8) -> bool {
        if slot == 0 && self.primary_auto_choose_none {
            return false;
        }
        let mask = self
            .auto_choose_masks
            .get(slot as usize)
            .copied()
            .unwrap_or(u32::MAX);
        // Missing INI still must not auto-pick retail button-only secondaries.
        if slot == 1
            && mask == u32::MAX
            && (crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(&self.name)
                || crate::game_logic::host_missile_defender::is_missile_defender_template(
                    &self.name,
                )
                || crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(&self.name))
        {
            return false;
        }
        const COMBAT: u32 = (1 << 0) | (1 << 2) | (1 << 4);
        (mask & COMBAT) != 0
    }
    pub fn slot_preferred_against(&self, slot: u8, target_kinds: impl Fn(KindOf) -> bool) -> bool {
        let Some(kinds) = self.preferred_against.get(slot as usize) else {
            return false;
        };
        !kinds.is_empty() && kinds.iter().copied().any(target_kinds)
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.kind_of.contains(&kind)
    }

    /// C++ Object ctor: `m_shroudClearingRange == -1` → `m_visionRange`.
    pub fn resolved_shroud_clearing_range(&self) -> f32 {
        if self.shroud_clearing_range < 0.0 {
            self.sight_range
        } else {
            self.shroud_clearing_range
        }
    }

    pub fn add_kind_of(&mut self, kind: KindOf) -> &mut Self {
        self.kind_of.insert(kind);
        self
    }

    pub fn set_health(&mut self, health: f32) -> &mut Self {
        self.max_health = health;
        self
    }

    /// Author a `RebuildHoleExposeDie` HoleName / HoleMaxHealth pair.
    pub fn set_rebuild_hole_expose(&mut self, hole_name: &str, hole_max_health: f32) -> &mut Self {
        self.rebuild_hole_expose = Some(RebuildHoleExposeDieMetadata::authored(
            hole_name,
            hole_max_health,
        ));
        self
    }

    pub fn set_cost(&mut self, supplies: u32, power: i32) -> &mut Self {
        self.build_cost = Resources { supplies, power };
        self
    }

    /// C++ `ThingTemplate::getThreatValue`. Leftover factory wins when loaded.
    pub fn get_threat_value(&self) -> u16 {
        leftover_template_threat_value(&self.name).unwrap_or(self.threat_value)
    }

    /// C++ ControlBarCommand.cpp:1119-1121 / 1175-1177 — hide for humans.
    /// `ThingTemplate::getBuildable` consults GameLogic override first.
    pub fn human_control_bar_buildable_hidden(&self) -> bool {
        let status = gamelogic::helpers::TheGameLogic::find_buildable_status_override(&self.name)
            .map(|s| s.max(0) as u32)
            .unwrap_or(self.buildable_status);
        !crate::game_logic::host_production_buildable_command_residual::buildable_status_allows_human_residual(
            status,
        )
    }

    /// C++ `ThingTemplate::parsePrerequisites` via leftover parse_prerequisites_block.
    pub fn parse_prerequisites_from_ini_lines(&mut self, lines: &[String]) {
        let mut scratch = game_engine::common::thing::thing_template::ThingTemplate::new();
        scratch.parse_prerequisites_block(lines);
        self.production_prerequisites = scratch.get_prereqs().to_vec();
    }

    /// Copy leftover-factory `m_prereqInfo` (already parsed + resolveNames).
    pub fn set_production_prerequisites(
        &mut self,
        prereqs: Vec<game_engine::common::rts::ProductionPrerequisite>,
    ) {
        self.production_prerequisites = prereqs;
    }

    pub fn set_model(&mut self, model: &str) -> &mut Self {
        self.model_name = Some(model.to_string());
        self
    }

    /// Get the model name for this template, or fall back to template name
    pub fn get_model_name(&self) -> &str {
        self.model_name.as_deref().unwrap_or(&self.name)
    }

    /// Get the W3D model filename (with .w3d extension if needed)
    pub fn get_w3d_filename(&self) -> String {
        let model_name = self.get_model_name();
        if model_name.to_lowercase().ends_with(".w3d") {
            model_name.to_string()
        } else {
            format!("{}.w3d", model_name)
        }
    }
}

/// Leftover Common ThingTemplate::getThreatValue when the factory is live.
fn leftover_template_threat_value(template_name: &str) -> Option<u16> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    Some(tmpl.get_threat_value())
}

fn default_auto_choose_masks() -> [u32; 3] {
    // C++ WeaponTemplateSet::clear: m_autoChooseMask[i] = 0xffffffff
    [u32::MAX; 3]
}

/// C++ `WeaponTemplateSet::parseAutoChoose`: slot then CommandSourceMask bits.
fn parse_auto_choose_value(value: &str) -> Option<(u8, u32)> {
    let mut tokens = value.split_whitespace();
    let first = tokens.next()?;
    let slot = match first.to_ascii_uppercase().as_str() {
        "PRIMARY" => 0u8,
        "SECONDARY" => 1,
        "TERTIARY" => 2,
        _ => return None,
    };
    let mut mask = 0u32;
    for token in tokens {
        match token.to_ascii_uppercase().as_str() {
            "NONE" => {}
            "FROM_PLAYER" => mask |= 1 << 0,
            "FROM_SCRIPT" => mask |= 1 << 1,
            "FROM_AI" => mask |= 1 << 2,
            "FROM_DOZER" => mask |= 1 << 3,
            "DEFAULT_SWITCH_WEAPON" => mask |= 1 << 4,
            _ => {}
        }
    }
    Some((slot, mask))
}

/// C++ FiringTracker fields bound from a WeaponStore template onto a host Object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponTrackerBind {
    pub continuous_fire_one_shots: u32,
    pub continuous_fire_two_shots: u32,
    pub continuous_fire_coast_frames: u32,
    pub auto_reload_when_idle_frames: u32,
}

impl Default for WeaponTrackerBind {
    fn default() -> Self {
        Self {
            continuous_fire_one_shots: u32::MAX,
            continuous_fire_two_shots: u32::MAX,
            continuous_fire_coast_frames: 0,
            auto_reload_when_idle_frames: 0,
        }
    }
}

fn shots_needed_to_host(value: i32) -> u32 {
    if value <= 0 || value == i32::MAX {
        u32::MAX
    } else {
        value as u32
    }
}

fn parse_ini_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "yes" | "true" | "1"
    )
}

fn kind_of_from_preferred_token(token: &str) -> Option<KindOf> {
    match token.trim().to_ascii_uppercase().replace('-', "_").as_str() {
        "INFANTRY" => Some(KindOf::Infantry),
        "VEHICLE" => Some(KindOf::Vehicle),
        "AIRCRAFT" => Some(KindOf::Aircraft),
        "STRUCTURE" => Some(KindOf::Structure),
        "PROJECTILE" => Some(KindOf::Projectile),
        "BALLISTIC_MISSILE" => Some(KindOf::BallisticMissile),
        "SMALL_MISSILE" => Some(KindOf::SmallMissile),
        "MINE" => Some(KindOf::Mine),
        "DEMOTRAP" => Some(KindOf::DemoTrap),
        "PARACHUTE" => Some(KindOf::Parachute),
        _ => None,
    }
}

/// C++ `WeaponTemplateSet::parsePreferredAgainst`: first token is the slot.
fn parse_preferred_against_value(value: &str) -> Option<(u8, Vec<KindOf>)> {
    let mut tokens = value.split_whitespace();
    let first = tokens.next()?;
    let (slot, leftover) = match first.to_ascii_uppercase().as_str() {
        "PRIMARY" => (0u8, None),
        "SECONDARY" => (1, None),
        "TERTIARY" => (2, None),
        _ => (0, Some(first)),
    };
    let mut kinds = Vec::new();
    if let Some(token) = leftover {
        if let Some(kind) = kind_of_from_preferred_token(token) {
            kinds.push(kind);
        }
    }
    for token in tokens {
        if let Some(kind) = kind_of_from_preferred_token(token) {
            kinds.push(kind);
        }
    }
    if kinds.is_empty() {
        None
    } else {
        Some((slot, kinds))
    }
}

fn default_template_shroud_clearing_range() -> f32 {
    // C++ ThingTemplate m_shroudClearingRange default -1 → use VisionRange.
    -1.0
}

fn default_template_skill_point_values() -> [i32; 4] {
    [crate::game_logic::host_rank_ui_residual::USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL; 4]
}

fn default_template_shroud_reveal_to_all_range() -> f32 {
    // C++ ThingTemplate m_shroudRevealToAllRange default -1.
    -1.0
}

fn default_template_crushable_level() -> u8 {
    // C++ ThingTemplate.cpp:1024 m_crushableLevel = 255 (uncrushable).
    255
}

fn default_asset_scale() -> f32 {
    1.0
}

fn default_stealth_friendly_opacity_min() -> f32 {
    0.5
}

fn default_stealth_friendly_opacity_max() -> f32 {
    1.0
}

fn default_template_physics_mass() -> f32 {
    1.0
}

fn default_template_pitch_roll_yaw_factor() -> f32 {
    2.0
}

fn default_template_forward_friction() -> f32 {
    0.15
}
fn default_template_lateral_friction() -> f32 {
    0.15
}
fn default_template_z_friction() -> f32 {
    0.8
}
fn default_template_allow_collide_force() -> bool {
    true
}
fn default_template_min_fall_speed() -> f32 {
    // Leftover height_to_speed(40) with retail Gravity -64/900 (~2.385).
    (2.0 * (64.0_f32 / 900.0) * 40.0).sqrt()
}
fn default_template_fall_height_damage_factor() -> f32 {
    1.0
}

#[cfg(test)]
mod weapon_resolve_tests {
    use super::*;

    #[test]
    fn explicit_primary_weapon_beats_store_and_default() {
        let mut t = ThingTemplate::new("Armed");
        t.add_kind_of(KindOf::Infantry);
        t.set_primary_weapon(Weapon {
            damage: 40.0,
            range: 80.0,
            reload_time: 0.5,
            ..Weapon::default()
        });
        t.set_primary_weapon_name("DoesNotExistInStoreHopefully");
        let w = t.resolve_primary_weapon().expect("weapon");
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.range - 80.0).abs() < 0.01);
    }

    #[test]
    fn infantry_without_weapon_gets_kind_fallback() {
        let mut t = ThingTemplate::new("BareInfantry");
        t.add_kind_of(KindOf::Infantry);
        let w = t.resolve_primary_weapon().expect("fallback");
        assert!((w.damage - Weapon::default().damage).abs() < 0.01);
    }

    #[test]
    fn structure_without_weapon_stays_unarmed() {
        let mut t = ThingTemplate::new("BareStructure");
        t.add_kind_of(KindOf::Structure);
        assert!(t.resolve_primary_weapon().is_none());
    }

    #[test]
    fn primary_weapon_name_resolves_non_default_store_stats() {
        // Prove store bind path for USA_Ranger / GoldenRanger weapon name.
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_primary_weapon_name(super::super::weapon_bootstrap::RANGER_PRIMARY_WEAPON);
        let w = t.resolve_primary_weapon().expect("store-bound weapon");
        assert!(
            (w.damage - Weapon::default().damage).abs() > 0.01,
            "store path must not yield host default damage; got {}",
            w.damage
        );
        assert!((w.damage - 5.0).abs() < 0.01);
        assert!((w.range - 97.5).abs() < 0.01);
    }

    #[test]
    fn secondary_weapon_name_resolves_non_default_store_stats() {
        // Prove SECONDARY store bind path (Ranger flashbang residual).
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_secondary_weapon_name(super::super::weapon_bootstrap::RANGER_SECONDARY_WEAPON);
        let w = t.resolve_secondary_weapon().expect("store-bound secondary");
        assert!(
            (w.damage - Weapon::default().damage).abs() > 0.01,
            "secondary store path must not yield host default damage; got {}",
            w.damage
        );
        // Retail RangerFlashBangGrenadeWeapon PrimaryDamage 35, AttackRange 175.
        // Leftover get_attack_range undersize −¼ cell → 172.5.
        assert!((w.damage - 35.0).abs() < 0.01);
        assert!((w.range - 172.5).abs() < 0.01);
    }

    #[test]
    fn secondary_without_name_stays_none_even_for_infantry() {
        // Fail-closed: no kind-based default for secondary slots.
        let mut t = ThingTemplate::new("BareInfantry");
        t.add_kind_of(KindOf::Infantry);
        assert!(t.resolve_secondary_weapon().is_none());
    }

    #[test]
    fn unit_name_residual_map_binds_without_explicit_weapon_name() {
        // units.rs / setup_templates often omit primary_weapon_name; residual map
        // must still prefer retail store stats over kind-based Weapon::default.
        let mut technical = ThingTemplate::new("GLA_Technical");
        technical
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        let tw = technical
            .resolve_primary_weapon()
            .expect("technical residual weapon");
        assert!(
            (tw.damage - Weapon::default().damage).abs() > 0.01,
            "GLA_Technical must not fall through to Weapon::default (got dmg={})",
            tw.damage
        );
        // Retail TechnicalMachineGunWeapon PrimaryDamage 10.
        assert!((tw.damage - 10.0).abs() < 0.01);
        assert!((tw.range - 147.5).abs() < 0.01);

        let mut battle = ThingTemplate::new("China_BattleTank");
        battle
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        let bw = battle
            .resolve_primary_weapon()
            .expect("battlemaster residual weapon");
        assert!(
            (bw.damage - Weapon::default().damage).abs() > 0.01,
            "China_BattleTank must not fall through to Weapon::default (got dmg={})",
            bw.damage
        );
        // Retail BattleMasterTankGun PrimaryDamage 60.
        assert!((bw.damage - 60.0).abs() < 0.01);
        assert!((bw.range - 147.5).abs() < 0.01);
    }

    #[test]
    fn secondary_unit_name_residual_map_binds_ranger_flashbang() {
        let mut t = ThingTemplate::new("AmericaInfantryColonelBurton");
        t.set_secondary_weapon(Weapon {
            damage: 99.0,
            range: 50.0,
            reload_time: 1.0,
            ..Weapon::default()
        });
        t.set_secondary_weapon_name("DoesNotExistInStoreHopefully");
        let w = t.resolve_secondary_weapon().expect("weapon");
        assert!((w.damage - 99.0).abs() < 0.01);
        assert!((w.range - 50.0).abs() < 0.01);
    }

    #[test]
    fn preferred_against_ini_parses_slot_and_kinds() {
        // C++ WeaponSet.cpp:119-122 parsePreferredAgainst: slot then KindOf list.
        let (slot, kinds) = parse_preferred_against_value("PRIMARY INFANTRY").unwrap();
        assert_eq!(slot, 0);
        assert_eq!(kinds, vec![KindOf::Infantry]);
        let (slot, kinds) =
            parse_preferred_against_value("SECONDARY AIRCRAFT BALLISTIC_MISSILE").unwrap();
        assert_eq!(slot, 1);
        assert_eq!(kinds, vec![KindOf::Aircraft, KindOf::BallisticMissile]);
    }

    #[test]
    fn apply_weapon_set_definition_binds_preferred_and_share_reload() {
        let mut set = crate::assets::WeaponSetDefinition::default();
        set.attributes.insert(
            "PreferredAgainst".to_string(),
            "PRIMARY INFANTRY".to_string(),
        );
        set.attributes
            .insert("ShareWeaponReloadTime".to_string(), "Yes".to_string());
        set.attributes.insert(
            "AutoChooseSources".to_string(),
            "SECONDARY NONE".to_string(),
        );
        set.attributes
            .insert("WeaponLockSharedAcrossSets".to_string(), "No".to_string());
        let mut t = ThingTemplate::new("AmericaVehicleComanche");
        t.apply_weapon_set_definition(&set);
        assert_eq!(t.preferred_against[0], vec![KindOf::Infantry]);
        assert!(t.share_weapon_reload_time);
        assert!(t.slot_preferred_against(0, |k| k == KindOf::Infantry));
        assert!(!t.slot_preferred_against(0, |k| k == KindOf::Vehicle));
        assert_eq!(t.auto_choose_masks[1], 0);
        assert!(t.slot_allows_auto_choose(0));
        assert!(!t.slot_allows_auto_choose(1));
        assert!(!t.weapon_lock_shared_across_sets);
    }

    #[test]
    fn weapon_tracker_from_store_maps_int_max_to_unbound() {
        // C++ WeaponTemplate defaults ContinuousFireOne/Two to INT_MAX (off).
        let bind = ThingTemplate::weapon_tracker_from_store(
            super::super::weapon_bootstrap::RANGER_PRIMARY_WEAPON,
        );
        assert_eq!(bind.continuous_fire_one_shots, u32::MAX);
        assert_eq!(bind.continuous_fire_two_shots, u32::MAX);
    }

    #[test]
    fn pack_unpack_variation_matches_cpp_inclusive_range() {
        // SpecialAbilityUpdate.cpp:721/774 GameLogicRandomValueReal(1-f, 1+f).
        assert_eq!(
            apply_pack_unpack_variation_ms(5500, pack_unpack_variation_multiplier(0.2, 0.0)),
            4400
        );
        assert_eq!(
            apply_pack_unpack_variation_ms(5500, pack_unpack_variation_multiplier(0.2, 1.0)),
            6600
        );
        assert_eq!(
            apply_pack_unpack_variation_ms(5500, pack_unpack_variation_multiplier(0.0, 0.37)),
            5500
        );
        assert_eq!(vary_pack_unpack_duration_ms(0, 0.5), 0);
        assert_eq!(vary_pack_unpack_duration_ms(5500, 0.0), 5500);
    }

    #[test]
    fn weapon_from_store_uses_leftover_delay_not_max_flatten() {
        // Old flatten treated max as clip and used max(min,max) when clip_size==0.
        // Leftover get_delay_between_shots yardstick is Min (Weapon.cpp:475-490).
        const NAME: &str = "__RustLiveDelayBetweenShotsRange";
        let _ = super::super::weapon_bootstrap::ensure_host_weapon_store();
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
            template.primary_damage = 10.0;
            template.attack_range = 100.0;
            template.min_delay_between_shots = 6;
            template.max_delay_between_shots = 30;
            template.clip_size = 0;
            store.add_weapon_template(template);
        });
        let weapon = ThingTemplate::weapon_from_store(NAME).expect("store weapon");
        let leftover = {
            let mut yardstick = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
            yardstick.min_delay_between_shots = 6;
            yardstick.max_delay_between_shots = 6;
            yardstick.get_delay_between_shots(&gamelogic::weapon::WeaponBonus::new())
        };
        assert_eq!(leftover, 6);
        assert!(
            (weapon.reload_time - leftover as f32 / 30.0).abs() < 1e-6,
            "reload_time={} leftover_min={} flattened_max={}",
            weapon.reload_time,
            leftover as f32 / 30.0,
            30.0 / 30.0
        );
        let rof = 2.0;
        let with_rof =
            super::super::weapon_bootstrap::host_delay_between_shots_secs_nominal_with_rof(
                NAME, rof,
            )
            .expect("leftover ROF yardstick");
        // leftover REAL_TO_INT_FLOOR(6 / 2) = 3 frames → 0.1s, not (6/30)/2.
        assert!((with_rof - 3.0 / 30.0).abs() < 1e-6, "with_rof={with_rof}");
    }

    #[test]
    fn weapon_from_store_uses_leftover_rationalize_attack_range() {
        // Old flatten copied raw AttackRange / MinimumAttackRange.
        // Leftover get_attack_range / get_minimum_attack_range undersize −¼ cell.
        const NAME: &str = "__RustLiveRationalizeAttackRange";
        let _ = super::super::weapon_bootstrap::ensure_host_weapon_store();
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
            template.primary_damage = 10.0;
            template.attack_range = 100.0;
            template.minimum_attack_range = 10.0;
            store.add_weapon_template(template);
        });
        let weapon = ThingTemplate::weapon_from_store(NAME).expect("store weapon");
        let leftover = {
            let mut yardstick = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
            yardstick.attack_range = 100.0;
            yardstick.minimum_attack_range = 10.0;
            (
                yardstick.get_attack_range(&gamelogic::weapon::WeaponBonus::new()),
                yardstick.get_minimum_attack_range(),
                yardstick.is_contact_weapon(),
            )
        };
        assert!(
            (leftover.0 - 97.5).abs() < 1e-6,
            "leftover max={}",
            leftover.0
        );
        assert!(
            (leftover.1 - 7.5).abs() < 1e-6,
            "leftover min={}",
            leftover.1
        );
        assert!(!leftover.2);
        assert!(
            (weapon.range - leftover.0).abs() < 1e-6,
            "range={} leftover={} raw=100",
            weapon.range,
            leftover.0
        );
        assert!(
            (weapon.min_range - leftover.1).abs() < 1e-6,
            "min_range={} leftover={} raw=10",
            weapon.min_range,
            leftover.1
        );
    }

    #[test]
    fn leftover_is_contact_weapon_authored_range_under_12_5() {
        let mut contact = gamelogic::weapon::WeaponTemplate::new("c".into());
        contact.attack_range = 10.0;
        assert!(contact.is_contact_weapon());
        let mut edge = gamelogic::weapon::WeaponTemplate::new("e".into());
        edge.attack_range = 12.5;
        assert!(!edge.is_contact_weapon());
        assert!(super::super::weapon_bootstrap::is_contact_weapon_range(
            10.0
        ));
        assert!(!super::super::weapon_bootstrap::is_contact_weapon_range(
            12.5
        ));

        // Authored 10: leftover/C++ is contact (7.5 < 10); #else FUDGE was not.
        const CONTACT: &str = "__RustLiveContactAuthored10";
        const EDGE: &str = "__RustLiveContactAuthored12_5";
        let _ = super::super::weapon_bootstrap::ensure_host_weapon_store();
        let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
            let mut t = gamelogic::weapon::WeaponTemplate::new(CONTACT.to_string());
            t.primary_damage = 10.0;
            t.attack_range = 10.0;
            store.add_weapon_template(t);
            let mut t = gamelogic::weapon::WeaponTemplate::new(EDGE.to_string());
            t.primary_damage = 10.0;
            t.attack_range = 12.5;
            store.add_weapon_template(t);
        });
        assert!(super::super::weapon_bootstrap::host_is_contact_weapon_name(
            CONTACT
        ));
        assert!(!super::super::weapon_bootstrap::host_is_contact_weapon_name(EDGE));
        let w = ThingTemplate::weapon_from_store(CONTACT).expect("contact store");
        assert!((w.range - 7.5).abs() < 1e-6, "range={}", w.range);
        assert!(super::super::weapon_bootstrap::is_contact_effective_range(
            w.range
        ));
        let edge_w = ThingTemplate::weapon_from_store(EDGE).expect("edge store");
        assert!(
            (edge_w.range - 10.0).abs() < 1e-6,
            "edge range={}",
            edge_w.range
        );
        assert!(!super::super::weapon_bootstrap::is_contact_effective_range(
            edge_w.range
        ));
    }
}

/// Base Thing class - common functionality for all game entities
#[derive(Debug, Serialize, Deserialize)]
pub struct Thing {
    pub template: ThingTemplate,
    pub geometry: GeometryInfo,
    pub transform: Mat4,

    // Cached values for performance
    cached_position: Vec3,
    cached_angle: f32,
    cached_dir_vector: Vec3,
    cache_valid: bool,
}

impl Thing {
    pub fn new(template: ThingTemplate) -> Self {
        let geometry = template.geometry_info.to_host_geometry();
        let mut thing = Self {
            template,
            geometry,
            transform: Mat4::IDENTITY,
            cached_position: Vec3::ZERO,
            cached_angle: 0.0,
            cached_dir_vector: Vec3::X,
            cache_valid: false,
        };
        thing.update_cache();
        thing
    }

    pub fn get_template(&self) -> &ThingTemplate {
        &self.template
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.template.is_kind_of(kind)
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.geometry.position = position;
        self.transform =
            Mat4::from_translation(position) * Mat4::from_rotation_y(self.cached_angle);
        self.update_cache();
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.cached_angle = angle;
        self.transform =
            Mat4::from_translation(self.cached_position) * Mat4::from_rotation_y(angle);
        self.update_cache();
    }

    pub fn get_position(&self) -> Vec3 {
        self.cached_position
    }

    pub fn get_orientation(&self) -> f32 {
        self.cached_angle
    }

    pub fn get_direction_vector(&self) -> Vec3 {
        self.cached_dir_vector
    }

    pub fn set_transform_matrix(&mut self, transform: Mat4) {
        self.transform = transform;
        self.update_cache();
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.transform
    }

    fn update_cache(&mut self) {
        // Extract position from transform matrix
        let translation = self.transform.w_axis.truncate();
        self.cached_position = translation;

        // Extract yaw from the facing basis. Host movement / aim residual uses
        // forward = (cos θ, 0, -sin θ), which is the X column of from_rotation_y(θ).
        // (Previously used Z column, which shifted θ by -π/2 and broke aim checks.)
        let forward = self.transform.x_axis.truncate();
        self.cached_angle = (-forward.z).atan2(forward.x);

        // Calculate direction vector
        self.cached_dir_vector = Vec3::new(self.cached_angle.cos(), 0.0, -self.cached_angle.sin());

        // Update geometry position
        self.geometry.position = self.cached_position;
        self.geometry.rotation = self.cached_angle;

        self.cache_valid = true;
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        (self.transform * point.extend(1.0)).truncate()
    }

    pub fn get_distance_to(&self, other: &Thing) -> f32 {
        self.cached_position.distance(other.cached_position)
    }

    pub fn get_distance_to_position(&self, position: Vec3) -> f32 {
        self.cached_position.distance(position)
    }

    pub fn is_within_range(&self, other: &Thing, range: f32) -> bool {
        self.get_distance_to(other) <= range
    }

    pub fn get_bounds(&self) -> (Vec3, Vec3) {
        let half_size = Vec3::splat(self.geometry.radius);
        (
            self.cached_position - half_size,
            self.cached_position + half_size,
        )
    }

    pub fn intersects_bounds(&self, other: &Thing) -> bool {
        let (min_a, max_a) = self.get_bounds();
        let (min_b, max_b) = other.get_bounds();

        max_a.x >= min_b.x
            && min_a.x <= max_b.x
            && max_a.y >= min_b.y
            && min_a.y <= max_b.y
            && max_a.z >= min_b.z
            && min_a.z <= max_b.z
    }
}

impl Clone for Thing {
    fn clone(&self) -> Self {
        Self {
            template: self.template.clone(),
            geometry: self.geometry.clone(),
            transform: self.transform,
            cached_position: self.cached_position,
            cached_angle: self.cached_angle,
            cached_dir_vector: self.cached_dir_vector,
            cache_valid: self.cache_valid,
        }
    }
}
