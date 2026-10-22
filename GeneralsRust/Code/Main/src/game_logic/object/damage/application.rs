//! C++ ActiveBody.cpp:585–662/785–848: callbacks precede the final DamageFX.
//! One owned continuation is consumed before another victim is processed.
use super::super::*;
use crate::game_logic::host_transition_damage_fx::HostDamageFxVictim;

#[derive(Debug)]
#[must_use = "complete this impact synchronously before processing another hit"]
pub(in crate::game_logic) enum DamageApplication {
    // Special/no-HP branches already complete their existing effects.
    Complete(bool),
    Active(ActiveDamageContinuation),
    Healing(HealingContinuation),
}

/// Owned data crossing the released body borrow at the original callback point.
#[derive(Debug, Clone, Copy)]
pub(in crate::game_logic) struct BridgeBodyCallbacks {
    pub victim: ObjectId,
    pub source: Option<ObjectId>,
    pub input_amount: f32,
    pub old_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    pub health_changed: bool,
    pub damage_type: crate::game_logic::combat::DamageType,
    pub death_type: crate::game_logic::host_usa_pilot::HostDeathType,
}

/// Accepted healing and optional bridge callbacks crossing a released Body
/// borrow. This operation data is ephemeral; no match registry or saved mirror.
#[derive(Debug)]
pub(in crate::game_logic) struct HealingContinuation {
    pub(super) victim_id: ObjectId,
    pub(super) source: Option<ObjectId>,
    pub(super) old_body_state:
        Option<crate::game_logic::host_enum_table_residual::HostBodyDamageType>,
    pub(super) bridge_callbacks: Option<BridgeBodyCallbacks>,
    pub(super) fx_type: crate::game_logic::combat::DamageType,
    pub(super) actual_amount: f32,
    pub(super) frame: u32,
}

#[derive(Debug)]
pub(in crate::game_logic) struct ActiveDamageContinuation {
    pub(super) victim_id: ObjectId,
    pub(super) source: Option<ObjectId>,
    pub(super) fx_type: crate::game_logic::combat::DamageType,
    pub(super) actual_damage: f32,
    pub(super) old_body_state:
        Option<crate::game_logic::host_enum_table_residual::HostBodyDamageType>,
    pub(super) bridge_callbacks: Option<BridgeBodyCallbacks>,
    pub(super) frame: u32,
    pub(super) lethal: bool,
    pub(super) start_second_life: bool,
}

/// Linear post-FX phase: the world may release its victim borrow and observe
/// current owner policy before finishing the original synchronous impact.
#[must_use = "finish the same impact after observing its owner's post-FX policy"]
pub(in crate::game_logic) struct DamageAfterFx(DamageApplication);

impl DamageApplication {
    pub(in crate::game_logic) fn killed(&self) -> bool {
        match self {
            Self::Complete(killed) => *killed,
            Self::Active(tail) => tail.lethal,
            Self::Healing(_) => false,
        }
    }

    /// Actual post-armor DamageInfo amount at ActiveBody's released onDamage
    /// point. Overkill is not truncated to the remaining hit points.
    pub(in crate::game_logic) fn actual_damage_dealt(&self) -> Option<f32> {
        match self {
            Self::Active(tail) => Some(tail.actual_damage),
            Self::Complete(_) | Self::Healing(_) => None,
        }
    }

    pub(in crate::game_logic) fn set_killed_after_callbacks(&mut self, killed: bool) {
        if let Self::Active(tail) = self {
            tail.lethal = killed;
        }
    }

    pub(in crate::game_logic) fn source_id(&self) -> Option<ObjectId> {
        match self {
            Self::Complete(_) => None,
            Self::Active(tail) => tail.source,
            Self::Healing(tail) => tail.source,
        }
    }

    pub(in crate::game_logic) fn bridge_callbacks(&self) -> Option<BridgeBodyCallbacks> {
        match self {
            Self::Active(tail) => tail.bridge_callbacks,
            Self::Healing(tail) => tail.bridge_callbacks,
            Self::Complete(_) => None,
        }
    }

    /// CPP ActiveBody603–638/835–844 observes the live Body after the
    /// synchronous onDamage/onHealing chain and before Armor DamageFX.
    pub(in crate::game_logic) fn complete_body_state_callbacks(
        &mut self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
    ) {
        self.complete_body_state_callbacks_with_transition_fx(
            victim,
            source,
            &mut |object, event, source| {
                object.queue_transition_damage_fx_event_with_source(event, source)
            },
        );
    }

    pub(in crate::game_logic) fn complete_body_state_callbacks_with_transition_fx(
        &mut self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        transition_fx: &mut dyn FnMut(
            &mut Object,
            crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent,
            Option<&HostDamageFxVictim>,
        ),
    ) {
        self.complete_body_state_callbacks_with_body_fx(
            victim,
            source,
            &mut |object, effect, source| match effect {
                BodyStateFx::Transition(event) => transition_fx(object, event, source),
                effect => Object::standalone_body_fx(object, effect, source),
            },
        );
    }

