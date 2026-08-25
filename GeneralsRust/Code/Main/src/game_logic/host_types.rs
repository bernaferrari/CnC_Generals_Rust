//! Core host gameplay types (`ObjectId`, `Team`, `KindOf`, `Health`, …).
//!
//! Re-exported from `crate::game_logic` so public paths stay stable.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Unique identifier for game objects
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct ObjectId(pub u32);

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectId = ObjectId(0);

/// Team/faction identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    GLA,
    USA,
    China,
    Neutral,
}

impl Team {
    /// Convert player ID to team
    pub fn from_player_id(player_id: u32) -> Self {
        match player_id {
            0 => Team::USA,
            1 => Team::China,
            2 => Team::GLA,
            _ => Team::Neutral,
        }
    }

    /// Get the team's primary color for UI display
    pub fn get_color(&self) -> [f32; 4] {
        match self {
            Team::USA => [0.2, 0.4, 0.8, 1.0],     // Blue
            Team::China => [0.8, 0.2, 0.2, 1.0],   // Red
            Team::GLA => [0.8, 0.6, 0.2, 1.0],     // Desert/Tan
            Team::Neutral => [0.5, 0.5, 0.5, 1.0], // Gray
        }
    }

    /// Get the team's name as a string
    pub fn get_name(&self) -> &'static str {
        match self {
            Team::USA => "USA",
            Team::China => "China",
            Team::GLA => "GLA",
            Team::Neutral => "Neutral",
        }
    }

    /// Get the team's secondary color for highlights
    pub fn get_highlight_color(&self) -> [f32; 4] {
        match self {
            Team::USA => [0.4, 0.6, 1.0, 1.0],     // Light blue
            Team::China => [1.0, 0.4, 0.4, 1.0],   // Light red
            Team::GLA => [1.0, 0.8, 0.4, 1.0],     // Light tan
            Team::Neutral => [0.7, 0.7, 0.7, 1.0], // Light gray
        }
    }
}

