#![cfg(feature = "w3d")]

use game_engine_device::w3d::fow_terrain_overlay::{
    ShroudCopyRect, ShroudMapMetrics, W3DShroudConfig, W3DShroudState,
};

fn map_metrics() -> ShroudMapMetrics {
    ShroudMapMetrics {
        x_extent: 130,
        y_extent: 66,
        border_size_inline: 1,
        draw_width: 33,
        draw_height: 17,
        draw_origin_x: 8,
        draw_origin_y: 4,
        map_xy_factor: 50.0,
    }
}

#[test]
fn init_matches_cpp_cell_and_texture_sizing() {
    let mut shroud = W3DShroudState::new(W3DShroudConfig {
        shroud_alpha: 3,
        shroud_color: 0x00ff_ffff,
        fog_of_war_on: false,
    });

    shroud.init(map_metrics(), 100.0, 100.0);

    assert_eq!(shroud.num_cells_x(), 64);
    assert_eq!(shroud.num_cells_y(), 32);
    assert_eq!(shroud.texture_width(), 128);
    assert_eq!(shroud.texture_height(), 64);
    assert_eq!(shroud.cell_width(), 100.0);
    assert_eq!(shroud.cell_height(), 100.0);
    assert!(shroud.clear_dst_texture());
}

#[test]
fn rgb565_get_set_clamps_to_shroud_alpha_and_rejects_border_row() {
    let mut shroud = W3DShroudState::new(W3DShroudConfig {
        shroud_alpha: 25,
        shroud_color: 0x00ff_ffff,
        fog_of_war_on: false,
    });
    shroud.init(map_metrics(), 100.0, 100.0);

    assert!(shroud.set_shroud_level(2, 3, 10, false));
    assert_eq!(shroud.get_shroud_level(2, 3), 24);

    assert!(shroud.set_shroud_level(2, 3, 255, false));
    assert_eq!(shroud.pixel_at(2, 3), Some(0xffff));
    assert_eq!(shroud.get_shroud_level(2, 3), 255);

    assert!(!shroud.set_shroud_level(99, 99, 255, false));
    assert_eq!(shroud.get_shroud_level(99, 99), 0);
    assert!(shroud.pixel_at(0, shroud.num_cells_y()).is_some());
    assert!(!shroud.set_shroud_level(0, shroud.num_cells_y(), 255, false));
    assert_eq!(shroud.get_shroud_level(0, shroud.num_cells_y()), 0);
}

#[test]
fn fog_mode_uses_alpha_nibble_inverse_level() {
    let mut shroud = W3DShroudState::new(W3DShroudConfig {
        shroud_alpha: 0,
        shroud_color: 0x00a0_8040,
        fog_of_war_on: true,
    });
    shroud.init(map_metrics(), 100.0, 100.0);

    shroud.set_shroud_level(1, 1, 255, false);
    assert_eq!(shroud.pixel_at(1, 1).unwrap() >> 12, 0);
    assert_eq!(shroud.get_shroud_level(1, 1), 255);

    shroud.set_shroud_level(1, 1, 0, false);
    assert_eq!(shroud.pixel_at(1, 1).unwrap() >> 12, 15);
    assert_eq!(shroud.get_shroud_level(1, 1), 0);
}

#[test]
fn fill_and_border_clear_match_cpp_flags() {
    let mut shroud = W3DShroudState::new(W3DShroudConfig {
        shroud_alpha: 0,
        shroud_color: 0x00ff_ffff,
        fog_of_war_on: false,
    });
    shroud.init(map_metrics(), 100.0, 100.0);
    shroud.fill_shroud_data(255);
    assert_eq!(shroud.pixel_at(0, 0), Some(0xffff));

    shroud.set_border_shroud_level(127);
    assert!(shroud.clear_dst_texture());
    let rect = shroud.render(map_metrics()).unwrap();
    assert!(rect.cleared_border);
    assert_eq!(rect.dst_x, 1);
    assert_eq!(rect.dst_y, 1);
    assert!(!shroud.clear_dst_texture());
    assert!(shroud.pixel_at(0, shroud.num_cells_y()).is_some());
}

#[test]
fn render_updates_full_map_copy_rect_and_draw_origin() {
    let mut shroud = W3DShroudState::default();
    shroud.init(map_metrics(), 100.0, 100.0);

    let rect = shroud.render(map_metrics()).unwrap();

    assert_eq!(
        rect,
        ShroudCopyRect {
            left: 0,
            top: 0,
            right: 64,
            bottom: 32,
            dst_x: 1,
            dst_y: 1,
            cleared_border: true,
        }
    );
    assert_eq!(shroud.draw_origin_x(), 0.0);
    assert_eq!(shroud.draw_origin_y(), 0.0);
}

#[test]
fn reset_reacquire_and_filter_flags_match_cpp_lifecycle() {
    let mut shroud = W3DShroudState::default();
    shroud.init(map_metrics(), 100.0, 100.0);
    assert!(shroud.reacquire_resources());

    shroud.set_shroud_filter(false);
    assert!(!shroud.shroud_filter_enabled());
    shroud.set_shroud_filter(true);
    assert!(shroud.shroud_filter_enabled());

    shroud.reset();
    assert_eq!(shroud.get_shroud_level(0, 0), 0);
    assert!(shroud.clear_dst_texture());
    shroud.release_resources();
    assert!(shroud.reacquire_resources());
}

#[test]
fn fog_interpolation_moves_current_toward_final() {
    let mut shroud = W3DShroudState::new(W3DShroudConfig {
        shroud_alpha: 0,
        shroud_color: 0x00ff_ffff,
        fog_of_war_on: false,
    });
    shroud.init(map_metrics(), 100.0, 100.0);
    shroud.set_shroud_level(0, 0, 255, false);

    shroud.interpolate_fog_levels(500);
    let mid = shroud.get_shroud_level(0, 0);
    assert!((120..=130).contains(&mid));

    shroud.interpolate_fog_levels(1000);
    assert_eq!(shroud.get_shroud_level(0, 0), 255);
}
