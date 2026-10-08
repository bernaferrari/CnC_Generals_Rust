impl Module for W3DModelDraw {
    fn on_drawable_bound_to_object(&mut self) {
        self.seed_hex_color_from_owner();
        self.apply_receives_dynamic_lights();
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
        // C++ `ModelConditionInfo::preloadAssets` only calls
        // `preloadModelAssets(m_modelName)` for `m_conditionStates`.
        // `timeOfDay` and the drawable scale are unused. Transition states
        // and animation names are not preloaded.
        for state in &self.data.condition_states {
            preload_draw_asset(state.model_name.as_str());
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
    fn on_drawable_bound_to_object_with_context(
        &mut self,
        binding: &DrawModuleBindingContext,
        drawable: &ModelDrawContext,
    ) -> bool {
        self.on_bound_with_context(binding, drawable);
        true
    }

    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        self.do_draw_module_for_owner(transform_mtx, None);
    }

    fn do_draw_module_for_owner(
        &mut self,
        transform_mtx: &Matrix3D,
        driver: Option<&crate::object::drawable::Drawable>,
    ) {
        // C++: setPauseAnimation(!getDrawable()->getShouldAnimate(m_animationsRequirePower))
        let should_animate = driver
            .map(|drawable| drawable.get_should_animate(self.data.animations_require_power))
            .unwrap_or_else(|| self.owner_should_animate());
        self.set_pause_animation(!should_animate);
        let state_context = driver.map(|drawable| ModelDrawContext {
            instance_scale: drawable.get_instance_scale(),
            state_particles: !drawable.test_drawable_status(DRAWABLE_STATUS_NO_STATE_PARTICLES),
        });
        // C++ doDrawModule never early-returns on hidden/shroud; hide is Set_Hidden only.

        self.tick_animation_state();
        if self.current_animation_complete() {
            if let Some(next_state_index) = self.next_state {
                let next_duration = self.next_state_anim_loop_duration;
                self.next_state = None;
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                self.set_model_state_for_drawable(next_state_index, state_context.as_ref());
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

        let mut source = *transform_mtx;
        let instance_scale = driver
            .map(|drawable| drawable.get_instance_scale())
            .or_else(|| self.with_owner_drawable(|drawable| drawable.get_instance_scale()))
            .unwrap_or(1.0);
        if instance_scale != 1.0 {
            // C++ `doDrawModule` scales the matrix and calls `Set_ObjectScale`.
            source.x_axis *= instance_scale;
            source.y_axis *= instance_scale;
            source.z_axis *= instance_scale;
        }
        // CPP W3DModelDraw.cpp:2072-2078 only adjusts a live render object.
        // This Rust bridge represents an absent/empty state as no render model.
        // Asset admission for a named model is a separate renderer boundary.
        let adjusted = if self.has_render_model() {
            self.adjust_transform_mtx(&source)
        } else {
            source
        };
        self.submit_draw_to_bridge_for_drawable(&adjusted, driver);
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
        // C++ `setTerrainDecalSize` / `setTerrainDecalOpacity` no-op when
        // `m_terrainDecal` is null. A released or never-created decal must not
        // receive a size or opacity that a later `setTerrainDecal` would replay.
        if self.terrain_decal == TerrainDecalType::None {
            return;
        }
        if let Some(owner_id) = self.owner_id {
            if let Some(client) = terrain_decal_client() {
                client.set_size(owner_id, x, y);
            }
        }
    }

    fn set_terrain_decal_opacity(&mut self, opacity: Real) {
        if self.terrain_decal == TerrainDecalType::None {
            return;
        }
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
        // C++ `m_renderObject && Is_Really_Visible()`. Shroud does not hide
        // the render object. No current model stands in for a null render object.
        if self.hidden {
            return false;
        }
        self.current_state()
            .is_some_and(|state| !state.model_name.as_str().is_empty())
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
        if self
            .current_state()
            .is_none_or(|state| state.model_name.as_str().is_empty())
        {
            return false;
        }
        let Some(world_transform) =
            self.with_owner_drawable(|drawable| drawable.get_transform_matrix())
        else {
            return false;
        };

        let mut source = world_transform;
        let instance_scale = self
            .with_owner_drawable(|drawable| drawable.get_instance_scale())
            .unwrap_or(1.0);
        if instance_scale != 1.0 {
            source.x_axis *= instance_scale;
            source.y_axis *= instance_scale;
            source.z_axis *= instance_scale;
        }
        let adjusted = self.adjust_transform_mtx(&source);
        *pos = Coord3D::new(adjusted.w_axis.x, adjusted.w_axis.y, adjusted.w_axis.z);
        *bounding_sphere_radius = self
            .current_state()
            .and_then(|state| lookup_model_obj_bounds(state.model_name.as_str()))
            .map(|(_, extent)| {
                (extent[0] * extent[0] + extent[1] * extent[1] + extent[2] * extent[2]).sqrt()
            })
            .unwrap_or(0.0);
        *transform = adjusted;
        true
    }

    fn client_only_get_render_obj_bound_box(&self, boundbox: &mut BoundingBox) -> bool {
        let Some(model) = self.current_state().and_then(|state| {
            let name = state.model_name.as_str();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        }) else {
            return false;
        };
        let Some((center, extent)) = lookup_model_obj_bounds(&model) else {
            return false;
        };
        let Some(world_transform) = self.with_owner_drawable(|drawable| {
            let mut source = drawable.get_transform_matrix();
            let scale = drawable.get_instance_scale();
            if scale.is_finite() && scale != 1.0 {
                source.x_axis *= scale;
                source.y_axis *= scale;
                source.z_axis *= scale;
            }
            source
        }) else {
            return false;
        };
        let adjusted = self.adjust_transform_mtx(&world_transform);
        let local = Coord3D::new(center[0], center[1], center[2]);
        let rotated = adjusted.transform_vector3(local);
        boundbox.center = Coord3D::new(
            adjusted.w_axis.x + rotated.x,
            adjusted.w_axis.y + rotated.y,
            adjusted.w_axis.z + rotated.z,
        );
        boundbox.extents = Coord3D::new(extent[0], extent[1], extent[2]);
        let mut rotation = adjusted;
        rotation.w_axis = glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
        boundbox.rotation = rotation;
        true
    }

    fn client_only_get_render_obj_bone_transform(
        &self,
        bone_name: &AsciiString,
        transform: &mut Matrix3D,
    ) -> bool {
        // C++ returns false on a null render object and does not write.
        let Some(state) = self.current_state() else {
            return false;
        };
        if state.model_name.as_str().is_empty() {
            return false;
        }
        let bone_index = state
            .find_pristine_bone_by_name(bone_name.as_str())
            .map(|(_, bone)| bone.bone_index)
            .unwrap_or(0);
        if bone_index == 0 {
            *transform = Matrix3D::IDENTITY;
            return false;
        }
        let Some(world_bone) =
            self.with_owner_drawable(|drawable| drawable.get_bone_transform(bone_name.as_str()))
        else {
            *transform = Matrix3D::IDENTITY;
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
        // C++ W3DModelDraw.cpp:3379-3485. Walk the prefix, or prefix01..prefix99,
        // and stop at the first missing name. A later bone does not fill the gap.
        // The miss writes a fallback matrix that is not counted.
        const MAX_BONE_GET: usize = 64;
        let Some(state) = self.data.find_best_info(condition) else {
            return 0;
        };

        let limit = max_bones
            .min(positions.len())
            .min(transforms.len())
            .min(MAX_BONE_GET);
        if limit == 0 {
            return 0;
        }

        let start = start_index.max(0);
        let end_index = if start == 0 { 0 } else { 99 };
        let prefix = bone_name_prefix.to_ascii_lowercase();
        let mut count = 0usize;
        for idx in start..=end_index {
            if count >= limit {
                break;
            }
            let bone_name = if idx == 0 {
                prefix.clone()
            } else {
                format!("{prefix}{idx:02}")
            };
            let key = NameKeyGenerator::name_to_key(&bone_name);
            let Some(info) = state.pristine_bones.get(&key) else {
                break;
            };
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
        // C++ `W3DModelDraw::getCurrentBonePositions` (W3DModelDraw.cpp:3545-3621):
        // walk `Name` (start==0) or `Name01`… until the first miss, using the live
        // W3D render-object HTree — not GameLogic Drawable's empty skeleton.
        const MAX_BONE_GET: usize = 64;
        let limit = max_bones
            .min(positions.len())
            .min(transforms.len())
            .min(MAX_BONE_GET);
        if limit == 0 || bone_name_prefix.is_empty() {
            return 0;
        }

        let start = start_index.max(0);
        let end_index = if start == 0 { 0 } else { 99 };
        let scale = self
            .with_owner_drawable(|drawable| {
                let scale = drawable.get_instance_scale();
                if scale.is_finite() && scale > 0.0 {
                    scale
                } else {
                    1.0
                }
            })
            .unwrap_or(1.0);
        let model = self
            .current_state()
            .map(|state| state.model_name.as_str().to_string())
            .unwrap_or_default();
        let frame = self.current_anim_frame;

        let mut count = 0usize;
        for idx in start..=end_index {
            if count >= limit {
                break;
            }
            let bone_name = if idx == 0 {
                bone_name_prefix.to_string()
            } else {
                format!("{bone_name_prefix}{idx:02}")
            };
            let Some(local) = self.lookup_current_client_bone(&model, scale, frame, &bone_name)
            else {
                break;
            };
            transforms[count] = local;
            let (_, _, translation) = local.to_scale_rotation_translation();
            positions[count] = translation;
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
        // Standalone compatibility adapter. Installed Drawable queries use
        // prepare/query/commit below and pass their driving Drawable explicitly.
        let Some(plan) =
            self.projectile_launch_plan(condition, weapon_slot, barrel_index, turret_type)
        else {
            return false;
        };
        let attachment = plan
            .attachment_bone
            .as_ref()
            .and_then(|_| self.attach_to_drawable_bone_offset());
        let fallback = std::array::from_fn(|index| {
            let key = plan.fallback_pivots[index];
            if key == 0 {
                return None;
            }
            let name = NameKeyGenerator::key_to_name(key)?;
            self.with_owner_drawable(|drawable| drawable.get_bone_local_transform(&name))
                .flatten()
                .map(|transform| transform.w_axis.truncate())
        });
        plan.finish(
            attachment,
            fallback,
            launch_pos,
            turret_rot_pos,
            turret_pitch_pos,
        )
    }

    fn prepare_projectile_launch_offset(
        &self,
        condition: &ModelConditionFlags,
        weapon_slot: usize,
        barrel_index: i32,
        turret_type: TurretType,
    ) -> Option<ProjectileLaunchPlan> {
        self.projectile_launch_plan(condition, weapon_slot, barrel_index, turret_type)
    }

    fn cache_projectile_attachment(&mut self, bone: &str, offset: Coord3D) -> Coord3D {
        self.commit_projectile_attachment(bone, offset)
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
        self.replace_model_conditions_for_drawable(condition, None);
    }

    fn replace_model_condition_state_with_context(
        &mut self,
        condition: &ModelConditionFlags,
        drawable: &ModelDrawContext,
    ) {
        self.replace_model_conditions_for_drawable(condition, Some(drawable));
    }

    fn replace_indicator_color_with_context(&mut self, color: i32, drawable: &ModelDrawContext) {
        self.replace_indicator_color_for_drawable(color, Some(drawable));
    }

    fn handle_weapon_fire_fx(
        &mut self,
        weapon_slot: usize,
        barrel_index: i32,
        fx: Option<&crate::effects::FXList>,
        victim_pos: &Coord3D,
        weapon_speed: f32,
        damage_radius: f32,
        source_pose: Option<&WeaponFireFxSource>,
        live_bone: Option<&Matrix3D>,
    ) -> bool {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return false;
        }

        let (selected_barrel, barrel_info) = {
            let Some(state) = self.current_state() else {
                return false;
            };
            // C++ returns false before recoil when m_validStuff lacks BARRELS_VALID.
            if !state.barrels_are_valid() {
                return false;
            }
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
            )
        };

        // C++ W3DModelDraw.cpp:3700-3750 executes FX before starting recoil.
        // A null FX still starts recoil, without inspecting any source pose.
        let handled = fx.is_some()
            && self
                .weapon_fire_fx_pose(
                    source_pose,
                    live_bone,
                    weapon_slot,
                    selected_barrel,
                    barrel_info.fx_bone,
                )
                .is_some_and(|(position, matrix)| {
                    self.fire_owner_weapon_fx(
                        fx,
                        &position,
                        Some(&matrix),
                        Some(victim_pos),
                        weapon_speed,
                        damage_radius,
                    )
                });

        if (barrel_info.recoil_bone != 0 || barrel_info.muzzle_flash_bone != 0)
            && selected_barrel < self.weapon_recoil_info[weapon_slot].len()
        {
            self.weapon_recoil_info[weapon_slot][selected_barrel].state = RecoilState::RecoilStart;
            self.weapon_recoil_info[weapon_slot][selected_barrel].recoil_rate =
                self.data.initial_recoil;
            if barrel_info.muzzle_flash_bone != 0 {
                self.set_muzzle_flash_hidden(weapon_slot, selected_barrel, false);
            }
        }
        handled
    }

    fn get_barrel_count(&self, weapon_slot: usize) -> i32 {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return 0;
        }

        if let Some(state) = self.current_state() {
            if !state.barrels_are_valid() {
                return 0;
            }
            return state.weapon_barrels[weapon_slot].len() as i32;
        }

        0
    }

    fn replace_indicator_color(&mut self, color: i32) {
        W3DModelDraw::replace_indicator_color(self, color);
    }
}
