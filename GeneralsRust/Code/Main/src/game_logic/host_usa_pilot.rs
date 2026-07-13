//! Host USA Pilot residual (eject recrew of unmanned vehicles + EjectPilotDie).
//!
//! Residual slice (playability):
//! - `AmericaInfantryPilot` / AirF_ / CINE_ / TestPilot enter unmanned ground
//!   vehicles (DISABLED_UNMANNED residual from Jarmen Kell snipe / neutron) →
//!   recrew: clear unmanned, transfer team to pilot's team, transfer pilot
//!   veterancy (retail `VeterancyCrateCollide` IsPilot + AddsOwnerVeterancy),
//!   consume pilot.
//! - Pilots spawn residual at least VETERAN (VeterancyGainCreate StartingLevel).
//! - **EjectPilotDie residual**: eligible USA ground vehicles (Humvee / Tomahawk /
//!   Crusader / Paladin / Avenger / Microwave + general variants) spawn
//!   `AmericaInfantryPilot` on death via OCL_EjectPilotOnGround residual path.
//!   Fail-closed: unmanned vehicles do not eject (no pilot left).
//! - **VeterancyLevels residual**: retail `VeterancyLevels = ALL -REGULAR`
//!   ("only vet+ gives pilot"). Rookie / LEVEL_REGULAR vehicles do **not** eject.
//! - **InvulnerableTime residual**: OCL_EjectPilotOnGround `InvulnerableTime = 2000`
//!   ms → **60 frames**. Post-eject pilot residual blocks damage (host of C++
//!   `goInvulnerable` / undetected-defector relationship shield).
//! - **PilotFindVehicleUpdate residual**: AI-only idle pilot auto-scan
//!   (ScanRate **1000**ms → **30** frames, ScanRange **300**, MinHealth **0.5**)
//!   toward nearest recrewable unmanned vehicle → Enter residual.
//!   C++ human players sleep forever (no auto-scan).
//! - **Base-center fallback residual**: when no vehicle found, AI pilot moves once
//!   toward team command-center / base (`m_didMoveToBase`).
//! - **AutoFindHealingUpdate residual**: AI-only idle injured USA infantry
//!   auto-scan (ScanRate **1000**ms → **30** frames, ScanRange **300**,
//!   NeverHeal **0.85**, AlwaysHeal **0.25**) toward nearest HealPad →
//!   SeekingHealing residual. Templates: Pilot / Ranger / MissileDefender /
//!   Pathfinder / ColonelBurton residual family.
//! - **Air OCL parachute residual**: when dying vehicle is significantly above
//!   terrain (C++ `isSignificantlyAboveTerrain` / OCL_EjectPilotViaParachute),
//!   pilot spawns elevated + parachuting residual sink until ground.
//! - **AmericaParachute OpenDist residual**: freefall until fallen **100**
//!   (`ParachuteOpenDist`) then open chute (slower sink + open audio residual).
//! - **Low-altitude open fudge residual**: if startZ − ground < **2×OpenDist**,
//!   fudge start height so the chute can open (C++ ParachuteContain update).
//! - **FreeFallDamage residual**: when chute is destroyed mid-air while
//!   significantly above terrain, rider takes **FreeFallDamagePercent 0.5**
//!   max-health residual (DAMAGE_FALLING / DEATH_SPLATTED honesty).
//! - **Pitch/roll sway residual** (AmericaParachute ParachuteContain +
//!   ParachuteLocomotor): while chute open, spring-damper pitch/roll residual
//!   with PitchRateMax/RollRateMax **60** deg/s → **π/90** rad/frame seed band,
//!   Pitch/RollStiffness **0.02**, Pitch/RollDamping **0.01**, LowAltitudeDamping
//!   **0.2** below **20** height (ALTITUDE_DAMP_START). Deterministic host seed
//!   (±half max rate).
//! - **Bone PARA_COG / PARA_ATTCH / PARA_MAN residual**: host pristine bone
//!   offsets (GeometryHeight **10** layout) + calcSwayMtx residual for rider
//!   logic attach + presentation sway about PARA_COG. Fail-closed: not full
//!   W3D pristine bone extract / DeliverPayload cargo plane path.
//!
//! - **EjectPilotDie DieMux residual**: retail `DeathTypes = ALL -CRUSHED
//!   -SPLATTED` + `ExemptStatus = HIJACKED`. Crushed/splatted deaths and
//!   hijacked vehicles do **not** eject.
//! - **PilotFindVehicle CollideModule residual**: `VeterancyCrateCollide`
//!   wouldLikeToCollideWith host gates — RequiredKindOf VEHICLE /
//!   ForbiddenKindOf DOZER, not significantly above terrain, not airborne
//!   locomotor, trainable, can gain exp for pilot levels (Heroic max).
//! - **PilotFindVehicle PartitionFilterPlayer residual**: same controlling
//!   player / host Neutral unmanned with matching `unmanned_owner_team`.
//!
//! Wave 61 residual pack (retail AmericaInfantry.ini / OCL / AmericaParachute honesty):
//! - Pilot body: MaxHealth **100**, VisionRange **150**, ShroudClearingRange **300**,
//!   StartingLevel **VETERAN**, TransportSlotCount **1**, Mass **5**
//! - Ejection OCL residual: InvulnerableTime **2000**ms → **60**f, InheritsVeterancy,
//!   Ground force Min/Max **2/3** pitch **50–60**, Air force Min/Max **10/12** pitch **50–60**,
//!   PutInContainer **AmericaParachute**, DeathTypes **ALL -CRUSHED -SPLATTED**,
//!   ExemptStatus **HIJACKED**, VeterancyLevels **ALL -REGULAR**
//! - Parachute residual: retail AmericaParachute OpenDist **25**, host air-eject span **100**
//!   (CINE), FreeFallDamage **50%**, Pitch/RollRateMax **60** deg/s, LowAltitudeDamping **0.2**,
//!   GeometryHeight **10** / MajorRadius **15**, open audio **ParachuteOpen**
//! - Return-to-base residual: PilotFindVehicle base-center fallback (`m_didMoveToBase`),
//!   ScanRate **1000**ms → **30**f, ScanRange **300**, MinHealth **0.5**;
//!   AutoFindHealing ScanRate **1000**ms, NeverHeal **0.85**, AlwaysHeal **0.25**
//!
//! Fail-closed honesty:
//! - Not full W3D pristine bone extract / DeliverPayload cargo plane path
//!   (PARA_COG host offset + calcSwayMtx residual closed 2026-07-13)
//! - AutoFindHealingUpdate AlwaysHeal busy-interrupt path is **dead code in retail
//!   C++** (`return UPDATE_SLEEP_NONE` before AlwaysHeal check) — host residual
//!   intentionally matches idle-only (AlwaysHeal constant retained for honesty).
//! - Not full dual-template ParachuteOpenDist matrix (retail 25 vs CINE 100)
//! - Not full defector FX flash / UNDETECTED_DEFECTOR relationship matrix
//! - Not full same-map PartitionFilterSameMapStatus matrix
//! - Not network recrew / pilot-eject replication (network deferred)

use super::VeterancyLevel;
use serde::{Deserialize, Serialize};

/// Residual death type for DieMuxData DeathTypes residual (EjectPilotDie etc).
///
/// Host residual of C++ DeathType enum subset used by DeathTypes filters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostDeathType {
    /// Default combat / generic residual death (passes ALL -CRUSHED -SPLATTED).
    #[default]
    Normal,
    /// DEATH_CRUSHED residual — blocked by EjectPilotDie DeathTypes filter.
    Crushed,
    /// DEATH_SPLATTED residual — blocked by EjectPilotDie DeathTypes filter.
    Splatted,
}

/// Retail pilot template family residual.
pub const PILOT_RECREW_AUDIO: &str = "PilotEnterVehicle";

/// Retail OCL_EjectPilotOnGround / OCL_EjectPilotViaParachute ObjectNames residual.
pub const EJECT_PILOT_TEMPLATE: &str = "AmericaInfantryPilot";

/// Residual eject audio (VoiceEject / SoundEject fail-closed host cue).
pub const PILOT_EJECT_AUDIO: &str = "VoiceEject";

/// Retail OCL_EjectPilotOnGround InvulnerableTime (ms).
pub const EJECT_PILOT_INVULNERABLE_MS: u32 = 2000;

/// Logic frames per second (host fixed step).
pub const EJECT_PILOT_LOGIC_FPS: f32 = 30.0;

/// Retail InvulnerableTime → frames at 30 FPS (2000 / (1000/30) = 60).
pub const EJECT_PILOT_INVULNERABLE_FRAMES: u32 = 60;

// --- PilotFindVehicleUpdate residual (AmericaInfantryPilot) ---

/// Retail PilotFindVehicleUpdate ScanRate (ms).
pub const PILOT_FIND_VEHICLE_SCAN_RATE_MS: u32 = 1000;

/// ScanRate → frames at 30 FPS (1000 / (1000/30) = 30).
pub const PILOT_FIND_VEHICLE_SCAN_FRAMES: u32 = 30;

/// Retail PilotFindVehicleUpdate ScanRange.
pub const PILOT_FIND_VEHICLE_SCAN_RANGE: f32 = 300.0;

/// Retail PilotFindVehicleUpdate MinHealth (don't enter vehicle below this ratio).
pub const PILOT_FIND_VEHICLE_MIN_HEALTH: f32 = 0.5;

// --- AutoFindHealingUpdate residual (AmericaInfantryPilot ModuleTag_06) ---

/// Retail AutoFindHealingUpdate ScanRate (ms).
pub const AUTO_FIND_HEALING_SCAN_RATE_MS: u32 = 1000;

/// ScanRate → frames at 30 FPS (1000 / (1000/30) = 30).
pub const AUTO_FIND_HEALING_SCAN_FRAMES: u32 = 30;

/// Retail AutoFindHealingUpdate ScanRange.
pub const AUTO_FIND_HEALING_SCAN_RANGE: f32 = 300.0;

/// Retail AutoFindHealingUpdate NeverHeal (don't heal above this ratio).
pub const AUTO_FIND_HEALING_NEVER_HEAL: f32 = 0.85;

/// Retail AutoFindHealingUpdate AlwaysHeal (busy-interrupt threshold residual).
///
/// **Dead code in retail C++**: `update()` early-returns when `!ai->isIdle()`
/// before the AlwaysHeal branch is reached. Host residual keeps the constant for
/// honesty and intentionally does **not** implement busy-interrupt (parity with
/// unreachable retail path).
pub const AUTO_FIND_HEALING_ALWAYS_HEAL: f32 = 0.25;

/// Whether AlwaysHeal busy-interrupt residual is live in host path.
///
/// Always `false` — matches C++ early-return (busy units never reach AlwaysHeal).
pub fn auto_find_healing_always_heal_busy_interrupt_live() -> bool {
    false
}

// --- EjectPilotDie air OCL / isSignificantlyAboveTerrain residual ---

/// Retail GameData.ini Gravity residual.
pub const HOST_GRAVITY: f32 = -64.0;

