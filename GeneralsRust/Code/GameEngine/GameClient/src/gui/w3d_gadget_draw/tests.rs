use super::{
    PushButtonDrawBank, push_button_color_entry_index, push_button_one_image_source,
    w3d_gadget_push_button_draw,
};
use super::{
    TextEntryImageTileKind, text_entry_clip_region, text_entry_cursor_window_x,
    text_entry_draws_visible_composition, text_entry_focus_matches, text_entry_image_tile_rects,
    text_entry_password_composition_is_masked, text_entry_start_y, text_entry_text_color_defined,
    text_entry_text_draw_x, text_entry_w3d_display_text, truncate_to_i32,
};
use super::{
    WIN_COLOR_UNDEFINED, static_text_draw_data, static_text_text_colors, static_text_text_position,
};
use super::{
    check_box_image_source, combo_box_title_adjustment, compute_tab_layout,
    horizontal_slider_box_counts, horizontal_slider_box_image_sources,
    horizontal_slider_image_draw_a_sources, horizontal_slider_image_draw_b_sources,
    progress_bar_image_draw_a_bank, progress_bar_image_draw_a_sources, progress_bar_image_sources,
    progress_bar_image_width, progress_bar_solid_sources, progress_bar_solid_width,
    progress_percent, push_button_three_piece_tail_clip, radio_button_image_set_complete,
    radio_button_image_sources, radio_button_solid_box_source, solid_check_box_mark_color,
};
use super::{
    list_box_image_content_width, list_box_selected_image_rect, list_box_selected_image_slots,
    list_box_solid_content_width, list_box_solid_frame_and_content_widths,
};
use crate::gui::gadgets::{
    Color, ListBox, ListBoxItemData, ProgressBar, PushButton, TabControl, TabControlData,
    TextAlignment, TextEntry, VerticalAlignment,
};
use crate::gui::game_window::{
    GameWindow, WindowInstanceData, WindowState, WindowStatus, WindowWidget,
};

#[test]
fn test_truncate_to_i32_matches_cpp_cast_behavior() {
    assert_eq!(truncate_to_i32(76.8), 76);
    assert_eq!(truncate_to_i32(76.2), 76);
    assert_eq!(truncate_to_i32(-3.7), -3);
}

#[test]
fn test_text_entry_image_tiles_match_cpp_order_and_overlap() {
    let tiles = text_entry_image_tile_rects(100, 50, 73, 20, 3, 4, 8, 9, 16, 5);

    assert_eq!(
        tiles,
        vec![
            (TextEntryImageTileKind::Center, 111, 54, 127, 74),
            (TextEntryImageTileKind::Center, 127, 54, 143, 74),
            (TextEntryImageTileKind::Center, 143, 54, 159, 74),
            (TextEntryImageTileKind::SmallCenter, 159, 54, 164, 74),
            (TextEntryImageTileKind::SmallCenter, 164, 54, 169, 74),
            (TextEntryImageTileKind::Left, 103, 54, 111, 74),
            (TextEntryImageTileKind::Right, 167, 54, 176, 74),
        ]
    );
}

#[test]
fn test_text_entry_image_tiles_preserve_cpp_small_center_gap_behavior() {
    let tiles = text_entry_image_tile_rects(0, 0, 20, 10, 0, 0, 8, 8, 16, 5);

    assert_eq!(
        tiles,
        vec![
            (TextEntryImageTileKind::SmallCenter, 8, 0, 13, 10),
            (TextEntryImageTileKind::Left, 0, 0, 8, 10),
            (TextEntryImageTileKind::Right, 12, 0, 20, 10),
        ]
    );
}

#[test]
fn text_entry_text_draw_x_keeps_long_end_visible_like_cpp() {
    assert_eq!(text_entry_text_draw_x(false, 10, 100, 40), 12);
    assert_eq!(text_entry_text_draw_x(false, 10, 100, 240), -138);
}

