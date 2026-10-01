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
    /// Fractional leftover from AnimationSpeedFactor / duration multiplier ticks.
    anim_frame_accumulator: Real,
    /// +1 forward / -1 reverse for `ANIM_MODE_LOOP_PINGPONG`.
    anim_direction: i32,

    /// Deferred MODELCONDITION_CARRYING intent (C++ writes this on the Drawable).
    pending_carrying: Option<bool>,

    /// True after allocateShadows created a projected template shadow.
    shadow_allocated: bool,


    /// Sub-objects to hide/show
    sub_object_vec: Vec<HideShowSubObjInfo>,
    /// Projectile clip hides for this call only. Not part of `m_subObjectVec`.
    projectile_clip_hides: [Vec<HideShowSubObjInfo>; WEAPONSLOT_COUNT],
    /// Muzzle-flash visibility for this frame. Not saved in `m_subObjectVec`.
    muzzle_flash_hides: Vec<HideShowSubObjInfo>,
    unsaved_subobject_hides: Vec<HideShowSubObjInfo>,

    /// Whether sub-object visibility needs to be pushed to renderer.
    sub_objects_dirty: bool,

    /// Current terrain decal type for this draw module.
    terrain_decal: TerrainDecalType,
    /// Bound terrain-track handle from `TheTerrainTracksRenderObjClassSystem`.
    track_handle: Option<u32>,

    /// Particle systems currently active for state bone attachments.
    particle_systems: Vec<ParticleSysTracker>,

    /// Animation override state
    animation_override: AnimationOverride,

    /// Last model conditions (for detecting state changes)
    last_model_conditions: ModelConditionFlags,

    /// First pristine attach-bone result. None means not queried yet.
    attach_offset_cache: std::sync::Mutex<Option<Coord3D>>,
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
            anim_frame_accumulator: 0.0,
            anim_direction: 1,
            pending_carrying: None,
            shadow_allocated: false,

            sub_object_vec: Vec::new(),
            projectile_clip_hides: std::array::from_fn(|_| Vec::new()),
            muzzle_flash_hides: Vec::new(),
            unsaved_subobject_hides: Vec::new(),
            sub_objects_dirty: false,
            terrain_decal: TerrainDecalType::None,
            track_handle: None,
            particle_systems: Vec::new(),
            animation_override: AnimationOverride::new(),
            last_model_conditions: ModelConditionFlags::empty(),
            attach_offset_cache: std::sync::Mutex::new(None),
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
        self.seed_hex_color_from_owner();
        self.apply_receives_dynamic_lights();
    }

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.owner_id
    }

    fn seed_hex_color_from_owner(&mut self) {
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj) = object.read() else {
            return;
        };
        let night = TheGlobalData::get()
            .map(|data| data.get_time_of_day() == crate::common::audio::TimeOfDay::Night)
            .unwrap_or(false);
        let color = if night {
            obj.get_night_indicator_color()
        } else {
            obj.get_indicator_color()
        };
        self.hex_color = packed_indicator_hex(color);
    }

    fn apply_receives_dynamic_lights(&self) {
        if self.data.receives_dynamic_lights {
            return;
        }
        let Some(owner_id) = self.owner_id else {
            return;
        };
        crate::object::draw::client_visual::set_receives_dynamic_lights(owner_id, false);
    }

    /// C++ `W3DModelDraw::replaceIndicatorColor`.
    pub fn replace_indicator_color(&mut self, color: i32) {
        if !self.data.ok_to_change_model_color {
            return;
        }
        // C++ returns before the store when getRenderObject() is null.
        if self.cur_state.is_none() {
            return;
        }
        let new_color = if color == 0 { 0 } else { color | 0xFF00_0000u32 as i32 };
        if new_color == self.hex_color {
            return;
        }
        self.hex_color = new_color;
        let Some(cur) = self.cur_state else {
            return;
        };
        // C++ nulls m_curState then setModelState(tmp) so house-color textures rebuild.
        self.cur_state = None;
        self.next_state = None;
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        match cur {
            ActiveModelState::Condition(index) => self.set_model_state(index),
            ActiveModelState::Transition(index) => {
                if let Some(state) = self.data.transition_states.get(index) {
                    if let Some(dest) = self
                        .data
                        .condition_states
                        .iter()
                        .position(|candidate| candidate.transition_key == state.transition_to_key)
                    {
                        self.set_model_state(dest);
                    }
                }
            }
        }
    }

    pub fn hex_color(&self) -> i32 {
        self.hex_color
    }


    pub fn fully_obscured_by_shroud(&self) -> bool {
        self.fully_obscured_by_shroud
    }

    pub fn last_model_conditions(&self) -> ModelConditionFlags {
        self.last_model_conditions
    }

    /// C++ `isAnimationComplete` / `W3DModelDraw` cur-anim finished.
    pub fn current_animation_complete(&self) -> bool {
        self.current_anim_complete
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

    pub fn has_render_model(&self) -> bool {
        self.current_state()
            .is_some_and(|state| !state.model_name.as_str().is_empty())
    }
    pub fn anim_frame_count(&self) -> i32 {
        self.current_anim_num_frames
    }
    pub fn has_bound_animation(&self) -> bool {
        self.which_anim_in_cur_state >= 0
    }
    fn current_state(&self) -> Option<&ModelConditionInfo> {
        self.cur_state
            .and_then(|state_ref| self.resolve_state(state_ref))
    }
    pub(crate) fn fx_bone_name_for_shot(
        &self,
        weapon_slot: usize,
        barrel_index: i32,
    ) -> Option<String> {
        let state = self.current_state()?;
        let prefix = state.weapon_fire_fx_bone.get(weapon_slot)?;
        if prefix.is_empty() {
            return None;
        }
        let barrels = state.weapon_barrels.get(weapon_slot)?;
        if barrels.is_empty() {
            return None;
        }
        let mut selected = barrel_index;
        if selected < 0 || selected as usize >= barrels.len() {
            selected = 0;
        }
        Some(format!("{prefix}{:02}", selected as usize + 1))
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
        if state.model_name.as_str().is_empty() {
            return -1.0;
        }
        if !is_any_maintain_frame_flag_set(state.flags) {
            return -1.0;
        }
        if self.current_anim_num_frames <= 1 {
            return 0.0;
        }
        let denom = (self.current_anim_num_frames - 1) as Real;
        let frame = self
            .current_anim_frame
            .clamp(0, self.current_anim_num_frames - 1) as Real;
        (frame / denom).clamp(0.0, 1.0)
    }

    fn animation_total_frames(&self, state: &ModelConditionInfo) -> i32 {
        // Native clip length only. Duration overrides become a playback
        // multiplier (C++ Set_Animation_Frame_Rate_Multiplier), not a
        // rewritten frame count.
        if self.which_anim_in_cur_state >= 0
            && (self.which_anim_in_cur_state as usize) < state.animations.len()
        {
            let anim = &state.animations[self.which_anim_in_cur_state as usize];
            if anim.natural_duration_ms > 0.0 {
                let frames =
                    (anim.natural_duration_ms / MSEC_PER_LOGICFRAME_REAL).round() as i32;
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
        // C++ is `m_hidden || m_hiddenByStealth || m_fullyObscuredByShroud`.
        // Stealth already calls `set_hidden`. `is_drawable_effectively_hidden`
        // also treats `!is_visible` as hidden, which this function does not.
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

    /// C++ `recalcBonesForClientParticleSystems`: identity×scale then live bone.
    fn live_particle_bone_model_space(&self, bone_name: &str) -> Option<(i32, Matrix3D)> {
        if bone_name.is_empty() {
            return None;
        }
        self.with_owner_drawable(|drawable| {
            let local = drawable.get_bone_local_transform(bone_name)?;
            let scale = drawable.get_instance_scale();
            let scale = if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            };
            let scaled = Matrix3D::from_scale(Coord3D::splat(scale)) * local;
            let index = self
                .current_state()
                .and_then(|state| state.find_pristine_bone_by_name(bone_name))
                .map(|(_, bone)| bone.bone_index)
                .unwrap_or(0);
            Some((index, scaled))
        })
        .flatten()
    }

    /// C++ `Get_Bone_Index` / `Get_Bone_Transform` then `preMul(inverse)`
    /// (W3DModelDraw.cpp:3590-3596). Uses the registered W3D HTree hook
    /// (current anim frame) and the validated pristine cache — never the
    /// empty GameLogic skeleton.
    fn lookup_current_client_bone(
        &self,
        model: &str,
        scale: Real,
        frame: i32,
        bone_name: &str,
    ) -> Option<Matrix3D> {
        if bone_name.is_empty() {
            return None;
        }
        let bind = if !model.is_empty() {
            lookup_pristine_bone(model, scale, frame, bone_name)
                .or_else(|| {
                    let lower = bone_name.to_ascii_lowercase();
                    (lower != bone_name)
                        .then(|| lookup_pristine_bone(model, scale, frame, &lower))
                        .flatten()
                })
                .map(|(_, mtx)| mtx)
        } else {
            None
        };
        let bind = bind.or_else(|| {
            self.current_state().and_then(|state| {
                state
                    .find_pristine_bone_by_name(bone_name)
                    .map(|(_, info)| info.transform)
                    .or_else(|| {
                        let lower = bone_name.to_ascii_lowercase();
                        (lower != bone_name)
                            .then(|| {
                                state
                                    .find_pristine_bone_by_name(&lower)
                                    .map(|(_, info)| info.transform)
                            })
                            .flatten()
                    })
            })
        })?;
        Some(self.captured_bone_or_bind(bone_name, bind))
    }

    /// C++ `PivotClass::Capture_Update` post-multiplies the control onto the
    /// posed bone. Turret and recoil controls are local.
    fn captured_bone_or_bind(&self, bone_name: &str, bind: Matrix3D) -> Matrix3D {
        let Some(index) = self.current_state().and_then(|state| {
            state
                .find_pristine_bone_by_name(bone_name)
                .or_else(|| {
                    let lower = bone_name.to_ascii_lowercase();
                    state.find_pristine_bone_by_name(&lower)
                })
                .map(|(_, info)| info.bone_index)
        }) else {
            return bind;
        };
        if index == 0 {
            return bind;
        }
        self.collect_bone_overrides()
            .into_iter()
            .find(|entry| entry.bone_index == index)
            .map(|entry| bind * entry.transform)
            .unwrap_or(bind)
    }


    /// C++ `Matrix3D::Translate_Z` — post-multiply a local-Z translation.
    fn translate_z(mtx: &mut Matrix3D, z: Real) {
        *mtx *= Matrix3D::from_translation(Coord3D::new(0.0, 0.0, z));
    }

    /// C++ `W3DModelDraw.cpp:2005-2009`.
    /// `getConstructionPercent() >= 0` then `Translate_Z(-height + height * pct / 100)`.
    /// Completed objects use `CONSTRUCTION_COMPLETE = -1` and are not sunk.
    fn construction_percent_z_delta(pct: Real, height: Real) -> Option<Real> {
        if pct < 0.0 {
            None
        } else {
            Some(-height + height * pct / 100.0)
        }
    }

    /// C++ `CACHE_ATTACH_BONE` path (`W3DModelDraw.cpp:1974-1981`):
    /// `Rotate_Vector(offset)` then `Adjust_*_Translation`.
    fn apply_attach_to_drawable_bone_offset(mtx: &mut Matrix3D, offset: Coord3D) {
        let rotated = mtx.transform_vector3(offset);
        mtx.w_axis.x += rotated.x;
        mtx.w_axis.y += rotated.y;
        mtx.w_axis.z += rotated.z;
    }

    /// C++ `W3DModelDrawModuleData::getAttachToDrawableBoneOffset`.
    /// Empty name → no offset. Otherwise always an offset (zero if the bone is missing).
    fn attach_to_drawable_bone_offset(&self) -> Option<Coord3D> {
        if self.data.attach_to_drawable_bone.is_empty() {
            return None;
        }
        if let Ok(guard) = self.attach_offset_cache.lock() {
            if let Some(cached) = *guard {
                return Some(cached);
            }
        }

        let queried = self.with_owner_drawable(|drawable| {
            drawable
                .get_pristine_bone_positions(self.data.attach_to_drawable_bone.as_str(), 0, 1)
                .into_iter()
                .next()
        });
        if let Some(pos) = queried {
            let offset = pos.unwrap_or(Coord3D::origin());
            if let Ok(mut guard) = self.attach_offset_cache.lock() {
                *guard = Some(offset);
            }
            return Some(offset);
        }

        Some(self.data.attach_to_drawable_bone_offset)
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

        // C++ does not stop here. setModelState already stopped the old systems.
        // Recalc only creates and appends.

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
                .live_particle_bone_model_space(info.bone_name.as_str())
                .or_else(|| {
                    self.current_state()
                        .and_then(|state| state.find_pristine_bone_by_name(info.bone_name.as_str()))
                        .map(|(_, bone)| (bone.bone_index, bone.transform))
                })
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
        if self.current_state().is_none()
            || self
                .current_state()
                .is_some_and(|state| state.model_name.as_str().is_empty())
        {
            return true;
        }



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
        let speed = physics.get_velocity().length();
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
        let mut found = false;
        for entry in &mut self.sub_object_vec {
            if entry
                .sub_obj_name
                .as_str()
                .eq_ignore_ascii_case(&normalized_name)
            {
                entry.hide = hide;
                found = true;
            }
        }
        if !found {
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

    fn set_pause_animation(&mut self, pause: bool) {
        if self.pause_animation == pause {
            return;
        }
        self.pause_animation = pause;
    }

    fn hide_all_headlights(&mut self) {
        // C++ scans every render sub-object for HEADLIGHT and Set_Hidden.
        // Those names are not pushed into m_subObjectVec, so they are not saved.
        let hide = self.hide_headlights;
        self.unsaved_subobject_hides
            .retain(|entry| !entry.sub_obj_name.as_str().contains("HEADLIGHT"));
        let model = self
            .current_state()
            .map(|state| state.model_name.as_str().to_string())
            .unwrap_or_default();
        if hide && !model.is_empty() {
            for name in lookup_sub_object_names(&model) {
                if name.contains("HEADLIGHT") {
                    self.unsaved_subobject_hides.push(HideShowSubObjInfo {
                        sub_obj_name: AsciiString::from(name.as_str()),
                        hide: true,
                    });
                }
            }
        }
        for entry in &mut self.sub_object_vec {
            if entry.sub_obj_name.as_str().contains("HEADLIGHT") {
                entry.hide = hide;
            }
        }
        self.sub_objects_dirty = true;
    }

    fn adjust_transform_mtx(&self, transform_mtx: &Matrix3D) -> Matrix3D {
        let mut mtx = *transform_mtx;

        // C++ W3DModelDraw.cpp:1974-1982 (CACHE_ATTACH_BONE is defined).
        if let Some(offset) = self.attach_to_drawable_bone_offset() {
            Self::apply_attach_to_drawable_bone_offset(&mut mtx, offset);
        }

        // C++ W3DModelDraw.cpp:2000-2012 — construction-percent height sink.
        let adjust_height = self
            .current_state()
            .map(|state| test_flag_bit(state.flags, ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT))
            .unwrap_or(false);
        if adjust_height {
            if let Some(owner_id) = self.owner_id {
                if let Some(object) = TheGameLogic::find_object_by_id(owner_id) {
                    if let Ok(obj) = object.read() {
                        let pct = obj.get_construction_percent() as Real;
                        let height = obj.get_geometry_info().get_max_height_above_position();
                        if let Some(dz) = Self::construction_percent_z_delta(pct, height) {
                            Self::translate_z(&mut mtx, dz);
                        }
                    }
                }
            }
        }
        mtx
    }

    fn bind_terrain_track_if_needed(&mut self) {
        if self.track_handle.is_some() {
            return;
        }
        if self.data.track_file.is_empty() {
            return;
        }
        if !game_engine::common::global_data::read().make_track_marks {
            return;
        }
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = terrain_track_client() else {
            return;
        };
        self.track_handle = client.bind_track(
            owner_id,
            MAP_XY_FACTOR,
            self.data.track_file.as_str(),
        );
    }

    fn update_terrain_track(&mut self) {
        let Some(handle) = self.track_handle else {
            return;
        };
        let Some(client) = terrain_track_client() else {
            return;
        };
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj) = object.read() else {
            return;
        };
        let pos = self
            .with_owner_drawable(|drawable| drawable.get_position())
            .unwrap_or_else(|| *obj.get_position());
        let now = TheGameLogic::get_frame();
        if self.fully_obscured_by_shroud || obj.test_status(ObjectStatusTypes::Stealthed) {
            client.add_cap(handle, pos.x, pos.y, now);
        } else {
            if obj.is_significantly_above_terrain() {
                client.set_airborne(handle);
            }
            client.add_edge(handle, pos.x, pos.y, now);
        }
    }

    fn cap_terrain_track(&mut self) {
        let Some(handle) = self.track_handle else {
            return;
        };
        let Some(client) = terrain_track_client() else {
            return;
        };
        let pos = if let Some(pos) = self.with_owner_drawable(|drawable| drawable.get_position()) {
            pos
        } else if let Some(owner_id) = self.owner_id {
            let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
                return;
            };
            let Ok(obj) = object.read() else {
                return;
            };
            *obj.get_position()
        } else {
            return;
        };
        client.add_cap(handle, pos.x, pos.y, TheGameLogic::get_frame());
    }

    fn unbind_terrain_track(&mut self) {
        if let Some(handle) = self.track_handle.take() {
            if let Some(client) = terrain_track_client() {
                client.unbind_track(handle);
            }
        }
    }

    fn apply_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        self.terrain_decal = decal_type;
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = terrain_decal_client() else {
            return;
        };
        if decal_type == TerrainDecalType::None {
            client.release(owner_id);
            return;
        }

        let mut texture = terrain_decal_texture_name(decal_type).to_string();
        // C++ setTerrainDecal always uses ThingTemplate shadow size. setTerrainDecalSize
        // only calls setSize on a live decal and stores nothing, so a later
        // setTerrainDecal after release must not replay that override.
        let mut size = (0.0, 0.0);
        let mut offset = (0.0, 0.0);
        let mut position = Coord3D::new(0.0, 0.0, 0.0);
        let mut angle = 0.0;
        let mut shadow_type = crate::common::types::SHADOW_ALPHA_DECAL;
        if let Some(object) = TheGameLogic::find_object_by_id(owner_id) {
            if let Ok(obj) = object.read() {
                if let Some(drawable_mtx) =
                    self.with_owner_drawable(|drawable| drawable.get_transform_matrix())
                {
                    let adjusted = self.adjust_transform_mtx(&drawable_mtx);
                    position = Coord3D::new(adjusted.w_axis.x, adjusted.w_axis.y, adjusted.w_axis.z);
                    angle = Self::matrix_z_rotation(&adjusted);
                } else {
                    position = *obj.get_position();
                    angle = obj.get_orientation();
                }
                let tmpl = obj.get_template().as_ref();
                if decal_type == TerrainDecalType::ShadowTexture || texture.is_empty() {
                    texture = leftover_default_shadow_texture(
                        tmpl.get_template_geometry_type(),
                        tmpl.get_shadow_texture_name(),
                    );
                }
                if decal_type == TerrainDecalType::ShadowTexture {
                    shadow_type = tmpl.get_shadow_type_bits();
                }
                size = (tmpl.get_shadow_size_x(), tmpl.get_shadow_size_y());
                offset = (tmpl.get_shadow_offset_x(), tmpl.get_shadow_offset_y());
            }
        }


        client.set_decal(&TerrainDecalDesc {
            object_id: owner_id,
            texture_name: texture,
            size_x: size.0,
            size_y: size.1,
            opacity: 1.0,
            position,
            angle,
            offset_x: offset.0,
            offset_y: offset.1,
            hidden: self.hidden,
            shrouded: self.fully_obscured_by_shroud,
            shadow_enabled: !self.hidden && self.shadow_enabled,
            is_unit_blob: decal_type == TerrainDecalType::ShadowTexture,
            shadow_type,
        });

    }

    fn sync_terrain_decal_pose(&self) {
        if self.terrain_decal == TerrainDecalType::None {
            return;
        }
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = terrain_decal_client() else {
            return;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj) = object.read() else {
            return;
        };
        if let Some(drawable_mtx) = self.with_owner_drawable(|drawable| drawable.get_transform_matrix())
        {
            let adjusted = self.adjust_transform_mtx(&drawable_mtx);
            let position = Coord3D::new(adjusted.w_axis.x, adjusted.w_axis.y, adjusted.w_axis.z);
            client.set_pose(owner_id, position, Self::matrix_z_rotation(&adjusted));
            return;
        }
        client.set_pose(owner_id, *obj.get_position(), obj.get_orientation());
    }

    fn logic_fire_fx_fallback(&self) -> (Coord3D, Matrix3D) {
        if let Some(owner_id) = self.owner_id {
            if let Some(object) = TheGameLogic::find_object_by_id(owner_id) {
                if let Ok(obj) = object.read() {
                    return (*obj.get_position(), obj.get_transform_matrix());
                }
            }
        }
        (Coord3D::new(0.0, 0.0, 0.0), Matrix3D::IDENTITY)
    }

    fn owner_weapon_fx_params(&self, weapon_slot: usize) -> (Real, Real) {
        let Some(owner_id) = self.owner_id else {
            return (0.0, 0.0);
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return (0.0, 0.0);
        };
        let Ok(obj) = object.read() else {
            return (0.0, 0.0);
        };
        let slot = match weapon_slot {
            0 => WeaponSlotType::Primary,
            1 => WeaponSlotType::Secondary,
            _ => WeaponSlotType::Tertiary,
        };
        let Some(weapon) = obj.get_weapon_in_slot(slot.into()) else {
            return (0.0, 0.0);
        };
        let template = weapon.get_template();
        (template.weapon_speed, template.primary_damage_radius)
    }

    fn fire_owner_weapon_fx(
        &self,
        fx: Option<&crate::effects::FXList>,
        pos: &Coord3D,
        mtx: Option<&Matrix3D>,
        victim_pos: Option<&Coord3D>,
        weapon_speed: Real,
        damage_radius: Real,
    ) -> bool {
        let Some(fx) = fx else {
            return false;
        };
        let _ = fx.do_fx_pos(pos, mtx, weapon_speed, victim_pos, damage_radius);
        true
    }

    fn owner_should_animate(&self) -> bool {
        if let Some(should) = self.with_owner_drawable(|drawable| {
            drawable.get_should_animate(self.data.animations_require_power)
        }) {
            return should;
        }
        let Some(owner_id) = self.owner_id else {
            return true;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return true;
        };
        let Ok(obj) = object.read() else {
            return true;
        };
        object_should_animate(&obj, self.data.animations_require_power)
    }
}

/// C++ `replaceIndicatorColor`: zero stays zero, else OR opaque alpha.
fn packed_indicator_hex(color: crate::common::Color) -> i32 {
    let packed = ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32);
    if packed == 0 {
        0
    } else {
        (packed | 0xFF00_0000) as i32
    }
}