/// C++ `Thing::isSignificantlyAboveTerrain` threshold: `-(3*3)*gravity`.
/// With Gravity=-64 → height > **576** is significantly airborne.
pub fn significantly_above_terrain_threshold() -> f32 {
    -(3.0 * 3.0) * HOST_GRAVITY
}

/// Host residual open-chute sink (world units per logic frame).
/// Fail-closed: not full AmericaParachute PhysicsUpdate damping matrix.
pub const EJECT_PARACHUTE_SINK_PER_FRAME: f32 = 20.0;

/// Host residual freefall sink before chute opens (faster than open-chute residual).
pub const EJECT_PARACHUTE_FREEFALL_PER_FRAME: f32 = 40.0;

/// Retail AmericaParachute `ParachuteOpenDist` — freefall distance before open.
///
/// Host residual uses **100** (CINE / safe air-eject span). Retail base
/// `AmericaParachute` INI is 25; fail-closed not dual-template OpenDist matrix.
pub const PARACHUTE_OPEN_DIST: f32 = 100.0;

/// Retail base `AmericaParachute` ParachuteOpenDist residual (honesty dual-track).
pub const PARACHUTE_OPEN_DIST_RETAIL_BASE: f32 = 25.0;

/// CINE / host air-eject ParachuteOpenDist residual (matches PARACHUTE_OPEN_DIST).
pub const PARACHUTE_OPEN_DIST_CINE: f32 = 100.0;

// --- Wave 61 pilot body residual (AmericaInfantryPilot) ---

/// Retail AmericaInfantryPilot MaxHealth residual.
pub const PILOT_MAX_HEALTH: f32 = 100.0;
/// Retail AmericaInfantryPilot VisionRange residual.
pub const PILOT_VISION_RANGE: f32 = 150.0;
/// Retail AmericaInfantryPilot ShroudClearingRange residual.
pub const PILOT_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail TransportSlotCount residual.
pub const PILOT_TRANSPORT_SLOT_COUNT: u32 = 1;
/// Retail PhysicsBehavior Mass residual.
pub const PILOT_PHYSICS_MASS: f32 = 5.0;
/// Retail VeterancyGainCreate StartingLevel residual name.
pub const PILOT_STARTING_LEVEL: &str = "VETERAN";

// --- Wave 61 OCL_EjectPilot force residual ---

/// OCL_EjectPilotOnGround MinForceMagnitude residual.
pub const EJECT_GROUND_MIN_FORCE_MAG: f32 = 2.0;
/// OCL_EjectPilotOnGround MaxForceMagnitude residual.
pub const EJECT_GROUND_MAX_FORCE_MAG: f32 = 3.0;
/// OCL_EjectPilotViaParachute MinForceMagnitude residual.
pub const EJECT_AIR_MIN_FORCE_MAG: f32 = 10.0;
/// OCL_EjectPilotViaParachute MaxForceMagnitude residual.
pub const EJECT_AIR_MAX_FORCE_MAG: f32 = 12.0;
/// OCL eject MinForcePitch residual (deg).
pub const EJECT_FORCE_MIN_PITCH_DEG: f32 = 50.0;
/// OCL eject MaxForcePitch residual (deg).
pub const EJECT_FORCE_MAX_PITCH_DEG: f32 = 60.0;
/// OCL PutInContainer residual for air path.
pub const EJECT_AIR_PUT_IN_CONTAINER: &str = "AmericaParachute";
/// OCL InheritsVeterancy residual.
pub const EJECT_INHERITS_VETERANCY: bool = true;
/// OCL RequiresLivePlayer residual.
pub const EJECT_REQUIRES_LIVE_PLAYER: bool = true;
/// Retail EjectPilotDie DeathTypes residual tokens.
pub const EJECT_DEATH_TYPES: &str = "ALL -CRUSHED -SPLATTED";
/// Retail EjectPilotDie ExemptStatus residual.
pub const EJECT_EXEMPT_STATUS: &str = "HIJACKED";
/// Retail EjectPilotDie VeterancyLevels residual.
pub const EJECT_VETERANCY_LEVELS: &str = "ALL -REGULAR";

/// Retail ParachuteContainModuleData default FreeFallDamagePercent (0.5).
/// AmericaParachute INI does not override → 50% max health residual on chute die.
pub const FREE_FALL_DAMAGE_PERCENT: f32 = 0.5;

/// C++ low-altitude open fudge: startZ − ground must be ≥ **2×** ParachuteOpenDist.
pub const PARACHUTE_LOW_ALTITUDE_OPEN_MULT: f32 = 2.0;

/// Residual audio when AmericaParachute residual chute opens.
pub const PILOT_PARACHUTE_OPEN_AUDIO: &str = "ParachuteOpen";

/// Residual audio when air-ejected pilot lands (host of parachute open residual).
pub const PILOT_PARACHUTE_LAND_AUDIO: &str = "ParadropLanding";

/// Residual audio honesty when FreeFallDamage residual applies (splat path).
pub const PILOT_FREE_FALL_DAMAGE_AUDIO: &str = "BodyFallGeneric";

// --- AmericaParachute pitch/roll sway residual (ParachuteContain + ParachuteLocomotor) ---

/// Retail AmericaParachute `PitchRateMax` / `RollRateMax` (deg/sec).
pub const PARACHUTE_PITCH_RATE_MAX_DEG_PER_SEC: f32 = 60.0;
/// Same as pitch (retail RollRateMax = 60 deg/sec).
pub const PARACHUTE_ROLL_RATE_MAX_DEG_PER_SEC: f32 = 60.0;

/// C++ `ConvertAngularVelocityInDegreesPerSecToRadsPerFrame(60)` at 30 FPS:
/// `60 * (1/30) * (π/180) = π/90`.
pub fn parachute_rate_max_rads_per_frame() -> f32 {
    PARACHUTE_PITCH_RATE_MAX_DEG_PER_SEC * (std::f32::consts::PI / 180.0) / EJECT_PILOT_LOGIC_FPS
}

/// Retail ParachuteLocomotor PitchStiffness / RollStiffness.
pub const PARACHUTE_PITCH_STIFFNESS: f32 = 0.02;
/// Retail ParachuteLocomotor PitchStiffness / RollStiffness.
pub const PARACHUTE_ROLL_STIFFNESS: f32 = 0.02;
/// Retail ParachuteLocomotor PitchDamping / RollDamping.
pub const PARACHUTE_PITCH_DAMPING: f32 = 0.01;
/// Retail ParachuteLocomotor PitchDamping / RollDamping.
pub const PARACHUTE_ROLL_DAMPING: f32 = 0.01;
/// Retail AmericaParachute LowAltitudeDamping.
pub const PARACHUTE_LOW_ALTITUDE_DAMPING: f32 = 0.2;
/// C++ ParachuteContain ALTITUDE_DAMP_START (height above terrain).
pub const PARACHUTE_ALTITUDE_DAMP_START: f32 = 20.0;

/// Deterministic host residual initial pitch rate (C++ random in ±PitchRateMax).
/// Host uses **+½ max** so tests are stable and still non-zero.
pub fn parachute_initial_pitch_rate() -> f32 {
    parachute_rate_max_rads_per_frame() * 0.5
}

/// Deterministic host residual initial roll rate (C++ random in ±RollRateMax).
/// Host uses **−½ max** so pitch/roll axes are independently exercised.
pub fn parachute_initial_roll_rate() -> f32 {
    -parachute_rate_max_rads_per_frame() * 0.5
}

/// C++ ParachuteContain open-chute spring/damper residual (one logic frame).
///
/// ```text
/// pitchRate += (-stiffness * pitch) + (-(damping + altDamp) * pitchRate)
/// pitch     += pitchRate
/// ```
/// Same for roll. `altitude_damping` is LowAltitudeDamping when height ≤ 20, else 0.
/// Returns `(pitch, roll, pitch_rate, roll_rate)`.
pub fn tick_parachute_sway(
    pitch: f32,
    roll: f32,
    pitch_rate: f32,
    roll_rate: f32,
    height_above_terrain: f32,
) -> (f32, f32, f32, f32) {
    let alt_damp = if height_above_terrain <= PARACHUTE_ALTITUDE_DAMP_START {
        PARACHUTE_LOW_ALTITUDE_DAMPING
    } else {
        0.0
    };
    let pitch_damp = PARACHUTE_PITCH_DAMPING + alt_damp;
    let roll_damp = PARACHUTE_ROLL_DAMPING + alt_damp;
    let mut pr = pitch_rate;
    let mut rr = roll_rate;
    pr += (-PARACHUTE_PITCH_STIFFNESS * pitch) + (-pitch_damp * pr);
    rr += (-PARACHUTE_ROLL_STIFFNESS * roll) + (-roll_damp * rr);
    let p = pitch + pr;
    let r = roll + rr;
    (p, r, pr, rr)
}

// --- AmericaParachute bone PARA_COG / PARA_ATTCH / PARA_MAN residual ---

/// Retail AmericaParachute model `PMparacht_SKNc` pristine bone name residual.
pub const PARACHUTE_BONE_PARA_COG: &str = "PARA_COG";
/// Retail parachute attach bone (rider hang point on chute).
pub const PARACHUTE_BONE_PARA_ATTCH: &str = "PARA_ATTCH";
/// Retail rider attach bone (PARA_MAN on infantry drawable; falls back to height).
pub const PARACHUTE_BONE_PARA_MAN: &str = "PARA_MAN";

/// Host residual pristine bone offsets when W3D bones are unavailable.
///
/// C++ uses `getPristineBonePositions`; missing bones zero / height-fallback.
/// Host residual uses stable geometry from AmericaParachute GeometryHeight **10**
/// and GeometryMajorRadius **15** so sway/rider matrices are host-testable.
///
/// Layout (object-local, +Z up in C++; host residual uses +Y up):
/// - PARA_COG: sway pivot near chute canopy center
/// - PARA_ATTCH: hang point below COG
/// - PARA_MAN: rider attach relative to rider origin (height of infantry)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostParachuteBoneOffsets {
    /// Object-local PARA_COG (sway pivot).
    pub para_cog: (f32, f32, f32),
    /// Object-local PARA_ATTCH (rider hang on chute).
    pub para_attch: (f32, f32, f32),
    /// Rider-local PARA_MAN (or height fallback).
    pub para_man: (f32, f32, f32),
}

/// Retail AmericaParachute GeometryHeight residual.
pub const PARACHUTE_GEOMETRY_HEIGHT: f32 = 10.0;
/// Retail AmericaParachute GeometryMajorRadius residual.
pub const PARACHUTE_GEOMETRY_MAJOR_RADIUS: f32 = 15.0;
/// Host residual infantry height for PARA_MAN height-fallback (typical soldier).
pub const PARACHUTE_RIDER_HEIGHT_FALLBACK: f32 = 8.0;

/// Host residual pristine bone offsets (no W3D asset).
///
/// PARA_COG at canopy (~0.7×GeometryHeight above origin), PARA_ATTCH lower
/// (~0.2×height), PARA_MAN at rider height fallback.
pub fn parachute_host_bone_offsets() -> HostParachuteBoneOffsets {
    HostParachuteBoneOffsets {
        // Host Y-up: canopy above origin.
        para_cog: (0.0, PARACHUTE_GEOMETRY_HEIGHT * 0.7, 0.0),
        para_attch: (0.0, PARACHUTE_GEOMETRY_HEIGHT * 0.2, 0.0),
        para_man: (0.0, PARACHUTE_RIDER_HEIGHT_FALLBACK, 0.0),
    }
}