/// Object kinds for type checking and behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KindOf {
    Structure,
    Infantry,
    Vehicle,
    Aircraft,
    Projectile,
    Resource,
    Selectable,
    Attackable,
    CommandCenter,
    Worker,
    Hero,
    SupplyCenter,
    PowerPlant,
    FSBarracks,
    FSWarFactory,
    FSAirfield,
    FSInternetCenter,
    FSPower,
    FSBaseDefense,
    FSSupplyDropzone,
    FSSupplyCenter,
    FSSuperweapon,
    FSStrategyCenter,
    FSFake,
    FSTechnology,
    FSBlackMarket,
    FSAdvancedTech,
    Harvestable,
    /// C++ KINDOF_POWERED: object gets DISABLED_UNDERPOWERED when player
    /// power consumption exceeds supply (defenses, factories, etc).
    Powered,
    /// C++ KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT — fire gated by attack view LOS.
    AttackNeedsLineOfSight,
    /// C++ KINDOF_IMMOBILE — structures/defenses; skip terrain LOS detour residual.
    Immobile,
    /// C++ KINDOF_CAN_BE_REPULSED — civilians flee OBJECT_STATUS_REPULSOR.
    CanBeRepulsed,
    /// C++ KINDOF_CANNOT_RETALIATE — excluded from friend retaliation.
    CannotRetaliate,
    /// C++ KINDOF_DRONE — drones never retaliate.
    Drone,
    /// C++ KINDOF_IGNORED_IN_GUI — mouseover remaps to slaver/producer.
    IgnoredInGui,
    /// C++ KINDOF_SALVAGER — GLA salvage crate pickers.
    Salvager,
    /// C++ KINDOF_WEAPON_SALVAGER — can gain weapon crate upgrades.
    WeaponSalvager,
    /// C++ KINDOF_ARMOR_SALVAGER — can gain armor crate upgrades.
    ArmorSalvager,
    /// C++ KINDOF_AIRCRAFT_PATH_AROUND — tall buildings aircraft path around.
    AircraftPathAround,
    /// C++ KINDOF_WAVEGUIDE — flood wave objects enabled by DamDie.
    WaveGuide,
    /// C++ `KINDOF_DOZER` (KindOfType ordinal 12).  Kept distinct from
    /// `Worker`: a builder is not necessarily a resource collector.
    Dozer,
    /// C++ `KINDOF_HARVESTER` (KindOfType ordinal 13).  Retail Chinooks,
    /// Supply Trucks, and GLA Workers carry this capability and may gather
    /// supplies; it is not the same as a `Harvestable` supply source.
    Harvester,
    /// C++ `KINDOF_UNATTACKABLE`: an object may exist for lifetime, vision,
    /// or presentation while never being a legal WeaponSet target (bridges,
    /// props, parachutes, and special objects use it heavily in retail INIs).
    /// The compact presentation bit bank is full; its distinct targetability
    /// meaning is carried through the dedicated presentation/shadow boolean
    /// instead of changing any established bit positions.
    Unattackable,
    /// C++ `KINDOF_MINE`: a mine is targetable by either AntiMine or
    /// AntiGround weapons while retaining its own targeting category.
    Mine,
    /// C++ `KINDOF_DEMOTRAP`, which shares the mine target anti-mask.
    DemoTrap,
    /// C++ `KINDOF_SMALL_MISSILE`: the most specific WeaponSet victim mask.
    SmallMissile,
    /// C++ `KINDOF_BALLISTIC_MISSILE`: distinct from normal projectiles for
    /// PointDefenseLaser and WeaponSet targeting.
    BallisticMissile,
    /// C++ `KINDOF_PARACHUTE`, used when an airborne parachute is a legal
    /// AntiParachute target.
    Parachute,
    /// C++ `KINDOF_DISGUISER`.  This is separate from the temporary
    /// DISGUISED object status: WeaponSet grants the force-attack stealth
    /// exception only to objects whose authored template carries this bit.
    Disguiser,
    /// C++ `KINDOF_REPAIR_PAD`.  `ActionManager::canGetRepairedAt` uses this
    /// authored capability for ground vehicles; it is not inferred from a
    /// producer/building basename or a `RepairDockUpdate` approximation.
    RepairPad,
    /// C++ `KINDOF_HEAL_PAD`.  `ActionManager::canGetHealedAt` uses this
    /// authored capability for infantry; a medical-looking template without
    /// this source tag must not become a service destination.
    HealPad,
    /// C++ `KINDOF_MONEY_HACKER`.  `InternetHackContain` admits this exact
    /// source category; a template name containing "hacker" must never
    /// fabricate either the Hacker command or Internet Center access.
    ///
    /// This is gameplay-only metadata.  It intentionally has no presentation
    /// bit assignment because that packed 32-bit order is already full.
    MoneyHacker,
    /// C++ `KINDOF_SUPPLY_SOURCE`.  Retail SupplyDock/SupplyPile objects
    /// advertise this explicitly; a template spelling containing "supply"
    /// must not create a gather target or construction-exclusion source.
    ///
    /// This stays outside the compact presentation KindOf bank.  Parsed
    /// supply sources also retain the existing Resource/Harvestable bridge
    /// bits used by the host gather UI, while this bit preserves the exact
    /// C++ distinction for supply-source-only authority.
    SupplySource,
    /// C++ `KINDOF_CANNOT_BUILD_NEAR_SUPPLIES`.  BuildAssistant applies its
    /// expanded-footprint check only when the *requested building* has this
    /// authored capability; SupplyCenter-like spelling is not sufficient.
    ///
    /// Gameplay-only and append-only for the same compact-presentation-bank
    /// reason as [`Self::SupplySource`].
    CannotBuildNearSupplies,
    /// C++ `KINDOF_CRATE`. Gameplay-only and append-only: the compact
    /// presentation KindOf bank is already full, so crate identity is also
    /// frozen as dedicated presentation booleans for physical click routing.
    Crate,
    /// C++ `KINDOF_IGNORES_SELECT_ALL` — excluded from SELECT_ALL / matching.
    IgnoresSelectAll,
    /// C++ `KINDOF_ALWAYS_SELECTABLE` (KindOf.h:86). Never unselectable,
    /// even if effectively dead — UI feedback objects / rubble clicks.
    AlwaysSelectable,
    /// C++ `KINDOF_MP_COUNT_FOR_VICTORY` (KindOf.h:63). Radar/EVA/victory
    /// count this bit, not an FS-kind union.
    MpCountForVictory,
    /// C++ `KINDOF_SCORE` (KindOf.h:65). Multiplayer score + short-game.
    Score,
    /// C++ `KINDOF_SCORE_CREATE` (KindOf.h:66).
    ScoreCreate,
    /// C++ `KINDOF_SCORE_DESTROY` (KindOf.h:67).
    ScoreDestroy,
    /// C++ `KINDOF_CAN_SEE_THROUGH` — transparent obstacles skipped in attack LOS.
    CanSeeThrough,
    /// C++ `KINDOF_NO_GARRISON`. Heroes / special infantry cannot enter GarrisonContain.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    NoGarrison,
    /// C++ `KINDOF_GARRISONABLE_UNTIL_DESTROYED`. Firebase/bunker stays occupied
    /// through BODY_REALLYDAMAGED until death.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    GarrisonableUntilDestroyed,
    /// C++ `KINDOF_BOAT`. Hijack / ConvertToCarBomb reject boats.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Boat,
    /// C++ `KINDOF_TRANSPORT`. Occupied transports cannot be hijacked.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Transport,
    /// C++ `KINDOF_IMMUNE_TO_CAPTURE` (Patch 1.03 battle bus).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    ImmuneToCapture,
    /// C++ `KINDOF_DEFENSIVE_WALL`. FenceWidth objects with this bit stay solid walls.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    DefensiveWall,
    /// C++ `KINDOF_WALK_ON_TOP_OF_WALL`. Infantry path on these structure decks.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    WalkOnTopOfWall,
    /// C++ `KINDOF_BRIDGE`. Map-span GenericBridge objects are targetable.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Bridge,
    /// C++ `KINDOF_BRIDGE_TOWER`. Water waves skip towers (hit the span).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    BridgeTower,
    /// C++ `KINDOF_STEALTH_GARRISON`. Ninja / stealth-garrison infantry hide
    /// civilian buildings from non-allies and can be kicked by enemy Enter.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    StealthGarrison,
    /// C++ `KINDOF_TECH_BUILDING`. Captured oil/tech reverts to Neutral on killPlayer.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    TechBuilding,
    /// C++ `KINDOF_LANDMARK_BRIDGE`. Map-placed non-resizable spans (TsingMa).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    LandmarkBridge,
    /// C++ `KINDOF_AUTO_RALLYPOINT`. Factories/producers that accept
    /// ACTIONTYPE_SET_RALLY_POINT. Gameplay-only: the compact presentation
    /// KindOf bank is full.
    AutoRallypoint,
    /// C++ `KINDOF_MOB_NEXUS`. Angry Mob coordinator. ImmuneToGPS strips the
    /// default OverrideableByLikeKind StealthUpdate (ThingTemplate.cpp:391).
    /// Gameplay-only: the compact presentation KindOf bank is full.
    MobNexus,
    /// C++ `KINDOF_NO_COLLIDE` (KindOf.h:56). Partition never collides
    /// these objects. Gameplay-only: the compact presentation KindOf bank
    /// is full.
    NoCollide,
    /// C++ `KINDOF_FORCEATTACKABLE` (KindOf.h:63). Always attackable via
    /// force-attack / hover pick even when not Selectable (civ fences,
    /// cargo planes). Gameplay-only: the compact presentation KindOf bank
    /// is full, so pick uses a dedicated presentation boolean.
    ForceAttackable,
    /// C++ `KINDOF_SHRUBBERY` (KindOf.h:32). Trees/bushes auto-cleared
    /// by construction. Gameplay-only: the compact presentation KindOf bank
    /// is full.
    Shrubbery,
    /// C++ `KINDOF_CLEARED_BY_BUILD` (KindOf.h:72). Debris/rubble under a
    /// footprint. Gameplay-only: the compact presentation KindOf bank is full.
    ClearedByBuild,
    /// C++ `KINDOF_INERT` (KindOf.h:110). Never removable for construction.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    Inert,
    /// C++ `KINDOF_BLAST_CRATER` (KindOf.h). Permanent pathfind footprints
    /// that also skip the airborne height gate. Gameplay-only: the compact
    /// presentation KindOf bank is full.
    BlastCrater,
    /// C++ `KINDOF_HUGE_VEHICLE` (KindOf.h:35). Overlord / Helix class.
    /// Gameplay-only: the compact presentation KindOf bank is full.
    HugeVehicle,
}

