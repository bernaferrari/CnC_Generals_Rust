impl Module for W3DModelDraw {
    fn on_drawable_bound_to_object(&mut self) {
        if self.data.default_state >= 0 {
            self.set_model_state(self.data.default_state as usize);
        } else if let Some(state_index) = self.find_best_state_index(&ModelConditionFlags::empty())
        {
            self.set_model_state(state_index);
        }
    }

    fn on_delete(&mut self) {
        self.stop_client_particle_systems();
        self.unbind_terrain_track();
        self.release_template_shadow();
        if let Some(owner_id) = self.owner_id {
            if let Some(client) = terrain_decal_client() {
                client.release(owner_id);
            }
        }
    }

    fn preload_assets(&mut self, _time_of_day: TimeOfDay) {
        for state in self
            .data
            .condition_states
            .iter()
            .chain(self.data.transition_states.iter())
        {
            preload_draw_asset(state.model_name.as_str());
            for anim in &state.animations {
                preload_draw_asset(anim.name.as_str());
            }
        }
    }

    fn get_module_name_key(&self) -> NameKeyType {
        NameKeyGenerator::name_to_key("W3DModelDraw")
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DModelDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        // C++: setPauseAnimation(!getDrawable()->getShouldAnimate(m_animationsRequirePower))
        self.set_pause_animation(!self.owner_should_animate());

        if self.fully_obscured_by_shroud || self.hidden {
            return;
        }

        self.tick_animation_state();
        if self.current_animation_complete() {
            if let Some(next_state_index) = self.next_state {
                let next_duration = self.next_state_anim_loop_duration;
                self.next_state = None;
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                self.set_model_state(next_state_index);
                if next_duration != NO_NEXT_DURATION {
                    self.set_animation_loop_duration(next_duration);
                }
            }

            if let Some(state) = self.current_state() {
                let anim_index = self.which_anim_in_cur_state;
                if anim_index >= 0 && (anim_index as usize) < state.animations.len() {
                    let should_restart = state.animations[anim_index as usize].is_idle_anim
                        || test_flag_bit(state.flags, ACBIT_RESTART_ANIM_WHEN_COMPLETE);
                    if should_restart {
                        let cur_ref = self.cur_state;
                        self.adjust_animation(cur_ref, -1.0);
                    }
                }
            }
        }

        self.adjust_anim_speed_to_movement_speed();
        self.handle_client_turret_positioning();

        if self.sub_objects_dirty {
            self.update_sub_objects();
        }

        self.recalc_bones_for_client_particle_systems();
        if self.data.particles_attached_to_animated_bones {
            let _ = self.update_bones_for_client_particle_systems();
        }

        self.handle_client_recoil();

        let adjusted = self.adjust_transform_mtx(transform_mtx);
        self.submit_draw_to_bridge(&adjusted);
        self.sync_terrain_decal_pose();
    }

    fn set_shadows_enabled(&mut self, enable: bool) {
        self.apply_shadows_enabled(enable);
    }

    fn release_shadows(&mut self) {
        self.release_template_shadow();
    }

    fn allocate_shadows(&mut self) {
        self.allocate_template_shadow();
    }

    fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        self.apply_terrain_decal(decal_type);
    }

    fn set_terrain_decal_size(&mut self, x: Real, y: Real) {
        self.terrain_decal_size = Some((x, y));
        if let Some(owner_id) = self.owner_id {
            if let Some(client) = terrain_decal_client() {
                client.set_size(owner_id, x, y);
            }
        }
    }

    fn set_terrain_decal_opacity(&mut self, opacity: Real) {
        self.terrain_decal_opacity = Some(opacity);
        if let Some(owner_id) = self.owner_id {
            if let Some(client) = terrain_decal_client() {
                client.set_opacity(owner_id, opacity);
            }
        }
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.apply_hidden_shadow_and_decal(hidden);
    }

    fn update_bones_for_client_particle_systems(&mut self) -> bool {
        W3DModelDraw::update_bones_for_client_particle_systems(self)
    }

    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.fully_obscured_by_shroud != fully_obscured {
            self.fully_obscured_by_shroud = fully_obscured;
            self.do_start_or_stop_particle_sys();
            if let Some(owner_id) = self.owner_id {
                if let Some(client) = terrain_decal_client() {
                    client.set_shrouded(owner_id, fully_obscured);
                }
            }
        }
    }

    fn is_visible(&self) -> bool {
        !self.fully_obscured_by_shroud && !self.hidden
    }

    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        self.update_terrain_track();
        self.sync_terrain_decal_pose();
    }

    fn react_to_geometry_change(&mut self) {
        // C++ W3DModelDraw declares reactToGeometryChange() as a no-op.
    }

    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(self)
    }

    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(self)
    }
}