/// C++ ParachuteContain::updateOffsetsFromBones residual (object-local).
///
/// Returns `(rider_attach_offset, rider_sway_offset, para_sway_offset)` in
/// object-local space (host Y-up). Rider attach = para_attch − para_man;
/// rider sway pivot relative = para_cog − rider_attach.
pub fn parachute_offsets_from_bones(
    bones: HostParachuteBoneOffsets,
) -> ((f32, f32, f32), (f32, f32, f32), (f32, f32, f32)) {
    let para_sway = bones.para_cog;
    let rider_attach = (
        bones.para_attch.0 - bones.para_man.0,
        bones.para_attch.1 - bones.para_man.1,
        bones.para_attch.2 - bones.para_man.2,
    );
    let rider_sway = (
        para_sway.0 - rider_attach.0,
        para_sway.1 - rider_attach.1,
        para_sway.2 - rider_attach.2,
    );
    (rider_attach, rider_sway, para_sway)
}

/// C++ `ParachuteContain::calcSwayMtx` residual: rotate around offset pivot.
///
/// Applies roll about X then pitch about Y (C++ In_Place_Pre_Rotate_X/Y) to a
/// point expressed in object-local space relative to the pivot offset.
/// Returns the delta that should be applied to the object/rider position for
/// presentation (instance-matrix residual without full Mat4).
pub fn parachute_sway_point_delta(
    offset: (f32, f32, f32),
    pitch: f32,
    roll: f32,
    local_point: (f32, f32, f32),
) -> (f32, f32, f32) {
    // Translate to pivot-local, rotate roll(X) then pitch(Y), translate back.
    let px = local_point.0 - offset.0;
    let py = local_point.1 - offset.1;
    let pz = local_point.2 - offset.2;
    // Roll about X (host X axis): affects Y/Z.
    let (sr, cr) = roll.sin_cos();
    let y1 = py * cr - pz * sr;
    let z1 = py * sr + pz * cr;
    let x1 = px;
    // Pitch about Y (host Y axis): affects X/Z — C++ Pre_Rotate_Y uses pitch.
    // In C++ Z-up, Pre_Rotate_Y(pitch) tilts around world-Y. Host Y-up maps
    // C++ Y→host Z and C++ Z→host Y, so C++ Pre_Rotate_Y ≈ host rotate about Z.
    // Keep C++ axis names on host-mapped coords: pitch about "lateral" axis.
    // Practical residual: pitch tilts X/Y in host Y-up (forward/up plane).
    let (sp, cp) = pitch.sin_cos();
    let x2 = x1 * cp + y1 * sp;
    let y2 = -x1 * sp + y1 * cp;
    let z2 = z1;
    let out = (x2 + offset.0, y2 + offset.1, z2 + offset.2);
    (
        out.0 - local_point.0,
        out.1 - local_point.1,
        out.2 - local_point.2,
    )
}

/// Rider world residual position from parachute object + bone offsets.
///
/// C++ `positionRider`: rider.pos = para.pos + rider_attach_offset (no sway on
/// logic position; sway is drawable instance matrix). Host residual also
/// returns sway-applied presentation position for open-chute drawable parity.
pub fn parachute_rider_logic_position(
    para_pos: (f32, f32, f32),
    rider_attach: (f32, f32, f32),
) -> (f32, f32, f32) {
    (
        para_pos.0 + rider_attach.0,
        para_pos.1 + rider_attach.1,
        para_pos.2 + rider_attach.2,
    )
}

/// Presentation residual: rider logic pos + sway delta about rider_sway pivot.
pub fn parachute_rider_presentation_position(
    para_pos: (f32, f32, f32),
    rider_attach: (f32, f32, f32),
    rider_sway: (f32, f32, f32),
    pitch: f32,
    roll: f32,
) -> (f32, f32, f32) {
    let logic = parachute_rider_logic_position(para_pos, rider_attach);
    // Sway rotates the attach point about the rider sway pivot in object space.
    // Local attach point relative to parachute origin is rider_attach.
    let delta = parachute_sway_point_delta(rider_sway, pitch, roll, rider_attach);
    (logic.0 + delta.0, logic.1 + delta.1, logic.2 + delta.2)
}

/// Chute drawable presentation residual: identity when freefall / closed;
/// sway about PARA_COG when open.
pub fn parachute_chute_presentation_delta(
    para_sway: (f32, f32, f32),
    pitch: f32,
    roll: f32,
    chute_open: bool,
) -> (f32, f32, f32) {
    if !chute_open {
        return (0.0, 0.0, 0.0);
    }
    // Origin (0,0,0) rotated about para_sway pivot → delta for instance matrix.
    parachute_sway_point_delta(para_sway, pitch, roll, (0.0, 0.0, 0.0))
}

/// Convert InvulnerableTime ms → logic frames (30 FPS residual).
pub fn eject_pilot_invulnerable_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / EJECT_PILOT_LOGIC_FPS)).round() as u32
}

/// Absolute host frame when post-eject InvulnerableTime residual expires.
pub fn eject_pilot_invulnerable_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(EJECT_PILOT_INVULNERABLE_FRAMES.max(1))
}

/// Whether template is a residual USA Pilot infantry.
///
/// Fail-closed: name residual. Excludes weapons / science / debris / pathfinder.
pub fn is_pilot_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("pathfinder")
        || n.contains("ranger")
        || n.contains("colonel")
        || n.contains("burton")
        || n.contains("commandset")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testpilot" || n == "usa_pilot" || n == "americainfantrypilot" {
        return true;
    }
    // AmericaInfantryPilot / AirF_AmericaInfantryPilot / CINE_AmericaInfantryPilot
    n.contains("infantrypilot") || n.ends_with("pilot") && n.contains("america")
}

/// Residual pilot starting veterancy (VeterancyGainCreate StartingLevel = VETERAN).
pub fn pilot_default_veterancy() -> VeterancyLevel {
    VeterancyLevel::Veteran
}

/// Whether template is a residual USA vehicle with EjectPilotDie module.
///
/// Retail AmericaVehicle.ini / general variants: Humvee, Tomahawk, Crusader,
/// Paladin, Avenger, Microwave. Fail-closed name residual (not full DieMux).
pub fn is_eject_pilot_eligible_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Exclude drones / hulks / weapons / infantry pilots themselves.
    if n.contains("drone")
        || n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("infantry")
        || n.contains("pilot")
        || n.contains("dozer")
        || n.contains("sentry")
        || n.contains("chinook")
        || n.contains("comanche")
        || n.contains("raptor")
        || n.contains("stealthfighter")
        || n.contains("aurora")
        || n.contains("jet")
        || n.contains("helicopter")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testejectvehicle"
        || n == "testejectpilotvehicle"
        || n == "goldenhumvee"
        || n == "usa_humvee"
        || n == "usa_crusader"
        || n == "usa_paladin"
        || n == "usa_tomahawk"
        || n == "usa_avenger"
        || n == "usa_microwave"
    {
        return true;
    }
    n.contains("humvee")
        || n.contains("tomahawk")
        || n.contains("tankcrusader")
        || n.contains("tankpaladin")
        || n.contains("tankavenger")
        || n.contains("tankmicrowave")
        || (n.contains("crusader") && n.contains("tank"))
        || (n.contains("paladin") && n.contains("tank"))
        || (n.contains("avenger") && n.contains("tank"))
        || (n.contains("microwave") && (n.contains("tank") || n.contains("vehicle")))
}

/// Retail `VeterancyLevels = ALL -REGULAR` residual gate.
///
/// C++ DieMuxData::isDieApplicable → getVeterancyLevelFlag(m_veterancyLevels, level).
/// LEVEL_REGULAR / Rookie is excluded; Veteran / Elite / Heroic eject.
pub fn meets_eject_pilot_veterancy_gate(level: VeterancyLevel) -> bool {
    !matches!(level, VeterancyLevel::Rookie)
}

/// Retail `DeathTypes = ALL -CRUSHED -SPLATTED` residual gate.
///
/// C++ DieMuxData::isDieApplicable → getDamageTypeFlag(m_deathTypes, deathType).
/// DEATH_CRUSHED / DEATH_SPLATTED do not eject (crushed under tank / splat).
pub fn meets_eject_pilot_death_types_gate(death_type: HostDeathType) -> bool {
    !matches!(death_type, HostDeathType::Crushed | HostDeathType::Splatted)
}

/// Retail `ExemptStatus = HIJACKED` residual gate.
///
/// C++ DieMuxData::isDieApplicable → if object has HIJACKED status bits, skip.
/// Hijacked vehicles do not eject a pilot (driver already replaced).
pub fn meets_eject_pilot_exempt_status_gate(is_hijacked: bool) -> bool {
    !is_hijacked
}

/// Whether residual EjectPilotDie should fire on death.
///
/// Fail-closed: eligible template, not unmanned, not under construction,
/// vehicle kind residual (not structure / aircraft), VeterancyLevels ALL -REGULAR,
/// DeathTypes ALL -CRUSHED -SPLATTED, ExemptStatus HIJACKED.
pub fn can_eject_pilot_on_death(
    is_eligible_template: bool,
    is_unmanned: bool,
    under_construction: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    meets_veterancy_gate: bool,
    meets_death_types_gate: bool,
    meets_exempt_status_gate: bool,
) -> bool {
    is_eligible_template
        && !is_unmanned
        && !under_construction
        && is_vehicle
        && !is_aircraft
        && meets_veterancy_gate
        && meets_death_types_gate
        && meets_exempt_status_gate
}

/// Whether PilotFindVehicleUpdate residual may auto-scan this pilot.
///
/// C++: AI-only (`PLAYER_HUMAN` → UPDATE_SLEEP_FOREVER); idle AI only.
pub fn pilot_find_vehicle_scan_eligible(
    is_pilot: bool,
    is_alive: bool,
    is_idle: bool,
    is_ai_controlled: bool,
) -> bool {
    is_pilot && is_alive && is_idle && is_ai_controlled
}

/// Whether vehicle health meets PilotFindVehicleUpdate MinHealth residual.
///
/// C++ skips targets with `health < maxHealth * MinHealth` (default 0.5).
pub fn vehicle_meets_pilot_find_min_health(health: f32, max_health: f32, min_ratio: f32) -> bool {
    if max_health <= 0.0 {
        return false;
    }
    health >= max_health * min_ratio
}

/// Whether a candidate vehicle is a valid PilotFindVehicle target residual.
///
/// Host residual: recrewable unmanned path + CollideModule wouldLikeToCollideWith
/// gates (MinHealth + range preserved).
pub fn is_pilot_find_vehicle_target(
    recrewable: bool,
    health_ok: bool,
    in_scan_range: bool,
) -> bool {
    recrewable && health_ok && in_scan_range
}

