use super::*;

impl Object {
    pub fn take_damage_from(&mut self, damage: f32, source: Option<ObjectId>) -> bool {
        self.take_damage_from_typed(
            damage,
            source,
            crate::game_logic::combat::DamageType::Unresistable,
        )
    }

    /// Superweapon / strike residual: always mutate host HP (and still log for shadow).
    /// Combat fire under DAMAGE_AUTHORITY defers HP to GameWorld writeback; strikes
    /// call this path so host-only update_special_power_strikes still applies damage.
    pub fn take_damage_from_immediate(&mut self, damage: f32, source: Option<ObjectId>) -> bool {
        self.take_damage_from_typed_death_with_host_hp(
            damage,
            source,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                crate::game_logic::combat::DamageType::Unresistable,
            ),
            true, // force host HP apply
        )
    }

    /// Apply damage with host combat DamageType for Armor.ini residual coefficients.
    pub fn take_damage_from_typed(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
    ) -> bool {
        self.take_damage_from_typed_death(
            damage,
            source,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
        )
    }

    /// Apply damage with Armor.ini type residual and Weapon.ini DeathType on kill.
    pub fn take_damage_from_typed_death(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> bool {
        // C++ DAMAGE_DISARM residual: destroy mine without detonation splash.
        if matches!(damage_type, crate::game_logic::combat::DamageType::Disarm) {
            let _ = (source, death_type, damage);
            return self.disarm_mine_safe();
        }
        // C++ DAMAGE_DEPLOY residual: no HP on victim.
        // AssaultTransportAI::beginAssault is source-side (GameLogic combat path).
        if matches!(damage_type, crate::game_logic::combat::DamageType::Deploy) {
            let _ = (source, death_type, damage);
            return false;
        }
        // C++ DAMAGE_HACK residual: fire does not deal HP (effect is timer-driven).
        if matches!(damage_type, crate::game_logic::combat::DamageType::Hack) {
            let _ = (source, death_type, damage);
            return false;
        }
        // C++ DAMAGE_KILL_GARRISONED residual: structure HP untouched; occupants
        // cleared by GameLogic using pending kill count = floor(amount).
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::KillGarrisoned
        ) {
            let _ = (source, death_type);
            let kills = damage.max(0.0).floor() as u32;
            self.status.pending_kill_garrisoned =
                self.status.pending_kill_garrisoned.saturating_add(kills);
            return false;
        }
        // C++ DAMAGE_SURRENDER residual: lethal hit on surrender-capable infantry
        // sets surrendered instead of destroying (ActiveBody commented path residual).
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Surrender
        ) {
            let _ = death_type;
            if self.can_surrender_from_damage() {
                let would_kill = damage >= self.health.current && self.health.current > 0.0;
                if would_kill {
                    self.set_surrendered(true);
                    self.status.attacking = false;
                    self.target = None;
                    return false;
                }
            }
            // Non-lethal or non-capable: fall through to normal HP.
        }
        // DAMAGE_PENALTY: normal HP path (no special intercept).
        // C++ DAMAGE_HEALING residual: restore HP via attemptHealing; never destroys.
        // Does not stamp last_damage_source (C++ AIGuardRetaliate / stealth skip).
        if matches!(damage_type, crate::game_logic::combat::DamageType::Healing) {
            let _ = death_type;
            if self.status.destroyed || !self.is_alive() {
                return false;
            }
            // C++ PoisonedBehavior::onHealing residual (heal path).
            self.clear_poisoned_on_healing();
            // amount is heal strength; negative ignored by heal().
            self.heal(damage.max(0.0));
            // Optional: record healer without treating as hostile damage source.
            let _ = source;
            return false;
        }
        // DAMAGE_WATER: normal HP damage path (type distinguishes FX in C++).
        // C++ DAMAGE_KILL_PILOT residual: unmanned vehicle, no HP damage.
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::KillPilot
        ) {
            if self.is_kind_of(crate::game_logic::KindOf::Vehicle)
                || self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            {
                // C++ car-bomb dead-man residual when sniped.
                if self.is_car_bomb() {
                    // Detonation handled by combat caller; mark unmanned edge.
                }
                self.apply_kill_pilot_unmanned();
                self.set_team(crate::game_logic::Team::Neutral);
            }
            let _ = (source, death_type, damage);
            return false;
        }
        // C++ IsSubdualDamage residual (Microwave/EMP maps to host EMP class).
        if matches!(damage_type, crate::game_logic::combat::DamageType::EMP) {
            self.apply_subdual_damage(damage.max(0.0));
            let _ = (source, death_type);
            return false;
        }
        // C++ DAMAGE_STATUS residual: amount is duration msec, not hitpoints.
        if matches!(damage_type, crate::game_logic::combat::DamageType::Status) {
            let frames = ((damage.max(0.0) * 30.0) / 1000.0).ceil() as u32;
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            // Default status peel when caller didn't already apply a named status.
            // FAERIE_FIRE is the primary retail STATUS residual.
            if frames > 0 {
                self.do_status_damage("FAERIE_FIRE", frames.max(1), frame);
            }
            let _ = (source, death_type);
            return false;
        }

        self.take_damage_from_typed_death_with_host_hp(
            damage,
            source,
            damage_type,
            death_type,
            false,
        )
    }

    fn take_damage_from_typed_death_with_host_hp(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        force_host_hp: bool,
    ) -> bool {
        if self.status.destroyed {
            return false;
        }
        // OCL InvulnerableTime residual (post-eject pilot shield).
        if self.status.eject_invulnerable {
            return false;
        }

        // C++ BaseRegenerateUpdate::onDamage residual (delay before auto-heal).
        if damage > 0.0 {
            if let Some(br) = self.base_regenerate.as_mut() {
                br.mark_damaged();
            }
            // C++ ProneUpdate::goProne residual.
            if let Some(pu) = self.prone_update.as_mut() {
                let _ = pu.go_prone_damage(damage);
            }
        }

        // C++ StealthForbiddenConditions TAKING_DAMAGE residual (CamoNetting structures).
        if self.stealth_breaks_on_damage && self.status.stealthed {
            self.break_stealth();
        }

        // BodyModule last damage source residual (Passive WaitForAttack).
        if let Some(src) = source {
            self.last_damage_source = Some(src);
        }

        // Armor.ini residual coefficient (by object kind + damage type), then
        // legacy scalar armor + HoldTheLine plan residual.
        // C++ DAMAGE_UNRESISTABLE bypasses ArmorTemplate + scalar armor residual.
        let typed =
            crate::game_logic::host_armor_residual::apply_residual_armor(self, damage_type, damage);
        // C++ DAMAGE_UNRESISTABLE bypasses ArmorTemplate/scalar armor, but Strategy Center
        // HoldTheLinePlanArmorDamageScalar still multiplies body damage (LESS is better).
        let battle_plan_armor = self.battle_plan_armor_damage_scalar();
        let mut actual_damage = if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Unresistable
        ) {
            typed * battle_plan_armor
        } else {
            let armor_factor =
                1.0 - (self.thing.template.armor / (self.thing.template.armor + 100.0));
            typed * armor_factor * battle_plan_armor
        };

        // C++ ActiveBody: damaged CAN_BE_REPULSED civilians scare others when EnableRepulsors.
        // Object::setStatus(REPULSOR) + ObjectRepulsorHelper sleepUntil(+2 sec).
        if crate::game_logic::host_repulsor_gate::is_enabled()
            && actual_damage > 0.0
            && self.is_kind_of(KindOf::CanBeRepulsed)
        {
            self.set_status_repulsor(true);
            // 2 * LOGICFRAMES_PER_SECOND residual; frame base applied by host tick if 0.
            // Store absolute if known; else relative sentinel cleared by tick with current frame.
            if self.repulsor_until_frame == 0 || self.repulsor_until_frame < 100_000 {
                // relative duration residual; tick converts with current_frame
                self.repulsor_until_frame = 60; // 2 seconds @ 30Hz
            }
        }

        // C++ UndeadBody::attemptDamage residual (Battle Bus first life).
        // Clamp lethal non-UNRESISTABLE damage to leave 1 HP, then startSecondLife.
        let mut battle_bus_start_second = false;
        if self.battle_bus_should_intercept_lethal(damage_type, actual_damage) {
            actual_damage = (self.health.current - 1.0).max(0.0);
            battle_bus_start_second = true;
        }

        // C++ HighlanderBody::attemptDamage residual.
        let mut _highlander_clamped = false;
        if self.highlander_body && !battle_bus_start_second {
            // C++ HighlanderBody.cpp compares exactly against
            // DAMAGE_UNRESISTABLE.  DAMAGE_PENALTY is still ordinary damage
            // here, so it must leave the one-HP Highlander floor intact.
            let unres = matches!(
                damage_type,
                crate::game_logic::combat::DamageType::Unresistable
            );
            let (clamped, did) = crate::game_logic::host_highlander_body::highlander_clamp_damage(
                self.health.current,
                actual_damage,
                unres,
            );
            if did {
                actual_damage = clamped;
                _highlander_clamped = true;
            }
        }

        // GameWorld damage authority: host logs intent only; HP/destroyed last-write
        // via shadow session mutations + writeback_health_to_host (no mid-frame host HP mutate).
        // Defer only when a live shadow session can consume the log. Otherwise host-only
        // combat would record damage and never apply HP (authority without writeback).
        // force_host_hp: superweapon/residual paths always mutate host immediately.
        let damage_auth =
            crate::gameworld_shadow::gameworld_damage_authority_live() && !force_host_hp;
        let destroyed = if damage_auth {
            let projected = (self.health.current - actual_damage).max(0.0);
            let will_die = projected <= 0.0 || actual_damage >= self.health.current;
            crate::game_logic::host_damage_log::record(self.id, actual_damage, source, will_die);
            // Projected lethal: mark destroyed so is_alive() fails mid-frame without
            // mutating HP (shadow remains last-writer for the numeric health value).
            // Prevents multi-attacker overkill / retarget of a corpse before writeback.
            if will_die && !self.status.destroyed {
                self.status.destroyed = true;
                self.status.death_type = death_type;
                crate::game_logic::host_death_type_log::record(
                    self.id,
                    self.status.death_type.ordinal(),
                );
                self.set_ai_state(AIState::Idle);
                self.target = None;
            }
            will_die
        } else {
            self.health.damage(actual_damage);
            let destroyed = if !self.health.is_alive() {
                self.status.destroyed = true;
                self.status.death_type = death_type;
                crate::game_logic::host_death_type_log::record(
                    self.id,
                    self.status.death_type.ordinal(),
                );
                self.set_ai_state(AIState::Idle);
                self.target = None;
                true
            } else {
                false
            };
            crate::game_logic::host_damage_log::record(self.id, actual_damage, source, destroyed);
            destroyed
        };

        // C++ UndeadBody::startSecondLife after ActiveBody::attemptDamage residual.
        if battle_bus_start_second {
            self.start_battle_bus_second_life();
        }

        // C++ PoisonedBehavior::onDamage residual.
        if actual_damage > 0.0 {
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            self.notify_poisoned_on_damage(frame, damage_type, actual_damage, death_type);
        }
        // C++ FireWeaponWhenDamagedBehavior::onDamage residual (frame filled by GameLogic).
        if actual_damage > 0.0
            && !matches!(
                damage_type,
                crate::game_logic::combat::DamageType::Healing
                    | crate::game_logic::combat::DamageType::Status
                    | crate::game_logic::combat::DamageType::Hack
                    | crate::game_logic::combat::DamageType::Deploy
                    | crate::game_logic::combat::DamageType::Disarm
                    | crate::game_logic::combat::DamageType::KillPilot
                    | crate::game_logic::combat::DamageType::KillGarrisoned
            )
        {
            // Wave 779: under damage authority, FWWDB onDamage reaction is owned by
            // GW apply_host_damage_events + host_fwwd_reaction_log drain (post-HP).
            if !(crate::gameworld_shadow::gameworld_damage_authority_live() && !force_host_hp) {
                self.ensure_fire_weapon_when_damaged();
                if let Some(fw) = self.fire_weapon_when_damaged.as_mut() {
                    // Frame 0: debounce via serial on data; GameLogic may also call with real frame.
                    if let Some(w) = fw.on_damage(
                        actual_damage,
                        self.health.current,
                        self.health.maximum.max(self.max_health).max(1.0),
                        fw.last_reaction_frame.saturating_add(2),
                    ) {
                        self.pending_fire_when_damaged_weapon = Some(w);
                    }
                }
            }
        }

        self.refresh_model_condition_bits();
        if battle_bus_start_second {
            false
        } else {
            destroyed
        }
    }

    /// C++ AttitudeType residual (Sleep/Passive/Normal/Alert/Aggressive).
    pub fn ai_attitude(&self) -> crate::game_logic::host_strategy_center::HostAiAttitude {
        crate::game_logic::host_strategy_center::HostAiAttitude::from_i8(self.ai_attitude)
    }

    /// Set C++ AttitudeType residual for TurretAI mood matrix.
    pub fn set_ai_attitude(
        &mut self,
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) {
        self.ai_attitude = attitude.as_i8();
        crate::game_logic::host_ai_attitude_log::record(self.id, self.ai_attitude);
    }
}