    pub(in crate::game_logic) fn complete_body_state_callbacks_with_body_fx(
        &mut self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) {
        let (victim_id, old_body_state, frame, fear) = match self {
            Self::Active(tail) => (tail.victim_id, tail.old_body_state.take(), tail.frame, true),
            Self::Healing(tail) => (
                tail.victim_id,
                tail.old_body_state.take(),
                tail.frame,
                false,
            ),
            Self::Complete(_) => return,
        };
        assert_eq!(
            victim_id, victim.id,
            "body callback victim identity changed"
        );
        let Some(old_body_state) = old_body_state else {
            return;
        };
        victim.react_to_body_damage_state_change_with_body_fx(
            old_body_state,
            source,
            frame,
            body_fx,
        );
        if fear {
            if let Some(request) = victim.voice_fear_request(
                crate::game_logic::host_transition_damage_fx::take_voice_fear_roll,
            ) {
                body_fx(victim, BodyStateFx::VoiceFear(request), source);
            }
        }
    }

    pub(in crate::game_logic) fn dispatch_damage_fx(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
    ) -> DamageAfterFx {
        self.dispatch_damage_fx_with_body_fx(victim, source, &mut Object::standalone_body_fx)
    }

    pub(in crate::game_logic) fn dispatch_damage_fx_with_body_fx(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) -> DamageAfterFx {
        if let Self::Active(tail) = &self {
            assert_eq!(
                tail.victim_id, victim.id,
                "damage continuation victim identity changed"
            );
            body_fx(
                victim,
                BodyStateFx::ArmorDamage {
                    damage_type: tail.fx_type,
                    actual_damage: tail.actual_damage,
                    frame: tail.frame,
                },
                source,
            );
        }
        if let Self::Healing(tail) = &self {
            assert_eq!(tail.victim_id, victim.id);
            body_fx(
                victim,
                BodyStateFx::ArmorDamage {
                    damage_type: tail.fx_type,
                    actual_damage: tail.actual_amount,
                    frame: tail.frame,
                },
                source,
            );
        }
        DamageAfterFx(self)
    }

    /// Standalone calls supply an explicit inert policy: there is no world to
    /// discover. The driving-world path observes policy only after actual FX.
    pub(in crate::game_logic) fn finish(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        enable_repulsors: bool,
    ) -> bool {
        self.finish_standalone(victim, source, &enable_repulsors)
    }

    /// Detached Body operations observe policy after DamageFX; world callbacks
    /// require the receiving GameLogic and never publish ambient work.
    pub(in crate::game_logic) fn finish_standalone(
        mut self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        repulsor_policy: &bool,
    ) -> bool {
        self.finish_standalone_with_body_fx(
            victim,
            source,
            repulsor_policy,
            &mut Object::standalone_body_fx,
        )
    }

    pub(in crate::game_logic) fn finish_standalone_with_body_fx(
        mut self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        repulsor_policy: &bool,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) -> bool {
        self.complete_body_state_callbacks_with_body_fx(victim, source, body_fx);
        let lethal = self.killed();
        if lethal {
            // Explicit bounded adapter: no world onDie admission latch. Physics
            // and residual callers still need the full-world yield migration.
            victim.complete_standalone_die_with_body_fx(source, body_fx);
        }
        let after_fx = self.dispatch_damage_fx_with_body_fx(victim, source, body_fx);
        after_fx.finish_with_body_fx(victim, source, *repulsor_policy, body_fx)
    }
}

impl DamageAfterFx {
    pub(in crate::game_logic) fn had_active_body(&self) -> bool {
        matches!(self.0, DamageApplication::Active(_))
    }
    pub(in crate::game_logic) fn finish(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        enable_repulsors: bool,
    ) -> bool {
        self.finish_with_body_fx(
            victim,
            source,
            enable_repulsors,
            &mut Object::standalone_body_fx,
        )
    }

    pub(in crate::game_logic) fn finish_with_body_fx(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        enable_repulsors: bool,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) -> bool {
        match self.0 {
            DamageApplication::Complete(killed) => killed,
            DamageApplication::Healing(_) => false,
            DamageApplication::Active(tail) => {
                tail.finish_after_damage_fx(victim, source, enable_repulsors, body_fx)
            }
        }
    }
}

impl ActiveDamageContinuation {
    fn finish_after_damage_fx(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        enable_repulsors: bool,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) -> bool {
        assert_eq!(
            self.victim_id, victim.id,
            "damage continuation victim identity changed"
        );
        // C++ ActiveBody.cpp:655–662 follows DamageFX, even on a lethal hit.
        if enable_repulsors && victim.is_kind_of(KindOf::CanBeRepulsed) && !victim.status.repulsor {
            victim.repulsor_until_frame = 60; // 2 seconds @ 30Hz
            victim.set_status_repulsor(true);
        }
        // C++ UndeadBody.cpp:68–72 completes ActiveBody before second life.
        if self.start_second_life {
            victim.start_battle_bus_second_life_with_body_fx(self.frame, source, body_fx);
        }

        if victim.is_host_bridge_member() {
            if self.lethal {
                // World onDie already converted the bridge. Standalone Object
                // calls retain their immediate repairable-husk adapter.
                if !victim.status.keep_as_rubble {
                    victim.convert_bridge_to_rubble_husk(self.frame);
                }
                return false;
            }
        }
        self.lethal && !self.start_second_life
    }
}
