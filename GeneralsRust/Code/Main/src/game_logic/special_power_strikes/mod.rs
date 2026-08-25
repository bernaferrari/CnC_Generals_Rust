//! Host special-power / superweapon strike residual.
//!
//! Residual slice: host `DoSpecialPower` for DaisyCutter / A10 / ScudStorm /
//! ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship / CarpetBomb /
//! ArtilleryBarrage / CruiseMissile queues a real strike that completes with
//! area damage on host GameLogic objects. NuclearMissile also spawns a residual
//! radiation field (`NukeRadiationFieldWeapon`) that ticks after impact.
//! AnthraxBomb also spawns a residual toxin field
//! (`AnthraxBombPoisonFieldWeapon` / `OCL_PoisonFieldAnthraxBomb`) that ticks
//! after impact. ScudStorm is a delayed multi-missile residual
//! (`ScudStormWeapon` ClipSize 9 / ScatterTarget + `ScudStormDamageWeapon`
//! primary/secondary blast) that also spawns residual LargePoisonField toxin
//! ticks after each missile. SpectreGunship completes orbit insertion with no
//! one-shot blast, then spawns a residual orbit field (`SpectreHowitzerGun`
//! residual) that periodically damages in `AttackAreaRadius` for science-tier
//! `OrbitTime` (L1/L2/L3). ParticleCannon (Particle Uplink) completes charge
//! residual with no one-shot blast, then spawns a residual continuous beam field
//! (`ParticleUplinkCannonUpdate` TotalFiringTime / TotalDamagePulses /
//! DamagePerSecond residual) that pulses damage at the target for the beam dwell.
//! CarpetBomb is a delayed multi-strike line residual (`SUPERWEAPON_CarpetBomb` /
//! `CarpetBombWeapon`): after bomber approach delay, applies explosive damage at
//! DropDelay-staggered epicenters along a line through the target with DropVariance
//! residual scatter (fail-closed vs full B52 OCL DeliverPayload transport Object).
//! ArtilleryBarrage is a delayed multi-shell scatter residual
//! (`SUPERWEAPON_ArtilleryBarrage1` / `ArtilleryBarrageDamageWeapon`): after
//! DelayDeliveryMax residual, applies explosive damage at WeaponErrorRadius-
//! scattered shell epicenters with per-shell DelayDelivery stagger (fail-closed
//! vs full ChinaArtilleryCannon OCL DeliverPayload transport Object).
//! CruiseMissile is a delayed loft-to-target residual
//! (`SUPR_SPECIAL_CRUISE_MISSILE` / `SupW_CruiseMissile` /
//! `SUPERWEAPON_CruiseMissile` / `MOABDetonationWeapon`): after NeutronMissile
//! loft residual delay, applies MOAB area damage at the target (fail-closed vs
//! full NeutronMissileUpdate door/loft path / OCL FireWeapon projectile /
//! MOABFlameWeapon secondary). Pending strikes (absolute `impact_frame`) are
//! captured in `WorldSnapshot.special_power_strikes` so mid-flight save/load
//! continues remaining delay and still fires impact / orbit / beam residual.
//!
//! Fail-closed: not full retail OCL / NeutronMissileUpdate flight / multi-blast
//! SlowDeath wave / multiplayer superweapon parity or C++ SpecialPowerModule
//! Xfer tables. Radiation / toxin / Spectre orbit / PUC beam residual is a
//! single host field (not full HazardousMaterialArmor / cleanup-hazard object
//! stack / SpectreGunshipUpdate continuous-fire ROF + ContinuousFireCoast residual
//! (host residual; SpectreHowitzerShell projectile residual closed at shell
//! spawn/FireFX/detonation/HeightDie InitialDelay + DumbProjectileBehavior /
//! Physics mass / InstantDeath path honesty — not full W3D shell drawable Object);
//! MODELCONDITION CONTINUOUS_FIRE_* honesty residual closed) /
//! ParticleUplinkCannonUpdate outer-node + connector laser residual closed at
//! STATUS_FIRING; intensity schedule residual closed
//! (CHARGING/PREPARING/ALMOST_READY/READY Light→Medium→Intense client residual);
//! OuterBeamWidth × width_scalar orbital laser draw / getCurrentLaserRadius
//! retail damage-radius formula residual closed (host combat peak =
//! OuterBeamWidth×0.5×DamageRadiusScalar = 44.2); manual beam driving residual closed (override destination +
//! ManualDrivingSpeed / ManualFastDrivingSpeed / DoubleClickToFastDriveDelay);
//! DamagePulseRemnant trail residual closed; swath sine residual closed;
//! WidthGrow damage-radius grow+hold+decay shrink residual closed;
//! TotalScorchMarks/RevealRange residual closed). CarpetBomb residual is
//! DropDelay-staggered multi-point blasts with DropVariance residual scatter
//! (not full AmericaJetB52 Object / pathfinder). ArtilleryBarrage residual is
//! WeaponErrorRadius-scattered shells with per-shell DelayDelivery stagger and
//! science-tier FormationSize (once-at-queue pure ADC GameLogicRandomValueReal residual
//! stored on the strike at queue; plan_due uses stored epicenters/shell frames —
//! fail-closed vs live mid-sim global stream mutation / full ChinaArtilleryCannon
//! transport Object). ScudStorm residual is ClipSize-9
//! ScatterTarget multi-missile + ScudStormDamageWeapon primary/secondary +
//! LargePoisonField residual, with Anthrax Beta/Gamma upgraded Secondary 200 +
//! upgraded poison 25 residual; PreAttack PER_CLIP + FireFX/IgnitionFX/launch
//! residual + Chem FXBone residual + ScudStormMissile MissileAIUpdate loft /
//! HeightDie / PreferredHeight spring residual closed (not full ThingFactory
//! projectile Object / live MissileAIUpdate physics flight sim).
//! OuterBeamWidth multi-beam NumBeams + ScrollRate / TilingScalar residual closed
//! (host tracks multi-beam cylinder count + scroll UV honesty).
//! Multi-beam soft-edge width/alpha/color lerp residual closed
//! (W3DLaserDraw scale = i/(NumBeams-1) cylinders + tile-factor honesty;
//! fail-closed vs full SegLineRenderer GPU texture atlas submit).
//! ScudStormMissile ballistic flight residual closed
//! (locomotor speed/accel + OnlyWhenMovingDown/SnapToGround + model UBScudStrm_M
//! + geometry residual; fail-closed vs full ThingFactory Object physics).
//! SpectreHowitzerShell W3D ModelDraw residual closed
//! (model AVSpectreShell1 + Scale/Shadow/MaxHealth honesty; fail-closed vs full
//! W3D drawable Object / live Physics flight).
//! Outer-node bone layout residual closed (FX01..FX05 ring positions host residual;
//! fail-closed vs full W3D bone-world extract / LaserUpdate drawable matrix).
//! LaserUpdate client residual closed (initLaser ground-to-orbit / orbit-to-target
//! start/end + drawable midpoint + WidthGrow sizeDelta widen/decay scalar + dirty
//! + orbit altitude 500; fail-closed vs full LaserUpdate GPU drawable / shroud).
//! Medium connector soft-edge residual closed (POSTFIRE NumBeams 4, 0.4→1.2).
//! OrbitalLaser VisionRange/ShroudClearing residual closed (100/120 design params).
//! ScudStormMissile Geometry residual closed (Cylinder / radius 7 / height 30).
//! SpectreHowitzerShell InstantDeath LASERED OCL residual closed
//! (OCL_GenericMissileDisintegrate). Multi-locale LanguageId CSF path residual
//! closed (English/German/French/Spanish/Italian path table; fail-closed vs full
//! multi-locale CSF boot UI for all LanguageId assets).
//! Soft-edge RGB innerAlpha premultiply + additive/tiled residual closed.
//! Scud/Howitzer object-params residual closed (VisionRange/KindOf/Armor).
//! Wave 34 residual closed: Scud FireWeaponWhenDead death-weapon matrix +
//! InitialHealth/EditorSorting/OkToChangeModelColor + Locomotor Appearance THRUST
//! Surfaces AIR; Howitzer TargetHeightIncludesStructures=No + InitialHealth +
//! InstantDeath GENERIC death FX; OrbitalLaser KindOf IMMOBILE + Segments/ArcHeight
//! defaults; single-beam RGB × innerAlpha residual (NumBeams==1 path).
//! Wave 36 residual closed: Scud DestroyDie + Locomotor template name + Armor
//! DamageFX=None; SpectreHowitzerShellLocomotor template residual (AIR/THRUST
//! MinSpeed/Accel/TurnRate); Howitzer Armor DamageFX=None; connector KindOf
//! IMMOBILE + MaxIntensity/Fade defaults + Tile=No; TrailRemnant KindOf
//! ImmortalBody residual; DisplayString setFont residual (graphics).
//! Wave 38 residual closed: Scud DeathWeapon FireOCL PoisonField + Locomotor
//! SpeedDamaged/MinSpeed/MaxThrustAngle residual; SpectreHowitzerGun
//! AcceptableAimDelta/AttackRange residual; DisplayString getTextLength residual;
//! multi-locale UK LanguageId CSF path residual (graphics).
//! Wave 39 residual closed: Scud DeathWeapon damage table (Primary/Secondary/
//! DamageType/DeathType/WeaponSpeed/FireFX/AttackRange); SpectreHowitzerGun fire
//! residual (DelayBetweenShots 777ms / DamageType / DeathType / RadiusDamageAffects /
//! FireFX / FireSound / ClipSize / GroupMovementPriority MOVES_BACK); TrailRemnant
//! FireWeaponUpdate + DeletionUpdate residual (DamageType PARTICLE_BEAM / DeathType
//! BURNED / MinLifetime==MaxLifetime 4000ms); DisplayString getText/reset/appendChar/
//! removeLastChar/getWidth residual + multi-locale Japanese/Jabber/Korean/Unknown
//! LanguageId residual (graphics; fail-closed vs full CSF boot UI).
//! Wave 40 residual closed: ScudStormWeapon launch residual (ClipSize 9 /
//! ClipReloadTime 10000ms / AutoReloadsClip / ScatterTargetScalar+table /
//! AcceptableAimDelta 180 / ProjectileCollidesWith STRUCTURES / DelayBetweenShots
//! Min:Max 100:1000 / Death ClipReloadTime 0); SpectreHowitzerGun anti residual
//! (AntiAirborne*/AntiMissile No + ProjectileObject + ContinuousFireCoast 2000ms);
//! DisplayString getSize/setWordWrap/setWordWrapCentered/setUseHotkey/setClipRegion
//! residual (graphics).
//! Wave 41 residual closed: complete Wave 40 honesty/application/test wiring +
//! DisplayString getSize/word-wrap/hotkey/clip residual host-testable path +
//! Snapshot/Xfer anti/launch residual fields.
//! Wave 42 residual closed: ScudStormWeapon special residual (PrimaryDamage 0 /
//! AttackRange 999999 / DamageType/DeathType / WeaponSpeed 99999 / ScatterRadius 0 /
//! PreAttackType PER_CLIP); SpectreGattlingGun anti/fire residual (Anti* No /
//! ProjectileObject NONE / DamageType Gattling / PrimaryDamageRadius 0 / Clip 0);
//! DisplayString getFont/draw residual (graphics).
//! Wave 43 residual closed: complete Wave 42 host residual + MissileAIUpdate defaults
//! residual (IgnitionDelay 0 / UseWeaponSpeed No / DetonateOnNoFuel No / LockDistance 75 /
//! DistanceScatterWhenJammed 75 / DetonateCallsKill No / KillSelfDelay 3);
//! TrailRemnant ImmortalBody health-floor residual (never below 1 HP / never dead);
//! Anim2DMode full residual table (INVALID..PING_PONG_BACKWARDS) (graphics);
//! DisplayString draw shadow-position residual (x+xDrop / y+yDrop order honesty).
//! Wave 44 residual closed: SupW ParticleUplink magenta OuterColor residual
//! (R:255 G:0 B:255); DeletionUpdate calcSleepDelay residual (min/max frames,
//! clamp ≥1; remnant fixed 120 frames); graphics GameText MISSING fetch +
//! DisplayStringManager link + Anim2D status/alpha residual.
//! Wave 45 residual closed: PUC sound residual pack honesty
//! (PoweringUp/UnpackToIdle/FiringToPack/GroundAnnihilation + BeamLaunchFX
//! interval 30 + GroundHitFX names; applications on charge status + beam spawn);
//! Scorch residual pack honesty (ScorchMarkScalar 2.4 / Swath 200/50 /
//! ManualDriving 20/40 / DoubleClick 15 frames); SupW PointDefenseDroneLaserBeam
//! LifetimeUpdate Min=Max 95 ms → ceil → 3 frames; PUC FlammableUpdate residual
//! (AflameDuration 5000 ms / DamageAmount 5 / DamageDelay 500 ms).
//! Wave 50 residual closed: PUC OuterNodes flare residual pack honesty
//! (OuterNode Light/Medium/Intense + LaserBaseReadyToFire + Medium/Intense
//! connector laser names; pack armed on beam STATUS_FIRING spawn);
//! PUC SlowDeath / InstantDeath residual pack (DestructionDelay 2000 ms → 60
//! frames; INITIAL FX_ParticleUplinkDeathInitial / OCL_SDILinkLasers; FINAL
//! FX_StructureMediumDeath / OCL_ParticleUplinkDeathFinal; InstantDeath
//! UNDER_CONSTRUCTION OCL_ABPowerPlantExplode + FX_StructureMediumDeath);
//! Gattling ContinuousFire WeaponBonus ROF application counters (MEAN 200% /
//! FAST 300% residual tick applications); laser soft-edge texture bind pack
//! residual lives in graphics/laser_segment_upload (EXNoise02 / EXBinaryStream32
//! + MaxIntensity/Fade defaults; fail-closed vs live wgpu write_buffer).
//! CruiseMissile residual is a MOAB primary + MOABFlame secondary residual
//! (not full loft projectile / HeightDieUpdate / door animation / tree burn state).
//! Wave 56 residual closed: CarpetBomb residual deepen (DropDelay stagger +
//! DropVariance 30/40/0 + AmericaJetB52 PreferredHeight 100 + science-tier bomb
//! counts USA15/AirF12/China10 + DeliveryDistance 400/500/350 + FireFX
//! FX_CarpetBomb honesty + application counters); CruiseMissile/MOAB residual
//! deepen (NeutronMissileUpdate loft DistanceToTravelBeforeTurning 200 /
//! SpecialSpeedTime 1500ms→45f / HeightDie InitialDelay 1000ms→30f /
//! TargetHeight 10 + projectile CruiseMissile + MOABDetonationWeapon damage/
//! radius/ShockWave + WeaponFX_MOAB_Blast FireFX honesty); ArtilleryBarrage
//! residual deepen (FormationSize 12/24/36 + DelayDeliveryMin 0 / Max 3000ms→90f
//! + WeaponErrorRadius 100 + ChinaArtilleryCannon transport / PreferredHeight
//! 500 / DeliveryDistance 250 honesty); NuclearMissile radiation residual
//! deepen (SuspendFXDelay 10000ms→300f + FireFX WeaponFX_LargeRadiationFieldWeapon
//! + DamageType RADIATION + OCL_NukeRadiationField); AnthraxBomb poison residual
//! deepen (FireFX WeaponFX_LargePoisonFieldWeaponUpgraded + DeathType
//! POISONED_BETA + WeaponSpeed 600 + OCL_PoisonFieldAnthraxBomb). Fail-closed:
//! not full B52 pathfinder / NeutronMissileUpdate door/loft physics /
//! ChinaArtilleryCannon transport Object / HazardousMaterialArmor stack.
//! Wave 65 residual closed: ScudStormMissile ThingFactory object residual pack
//! (Physics Mass 500 / TransportSlotCount 10 / ShroudClearingRange 0 /
//! Armor ProjectileArmor / SpecialPowerCompletionDie SuperweaponScudStorm /
//! HeightDie TargetHeightIncludesStructures Yes / DAMAGED model NONE);
//! SpectreHowitzerShell ThingFactory InstantDeath death-type table residual
//! (DETONATED / LASERED+OCL / GENERIC) + Scale 0.6 / Geometry / Shadow pack;
//! TrailRemnant KindOf NO_COLLIDE+UNATTACKABLE+IMMOBILE + ImmortalBody MaxHealth
//! 50 residual pack. Fail-closed: not full ThingFactory Object / live module stack.
//! Wave 73 residual closed: SpectreGunship orbit residual pack deepen
//! (HowitzerFiringRate 300ms→9f / HowitzerFollowLag 400ms→12f /
//! GunshipOrbitRadius 250 vs AttackAreaRadius 200 science-tier table /
//! TargetingReticleRadius 25 / StrafingIncrement 20 / OrbitInsertionSlope 0.7 /
//! AttackAreaDecal SCCSpecTarg + TargetingReticleDecal SCCSpecRet /
//! dual-weapon ROF howitzer 9→6→4 / gattling 3→1→1 schedule /
//! SuperweaponSpectreGunship Reload 240s / AirF 180s / ViewObject 30s/250);
//! NuclearMissile radiation residual pack deepen (AttackRange 15 /
//! MinimumAttackRange 10 / KindOf IMMOBILE+CLEANUP_HAZARD+INERT+NO_COLLIDE /
//! HazardousMaterialArmor / Geometry CYLINDER h1 / HazardFieldCoreWeapon /
//! DeathFX FX_RadiationPoolDie / SuperweaponNeutronMissile Reload 360s);
//! SupW variants residual pack (SupW Neutron Reload 240s / SupW PUC 180s /
//! Nuke_ Neutron 300s / AirF Spectre 180s / RadiusCursor 210).
//! Wave 74 residual closed: ThingFactory object residual spawn bookkeeping
//! (ScudStormMissile impact spawn residual ledger + SpectreHowitzerShell
//! spawn residual ledger + TrailRemnant spawn residual ledger with
//! ImmortalBody/DeletionUpdate already closed). Fail-closed:
//! not full SpectreGunshipUpdate OCL aircraft / HazardousMaterialArmor cleanup
//! stack / SupW ThingFactory Object.

