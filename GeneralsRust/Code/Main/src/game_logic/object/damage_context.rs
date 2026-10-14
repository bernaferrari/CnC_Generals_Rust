//! Per-impact inputs, never published through a thread-local current hit.
//! C++ DamageInfoInput carries source/status on each operation; DamageFX.cpp61–93
//! selects source veterancy, HiveStructureBody.cpp55–65 selects source position.
use super::{Object, ObjectId};
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_transition_damage_fx::{HostDamageFxVictim, snapshot_damage_fx_source};
use gamelogic::common::ObjectStatusTypes;

/// Immediate Player mutations issued by one Body operation. These are callbacks
/// at original C++ observation points, never a queued frame action or mirror.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BodyPlayerEffect {
    AttackedBy {
        victim_player: Option<u32>,
        selected_source: Option<ObjectId>,
    },
    NeutralVehicleSniped,
}

/// C++ ActiveBody.cpp552–568 observes the actual incoming/retained object.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LastDamageSourceObservation {
    pub(crate) exists: bool,
    pub(crate) preferred: bool,
}
impl LastDamageSourceObservation {
    pub(crate) fn observe(source: Option<&Object>) -> Self {
        source.map_or(Self::default(), |source| Self {
            exists: true,
            preferred: source.is_kind_of(crate::game_logic::KindOf::Vehicle)
                || source.is_kind_of(crate::game_logic::KindOf::Infantry)
                || source.is_faction_structure(),
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct DamageHitContext<'a> {
    source: Option<HostDamageFxVictim>,
    status: Option<ObjectStatusTypes>,
    // C++ DamageInfoInput::m_kill: an input to this one body operation.
    kill: bool,
    neutral_player_id: Option<u32>,
    incoming_history: LastDamageSourceObservation,
    previous_history: LastDamageSourceObservation,
    // Borrowed for one synchronous hit; canonical overrides stay in the engine.
    topple_interpreter: Option<&'a gamelogic::scripting::engine::ScriptEngine>,
}

impl<'a> DamageHitContext<'a> {
    pub(crate) fn new(source: Option<&Object>, weapon: Option<&str>, kind: DamageType) -> Self {
        Self {
            kill: false,
            neutral_player_id: None,
            incoming_history: LastDamageSourceObservation::observe(source),
            previous_history: LastDamageSourceObservation::default(),
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

    /// Selected immutable rule for this hit; an authored NONE status must not
    /// be replaced by a different world's same-name Weapon definition.
    pub(crate) fn new_selected(
        source: Option<&Object>,
        weapon: Option<&str>,
        kind: DamageType,
        rule: Option<&gamelogic::weapon::WeaponTemplate>,
    ) -> Self {
        let Some(rule) = rule else {
            return Self::new(source, weapon, kind);
        };
        let mut context = Self::new(source, None, kind);
        if kind == DamageType::Status {
            context.status = Some(rule.damage_status_type.into());
        }
        context
    }

    /// Refresh history identity/classification without replacing launch/hit FX
    /// observations or the borrowed script interpreter carried by this impact.
    pub(crate) fn with_last_damage_sources(
        self,
        incoming: Option<&Object>,
        previous: Option<&Object>,
    ) -> Self {
        self.with_last_damage_observations(
            LastDamageSourceObservation::observe(incoming),
            LastDamageSourceObservation::observe(previous),
        )
    }
    pub(crate) fn with_last_damage_observations(
        mut self,
        incoming: LastDamageSourceObservation,
        previous: LastDamageSourceObservation,
    ) -> Self {
        self.incoming_history = incoming;
        self.previous_history = previous;
        self
    }
    pub(super) fn last_damage_observations(
        &self,
    ) -> (LastDamageSourceObservation, LastDamageSourceObservation) {
        (self.incoming_history, self.previous_history)
    }

    pub(crate) fn with_neutral_player_id(mut self, id: Option<u32>) -> Self {
        self.neutral_player_id = id;
        self
    }
    pub(super) fn neutral_player_id(&self) -> Option<u32> {
        self.neutral_player_id
    }

    pub(in crate::game_logic) fn with_status(status: ObjectStatusTypes) -> Self {
        Self {
            source: None,
            status: Some(status),
            ..Self::default()
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
