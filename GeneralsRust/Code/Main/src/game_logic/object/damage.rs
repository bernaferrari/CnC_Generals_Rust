use super::*;

thread_local! {
    static HIVE_SHOOTER_XZ: std::cell::Cell<Option<(f32, f32)>> =
        const { std::cell::Cell::new(None) };
    static PENDING_DAMAGE_STATUS: std::cell::Cell<Option<&'static str>> =
        const { std::cell::Cell::new(None) };
}

/// C++ `getClosestSlave(shooter->pos)` context for live `Object::take_damage`.
pub fn set_hive_shooter_xz(xz: Option<(f32, f32)>) {
    HIVE_SHOOTER_XZ.with(|c| c.set(xz));
}

fn take_hive_shooter_xz() -> Option<(f32, f32)> {
    HIVE_SHOOTER_XZ.with(|c| c.replace(None))
}

/// C++ `DamageInfo.in.m_damageStatusType` for the current `take_damage` apply.
/// `None` / `"NONE"` → no status paint (Weapon.ini default OBJECT_STATUS_NONE).
pub fn set_pending_damage_status_type(name: Option<&'static str>) {
    PENDING_DAMAGE_STATUS.with(|c| c.set(name));
}

fn peek_pending_damage_status_type() -> Option<&'static str> {
    PENDING_DAMAGE_STATUS.with(|c| c.get())
}

fn clear_pending_damage_status_type() {
    PENDING_DAMAGE_STATUS.with(|c| c.set(None));
}

/// Prime live combat fire: attacker DamageFX vet + authored DamageStatusType.
pub fn prime_live_damage_context(
    source: Option<&Object>,
    weapon_name: Option<&str>,
    damage_type: crate::game_logic::combat::DamageType,
) {
    if let Some(src) = source {
        crate::game_logic::host_transition_damage_fx::set_damage_fx_source(Some(
            crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source(src),
        ));
    } else {
        crate::game_logic::host_transition_damage_fx::set_damage_fx_source(None);
    }
    if matches!(damage_type, crate::game_logic::combat::DamageType::Status) {
        set_pending_damage_status_type(weapon_name.and_then(
            crate::game_logic::weapon_bootstrap::host_damage_status_type_for_weapon_name,
        ));
    } else {
        set_pending_damage_status_type(None);
    }
}

struct PendingDamageContextGuard;