impl ObjectDrawInterface for W3DModelDraw {
    fn client_only_get_render_obj_info(
        &self,
        pos: &mut Coord3D,
        bounding_sphere_radius: &mut Real,
        transform: &mut Matrix3D,
    ) -> bool {
        let Some((position, radius, world_transform)) = self.with_owner_drawable(|drawable| {
            (
                drawable.get_position(),
                drawable.get_bounding_sphere_radius(),
                drawable.get_transform_matrix(),
            )
        }) else {
            return false;
        };

        *pos = position;
        *bounding_sphere_radius = radius;
        *transform = world_transform;
        true
    }

    fn client_only_get_render_obj_bound_box(&self, boundbox: &mut BoundingBox) -> bool {
        let Some((min, max)) = self.with_owner_drawable(|drawable| {
            let world_box = drawable.get_bounding_box();
            (world_box.min, world_box.max)
        }) else {
            return false;
        };
        boundbox.center = (min + max) * 0.5;
        boundbox.extents = (max - min) * 0.5;
        boundbox.rotation = Matrix3D::IDENTITY;
        true
    }

    fn client_only_get_render_obj_bone_transform(
        &self,
        bone_name: &AsciiString,
        transform: &mut Matrix3D,
    ) -> bool {
        let Some(world_bone) =
            self.with_owner_drawable(|drawable| drawable.get_bone_transform(bone_name.as_str()))
        else {
            return false;
        };

        if let Some(world_bone) = world_bone {
            *transform = world_bone;
            true
        } else {
            *transform = Matrix3D::IDENTITY;
            false
        }
    }

