impl W3DModelDraw {
    /// Set current model state
    fn set_model_state(&mut self, state_index: usize) {
        if state_index >= self.data.condition_states.len() {
            return;
        }
        let extra_public_bones = self.data.extra_public_bones.clone();
        self.data.condition_states[state_index].validate_runtime_caches(&extra_public_bones);

        let mut new_state_ref = ActiveModelState::Condition(state_index);
        let mut pending_next_state: Option<usize> = None;

        if let Some(cur_state_ref) = self.cur_state {
            if (cur_state_ref == new_state_ref && self.next_state.is_none())
                || self.next_state == Some(state_index)
            {
                return;
            }

            let cur_transition_key = self
                .resolve_state(cur_state_ref)
                .map(|state| state.transition_key)
                .unwrap_or(NAMEKEY_INVALID);
            let requested_state = &self.data.condition_states[state_index];

            if new_state_ref != cur_state_ref
                && requested_state.allow_to_finish_key != NAMEKEY_INVALID
                && requested_state.allow_to_finish_key == cur_transition_key
                && !self.current_animation_complete()
            {
                self.next_state = Some(state_index);
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                return;
            }

            if new_state_ref != cur_state_ref
                && cur_transition_key != NAMEKEY_INVALID
                && requested_state.transition_key != NAMEKEY_INVALID
            {
                if let Some(transition_index) = self
                    .find_transition_state_index(cur_transition_key, requested_state.transition_key)
                {
                    new_state_ref = ActiveModelState::Transition(transition_index);
                    pending_next_state = Some(state_index);
                }
            }
        }

        if let Some(state) = self.resolve_state_mut(new_state_ref) {
            state.validate_runtime_caches(&extra_public_bones);
        }

        let prev_state = self.cur_state;
        let prev_anim_fraction = self.get_current_anim_fraction();

        self.need_recalc_bone_particle_systems = self
            .with_owner_drawable(|drawable| {
                !drawable.test_drawable_status(DRAWABLE_STATUS_NO_STATE_PARTICLES)
            })
            .unwrap_or(true);
        self.stop_client_particle_systems();
        // C++ hideAllMuzzleFlashes(newState) before swap so leftover flashes die.
        self.hide_all_muzzle_flashes();
        // C++ does NOT replace m_subObjectVec with the state's HideShowVec.
        // Apply authored hide/show via dirty compose; runtime show_sub_object
        // overrides stay in sub_object_vec and win in updateSubObjects.
        self.sub_objects_dirty = true;
        self.rebuild_weapon_recoil_info(Some(new_state_ref));

        self.cur_state = Some(new_state_ref);
        self.hide_all_muzzle_flashes();

        self.next_state = pending_next_state;
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        self.bind_terrain_track_if_needed();
        self.hide_all_headlights();
        self.adjust_animation(prev_state, prev_anim_fraction);
    }

