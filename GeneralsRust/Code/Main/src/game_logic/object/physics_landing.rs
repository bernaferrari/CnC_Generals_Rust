//! PhysicsUpdate landing Body effects, bounce and stunned destruction.
use super::*;
mod contact;
pub(in crate::game_logic) use contact::{
    ContactDamage, ContactStep, MotionContact, StampContact, StunContact,
};

#[cfg(test)]
mod supplied_fixture_clock_tests {
    use super::*;

    #[test]
    fn resting_death_uses_supplied_frame_without_retiming_same_id_peer() {
        fn body() -> Object {
            let mut template = ThingTemplate::new("SuppliedRestingBody");
            template.set_health(100.0).add_kind_of(KindOf::Vehicle);
            let mut object = Object::new(template, ObjectId(7), Team::USA);
            object.kill_when_resting_on_ground = true;
            object
        }
        let mut owner = body();
        let mut peer = body();
        let mut owner_events = crate::game_logic::HostHealthEvents::default();
        let mut peer_events = crate::game_logic::HostHealthEvents::default();
        assert!(peer.maybe_kill_when_resting_on_ground(900, &mut peer_events));
        assert_eq!(owner.health.current, 100.0);
        assert_eq!(owner.last_damage_timestamp, None);
        assert!(owner.maybe_kill_when_resting_on_ground(501, &mut owner_events));
        assert_eq!(owner.health.current, 0.0);
        assert_eq!(owner.last_damage_timestamp, Some(501));
        assert_eq!(peer.last_damage_timestamp, Some(900));
    }
}

impl Object {
    #[cfg(test)]
    pub fn maybe_kill_when_resting_on_ground(
        &mut self,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> bool {
        let mut ignored = |_| {};
        let repulsors = false;
        let sources = super::PhysicsDamageSources::default();
        let mut services = super::PhysicsDamageServices {
            sources: &sources,
            frame,
            health_events,
            repulsors: &repulsors,
            player_effects: &mut ignored,
            body_fx: &mut Object::standalone_body_fx,
        };
        self.maybe_kill_when_resting_on_ground_with_services(&mut services)
    }

    pub(in crate::game_logic) fn maybe_kill_when_resting_on_ground_with_services(
        &mut self,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> bool {
        self.prepare_resting_kill()
            .map(|request| services.apply_contact(self, &request))
            .unwrap_or(false)
    }

    /// C++ Object::kill — lethal UNRESISTABLE so Body/Die modules (FX, OCL) run.
    pub fn kill(&mut self, health_events: &mut crate::game_logic::HostHealthEvents) -> bool {
        if self.status.destroyed {
            return false;
        }
        // C++ m_kill replaces the amount with current health after the
        // indestructible and effectively-dead returns. It does not skip them.
        // onDie requires m_prevHealth > 0 after internalChangeHealth stores
        // the pre-subtract value, so a kill at 0 HP does not die again.
        let amount = self.health.current.max(0.0);
        self.take_damage_from_typed_death(
            amount,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            health_events,
        )
    }

    #[cfg(test)]
    pub fn apply_shock_fall_damage(
        &mut self,
        frame: u32,
        impact_vy: f32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> f32 {
        let mut ignored = |_| {};
        let repulsors = false;
        let sources = super::PhysicsDamageSources::default();
        let mut services = super::PhysicsDamageServices {
            sources: &sources,
            frame,
            health_events,
            repulsors: &repulsors,
            player_effects: &mut ignored,
            body_fx: &mut Object::standalone_body_fx,
        };
        self.apply_shock_fall_damage_with_services(impact_vy, &mut services)
    }

    pub(in crate::game_logic) fn apply_shock_fall_damage_with_services(
        &mut self,
        impact_vy: f32,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> f32 {
        let Some(request) = self.prepare_fall_damage(impact_vy) else {
            return 0.0;
        };
        let amount = request.amount;
        services.apply_contact(self, &request);
        self.finish_fall_contact();
        amount
    }

    /// C++ PhysicsBehavior::applyYPRDamping residual.
    pub(super) fn finish_physics_landing_bookkeeping(
        &mut self,
        old_y: f32,
        ground_y: f32,
        impact_vy: f32,
        services: &mut super::PhysicsDamageServices<'_>,
    ) {
        let airborne_end = self.get_position().y > ground_y + 0.05;
        if self.was_airborne_last_frame
            && !airborne_end
            && !self.immune_to_falling_damage
            && !self.landing_splat_done
        {
            self.record_bounce_land(old_y);
            self.pending_ground_collide = true;
            let _ = self.apply_shock_fall_damage_with_services(impact_vy, services);
        }
        self.was_airborne_last_frame = airborne_end;
        self.stamp_airborne_target_from_locomotor();
        if !airborne_end {
            let _ = self.maybe_kill_when_resting_on_ground_with_services(services);
        }
    }

    /// C++ PhysicsBehavior::handleBounce residual (world-Y = C++ Z).
    ///
    /// Returns upward bounce velocity applied (0 if no bounce).
    #[cfg(test)]
    pub fn handle_shock_ground_bounce(
        &mut self,
        frame: u32,
        old_y: f32,
        new_y: f32,
        ground_y: f32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> f32 {
        let mut ignored = |_| {};
        let repulsors = false;
        let sources = super::PhysicsDamageSources::default();
        let mut services = super::PhysicsDamageServices {
            sources: &sources,
            frame,
            health_events,
            repulsors: &repulsors,
            player_effects: &mut ignored,
            body_fx: &mut Object::standalone_body_fx,
        };
        self.handle_shock_ground_bounce_with_services(old_y, new_y, ground_y, &mut services)
    }

    pub(in crate::game_logic) fn handle_shock_ground_bounce_with_services(
        &mut self,
        old_y: f32,
        new_y: f32,
        ground_y: f32,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> f32 {
        let (bounce, request) = self.begin_contact_bounce(old_y, new_y, ground_y);
        if let Some(request) = request {
            let killed = services.apply_contact(self, &request);
            self.finish_stun_kill(killed);
            if killed {
                return 0.0;
            }
        }
        bounce
    }

    /// Default locomotor surfaces residual from KindOf (fail-closed ground units).
    #[cfg(test)]
    pub fn test_stunned_unit_for_destruction(
        &mut self,
        frame: u32,
        health_events: &mut crate::game_logic::HostHealthEvents,
    ) -> bool {
        let mut ignored = |_| {};
        let repulsors = false;
        let sources = super::PhysicsDamageSources::default();
        let mut services = super::PhysicsDamageServices {
            sources: &sources,
            frame,
            health_events,
            repulsors: &repulsors,
            player_effects: &mut ignored,
            body_fx: &mut Object::standalone_body_fx,
        };
        self.test_stunned_unit_for_destruction_with_services(&mut services)
    }

    pub(in crate::game_logic) fn test_stunned_unit_for_destruction_with_services(
        &mut self,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> bool {
        let Some(request) = self.prepare_stun_kill() else {
            return false;
        };
        let killed = services.apply_contact(self, &request);
        self.finish_stun_kill(killed);
        killed
    }
}
