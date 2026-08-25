//! Snapshot save/load residual tests.

use super::*;
use crate::ai::AIDifficulty;
use crate::game_logic::{
    AIState, Experience, GameLogic, GuardMode, HostStrikePhase, HostSuperweaponKind, KindOf,
    Object, ObjectId, Player, PlayerTemplateIdentity, Resources, Team, ThingTemplate,
    VeterancyLevel, Weapon, WeaponLockType,
};
use glam::Vec3;

/// C++ `TempWeaponBonusHelper::xfer` (`TempWeaponBonusHelper.cpp:112-113`)
/// persists `m_currentBonus` + `m_frameToRemove`. A mid-Frenzy host save
/// must restore `weapon_bonus_frenzy` / `_level` / `_until_frame`.

/// Older ObjectSnapshot records omit the Frenzy tail. `serde(default)` must
/// fail-closed to inactive so a v12 decode does not invent a mid-Frenzy buff.

/// C++ Object::xfer named UNSELECTABLE / DEPLOYED + m_scriptStatus must
/// survive live snapshot/restore (sell latch, DeployStyle unpack, WB script).

/// C++ Object::xfer DISABLED_HELD + AIUpdateInterface::m_attitude.

/// Residual: secondary_weapon + active_weapon_slot must survive snapshot save/load.
/// Prior gap: capture only stored primary in `weapons[0]`, restore left secondary None.

/// End-to-end SaveFileManager path: secondary stays bound after save → load.

/// v4 keeps mutable barrel cursors at the Object tail, not inside every host
/// Weapon. Restore stages a multi-barrel cursor until fresh topology is known.

/// A pristine C++ Weapon already owns its authored `m_numShotsForCurBarrel`,
/// even if it has not fired. Main derives that lazy cursor at first use, so
/// the v4 snapshot must persist the zero sentinel rather than its temporary
/// one-shot representation; restore then rebuilds the exact authored cadence.

fn ensure_strike_test_tank(logic: &mut GameLogic) {
    let mut t = ThingTemplate::new("StrikeTestTank");
    t.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("StrikeTestTank".to_string(), t);
}

/// Residual: DaisyCutter queued mid-flight must survive snapshot and still
/// apply area damage once the restored impact frame is reached.

/// Residual: A10 strike mid-flight save/load continues remaining delay and impacts.

/// Bincode / SaveFileManager path also keeps pending strikes.

fn ensure_upgrade_test_templates(logic: &mut GameLogic) {
    if !logic.templates.contains_key("TestInfantry") {
        let mut t = ThingTemplate::new("TestInfantry");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(80.0)
            .set_cost(100, 0);
        // Model the explicit Object INI capture SpecialAbility; capture is
        // data-driven and must not depend on the fixture name.
        t.capture_power = crate::game_logic::CapturePowerKind::Ranger;
        t.capture_start_ability_range = Some(5.0);
        logic.templates.insert("TestInfantry".to_string(), t);
    }
    if !logic.templates.contains_key("TestBuilding") {
        let mut t = ThingTemplate::new("TestBuilding");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1200.0)
            .set_cost(500, -1);
        logic.templates.insert("TestBuilding".to_string(), t);
    }
    if !logic.templates.contains_key("TestBarracks") {
        let mut t = ThingTemplate::new("TestBarracks");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBarracks)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_cost(600, -1);
        logic.templates.insert("TestBarracks".to_string(), t);
    }
}

mod legacy_and_timer_snapshots;
/// Residual: CaptureBuilding queued mid-flight must survive snapshot and still
/// complete with capture unlock after load.

/// Bincode / SaveFileManager path also keeps pending host upgrade research.

/// Wave 79: Drawable residual camo_stealth_look survives snapshot capture/restore.

/// C++ StealthUpdate::xfer (`StealthUpdate.cpp:1127-1130`) persists
/// `m_detectionExpiresFrame` + `m_stealthAllowedFrame`. Host
/// `update_stealth_and_detection` only expires DETECTED when
/// `detection_expires_frame > 0` — omitting the field left DETECTED stuck
/// after load (hq-0jeh6).

/// Popup and host write the same Common CHUNK_*.sav tokens. Load restores
/// into the store `host_authoritative_*` reads.

/// The renderer companion enters `.sav` only through the explicit host-aware
/// SaveFileManager API; logic-only callers retain the default empty payload.

/// Direct Common Xfer is positional: pre-HDB world version 2 objects end at
/// `ObjectType`, so the current reader must not consume the following world
/// fields as an HDB option discriminator.  This deliberately uses the direct
/// Xfer route rather than bincode, whose v2->v3 mirror has separate coverage.

/// Direct Xfer v3 already contains the HDB Object tail even after bincode
/// advances to v4. A following player and raw sentinel prove the object gate
/// is tied to the direct envelope version, not the bincode constant.

/// Direct v4 appends the logical Object/world tails in one explicit order.
/// The sentinel proves client Drawable Xfer data does not steal bytes from
/// subsequent records.

/// V5 appends exact offline PlayerTemplate bindings after the v4 client
/// Drawable tail. The sentinel proves this new world-owned identity extension
/// cannot steal bytes from a following direct-Xfer record.

