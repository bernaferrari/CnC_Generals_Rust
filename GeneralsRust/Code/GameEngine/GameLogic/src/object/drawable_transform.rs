//! Drawable opacity, model conditions, transforms, and bone/animation queries.
//!
//! The methods stay on `Drawable` to preserve the public surface and are
//! grouped here by their transform/model-state responsibilities.

use super::*;

impl Drawable {
    /// C++ FadingMode stored as u32 for xfer parity (Drawable.cpp:5068).
    pub const FADING_NONE: u32 = 0;
    pub const FADING_IN: u32 = 1;
    pub const FADING_OUT: u32 = 2;

    /// C++ `Drawable::setDrawableOpacity` — sets explicit/override alpha.
    pub fn set_drawable_opacity(&mut self, opacity: Real) {
        self.alpha = opacity.clamp(0.0, 1.0);
    }

    /// C++ `Drawable::friend_getExplicitOpacity`.
    pub fn get_explicit_opacity(&self) -> Real {
        self.alpha
    }

    /// C++ `Drawable::getEffectiveOpacity` = explicit * stealth pulse.
    pub fn get_effective_opacity(&self) -> Real {
        (self.alpha * self.effective_stealth_opacity).clamp(0.0, 1.0)
    }

    pub fn fading_mode(&self) -> u32 {
        self.fade_mode
    }

    pub fn time_to_fade(&self) -> UnsignedInt {
        self.time_to_fade
    }

    pub fn time_elapsed_fade(&self) -> UnsignedInt {
        self.time_elapsed_fade
    }

    pub fn is_fading(&self) -> bool {
        self.fade_mode != Self::FADING_NONE
    }

    /// C++ `Drawable::fadeIn` (Drawable.cpp:1059-1065).
    /// OCL GenericObjectCreationNugget calls this when `FadeIn` is set.
    pub fn fade_in(&mut self, frames: UnsignedInt) {
        self.set_drawable_opacity(0.0);
        self.fade_mode = Self::FADING_IN;
        self.time_elapsed_fade = 0;
        self.time_to_fade = frames.max(1);
    }

    /// C++ `Drawable::fadeOut` (Drawable.cpp:1048-1054).
    pub fn fade_out(&mut self, frames: UnsignedInt) {
        self.set_drawable_opacity(1.0);
        self.fade_mode = Self::FADING_OUT;
        self.time_elapsed_fade = 0;
        self.time_to_fade = frames.max(1);
    }

    /// One C++ `updateDrawable` fade tick: ramp opacity, then increment elapsed.
    pub fn update_fade(&mut self) {
        if self.fade_mode == Self::FADING_NONE {
            return;
        }
        let numerator = if self.fade_mode == Self::FADING_IN {
            self.time_elapsed_fade as Real
        } else {
            self.time_to_fade.saturating_sub(self.time_elapsed_fade) as Real
        };
        let denom = self.time_to_fade.max(1) as Real;
        self.set_drawable_opacity((numerator / denom).clamp(0.0, 1.0));
        self.time_elapsed_fade = self.time_elapsed_fade.saturating_add(1);
        if self.time_elapsed_fade > self.time_to_fade {
            self.fade_mode = Self::FADING_NONE;
        }
    }

    /// Set effective stealth opacity using C++ pulse semantics.
    /// `pulse_factor` is clamped [0..1], and `explicit_opacity` updates the stealth floor when set.
    pub fn set_effective_opacity(&mut self, pulse_factor: Real, explicit_opacity: Option<Real>) {
        if let Some(opacity) = explicit_opacity {
            self.stealth_factor = opacity.clamp(0.0, 1.0);
        }

        let pf = pulse_factor.clamp(0.0, 1.0);
        let pulse_margin = 1.0 - self.stealth_factor;
        let pulse_amount = pulse_margin * pf;
        self.effective_stealth_opacity = (self.stealth_factor + pulse_amount).clamp(0.0, 1.0);
    }

