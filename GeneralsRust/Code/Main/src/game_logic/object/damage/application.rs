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
}

#[derive(Debug)]
pub(in crate::game_logic) struct ActiveDamageContinuation {
    pub(super) victim_id: ObjectId,
    pub(super) source: Option<ObjectId>,
    pub(super) damage_type: crate::game_logic::combat::DamageType,
    pub(super) death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    pub(super) fx_type: crate::game_logic::combat::DamageType,
    pub(super) actual_damage: f32,
    pub(super) max_health: f32,
    pub(super) frame: u32,
    pub(super) lethal: bool,
    pub(super) start_second_life: bool,
}

impl DamageApplication {
    pub(in crate::game_logic) fn killed(&self) -> bool {
        match self {
            Self::Complete(killed) => *killed,
            Self::Active(tail) => tail.lethal,
        }
    }

    pub(in crate::game_logic) fn source_id(&self) -> Option<ObjectId> {
        match self {
            Self::Complete(_) => None,
            Self::Active(tail) => tail.source,
        }
    }

    /// Source is freshly observed after world credit and death callbacks, or
    /// supplied by the immediate standalone wrapper with no world credit.
    pub(in crate::game_logic) fn finish(
        self,
        victim: &mut Object,
        source: Option<&HostDamageFxVictim>,
    ) -> bool {
        match self {
            Self::Complete(killed) => killed,
            Self::Active(tail) => tail.finish(victim, source),
        }
    }
}

impl ActiveDamageContinuation {
    fn finish(self, victim: &mut Object, source: Option<&HostDamageFxVictim>) -> bool {
        assert_eq!(
            self.victim_id, victim.id,
            "damage continuation victim identity changed"
        );
        let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
            victim,
            self.fx_type,
            self.actual_damage,
            self.frame,
            source,
        );

        // C++ ActiveBody.cpp:655–662 follows DamageFX, even on a lethal hit.
        if crate::game_logic::host_repulsor_gate::is_enabled()
            && victim.is_kind_of(KindOf::CanBeRepulsed)
            && !victim.status.repulsor
        {
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
            crate::game_logic::host_bridge_behavior::record_mirror(
                victim.id,
                self.actual_damage,
                self.max_health,
                self.source,
                self.damage_type.to_store() as u32,
                self.death_type.ordinal() as u32,
                crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind::Damage,
            );
            if self.lethal {
                // World onDie already converted the bridge. Standalone Object
                // calls retain their immediate repairable-husk adapter.
                if !victim.status.keep_as_rubble {
                    victim.convert_bridge_to_rubble_husk();
                    crate::game_logic::host_bridge_behavior::record_death_link(victim.id);
                }
                return false;
            }
        }
        self.lethal && !self.start_second_life
    }
}