use crate::command_system::SpecialPowerType;
use crate::game_logic::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod a10_strike;
mod artillery_barrage;
mod carpet_bomb;
mod cruise_missile;
mod fields;
mod honesty;
mod particle_uplink;
mod radiation;
mod registry;
mod registry_fields;
mod registry_honesty;
mod registry_impact;
mod scud_storm;
mod spectre;
mod spectre_science;
mod strike;
mod superweapon_kind;
mod toxin;
mod types;

pub use a10_strike::*;
pub use artillery_barrage::*;
pub use carpet_bomb::*;
pub use cruise_missile::*;
pub use fields::*;
pub use honesty::*;
pub use particle_uplink::*;
pub use radiation::*;
pub use registry::*;
pub use registry_fields::*;
pub use registry_honesty::*;
pub use registry_impact::*;
pub use scud_storm::*;
pub use spectre::*;
pub use spectre_science::*;
pub use strike::*;
pub use superweapon_kind::*;
pub use toxin::*;
pub(crate) use types::horizontal_distance;
pub use types::{SP_LOGIC_FPS, duration_ms_to_logic_frames, lifetime_update_fixed_frames};

#[cfg(test)]
mod tests_core;
#[cfg(test)]
mod tests_honesty;
#[cfg(test)]
mod tests_residuals;
