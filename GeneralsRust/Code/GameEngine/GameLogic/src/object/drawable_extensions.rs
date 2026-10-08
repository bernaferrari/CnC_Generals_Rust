//! Object and shared-handle adapters for accessing drawable state.
//!
//! These extension traits keep the original cross-subsystem API while
//! centralizing lock/error handling at the drawable boundary.

use super::*;

/// Extension trait for Object to provide Drawable access
pub trait DrawableExt {
    /// Get drawable associated with this object
    fn get_drawable(&self) -> Option<Arc<RwLock<Drawable>>>;
    fn set_drawable(&mut self, drawable: Option<Arc<RwLock<Drawable>>>);
}

#[derive(Debug, Clone)]
pub(crate) struct DrawableThingHandle {
    drawable: Weak<RwLock<Drawable>>,
}

impl DrawableThingHandle {
    pub fn new(drawable: &Arc<RwLock<Drawable>>) -> Self {
        Self {
            drawable: Arc::downgrade(drawable),
        }
    }

    pub fn upgrade(&self) -> Option<Arc<RwLock<Drawable>>> {
        self.drawable.upgrade()
    }
}

impl ModuleDrawableTrait for DrawableThingHandle {
    fn get_drawable_id(&self) -> u32 {
        self.upgrade()
            .and_then(|drawable| drawable.read().ok().map(|guard| guard.drawable_id))
            .unwrap_or(0)
    }
}

impl ModuleThing for DrawableThingHandle {
    fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
        Some(self)
    }
}

/// Operations on a shared Drawable handle with short, explicit borrows.
pub trait DrawableArcExt {
    fn get_id(&self) -> DrawableID;
    fn get_object_id(&self) -> ObjectID;
    fn get_model_condition_flags(&self) -> ModelConditionFlags;
    fn get_transform(&self) -> Matrix3D;
    fn get_instance_matrix(&self) -> Matrix3D;
    fn set_instance_matrix(&self, matrix: Option<&Matrix3D>);
    fn set_shadows_enabled(&self, enabled: bool);
    fn set_terrain_decal(&self, decal_type: TerrainDecalType);
    /// Synchronous borrowed-owner path; visual callbacks run outside Drawable/module guards.
    fn set_terrain_decal_for_object(&self, decal_type: TerrainDecalType, owner: &Object);
    fn set_terrain_decal_size(&self, x: Real, y: Real);
    fn set_terrain_decal_fade_target(&self, target: Real, rate: Real);
    fn init_rope_draw_params(
        &self,
        length: Real,
        width: Real,
        color: RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    );
    fn set_rope_cur_len(&self, length: Real);
    fn set_rope_speed(&self, cur_speed: Real, max_speed: Real, accel: Real);
    fn update_bones_for_client_particle_systems(&self) -> bool;
    fn get_laser_template_width(&self) -> Option<Real>;
    fn set_model_condition_state(&self, state: ModelConditionFlags);
    fn set_drawable_hidden(&self, hidden: bool);
    fn is_drawable_effectively_hidden(&self) -> bool;
    fn fade_in(&self, frames: UnsignedInt);
    fn fade_out(&self, frames: UnsignedInt);
    fn set_swaying_enabled(&self, enabled: bool);
    fn clear_model_condition_flags(&self, clear: ModelConditionFlags);
    fn clear_model_condition_state(&self, state: ModelConditionFlags);
    fn clear_and_set_model_condition_flags(
        &self,
        clear: &ModelConditionFlags,
        set: &ModelConditionFlags,
    );
    fn clear_and_set_model_condition_state(
        &self,
        clear: ModelConditionFlags,
        set: ModelConditionFlags,
    );
    fn get_projectile_launch_offset(
        &self,
        weapon_slot: WeaponSlotType,
        barrel_index: i32,
        turret_type: TurretType,
    ) -> Option<ProjectileLaunchOffset>;
    fn get_draw_modules(&self) -> Vec<DrawableModuleHandle>;
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectileLaunchOffset {
    pub transform: Matrix3D,
    pub turret_rot_pos: Coord3D,
    pub turret_pitch_pos: Coord3D,
}

impl DrawableArcExt for Arc<RwLock<Drawable>> {
    /// Get the drawable ID associated with this drawable
    fn get_id(&self) -> DrawableID {
        if let Ok(guard) = self.read() {
            guard.drawable_id
        } else {
            INVALID_ID
        }
    }

