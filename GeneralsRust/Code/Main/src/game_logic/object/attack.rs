use super::*;

#[derive(Clone, Copy)]
enum WeaponShotRoute {
    Autonomous {
        owned_rof: Option<[f32; 3]>,
        owned_bonuses: Option<[(f32, f32, f32, f32, f32); 3]>,
    },
    Passive {
        slot: u8,
        origin: Vec3,
        damage_bonus: Option<f32>,
        pre_attack_bonus: Option<f32>,
        owned_rof: Option<[f32; 3]>,
        allow_held: bool,
    },
}

/// C++ `Weapon::fireWeapon(source, victim)` vs `fireWeapon(source, pos)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ShotVictim {
    Object(ObjectId),
    Position(Vec3),
}

impl ShotVictim {
    pub(crate) fn object(self) -> Option<ObjectId> {
        match self {
            Self::Object(id) => Some(id),
            Self::Position(_) => None,
        }
    }

    pub(crate) fn position(self) -> Option<Vec3> {
        match self {
            Self::Object(_) => None,
            Self::Position(pos) => Some(pos),
        }
    }
}

impl Object {
    /// Freeze Weapon::calcProjectileLaunchPosition while the selected barrel,
    /// logical Draw state and turret pose still belong to this shot.
    fn projectile_launch_transform(
        &self,
        slot: u8,
        barrel: u8,
        assets: Option<&dyn crate::assets::pristine_bones::PristineBoneAssets>,
    ) -> Option<glam::Mat4> {
        if self.contained_by.is_some() && self.status.masked {
            return Some(self.get_transform_matrix());
        }
        let Some(drawings) = self.get_template().receiving_draw_models_ref() else {
            // Weapon.cpp uses an identity attachment when there is no Drawable.
            return Some(self.get_transform_matrix());
        };
        let draws = drawings.object_draws(self.model_condition_bits).ok()?;
        if draws.is_empty() {
            return Some(self.get_transform_matrix());
        }
        let assets = assets?;
        let turret = self.which_turret_for_weapon_slot(slot);
        for draw in &draws {
            if !draw.weapon_bone_bindings.source_fields_valid {
                return None;
            }
            let binding = draw.weapon_bone_bindings.slot(slot)?;
            let bases = [
                binding.fire_fx_bone_base.as_deref(),
                binding.recoil_bone_base.as_deref(),
                binding.muzzle_flash_bone_base.as_deref(),
                binding.launch_bone_base.as_deref(),
            ];
            let bone = |name: &str, index: i32| {
                assets.pristine_bones_for_selected_draw(
                    draw, self.drawable_instance_scale, name, index, 1,
                ).ok().map(|bones| bones.into_iter().next())
            };
            let mut first = None;
            let mut selected = None;
            for index in 1..=i32::from(barrel) + 1 {
                let mut found = false;
                let mut launch = None;
                for (kind, base) in bases.iter().enumerate() {
                    if let Some(base) = base {
                        let value = bone(base, index)?;
                        found |= value.is_some();
                        if kind == 3 { launch = value; }
                    }
                }
                if !found { break; }
                if first.is_none() { first = Some(launch); }
                if index == i32::from(barrel) + 1 { selected = Some(launch); }
            }
            if first.is_none() {
                let mut found = false;
                let mut launch = None;
                for (kind, base) in bases.iter().enumerate() {
                    if let Some(base) = base {
                        let value = bone(base, 0)?;
                        found |= value.is_some();
                        if kind == 3 { launch = value; }
                    }
                }
                if found { first = Some(launch); }
            }
            let Some(launch) = selected.or(first) else { continue };
            let mut local = launch.unwrap_or(glam::Mat4::IDENTITY);
            if let Some(turret) = turret {
                let info = &draw.primary_turret;
                let (yaw_name, pitch_name, art_yaw, art_pitch, ai) = match turret {
                    0 if info.primary_fields_valid => (
                        info.yaw_bone.as_deref(), info.pitch_bone.as_deref(),
                        info.yaw_art_angle_radians(), info.pitch_art_angle_radians(),
                        &self.turret_ai,
                    ),
                    1 if info.alternate_fields_valid => (
                        info.alternate_yaw_bone.as_deref(), info.alternate_pitch_bone.as_deref(),
                        info.alternate_yaw_art_angle_radians(), info.alternate_pitch_art_angle_radians(),
                        self.alt_turret_ai.as_ref()?,
                    ),
                    _ => return None,
                };
                let pivot = |name: Option<&str>| -> Option<Vec3> {
                    match name {
                        Some(name) => Some(bone(name, 0)?.map_or(Vec3::ZERO, |m| m.w_axis.truncate())),
                        None => Some(Vec3::ZERO),
                    }
                };
                let attachment = match draw.attach_to_drawable_bone.as_deref() {
                    Some(name) => {
                        let source = crate::assets::pristine_bones::PristineBoneSource {
                            drawings, condition_bits: self.model_condition_bits,
                            mesh_scale: self.drawable_instance_scale,
                        };
                        assets.pristine_bones_for_draws(&source, name, 0, 1).ok()?
                            .first().map_or(Vec3::ZERO, |m| m.w_axis.truncate())
                    }
                    None => Vec3::ZERO,
                };
                let yaw = pivot(yaw_name)? + attachment;
                let pitch = pivot(pitch_name)? + attachment;
                local = glam::Mat4::from_rotation_y(-art_pitch)
                    * glam::Mat4::from_rotation_z(art_yaw) * local;
                local = glam::Mat4::from_translation(yaw)
                    * glam::Mat4::from_rotation_z(ai.turret_angle_deg.to_radians())
                    * glam::Mat4::from_translation(-yaw)
                    * glam::Mat4::from_translation(pitch)
                    * glam::Mat4::from_rotation_y(-ai.turret_pitch_deg.to_radians())
                    * glam::Mat4::from_translation(-pitch) * local;
            }
            // Bone matrices are C++ Z-up; the host pose is Y-up.
            let local = crate::game_logic::host_fx_list_dispatch::host_to_leftover_mat4(local);
            return Some(self.get_transform_matrix() * local);
        }
        None
    }

    /// Fire at target. `target_is_infantry` selects ScatterRadiusVsInfantry residual.
    pub fn fire_at(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        logic_frame: u32,
        combat: &mut super::combat::CombatSystem,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        self.fire_at_ex(
            target_id,
            current_time,
            false,
            false,
            logic_frame,
            combat,
            attack_events,
            audio,
        )
    }

    /// Fire at target with KindOf-aware scatter residual.
    /// `target_has_faerie_fire`: C++ TARGET_FAERIE_FIRE WeaponBonus ROF residual.
    pub fn fire_at_ex(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        combat: &mut super::combat::CombatSystem,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        let Some(slot) = self.fire_at_ex_defer_weapon_barrel_advance(
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            combat,
            attack_events,
            audio,
        ) else {
            return false;
        };
        // This legacy Object-only entry point has no owning GameLogic from
        // which to allocate a durable actual-discharge sequence. Preserve its
        // logical cursor behavior, but keep it deliberately presentation
        // eventless. Live GameLogic fire paths use the deferred entry point
        // and record_accepted_weapon_discharge instead.
        self.advance_weapon_barrel_after_shot(slot);
        true
    }

    /// Execute one accepted shot without advancing its exact WeaponSet barrel.
    ///
    /// The owning GameLogic must immediately finish this through
    /// `record_accepted_weapon_discharge`, which captures the pre-advance
    /// barrel, advances it once, stamps its durable marker, and freezes a
    /// renderer-facing event. Object-only legacy paths retain `fire_at_ex`.
    pub(crate) fn fire_at_ex_defer_weapon_barrel_advance(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        combat: &mut super::combat::CombatSystem,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> Option<u8> {
        let (slot, pending) = self.prepare_weapon_shot(
            WeaponShotRoute::Autonomous {
                owned_rof: None,
                owned_bonuses: None,
            },
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            &mut combat.fire_sound_loop_events,
            attack_events,
            None,
            audio,
        )?;
        super::combat::queue_projectile(combat, pending);
        Some(slot)
    }

    /// One ordinary current-weapon discharge without changing the passenger's
    /// or turret's AI state. The receiving world emits this exact owned shot
    /// and commits its barrel once; earlier accepted shots are not drained.
    pub(crate) fn prepare_passive_weapon_shot(
        &mut self,
        slot: u8,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        muzzle_origin: Vec3,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_passive_weapon_shot_with_operation(
            slot,
            target_id,
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            muzzle_origin,
            sound_events,
            attack_events,
            content,
            audio,
            0,
            &mut Default::default(),
        )
    }

    pub(crate) fn prepare_passive_weapon_shot_with_operation(
        &mut self,
        slot: u8,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        muzzle_origin: Vec3,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Passive {
                slot,
                origin: muzzle_origin,
                damage_bonus: None,
                pre_attack_bonus: None,
                owned_rof: None,
                allow_held: false,
            },
            ShotVictim::Object(target_id),
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
            audio,
            operation_frame,
            operation_effects,
            None,
            None,
            None,
            false, // defer_post_fire_commit
        )
    }

