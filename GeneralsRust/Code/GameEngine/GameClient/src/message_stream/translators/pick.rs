use super::*;

/// Wave 974: context pick from presentation translator catalog residual.
pub(super) fn collect_selectable_objects_from_presentation(
    region: &IRegion2D,
    is_point: bool,
    radius: f32,
    point_world: Option<&Coord3D>,
    profile: ContextPickProfile,
) -> (Vec<(ObjectID, f32)>, Vec<(ObjectID, f32)>) {
    let mut mine = Vec::new();
    let mut other = Vec::new();
    with_translator_catalog(|catalog| {
        for entry in catalog {
            let matches = (profile.include_selectable
                && entry.selectable
                && translator_entry_has_kind(entry, "Selectable"))
                || (profile.include_force_attackable
                    && translator_entry_has_kind(entry, "ForceAttackable"))
                || (profile.include_mines && translator_entry_has_kind(entry, "Mine"))
                || (profile.include_shrubbery && translator_entry_has_kind(entry, "Shrubbery"));
            if !matches {
                continue;
            }
            // Wave 1039: C++ status/stealth/FOW residual for dual context pick.
            // Wave 1075: disabled residual fail-closed for dual context pick.
            if entry.destroyed || entry.sold || entry.unselectable || entry.masked || entry.disabled
            {
                continue;
            }
            if entry.effectively_stealthed && !translator_entry_is_local(entry) {
                continue;
            }
            // SelectionInfo: enemy/neutral FOW fogged+ fails closed (shroud_status >= 2).
            if !translator_entry_is_local(entry) && entry.shroud_status >= 2 {
                continue;
            }
            let pos = Coord3D::new(entry.position[0], entry.position[1], entry.position[2]);
            if world_position_is_under_opaque_window_for_command(&pos) {
                continue;
            }
            let Some(distance) = object_pick_distance(&pos, region, is_point, point_world, radius)
            else {
                continue;
            };
            if translator_entry_is_local(entry) {
                mine.push((entry.object_id, distance));
            } else {
                other.push((entry.object_id, distance));
            }
        }
    });
    (mine, other)
}

pub(super) fn collect_selectable_objects(
    region: &IRegion2D,
    is_point: bool,
    radius: f32,
    _local_player: Option<u32>,
    profile: ContextPickProfile,
) -> (Vec<(ObjectID, f32)>, Vec<(ObjectID, f32)>) {
    let local_player_index = player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())
        .unwrap_or(-1);
    if is_point {
        if let Some(picked) = with_tactical_view_ref(|view| {
            view.pick_drawable(
                &IPoint2::new(region.x, region.y),
                true,
                crate::display::view::PickType::Selectable,
            )
        }) {
            if let Some(obj_ref) = OBJECT_REGISTRY.get_object(ObjectID::from(picked)) {
                if let Ok(obj) = obj_ref.read() {
                    if object_matches_context_pick_profile(&obj, profile)
                        && !object_is_hidden_for_player(&obj, local_player_index)
                    {
                        let pos = obj.get_position();
                        let world = Coord3D::new(pos.x, pos.y, pos.z);
                        if !world_position_is_under_opaque_window_for_command(&world) {
                            if obj.is_locally_controlled() {
                                return (vec![(obj.get_id(), 0.0)], Vec::new());
                            }
                            return (Vec::new(), vec![(obj.get_id(), 0.0)]);
                        }
                    }
                }
            }
        }
    }
    let point_world = is_point
        .then(|| screen_to_terrain(&ICoord2D::new(region.x, region.y)))
        .flatten();
    if is_point && point_world.is_none() {
        return (Vec::new(), Vec::new());
    }

    // Wave 974: host empty dual-world → presentation translator catalog residual.
    if dual_world_registry_unavailable() {
        return collect_selectable_objects_from_presentation(
            region,
            is_point,
            radius,
            point_world.as_ref(),
            profile,
        );
    }
    let mut mine = Vec::new();
    let mut other = Vec::new();
    for obj_ref in OBJECT_REGISTRY.get_all_objects() {
        let Ok(obj) = obj_ref.read() else {
            continue;
        };
        if !object_matches_context_pick_profile(&obj, profile) {
            continue;
        }
        if object_is_hidden_for_player(&obj, local_player_index) {
            continue;
        }
        let pos = obj.get_position();
        let pos = Coord3D::new(pos.x, pos.y, pos.z);
        if world_position_is_under_opaque_window_for_command(&pos) {
            continue;
        }

        let Some(distance) =
            object_pick_distance(&pos, region, is_point, point_world.as_ref(), radius)
        else {
            continue;
        };

        if obj.is_locally_controlled() {
            mine.push((obj.get_id(), distance));
        } else {
            other.push((obj.get_id(), distance));
        }
    }

    (mine, other)
}

