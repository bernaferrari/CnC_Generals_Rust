//! Cache-only host configuration of C++ `Drawable::getBarrelCount`.
//!
//! `Weapon::privateFireWeapon` asks the live Drawable for a slot's count just
//! before it chooses the barrel.  Main's renderer must never write GameLogic,
//! so the equivalent answer is built from the exact current Object INI Draw
//! state and a previously validated W3D topology at a map/startup boundary.
//! A combat tick only consults that immutable cache; it never opens an archive
//! or guesses from a template/model suffix.

use super::super::*;

impl GameLogic {
    /// Apply cache-resident C++ `Drawable::getBarrelCount` answers to one
    /// concrete host Object before an accepted WeaponSet discharge.
    ///
    /// `false` means the current exact Draw state is not yet known or cannot
    /// be represented by Main's supported rigid W3D topology. The Object
    /// deliberately retains its existing runtime topology in that case: an
    /// unavailable cache lookup is not evidence that C++ Drawable returned
    /// zero barrels.
    pub(in super::super) fn configure_cached_weapon_barrel_topology_for_object(
        &mut self,
        source: ObjectId,
    ) -> bool {
        let (object_name, condition_bits) = match self.objects.get(&source) {
            Some(object) if !object.template_name.trim().is_empty() => {
                (object.template_name.clone(), object.model_condition_bits)
            }
            _ => return false,
        };

        let Some(manager_arc) = crate::assets::get_asset_manager() else {
            return false;
        };
        // The model catalogue is shared with WGPU preparation. A fixed-step
        // combat path must not wait for it: map/start prewarming is the
        // authorized I/O boundary, and a busy cache leaves the prior topology
        // untouched (the first-use default is one barrel).
        let Ok(mut manager) = manager_arc.try_lock() else {
            return false;
        };
        let Some(counts) =
            manager.cached_weapon_barrel_counts_for_object_conditions(&object_name, condition_bits)
        else {
            return false;
        };
        drop(manager);

        let Some(object) = self.objects.get_mut(&source) else {
            return false;
        };
        for slot in 0..3u8 {
            if object.weapon_slot(slot).is_some() {
                // A successful cache lookup with no first nonzero Draw-module
                // answer is C++ `Drawable::getBarrelCount(slot) == 0`. Keep
                // that distinct from the outer unavailable-cache case above:
                // `Weapon::privateFireWeapon` resets its cursor/cadence before
                // every zero-topology shot.
                let _ = object
                    .set_weapon_barrel_count_for_slot(slot, counts[slot as usize].unwrap_or(0));
            }
        }
        true
    }

