/// W3DModelDraw module instance
///
/// Reference: W3DModelDraw in W3DModelDraw.h
#[allow(dead_code)]
pub struct W3DModelDraw {
    /// Module data
    data: W3DModelDrawModuleData,

    /// Current model condition state
    cur_state: Option<ActiveModelState>,

    /// Next state to transition to
    next_state: Option<usize>,

    /// Animation loop duration for next state
    next_state_anim_loop_duration: u32,

    /// Current hex color
    #[allow(dead_code)]
    hex_color: i32,

    /// Index of currently playing animation in current state
    which_anim_in_cur_state: i32,

    /// Weapon recoil info per slot
    weapon_recoil_info: Vec<Vec<WeaponRecoilInfo>>,

    /// Whether bone particle systems need recalculation
    need_recalc_bone_particle_systems: bool,

    /// Whether fully obscured by shroud
    fully_obscured_by_shroud: bool,
    /// Explicit hidden state propagated by Drawable::update_hidden_status.
    hidden: bool,

    /// Whether shadows are enabled
    shadow_enabled: bool,

    /// Whether headlights are hidden
    hide_headlights: bool,

    /// Whether animation is paused
    pause_animation: bool,

    /// Current animation mode
    animation_mode: i32,

    /// Current animation frame index tracked by the logic-side draw runtime.
    current_anim_frame: i32,

    /// Current animation frame count used for completion checks.
    current_anim_num_frames: i32,

    /// Current animation completion state.
    current_anim_complete: bool,

    /// Cached animation speed factor selected at animation start.
    current_anim_speed_factor: Real,

    /// Sub-objects to hide/show
    sub_object_vec: Vec<HideShowSubObjInfo>,

    /// Whether sub-object visibility needs to be pushed to renderer.
    sub_objects_dirty: bool,

    /// Current terrain decal type for this draw module.
    terrain_decal: TerrainDecalType,
    /// Optional terrain decal size override (width, height).
    terrain_decal_size: Option<(Real, Real)>,
    /// Optional terrain decal opacity override.
    terrain_decal_opacity: Option<Real>,

    /// Particle systems currently active for state bone attachments.
    particle_systems: Vec<ParticleSysTracker>,

    /// Animation override state
    animation_override: AnimationOverride,

    /// Last model conditions (for detecting state changes)
    last_model_conditions: ModelConditionFlags,

    /// Owning object ID (used for turret aiming).
    owner_id: Option<ObjectID>,
}

impl W3DModelDraw {
    pub fn new(data: W3DModelDrawModuleData) -> Self {
        let weapon_recoil_info = vec![Vec::new(); WEAPONSLOT_COUNT];

        Self {
            data,
            cur_state: None,
            next_state: None,
            next_state_anim_loop_duration: NO_NEXT_DURATION,
            hex_color: 0,
            which_anim_in_cur_state: -1,
            weapon_recoil_info,
            need_recalc_bone_particle_systems: false,
            fully_obscured_by_shroud: false,
            hidden: false,
            shadow_enabled: true,
            hide_headlights: false,
            pause_animation: false,
            animation_mode: 0,
            current_anim_frame: 0,
            current_anim_num_frames: DEFAULT_ANIMATION_FRAMES,
            current_anim_complete: true,
            current_anim_speed_factor: 1.0,
            sub_object_vec: Vec::new(),
            sub_objects_dirty: false,
            terrain_decal: TerrainDecalType::None,
            terrain_decal_size: None,
            terrain_decal_opacity: None,
            particle_systems: Vec::new(),
            animation_override: AnimationOverride::new(),
            last_model_conditions: ModelConditionFlags::empty(),
            owner_id: None,
        }
    }

    fn rebuild_weapon_recoil_info(&mut self, state_ref: Option<ActiveModelState>) {
        let mut target_counts = [0usize; WEAPONSLOT_COUNT];
        if let Some(state_ref) = state_ref {
            if let Some(state) = self.resolve_state(state_ref) {
                for (slot, count) in target_counts.iter_mut().enumerate() {
                    *count = state.weapon_barrels[slot].len();
                }
            }
        }

        for (slot, target_count) in target_counts.iter().copied().enumerate() {
            if let Some(recoils) = self.weapon_recoil_info.get_mut(slot) {
                recoils.resize_with(target_count, WeaponRecoilInfo::new);
                for recoil in recoils.iter_mut() {
                    recoil.state = RecoilState::Idle;
                    recoil.shift = 0.0;
                    recoil.recoil_rate = 0.0;
                }
            }
        }
    }

