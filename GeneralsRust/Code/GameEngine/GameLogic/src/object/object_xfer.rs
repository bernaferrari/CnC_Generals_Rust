//! Snapshot / xfer helpers and [`Snapshot`] impl for [`Object`].

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

fn xfer_matrix3d(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) {
    let mut cols = matrix.to_cols_array();
    for value in &mut cols {
        let _ = xfer.xfer_real(value);
    }
    *matrix = Matrix3D::from_cols_array(&cols);
}

/// C++ `Object::crc` → `xferSnapshot(thisWeapon)` → `Weapon::crc`.
fn xfer_weapon_crc_like_cpp(xfer: &mut dyn Xfer, weapon: &crate::weapon::Weapon) {
    let snap = weapon.crc_snapshot_fields();
    let mut name = snap.template_name;
    let _ = xfer.xfer_ascii_string(&mut name);

    let mut wslot = snap.wslot;
    unsafe {
        let _ = xfer.xfer_user(
            (&mut wslot as *mut i32).cast::<u8>(),
            std::mem::size_of::<i32>(),
        );
    }

    let mut ammo = snap.ammo_in_clip;
    let _ = xfer.xfer_unsigned_int(&mut ammo);
    let mut when_fire = snap.when_we_can_fire_again;
    let _ = xfer.xfer_unsigned_int(&mut when_fire);
    let mut when_pre = snap.when_pre_attack_finished;
    let _ = xfer.xfer_unsigned_int(&mut when_pre);
    let mut when_reload = snap.when_last_reload_started;
    let _ = xfer.xfer_unsigned_int(&mut when_reload);
    let mut last_fire = snap.last_fire_frame;
    let _ = xfer.xfer_unsigned_int(&mut last_fire);

    let mut stream_id = snap.projectile_stream_id;
    let _ = xfer.xfer_object_id(&mut stream_id);
    let mut laser_id_unused: ObjectID = crate::common::INVALID_ID;
    let _ = xfer.xfer_object_id(&mut laser_id_unused);

    let mut max_shots = snap.max_shot_count;
    let _ = xfer.xfer_int(&mut max_shots);
    let mut cur_barrel = snap.cur_barrel;
    let _ = xfer.xfer_int(&mut cur_barrel);
    let mut shots_for_barrel = snap.num_shots_for_cur_barrel;
    let _ = xfer.xfer_int(&mut shots_for_barrel);

    let mut scatter_count = snap.scatter_targets_unused.len() as u16;
    let _ = xfer.xfer_unsigned_short(&mut scatter_count);
    for target in &snap.scatter_targets_unused {
        let mut int_data = *target;
        let _ = xfer.xfer_int(&mut int_data);
    }

    let mut pitch_limited = snap.pitch_limited;
    let _ = xfer.xfer_bool(&mut pitch_limited);
    let mut leech = snap.leech_weapon_range_active;
    let _ = xfer.xfer_bool(&mut leech);
}

/// C++ `xferUser((Matrix3D *)getTransformMatrix(), sizeof(Matrix3D))`.
/// WWMath `Matrix3D` is 3×4 Reals (48 bytes), row-major.
fn xfer_matrix3d_user_blob(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) {
    const CPP_MATRIX3D_FLOATS: usize = 12;
    let cols = matrix.to_cols_array();
    let mut blob = [0f32; CPP_MATRIX3D_FLOATS];
    for row in 0..3 {
        for col in 0..4 {
            blob[row * 4 + col] = cols[col * 4 + row];
        }
    }
    unsafe {
        let _ = xfer.xfer_user(
            blob.as_mut_ptr().cast::<u8>(),
            CPP_MATRIX3D_FLOATS * std::mem::size_of::<f32>(),
        );
    }
    let mut cols = [0f32; 16];
    for row in 0..3 {
        for col in 0..4 {
            cols[col * 4 + row] = blob[row * 4 + col];
        }
    }
    cols[15] = 1.0;
    *matrix = Matrix3D::from_cols_array(&cols);
}

