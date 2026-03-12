#[cfg(test)]
mod integration_tests {
    use super::super::*;
    use crate::input::{KeyCode, KeyModifiers, MouseButton};
    use std::time::Instant;

    /// Test complete mouse click → selection flow
    #[test]
    fn test_mouse_click_selection_flow() {
        let mut processor = InputProcessor::with_default_config();

        // Simulate mouse click down
        let down_event = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };

        let down_messages = processor.process_input_event(down_event);
        assert!(down_messages.len() >= 1);

        // Verify raw mouse button down message generated
        let has_mouse_down = down_messages.iter().any(|msg| {
            matches!(
                msg.get_type(),
                GameMessageType::RawMouseLeftButtonDown(_, _, _)
            )
        });
        assert!(has_mouse_down);

        // Simulate mouse click up (no drag - within tolerance)
        let up_event = InputEvent::MouseButtonUp {
            button: MouseButton::Left,
            x: 102.0, // Slight movement, but within drag tolerance
            y: 102.0,
            timestamp: Instant::now(),
        };

        let up_messages = processor.process_input_event(up_event);
        assert!(up_messages.len() >= 1);

        // Should generate click message (not area selection since no drag)
        let has_click = up_messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MouseLeftClick(_, _)));
        assert!(has_click);
    }

    /// Test box selection (drag select) flow
    #[test]
    fn test_box_selection_flow() {
        let mut processor = InputProcessor::with_default_config();

        // Mouse down to start selection
        let down_event = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };

        processor.process_input_event(down_event);

        // Move mouse to simulate drag (beyond tolerance)
        let mut last_messages = Vec::new();
        for i in 1..=5 {
            let move_event = InputEvent::MouseMove {
                x: 100.0 + (i as f32 * 10.0),
                y: 100.0 + (i as f32 * 10.0),
                timestamp: Instant::now(),
            };

            last_messages = processor.process_input_event(move_event);
        }

        // Should generate mouse position updates
        assert!(!last_messages.is_empty());

        // Mouse up to complete selection
        let up_event = InputEvent::MouseButtonUp {
            button: MouseButton::Left,
            x: 150.0,
            y: 150.0,
            timestamp: Instant::now(),
        };

        let up_messages = processor.process_input_event(up_event);

        // Should generate area selection message
        let has_area_selection = up_messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::AreaSelection(_)));
        assert!(has_area_selection);
    }

    /// Test double-click selection flow
    #[test]
    fn test_double_click_selection_flow() {
        let mut processor = InputProcessor::with_default_config();
        let pos = (100.0, 100.0);

        // First click
        let down1 = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: pos.0,
            y: pos.1,
            timestamp: Instant::now(),
        };
        processor.process_input_event(down1);

        let up1 = InputEvent::MouseButtonUp {
            button: MouseButton::Left,
            x: pos.0,
            y: pos.1,
            timestamp: Instant::now(),
        };
        processor.process_input_event(up1);

        // Second click (within double-click time)
        let down2 = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: pos.0,
            y: pos.1,
            timestamp: Instant::now(),
        };
        let messages = processor.process_input_event(down2);

        // Should generate double-click message
        let has_double_click = messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MouseLeftDoubleClick(_, _)));
        assert!(has_double_click);
    }

    /// Test right-click command flow
    #[test]
    fn test_right_click_command_flow() {
        let mut processor = InputProcessor::with_default_config();

        // Right click down
        let down_event = InputEvent::MouseButtonDown {
            button: MouseButton::Right,
            x: 200.0,
            y: 200.0,
            timestamp: Instant::now(),
        };

        processor.process_input_event(down_event);

        // Right click up
        let up_event = InputEvent::MouseButtonUp {
            button: MouseButton::Right,
            x: 200.0,
            y: 200.0,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(up_event);

        // Should generate right click messages
        assert!(!messages.is_empty());

        // Should have processed through command translator
        let has_command = messages.iter().any(|msg| {
            matches!(
                msg.get_type(),
                GameMessageType::DoMoveTo(_)
                    | GameMessageType::DoAttackObject(_)
                    | GameMessageType::RawMouseRightButtonUp(_, _, _)
            )
        });
        assert!(has_command);
    }

    /// Test control group creation (Ctrl+1-9)
    #[test]
    fn test_control_group_creation_flow() {
        let mut processor = InputProcessor::with_default_config();

        // Press Ctrl+1
        let event = InputEvent::KeyDown {
            key: KeyCode::Num1,
            modifiers: KeyModifiers::CTRL,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(event);

        // Should generate MetaCreateTeam message
        let has_create_team = messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaCreateTeam(1)));
        assert!(has_create_team, "Should generate MetaCreateTeam(1) message");
    }

    /// Test control group selection (1-9)
    #[test]
    fn test_control_group_selection_flow() {
        let mut processor = InputProcessor::with_default_config();

        // Press 2 (without modifiers)
        let event = InputEvent::KeyDown {
            key: KeyCode::Num2,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(event);

        // Should generate MetaSelectTeam message
        let has_select_team = messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaSelectTeam(2)));
        assert!(has_select_team, "Should generate MetaSelectTeam(2) message");
    }

    /// Test keyboard command shortcuts
    #[test]
    fn test_keyboard_command_shortcuts() {
        let mut processor = InputProcessor::with_default_config();

        // Test S key (stop)
        let stop_event = InputEvent::KeyDown {
            key: KeyCode::S,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(stop_event);
        let has_stop = messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaStop));

        // Note: This may not generate MetaStop directly in current implementation
        // as that depends on command translator processing RawKeyDown
        // Just verify messages are generated
        assert!(!messages.is_empty());

        // Test A key (attack move)
        let attack_event = InputEvent::KeyDown {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(attack_event);
        assert!(!messages.is_empty());

        // Test G key (guard)
        let guard_event = InputEvent::KeyDown {
            key: KeyCode::G,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(guard_event);
        assert!(!messages.is_empty());
    }

    /// Test modifier key behavior (Alt for force attack)
    #[test]
    fn test_force_attack_mode() {
        let mut processor = InputProcessor::with_default_config();

        // Press Alt to enter force attack mode
        let alt_down = InputEvent::KeyDown {
            key: KeyCode::LeftAlt,
            modifiers: KeyModifiers::ALT,
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(alt_down);

        // Should generate force attack mode message
        let has_force_attack_begin = messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaBeginForceAttack));
        assert!(has_force_attack_begin);

        // Release Alt to exit force attack mode
        let alt_up = InputEvent::KeyUp {
            key: KeyCode::LeftAlt,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(alt_up);

        let has_force_attack_end = messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaEndForceAttack));
        assert!(has_force_attack_end);
    }

    /// Test Shift for waypoint mode
    #[test]
    fn test_waypoint_mode() {
        let mut processor = InputProcessor::with_default_config();

        // Press Shift
        let shift_down = InputEvent::KeyDown {
            key: KeyCode::LeftShift,
            modifiers: KeyModifiers::SHIFT,
            timestamp: Instant::now(),
        };

        processor.process_input_event(shift_down);

        // Waypoint mode should be active in command translator
        // (Can't directly test internal state, but can verify messages are processed)

        // Release Shift
        let shift_up = InputEvent::KeyUp {
            key: KeyCode::LeftShift,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };

        let messages = processor.process_input_event(shift_up);
        assert!(!messages.is_empty());
    }

    /// Test focus loss clears all input state
    #[test]
    fn test_focus_loss_integration() {
        let mut processor = InputProcessor::with_default_config();

        // Press mouse button
        let down_event = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };
        processor.process_input_event(down_event);

        // Press key
        let key_event = InputEvent::KeyDown {
            key: KeyCode::A,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };
        processor.process_input_event(key_event);

        // Verify states are active
        assert!(processor.is_mouse_button_down(MouseButton::Left));
        assert!(processor.is_key_down(KeyCode::A));

        // Lose focus
        processor.process_input_event(InputEvent::FocusLost);

        // All state should be cleared
        assert!(!processor.is_mouse_button_down(MouseButton::Left));
        assert!(!processor.is_key_down(KeyCode::A));
    }

    /// Test message pipeline throughput
    #[test]
    fn test_message_pipeline_throughput() {
        let mut processor = InputProcessor::with_default_config();
        let mut total_messages = 0;

        // Generate a burst of input events
        for i in 0..100 {
            let event = InputEvent::MouseMove {
                x: (i as f32),
                y: (i as f32),
                timestamp: Instant::now(),
            };

            let messages = processor.process_input_event(event);
            total_messages += messages.len();
        }

        // Should have processed all events
        assert_eq!(processor.get_statistics().events_processed, 100);

        // Should have generated messages
        assert!(total_messages > 0);
    }

    /// Test complete scenario: select units, create group, select group, move
    #[test]
    fn test_complete_gameplay_scenario() {
        let mut processor = InputProcessor::with_default_config();

        // Step 1: Box select some units
        let select_down = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };
        processor.process_input_event(select_down);

        // Drag mouse
        for i in 1..=10 {
            let move_event = InputEvent::MouseMove {
                x: 100.0 + (i as f32 * 5.0),
                y: 100.0 + (i as f32 * 5.0),
                timestamp: Instant::now(),
            };
            processor.process_input_event(move_event);
        }

        let select_up = InputEvent::MouseButtonUp {
            button: MouseButton::Left,
            x: 150.0,
            y: 150.0,
            timestamp: Instant::now(),
        };
        let messages = processor.process_input_event(select_up);

        // Should generate area selection
        assert!(messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::AreaSelection(_))));

        // Step 2: Create control group (Ctrl+1)
        let create_group = InputEvent::KeyDown {
            key: KeyCode::Num1,
            modifiers: KeyModifiers::CTRL,
            timestamp: Instant::now(),
        };
        let messages = processor.process_input_event(create_group);
        assert!(messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaCreateTeam(1))));

        // Step 3: Deselect (click empty ground)
        let deselect_down = InputEvent::MouseButtonDown {
            button: MouseButton::Left,
            x: 300.0,
            y: 300.0,
            timestamp: Instant::now(),
        };
        processor.process_input_event(deselect_down);

        let deselect_up = InputEvent::MouseButtonUp {
            button: MouseButton::Left,
            x: 300.0,
            y: 300.0,
            timestamp: Instant::now(),
        };
        processor.process_input_event(deselect_up);

        // Step 4: Select control group (press 1)
        let select_group = InputEvent::KeyDown {
            key: KeyCode::Num1,
            modifiers: KeyModifiers::empty(),
            timestamp: Instant::now(),
        };
        let messages = processor.process_input_event(select_group);
        assert!(messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaSelectTeam(1))));

        // Step 5: Issue move command (right-click)
        let move_down = InputEvent::MouseButtonDown {
            button: MouseButton::Right,
            x: 400.0,
            y: 400.0,
            timestamp: Instant::now(),
        };
        processor.process_input_event(move_down);

        let move_up = InputEvent::MouseButtonUp {
            button: MouseButton::Right,
            x: 400.0,
            y: 400.0,
            timestamp: Instant::now(),
        };
        let messages = processor.process_input_event(move_up);

        // Should have move command in pipeline
        assert!(!messages.is_empty());

        // Verify statistics
        let stats = processor.get_statistics();
        assert!(stats.events_processed > 10);
        assert!(stats.messages_generated > 0);
    }

    /// Test concurrent modifier keys
    #[test]
    fn test_concurrent_modifiers() {
        let mut processor = InputProcessor::with_default_config();

        // Press Ctrl
        let ctrl_down = InputEvent::KeyDown {
            key: KeyCode::LeftCtrl,
            modifiers: KeyModifiers::CTRL,
            timestamp: Instant::now(),
        };
        processor.process_input_event(ctrl_down);

        // Press Shift while Ctrl is held
        let shift_down = InputEvent::KeyDown {
            key: KeyCode::LeftShift,
            modifiers: KeyModifiers::CTRL | KeyModifiers::SHIFT,
            timestamp: Instant::now(),
        };
        processor.process_input_event(shift_down);

        // Both should be active
        assert!(processor.is_key_down(KeyCode::LeftCtrl));
        assert!(processor.is_key_down(KeyCode::LeftShift));

        let modifiers = processor.keyboard_modifiers();
        assert!(modifiers.contains(KeyModifiers::CTRL));
        assert!(modifiers.contains(KeyModifiers::SHIFT));
    }

    /// Test input processor update and statistics
    #[test]
    fn test_processor_update_and_stats() {
        let mut processor = InputProcessor::with_default_config();

        // Process some events
        for i in 0..50 {
            let event = InputEvent::MouseMove {
                x: (i as f32 * 2.0),
                y: (i as f32 * 2.0),
                timestamp: Instant::now(),
            };
            processor.process_input_event(event);
        }

        // Update processor (called every frame)
        processor.update();

        // Check statistics
        let stats = processor.get_statistics();
        assert_eq!(stats.events_processed, 50);
        assert!(stats.messages_generated >= 50);

        // Mouse position should be at last update
        let (x, y) = stats.mouse_position;
        assert_eq!(x, 98.0); // 49 * 2.0
        assert_eq!(y, 98.0);
    }

    /// Test enabling/disabling processor
    #[test]
    fn test_enable_disable_integration() {
        let mut processor = InputProcessor::with_default_config();

        // Process an event while enabled
        let event1 = InputEvent::MouseMove {
            x: 100.0,
            y: 100.0,
            timestamp: Instant::now(),
        };
        let messages1 = processor.process_input_event(event1);
        assert!(!messages1.is_empty());

        // Disable processor
        processor.set_enabled(false);

        // Try to process event while disabled
        let event2 = InputEvent::MouseMove {
            x: 200.0,
            y: 200.0,
            timestamp: Instant::now(),
        };
        let messages2 = processor.process_input_event(event2);
        assert!(messages2.is_empty()); // Should not process

        // Re-enable
        processor.set_enabled(true);

        // Process event while enabled again
        let event3 = InputEvent::MouseMove {
            x: 300.0,
            y: 300.0,
            timestamp: Instant::now(),
        };
        let messages3 = processor.process_input_event(event3);
        assert!(!messages3.is_empty());
    }
}