/// C++ `VeterancyCrateCollide::isValidToExecute` / wouldLikeToCollideWith residual
/// gates used by PilotFindVehicleUpdate module iterate.
///
/// Host residual of:
/// - RequiredKindOf = VEHICLE / ForbiddenKindOf = DOZER
/// - not effectively dead
/// - not isSignificantlyAboveTerrain
/// - not isUsingAirborneLocomotor (IsPilot path)
/// - ExperienceTracker isTrainable + canGainExpForLevel(pilot levels)
///
/// Fail-closed: not full AI goal-object enter check from executeCrateBehavior.
pub fn pilot_collide_would_like_to_collide_with(
    is_alive: bool,
    is_vehicle: bool,
    is_dozer: bool,
    is_significantly_above_terrain: bool,
    is_airborne_locomotor: bool,
    is_trainable: bool,
    can_gain_exp_for_pilot_levels: bool,
) -> bool {
    is_alive
        && is_vehicle
        && !is_dozer
        && !is_significantly_above_terrain
        && !is_airborne_locomotor
        && is_trainable
        && can_gain_exp_for_pilot_levels
}

/// C++ `PartitionFilterPlayer(me->getControllingPlayer(), true)` residual used by
/// PilotFindVehicleUpdate::scanClosestTarget.
///
/// Host killpilot sets vehicle team Neutral while recording `unmanned_owner_team`.
/// Accept when:
/// - vehicle still same team as pilot, OR
/// - Neutral unmanned whose recorded owner matches pilot team.
///
/// Fail-closed: not full same-map PartitionFilterSameMapStatus; player Enter
/// recrew path is not gated by this residual (AI auto-scan only).
pub fn pilot_find_vehicle_same_player_ok(
    same_team: bool,
    is_neutral: bool,
    owner_matches_pilot: bool,
) -> bool {
    same_team || (is_neutral && owner_matches_pilot)
}

/// C++ `VeterancyCrateCollide::getLevelsToGain` with AddsOwnerVeterancy residual.
/// Pilot veterancy rank (Regular=0 → levels 0 blocked; Veteran=1, Elite=2, Heroic=3).
pub fn pilot_levels_to_gain(pilot_level: VeterancyLevel) -> u8 {
    veterancy_rank(pilot_level)
}

/// C++ ExperienceTracker::canGainExpForLevel residual.
///
/// Vehicle can absorb `levels` only if resulting rank stays within Heroic (3).
/// Fail-closed: Heroic vehicle cannot gain further levels → wouldLikeToCollideWith false.
pub fn vehicle_can_gain_exp_for_levels(vehicle_level: VeterancyLevel, levels: u8) -> bool {
    if levels == 0 {
        return false;
    }
    (veterancy_rank(vehicle_level) as u16 + levels as u16) <= 3
}

/// Combined PilotFindVehicle scan target residual (recrewable + MinHealth + range
/// + CollideModule wouldLikeToCollideWith).
pub fn is_pilot_find_vehicle_collide_target(
    recrewable: bool,
    health_ok: bool,
    in_scan_range: bool,
    collide_ok: bool,
) -> bool {
    is_pilot_find_vehicle_target(recrewable, health_ok, in_scan_range) && collide_ok
}

/// Whether current frame is a PilotFindVehicle scan tick residual.
pub fn pilot_find_vehicle_scan_frame(frame: u32) -> bool {
    frame.is_multiple_of(PILOT_FIND_VEHICLE_SCAN_FRAMES)
}

/// Whether PilotFindVehicle residual should issue base-center fallback.
///
/// C++: when `scanClosestTarget()` is null and `!m_didMoveToBase` and
/// `getAiBaseCenter` succeeds → `aiMoveToPosition` once.
pub fn should_pilot_base_center_fallback(
    found_vehicle: bool,
    did_move_to_base: bool,
    has_base_center: bool,
) -> bool {
    !found_vehicle && !did_move_to_base && has_base_center
}

/// Whether template has residual AutoFindHealingUpdate module.
///
/// Retail America infantry: Pilot / Ranger / MissileDefender / Pathfinder /
/// ColonelBurton (+ general variants). Fail-closed name residual (not full INI
/// module parse).
pub fn is_auto_find_healing_template(template_name: &str) -> bool {
    if is_pilot_template(template_name) {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missileweapon")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("commandset")
        || n.contains("flashbang")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testranger"
        || n == "usa_ranger"
        || n == "goldenranger"
        || n == "testmissiledefender"
        || n == "usa_missiledefender"
        || n == "testpathfinder"
        || n == "usa_pathfinder"
        || n == "testcolonelburton"
        || n == "usa_colonelburton"
        || n == "testburton"
    {
        return true;
    }
    // AmericaInfantryRanger / *MissileDefender / *Pathfinder / *ColonelBurton
    n.contains("infantryranger")
        || (n.contains("ranger") && n.contains("america"))
        || n.contains("missiledefender")
        || n.contains("infantrypathfinder")
        || (n.contains("pathfinder") && (n.contains("america") || n.contains("usa")))
        || n.contains("colonelburton")
}

/// C++ Thing::isSignificantlyAboveTerrain residual.
///
/// Host: height_above_terrain > -(3*3)*Gravity (576 with Gravity=-64).
pub fn is_significantly_above_terrain(height_above_terrain: f32) -> bool {
    height_above_terrain > significantly_above_terrain_threshold()
}

/// Whether EjectPilotDie should use air OCL (OCL_EjectPilotViaParachute residual).
///
/// C++: `isSignificantlyAboveTerrain() ? m_oclInAir : m_oclOnGround`.
/// Host also accepts `airborne_target` residual flag.
pub fn uses_air_eject_ocl(height_above_terrain: f32, airborne_target: bool) -> bool {
    airborne_target || is_significantly_above_terrain(height_above_terrain)
}

/// Residual air-eject spawn height (keep elevated; floor at threshold+1).
pub fn air_eject_spawn_height(death_height: f32) -> f32 {
    death_height.max(significantly_above_terrain_threshold() + 1.0)
}

/// Whether AmericaParachute residual should open after freefall OpenDist.
///
/// C++ ParachuteContain: open when fallen distance ≥ ParachuteOpenDist (100).
pub fn should_open_parachute(start_height: f32, current_height: f32) -> bool {
    (start_height - current_height) >= PARACHUTE_OPEN_DIST
}

/// C++ ParachuteContain low-altitude open fudge residual.
///
/// If `start_height - ground_height < 2 * ParachuteOpenDist`, return a fudged
/// start height of `ground + 2*OpenDist` so the chute can still open. Otherwise
/// return `start_height` unchanged.
pub fn fudge_parachute_start_height(start_height: f32, ground_height: f32) -> f32 {
    let min_span = PARACHUTE_LOW_ALTITUDE_OPEN_MULT * PARACHUTE_OPEN_DIST;
    if start_height - ground_height < min_span {
        ground_height + min_span
    } else {
        start_height
    }
}

/// Whether low-altitude open fudge residual was applied.
pub fn parachute_start_height_was_fudged(start_height: f32, ground_height: f32) -> bool {
    let min_span = PARACHUTE_LOW_ALTITUDE_OPEN_MULT * PARACHUTE_OPEN_DIST;
    start_height - ground_height < min_span
}

/// FreeFallDamage residual amount (max_health × FreeFallDamagePercent).
pub fn free_fall_damage_amount(max_health: f32) -> f32 {
    (max_health.max(0.0) * FREE_FALL_DAMAGE_PERCENT).max(0.0)
}

/// Whether FreeFallDamage residual applies (chute destroyed mid-air).
///
/// C++ ParachuteContain::onDie: if significantly above terrain, damage rider
/// with DAMAGE_FALLING / DEATH_SPLATTED for FreeFallDamagePercent max health.
pub fn should_apply_parachute_free_fall_damage(
    is_parachuting: bool,
    height_above_terrain: f32,
) -> bool {
    is_parachuting && is_significantly_above_terrain(height_above_terrain)
}

/// Advance parachute residual sink toward ground (y height axis).
///
/// Returns (new_height, landed). Open-chute residual rate (legacy helper).
/// Fail-closed linear sink (not full parachute physics).
pub fn tick_parachute_height(current_height: f32, ground_height: f32) -> (f32, bool) {
    tick_parachute_height_with_state(current_height, ground_height, true)
}

/// Advance parachute residual with freefall vs open-chute rates.
///
/// `chute_open == false` uses freefall rate; `true` uses open-chute sink.
/// Returns (new_height, landed).
pub fn tick_parachute_height_with_state(
    current_height: f32,
    ground_height: f32,
    chute_open: bool,
) -> (f32, bool) {
    if current_height <= ground_height + 0.01 {
        return (ground_height, true);
    }
    let rate = if chute_open {
        EJECT_PARACHUTE_SINK_PER_FRAME
    } else {
        EJECT_PARACHUTE_FREEFALL_PER_FRAME
    };
    let next = (current_height - rate).max(ground_height);
    let landed = next <= ground_height + 0.01;
    (if landed { ground_height } else { next }, landed)
}

/// Whether AutoFindHealingUpdate residual may auto-scan this unit.
///
/// C++: human players return early (no scan); AI idle only (busy path fail-closed).
/// `has_auto_find_healing` covers pilot + residual USA infantry module templates.
pub fn auto_find_healing_scan_eligible(
    has_auto_find_healing: bool,
    is_alive: bool,
    is_idle: bool,
    is_ai_controlled: bool,
) -> bool {
    has_auto_find_healing && is_alive && is_idle && is_ai_controlled
}

/// Whether health is low enough to seek healing residual.
///
/// C++ skips scan when `health > maxHealth * NeverHeal` (retail pilot NeverHeal 0.85).
pub fn health_needs_auto_find_healing(health: f32, max_health: f32, never_heal: f32) -> bool {
    if max_health <= 0.0 {
        return false;
    }
    health <= max_health * never_heal
}

/// Whether current frame is an AutoFindHealing scan tick residual.
pub fn auto_find_healing_scan_frame(frame: u32) -> bool {
    frame.is_multiple_of(AUTO_FIND_HEALING_SCAN_FRAMES)
}

/// Whether a candidate is a valid AutoFindHealing HealPad residual target.
pub fn is_auto_find_healing_target(is_heal_pad: bool, is_alive: bool, in_scan_range: bool) -> bool {
    is_heal_pad && is_alive && in_scan_range
}

/// Rank for residual veterancy transfer (higher wins).
pub fn veterancy_rank(level: VeterancyLevel) -> u8 {
    match level {
        VeterancyLevel::Rookie => 0,
        VeterancyLevel::Veteran => 1,
        VeterancyLevel::Elite => 2,
        VeterancyLevel::Heroic => 3,
    }
}

/// Whether residual vehicle can be recrewed by a pilot.
///
/// Fail-closed: live ground vehicle, unmanned, not under construction, not aircraft.
pub fn is_recrewable_unmanned_vehicle(
    is_alive: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_unmanned: bool,
    under_construction: bool,
    is_dozer: bool,
) -> bool {
    // Retail VeterancyCrateCollide ForbiddenKindOf = DOZER residual.
    is_alive && is_vehicle && !is_aircraft && is_unmanned && !under_construction && !is_dozer
}

/// Whether an enter command should take the pilot recrew residual path.
pub fn should_recrew_on_enter(is_pilot: bool, vehicle_recrewable: bool) -> bool {
    is_pilot && vehicle_recrewable
}

/// Merged veterancy after recrew: max(vehicle, pilot).
pub fn merged_recrew_veterancy(
    vehicle_level: VeterancyLevel,
    pilot_level: VeterancyLevel,
) -> VeterancyLevel {
    if veterancy_rank(pilot_level) >= veterancy_rank(vehicle_level) {
        pilot_level
    } else {
        vehicle_level
    }
}

/// Host residual honesty counters for USA Pilot recrew + EjectPilotDie.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostUsaPilotRegistry {
    /// Successful unmanned-vehicle recrews (pilot consumed).
    pub recrews: u32,
    /// Veterancy promotions applied onto recrewed vehicles.
    pub veterancy_transfers: u32,
    /// Successful EjectPilotDie residual pilot spawns on vehicle death.
    #[serde(default)]
    pub ejections: u32,
    /// Post-eject InvulnerableTime residual grants applied.
    #[serde(default)]
    pub invulnerable_grants: u32,
    /// Residual damage attempts blocked by InvulnerableTime.
    #[serde(default)]
    pub invulnerable_blocks: u32,
    /// PilotFindVehicleUpdate residual auto-scan Enter orders issued.
    #[serde(default)]
    pub find_vehicle_orders: u32,
    /// Rookie / REGULAR deaths skipped by VeterancyLevels gate residual.
    #[serde(default)]
    pub eject_veterancy_blocks: u32,
    /// PilotFindVehicleUpdate base-center fallback residual moves issued.
    #[serde(default)]
    pub base_center_moves: u32,
    /// AutoFindHealingUpdate residual SeekingHealing orders issued.
    #[serde(default)]
    pub auto_heal_orders: u32,
    /// EjectPilotDie air OCL parachute residual spawns (significantly above terrain).
    #[serde(default)]
    pub air_ejections: u32,
    /// Air-ejected pilot parachute residual landings completed.
    #[serde(default)]
    pub parachute_lands: u32,
    /// AutoFindHealingUpdate residual orders issued by non-pilot infantry.
    #[serde(default)]
    pub infantry_auto_heal_orders: u32,
    /// DeathTypes CRUSHED/SPLATTED residual blocks on EjectPilotDie.
    #[serde(default)]
    pub eject_death_type_blocks: u32,
    /// ExemptStatus HIJACKED residual blocks on EjectPilotDie.
    #[serde(default)]
    pub eject_hijacked_blocks: u32,
    /// PilotFindVehicle CollideModule wouldLikeToCollideWith residual rejects.
    #[serde(default)]
    pub find_vehicle_collide_rejects: u32,
    /// PilotFindVehicle PartitionFilterPlayer residual rejects (wrong owner).
    #[serde(default)]
    pub find_vehicle_player_rejects: u32,
    /// AmericaParachute residual chute-open events (past OpenDist freefall).
    #[serde(default)]
    pub parachute_opens: u32,
    /// Pitch/roll sway residual steps applied while chute open.
    #[serde(default)]
    pub parachute_sway_ticks: u32,
    /// Low-altitude open fudge residual applications.
    #[serde(default)]
    pub parachute_open_fudges: u32,
    /// FreeFallDamage residual applications (chute destroyed mid-air).
    #[serde(default)]
    pub free_fall_damages: u32,
}

