//! Explicit driving-world policy for residual impacts that retain batched death.
//! Read the borrowed policy only after actual DamageFX. Standalone APIs remain inert.
//! C++ ActiveBody.cpp:653-662; full synchronous onDie transfer is tracked separately.
use super::*;

impl Object {
    pub(in crate::game_logic) fn take_damage_from_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_with_repulsor_policy(
            damage,
            source,
            crate::game_logic::combat::DamageType::Unresistable,
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_immediate_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_death_with_host_hp_with_repulsor_policy(
            damage,
            source,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                crate::game_logic::combat::DamageType::Unresistable,
            ),
            true, // force host HP apply
            None,
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_immediate_typed_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_immediate_typed_death_with_repulsor_policy(
            damage,
            source,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_immediate_typed_death_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_death_with_host_hp_with_repulsor_policy(
            damage,
            source,
            damage_type,
            death_type,
            true,
            None,
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_immediate_residual_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type_name: &str,
        death_type_name: &str,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        let damage_type =
            crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                damage_type_name,
            );
        let death_type = crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
            death_type_name,
        );
        self.take_damage_from_immediate_typed_death_with_repulsor_policy(
            damage,
            source,
            damage_type,
            death_type,
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_typed_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_death_with_repulsor_policy(
            damage,
            source,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_typed_death_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_death_fx_with_repulsor_policy(
            damage,
            source,
            damage_type,
            death_type,
            None,
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_typed_death_fx_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_death_fx_at_frame_with_repulsor_policy(
            damage,
            source,
            damage_type,
            death_type,
            fx_override,
            crate::game_logic::host_historic_bonus::logic_frame(),
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_typed_death_at_frame_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_death_fx_at_frame_with_repulsor_policy(
            damage,
            source,
            damage_type,
            death_type,
            None,
            frame,
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_from_typed_death_fx_at_frame_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_with_context_with_repulsor_policy(
            damage,
            source,
            damage_type,
            death_type,
            fx_override,
            frame,
            &DamageHitContext::default(),
            health_events,
            repulsor_policy,
        )
    }

    pub(in crate::game_logic) fn take_damage_with_context_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        frame: u32,
        context: &DamageHitContext,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        let application = self.begin_damage_with_context(
            damage,
            source,
            damage_type,
            death_type,
            fx_override,
            frame,
            context,
            health_events,
        );
        application.finish_standalone(self, context.source(), repulsor_policy)
    }

    fn take_damage_from_typed_death_with_host_hp_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        force_host_hp: bool,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        self.take_damage_from_typed_death_with_host_hp_at_frame_with_repulsor_policy(
            damage,
            source,
            damage_type,
            death_type,
            force_host_hp,
            fx_override,
            crate::game_logic::host_historic_bonus::logic_frame(),
            &DamageHitContext::default(),
            health_events,
            repulsor_policy,
        )
    }

    fn take_damage_from_typed_death_with_host_hp_at_frame_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        force_host_hp: bool,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        frame: u32,
        context: &DamageHitContext,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        let application = self.begin_damage_from_typed_death_with_host_hp_at_frame(
            damage,
            source,
            damage_type,
            death_type,
            force_host_hp,
            fx_override,
            frame,
            context,
            health_events,
        );
        application.finish_standalone(self, context.source(), repulsor_policy)
    }
    /// Preserve Weapon.cpp:1351 airborne exclusion before ordinary radiation damage.
    pub(in crate::game_logic) fn take_radiation_field_tick_with_repulsor_policy(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        health_events: &mut crate::game_logic::HostHealthEvents,
        repulsor_policy: &bool,
    ) -> bool {
        if self.status.airborne_target || self.is_significantly_above_terrain() {
            return false;
        }
        self.take_damage_from_immediate_typed_death_with_repulsor_policy(
            damage,
            source,
            crate::game_logic::combat::DamageType::Radiation,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            health_events,
            repulsor_policy,
        )
    }
}
