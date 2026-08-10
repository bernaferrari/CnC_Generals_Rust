// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

fn get_meta_map() -> &'static RwLock<MetaMap> {
    META_MAP.get_or_init(|| RwLock::new(MetaMap::default()))
}

fn get_lower_detail_toggle_state() -> &'static RwLock<LowerDetailToggleState> {
    LOWER_DETAIL_TOGGLE_STATE.get_or_init(|| RwLock::new(LowerDetailToggleState::default()))
}

fn get_objective_movie_index() -> &'static RwLock<i32> {
    OBJECTIVE_MOVIE_INDEX.get_or_init(|| RwLock::new(1))
}

fn get_motion_blur_zoom_saturate_state() -> &'static RwLock<bool> {
    MOTION_BLUR_ZOOM_SATURATE.get_or_init(|| RwLock::new(false))
}

fn get_demo_camera_adjust_state() -> &'static RwLock<DemoCameraAdjustState> {
    DEMO_CAMERA_ADJUST_STATE.get_or_init(|| RwLock::new(DemoCameraAdjustState::default()))
}

fn hand_of_god_mode_state() -> &'static RwLock<bool> {
    HAND_OF_GOD_MODE.get_or_init(|| RwLock::new(false))
}

fn hurt_me_mode_state() -> &'static RwLock<bool> {
    HURT_ME_MODE.get_or_init(|| RwLock::new(false))
}

fn debug_selection_mode_state() -> &'static RwLock<bool> {
    DEBUG_SELECTION_MODE.get_or_init(|| RwLock::new(false))
}

fn bw_view_mode_state() -> &'static RwLock<u8> {
    BW_VIEW_MODE_STATE.get_or_init(|| RwLock::new(0))
}

fn toggle_shared_bool_state(state: &'static RwLock<bool>) -> bool {
    if let Ok(mut guard) = state.write() {
        *guard = !*guard;
        return *guard;
    }
    false
}

#[cfg(test)]
fn set_bool_state_for_tests(state: &'static RwLock<bool>, value: bool) {
    if let Ok(mut guard) = state.write() {
        *guard = value;
    }
}

#[cfg(test)]
fn bool_state_for_tests(state: &'static RwLock<bool>) -> bool {
    state.read().map(|guard| *guard).unwrap_or(false)
}

#[cfg(test)]
fn bw_view_mode_for_tests() -> u8 {
    bw_view_mode_state().read().map(|guard| *guard).unwrap_or(0)
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
fn reset_bw_view_state_for_tests() {
    if let Ok(mut mode) = bw_view_mode_state().write() {
        *mode = 0;
    }
    script_set_3d_wireframe_mode(false);
    crate::display::view::with_tactical_view(|view| {
        view.update_view();
        view.update_view();
    });
}

fn set_demo_pitch_adjusting(enabled: bool) {
    if let Ok(mut state) = get_demo_camera_adjust_state().write() {
        state.is_pitching = enabled;
    }
}

fn set_demo_fov_adjusting(enabled: bool) {
    if let Ok(mut state) = get_demo_camera_adjust_state().write() {
        state.is_changing_fov = enabled;
        if enabled {
            state.anchor = state.current_pos.clone();
        }
    }
}

fn apply_demo_camera_adjust_from_mouse_position(pos: &ICoord2D) {
    let (is_pitching, is_changing_fov, delta_y) = {
        let Ok(mut state) = get_demo_camera_adjust_state().write() else {
            return;
        };

        state.current_pos = pos.clone();
        if !state.is_pitching && !state.is_changing_fov {
            state.anchor = state.current_pos.clone();
            return;
        }

        let delta_y = (state.current_pos.y - state.anchor.y) as f32;
        state.anchor = state.current_pos.clone();
        (state.is_pitching, state.is_changing_fov, delta_y)
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
fn reset_demo_camera_adjust_state_for_tests() {
    if let Ok(mut state) = get_demo_camera_adjust_state().write() {
        *state = DemoCameraAdjustState::default();
    }
}

#[cfg(test)]
fn demo_camera_adjust_state_for_tests() -> DemoCameraAdjustState {
    get_demo_camera_adjust_state()
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default()
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

fn apply_extent_adjust_to_local_selection(spec: ExtentAdjustSpec) {
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

        TheInGameUI::message(&format!(
            "Extent {} --> {}   {} {}",
            format_extent_debug(&old_geometry),
            format_extent_debug(&new_geometry),
            geometry_extent_mod_type_code(spec.axis),
            spec.amount
        ));
    }
}