impl KindOf {
    /// Map one C++ KindOf.h token onto the live host capability set.
    pub fn from_ini_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_uppercase().as_str() {
            "ALWAYS_SELECTABLE" => Some(Self::AlwaysSelectable),
            "MP_COUNT_FOR_VICTORY" => Some(Self::MpCountForVictory),
            "SCORE" => Some(Self::Score),
            "SCORE_CREATE" => Some(Self::ScoreCreate),
            "SCORE_DESTROY" => Some(Self::ScoreDestroy),
            "CAN_SEE_THROUGH" | "CAN_SEE_THROUGH_STRUCTURE" => Some(Self::CanSeeThrough),
            "NO_GARRISON" | "NOGARRISON" => Some(Self::NoGarrison),
            "GARRISONABLE_UNTIL_DESTROYED" => Some(Self::GarrisonableUntilDestroyed),
            "BOAT" => Some(Self::Boat),
            "TRANSPORT" => Some(Self::Transport),
            "IMMUNE_TO_CAPTURE" | "IMMUNETOCAPTURE" => Some(Self::ImmuneToCapture),
            "DEFENSIVE_WALL" | "DEFENSIVEWALL" => Some(Self::DefensiveWall),
            "WALK_ON_TOP_OF_WALL" | "WALKONTOPOFWALL" => Some(Self::WalkOnTopOfWall),
            "BRIDGE" => Some(Self::Bridge),
            "LANDMARK_BRIDGE" | "LANDMARKBRIDGE" => Some(Self::LandmarkBridge),
            "BRIDGE_TOWER" | "BRIDGETOWER" => Some(Self::BridgeTower),
            "STEALTH_GARRISON" | "STEALTHGARRISON" => Some(Self::StealthGarrison),
            "TECH_BUILDING" | "TECHBUILDING" => Some(Self::TechBuilding),
            "AUTO_RALLYPOINT" | "AUTO_RALLY_POINT" => Some(Self::AutoRallypoint),
            "MOB_NEXUS" | "MOBNEXUS" => Some(Self::MobNexus),
            "NO_COLLIDE" | "NOCOLLIDE" => Some(Self::NoCollide),
            "FORCEATTACKABLE" | "FORCE_ATTACKABLE" => Some(Self::ForceAttackable),
            "SHRUBBERY" => Some(Self::Shrubbery),
            "CLEARED_BY_BUILD" | "CLEAREDBYBUILD" => Some(Self::ClearedByBuild),
            "INERT" => Some(Self::Inert),
            "BLAST_CRATER" | "BLASTCRATER" => Some(Self::BlastCrater),
            "HUGE_VEHICLE" | "HUGEVEHICLE" => Some(Self::HugeVehicle),

            "DRONE" => Some(Self::Drone),
            _ => None,
        }
    }
}