fn xfer_u128_bits(xfer: &mut dyn Xfer, value: &mut u128) {
    let mut lo = (*value & 0xFFFF_FFFF_FFFF_FFFF) as u64;
    let mut hi = (*value >> 64) as u64;
    if let Err(err) = xfer.xfer_u64(&mut lo) {
        panic!("Object xfer_u128_bits failed (lo): {err}");
    }
    if let Err(err) = xfer.xfer_u64(&mut hi) {
        panic!("Object xfer_u128_bits failed (hi): {err}");
    }
    *value = ((hi as u128) << 64) | (lo as u128);
}

fn xfer_coord3d_values(xfer: &mut dyn Xfer, value: &mut Coord3D) {
    let _ = xfer.xfer_real(&mut value.x);
    let _ = xfer.xfer_real(&mut value.y);
    let _ = xfer.xfer_real(&mut value.z);
}

fn xfer_sighting_info(xfer: &mut dyn Xfer, sighting: &mut SightingInfo) {
    xfer_coord3d_values(xfer, &mut sighting.where_pos);
    let _ = xfer.xfer_real(&mut sighting.how_far);
    let mut for_whom = sighting.for_whom.bits();
    let _ = xfer.xfer_unsigned_int(&mut for_whom);
    sighting.for_whom = PlayerMaskType::from_bits_retain(for_whom);
    let _ = xfer.xfer_unsigned_int(&mut sighting.data);
}

fn xfer_coord2d_values(xfer: &mut dyn Xfer, value: &mut Coord2D) {
    let _ = xfer.xfer_real(&mut value.x);
    let _ = xfer.xfer_real(&mut value.y);
}

fn xfer_color_rgba(xfer: &mut dyn Xfer, value: &mut Color) {
    let mut packed = ((value.a as u32) << 24)
        | ((value.b as u32) << 16)
        | ((value.g as u32) << 8)
        | (value.r as u32);
    let _ = xfer.xfer_unsigned_int(&mut packed);
    value.r = (packed & 0xFF) as u8;
    value.g = ((packed >> 8) & 0xFF) as u8;
    value.b = ((packed >> 16) & 0xFF) as u8;
    value.a = ((packed >> 24) & 0xFF) as u8;
}

fn weapon_set_flags_to_bits(flags: WeaponSetFlags) -> u32 {
    let mut bits = 0u32;
    const TYPES: [WeaponSetType; 17] = [
        WeaponSetType::Veteran,
        WeaponSetType::Elite,
        WeaponSetType::Hero,
        WeaponSetType::PlayerUpgrade,
        WeaponSetType::CrateUpgradeOne,
        WeaponSetType::CrateUpgradeTwo,
        WeaponSetType::VehicleHijack,
        WeaponSetType::CarBomb,
        WeaponSetType::MineClearingDetail,
        WeaponSetType::WeaponRider1,
        WeaponSetType::WeaponRider2,
        WeaponSetType::WeaponRider3,
        WeaponSetType::WeaponRider4,
        WeaponSetType::WeaponRider5,
        WeaponSetType::WeaponRider6,
        WeaponSetType::WeaponRider7,
        WeaponSetType::WeaponRider8,
    ];
    for kind in TYPES {
        if flags.test(kind) {
            bits |= 1u32 << (kind as u32);
        }
    }
    bits
}

fn weapon_set_flags_from_bits(bits: u32) -> WeaponSetFlags {
    let mut flags = WeaponSetFlags::new();
    const TYPES: [WeaponSetType; 17] = [
        WeaponSetType::Veteran,
        WeaponSetType::Elite,
        WeaponSetType::Hero,
        WeaponSetType::PlayerUpgrade,
        WeaponSetType::CrateUpgradeOne,
        WeaponSetType::CrateUpgradeTwo,
        WeaponSetType::VehicleHijack,
        WeaponSetType::CarBomb,
        WeaponSetType::MineClearingDetail,
        WeaponSetType::WeaponRider1,
        WeaponSetType::WeaponRider2,
        WeaponSetType::WeaponRider3,
        WeaponSetType::WeaponRider4,
        WeaponSetType::WeaponRider5,
        WeaponSetType::WeaponRider6,
        WeaponSetType::WeaponRider7,
        WeaponSetType::WeaponRider8,
    ];
    for kind in TYPES {
        if (bits & (1u32 << (kind as u32))) != 0 {
            flags.set(kind);
        }
    }
    flags
}

