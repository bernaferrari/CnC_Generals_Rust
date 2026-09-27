/// Compose authored ConditionState HideShowVec with runtime `m_subObjectVec` overrides.
///
/// C++ `doHideShowSubObjs` applies the state's list first, then if `m_subObjectVec`
/// is non-empty calls `updateSubObjects()` so UpgradeSubObject / A10 payload wins.
fn compose_hide_show_list(
    state_list: &[HideShowSubObjInfo],
    overrides: &[HideShowSubObjInfo],
) -> Vec<HideShowSubObjInfo> {
    let mut composed = state_list.to_vec();
    for override_entry in overrides {
        let key = override_entry.sub_obj_name.as_str();
        if key.is_empty() {
            continue;
        }
        if let Some(index) = composed.iter().position(|entry| {
            entry
                .sub_obj_name
                .as_str()
                .eq_ignore_ascii_case(key)
        }) {
            composed.remove(index);
        }
        // C++ applies the whole state list, then m_subObjectVec. Last write
        // wins, including bone children of this name, so the override must
        // sit after every state entry.
        composed.push(override_entry.clone());
    }
    composed
}

/// C++ `doHideShowProjectileObjects`: numbered launch bones only when the
/// authored hide-show name is empty; otherwise a single mesh toggled by hideCount.
fn projectile_clip_hide_show(
    hide_show_name: &str,
    launch_bone_name: &str,
    shots_remaining: u32,
    max_shots: u32,
) -> Vec<HideShowSubObjInfo> {
    let hide_count = max_shots.saturating_sub(shots_remaining);
    if hide_show_name.is_empty() {
        (0..max_shots)
            .map(|projectile_index| HideShowSubObjInfo {
                sub_obj_name: AsciiString::from(
                    format!("{}{:02}", launch_bone_name, projectile_index + 1).as_str(),
                ),
                hide: (projectile_index + 1) <= hide_count,
            })
            .collect()
    } else {
        vec![HideShowSubObjInfo {
            sub_obj_name: AsciiString::from(hide_show_name),
            hide: hide_count > 0,
        }]
    }
}

fn muzzle_flash_sub_object_name(prefix: &str, barrel_index: usize) -> String {
    // C++ hides `Get_Sub_Object_On_Bone(muzzleFlashBone)` only. The numbered
    // mesh is that child. The bare prefix is a different sub-object.
    format!("{prefix}{:02}", barrel_index + 1)
}

impl W3DModelDraw {
    fn composed_sub_object_visibility(&self) -> Vec<HideShowSubObjInfo> {
        let state_list = self
            .current_state()
            .map(|state| state.hide_show_list.as_slice())
            .unwrap_or(&[]);
        let clip: Vec<HideShowSubObjInfo> =
            self.projectile_clip_hides.iter().flatten().cloned().collect();
        let with_clip = compose_hide_show_list(state_list, &clip);
        let with_unsaved = compose_hide_show_list(&with_clip, &self.unsaved_subobject_hides);
        let with_saved = compose_hide_show_list(&with_unsaved, &self.sub_object_vec);
        compose_hide_show_list(&with_saved, &self.muzzle_flash_hides)
    }

    pub fn set_unsaved_subobject_hides(&mut self, entries: Vec<(String, bool)>) {
        self.unsaved_subobject_hides = entries
            .into_iter()
            .filter(|(name, _)| !name.is_empty())
            .map(|(name, hide)| HideShowSubObjInfo {
                sub_obj_name: AsciiString::from(name.as_str()),
                hide,
            })
            .collect();
        self.sub_objects_dirty = true;
    }

    fn note_muzzle_flash(&mut self, name: &str, show: bool) {
        let key = name.to_ascii_lowercase();
        if key.is_empty() {
            return;
        }
        if let Some(entry) = self.muzzle_flash_hides.iter_mut().find(|entry| {
            entry.sub_obj_name.as_str().eq_ignore_ascii_case(&key)
        }) {
            entry.hide = !show;
            self.sub_objects_dirty = true;
            return;
        }
        self.muzzle_flash_hides.push(HideShowSubObjInfo {
            sub_obj_name: AsciiString::from(key.as_str()),
            hide: !show,
        });
        self.sub_objects_dirty = true;
    }

    fn hide_all_muzzle_flashes(&mut self) {
        let Some(state) = self.current_state().cloned() else {
            return;
        };
        if !state.barrels_are_valid() {
            return;
        }
        self.muzzle_flash_hides.clear();
        for wslot in 0..WEAPONSLOT_COUNT {
            let prefix = state.weapon_muzzle_flash[wslot].as_str();
            if prefix.is_empty() {
                continue;
            }
            let prefix = prefix.to_string();
            for (barrel_index, barrel) in state.weapon_barrels[wslot].iter().enumerate() {
                if barrel.muzzle_flash_bone == 0 {
                    continue;
                }
                self.note_muzzle_flash(
                    &muzzle_flash_sub_object_name(&prefix, barrel_index),
                    false,
                );
            }
        }
    }

    fn set_muzzle_flash_hidden(&mut self, slot: usize, barrel_index: usize, hidden: bool) {
        let Some(state) = self.current_state() else {
            return;
        };
        let prefix = state.weapon_muzzle_flash[slot].as_str();
        if prefix.is_empty() {
            return;
        }
        self.note_muzzle_flash(
            &muzzle_flash_sub_object_name(prefix, barrel_index),
            !hidden,
        );
    }

    fn apply_projectile_clip_status(
        &mut self,
        shots_remaining: u32,
        max_shots: u32,
        weapon_slot: usize,
    ) {
        if weapon_slot >= WEAPONSLOT_COUNT || max_shots < shots_remaining {
            return;
        }
        if (self.data.projectile_bone_feedback_enabled_slots & (1u32 << weapon_slot)) == 0 {
            return;
        }
        let Some(state) = self.current_state() else {
            return;
        };
        let entries = projectile_clip_hide_show(
            state.weapon_projectile_hide_show_bone[weapon_slot].as_str(),
            state.weapon_projectile_launch_bone[weapon_slot].as_str(),
            shots_remaining,
            max_shots,
        );
        self.projectile_clip_hides[weapon_slot] = entries;
        self.sub_objects_dirty = true;
    }
}