/// Object status flags
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ObjectStatus {
    pub destroyed: bool,
    /// C++ isEffectivelyDead residual (rubble kept in world).
    #[serde(default)]
    pub effectively_dead: bool,
    /// C++ KeepObjectDie rubble residual (object stays after death).
    #[serde(default)]
    pub keep_as_rubble: bool,
    /// C++ OBJECT_STATUS_REPULSOR residual (script SetRepulsor / dead civs).
    #[serde(default)]
    pub repulsor: bool,
    pub under_construction: bool,
    /// C++ OBJECT_STATUS_SOLD residual (BuildAssistant sell process).
    #[serde(default)]
    pub sold: bool,
    /// C++ OBJECT_STATUS_RECONSTRUCTING residual (RebuildHoleBehavior).
    #[serde(default)]
    pub reconstructing: bool,
    pub selected: bool,
    /// C++ OBJECT_STATUS_DEPLOYED residual (DeployStyle AI / artillery unpack).
    #[serde(default)]
    pub deployed: bool,
    pub moving: bool,
    pub attacking: bool,
    /// C++ OBJECT_STATUS_IS_FIRING_WEAPON residual (AttackFireWeaponState).
    #[serde(default)]
    pub is_firing_weapon: bool,
    /// C++ OBJECT_STATUS_IGNORING_STEALTH residual.
    #[serde(default)]
    pub ignoring_stealth: bool,
    /// C++ OBJECT_STATUS_IS_AIMING_WEAPON residual (AttackAimAtTargetState).
    #[serde(default)]
    pub is_aiming_weapon: bool,
    /// C++ OBJECT_STATUS_IS_USING_ABILITY residual (SpecialAbilityUpdate prep/fire).
    /// CamoNetting StealthForbiddenConditions USING_ABILITY residual gate.
    #[serde(default)]
    pub using_ability: bool,
    pub airborne_target: bool,
    /// C++ OBJECT_STATUS_STEALTHED residual.
    pub stealthed: bool,
    /// C++ OBJECT_STATUS_DETECTED residual (revealed by detector / temporary reveal).
    /// Stealthed + not detected => not targetable / not visible to enemies.
    pub detected: bool,
    /// C++ DISABLED_UNDERPOWERED: set when player's power supply < demand.
    pub disabled_underpowered: bool,
    /// C++ DISABLED_DEFAULT residual (WaveGuide starts disabled until DamDie).
    #[serde(default)]
    pub disabled_default: bool,
    /// C++ DISABLED_UNMANNED residual (DAMAGE_KILLPILOT / Jarmen Kell snipe).
    /// Vehicle stays alive but cannot act; team is typically Neutral.
    #[serde(default)]
    pub disabled_unmanned: bool,
    /// C++ DAMAGE_DEPLOY residual edge (AssaultTransport beginAssault signal).
    #[serde(default)]
    pub pending_deploy_assault: bool,
    /// C++ DAMAGE_KILL_GARRISONED residual: floor(amount) occupants pending clear.
    #[serde(default)]
    pub pending_kill_garrisoned: u32,
    /// C++ GarrisonContain::onBodyDamageStateChange — walk occupants out on
    /// the edge into BODY_REALLYDAMAGED (unless GARRISONABLE_UNTIL_DESTROYED).
    #[serde(default)]
    pub pending_garrison_really_damaged_eject: bool,
    /// C++ ActiveBody::onSubdualChange → OpenContain::orderAllPassengersToIdle.
    /// HostObject cannot mutate occupants; GameLogic flushes this edge.
    #[serde(default)]
    pub pending_subdual_passenger_idle: bool,
    /// C++ Patch 1.01 IC un-subdual → orderAllPassengersToHackInternet.
    #[serde(default)]
    pub pending_internet_center_resume_hack: bool,
    /// C++ DISABLED_HACKED residual (Black Lotus DisableVehicleHack).
    /// Vehicle stays alive on its team but cannot move/attack until frame expires.
    #[serde(default)]
    pub disabled_hacked: bool,
    /// Absolute host logic frame when DISABLED_HACKED expires (0 = inactive).
    #[serde(default)]
    pub disabled_hacked_until_frame: u32,
    /// C++ DISABLED_EMP residual (EMPUpdate / SuperweaponEMPPulse).
    /// Vehicle/structure stays alive but cannot move/attack/produce until frame expires.
    #[serde(default)]
    pub disabled_emp: bool,
    /// Absolute host logic frame when DISABLED_EMP expires (0 = inactive).
    #[serde(default)]
    pub disabled_emp_until_frame: u32,
    /// C++ DISABLED_PARALYZED residual (BattlePlanChangeParalyzeTime).
    /// Army members freeze on Strategy Center plan change until frame expires.
    #[serde(default)]
    pub disabled_paralyzed: bool,
    /// Absolute host logic frame when DISABLED_PARALYZED expires (0 = inactive).
    #[serde(default)]
    pub disabled_paralyzed_until_frame: u32,
    /// Fire-only residual: weapons cannot fire (`canFireWeapon`).
    /// C++ MODELCONDITION_JAMMED is missile-jam; vehicle ECM uses DISABLED_SUBDUED.
    /// Does **not** skip update modules or freeze movement.
    #[serde(default)]
    pub weapons_jammed: bool,
    /// C++ DISABLED_SUBDUED (ActiveBody::onSubdualChange). Full halt:
    /// GameLogic skips update modules (AIUpdate processes only DISABLED_HELD).
    /// ECM vehicle disabler + Microwave building disabler both set this
    /// when `isSubdued()` (`currentSubdual >= maxHealth`).
    #[serde(default)]
    pub disabled_subdued: bool,
    /// C++ DISABLED_FREEFALL residual (PhysicsBehavior IS_IN_FREEFALL while airborne).
    #[serde(default)]
    pub disabled_freefall: bool,
    /// C++ OBJECT_STATUS_IS_CARBOMB residual (ConvertToCarBombCrateCollide).
    /// Vehicle uses SuicideCarBomb weapon set residual and detonates on attack fire.
    #[serde(default)]
    pub is_carbomb: bool,
    /// C++ OBJECT_STATUS_HIJACKED residual (ConvertToHijackedVehicleCrateCollide).
    #[serde(default)]
    pub hijacked: bool,
    /// C++ Object::m_privateStatus CAPTURED residual (setCaptured).
    /// Sticky once true (C++ rarely clears).
    #[serde(default)]
    pub private_captured: bool,
    /// C++ OBJECT_STATUS_NO_COLLISIONS residual (hijacker in vehicle / parachute).
    #[serde(default)]
    pub no_collisions: bool,
    /// C++ OBJECT_STATUS_MASKED residual (not selectable/targetable by AI/player).
    #[serde(default)]
    pub masked: bool,
    /// C++ OBJECT_STATUS_UNSELECTABLE residual.
    #[serde(default)]
    pub unselectable: bool,
    /// C++ OBJECT_STATUS_DISGUISED residual (Bomb Truck StealthUpdate disguise).
    /// Disguised units are not pure-stealth invisible; enemies see disguise team.
    #[serde(default)]
    pub disguised: bool,

    /// C++ StealthUpdate m_disguiseTransitionFrames residual.
    #[serde(default)]
    pub disguise_transition_frames: u32,
    /// C++ m_transitioningToDisguise residual (true = gaining look).
    #[serde(default)]
    pub disguise_transitioning_to: bool,
    /// C++ m_disguiseHalfpointReached residual (model swap at mid transition).
    #[serde(default)]
    pub disguise_halfpoint_reached: bool,
    /// Host residual opacity factor during disguise transition (0..1 presentation).
    #[serde(default)]
    pub disguise_transition_opacity: f32,
    /// C++ SpyVisionUpdate::setDisabledUntilFrame residual (Internet Center sabotage).
    #[serde(default)]
    pub spy_vision_disabled_until_frame: u32,
    /// C++ SpyVisionUpdate m_resetTimersNextUpdate (sabotage/EMP edge).
    #[serde(default)]
    pub spy_vision_reset_timers: bool,
    /// C++ SpyVisionUpdate self-powered Hack II next wake (after interval).
    #[serde(default)]
    pub spy_vision_hack_two_wake_frame: u32,
    /// C++ OBJECT_STATUS_FAERIE_FIRE residual (AvengerTargetDesignator paint).
    /// Attackers shooting a painted target gain TARGET_FAERIE_FIRE 150% ROF.
    #[serde(default)]
    pub faerie_fire: bool,
    /// C++ OBJECT_STATUS_BOOBY_TRAPPED residual (Rebel SpecialAbilityBoobyTrap).
    #[serde(default)]
    pub booby_trapped: bool,
    /// Host residual: OCL_EjectPilotOnGround InvulnerableTime post-eject shield.
    /// C++ goInvulnerable / UNDETECTED_DEFECTOR residual (damage blocked host-side).
    #[serde(default)]
    pub eject_invulnerable: bool,
    /// Absolute host logic frame when eject InvulnerableTime expires (0 = inactive).
    #[serde(default)]
    pub eject_invulnerable_until_frame: u32,
    /// C++ PilotFindVehicleUpdate::m_didMoveToBase residual.
    /// AI pilot attempts base-center fallback once when no recrewable vehicle is found.
    #[serde(default)]
    pub pilot_did_move_to_base: bool,
    /// C++ OBJECT_STATUS_PARACHUTING residual (OCL_EjectPilotViaParachute /
    /// AmericaParachute host residual). Pilot sinks until ground.
    #[serde(default)]
    pub parachuting: bool,
    /// AmericaParachute OpenClose residual: chute has opened after OpenDist freefall.
    #[serde(default)]
    pub parachute_open: bool,
    /// Spawn height (y) when parachuting began — OpenDist freefall distance residual.
    #[serde(default)]
    pub parachute_start_height: f32,
    /// AmericaParachute pitch/roll sway residual (radians). C++ ParachuteContain
    /// `m_pitch` / `m_roll` spring-damper while chute open.
    #[serde(default)]
    pub parachute_pitch: f32,
    #[serde(default)]
    pub parachute_roll: f32,
    /// AmericaParachute pitch/roll rates residual (radians per logic frame).
    #[serde(default)]
    pub parachute_pitch_rate: f32,
    #[serde(default)]
    pub parachute_roll_rate: f32,
    /// C++ ParachuteContain::m_landingOverride residual (DeliverPayload aim).
    #[serde(default)]
    pub parachute_landing_override: Option<glam::Vec3>,
    /// C++ ParachuteContain::m_isLandingOverrideSet residual.
    #[serde(default)]
    pub parachute_landing_override_set: bool,
    /// C++ `AIRappelState` residual — descending a combat-drop rope.
    #[serde(default)]
    pub rappelling: bool,
    /// Host-Y dest (C++ `m_destZ`: layer height + structure roof).
    #[serde(default)]
    pub rappel_dest_y: f32,
    /// C++ `m_targetIsBldg`.
    #[serde(default)]
    pub rappel_target_is_bldg: bool,
    /// C++ rappel goal object (garrison structure).
    #[serde(default)]
    pub rappel_target: Option<ObjectId>,
    /// Locomotor max speed saved across `setDesiredSpeed(m_rappelSpeed)`.
    #[serde(default)]
    pub rappel_saved_speed: f32,
    /// Original controlling team when DISABLED_UNMANNED was applied.
    /// Host killpilot sets team Neutral; this preserves PartitionFilterPlayer residual.
    #[serde(default)]
    pub unmanned_owner_team: Option<Team>,
    /// Exact controlling-player provenance captured with `DISABLED_UNMANNED`.
    /// A kill-pilot transition normally makes the vehicle Neutral and clears
    /// `Object::owner_player_id`; retaining this separately lets the pilot
    /// `VeterancyCrateCollide IsPilot` path require the same C++ controlling
    /// player rather than treating a shared faction as one owner.
    #[serde(default)]
    pub unmanned_owner_player_id: Option<u32>,
    /// Residual death type for DieMux DeathTypes filters (EjectPilotDie etc).
    /// Default Normal (combat residual). Set to Crushed/Splatted for crush deaths.
    #[serde(default)]
    pub death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    /// C++ OBJECT_STATUS_DECK_HEIGHT_OFFSET residual (parked on airfield/carrier).
    #[serde(default)]
    pub deck_height_offset: bool,
    /// C++ OBJECT_STATUS_WET residual (WaveGuideUpdate doDamage once-gate).
    #[serde(default)]
    pub wet: bool,
    /// C++ DISABLED_SCRIPT_DISABLED (WB/script Enabled=No).
    #[serde(default)]
    pub disabled_script_disabled: bool,
    /// C++ DISABLED_SCRIPT_UNDERPOWERED (WB/script Powered=No).
    #[serde(default)]
    pub disabled_script_underpowered: bool,
    /// C++ DISABLED_HELD (Battle Bus second-life hulk / contain freeze).
    #[serde(default)]
    pub disabled_held: bool,
    /// C++ OBJECT_STATUS_MISSILE_KILLING_SELF (MissileAIUpdate::detonate → KillSelf).
    #[serde(default)]
    pub missile_killing_self: bool,
}