impl Drop for PendingDamageContextGuard {
    fn drop(&mut self) {
        clear_pending_damage_status_type();
        crate::game_logic::host_transition_damage_fx::clear_damage_fx_source();
    }
}

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
            None,
        )
    }

    /// Host-only field / strike tick with Armor.ini type (force host HP).
    pub fn take_damage_from_immediate_typed(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
    ) -> bool {
        self.take_damage_from_immediate_typed_death(
            damage,
            source,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
        )
    }

    /// C++ Nuke/Medium/SmallRadiationFieldWeapon tick: DAMAGE_RADIATION + NOT_AIRBORNE.
    /// Weapon.cpp:1351 skips `isSignificantlyAboveTerrain`; Armor.ini then applies
    /// (structures 0%, tanks 50%, aircraft 25%). DeathType is NORMAL, not Detonated.
    pub fn take_radiation_field_tick(&mut self, damage: f32, source: Option<ObjectId>) -> bool {
        if self.status.airborne_target || self.is_significantly_above_terrain() {
            return false;
        }
        self.take_damage_from_immediate_typed_death(
            damage,
            source,
            crate::game_logic::combat::DamageType::Radiation,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        )
    }

    /// Host-only field tick: armor-typed HP + Weapon.ini DeathType.
    /// C++ FireWeaponUpdate poison puddles use DAMAGE_POISON, not UNRESISTABLE.
    pub fn take_damage_from_immediate_typed_death(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> bool {
        self.take_damage_from_typed_death_with_host_hp(
            damage,
            source,
            damage_type,
            death_type,
            true,
            None,
        )
    }

    /// Residual unit splash: leftover Weapon.ini DamageType/DeathType through Armor.ini.
    /// C++ ActiveBody::attemptDamage — not DAMAGE_UNRESISTABLE.
    pub fn take_damage_from_immediate_residual(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type_name: &str,
        death_type_name: &str,
    ) -> bool {
        let damage_type =
            crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                damage_type_name,
            );
        let death_type = crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
            death_type_name,
        );
        self.take_damage_from_immediate_typed_death(damage, source, damage_type, death_type)
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
        self.take_damage_from_typed_death_fx(damage, source, damage_type, death_type, None)
    }

    /// Typed death with C++ `DamageInfo.m_damageFXOverride` residual.
    /// PoisonedBehavior DoT retakes UNRESISTABLE but plays DAMAGE_POISON FX.
    pub fn take_damage_from_typed_death_fx(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
    ) -> bool {
        let _ctx = PendingDamageContextGuard;
        // C++ InactiveBody::attemptDamage (InactiveBody.cpp:53-86): no HP except
        // DAMAGE_UNRESISTABLE (onDie once, never DamageFX).
        if self.is_inactive_body() {
            return self.apply_inactive_body_damage(damage_type);
        }
        // C++ HiveStructureBody::attemptDamage (HiveStructureBody.cpp:45-112):
        // propagate SMALL_ARMS/SNIPER/POISON/RADIATION/SURRENDER/MICROWAVE to
        // closest slave; swallow SNIPER/POISON/SURRENDER when none remain.
        if self.try_hive_structure_body_damage(damage, source, damage_type) {
            return false;
        }
        // C++ ActiveBody::attemptDamage (ActiveBody.cpp:329-330) bails before
        // type switch / armor / HP when m_indestructible.
        if self.indestructible {
            return false;
        }

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
            let kills = damage.max(0.0).floor() as u32;
            self.status.pending_kill_garrisoned =
                self.status.pending_kill_garrisoned.saturating_add(kills);
            if damage > 0.0 {
                self.stamp_last_damage_cpp(source, false, damage_type);
            }
            let fx_type = fx_override.unwrap_or(damage_type);
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                self,
                fx_type,
                damage.max(0.0),
            );
            let _ = (source, death_type);
            return false;
        }
        // DAMAGE_PENALTY: normal HP path (no special intercept).
        // C++ DAMAGE_HEALING residual: restore HP via attemptHealing; never destroys.
        // C++ ActiveBody.cpp:817-820 stamps lastDamageInfo + lastHealingTimestamp
        // (overwrites prior attacker). doDamageFX always runs (848).
        if matches!(damage_type, crate::game_logic::combat::DamageType::Healing) {
            let _ = death_type;
            let is_bridge = self.is_host_bridge_member();
            if self.status.destroyed && !self.status.keep_as_rubble && !is_bridge {
                return false;
            }
            if !self.is_alive() && !is_bridge {
                return false;
            }
            if is_bridge {
                self.revive_from_bridge_rubble();
            }
            self.clear_poisoned_on_healing();
            let amount = crate::game_logic::host_armor_residual::apply_residual_armor(
                self,
                damage_type,
                damage,
            );
            self.heal(amount.max(0.0));
            if amount > 0.0 {
                let now = crate::game_logic::host_historic_bonus::logic_frame();
                self.last_healing_timestamp = Some(now);
                self.last_damage_info_type = Some(crate::game_logic::combat::DamageType::Healing);
                if is_bridge {
                    let max_health = self.health.maximum.max(self.max_health).max(1.0);
                    crate::game_logic::host_bridge_behavior::record_mirror(
                        self.id,
                        amount.max(0.0),
                        max_health,
                        source,
                        damage_type.to_store() as u32,
                        death_type.ordinal() as u32,
                        crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind::Heal,
                    );
                }
            }
            let fx_type = fx_override.unwrap_or(damage_type);
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                self,
                fx_type,
                amount.max(0.0),
            );
            return false;
        }
        // DAMAGE_WATER: normal HP damage path (type distinguishes FX in C++).
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::KillPilot
        ) {
            let fx_type = fx_override.unwrap_or(damage_type);
            if self.is_kind_of(crate::game_logic::KindOf::Vehicle)
                || self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            {
                let rider_change = self.is_combat_cycle_transport
                    || self.thing.template.contain_module.kind
                        == crate::game_logic::ContainModuleKind::RiderChange;
                if rider_change {
                    if self.status.moving {
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
                        self.stamp_last_damage_cpp(source, false, damage_type);
                        let _ =
                            crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                                self,
                                fx_type,
                                damage.max(0.0),
                            );
                        return true;
                    }
                    self.occupants.clear();
                    self.rider_change_scuttled_on_frame =
                        self.rider_change_scuttled_on_frame.max(1);
                    if damage > 0.0 {
                        self.stamp_last_damage_cpp(source, false, damage_type);
                    }
                    let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                        self,
                        fx_type,
                        damage.max(0.0),
                    );
                    let _ = (source, death_type);
                    return false;
                }
                if self.is_car_bomb() {
                    // Detonation handled by combat caller; mark unmanned edge.
                }
                self.apply_kill_pilot_unmanned();
                self.set_team(crate::game_logic::Team::Neutral);
            }
            if damage > 0.0 {
                self.stamp_last_damage_cpp(source, false, damage_type);
            }
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                self,
                fx_type,
                damage.max(0.0),
            );
            let _ = (source, death_type);
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
            if typed > 0.0 {
                self.stamp_last_damage_cpp(source, false, damage_type);
            }
            let fx_type = fx_override.unwrap_or(damage_type);
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                self,
                fx_type,
                typed.max(0.0),
            );
            let _ = (source, death_type);
            return false;
        }
        // C++ DAMAGE_STATUS residual: amount is duration msec, not hitpoints.
        if matches!(damage_type, crate::game_logic::combat::DamageType::Status) {
            let amount = crate::game_logic::host_armor_residual::apply_residual_armor(
                self,
                damage_type,
                damage,
            );
            let frames = ((amount.max(0.0) * 30.0) / 1000.0).ceil() as u32;
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            if frames > 0 {
                // C++ ActiveBody.cpp:460-464 doStatusDamage(m_damageStatusType).
                // Default OBJECT_STATUS_NONE: no paint. Avenger authors FAERIE_FIRE.
                if let Some(name) = peek_pending_damage_status_type() {
                    if !name.is_empty() && !name.eq_ignore_ascii_case("NONE") {
                        self.do_status_damage(name, frames.max(1), frame);
                    }
                }
            }
            if amount > 0.0 {
                self.stamp_last_damage_cpp(source, false, damage_type);
            }
            let fx_type = fx_override.unwrap_or(damage_type);
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                self,
                fx_type,
                amount.max(0.0),
            );
            let _ = (source, death_type);
            return false;
        }

        self.take_damage_from_typed_death_with_host_hp(
            damage,
            source,
            damage_type,
            death_type,
            false,
            fx_override,
        )
    }

    fn take_damage_from_typed_death_with_host_hp(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        force_host_hp: bool,
        fx_override: Option<crate::game_logic::combat::DamageType>,
    ) -> bool {
        let _ctx = PendingDamageContextGuard;
        if self.status.destroyed {
            return false;
        }
        if self.is_inactive_body() {
            return self.apply_inactive_body_damage(damage_type);
        }
        // OCL InvulnerableTime residual (post-eject pilot shield).
        if self.status.eject_invulnerable {
            return false;
        }
        if self.try_hive_structure_body_damage(damage, source, damage_type) {
            return false;
        }
        let prev_health = self.health.current;
        self.previous_health = prev_health;
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

        // C++ UndeadBody.cpp:58-68 / HighlanderBody.cpp:30-36 clamp
        // DamageInfo.in.m_amount PRE-armor, then ActiveBody applies armor
        // to the (possibly clamped) raw amount.
        let mut incoming = damage;
        let mut battle_bus_start_second = false;
        if self.battle_bus_should_intercept_lethal(damage_type, incoming) {
            incoming = incoming.min((self.health.current - 1.0).max(0.0));
            battle_bus_start_second = true;
        }
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
                incoming,
                unres,
            );
            if did {
                incoming = clamped;
                _highlander_clamped = true;
            }
        }

        // C++ ActiveBody::attemptDamage (ActiveBody.cpp:329-330): after
        // subclass raw clamp, bail before armor / lastDamage / FX.
        if self.indestructible {
            if battle_bus_start_second {
                self.start_battle_bus_second_life();
            }
            return false;
        }

        // C++ ActiveBody::attemptDamage: ArmorTemplate::adjustDamage, then
        // m_damageScalar only for non-UNRESISTABLE (ActiveBody.cpp:351, 490-497).
        // No invented armor/(armor+100) extra mitigation.
        let typed = crate::game_logic::host_armor_residual::apply_residual_armor(
            self,
            damage_type,
            incoming,
        );
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
        // C++ MinefieldBehavior::onDamage — next mine tick syncs virtuals from HP.
        if let Some(md) = self.mine_data.as_mut() {
            md.last_synced_health = None;
        }
        // C++ onDamage chain-detonates remaining virtuals before destroyObject.
        // A >=100 hit must not mark the pad destroyed here or the tick skips it.
        let defer_mine_death = self
            .mine_data
            .as_ref()
            .is_some_and(|md| md.defers_lethal_body_destroy());
        let mut destroyed = if !self.health.is_alive() && !defer_mine_death {
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
        // C++ ActiveBody.cpp:501-572: lastDamageInfo only when amount>0 or kill,
        // after armor+scalar. Same-or-next-frame prefers VEHICLE/INFANTRY/faction
        // structure over projectiles; 0-amount crush FX does not stamp.
        if actual_damage > 0.0 || destroyed {
            self.stamp_last_damage_cpp(source, destroyed, damage_type);
        }
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
        let fx_type = fx_override.unwrap_or(damage_type);
        let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
            self,
            fx_type,
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
                if fs.apply_flame_damage(actual_damage, frame) {
                    self.apply_flammable_ignite_visuals();
                }
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
        if self.is_host_bridge_member() {
            crate::game_logic::host_bridge_behavior::record_mirror(
                self.id,
                actual_damage,
                max_health,
                source,
                damage_type.to_store() as u32,
                death_type.ordinal() as u32,
                crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind::Damage,
            );
            if destroyed {
                self.convert_bridge_to_rubble_husk();
                crate::game_logic::host_bridge_behavior::record_death_link(self.id);
                destroyed = false;
            }
        }
        let voice_fear_id = self.id;
        let voice_fear_pos = self.get_position();
        let voice_fear_player = self.owner_player_id;
        crate::game_logic::host_transition_damage_fx::queue_voice_fear_event(
            &mut self.pending_transition_damage_fx,
            &self.template_name,
            old_body_state,
            self.body_damage_state,
            prev_health,
            self.health.current,
            max_health,
            voice_fear_id,
            voice_fear_pos,
            voice_fear_player,
        );
        if battle_bus_start_second {
            false
        } else {
            destroyed
        }
    }

    fn is_host_bridge_member(&self) -> bool {
        self.is_kind_of(KindOf::Bridge)
            || self.is_kind_of(KindOf::BridgeTower)
            || crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                &self.template_name,
            )
    }

    /// C++ KeepObjectDie-style husk so rubble spans/towers stay repairable.
    pub fn convert_bridge_to_rubble_husk(&mut self) {
        self.status.destroyed = false;
        self.status.keep_as_rubble = true;
        self.status.effectively_dead = true;
        self.health.current = 0.0;
        self.refresh_model_condition_bits();
    }

    /// C++ rubble heal: leaving BODY_RUBBLE clears effectively-dead.
    pub fn revive_from_bridge_rubble(&mut self) {
        if self.status.keep_as_rubble || self.status.effectively_dead {
            self.status.keep_as_rubble = false;
            self.status.effectively_dead = false;
            self.status.destroyed = false;
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

    /// C++ InactiveBody templates: FireField / PoisonField / RadiationField.
    pub fn is_inactive_body(&self) -> bool {
        if self.uses_inactive_body
            || self.inferno_fire_field
            || self.nuke_radiation_field
            || self.anthrax_toxin_field
        {
            return true;
        }
        let n = self.template_name.to_ascii_lowercase();
        n.contains("firefield") || n.contains("poisonfield") || n.contains("radiationfield")
    }

    /// C++ InactiveBody::attemptDamage — no HP; UNRESISTABLE calls onDie once.
    fn apply_inactive_body_damage(
        &mut self,
        damage_type: crate::game_logic::combat::DamageType,
    ) -> bool {
        if matches!(damage_type, crate::game_logic::combat::DamageType::Healing) {
            return false;
        }
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Unresistable
        ) && !self.inactive_body_die_called
        {
            self.inactive_body_die_called = true;
            self.status.destroyed = true;
            self.status.effectively_dead = true;
            return true;
        }
        false
    }

    /// C++ HiveStructureBody::attemptDamage on the live host take_damage path.
    fn try_hive_structure_body_damage(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
    ) -> bool {
        use crate::game_logic::host_base_defense::{
            HostHiveDamageClass, hive_damage_class_for_type, is_stinger_site_structure,
            resolve_hive_structure_damage_roster, sync_hive_slave_mirrors,
        };
        if source.is_none() {
            return false;
        }
        if !is_stinger_site_structure(&self.template_name) {
            return false;
        }
        let class = hive_damage_class_for_type(damage_type);
        if matches!(class, HostHiveDamageClass::HitStructure) {
            return false;
        }
        let struct_hp = self.health.current;
        let pos = self.get_position();
        let shooter_xz = take_hive_shooter_xz().map(|(qx, qz)| (pos.x, pos.z, qx, qz));
        let (_, _, result) = resolve_hive_structure_damage_roster(
            &mut self.hive_slaves,
            struct_hp,
            damage.max(0.0),
            class,
            shooter_xz,
        );
        let (count, hp) = sync_hive_slave_mirrors(&self.hive_slaves);
        self.hive_slave_count = count;
        self.hive_slave_hp = hp;
        self.record_host_hive();
        if result.slaves_killed > 0 {
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            self.hive_slave_respawn_frame =
                crate::game_logic::host_base_defense::next_stinger_slave_respawn_frame(
                    frame,
                    self.hive_slave_respawn_frame,
                );
        }
        if result.slave_damage_applied > 0.0 || result.swallowed {
            let dealt = if result.swallowed {
                0.0
            } else {
                result.slave_damage_applied
            };
            let _ = crate::game_logic::host_transition_damage_fx::dispatch_armor_damage_fx(
                self,
                damage_type,
                dealt,
            );
            return true;
        }
        false
    }
    fn stamp_last_damage_always(&mut self, _source: Option<ObjectId>) {
        let frame = crate::game_logic::host_historic_bonus::logic_frame();
        // C++ lastDamageInfo.m_damageType = HEALING so Guard/stealth skip the healer.
        self.last_damage_source = None;
        self.last_damage_source_preferred = false;
        self.last_damage_timestamp = Some(frame);
        self.last_damage_info_type = Some(crate::game_logic::combat::DamageType::Healing);
    }

    /// C++ ActiveBody.cpp:501-572 lastDamageInfo: amount>0 or kill, after armor.
    /// Same-or-next-frame overwrites only if new source exists and (old is gone
    /// or new is VEHICLE/INFANTRY/faction structure).
    fn stamp_last_damage_cpp(
        &mut self,
        source: Option<ObjectId>,
        preferred: bool,
        damage_type: crate::game_logic::combat::DamageType,
    ) {
        self.last_damage_info_type = Some(damage_type);
        let frame = crate::game_logic::host_historic_bonus::logic_frame();
        let same_or_next = self.last_damage_timestamp == Some(frame)
            || self.last_damage_timestamp == Some(frame.saturating_sub(1));
        if !same_or_next {
            self.last_damage_source = source;
            self.last_damage_timestamp = Some(frame);
            self.last_damage_source_preferred = preferred;
            return;
        }
        if source.is_none() {
            return;
        }
        if self.last_damage_source.is_none() || preferred {
            self.last_damage_source = source;
            self.last_damage_timestamp = Some(frame);
            self.last_damage_source_preferred = preferred;
        }
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
    fn subdual_vehicle_is_not_hp_unresistable() {
        // C++ IsSubdualDamage (Damage.h:95-107) + ActiveBody.cpp:471-488.
        // ECMTankVehicleDisabler is SUBDUAL_VEHICLE — accumulate, never HP.
        let mut tank = vehicle("SubdualVehTank", 21, 400.0);
        tank.subdual_damage_cap = 600.0;
        assert!(!tank.take_damage_from_typed(24.0, None, DamageType::SubdualVehicle));
        assert!(
            (tank.health.current - 400.0).abs() < 1e-3,
            "SUBDUAL_VEHICLE must not deal HP, got {}",
            tank.health.current
        );
        assert!(
            (tank.subdual_damage - 24.0).abs() < 1e-3,
            "TankArmor SUBDUAL_VEHICLE 100% must accumulate, got {}",
            tank.subdual_damage
        );
        assert!(!tank.is_weapons_jammed());
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
        register_coeff_armor(
            "StatusHalfArmor",
            gamelogic::damage::DamageType::Status,
            0.5,
        );
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
        // Avenger-shaped STATUS weapon authors FAERIE_FIRE.
        set_pending_damage_status_type(Some("FAERIE_FIRE"));
        assert!(!o.take_damage_from_typed(2000.0, None, DamageType::Status));
        assert!((o.health.current - 100.0).abs() < 1e-3);
        assert!(o.is_faerie_fire());
        assert_eq!(o.faerie_fire_until_frame, frame.saturating_add(30));
    }

    #[test]
    fn status_none_does_not_hardcode_faerie_fire() {
        let mut o = vehicle("StatusNone", 46, 100.0);
        assert!(!o.take_damage_from_typed(2000.0, None, DamageType::Status));
        assert!(!o.is_faerie_fire());
        assert!((o.health.current - 100.0).abs() < 1e-3);
    }

    #[test]
    fn radiation_field_tick_uses_armor_and_skips_airborne() {
        // C++ NukeRadiationFieldWeapon DamageType RADIATION + NOT_AIRBORNE.
        register_coeff_armor(
            "RadTankArmor",
            gamelogic::damage::DamageType::Radiation,
            0.5,
        );
        let mut tank = vehicle("RadTank", 47, 100.0);
        tank.thing
            .template
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some("RadTankArmor".into()),
                damage_fx: None,
            });
        assert!(!tank.take_radiation_field_tick(20.0, None));
        assert!(
            (tank.health.current - 90.0).abs() < 1e-3,
            "Radiation must apply armor (50% of 20), got {}",
            tank.health.current
        );

        let mut jet = vehicle("RadJet", 48, 100.0);
        jet.status.airborne_target = true;
        assert!(!jet.take_radiation_field_tick(25.0, None));
        assert!(
            (jet.health.current - 100.0).abs() < 1e-3,
            "NOT_AIRBORNE must skip flying victims, got {}",
            jet.health.current
        );
    }

    #[test]
    fn damage_fx_uses_attacker_veterancy_not_victim() {
        use crate::game_logic::host_transition_damage_fx::{
            set_damage_fx_source, snapshot_damage_fx_source, take_dispatched_armor_damage_fx,
        };
        game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
        let mut dfx = game_engine::common::ini::ini_damage_fx::DamageFX::new();
        dfx.set_major_minor_fx_at_level(
            game_engine::common::ini::ini_damage_fx::DamageType::Unresistable,
            0,
            Some("FX_RegularHit".into()),
            None,
            0.0,
        );
        dfx.set_major_minor_fx_at_level(
            game_engine::common::ini::ini_damage_fx::DamageType::Unresistable,
            3,
            Some("FX_HeroHit".into()),
            None,
            0.0,
        );
        if let Some(mut store) = game_engine::common::ini::ini_damage_fx::get_damage_fx_store_mut()
        {
            store.add_damage_fx("VetDamageFX".into(), dfx);
        }
        let _ = take_dispatched_armor_damage_fx();
        let mut attacker = vehicle("HeroGun", 82, 100.0);
        attacker.experience.level = crate::game_logic::VeterancyLevel::Heroic;
        set_damage_fx_source(Some(snapshot_damage_fx_source(&attacker)));
        let mut victim = vehicle("RookieTank", 83, 200.0);
        victim.experience.level = crate::game_logic::VeterancyLevel::Rookie;
        victim
            .thing
            .template
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some("TankArmor".into()),
                damage_fx: Some("VetDamageFX".into()),
            });
        assert!(!victim.take_damage_from_typed(20.0, Some(ObjectId(82)), DamageType::Unresistable));
        let dispatched = take_dispatched_armor_damage_fx();
        assert!(
            dispatched.iter().any(|n| n == "FX_HeroHit"),
            "DamageFX must use attacker (hero) list, got {dispatched:?}"
        );
        assert!(
            !dispatched.iter().any(|n| n == "FX_RegularHit"),
            "must not pick victim Regular list, got {dispatched:?}"
        );
    }

    #[test]
    fn take_damage_dispatches_armor_damage_fx() {
        // C++ ActiveBody.cpp:653 doDamageFX after attemptDamage.
        use crate::game_logic::host_transition_damage_fx::{
            TemplateDamageAudio, take_dispatched_armor_damage_fx,
        };
        game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
        let mut dfx = game_engine::common::ini::ini_damage_fx::DamageFX::new();
        dfx.set_major_minor_fx(
            game_engine::common::ini::ini_damage_fx::DamageType::Unresistable,
            Some("FX_TestHitSpark".into()),
            None,
            0.0,
        );
        if let Some(mut store) = game_engine::common::ini::ini_damage_fx::get_damage_fx_store_mut()
        {
            store.add_damage_fx("TankDamageFX".into(), dfx);
        }
        let _ = take_dispatched_armor_damage_fx();
        let mut tank = vehicle("FxTank", 61, 200.0);
        tank.thing
            .template
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some("TankArmor".into()),
                damage_fx: Some("TankDamageFX".into()),
            });
        assert!(!tank.take_damage_from_typed(20.0, None, DamageType::Unresistable));
        let dispatched = take_dispatched_armor_damage_fx();
        assert!(
            dispatched
                .iter()
                .any(|n| n == "TankDamageFX" || n == "FX_TestHitSpark"),
            "armor DamageFX must dispatch, got {dispatched:?}"
        );
        let _ = TemplateDamageAudio::default();
    }

    #[test]
    fn take_damage_queues_template_voice_fear_and_attacked_by() {
        // C++ ActiveBody.cpp:574 setAttackedBy, :624-637 VoiceFear.
        use crate::game_logic::host_transition_damage_fx::{
            TemplateDamageAudio, set_test_template_audio, set_test_voice_fear_roll,
            take_pending_attacked_by,
        };
        crate::game_logic::host_transition_damage_fx::clear_test_template_audio();
        crate::game_logic::host_voice_fear_log::clear();
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
        ranger.set_position(glam::Vec3::new(120.0, 4.0, 80.0));
        assert!(!ranger.take_damage_from_typed(20.0, Some(ObjectId(77)), DamageType::Unresistable));
        assert!((ranger.health.current - 20.0).abs() < 1e-3);
        let pending = ranger.take_pending_transition_damage_fx();
        assert!(
            pending
                .iter()
                .any(|e| e.audio_name.as_deref() == Some("RangerVoiceFear")),
            "VoiceFear must queue on yellow cross, got {pending:?}"
        );
        let fear = crate::game_logic::host_voice_fear_log::drain();
        assert_eq!(fear.len(), 1);
        assert_eq!(fear[0].event_name, "RangerVoiceFear");
        assert_eq!(fear[0].victim, ObjectId(62));
        assert_eq!(fear[0].position, glam::Vec3::new(120.0, 4.0, 80.0));
        assert_eq!(fear[0].player_id, Some(1));
        assert_ne!(fear[0].event_name, "FearRangerVoiceFear");
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
        let attacker =
            std::sync::Arc::new(std::sync::RwLock::new(gamelogic::player::Player::new(4)));
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

    #[test]
    fn poison_field_tick_infects_and_unresistable_does_not() {
        // C++ FireWeaponUpdate DAMAGE_POISON → armor + PoisonedBehavior::onDamage.
        let mut unit = Object::new(
            ThingTemplate::new("PoisonInfantry"),
            ObjectId(31),
            Team::USA,
        );
        unit.health.current = 100.0;
        unit.health.maximum = 100.0;
        assert!(!unit.take_damage_from_immediate_typed_death(
            10.0,
            None,
            DamageType::Toxin,
            crate::game_logic::host_usa_pilot::HostDeathType::PoisonedBeta,
        ));
        let p = unit
            .poisoned_behavior
            .as_ref()
            .expect("field tick must infect");
        assert!(p.is_active());
        assert_eq!(
            p.death_type,
            crate::game_logic::host_usa_pilot::HostDeathType::PoisonedBeta
        );
        assert!((p.poison_damage_amount - 10.0).abs() < 0.01);

        let mut bare = Object::new(ThingTemplate::new("NoPoison"), ObjectId(32), Team::USA);
        bare.health.current = 100.0;
        bare.health.maximum = 100.0;
        bare.take_damage_from_immediate(10.0, None);
        assert!(bare.poisoned_behavior.is_none());
    }

    #[test]
    fn poison_dot_unresistable_does_not_reinfect() {
        // C++ PoisonedBehavior::update retakes UNRESISTABLE with POISON FX override.
        let mut unit = Object::new(ThingTemplate::new("DotInfantry"), ObjectId(33), Team::USA);
        unit.health.current = 100.0;
        unit.health.maximum = 100.0;
        unit.take_damage_from_immediate_typed_death(
            8.0,
            None,
            DamageType::Toxin,
            crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
        );
        let amount = unit
            .poisoned_behavior
            .as_ref()
            .unwrap()
            .poison_damage_amount;
        unit.take_damage_from_typed_death_fx(
            8.0,
            None,
            DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
            Some(DamageType::Toxin),
        );
        let p = unit.poisoned_behavior.as_ref().unwrap();
        assert!(p.is_active());
        assert!(
            (p.poison_damage_amount - amount).abs() < 0.01,
            "Unresistable DoT must not refresh poison amount"
        );
    }

    #[test]
    fn last_damage_skips_zero_and_same_frame_first_write_wins() {
        let mut unit = vehicle("StampTank", 71, 200.0);
        assert!(!unit.take_damage_from_typed(0.0, Some(ObjectId(1)), DamageType::Bullet));
        assert!(unit.last_damage_source.is_none());
        assert!(unit.last_damage_timestamp.is_none());

        assert!(!unit.take_damage_from_typed(10.0, Some(ObjectId(1)), DamageType::Unresistable));
        assert_eq!(unit.last_damage_source, Some(ObjectId(1)));
        let ts = unit.last_damage_timestamp;
        assert!(!unit.take_damage_from_typed(10.0, Some(ObjectId(2)), DamageType::Unresistable));
        assert_eq!(
            unit.last_damage_source,
            Some(ObjectId(1)),
            "same-frame later hit must not last-write-wins"
        );
        assert_eq!(unit.last_damage_timestamp, ts);
    }

    #[test]
    fn healing_stamps_last_healing_not_hostile_source() {
        let mut unit = vehicle("HealTank", 72, 200.0);
        unit.health.current = 50.0;
        assert!(!unit.take_damage_from_typed(20.0, Some(ObjectId(9)), DamageType::Healing));
        assert!(unit.last_damage_source.is_none());
        assert!(unit.last_healing_timestamp.is_some());
    }

    #[test]
    fn inactive_body_fire_field_ignores_hp_unresistable_dies_once() {
        let mut field = Object::new(
            ThingTemplate::new("FireFieldSmall"),
            ObjectId(73),
            Team::China,
        );
        field.health.current = 50.0;
        field.health.maximum = 50.0;
        field.uses_inactive_body = true;
        field.status.effectively_dead = true;
        assert!(!field.take_damage_from_typed(20.0, Some(ObjectId(1)), DamageType::Bullet));
        assert!((field.health.current - 50.0).abs() < 1e-3);
        assert!(!field.status.destroyed);
        assert!(field.take_damage_from_typed(1.0, Some(ObjectId(1)), DamageType::Unresistable));
        assert!(field.status.destroyed);
        assert!(!field.take_damage_from_typed(1.0, Some(ObjectId(1)), DamageType::Unresistable));
    }

    #[test]
    fn stinger_small_arms_and_sniper_hit_slaves_not_structure() {
        use crate::game_logic::host_base_defense::{
            HostHiveDamageClass, hive_damage_class_for_type, init_stinger_hive_slave_roster,
            sync_hive_slave_mirrors,
        };
        assert_eq!(
            hive_damage_class_for_type(DamageType::Bullet),
            HostHiveDamageClass::PropagateToSlaves
        );
        assert_eq!(
            hive_damage_class_for_type(DamageType::Sniper),
            HostHiveDamageClass::SwallowIfNoSlaves
        );
        let mut tmpl = ThingTemplate::new("GLAStingerSite");
        tmpl.set_health(1000.0);
        tmpl.add_kind_of(KindOf::Structure);
        let mut site = Object::new(tmpl, ObjectId(74), Team::GLA);
        site.health.current = 1000.0;
        site.health.maximum = 1000.0;
        site.hive_slaves = init_stinger_hive_slave_roster();
        let (c, h) = sync_hive_slave_mirrors(&site.hive_slaves);
        site.hive_slave_count = c;
        site.hive_slave_hp = h;
        assert!(!site.take_damage_from_typed(40.0, Some(ObjectId(5)), DamageType::Bullet));
        assert!((site.health.current - 1000.0).abs() < 0.01);
        assert!((site.hive_slave_hp - 60.0).abs() < 0.01);
        assert_eq!(site.hive_slave_count, 3);

        let mut empty = site;
        crate::game_logic::host_base_defense::clear_hive_slave_roster(&mut empty.hive_slaves);
        empty.hive_slave_count = 0;
        empty.hive_slave_hp = 0.0;
        empty.health.current = 1000.0;
        assert!(!empty.take_damage_from_typed(200.0, Some(ObjectId(5)), DamageType::Sniper));
        assert!((empty.health.current - 1000.0).abs() < 0.01);
    }

    #[test]
    fn health_box_uses_geometry_not_fixed_twenty() {
        let mut tmpl = ThingTemplate::new("BoxTank");
        tmpl.set_health(400.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.geometry_info = crate::game_logic::HostGeometryInfo {
            geom_type: crate::game_logic::HostGeometryType::Box,
            is_small: true,
            height: 10.0,
            major_radius: 13.0,
            minor_radius: 9.0,
            authored: true,
        };
        let tank = Object::new(tmpl, ObjectId(75), Team::USA);
        let (h, w) = tank.get_health_box_dimensions();
        assert!((h - 3.0).abs() < 1e-3);
        assert!(
            (w - 44.0).abs() < 1e-3,
            "width = clamp(13+9,20,150)*2 = 44, got {w}"
        );
        let pos = tank.get_health_box_position();
        assert!(
            (pos.y - 20.0).abs() < 1e-3,
            "y = 0 + height 10 + 10, got {}",
            pos.y
        );
    }

    #[test]
    fn undead_body_intercepts_on_raw_amount_not_post_armor() {
        // C++ UndeadBody.cpp:58-64 compares damageInfo->in.m_amount PRE-armor.
        // Flame 0.1 vs 50 HP: raw 200 intercepts even though 20 post-armor would not kill.
        register_coeff_armor("BusFlameArmor", gamelogic::damage::DamageType::Flame, 0.1);
        let mut bus = vehicle("GLAVehicleBattleBus", 91, 50.0);
        bus.install_battle_bus_transport();
        bus.health.maximum = 50.0;
        bus.health.current = 50.0;
        bus.thing
            .template
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some("BusFlameArmor".into()),
                damage_fx: None,
            });
        assert!(!bus.take_damage_from_typed(200.0, None, DamageType::Flame));
        assert!(
            bus.armor_set_second_life,
            "raw 200 >= 50 must start second life despite 0.1 flame armor"
        );
        assert!(bus.is_alive());
    }

    #[test]
    fn undead_body_does_not_intercept_when_raw_below_health() {
        // >100% coefficient: raw 40 < 50, post-armor 80 would kill — C++ does not intercept.
        register_coeff_armor(
            "BusVulnArmor",
            gamelogic::damage::DamageType::Explosion,
            2.0,
        );
        let mut bus = vehicle("GLAVehicleBattleBus", 92, 50.0);
        bus.install_battle_bus_transport();
        bus.health.maximum = 50.0;
        bus.health.current = 50.0;
        bus.thing
            .template
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some("BusVulnArmor".into()),
                damage_fx: None,
            });
        let killed = bus.take_damage_from_typed(40.0, None, DamageType::Explosive);
        assert!(
            killed,
            "raw 40 < health must not intercept; 2.0 armor should kill"
        );
        assert!(!bus.armor_set_second_life);
    }

    #[test]
    fn highlander_clamps_raw_then_applies_armor() {
        // C++ HighlanderBody.cpp:33-36: min(raw, health-1) then ActiveBody armor.
        // 999 raw → 49, * 0.25 flame = 12.25, HP 37.75. Post-armor clamp would leave 1.
        register_coeff_armor("TreeFlameArmor", gamelogic::damage::DamageType::Flame, 0.25);
        let mut tree = vehicle("Tree01", 93, 50.0);
        tree.install_highlander_body();
        tree.thing
            .template
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some("TreeFlameArmor".into()),
                damage_fx: None,
            });
        assert!(!tree.take_damage_from_typed(999.0, None, DamageType::Flame));
        assert!(
            (tree.health.current - 37.75).abs() < 0.05,
            "raw clamp then 0.25 armor must leave 37.75, got {}",
            tree.health.current
        );
        assert!(!tree.status.destroyed);
    }

    #[test]
    fn highlander_over100_coeff_can_die_from_non_unresistable() {
        // C++ clamps raw only: 40 < 49 so no clamp, *2.0 = 80 kills.
        register_coeff_armor(
            "TreeVulnArmor",
            gamelogic::damage::DamageType::Explosion,
            2.0,
        );
        let mut tree = vehicle("Tree01", 94, 50.0);
        tree.install_highlander_body();
        tree.thing
            .template
            .armor_sets
            .push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some("TreeVulnArmor".into()),
                damage_fx: None,
            });
        assert!(tree.take_damage_from_typed(40.0, None, DamageType::Explosive));
        assert!(tree.status.destroyed);
    }

    #[test]
    fn indestructible_bails_before_hp() {
        // C++ ActiveBody.cpp:329-330: m_indestructible returns before armor/HP.
        let mut o = vehicle("ScriptProp", 95, 100.0);
        o.set_indestructible(true);
        assert!(!o.take_damage_from_typed(50.0, None, DamageType::Explosive));
        assert!((o.health.current - 100.0).abs() < 1e-3);
        assert!(!o.take_damage_from_typed(999.0, None, DamageType::Unresistable));
        assert!((o.health.current - 100.0).abs() < 1e-3);
        assert!(!o.status.destroyed);
        o.set_indestructible(false);
        assert!(!o.take_damage_from_typed(10.0, None, DamageType::Unresistable));
        assert!((o.health.current - 90.0).abs() < 1e-3);
    }
}
