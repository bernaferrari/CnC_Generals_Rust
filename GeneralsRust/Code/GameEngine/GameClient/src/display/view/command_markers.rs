use super::*;

pub(super) fn draw_move_hint_and_locater_models_with_owner(
    view: &View,
    ui: &crate::core::subsystems::InGameUISubsystem,
    client_frame: u32,
    manager: &mut crate::drawable::DrawableManager,
    cache: &mut HintModelCache,
) {
    let hint_name = get_global_data()
        .map(|data| {
            let name = data.read().move_hint_name.clone();
            if name.is_empty() {
                "MoveHint".to_string()
            } else {
                name
            }
        })
        .unwrap_or_else(|| "MoveHint".to_string());
    let live: Vec<Point3> = ui
        .get_hints()
        .into_iter()
        .filter(|hint| {
            hint.hint_type == HintType::Move
                && client_frame.saturating_sub(hint.creation_frame) <= 40
        })
        .map(|hint| Point3::new(hint.end.x, hint.end.y, hint.end.z))
        .collect();

    while cache.move_hints.len() < live.len() {
        let pos = live[cache.move_hints.len()];
        let id = manager.create_drawable(DrawableType::Model {
            model_name: hint_name.clone(),
            position: DrawVec3::new(pos.x, pos.y, pos.z),
            scale: 1.0,
            animation_state: "ONCE".to_string(),
        });
        cache.move_hints.push(Some(id));
    }
    for (slot, id) in cache.move_hints.iter().enumerate() {
        let Some(id) = id else { continue };
        if let Some(drawable) = manager.get_drawable_mut(*id) {
            if let Some(pos) = live.get(slot) {
                drawable.set_visible(true);
                drawable.set_position(DrawVec3::new(pos.x, pos.y, pos.z));
            } else {
                drawable.set_visible(false);
            }
        }
    }

    if ui.is_placement_anchored() {
        if let Some((start, end)) = ui.get_placement_points() {
            let start_world = view
                .screen_to_terrain(&IPoint2::new(start.x, start.y))
                .unwrap_or(Point3::new(start.x as f32, start.y as f32, 0.0));
            let end_world = view
                .screen_to_terrain(&IPoint2::new(end.x, end.y))
                .unwrap_or(Point3::new(end.x as f32, end.y as f32, 0.0));
            let dx = (end.x - start.x) as f32;
            let dy = (end.y - start.y) as f32;
            let show_arrow = (dx * dx + dy * dy).sqrt() >= 5.0;
            if cache.locater_anchor.is_none() {
                cache.locater_anchor = Some(manager.create_drawable(DrawableType::Model {
                    model_name: "Locater01".to_string(),
                    position: DrawVec3::new(start_world.x, start_world.y, start_world.z),
                    scale: 1.0,
                    animation_state: "LOOP".to_string(),
                }));
            }
            if cache.locater_arrow.is_none() {
                cache.locater_arrow = Some(manager.create_drawable(DrawableType::Model {
                    model_name: "Locater02".to_string(),
                    position: DrawVec3::new(start_world.x, start_world.y, start_world.z),
                    scale: 1.0,
                    animation_state: "LOOP".to_string(),
                }));
            }
            if let Some(id) = cache
                .locater_anchor
                .and_then(|id| manager.get_drawable_mut(id))
            {
                if show_arrow {
                    id.set_visible(false);
                } else {
                    id.set_visible(true);
                    id.set_position(DrawVec3::new(start_world.x, start_world.y, start_world.z));
                }
            }
            if let Some(id) = cache
                .locater_arrow
                .and_then(|id| manager.get_drawable_mut(id))
            {
                id.set_visible(show_arrow);
                if show_arrow {
                    id.set_position(DrawVec3::new(end_world.x, end_world.y, end_world.z));
                }
            }
        }
    } else {
        for id in [cache.locater_anchor, cache.locater_arrow]
            .into_iter()
            .flatten()
        {
            if let Some(drawable) = manager.get_drawable_mut(id) {
                drawable.set_visible(false);
            }
        }
    }
}

/// Live move/attack command markers (right-HUD lane: issued-order feedback).
///
/// C++ parity: `HintSpyTranslator` (HintSpy.cpp:91-100) turns MSG_DO_MOVETO /
/// MSG_DO_ATTACKMOVETO / MSG_DO_FORCEMOVETO / MSG_ADD_WAYPOINT into
/// `InGameUI::createMoveHint` (InGameUI.cpp:2141-2171), and
/// `W3DInGameUI::drawMoveHints` (W3DInGameUI.cpp:444-561) renders each hint at
/// its world position for 40 client frames (`elapsed <= 40`), terrain-aligned,
/// playing the `MoveHintName` (GameData.ini: `SCMoveHint`) animation once.
/// The retail marker is a W3D render object from the Art BIGs, which this
/// host's extracts do not carry, so the marker is drawn as a projected
/// screen-space ring in the UI layer (same layer the radar events and window
/// chrome draw through). Attack hints are fed the same hook the C++ code
/// reserves (`HintSpy.cpp:99-100` → `InGameUI::createAttackHint`); the C++
/// body is intentionally empty (InGameUI.cpp:2176-2179), so the red attack
/// ring is a documented host adaptation requested for order feedback.
pub fn draw_live_command_markers(
    renderer: &mut crate::gui::UIRenderer,
    view: &View,
    ui: &crate::core::subsystems::InGameUISubsystem,
    client_frame: u32,
) {
    let frame = client_frame;
    let hints: Vec<(IPoint2, HintType, u32)> = ui
        .get_hints()
        .into_iter()
        .filter(|hint| matches!(hint.hint_type, HintType::Move | HintType::Attack))
        .filter_map(|hint| {
            let elapsed = frame.saturating_sub(hint.creation_frame);
            if elapsed > 40 {
                return None;
            }
            let world = Point3::new(hint.end.x, hint.end.y, hint.end.z);
            view.world_to_screen(&world)
                .map(|screen| (screen, hint.hint_type, elapsed))
        })
        .collect();
    if hints.is_empty() {
        return;
    }
    if std::env::var_os("GENERALS_RIGHTHUD_PROBE").is_some() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            for (screen, kind, elapsed) in &hints {
                eprintln!(
                    "[RHUDPROBE] command marker type={kind:?} screen=({},{}) elapsed={elapsed}",
                    screen.x, screen.y
                );
            }
        });
    }
    let segments = 16;
    let tau = std::f32::consts::TAU;
    {
        for (screen, kind, elapsed) in hints {
            let t = (elapsed.min(40) as f32) / 40.0;
            let (rgb, base_radius) = match kind {
                HintType::Attack => ([1.0_f32, 0.25, 0.2], 15.0),
                _ => ([0.2_f32, 1.0, 0.2], 19.0),
            };
            let alpha = 0.15 + 0.75 * (1.0 - t);
            let radius = 3.0 + base_radius * (1.0 - 0.6 * t);
            let color = [rgb[0], rgb[1], rgb[2], alpha];
            let center = glam::Vec2::new(screen.x as f32, screen.y as f32);
            for i in 0..segments {
                let a0 = (i as f32) * tau / segments as f32;
                let a1 = ((i + 1) as f32) * tau / segments as f32;
                renderer.draw_line(
                    glam::Vec2::new(center.x + radius * a0.cos(), center.y + radius * a0.sin()),
                    glam::Vec2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin()),
                    2.0,
                    color,
                    0.0,
                );
            }
            renderer.draw_rect(
                crate::gui::ui_renderer::UIRect::new(center.x - 1.5, center.y - 1.5, 3.0, 3.0),
                color,
                0.0,
            );
        }
    }
}