    fn get_pristine_bone_positions(
        &self,
        condition: &ModelConditionFlags,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize {
        let Some(state) = self.data.find_best_info(condition) else {
            return 0;
        };

        let mut matches: Vec<(i32, &PristineBoneInfo)> = Vec::new();

        for (key, info) in &state.pristine_bones {
            let Some(name) = NameKeyGenerator::key_to_name(*key) else {
                continue;
            };

            if start_index == 0 {
                if name == bone_name_prefix {
                    matches.push((0, info));
                }
                continue;
            }

            if !name.starts_with(bone_name_prefix) {
                continue;
            }

            let suffix = &name[bone_name_prefix.len()..];
            if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            if let Ok(index) = suffix.parse::<i32>() {
                if index >= start_index {
                    matches.push((index, info));
                }
            }
        }

        matches.sort_by_key(|(index, _)| *index);

        let limit = max_bones.min(positions.len()).min(transforms.len());
        let mut count = 0usize;
        for (_, info) in matches.into_iter().take(limit) {
            transforms[count] = info.transform;
            let (_, _, translation) = info.transform.to_scale_rotation_translation();
            positions[count] = translation;
            count += 1;
        }

        count
    }

    fn get_current_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize {
        let limit = max_bones.min(positions.len()).min(transforms.len()).min(64);
        if limit == 0 {
            return 0;
        }

        let start = start_index.max(0);
        let Some((to_model_space, world_bones)) = self.with_owner_drawable(|drawable| {
            let inverse = drawable.get_transform_matrix().inverse();
            let inverse = if inverse.is_finite() {
                inverse
            } else {
                Matrix3D::IDENTITY
            };
            let uniform_scale = drawable.get_world_scale().x;
            let to_model_space = inverse * Matrix3D::from_scale(Coord3D::splat(uniform_scale));

            let mut world_bones = Vec::new();
            let end_index = if start == 0 { 0 } else { 99 };
            for idx in start..=end_index {
                let bone_name = if idx == 0 {
                    bone_name_prefix.to_string()
                } else {
                    format!("{bone_name_prefix}{idx:02}")
                };

                let Some(world_bone) = drawable.get_bone_transform(&bone_name) else {
                    break;
                };
                world_bones.push(world_bone);
                if world_bones.len() >= limit {
                    break;
                }
            }

            (to_model_space, world_bones)
        }) else {
            return 0;
        };

        let mut count = 0usize;
        for world_bone in world_bones {
            let local_bone = to_model_space * world_bone;
            transforms[count] = local_bone;
            positions[count] = local_bone.w_axis.truncate();
            count += 1;
        }

        count
    }

    fn get_projectile_launch_offset(
        &self,
        condition: &ModelConditionFlags,
        weapon_slot: usize,
        barrel_index: i32,
        launch_pos: &mut Matrix3D,
        turret_type: TurretType,
        turret_rot_pos: &mut Coord3D,
        turret_pitch_pos: &mut Coord3D,
    ) -> bool {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return false;
        }

        *turret_rot_pos = Coord3D::origin();
        *turret_pitch_pos = Coord3D::origin();

        let Some(state) = self.data.find_best_info(condition) else {
            return false;
        };

        let drawable_arc = self.owner_id.and_then(|id| {
            TheGameLogic::find_object_by_id(id)
                .and_then(|obj_arc| obj_arc.read().ok().and_then(|guard| guard.get_drawable()))
        });
        let owner_orientation = self
            .owner_id
            .and_then(TheGameLogic::find_object_by_id)
            .and_then(|obj_arc| obj_arc.read().ok().map(|guard| guard.get_orientation()))
            .unwrap_or(0.0);

        let resolve_pivot_transform = |name_key: NameKeyType| -> Option<Matrix3D> {
            if name_key == 0 {
                return None;
            }

            if let Some(info) = state.pristine_bones.get(&name_key) {
                return Some(info.transform);
            }

            let Some(name) = NameKeyGenerator::key_to_name(name_key) else {
                return None;
            };

            let Some(drawable) = &drawable_arc else {
                return None;
            };

            let Ok(draw_guard) = drawable.read() else {
                return None;
            };

            draw_guard.get_bone_local_transform(&name)
        };

        let mut tech_offset = Coord3D::origin();
        if !self.data.attach_to_drawable_bone.is_empty() {
            let attach_key =
                NameKeyGenerator::name_to_key(self.data.attach_to_drawable_bone.as_str());
            if let Some(pivot) = resolve_pivot_transform(attach_key) {
                let rotated = Matrix3D::from_rotation_z(owner_orientation) * pivot;
                tech_offset = rotated.w_axis.truncate();
            }
        }

        if turret_type != TurretType::Invalid {
            let turret_index = match turret_type {
                TurretType::Primary => Some(0),
                TurretType::Secondary => Some(1),
                TurretType::Invalid => None,
            };

            if let Some(index) = turret_index {
                if let Some(turret) = state.turrets.get(index) {
                    if let Some(rot) = resolve_pivot_transform(turret.turret_angle_name_key) {
                        *turret_rot_pos = rot.w_axis.truncate();
                    }

                    if let Some(pitch) = resolve_pivot_transform(turret.turret_pitch_name_key) {
                        *turret_pitch_pos = pitch.w_axis.truncate();
                    }
                }
            }
        }

        let barrels = &state.weapon_barrels[weapon_slot];
        if barrels.is_empty() {
            return false;
        }

        let mut selected_barrel = barrel_index;
        if selected_barrel < 0 || (selected_barrel as usize) >= barrels.len() {
            selected_barrel = 0;
        }

        let Some(barrel) = barrels.get(selected_barrel as usize) else {
            return false;
        };
        *launch_pos = barrel.projectile_offset_mtx;

        if turret_type != TurretType::Invalid {
            let turret_index = match turret_type {
                TurretType::Primary => Some(0),
                TurretType::Secondary => Some(1),
                TurretType::Invalid => None,
            };

            if let Some(index) = turret_index {
                if let Some(turret) = state.turrets.get(index) {
                    *launch_pos = Matrix3D::from_rotation_z(turret.turret_art_angle) * *launch_pos;
                    *launch_pos = Matrix3D::from_rotation_y(-turret.turret_art_pitch) * *launch_pos;
                }
            }
        }

        launch_pos.w_axis.x += tech_offset.x;
        launch_pos.w_axis.y += tech_offset.y;
        launch_pos.w_axis.z += tech_offset.z;

        true
    }

