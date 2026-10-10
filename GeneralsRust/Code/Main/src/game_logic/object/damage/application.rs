//! C++ ActiveBody.cpp:585–662: callbacks precede credit/onDie, then DamageFX.
//! One owned continuation is consumed before another victim is processed.
use super::super::*;
use crate::game_logic::host_transition_damage_fx::HostDamageFxVictim;

#[derive(Debug)]
#[must_use = "complete this impact synchronously before processing another hit"]
pub(in crate::game_logic) enum DamageApplication {
    // Special/no-HP branches already complete their existing effects.
    Complete(bool),
    Active(ActiveDamageContinuation),
    Healing(BridgeHealingContinuation),
}

/// Owned data crossing the released body borrow at the original callback point.
#[derive(Debug, Clone, Copy)]
pub(in crate::game_logic) struct BridgeBodyCallbacks {
    pub victim: ObjectId,
    pub source: Option<ObjectId>,
    pub input_amount: f32,
    pub max_health: f32,
    pub old_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    pub health_changed: bool,
    pub damage_type: crate::game_logic::combat::DamageType,
    pub death_type: crate::game_logic::host_usa_pilot::HostDeathType,
}

#[derive(Debug)]
pub(in crate::game_logic) struct BridgeHealingContinuation {
    pub(super) callbacks: BridgeBodyCallbacks,
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

    pub(in crate::game_logic) fn set_killed_after_callbacks(&mut self, killed: bool) {
        if let Self::Active(tail) = self {
            tail.lethal = killed;
        }
    }

    pub(in crate::game_logic) fn source_id(&self) -> Option<ObjectId> {
        match self {
            Self::Complete(_) => None,
            Self::Active(tail) => tail.source,
            Self::Healing(tail) => tail.callbacks.source,
        }
    }

    pub(in crate::game_logic) fn bridge_callbacks(&self) -> Option<BridgeBodyCallbacks> {
        match self {
            Self::Active(tail) => tail.bridge_callbacks,
            Self::Healing(tail) => Some(tail.callbacks),
            Self::Complete(_) => None,
        }
    }

    pub(in crate::game_logic) fn dispatch_damage_fx(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
    ) -> DamageAfterFx {
        if let Self::Active(tail) = &self {
            assert_eq!(
                tail.victim_id, victim.id,
                "damage continuation victim identity changed"
            );
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                victim,
                tail.fx_type,
                tail.actual_damage,
                tail.frame,
                source,
            );
        }
        if let Self::Healing(tail) = &self {
            assert_eq!(tail.callbacks.victim, victim.id);
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                victim,
                tail.fx_type,
                tail.actual_amount,
                tail.frame,
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

    /// Preserve standalone bridge publication and observe policy after DamageFX.
    pub(in crate::game_logic) fn finish_standalone(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
        repulsor_policy: &bool,
    ) -> bool {
        let callbacks = self.bridge_callbacks();
        let lethal = self.killed();
        let after_fx = self.dispatch_damage_fx(victim, source);
        let result = after_fx.finish(victim, source, *repulsor_policy);
        // Standalone adapters have no driving owner. Preserve their existing
        // deferred boundary while world callers consume callbacks directly.
        if let Some(callbacks) = callbacks {
            if callbacks.health_changed {
                crate::game_logic::host_bridge_behavior::record_mirror(
                    callbacks.victim,
                    callbacks.input_amount,
                    callbacks.max_health,
                    callbacks.source,
                    callbacks.damage_type.to_store() as u32,
                    callbacks.death_type.ordinal() as u32,
                    if callbacks.damage_type == crate::game_logic::combat::DamageType::Healing {
                        crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind::Heal
                    } else {
                        crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind::Damage
                    },
                );
            }
            if lethal {
                crate::game_logic::host_bridge_behavior::record_death_link(callbacks.victim);
            }
        }
        result
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
        match self.0 {
            DamageApplication::Complete(killed) => killed,
            DamageApplication::Healing(_) => false,
            DamageApplication::Active(tail) => {
                tail.finish_after_damage_fx(victim, source, enable_repulsors)
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
            victim.start_battle_bus_second_life_at_frame(self.frame);
            // C++ setMaxHealth(FULLY_HEAL) calls internalChangeHealth, which
            // immediately recalculates the healthy body and model state.
            victim.refresh_model_condition_bits_with_source(source);
        }

        if victim.is_host_bridge_member() {
            if self.lethal {
                // World onDie already converted the bridge. Standalone Object
                // calls retain their immediate repairable-husk adapter.
                if !victim.status.keep_as_rubble {
                    victim.convert_bridge_to_rubble_husk();
                }
                return false;
            }
        }
        self.lethal && !self.start_second_life
    }
}
