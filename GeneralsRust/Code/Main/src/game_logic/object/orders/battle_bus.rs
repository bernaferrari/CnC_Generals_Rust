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
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a Battle Bus residual transport.
    pub fn is_battle_bus_style_container(&self) -> bool {
        self.is_battle_bus_transport
    }

    /// C++ UndeadBody::startSecondLife + BattleBus first-death begin residual.
    pub fn start_battle_bus_second_life(&mut self) {
        self.start_battle_bus_second_life_at_frame(
            crate::game_logic::host_historic_bonus::logic_frame(),
        );
    }

    pub(in crate::game_logic) fn start_battle_bus_second_life_at_frame(&mut self, frame: u32) {
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
        // C++ applyShock throwForce.z (up). Host is Y-up, so +Y.
        // scrubVelocity2D then throw — do not stop_moving (that zeroes the hop).
        let _ = self.apply_shock_wave_impulse(glam::Vec3::new(0.0, BATTLE_BUS_THROW_FORCE, 0.0));
        self.apply_shock_random_rotation(frame);
        self.movement.velocity.x = 0.0;
        self.movement.velocity.z = 0.0;
        self.movement.target_position = None;
        self.set_ai_state(AIState::Idle);
        self.target = None;
        self.status.attacking = false;
        let _ = BATTLE_BUS_MC_BIT_SECOND_LIFE; // set on land
        self.record_host_weapon_set();
        // Leftover `execute_fx_at_object_id` / C++ `FXList::doFXObj(m_fxStartUndeath, me)`.
        let fx = battle_bus_start_undeath_fx_name(&self.template_name);
        crate::game_logic::publish_host_fx_object(
            self.id.0,
            self.get_position(),
            self.get_orientation(),
            self.owner_player_id.map(|p| p as i32).unwrap_or(-1),
        );
        let _ = crate::game_logic::dispatch_fx_list_at_object(&fx, self.id.0, None);
    }

    /// Tick BattleBusSlowDeath first-death air time + empty hulk arming.
    /// Returns (landed_this_tick, empty_hulk_kill).
    pub fn tick_battle_bus_slow_death(
        &mut self,
        current_frame: u32,
        _above_terrain_hint: bool,
        passenger_count: usize,
    ) -> (bool, bool) {
        use crate::game_logic::host_battle_bus::{
            BATTLE_BUS_MC_BIT_SECOND_LIFE, battle_bus_hit_ground_fx_name,
        };
        if self.battle_bus_body.is_none() {
            return (false, false);
        }
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
            self.set_position(glam::Vec3::new(pos.x, y.max(ground), pos.z));
            if let Some(body) = self.battle_bus_body.as_mut() {
                body.throw_vz = new_vz;
            }
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
            self.stop_moving();
            if self.ai_state != AIState::Idle {
                self.set_ai_state(AIState::Idle);
            }
            self.refresh_model_condition_bits();
            // Leftover `finish_first_death` / C++ `FXList::doFXObj(m_fxHitGround, me)`.
            let fx = battle_bus_hit_ground_fx_name(&self.template_name);
            crate::game_logic::publish_host_fx_object(
                self.id.0,
                self.get_position(),
                self.get_orientation(),
                self.owner_player_id.map(|p| p as i32).unwrap_or(-1),
            );
            let _ = crate::game_logic::dispatch_fx_list_at_object(&fx, self.id.0, None);
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