    fn get_object_id(&self) -> ObjectID {
        if let Ok(guard) = self.read() {
            guard.object_id
        } else {
            INVALID_ID
        }
    }

    /// Get the current model condition flags
    fn get_model_condition_flags(&self) -> ModelConditionFlags {
        if let Ok(guard) = self.read() {
            guard.model_conditions
        } else {
            ModelConditionFlags::empty()
        }
    }

    /// Get the current world transform
    fn get_transform(&self) -> Matrix3D {
        if let Ok(guard) = self.read() {
            guard.transform
        } else {
            Matrix3D::IDENTITY
        }
    }

    fn get_instance_matrix(&self) -> Matrix3D {
        if let Ok(guard) = self.read() {
            guard.instance_matrix.unwrap_or(Matrix3D::IDENTITY)
        } else {
            Matrix3D::IDENTITY
        }
    }

    /// Set the instance matrix for this drawable (used for jitter effects, rocking, etc.)
    fn set_instance_matrix(&self, matrix: Option<&Matrix3D>) {
        if let Ok(mut guard) = self.write() {
            guard.instance_matrix = matrix.cloned();
        }
    }

    /// Enable or disable shadow casting for this drawable
    fn set_shadows_enabled(&self, enabled: bool) {
        if let Ok(mut guard) = self.write() {
            guard.set_shadows_enabled(enabled);
        }
    }

    fn set_terrain_decal(&self, decal_type: TerrainDecalType) {
        if let Ok(mut guard) = self.write() {
            guard.set_terrain_decal(decal_type);
        }
    }

    fn set_terrain_decal_for_object(&self, decal_type: TerrainDecalType, owner: &Object) {
        let first = {
            let mut drawable = self.write().expect("terrain-decal Drawable poisoned");
            assert_eq!(
                drawable.object_id,
                owner.get_id(),
                "terrain-decal owner mismatch"
            );
            if drawable.terrain_decal == decal_type {
                return;
            }
            drawable.terrain_decal = decal_type;
            drawable
                .get_draw_modules_with_interface(ModuleInterfaceType::DRAW)
                .first()
                .cloned()
        };
        let Some(first) = first else {
            return;
        };
        let is_model = first.with_module(|module| {
            let mut is_model = false;
            let _ = with_draw_module_kind(module, |draw| match draw {
                DrawModuleKindMut::Model(model) => {
                    model.set_terrain_decal_type_for_object(decal_type, owner);
                    is_model = true;
                }
                other => other.set_terrain_decal(decal_type),
            });
            is_model
        });
        if !is_model {
            return;
        }
        // Existing temporary global visual bridge, captured once for this operation.
        // This does not establish per-game-instance client ownership.
        let Some(client) = crate::object::draw::terrain_decal_client() else {
            return;
        };
        // CPP releases the old decal BEFORE reading template/pose and allocating.
        // A client may inspect or change the actual Drawable/module synchronously.
        client.release(owner.get_id());
        if decal_type == TerrainDecalType::None {
            return;
        }
        let transform = self
            .read()
            .expect("terrain-decal Drawable poisoned")
            .get_transform_matrix();
        // Query sibling pristine bones only after releasing this draw entry.
        // C++ CACHE_ATTACH_BONE preserves the existing first-query cache.
        let attachment = first.with_module(|module| {
            let mut query = None;
            let _ = with_draw_module_kind(module, |draw| {
                if let DrawModuleKindMut::Model(model) = draw {
                    query = model.terrain_decal_attachment_query();
                }
            });
            query
        });
        let offset = attachment.map(|name| {
            self.read()
                .expect("terrain-decal Drawable poisoned")
                .get_pristine_bone_positions(name.as_str(), 0, 1)
                .into_iter()
                .next()
                .unwrap_or(Coord3D::origin())
        });
        let command = first.with_module(|module| {
            let mut command = None;
            let _ = with_draw_module_kind(module, |draw| match draw {
                DrawModuleKindMut::Model(model) => {
                    command = model
                        .prepare_terrain_decal_for_object(decal_type, owner, &transform, offset);
                }
                _ => unreachable!("first terrain-decal draw module changed kind"),
            });
            command
        });
        // Neither the Drawable nor its first module is borrowed by the client callback.
        if let Some(desc) = command {
            client.set_decal(&desc);
        }
    }