    fn adjust_animation(&mut self, prev_state: Option<ActiveModelState>, prev_anim_fraction: Real) {
        let Some(cur_state_ref) = self.cur_state else {
            self.which_anim_in_cur_state = -1;
            self.current_anim_complete = true;
            return;
        };
        let Some(cur_state) = self.resolve_state(cur_state_ref) else {
            self.which_anim_in_cur_state = -1;
            self.current_anim_complete = true;
            return;
        };

        let num_anims = cur_state.animations.len();
        if num_anims == 0 {
            self.which_anim_in_cur_state = -1;
            self.current_anim_frame = 0;
            self.current_anim_num_frames = DEFAULT_ANIMATION_FRAMES;
            self.current_anim_complete = true;
            return;
        }

        if num_anims == 1 {
            self.which_anim_in_cur_state = 0;
        } else if prev_state == Some(cur_state_ref) {
            let anim_to_avoid = self.which_anim_in_cur_state;
            while self.which_anim_in_cur_state == anim_to_avoid {
                self.which_anim_in_cur_state = game_client_random_value(0, num_anims as i32 - 1);
            }
        } else {
            self.which_anim_in_cur_state = game_client_random_value(0, num_anims as i32 - 1);
        }

        if self.which_anim_in_cur_state >= 0 {
            self.ensure_animation_duration_loaded(
                cur_state_ref,
                self.which_anim_in_cur_state as usize,
            );
        }

        let Some(cur_state) = self.resolve_state(cur_state_ref).cloned() else {
            self.which_anim_in_cur_state = -1;
            self.current_anim_complete = true;
            return;
        };

        let total_frames = self.animation_total_frames(&cur_state).max(1);
        let mut start_frame = if cur_state.anim_mode == AnimMode::OnceBackwards
            || cur_state.anim_mode == AnimMode::LoopBackwards
        {
            total_frames - 1
        } else {
            0
        };

        if test_flag_bit(cur_state.flags, ACBIT_RANDOMSTART) {
            start_frame = game_client_random_value(0, total_frames - 1);
        } else if test_flag_bit(cur_state.flags, ACBIT_START_FRAME_FIRST) {
            start_frame = 0;
        } else if test_flag_bit(cur_state.flags, ACBIT_START_FRAME_LAST) {
            start_frame = total_frames - 1;
        } else if is_any_maintain_frame_flag_set(cur_state.flags)
            && prev_state.is_some()
            && prev_state != Some(cur_state_ref)
            && prev_state
                .and_then(|state_ref| self.resolve_state(state_ref))
                .map(|state| {
                    is_any_maintain_frame_flag_set(state.flags)
                        && is_common_maintain_frame_flag_set(cur_state.flags, state.flags)
                })
                .unwrap_or(false)
            && prev_anim_fraction >= 0.0
        {
            let target = prev_anim_fraction * (total_frames - 1) as Real;
            start_frame = target.round() as i32;
        }

        self.current_anim_num_frames = total_frames.max(1);
        self.current_anim_frame = start_frame.clamp(0, self.current_anim_num_frames - 1);
        self.current_anim_speed_factor =
            if cur_state.anim_min_speed_factor <= cur_state.anim_max_speed_factor {
                game_client_random_value_real(
                    cur_state.anim_min_speed_factor,
                    cur_state.anim_max_speed_factor,
                )
            } else {
                1.0
            };
        self.anim_frame_accumulator = 0.0;
        self.current_anim_complete = false;
    }

    fn tick_animation_state(&mut self) {
        self.tick_animation_with_speed();
    }

