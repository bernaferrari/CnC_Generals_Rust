impl TheGameLogic {
    /// Player-selection half of `TheGameLogic::select_object`.
    ///
    /// C++ `GameLogic.cpp:2595-2641` assigns an AIGroup to the player then
    /// optionally selects the drawable. Callers must not hold the object's
    /// `RwLock` — `AIGroup::add` try-reads the same Arc.
    pub fn apply_select_object(
        object_id: ObjectID,
        create_new_selection: bool,
        mask: PlayerMaskType,
        can_add_to_group: bool,
        affect_client: bool,
        drawable: Option<Arc<RwLock<crate::object::drawable::Drawable>>>,
    ) {
        use crate::commands::{get_selection_manager, SelectionType};

        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return;
        };
        let selection_type = if create_new_selection {
            SelectionType::Replace
        } else {
            SelectionType::Add
        };
        let Ok(list) = crate::player::player_list().read() else {
            return;
        };
        for (player_index, player_arc) in list.iter().enumerate() {
            let bit = PlayerMaskType::from_bits_truncate(1u32 << (player_index as u32));
            if !mask.contains(bit) {
                continue;
            }
            let mut added_to_group = false;
            if let Ok(mut player) = player_arc.write() {
                if create_new_selection {
                    if can_add_to_group {
                        player.set_current_selection_to_object(object_id);
                        added_to_group = true;
                    } else {
                        player.set_currently_selected_ai_group(None);
                    }
                } else if can_add_to_group {
                    player.add_object_to_current_selection(object_id);
                    added_to_group = true;
                }
            }
            if added_to_group {
                if let Some(selection) = manager.get_player_selection(player_index as i32) {
                    selection.select_objects(vec![object_id], selection_type);
                }
            }
            if affect_client {
                if let Some(drawable) = drawable.as_ref() {
                    TheInGameUI::select_drawable(drawable);
                }
            }
        }
    }

    /// Player-selection half of `TheGameLogic::deselect_object`.
    /// C++ `GameLogic.cpp:2646-2690`.
    pub fn apply_deselect_object(
        object_id: ObjectID,
        mask: PlayerMaskType,
        affect_client: bool,
        drawable: Option<Arc<RwLock<crate::object::drawable::Drawable>>>,
    ) {
        use crate::commands::{get_selection_manager, SelectionType};

        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return;
        };
        let Ok(list) = crate::player::player_list().read() else {
            return;
        };
        for (player_index, player_arc) in list.iter().enumerate() {
            let bit = PlayerMaskType::from_bits_truncate(1u32 << (player_index as u32));
            if !mask.contains(bit) {
                continue;
            }
            let mut actually_removed = false;
            if let Ok(mut player) = player_arc.write() {
                actually_removed = player.remove_object_from_current_selection(object_id);
                if actually_removed && affect_client {
                    if let Some(drawable) = drawable.as_ref() {
                        TheInGameUI::deselect_drawable(drawable);
                    }
                }
            }
            if actually_removed {
                if let Some(selection) = manager.get_player_selection(player_index as i32) {
                    selection.select_objects(vec![object_id], SelectionType::Remove);
                }
            }
        }
    }
}