    /// C++ Drawable::setSecondMaterialPassOpacity — heat-vision overlay.
    pub fn set_second_material_pass_opacity(&mut self, opacity: Real) {
        self.second_material_pass_opacity = opacity.clamp(0.0, 1.0);
    }

    pub fn get_second_material_pass_opacity(&self) -> Real {
        self.second_material_pass_opacity
    }

    /// Set stealth appearance mode and hidden state (C++ Drawable::setStealthLook parity).
    pub fn set_stealth_look(&mut self, look: StealthLookType) {
        if look == self.stealth_look {
            return;
        }

        // C++ parity: reset stealth floor before applying look-specific behavior.
        self.stealth_factor = 1.0;

        let is_mine = self
            .object_ref
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|object| {
                object
                    .read()
                    .ok()
                    .map(|guard| guard.is_kind_of(KindOf::Mine))
            })
            .unwrap_or(false);

        self.stealth_look = look;
        self.hidden_by_stealth = matches!(look, StealthLookType::Invisible);
        self.second_material_pass_opacity = match look {
            StealthLookType::VisibleDetected if !is_mine => 1.0,
            StealthLookType::VisibleFriendlyDetected if !is_mine => 1.0,
            _ => 0.0,
        };

        // C++ parity: disable shadows while in globally detected visualization state.
        self.set_shadows_enabled(!matches!(look, StealthLookType::VisibleDetected));