/// Basic geometry information for objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryInfo {
    pub position: Vec3,
    pub rotation: f32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub radius: f32,
}

impl Default for GeometryInfo {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: 0.0,
            bounds_min: Vec3::splat(-1.0),
            bounds_max: Vec3::splat(1.0),
            radius: 1.0,
        }
    }
}

/// Health and damage system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(max_health: f32) -> Self {
        Self {
            current: max_health,
            maximum: max_health,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.maximum
    }

    pub fn percentage(&self) -> f32 {
        if self.maximum > 0.0 {
            self.current / self.maximum
        } else {
            0.0
        }
    }

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
    }
}

/// Movement and pathfinding state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movement {
    pub target_position: Option<Vec3>,
    pub velocity: Vec3,
    pub max_speed: f32,
    /// C++ LocomotorTemplate::m_maxSpeedDamaged residual.
    pub max_speed_damaged: f32,
    pub acceleration: f32,
    /// C++ LocomotorTemplate::m_accelerationDamaged residual.
    pub acceleration_damaged: f32,
    pub turn_rate: f32,
    /// C++ LocomotorTemplate::m_maxTurnRateDamaged residual.
    pub turn_rate_damaged: f32,
    pub path: Vec<Vec3>,
    pub current_path_index: usize,
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            target_position: None,
            velocity: Vec3::ZERO,
            max_speed: 10.0,
            // Retail default often ~ half pristine when damaged; host uses 50%.
            max_speed_damaged: 5.0,
            acceleration: 5.0,
            acceleration_damaged: 2.5,
            turn_rate: std::f32::consts::PI,
            turn_rate_damaged: std::f32::consts::PI * 0.5,
            path: Vec::new(),
            current_path_index: 0,
        }
    }
}