#[test]
fn text_entry_draw_from_start_uses_cpp_x_and_full_window_clip() {
    assert_eq!(text_entry_text_draw_x(true, 10, 100, 240), 15);
    assert_eq!(
        text_entry_clip_region(true, 10, 20, 100, 14, 7, 9, 120, 22),
        super::region_from_corners(7, 9, 127, 31)
    );
    assert_eq!(
        text_entry_clip_region(false, 10, 20, 100, 14, 7, 9, 120, 22),
        super::region_from_corners(10, 20, 110, 34)
    );
}

#[test]
fn text_entry_one_line_y_and_cursor_position_match_cpp() {
    assert_eq!(text_entry_start_y(50, 20, 12, true), 4);
    assert_eq!(text_entry_start_y(50, 20, 12, false), 55);
    assert_eq!(text_entry_cursor_window_x(138, 100), 40);
}

#[test]
fn text_entry_skips_undefined_text_color_like_cpp() {
    assert!(text_entry_text_color_defined(0xFF102030));
    assert!(!text_entry_text_color_defined(WIN_COLOR_UNDEFINED));
}

#[test]
fn text_entry_password_ime_composition_is_masked_like_cpp() {
    let mut entry = TextEntry::new(1, 0, 0, 100, 30).as_password();
    entry.set_text("abc");
    entry.set_ime_composition("de", 1);

    assert_eq!(text_entry_w3d_display_text(&entry), "*****");
    assert!(text_entry_password_composition_is_masked(&entry));
    assert!(!text_entry_draws_visible_composition(&entry));
}

#[test]
fn text_entry_normal_ime_composition_stays_visible_like_cpp() {
    let mut entry = TextEntry::new(1, 0, 0, 100, 30);
    entry.set_text("abc");
    entry.set_ime_composition("de", 1);

    assert_eq!(text_entry_w3d_display_text(&entry), "abc");
    assert!(!text_entry_password_composition_is_masked(&entry));
    assert!(text_entry_draws_visible_composition(&entry));
}

#[test]
fn text_entry_caret_focus_matches_cpp_entry_or_combo_parent_rule() {
    assert!(text_entry_focus_matches(10, None, Some(10)));
    assert!(text_entry_focus_matches(10, Some(20), Some(20)));
    assert!(!text_entry_focus_matches(10, Some(20), Some(30)));
    assert!(!text_entry_focus_matches(10, None, None));
}

#[test]
fn push_button_one_image_enabled_selected_uses_hilite_selected() {
    assert_eq!(
        push_button_one_image_source(WindowStatus::ENABLED, WindowState::SELECTED, true,),
        (PushButtonDrawBank::Hilite, 1)
    );
}

#[test]
fn push_button_one_image_overlay_uses_enabled_base() {
    assert_eq!(
        push_button_one_image_source(
            WindowStatus::ENABLED | WindowStatus::USE_OVERLAY_STATES,
            WindowState::HILITED | WindowState::SELECTED,
            true,
        ),
        (PushButtonDrawBank::Enabled, 0)
    );
}

#[test]
fn push_button_color_selected_uses_current_bank_selected_slot() {
    assert_eq!(
        push_button_color_entry_index(WindowStatus::ENABLED, WindowState::SELECTED, true),
        (PushButtonDrawBank::Enabled, 1)
    );
    assert_eq!(
        push_button_color_entry_index(
            WindowStatus::ENABLED,
            WindowState::HILITED | WindowState::SELECTED,
            true,
        ),
        (PushButtonDrawBank::Hilite, 1)
    );
    assert_eq!(
        push_button_color_entry_index(WindowStatus::empty(), WindowState::SELECTED, false),
        (PushButtonDrawBank::Disabled, 1)
    );
}

#[test]
fn push_button_solid_draw_keeps_color_slot_separate_from_image_draw() {
    assert_eq!(
        push_button_color_entry_index(WindowStatus::ENABLED, WindowState::SELECTED, true),
        (PushButtonDrawBank::Enabled, 1)
    );
    assert_eq!(
        push_button_one_image_source(WindowStatus::ENABLED, WindowState::SELECTED, true),
        (PushButtonDrawBank::Hilite, 1)
    );
}

