// C++ InGameUI::selectAllUnitsByType (InGameUI.cpp:95-140).
// Included by ingame_ui/mod.rs.

impl InGameUI {
    /// Select locally-controlled mass-selectable units, excluding dozers,
    /// harvesters, IGNORES_SELECT_ALL, contained/dead objects.
    pub fn select_all_units_by_type(
        &mut self,
        aircraft_only: bool,
        window_query: &mut dyn FnMut(i32, i32, &mut crate::message_stream::ReceivingUi<'_>) -> bool,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) -> Vec<ObjectID> {
        let ids = collect_select_all_unit_ids(aircraft_only, window_query, ui);
        if ids.is_empty() {
            return ids;
        }
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(ids.clone(), SelectionType::Add);
            }
        }
        self.sync_selection_state();
        ids
    }
}

fn collect_select_all_unit_ids(
    aircraft_only: bool,
    window_query: &mut dyn FnMut(i32, i32, &mut crate::message_stream::ReceivingUi<'_>) -> bool,
    ui: &mut crate::message_stream::ReceivingUi<'_>,
) -> Vec<ObjectID> {
    let max_select = ui.get_max_select_count();
    let mut selected = Vec::new();

    for obj_ref in OBJECT_REGISTRY.get_all_objects() {
        let Ok(obj) = obj_ref.read() else {
            continue;
        };
        if !obj.is_locally_controlled() || obj.is_contained() || obj.is_effectively_dead() {
            continue;
        }
        // C++ kindOfUnitSelection: skip already-selected so SELECT_ALL adds.
        if obj
            .get_drawable()
            .and_then(|draw| draw.read().ok().map(|guard| guard.is_selected()))
            .unwrap_or(false)
        {
            continue;
        }
        if !obj.is_mass_selectable() {
            continue;
        }
        if obj.is_kind_of(KindOf::Dozer)
            || obj.is_kind_of(KindOf::Harvester)
            || obj.is_kind_of(KindOf::IgnoresSelectAll)
        {
            continue;
        }
        if aircraft_only && !obj.is_kind_of(KindOf::Aircraft) {
            continue;
        }
        if !aircraft_only && obj.is_kind_of(KindOf::Structure) {
            continue;
        }
        selected.push(obj.get_id());
    }

    // C++ selectAllUnitsByType: screen region first, then whole map (InGameUI.cpp:4877).
    let owned_screen_ids = ui.with_drawable_manager(|manager, split_ui| {
        with_tactical_view_ref(|view| {
            view.iterate_drawables_in_region_with_manager(
                Some((
                    IPoint2::new(0, 0),
                    IPoint2::new(view.width(), view.height()),
                )),
                &mut |x, y| window_query(x, y, split_ui),
                manager,
            )
        })
    });
    let screen_ids = owned_screen_ids.unwrap_or_else(|| {
        with_tactical_view_ref(|view| {
            view.iterate_drawables_in_region(
                Some((
                    IPoint2::new(0, 0),
                    IPoint2::new(view.width(), view.height()),
                )),
                &mut |x, y| window_query(x, y, ui),
            )
        })
    });
    let on_screen: Vec<ObjectID> = selected
        .iter()
        .copied()
        .filter(|id| screen_ids.iter().any(|sid| *sid == *id))
        .collect();
    let mut chosen = if !on_screen.is_empty() {
        ui.message(&GameText::fetch("GUI:SelectedAcrossScreen"));
        on_screen
    } else if !selected.is_empty() {
        ui.message(&GameText::fetch("GUI:SelectedAcrossMap"));
        selected
    } else {
        selected
    };

    if max_select > 0 && chosen.len() > max_select as usize {
        chosen.truncate(max_select as usize);
        let label = GameText::fetch("GUI:MaxSelectionSize");
        ui.message(
            &label
                .replace("%d", &max_select.to_string())
                .replace("%i", &max_select.to_string()),
        );
    }
    chosen
}

/// Crate/command-translator entry: apply select-all and emit MSG_CREATE_SELECTED_GROUP.
pub fn select_all_units_by_type(
    aircraft_only: bool,
    window_query: &mut dyn FnMut(i32, i32, &mut crate::message_stream::ReceivingUi<'_>) -> bool,
    ui: &mut crate::message_stream::ReceivingUi<'_>,
) -> Vec<crate::message_stream::game_message::GameMessageType> {
    use crate::message_stream::game_message::GameMessageType;
    use crate::message_stream::player_state::get_local_player_id;

    let selected = collect_select_all_unit_ids(aircraft_only, window_query, ui);
    let local_player = get_local_player_id();
    if local_player >= 0 {
        if let Ok(mut manager) = get_selection_manager().write() {
            if let Some(selection) = manager.get_player_selection(local_player) {
                let create_new = selection.get_selected_objects().is_empty();
                if !selected.is_empty() {
                    selection.select_objects(selected.clone(), SelectionType::Add);
                }
                if selected.is_empty() {
                    return Vec::new();
                }
                return vec![GameMessageType::CreateSelectedGroup(create_new, selected)];
            }
        }
    }
    if selected.is_empty() {
        Vec::new()
    } else {
        vec![GameMessageType::CreateSelectedGroup(true, selected)]
    }
}