    /// Passive/contained-unit discharge using the receiving GameLogic's
    /// admitted SOLO set. The common shot operation retains its read order.
    pub(crate) fn prepare_passive_weapon_shot_with_operation_and_solo_set(
        &mut self,
        slot: u8,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        muzzle_origin: Vec3,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        solo_set: &gamelogic::weapon::WeaponBonusSet,
        damage_bonus: f32,
        pre_attack_bonus: f32,
        owned_rof: [f32; 3],
        random: &game_engine::common::random_value::SessionRandom,
        projectile_team_id: Option<u32>,
        allow_held: bool,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Passive {
                slot,
                origin: muzzle_origin,
                damage_bonus: Some(damage_bonus),
                pre_attack_bonus: Some(pre_attack_bonus),
                owned_rof: Some(owned_rof),
                allow_held,
            },
            ShotVictim::Object(target_id),
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
            audio,
            operation_frame,
            operation_effects,
            Some(solo_set),
            Some(random),
            projectile_team_id,
            true, // defer_post_fire_commit
        )
    }

    /// Same shot operation fired at a ground position (C++
    /// `Weapon::fireWeapon(source, pos)`): no victim identity, pre-attack or
    /// AI target is recorded by the operation itself.
    pub(crate) fn prepare_passive_position_shot_with_operation_and_solo_set(
        &mut self,
        slot: u8,
        target_position: Vec3,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        muzzle_origin: Vec3,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        solo_set: &gamelogic::weapon::WeaponBonusSet,
        damage_bonus: f32,
        pre_attack_bonus: f32,
        owned_rof: [f32; 3],
        random: &game_engine::common::random_value::SessionRandom,
        projectile_team_id: Option<u32>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Passive {
                slot,
                origin: muzzle_origin,
                damage_bonus: Some(damage_bonus),
                pre_attack_bonus: Some(pre_attack_bonus),
                owned_rof: Some(owned_rof),
                allow_held: false,
            },
            ShotVictim::Position(target_position),
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
            audio,
            operation_frame,
            operation_effects,
            Some(solo_set),
            Some(random),
            projectile_team_id,
            true, // defer_post_fire_commit
        )
    }

    pub(crate) fn prepare_autonomous_weapon_shot(
        &mut self,
        target: ObjectId,
        time: f32,
        infantry: bool,
        faerie: bool,
        frame: u32,
        sound: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attacks: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_autonomous_weapon_shot_with_operation(
            target,
            time,
            infantry,
            faerie,
            frame,
            sound,
            attacks,
            content,
            audio,
            0,
            &mut Default::default(),
        )
    }

    pub(crate) fn prepare_autonomous_weapon_shot_with_operation(
        &mut self,
        target: ObjectId,
        time: f32,
        infantry: bool,
        faerie: bool,
        frame: u32,
        sound: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attacks: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Autonomous {
                owned_rof: None,
                owned_bonuses: None,
            },
            ShotVictim::Object(target),
            time,
            infantry,
            faerie,
            frame,
            sound,
            attacks,
            content,
            audio,
            operation_frame,
            operation_effects,
            None,
            None,
            None,
            false, // defer_post_fire_commit
        )
    }

    /// Autonomous AttackStateMachine shot using its receiving GameLogic owner's
    /// immutable SOLO WeaponBonusSet. The borrow is observed at every existing
    /// bonus read point in the unchanged shot operation order.
    pub(crate) fn prepare_autonomous_weapon_shot_with_operation_and_solo_set(
        &mut self,
        target: ObjectId,
        time: f32,
        infantry: bool,
        faerie: bool,
        frame: u32,
        sound: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attacks: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        solo_set: &gamelogic::weapon::WeaponBonusSet,
        owned_rof: [f32; 3],
        owned_bonuses: [(f32, f32, f32, f32, f32); 3],
        random: &game_engine::common::random_value::SessionRandom,
        projectile_team_id: Option<u32>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            WeaponShotRoute::Autonomous {
                owned_rof: Some(owned_rof),
                owned_bonuses: Some(owned_bonuses),
            },
            ShotVictim::Object(target),
            time,
            infantry,
            faerie,
            frame,
            sound,
            attacks,
            content,
            audio,
            operation_frame,
            operation_effects,
            Some(solo_set),
            Some(random),
            projectile_team_id,
            true, // defer_post_fire_commit
        )
    }
    /// The receiving content catalogue cannot silently substitute a process
    /// bootstrap profile for an unimplemented authored projectile family.
    pub(in crate::game_logic) fn selected_projectile_is_admitted(
        &self,
        slot: u8,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
    ) -> bool {
        let Some(content) = content else {
            return true;
        };
        let Some(rule) = self.selected_native_weapon_rule(slot) else {
            return false;
        };
        let projectile = rule.projectile_name.trim();
        if projectile.is_empty()
            || projectile.eq_ignore_ascii_case("None")
            || content.get(projectile).is_some()
        {
            return true;
        }
        log::error!(
            "Receiving projectile definition '{}' has no admitted flight family",
            projectile
        );
        false
    }

    fn prepare_weapon_shot(
        &mut self,
        route: WeaponShotRoute,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        self.prepare_weapon_shot_with_operation(
            route,
            ShotVictim::Object(target_id),
            current_time,
            target_is_infantry,
            target_has_faerie_fire,
            logic_frame,
            sound_events,
            attack_events,
            content,
            audio,
            logic_frame,
            &mut Default::default(),
            None,
            None,
            None,
            false, // defer_post_fire_commit
        )
    }

    fn prepare_weapon_shot_with_operation(
        &mut self,
        route: WeaponShotRoute,
        victim: ShotVictim,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
        logic_frame: u32,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        content: Option<&crate::game_logic::projectile_definitions::ProjectileDefinitions>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        solo_set: Option<&gamelogic::weapon::WeaponBonusSet>,
        random: Option<&game_engine::common::random_value::SessionRandom>,
        projectile_team_id: Option<u32>,
        defer_post_fire_commit: bool,
    ) -> Option<(u8, super::combat::PendingProjectile)> {
        let autonomous = matches!(route, WeaponShotRoute::Autonomous { .. });
        // C++ Weapon::fireWeapon(source, victim) vs fireWeapon(source, pos).
        let target_object = victim.object();
        let owned_rof = match &route {
            WeaponShotRoute::Passive { owned_rof, .. } => owned_rof.as_ref(),
            WeaponShotRoute::Autonomous { owned_rof, .. } => owned_rof.as_ref(),
        };
        // C++ Weapon::getMaxShotCount residual — AI burst / scatter limits.
        if !self.has_max_shots_remaining() {
            return None;
        }

        // Spawn-point soldiers are held in place yet autonomously fire.
        // Passive passenger routes retain their explicit containment policy.
        let held_passenger = (autonomous && self.contained_by.is_none())
            || matches!(route, WeaponShotRoute::Passive { allow_held: true, .. });
        let disabled = if held_passenger {
            self.status.under_construction || self.has_disabled_type_except_held()
        } else {
            self.is_disabled()
        };
        if self.status.weapons_jammed || disabled {
            return None;
        }
        // C++ Object::isAbleToAttack (Object.cpp:3171-3176): sold / under
        // construction cannot discharge even if a weapon is otherwise ready.
        if self.status.under_construction || self.status.sold {
            return None;
        }
        // C++ setWeaponBonusCondition → onWeaponBonusChange: restart an
        // in-progress wait with the new RATE_OF_FIRE delay before the ready
        // check so mid-reload bonuses cannot fire sooner than C++.
        if let Some(rates) = owned_rof {
            self.apply_weapon_bonus_rof_restart_with_rates(current_time, rates);
        } else {
            self.apply_weapon_bonus_rof_restart_with_optional_solo_set(current_time, solo_set);
        }

        // Prefer an explicit locked/active slot when ready.  The normal
        // fallback remains PRIMARY then SECONDARY; TERTIARY is deliberately
        // excluded from autonomous selection (retail Comanche rocket pods
        // declare `AutoChooseSources = TERTIARY NONE`).
        let slot = if let WeaponShotRoute::Passive { slot, .. } = route {
            let mut rof = owned_rof
                .map(|rates| rates[slot as usize])
                .unwrap_or_else(|| {
                    solo_set.map_or_else(
                        || self.weapon_bonus_fields().2,
                        |set| self.weapon_bonus_fields_with_solo_set(set).2,
                    )
                });
            if target_has_faerie_fire {
                rof *= crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
            }
            if !self.selected_weapon_ready_at_frame(slot, current_time, logic_frame, rof) {
                return None;
            }
            slot
        } else {
            let rates = owned_rof.copied().unwrap_or_else(|| {
                [solo_set.map_or_else(
                    || self.weapon_bonus_fields().2,
                    |set| self.weapon_bonus_fields_with_solo_set(set).2,
                ); 3]
            });
            let rof = |slot: usize| {
                rates[slot]
                    * if target_has_faerie_fire {
                        crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER
                    } else {
                        1.0
                    }
            };
            let primary_ready = self.selected_weapon_ready_at_frame(0, current_time, logic_frame, rof(0));
            let secondary_ready = self.selected_weapon_ready_at_frame(1, current_time, logic_frame, rof(1));
            let tertiary_ready = self.selected_weapon_ready_at_frame(2, current_time, logic_frame, rof(2));

            if self.weapon_lock_type != WeaponLockType::NotLocked {
                // The lock owns its own concrete slot identity.  Do not use
                // `active_weapon_slot` here: an asynchronous UI/writeback
                // update can change the displayed slot while a permanent
                // lock is still in effect, and must not redirect a tertiary
                // command into PRIMARY or SECONDARY.
                match self.weapon_lock_slot {
                    0 if primary_ready => 0u8,
                    1 if secondary_ready => 1u8,
                    2 if tertiary_ready => 2u8,
                    // A lock must not silently redirect an explicit manual
                    // weapon command to PRIMARY when the requested slot is
                    // unavailable or still reloading.
                    _ => return None,
                }
            } else if self.active_weapon_slot == 2 && tertiary_ready {
                // Active TERTIARY is an explicit weapon toggle, not an auto
                // candidate.  Preserve that user selection.
                2u8
            } else if self.active_weapon_slot == 1 && secondary_ready {
                1u8
            } else if primary_ready {
                0u8
            } else if secondary_ready && self.thing.template.slot_allows_auto_choose(1) {
                1u8
            } else {
                return None;
            }
        };

        if !self.selected_projectile_is_admitted(slot, content) {
            return None;
        }

        // C++ Weapon::getPreAttackDelay / PreAttackType residual.
        let pre_delay = {
            let base = self
                .weapon_slot(slot)
                .map(|w| w.pre_attack_delay.max(0.0))
                .unwrap_or(0.0);
            let bonus = match route {
                WeaponShotRoute::Passive {
                    pre_attack_bonus, ..
                } => pre_attack_bonus,
                WeaponShotRoute::Autonomous { owned_bonuses, .. } => {
                    owned_bonuses.map(|values| values[slot as usize].3)
                }
            };
            if let Some(bonus) = bonus {
                self.selected_native_weapon_rule(slot)
                    .map_or(base * bonus, |rule| {
                        ((rule.pre_attack_delay.max(0) as f32 * bonus) as u32) as f32 / 30.0
                    })
            } else {
                base * solo_set.map_or_else(
                    || self.weapon_bonus_fields().3,
                    |set| self.weapon_bonus_fields_with_solo_set(set).3,
                )
            }
        };
        let prefire = {
            let name = self.weapon_name_for_slot(slot);
            self.selected_native_weapon_rule(slot)
                .map(|rule| {
                    use crate::game_logic::weapon_bootstrap::HostPrefireType as Host;
                    use gamelogic::weapon::WeaponPrefireType as Native;
                    match rule.prefire_type {
                        Native::PrefirePerShot => Host::PerShot,
                        Native::PrefirePerAttack => Host::PerAttack,
                        Native::PrefirePerClip => Host::PerClip,
                    }
                })
                .unwrap_or_else(|| {
                    name.map(crate::game_logic::weapon_bootstrap::host_prefire_type_for_weapon_name)
                        .unwrap_or(crate::game_logic::weapon_bootstrap::HostPrefireType::PerShot)
                })
        };
        // Ground-position AI fire enters its null-victim wind-up before this
        // shared passive shot operation; do not arm it a second time here.
        let apply_delay = target_object.is_some_and(|target_id| {
            self.pre_attack_delay_applies(slot, target_id, prefire, pre_delay)
        });
        if apply_delay {
            // Arm a wind-up when:
            // - new target, or
            // - ready_at == 0 (previous shot completed / no active cycle).
            // Once armed, wait until ready_at; do NOT re-arm while ready_at is set
            // (even after it elapses) until record_shot_at_target clears it.
            let needs_arm =
                self.pre_attack_target != target_object || self.pre_attack_ready_at <= 0.0;
            if needs_arm {
                self.pre_attack_target = target_object;

                self.pre_attack_ready_at = current_time + pre_delay;
                self.weapon_fire_status = WeaponFireStatus::PreAttack;
                self.sync_weapon_model_conditions_from_status();

                // C++ Weapon::preFireWeapon LeechRange activate residual.
                self.activate_leech_range_for_slot(slot);
            }
            if current_time + 1e-6 < self.pre_attack_ready_at {
                if let Some(target_id) = target_object {
                    self.note_attack_target_with_operation(
                        target_id,
                        attack_events,
                        audio,
                        operation_frame,
                        operation_effects,
                    );
                }
                let entered_attack = !matches!(
                    self.ai_state(),
                    AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
                );
                if autonomous && entered_attack {
                    self.set_ai_state(AIState::Attacking, logic_frame, audio);
                }
                {}
                return None;
            }
            // Delay complete — fall through to fire; record_shot clears ready_at.
        } else if let Some(target_id) = target_object {
            self.pre_attack_target = Some(target_id);
        }

        if let Some(target_id) = target_object {
            self.note_attack_target_with_operation(
                target_id,
                attack_events,
                audio,
                operation_frame,
                operation_effects,
            );
        }
        let entered_attack = !matches!(
            self.ai_state(),
            AIState::Patrolling | AIState::AttackMoving | AIState::Attacking
        );
        if autonomous && entered_attack {
            self.set_ai_state(AIState::Attacking, logic_frame, audio);
        }
        {}

        let shot_rof = owned_rof.map(|rates| rates[slot as usize]).unwrap_or_else(|| {
            solo_set.map_or_else(
                || self.weapon_bonus_fields().2,
                |set| self.weapon_bonus_fields_with_solo_set(set).2,
            )
        }) * if owned_rof.is_none() && target_has_faerie_fire {
            crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER
        } else { 1.0 };
        let fire_weapon_name = self.weapon_name_for_slot(slot).map(str::to_owned);
        let name = fire_weapon_name.as_deref();
        // C++ WeaponTemplate keeps each of these references per veterancy
        // level. Capture the shooter rank before the mutable weapon-slot
        // borrow below so the launched projectile carries the exact names
        // selected for this shot.
        let veterancy = self.experience.level;
        let (fallback_damage, fallback_range, fallback_min_range) = self
            .weapon_slot(slot)
            .map(|weapon| (weapon.damage, weapon.range, weapon.min_range))
            .unwrap_or((0.0, 0.0, 0.0));
        // Prefer store PrimaryDamage so baked promotion scalars on PRIMARY
        // are not stacked again with weapon_bonus_fields veterancy.
        let native_rule = self.selected_native_weapon_rule(slot);
        let weapon_dtype = native_rule.as_ref()
            .map(|rule| crate::game_logic::host_armor_residual::map_store_damage_type(rule.damage_type))
            .or_else(|| name.and_then(crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name))?;
        let template_damage = native_rule
            .as_ref()
            .map(|rule| rule.primary_damage)
            .unwrap_or_else(|| {
                name.and_then(
                    crate::game_logic::weapon_bootstrap::host_primary_damage_for_weapon_name,
                )
                .unwrap_or(fallback_damage)
            });
        let damage_bonus = match route {
            WeaponShotRoute::Passive { damage_bonus, .. } => damage_bonus,
            WeaponShotRoute::Autonomous { owned_bonuses, .. } => {
                owned_bonuses.map(|values| values[slot as usize].0)
            }
        };
        let weapon_damage = damage_bonus.map_or_else(
            || {
                solo_set.map_or_else(
                    || self.effective_weapon_damage(template_damage),
                    |set| self.effective_weapon_damage_with_solo_set(template_damage, set),
                )
            },
            |bonus| template_damage * bonus,
        );

        // Resolve immutable Weapon.ini peels before borrowing the live slot.
        // Keeping the selected slot's name here prevents tertiary fire from
        // borrowing primary/secondary presentation and damage data.
        let projectile_object_name = self
            .selected_projectile_name(slot)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_projectile_name_for_weapon_name)
                    .unwrap_or_default()
            });
        let native_rank = match veterancy {
            crate::game_logic::VeterancyLevel::Rookie => gamelogic::common::VeterancyLevel::Regular,
            crate::game_logic::VeterancyLevel::Veteran => {
                gamelogic::common::VeterancyLevel::Veteran
            }
            crate::game_logic::VeterancyLevel::Elite => gamelogic::common::VeterancyLevel::Elite,
            crate::game_logic::VeterancyLevel::Heroic => gamelogic::common::VeterancyLevel::Heroic,
        };
        let fire_fx_name = native_rule
            .as_ref()
            .map(|rule| {
                rule.get_fire_fx(native_rank)
                    .map(|fx| fx.name().to_owned())
                    .unwrap_or_default()
            })
            .unwrap_or_else(|| {
                name.map(|weapon_name| {
                    crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
                        weapon_name,
                        veterancy,
                    )
                })
                .unwrap_or_default()
            });
        let fire_ocl_name = native_rule
            .as_ref()
            .map(|rule| {
                rule.get_fire_ocl_name(native_rank)
                    .unwrap_or_default()
                    .to_owned()
            })
            .unwrap_or_else(|| {
                name.map(|weapon_name| {
                    crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(
                        weapon_name,
                        veterancy,
                    )
                })
                .unwrap_or_default()
            });
        let detonation_fx_name = native_rule.as_ref().map(|rule| rule.get_projectile_detonate_fx(native_rank).map(|fx| fx.name().to_owned()).unwrap_or_default()).unwrap_or_else(|| name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default());
        let detonation_ocl_name = native_rule.as_ref().map(|rule| rule.get_projectile_detonation_ocl_name(native_rank).unwrap_or_default().to_owned()).unwrap_or_else(|| name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default());
        let exhaust_name = native_rule.as_ref().map(|rule| rule.get_projectile_exhaust_name(native_rank).unwrap_or_default().to_owned()).unwrap_or_else(|| name
            .map(|weapon_name| {
                crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_weapon_name_at_veterancy(
                    weapon_name,
                    veterancy,
                )
            })
            .unwrap_or_default());
        // C++ getSecondaryDamage(bonus) — WeaponBonus::DAMAGE at fire time.
        let secondary_base = native_rule
            .as_ref()
            .map(|rule| rule.secondary_damage)
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name)
                    .unwrap_or(0.0)
            });
        let secondary_damage = damage_bonus.map_or_else(
            || {
                solo_set.map_or_else(
                    || self.effective_weapon_damage(secondary_base),
                    |set| self.effective_weapon_damage_with_solo_set(secondary_base, set),
                )
            },
            |bonus| secondary_base * bonus,
        );
        let radius_bonus = match route {
            WeaponShotRoute::Autonomous { owned_bonuses, .. } => {
                owned_bonuses.map_or(1.0, |values| values[slot as usize].4)
            }
            WeaponShotRoute::Passive { .. } => 1.0,
        };
        let secondary_damage_radius = radius_bonus * native_rule.as_ref().map(|rule| rule.secondary_damage_radius).unwrap_or_else(||
            name.map(crate::game_logic::weapon_bootstrap::host_secondary_damage_radius_for_weapon_name).unwrap_or(0.0));
        let shock_wave_amount = native_rule
            .as_ref()
            .map(|rule| rule.shock_wave_amount)
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_shock_wave_amount_for_weapon_name,
                )
                .unwrap_or(0.0)
            });
        let shock_wave_radius = native_rule
            .as_ref()
            .map(|rule| rule.shock_wave_radius)
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_shock_wave_radius_for_weapon_name,
                )
                .unwrap_or(0.0)
            });
        let shock_wave_taper_off = native_rule
            .as_ref()
            .map(|rule| rule.shock_wave_taper_off)
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_shock_wave_taper_for_weapon_name)
                    .unwrap_or(0.0)
            });
        let radius_damage_affects = native_rule
            .as_ref()
            .map(|rule| rule.affects_mask.bits())
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_radius_damage_affects_for_weapon_name,
                )
                .unwrap_or(crate::game_logic::weapon_bootstrap::WEAPON_AFFECTS_DEFAULT)
            });
        let projectile_collides = native_rule
            .as_ref()
            .map(|rule| rule.collide_mask.bits())
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_projectile_collides_for_weapon_name,
                )
                .unwrap_or(crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT)
            });
        let scatter_table_offset = self.take_scatter_table_offset(slot, name);
        let scatter_radius = if scatter_table_offset.is_some() {
            0.0
        } else {
            native_rule
                .as_ref()
                .map(|rule| {
                    rule.scatter_radius
                        + if target_is_infantry {
                            rule.infantry_inaccuracy_dist
                        } else {
                            0.0
                        }
                })
                .unwrap_or_else(|| {
                    name.map(|weapon_name| {
                        crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                            weapon_name,
                            target_is_infantry,
                        )
                    })
                    .unwrap_or(0.0)
                })
        };
        let speed_peel = native_rule
            .as_ref()
            .map(
                |rule| crate::game_logic::weapon_bootstrap::HostWeaponSpeedPeel {
                    weapon_speed: rule.weapon_speed,
                    min_weapon_speed: rule.min_weapon_speed,
                    scale_weapon_speed: rule.is_scale_weapon_speed,
                    attack_range: rule.attack_range,
                    min_attack_range: rule.minimum_attack_range,
                },
            )
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name,
                )
                .unwrap_or_default()
            });
        let historic_bonus = native_rule
            .as_ref()
            .map(
                |rule| crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel {
                    time_frames: rule.historic_bonus_time,
                    count: rule.historic_bonus_count,
                    radius: rule.historic_bonus_radius.max(0.0),
                    bonus_weapon: rule.historic_bonus_weapon_name.clone(),
                },
            )
            .unwrap_or_else(|| {
                name.map(crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name)
                    .unwrap_or_default()
            });

        {
            let _ = self.capture_pending_weapon_visual_dispatch(
                slot,
                logic_frame,
                target_object,
                victim.position(),
            );
        }
        let launch_transform = if projectile_object_name.is_empty() {
            None
        } else {
            let barrel = self.pending_weapon_visual_capture.as_ref()
                .map_or(0, |capture| capture.fired_barrel);
            self.projectile_launch_transform(slot, barrel, operation_effects.pristine_bones)
        };
        // Read peels before optional ammo commit. GameLogic-owned paths defer
        // C++ Weapon.cpp2617–2669 mutations until after FireFX/FireOCL.
        let (weapon_speed, weapon_splash, weapon_homing) = match self.weapon_slot(slot) {
            Some(weapon) => (
                weapon.projectile_speed,
                weapon.splash_radius,
                // AA residual: air-only weapons home on the live target.
                weapon.can_target_air && !weapon.can_target_ground,
            ),
            None => return None,
        };
        let mut auto_reloaded_clip = false;
        if defer_post_fire_commit {
            self.pending_weapon_fire_commit = None; // replaced after pending is built
        } else {
            auto_reloaded_clip = self.commit_weapon_ammo_and_clock(
                slot, current_time, logic_frame, name, native_rule.as_deref(), shot_rof,
                owned_rof.map_or(shot_rof, |rates| rates[slot as usize]), random,
            )?;
        }
        let shooter_id = self.id;
        let shooter_pos = match route {
            WeaponShotRoute::Autonomous { .. } => self.get_position(),
            WeaponShotRoute::Passive { origin, .. } => origin,
        };
        if let Some(target_id) = target_object {
            self.target = Some(target_id);
        }

        let at_self = native_rule.as_ref().map(|rule| rule.damage_dealt_at_self_position).unwrap_or_else(||
            name.map(crate::game_logic::weapon_bootstrap::host_damage_dealt_at_self_position_for_weapon_name).unwrap_or(false));
        // One receiving CombatSystem owns projectileless travel and damage.
        // Never mirror the shot into a different Core world's WeaponStore.
        let projectile_lifecycle = content
            .and_then(|definitions| definitions.get(&projectile_object_name))
            .map(|profile| profile.lifecycle);
        let pending = super::combat::PendingProjectile {
            shooter_id,
            shooter_pos,
            source_context: Some(super::combat::ProjectileLaunchContext {
                source_team: self.team,
                projectile_team_id,
                source_owner_player_id: self.owner_player_id,
                source_veterancy: veterancy,
                source_orientation: self.get_orientation(),
                source_velocity: self.movement.velocity,
                launch_transform,
                launcher_bonus_mask: self.projectile_launch_bonus_mask(),
            }),
            // Weapon.cpp uses the intended target to determine travel time,
            // even when the eventual projectileless damage is at the source.
            target_id: target_object,
            target_pos: victim.position(),
            self_damage_position: at_self.then_some(shooter_pos),
            selected_weapon_is_laser: native_rule.as_ref().map(|rule| rule.is_laser()),
            damage: weapon_damage,
            speed: weapon_speed,
            speed_unit: super::combat::ProjectileSpeedUnit::DistancePerLogicFrame,
            splash_radius: weapon_splash * radius_bonus,
            is_homing: weapon_homing,
            damage_type: weapon_dtype,
            death_type: if let Some(rule) = native_rule.as_ref() {
                crate::game_logic::host_usa_pilot::HostDeathType::from_store(rule.death_type)
            } else if name.is_none() {
                // C++ initializes DEATH_NORMAL independently of DamageType.
                crate::game_logic::host_usa_pilot::HostDeathType::Normal
            } else {
                crate::game_logic::host_armor_residual::resolve_host_death_type(name, weapon_dtype)
            },
            projectile_object_name,
            projectile_lifecycle,
            fire_fx_name,
            fire_ocl_name,
            detonation_fx_name,
            detonation_ocl_name,
            exhaust_name,
            secondary_damage,
            secondary_damage_radius,
            shock_wave_amount,
            shock_wave_radius,
            shock_wave_taper_off,
            radius_damage_affects,
            projectile_collides,
            // C++ ScatterRadius + ScatterRadiusVsInfantry residual.
            scatter_radius,
            scatter_table_offset,
            min_weapon_speed: speed_peel.min_weapon_speed,
            scale_weapon_speed: speed_peel.scale_weapon_speed,
            attack_range: if speed_peel.attack_range > 0.0 {
                speed_peel.attack_range
            } else {
                fallback_range
            },
            min_attack_range: if speed_peel.min_attack_range > 0.0 {
                speed_peel.min_attack_range
            } else {
                fallback_min_range
            },
            historic_weapon_key: fire_weapon_name.clone().unwrap_or_default(),
            damage_status_type: native_rule
                .as_ref()
                .map(|rule| rule.damage_status_type.into()),
            historic_bonus_time_frames: historic_bonus.time_frames,
            historic_bonus_count: historic_bonus.count,
            historic_bonus_radius: historic_bonus.radius,
            historic_bonus_weapon: historic_bonus.bonus_weapon,
            die_on_detonate: native_rule
                .as_ref()
                .map(|rule| rule.die_on_detonate)
                .unwrap_or_else(|| {
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_die_on_detonate_for_weapon_name,
                    )
                    .unwrap_or(false)
                }),
        };

        // C++ fireWeaponTemplate LeechRange activate residual.
        self.activate_leech_range_for_slot(slot);
        if defer_post_fire_commit {
            // CPP privateFireWeapon commits ammo/timers only after fireWeaponTemplate.
            self.pending_weapon_fire_commit = Some(super::PendingWeaponFireCommit {
                slot,
                current_time,
                logic_frame,
                target_id: target_object,
                shot_rof,
                fire_weapon_name: fire_weapon_name.clone(),
                owned_rof: owned_rof.copied(),
            });
            return Some((slot, pending));
        }
        self.apply_weapon_fire_post_effects_commit(
            slot,
            current_time,
            logic_frame,
            target_object,
            fire_weapon_name.as_deref(),
            owned_rof,
            solo_set,
            random,
            native_rule.as_deref(),
            auto_reloaded_clip,
            true,
            sound_events,
        );
        Some((slot, pending))
    }

    /// Apply C++ Weapon.cpp2617–2669 ammo/timer/tracker commit after FireFX/FireOCL.
    pub(crate) fn commit_deferred_weapon_fire_after_effects(
        &mut self,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
        solo_set: Option<&gamelogic::weapon::WeaponBonusSet>,
        random: Option<&game_engine::common::random_value::SessionRandom>,
        reload_rates: Option<&[f32; 3]>,
    ) -> Option<(u8, Option<ObjectId>, Option<String>)> {
        let commit = self.pending_weapon_fire_commit.take()?;
        let slot = commit.slot;
        let name = commit.fire_weapon_name.as_deref();
        let native_owned = self.selected_native_weapon_rule(slot);
        let auto_reloaded_clip = self.commit_weapon_ammo_and_clock(
            slot, commit.current_time, commit.logic_frame, name,
            native_owned.as_deref(), commit.shot_rof,
            reload_rates.map_or(commit.shot_rof, |rates| rates[slot as usize]), random,
        )?;
        self.apply_weapon_fire_post_effects_commit(
            slot,
            commit.current_time,
            commit.logic_frame,
            commit.target_id,
            name,
            commit.owned_rof.as_ref(),
            solo_set,
            random,
            native_owned.as_deref(),
            auto_reloaded_clip,
            false,
            sound_events,
        );
        Some((slot, commit.target_id, commit.fire_weapon_name))
    }

    /// C++ privateFireWeapon commits the one sampled deadline after delivery.
    fn commit_weapon_ammo_and_clock(
        &mut self,
        slot: u8,
        current_time: f32,
        frame: u32,
        name: Option<&str>,
        rule: Option<&gamelogic::weapon::WeaponTemplate>,
        rof: f32,
        reload_rof: f32,
        random: Option<&game_engine::common::random_value::SessionRandom>,
    ) -> Option<bool> {
        let weapon = self.weapon_slot_mut(slot)?;
        Self::consume_ammo_on_fire_selected(weapon, current_time, name, rule);
        weapon.last_fire_frame = frame;
        let empty = weapon.ammo == Some(0) && (rule.is_none() || weapon.clip_size > 0);
        let auto_reload = Self::auto_reloaded_clip_after_firing_selected(weapon, name, rule);
        weapon.reloading_clip = empty;
        if let Some(rule) = rule {
            let deadline = if empty && !auto_reload {
                0x7fff_ffff
            } else {
                frame.wrapping_add(Self::authored_weapon_delay_frames(
                    rule, auto_reload, if auto_reload { reload_rof } else { rof }, random,
                ))
            };
            weapon.frame_clock_active = true;
            weapon.next_fire_frame = deadline;
            weapon.frame_clock_status = if auto_reload {
                WeaponFireStatus::ReloadingClip
            } else if empty {
                WeaponFireStatus::OutOfAmmo
            } else {
                WeaponFireStatus::BetweenFiringShots
            };
            if !empty || auto_reload {
                weapon.reload_started_frame = frame;
            }
            weapon.reloading_clip = auto_reload;
            weapon.last_bonus_rof = rof;
            if auto_reload {
                // reloadWithBonus refills now; getStatus exposes the wait.
                weapon.ammo = Some(weapon.clip_size);
            }
            if !empty || auto_reload {
                self.copy_authored_weapon_deadline(slot, auto_reload);
            }
            if auto_reload {
                self.rebuild_scatter_targets_unused(slot, name);
            }
        } else if empty {
            self.rebuild_scatter_targets_unused(slot, name);
        } else if let Some(name) = name {
            let rolled = match random {
                Some(random) => crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_with_rof_and_random(name, rof, random),
                None => crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_with_rof(name, rof),
            };
            if let Some(rolled) = rolled {
                let nominal = self.weapon_slot(slot)
                    .map(|weapon| self.live_reload_interval(weapon, Some(name), rof))
                    .unwrap_or(rolled);
                self.weapon_slot_mut(slot)?.last_fire_time = current_time + rolled - nominal;
            }
        }
        Some(auto_reload)
    }

    pub(super) fn authored_weapon_delay_frames(
        rule: &gamelogic::weapon::WeaponTemplate,
        reloading: bool,
        rof: f32,
        random: Option<&game_engine::common::random_value::SessionRandom>,
    ) -> u32 {
        // C++ divides directly then REAL_TO_INT_FLOOR. Nonpositive/nonfinite
        // ROF has no supported portable conversion; do not invent a .01 floor.
        assert!(rof.is_finite() && rof > 0.0, "unsupported authored weapon RATE_OF_FIRE");
        let mut bonus = gamelogic::weapon::WeaponBonus::new();
        bonus.set_field(gamelogic::weapon::WeaponBonusField::RateOfFire, rof);
        let frames = if reloading {
            rule.get_clip_reload_time(&bonus)
        } else {
            match random {
                Some(random) => rule.get_delay_between_shots_with_random(&bonus, random),
                None => rule.get_delay_between_shots(&bonus),
            }
        };
        frames as u32
    }

    fn apply_weapon_fire_post_effects_commit(
        &mut self,
        slot: u8,
        current_time: f32,
        logic_frame: u32,
        target_id: Option<ObjectId>,
        fire_weapon_name: Option<&str>,
        owned_rof: Option<&[f32; 3]>,
        solo_set: Option<&gamelogic::weapon::WeaponBonusSet>,
        _random: Option<&game_engine::common::random_value::SessionRandom>,
        native_rule: Option<&gamelogic::weapon::WeaponTemplate>,
        auto_reloaded_clip: bool,
        record_tracker: bool,
        sound_events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
    ) {
        if record_tracker {
            let possible_next_shot_frame = self.possible_next_shot_frame_for_slot_with_optional_solo_set(
                slot, logic_frame, false, solo_set, owned_rof,
            );
            self.record_shot_at_target_with_optional_solo_set(
                target_id, logic_frame, possible_next_shot_frame, solo_set, owned_rof, native_rule,
            );
        }
        // AssistingClipSize residual: count one assist fire per real shot
        // committed while TEMP-locked on the AssistingWeaponSlot.
        if self.weapon_lock_type == WeaponLockType::LockedTemporarily
            && self
                .assisted_targeting
                .is_some_and(|assist| assist.weapon_slot == slot)
        {
            self.assist_clip_fire_commits = self.assist_clip_fire_commits.saturating_add(1);
        }
        // C++ --m_maxShotCount residual.
        self.consume_max_shot_count();
        if native_rule.is_some() {
            self.refresh_weapon_fire_status_at_frame(logic_frame, owned_rof.unwrap_or(&[1.0; 3]));
        } else if let Some(rates) = owned_rof {
            self.refresh_weapon_fire_status_with_owned_rof(current_time, rates);
        } else if let Some(set) = solo_set {
            self.refresh_weapon_fire_status_with_solo_set(current_time, set);
        } else {
            self.refresh_weapon_fire_status(current_time);
        }
        // C++ Object::fireCurrentWeapon releases a temporary lock only when
        // the *locked* weapon completed an auto-reloading clip.
        if auto_reloaded_clip
            && self.weapon_lock_type == WeaponLockType::LockedTemporarily
            && self.weapon_lock_slot == slot
        {
            self.release_weapon_lock(WeaponLockType::LockedTemporarily);
        }
        if let Some(weapon) = self.weapon_slot_mut(slot) {
            weapon.last_fire_frame = logic_frame;
        }
        if record_tracker {
            let frame = logic_frame;
            self.stamp_fire_sound_loop_after_shot(frame, fire_weapon_name, sound_events);
            self.stamp_auto_reload_when_idle_from_slot(slot, frame);
        }
        {
            let (dmg, rng) = self
                .weapon_slot(slot)
                .map(|w| (w.damage, w.range))
                .unwrap_or((0.0, 0.0));
            let frame = logic_frame;
            let next_count = self.fire_intent_count.saturating_add(1);
            {
                self.last_fire_victim_host = target_id.map_or(0, |id| id.0);
                self.last_fire_slot = slot;
                self.last_fire_damage = dmg;
                self.last_fire_range = rng;
                self.last_fire_sim_time = current_time;
                self.last_fire_frame = frame;
                self.fire_intent_count = next_count;
            }
        }
    }

    pub(crate) fn move_to(
        &mut self,
        position: Vec3,
        frame: u32,
        movement_events: &mut crate::game_logic::movement_events::MovementEventWriter<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        self.move_to_inner(position, frame, movement_events, audio);
    }

    pub(crate) fn move_to_at_frame(
        &mut self,
        position: Vec3,
        frame: u32,
        movement_events: &mut crate::game_logic::movement_events::MovementEventWriter<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        self.move_to_inner(position, frame, movement_events, audio);
    }

    fn move_to_inner(
        &mut self,
        position: Vec3,
        frame: u32,
        movement_events: &mut crate::game_logic::movement_events::MovementEventWriter<'_>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        if self.is_mobile() && self.is_alive() {
            self.movement_goal_object = None;
            self.movement.target_position = Some(position);
            self.set_ai_state(AIState::Moving, frame, audio);
            self.set_status_moving(true);
            movement_events.record(self.id, Some([position.x, position.y, position.z]));
        }
    }
    /// Drop a `setTemporaryState` move without `onExit` of the underlying state.
    pub fn end_temporary_move_overlay(&mut self, audio: &mut crate::game_logic::ObjectAudio<'_>) {
        let Some(previous_temporary) = self.unit_ai_runtime.take_temporary_move() else {
            return;
        };
        audio.stop_loop(self);
        self.movement.velocity = glam::Vec3::ZERO;
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.movement.target_position = None;
        self.set_status_moving(false);
        self.queue_for_path_frame = 0;
        self.waiting_for_path = false;
        self.pending_quick_path_goal = false;
        // None is meaningful: do not leave the temporary goal installed when
        // the underlying machine never had a physical destination.
        self.requested_destination = previous_temporary.saved_destination;
        self.path_goal_position = previous_temporary.saved_path_goal;
    }

    pub(crate) fn movement_goal_object(&self) -> Option<ObjectId> {
        self.movement_goal_object
    }

    pub(crate) fn set_movement_goal_object(&mut self, goal: Option<ObjectId>) {
        self.movement_goal_object = goal;
    }

    pub fn stop_moving(&mut self, audio: &mut crate::game_logic::ObjectAudio<'_>) {
        self.stop_moving_with_idle_transition(audio);
    }

    /// Stop locomotion and report whether a normal move leaf reached its
    /// terminal point. The World owns any resulting AI transition.
    pub(crate) fn stop_moving_for_owned_idle_entry(
        &mut self,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        self.stop_moving_with_idle_transition(audio)
    }

    fn stop_moving_with_idle_transition(
        &mut self,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) -> bool {
        let overlay = self.has_temporary_ai_move();
        self.end_temporary_move_overlay(audio);
        if overlay {
            return false;
        }
        self.movement_goal_object = None;
        audio.stop_loop(self);
        self.movement.target_position = None;
        self.movement.velocity = Vec3::ZERO;
        self.set_status_moving(false);
        self.queue_for_path_frame = 0;
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.set_status_moving(false);
        self.requested_destination = None;
        self.waiting_for_path = false;
        self.pending_quick_path_goal = false;
        self.is_attack_path = false;
        self.is_approach_path = false;
        self.is_safe_path = false;
        self.path_extra_distance = 0.0;
        if self.is_kind_of(crate::game_logic::KindOf::Projectile) {
            self.set_precise_z_pos(false);
        }

        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        // Physical movement can stop without completing the current AI state:
        // the C++ AI state machine, not Object locomotion, owns that transition.
        !matches!(
            self.retained_ai_root_state(),
            AIState::JetReturningForLanding | AIState::JetReturnToDeadAirfield
        ) && matches!(self.ai_state(), AIState::Moving | AIState::AttackMoving)
    }

    pub fn attack_target(
        &mut self,
        target_id: ObjectId,
        frame: u32,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        self.attack_target_with_operation(
            target_id,
            attack_events,
            audio,
            frame,
            &mut Default::default(),
        )
    }

    pub fn attack_target_with_operation(
        &mut self,
        target_id: ObjectId,
        attack_events: &mut crate::game_logic::host_attack_log::AttackEvents,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
        operation_frame: u32,
        operation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if !self.is_alive() {
            return;
        }
        // Shock stun residual: ignore new attack orders while stunned.
        if self.is_shock_stunned() {
            return;
        }
        // Jet takeoff residual: leave hangar before engaging.
        let _ = self.takeoff_from_airfield_parking_with_operation(
            audio,
            operation_frame,
            operation_effects,
        );
        if self.can_attack() {
            if self.pre_attack_target != Some(target_id) {
                // New target — fire_at will start PRE_ATTACK clock.
                self.pre_attack_target = None;

                self.pre_attack_ready_at = 0.0;
            }
            self.target = Some(target_id);
            self.target_location = None;
            self.set_status_force_attack(false);
            self.set_ai_state(AIState::Attacking, operation_frame, audio);
            self.status.attacking = true;
            attack_events.record(self.id, target_id);
        }
    }

    /// C++ Weapon::setLeechRangeActive residual for a weapon slot.

    /// C++ FiringTracker::shotFired FireSoundLoopTime residual.
    /// Extends the looping fire-audio deadline; records start when newly armed.
    pub fn stamp_fire_sound_loop_after_shot(
        &mut self,
        frame: u32,
        weapon_name: Option<&str>,
        events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
    ) {
        let native = (0..3).find_map(|slot| {
            let rule = self.selected_native_weapon_rule(slot)?;
            weapon_name
                .is_some_and(|name| rule.name.eq_ignore_ascii_case(name))
                .then_some(rule)
        });
        let loop_frames = native.as_ref().map(|rule| rule.fire_sound_loop_time).unwrap_or_else(||
            weapon_name.map(crate::game_logic::weapon_bootstrap::host_fire_sound_loop_frames_for_weapon_name).unwrap_or(0));
        if loop_frames == 0 {
            return;
        }
        let sound = native
            .as_ref()
            .map(|rule| rule.fire_sound.name().to_owned())
            .unwrap_or_else(|| {
                weapon_name
                    .map(crate::game_logic::weapon_bootstrap::host_fire_sound_for_weapon_name)
                    .unwrap_or_default()
            });
        if sound.is_empty() {
            return;
        }
        let was_active = self.fire_sound_loop_until_frame > frame;
        self.fire_sound_loop_until_frame = frame.wrapping_add(loop_frames);
        self.fire_sound_loop_name = sound.clone();
        if !was_active {
            events.record(self.id, sound, true);
        }
    }

    /// C++ FiringTracker::update stop-loop residual when deadline elapses.
    pub fn tick_fire_sound_loop(
        &mut self,
        frame: u32,
        events: &mut crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents,
    ) {
        if self.fire_sound_loop_until_frame == 0 {
            return;
        }
        if frame >= self.fire_sound_loop_until_frame {
            let sound = std::mem::take(&mut self.fire_sound_loop_name);
            self.fire_sound_loop_until_frame = 0;
            if !sound.is_empty() {
                events.record(self.id, sound, false);
            }
        }
    }

    pub fn activate_leech_range_for_slot(&mut self, slot: u8) {
        let name = self.weapon_name_for_slot(slot);
        let is_leech = self
            .selected_native_weapon_rule(slot)
            .map(|rule| rule.leech_range_weapon)
            .unwrap_or_else(|| {
                name.map(
                    crate::game_logic::weapon_bootstrap::host_leech_range_weapon_for_weapon_name,
                )
                .unwrap_or(false)
            });
        if !is_leech {
            return;
        }
        match slot {
            0 => {
                self.leech_range_active_primary = true;
            }
            1 => {
                self.leech_range_active_secondary = true;
            }
            // The host only has persisted leech flags for A/B.  Do not alias
            // a tertiary leech weapon to primary; it would alter a different
            // slot's targeting range.
            _ => {}
        }
    }

    /// C++ Object::clearLeechRangeModeForAllWeapons residual.
    pub fn clear_leech_range_mode_for_all_weapons(&mut self) {
        self.leech_range_active_primary = false;
        self.leech_range_active_secondary = false;
    }

    fn clear_attack_scalar_state(&mut self) {
        // C++ WeaponSet::releaseWeaponLock: any non-special player order and
        // an exited attack state release a temporary special-weapon lock.
        // Permanent user locks remain intact.
        self.release_weapon_lock(WeaponLockType::LockedTemporarily);
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.pre_attack_target = None;
        self.pre_attack_ready_at = 0.0;
        self.consecutive_shot_target = None;
        self.consecutive_shots_at_target = 0;
        self.clear_leech_range_mode_for_all_weapons();
        self.status.attacking = false;
        self.set_status_aiming_weapon(false);
        self.set_status_firing_weapon(false);
        self.set_attack_substate(crate::game_logic::AttackSubState::AimAtTarget);
    }

    /// Clear combat engagement data without changing the retained AI machine.
    /// The owning World applies any state-machine result at its observation point.
    pub fn stop_attack(&mut self, _audio: &mut crate::game_logic::ObjectAudio<'_>) {
        self.clear_attack_scalar_state();
    }

    pub fn clear_all_occupants(&mut self) {
        if let Some(building) = self.building_data.as_mut() {
            building.garrisoned_units.clear();
        }
        self.occupants.clear();
    }
    /// DelayBetweenShots or ClipReloadTime, both scaled by RATE_OF_FIRE.
    ///
    /// Template delay is preferred so baked PRIMARY reload scalars are not
    /// stacked with `weapon_bonus_fields` veterancy. Ready-checks use leftover
    /// `get_delay_between_shots` on the Min yardstick (no GameLogicRandomValue);
    /// per-shot leftover roll + ROF floor is applied at fire (C++
    /// `privateFireWeapon` / leftover `get_delay_between_shots`).
    pub(super) fn live_reload_interval(
        &self,
        weapon: &Weapon,
        weapon_name: Option<&str>,
        rof: f32,
    ) -> f32 {
        // A supplied immutable row is authoritative even when its delay is zero.
        // Readiness uses the minimum yardstick; only accepted fire draws the range.
        let native = (0..3).find_map(|slot| {
            let rule = self.selected_native_weapon_rule(slot)?;
            weapon_name
                .is_some_and(|name| rule.name.eq_ignore_ascii_case(name))
                .then_some(rule)
        });
        if let Some(rule) = native {
            let waiting_clip = weapon.reloading_clip
                || (rule.get_auto_reloads_clip() && weapon.clip_size > 0 && weapon.ammo == Some(0));
            if waiting_clip {
                // WeaponTemplate::getClipReloadTime floors the bonused delay
                // in logic frames before it becomes a deadline.
                return (rule.clip_reload_time as f32 / rof).floor() / 30.0;
            }
            return (rule.min_delay_between_shots as f32 / rof).floor() / 30.0;
        }
        let auto_clip = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_reload_type_for_weapon_name)
            .unwrap_or(crate::game_logic::weapon_bootstrap::HostReloadType::Auto)
            == crate::game_logic::weapon_bootstrap::HostReloadType::Auto;
        let waiting_clip =
            weapon.reloading_clip || (auto_clip && weapon.clip_size > 0 && weapon.ammo == Some(0));
        if waiting_clip {
            return (weapon.clip_reload_time / rof.max(0.01)).max(0.0);
        }
        if let Some(name) = weapon_name {
            if let Some(secs) =
                crate::game_logic::weapon_bootstrap::host_delay_between_shots_secs_nominal_with_rof(
                    name, rof,
                )
            {
                return secs;
            }
        }
        // Fallback: leftover ROF floor on the baked yardstick (seconds → frames).
        let frames = (weapon.reload_time * 30.0).round();
        ((frames / rof.max(0.01)).floor() / 30.0).max(0.0)
    }

    /// C++ `Weapon::rebuildScatterTargets` — refill unused indices for this clip.
    pub(crate) fn rebuild_scatter_targets_unused(&mut self, slot: u8, weapon_name: Option<&str>) {
        let index = usize::from(slot);
        if index >= 3 {
            return;
        }
        let count = self
            .selected_native_weapon_rule(slot)
            .map(|rule| rule.scatter_targets.len())
            .unwrap_or_else(|| {
                weapon_name
                    .map(crate::game_logic::weapon_bootstrap::host_scatter_targets_for_weapon_name)
                    .map(|targets| targets.len())
                    .unwrap_or(0)
            });
        self.weapon_scatter_targets_unused[index] = (0..count)
            .filter_map(|idx| i32::try_from(idx).ok())
            .collect();
        self.weapon_scatter_targets_inited[index] = true;
    }

    /// C++ Weapon.cpp:2584-2609 — one unused ScatterTarget per shot.
    pub(crate) fn take_scatter_table_offset(
        &mut self,
        slot: u8,
        weapon_name: Option<&str>,
    ) -> Option<glam::Vec2> {
        let name = weapon_name?;
        let native = self.selected_native_weapon_rule(slot);
        let targets = native
            .as_ref()
            .map(|rule| {
                rule.scatter_targets
                    .iter()
                    .map(|point| (point.x, point.y))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                crate::game_logic::weapon_bootstrap::host_scatter_targets_for_weapon_name(name)
            });
        if targets.is_empty() {
            return None;
        }
        let index = usize::from(slot);
        if index >= 3 {
            return None;
        }
        if !self.weapon_scatter_targets_inited[index] {
            self.rebuild_scatter_targets_unused(slot, Some(name));
        }
        if self.weapon_scatter_targets_unused[index].is_empty() {
            return None;
        }
        let last = self.weapon_scatter_targets_unused[index].len() as i32 - 1;
        let seed = self.id.0.wrapping_add(self.last_fire_frame);
        let pick =
            crate::game_logic::host_rng_residual::pure_logic_random_int(seed, 0, 0, last) as usize;
        let target_index = self.weapon_scatter_targets_unused[index][pick] as usize;
        let (offset_x, offset_y) = *targets.get(target_index)?;
        let scalar = native
            .as_ref()
            .map(|rule| rule.scatter_target_scalar)
            .unwrap_or_else(|| {
                crate::game_logic::weapon_bootstrap::host_scatter_target_scalar_for_weapon_name(
                    name,
                )
            });
        self.weapon_scatter_targets_unused[index].swap_remove(pick);
        Some(glam::Vec2::new(offset_x * scalar, offset_y * scalar))
    }
}