#[test]
fn gadget_gpu_fill_rect_mesh_is_two_triangles_matching_window_rect() {
    let rect = crate::gui::ui_renderer::UIRect::new(10.0, 20.0, 100.0, 30.0);
    let color = [1.0, 0.0, 0.0, 1.0];
    let (positions, uvs, colors, indices) =
        crate::gui::ui_renderer::UIRenderer::gadget_gpu_fill_rect_mesh(rect, color, 0.0);
    assert_eq!(positions.len(), 4);
    assert_eq!(uvs.len(), 4);
    assert_eq!(colors.len(), 4);
    assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(positions[0], [10.0, 20.0, 0.0]);
    assert_eq!(positions[2], [110.0, 50.0, 0.0]);

    let mut window = GameWindow::new();
    window.set_status(WindowStatus::ENABLED);
    let _ = window.set_position(10, 20);
    let _ = window.set_size(100, 30);
    let scaled = super::press_scaled_rect(&window);
    let (btn_pos, _, _, btn_idx) = crate::gui::ui_renderer::UIRenderer::gadget_gpu_fill_rect_mesh(
        scaled,
        [1.0, 1.0, 1.0, 1.0],
        0.0,
    );
    assert_eq!(btn_idx.len(), 6);
    assert_eq!(btn_pos[0][0], scaled.x);
    assert_eq!(btn_pos[0][1], scaled.y);
}

#[test]
fn w3d_push_button_draw_consumes_clock_request_like_cpp() {
    let mut window = GameWindow::new();
    window.set_status(WindowStatus::ENABLED);
    let mut button = PushButton::new(7, 0, 0, 100, 30);
    button.set_clock_progress(50, Color::GREEN);
    window.set_widget(WindowWidget::PushButton(button));

    w3d_gadget_push_button_draw(&window, window.instance_data());

    let Some(WindowWidget::PushButton(button)) = window.widget() else {
        panic!("push button widget missing");
    };
    assert_eq!(button.clock_request(), None);
    assert_eq!(button.consume_clock_request(), None);
}

#[test]
fn push_button_three_piece_tail_uses_full_tile_with_clip_like_cpp() {
    let (draw, clip) = push_button_three_piece_tail_clip(132, 137, 40, 64, 16).unwrap();

    assert_eq!(draw, (132, 40, 148, 64));
    assert_eq!(clip, super::region_from_corners(132, 40, 137, 64));
    assert_eq!(clip.width, 5);
}

#[test]
fn push_button_three_piece_tail_skips_exact_fit_like_cpp() {
    assert_eq!(
        push_button_three_piece_tail_clip(144, 144, 40, 64, 16),
        None
    );
}

#[test]
fn list_box_selected_image_slots_require_all_cpp_images() {
    assert_eq!(
        list_box_selected_image_slots([true, true, true, true]),
        Some([1, 2, 3, 4])
    );
    assert_eq!(
        list_box_selected_image_slots([true, true, false, true]),
        None
    );
}

#[test]
fn list_box_selected_image_rect_matches_cpp_clip() {
    let clip = super::region_from_corners(11, 17, 79, 60);
    assert_eq!(
        list_box_selected_image_rect(10, 14, 70, 9, &clip),
        Some((11, 17, 80, 24))
    );
    assert_eq!(
        list_box_selected_image_rect(10, 55, 70, 9, &clip),
        Some((11, 55, 80, 60))
    );
    assert_eq!(list_box_selected_image_rect(10, 60, 70, 9, &clip), None);
}

#[test]
fn list_box_scrollbar_width_adjustment_matches_cpp_draw_variants() {
    assert_eq!(list_box_solid_content_width(100, Some((17, false))), 80);
    assert_eq!(list_box_solid_content_width(100, Some((17, true))), 100);
    assert_eq!(list_box_solid_content_width(100, None), 100);

    assert_eq!(list_box_image_content_width(100, Some(17)), 83);
    assert_eq!(list_box_image_content_width(100, None), 100);
}

#[test]
fn solid_list_box_keeps_full_frame_width_when_scrollbar_is_visible_like_cpp() {
    assert_eq!(
        list_box_solid_frame_and_content_widths(100, Some((17, false))),
        (100, 80)
    );
    assert_eq!(
        list_box_solid_frame_and_content_widths(100, Some((17, true))),
        (100, 100)
    );
}

