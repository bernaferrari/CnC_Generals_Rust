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
}

/// The subset of C++ `AllowInsideKindOf`/`ForbidInsideKindOf` that the active
/// Rust object model can represent without guessing.  An unrepresentable mask
/// is deliberately `Unsupported`, which prevents the physical Enter path from
/// advertising or accepting a container it cannot validate faithfully.
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
    /// GarrisonContain. `Some(0)` is an authored zero-capacity module and must
    /// remain distinct from no contain module.
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
    /// C++ `m_turretsMustCenterBeforePacking`.  The host has no faithful
    /// per-weapon turret/animation binding here, so this is retained without
    /// manufacturing a guessed recenter duration.
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

/// Authored `SupplyTruckAIUpdate` timing and capacity data. This remains
/// absent for ordinary Harvesters: C++ exposes the collector state machine
/// only when the unit owns that exact update module.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SupplyTruckMetadata {
    pub max_boxes: u32,
    pub warehouse_scan_distance: f32,
    pub warehouse_delay_frames: u32,
    pub center_delay_frames: u32,
}

/// Compact runtime mirror of the C++ supply-truck Wanting/dock state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SupplyTruckState {
    #[default]
    Idle,
    Wanting,
    DockingWarehouse,
    DockingCenter,
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
    /// C++ Queue `m_allowAirborneCreationData`.  It is retained for snapshot
    /// parity; the bounded ground producer path does not invent airborne
    /// velocity/layer physics from this bit.
    pub allow_airborne_creation: bool,
    /// C++ Queue `m_initialBurst`.  The runtime counter is initialized once
    /// per producer Object from this template value.
    pub initial_burst: u32,
    /// C++ Default `m_useSpawnRallyPoint`.  This is retained for the separate
    /// spawn/parachute path; ordinary unit production always follows its
    /// authored natural/custom exit route.
    pub use_spawn_rally_point: bool,
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
    /// `SpecialAbilityUpdate::PersistenceRequiresRecharge`.
    pub persistence_requires_recharge: bool,
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

/// Thing Template - shared configuration data for Things
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThingTemplate {
    pub name: String,
    pub display_name: String,
    pub kind_of: HashSet<KindOf>,
    pub max_health: f32,
    pub armor: f32,
    pub sight_range: f32,
    pub build_cost: Resources,
    pub build_time: f32,
    /// C++ `ThingTemplate::m_refundValue` from Object INI `RefundValue`.
    /// A zero value means "use BuildCost × GlobalData::SellPercentage";
    /// a non-zero value is an exact sale refund.
    #[serde(default)]
    pub refund_value: u16,
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
    /// `RailedTransportContain::Slots`, when that exact contain module is
    /// present.  A railed dock with no contain module never gains synthetic
    /// transport capacity.
    #[serde(default)]
    pub railed_transport_slots: Option<usize>,
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
    pub special_power_cooldown: f32,
    /// C++ parity: XP awarded to the killer when this object is destroyed.
    /// In C++ this is per-veterancy-level; here we store the Rookie-level
    /// value and scale by veterancy level at kill time.
    pub experience_value: f32,
    /// C++ parity (Object::ExperienceValues): per-template veterancy XP
    /// thresholds [Veteran, Elite, Heroic].  Defaults to [60, 150, 300].
    pub veterancy_xp_thresholds: [f32; 3],
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
    /// Fail-closed residual: single primary locomotor only (not multi-set / surface matrix).
    pub locomotor_name: Option<String>,
    /// C++ CreateCrateDieModuleData::m_crateNameList residual (CrateData names).
    #[serde(default)]
    pub create_crate_data: Vec<String>,
}