// Implement Snapshot trait for Object
impl Snapshot for Object {
    fn crc(&self, xfer: &mut dyn Xfer) {
        // C++ Object::crc (Object.cpp): privateStatus, mtx, id, upgrades Int64,
        // experienceTracker snapshot, health, weaponBonus, damageScalar, weapons.
        let mut private_status = self.private_status;
        let _ = xfer.xfer_unsigned_byte(&mut private_status);

        let mut transform = self.get_transform_matrix();
        xfer_matrix3d_user_blob(xfer, &mut transform);

        // C++ xferUser(&m_id, sizeof(m_id)); ObjectID is UnsignedInt.
        let mut id = self.id;
        unsafe {
            let _ = xfer.xfer_user(
                (&mut id as *mut ObjectID).cast::<u8>(),
                std::mem::size_of::<ObjectID>(),
            );
        }

        // C++ xferUser(&m_objectUpgradesCompleted, sizeof(Int64)).
        let mut upgrades = (self.object_upgrades_completed.bits() & 0xFFFF_FFFF_FFFF_FFFF) as i64;
        unsafe {
            let _ = xfer.xfer_user(
                (&mut upgrades as *mut i64).cast::<u8>(),
                std::mem::size_of::<i64>(),
            );
        }

        // C++ ExperienceTracker::crc: xferInt(xp) + xferUser(level, sizeof(VeterancyLevel)).
        if let Some(tracker) = &self.experience_tracker {
            if let Ok(guard) = tracker.lock() {
                let mut xp = guard.get_current_experience();
                let _ = xfer.xfer_int(&mut xp);
                let mut level = guard.get_veterancy_level() as i32;
                unsafe {
                    let _ = xfer.xfer_user(
                        (&mut level as *mut i32).cast::<u8>(),
                        std::mem::size_of::<i32>(),
                    );
                }
            }
        }

        // C++ always xfers health, weaponBonus, damageScalar (body is required).
        let mut health = if let Some(body) = &self.body {
            body.lock().map(|g| g.get_health()).unwrap_or(0.0)
        } else {
            0.0
        };
        unsafe {
            let _ = xfer.xfer_user(
                (&mut health as *mut f32).cast::<u8>(),
                std::mem::size_of::<f32>(),
            );
        }

        let mut weapon_bonus_condition = self.weapon_bonus_condition.bits();
        let _ = xfer.xfer_unsigned_int(&mut weapon_bonus_condition);

        let mut damage_scalar = if let Some(body) = &self.body {
            body.lock().map(|g| g.get_damage_scalar()).unwrap_or(1.0)
        } else {
            1.0
        };
        unsafe {
            let _ = xfer.xfer_user(
                (&mut damage_scalar as *mut f32).cast::<u8>(),
                std::mem::size_of::<f32>(),
            );
        }

        for slot in 0..WEAPONSLOT_COUNT {
            let slot_ty = match slot {
                0 => crate::weapon::WeaponSlotType::Primary,
                1 => crate::weapon::WeaponSlotType::Secondary,
                _ => crate::weapon::WeaponSlotType::Tertiary,
            };
            if let Some(weapon) = self.get_weapon_in_weapon_slot(slot_ty) {
                xfer_weapon_crc_like_cpp(xfer, weapon);
            }
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let current_version: u8 = 9;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);

        let xfer_mode = xfer.get_xfer_mode();
        let is_loading = xfer_mode == game_engine::system::XferMode::Load;
        let is_saving = matches!(
            xfer_mode,
            game_engine::system::XferMode::Save | game_engine::system::XferMode::Crc
        );

        let mut id = self.get_id();
        let _ = xfer.xfer_unsigned_int(&mut id);
        self.set_id(id);

        let mut transform = self.get_transform_matrix();
        xfer_matrix3d(xfer, &mut transform);
        self.set_transform_matrix(&transform);

        let mut team_id = self.get_team_id().unwrap_or(crate::team::TEAM_ID_INVALID);
        let _ = xfer.xfer_unsigned_int(&mut team_id);

        let _ = xfer.xfer_unsigned_int(&mut self.producer_id);
        let _ = xfer.xfer_unsigned_int(&mut self.builder_id);

        let mut drawable_id = self
            .drawable
            .as_ref()
            .and_then(|drawable| drawable.read().ok().map(|guard| guard.get_drawable_id()))
            .unwrap_or(INVALID_ID);
        let _ = xfer.xfer_unsigned_int(&mut drawable_id);
        if is_loading {
            if let Some(drawable) = &self.drawable {
                if let Ok(mut drawable_guard) = drawable.write() {
                    drawable_guard.set_drawable_id(drawable_id);
                }
            }
        }

        let mut name = self.name.to_string();
        let _ = xfer.xfer_ascii_string(&mut name);
        if is_loading {
            self.name = AsciiString::from(name.as_str());
        }

        if version >= 8 {
            let mut status_bits = self.status.bits();
            let _ = xfer.xfer_u64(&mut status_bits);
            self.status = ObjectStatusMaskType::from_bits_retain(status_bits);
        } else {
            let mut old_status: u32 = self.status.bits() as u32;
            let _ = xfer.xfer_unsigned_int(&mut old_status);
            if is_loading {
                self.status = ObjectStatusMaskType::from_bits_retain(old_status as u64);
            }
        }

        let _ = xfer.xfer_unsigned_byte(&mut self.script_status);
        let _ = xfer.xfer_unsigned_byte(&mut self.private_status);

        if is_loading {
            if let Ok(factory) = crate::team::get_team_factory().lock() {
                let restored_team = factory.find_team_by_id(team_id);
                if let Err(err) = self.set_or_restore_team(restored_team, true) {
                    warn!(
                        "Object::xfer failed to restore team for object {}: {}",
                        self.id, err
                    );
                }
            }
        }

        xfer_coord3d_values(xfer, &mut self.geometry_info.position);
        let _ = xfer.xfer_real(&mut self.geometry_info.angle);
        xfer_coord3d_values(xfer, &mut self.geometry_info.bounds.min);
        xfer_coord3d_values(xfer, &mut self.geometry_info.bounds.max);
        let _ = xfer.xfer_real(&mut self.geometry_info.height_above_terrain);

        xfer_sighting_info(xfer, &mut self.partition_last_look);
        if version >= 9 {
            xfer_sighting_info(xfer, &mut self.partition_reveal_all_last_look);
        } else if is_loading {
            self.partition_reveal_all_last_look.reset();
        }
        xfer_sighting_info(xfer, &mut self.partition_last_shroud);

        let mut vision_spied_mask = self.vision_spied_mask.bits();
        for value in &mut self.vision_spied_by {
            let _ = xfer.xfer_int(value);
        }
        let _ = xfer.xfer_unsigned_int(&mut vision_spied_mask);
        self.vision_spied_mask = PlayerMaskType::from_bits_retain(vision_spied_mask);

        let _ = xfer.xfer_real(&mut self.vision_range);
        let _ = xfer.xfer_real(&mut self.shroud_clearing_range);
        let _ = xfer.xfer_real(&mut self.shroud_range);

        let mut disabled_mask_bits = self.disabled_mask.bits();
        let _ = xfer.xfer_unsigned_int(&mut disabled_mask_bits);
        self.disabled_mask = DisabledMaskType::from_bits_retain(disabled_mask_bits);

        if is_saving || version >= 2 {
            let _ = xfer.xfer_bool(&mut self.single_use_command_used);
        } else {
            self.single_use_command_used = false;
        }

        for frame in &mut self.disabled_till_frame {
            let _ = xfer.xfer_unsigned_int(frame);
        }

        let _ = xfer.xfer_unsigned_int(&mut self.smc_until);

        if self.experience_tracker.is_none() {
            self.experience_tracker = Some(Arc::new(Mutex::new(ExperienceTracker::new(self.id))));
        }
        if let Some(tracker) = &self.experience_tracker {
            if let Ok(mut tracker_guard) = tracker.lock() {
                if let Err(err) = tracker_guard.xfer_state(xfer) {
                    warn!(
                        "Object::xfer failed for experience tracker on object {}: {}",
                        self.id, err
                    );
                }
            } else {
                warn!(
                    "Object::xfer could not lock experience tracker for object {}",
                    self.id
                );
            }
        }

        if version >= 6 {
            let mut contained_by_id = self.contained_by_id;
            let _ = xfer.xfer_unsigned_int(&mut contained_by_id);
            if !is_saving {
                self.contained_by_id = contained_by_id;
            }
        }

        let _ = xfer.xfer_unsigned_int(&mut self.contained_by_frame);
        let _ = xfer.xfer_real(&mut self.construction_percent);

        let mut upgrade_mask_bits = self.object_upgrades_completed.bits();
        xfer_u128_bits(xfer, &mut upgrade_mask_bits);
        self.object_upgrades_completed = UpgradeMaskType::from_bits_retain(upgrade_mask_bits);

        let mut original_team_name = self.original_team_name.to_string();
        let _ = xfer.xfer_ascii_string(&mut original_team_name);
        if is_loading {
            self.original_team_name = AsciiString::from(original_team_name.as_str());
        }

        xfer_color_rgba(xfer, &mut self.indicator_color);
        xfer_coord3d_values(xfer, &mut self.health_box_offset);

        let _ = xfer.xfer_unsigned_byte(&mut self.num_trigger_areas_active);
        let _ = xfer.xfer_unsigned_int(&mut self.entered_or_exited_frame);
        let _ = xfer.xfer_int(&mut self.i_pos.x);
        let _ = xfer.xfer_int(&mut self.i_pos.y);
        let _ = xfer.xfer_int(&mut self.i_pos.z);

        let trigger_count = (self.num_trigger_areas_active as usize).min(MAX_TRIGGER_AREA_INFOS);
        for i in 0..trigger_count {
            let mut trigger_name = self.trigger_info[i]
                .trigger
                .as_ref()
                .map(|trigger| trigger.get_trigger_name().to_string())
                .unwrap_or_default();
            let _ = xfer.xfer_ascii_string(&mut trigger_name);
            if is_loading {
                self.trigger_info[i].trigger = None;
                if !trigger_name.is_empty() {
                    let terrain = crate::terrain::get_terrain_logic();
                    if let Ok(terrain_guard) = terrain.read() {
                        self.trigger_info[i].trigger = terrain_guard
                            .get_trigger_area_by_name(&trigger_name)
                            .cloned()
                            .map(Arc::new);
                    }
                }
            }
            let mut entered = u8::from(self.trigger_info[i].entered);
            let _ = xfer.xfer_unsigned_byte(&mut entered);
            self.trigger_info[i].entered = entered != 0;

            let mut exited = u8::from(self.trigger_info[i].exited);
            let _ = xfer.xfer_unsigned_byte(&mut exited);
            self.trigger_info[i].exited = exited != 0;

            let mut is_inside = u8::from(self.trigger_info[i].is_inside);
            let _ = xfer.xfer_unsigned_byte(&mut is_inside);
            self.trigger_info[i].is_inside = is_inside != 0;
        }

        let mut layer = self.layer as u32;
        let _ = xfer.xfer_unsigned_int(&mut layer);
        if is_loading {
            self.layer = PathfindLayerEnum::from_u32(layer);
        }

        let mut destination_layer = self.destination_layer as u32;
        let _ = xfer.xfer_unsigned_int(&mut destination_layer);
        if is_loading {
            self.destination_layer = PathfindLayerEnum::from_u32(destination_layer);
        }

        let _ = xfer.xfer_bool(&mut self.is_selectable);
        let _ = xfer.xfer_unsigned_int(&mut self.safe_occlusion_frame);

        let mut formation_id = self.formation_id.as_u32();
        let _ = xfer.xfer_unsigned_int(&mut formation_id);
        self.formation_id = FormationID::new(formation_id);
        if !self.formation_id.is_none() {
            xfer_coord2d_values(xfer, &mut self.formation_offset);
        }

        let mut module_count = self.modules.len().min(u16::MAX as usize) as u16;
        let _ = xfer.xfer_unsigned_short(&mut module_count);

        if is_saving {
            for entry in self.modules.iter().take(module_count as usize) {
                let mut module_identifier = entry
                    .with_module(|module| {
                        NameKeyGenerator::key_to_name(module.get_module_tag_name_key())
                    })
                    .unwrap_or_else(|| entry.tag().to_string());
                let _ = xfer.xfer_ascii_string(&mut module_identifier);

                if xfer.begin_block().is_ok() {
                    entry.with_module(|module| {
                        if let Err(err) = module.xfer(xfer) {
                            warn!(
                                "Object::xfer failed for module '{}' on object {}: {}",
                                module_identifier, self.id, err
                            );
                        }
                    });
                    let _ = xfer.end_block();
                }
            }
        } else {
            for _ in 0..module_count {
                let mut module_identifier = String::new();
                let _ = xfer.xfer_ascii_string(&mut module_identifier);
                let module_identifier_key = NameKeyGenerator::name_to_key(&module_identifier);

                let module_index = self.modules.iter().position(|entry| {
                    entry.with_module(|module| {
                        module.get_module_tag_name_key() == module_identifier_key
                    })
                });

                let data_size = xfer.begin_block().unwrap_or(0);
                if let Some(index) = module_index {
                    let entry = &self.modules[index];
                    entry.with_module(|module| {
                        if let Err(err) = module.xfer(xfer) {
                            warn!(
                                "Object::xfer load failed for module '{}' on object {}: {}",
                                module_identifier, self.id, err
                            );
                        }
                    });
                } else if data_size > 0 {
                    let _ = xfer.skip(data_size);
                }
                let _ = xfer.end_block();
            }
        }

        if version >= 3 {
            let _ = xfer.xfer_unsigned_int(&mut self.sole_healing_benefactor_id);
            let _ = xfer.xfer_unsigned_int(&mut self.sole_healing_benefactor_expiration_frame);
        } else if is_loading {
            self.sole_healing_benefactor_id = INVALID_ID;
            self.sole_healing_benefactor_expiration_frame = 0;
        }

        if version >= 4 {
            let mut cur_weapon_set_flags = weapon_set_flags_to_bits(self.cur_weapon_set_flags);
            let _ = xfer.xfer_unsigned_int(&mut cur_weapon_set_flags);
            self.cur_weapon_set_flags = weapon_set_flags_from_bits(cur_weapon_set_flags);

            let mut weapon_bonus_condition = self.weapon_bonus_condition.bits();
            let _ = xfer.xfer_unsigned_int(&mut weapon_bonus_condition);
            self.weapon_bonus_condition =
                WeaponBonusConditionFlags::from_bits_retain(weapon_bonus_condition);

            for condition in &mut self.last_weapon_condition {
                let _ = xfer.xfer_unsigned_byte(condition);
            }

            if is_loading {
                if let Err(err) = self
                    .weapon_set
                    .update_weapon_set(self.id, &self.cur_weapon_set_flags)
                {
                    warn!(
                        "Object::xfer failed to prepare weapon set for object {}: {}",
                        self.id, err
                    );
                }
            }
            if let Err(err) = self.weapon_set.xfer_state(xfer) {
                warn!(
                    "Object::xfer failed to serialize weapon set for object {}: {}",
                    self.id, err
                );
            }

            let mut special_power_bits = self.special_power_bits.bits();
            xfer_u128_bits(xfer, &mut special_power_bits);
            self.special_power_bits = SpecialPowerMask::from_bits_retain(special_power_bits);

            let mut command_override = self.command_set_string_override.to_string();
            let _ = xfer.xfer_ascii_string(&mut command_override);
            if is_loading {
                self.command_set_string_override = AsciiString::from(command_override.as_str());
            }

            let _ = xfer.xfer_bool(&mut self.modules_ready);
        }

        if version >= 5 {
            let _ = xfer.xfer_bool(&mut self.is_receiving_difficulty_bonus);
        } else {
            self.is_receiving_difficulty_bonus = false;
        }
    }

    fn load_post_process(&mut self) {
        // contained_by_id already restored during xfer (v6+).

        for entry in &self.modules {
            entry.with_module(|module| {
                if let Err(err) = module.load_post_process() {
                    warn!(
                        "Object::load_post_process module '{}' on object {} failed: {}",
                        entry.name(),
                        self.id,
                        err
                    );
                }
            });
        }

        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable_guard) = drawable.write() {
                drawable_guard.load_post_process();
            }
        }
    }
}