/// Economic resources
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Resources {
    pub supplies: u32,
    pub power: i32, // Can be negative
}

/// Experience and veterancy system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VeterancyLevel {
    Rookie,
    Veteran,
    Elite,
    Heroic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub current: f32,
    pub level: VeterancyLevel,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            current: 0.0,
            level: VeterancyLevel::Rookie,
        }
    }
}

/// Weapon and combat stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    pub damage: f32,
    pub range: f32,
    /// C++ parity (WeaponTemplate::m_minimumAttackRange): weapons cannot fire
    /// at targets closer than this distance.  0.0 = no minimum range.
    pub min_range: f32,
    pub reload_time: f32,
    pub last_fire_time: f32,
    pub ammo: Option<u32>,
    /// C++ ClipSize residual. 0 = unlimited (ammo ignored for readiness).
    #[serde(default)]
    pub clip_size: u32,
    /// C++ ClipReloadTime residual (seconds) when clip empties. 0 = ready same frame.
    #[serde(default)]
    pub clip_reload_time: f32,
    pub can_target_air: bool,
    pub can_target_ground: bool,
    /// C++ parity (WeaponTemplate::m_weaponSpeed): projectile travel speed.
    /// 0.0 = instant-hit (laser/flame weapons).
    pub projectile_speed: f32,
    /// C++ parity (WeaponTemplate::m_preAttackDelay): delay before firing
    /// after a target is acquired, in seconds.  0.0 = no delay.
    pub pre_attack_delay: f32,
    /// C++ radius damage residual (WeaponTemplate primary/secondary radius).
    /// 0.0 = no splash (direct hit only).
    #[serde(default)]
    pub splash_radius: f32,
    /// C++ `Weapon::m_status == RELOADING_CLIP`. ShareWeaponReloadTime siblings
    /// keep this set so `getRemainingAmmo` / pips report 0 while the clip waits.
    #[serde(default)]
    pub reloading_clip: bool,
    /// Last RATE_OF_FIRE bonus applied to this slot. 0 = unset.
    /// C++ `Weapon::onWeaponBonusChange` restarts an in-progress wait.
    #[serde(default)]
    pub last_bonus_rof: f32,
    /// C++ `Weapon::m_suspendFXFrame` (Weapon.cpp:1742).  This is client-only
    /// runtime state: FireFX is suppressed while the current logic frame is
    /// below this absolute frame, but recoil still broadcasts.  Keep it out
    /// of the nested serde/bincode Weapon payload; ObjectSnapshot owns the
    /// versioned parallel tail so historical positional records remain safe.
    #[serde(skip)]
    pub suspend_fx_frame: u32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 25.0,
            range: 100.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 200.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
            suspend_fx_frame: 0,
        }
    }
}

