//! Synchronous ground-contact phases. Only owned scalar continuations cross a Body callback.
use super::*;
use game_engine::common::ini::ini_locomotor::LocomotorStore;

pub(in crate::game_logic) struct ContactDamage {
    pub amount: f32,
    pub source: Option<ObjectId>,
    pub kind: crate::game_logic::combat::DamageType,
    pub death: crate::game_logic::host_usa_pilot::HostDeathType,
    pub kill: bool,
}
impl ContactDamage {
    fn kill() -> Self {
        Self {
            amount: 0.0,
            source: None,
            kind: crate::game_logic::combat::DamageType::Unresistable,
            death: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            kill: true,
        }
    }
}
pub(in crate::game_logic) enum ContactStep<S, T> {
    Complete(T),
    /// Physics invalid-position disposal is not an ActiveBody damage attempt.
    DirectDelete(T),
    Damage {
        request: ContactDamage,
        next: S,
    },
    GroundCollide {
        next: S,
    },
    PositionFx {
        reaction: super::super::PositionReaction,
        next: S,
    },
}
pub(in crate::game_logic) enum StampContact {
    Start(f32),
    AfterGroundCollide(f32),
    AfterFall,
    Grounded,
    AfterRest,
}
pub(in crate::game_logic) struct LandingTail {
    airborne: bool,
    old_y: f32,
    impact_vy: f32,
    bounced: bool,
}
pub(in crate::game_logic) enum MotionContact {
    Start(f32),
    AfterBounce(LandingTail),
    AfterPosition {
        ground_y: f32,
        new_pos: glam::Vec3,
        old_y: f32,
        impact_vy: f32,
        bounce_force: Option<glam::Vec3>,
    },
    AfterGroundCollide(LandingTail),
    AfterFall(LandingTail),
    AfterRest(bool),
}
pub(in crate::game_logic) struct StunTail {
    already_ground: bool,
    was_airborne: bool,
}
pub(in crate::game_logic) struct StunLanding {
    tail: StunTail,
    pos: glam::Vec3,
    ground_y: f32,
    old_y: f32,
    impact_vy: f32,
    was_air: bool,
    bounce: f32,
}
pub(in crate::game_logic) enum StunContact {
    Start,
    AfterBounce(StunLanding),
    AfterLandingPosition(StunLanding),
    AfterGroundCollide(StunLanding),
    AfterFall(StunLanding),
    AfterRest(StunTail),
    SettledRest,
}
impl Object {
    pub(in crate::game_logic) fn drive_standalone_contact<S, T>(
        &mut self,
        mut state: S,
        services: &mut super::PhysicsDamageServices<'_>,
        step: fn(
            &mut Object,
            S,
            bool,
            &mut crate::game_logic::ObjectAudio<'_>,
        ) -> ContactStep<S, T>,
    ) -> T {
        let mut killed = false;
        loop {
            match step(
                self,
                state,
                killed,
                &mut crate::game_logic::ObjectAudio::callback(&mut |object, request| {
                    (services.body_fx)(
                        object,
                        super::super::fx::BodyStateFx::MovementStop(request),
                        None,
                    )
                }),
            ) {
                ContactStep::Complete(value) => return value,
                ContactStep::DirectDelete(value) => {
                    // Detached compatibility has no owning roster to retire.
                    // The live World driver below performs the synchronous
                    // receiving delete callbacks and queues physical disposal.
                    self.status.destroyed = true;
                    self.set_locomotor_goal_none();
                    self.movement.path.clear();
                    return value;
                }
                ContactStep::PositionFx { reaction, next } => {
                    // Detached physics has no native terrain loan. Registered
                    // actors always use drive_owned_contact_with_effects below.
                    debug_assert!(!reaction.has_tree_fx());
                    self.finish_position_reaction(reaction);
                    state = next;
                }
                ContactStep::GroundCollide { next } => {
                    // This component driver has no receiving World callbacks.
                    // A real Parachute must use the registered World driver.
                    assert!(
                        self.thing().template.contain_module.kind
                            != crate::game_logic::ContainModuleKind::Parachute,
                        "Parachute ground collision requires its receiving World"
                    );
                    self.pending_ground_collide = true;
                    state = next;
                }
                ContactStep::Damage { request, next } => {
                    killed = services.apply_contact(self, &request);
                    state = next;
                }
            }
        }
    }
    pub(super) fn prepare_fall_damage(&self, impact_vy: f32) -> Option<ContactDamage> {
        if self.immune_to_falling_damage || self.is_kind_of(KindOf::Projectile) {
            return None;
        }
        // Leftover/C++: netSpeed = -activeVelZ - height_to_speed(minFallHeight).
        // Host Y-up. Compare leftover default ~2.385, not live sqrt(80).
        let min_fall = self.fall_damage_speed_threshold(self.min_fall_speed_for_damage);
        let net_speed = (-impact_vy) - min_fall;
        if net_speed <= 0.0 {
            return None;
        }
        let vx = self.movement.velocity.x;
        let vz = self.movement.velocity.z;
        // Steep-fall gate residual.
        let steep_x =
            vx.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vx).abs() >= Self::FALL_MIN_ANGLE_TAN;
        let steep_z =
            vz.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vz).abs() >= Self::FALL_MIN_ANGLE_TAN;
        if !(steep_x && steep_z) {
            return None;
        }
        let damage_amt = net_speed * self.physics_get_mass() * self.fall_height_damage_factor;
        if damage_amt <= 0.0 {
            return None;
        }
        Some(ContactDamage {
            amount: damage_amt,
            source: Some(self.id),
            kind: crate::game_logic::combat::DamageType::Falling,
            death: crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
            kill: false,
        })
    }
    pub(super) fn prepare_resting_kill(&self) -> Option<ContactDamage> {
        if !self.kill_when_resting_on_ground {
            return None;
        }
        if self.is_above_terrain() {
            return None;
        }
        if !self.velocity_is_very_small() {
            return None;
        }
        let is_drone = self.is_kind_of(KindOf::Drone);
        // C++: kill if !KINDOF_DRONE OR dead OR unmanned.
        if is_drone && self.is_alive() && !self.status.disabled_unmanned {
            return None;
        }
        Some(ContactDamage::kill())
    }
    pub(super) fn prepare_stun_kill(&mut self) -> Option<ContactDamage> {
        if !self.is_shock_stunned() {
            return None;
        }
        self.ensure_locomotor_surfaces();
        // Upside down when integrated transform up-Y < 0 (C++ Get_Z_Vector().Z).
        if self.physics_transform_up_y() < 0.0 {
            return Some(ContactDamage::kill());
        }
        // C++ obj->isOffMap residual.
        let pos = self.get_position();
        if crate::game_logic::host_deliver_payload::is_off_map_default_residual(pos) {
            return Some(ContactDamage::kill());
        }
        // C++ AIUpdateInterface *aiInt = obj->getAI(); if (!aiInt) return;
        if !self.has_ai_update_interface() {
            return None;
        }
        // C++ isCliffCell && !hasLocomotorForSurface(CLIFF).
        if self.cell_is_cliff && !self.has_locomotor_for_surface(LOCO_SURFACE_CLIFF) {
            return Some(ContactDamage::kill());
        }
        // C++ isUnderwater && !hasLocomotorForSurface(WATER).
        if self.cell_is_underwater && !self.has_locomotor_for_surface(LOCO_SURFACE_WATER) {
            return Some(ContactDamage::kill());
        }
        None
    }
    pub(super) fn finish_fall_contact(&mut self) {
        // CPP PhysicsUpdate865 rereads effective death after all callbacks.
        if !self.is_alive() {
            let mask = 1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED;
            self.model_condition_bits |= mask;
            self.refresh_model_condition_bits();
            self.model_condition_bits |= mask;
        }
    }
    pub(super) fn finish_stun_kill(
        &mut self,
        killed: bool,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        if killed {
            self.set_ai_state(AIState::Idle, audio);
            self.target = None;
            self.shock_stun_frames = 0;
            self.set_status_disabled_freefall(false);
            self.refresh_model_condition_bits();
        }
    }
    fn clear_ground_freefall(&mut self) {
        self.is_in_freefall = false;
        self.set_status_disabled_freefall(false);
        self.model_condition_bits &=
            !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL);
    }
    pub(in crate::game_logic) fn step_stamp_contact(
        &mut self,
        state: StampContact,
        killed: bool,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> ContactStep<StampContact, ()> {
        self.step_stamp_contact_with_effects(state, killed, 0, None, &mut Default::default(), audio)
    }
    pub(in crate::game_logic) fn step_stamp_contact_with_effects(
        &mut self,
        state: StampContact,
        _killed: bool,
        frame: u32,
        locomotors: Option<&LocomotorStore>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> ContactStep<StampContact, ()> {
        match state {
            StampContact::Start(ground_y) => {
                self.ground_height = ground_y;
                let mut pos = self.get_position();
                if pos.y <= ground_y {
                    let impact_vy = self.movement.velocity.y;
                    let was_airborne = self.was_airborne_last_frame;
                    let old_y = pos.y;
                    self.movement.velocity.y += ground_y - pos.y;
                    if self.movement.velocity.y > 0.0 {
                        self.movement.velocity.y = 0.0;
                    }
                    self.invalidate_velocity_magnitude();
                    pos.y = ground_y;
                    self.set_position_with_effects(pos, frame, effects);
                    self.allow_to_fall = false;
                    if was_airborne && !self.immune_to_falling_damage {
                        self.record_bounce_land(old_y);
                        return ContactStep::GroundCollide {
                            next: StampContact::AfterGroundCollide(impact_vy),
                        };
                    }
                    if was_airborne && !self.immune_to_falling_damage {
                        self.landing_splat_done = true;
                    }
                    return self.step_stamp_contact_with_effects(
                        StampContact::Grounded,
                        false,
                        frame,
                        locomotors,
                        effects,
                        audio,
                    );
                } else if self.is_in_freefall {
                    self.set_status_disabled_freefall(true);
                    self.model_condition_bits |=
                        1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
                } else if self.stick_to_ground && !self.allow_to_fall {
                    pos.y = ground_y;
                    self.set_position_with_effects(pos, frame, effects);
                }
            }
            StampContact::AfterGroundCollide(impact_vy) => {
                if let Some(request) = self.prepare_fall_damage(impact_vy) {
                    return ContactStep::Damage {
                        request,
                        next: StampContact::AfterFall,
                    };
                }
                self.landing_splat_done = true;
                return self.step_stamp_contact_with_effects(
                    StampContact::Grounded,
                    false,
                    frame,
                    locomotors,
                    effects,
                    audio,
                );
            }
            StampContact::AfterFall => {
                self.finish_fall_contact();
                self.landing_splat_done = true;
                return self.step_stamp_contact_with_effects(
                    StampContact::Grounded,
                    false,
                    frame,
                    locomotors,
                    effects,
                    audio,
                );
            }
            StampContact::Grounded => {
                self.was_airborne_last_frame = false;
                self.clear_ground_freefall();
                if self.shock_stun_frames > 0 {
                    self.shock_grounded_once = true;
                    self.model_condition_bits &= !(1u128
                        << crate::game_logic::host_enum_table_residual::MC_BIT_STUNNED_FLAILING);
                    self.model_condition_bits |=
                        1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_STUNNED;
                }
                if self.velocity_is_very_small() {
                    if let Some(request) = self.prepare_resting_kill() {
                        return ContactStep::Damage {
                            request,
                            next: StampContact::AfterRest,
                        };
                    }
                }
            }
            StampContact::AfterRest => {}
        }
        self.stamp_airborne_target_from_locomotor();
        ContactStep::Complete(())
    }
    pub(in crate::game_logic) fn step_motion_contact(
        &mut self,
        state: MotionContact,
        killed: bool,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> ContactStep<MotionContact, bool> {
        self.step_motion_contact_with_effects(
            state,
            killed,
            0,
            None,
            &mut Default::default(),
            audio,
        )
    }
    pub(in crate::game_logic) fn step_motion_contact_with_effects(
        &mut self,
        state: MotionContact,
        killed: bool,
        frame: u32,
        locomotors: Option<&LocomotorStore>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> ContactStep<MotionContact, bool> {
        match state {
            MotionContact::Start(ground_y) => {
                self.apply_leftover_taxiing_locomotor_set(locomotors);

                self.ground_height = ground_y;

                // C++ PhysicsUpdate.cpp:626 — DISABLED_HELD skips gravity/friction/Euler.
                // Landing onCollide / bounce still run outside that gate.
                if self.is_physics_held() {
                    let old_y = self.get_position().y;
                    let impact_vy = self.movement.velocity.y;
                    let tail = LandingTail {
                        airborne: self.get_position().y > ground_y + 0.05,
                        old_y,
                        impact_vy,
                        bounced: false,
                    };
                    return self.begin_motion_landing(tail);
                }

                // C++ does not bail on isEffectivelyDead — wrecks keep Euler.
                if self.is_kind_of(crate::game_logic::KindOf::Structure) && !self.allow_to_fall {
                    return ContactStep::Complete(false);
                }

                self.ground_height = ground_y;

                // Gravity/friction/acceleration were admitted once by the
                // receiving Physics force phase before this contact continuation.

                let old_pos = self.get_position();
                let old_y = old_pos.y;

                // C++ PhysicsBehavior::update is the sole Euler step. Live march already
                // integrated pos += v*dt in update_movement (v is units/second).
                // Applying `old_pos + v` here treats per-second velocity as per-frame
                // and ~25x retail speed. Only integrate leftover Y / shock here when
                // the unit is not already path-marching this frame.
                let v = self.movement.velocity;
                let marching = self.movement.target_position.is_some()
                    || !self.movement.path.is_empty()
                    || (self.locomotor_goal_type == LocoGoalType::Angle && self.min_speed > 0.0);
                let vertical = if self.loco_appearance == LocomotorAppearance::Thrust {
                    v.y / 30.
                } else {
                    v.y
                };
                let mut new_pos = if self.is_braking && self.is_kind_of(KindOf::Projectile) {
                    old_pos
                } else if self.is_braking {
                    old_pos + glam::Vec3::new(0., vertical, 0.)
                } else if self.loco_appearance == LocomotorAppearance::Thrust {
                    old_pos + v / 30.
                } else if marching {
                    old_pos + glam::Vec3::new(0., vertical, 0.)
                } else {
                    old_pos + v
                };

                // YPR: stun tick already integrates while stunned (avoid double apply).
                if self.shock_stun_frames == 0 {
                    let pryf = self.pitch_roll_yaw_factor;
                    let yaw_rate = self.shock_yaw_rate * pryf;
                    let pitch_rate =
                        self.shock_pitch_rate * pryf * self.center_of_mass_pitch_scale();
                    let roll_rate = self.shock_roll_rate * pryf;
                    self.apply_physics_ypr_with_effects(
                        yaw_rate, pitch_rate, roll_rate, frame, effects,
                    );
                }

                let bounce_force = self.compute_ground_bounce_force_with_operation(
                    old_y, new_pos.y, ground_y, frame, effects,
                );

                // Remember z-vel prior to ground-slam (host Y).
                if new_pos.y <= ground_y {
                    let dy = ground_y - new_pos.y;
                    self.movement.velocity.y += dy;
                    if self.movement.velocity.y > 0.0 {
                        self.movement.velocity.y = 0.0;
                    } else if dy.abs() < 1e-6 && self.movement.velocity.y < 0.0 {
                        // Marching / no Y integrate: do not accumulate gravity into vy.
                        self.movement.velocity.y = 0.0;
                    }
                    self.invalidate_velocity_magnitude();
                    new_pos.y = ground_y;
                    self.allow_to_fall = false;
                    // Stunned flailing → stunned residual on first ground hit.
                    if self.shock_stun_frames > 0 && !self.shock_grounded_once {
                        self.shock_grounded_once = true;
                    }
                } else if new_pos.y > ground_y {
                    if self.stick_to_ground && !self.allow_to_fall {
                        new_pos.y = ground_y;
                    }
                    // Uphill refusal is moveTowardsPositionLegs, not PhysicsBehavior.
                    // Climber slope: while FLAG_CLIMBING, scale leftover XY when slope>1
                    // (Locomotor.cpp:1734-1739 desiredSpeed /= groundSlope*4).
                    if self.is_climbing
                        && matches!(self.loco_appearance, LocomotorAppearance::Climber)
                    {
                        let slope = (new_pos.y - old_pos.y).abs().max(1.0);
                        if slope > 1.0 {
                            let scale = 1.0 / (slope * 4.0);
                            self.movement.velocity.x *= scale;
                            self.movement.velocity.z *= scale;
                            self.invalidate_velocity_magnitude();
                        }
                    }
                }

                // C++ PhysicsUpdate.cpp:665-669 calls destroyObject for NaN;
                // it never changes Body health or invents an onDie callback.
                // Retain Main's broader pre-existing infinity rejection and
                // stop before publishing an invalid transform (a Rust safety
                // policy, not an assertion that C++ tests infinity or returns).
                if !new_pos.is_finite() {
                    return ContactStep::DirectDelete(false);
                }
                let mut transform = self.get_transform_matrix();
                transform.w_axis = new_pos.extend(1.0);
                let reaction = self.prepare_transform_tree_reaction(transform, frame, effects);
                let next = MotionContact::AfterPosition {
                    ground_y,
                    new_pos,
                    old_y,
                    impact_vy: v.y,
                    bounce_force,
                };
                // Keep the world-owned continuation even when no tree cursor
                // was produced. Its receiving team factory marks the edge
                // after this Object loan ends; the standalone driver retains
                // its local attachment-backed fallback.
                ContactStep::PositionFx { reaction, next }
            }
            MotionContact::AfterPosition {
                ground_y,
                new_pos,
                old_y,
                impact_vy,
                bounce_force,
            } => {
                // PhysicsUpdate.cpp804: reset AFTER setTransform callbacks,
                // before bounce admission. Callback-added force is cleared at
                // this original boundary; bounce accumulates for next frame.
                self.physics_accel = glam::Vec3::ZERO;
                self.sync_shock_up_from_transform();
                let tail = LandingTail {
                    airborne: new_pos.y > ground_y + 0.05,
                    old_y,
                    impact_vy,
                    bounced: bounce_force.is_some() && self.shock_allow_bounce,
                };
                if let Some(force) = bounce_force {
                    if self.shock_allow_bounce {
                        // PhysicsUpdate.cpp813: bounce force is admitted after
                        // this frame's Euler and remains pending for next update.
                        self.apply_physics_force(force);
                        if let Some(request) = self.prepare_stun_kill() {
                            return ContactStep::Damage {
                                request,
                                next: MotionContact::AfterBounce(tail),
                            };
                        }
                        if !self.status.destroyed {
                            self.right_physics_pitch_keep_flip_with_operation(frame, effects);
                        }
                    }
                }
                self.begin_motion_landing(tail)
            }
            MotionContact::AfterBounce(tail) => {
                self.finish_stun_kill(killed, audio);
                if !killed && !self.status.destroyed {
                    self.right_physics_pitch_keep_flip_with_operation(frame, effects);
                }
                self.begin_motion_landing(tail)
            }
            MotionContact::AfterGroundCollide(tail) => {
                if let Some(request) = self.prepare_fall_damage(tail.impact_vy) {
                    ContactStep::Damage {
                        request,
                        next: MotionContact::AfterFall(tail),
                    }
                } else {
                    self.finish_motion_landing(tail)
                }
            }
            MotionContact::AfterFall(tail) => {
                self.finish_fall_contact();
                self.finish_motion_landing(tail)
            }
            MotionContact::AfterRest(bounced) => ContactStep::Complete(bounced),
        }
    }
    fn begin_motion_landing(&mut self, tail: LandingTail) -> ContactStep<MotionContact, bool> {
        if self.was_airborne_last_frame
            && !tail.airborne
            && !self.immune_to_falling_damage
            && !self.landing_splat_done
        {
            self.record_bounce_land(tail.old_y);
            return ContactStep::GroundCollide {
                next: MotionContact::AfterGroundCollide(tail),
            };
        }
        self.finish_motion_landing(tail)
    }
    fn finish_motion_landing(&mut self, tail: LandingTail) -> ContactStep<MotionContact, bool> {
        if !tail.airborne {
            self.clear_ground_freefall();
        }
        self.was_airborne_last_frame = tail.airborne;
        self.stamp_airborne_target_from_locomotor();
        if !tail.airborne {
            if let Some(request) = self.prepare_resting_kill() {
                return ContactStep::Damage {
                    request,
                    next: MotionContact::AfterRest(tail.bounced),
                };
            }
        }
        ContactStep::Complete(tail.bounced)
    }
    pub(in crate::game_logic) fn step_stun_contact(
        &mut self,
        state: StunContact,
        killed: bool,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> ContactStep<StunContact, ()> {
        self.step_stun_contact_with_effects(state, killed, 0, None, &mut Default::default(), audio)
    }
    pub(in crate::game_logic) fn step_stun_contact_with_effects(
        &mut self,
        state: StunContact,
        killed: bool,
        frame: u32,
        _locomotors: Option<&LocomotorStore>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> ContactStep<StunContact, ()> {
        match state {
            StunContact::Start => {
                if self.shock_stun_frames == 0 {
                    self.shock_yaw_rate *= 0.85;
                    self.shock_pitch_rate *= 0.85;
                    self.shock_roll_rate *= 0.85;
                    if self.shock_yaw_rate.abs() < 1e-4 {
                        self.shock_yaw_rate = 0.0;
                    }
                    if self.movement.velocity.y.abs() < 0.25 {
                        self.movement.velocity.y = 0.0;
                        self.shock_was_airborne = false;
                        self.shock_allow_bounce = false;
                        self.set_status_disabled_freefall(false);
                        if let Some(request) = self.prepare_resting_kill() {
                            return ContactStep::Damage {
                                request,
                                next: StunContact::SettledRest,
                            };
                        }
                    }
                    self.refresh_model_condition_bits();
                    return ContactStep::Complete(());
                }
                // Capture before this frame's land so first ground contact keeps STUNNED
                // one frame (C++ ≈1 frame grounded stun; FLAILING → STUNNED visible).
                // Also record pre-tick height band so relief defers on the landing frame.
                let already_on_ground =
                    self.shock_grounded_once || self.get_position().y <= self.ground_height + 0.05;
                let was_significantly_airborne = self.is_significantly_above_terrain();
                // C++ has no stun timer. Keep IS_STUNNED until settle relief.
                // Integrate YPR while stunned (tumble settle). Motion step skips YPR
                // when stunned so this is the sole live HAS_PITCHROLLYAW pass.
                // C++ PhysicsUpdate.cpp:715-727 COM sine applies every YPR frame,
                // including stunned (shocked units keep pitch/roll rates).
                let pryf = self.pitch_roll_yaw_factor;
                self.apply_physics_ypr_with_effects(
                    self.shock_yaw_rate * pryf,
                    self.shock_pitch_rate * pryf * self.center_of_mass_pitch_scale(),
                    self.shock_roll_rate * pryf,
                    frame,
                    effects,
                );
                self.shock_yaw_rate *= 0.92;
                self.shock_pitch_rate *= 0.92;
                self.shock_roll_rate *= 0.92;

                // Vertical freefall / bounce residual (host Y-up == C++ Z).
                let ground_y = self.ground_height;
                let old_y = self.get_position().y;
                // Gravity while airborne or still carrying vertical velocity.
                if old_y > ground_y + 0.01 || self.movement.velocity.y.abs() > 0.01 {
                    self.movement.velocity.y += self.physics_gravity();
                    let mut pos = self.get_position();
                    let new_y = pos.y + self.movement.velocity.y;
                    if new_y <= ground_y {
                        // Capture impact velocity before bounce/slam (C++ activeVelZ residual).
                        let impact_vy = self.movement.velocity.y;
                        let was_air = self.shock_was_airborne || old_y > ground_y + 0.01;

                        let (bounce, request) = self.begin_contact_bounce_with_operation(
                            old_y, new_y, ground_y, frame, effects,
                        );
                        let data = StunLanding {
                            tail: StunTail {
                                already_ground: already_on_ground,
                                was_airborne: was_significantly_airborne,
                            },
                            pos,
                            ground_y,
                            old_y,
                            impact_vy,
                            was_air,
                            bounce,
                        };
                        if let Some(request) = request {
                            return ContactStep::Damage {
                                request,
                                next: StunContact::AfterBounce(data),
                            };
                        }
                        return self.continue_stun_landing(data, frame, effects);
                    } else {
                        pos.y = new_y;
                        self.set_position_keep_rotation_with_effects(pos, frame, effects);
                        self.shock_was_airborne = true;
                        self.set_status_disabled_freefall(true);
                    }
                } else {
                    if self.movement.velocity.y.abs() < 0.5 {
                        self.movement.velocity.y = 0.0;
                    }
                    self.set_status_disabled_freefall(false);
                    self.shock_was_airborne = false;
                }
                self.finish_stun_ground(StunTail {
                    already_ground: already_on_ground,
                    was_airborne: was_significantly_airborne,
                })
            }
            StunContact::AfterBounce(mut data) => {
                self.finish_stun_kill(killed, audio);
                if killed {
                    data.bounce = 0.0;
                }
                self.continue_stun_landing(data, frame, effects)
            }
            StunContact::AfterLandingPosition(data) => self.finish_stun_landing_position(data),
            StunContact::AfterGroundCollide(data) => {
                if let Some(request) = self.prepare_fall_damage(data.impact_vy) {
                    ContactStep::Damage {
                        request,
                        next: StunContact::AfterFall(data),
                    }
                } else {
                    self.finish_stun_landing(data)
                }
            }
            StunContact::AfterFall(data) => {
                self.finish_fall_contact();
                self.finish_stun_landing(data)
            }
            StunContact::AfterRest(tail) => {
                self.complete_stun_tail(tail);
                ContactStep::Complete(())
            }
            StunContact::SettledRest => {
                self.refresh_model_condition_bits();
                ContactStep::Complete(())
            }
        }
    }
    pub(super) fn begin_contact_bounce(
        &mut self,
        old_y: f32,
        new_y: f32,
        ground_y: f32,
    ) -> (f32, Option<ContactDamage>) {
        self.begin_contact_bounce_with_operation(old_y, new_y, ground_y, 0, &mut Default::default())
    }

    pub(super) fn begin_contact_bounce_with_operation(
        &mut self,
        old_y: f32,
        new_y: f32,
        ground_y: f32,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (f32, Option<ContactDamage>) {
        if !self.shock_allow_bounce || new_y > ground_y {
            return (0.0, None);
        }
        let vy = self.movement.velocity.y;
        let bounce = if old_y > ground_y && vy < 0.0 {
            vy.abs() * self.physics_ground_stiffness()
        } else {
            0.0
        };
        self.shock_yaw_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_pitch_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_roll_rate *= Self::BOUNCE_YPR_DAMPING;
        if vy < 0.0 {
            self.right_physics_pitch_keep_flip_with_operation(operation_frame, operation_effects);
        }
        if bounce > 0.0 {
            self.movement.velocity.y = bounce;
            (bounce, self.prepare_stun_kill())
        } else {
            self.shock_allow_bounce = self.original_allow_bounce;
            (0.0, None)
        }
    }
    fn continue_stun_landing(
        &mut self,
        mut data: StunLanding,
        frame: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> ContactStep<StunContact, ()> {
        data.pos.y = data.ground_y;
        // CPP PhysicsUpdate628 keeps mtx by value across handleBounce's
        // destruction callback, then796 restores that saved translation on
        // the current afterBounceMatrix. XZ is intentionally not reread here.
        let mut matrix = self.get_transform_matrix();
        matrix.w_axis = data.pos.extend(1.0);
        let reaction = self.prepare_transform_tree_reaction(matrix, frame, effects);
        self.sync_shock_up_from_transform();
        ContactStep::PositionFx {
            reaction,
            next: StunContact::AfterLandingPosition(data),
        }
    }
    fn finish_stun_landing_position(&mut self, data: StunLanding) -> ContactStep<StunContact, ()> {
        if !self.shock_grounded_once {
            self.shock_grounded_once = true;
        }
        if data.was_air && !self.landing_splat_done {
            self.record_bounce_land(data.old_y);
            return ContactStep::GroundCollide {
                next: StunContact::AfterGroundCollide(data),
            };
        }
        self.finish_stun_landing(data)
    }
    fn finish_stun_landing(&mut self, data: StunLanding) -> ContactStep<StunContact, ()> {
        if data.bounce <= 0.0 {
            if self.movement.velocity.y < 0.0 {
                self.movement.velocity.y = 0.0;
            }
            self.shock_was_airborne = false;
            self.set_status_disabled_freefall(false);
        } else {
            self.shock_was_airborne = true;
            self.set_status_disabled_freefall(true);
        }
        self.finish_stun_ground(data.tail)
    }
    fn finish_stun_ground(&mut self, tail: StunTail) -> ContactStep<StunContact, ()> {
        if self.get_position().y <= self.ground_height + 0.01 {
            self.movement.velocity.x *= 0.92;
            self.movement.velocity.z *= 0.92;
            if self.movement.velocity.y <= 0.01 {
                self.set_status_disabled_freefall(false);
            }
            if let Some(request) = self.prepare_resting_kill() {
                return ContactStep::Damage {
                    request,
                    next: StunContact::AfterRest(tail),
                };
            }
        }
        self.complete_stun_tail(tail);
        ContactStep::Complete(())
    }
    fn complete_stun_tail(&mut self, tail: StunTail) {
        self.maybe_clear_shock_stun_relief(tail.already_ground, tail.was_airborne);
        self.refresh_model_condition_bits();
    }
}

impl crate::game_logic::GameLogic {
    /// Drive one physics domain operation at its original call site. No object
    /// loan survives the synchronous score/onDie/Armor boundary.
    pub(in crate::game_logic) fn drive_owned_contact<S, T>(
        &mut self,
        id: ObjectId,
        state: S,
        step: fn(
            &mut Object,
            S,
            bool,
            u32,
            Option<&LocomotorStore>,
            &mut crate::game_logic::ScriptPresentationEffects<'_>,
            &mut crate::game_logic::ObjectAudio<'_>,
        ) -> ContactStep<S, T>,
    ) -> Option<T> {
        self.drive_owned_contact_with_effects(
            id,
            state,
            step,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("receiving Physics contact callback failed")
    }

    pub(in crate::game_logic) fn drive_owned_contact_with_effects<S, T>(
        &mut self,
        id: ObjectId,
        mut state: S,
        step: fn(
            &mut Object,
            S,
            bool,
            u32,
            Option<&LocomotorStore>,
            &mut crate::game_logic::ScriptPresentationEffects<'_>,
            &mut crate::game_logic::ObjectAudio<'_>,
        ) -> ContactStep<S, T>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<T>> {
        let Some(birth) = self
            .objects
            .get(&id)
            .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token))
        else {
            return Ok(None);
        };
        let mut killed = false;
        loop {
            if !self.objects.get(&id).is_some_and(|object| {
                birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) {
                return Ok(None);
            }
            let frame = self.frame;
            let next = step(
                self.objects
                    .get_mut(&id)
                    .expect("receiving birth checked above"),
                state,
                killed,
                frame,
                self.locomotor_definitions.as_ref(),
                &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            match next {
                ContactStep::Complete(value) => return Ok(Some(value)),
                ContactStep::DirectDelete(value) => {
                    // The Object loan has ended; direct deletion is terminal,
                    // so no actor state is resumed after its callbacks.
                    self.destroy_object_with_effects(id, presentation_effects);
                    return Ok(Some(value));
                }
                ContactStep::GroundCollide { next } => {
                    // PhysicsUpdate.cpp:827-858: onCollide(NULL) observes the
                    // installed pose before this same actor's falling damage.
                    self.dispatch_physics_ground_collide_with_effects(id, presentation_effects)?;
                    state = next;
                }
                ContactStep::PositionFx { reaction, next } => {
                    if !self.complete_owned_position_reaction(id, reaction, presentation_effects) {
                        return Ok(None);
                    }
                    state = next;
                }
                ContactStep::Damage { request, next } => {
                    let result = if request.kill {
                        self.apply_owned_kill_with_effects(
                            id,
                            request.kind,
                            request.death,
                            presentation_effects,
                        )
                    } else {
                        let context = DamageHitContext::new(
                            request.source.and_then(|source| self.objects.get(&source)),
                            None,
                            request.kind,
                        );
                        self.apply_owned_damage_with_effects(
                            id,
                            request.amount,
                            request.source,
                            request.kind,
                            request.death,
                            None,
                            &context,
                            presentation_effects,
                        )
                    };
                    killed = result.is_some_and(|result| result.destroyed());
                    state = next;
                }
            }
        }
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "tree_topple_phase_tests.rs"]
mod tree_topple_phase_tests;

#[cfg(all(test, feature = "game_client"))]
#[path = "stun_landing_position_owner_tests.rs"]
mod stun_landing_position_owner_tests;
