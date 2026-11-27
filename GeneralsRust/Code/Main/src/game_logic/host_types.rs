//! Core host gameplay types (`ObjectId`, `Team`, `KindOf`, `Health`, …).
//!
//! Re-exported from `crate::game_logic` so public paths stay stable.

pub use generals_game_domain::{ObjectId, Team};
pub use generals_sim::kind_of::KindOf;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectId = ObjectId(0);

/// Runtime of the installed C++ PilotFindVehicleUpdate module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PilotFindVehicleState {
    /// None is UPDATE_SLEEP_FOREVER for a human or a pilot without AI.
    pub next_wake_frame: Option<u32>,
    pub did_move_to_base: bool,
}

/// Object status flags
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ObjectStatus {
    /// C++ Object::OFF_MAP private status; persisted by the object continuation.
    #[serde(skip)]
    pub(crate) off_map: bool,
    /// `Object::m_scriptStatus` is distinct from the disabled mask, which may
    /// also be changed by a containing object or its SpawnBehavior owner.
    /// The ODIS continuation carries this without changing positional saves.
    #[serde(skip)]
    pub(crate) script_status_disabled: bool,
    #[serde(skip)]
    pub(crate) script_status_underpowered: bool,
    #[serde(skip)]
    pub(crate) disabled_script_disabled_until_frame: u32,
    #[serde(skip)]
    pub(crate) disabled_script_underpowered_until_frame: u32,
    /// Original disabled mask bits 8/9. Companion framing preserves their
    /// continuation without changing positional ObjectStatus save bytes.
    #[serde(skip)]
    pub disabled_awestruck: bool,
    #[serde(skip)]
    pub disabled_brainwashed: bool,
    #[serde(skip)]
    pub disabled_awestruck_until_frame: u32,
    #[serde(skip)]
    pub disabled_brainwashed_until_frame: u32,

    pub destroyed: bool,
    /// C++ `Object::onDie` ran once for this death (die modules must not re-fire).
    #[serde(default)]
    pub on_die_started: bool,
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
    /// Object::setDisabled stores FOREVER even when frame equality leaves the bit clear.
    #[serde(skip)]
    pub(crate) disabled_underpowered_until_frame: u32,
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
    /// The PilotFindVehicleUpdate wake and base latch are written by OXOB.
    #[serde(skip)]
    pub(crate) pilot_find_vehicle: Option<PilotFindVehicleState>,
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
    /// C++ m_disabledTillFrame[DISABLED_HELD], carried by the native disabled
    /// continuation when it cannot be derived from the legacy active bit.
    #[serde(skip)]
    pub(crate) disabled_held_until_frame: u32,
    /// C++ OBJECT_STATUS_MISSILE_KILLING_SELF (MissileAIUpdate::detonate → KillSelf).
    #[serde(default)]
    pub missile_killing_self: bool,
}

pub use generals_sim::object::GeometryInfo;
pub use generals_sim::object::body::Health;
pub use generals_sim::object::Movement;

/// Economic resources
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Resources {
    pub supplies: u32,
    pub power: i32, // Can be negative
}

/// Experience and veterancy system
pub use generals_sim::object::VeterancyLevel;

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

pub use generals_sim::weapon::Weapon;

#[cfg(test)]
mod tests {
    use super::{KindOf, Weapon};

