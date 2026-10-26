use super::*;
use std::{cell::Cell, rc::Rc};

#[test]
fn owned_radar_role_preserves_custom_callback_and_hierarchy_order() {
    let mut manager = crate::gui::window_manager::WindowManager::new();
    let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
    let child = manager.create_window(Some(&parent), 1, 1, 10, 10).unwrap();
    let order = Rc::new(Cell::new(0));
    for (window, digit) in [(&parent, 1), (&child, 2)] {
        let mut window = window.borrow_mut();
        window.set_status(WindowStatus::ENABLED);
        window.set_radar_draw_callback();
        let order = order.clone();
        window.set_draw_callback(move |_, _, _| order.set(order.get() * 10 + digit));
        assert!(!window.callbacks.owned_radar_draw);
    }
    let mut context = RadarDrawContext {
        radar: None,
        view_box: None,
        admit: &mut |_| panic!("missing radar cannot admit"),
    };
    manager.draw_all_with_radar(
        &mut context,
        &mut crate::gui::window_draw_context::WindowDrawContext::default(),
    );
    assert_eq!(order.get(), 12);
    parent.borrow_mut().set_radar_draw_callback();
    assert!(parent.borrow().callbacks.owned_radar_draw);
    parent.borrow_mut().reset_draw_callback();
    assert!(!parent.borrow().callbacks.owned_radar_draw);
    parent.borrow_mut().set_radar_draw_callback();
    parent.borrow_mut().set_callbacks(None, None, None);
    assert!(!parent.borrow().callbacks.owned_radar_draw);
}

#[test]
fn explicit_missing_radar_and_geometry_fail_closed_without_core_projection() {
    let mut manager = crate::gui::window_manager::WindowManager::new();
    let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
    window.borrow_mut().set_status(WindowStatus::ENABLED);
    window.borrow_mut().set_radar_draw_callback();
    let mut context = RadarDrawContext {
        radar: None,
        view_box: None,
        admit: &mut |_| panic!("missing radar cannot admit"),
    };
    manager.draw_all_with_radar(
        &mut context,
        &mut crate::gui::window_draw_context::WindowDrawContext::default(),
    );
    assert!(owned_view_box_lines(&context, (1, 1, 98, 98), (0, 0, 100, 100)).is_empty());
    let radar = game_engine::common::system::radar::RadarSystem::new().presentation();
    let mut context = RadarDrawContext {
        radar: Some(&radar),
        view_box: None,
        admit: &mut |_| {},
    };
    assert!(owned_view_box_lines(&context, (1, 1, 98, 98), (0, 0, 100, 100)).is_empty());
}

#[test]
fn owned_corner_lines_use_receiving_radar_and_original_cell_order() {
    let mut radar = game_engine::common::system::radar::RadarSystem::new();
    radar.new_map_owned(Coord3D::new(0., 0., 0.), Coord3D::new(128., 128., 0.), &[]);
    let presentation = radar.presentation();
    let view = RadarViewCorners {
        origin_world: Coord3D::new(32., 32., 0.),
        corners: [
            Coord3D::new(32., 32., 0.),
            Coord3D::new(96., 32., 0.),
            Coord3D::new(96., 96., 0.),
            Coord3D::new(32., 96., 0.),
        ],
    };
    let mut context = RadarDrawContext {
        radar: Some(&presentation),
        view_box: Some(view),
        admit: &mut |_| {},
    };
    let lines = owned_view_box_lines(&context, (10, 20, 128, 128), (9, 19, 130, 130));
    let expected = presentation.build_view_box_lines(
        view.origin_world,
        view.corners,
        10,
        20,
        128,
        128,
        9,
        19,
        130,
        130,
    );
    assert_eq!(lines.len(), 4);
    for (actual, expected) in lines.iter().zip(expected) {
        assert_eq!(
            (actual.start.x, actual.start.y, actual.end.x, actual.end.y),
            (
                expected.start.x,
                expected.start.y,
                expected.end.x,
                expected.end.y
            )
        );
    }
}
