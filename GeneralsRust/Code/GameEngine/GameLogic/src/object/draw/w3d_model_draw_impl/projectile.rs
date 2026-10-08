impl W3DModelDraw {
    // CPP W3DModelDraw.cpp:3232-3372. Select the logic condition once, rather
    // than using the client's current (possibly transitional) animation state.
    fn projectile_launch_plan(
        &self,
        condition: &ModelConditionFlags,
        weapon_slot: usize,
        barrel_index: i32,
        turret_type: TurretType,
    ) -> Option<ProjectileLaunchPlan> {
        if weapon_slot >= WEAPONSLOT_COUNT {
            return None;
        }
        let state = self.data.find_best_info(condition)?;
        let turret_index = match turret_type {
            TurretType::Primary => Some(0),
            TurretType::Secondary => Some(1),
            TurretType::Invalid => None,
        };
        let turret = turret_index.and_then(|index| state.turrets.get(index));
        let barrels = &state.weapon_barrels[weapon_slot];
        let index = if barrel_index < 0 || barrel_index as usize >= barrels.len() {
            0
        } else {
            barrel_index as usize
        };
        let launch = barrels.get(index).map(|barrel| {
            let mut transform = barrel.projectile_offset_mtx;
            if let Some(turret) = turret {
                transform = Matrix3D::from_rotation_z(turret.turret_art_angle) * transform;
                transform = Matrix3D::from_rotation_y(-turret.turret_art_pitch) * transform;
            }
            transform
        });

        // C++ still computes turret pivots for an empty barrel vector, but
        // TURRET_INVALID must never initialize the attachment cache.
        let mut attachment_bone = None;
        let mut cached_attachment = None;
        if turret_type != TurretType::Invalid && !self.data.attach_to_drawable_bone.is_empty() {
            cached_attachment = self
                .attach_offset_cache
                .lock()
                .ok()
                .and_then(|cache| *cache);
            if cached_attachment.is_none() {
                attachment_bone = Some(self.data.attach_to_drawable_bone.clone());
            }
        }
        let keys = turret
            .map(|info| [info.turret_angle_name_key, info.turret_pitch_name_key])
            .unwrap_or([0; 2]);
        let mut turret_positions = [Coord3D::origin(); 2];
        let mut fallback_pivots = [0; 2];
        for (index, key) in keys.into_iter().enumerate() {
            if key == 0 {
                continue;
            }
            if let Some(bone) = state.pristine_bones.get(&key) {
                turret_positions[index] = bone.transform.w_axis.truncate();
            } else {
                fallback_pivots[index] = key;
            }
        }
        Some(ProjectileLaunchPlan {
            launch,
            apply_attachment: turret.is_some(),
            turret_positions,
            fallback_pivots,
            attachment_bone,
            cached_attachment,
        })
    }

    fn commit_projectile_attachment(&mut self, bone: &str, offset: Coord3D) -> Coord3D {
        // An escaped handle may replace definition data between preparation
        // and resolution. Finish the frozen query without caching it against
        // a different attachment name. No animation/state effects are resumed.
        if self.data.attach_to_drawable_bone.as_str() != bone {
            return offset;
        }
        match self.attach_offset_cache.get_mut() {
            Ok(cache) => *cache.get_or_insert(offset),
            // The old lazy getter returned the queried value without caching
            // when this private mutex was poisoned. Preserve that behavior.
            Err(_) => offset,
        }
    }
}