    fn update_projectile_clip_status(
        &mut self,
        shots_remaining: u32,
        max_shots: u32,
        weapon_slot: usize,
    ) {
        self.apply_projectile_clip_status(shots_remaining, max_shots, weapon_slot);
    }

    fn update_supply_status(&mut self, _max_supply: i32, current_supply: i32) {
        // C++ writes Drawable CARRYING. This callback is under the drawable lock,
        // so persist the bit on the module and merge it into every later replace.
        self.note_supply_carrying(current_supply);
        let conditions = self.apply_pending_carrying(self.last_model_conditions);
        self.last_model_conditions = conditions;
        self.replace_model_condition_state(&conditions);
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.apply_hidden_shadow_and_decal(hidden);
    }

    fn notify_draw_module_dependency_cleared(&mut self) {
        self.update_sub_objects();
    }

    fn replace_model_condition_state(&mut self, condition: &ModelConditionFlags) {
        let condition = self.apply_pending_carrying(*condition);
        self.last_model_conditions = condition;
        self.hide_headlights = !condition.contains(ModelConditionFlags::NIGHT);
        if let Some(state_index) = self.find_best_state_index(&condition) {
            self.set_model_state(state_index);
        }
        self.hide_all_headlights();
    }

    fn handle_weapon_fire_fx(
        &mut self,
        weapon_slot: usize,
        barrel_index: i32,
        victim_pos: &Coord3D,
    ) -> bool {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return false;
        }

        let (selected_barrel, barrel_info, fx_bone_name, muzzle_name) = {
            let Some(state) = self.current_state() else {
                return false;
            };
            let barrels = &state.weapon_barrels[weapon_slot];
            if barrels.is_empty() {
                return false;
            }

            let mut selected_barrel = barrel_index;
            if selected_barrel < 0 || (selected_barrel as usize) >= barrels.len() {
                selected_barrel = 0;
            }

            (
                selected_barrel as usize,
                barrels[selected_barrel as usize].clone(),
                state.weapon_fire_fx_bone[weapon_slot].to_string(),
                state.weapon_muzzle_flash[weapon_slot].to_string(),
            )
        };

        if selected_barrel < self.weapon_recoil_info[weapon_slot].len() {
            self.weapon_recoil_info[weapon_slot][selected_barrel].state = RecoilState::RecoilStart;
            self.weapon_recoil_info[weapon_slot][selected_barrel].recoil_rate =
                self.data.initial_recoil;
        }

        if barrel_info.muzzle_flash_bone != 0 && !muzzle_name.is_empty() {
            let index = selected_barrel + 1;
            let named = format!("{muzzle_name}{index:02}");
            self.show_sub_object(&named, true);
            self.show_sub_object(&muzzle_name, true);
        }

        let mut handled = false;
        if barrel_info.fx_bone != 0 {
            let (pos, mtx) = if !self.hidden {
                let bone_name = if fx_bone_name.is_empty() {
                    None
                } else {
                    let index = selected_barrel + 1;
                    Some(format!("{fx_bone_name}{index:02}"))
                };
                let world = bone_name.and_then(|name| {
                    self.with_owner_drawable(|drawable| drawable.get_bone_transform(&name))
                        .flatten()
                        .or_else(|| {
                            self.with_owner_drawable(|drawable| {
                                drawable.get_current_worldspace_client_bone_positions(
                                    &fx_bone_name,
                                )
                            })
                            .flatten()
                        })
                });
                if let Some(world) = world {
                    let pos = Coord3D::new(world.w_axis.x, world.w_axis.y, world.w_axis.z);
                    (pos, world)
                } else {
                    self.logic_fire_fx_fallback()
                }
            } else {
                self.logic_fire_fx_fallback()
            };
            let _ = (mtx, victim_pos);
            self.fire_owner_weapon_fx(weapon_slot, &pos);
            handled = true;
        }

        handled
    }

    fn get_barrel_count(&self, weapon_slot: usize) -> i32 {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return 0;
        }

        if let Some(state) = self.current_state() {
            return state.weapon_barrels[weapon_slot].len() as i32;
        }

        0
    }
}