    fn set_terrain_decal_size(&self, x: Real, y: Real) {
        if let Ok(mut guard) = self.write() {
            guard.set_terrain_decal_size(x, y);
        }
    }

    /// Set terrain decal fade target and rate
    fn set_terrain_decal_fade_target(&self, target: Real, rate: Real) {
        if let Ok(mut guard) = self.write() {
            guard.set_terrain_decal_fade_target(target, rate);
        }
    }

    fn init_rope_draw_params(
        &self,
        length: Real,
        width: Real,
        color: RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    ) {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_rope_draw_interface_mut(module, |rope| {
                        rope.init_rope_parms(
                            length,
                            width,
                            &color,
                            wobble_len,
                            wobble_amp,
                            wobble_rate,
                        );
                    });
                });
            }
        }
    }

    fn set_rope_cur_len(&self, length: Real) {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_rope_draw_interface_mut(module, |rope| {
                        rope.set_rope_cur_len(length);
                    });
                });
            }
        }
    }

    fn set_rope_speed(&self, cur_speed: Real, max_speed: Real, accel: Real) {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_rope_draw_interface_mut(module, |rope| {
                        rope.set_rope_speed(cur_speed, max_speed, accel);
                    });
                });
            }
        }
    }

    fn update_bones_for_client_particle_systems(&self) -> bool {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                let updated = module_handle.with_module(|module| {
                    let mut result = false;
                    with_draw_module_mut(module, |draw| {
                        result = draw.update_bones_for_client_particle_systems();
                    });
                    result
                });
                if updated {
                    return true;
                }
            }
        }

        false
    }

    fn get_laser_template_width(&self) -> Option<Real> {
        let guard = self.read().ok()?;
        for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            let width = module_handle.with_module(|module| {
                let mut width = None;
                with_draw_module_mut(module, |draw| {
                    if let Some(laser) = draw.get_laser_draw_interface() {
                        width = Some(laser.get_laser_template_width());
                    }
                });
                width
            });
            if width.is_some() {
                return width;
            }
        }

        None
    }

    /// Set model condition state
    fn set_model_condition_state(&self, state: ModelConditionFlags) {
        if let Ok(mut guard) = self.write() {
            guard.set_model_condition_state(state);
        }
    }

    /// Set whether the drawable is hidden
    fn set_drawable_hidden(&self, hidden: bool) {
        if let Ok(mut guard) = self.write() {
            let _ = guard.set_drawable_hidden(hidden);
        }
    }

    fn fade_in(&self, frames: UnsignedInt) {
        if let Ok(mut guard) = self.write() {
            guard.fade_in(frames);
        }
    }

    fn fade_out(&self, frames: UnsignedInt) {
        if let Ok(mut guard) = self.write() {
            guard.fade_out(frames);
        }
    }

    fn set_swaying_enabled(&self, enabled: bool) {
        if let Ok(mut guard) = self.write() {
            guard.set_swaying_enabled(enabled);
        }
    }

    /// Check if the drawable is effectively hidden (by explicit hide or stealth)
    /// Matches C++ Drawable.h line 305: isDrawableEffectivelyHidden()
    fn is_drawable_effectively_hidden(&self) -> bool {
        if let Ok(guard) = self.read() {
            guard.is_drawable_effectively_hidden()
        } else {
            false
        }
    }

    /// Clear model condition flags
    fn clear_model_condition_flags(&self, clear: ModelConditionFlags) {
        if let Ok(mut guard) = self.write() {
            guard.clear_model_condition_flags(clear);
        }
    }

    fn clear_model_condition_state(&self, state: ModelConditionFlags) {
        if let Ok(mut guard) = self.write() {
            guard.clear_model_condition_state(state);
        }
    }

    /// Clear and set model condition flags atomically
    fn clear_and_set_model_condition_flags(
        &self,
        clear: &ModelConditionFlags,
        set: &ModelConditionFlags,
    ) {
        if let Ok(mut guard) = self.write() {
            guard.clear_and_set_model_condition_state(*clear, *set);
        }
    }

    /// Clear and set model condition state atomically (alias for clear_and_set_model_condition_flags)
    /// This method provides backward compatibility with code expecting this method name
    fn clear_and_set_model_condition_state(
        &self,
        clear: ModelConditionFlags,
        set: ModelConditionFlags,
    ) {
        if let Ok(mut guard) = self.write() {
            guard.clear_and_set_model_condition_state(clear, set);
        }
    }

    /// Get projectile launch offset for a specific weapon slot and barrel
    fn get_projectile_launch_offset(
        &self,
        weapon_slot: WeaponSlotType,
        barrel_index: i32,
        turret_type: TurretType,
    ) -> Option<ProjectileLaunchOffset> {
        self.read()
            .ok()?
            .projectile_launch_offset(weapon_slot, barrel_index, turret_type)
    }

    /// Get all draw modules registered with this drawable
    fn get_draw_modules(&self) -> Vec<DrawableModuleHandle> {
        if let Ok(guard) = self.read() {
            guard.modules()
        } else {
            Vec::new()
        }
    }
}