#[test]
fn list_box_solid_row_text_does_not_overwrite_title_display_string() {
    let mut window = GameWindow::new();
    window.set_size(160, 60).unwrap();
    window.set_text("Title").unwrap();

    let mut listbox = ListBox::new(7, 0, 0, 160, 60);
    listbox.set_columns(2);
    let row = listbox.add_item_with_data_and_color(1, "Fallback", None, None);
    assert!(listbox.set_item_column_data(row, 0, ListBoxItemData::Text("Alpha".to_string())));
    assert!(listbox.set_item_column_data(row, 1, ListBoxItemData::Text("Bravo".to_string())));
    window.set_widget(WindowWidget::ListBox(listbox));

    let title = window
        .instance_data()
        .display_text
        .as_ref()
        .expect("set_text should create title display string")
        .clone();
    assert_eq!(title.borrow().get_text(), "Title");

    super::w3d_gadget_list_box_draw(&window, window.instance_data());

    assert_eq!(title.borrow().get_text(), "Title");
}

#[test]
fn list_box_image_row_text_does_not_require_title_display_string() {
    let mut window = GameWindow::new();
    window.set_size(160, 60).unwrap();

    let mut listbox = ListBox::new(7, 0, 0, 160, 60);
    listbox.set_columns(1);
    let row = listbox.add_item_with_data_and_color(1, "Alpha", None, None);
    assert!(listbox.set_item_column_data(row, 0, ListBoxItemData::Text("Alpha".to_string())));
    window.set_widget(WindowWidget::ListBox(listbox));

    assert!(window.instance_data().display_text.is_none());

    super::w3d_gadget_list_box_image_draw(&window, window.instance_data());

    assert!(window.instance_data().display_text.is_none());
}

#[test]
fn horizontal_slider_default_image_sources_match_cpp() {
    assert_eq!(horizontal_slider_box_image_sources(), (0, 1, 0));
}

#[test]
fn tab_control_layout_does_not_force_phantom_tab() {
    let mut window = GameWindow::new();
    window.set_size(200, 100).unwrap();
    let mut tab_control = TabControl::new(7, 0, 0, 200, 100);
    tab_control.set_tab_data(TabControlData {
        tab_edge: super::TP_TOP_SIDE,
        tab_width: 40,
        tab_height: 20,
        tab_count: 0,
        pane_border: 3,
        ..Default::default()
    });

    let (_, _, _, _, _, _, tab_count) = compute_tab_layout(&window, &tab_control);

    assert_eq!(tab_count, 0);
}

#[test]
fn horizontal_slider_box_counts_match_cpp_centering() {
    assert_eq!(horizontal_slider_box_counts(10, 52, 0.5), (4, 3, 2));
    assert_eq!(horizontal_slider_box_counts(10, 52, 0.0), (4, 0, 2));
}

#[test]
fn horizontal_slider_image_draw_b_sources_match_cpp() {
    assert_eq!(horizontal_slider_image_draw_b_sources(), (0, 1, 0));
}

#[test]
fn horizontal_slider_image_draw_a_sources_match_cpp_enabled_path() {
    assert_eq!(
        horizontal_slider_image_draw_a_sources(true),
        [
            (PushButtonDrawBank::Hilite, 0),
            (PushButtonDrawBank::Hilite, 1),
            (PushButtonDrawBank::Hilite, 2),
            (PushButtonDrawBank::Hilite, 3),
            (PushButtonDrawBank::Enabled, 0),
            (PushButtonDrawBank::Enabled, 1),
            (PushButtonDrawBank::Enabled, 2),
            (PushButtonDrawBank::Enabled, 3),
        ]
    );
}

#[test]
fn horizontal_slider_image_draw_a_sources_share_disabled_images() {
    assert_eq!(
        horizontal_slider_image_draw_a_sources(false),
        [
            (PushButtonDrawBank::Disabled, 0),
            (PushButtonDrawBank::Disabled, 1),
            (PushButtonDrawBank::Disabled, 2),
            (PushButtonDrawBank::Disabled, 3),
            (PushButtonDrawBank::Disabled, 0),
            (PushButtonDrawBank::Disabled, 1),
            (PushButtonDrawBank::Disabled, 2),
            (PushButtonDrawBank::Disabled, 3),
        ]
    );
}