pub(super) fn object_matches_context_pick_profile(
    obj: &gamelogic::object::Object,
    profile: ContextPickProfile,
) -> bool {
    if obj.is_destroyed() || obj.is_effectively_dead() {
        return false;
    }
    let status = obj.get_status_bits();
    if status.contains(LogicObjectStatusMaskType::UNSELECTABLE)
        || status.contains(LogicObjectStatusMaskType::MASKED)
    {
        return false;
    }

    (profile.include_selectable && obj.is_selectable() && obj.is_kind_of(KindOf::Selectable))
        || (profile.include_force_attackable && obj.is_kind_of(KindOf::ForceAttackable))
        || (profile.include_mines && obj.is_kind_of(KindOf::Mine))
        || (profile.include_shrubbery && obj.is_kind_of(KindOf::Shrubbery))
}

pub(super) fn object_is_hidden_for_player(
    obj: &gamelogic::object::Object,
    local_player_index: i32,
) -> bool {
    matches!(
        obj.get_shrouded_status(local_player_index),
        ObjectShroudStatus::Fogged
            | ObjectShroudStatus::Shrouded
            | ObjectShroudStatus::InvalidButPreviousValid
    )
}

pub(super) fn object_pick_distance(
    pos: &Coord3D,
    region: &IRegion2D,
    is_point: bool,
    point_world: Option<&Coord3D>,
    radius: f32,
) -> Option<f32> {
    if is_point {
        let world = point_world?;
        let dx = pos.x - world.x;
        let dy = pos.y - world.y;
        let dist_sq = dx * dx + dy * dy;
        return (dist_sq <= radius * radius).then_some(dist_sq);
    }

    let min_x = region.x.min(region.x + region.width);
    let min_y = region.y.min(region.y + region.height);
    let max_x = region.x.max(region.x + region.width);
    let max_y = region.y.max(region.y + region.height);
    let point = Point3::new(pos.x, pos.y, pos.z);
    let screen = with_tactical_view_ref(|view| view.world_to_screen(&point))?;
    let in_region =
        screen.x >= min_x && screen.x <= max_x && screen.y >= min_y && screen.y <= max_y;
    in_region.then(|| {
        let center_x = (min_x + max_x) as f32 * 0.5;
        let center_y = (min_y + max_y) as f32 * 0.5;
        let dx = screen.x as f32 - center_x;
        let dy = screen.y as f32 - center_y;
        dx * dx + dy * dy
    })
}

pub(super) fn world_position_is_under_opaque_window_for_command(pos: &Coord3D) -> bool {
    let point = Point3::new(pos.x, pos.y, pos.z);
    let Some(screen) = with_tactical_view_ref(|view| view.world_to_screen(&point)) else {
        return false;
    };
    with_window_manager_ref(|manager| {
        let mut window = manager.get_window_under_cursor(screen.x, screen.y, false);
        while let Some(current) = window {
            let guard = current.borrow();
            if !guard.get_status().contains(WindowStatus::SEE_THRU) {
                return true;
            }
            window = guard.get_parent();
        }
        false
    })
}