    pub fn has_any_turrets(&self) -> bool {
        self.data
            .condition_states
            .iter()
            .any(|state| !state.turrets.is_empty())
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.owner_id
    }

    pub fn fully_obscured_by_shroud(&self) -> bool {
        self.fully_obscured_by_shroud
    }

    fn with_owner_drawable<R>(
        &self,
        func: impl FnOnce(&crate::object::drawable::Drawable) -> R,
    ) -> Option<R> {
        let owner_id = self.owner_id?;
        let object = TheGameLogic::find_object_by_id(owner_id)?;
        let drawable = {
            let obj_guard = object.read().ok()?;
            obj_guard.get_drawable()?
        };
        let drawable_guard = drawable.read().ok()?;
        Some(func(&drawable_guard))
    }

    fn resolve_state(&self, state_ref: ActiveModelState) -> Option<&ModelConditionInfo> {
        match state_ref {
            ActiveModelState::Condition(index) => self.data.condition_states.get(index),
            ActiveModelState::Transition(index) => self.data.transition_states.get(index),
        }
    }

    fn resolve_state_mut(
        &mut self,
        state_ref: ActiveModelState,
    ) -> Option<&mut ModelConditionInfo> {
        match state_ref {
            ActiveModelState::Condition(index) => self.data.condition_states.get_mut(index),
            ActiveModelState::Transition(index) => self.data.transition_states.get_mut(index),
        }
    }

    fn current_state(&self) -> Option<&ModelConditionInfo> {
        self.cur_state
            .and_then(|state_ref| self.resolve_state(state_ref))
    }

    fn is_current_transition_state(&self) -> bool {
        matches!(self.cur_state, Some(ActiveModelState::Transition(_)))
    }

    fn find_best_state_index(&self, conditions: &ModelConditionFlags) -> Option<usize> {
        let best_info = self.data.find_best_info(conditions)?;
        self.data
            .condition_states
            .iter()
            .position(|state| std::ptr::eq(state, best_info))
    }

    fn find_transition_state_index(
        &self,
        from_key: NameKeyType,
        to_key: NameKeyType,
    ) -> Option<usize> {
        self.data.transition_states.iter().position(|state| {
            state.transition_from_key == from_key && state.transition_to_key == to_key
        })
    }

    fn get_current_anim_fraction(&self) -> Real {
        let Some(state) = self.current_state() else {
            return -1.0;
        };
        if !is_any_maintain_frame_flag_set(state.flags) {
            return -1.0;
        }
        if self.current_anim_num_frames <= 1 {
            return 0.0;
        }
        let denom = (self.current_anim_num_frames - 1) as Real;
        if denom <= 0.0 {
            return 0.0;
        }
        let frame = self
            .current_anim_frame
            .clamp(0, self.current_anim_num_frames - 1) as Real;
        (frame / denom).clamp(0.0, 1.0)
    }

    fn current_animation_complete(&self) -> bool {
        self.current_anim_complete
    }

    fn animation_total_frames(&self, state: &ModelConditionInfo) -> i32 {
        if let Some(frames) = self.animation_override.duration_frames {
            return frames.max(1) as i32;
        }
        if self.which_anim_in_cur_state >= 0
            && (self.which_anim_in_cur_state as usize) < state.animations.len()
        {
            let anim = &state.animations[self.which_anim_in_cur_state as usize];
            if anim.natural_duration_ms > 0.0 {
                let frames = (anim.natural_duration_ms / MSEC_PER_LOGICFRAME_REAL).round() as i32;
                return frames.max(1);
            }
        }
        DEFAULT_ANIMATION_FRAMES
    }

