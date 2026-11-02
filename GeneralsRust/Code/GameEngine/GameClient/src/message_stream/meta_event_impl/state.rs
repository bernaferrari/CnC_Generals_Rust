// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

use crate::meta_demo_state::DemoCoord2D;

/// Run `f` with mutable access to the parsed CommandMap table.
fn with_meta_map<R>(f: impl FnOnce(&mut MetaMap) -> R) -> R {
    f(&mut get_meta_map().write().unwrap_or_else(|e| e.into_inner()))
}

/// Run `f` with shared access to the parsed CommandMap table.
fn with_meta_map_ref<R>(f: impl FnOnce(&MetaMap) -> R) -> R {
    f(&get_meta_map().read().unwrap_or_else(|e| e.into_inner()))
}

fn get_meta_map() -> &'static RwLock<MetaMap> {
    META_MAP.get_or_init(|| RwLock::new(MetaMap::default()))
}

fn toggle_shared_bool_state(flag: &mut bool) -> bool {
    MetaDemoState::toggle_bool(flag)
}

#[cfg(test)]
fn set_bool_state_for_tests(flag: &mut bool, value: bool) {
    *flag = value;
}

#[cfg(test)]
fn bool_state_for_tests(flag: &bool) -> bool {
    *flag
}

#[cfg(test)]
fn bw_view_mode_for_tests(demo: &MetaDemoState) -> u8 {
    demo.bw_view_mode
}

#[cfg(test)]
fn bw_view_wireframe_for_tests() -> (bool, bool) {
    crate::display::view::with_tactical_view_ref(|view| {
        (
            view.is_3d_wireframe_mode(),
            view.pending_3d_wireframe_mode(),
        )
    })
}

#[cfg(test)]
fn reset_bw_view_state_for_tests(demo: &mut MetaDemoState) {
    demo.bw_view_mode = 0;
    script_set_3d_wireframe_mode(false);
    crate::display::view::with_tactical_view(|view| {
        view.update_view();
        view.update_view();
    });
}

fn set_demo_pitch_adjusting(demo: &mut MetaDemoState, enabled: bool) {
    demo.demo_camera_adjust.is_pitching = enabled;
}

fn set_demo_fov_adjusting(demo: &mut MetaDemoState, enabled: bool) {
    let state = &mut demo.demo_camera_adjust;
    state.is_changing_fov = enabled;
    if enabled {
        state.anchor = state.current_pos.clone();
    }
}

fn apply_demo_camera_adjust_from_mouse_position(demo: &mut MetaDemoState, pos: &ICoord2D) {
    let state = &mut demo.demo_camera_adjust;
    state.current_pos = DemoCoord2D::new(pos.x, pos.y);
    let adjustment = if !state.is_pitching && !state.is_changing_fov {
        state.anchor = state.current_pos.clone();
        None
    } else {
        let delta_y = (state.current_pos.y - state.anchor.y) as f32;
        state.anchor = state.current_pos.clone();
        Some((state.is_pitching, state.is_changing_fov, delta_y))
    };
    let Some((is_pitching, is_changing_fov, delta_y)) = adjustment else {
        return;
    };

    if delta_y.abs() < f32::EPSILON {
        return;
    }

    with_tactical_view(|view| {
        if is_pitching {
            view.set_pitch(view.pitch() + (delta_y * DEMO_CAMERA_ADJUST_FACTOR));
        }
        if is_changing_fov {
            view.set_field_of_view(view.field_of_view() + (delta_y * DEMO_CAMERA_ADJUST_FACTOR));
        }
    });
}

#[cfg(test)]
fn reset_demo_camera_adjust_state_for_tests(demo: &mut MetaDemoState) {
    demo.demo_camera_adjust = DemoCameraAdjustState::default();
}

#[cfg(test)]
fn demo_camera_adjust_state_for_tests(demo: &MetaDemoState) -> DemoCameraAdjustState {
    demo.demo_camera_adjust.clone()
}