impl ThingTemplate {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: name.to_string(),
            kind_of: HashSet::new(),
            max_health: 100.0,
            armor: 0.0,
            sight_range: 150.0,
            build_cost: Resources::default(),
            build_time: 1.0,
            refund_value: 0,
            model_name: None,
            texture_name: None,
            asset_scale: default_asset_scale(),
            dock_kind: DockKind::None,
            dock_starting_boxes: None,
            dock_delete_when_empty: false,
            supply_truck_metadata: None,
            railed_transport_slots: None,
            contain_module: ContainModuleMetadata::default(),
            stealth_friendly_opacity_min: default_stealth_friendly_opacity_min(),
            stealth_friendly_opacity_max: default_stealth_friendly_opacity_max(),
            parking_place: None,
            deploy_style_metadata: None,
            production_exit_metadata: None,
            veterancy_crate_collide: None,
            eject_pilot_die: None,
            hack_internet_ai_update: None,
            hacker_disable_building: None,
            special_power_modules: Vec::new(),
            energy_production: None,
            max_simultaneous_link_key: None,
            max_simultaneous_determined_by_superweapon_restriction: false,
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
            special_power_cooldown: 10.0,
            experience_value: 0.0,
            veterancy_xp_thresholds: [60.0, 150.0, 300.0],
            primary_weapon: None,
            primary_weapon_name: None,
            primary_weapon_explicitly_none: false,
            mine_clearing_primary_weapon: None,
            mine_clearing_primary_weapon_name: None,
            secondary_weapon: None,
            secondary_weapon_name: None,
            tertiary_weapon: None,
            tertiary_weapon_name: None,
            fire_weapon_when_damaged_behaviors: Vec::new(),
            fire_weapon_when_dead_behaviors: Vec::new(),
            locomotor_name: None,
            create_crate_data: Vec::new(),
        }
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
        if self.primary_weapon_explicitly_none {
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
        use gamelogic::weapon::{with_weapon_store, WeaponAntiMask};
        const FPS: f32 = 30.0;
        let wt = with_weapon_store(|store| store.find_weapon_template(name).cloned()).ok()??;
        if wt.primary_damage <= 0.0 || wt.attack_range <= 0.0 {
            return None;
        }
        let between_frames = wt.min_delay_between_shots.max(0) as f32;
        let clip_frames = wt.max_delay_between_shots.max(0) as f32;
        let delay_frames = if wt.clip_size > 0 {
            // Within-clip cadence residual (C++ DelayBetweenShots).
            if between_frames > 0.0 {
                between_frames
            } else {
                clip_frames
            }
        } else {
            between_frames.max(clip_frames)
        };
        let reload_time = if delay_frames > 0.0 {
            delay_frames / FPS
        } else {
            1.0
        };
        let pre_attack_delay = (wt.pre_attack_delay.max(0) as f32) / FPS;
        let projectile_speed = if wt.weapon_speed >= 999_999.0 {
            0.0
        } else {
            wt.weapon_speed
        };
        Some(Weapon {
            damage: wt.primary_damage,
            range: wt.attack_range,
            min_range: wt.minimum_attack_range.max(0.0),
            reload_time,
            last_fire_time: 0.0,
            ammo: if wt.clip_size > 0 {
                Some(wt.clip_size as u32)
            } else {
                None
            },
            clip_size: wt.clip_size.max(0) as u32,
            // Clip reload residual: store often encodes clip reload as max delay.
            clip_reload_time: if wt.clip_size > 0 {
                (wt.max_delay_between_shots.max(0) as f32) / 30.0
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
        })
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.kind_of.contains(&kind)
    }

    pub fn add_kind_of(&mut self, kind: KindOf) -> &mut Self {
        self.kind_of.insert(kind);
        self
    }

    pub fn set_health(&mut self, health: f32) -> &mut Self {
        self.max_health = health;
        self
    }

    pub fn set_cost(&mut self, supplies: u32, power: i32) -> &mut Self {
        self.build_cost = Resources { supplies, power };
        self
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

fn default_asset_scale() -> f32 {
    1.0
}

fn default_stealth_friendly_opacity_min() -> f32 {
    0.5
}

fn default_stealth_friendly_opacity_max() -> f32 {
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
        assert!((w.range - 100.0).abs() < 0.01);
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
        assert!((w.damage - 35.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
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
        assert!((tw.range - 150.0).abs() < 0.01);

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
        assert!((bw.range - 150.0).abs() < 0.01);
    }

    #[test]
    fn secondary_unit_name_residual_map_binds_ranger_flashbang() {
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable);
        // No secondary_weapon_name set — residual map by template name.
        let w = t
            .resolve_secondary_weapon()
            .expect("ranger residual secondary");
        assert!((w.damage - 35.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
    }

    #[test]
    fn explicit_secondary_weapon_beats_store() {
        let mut t = ThingTemplate::new("Armed");
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
        let mut thing = Self {
            template,
            geometry: GeometryInfo::default(),
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
