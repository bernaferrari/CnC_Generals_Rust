//! Per-frame drawable updates: animation, particles, effects, and attachments.
//!
//! This is the GameLogic-side update loop corresponding to the C++ drawable
//! update path; it deliberately keeps frame ordering and fail-open behavior.

use super::*;

impl Drawable {
    /// Update drawable for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
        frame_number: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // C++ Drawable::updateDrawable always advances fade even when other
        // client work is skipped (frozen / lod frequency).
        self.update_fade();

        // Skip update if frozen or not time to update
        if self.frozen || (frame_number - self.last_update_frame) < self.update_frequency {
            return Ok(());
        }

        self.last_update_frame = frame_number;

        // Update animations
        self.update_animation(delta_time)?;

        // Update particle systems
        self.update_particle_systems(delta_time)?;

        // Update visual effects
        self.update_visual_effects(delta_time)?;

        // Update terrain decal opacity fade
        if self.terrain_decal != TerrainDecalType::None {
            if self.decal_opacity_fade_rate != 0.0 {
                self.decal_opacity += self.decal_opacity_fade_rate;
                if let Some(first) = self
                    .get_draw_modules_with_interface(ModuleInterfaceType::DRAW)
                    .first()
                    .cloned()
                {
                    first.with_module(|module| {
                        with_draw_module_mut(module, |draw| {
                            draw.set_terrain_decal_opacity(self.decal_opacity)
                        });
                    });
                }

                if self.decal_opacity_fade_rate < 0.0 && self.decal_opacity <= 0.0 {
                    self.decal_opacity_fade_rate = 0.0;
                    self.decal_opacity = 0.0;
                    self.set_terrain_decal(TerrainDecalType::None);
                } else if self.decal_opacity_fade_rate > 0.0 && self.decal_opacity >= 1.0 {
                    self.decal_opacity = 1.0;
                    self.decal_opacity_fade_rate = 0.0;
                }
            }
        } else {
            self.decal_opacity = 0.0;
        }

        // Update damage visualization
        self.update_damage_state()?;

        // Update level of detail
        self.update_level_of_detail()?;

        // Update stealth effects
        self.update_stealth_effects(delta_time)?;

        // Update environmental effects
        self.update_environmental_effects(delta_time)?;

        // Update attachments
        self.update_attachments(delta_time)?;

