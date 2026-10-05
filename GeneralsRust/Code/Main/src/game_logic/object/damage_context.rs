//! Per-impact inputs, never published through a thread-local current hit.
//! C++ DamageInfoInput carries source/status on each operation; DamageFX.cpp61–93
//! selects source veterancy, HiveStructureBody.cpp55–65 selects source position.
use super::Object;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_transition_damage_fx::{HostDamageFxVictim, snapshot_damage_fx_source};
use gamelogic::common::ObjectStatusTypes;

#[derive(Debug, Default)]
pub(in crate::game_logic) struct DamageHitContext {
    source: Option<HostDamageFxVictim>,
    status: Option<ObjectStatusTypes>,
}

impl DamageHitContext {
    pub(in crate::game_logic) fn new(
        source: Option<&Object>,
        weapon: Option<&str>,
        kind: DamageType,
    ) -> Self {
        Self {
            source: source.map(snapshot_damage_fx_source),
            status: if kind == DamageType::Status {
                weapon.and_then(
                    crate::game_logic::weapon_bootstrap::host_damage_status_for_weapon_name,
                )
            } else {
                None
            },
        }
    }

    pub(in crate::game_logic) fn with_status(status: ObjectStatusTypes) -> Self {
        Self {
            source: None,
            status: Some(status),
        }
    }

    pub(super) fn source(&self) -> Option<&HostDamageFxVictim> {
        self.source.as_ref()
    }
    pub(in crate::game_logic) fn status(&self) -> Option<ObjectStatusTypes> {
        self.status
    }

    pub(super) fn status_name(&self) -> Option<&'static str> {
        self.status
            .and_then(crate::game_logic::weapon_bootstrap::object_status_bit_name)
    }
}