    fn ensure_animation_duration_loaded(&mut self, state_ref: ActiveModelState, anim_index: usize) {
        let Some(state) = self.resolve_state(state_ref) else {
            return;
        };
        let Some(anim) = state.animations.get(anim_index) else {
            return;
        };
        if anim.natural_duration_ms > 0.0 || anim.name.is_empty() {
            return;
        }

        let Some(client) = TheGameClient::get() else {
            return;
        };
        let Some(duration_ms) = client.get_animation_duration_ms(anim.name.as_str()) else {
            return;
        };
        if duration_ms <= 0.0 {
            return;
        }

        if let Some(state) = self.resolve_state_mut(state_ref) {
            if let Some(anim) = state.animations.get_mut(anim_index) {
                if anim.natural_duration_ms <= 0.0 {
                    anim.natural_duration_ms = duration_ms;
                }
            }
        }
    }

    fn particle_hidden(&self) -> bool {
        self.hidden || self.fully_obscured_by_shroud
    }

    fn current_state_particle_bones(&self) -> Option<Vec<ParticleSysBoneInfo>> {
        let Some(state) = self.current_state() else {
            return None;
        };
        let particle_sys_bones = state.particle_sys_bones.clone();
        if particle_sys_bones.is_empty() {
            return None;
        }
        Some(particle_sys_bones)
    }

    fn owner_drawable_handles(
        &self,
    ) -> Option<(
        ObjectID,
        std::sync::Arc<rhai::Locked<crate::object::Object>>,
        std::sync::Arc<rhai::Locked<crate::object::drawable::Drawable>>,
    )> {
        let Some(owner_id) = self.owner_id else {
            return None;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return None;
        };
        let Ok(obj_guard) = object.read() else {
            return None;
        };
        let Some(drawable) = obj_guard.get_drawable() else {
            return None;
        };
        drop(obj_guard);
        Some((owner_id, object, drawable))
    }