impl Drawable {
    /// Resolve one launch query on this driving Drawable without owner lookup.
    pub(crate) fn projectile_launch_offset(
        &self,
        weapon_slot: WeaponSlotType,
        barrel_index: i32,
        turret_type: TurretType,
    ) -> Option<ProjectileLaunchOffset> {
        let condition = self.model_conditions;
        let mut launch_pos = Matrix3D::IDENTITY;
        let mut turret_rot_pos = Coord3D::origin();
        let mut turret_pitch_pos = Coord3D::origin();

        // Iterate through all draw modules and find one that can provide the launch offset
        for module_handle in self.modules() {
            let plan = module_handle.with_module(|module| {
                let mut plan = None;
                with_object_draw_interface_mut(module, |draw| {
                    plan = draw.prepare_projectile_launch_offset(
                        &condition,
                        weapon_slot as usize,
                        barrel_index,
                        turret_type,
                    );
                });
                plan
            });
            let Some(plan) = plan else {
                continue;
            };
            // No DrawModuleEntry guard is held while the driving Drawable
            // searches its entries in pristine-bone order, including this one.
            let attachment = plan.attachment_bone.as_ref().map(|name| {
                let offset = self
                    .get_pristine_bone_positions(name.as_str(), 0, 1)
                    .into_iter()
                    .next()
                    .unwrap_or(Coord3D::origin());
                module_handle.with_module(|module| {
                    let mut result = offset;
                    with_object_draw_interface_mut(module, |draw| {
                        result = draw.cache_projectile_attachment(name.as_str(), offset);
                    });
                    result
                })
            });
            let fallback = std::array::from_fn(|index| {
                let key = plan.fallback_pivots[index];
                if key == 0 {
                    return None;
                }
                let name = NameKeyGenerator::key_to_name(key)?;
                self.get_bone_local_transform(&name)
                    .map(|transform| transform.w_axis.truncate())
            });
            let found = plan.finish(
                attachment,
                fallback,
                &mut launch_pos,
                &mut turret_rot_pos,
                &mut turret_pitch_pos,
            );

            if found {
                return Some(ProjectileLaunchOffset {
                    transform: launch_pos,
                    turret_rot_pos,
                    turret_pitch_pos,
                });
            }
        }

        None
    }
}
