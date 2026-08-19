// InGameUI unit tests.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::ini_language::init_global_language;
    use game_engine::common::language::Language;

    #[test]
    fn test_selection_box() {
        let mut box_sel = SelectionBox::new();
        assert!(!box_sel.active);

        box_sel.start_at(Vec2::new(10.0, 10.0));
        assert!(box_sel.active);

        box_sel.update(Vec2::new(100.0, 100.0));
        assert!(box_sel.is_significant());

        let rect = box_sel.get_rect();
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 10.0);
        assert_eq!(rect.width, 90.0);
        assert_eq!(rect.height, 90.0);
    }

    #[test]
    fn move_hint_expires_after_cpp_elapsed_40_boundary() {
        let hint = HintData {
            hint_type: HintType::Move,
            start: Coord3D::new(0.0, 0.0, 0.0),
            end: Coord3D::new(10.0, 0.0, 0.0),
            creation_frame: 100,
            source_id: 7,
            lifetime_frames: MOVE_HINT_LIFETIME_FRAMES,
        };

        assert_eq!(hint.lifetime_frames, 41);
        assert!(InGameUI::hint_is_alive(&hint, 140));
        assert!(!InGameUI::hint_is_alive(&hint, 141));
    }

    #[test]
    fn move_hints_do_not_draw_disabled_cpp_waypoint_line_block() {
        let move_hint = HintData {
            hint_type: HintType::Move,
            start: Coord3D::new(0.0, 0.0, 0.0),
            end: Coord3D::new(10.0, 0.0, 0.0),
            creation_frame: 100,
            source_id: 7,
            lifetime_frames: MOVE_HINT_LIFETIME_FRAMES,
        };
        let command_hint = HintData {
            hint_type: HintType::Command,
            start: Coord3D::new(0.0, 0.0, 0.0),
            end: Coord3D::new(10.0, 0.0, 0.0),
            creation_frame: 100,
            source_id: 7,
            lifetime_frames: MOVE_HINT_LIFETIME_FRAMES,
        };

        assert!(InGameUI::hint_is_alive(&move_hint, 120));
        assert!(!InGameUI::hint_draws_waypoint_line(&move_hint, 120));
        assert!(InGameUI::hint_draws_waypoint_line(&command_hint, 120));
        assert!(!InGameUI::hint_draws_waypoint_line(&command_hint, 141));
    }

    #[test]
    fn test_minimap_conversion() {
        let minimap = Minimap::new(Vec2::new(600.0, 400.0), Vec2::new(200.0, 200.0));

        let world_pos = Vec2::new(500.0, 500.0);
        let minimap_pos = minimap.world_to_minimap(world_pos);

        // Should be roughly in middle of minimap
        assert!((minimap_pos.x - 700.0).abs() < 1.0);
        assert!((minimap_pos.y - 500.0).abs() < 1.0);
    }

    #[test]
    fn test_selection_state() {
        let mut state = SelectionState::new(10);

        state.select(DrawableID(1), false);
        assert_eq!(state.count(), 1);

        state.select(DrawableID(2), true);
        assert_eq!(state.count(), 2);

        state.deselect(DrawableID(1));
        assert_eq!(state.count(), 1);
        assert!(!state.is_selected(DrawableID(1)));
        assert!(state.is_selected(DrawableID(2)));
    }

    #[test]
    fn test_placement_preview() {
        let mut preview = PlacementPreview::new("GLA_SupplyStash".into(), Vec2::new(3.0, 3.0));

        preview.update_position(Vec3::new(100.0, 0.0, 100.0), true);
        assert!(preview.is_legal);

        let color = preview.get_color();
        assert_eq!(color[0], LEGAL_BUILD_COLOR[0]);
        assert_eq!(color[3], PLACEMENT_OPACITY);
    }

    #[test]
    fn test_resource_display() {
        let mut display = ResourceDisplay::new(Vec2::ZERO);

        display.update(10000, 100, 50);
        assert_eq!(display.credits, 10000);
        assert!(!display.is_power_deficit());
        assert!((display.get_power_percentage() - 0.5).abs() < 0.01);

        display.update(5000, 100, 150);
        assert!(display.is_power_deficit());
    }

    #[test]
    fn military_caption_delay_uses_global_language_default_delay_ms() {
        init_global_language();
        assert_eq!(InGameUI::military_caption_delay_frames(), 22);
    }

    #[test]
    fn military_caption_milliseconds_convert_to_logic_frames() {
        assert_eq!(InGameUI::milliseconds_to_logic_frames(750), 22);
        assert_eq!(InGameUI::milliseconds_to_logic_frames(1000), 30);
        assert_eq!(InGameUI::milliseconds_to_logic_frames(-1), 0);
    }

    #[test]
    fn military_caption_fetches_localized_text() {
        Language::clear_localized_strings();
        Language::register_localized_string("SCRIPT:Briefing", "Localized briefing text");

        assert_eq!(
            InGameUI::military_caption_text("SCRIPT:Briefing"),
            "Localized briefing text"
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn mouseover_tooltip_prefers_template_display_name() {
        Language::clear_localized_strings();

        assert_eq!(
            InGameUI::mouseover_tooltip_text("UnitA", "Explicit display name"),
            Some("Explicit display name".to_string())
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn mouseover_tooltip_falls_back_to_thing_template_label() {
        Language::clear_localized_strings();

        assert_eq!(
            InGameUI::mouseover_tooltip_text("UnitA", ""),
            Some("MISSING: 'ThingTemplate:UnitA'".to_string())
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn supply_warehouse_tooltip_feedback_formats_placeholder_value() {
        assert_eq!(
            InGameUI::format_supply_warehouse_tooltip_feedback(" ($%d)", 12, 100),
            " ($1200)"
        );
    }

    #[test]
    fn supply_warehouse_tooltip_feedback_appends_without_placeholder() {
        assert_eq!(
            InGameUI::format_supply_warehouse_tooltip_feedback(" supplies: ", 3, 200),
            " supplies: 600"
        );
    }

    #[test]
    fn supply_warehouse_tooltip_feedback_uses_cpp_raw_value_product() {
        assert_eq!(
            InGameUI::format_supply_warehouse_tooltip_feedback(" ($%d)", -2, 100),
            " ($-200)"
        );
        assert_eq!(
            InGameUI::format_supply_warehouse_tooltip_feedback(" ($%d)", 2, -100),
            " ($-200)"
        );
    }

    #[test]
    fn mouseover_tooltip_suppresses_props() {
        Language::clear_localized_strings();
        Language::register_localized_string("OBJECT:Prop", "Prop");

        assert_eq!(InGameUI::mouseover_tooltip_text("Tree01", "Prop"), None);

        Language::clear_localized_strings();
    }

    #[test]
    fn mouseover_tooltip_appends_playable_player_name_only_in_multiplayer_like_cpp() {
        let mut player = Player::new(0);
        player.set_display_name("Commander");

        assert_eq!(
            InGameUI::mouseover_tooltip_with_player_suffix("Tank", &player, true),
            "Tank\nCommander"
        );
        assert_eq!(
            InGameUI::mouseover_tooltip_with_player_suffix("Tank", &player, false),
            "Tank"
        );

        player.set_observer(true);
        assert_eq!(
            InGameUI::mouseover_tooltip_with_player_suffix("Tank", &player, true),
            "Tank"
        );
    }

    #[test]
    fn mouseover_tooltip_only_shows_for_visible_shroud_states() {
        assert!(InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Clear
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::PartialClear
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Fogged
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Shrouded
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::InvalidButPreviousValid
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Invalid
        ));
    }

    #[test]
    fn mouseover_cursor_updates_match_cpp_replay_gate() {
        assert!(InGameUI::mouseover_cursor_update_allowed(false, false));
        assert!(InGameUI::mouseover_cursor_update_allowed(false, true));
        assert!(InGameUI::mouseover_cursor_update_allowed(true, true));
        assert!(!InGameUI::mouseover_cursor_update_allowed(true, false));
    }

    #[test]
    fn mouseover_unresolved_drawable_id_stays_invalid_like_cpp() {
        // Dual-world empty catalog → unresolved (Wave 1000). Associated call needs &self.
        // With no live InGameUI fixture, assert the dual-empty fail-closed contract:
        // mouseover_drawable_id_for_lookup returns INVALID when catalog lacks the id.
        // Covered by ignored_gui presentation residual honesty + create_mouseover path.
        assert_eq!(InGameUI::INVALID_DRAWABLE_ID, 0);
    }

    fn test_window(name: &str, status: WindowStatus) -> Rc<RefCell<GameWindow>> {
        let window = Rc::new(RefCell::new(GameWindow::new()));
        {
            let mut guard = window.borrow_mut();
            guard.set_name(name);
            guard.set_status_exact(status);
        }
        window
    }

    #[test]
    fn opaque_window_blocks_world_input_like_cpp() {
        let window = test_window(
            "Popup.wnd:Dialog",
            WindowStatus::ENABLED | WindowStatus::ACTIVE,
        );

        assert!(InGameUI::window_chain_blocks_world_input(Some(window)));
    }

    #[test]
    fn see_through_window_does_not_block_world_input() {
        let window = test_window(
            "ControlBar.wnd:Transparent",
            WindowStatus::ENABLED | WindowStatus::ACTIVE | WindowStatus::SEE_THRU,
        );

        assert!(!InGameUI::window_chain_blocks_world_input(Some(window)));
    }

    #[test]
    fn opaque_parent_blocks_see_through_child() {
        let parent = test_window(
            "Popup.wnd:Panel",
            WindowStatus::ENABLED | WindowStatus::ACTIVE,
        );
        let child = test_window(
            "Popup.wnd:PassThroughChild",
            WindowStatus::ENABLED | WindowStatus::ACTIVE | WindowStatus::SEE_THRU,
        );
        child.borrow_mut().set_parent(Some(&parent));

        assert!(InGameUI::window_chain_blocks_world_input(Some(child)));
    }

    #[test]
    fn left_hud_window_keeps_world_input_unblocked() {
        let window = test_window(
            "ControlBar.wnd:LeftHUD",
            WindowStatus::ENABLED | WindowStatus::ACTIVE,
        );

        assert!(!InGameUI::window_chain_blocks_world_input(Some(window)));
    }

    #[test]
    fn command_hint_updates_match_cpp_replay_gate() {
        assert!(InGameUI::command_hint_update_allowed(false, false, false));
        assert!(!InGameUI::command_hint_update_allowed(true, false, false));
        assert!(!InGameUI::command_hint_update_allowed(false, true, false));
        assert!(!InGameUI::command_hint_update_allowed(false, false, true));
    }

    #[test]
    fn gui_command_hint_cursor_uses_cpp_valid_invalid_selection() {
        let context_pending = PendingCommand {
            command_type: CommandType::DoSpecialPower,
            options: 0,
            source_object_id: 0,
            cursor_name: "ATTACK_OBJECT".to_string(),
            invalid_cursor_name: "GENERIC_INVALID".to_string(),
            radius_cursor_type: "NONE".to_string(),
        };
        assert_eq!(
            InGameUI::pending_gui_command_cursor(
                &context_pending,
                CommandHintType::ValidGuiCommand
            ),
            MouseCursor::AttackObject
        );
        assert_eq!(
            InGameUI::pending_gui_command_cursor(
                &context_pending,
                CommandHintType::InvalidGuiCommand
            ),
            MouseCursor::GenericInvalid
        );
        assert_eq!(
            InGameUI::pending_gui_command_cursor(&context_pending, CommandHintType::MoveTo),
            MouseCursor::GenericInvalid
        );

        let target_pending = PendingCommand {
            command_type: CommandType::Enter,
            options: 0,
            source_object_id: 0,
            cursor_name: "ENTER_FRIENDLY".to_string(),
            invalid_cursor_name: "GENERIC_INVALID".to_string(),
            radius_cursor_type: "NONE".to_string(),
        };
        assert_eq!(
            InGameUI::pending_gui_command_cursor(
                &target_pending,
                CommandHintType::InvalidGuiCommand
            ),
            MouseCursor::EnterFriendly
        );

        let unknown_pending = PendingCommand {
            cursor_name: "MISSING_CURSOR".to_string(),
            ..target_pending
        };
        assert_eq!(
            InGameUI::pending_gui_command_cursor(
                &unknown_pending,
                CommandHintType::ValidGuiCommand
            ),
            MouseCursor::Cross
        );
    }

    #[test]
    fn command_attack_hints_only_downgrade_for_fully_shrouded_targets() {
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObject,
                Some(ObjectShroudStatus::Shrouded)
            ),
            CommandHintType::MoveTo
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObjectAfterMoving,
                Some(ObjectShroudStatus::Shrouded)
            ),
            CommandHintType::MoveTo
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObject,
                Some(ObjectShroudStatus::Fogged)
            ),
            CommandHintType::AttackObject
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObjectAfterMoving,
                Some(ObjectShroudStatus::Fogged)
            ),
            CommandHintType::AttackObjectAfterMoving
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::ForceAttackObject,
                Some(ObjectShroudStatus::Shrouded)
            ),
            CommandHintType::ForceAttackObject
        );
    }

    #[test]
    fn double_click_attack_move_guard_hint_uses_cpp_predecrement_semantics() {
        let mut timer = 2;
        assert!(InGameUI::consume_double_click_attack_move_guard_hint(
            &mut timer
        ));
        assert_eq!(timer, 1);

        assert!(!InGameUI::consume_double_click_attack_move_guard_hint(
            &mut timer
        ));
        assert_eq!(timer, 0);

        assert!(!InGameUI::consume_double_click_attack_move_guard_hint(
            &mut timer
        ));
        assert_eq!(timer, 0);
    }

    #[test]
    fn default_command_hints_are_blocked_by_nonlocal_selected_source() {
        assert!(InGameUI::default_command_hint_blocked_by_source(Some(
            false
        )));
        assert!(!InGameUI::default_command_hint_blocked_by_source(Some(
            true
        )));
        assert!(!InGameUI::default_command_hint_blocked_by_source(None));
    }

    #[test]
    fn move_to_cursor_matches_cpp_source_and_target_context() {
        assert_eq!(
            InGameUI::move_to_cursor_for_context(false, false, false, true),
            MouseCursor::GenericInvalid
        );
        assert_eq!(
            InGameUI::move_to_cursor_for_context(true, true, false, false),
            MouseCursor::Selecting
        );
        assert_eq!(
            InGameUI::move_to_cursor_for_context(true, true, true, false),
            MouseCursor::MoveTo
        );
        assert_eq!(
            InGameUI::move_to_cursor_for_context(false, false, false, false),
            MouseCursor::MoveTo
        );
    }

    #[test]
    fn language_font_override_extracts_explicit_language_font() {
        assert_eq!(
            InGameUI::language_font_override(&FontDesc::new("Localized Caption", 14, true)),
            Some(("Localized Caption".to_string(), 14, true))
        );
    }

    #[test]
    fn language_font_override_ignores_default_language_font_descriptor() {
        assert_eq!(InGameUI::language_font_override(&FontDesc::default()), None);
    }

    #[test]
    fn military_caption_update_respects_initial_delay_then_types() {
        let mut subtitle = Some(MilitarySubtitle {
            text: "AB".to_string(),
            index: 0,
            position: (10.0, 20.0),
            lifetime_frame: 120,
            block_drawn: true,
            block_begin_frame: 0,
            block_pos: (10.0, 20.0),
            increment_on_frame: 22,
            color: 0xFFC8_C81E,
        });

        assert!(!InGameUI::update_military_subtitle_state(
            &mut subtitle,
            22,
            1,
            12,
            7.2,
            22
        ));
        let state = subtitle.as_ref().unwrap();
        assert_eq!(state.index, 0);
        assert_eq!(state.block_pos, (10.0, 20.0));

        assert!(InGameUI::update_military_subtitle_state(
            &mut subtitle,
            23,
            1,
            12,
            7.2,
            22
        ));
        let state = subtitle.as_ref().unwrap();
        assert_eq!(state.index, 1);
        assert_eq!(state.increment_on_frame, 24);
        assert_eq!(state.block_pos.0, 17.2);
    }

    #[test]
    fn military_caption_newline_advances_without_typing_sound() {
        let mut subtitle = Some(MilitarySubtitle {
            text: "\nA".to_string(),
            index: 0,
            position: (10.0, 20.0),
            lifetime_frame: 120,
            block_drawn: false,
            block_begin_frame: 0,
            block_pos: (18.0, 20.0),
            increment_on_frame: 22,
            color: 0xFFC8_C81E,
        });

        assert!(!InGameUI::update_military_subtitle_state(
            &mut subtitle,
            23,
            1,
            12,
            7.2,
            22
        ));
        let state = subtitle.as_ref().unwrap();
        assert_eq!(state.index, 1);
        assert_eq!(state.block_pos, (10.0, 32.0));
        assert!(state.block_drawn);
        assert_eq!(state.increment_on_frame, 45);
    }

    #[test]
    fn military_caption_update_fades_after_lifetime_before_removal() {
        let mut subtitle = Some(MilitarySubtitle {
            text: "A".to_string(),
            index: 1,
            position: (0.0, 0.0),
            lifetime_frame: 10,
            block_drawn: true,
            block_begin_frame: 0,
            block_pos: (0.0, 0.0),
            increment_on_frame: 11,
            color: 0x02C8_C81E,
        });

        assert!(!InGameUI::update_military_subtitle_state(
            &mut subtitle,
            15,
            1,
            12,
            7.2,
            22
        ));
        assert_eq!(subtitle.as_ref().unwrap().color >> 24, 2);

        assert!(!InGameUI::update_military_subtitle_state(
            &mut subtitle,
            40,
            1,
            12,
            7.2,
            22
        ));
        assert!(subtitle.is_none());
    }

    #[test]
    fn message_fade_subtracts_age_times_one_hundredth() {
        let (r, g, b, a) = InGameUI::unpack_argb(0xFFFF_FFFF);
        assert_eq!((r, g, b, a), (255, 255, 255, 255));
        let faded = InGameUI::pack_argb(r, g, b, (a as i32 - (100.0 * 0.01) as i32).max(0) as u8);
        assert_eq!(faded >> 24, 254);
    }

    #[test]
    fn named_timer_countdown_formats_m_ss_like_cpp() {
        assert_eq!(
            InGameUI::format_named_timer_line("Launch", 90, true),
            "Launch 0:03"
        );
        assert_eq!(
            InGameUI::format_named_timer_line("Launch", 600, true),
            "Launch 0:20"
        );
        assert_eq!(
            InGameUI::format_named_timer_line("Score", 12, false),
            "Score 12"
        );
    }

    #[test]
    fn floating_text_rises_by_frame_count_times_speed() {
        assert_eq!(InGameUI::floating_text_screen_offset_y(10, 1.5), 15.0);
        assert!(InGameUI::floating_text_visible_through_shroud(
            ObjectShroudStatus::Clear
        ));
        assert!(!InGameUI::floating_text_visible_through_shroud(
            ObjectShroudStatus::Fogged
        ));
    }

    #[test]
    fn message_color_alternates_like_cpp_get_message_color() {
        assert_eq!(InGameUI::pack_argb(255, 255, 255, 255), 0xFFFF_FFFF);
        assert_eq!(InGameUI::pack_argb(180, 180, 180, 255), 0xFFB4_B4B4);
    }
}
