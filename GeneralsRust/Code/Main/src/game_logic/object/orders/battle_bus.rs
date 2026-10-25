use super::*;

impl Object {
    /// Install residual GLA Battle Bus transport:
    /// C++ TransportContain Slots=8, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY.
    /// Fail-closed: not multi-door exit / SlowDeath undeath SECOND_LIFE.
    pub fn install_battle_bus_transport(&mut self) {
        self.is_battle_bus_transport = true;
        self.max_transport = crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        self.thing
            .template
            .contain_module
            .weapon_bonus_passed_to_passengers =
            crate::game_logic::host_battle_bus::BATTLE_BUS_WEAPON_BONUS_PASSED_TO_PASSENGERS;
        if self.battle_bus_body.is_none() {
            self.battle_bus_body =
                Some(crate::game_logic::host_battle_bus::HostBattleBusBodyData::new());
        }
        // First-life max health residual (UndeadBody / ActiveBody).
        if self.health.maximum < crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH {
            self.health.maximum = crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH;
            self.health.current = crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH;
        }
    }

    /// True when this vehicle is a Battle Bus residual transport.
    pub fn is_battle_bus_style_container(&self) -> bool {
        self.is_battle_bus_transport
    }

    /// C++ UndeadBody::startSecondLife + BattleBus first-death begin.
    pub(in crate::game_logic) fn start_battle_bus_second_life_at_frame(
        &mut self,
        frame: u32,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
    ) {
        self.start_battle_bus_second_life_with_body_fx(
            frame,
            source,
            &mut Object::standalone_body_fx,
        );
    }

