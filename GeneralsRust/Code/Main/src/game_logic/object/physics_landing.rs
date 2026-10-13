//! PhysicsUpdate landing Body effects, bounce and stunned destruction.
use super::*;

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
        };
        self.maybe_kill_when_resting_on_ground_with_services(&mut services)
    }

    pub(in crate::game_logic) fn maybe_kill_when_resting_on_ground_with_services(
        &mut self,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> bool {
        if !self.kill_when_resting_on_ground || self.status.destroyed {
            return false;
        }
        if self.is_above_terrain() {
            return false;
        }
        if !self.velocity_is_very_small() {
            return false;
        }
        let is_drone = self.is_kind_of(KindOf::Drone);
        // C++: kill if !KINDOF_DRONE OR dead OR unmanned.
        if is_drone && self.is_alive() && !self.status.disabled_unmanned {
            return false;
        }
        if self.status.destroyed {
            return false;
        }
        let amount = self.health.current.max(0.0);
        services.apply(
            self,
            amount,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        )
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
        };
        self.apply_shock_fall_damage_with_services(impact_vy, &mut services)
    }

    pub(in crate::game_logic) fn apply_shock_fall_damage_with_services(
        &mut self,
        impact_vy: f32,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> f32 {
        if self.immune_to_falling_damage || self.is_kind_of(KindOf::Projectile) {
            return 0.0;
        }
        // Leftover/C++: netSpeed = -activeVelZ - height_to_speed(minFallHeight).
        // Host Y-up. Compare leftover default ~2.385, not live sqrt(80).
        let min_fall = self.fall_damage_speed_threshold(self.min_fall_speed_for_damage);
        let net_speed = (-impact_vy) - min_fall;
        if net_speed <= 0.0 {
            return 0.0;
        }
        let vx = self.movement.velocity.x;
        let vz = self.movement.velocity.z;
        // Steep-fall gate residual.
        let steep_x =
            vx.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vx).abs() >= Self::FALL_MIN_ANGLE_TAN;
        let steep_z =
            vz.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vz).abs() >= Self::FALL_MIN_ANGLE_TAN;
        if !(steep_x && steep_z) {
            return 0.0;
        }
        let damage_amt = net_speed * self.physics_get_mass() * self.fall_height_damage_factor;
        if damage_amt <= 0.0 {
            return 0.0;
        }
        let source = Some(self.id);
        let killed = services.apply(
            self,
            damage_amt,
            source,
            crate::game_logic::combat::DamageType::Falling,
            crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
        );
        if killed {
            use crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED;
            self.model_condition_bits |= 1u128 << MC_BIT_SPLATTED;
            self.refresh_model_condition_bits();
            // refresh may clear SPLATTED if not wired — re-set after.
            self.model_condition_bits |= 1u128 << MC_BIT_SPLATTED;
        }
        damage_amt
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
        if !self.shock_allow_bounce || new_y > ground_y {
            return 0.0;
        }
        let mut bounce_vy = 0.0;
        let vy = self.movement.velocity.y;
        if old_y > ground_y && vy < 0.0 {
            let stiffness = self.physics_ground_stiffness();
            // C++ desiredAccelZ = fabs(vz)*stiffness; mass≈1 → velocity kick.
            bounce_vy = vy.abs() * stiffness;
        }
        // Damp tumble rates on bounce.
        self.shock_yaw_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_pitch_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_roll_rate *= Self::BOUNCE_YPR_DAMPING;
        // C++ handleBounce vz<0: setAngles(yaw, 0, up>0 ? 0 : PI) before
        // testStunnedUnitForDestruction. Yaw-only overwrite is leftover
        // update_simple gotBounceForce — it must not run before the stun
        // test, and leftover handle_bounce pose is the 0-or-PI roll.
        if vy < 0.0 {
            self.right_physics_pitch_keep_flip();
        }
        if bounce_vy > 0.0 {
            self.movement.velocity.y = bounce_vy;
            // C++ testStunnedUnitForDestruction on successful bounce force.
            if self.test_stunned_unit_for_destruction_with_services(services) {
                return 0.0;
            }
            return bounce_vy;
        }
        // C++ handleBounce bounceForce.z==0: setAllowBouncing(m_originalAllowBounce).
        self.shock_allow_bounce = self.original_allow_bounce;
        0.0
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
        };
        self.test_stunned_unit_for_destruction_with_services(&mut services)
    }

    pub(in crate::game_logic) fn test_stunned_unit_for_destruction_with_services(
        &mut self,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> bool {
        if !self.is_shock_stunned() || self.status.destroyed {
            return false;
        }
        self.ensure_locomotor_surfaces();
        // Upside down when integrated transform up-Y < 0 (C++ Get_Z_Vector().Z).
        if self.physics_transform_up_y() < 0.0 {
            return self.kill_from_stun_destruction(services);
        }
        // C++ obj->isOffMap residual.
        let pos = self.get_position();
        if crate::game_logic::host_deliver_payload::is_off_map_default_residual(pos) {
            return self.kill_from_stun_destruction(services);
        }
        // C++ AIUpdateInterface *aiInt = obj->getAI(); if (!aiInt) return;
        if !self.has_ai_update_interface() {
            return false;
        }
        // C++ isCliffCell && !hasLocomotorForSurface(CLIFF).
        if self.cell_is_cliff && !self.has_locomotor_for_surface(LOCO_SURFACE_CLIFF) {
            return self.kill_from_stun_destruction(services);
        }
        // C++ isUnderwater && !hasLocomotorForSurface(WATER).
        if self.cell_is_underwater && !self.has_locomotor_for_surface(LOCO_SURFACE_WATER) {
            return self.kill_from_stun_destruction(services);
        }
        false
    }

    fn kill_from_stun_destruction(
        &mut self,
        services: &mut super::PhysicsDamageServices<'_>,
    ) -> bool {
        if self.status.destroyed {
            return false;
        }
        // C++ PhysicsBehavior::testStunnedUnitForDestruction → Object::kill().
        let amount = self.health.current.max(0.0);
        let killed = services.apply(
            self,
            amount,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        );
        if killed {
            self.set_ai_state(AIState::Idle);
            self.target = None;
            self.shock_stun_frames = 0;
            self.set_status_disabled_freefall(false);
            self.refresh_model_condition_bits();
        }
        killed
    }
}