pub(super) fn pick_closest(objects: &mut Vec<(ObjectID, f32)>) -> Option<ObjectID> {
    objects.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    objects.first().map(|(id, _)| *id)
}

pub(super) fn is_enemy_target(_local_player: i32, _target_id: ObjectID) -> bool {
    false
}

pub(super) fn selection_has_quick_path_to(selection: &HashSet<ObjectID>, world: &Coord3D) -> bool {
    let dest = LogicCoord3D::new(world.x, world.y, world.z);
    let local_player = get_local_player_id();
    if local_player >= 0 {
        if let Ok(mgr) = get_shroud_manager().lock() {
            if mgr.get_shroud_state(local_player as u32, &dest) != ShroudState::Visible {
                return true;
            }
        }
    }

    for id in selection {
        let Some(obj) = OBJECT_REGISTRY.get_object(*id) else {
            continue;
        };
        let Ok(guard) = obj.read() else {
            continue;
        };
        let Some(ai) = guard.get_ai() else {
            continue;
        };
        let Ok(ai_guard) = ai.lock() else {
            continue;
        };
        if ai_guard.is_quick_path_available(&dest) {
            return true;
        }
        if ai_guard.has_locomotor_for_surface(SURFACE_CLIFF)
            && TheTerrainLogic.is_cliff_cell(world.x, world.y)
        {
            return true;
        }
    }
    false
}

pub(super) fn selection_can_set_rally_point(selection: &HashSet<ObjectID>) -> bool {
    if selection.is_empty() {
        return false;
    }
    for id in selection {
        let Some(obj) = OBJECT_REGISTRY.get_object(*id) else {
            return false;
        };
        let Ok(guard) = obj.read() else {
            return false;
        };
        if !guard.is_locally_controlled() || guard.is_effectively_dead() {
            return false;
        }
        // C++ InGameUI.cpp:4373-4380 ACTIONTYPE_SET_RALLY_POINT: KINDOF_AUTO_RALLYPOINT.
        if !guard.is_kind_of(KindOf::AutoRallypoint) {
            return false;
        }
    }
    true
}

pub(super) fn selection_counts(
    _local_player: Option<u32>,
    _selection: &HashSet<ObjectID>,
) -> (i32, i32, i32, i32) {
    (0, 0, 0, 0)
}

pub(super) fn dispatch_translated_message(msg: &GameMessageType) {
    let command_list = get_command_list();
    if let Ok(mut guard) = command_list.write() {
        guard.append_message(GameMessage::new(msg.clone()));
    };
}

pub(super) fn logic_to_message_coord(pos: &LogicCoord3D) -> Coord3D {
    Coord3D::new(pos.x, pos.y, pos.z)
}

pub(super) fn screen_to_terrain(pos: &ICoord2D) -> Option<Coord3D> {
    let screen = IPoint2::new(pos.x, pos.y);
    with_tactical_view_ref(|view| {
        view.screen_to_terrain(&screen)
            .ok()
            .map(|point| Coord3D::new(point.x, point.y, point.z))
    })
}

pub(super) fn is_alternate_mouse_enabled() -> bool {
    get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false)
}

pub(super) fn is_double_click_attack_move_enabled() -> bool {
    get_global_data()
        .map(|data| data.read().double_click_attack_move)
        .unwrap_or(false)
}

pub(super) fn point_click_is_actionable(
    right_click: bool,
    alternate_mouse: bool,
    pending_command_active: bool,
) -> bool {
    if right_click {
        // C++ only processes right-click point commands in alternate mouse mode,
        // except when a pending GUI command is active and the click is used to cancel it.
        alternate_mouse || pending_command_active
    } else {
        // C++ only processes left-click point commands in alternate mouse mode when
        // a GUI command is actively firing.
        !alternate_mouse || pending_command_active
    }
}