#[cfg(test)]
#[path = "projectileless_fire_tests.rs"]
mod projectileless_fire_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_tertiary_fire_uses_the_real_third_slot() {
        let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

        let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
        let mut attacker = Object::new(
            ThingTemplate::new("ThreeSlotAttacker"),
            ObjectId(1),
            Team::USA,
        );
        crate::fixture_content::arm_slot_with_stats(
            &mut attacker,
            0,
            Weapon {
                damage: 5.0,
                range: 100.0,
                ..Weapon::default()
            },
        );
        crate::fixture_content::arm_slot_with_stats(
            &mut attacker,
            2,
            Weapon {
                damage: 37.0,
                range: 250.0,
                last_fire_time: -10.0,
                ..Weapon::default()
            },
        );
        assert!(attacker.set_weapon_lock(2, WeaponLockType::LockedPermanently));

        // Simulate a stale displayed active slot.  The stored weapon lock is
        // authoritative and must retain tertiary identity while firing.
        attacker.set_active_weapon_slot(0);

        assert!(attacker.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
        ));
        assert_eq!(attacker.last_fire_slot, 2);
        assert!((attacker.last_fire_damage - 37.0).abs() < f32::EPSILON);
        assert_eq!(
            attacker
                .tertiary_weapon
                .as_ref()
                .map(|weapon| weapon.last_fire_time),
            Some(1.0)
        );
        assert_eq!(
            attacker.weapon.as_ref().map(|weapon| weapon.last_fire_time),
            Some(0.0),
            "primary must not be consumed by an explicit tertiary fire"
        );
    }

    #[test]
    fn fire_at_rejects_sold_and_under_construction() {
        let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

        let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
        let mut unit = Object::new(ThingTemplate::new("PatriotBattery"), ObjectId(1), Team::USA);
        crate::fixture_content::arm_slot_with_stats(
            &mut unit,
            0,
            Weapon {
                damage: 20.0,
                range: 150.0,
                last_fire_time: -10.0,
                ..Weapon::default()
            },
        );
        unit.status.under_construction = true;
        assert!(!unit.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
        ));
        unit.status.under_construction = false;
        unit.status.sold = true;
        assert!(!unit.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
        ));
        unit.status.sold = false;
        assert!(unit.fire_at(
            ObjectId(2),
            1.0,
            0,
            &mut shot_combat,
            &mut _attack_output,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
        ));
    }

    #[test]
    fn held_autonomous_weapon_fires_but_other_disabled_types_still_block() {
        fn fire(held: bool, emp: bool, contained: bool) -> (bool, bool) {
            let mut object = Object::new(ThingTemplate::new("HeldShooter"), ObjectId(1), Team::USA);
            crate::fixture_content::arm_slot_with_stats(
                &mut object,
                0,
                Weapon {
                    damage: 20.0,
                    range: 150.0,
                    last_fire_time: -10.0,
                    ..Weapon::default()
                },
            );
            object.status.disabled_held = held;
            object.status.disabled_emp = emp;
            object.contained_by = contained.then_some(ObjectId(3));
            let capable = object.can_attack();
            let fired = object.fire_at(
                ObjectId(2),
                1.0,
                0,
                &mut crate::game_logic::combat::CombatSystem::new(),
                &mut crate::game_logic::host_attack_log::AttackEvents::default(),
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            (capable, fired)
        }
        assert_eq!(fire(false, false, false), (true, true));
        assert_eq!(fire(true, false, false), (true, true));
        assert_eq!(fire(true, true, false), (false, false));
        assert_eq!(fire(true, false, true), (true, false));
    }

    #[test]
    fn unarmed_victims_use_their_own_targetability_not_can_attack() {
        let id = ObjectId(17);
        let mut worker = Object::new(ThingTemplate::new("Worker"), id, Team::GLA);
        let mut peer = Object::new(ThingTemplate::new("Worker"), id, Team::GLA);
        assert!(!worker.is_kind_of(KindOf::Attackable));
        assert!(worker.is_targetable_by_enemy_of(Team::USA));
        assert!(peer.is_targetable_by_enemy_of(Team::USA));

        worker.status.masked = true;
        assert!(!worker.is_targetable_by_enemy_of(Team::USA));
        assert!(peer.is_targetable_by_enemy_of(Team::USA));
        worker.status.masked = false;
        worker.status.destroyed = true;
        assert!(!worker.is_targetable_by_enemy_of(Team::USA));
        assert!(peer.is_targetable_by_enemy_of(Team::USA));

        let mut protected_template = ThingTemplate::new("Protected");
        protected_template.add_kind_of(KindOf::Unattackable);
        peer = Object::new(protected_template, id, Team::GLA);
        assert!(!peer.is_targetable_by_enemy_of(Team::USA));
    }
}

#[cfg(test)]
#[path = "passive_shot_tests.rs"]
mod passive_shot_tests;