#[test]
fn progress_bar_image_sources_match_cpp_slots() {
    assert_eq!(progress_bar_image_sources(), (0, 1, 2, 5, 6));
}

#[test]
fn progress_bar_image_draw_a_sources_match_cpp_enabled_slots() {
    assert_eq!(progress_bar_image_draw_a_sources(), (6, 5, 0, 1, 2));
    assert_eq!(
        progress_bar_image_draw_a_bank(),
        PushButtonDrawBank::Enabled
    );
}

#[test]
fn progress_bar_solid_sources_match_cpp_color_slots() {
    assert_eq!(progress_bar_solid_sources(), (0, 4));
}

#[test]
fn progress_bar_w3d_draw_uses_raw_user_data_without_clamping() {
    assert_eq!(progress_bar_solid_width(100, 125), 125);
    assert_eq!(progress_bar_image_width(120, 125), 125);
    assert_eq!(progress_bar_solid_width(100, -25), -25);
    assert_eq!(progress_bar_image_width(120, -25), -25);
}

#[test]
fn progress_bar_w3d_progress_prefers_raw_user_data_like_cpp() {
    let mut window = GameWindow::new();
    let mut bar = ProgressBar::new(7, 0, 0, 100, 16);
    bar.set_percentage(50.0);
    window.set_widget(WindowWidget::ProgressBar(bar));
    window.set_user_data(125i32);

    assert_eq!(progress_percent(&window), 125);
}

#[test]
fn static_text_draw_data_ignores_hilite_like_cpp() {
    let mut window = GameWindow::new();
    window.enable(true).unwrap();
    let mut inst_data = WindowInstanceData::default();
    inst_data.state = WindowState::HILITED | WindowState::SELECTED;
    inst_data.enabled_draw_data[0].color = 0xFF112233;
    inst_data.disabled_draw_data[0].color = 0xFF445566;
    inst_data.hilite_draw_data[0].color = 0xFF778899;
    inst_data.enabled_text.color = 0xFFABCDEF;
    inst_data.disabled_text.color = 0xFF102030;
    inst_data.hilite_text.color = 0xFF506070;

    let (draw_data, text) = static_text_draw_data(&window, &inst_data);

    assert_eq!(draw_data[0].color, 0xFF112233);
    assert_eq!(text.color, 0xFFABCDEF);
}

#[test]
fn static_text_draw_data_uses_disabled_when_window_disabled() {
    let window = GameWindow::new();
    let mut inst_data = WindowInstanceData::default();
    inst_data.enabled_draw_data[0].color = 0xFF112233;
    inst_data.disabled_draw_data[0].color = 0xFF445566;
    inst_data.enabled_text.color = 0xFFABCDEF;
    inst_data.disabled_text.color = 0xFF102030;

    let (draw_data, text) = static_text_draw_data(&window, &inst_data);

    assert_eq!(draw_data[0].color, 0xFF445566);
    assert_eq!(text.color, 0xFF102030);
}

#[test]
fn static_text_text_colors_skip_undefined_like_cpp() {
    let mut window = GameWindow::new();
    window.enable(true).unwrap();
    let mut inst_data = WindowInstanceData::default();
    inst_data.enabled_text.color = WIN_COLOR_UNDEFINED;
    inst_data.enabled_text.border_color = 0xFF102030;

    assert_eq!(static_text_text_colors(&window, &inst_data), None);
}

#[test]
fn static_text_position_ignores_right_and_bottom_like_cpp() {
    assert_eq!(
        static_text_text_position(
            10,
            20,
            100,
            40,
            30,
            12,
            7,
            5,
            TextAlignment::Right,
            VerticalAlignment::Bottom,
        ),
        (17, 25)
    );
}

#[test]
fn static_text_position_centers_only_cpp_center_flags() {
    assert_eq!(
        static_text_text_position(
            10,
            20,
            100,
            40,
            30,
            12,
            7,
            5,
            TextAlignment::Center,
            VerticalAlignment::Center,
        ),
        (45, 34)
    );
}

