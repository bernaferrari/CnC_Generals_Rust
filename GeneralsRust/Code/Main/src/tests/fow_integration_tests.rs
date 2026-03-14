#![cfg(test)]

use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility};
use crate::game_logic::ObjectId;

#[test]
fn object_visibility_values_stay_in_valid_ranges() {
    let visibility = FOWRenderingBridge::get_object_visibility(0, ObjectId(100));
    assert!((0.0..=1.0).contains(&visibility.visibility_alpha));
    assert!(visibility.is_explored == 0.0 || visibility.is_explored == 1.0);
    assert!(visibility.visibility_falloff >= 0.0);
}

#[test]
fn batch_visibility_query_returns_entry_for_each_object() {
    let objects = vec![ObjectId(101), ObjectId(102), ObjectId(103), ObjectId(104)];
    let visibilities = FOWRenderingBridge::get_all_object_visibilities(0, &objects);

    assert_eq!(visibilities.len(), objects.len());
    for object_id in objects {
        assert!(visibilities.contains_key(&object_id));
    }
}

#[test]
fn renderability_matches_visibility_contract() {
    let object_id = ObjectId(200);
    let visibility = FOWRenderingBridge::get_object_visibility(0, object_id);
    let should_render = FOWRenderingBridge::should_render_object(0, object_id);

    if visibility.visibility_alpha > 0.0 || visibility.is_explored > 0.0 {
        assert!(should_render);
    }
}

#[test]
fn force_visibility_update_is_safe_to_call() {
    FOWRenderingBridge::force_visibility_update();
    let _ = FOWRenderingBridge::get_object_visibility_with_stealth(0, ObjectId(300));
}

#[test]
fn object_visibility_struct_supports_expected_states() {
    let hidden = ObjectVisibility {
        visibility_alpha: 0.0,
        is_explored: 0.0,
        visibility_falloff: 1.0,
    };
    let explored = ObjectVisibility {
        visibility_alpha: 0.3,
        is_explored: 1.0,
        visibility_falloff: 1.0,
    };
    let visible = ObjectVisibility::default();

    assert!(hidden.visibility_alpha < explored.visibility_alpha);
    assert!(explored.visibility_alpha < visible.visibility_alpha);
}