impl Weapon {
    /// C++ `TheGameLogic->getFrame() < getSuspendFXFrame()` gate used by
    /// Weapon::fireWeapon.  The renderer/DrawModule route remains fail-closed
    /// until its pre-mutation owner snapshot exists; this helper only exposes
    /// the authoritative source state.
    #[inline]
    pub fn fire_fx_is_suspended_at(&self, logic_frame: u32) -> bool {
        logic_frame < self.suspend_fx_frame
    }

    /// Set the absolute frame at which FireFX becomes eligible.  This mirrors
    /// construction/copy state without deriving a delay from a model name.
    #[inline]
    pub fn set_suspend_fx_frame(&mut self, frame: u32) {
        self.suspend_fx_frame = frame;
    }
}

#[cfg(test)]
mod tests {
    use super::{KindOf, Weapon};

    #[test]
    fn suspend_fx_frame_is_an_absolute_logic_frame_and_clone_preserves_it() {
        let mut weapon = Weapon::default();
        weapon.set_suspend_fx_frame(42);
        assert!(weapon.fire_fx_is_suspended_at(41));
        assert!(!weapon.fire_fx_is_suspended_at(42));
        assert_eq!(weapon.clone().suspend_fx_frame, 42);
    }

    #[test]
    fn host_kindof_maps_cpp_always_selectable_and_mp_count_tokens() {
        // C++ KindOf.h:63-67,86 — live host bits, not an FS-kind union.
        assert_eq!(
            KindOf::from_ini_token("ALWAYS_SELECTABLE"),
            Some(KindOf::AlwaysSelectable)
        );
        assert_eq!(
            KindOf::from_ini_token("mp_count_for_victory"),
            Some(KindOf::MpCountForVictory)
        );
        assert_eq!(KindOf::from_ini_token("SCORE"), Some(KindOf::Score));
        assert_eq!(
            KindOf::from_ini_token("SCORE_CREATE"),
            Some(KindOf::ScoreCreate)
        );
        assert_eq!(
            KindOf::from_ini_token("SCORE_DESTROY"),
            Some(KindOf::ScoreDestroy)
        );
        assert_eq!(
            KindOf::from_ini_token("NO_GARRISON"),
            Some(KindOf::NoGarrison)
        );
        assert_eq!(
            KindOf::from_ini_token("GARRISONABLE_UNTIL_DESTROYED"),
            Some(KindOf::GarrisonableUntilDestroyed)
        );
        assert_eq!(
            KindOf::from_ini_token("STEALTH_GARRISON"),
            Some(KindOf::StealthGarrison)
        );
        assert_eq!(
            KindOf::from_ini_token("AUTO_RALLYPOINT"),
            Some(KindOf::AutoRallypoint)
        );
        assert_eq!(KindOf::from_ini_token("MOB_NEXUS"), Some(KindOf::MobNexus));
        assert_eq!(
            KindOf::from_ini_token("NO_COLLIDE"),
            Some(KindOf::NoCollide)
        );
        assert_eq!(
            KindOf::from_ini_token("FORCEATTACKABLE"),
            Some(KindOf::ForceAttackable)
        );
        assert_eq!(KindOf::from_ini_token("SHRUBBERY"), Some(KindOf::Shrubbery));
        assert_eq!(
            KindOf::from_ini_token("CLEARED_BY_BUILD"),
            Some(KindOf::ClearedByBuild)
        );
        assert_eq!(KindOf::from_ini_token("INERT"), Some(KindOf::Inert));
        assert_eq!(
            KindOf::from_ini_token("BLAST_CRATER"),
            Some(KindOf::BlastCrater)
        );
        assert_eq!(
            KindOf::from_ini_token("HUGE_VEHICLE"),
            Some(KindOf::HugeVehicle)
        );

        assert_eq!(KindOf::from_ini_token("FS_FACTORY"), None);
    }
}