#[test]
fn check_box_image_source_uses_cpp_checked_slots() {
    assert_eq!(
        check_box_image_source(WindowState::empty(), true),
        (PushButtonDrawBank::Enabled, 1)
    );
    assert_eq!(
        check_box_image_source(WindowState::SELECTED, true),
        (PushButtonDrawBank::Enabled, 2)
    );
    assert_eq!(
        check_box_image_source(WindowState::HILITED | WindowState::SELECTED, true),
        (PushButtonDrawBank::Hilite, 2)
    );
    assert_eq!(
        check_box_image_source(WindowState::DISABLED | WindowState::SELECTED, true),
        (PushButtonDrawBank::Disabled, 2)
    );
    assert_eq!(
        check_box_image_source(WindowState::SELECTED, false),
        (PushButtonDrawBank::Disabled, 2)
    );
}

#[test]
fn solid_check_box_mark_draws_whenever_slot_color_is_defined_like_cpp() {
    assert_eq!(solid_check_box_mark_color(0xFF123456), Some(0xFF123456));
    assert_eq!(solid_check_box_mark_color(WIN_COLOR_UNDEFINED), None);
}

#[test]
fn radio_button_image_sources_match_cpp_branch_order() {
    assert_eq!(
        radio_button_image_sources(WindowState::SELECTED, true),
        (PushButtonDrawBank::Hilite, [3, 4, 5])
    );
    assert_eq!(
        radio_button_image_sources(WindowState::SELECTED | WindowState::DISABLED, false),
        (PushButtonDrawBank::Hilite, [3, 4, 5])
    );
    assert_eq!(
        radio_button_image_sources(WindowState::DISABLED, true),
        (PushButtonDrawBank::Disabled, [0, 1, 2])
    );
    assert_eq!(
        radio_button_image_sources(WindowState::HILITED, true),
        (PushButtonDrawBank::Hilite, [0, 1, 2])
    );
    assert_eq!(
        radio_button_image_sources(WindowState::empty(), true),
        (PushButtonDrawBank::Enabled, [0, 1, 2])
    );
}

#[test]
fn radio_button_solid_box_source_uses_selected_slot_like_cpp() {
    assert_eq!(
        radio_button_solid_box_source(WindowState::empty(), true),
        (PushButtonDrawBank::Enabled, 1)
    );
    assert_eq!(
        radio_button_solid_box_source(WindowState::SELECTED, true),
        (PushButtonDrawBank::Enabled, 2)
    );
    assert_eq!(
        radio_button_solid_box_source(WindowState::HILITED | WindowState::SELECTED, true),
        (PushButtonDrawBank::Hilite, 2)
    );
    assert_eq!(
        radio_button_solid_box_source(WindowState::DISABLED | WindowState::SELECTED, true),
        (PushButtonDrawBank::Disabled, 2)
    );
    assert_eq!(
        radio_button_solid_box_source(WindowState::SELECTED, false),
        (PushButtonDrawBank::Disabled, 2)
    );
}

#[test]
fn radio_button_image_draw_requires_all_strip_images_like_cpp() {
    assert!(radio_button_image_set_complete([true, true, true]));
    assert!(!radio_button_image_set_complete([false, true, true]));
    assert!(!radio_button_image_set_complete([true, false, true]));
    assert!(!radio_button_image_set_complete([true, true, false]));
}

#[test]
fn combo_box_title_adjustment_matches_cpp_draw_variants() {
    assert_eq!(combo_box_title_adjustment(false, 14, false), None);
    assert_eq!(combo_box_title_adjustment(true, 14, false), Some((15, 15)));
    assert_eq!(combo_box_title_adjustment(true, 14, true), Some((14, 15)));
}

fn test_window(x: i32, y: i32, w: i32, h: i32) -> GameWindow {
    let mut window = GameWindow::new();
    window.set_status(WindowStatus::ENABLED);
    let _ = window.set_position(x, y);
    let _ = window.set_size(w, h);
    window
}

