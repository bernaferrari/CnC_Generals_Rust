//! Frame-local host EVA pulse log for presentation audio residual.
//!
//! C++ `TheEva->setShouldPlay` edges are recorded here so PresentationFrame can
//! emit snapshot EVA audio without dual-reading live GameLogic mid-render.

use gamelogic::helpers::EvaEvent;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEvaEvent {
    /// Presentation/audio event name (`EVA_` + C++ `TheEvaMessageNames` token).
    pub name: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostEvaEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostEvaEvent>> = RefCell::new(Vec::new());
}

/// Map C++ `EvaEvent` residual → C++ `TheEvaMessageNames` token (no `EVA_` prefix).
/// Wave 536: exact table tokens so GameClient `EvaMessage::from_name` resolves.
pub fn eva_event_table_token(event: EvaEvent) -> &'static str {
    use EvaEvent::*;
    match event {
        LowPower => "LOWPOWER",
        InsufficientFunds => "INSUFFICIENTFUNDS",
        SuperweaponDetectedOwnParticleCannon => "SUPERWEAPONDETECTED_OWN_PARTICLECANNON",
        SuperweaponDetectedOwnNuke => "SUPERWEAPONDETECTED_OWN_NUKE",
        SuperweaponDetectedOwnScudStorm => "SUPERWEAPONDETECTED_OWN_SCUDSTORM",
        SuperweaponDetectedAllyParticleCannon => "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON",
        SuperweaponDetectedAllyNuke => "SUPERWEAPONDETECTED_ALLY_NUKE",
        SuperweaponDetectedAllyScudStorm => "SUPERWEAPONDETECTED_ALLY_SCUDSTORM",
        SuperweaponDetectedEnemyParticleCannon => "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON",
        SuperweaponDetectedEnemyNuke => "SUPERWEAPONDETECTED_ENEMY_NUKE",
        SuperweaponDetectedEnemyScudStorm => "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM",
        SuperweaponLaunchedOwnParticleCannon => "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON",
        SuperweaponLaunchedOwnNuke => "SUPERWEAPONLAUNCHED_OWN_NUKE",
        SuperweaponLaunchedOwnScudStorm => "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM",
        SuperweaponLaunchedAllyParticleCannon => "SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON",
        SuperweaponLaunchedAllyNuke => "SUPERWEAPONLAUNCHED_ALLY_NUKE",
        SuperweaponLaunchedAllyScudStorm => "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM",
        SuperweaponLaunchedEnemyParticleCannon => "SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON",
        SuperweaponLaunchedEnemyNuke => "SUPERWEAPONLAUNCHED_ENEMY_NUKE",
        SuperweaponLaunchedEnemyScudStorm => "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM",
        SuperweaponReadyOwnParticleCannon => "SUPERWEAPONREADY_OWN_PARTICLECANNON",
        SuperweaponReadyOwnNuke => "SUPERWEAPONREADY_OWN_NUKE",
        SuperweaponReadyOwnScudStorm => "SUPERWEAPONREADY_OWN_SCUDSTORM",
        SuperweaponReadyAllyParticleCannon => "SUPERWEAPONREADY_ALLY_PARTICLECANNON",
        SuperweaponReadyAllyNuke => "SUPERWEAPONREADY_ALLY_NUKE",
        SuperweaponReadyAllyScudStorm => "SUPERWEAPONREADY_ALLY_SCUDSTORM",
        SuperweaponReadyEnemyParticleCannon => "SUPERWEAPONREADY_ENEMY_PARTICLECANNON",
        SuperweaponReadyEnemyNuke => "SUPERWEAPONREADY_ENEMY_NUKE",
        SuperweaponReadyEnemyScudStorm => "SUPERWEAPONREADY_ENEMY_SCUDSTORM",
        BuildingLost => "BUILDINGLOST",
        BaseUnderAttack => "BASEUNDERATTACK",
        AllyUnderAttack => "ALLYUNDERATTACK",
        BeaconDetected => "BEACONDETECTED",
        EnemyBlackLotusDetected => "ENEMYBLACKLOTUSDETECTED",
        EnemyJarmenKellDetected => "ENEMYJARMENKELLDETECTED",
        EnemyColonelBurtonDetected => "ENEMYCOLONELBURTONDETECTED",
        OwnBlackLotusDetected => "OWNBLACKLOTUSDETECTED",
        OwnJarmenKellDetected => "OWNJARMENKELLDETECTED",
        OwnColonelBurtonDetected => "OWNCOLONELBURTONDETECTED",
        UnitLost => "UNITLOST",
        GeneralLevelUp => "GENERALLEVELUP",
        VehicleStolen => "VEHICLESTOLEN",
        BuildingStolen => "BUILDINGSTOLEN",
        CashStolen => "CASHSTOLEN",
        UpgradeComplete => "UPGRADECOMPLETE",
        BuildingBeingStolen => "BUILDINGBEINGSTOLEN",
        BuildingSabotaged => "BUILDINGSABOTAGED",
        SuperweaponLaunchedOwnGpsScrambler => "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER",
        SuperweaponLaunchedAllyGpsScrambler => "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER",
        SuperweaponLaunchedEnemyGpsScrambler => "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER",
        SuperweaponLaunchedOwnSneakAttack => "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK",
        SuperweaponLaunchedAllySneakAttack => "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK",
        SuperweaponLaunchedEnemySneakAttack => "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK",
    }
}

/// Map C++ EvaEvent residual → presentation audio event name (`EVA_` + table token).
pub fn eva_event_audio_name(event: EvaEvent) -> String {
    format!("EVA_{}", eva_event_table_token(event))
}

pub fn record(name: impl Into<String>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostEvaEvent { name: name.into() });
    });
}

/// Wave 534/536: record from typed EvaEvent (full setShouldPlay matrix).
pub fn record_event(event: EvaEvent) {
    record(eva_event_audio_name(event));
}

pub fn drain() -> Vec<HostEvaEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn take_last_drain() -> Vec<HostEvaEvent> {
    let pending = drain();
    if !pending.is_empty() {
        return pending;
    }
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

#[cfg(test)]
mod host_eva_drain_tests {
    #[test]
    fn process_eva_events_must_not_steal_the_eva_queue() {
        // C++ Eva::update (Eva.cpp:264-525) is the sole consumer of setShouldPlay.
        let src = include_str!("world_scripts/scripts_camera.rs");
        let process = src
            .split("pub(in super::super) fn process_eva_events")
            .nth(1)
            .and_then(|s| s.split("pub(in super::super) fn mission_script_count").next())
            .unwrap_or("");
        assert!(
            !process.contains("TheEva::drain_events"),
            "host process_eva_events must not steal TheEva so Eva.ini SideSounds can play"
        );
        assert!(
            !process.contains("dispatch_eva_announcement"),
            "generic EVA_* names must not replace Eva.ini SideSounds"
        );
    }
}