impl HostUsaPilotRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_recrew(&mut self, transferred_veterancy: bool) {
        self.recrews = self.recrews.saturating_add(1);
        if transferred_veterancy {
            self.veterancy_transfers = self.veterancy_transfers.saturating_add(1);
        }
    }

    pub fn record_ejection(&mut self) {
        self.ejections = self.ejections.saturating_add(1);
    }

    pub fn record_invulnerable_grant(&mut self) {
        self.invulnerable_grants = self.invulnerable_grants.saturating_add(1);
    }

    pub fn record_invulnerable_block(&mut self) {
        self.invulnerable_blocks = self.invulnerable_blocks.saturating_add(1);
    }

    pub fn record_find_vehicle_order(&mut self) {
        self.find_vehicle_orders = self.find_vehicle_orders.saturating_add(1);
    }

    pub fn record_eject_veterancy_block(&mut self) {
        self.eject_veterancy_blocks = self.eject_veterancy_blocks.saturating_add(1);
    }

    pub fn record_base_center_move(&mut self) {
        self.base_center_moves = self.base_center_moves.saturating_add(1);
    }

    pub fn record_auto_heal_order(&mut self) {
        self.auto_heal_orders = self.auto_heal_orders.saturating_add(1);
    }

    pub fn record_air_ejection(&mut self) {
        // Caller also records a normal ejection; this only tags air OCL residual.
        self.air_ejections = self.air_ejections.saturating_add(1);
    }

    pub fn record_parachute_land(&mut self) {
        self.parachute_lands = self.parachute_lands.saturating_add(1);
    }

    pub fn record_infantry_auto_heal_order(&mut self) {
        // Caller may also use record_auto_heal_order for pilot; this tags non-pilot.
        self.infantry_auto_heal_orders = self.infantry_auto_heal_orders.saturating_add(1);
        self.auto_heal_orders = self.auto_heal_orders.saturating_add(1);
    }

    pub fn record_eject_death_type_block(&mut self) {
        self.eject_death_type_blocks = self.eject_death_type_blocks.saturating_add(1);
    }

    pub fn record_eject_hijacked_block(&mut self) {
        self.eject_hijacked_blocks = self.eject_hijacked_blocks.saturating_add(1);
    }

    pub fn record_find_vehicle_collide_reject(&mut self) {
        self.find_vehicle_collide_rejects = self.find_vehicle_collide_rejects.saturating_add(1);
    }

    pub fn record_find_vehicle_player_reject(&mut self) {
        self.find_vehicle_player_rejects = self.find_vehicle_player_rejects.saturating_add(1);
    }

    pub fn record_parachute_open(&mut self) {
        self.parachute_opens = self.parachute_opens.saturating_add(1);
    }

    pub fn record_parachute_sway_tick(&mut self) {
        self.parachute_sway_ticks = self.parachute_sway_ticks.saturating_add(1);
    }

    /// Residual honesty: at least one recrew completed.
    pub fn honesty_recrew_ok(&self) -> bool {
        self.recrews > 0
    }

    /// Residual honesty: recrew path with veterancy transfer observed.
    pub fn honesty_veterancy_transfer_ok(&self) -> bool {
        self.recrews > 0 && self.veterancy_transfers > 0
    }

    /// Residual honesty: at least one EjectPilotDie pilot spawn.
    pub fn honesty_eject_ok(&self) -> bool {
        self.ejections > 0
    }

    /// Residual honesty: InvulnerableTime residual granted on eject.
    pub fn honesty_invulnerable_ok(&self) -> bool {
        self.invulnerable_grants > 0
    }

    /// Residual honesty: InvulnerableTime blocked at least one damage attempt.
    pub fn honesty_invulnerable_block_ok(&self) -> bool {
        self.invulnerable_blocks > 0
    }

    /// Residual honesty: PilotFindVehicleUpdate issued at least one Enter order.
    pub fn honesty_find_vehicle_ok(&self) -> bool {
        self.find_vehicle_orders > 0
    }

    /// Residual honesty: VeterancyLevels REGULAR gate blocked at least one eject.
    pub fn honesty_eject_veterancy_gate_ok(&self) -> bool {
        self.eject_veterancy_blocks > 0
    }

    /// Residual honesty: PilotFindVehicle base-center fallback issued at least once.
    pub fn honesty_base_center_ok(&self) -> bool {
        self.base_center_moves > 0
    }

    /// Residual honesty: AutoFindHealingUpdate issued at least one SeekingHealing order.
    pub fn honesty_auto_heal_ok(&self) -> bool {
        self.auto_heal_orders > 0
    }

    /// Residual honesty: at least one air OCL parachute eject residual.
    pub fn honesty_air_eject_ok(&self) -> bool {
        self.air_ejections > 0
    }

    /// Residual honesty: at least one parachute residual landing completed.
    pub fn honesty_parachute_land_ok(&self) -> bool {
        self.parachute_lands > 0
    }

    /// Residual honesty: non-pilot infantry AutoFindHealing residual issued.
    pub fn honesty_infantry_auto_heal_ok(&self) -> bool {
        self.infantry_auto_heal_orders > 0
    }

    /// Residual honesty: DeathTypes CRUSHED/SPLATTED gate blocked at least once.
    pub fn honesty_eject_death_type_gate_ok(&self) -> bool {
        self.eject_death_type_blocks > 0
    }

    /// Residual honesty: ExemptStatus HIJACKED gate blocked at least once.
    pub fn honesty_eject_hijacked_gate_ok(&self) -> bool {
        self.eject_hijacked_blocks > 0
    }

    /// Residual honesty: DieMux residual (death type or hijacked) blocked eject.
    pub fn honesty_eject_die_mux_ok(&self) -> bool {
        self.eject_death_type_blocks > 0 || self.eject_hijacked_blocks > 0
    }

    /// Residual honesty: CollideModule residual rejected at least one candidate.
    pub fn honesty_find_vehicle_collide_ok(&self) -> bool {
        self.find_vehicle_collide_rejects > 0
    }

    /// Residual honesty: PartitionFilterPlayer residual rejected at least one candidate.
    pub fn honesty_find_vehicle_player_ok(&self) -> bool {
        self.find_vehicle_player_rejects > 0
    }

    /// Residual honesty: AmericaParachute residual opened chute at least once.
    pub fn honesty_parachute_open_ok(&self) -> bool {
        self.parachute_opens > 0
    }

    /// Residual honesty: pitch/roll sway residual stepped at least once.
    pub fn honesty_parachute_sway_ok(&self) -> bool {
        self.parachute_sway_ticks > 0
    }

    pub fn record_parachute_open_fudge(&mut self) {
        self.parachute_open_fudges = self.parachute_open_fudges.saturating_add(1);
    }

    pub fn record_free_fall_damage(&mut self) {
        self.free_fall_damages = self.free_fall_damages.saturating_add(1);
    }

    /// Residual honesty: low-altitude open fudge residual applied at least once.
    pub fn honesty_parachute_open_fudge_ok(&self) -> bool {
        self.parachute_open_fudges > 0
    }

    /// Residual honesty: FreeFallDamage residual applied at least once.
    pub fn honesty_free_fall_damage_ok(&self) -> bool {
        self.free_fall_damages > 0
    }

    /// Combined pilot residual honesty (recrew or eject path).
    pub fn honesty_pilot_ok(&self) -> bool {
        self.honesty_recrew_ok()
            || self.honesty_eject_ok()
            || self.honesty_find_vehicle_ok()
            || self.honesty_base_center_ok()
            || self.honesty_auto_heal_ok()
            || self.honesty_air_eject_ok()
            || self.honesty_parachute_land_ok()
            || self.honesty_parachute_open_ok()
            || self.honesty_parachute_sway_ok()
            || self.honesty_parachute_open_fudge_ok()
            || self.honesty_free_fall_damage_ok()
            || self.honesty_find_vehicle_player_ok()
    }
}

