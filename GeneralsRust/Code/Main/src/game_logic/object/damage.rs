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
            // C++ ActiveBody::attemptHealing (ActiveBody.cpp:801):
            // amount = m_curArmor.adjustDamage(DAMAGE_HEALING, in.m_amount)
            // before internalChangeHealth. Armor.cpp:43-55 apply coefficient.
            let amount = crate::game_logic::host_armor_residual::apply_residual_armor(
                self,
                damage_type,
                damage,
            );
            self.heal(amount.max(0.0));
            // Optional: record healer without treating as hostile damage source.
            let _ = source;
            return false;
        }
        // DAMAGE_WATER: normal HP damage path (type distinguishes FX in C++).
        // C++ ActiveBody.cpp:365-418 DAMAGE_KILLPILOT: no hull HP.
        // RiderChangeContain (combat bike): moving bike is scored+killed;
        // stationary bike evacuates then the rider is killed so the bike
        // scuttles. Ordinary vehicles become unmanned + Neutral.
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::KillPilot
        ) {
            if self.is_kind_of(crate::game_logic::KindOf::Vehicle)
                || self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            {
                let rider_change = self.is_combat_cycle_transport
                    || self.thing.template.contain_module.kind
                        == crate::game_logic::ContainModuleKind::RiderChange;
                if rider_change {
                    if self.status.moving {
                        // C++: damager->scoreTheKill(obj); obj->kill();
                        self.health.current = 0.0;
                        self.status.destroyed = true;
                        self.status.death_type = death_type;
                        crate::game_logic::host_death_type_log::record(
                            self.id,
                            self.status.death_type.ordinal(),
                        );
                        self.set_ai_state(AIState::Idle);
                        self.target = None;
                        crate::game_logic::host_damage_log::record_typed(
                            self.id,
                            self.health.maximum.max(self.max_health).max(1.0),
                            source,
                            true,
                            damage_type.to_store() as u32,
                        );
                        let _ = damage;
                        return true;
                    }
                    // Stationary: evacuate the rider so the bike scuttles.
                    // Occupant kill is the GameLogic contain list (C++ rider->kill).
                    self.occupants.clear();
                    self.rider_change_scuttled_on_frame = self
                        .rider_change_scuttled_on_frame
                        .max(1);
                    let _ = (source, death_type, damage);
                    return false;
                }
                if self.is_car_bomb() {
                    // Detonation handled by combat caller; mark unmanned edge.
                }
                self.apply_kill_pilot_unmanned();
                self.set_team(crate::game_logic::Team::Neutral);
            }
            let _ = (source, death_type, damage);
            return false;
        }

        // C++ DAMAGE_MICROWAVE (Damage.h:63) is ordinary HP through armor.
        // IsSubdualDamage is false (Damage.h:95-107). Do not peel EMP/Microwave
        // into the subdual pool — TankArmor MICROWAVE 0% zeros HP, infantry
        // take armor-scaled HP. Host EMP is a leftover alias of Microwave.
        // C++ SUBDUAL_* is not HP and is not DAMAGE_UNRESISTABLE.
        if damage_type.is_subdual() {
            let typed = crate::game_logic::host_armor_residual::apply_residual_armor(
                self,
                damage_type,
                damage,
            );
            self.apply_subdual_damage(typed);
            let _ = (source, death_type);
            return false;
        }
        // C++ DAMAGE_STATUS residual: amount is duration msec, not hitpoints.
        if matches!(damage_type, crate::game_logic::combat::DamageType::Status) {
            // C++ ActiveBody.cpp:351 adjustDamage first; 460-464 convert the
            // ARMOR-ADJUSTED msec via ConvertDurationFromMsecsToFrames.
            let amount = crate::game_logic::host_armor_residual::apply_residual_armor(
                self,
                damage_type,
                damage,
            );
            let frames = ((amount.max(0.0) * 30.0) / 1000.0).ceil() as u32;
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
        let prev_health = self.health.current;
        let old_body_state = self.body_damage_state;
        let max_health = self.health.maximum.max(self.max_health).max(1.0);

        // C++ BaseRegenerateUpdate::onDamage residual (delay before auto-heal).
        if damage > 0.0 {
            if let Some(br) = self.base_regenerate.as_mut() {
                br.mark_damaged();
            }
            if let Some(ah) = self.default_auto_heal.as_mut() {
                ah.mark_damaged();
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
            self.last_damage_timestamp =
                Some(crate::game_logic::host_historic_bonus::logic_frame());
        }

        // C++ ActiveBody::attemptDamage: ArmorTemplate::adjustDamage, then
        // m_damageScalar only for non-UNRESISTABLE (ActiveBody.cpp:351, 490-497).
        // No invented armor/(armor+100) extra mitigation.
        let typed =
            crate::game_logic::host_armor_residual::apply_residual_armor(self, damage_type, damage);
        let battle_plan_armor = self.battle_plan_armor_damage_scalar();
        let mut actual_damage = if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Unresistable
        ) {
            typed
        } else {
            typed * battle_plan_armor
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

        // C++ ImmortalBody::internalChangeHealth (ImmortalBody.cpp:31-37):
        // delta = max(delta, -getHealth()+1) — never below 1, never dead.
        // SupplyWarehouse / FireWallSegment / TrailRemnant / GPS marker
        // use ImmortalBody. Distinct from Highlander (UNRESISTABLE still kills).
        if self.uses_immortal_body() && !battle_bus_start_second {
            let floor = 1.0;
            let max_dmg = (self.health.current - floor).max(0.0);
            if actual_damage > max_dmg {
                actual_damage = max_dmg;
            }
        }

        // C++ ActiveBody::internalChangeHealth is a single write
        // (ActiveBody.cpp:1188+). Always mutate host health.current this
        // frame so mid-frame death / HP visibility matches C++, and still
        // log for the GameWorld shadow channel.
        self.health.damage(actual_damage);
        let destroyed = if !self.health.is_alive() {
            if !self.status.destroyed {
                self.status.destroyed = true;
                self.status.death_type = death_type;
                crate::game_logic::host_death_type_log::record(
                    self.id,
                    self.status.death_type.ordinal(),
                );
                self.set_ai_state(AIState::Idle);
                self.target = None;
            }
            true
        } else {
            false
        };
        crate::game_logic::host_damage_log::record_typed(
            self.id,
            actual_damage,
            source,
            destroyed,
            damage_type.to_store() as u32,
        );
        // C++ ActiveBody.cpp:574-581 setAttackedBy + :653 doDamageFX.
        crate::game_logic::host_transition_damage_fx::queue_attacked_by(
            self.owner_player_id,
            source,
        );
        if let Some(src) = source {
            crate::game_logic::host_attacked_by_log::record(self.id, src);
        }
        let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
            self,
            damage_type,
            actual_damage,
        );



        // C++ UndeadBody::startSecondLife after ActiveBody::attemptDamage residual.
        if battle_bus_start_second {
            self.start_battle_bus_second_life();
        }

        // C++ PoisonedBehavior::onDamage residual.
        if actual_damage > 0.0 {
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            self.notify_poisoned_on_damage(frame, damage_type, actual_damage, death_type);
        }
        // C++ FlammableUpdate.cpp:78-100 onDamage FLAME / PARTICLE_BEAM → tryToIgnite.
        if actual_damage > 0.0
            && matches!(
                damage_type,
                crate::game_logic::combat::DamageType::Flame
                    | crate::game_logic::combat::DamageType::ParticleBeam
            )
        {
            if let Some(fs) = self.fire_spread.as_mut() {
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let _ = fs.apply_flame_damage(actual_damage, frame);
            }
        }
        // C++ FireWeaponWhenDamagedBehavior::onDamage residual (frame filled by GameLogic).
        if actual_damage > 0.0 {
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
                        damage_type.to_store() as u32,
                    ) {
                        self.pending_fire_when_damaged_weapon = Some(w);
                    }
                }
            }
        }

        self.refresh_model_condition_bits();
        crate::game_logic::host_transition_damage_fx::queue_voice_fear_event(
            &mut self.pending_transition_damage_fx,
            &self.template_name,
            old_body_state,
            self.body_damage_state,
            prev_health,
            self.health.current,
            max_health,
        );
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

    /// C++ ImmortalBody templates: SupplyWarehouse/Pile/Dock, FireWallSegment,
    /// ParticleUplink TrailRemnant, GPSScrambler invisible marker.
    /// ImmortalBody.cpp:31-37 never lets HP drop below 1 (unlike Highlander).
    pub fn uses_immortal_body(&self) -> bool {
        if self.firewall_segment {
            return true;
        }
        let n = self.template_name.to_ascii_lowercase();
        n.contains("supplywarehouse")
            || n.contains("supply_warehouse")
            || n.contains("supplypile")
            || n.contains("supply_pile")
            || n.contains("supplydock")
            || n.contains("supply_dock")
            || n.contains("firewallsegment")
            || n.contains("fire_wall_segment")
            || n.contains("trailremnant")
            || (n.contains("gpsscrambler") && n.contains("marker"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    fn vehicle(name: &str, id: u32, hp: f32) -> Object {
        let mut tmpl = ThingTemplate::new(name);
        tmpl.set_health(hp);
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut o = Object::new(tmpl, ObjectId(id), Team::USA);
        o.health.current = hp;
        o.health.maximum = hp;
        o
    }

    #[test]
    fn subdual_missile_is_not_hp_unresistable() {
        // C++ IsSubdualDamage (Damage.h:95-107) + ActiveBody.cpp:471-488.
        // Pre-fix: map_store collapsed SUBDUAL_MISSILE → Unresistable HP.
        let mut tank = vehicle("SubdualTank", 11, 200.0);
        assert!(!tank.take_damage_from_typed(80.0, None, DamageType::SubdualMissile));
        assert!(
            (tank.health.current - 200.0).abs() < 1e-3,
            "SUBDUAL_MISSILE must not deal HP, got {}",
            tank.health.current
        );
        // TankArmor SUBDUAL_MISSILE residual is 0% — still not Unresistable HP.
        assert!((tank.subdual_damage).abs() < 1e-3);

        let mut bare = Object::new(ThingTemplate::new("Bare"), ObjectId(12), Team::USA);
        bare.health.current = 100.0;
        bare.health.maximum = 100.0;
        assert!(!bare.take_damage_from_typed(40.0, None, DamageType::SubdualMissile));
        assert!((bare.health.current - 100.0).abs() < 1e-3);
        assert!(
            (bare.subdual_damage - 40.0).abs() < 1e-3,
            "default armor SUBDUAL_MISSILE should add subdual, got {}",
            bare.subdual_damage
        );
    }

    #[test]
    fn gattling_uses_tank_armor_ten_percent() {
        // C++ Armor.ini TankArmor GATTLING 10% via ArmorTemplate::adjustDamage.
        // Pre-fix: Gattling collapsed to Bullet (25%) then armor/(armor+100).
        let mut tank = vehicle("GattlingTank", 13, 1000.0);
        tank.thing.template.armor = 100.0;
        let hp0 = tank.health.current;
        tank.take_damage_from_typed(100.0, None, DamageType::Gattling);
        let dealt = hp0 - tank.health.current;
        assert!(
            (dealt - 10.0).abs() < 0.05,
            "expected TankArmor GATTLING 10 (no armor/(armor+100)), got {dealt}"
        );
    }

    #[test]
    fn take_damage_has_no_invented_scalar_armor_formula() {
        // C++ ActiveBody.cpp:351, 490-497: adjustDamage then m_damageScalar only.
        // A leftover scalar formula would halve this Explosion hit when armor=100.
        let mut tank = vehicle("ScalarArmorTank", 14, 1000.0);
        tank.thing.template.armor = 100.0;
        let hp0 = tank.health.current;
        tank.take_damage_from_typed(100.0, None, DamageType::Explosive);
        let dealt = hp0 - tank.health.current;
        assert!(
            (dealt - 100.0).abs() < 0.05,
            "TankArmor default Explosion is 100%; leftover scalar formula would deal 50, got {dealt}"
        );
    }

    #[test]
    fn kill_pilot_splits_rider_change_bike() {
        // C++ ActiveBody.cpp:365-418 DAMAGE_KILLPILOT RiderChangeContain split.
        let mut moving = vehicle("CombatBike", 21, 150.0);
        moving.is_combat_cycle_transport = true;
        moving.thing.template.contain_module.kind =
            crate::game_logic::ContainModuleKind::RiderChange;
        moving.set_status_moving(true);
        moving.occupants.push(ObjectId(99));
        assert!(moving.take_damage_from_typed(1.0, None, DamageType::KillPilot));
        assert!(moving.status.destroyed);
        assert!(moving.health.current <= 0.0);
        assert!(!moving.is_unmanned());

        let mut parked = vehicle("CombatBike", 22, 150.0);
        parked.is_combat_cycle_transport = true;
        parked.thing.template.contain_module.kind =
            crate::game_logic::ContainModuleKind::RiderChange;
        parked.set_status_moving(false);
        parked.occupants.push(ObjectId(98));
        assert!(!parked.take_damage_from_typed(1.0, None, DamageType::KillPilot));
        assert!((parked.health.current - 150.0).abs() < 1e-3);
        assert!(!parked.is_unmanned());
        assert!(parked.occupants.is_empty());
        assert!(parked.rider_change_scuttled_on_frame > 0);

        let mut tank = vehicle("Tank", 23, 200.0);
        assert!(!tank.take_damage_from_typed(1.0, None, DamageType::KillPilot));
        assert!((tank.health.current - 200.0).abs() < 1e-3);
        assert!(tank.is_unmanned());
        assert_eq!(tank.team, Team::Neutral);
    }

    #[test]
    fn take_damage_applies_host_hp_same_frame() {
        // C++ ActiveBody::internalChangeHealth (ActiveBody.cpp:1188+).
        // Pre-fix: gameworld_damage_authority_live left health.current stale.
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
        crate::gameworld_shadow::begin_shadow_coupled_tick();
        let mut tank = vehicle("SameFrameHp", 24, 100.0);
        assert!(!tank.take_damage_from_typed(25.0, None, DamageType::Unresistable));
        assert!(
            (tank.health.current - 75.0).abs() < 1e-3,
            "host HP must update this frame under damage authority, got {}",
            tank.health.current
        );
        assert!(tank.take_damage_from_typed(80.0, None, DamageType::Unresistable));
        assert!(tank.health.current <= 0.0);
        assert!(tank.status.destroyed);
        crate::gameworld_shadow::end_shadow_coupled_tick();
    }

    #[test]
    fn supply_warehouse_and_firewall_use_immortal_body_floor() {
        // C++ ImmortalBody.cpp:31-37 + CivilianBuilding.ini SupplyWarehouse
        // / FireWallSegment ImmortalBody. Live host take_damage is attemptDamage.
        let mut warehouse = Object::new(
            ThingTemplate::new("SupplyWarehouse"),
            ObjectId(31),
            Team::Neutral,
        );
        warehouse.health.current = 1000.0;
        warehouse.health.maximum = 1000.0;
        assert!(warehouse.uses_immortal_body());
        assert!(!warehouse.take_damage_from_typed(50_000.0, None, DamageType::Explosive));
        assert!(
            (warehouse.health.current - 1.0).abs() < 1e-3,
            "SupplyWarehouse must stay at 1 HP, got {}",
            warehouse.health.current
        );
        assert!(!warehouse.status.destroyed);
        assert!(!warehouse.take_damage_from_typed(99.0, None, DamageType::Unresistable));
        assert!((warehouse.health.current - 1.0).abs() < 1e-3);
        assert!(!warehouse.status.destroyed);

        let mut segment = Object::new(
            ThingTemplate::new("FireWallSegment"),
            ObjectId(32),
            Team::China,
        );
        segment.health.current = 50.0;
        segment.health.maximum = 50.0;
        segment.firewall_segment = true;
        assert!(segment.uses_immortal_body());
        assert!(!segment.take_damage_from_typed(500.0, None, DamageType::Explosive));
        assert!((segment.health.current - 1.0).abs() < 1e-3);
        assert!(!segment.status.destroyed);

        let mut tank = vehicle("AmericaTankCrusader", 33, 400.0);
        assert!(!tank.uses_immortal_body());
        assert!(tank.take_damage_from_typed(400.0, None, DamageType::Unresistable));
        assert!(tank.status.destroyed);
    }

    fn register_coeff_armor(name: &str, dt: gamelogic::damage::DamageType, coeff: f32) {
        let mut t = gamelogic::object::armor::ArmorTemplate::new();
        t.set_default(1.0);
        t.set_coefficient(dt, coeff);
        gamelogic::object::armor::TheArmorStore::register_template(
            &gamelogic::common::AsciiString::from(name),
            t,
        );
    }

    #[test]
    fn healing_applies_armor_coefficient() {
        // C++ ActiveBody::attemptHealing (ActiveBody.cpp:801) + Armor.cpp:43-55.
        register_coeff_armor("HealHalfArmor", gamelogic::damage::DamageType::Healing, 0.5);
        let mut tmpl = ThingTemplate::new("HealHalf");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.armor_sets.push(crate::game_logic::HostArmorSet {
            conditions: 0,
            armor: Some("HealHalfArmor".into()),
            damage_fx: None,
        });
        let mut unit = Object::new(tmpl, ObjectId(41), Team::USA);
        unit.health.current = 40.0;
        unit.health.maximum = 100.0;
        assert!(!unit.take_damage_from_typed(20.0, None, DamageType::Healing));
        assert!(
            (unit.health.current - 50.0).abs() < 1e-3,
            "HEALING must use armor coeff 0.5 (heal 10), got {}",
            unit.health.current
        );
    }

    #[test]
    fn microwave_is_hp_through_armor_not_subdual() {
        // C++ Damage.h:63 DAMAGE_MICROWAVE; IsSubdualDamage false (Damage.h:95-107).
        // TankArmor MICROWAVE 0%; HumanArmor default 100%.
        let mut tank = vehicle("MicrowaveTank", 42, 100.0);
        assert!(!tank.take_damage_from_typed(40.0, None, DamageType::Microwave));
        assert!(
            (tank.health.current - 100.0).abs() < 1e-3,
            "TankArmor MICROWAVE 0% must deal 0 HP, got {}",
            tank.health.current
        );
        assert!(
            tank.subdual_damage.abs() < 1e-3,
            "MICROWAVE must not enter EMP subdual pool, got {}",
            tank.subdual_damage
        );

        let mut inf = ThingTemplate::new("Ranger");
        inf.set_health(100.0);
        inf.add_kind_of(KindOf::Infantry);
        let mut ranger = Object::new(inf, ObjectId(43), Team::USA);
        ranger.health.current = 100.0;
        ranger.health.maximum = 100.0;
        assert!(!ranger.take_damage_from_typed(40.0, None, DamageType::Microwave));
        assert!(
            (ranger.health.current - 60.0).abs() < 1e-3,
            "infantry MICROWAVE is armor-scaled HP, got {}",
            ranger.health.current
        );
        assert!(ranger.subdual_damage.abs() < 1e-3);

        // Leftover host EMP alias is the same store type (Microwave).
        let mut tank2 = vehicle("EmpAliasTank", 44, 100.0);
        assert!(!tank2.take_damage_from_typed(40.0, None, DamageType::EMP));
        assert!((tank2.health.current - 100.0).abs() < 1e-3);
        assert!(tank2.subdual_damage.abs() < 1e-3);
    }

    #[test]
    fn status_duration_applies_armor_coefficient() {
        // C++ ActiveBody.cpp:351 adjustDamage; 460-464 ConvertDurationFromMsecsToFrames
        // on the armor-adjusted msec.
        register_coeff_armor("StatusHalfArmor", gamelogic::damage::DamageType::Status, 0.5);
        let mut tmpl = ThingTemplate::new("StatusHalf");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.armor_sets.push(crate::game_logic::HostArmorSet {
            conditions: 0,
            armor: Some("StatusHalfArmor".into()),
            damage_fx: None,
        });
        let mut o = Object::new(tmpl, ObjectId(45), Team::GLA);
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        let frame = crate::game_logic::host_historic_bonus::logic_frame();
        // 2000 msec * 0.5 armor = 1000 msec → 30 frames @ 30 FPS.
        assert!(!o.take_damage_from_typed(2000.0, None, DamageType::Status));
        assert!((o.health.current - 100.0).abs() < 1e-3);
        assert!(o.is_faerie_fire());
        assert_eq!(o.faerie_fire_until_frame, frame.saturating_add(30));
    }

    #[test]
    fn take_damage_dispatches_armor_damage_fx() {
        // C++ ActiveBody.cpp:653 doDamageFX after attemptDamage.
        use crate::game_logic::host_transition_damage_fx::{
            take_dispatched_armor_damage_fx, TemplateDamageAudio,
        };
        game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
        let mut dfx = game_engine::common::ini::ini_damage_fx::DamageFX::new();
        dfx.set_major_minor_fx(
            game_engine::common::ini::ini_damage_fx::DamageType::Unresistable,
            Some("FX_TestHitSpark".into()),
            None,
            0.0,
        );
        if let Some(mut store) = game_engine::common::ini::ini_damage_fx::get_damage_fx_store_mut() {
            store.add_damage_fx("TankDamageFX".into(), dfx);
        }
        let _ = take_dispatched_armor_damage_fx();
        let mut tank = vehicle("FxTank", 61, 200.0);
        tank.thing.template.armor_sets.push(crate::game_logic::HostArmorSet {
            conditions: 0,
            armor: Some("TankArmor".into()),
            damage_fx: Some("TankDamageFX".into()),
        });
        assert!(!tank.take_damage_from_typed(20.0, None, DamageType::Unresistable));
        let dispatched = take_dispatched_armor_damage_fx();
        assert!(
            dispatched.iter().any(|n| n == "TankDamageFX" || n == "FX_TestHitSpark"),
            "armor DamageFX must dispatch, got {dispatched:?}"
        );
        let _ = TemplateDamageAudio::default();
    }

    #[test]
    fn take_damage_queues_template_voice_fear_and_attacked_by() {
        // C++ ActiveBody.cpp:574 setAttackedBy, :624-637 VoiceFear.
        use crate::game_logic::host_transition_damage_fx::{
            set_test_template_audio, set_test_voice_fear_roll, take_pending_attacked_by,
            TemplateDamageAudio,
        };
        crate::game_logic::host_transition_damage_fx::clear_test_template_audio();
        set_test_template_audio(
            "FearRanger",
            TemplateDamageAudio {
                sound_on_damaged: Some("RangerSoundOnDamaged".into()),
                sound_on_really_damaged: Some("RangerSoundOnReallyDamaged".into()),
                voice_fear: Some("RangerVoiceFear".into()),
            },
        );
        set_test_voice_fear_roll(Some(0));
        let _ = take_pending_attacked_by();
        let mut tmpl = ThingTemplate::new("FearRanger");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        let mut ranger = Object::new(tmpl, ObjectId(62), Team::USA);
        ranger.health.current = 40.0;
        ranger.health.maximum = 100.0;
        ranger.owner_player_id = Some(1);
        ranger.template_name = "FearRanger".into();
        assert!(!ranger.take_damage_from_typed(20.0, Some(ObjectId(77)), DamageType::Unresistable));
        assert!((ranger.health.current - 20.0).abs() < 1e-3);
        let pending = ranger.take_pending_transition_damage_fx();
        assert!(
            pending
                .iter()
                .any(|e| e.audio_name.as_deref() == Some("RangerVoiceFear")),
            "VoiceFear must queue on yellow cross, got {pending:?}"
        );
        let attacked = take_pending_attacked_by();
        assert_eq!(attacked, vec![(1, ObjectId(77))]);
        set_test_voice_fear_roll(None);
        crate::game_logic::host_transition_damage_fx::clear_test_template_audio();
    }

    #[test]
    fn apply_set_attacked_by_marks_crate_player() {
        // C++ Player::setAttackedBy (Player.cpp:3173) for PLAYER_ATTACKED_BY.
        use crate::game_logic::host_transition_damage_fx::{
            apply_victim_attacked_by, take_attacked_by_log,
        };
        let _ = take_attacked_by_log();
        let victim = std::sync::Arc::new(std::sync::RwLock::new(gamelogic::player::Player::new(3)));
        let attacker = std::sync::Arc::new(std::sync::RwLock::new(gamelogic::player::Player::new(4)));
        if let Ok(mut list) = gamelogic::player::ThePlayerList().write() {
            list.add_player(victim.clone());
            list.add_player(attacker);
        }
        apply_victim_attacked_by(3, 4);
        assert!(
            victim.read().unwrap().get_attacked_by(4),
            "victim player must record attacker index"
        );
        let log = take_attacked_by_log();
        assert!(log.contains(&(3, 4)));
    }


}