    #[test]
    fn linebuild_kindof_is_appended_and_accepts_authored_token() {
        assert_eq!(KindOf::from_ini_token("LINEBUILD"), Some(KindOf::LineBuild));
        assert_eq!(
            KindOf::from_ini_token("line_build"),
            Some(KindOf::LineBuild)
        );
        assert_eq!(KindOf::LineBuild.name(), "LineBuild");
        assert_eq!(
            crate::game_logic::host_radar_stealth_vision_residual::live_kind_of_cpp_mask(
                KindOf::LineBuild
            ),
            1u128 << 15,
        );
        assert!(KindOf::ALL_KIND_OF.contains(&KindOf::LineBuild));
        assert_eq!(KindOf::ALL_KIND_OF.last(), Some(&KindOf::RevealsEnemyPaths));
        // Existing serde discriminants remain stable even where Xfer uses a
        // different established order (CliffJumper precedes CleanupHazard).
        for (kind, ordinal) in [
            (KindOf::Structure, 0u32),
            (KindOf::CliffJumper, 83),
            (KindOf::CleanupHazard, 84),
            (KindOf::LineBuild, 85),
            (KindOf::Hulk, 86),
            (KindOf::RejectUnmanned, 87),
            (KindOf::SpawnsAreTheWeapons, 88),
            (KindOf::PortableStructure, 89),
            (KindOf::BoobyTrap, 90),
        ] {
            let bytes = bincode_legacy::serialize(&kind).unwrap();
            assert_eq!(bytes, ordinal.to_le_bytes());
            assert_eq!(bincode_legacy::deserialize::<KindOf>(&bytes).unwrap(), kind);
        }
    }

    #[test]
    fn spawns_are_the_weapons_kindof_is_append_only_and_recognizes_authored_token() {
        assert_eq!(
            KindOf::from_ini_token("SPAWNS_ARE_THE_WEAPONS"),
            Some(KindOf::SpawnsAreTheWeapons)
        );
        assert_eq!(KindOf::SpawnsAreTheWeapons.name(), "SpawnsAreTheWeapons");
        assert!(KindOf::ALL_KIND_OF.contains(&KindOf::SpawnsAreTheWeapons));
        assert_eq!(
            crate::game_logic::host_radar_stealth_vision_residual::live_kind_of_cpp_mask(
                KindOf::SpawnsAreTheWeapons
            ),
            1u128 << 79,
        );
    }

    #[test]
    fn portable_structure_kindof_keeps_cpp_bit_and_append_only_save_id() {
        assert_eq!(
            KindOf::from_ini_token("PORTABLE_STRUCTURE"),
            Some(KindOf::PortableStructure)
        );
        assert_eq!(KindOf::PortableStructure.name(), "PortableStructure");
        assert!(KindOf::ALL_KIND_OF.contains(&KindOf::PortableStructure));
        assert_eq!(
            crate::game_logic::host_radar_stealth_vision_residual::live_kind_of_cpp_mask(
                KindOf::PortableStructure
            ),
            1u128 << 52,
        );
    }

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
        assert_eq!(
            KindOf::from_ini_token("BLAST_CRATER"),
            Some(KindOf::BlastCrater)
        );
        assert_eq!(
            KindOf::from_ini_token("HUGE_VEHICLE"),
            Some(KindOf::HugeVehicle)
        );
        assert_eq!(
            KindOf::from_ini_token("CLEANUP_HAZARD"),
            Some(KindOf::CleanupHazard)
        );

        assert_eq!(KindOf::from_ini_token("FS_FACTORY"), None);
    }
}

#[cfg(test)]
mod reject_unmanned_kind_tests {
    use super::KindOf;
    #[test]
    fn reject_unmanned_keeps_cpp_bit_and_append_only_save_ordinal() {
        assert_eq!(
            KindOf::from_ini_token("REJECT_UNMANNED"),
            Some(KindOf::RejectUnmanned)
        );
        assert_eq!(KindOf::RejectUnmanned.name(), "RejectUnmanned");
        assert_eq!(
            crate::game_logic::host_radar_stealth_vision_residual::live_kind_of_cpp_mask(
                KindOf::RejectUnmanned
            ),
            1u128 << 109
        );
        let bytes = bincode_legacy::serialize(&KindOf::RejectUnmanned).unwrap();
        assert_eq!(bytes, 87u32.to_le_bytes());
        assert_eq!(
            bincode_legacy::deserialize::<KindOf>(&bytes).unwrap(),
            KindOf::RejectUnmanned
        );
    }
}