// --- Wave 61 residual honesty packs (retail INI constants) ---

/// Wave 61 residual honesty: pilot body / StartingLevel residual.
pub fn honesty_usa_pilot_body_residual_ok() -> bool {
    (PILOT_MAX_HEALTH - 100.0).abs() < 0.01
        && (PILOT_VISION_RANGE - 150.0).abs() < 0.01
        && (PILOT_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && PILOT_TRANSPORT_SLOT_COUNT == 1
        && (PILOT_PHYSICS_MASS - 5.0).abs() < 0.01
        && PILOT_STARTING_LEVEL == "VETERAN"
        && pilot_default_veterancy() == VeterancyLevel::Veteran
        && EJECT_PILOT_TEMPLATE == "AmericaInfantryPilot"
        && is_pilot_template(EJECT_PILOT_TEMPLATE)
}

/// Wave 61 residual honesty: EjectPilotDie + OCL_EjectPilot residual.
pub fn honesty_usa_pilot_ejection_residual_ok() -> bool {
    EJECT_PILOT_INVULNERABLE_MS == 2000
        && EJECT_PILOT_INVULNERABLE_FRAMES
            == eject_pilot_invulnerable_frames_from_ms(EJECT_PILOT_INVULNERABLE_MS)
        && EJECT_PILOT_INVULNERABLE_FRAMES == 60
        && (EJECT_GROUND_MIN_FORCE_MAG - 2.0).abs() < 0.01
        && (EJECT_GROUND_MAX_FORCE_MAG - 3.0).abs() < 0.01
        && (EJECT_AIR_MIN_FORCE_MAG - 10.0).abs() < 0.01
        && (EJECT_AIR_MAX_FORCE_MAG - 12.0).abs() < 0.01
        && (EJECT_FORCE_MIN_PITCH_DEG - 50.0).abs() < 0.01
        && (EJECT_FORCE_MAX_PITCH_DEG - 60.0).abs() < 0.01
        && EJECT_AIR_PUT_IN_CONTAINER == "AmericaParachute"
        && EJECT_INHERITS_VETERANCY
        && EJECT_REQUIRES_LIVE_PLAYER
        && EJECT_DEATH_TYPES.contains("-CRUSHED")
        && EJECT_DEATH_TYPES.contains("-SPLATTED")
        && EJECT_EXEMPT_STATUS == "HIJACKED"
        && EJECT_VETERANCY_LEVELS.contains("-REGULAR")
        && is_eject_pilot_eligible_template("AmericaVehicleHumvee")
        && !is_eject_pilot_eligible_template("AmericaInfantryPilot")
}

/// Wave 61 residual honesty: AmericaParachute residual matrix.
pub fn honesty_usa_pilot_parachute_residual_ok() -> bool {
    (PARACHUTE_OPEN_DIST_RETAIL_BASE - 25.0).abs() < 0.01
        && (PARACHUTE_OPEN_DIST_CINE - 100.0).abs() < 0.01
        && (PARACHUTE_OPEN_DIST - PARACHUTE_OPEN_DIST_CINE).abs() < 0.01
        && (FREE_FALL_DAMAGE_PERCENT - 0.5).abs() < 0.001
        && (PARACHUTE_LOW_ALTITUDE_OPEN_MULT - 2.0).abs() < 0.01
        && (PARACHUTE_PITCH_RATE_MAX_DEG_PER_SEC - 60.0).abs() < 0.01
        && (PARACHUTE_ROLL_RATE_MAX_DEG_PER_SEC - 60.0).abs() < 0.01
        && (PARACHUTE_PITCH_STIFFNESS - 0.02).abs() < 0.001
        && (PARACHUTE_ROLL_STIFFNESS - 0.02).abs() < 0.001
        && (PARACHUTE_PITCH_DAMPING - 0.01).abs() < 0.001
        && (PARACHUTE_ROLL_DAMPING - 0.01).abs() < 0.001
        && (PARACHUTE_LOW_ALTITUDE_DAMPING - 0.2).abs() < 0.001
        && (PARACHUTE_ALTITUDE_DAMP_START - 20.0).abs() < 0.01
        && (PARACHUTE_GEOMETRY_HEIGHT - 10.0).abs() < 0.01
        && (PARACHUTE_GEOMETRY_MAJOR_RADIUS - 15.0).abs() < 0.01
        && PILOT_PARACHUTE_OPEN_AUDIO == "ParachuteOpen"
        && (EJECT_PARACHUTE_SINK_PER_FRAME - 20.0).abs() < 0.01
        && (EJECT_PARACHUTE_FREEFALL_PER_FRAME - 40.0).abs() < 0.01
        && (significantly_above_terrain_threshold() - 576.0).abs() < 0.1
}

/// Wave 61 residual honesty: PilotFindVehicle base-center / return-to-base residual.
pub fn honesty_usa_pilot_return_to_base_residual_ok() -> bool {
    PILOT_FIND_VEHICLE_SCAN_RATE_MS == 1000
        && PILOT_FIND_VEHICLE_SCAN_FRAMES == 30
        && (PILOT_FIND_VEHICLE_SCAN_RANGE - 300.0).abs() < 0.01
        && (PILOT_FIND_VEHICLE_MIN_HEALTH - 0.5).abs() < 0.01
        && AUTO_FIND_HEALING_SCAN_RATE_MS == 1000
        && AUTO_FIND_HEALING_SCAN_FRAMES == 30
        && (AUTO_FIND_HEALING_SCAN_RANGE - 300.0).abs() < 0.01
        && (AUTO_FIND_HEALING_NEVER_HEAL - 0.85).abs() < 0.01
        && (AUTO_FIND_HEALING_ALWAYS_HEAL - 0.25).abs() < 0.01
        && !auto_find_healing_always_heal_busy_interrupt_live()
        && should_pilot_base_center_fallback(false, false, true)
        && !should_pilot_base_center_fallback(true, false, true)
        && !should_pilot_base_center_fallback(false, true, true)
        && !should_pilot_base_center_fallback(false, false, false)
}

/// Combined Wave 61 USA Pilot residual honesty pack.
pub fn honesty_usa_pilot_residual_pack_ok() -> bool {
    honesty_usa_pilot_body_residual_ok()
        && honesty_usa_pilot_ejection_residual_ok()
        && honesty_usa_pilot_parachute_residual_ok()
        && honesty_usa_pilot_return_to_base_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_name_matrix() {
        assert!(is_pilot_template("AmericaInfantryPilot"));
        assert!(is_pilot_template("AirF_AmericaInfantryPilot"));
        assert!(is_pilot_template("CINE_AmericaInfantryPilot"));
        assert!(is_pilot_template("TestPilot"));
        assert!(is_pilot_template("USA_Pilot"));
        assert!(!is_pilot_template("AmericaInfantryRanger"));
        assert!(!is_pilot_template("AmericaInfantryPathfinder"));
        assert!(!is_pilot_template("AmericaInfantryColonelBurton"));
        assert!(!is_pilot_template("Upgrade_AmericaPilot"));
        assert!(!is_pilot_template("GLAInfantryWorker"));
    }

    #[test]
    fn recrewable_gate() {
        assert!(is_recrewable_unmanned_vehicle(
            true, true, false, true, false, false
        ));
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, false, false, false
        )); // manned
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, true, true, false, false
        )); // aircraft
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, true, false, true
        )); // dozer forbidden
        assert!(!is_recrewable_unmanned_vehicle(
            false, true, false, true, false, false
        )); // dead
    }

    #[test]
    fn veterancy_merge() {
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Rookie, VeterancyLevel::Veteran),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Heroic, VeterancyLevel::Veteran),
            VeterancyLevel::Heroic
        );
        assert_eq!(pilot_default_veterancy(), VeterancyLevel::Veteran);
    }

    #[test]
    fn honesty_flags() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_pilot_ok());
        reg.record_recrew(true);
        assert!(reg.honesty_recrew_ok());
        assert!(reg.honesty_veterancy_transfer_ok());
        assert!(reg.honesty_pilot_ok());
        assert_eq!(reg.recrews, 1);
        assert_eq!(reg.veterancy_transfers, 1);
    }

    #[test]
    fn eject_pilot_template_matrix() {
        assert!(is_eject_pilot_eligible_template("AmericaVehicleHumvee"));
        assert!(is_eject_pilot_eligible_template("AmericaVehicleTomahawk"));
        assert!(is_eject_pilot_eligible_template("AmericaTankCrusader"));
        assert!(is_eject_pilot_eligible_template("AmericaTankPaladin"));
        assert!(is_eject_pilot_eligible_template("AmericaTankAvenger"));
        assert!(is_eject_pilot_eligible_template("AmericaTankMicrowave"));
        assert!(is_eject_pilot_eligible_template("SupW_AmericaTankCrusader"));
        assert!(is_eject_pilot_eligible_template("Lazr_AmericaTankPaladin"));
        assert!(is_eject_pilot_eligible_template(
            "AirF_AmericaVehicleHumvee"
        ));
        assert!(is_eject_pilot_eligible_template("TestEjectVehicle"));
        assert!(!is_eject_pilot_eligible_template("AmericaVehicleDozer"));
        assert!(!is_eject_pilot_eligible_template(
            "AmericaVehicleScoutDrone"
        ));
        assert!(!is_eject_pilot_eligible_template("AmericaInfantryPilot"));
        assert!(!is_eject_pilot_eligible_template("AmericaJetRaptor"));
        assert!(!is_eject_pilot_eligible_template("TestTank"));
        assert!(!is_eject_pilot_eligible_template("GLATankScorpion"));
    }

    #[test]
    fn eject_on_death_gate() {
        // eligible, not unmanned, not construction, vehicle, not aircraft,
        // vet gate, death types ok, not hijacked
        assert!(can_eject_pilot_on_death(
            true, false, false, true, false, true, true, true
        ));
        assert!(!can_eject_pilot_on_death(
            true, true, false, true, false, true, true, true
        )); // unmanned
        assert!(!can_eject_pilot_on_death(
            true, false, true, true, false, true, true, true
        )); // construction
        assert!(!can_eject_pilot_on_death(
            false, false, false, true, false, true, true, true
        )); // ineligible
        assert!(!can_eject_pilot_on_death(
            true, false, false, false, false, true, true, true
        )); // not vehicle
        assert!(!can_eject_pilot_on_death(
            true, false, false, true, true, true, true, true
        )); // aircraft
        assert!(!can_eject_pilot_on_death(
            true, false, false, true, false, false, true, true
        )); // REGULAR / Rookie blocked
        assert!(!can_eject_pilot_on_death(
            true, false, false, true, false, true, false, true
        )); // DeathTypes CRUSHED/SPLATTED
        assert!(!can_eject_pilot_on_death(
            true, false, false, true, false, true, true, false
        )); // ExemptStatus HIJACKED
    }

    #[test]
    fn eject_death_types_and_exempt_status_gates() {
        assert!(meets_eject_pilot_death_types_gate(HostDeathType::Normal));
        assert!(!meets_eject_pilot_death_types_gate(HostDeathType::Crushed));
        assert!(!meets_eject_pilot_death_types_gate(HostDeathType::Splatted));
        assert!(meets_eject_pilot_exempt_status_gate(false));
        assert!(!meets_eject_pilot_exempt_status_gate(true));
    }

    #[test]
    fn pilot_collide_would_like_to_collide_gates() {
        assert!(pilot_collide_would_like_to_collide_with(
            true, true, false, false, false, true, true
        ));
        assert!(!pilot_collide_would_like_to_collide_with(
            false, true, false, false, false, true, true
        )); // dead
        assert!(!pilot_collide_would_like_to_collide_with(
            true, false, false, false, false, true, true
        )); // not vehicle
        assert!(!pilot_collide_would_like_to_collide_with(
            true, true, true, false, false, true, true
        )); // dozer
        assert!(!pilot_collide_would_like_to_collide_with(
            true, true, false, true, false, true, true
        )); // above terrain
        assert!(!pilot_collide_would_like_to_collide_with(
            true, true, false, false, true, true, true
        )); // airborne locomotor
        assert!(!pilot_collide_would_like_to_collide_with(
            true, true, false, false, false, false, true
        )); // not trainable
        assert!(!pilot_collide_would_like_to_collide_with(
            true, true, false, false, false, true, false
        )); // cannot gain exp

        // Pilot VETERAN (levels=1) can enter Rookie..Elite; not Heroic.
        assert_eq!(pilot_levels_to_gain(VeterancyLevel::Veteran), 1);
        assert_eq!(pilot_levels_to_gain(VeterancyLevel::Elite), 2);
        assert_eq!(pilot_levels_to_gain(VeterancyLevel::Heroic), 3);
        assert_eq!(pilot_levels_to_gain(VeterancyLevel::Rookie), 0);
        assert!(vehicle_can_gain_exp_for_levels(VeterancyLevel::Rookie, 1));
        assert!(vehicle_can_gain_exp_for_levels(VeterancyLevel::Elite, 1));
        assert!(!vehicle_can_gain_exp_for_levels(VeterancyLevel::Heroic, 1));
        assert!(!vehicle_can_gain_exp_for_levels(VeterancyLevel::Rookie, 0));
        // Elite vehicle + Elite pilot levels (2) → 2+2=4 > 3 blocked.
        assert!(!vehicle_can_gain_exp_for_levels(VeterancyLevel::Elite, 2));
        // Rookie + Heroic pilot levels (3) → ok.
        assert!(vehicle_can_gain_exp_for_levels(VeterancyLevel::Rookie, 3));

        assert!(is_pilot_find_vehicle_collide_target(true, true, true, true));
        assert!(!is_pilot_find_vehicle_collide_target(
            true, true, true, false
        ));
    }

    #[test]
    fn eject_veterancy_levels_all_minus_regular() {
        assert!(!meets_eject_pilot_veterancy_gate(VeterancyLevel::Rookie));
        assert!(meets_eject_pilot_veterancy_gate(VeterancyLevel::Veteran));
        assert!(meets_eject_pilot_veterancy_gate(VeterancyLevel::Elite));
        assert!(meets_eject_pilot_veterancy_gate(VeterancyLevel::Heroic));
    }

    #[test]
    fn pilot_find_vehicle_gates() {
        assert!(pilot_find_vehicle_scan_eligible(true, true, true, true));
        assert!(!pilot_find_vehicle_scan_eligible(true, true, true, false)); // human
        assert!(!pilot_find_vehicle_scan_eligible(true, true, false, true)); // not idle
        assert!(!pilot_find_vehicle_scan_eligible(false, true, true, true)); // not pilot

        assert!(vehicle_meets_pilot_find_min_health(50.0, 100.0, 0.5));
        assert!(vehicle_meets_pilot_find_min_health(100.0, 100.0, 0.5));
        assert!(!vehicle_meets_pilot_find_min_health(49.0, 100.0, 0.5));
        assert!(!vehicle_meets_pilot_find_min_health(10.0, 0.0, 0.5));

        assert!(is_pilot_find_vehicle_target(true, true, true));
        assert!(!is_pilot_find_vehicle_target(true, false, true)); // low HP
        assert!(!is_pilot_find_vehicle_target(false, true, true)); // not recrewable
        assert!(!is_pilot_find_vehicle_target(true, true, false)); // out of range

        assert!(pilot_find_vehicle_scan_frame(0));
        assert!(pilot_find_vehicle_scan_frame(30));
        assert!(!pilot_find_vehicle_scan_frame(1));
        assert_eq!(PILOT_FIND_VEHICLE_SCAN_FRAMES, 30);
        assert!((PILOT_FIND_VEHICLE_SCAN_RANGE - 300.0).abs() < 0.001);
        assert!((PILOT_FIND_VEHICLE_MIN_HEALTH - 0.5).abs() < 0.001);
    }

    #[test]
    fn find_vehicle_and_vet_gate_honesty() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_find_vehicle_ok());
        assert!(!reg.honesty_eject_veterancy_gate_ok());
        reg.record_find_vehicle_order();
        assert!(reg.honesty_find_vehicle_ok());
        assert!(reg.honesty_pilot_ok());
        reg.record_eject_veterancy_block();
        assert!(reg.honesty_eject_veterancy_gate_ok());
        assert_eq!(reg.find_vehicle_orders, 1);
        assert_eq!(reg.eject_veterancy_blocks, 1);
    }

    #[test]
    fn pilot_base_center_fallback_gate() {
        assert!(should_pilot_base_center_fallback(false, false, true));
        assert!(!should_pilot_base_center_fallback(true, false, true)); // found vehicle
        assert!(!should_pilot_base_center_fallback(false, true, true)); // already moved
        assert!(!should_pilot_base_center_fallback(false, false, false)); // no base
    }

    #[test]
    fn auto_find_healing_gates() {
        assert!(auto_find_healing_scan_eligible(true, true, true, true));
        assert!(!auto_find_healing_scan_eligible(true, true, true, false)); // human
        assert!(!auto_find_healing_scan_eligible(true, true, false, true)); // not idle
        assert!(!auto_find_healing_scan_eligible(false, true, true, true)); // no module
                                                                            // AlwaysHeal busy-interrupt is dead code in retail C++ — host residual closed as idle-only.
        assert!(!auto_find_healing_always_heal_busy_interrupt_live());
        assert!((AUTO_FIND_HEALING_ALWAYS_HEAL - 0.25).abs() < 0.001);

        assert!(is_auto_find_healing_template("AmericaInfantryPilot"));
        assert!(is_auto_find_healing_template("AmericaInfantryRanger"));
        assert!(is_auto_find_healing_template(
            "AmericaInfantryMissileDefender"
        ));
        assert!(is_auto_find_healing_template("AmericaInfantryPathfinder"));
        assert!(is_auto_find_healing_template(
            "AmericaInfantryColonelBurton"
        ));
        assert!(is_auto_find_healing_template("AirF_AmericaInfantryRanger"));
        assert!(is_auto_find_healing_template("TestRanger"));
        assert!(!is_auto_find_healing_template("GLAInfantryRebel"));
        assert!(!is_auto_find_healing_template("ChinaInfantryRedguard"));
        assert!(!is_auto_find_healing_template(
            "RangerFlashBangGrenadeWeapon"
        ));
        assert!(!is_auto_find_healing_template(
            "Upgrade_AmericaRangerFlashBangGrenade"
        ));

        assert!(health_needs_auto_find_healing(85.0, 100.0, 0.85));
        assert!(health_needs_auto_find_healing(50.0, 100.0, 0.85));
        assert!(!health_needs_auto_find_healing(86.0, 100.0, 0.85));
        assert!(!health_needs_auto_find_healing(100.0, 100.0, 0.85));
        assert!(!health_needs_auto_find_healing(10.0, 0.0, 0.85));

        assert!(is_auto_find_healing_target(true, true, true));
        assert!(!is_auto_find_healing_target(false, true, true));
        assert!(!is_auto_find_healing_target(true, false, true));
        assert!(!is_auto_find_healing_target(true, true, false));

        assert!(auto_find_healing_scan_frame(0));
        assert!(auto_find_healing_scan_frame(30));
        assert!(!auto_find_healing_scan_frame(1));
        assert_eq!(AUTO_FIND_HEALING_SCAN_FRAMES, 30);
        assert!((AUTO_FIND_HEALING_SCAN_RANGE - 300.0).abs() < 0.001);
        assert!((AUTO_FIND_HEALING_NEVER_HEAL - 0.85).abs() < 0.001);
        assert!((AUTO_FIND_HEALING_ALWAYS_HEAL - 0.25).abs() < 0.001);
    }

    #[test]
    fn air_eject_parachute_gates() {
        let thr = significantly_above_terrain_threshold();
        assert!((thr - 576.0).abs() < 0.001);
        assert!(!is_significantly_above_terrain(0.0));
        assert!(!is_significantly_above_terrain(thr));
        assert!(is_significantly_above_terrain(thr + 1.0));
        assert!(uses_air_eject_ocl(thr + 1.0, false));
        assert!(uses_air_eject_ocl(0.0, true)); // airborne_target residual
        assert!(!uses_air_eject_ocl(0.0, false));
        assert!((air_eject_spawn_height(0.0) - (thr + 1.0)).abs() < 0.001);
        assert!((air_eject_spawn_height(700.0) - 700.0).abs() < 0.001);

        let (h1, landed1) = tick_parachute_height(thr + 1.0, 0.0);
        assert!(!landed1);
        assert!(h1 < thr + 1.0);
        // Sink until land.
        let mut h = thr + 1.0;
        let mut landed = false;
        for _ in 0..100 {
            let (nh, l) = tick_parachute_height(h, 0.0);
            h = nh;
            if l {
                landed = true;
                break;
            }
        }
        assert!(landed);
        assert!((h - 0.0).abs() < 0.01);
        let (ground, land_ground) = tick_parachute_height(0.0, 0.0);
        assert!(land_ground);
        assert!((ground - 0.0).abs() < 0.01);

        // AmericaParachute OpenDist freefall residual.
        assert!((PARACHUTE_OPEN_DIST - 100.0).abs() < 0.001);
        // Low-altitude open fudge residual (2×OpenDist).
        assert!((PARACHUTE_LOW_ALTITUDE_OPEN_MULT - 2.0).abs() < 0.001);
        assert!(parachute_start_height_was_fudged(50.0, 0.0));
        assert!(!parachute_start_height_was_fudged(250.0, 0.0));
        assert!((fudge_parachute_start_height(50.0, 0.0) - 200.0).abs() < 0.001);
        assert!((fudge_parachute_start_height(250.0, 0.0) - 250.0).abs() < 0.001);
        // FreeFallDamage residual (default 50% max health).
        assert!((FREE_FALL_DAMAGE_PERCENT - 0.5).abs() < 0.001);
        assert!((free_fall_damage_amount(100.0) - 50.0).abs() < 0.001);
        assert!(should_apply_parachute_free_fall_damage(true, 600.0));
        assert!(!should_apply_parachute_free_fall_damage(false, 600.0));
        assert!(!should_apply_parachute_free_fall_damage(true, 10.0));
        assert!(!should_open_parachute(700.0, 650.0)); // fallen 50 < 100
        assert!(should_open_parachute(700.0, 600.0)); // fallen 100
        assert!(should_open_parachute(700.0, 500.0));
        let (ff, _) = tick_parachute_height_with_state(700.0, 0.0, false);
        let (open, _) = tick_parachute_height_with_state(700.0, 0.0, true);
        assert!(
            (700.0 - ff) > (700.0 - open) + 0.01,
            "freefall residual must sink faster than open chute"
        );
        assert!((EJECT_PARACHUTE_FREEFALL_PER_FRAME - 40.0).abs() < 0.001);
        assert!((EJECT_PARACHUTE_SINK_PER_FRAME - 20.0).abs() < 0.001);
        assert!(!PILOT_PARACHUTE_OPEN_AUDIO.is_empty());

        // Pitch/roll sway residual matrix (ParachuteContain + ParachuteLocomotor).
        let rate_max = parachute_rate_max_rads_per_frame();
        assert!((rate_max - std::f32::consts::PI / 90.0).abs() < 1e-6);
        assert!((parachute_initial_pitch_rate() - rate_max * 0.5).abs() < 1e-6);
        assert!((parachute_initial_roll_rate() + rate_max * 0.5).abs() < 1e-6);
        assert!((PARACHUTE_PITCH_STIFFNESS - 0.02).abs() < 0.001);
        assert!((PARACHUTE_ROLL_STIFFNESS - 0.02).abs() < 0.001);
        assert!((PARACHUTE_PITCH_DAMPING - 0.01).abs() < 0.001);
        assert!((PARACHUTE_ROLL_DAMPING - 0.01).abs() < 0.001);
        assert!((PARACHUTE_LOW_ALTITUDE_DAMPING - 0.2).abs() < 0.001);
        assert!((PARACHUTE_ALTITUDE_DAMP_START - 20.0).abs() < 0.001);
        // Open-chute residual: seed rates integrate into non-zero pitch/roll.
        let (p1, r1, pr1, rr1) = tick_parachute_sway(
            0.0,
            0.0,
            parachute_initial_pitch_rate(),
            parachute_initial_roll_rate(),
            100.0, // high altitude → no low-alt damp
        );
        assert!(
            p1.abs() > 1e-6,
            "pitch residual must leave zero after one step"
        );
        assert!(
            r1.abs() > 1e-6,
            "roll residual must leave zero after one step"
        );
        // Low-altitude damping residual damps rates more aggressively.
        let (_, _, pr_hi, _) = tick_parachute_sway(0.0, 0.0, rate_max, 0.0, 100.0);
        let (_, _, pr_lo, _) = tick_parachute_sway(0.0, 0.0, rate_max, 0.0, 10.0);
        assert!(
            pr_lo.abs() < pr_hi.abs(),
            "LowAltitudeDamping residual must reduce |pitch_rate| more near ground ({pr_lo} vs {pr_hi})"
        );
        // Many frames: spring/damper residual must not explode.
        let mut p = 0.0;
        let mut r = 0.0;
        let mut pr = parachute_initial_pitch_rate();
        let mut rr = parachute_initial_roll_rate();
        for _ in 0..600 {
            let (np, nr, npr, nrr) = tick_parachute_sway(p, r, pr, rr, 100.0);
            p = np;
            r = nr;
            pr = npr;
            rr = nrr;
        }
        assert!(p.is_finite() && r.is_finite() && pr.is_finite() && rr.is_finite());
        assert!(
            p.abs() < 2.0 && r.abs() < 2.0,
            "sway residual must stay bounded"
        );
        // Silence unused warning for intermediate rate after one high step.
        let _ = (pr1, rr1);

        // Bone PARA_COG / PARA_ATTCH / PARA_MAN residual matrix.
        assert_eq!(PARACHUTE_BONE_PARA_COG, "PARA_COG");
        assert_eq!(PARACHUTE_BONE_PARA_ATTCH, "PARA_ATTCH");
        assert_eq!(PARACHUTE_BONE_PARA_MAN, "PARA_MAN");
        assert!((PARACHUTE_GEOMETRY_HEIGHT - 10.0).abs() < 0.001);
        assert!((PARACHUTE_GEOMETRY_MAJOR_RADIUS - 15.0).abs() < 0.001);
        let bones = parachute_host_bone_offsets();
        assert!(bones.para_cog.1 > bones.para_attch.1, "COG above ATTCH");
        let (rider_attach, rider_sway, para_sway) = parachute_offsets_from_bones(bones);
        // Rider hangs below chute attach by PARA_MAN height.
        assert!(
            rider_attach.1 < 0.0,
            "rider attach Y residual below chute origin (got {})",
            rider_attach.1
        );
        assert!((para_sway.1 - bones.para_cog.1).abs() < 0.001);
        let para_pos = (100.0_f32, 200.0, 0.0);
        let logic = parachute_rider_logic_position(para_pos, rider_attach);
        assert!((logic.0 - 100.0).abs() < 0.001);
        assert!((logic.1 - (200.0 + rider_attach.1)).abs() < 0.001);
        // Non-zero pitch/roll presentation sway residual moves rider off logic pos.
        let pitch = 0.1_f32;
        let roll = -0.05_f32;
        let pres =
            parachute_rider_presentation_position(para_pos, rider_attach, rider_sway, pitch, roll);
        let dx = pres.0 - logic.0;
        let dy = pres.1 - logic.1;
        let dz = pres.2 - logic.2;
        let d2 = (dx * dx + dy * dy + dz * dz).sqrt();
        assert!(
            d2 > 1e-4,
            "open-chute presentation sway residual must displace rider (d={d2})"
        );
        // Closed chute: presentation delta zero.
        let closed = parachute_chute_presentation_delta(para_sway, pitch, roll, false);
        assert_eq!(closed, (0.0, 0.0, 0.0));
        let open_delta = parachute_chute_presentation_delta(para_sway, pitch, roll, true);
        let od = open_delta.0.abs() + open_delta.1.abs() + open_delta.2.abs();
        assert!(od > 1e-4, "open chute COG sway residual must be non-zero");
        // Identity sway (pitch=roll=0) → zero delta.
        let id = parachute_sway_point_delta(para_sway, 0.0, 0.0, (0.0, 0.0, 0.0));
        assert!(id.0.abs() < 1e-6 && id.1.abs() < 1e-6 && id.2.abs() < 1e-6);
    }

    #[test]
    fn pilot_find_vehicle_same_player_gates() {
        // Same team residual.
        assert!(pilot_find_vehicle_same_player_ok(true, false, false));
        // Neutral + matching owner residual.
        assert!(pilot_find_vehicle_same_player_ok(false, true, true));
        // Neutral + wrong / unknown owner residual rejects.
        assert!(!pilot_find_vehicle_same_player_ok(false, true, false));
        // Enemy team residual rejects.
        assert!(!pilot_find_vehicle_same_player_ok(false, false, false));
        assert!(!pilot_find_vehicle_same_player_ok(false, false, true));
    }

    #[test]
    fn base_center_and_auto_heal_honesty() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_base_center_ok());
        assert!(!reg.honesty_auto_heal_ok());
        reg.record_base_center_move();
        assert!(reg.honesty_base_center_ok());
        assert!(reg.honesty_pilot_ok());
        reg.record_auto_heal_order();
        assert!(reg.honesty_auto_heal_ok());
        assert_eq!(reg.base_center_moves, 1);
        assert_eq!(reg.auto_heal_orders, 1);
        reg.record_air_ejection();
        assert!(reg.honesty_air_eject_ok());
        assert_eq!(reg.air_ejections, 1);
        reg.record_parachute_land();
        assert!(reg.honesty_parachute_land_ok());
        assert_eq!(reg.parachute_lands, 1);
        reg.record_parachute_open();
        assert!(reg.honesty_parachute_open_ok());
        assert_eq!(reg.parachute_opens, 1);
        reg.record_infantry_auto_heal_order();
        assert!(reg.honesty_infantry_auto_heal_ok());
        assert_eq!(reg.infantry_auto_heal_orders, 1);
        assert_eq!(reg.auto_heal_orders, 2);
        reg.record_eject_death_type_block();
        assert!(reg.honesty_eject_death_type_gate_ok());
        assert!(reg.honesty_eject_die_mux_ok());
        reg.record_eject_hijacked_block();
        assert!(reg.honesty_eject_hijacked_gate_ok());
        assert_eq!(reg.eject_death_type_blocks, 1);
        assert_eq!(reg.eject_hijacked_blocks, 1);
        reg.record_find_vehicle_collide_reject();
        assert!(reg.honesty_find_vehicle_collide_ok());
        assert_eq!(reg.find_vehicle_collide_rejects, 1);
        reg.record_find_vehicle_player_reject();
        assert!(reg.honesty_find_vehicle_player_ok());
        assert_eq!(reg.find_vehicle_player_rejects, 1);
    }

    #[test]
    fn eject_honesty_alone_is_pilot_ok() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_pilot_ok());
        reg.record_ejection();
        assert!(reg.honesty_eject_ok());
        assert!(reg.honesty_pilot_ok());
        assert_eq!(reg.ejections, 1);
    }

    #[test]
    fn invulnerable_time_frames_match_retail() {
        assert_eq!(EJECT_PILOT_INVULNERABLE_MS, 2000);
        assert_eq!(EJECT_PILOT_INVULNERABLE_FRAMES, 60);
        assert_eq!(eject_pilot_invulnerable_frames_from_ms(2000), 60);
        assert_eq!(eject_pilot_invulnerable_until_frame(10), 70);
        assert_eq!(eject_pilot_invulnerable_frames_from_ms(0), 0);
    }

    #[test]
    fn invulnerable_honesty_flags() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_invulnerable_ok());
        reg.record_invulnerable_grant();
        assert!(reg.honesty_invulnerable_ok());
        assert!(!reg.honesty_invulnerable_block_ok());
        reg.record_invulnerable_block();
        assert!(reg.honesty_invulnerable_block_ok());
        assert_eq!(reg.invulnerable_grants, 1);
        assert_eq!(reg.invulnerable_blocks, 1);
    }

    #[test]
    fn usa_pilot_residual_pack_honesty_wave61() {
        assert!(honesty_usa_pilot_body_residual_ok());
        assert!(honesty_usa_pilot_ejection_residual_ok());
        assert!(honesty_usa_pilot_parachute_residual_ok());
        assert!(honesty_usa_pilot_return_to_base_residual_ok());
        assert!(honesty_usa_pilot_residual_pack_ok());
        assert_eq!(eject_pilot_invulnerable_frames_from_ms(2000), 60);
        assert!((PARACHUTE_OPEN_DIST_RETAIL_BASE - 25.0).abs() < 0.01);
        assert!((PARACHUTE_OPEN_DIST - 100.0).abs() < 0.01);
        assert!((PILOT_MAX_HEALTH - 100.0).abs() < 0.01);
        assert!((PILOT_VISION_RANGE - 150.0).abs() < 0.01);
        assert!((PILOT_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01);
    }
}