fn parse_extent_adjust_alias(name: &str) -> Option<ExtentAdjustSpec> {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "DEMO_CYCLE_EXTENT_TYPE" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Type,
            amount: 1.0,
        }),
        "DEMO_INCR_EXTENT_MAJOR" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Major,
            amount: 1.0,
        }),
        "DEMO_DECR_EXTENT_MAJOR" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Major,
            amount: -1.0,
        }),
        "DEMO_INCR_EXTENT_MAJOR_LARGE" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Major,
            amount: EXTENT_BIG_CHANGE,
        }),
        "DEMO_DECR_EXTENT_MAJOR_LARGE" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Major,
            amount: -EXTENT_BIG_CHANGE,
        }),
        "DEMO_INCR_EXTENT_MINOR" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Minor,
            amount: 1.0,
        }),
        "DEMO_DECR_EXTENT_MINOR" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Minor,
            amount: -1.0,
        }),
        "DEMO_INCR_EXTENT_MINOR_LARGE" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Minor,
            amount: EXTENT_BIG_CHANGE,
        }),
        "DEMO_DECR_EXTENT_MINOR_LARGE" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Minor,
            amount: -EXTENT_BIG_CHANGE,
        }),
        "DEMO_INCR_EXTENT_HEIGHT" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Height,
            amount: 1.0,
        }),
        "DEMO_DECR_EXTENT_HEIGHT" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Height,
            amount: -1.0,
        }),
        "DEMO_INCR_EXTENT_HEIGHT_LARGE" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Height,
            amount: EXTENT_BIG_CHANGE,
        }),
        "DEMO_DECR_EXTENT_HEIGHT_LARGE" => Some(ExtentAdjustSpec {
            axis: ExtentAdjustAxis::Height,
            amount: -EXTENT_BIG_CHANGE,
        }),
        _ => None,
    }
}

fn geometry_extent_mod_type(axis: ExtentAdjustAxis) -> GeometryExtentModType {
    match axis {
        ExtentAdjustAxis::Type => GeometryExtentModType::Type,
        ExtentAdjustAxis::Major => GeometryExtentModType::Major,
        ExtentAdjustAxis::Minor => GeometryExtentModType::Minor,
        ExtentAdjustAxis::Height => GeometryExtentModType::Height,
    }
}

fn geometry_extent_mod_type_code(axis: ExtentAdjustAxis) -> i32 {
    match axis {
        ExtentAdjustAxis::Type => 1,
        ExtentAdjustAxis::Major => 2,
        ExtentAdjustAxis::Minor => 3,
        ExtentAdjustAxis::Height => 4,
    }
}

fn apply_extent_adjust(geometry: &mut GeometryInfo, spec: ExtentAdjustSpec) {
    geometry.tweak_extents(geometry_extent_mod_type(spec.axis), spec.amount);
}

fn format_extent_debug(geometry: &GeometryInfo) -> String {
    geometry.get_descriptive_string()
}

fn apply_extent_adjust_to_local_selection(
    spec: ExtentAdjustSpec,
    ui: &mut crate::message_stream::ReceivingUi<'_>,
) {
    // Wave 976: host empty dual-world still routes extent adjust through TheGameLogic IDs.
    for object_id in local_selection_object_ids() {
        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            continue;
        };
        let Ok(mut object) = object_arc.write() else {
            continue;
        };

        let old_geometry = object.get_geometry_info().clone();
        let mut new_geometry = old_geometry.clone();
        apply_extent_adjust(&mut new_geometry, spec);
        object.set_geometry_info(new_geometry.clone());

        ui.message(&format!(
            "Extent {} --> {}   {} {}",
            format_extent_debug(&old_geometry),
            format_extent_debug(&new_geometry),
            geometry_extent_mod_type_code(spec.axis),
            spec.amount
        ));
    }
}
