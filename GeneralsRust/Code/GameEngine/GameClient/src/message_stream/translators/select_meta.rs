//! C++ CommandXlat.cpp MSG_META_SELECT_NEXT/PREV_UNIT, NEXT/PREV_WORKER, and MSG_META_SELECT_ALL.

use super::*;

fn is_select_next_candidate(obj: &gamelogic::object::Object) -> bool {
    obj.is_mobile()
        && obj.is_locally_controlled()
        && !obj.is_contained()
        && !obj.is_kind_of(KindOf::NoSelect)
        && !obj.is_effectively_dead()
}

fn collect_select_next_candidates() -> Vec<(ObjectID, Coord3D)> {
    let mut out = Vec::new();
    for obj_ref in OBJECT_REGISTRY.get_all_objects() {
        let Ok(obj) = obj_ref.read() else {
            continue;
        };
        if !is_select_next_candidate(&obj) {
            continue;
        }
        let pos = obj.get_position();
        out.push((obj.get_id(), Coord3D::new(pos.x, pos.y, pos.z)));
    }
    out
}

fn look_at_object_position(pos: &Coord3D) {
    with_tactical_view(|view| {
        view.look_at(&Point3::new(pos.x, pos.y, pos.z));
    });
}

fn current_selected_ids() -> Vec<ObjectID> {
    let local_player = get_local_player_id();
    if local_player < 0 {
        return Vec::new();
    }
    let selection_manager = get_selection_manager();
    let Ok(manager) = selection_manager.read() else {
        return Vec::new();
    };
    manager
        .get_player_selection_ref(local_player)
        .map(|selection| selection.get_selected_objects())
        .unwrap_or_default()
}

fn apply_single_selection(id: ObjectID) {
    let local_player = get_local_player_id();
    if local_player < 0 {
        return;
    }
    if let Ok(mut manager) = get_selection_manager().write() {
        if let Some(selection) = manager.get_player_selection(local_player) {
            selection.select_objects(
                vec![id],
                gamelogic::commands::selection::SelectionType::Replace,
            );
        }
    }
}

/// C++ CommandXlat.cpp:2346-2451 / 2456-2570.
pub(super) fn handle_select_next_or_prev_unit(next: bool) -> Vec<GameMessageType> {
    let candidates = collect_select_next_candidates();
    if candidates.is_empty() {
        return Vec::new();
    }

    let current = current_selected_ids();
    let current_id = current.first().copied();
    let pick = if let Some(current_id) = current_id {
        if let Some(idx) = candidates.iter().position(|(id, _)| *id == current_id) {
            if next {
                // Drawable list is prepended; NEXT walks backwards.
                candidates[(idx + candidates.len() - 1) % candidates.len()].clone()
            } else {
                candidates[(idx + 1) % candidates.len()].clone()
            }
        } else if next {
            candidates[candidates.len() - 1].clone()
        } else {
            candidates[0].clone()
        }
    } else if next {
        candidates[candidates.len() - 1].clone()
    } else {
        candidates[0].clone()
    };

    apply_single_selection(pick.0);
    look_at_object_position(&pick.1);
    vec![GameMessageType::CreateSelectedGroup(true, vec![pick.0])]
}

fn is_select_worker_candidate(obj: &gamelogic::object::Object, require_mobile: bool) -> bool {
    if require_mobile && !obj.is_mobile() {
        return false;
    }
    obj.is_locally_controlled() && !obj.is_contained() && obj.is_kind_of(KindOf::Dozer)
}

fn collect_select_worker_candidates(require_mobile: bool) -> Vec<(ObjectID, Coord3D)> {
    let mut out = Vec::new();
    for obj_ref in OBJECT_REGISTRY.get_all_objects() {
        let Ok(obj) = obj_ref.read() else {
            continue;
        };
        if !is_select_worker_candidate(&obj, require_mobile) {
            continue;
        }
        let pos = obj.get_position();
        out.push((obj.get_id(), Coord3D::new(pos.x, pos.y, pos.z)));
    }
    out
}

/// C++ CommandXlat.cpp:2573-2798 `MSG_META_SELECT_NEXT/PREV_WORKER`.
/// Next walks the prepended drawable list backwards and accepts KINDOF_DOZER.
/// Prev also requires `isMobile()`. Neither path includes harvesters.
pub(super) fn handle_select_next_or_prev_worker(next: bool) -> Vec<GameMessageType> {
    let candidates = collect_select_worker_candidates(!next);
    if candidates.is_empty() {
        return Vec::new();
    }

    let current = current_selected_ids();
    let current_id = current.first().copied();
    let pick = if let Some(current_id) = current_id {
        if let Some(idx) = candidates.iter().position(|(id, _)| *id == current_id) {
            if next {
                candidates[(idx + candidates.len() - 1) % candidates.len()].clone()
            } else {
                candidates[(idx + 1) % candidates.len()].clone()
            }
        } else if next {
            candidates[candidates.len() - 1].clone()
        } else {
            candidates[0].clone()
        }
    } else if next {
        candidates[candidates.len() - 1].clone()
    } else {
        candidates[0].clone()
    };

    apply_single_selection(pick.0);
    look_at_object_position(&pick.1);
    vec![GameMessageType::CreateSelectedGroup(true, vec![pick.0])]
}

fn object_disqualifies_select_all(obj: &gamelogic::object::Object, aircraft_only: bool) -> bool {
    if obj.is_kind_of(KindOf::Dozer)
        || obj.is_kind_of(KindOf::Harvester)
        || obj.is_kind_of(KindOf::IgnoresSelectAll)
    {
        return true;
    }
    if aircraft_only {
        !obj.is_kind_of(KindOf::Aircraft)
    } else {
        obj.is_kind_of(KindOf::Structure)
    }
}

/// C++ CommandXlat.cpp:2864-2902 + InGameUI::selectAllUnitsByType.
pub(super) fn handle_select_all(aircraft_only: bool) -> Vec<GameMessageType> {
    for id in current_selected_ids() {
        if let Some(obj_ref) = OBJECT_REGISTRY.get_object(id) {
            if let Ok(obj) = obj_ref.read() {
                if object_disqualifies_select_all(&obj, aircraft_only) {
                    let local_player = get_local_player_id();
                    if local_player >= 0 {
                        if let Ok(mut manager) = get_selection_manager().write() {
                            if let Some(selection) = manager.get_player_selection(local_player) {
                                selection.clear_selection();
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    crate::gui::ingame_ui::select_all_units_by_type(aircraft_only)
}