#[test]
fn w3d_main_menu_draw_queues_commands_without_mapped_art() {
    super::reset_shipped_ui_draw_command_count();
    let window = test_window(0, 0, 800, 600);
    super::w3d_main_menu_draw(&window, window.instance_data());
    assert!(
        super::shipped_ui_draw_command_count() > 0,
        "main menu chrome must queue fallback rects/lines when images are missing"
    );
}

#[test]
fn w3d_main_menu_four_and_metal_bar_queue_commands_without_art() {
    super::reset_shipped_ui_draw_command_count();
    let window = test_window(0, 80, 800, 40);
    super::w3d_main_menu_four_draw(&window, window.instance_data());
    super::w3d_metal_bar_menu_draw(&window, window.instance_data());
    super::w3d_main_menu_map_border(&window, window.instance_data());
    super::w3d_thin_border_draw(&window, window.instance_data());
    assert!(
        super::shipped_ui_draw_command_count() > 0,
        "menu metal/map/thin chrome must still emit draw commands"
    );
}

#[test]
fn w3d_command_bar_background_draw_queues_hud_fallback() {
    super::reset_shipped_ui_draw_command_count();
    let window = test_window(0, 460, 800, 140);
    super::w3d_command_bar_background_draw(&window, window.instance_data());
    assert!(
        super::shipped_ui_draw_command_count() > 0,
        "control bar background must draw a HUD strip when scheme/art is missing"
    );
}

#[test]
fn w3d_command_bar_foreground_draw_queues_hud_fallback() {
    super::reset_shipped_ui_draw_command_count();
    let window = test_window(8, 595, 5, 5);
    super::w3d_command_bar_foreground_draw(&window, window.instance_data());
    assert!(
        super::shipped_ui_draw_command_count() > 0,
        "tiny BackgroundMarker-sized window must still expand into a visible HUD strip"
    );
}

#[test]
fn w3d_push_button_draw_queues_fallback_when_colors_undefined() {
    super::reset_shipped_ui_draw_command_count();
    let mut window = test_window(20, 20, 120, 32);
    let button = PushButton::new(7, 0, 0, 120, 32);
    window.set_widget(WindowWidget::PushButton(button));
    super::w3d_gadget_push_button_draw(&window, window.instance_data());
    assert!(
        super::shipped_ui_draw_command_count() > 0,
        "push button must queue a visible fill when draw-data colors/images are missing"
    );
}

#[test]
fn w3d_push_button_image_draw_queues_nothing_when_no_image_is_bound() {
    // C++ W3DGadgetPushButtonImageDrawOne (W3DPushButton.cpp:288-368) skips
    // the drawImage block when the state's image slot is empty: an
    // IMAGE-status button (ControlBar.wnd command buttons) renders nothing
    // until setControlCommand binds art — never the authored red fill.
    super::reset_shipped_ui_draw_command_count();
    let mut window = test_window(20, 60, 64, 64);
    window.set_status(WindowStatus::ENABLED | WindowStatus::IMAGE);
    let button = PushButton::new(8, 0, 0, 64, 64);
    window.set_widget(WindowWidget::PushButton(button));
    super::w3d_gadget_push_button_image_draw(&window, window.instance_data());
    assert_eq!(
        super::shipped_ui_draw_command_count(),
        0,
        "unbound image-status button must not paint a placeholder fill"
    );
}

#[test]
fn w3d_power_and_progress_queue_commands_without_art() {
    super::reset_shipped_ui_draw_command_count();
    let mut progress = test_window(10, 10, 200, 16);
    let mut bar = ProgressBar::new(3, 0, 0, 200, 16);
    bar.set_percentage(40.0);
    progress.set_widget(WindowWidget::ProgressBar(bar));
    super::w3d_gadget_progress_bar_draw(&progress, progress.instance_data());
    super::w3d_gadget_progress_bar_image_draw(&progress, progress.instance_data());

    let power = test_window(261, 473, 283, 7);
    super::w3d_power_draw(&power, power.instance_data());
    super::w3d_power_draw_a(&power, power.instance_data());

    assert!(
        super::shipped_ui_draw_command_count() > 0,
        "progress and power meters must emit fallback commands"
    );
}
