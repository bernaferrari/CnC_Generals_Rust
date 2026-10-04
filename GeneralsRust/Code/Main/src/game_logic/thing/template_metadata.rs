use super::*;

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
    /// C++ TransportContainModuleData::m_resetMoodCheckTimeOnExit (default true).
    #[serde(default = "default_reset_mood_check_time_on_exit")]
    pub reset_mood_check_time_on_exit: bool,
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

const fn default_reset_mood_check_time_on_exit() -> bool {
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
            reset_mood_check_time_on_exit: true,
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