    fn stop_client_particle_systems(&mut self) {
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            self.particle_systems.clear();
            return;
        };
        for tracker in self.particle_systems.drain(..) {
            ps_manager.destroy_particle_system(tracker.id);
        }
    }

    fn matrix_translation(matrix: &Matrix3D) -> Coord3D {
        let (_, _, translation) = matrix.to_scale_rotation_translation();
        translation
    }

    fn matrix_z_rotation(matrix: &Matrix3D) -> Real {
        let cols = matrix.to_cols_array();
        cols[1].atan2(cols[0])
    }

    fn recalc_bones_for_client_particle_systems(&mut self) {
        if !self.need_recalc_bone_particle_systems {
            return;
        }

        self.need_recalc_bone_particle_systems = false;

        let Some(particle_sys_bones) = self.current_state_particle_bones() else {
            return;
        };
        let Some((owner_id, _, drawable)) = self.owner_drawable_handles() else {
            return;
        };
        let Ok(drawable_guard) = drawable.read() else {
            return;
        };
        if drawable_guard.test_drawable_status(DRAWABLE_STATUS_NO_STATE_PARTICLES) {
            return;
        }

        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };

        self.stop_client_particle_systems();

        let hidden = self.particle_hidden();
        for info in particle_sys_bones.iter() {
            if info.particle_system.is_empty() {
                continue;
            }

            let Some(system_id) =
                ps_manager.create_particle_system(Some(info.particle_system.as_str()))
            else {
                continue;
            };

            let (bone_index, bone_transform) = self
                .current_state()
                .and_then(|state| state.find_pristine_bone_by_name(info.bone_name.as_str()))
                .map(|(_, bone)| (bone.bone_index, bone.transform))
                .unwrap_or((0, Matrix3D::IDENTITY));

            if bone_index != 0 {
                let position = Self::matrix_translation(&bone_transform);
                let rotation = Self::matrix_z_rotation(&bone_transform);
                ps_manager.set_particle_system_position(system_id, &position);
                ps_manager.rotate_particle_system_local_transform_z(system_id, rotation);
            } else {
                ps_manager.set_particle_system_position(system_id, &Coord3D::origin());
            }

            ps_manager.attach_particle_system_to_drawable(system_id, owner_id);
            ps_manager.set_particle_system_saveable(system_id, false);
            if hidden {
                ps_manager.stop_particle_system(system_id);
            }
            self.particle_systems.push(ParticleSysTracker {
                id: system_id,
                bone_index,
                bone_name: info.bone_name.clone(),
            });
        }
    }

    pub fn update_bones_for_client_particle_systems(&mut self) -> bool {
        let Some((_, _, drawable)) = self.owner_drawable_handles() else {
            return true;
        };
        if self.current_state().is_none() {
            return true;
        }

        self.recalc_bones_for_client_particle_systems();

        let Ok(drawable_guard) = drawable.read() else {
            return true;
        };
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return true;
        };

        for tracker in &self.particle_systems {
            if tracker.bone_index == 0 || tracker.bone_name.is_empty() {
                continue;
            }

            if ps_manager.find_particle_system(tracker.id).is_none() {
                continue;
            }

            if let Some(transform) = drawable_guard
                .get_current_worldspace_client_bone_positions(tracker.bone_name.as_str())
            {
                let position = Self::matrix_translation(&transform);
                let orientation = Self::matrix_z_rotation(&transform);
                ps_manager.set_particle_system_position(tracker.id, &position);
                ps_manager.rotate_particle_system_local_transform_z(tracker.id, orientation);
                ps_manager.set_particle_system_transform(tracker.id, &transform);
                ps_manager.set_particle_system_skip_parent_xfrm(tracker.id, true);
            }
        }

        true
    }

    fn do_start_or_stop_particle_sys(&self) {
        let hidden = self.particle_hidden();
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        for tracker in &self.particle_systems {
            if hidden {
                ps_manager.stop_particle_system(tracker.id);
            } else {
                ps_manager.start_particle_system(tracker.id);
            }
        }
    }

    fn adjust_anim_speed_to_movement_speed(&mut self) {
        let Some(state) = self.current_state() else {
            return;
        };
        if self.which_anim_in_cur_state < 0 {
            return;
        }
        let anim_index = self.which_anim_in_cur_state as usize;
        let Some(anim) = state.animations.get(anim_index) else {
            return;
        };
        let distance_covered = anim.distance_covered;
        if distance_covered <= 0.0 {
            return;
        }

        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };
        let Some(physics) = obj_guard.get_physics() else {
            return;
        };
        let Ok(physics_guard) = physics.lock() else {
            return;
        };
        let speed = physics_guard.get_velocity().length();
        if speed <= 0.0 {
            return;
        }

        // C++ parity: distance-covered animations scale loop duration to unit speed.
        let desired_duration_ms = distance_covered / speed * MSEC_PER_LOGICFRAME_REAL;
        self.set_cur_anim_duration_in_msec(desired_duration_ms);
    }

    /// Show or hide a named sub-object.
    pub fn show_sub_object(&mut self, name: &str, show: bool) {
        let normalized_name = name.to_ascii_lowercase();
        if normalized_name.is_empty() {
            return;
        }
        let hide = !show;
        if let Some(entry) = self.sub_object_vec.iter_mut().find(|entry| {
            entry
                .sub_obj_name
                .as_str()
                .eq_ignore_ascii_case(&normalized_name)
        }) {
            entry.hide = hide;
        } else {
            self.sub_object_vec.push(HideShowSubObjInfo {
                sub_obj_name: AsciiString::from(normalized_name.as_str()),
                hide,
            });
        }
        self.sub_objects_dirty = true;
    }

    fn normalize_sub_object_entries(&mut self) {
        let mut normalized: Vec<HideShowSubObjInfo> = Vec::new();

        for entry in self.sub_object_vec.drain(..) {
            let key = entry.sub_obj_name.as_str().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }

            if let Some(existing) = normalized
                .iter_mut()
                .find(|existing| existing.sub_obj_name.as_str().eq_ignore_ascii_case(&key))
            {
                // Last writer wins, matching repeated show/hide call behavior.
                existing.hide = entry.hide;
            } else {
                normalized.push(HideShowSubObjInfo {
                    sub_obj_name: AsciiString::from(key.as_str()),
                    hide: entry.hide,
                });
            }
        }

        self.sub_object_vec = normalized;
    }

    /// Apply pending sub-object visibility updates.
    pub fn update_sub_objects(&mut self) {
        self.normalize_sub_object_entries();
        self.sub_objects_dirty = false;
    }
}
