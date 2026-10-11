//! Per-match EVA intent. Native Eva owns speech checks and boolean latches.
use gamelogic::helpers::EvaEvent;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaOperation {
    Request(EvaEvent),
    SetEnabled(bool),
}

#[derive(Debug, Clone)]
pub struct EvaReceipt {
    session: Arc<()>,
    pub cursor: u64,
}
impl EvaReceipt {
    pub fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.session, &other.session)
    }
}
impl PartialEq for EvaReceipt {
    fn eq(&self, other: &Self) -> bool {
        self.same_session(other) && self.cursor == other.cursor
    }
}
impl Eq for EvaReceipt {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaDelivery {
    pub receipt: EvaReceipt,
    pub operations: Vec<EvaOperation>,
    pub enabled: bool,
}
impl Default for EvaDelivery {
    fn default() -> Self {
        Self {
            receipt: EvaReceipt {
                session: Arc::new(()),
                cursor: 0,
            },
            operations: Vec::new(),
            enabled: true,
        }
    }
}
#[derive(Debug)]
pub struct HostEvaAnnouncements {
    delivery: EvaDelivery,
}
impl Default for HostEvaAnnouncements {
    fn default() -> Self {
        Self {
            delivery: EvaDelivery::default(),
        }
    }
}
impl HostEvaAnnouncements {
    pub fn request(&mut self, event: EvaEvent) {
        self.delivery.operations.push(EvaOperation::Request(event));
    }
    /// Eva.cpp setEnabled clears latches even when enabled is unchanged.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.delivery.enabled = enabled;
        self.delivery
            .operations
            .push(EvaOperation::SetEnabled(enabled));
    }
    pub fn is_enabled(&self) -> bool {
        self.delivery.enabled
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn take_delivery(&mut self) -> EvaDelivery {
        let operations = std::mem::take(&mut self.delivery.operations);
        if !operations.is_empty() {
            self.delivery.receipt.cursor = self
                .delivery
                .receipt
                .cursor
                .checked_add(1)
                .expect("EVA receipt exhausted");
        }
        EvaDelivery {
            receipt: self.delivery.receipt.clone(),
            operations,
            enabled: self.delivery.enabled,
        }
    }
    #[cfg(test)]
    pub fn take_requests(&mut self) -> Vec<EvaEvent> {
        let mut requests = Vec::new();
        self.delivery.operations.retain(|operation| {
            if let EvaOperation::Request(event) = operation {
                requests.push(*event);
                false
            } else {
                true
            }
        });
        requests
    }
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

/// C++ table token lookup for debug input; no event registry or mutable state.
pub fn eva_event_from_table_token(token: &str) -> Option<EvaEvent> {
    use EvaEvent::*;
    match token.trim().to_ascii_uppercase().as_str() {
        "LOWPOWER" => Some(LowPower),
        "INSUFFICIENTFUNDS" => Some(InsufficientFunds),
        "SUPERWEAPONDETECTED_OWN_PARTICLECANNON" => Some(SuperweaponDetectedOwnParticleCannon),
        "SUPERWEAPONDETECTED_OWN_NUKE" => Some(SuperweaponDetectedOwnNuke),
        "SUPERWEAPONDETECTED_OWN_SCUDSTORM" => Some(SuperweaponDetectedOwnScudStorm),
        "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON" => Some(SuperweaponDetectedAllyParticleCannon),
        "SUPERWEAPONDETECTED_ALLY_NUKE" => Some(SuperweaponDetectedAllyNuke),
        "SUPERWEAPONDETECTED_ALLY_SCUDSTORM" => Some(SuperweaponDetectedAllyScudStorm),
        "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON" => Some(SuperweaponDetectedEnemyParticleCannon),
        "SUPERWEAPONDETECTED_ENEMY_NUKE" => Some(SuperweaponDetectedEnemyNuke),
        "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM" => Some(SuperweaponDetectedEnemyScudStorm),
        "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON" => Some(SuperweaponLaunchedOwnParticleCannon),
        "SUPERWEAPONLAUNCHED_OWN_NUKE" => Some(SuperweaponLaunchedOwnNuke),
        "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM" => Some(SuperweaponLaunchedOwnScudStorm),
        "SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON" => Some(SuperweaponLaunchedAllyParticleCannon),
        "SUPERWEAPONLAUNCHED_ALLY_NUKE" => Some(SuperweaponLaunchedAllyNuke),
        "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM" => Some(SuperweaponLaunchedAllyScudStorm),
        "SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON" => Some(SuperweaponLaunchedEnemyParticleCannon),
        "SUPERWEAPONLAUNCHED_ENEMY_NUKE" => Some(SuperweaponLaunchedEnemyNuke),
        "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM" => Some(SuperweaponLaunchedEnemyScudStorm),
        "SUPERWEAPONREADY_OWN_PARTICLECANNON" => Some(SuperweaponReadyOwnParticleCannon),
        "SUPERWEAPONREADY_OWN_NUKE" => Some(SuperweaponReadyOwnNuke),
        "SUPERWEAPONREADY_OWN_SCUDSTORM" => Some(SuperweaponReadyOwnScudStorm),
        "SUPERWEAPONREADY_ALLY_PARTICLECANNON" => Some(SuperweaponReadyAllyParticleCannon),
        "SUPERWEAPONREADY_ALLY_NUKE" => Some(SuperweaponReadyAllyNuke),
        "SUPERWEAPONREADY_ALLY_SCUDSTORM" => Some(SuperweaponReadyAllyScudStorm),
        "SUPERWEAPONREADY_ENEMY_PARTICLECANNON" => Some(SuperweaponReadyEnemyParticleCannon),
        "SUPERWEAPONREADY_ENEMY_NUKE" => Some(SuperweaponReadyEnemyNuke),
        "SUPERWEAPONREADY_ENEMY_SCUDSTORM" => Some(SuperweaponReadyEnemyScudStorm),
        "BUILDINGLOST" => Some(BuildingLost),
        "BASEUNDERATTACK" => Some(BaseUnderAttack),
        "ALLYUNDERATTACK" => Some(AllyUnderAttack),
        "BEACONDETECTED" => Some(BeaconDetected),
        "ENEMYBLACKLOTUSDETECTED" => Some(EnemyBlackLotusDetected),
        "ENEMYJARMENKELLDETECTED" => Some(EnemyJarmenKellDetected),
        "ENEMYCOLONELBURTONDETECTED" => Some(EnemyColonelBurtonDetected),
        "OWNBLACKLOTUSDETECTED" => Some(OwnBlackLotusDetected),
        "OWNJARMENKELLDETECTED" => Some(OwnJarmenKellDetected),
        "OWNCOLONELBURTONDETECTED" => Some(OwnColonelBurtonDetected),
        "UNITLOST" => Some(UnitLost),
        "GENERALLEVELUP" => Some(GeneralLevelUp),
        "VEHICLESTOLEN" => Some(VehicleStolen),
        "BUILDINGSTOLEN" => Some(BuildingStolen),
        "CASHSTOLEN" => Some(CashStolen),
        "UPGRADECOMPLETE" => Some(UpgradeComplete),
        "BUILDINGBEINGSTOLEN" => Some(BuildingBeingStolen),
        "BUILDINGSABOTAGED" => Some(BuildingSabotaged),
        "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER" => Some(SuperweaponLaunchedOwnGpsScrambler),
        "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER" => Some(SuperweaponLaunchedAllyGpsScrambler),
        "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER" => Some(SuperweaponLaunchedEnemyGpsScrambler),
        "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK" => Some(SuperweaponLaunchedOwnSneakAttack),
        "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK" => Some(SuperweaponLaunchedAllySneakAttack),
        "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK" => Some(SuperweaponLaunchedEnemySneakAttack),
        _ => None,
    }
}

#[cfg(test)]
#[path = "eva_announcement_owner_tests.rs"]
mod eva_announcement_owner_tests;
