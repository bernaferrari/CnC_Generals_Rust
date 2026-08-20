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
        if let Some(existing) = composed.iter_mut().find(|entry| {
            entry
                .sub_obj_name
                .as_str()
                .eq_ignore_ascii_case(key)
        }) {
            existing.hide = override_entry.hide;
        } else {
            composed.push(override_entry.clone());
        }
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
        if launch_bone_name.is_empty() {
            return Vec::new();
        }
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

fn muzzle_flash_sub_object_names(prefix: &str, barrel_index: usize) -> [String; 2] {
    [
        format!("{prefix}{:02}", barrel_index + 1),
        prefix.to_string(),
    ]
}

impl W3DModelDraw {
    fn composed_sub_object_visibility(&self) -> Vec<HideShowSubObjInfo> {
        let state_list = self
            .current_state()
            .map(|state| state.hide_show_list.as_slice())
            .unwrap_or(&[]);
        compose_hide_show_list(state_list, &self.sub_object_vec)
    }

    fn hide_all_muzzle_flashes(&mut self) {
        let Some(state) = self.current_state().cloned() else {
            return;
        };
        for wslot in 0..WEAPONSLOT_COUNT {
            let prefix = state.weapon_muzzle_flash[wslot].as_str();
            if prefix.is_empty() {
                continue;
            }
            let barrel_count = state.weapon_barrels[wslot].len().max(1);
            for barrel_index in 0..barrel_count {
                if state.weapon_barrels[wslot]
                    .get(barrel_index)
                    .map(|barrel| barrel.muzzle_flash_bone == 0)
                    .unwrap_or(false)
                    && barrel_index > 0
                {
                    continue;
                }
                for name in muzzle_flash_sub_object_names(prefix, barrel_index) {
                    self.show_sub_object(&name, false);
                }
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
        let prefix = prefix.to_string();
        for name in muzzle_flash_sub_object_names(&prefix, barrel_index) {
            self.show_sub_object(&name, !hidden);
        }
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
        for entry in entries {
            self.show_sub_object(entry.sub_obj_name.as_str(), !entry.hide);
        }
        self.update_sub_objects();
    }
}