/// V6 appends exact raw PartitionManager shroud counters and pending reveal
/// expiry records after the v5 PlayerTemplate tail. The sentinel proves the
/// full grid payload remains bounded by the world tail.

/// V7 appends the per-object C++ `Weapon::m_suspendFXFrame` values after the
/// v5 collector tail.  Keep the vector aligned with the serialized weapon
/// slots and prove a following world record is not consumed by the new tail.

/// V8 appends the source-keyed temporary Weapon behavior bundle after the v7
/// suspend-FX tail.  Its damaged roles are independent PRIMARY allocations;
/// the following player record proves the new tail consumes exactly its own
/// bytes and does not steal the next direct-Xfer record.

/// A direct v6 stream has no v7 parallel tail.  The current reader must clear
/// any pre-seeded value and leave the trailing sentinel aligned.

/// Bincode v6 had the collector/shroud tails but no per-object suspend-FX
/// vector.  Decode an exact predecessor record and verify migration produces
/// the current schema with a fail-closed empty vector.

/// Direct Xfer must reject a future envelope before it consumes timestamp or
/// any body byte. Marker labels are no-ops, so this is the actual boundary.

/// The outer direct-Xfer validator accepts every explicitly supported legacy
/// version. This is intentionally an envelope check; historical full-body
/// compatibility remains covered by exact tail fixtures above.

/// C++ `Player::xfer` (`Player.cpp:4268-4275`) persists `m_rankLevel`,
/// `m_skillPoints`, and `m_sciencePurchasePoints`. Host restore previously
/// hardcoded rank 1 / 0 / 0, wiping mid-game generals progress.

/// C++ `Energy::xfer` v3 persists `m_powerSabotagedTillFrame`. Host restore
/// previously hardcoded 0, ending GLA sabotage on load.

/// Pre-v10 streams have no rank tail. Restore must keep the fail-closed
/// rank 1 / 0 / 0 defaults instead of inventing mid-game progress.

/// V10 appends the rank tail after the v9 lifecycle envelope. The sentinel
/// proves the new world-owned residual cannot steal bytes from a following
/// direct-Xfer record.

/// Bincode v9 had the lifecycle tail but no Player rank residual. Decode
/// an exact predecessor record and verify migration produces the current
/// schema with a fail-closed empty rank tail.

/// C++ `OpenContain::xfer` (`OpenContain.cpp:1590`) persists the contain
/// list. Host HUD / `can_garrison` read `BuildingData.garrisoned_units`,
/// which must be rebuilt from the restored occupant ids.

/// C++ `Object::xfer` (`Object.cpp:4068`) persists `m_name` independently
/// of the ThingTemplate. Restore must not overwrite the instance name with
/// the template name or named script units stop matching.

/// C++ `AIUpdateInterface::xfer` (`AIUpdate.cpp:5015-5019`) persists guard
/// target type, `m_locationToGuard`, `m_objectToGuard`, and `m_guardMode`.
/// Host also stores the live guard radius. Restore must not re-anchor at
/// the unit's current position.

/// Pre-v11 streams have no instance-name / guard tail. Restore must keep
/// constructor defaults instead of inventing a template name or a guard
/// anchor at the unit's current position.

/// V11 appends the instance-name / guard tail after the v10 rank tail.
/// The sentinel proves the new world-owned residual cannot steal bytes
/// from a following direct-Xfer record.

/// Bincode v10 had the rank tail but no instance-name / guard residual.
/// Decode an exact predecessor record and verify migration produces the
/// current schema with a fail-closed empty name/guard tail.

/// C++ `Object::xfer` (`Object.cpp:4126-4130`) persists `m_visionSpiedMask`.
/// SpyVision / CIA keeps those units as moving lookers until duration expires.

/// C++ `Object::xfer` (`Object.cpp:4050-4053`) writes `m_builderID`.
/// Dozer BUILD exclusivity uses that id (`DozerAIUpdate.cpp:1986`).

/// C++ `GameLogic::xfer` v6 calls `TheBuildAssistant->xferTheSellList`.
/// Mid-sell buildings stay on that list until percent hits the sold threshold.

/// V13 appends CIA / builder / sell after the v12 overcharge tail.

/// Pre-v13 streams have no CIA / builder / sell tail. Restore must not invent
/// a mid-spy, exclusive builder, or mid-sell list.

/// C++ `SpyVisionUpdate::xfer` v2 persists `m_disabledUntilFrame`,
/// `m_resetTimersNextUpdate`, and the self-powered next-wake frame.
/// CIA vision-spied mask persist does not restore these module timers.

/// C++ `Object::xfer` writes `DISABLED_PARALYZED` + `m_disabledTillFrame`.
/// Strategy Center plan-change freeze must keep remaining frames after load.

/// C++ `ParachuteContain::xfer` keeps pitch/roll/rates, start Z, landing
/// override, and `m_opened`. Mid-fall pilots stay parachuting after load.

/// C++ `StatusDamageHelper::xfer` persists `m_statusToHeal` + `m_frameToHeal`.
/// Avenger FAERIE_FIRE paint must keep the 150% ROF timer after load.
// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod world_and_weapon_snapshots;