    /// Handle client-side turret positioning
    ///
    /// Updates turret bone rotations based on object's current turret angles.
    /// Reference: C++ W3DModelDraw.cpp:2391-2442
    fn handle_client_turret_positioning(&mut self) {
        let Some(state) = self.current_state() else {
            return;
        };

        // Process each turret slot (up to MAX_TURRETS)
        for (index, turret) in state.turrets.iter().enumerate() {
            if turret.turret_angle_bone == 0 && turret.turret_pitch_bone == 0 {
                continue;
            }

            let mut turret_angle = 0.0;
            let mut turret_pitch = 0.0;
            if let Some(owner_id) = self.owner_id {
                if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) {
                    if let Ok(obj_guard) = obj.read() {
                        if let Some(ai) = obj_guard.get_ai_update_interface() {
                            if let Ok(ai_guard) = ai.lock() {
                                let turret_type = if index == 0 {
                                    TurretType::Primary
                                } else {
                                    TurretType::Secondary
                                };
                                if let Some((angle, pitch)) =
                                    ai_guard.get_turret_rot_and_pitch(turret_type)
                                {
                                    turret_angle = angle;
                                    turret_pitch = pitch;
                                }
                            }
                        }
                    }
                }
            }

            // Apply turret angle bone rotation
            if turret.turret_angle_bone != 0 {
                // Add art-defined offset to turret angle
                turret_angle += turret.turret_art_angle;

                // Create rotation matrix around Z axis
                // Reference: W3DModelDraw.cpp:2416-2421
                let turret_transform = Matrix3D::from_rotation_z(turret_angle);

                // When render object system is implemented:
                // - Capture the bone to allow manual control
                // - Apply the rotation transform to the bone
                // Reference: C++ W3DModelDraw.cpp:2416-2421
                // m_renderObject->Capture_Bone(turret.turret_angle_bone);
                // m_renderObject->Control_Bone(turret.turret_angle_bone, turretXfrm);
                let _ = turret_transform;
            }

            // Apply turret pitch bone rotation
            if turret.turret_pitch_bone != 0 {
                // Add art-defined offset to turret pitch
                turret_pitch += turret.turret_art_pitch;

                // Create rotation matrix around Y axis
                // Reference: W3DModelDraw.cpp:2427-2432
                let pitch_transform = Matrix3D::from_rotation_y(turret_pitch);

                // When render object system is implemented:
                // Reference: C++ W3DModelDraw.cpp:2427-2432
                // m_renderObject->Capture_Bone(turret.turret_pitch_bone);
                // m_renderObject->Control_Bone(turret.turret_pitch_bone, pitchXfrm);
                let _ = pitch_transform;
            }
        }
    }
    fn handle_client_recoil(&mut self) {
        const TINY_RECOIL: Real = 0.01;
        let Some(state) = self.current_state().cloned() else {
            return;
        };

        for wslot in 0..WEAPONSLOT_COUNT {
            let barrels = &state.weapon_barrels[wslot];
            let recoil_len = self
                .weapon_recoil_info
                .get(wslot)
                .map(|recoils| recoils.len())
                .unwrap_or(0);
            let count = barrels.len().min(recoil_len);
            for i in 0..count {
                // C++ hides muzzle unless RECOIL_START (one visible frame).
                if barrels[i].muzzle_flash_bone != 0 {
                    let hidden = self.weapon_recoil_info[wslot][i].state != RecoilState::RecoilStart;
                    self.set_muzzle_flash_hidden(wslot, i, hidden);
                }

                let Some(recoils) = self.weapon_recoil_info.get_mut(wslot) else {
                    continue;
                };
                if barrels[i].recoil_bone == 0 {
                    recoils[i].state = RecoilState::Idle;
                    continue;
                }

                match recoils[i].state {
                    RecoilState::Idle => {}
                    RecoilState::RecoilStart | RecoilState::Recoil => {
                        recoils[i].shift += recoils[i].recoil_rate;
                        recoils[i].recoil_rate *= self.data.recoil_damping;
                        if recoils[i].shift >= self.data.max_recoil {
                            recoils[i].shift = self.data.max_recoil;
                            recoils[i].state = RecoilState::Settle;
                        } else if recoils[i].recoil_rate.abs() < TINY_RECOIL {
                            recoils[i].state = RecoilState::Settle;
                        } else {
                            recoils[i].state = RecoilState::Recoil;
                        }
                    }
                    RecoilState::Settle => {
                        recoils[i].shift -= self.data.recoil_settle;
                        if recoils[i].shift <= 0.0 {
                            recoils[i].shift = 0.0;
                            recoils[i].state = RecoilState::Idle;
                        }
                    }
                }
            }
        }
    }

    /// Update model condition state based on current conditions
    ///
    /// Finds the best matching ModelConditionInfo and switches to it if different
    pub fn update_model_condition_state(&mut self, conditions: ModelConditionFlags) {
        let conditions = self.apply_pending_carrying(conditions);
        if conditions == self.last_model_conditions {
            return;
        }

        self.last_model_conditions = conditions;

        if let Some(state_index) = self.find_best_state_index(&conditions) {
            self.set_model_state(state_index);
        }
    }

    /// Set animation to loop in N frames
    ///
    /// This call says, "I want the current animation (if any) to take n frames to complete a single cycle".
    /// If it's a looping anim, each loop will take n frames.
    /// Note that you must call this AFTER setting the condition codes.
    ///
    /// Reference: C++ W3DModelDraw.cpp:3748 - setAnimationLoopDuration
    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        self.animation_override.duration_frames = Some(num_frames);
        self.animation_override.completion_frames = None;
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        let desired_duration_ms = (num_frames as Real * MSEC_PER_LOGICFRAME_REAL).ceil();
        self.set_cur_anim_duration_in_msec(desired_duration_ms);
    }

    /// Set animation completion time
    ///
    /// Similar to setAnimationLoopDuration, but assumes that the current state is a "ONCE",
    /// and is smart about transition states... if there is a transition state "inbetween",
    /// it is included in the completion time.
    ///
    /// Reference: C++ W3DModelDraw.cpp:3774 - setAnimationCompletionTime
    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        self.animation_override.completion_frames = Some(num_frames);
        self.animation_override.duration_frames = None;

        if self.is_current_transition_state() {
            let Some(cur_state_ref) = self.cur_state else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            self.ensure_animation_duration_loaded(cur_state_ref, 0);

            let Some(next_state_index) = self.next_state else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            self.ensure_animation_duration_loaded(ActiveModelState::Condition(next_state_index), 0);

            let Some(cur_state) = self.current_state() else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            let Some(next_state) = self.data.condition_states.get(next_state_index) else {
                self.set_animation_loop_duration(num_frames);
                return;
            };
            if !cur_state.animations.is_empty() && !next_state.animations.is_empty() {
                let t1 = cur_state.animations[0].natural_duration_ms.max(1.0);
                let t2 = next_state.animations[0].natural_duration_ms.max(1.0);
                let numerator = num_frames as Real * t1;
                let trans_time = (numerator / (t1 + t2)).floor().max(1.0) as u32;
                self.set_animation_loop_duration(trans_time);
                self.next_state_anim_loop_duration = num_frames.saturating_sub(trans_time);
                return;
            }
        }

        self.set_animation_loop_duration(num_frames);
    }

    /// Set animation frame manually
    ///
    /// Manually set a drawable's current animation to a specific frame.
    ///
    /// Reference: C++ W3DModelDraw.cpp:3797 - setAnimationFrame
    pub fn set_animation_frame(&mut self, frame: i32) {
        self.apply_animation_frame_once(frame);
    }

    /// Set current animation duration in milliseconds
    ///
    /// C++ setCurAnimDurationInMsec sets HLOD frame-rate multiplier
    /// (natural / desired). Do not rewrite native frame count.
    fn set_cur_anim_duration_in_msec(&mut self, duration_ms: Real) {
        if duration_ms > 0.0 {
            self.apply_cur_anim_duration_multiplier(duration_ms);
        }
    }

    /// Build and submit a `ModelDrawState` to the rendering bridge.
    ///
    /// This is the primary rendering submission path that connects the
    /// GameLogic draw module to the GameClient rendering pipeline. The
    /// GameClient device layer reads the `ModelDrawState` from the shared
    /// `DRAWABLE_STATE` map and converts it into a
    /// `render_bridge::DrawSubmission` for the WWVegas renderer.
    ///
    /// Reference: C++ W3DModelDraw::doDrawModule() lines 2016-2088
    ///
    /// ## C++ parity behaviors
    ///
    /// 1. **Condition-state model selection**: Selects the correct model
    ///    name based on the current condition flags (Default, Damaged,
    ///    ReallyDamaged, Rubble, Night, Snow, etc.). This mirrors the C++
    ///    behavior where `setModelState()` swaps the W3D render object.
    ///
    /// 2. **Bone overrides**: Collects turret rotation, turret pitch, and
    ///    weapon recoil bone transforms into a single list. In C++, these
    ///    are applied via `Capture_Bone`/`Control_Bone` on the render
    ///    object. Here we pass them as a `Vec<BoneOverrideState>`.
    ///
    /// 3. **Animation state**: Passes the current animation name, mode,
    ///    and time fraction. In C++, `Set_Animation()` is called on the
    ///    HLod render object.
    ///
    /// 4. **Mesh UV overrides**: For tread/track animations, the C++ code
    ///    adjusts UV offsets on specific mesh sub-objects. We pass these
    ///    as `MeshUvOverrideState` entries.
    ///
    /// 5. **Sub-object visibility**: In C++, `doHideShowSubObjs()` is
    ///    called to show/hide sub-objects. The bridge converts
    ///    `sub_object_vec` into render-state visibility directives.
    ///
    /// 6. **Instance scaling**: C++ applies `getDrawable()->getInstanceScale()`
    ///    to the world transform before rendering. We include the scaled
    ///    transform.
    fn submit_draw_to_bridge(&mut self, transform_mtx: &Matrix3D) {
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };

        // Phase 1: Resolve the model name from the current condition state.
        //
        // C++ parity: W3DModelDraw::setModelState() swaps m_renderObject to the
        // model defined by the matching ModelConditionInfo. The model name
        // comes from ModelConditionInfo::m_modelName.
        let model_name = self
            .current_state()
            .map(|s| s.model_name.to_string())
            .unwrap_or_default();

        // Phase 2: Resolve animation state.
        //
        // C++ parity: W3DModelDraw::adjustAnimation() calls
        // m_renderObject->Set_Animation(animHandle, startFrame, mode).
        // The consumer maps anim_name + anim_time + anim_mode to the
        // WWVegas AnimationController.
        let anim_name = self.current_state().and_then(|state| {
            let idx = self.which_anim_in_cur_state;
            if idx >= 0 && (idx as usize) < state.animations.len() {
                Some(state.animations[idx as usize].name.to_string())
            } else {
                None
            }
        });

        let anim_mode = self
            .current_state()
            .map(|s| match s.anim_mode {
                AnimMode::Manual => 0,
                AnimMode::Loop => 1,
                AnimMode::Once => 2,
                AnimMode::LoopPingPong => 3,
                AnimMode::LoopBackwards => 4,
                AnimMode::OnceBackwards => 5,
            })
            .unwrap_or(0);

        let anim_time = self.get_current_anim_fraction().clamp(0.0, 1.0);

        // Phase 3: Collect bone overrides (turret + recoil).
        //
        // C++ parity: handleClientTurretPositioning() and handleClientRecoil()
        // each call Capture_Bone/Control_Bone on the render object. We collect
        // all overrides into a single list.
        let bone_overrides = self.collect_bone_overrides();

        // Phase 4: Collect mesh UV overrides for tread animations.
        //
        // C++ parity: W3DTankDraw::doDrawModule() adjusts UV offsets on
        // TREADSL/TREADSR mesh sub-objects. Other draw modules (truck, etc.)
        // do similar UV scrolling on moving parts.
        let mesh_uv_overrides = self.collect_mesh_uv_overrides();
        let sub_object_visibility: Vec<SubObjectVisibilityState> = self
            .composed_sub_object_visibility()
            .into_iter()
            .map(|entry| SubObjectVisibilityState {
                sub_object_name: entry.sub_obj_name.to_string(),
                hidden: entry.hide,
            })
            .collect();

        // C++ W3DGhostObject.cpp:113-120 Peek/Set_Animation freeze happens on
        // the live HLOD at snapshot time. The optional renderer hook walks the
        // cached instance; headless / unregistered hooks stay fail-closed.
        if let Some(states) = try_capture_hlod_live_child_states(&HlodLiveChildCaptureRequest {
            object_id: owner_id,
            model_name: &model_name,
            bone_overrides: &bone_overrides,
            sub_object_visibility: &sub_object_visibility,
            animation_name: anim_name.as_deref(),
            animation_time: anim_time,
        }) {
            publish_hlod_live_child_states(owner_id, states);
        }

        // Keep the authored source bases, not resolved local bone indices.
        // Renderer-local indices are nonportable across a save/load or asset
        // reload; the receiving side must validate these names against its
        // current W3D hierarchy before using a weapon-barrel topology.
        let weapon_bone_bindings = self
            .current_state()
            .map(|state| ModelDrawWeaponBoneBindings {
                fire_fx: std::array::from_fn(|slot| state.weapon_fire_fx_bone[slot].to_string()),
                recoil: std::array::from_fn(|slot| state.weapon_recoil_bone[slot].to_string()),
                muzzle_flash: std::array::from_fn(|slot| {
                    state.weapon_muzzle_flash[slot].to_string()
                }),
                launch: std::array::from_fn(|slot| {
                    state.weapon_projectile_launch_bone[slot].to_string()
                }),
            })
            .unwrap_or_default();

        // Phase 5: Apply instance scaling to the world transform.
        //
        // C++ parity: doDrawModule() applies getDrawable()->getInstanceScale()
        // before setting the render object transform.
        let world_transform = self.apply_instance_scale(transform_mtx);

        // Keep the two render-object properties separate from the presentation
        // Drawable state.  A missing owner Drawable is not equivalent to the
        // C++ default scale: the ghost adapter must reject that source rather
        // than manufacture a value.  `hex_color` is the live W3DModelDraw
        // field (the same value passed to Create_Render_Obj in C++), so the
        // adapter can preserve its bit pattern without deriving a tint.
        let render_object_scale = self
            .with_owner_drawable(|drawable| {
                let scale = drawable.get_world_scale().x;
                scale.is_finite().then_some(scale)
            })
            .flatten();
        let render_object_color = (!model_name.is_empty()).then_some(self.hex_color as u32);

        // Phase 6: Build the model draw state with all collected data.
        //
        // The consumer (GameClient device layer) maps condition_flags_bits to
        // render_bridge::RenderConditionFlags, which controls damage overlays,
        // night/snow maps, construction visibility, etc.
        let state = ModelDrawState {
            source: Default::default(),
            logic_drawable_id: 0,
            model_name,
            world_transform,
            render_object_scale,
            render_object_color,
            condition_flags_bits: self.last_model_conditions.bits(),
            bone_overrides,
            animation_name: anim_name,
            animation_time: anim_time,
            animation_mode: anim_mode,
            mesh_uv_overrides,
            sub_object_visibility,
            weapon_bone_bindings,
        };

        client.set_active_object_model_draw(owner_id, state);
    }

    /// Collect mesh UV overrides for tread/track animations.
    ///
    /// In C++, W3DTankDraw and similar subclasses adjust UV offsets on
    /// specific mesh sub-objects (e.g., "TREADSL", "TREADSR") based on
    /// the object's velocity and distance traveled. The base W3DModelDraw
    /// doesn't generate UV overrides itself, but the architecture allows
    /// subclass overrides to contribute UV scrolling via this method.
    ///
    /// Returns an empty vec for the base W3DModelDraw. Subclasses like
    /// W3DTankDraw override this to provide tread UV scrolling.
    fn collect_mesh_uv_overrides(&self) -> Vec<MeshUvOverrideState> {
        Vec::new()
    }

    /// Apply instance scaling to the world transform.
    ///
    /// C++ parity: doDrawModule() checks getDrawable()->getInstanceScale()
    /// and scales the transform matrix if != 1.0. Also calls
    /// m_renderObject->Set_ObjectScale() for proper LOD calculations.
    fn apply_instance_scale(&self, transform_mtx: &Matrix3D) -> Matrix3D {
        let instance_scale = self
            .with_owner_drawable(|drawable| drawable.get_world_scale().x)
            .unwrap_or(1.0);

        if (instance_scale - 1.0).abs() < f32::EPSILON {
            *transform_mtx
        } else {
            let scale_mtx = Matrix3D::from_scale(Coord3D::splat(instance_scale));
            *transform_mtx * scale_mtx
        }
    }

    fn collect_bone_overrides(&self) -> Vec<BoneOverrideState> {
        let mut overrides = Vec::new();
        let Some(state) = self.current_state() else {
            return overrides;
        };

        for (index, turret) in state.turrets.iter().enumerate() {
            let (turret_angle, turret_pitch) = self.get_turret_angles(index);

            if turret.turret_angle_bone != 0 {
                let angle = turret_angle + turret.turret_art_angle;
                overrides.push(BoneOverrideState {
                    bone_index: turret.turret_angle_bone,
                    transform: Matrix3D::from_rotation_z(angle),
                });
            }

            if turret.turret_pitch_bone != 0 {
                let pitch = turret_pitch + turret.turret_art_pitch;
                overrides.push(BoneOverrideState {
                    bone_index: turret.turret_pitch_bone,
                    transform: Matrix3D::from_rotation_y(-pitch),
                });
            }
        }

        for wslot in 0..WEAPONSLOT_COUNT {
            let barrels = &state.weapon_barrels[wslot];
            let Some(recoils) = self.weapon_recoil_info.get(wslot) else {
                continue;
            };
            let count = barrels.len().min(recoils.len());
            for i in 0..count {
                let shift = recoils[i].shift;
                if barrels[i].recoil_bone != 0 && shift.abs() > 0.001 {
                    overrides.push(BoneOverrideState {
                        bone_index: barrels[i].recoil_bone,
                        transform: Matrix3D::from_translation(glam::Vec3::new(-shift, 0.0, 0.0)),
                    });
                }
            }
        }

        overrides
    }

    fn get_turret_angles(&self, turret_index: usize) -> (Real, Real) {
        let mut angle = 0.0;
        let mut pitch = 0.0;
        let Some(owner_id) = self.owner_id else {
            return (angle, pitch);
        };
        let Some(obj) = TheGameLogic::find_object_by_id(owner_id) else {
            return (angle, pitch);
        };
        let Ok(obj_guard) = obj.read() else {
            return (angle, pitch);
        };
        let Some(ai) = obj_guard.get_ai_update_interface() else {
            return (angle, pitch);
        };
        let Ok(ai_guard) = ai.lock() else {
            return (angle, pitch);
        };
        let turret_type = if turret_index == 0 {
            TurretType::Primary
        } else {
            TurretType::Secondary
        };
        if let Some((a, p)) = ai_guard.get_turret_rot_and_pitch(turret_type) {
            angle = a;
            pitch = p;
        }
        (angle, pitch)
    }
}