    /// C++ `Drawable::handleWeaponFireFX` (`W3DModelDraw.cpp:3672-3727`):
    /// the fired barrel's FireFX plays at that barrel's FX-bone world matrix.
    ///
    /// Host equivalent resolved from the same cache boundary as
    /// [`GameLogic::configure_cached_weapon_barrel_topology_for_object`]:
    /// the cached per-condition barrel counts prove the barrel topology
    /// exists, the selected Draw models supply the authored FX-bone base,
    /// and the pristine W3D pose hook supplies the bone offset. Returns the
    /// host Y-up world position plus the bone's Z-rotation (the caller
    /// composes it onto the source object matrix). `None` means no cached
    /// topology or no FX bone: callers must fall back to the source position.
    pub(in super::super) fn weapon_fire_fx_bone_world(
        &self,
        source: ObjectId,
        weapon_slot: u8,
        fired_barrel: u8,
    ) -> Option<(Vec3, f32)> {
        if usize::from(weapon_slot) >= 3 {
            return None;
        }
        let object = self.objects.get(&source)?;
        if object.template_name.trim().is_empty() {
            return None;
        }

        let manager_arc = crate::assets::get_asset_manager()?;
        // The model catalogue is shared with WGPU preparation. A fixed-step
        // combat path must not wait for it: a busy cache is a fallback to the
        // source position, never evidence about barrels.
        let mut manager = manager_arc.try_lock().ok()?;
        let barrel_count = manager
            .cached_weapon_barrel_counts_for_object_conditions(
                &object.template_name,
                object.model_condition_bits,
            )?
            .get(usize::from(weapon_slot))
            .copied()
            .flatten()?;
        let fx_base = manager
            .select_draw_models_for_object_conditions(
                &object.template_name,
                object.model_condition_bits,
            )?
            .into_iter()
            .find_map(|model| {
                if !model.weapon_bone_bindings.source_fields_valid {
                    return None;
                }
                model
                    .weapon_bone_bindings
                    .slot(weapon_slot)
                    .and_then(|slot| slot.fire_fx_bone_base.clone())
                    .filter(|base| !base.trim().is_empty())
            })?;
        drop(manager);

        if barrel_count == 0 || u32::from(fired_barrel) >= u32::from(barrel_count) {
            return None;
        }

        // C++ `validateWeaponBarrelInfo` names barrels `{base}01`.. and a
        // numbered FireFX record reuses the previous numbered FX pivot when
        // its own pivot is missing (retail multi-flash exception); the
        // unadorned base is the single-barrel name.
        let model_name = object.thing.template.get_model_name();
        let scale = object.thing.template.asset_scale;
        let mut pose = None;
        for index in (1..=u32::from(fired_barrel) + 1).rev() {
            let numbered = format!("{fx_base}{index:02}");
            if let Some(found) =
                gamelogic::object::draw::lookup_pristine_bone_pose(model_name, scale, &numbered)
            {
                pose = Some(found);
                break;
            }
        }
        let (bone_local, bone_yaw) = pose.or_else(|| {
            gamelogic::object::draw::lookup_pristine_bone_pose(model_name, scale, &fx_base)
        })?;

        // C++ Z-up bone offset → host Y-up, then yaw-rotate into world space
        // (same mapping as the FIREPOINT helpers in `drones_and_garrison`).
        let local = Vec3::new(bone_local.x, bone_local.z, bone_local.y);
        let (sin, cos) = object.get_orientation().sin_cos();
        let world = object.get_position()
            + Vec3::new(
                local.x * cos - local.z * sin,
                local.y,
                local.x * sin + local.z * cos,
            );
        Some((world, bone_yaw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_zero_barrel_topology_resets_cxx_prefire_cadence_on_every_shot() {
        let mut object = Object::new(
            ThingTemplate::new("KnownZeroTopology"),
            ObjectId(114),
            Team::USA,
        );
        object.weapon = Some(Weapon {
            damage: 1.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.weapon_barrel_states[0] =
            crate::game_logic::object::WeaponBarrelState::new(3, 4, None);
        object.weapon_barrel_states[0].current_barrel = 3;
        object.weapon_barrel_states[0].shots_left_on_barrel = 1;

        // This is the exact conversion after a valid Draw sequence proves
        // `Drawable::getBarrelCount(slot) == 0`, not a cache miss.
        assert!(object.set_weapon_barrel_count_for_slot(0, 0));
        let state = object
            .weapon_barrel_state_for_slot(0)
            .expect("attached PRIMARY has a cursor");
        assert_eq!(state.barrel_count, 0);
        // C++ retains a post-last-shot overflow cursor until the next actual
        // `Weapon::privateFireWeapon` compares it with Drawable's current
        // count. Configuring a Draw state is not itself a fire operation.
        assert_eq!(state.current_barrel, 3);

        for _ in 0..2 {
            assert_eq!(object.fired_barrel_for_slot(0), Some(0));
            object.advance_weapon_barrel_after_shot(0);
            let state = object
                .weapon_barrel_state_for_slot(0)
                .expect("attached PRIMARY has a cursor");
            assert_eq!(
                (state.current_barrel, state.shots_left_on_barrel),
                (0, 2),
                "every zero-topology pre-fire resets a three-shot cadence before the shot"
            );
        }

        // When a later condition state exposes two barrels, C++ carries the
        // state left by the immediately preceding zero-topology shot. It does
        // not fabricate a one-barrel cadence or reset merely on state change.
        assert!(object.set_weapon_barrel_count_for_slot(0, 2));
        assert_eq!(object.fired_barrel_for_slot(0), Some(0));
        let state = object
            .weapon_barrel_state_for_slot(0)
            .expect("attached PRIMARY has a cursor");
        assert_eq!((state.current_barrel, state.shots_left_on_barrel), (0, 2));
        object.advance_weapon_barrel_after_shot(0);
        assert_eq!(
            object
                .weapon_barrel_state_for_slot(0)
                .expect("attached PRIMARY has a cursor")
                .shots_left_on_barrel,
            1
        );
    }
}