    pub(in crate::game_logic) fn start_battle_bus_second_life_with_body_fx(
        &mut self,
        frame: u32,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        body_fx: &mut BodyStateFxReceiver<'_>,
    ) {
        self.start_battle_bus_second_life_with_body_fx_with_operation(
            frame,
            source,
            body_fx,
            0,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn start_battle_bus_second_life_with_body_fx_with_operation(
        &mut self,
        frame: u32,
        source: Option<&crate::game_logic::host_transition_damage_fx::HostDamageFxVictim>,
        body_fx: &mut BodyStateFxReceiver<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_battle_bus::{
            BATTLE_BUS_MC_BIT_SECOND_LIFE, BATTLE_BUS_SECOND_LIFE_MAX_HEALTH,
            BATTLE_BUS_THROW_FORCE, HostBattleBusBodyData, battle_bus_start_undeath_fx_name,
        };
        let body = self
            .battle_bus_body
            .get_or_insert_with(HostBattleBusBodyData::new);
        if body.is_second_life && !body.is_in_first_death {
            // Already converted.
            return;
        }
        body.begin_first_life_undeath(frame);
        self.health.maximum = BATTLE_BUS_SECOND_LIFE_MAX_HEALTH;
        self.health.current = BATTLE_BUS_SECOND_LIFE_MAX_HEALTH;
        self.armor_set_second_life = true;
        self.status.destroyed = false;
        self.status.effectively_dead = false;
        // setMaxHealth(FULLY_HEAL) evaluates the Body immediately. Do this
        // before ordinary AI/pose setters can observe the restored health.
        self.refresh_model_condition_bits_with_body_fx(source, frame, body_fx);
        // C++ applyShock throwForce.z (up). Host is Y-up, so +Y.
        // scrubVelocity2D then throw — do not stop_moving (that zeroes the hop).
        let _ = self.apply_shock_wave_impulse_with_operation(
            glam::Vec3::new(0.0, BATTLE_BUS_THROW_FORCE, 0.0),
            operation_frame,
            operation_effects,
        );
        self.apply_shock_random_rotation_with_operation(frame, operation_frame, operation_effects);
        self.movement.velocity.x = 0.0;
        self.movement.velocity.z = 0.0;
        self.movement.target_position = None;
        self.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::callback(&mut |object, request| {
                body_fx(object, BodyStateFx::MovementStop(request), None)
            }),
        );
        self.target = None;
        self.status.attacking = false;
        let _ = BATTLE_BUS_MC_BIT_SECOND_LIFE; // set on land
        // Leftover `execute_fx_at_object_id` / C++ `FXList::doFXObj(m_fxStartUndeath, me)`.
        let fx = battle_bus_start_undeath_fx_name(&self.template_name);
        body_fx(
            self,
            BodyStateFx::ObjectPhase {
                name: fx,
                at_object: true,
            },
            source,
        );
    }

    /// Tick BattleBusSlowDeath first-death air time + empty hulk arming.
    /// Returns (landed_this_tick, empty_hulk_kill).
    pub fn tick_battle_bus_slow_death(
        &mut self,
        current_frame: u32,
        _above_terrain_hint: bool,
        passenger_count: usize,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> (bool, bool) {
        self.tick_battle_bus_slow_death_with_fx(
            current_frame,
            _above_terrain_hint,
            passenger_count,
            &mut Self::standalone_object_fx,
            audio,
        )
    }

    pub(in crate::game_logic) fn tick_battle_bus_slow_death_with_fx(
        &mut self,
        current_frame: u32,
        _above_terrain_hint: bool,
        passenger_count: usize,
        dispatch: &mut dyn FnMut(&str, &Object, bool),
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> (bool, bool) {
        self.tick_battle_bus_slow_death_with_fx_with_operation(
            current_frame,
            _above_terrain_hint,
            passenger_count,
            dispatch,
            audio,
            0,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn tick_battle_bus_slow_death_with_fx_with_operation(
        &mut self,
        current_frame: u32,
        _above_terrain_hint: bool,
        passenger_count: usize,
        dispatch: &mut dyn FnMut(&str, &Object, bool),
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (bool, bool) {
        self.advance_battle_bus_throw_with_effects(operation_frame, operation_effects);
        self.finish_battle_bus_slow_death_with_fx(current_frame, passenger_count, dispatch, audio)
    }

    /// Transform phase precedes every landing callback in this original tick.
    pub(crate) fn advance_battle_bus_throw_with_effects(
        &mut self,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Integrate residual throw height (host world-Y up).
        let (in_first, throw_vz) = self
            .battle_bus_body
            .as_ref()
            .map(|b| (b.is_in_first_death, b.throw_vz))
            .unwrap_or((false, 0.0));
        if in_first && throw_vz.abs() > 0.001 {
            let pos = self.get_position();
            let ground = self.ground_height;
            let mut y = pos.y + throw_vz;
            let mut new_vz = throw_vz - 0.5; // residual gravity peel
            if new_vz < 0.0 && y <= ground {
                y = ground;
                new_vz = 0.0;
            }
            self.set_position_with_effects(
                glam::Vec3::new(pos.x, y.max(ground), pos.z),
                operation_frame,
                operation_effects,
            );
            if let Some(body) = self.battle_bus_body.as_mut() {
                body.throw_vz = new_vz;
            }
        }
    }

    pub(crate) fn finish_battle_bus_slow_death_with_fx(
        &mut self,
        current_frame: u32,
        passenger_count: usize,
        dispatch: &mut dyn FnMut(&str, &Object, bool),
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> (bool, bool) {
        use crate::game_logic::host_battle_bus::{
            BATTLE_BUS_MC_BIT_SECOND_LIFE, battle_bus_hit_ground_fx_name,
        };
        if self.battle_bus_body.is_none() {
            return (false, false);
        }
        let above = self.get_position().y - self.ground_height > 0.5;
        let landed = self
            .battle_bus_body
            .as_mut()
            .map(|b| b.try_land_first_death(current_frame, above))
            .unwrap_or(false);
        if landed {
            // C++ setModelConditionState(MODELCONDITION_SECOND_LIFE) + DISABLED_HELD.
            self.model_condition_bits |= 1u128 << BATTLE_BUS_MC_BIT_SECOND_LIFE;
            self.set_status_disabled_held(true);
            self.stop_moving(audio);
            if self.ai_state() != AIState::Idle {
                self.set_ai_state(AIState::Idle, audio);
            }
            self.refresh_model_condition_bits();
            // Leftover `finish_first_death` / C++ `FXList::doFXObj(m_fxHitGround, me)`.
            let fx = battle_bus_hit_ground_fx_name(&self.template_name);
            dispatch(&fx, self, true);
        }
        let empty_kill = self
            .battle_bus_body
            .as_mut()
            .map(|b| b.tick_empty_hulk(passenger_count, current_frame))
            .unwrap_or(false);
        (landed, empty_kill)
    }

    /// True when UndeadBody should intercept a lethal hit (first life only).
    /// `raw_amount` is C++ `DamageInfo.in.m_amount` (PRE-armor).
    pub fn battle_bus_should_intercept_lethal(
        &self,
        damage_type: crate::game_logic::combat::DamageType,
        raw_amount: f32,
    ) -> bool {
        if !self.is_battle_bus_transport {
            return false;
        }
        // C++ UndeadBody.cpp:58-62 — only DAMAGE_UNRESISTABLE and
        // !IsHealthDamagingDamage (Damage.h:110-127) skip second-life
        // intercept. DAMAGE_PENALTY is ordinary HP and must trigger it.
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Unresistable
        ) || !damage_type.is_health_damaging()
        {
            return false;
        }
        let second = self
            .battle_bus_body
            .as_ref()
            .map(|b| b.is_second_life)
            .unwrap_or(false);
        !second && raw_amount >= self.health.current && self.health.current > 0.0
    }
}
