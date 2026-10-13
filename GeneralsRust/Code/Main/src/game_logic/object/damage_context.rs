//! Per-impact inputs, never published through a thread-local current hit.
//! C++ DamageInfoInput carries source/status on each operation; DamageFX.cpp61–93
//! selects source veterancy, HiveStructureBody.cpp55–65 selects source position.
use super::Object;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_transition_damage_fx::{HostDamageFxVictim, snapshot_damage_fx_source};
use gamelogic::common::ObjectStatusTypes;

#[derive(Default)]
pub(crate) struct DamageHitContext<'a> {
    source: Option<HostDamageFxVictim>,
    status: Option<ObjectStatusTypes>,
    // C++ DamageInfoInput::m_kill: an input to this one body operation.
    kill: bool,
    // Borrowed for one synchronous hit; canonical overrides stay in the engine.
    topple_interpreter: Option<&'a gamelogic::scripting::engine::ScriptEngine>,
}

impl<'a> DamageHitContext<'a> {
    pub(crate) fn new(source: Option<&Object>, weapon: Option<&str>, kind: DamageType) -> Self {
        Self {
            kill: false,
            topple_interpreter: None,
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
            kill: false,
            topple_interpreter: None,
        }
    }

    pub(in crate::game_logic) fn for_kill() -> Self {
        Self {
            kill: true,
            ..Self::default()
        }
    }

    pub(in crate::game_logic) fn with_topple_interpreter(
        mut self,
        engine: Option<&'a gamelogic::scripting::engine::ScriptEngine>,
    ) -> Self {
        self.topple_interpreter = engine;
        self
    }
    pub(in crate::game_logic) fn topple_interpreter(
        &self,
    ) -> Option<&'a gamelogic::scripting::engine::ScriptEngine> {
        self.topple_interpreter
    }

    pub(super) fn is_kill(&self) -> bool {
        self.kill
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

impl std::fmt::Debug for DamageHitContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DamageHitContext")
            .field("source", &self.source)
            .field("status", &self.status)
            .field("kill", &self.kill)
            .field(
                "borrowed_topple_interpreter",
                &self.topple_interpreter.is_some(),
            )
            .finish()
    }
}