        Ok(())
    }

    /// C++ `Drawable::getExpirationDate`.
    pub fn expiration_date(&self) -> u32 {
        self.expiration_date
    }

    /// C++ `Drawable::setExpirationDate`.
    pub fn set_expiration_date(&mut self, expiration_date: u32) {
        self.expiration_date = expiration_date;
    }

    /// C++ TracerFXNugget: walk draw modules, `getTracerDrawInterface()->setTracerParms`,
    /// then stamp expiration on the drawable + tracer modules.
    pub fn apply_tracer_parms(
        &mut self,
        speed: Real,
        length: Real,
        width: Real,
        color: &RGBColor,
        initial_opacity: Real,
        expiration_date: u32,
    ) -> usize {
        self.expiration_date = expiration_date;
        let mut applied = 0usize;
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| {
                    if let Some(tdi) = draw.get_tracer_draw_interface_mut() {
                        tdi.set_tracer_parms(speed, length, width, color, initial_opacity);
                        tdi.set_expiration_date(expiration_date);
                        applied += 1;
                    }
                });
            });
        }
        applied
    }

    /// Set the world transform
    pub fn set_transform(&mut self, transform: Matrix3D) {
        let old_mtx = self.transform;
        let old_pos = self.world_position;
        let old_angle = self.world_rotation.y;

        self.transform = transform;
        let (scale, rotation, translation) = transform.to_scale_rotation_translation();
        self.world_position = translation;
        self.world_scale = scale;
        let (rx, ry, rz) = rotation.to_euler(EulerRot::XYZ);
        self.world_rotation = Coord3D::new(rx, ry, rz);

        // Update bounding volumes
        self.update_bounding_volumes();

        self.react_to_transform_change(&old_mtx, &old_pos, old_angle);
    }

    /// Notify draw modules that the world transform changed.
    ///
    /// Mirrors C++ `Drawable::reactToTransformChange`.
    pub fn react_to_transform_change(
        &mut self,
        old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        old_angle: Real,
    ) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| {
                    draw.react_to_transform_change(old_mtx, old_pos, old_angle);
                });
            });
        }
    }

    /// Notify draw modules that geometry changed.
    ///
    /// Mirrors C++ `Drawable::reactToGeometryChange`.
    pub fn react_to_geometry_change(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.react_to_geometry_change());
            });
        }
    }

    /// Play animation
    pub fn play_animation(
        &mut self,
        animation_name: &str,
        _loop_animation: bool,
        blend_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.animation_clips.contains_key(animation_name) {
            if let Some(current) = &self.current_animation {
                if current == animation_name {
                    return Ok(()); // Already playing this animation
                }

                // Start blending from current animation
                if blend_time > 0.0 {
                    self.blend_animations.push(AnimationBlend {
                        animation_name: current.clone(),
                        weight: 1.0,
                        fade_time: blend_time,
                        current_fade: 0.0,
                    });
                }
            }

            self.current_animation = Some(animation_name.to_string());
            self.animation_time = 0.0;
        }

        Ok(())
    }

    /// Stop current animation
    pub fn stop_animation(&mut self) {
        self.current_animation = None;
        self.animation_time = 0.0;
        self.blend_animations.clear();
    }

    /// Set model condition flags (for conditional model switching)
    pub fn set_model_conditions(&mut self, conditions: ModelConditionFlags) {
        self.model_conditions = conditions;

        // Check if we need to switch models based on conditions
        for (flags, model_name) in &self.conditional_models {
            if self.model_conditions.intersects(*flags) {
                self.model_name = model_name.clone();
                break;
            }
        }
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
        self.update_hidden_status();
    }

    /// Check if currently visible (not culled)
    pub fn is_currently_visible(&self) -> bool {
        self.is_visible
            && !self.hidden
            && !self.hidden_by_stealth
            && !self.frustum_culled
            && !self.occlusion_culled
            && !self.distance_culled
    }

    /// Set selection state
    /// Flash this drawable as if selected (short-lived visual cue).
    pub fn flash_as_selected(&mut self) {
        self.flash_as_selected_with_color(Color::new(255, 255, 255, 255));
    }

    /// C++ `Drawable::flashAsSelected(&color)` — explicit house/script color.
    pub fn flash_as_selected_with_color(&mut self, color: Color) {
        let effect = VisualEffect {
            effect_type: "SelectionFlash".to_string(),
            bone_attachment: None,
            offset: Coord3D::new(0.0, 0.0, 0.0),
            scale: 1.0,
            color,
            parameters: HashMap::new(),
        };

        // Short flash; rendering layer can interpret this effect as a selection blink.
        self.add_effect(effect, Some(0.25));
    }

    /// Flash this drawable with a script-defined color for a duration.
    /// C++ parity path: ScriptActions::doNamedFlash/doTeamFlash.
    pub fn script_flash(&mut self, color: Color, duration_seconds: Real) {
        if duration_seconds <= 0.0 {
            return;
        }

        let effect = VisualEffect {
            effect_type: "ScriptFlash".to_string(),
            bone_attachment: None,
            offset: Coord3D::new(0.0, 0.0, 0.0),
            scale: 1.0,
            color,
            parameters: HashMap::new(),
        };

        self.add_effect(effect, Some(duration_seconds.max(0.1)));
    }

    /// Set a script-controlled emoticon above this drawable.
    /// C++ parity path: ScriptActions::doNamedEmoticon/doTeamEmoticon.
    pub fn script_set_emoticon(&mut self, emoticon_name: &str, duration_frames: i32) {
        if emoticon_name.is_empty() || duration_frames <= 0 {
            return;
        }

        // Keep only one script emoticon active at a time, matching set/replace behavior.
        self.active_effects
            .retain(|e| !e.effect_type.starts_with("ScriptEmoticon:"));
        self.timed_effects
            .retain(|e| !e.effect.effect_type.starts_with("ScriptEmoticon:"));

        let effect = VisualEffect {
            effect_type: format!("ScriptEmoticon:{}", emoticon_name),
            bone_attachment: None,
            offset: Coord3D::new(0.0, 0.0, 0.0),
            scale: 1.0,
            color: Color::white(),
            parameters: HashMap::new(),
        };
        let seconds = (duration_frames as Real / LOGICFRAMES_PER_SECOND as Real)
            .max(1.0 / LOGICFRAMES_PER_SECOND as Real);
        self.add_effect(effect, Some(seconds));
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.is_selected = selected;

        if selected && self.selection_circle.is_none() {
            // Create default selection circle
            self.selection_circle = Some(SelectionCircle {
                radius: self.bounding_sphere.radius * 1.2,
                color: Color::new(0, 255, 0, 204),
                texture: "SelectionRing.tga".to_string(),
                animation_speed: 2.0,
            });
        } else if !selected {
            self.selection_circle = None;
        }
    }

    /// C++ `Drawable::setSelectable`.
    pub fn set_selectable(&mut self, selectable: bool) {
        if !selectable {
            self.set_selected(false);
        }
    }

    /// Add visual effect
    pub fn add_effect(&mut self, effect: VisualEffect, duration: Option<Real>) {
        if let Some(dur) = duration {
            self.timed_effects.push(TimedEffect {
                effect,
                duration: dur,
                elapsed_time: 0.0,
                fade_in_time: 0.2,
                fade_out_time: 0.2,
            });
        } else {
            self.active_effects.push(effect);
        }
    }

    /// Remove visual effect
    pub fn remove_effect(&mut self, effect_type: &str) {
        self.active_effects.retain(|e| e.effect_type != effect_type);
        self.timed_effects
            .retain(|e| e.effect.effect_type != effect_type);
    }

    /// Set stealth minimum opacity floor (C++ m_stealthOpacity).
    pub fn set_stealth_factor(&mut self, factor: Real) {
        self.stealth_factor = factor.clamp(0.0, 1.0);
        self.effective_stealth_opacity = self.stealth_factor;

        // Enable distortion effect when partially stealthed
        let stealth_blend = (1.0 - self.effective_stealth_opacity).clamp(0.0, 1.0);
        if stealth_blend > 0.0 && !self.hidden_by_stealth {
            self.distortion_amount = stealth_blend * 0.5;
            self.render_flags |= RenderFlags::DISTORTION;
        } else {
            self.distortion_amount = 0.0;
            self.render_flags &= !RenderFlags::DISTORTION;
        }
    }

    /// Update the stealth-opacity floor without changing pulse output.
    pub fn set_stealth_min_opacity(&mut self, min_opacity: Real) {
        self.stealth_factor = min_opacity.clamp(0.0, 1.0);
    }

    /// Attach another drawable (for weapons, effects, etc.)
    pub fn attach_drawable(
        &mut self,
        name: String,
        drawable: Arc<RwLock<Drawable>>,
        bone_name: String,
        offset: Coord3D,
    ) {
        let attachment = Attachment {
            drawable,
            bone_name,
            offset,
            rotation: Coord3D::new(0.0, 0.0, 0.0),
            scale: Coord3D::new(1.0, 1.0, 1.0),
        };

        self.attachments.insert(name, attachment);
    }

    /// Detach drawable
    pub fn detach_drawable(&mut self, name: &str) -> Option<Attachment> {
        self.attachments.remove(name)
    }

    /// Get bone world transform by name
    pub fn get_bone_transform(&self, bone_name: &str) -> Option<Matrix3D> {
        // Find bone index
        for (index, bone) in self.skeleton.iter().enumerate() {
            if bone.name == bone_name {
                if index < self.bone_transforms.len() {
                    let local_transform = self.bone_transforms[index];
                    return Some(self.transform * local_transform);
                }
                break;
            }
        }
        None
    }

    /// Get bone local transform by name (without applying object transform).
    pub fn get_bone_local_transform(&self, bone_name: &str) -> Option<Matrix3D> {
        for (index, bone) in self.skeleton.iter().enumerate() {
            if bone.name == bone_name {
                if index < self.bone_transforms.len() {
                    return Some(self.bone_transforms[index]);
                }
                break;
            }
        }
        None
    }

    /// Get pristine bone transforms by prefix (approximation of C++ getPristineBonePositions).
    pub fn get_pristine_bone_transforms(
        &self,
        bone_name_prefix: &str,
        start_index: usize,
        max_bones: usize,
    ) -> Vec<Matrix3D> {
        let condition = self.model_conditions;
        let mut positions = vec![Coord3D::origin(); max_bones];
        let mut transforms = vec![Matrix3D::IDENTITY; max_bones];
        for module_handle in self.modules() {
            let count = module_handle.with_module(|module| {
                let mut count = 0;
                with_object_draw_interface_mut(module, |draw_module| {
                    count = draw_module.get_pristine_bone_positions(
                        &condition,
                        bone_name_prefix,
                        start_index as i32,
                        &mut positions,
                        &mut transforms,
                        max_bones,
                    );
                });
                count
            });

            if count > 0 {
                return transforms.into_iter().take(count).collect();
            }
        }

        let mut matches: Vec<&BoneData> = if start_index == 0 {
            self.skeleton
                .iter()
                .filter(|bone| bone.name == bone_name_prefix)
                .collect()
        } else {
            self.skeleton
                .iter()
                .filter(|bone| bone.name.starts_with(bone_name_prefix))
                .collect()
        };

        matches.sort_by(|a, b| a.name.cmp(&b.name));

        let skip = start_index.saturating_sub(1);
        matches
            .into_iter()
            .skip(skip)
            .take(max_bones)
            .filter_map(|bone| self.get_bone_transform(&bone.name))
            .collect()
    }

    /// Get pristine bone positions (local space) by prefix.
    pub fn get_pristine_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: usize,
        max_bones: usize,
    ) -> Vec<Coord3D> {
        let condition = self.model_conditions;
        let mut positions = vec![Coord3D::origin(); max_bones];
        let mut transforms = vec![Matrix3D::IDENTITY; max_bones];
        for module_handle in self.modules() {
            let count = module_handle.with_module(|module| {
                let mut count = 0;
                with_object_draw_interface_mut(module, |draw_module| {
                    count = draw_module.get_pristine_bone_positions(
                        &condition,
                        bone_name_prefix,
                        start_index as i32,
                        &mut positions,
                        &mut transforms,
                        max_bones,
                    );
                });
                count
            });

            if count > 0 {
                return positions.into_iter().take(count).collect();
            }
        }

        let mut matches: Vec<(usize, &BoneData)> = if start_index == 0 {
            self.skeleton
                .iter()
                .enumerate()
                .filter(|(_, bone)| bone.name == bone_name_prefix)
                .collect()
        } else {
            self.skeleton
                .iter()
                .enumerate()
                .filter(|(_, bone)| bone.name.starts_with(bone_name_prefix))
                .collect()
        };

        matches.sort_by(|a, b| a.1.name.cmp(&b.1.name));

        let skip = start_index.saturating_sub(1);
        matches
            .into_iter()
            .skip(skip)
            .take(max_bones)
            .filter_map(|(index, _)| self.bone_transforms.get(index).copied())
            .map(|transform| {
                let (_, _, translation) = transform.to_scale_rotation_translation();
                translation
            })
            .collect()
    }

    /// Update damage state based on health percentage
    pub fn update_damage_state_for_health(&mut self, health_percentage: Real) {
        let mut new_state = 0;

        for (index, damage_state) in self.damage_states.iter().enumerate() {
            if health_percentage <= damage_state.health_threshold {
                new_state = index;
                break;
            }
        }

        if new_state != self.current_damage_state {
            self.current_damage_state = new_state;

            if let Some(damage_state) = self.damage_states.get(new_state) {
                // Apply damage state effects
                self.color_tint = damage_state.color_tint;

                if let Some(alpha) = damage_state.alpha_override {
                    self.alpha = alpha;
                }

                // Start damage particles
                let particle_effects = damage_state.particle_effects.clone();
                for particle_name in &particle_effects {
                    self.start_particle_system(particle_name);
                }
            }
        }
    }

    // Private helper methods

    fn update_animation(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(anim_name) = &self.current_animation.clone() {
            if let Some(clip) = self.animation_clips.get(anim_name).cloned() {
                self.animation_time += delta_time * self.animation_speed;

                // Handle looping
                if self.animation_time >= clip.duration {
                    if clip.loop_animation {
                        self.animation_time = self.animation_time % clip.duration;
                    } else {
                        self.animation_time = clip.duration;
                        // Animation finished - could trigger callback here
                    }
                }

                // Update bone transforms based on current animation time
                self.update_bone_transforms(&clip)?;
            }
        }

        // Update animation blends
        self.blend_animations.retain_mut(|blend| {
            blend.current_fade += delta_time;
            blend.weight = 1.0 - (blend.current_fade / blend.fade_time);
            blend.weight > 0.0
        });

        Ok(())
    }

    fn update_bone_transforms(
        &mut self,
        clip: &AnimationClip,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if clip.keyframes.is_empty() {
            return Ok(());
        }

        if self.animation_time <= clip.keyframes[0].time {
            self.bone_transforms = clip.keyframes[0].bone_transforms.clone();
            return Ok(());
        }

        for window in clip.keyframes.windows(2) {
            let previous = &window[0];
            let next = &window[1];
            if self.animation_time > next.time {
                continue;
            }

            let span = (next.time - previous.time).max(f32::EPSILON);
            let t = ((self.animation_time - previous.time) / span).clamp(0.0, 1.0);
            self.bone_transforms =
                interpolate_bone_transforms(&previous.bone_transforms, &next.bone_transforms, t);
            return Ok(());
        }

        self.bone_transforms = clip
            .keyframes
            .last()
            .map(|keyframe| keyframe.bone_transforms.clone())
            .unwrap_or_default();
        Ok(())
    }

    fn update_particle_systems(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for particle_system in &mut self.particle_systems {
            if particle_system.is_active {
                // Update particle system parameters
                // Real implementation would update particle positions, spawn new particles, etc.
            }
        }
        Ok(())
    }

    fn update_visual_effects(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update timed effects
        self.timed_effects.retain_mut(|effect| {
            effect.elapsed_time += delta_time;
            effect.elapsed_time < effect.duration
        });

        Ok(())
    }

    fn update_damage_state(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // This would be called by the associated Object when health changes
        Ok(())
    }

    fn update_level_of_detail(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Calculate distance to camera and select appropriate LOD.
        // Until the camera system is exposed in GameLogic, use origin as a stable reference.
        let distance_to_camera = {
            let pos = self.world_position;
            (pos.x * pos.x + pos.y * pos.y + pos.z * pos.z).sqrt()
        };

        self.current_lod = if distance_to_camera < self.lod_distances[0] {
            LevelOfDetail::High
        } else if distance_to_camera < self.lod_distances[1] {
            LevelOfDetail::Medium
        } else if distance_to_camera < self.lod_distances[2] {
            LevelOfDetail::Low
        } else {
            LevelOfDetail::Impostor
        };

        Ok(())
    }

    fn update_stealth_effects(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update stealth visual effects like shimmering
        let stealth_blend = (1.0 - self.effective_stealth_opacity).clamp(0.0, 1.0);
        if stealth_blend > 0.0 && !self.hidden_by_stealth {
            // Add subtle animation to distortion
            self.distortion_amount += (delta_time * 2.0).sin() * 0.01;
        }

        Ok(())
    }

    fn update_environmental_effects(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update weather effects
        if self.weather_affected {
            // This would query the weather system
            // self.wetness_factor = weather_system.get_rain_intensity();
            // self.snow_accumulation += weather_system.get_snow_rate() * delta_time;
        }

        Ok(())
    }

    fn update_attachments(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let attachment_keys: Vec<String> = self.attachments.keys().cloned().collect();
        for key in attachment_keys {
            // Update attachment position based on bone transform
            let bone_name = if let Some(attachment) = self.attachments.get(&key) {
                attachment.bone_name.clone()
            } else {
                continue;
            };

            if let Some(bone_transform) = self.get_bone_transform(&bone_name) {
                if let Some(attachment) = self.attachments.get_mut(&key) {
                    let attachment_transform =
                        bone_transform * Matrix3D::from_translation(attachment.offset);

                    if let Ok(mut attached_drawable) = attachment.drawable.write() {
                        attached_drawable.set_transform(attachment_transform);
                        attached_drawable.update(delta_time, self.last_update_frame)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn update_bounding_volumes(&mut self) {
        // Update bounding sphere center
        self.bounding_sphere.center = self.world_position;

        // Update bounding box (simplified)
        let half_size = Coord3D::new(1.0, 1.0, 1.0); // Would be calculated from model
        self.bounding_box.min = self.world_position - half_size;
        self.bounding_box.max = self.world_position + half_size;
    }

    fn start_particle_system(&mut self, particle_name: &str) {
        for particle_system in &mut self.particle_systems {
            if particle_system.name == particle_name {
                particle_system.is_active = true;
                break;
            }
        }
    }
}