        self.update_hidden_status();
    }

    pub fn get_stealth_look(&self) -> StealthLookType {
        self.stealth_look
    }

    /// Helper to update the model based on conditional model settings
    pub(super) fn update_conditional_model(&mut self) {
        // Check if we need to switch models based on conditions
        for (flags, model_name) in &self.conditional_models {
            if self.model_conditions.intersects(*flags) {
                self.model_name = model_name.clone();
                return;
            }
        }
    }

    pub(super) fn propagate_model_condition_state_to_draw_modules(&mut self) {
        let conditions = self.model_conditions;
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.replace_model_condition_state(&conditions);
                });
            });
        }
    }

    /// Clear model condition flags
    /// Updates the model conditions by clearing the specified flags
    pub fn clear_model_condition_flags(&mut self, clear: ModelConditionFlags) {
        self.model_conditions &= !clear;
        self.update_conditional_model();
    }

    /// Clear garrisoned model condition
    pub fn clear_model_condition_garrisoned(&mut self) -> Result<(), String> {
        self.model_conditions &= !ModelConditionFlags::GARRISONED;
        self.update_conditional_model();
        Ok(())
    }

    /// Set the orientation (rotation) of the drawable
    /// Updates the world rotation to reflect the new angle
    pub fn set_orientation(&mut self, angle: Real) {
        // Update the Y-axis rotation (yaw) and rebuild world transform.
        self.world_rotation.y = angle;
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            self.world_rotation.x,
            self.world_rotation.y,
            self.world_rotation.z,
        );
        let new_transform = Matrix3D::from_scale_rotation_translation(
            self.world_scale,
            rotation,
            self.world_position,
        );
        self.set_transform(new_transform);
    }

    /// Get the current world position of the drawable
    pub fn get_position(&self) -> Coord3D {
        self.world_position
    }

    /// Get the current world transform matrix.
    pub fn get_transform_matrix(&self) -> Matrix3D {
        self.transform
    }

    /// Get current world-space bounding box.
    pub fn get_bounding_box(&self) -> BoundingBox {
        self.bounding_box.clone()
    }

    /// Get current world-space bounding sphere radius.
    pub fn get_bounding_sphere_radius(&self) -> Real {
        self.bounding_sphere.radius
    }

    /// Get decomposed world scale used by bone-space conversions.
    pub fn get_world_scale(&self) -> Coord3D {
        self.world_scale
    }

    /// Get the object associated with this drawable
    pub fn get_object(&self) -> Option<Arc<rhai::Locked<crate::object::Object>>> {
        self.object_ref.as_ref().and_then(|weak| weak.upgrade())
    }

    pub(crate) fn bind_object_ref(&mut self, object: &Arc<RwLock<crate::object::Object>>) {
        self.object_ref = Some(Arc::downgrade(object));
    }

    /// C++ `Drawable::friend_bindToObject` + `GameLogic::bindObjectAndDrawable`.
    pub fn friend_bind_to_object(&mut self, object: &Arc<RwLock<crate::object::Object>>) {
        let Some(new_object_id) = object.read().ok().map(|guard| guard.get_id()) else {
            return;
        };
        let previous_object_id = self.object_id;
        if let Some(client) = TheGameClient::get() {
            // A replacement Drawable for the *same* object must also retire
            // its predecessor's model output. C++ has no cross-drawable global
            // output cache to make that stale state visible for another frame.
            client.clear_object_model_draws(new_object_id);
            if previous_object_id != new_object_id {
                client.clear_object_model_draws(previous_object_id);
            }
        }
        self.object_id = new_object_id;
        self.bind_object_ref(object);

        self.notify_draw_modules_bound_to_current_object();
    }

    /// Notify draw modules only after this Drawable has a resolved gameplay
    /// object association. Used by both the ordinary bind path and Xfer load.
    pub(super) fn notify_draw_modules_bound_to_current_object(&mut self) {
        if self.object_id == INVALID_ID
            || self
                .object_ref
                .as_ref()
                .and_then(|weak| weak.upgrade())
                .is_none()
        {
            return;
        }

        // C++ Drawable::friend_bindToObject notifies each DrawModule after the
        // object association has been installed.  Rebinding owner IDs here is
        // essential: the normal GameClient allocation path constructs with
        // INVALID_ID and the factory path may construct before this callback.
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| {
                    draw.bind_owner_id(self.object_id);
                });
                module.on_drawable_bound_to_object();
            });
        }
    }

    /// Get current worldspace client bone positions
    /// Returns the transform matrix for a specific bone in worldspace
    pub fn get_current_worldspace_client_bone_positions(
        &self,
        bone_name: &str,
    ) -> Option<Matrix3D> {
        let bone_name_ascii = AsciiString::from(bone_name);
        for module_handle in self.modules() {
            let mut world_bone = Matrix3D::IDENTITY;
            let found = module_handle.with_module(|module| {
                let mut found = false;
                with_draw_module_mut(module, |draw| {
                    if let Some(interface) = draw.get_object_draw_interface_mut() {
                        found = interface.client_only_get_render_obj_bone_transform(
                            &bone_name_ascii,
                            &mut world_bone,
                        );
                    }
                });
                found
            });

            if found {
                return Some(world_bone);
            }
        }

        // Fallback for partially ported draw modules that expose skeleton data directly.
        self.get_bone_transform(bone_name)
    }

    /// C++ `Drawable::getCurrentClientBonePositions` (Drawable.cpp:776-802).
    /// Walks W3D draw modules' current (animated) client bones.
    pub fn get_current_client_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
    ) -> i32 {
        let mut count = 0;
        let mut remaining = positions.len().min(transforms.len());
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            if remaining == 0 {
                break;
            }
            let sub = module_handle
                .with_object_draw_interface(|draw| {
                    draw.get_current_bone_positions(
                        bone_name_prefix,
                        start_index,
                        &mut positions[count..],
                        &mut transforms[count..],
                        remaining,
                    )
                })
                .unwrap_or(0);
            if sub > 0 {
                count += sub;
                remaining = remaining.saturating_sub(sub);
            }
        }
        count as i32
    }

    /// Set animation to loop in N frames
    ///
    /// This call says, "I want the current animation (if any) to take n frames to complete a single cycle".
    /// If it's a looping anim, each loop will take n frames.
    /// Note that you must call this AFTER setting the condition codes.
    ///
    /// Reference: C++ Drawable.h:469 - setAnimationLoopDuration
    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| match draw {
                    DrawModuleKindMut::Model(w3d_draw) => {
                        w3d_draw.set_animation_loop_duration(num_frames)
                    }
                    DrawModuleKindMut::Tank(w3d_draw) => {
                        w3d_draw.set_animation_loop_duration(num_frames)
                    }
                    DrawModuleKindMut::TankTruck(w3d_draw) => {
                        w3d_draw.set_animation_loop_duration(num_frames)
                    }
                    _ => {}
                });
            });
        }
    }

    /// Set animation completion time
    ///
    /// Similar to setAnimationLoopDuration, but assumes that the current state is a "ONCE",
    /// and is smart about transition states... if there is a transition state "inbetween",
    /// it is included in the completion time.
    ///
    /// Reference: C++ Drawable.h:475 - setAnimationCompletionTime
    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| match draw {
                    DrawModuleKindMut::Model(w3d_draw) => {
                        w3d_draw.set_animation_completion_time(num_frames)
                    }
                    DrawModuleKindMut::Tank(w3d_draw) => {
                        w3d_draw.set_animation_completion_time(num_frames)
                    }
                    DrawModuleKindMut::TankTruck(w3d_draw) => {
                        w3d_draw.set_animation_completion_time(num_frames)
                    }
                    _ => {}
                });
            });
        }
    }

    /// Set animation frame manually
    ///
    /// Manually set a drawable's current animation to a specific frame.
    ///
    /// Reference: C++ Drawable.h:478 - setAnimationFrame
    pub fn set_animation_frame(&mut self, frame: i32) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| match draw {
                    DrawModuleKindMut::Model(w3d_draw) => w3d_draw.set_animation_frame(frame),
                    DrawModuleKindMut::Tank(w3d_draw) => w3d_draw.set_animation_frame(frame),
                    DrawModuleKindMut::TankTruck(w3d_draw) => w3d_draw.set_animation_frame(frame),
                    _ => {}
                });
            });
        }
    }

    /// Show or hide a named sub-object on the drawable.
    /// Mirrors C++ Drawable::showSubObject.
    pub fn show_sub_object(&mut self, name: &str, show: bool) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| match draw {
                    DrawModuleKindMut::Model(w3d_draw) => w3d_draw.show_sub_object(name, show),
                    DrawModuleKindMut::Tank(w3d_draw) => w3d_draw.show_sub_object(name, show),
                    DrawModuleKindMut::TankTruck(w3d_draw) => w3d_draw.show_sub_object(name, show),
                    _ => {}
                });
            });
        }
    }

    /// Update supply crate visual status on draw modules.
    /// Matches C++ Drawable::updateDrawableSupplyStatus.
    pub fn update_supply_status(&mut self, max_supply: i32, current_supply: i32) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.update_supply_status(max_supply, current_supply);
                });
            });
        }
    }

    /// Update projectile clip status for draw modules.
    /// Mirrors C++ Drawable::updateDrawableClipStatus.
    pub fn update_drawable_clip_status(
        &mut self,
        shots_remaining: u32,
        max_shots: u32,
        weapon_slot: WeaponSlotType,
    ) {
        let slot_index = weapon_slot as usize;
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.update_projectile_clip_status(shots_remaining, max_shots, slot_index);
                });
            });
        }
    }

    /// Route weapon-fire FX handling through draw modules.
    /// Mirrors C++ `Drawable::handleWeaponFireFX`.
    pub fn handle_weapon_fire_fx(
        &mut self,
        weapon_slot: WeaponSlotType,
        barrel_index: i32,
        victim_pos: &Coord3D,
    ) -> bool {
        let slot_index = weapon_slot as usize;
        let mut handled = false;
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    if draw.handle_weapon_fire_fx(slot_index, barrel_index, victim_pos) {
                        handled = true;
                    }
                });
            });
        }
        handled
    }

    /// Apply pending sub-object visibility changes.
    /// Mirrors C++ Drawable::updateSubObjects.
    pub fn update_sub_objects(&mut self) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| match draw {
                    DrawModuleKindMut::Model(w3d_draw) => w3d_draw.update_sub_objects(),
                    DrawModuleKindMut::Tank(w3d_draw) => w3d_draw.update_sub_objects(),
                    DrawModuleKindMut::TankTruck(w3d_draw) => w3d_draw.update_sub_objects(),
                    _ => {}
                });
            });
        }
    }
}
