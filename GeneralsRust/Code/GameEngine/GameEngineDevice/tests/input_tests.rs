//! Integration tests for input device system

#[cfg(feature = "input")]
mod input_tests {
    use game_engine_device::input::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_input_manager_creation() {
        let manager = InputManager::new().await;
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        let config = manager.config();

        assert!(config.keyboard_enabled);
        assert!(config.mouse_enabled);
        assert!(config.gamepad_enabled);
    }

    #[tokio::test]
    async fn test_custom_input_config() {
        let config = InputConfig {
            keyboard_enabled: true,
            mouse_enabled: false,
            gamepad_enabled: false,
            mouse_sensitivity: 2.0,
            raw_mouse_input: false,
            key_repeat_delay_ms: 1000,
            key_repeat_rate_ms: 50,
            gamepad_dead_zone: 0.2,
            recording_enabled: false,
            max_queue_size: 512,
        };

        let manager = InputManager::with_config(config).await;
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        let retrieved_config = manager.config();

        assert_eq!(retrieved_config.mouse_sensitivity, 2.0);
        assert_eq!(retrieved_config.key_repeat_delay_ms, 1000);
        assert_eq!(retrieved_config.gamepad_dead_zone, 0.2);
    }

    #[tokio::test]
    async fn test_keyboard_state() {
        let manager = InputManager::new().await.unwrap();
        let keyboard_state = manager.keyboard_state();

        assert!(!keyboard_state.any_key_pressed());
        assert_eq!(keyboard_state.modifiers(), ModifierKeys::empty());
    }

    #[tokio::test]
    async fn test_mouse_state() {
        let manager = InputManager::new().await.unwrap();
        let mouse_state = manager.mouse_state();

        assert_eq!(mouse_state.position(), (0, 0));
        assert!(!mouse_state.any_button_pressed());
        assert!(mouse_state.is_cursor_in_window());
    }

    #[tokio::test]
    async fn test_hotkey_registration() {
        let manager = InputManager::new().await.unwrap();

        let hotkey = Hotkey::new(KeyCode::S).ctrl();
        let result = manager.register_hotkey("save", hotkey);
        assert!(result.is_ok());

        // Test unregistration
        let result = manager.unregister_hotkey("save");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_hotkey_builder() {
        let hotkey = Hotkey::new(KeyCode::C).ctrl().shift();

        assert_eq!(hotkey.key, KeyCode::C);
        assert!(hotkey.modifiers.contains(ModifierKeys::CTRL));
        assert!(hotkey.modifiers.contains(ModifierKeys::SHIFT));

        let display = hotkey.display_string();
        assert!(display.contains("Ctrl"));
        assert!(display.contains("Shift"));
        assert!(display.contains("C"));
    }

    #[tokio::test]
    async fn test_hotkey_from_string() {
        let hotkey = Hotkey::from_string("Ctrl+Shift+A");
        assert!(hotkey.is_some());

        let hotkey = hotkey.unwrap();
        assert_eq!(hotkey.key, KeyCode::A);
        assert!(hotkey.modifiers.contains(ModifierKeys::CTRL));
        assert!(hotkey.modifiers.contains(ModifierKeys::SHIFT));
    }

    #[tokio::test]
    async fn test_action_binding() {
        let manager = InputManager::new().await.unwrap();

        let binding = InputBinding::key(KeyCode::W);
        let result = manager.bind_action("forward", binding);
        assert!(result.is_ok());

        let result = manager.unbind_action("forward");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_binding_config_creation() {
        let config = BindingConfig::new("Test Config");
        assert_eq!(config.name, "Test Config");
        assert!(config.actions.is_empty());
    }

    #[tokio::test]
    async fn test_default_rts_bindings() {
        let config = BindingConfig::default_rts();

        assert!(!config.actions.is_empty());
        assert!(config.actions.contains_key("move"));
        assert!(config.actions.contains_key("select"));
        assert!(config.actions.contains_key("camera_up"));
    }

    #[tokio::test]
    async fn test_input_recording() {
        let manager = InputManager::new().await.unwrap();

        let result = manager.start_recording();
        assert!(result.is_ok());

        let result = manager.stop_recording();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_input_recorder() {
        let mut recorder = InputRecorder::new();

        assert!(!recorder.is_recording());

        recorder.start();
        assert!(recorder.is_recording());

        let event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: ModifierKeys::empty(),
            timestamp: Duration::from_millis(100),
        };

        recorder.record_event(&event);

        recorder.stop();
        assert!(!recorder.is_recording());

        let recording = recorder.get_recording();
        assert!(recording.is_some());
    }

    #[tokio::test]
    async fn test_input_playback() {
        let mut recorder = InputRecorder::new();

        // Create a simple recording
        let mut recording = InputRecording::new("Test");
        recording.frames.push(InputFrame {
            timestamp: Duration::from_millis(100),
            events: vec![InputEvent::KeyPressed {
                key: KeyCode::A,
                modifiers: ModifierKeys::empty(),
                timestamp: Duration::from_millis(100),
            }],
        });

        recording.compress();
        assert_eq!(recording.event_count(), 1);

        recorder.recording = Some(recording);

        let result = recorder.start_playback(PlaybackMode::Once);
        assert!(result.is_ok());
        assert!(recorder.is_playing());

        recorder.stop_playback();
        assert!(!recorder.is_playing());
    }

    #[tokio::test]
    async fn test_input_state_tracker() {
        let mut tracker = InputStateTracker::new();

        assert_eq!(tracker.frame(), 0);

        tracker.next_frame();
        assert_eq!(tracker.frame(), 1);

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.frame, 1);
    }

    #[tokio::test]
    async fn test_gamepad_creation() {
        let gamepad = GamepadDevice::new(GamepadId::new(0), "Test Gamepad".into(), 0.15);
        assert!(gamepad.is_ok());

        let gamepad = gamepad.unwrap();
        assert_eq!(gamepad.id(), GamepadId::new(0));
        assert_eq!(gamepad.name(), "Test Gamepad");
        assert!(gamepad.is_connected());
    }

    #[tokio::test]
    async fn test_gamepad_button_state() {
        let mut gamepad = GamepadDevice::new(GamepadId::new(0), "Test".into(), 0.15).unwrap();

        gamepad.handle_button_press(GamepadButton::South);
        assert!(gamepad.is_button_pressed(GamepadButton::South));

        gamepad.handle_button_release(GamepadButton::South);
        assert!(!gamepad.is_button_pressed(GamepadButton::South));
    }

    #[tokio::test]
    async fn test_gamepad_axis() {
        let mut gamepad = GamepadDevice::new(GamepadId::new(0), "Test".into(), 0.15).unwrap();

        gamepad.handle_axis(GamepadAxis::LeftStickX, 0.5);
        assert!(gamepad.axis(GamepadAxis::LeftStickX) > 0.0);

        // Test dead zone
        gamepad.handle_axis(GamepadAxis::LeftStickY, 0.1);
        assert_eq!(gamepad.axis(GamepadAxis::LeftStickY), 0.0);
    }

    #[tokio::test]
    async fn test_gamepad_dead_zone() {
        let mut gamepad = GamepadDevice::new(GamepadId::new(0), "Test".into(), 0.2).unwrap();

        // Below dead zone
        gamepad.handle_axis(GamepadAxis::LeftStickX, 0.15);
        assert_eq!(gamepad.axis(GamepadAxis::LeftStickX), 0.0);

        // Above dead zone
        gamepad.handle_axis(GamepadAxis::LeftStickX, 0.5);
        assert!(gamepad.axis(GamepadAxis::LeftStickX) > 0.0);
    }

    #[tokio::test]
    async fn test_gamepad_mapping() {
        let mut mapping = GamepadMapping::default_mapping();

        mapping.map_button(GamepadButton::South, GamepadButton::East);
        assert_eq!(
            mapping.apply_button(GamepadButton::South),
            GamepadButton::East
        );

        mapping.set_axis_inverted(GamepadAxis::LeftStickY, true);
        let (axis, value) = mapping.apply_axis(GamepadAxis::LeftStickY, 0.5);
        assert_eq!(axis, GamepadAxis::LeftStickY);
        assert_eq!(value, -0.5);
    }

    #[tokio::test]
    async fn test_mouse_device_creation() {
        let device = MouseDevice::new(1.0, true);
        assert!(device.is_ok());

        let device = device.unwrap();
        assert_eq!(device.sensitivity(), 1.0);
        assert!(device.is_raw_input());
    }

    #[tokio::test]
    async fn test_mouse_movement() {
        let mut device = MouseDevice::new(1.0, false).unwrap();

        device.handle_move(100, 200);
        assert_eq!(device.position(), (100, 200));

        device.handle_move(150, 250);
        assert_eq!(device.position(), (150, 250));
        assert_eq!(device.delta(), (50, 50));
    }

    #[tokio::test]
    async fn test_mouse_button() {
        let mut device = MouseDevice::new(1.0, false).unwrap();

        device.handle_button_press(MouseButton::Left);
        assert!(device.is_button_pressed(MouseButton::Left));

        device.handle_button_release(MouseButton::Left);
        assert!(!device.is_button_pressed(MouseButton::Left));
    }

    #[tokio::test]
    async fn test_mouse_sensitivity() {
        let mut device = MouseDevice::new(2.0, true).unwrap();

        device.handle_raw_move(10.0, 10.0);
        let pos = device.position();
        assert!(pos.0 >= 19 && pos.0 <= 21); // Allow for rounding
        assert!(pos.1 >= 19 && pos.1 <= 21);
    }

    #[tokio::test]
    async fn test_keyboard_device_creation() {
        let device = KeyboardDevice::new(500, 33);
        assert!(device.is_ok());
    }

    #[tokio::test]
    async fn test_keyboard_key_press() {
        let mut device = KeyboardDevice::new(500, 33).unwrap();

        device.handle_key_press(KeyCode::A);
        assert!(device.is_key_pressed(KeyCode::A));

        device.handle_key_release(KeyCode::A);
        assert!(!device.is_key_pressed(KeyCode::A));
    }

    #[tokio::test]
    async fn test_keyboard_modifiers() {
        let mut device = KeyboardDevice::new(500, 33).unwrap();

        device.handle_key_press(KeyCode::LeftCtrl);
        assert!(device.modifiers().contains(ModifierKeys::CTRL));

        device.handle_key_press(KeyCode::LeftShift);
        assert!(device.modifiers().contains(ModifierKeys::CTRL));
        assert!(device.modifiers().contains(ModifierKeys::SHIFT));

        device.handle_key_release(KeyCode::LeftCtrl);
        assert!(!device.modifiers().contains(ModifierKeys::CTRL));
        assert!(device.modifiers().contains(ModifierKeys::SHIFT));
    }

    #[tokio::test]
    async fn test_key_code_names() {
        assert_eq!(KeyCode::A.name(), "A");
        assert_eq!(KeyCode::Space.name(), "Space");
        assert_eq!(KeyCode::Enter.name(), "Enter");
        assert_eq!(KeyCode::Escape.name(), "Escape");
    }

    #[tokio::test]
    async fn test_key_code_from_name() {
        assert_eq!(KeyCode::from_name("A"), Some(KeyCode::A));
        assert_eq!(KeyCode::from_name("space"), Some(KeyCode::Space));
        assert_eq!(KeyCode::from_name("ENTER"), Some(KeyCode::Enter));
        assert_eq!(KeyCode::from_name("invalid"), None);
    }

    #[tokio::test]
    async fn test_modifier_keys_bitflags() {
        let mut modifiers = ModifierKeys::empty();
        assert!(modifiers.is_empty());

        modifiers.insert(ModifierKeys::CTRL);
        assert!(modifiers.contains(ModifierKeys::CTRL));
        assert!(!modifiers.contains(ModifierKeys::SHIFT));

        modifiers.insert(ModifierKeys::SHIFT);
        assert!(modifiers.contains(ModifierKeys::CTRL | ModifierKeys::SHIFT));

        modifiers.remove(ModifierKeys::CTRL);
        assert!(!modifiers.contains(ModifierKeys::CTRL));
        assert!(modifiers.contains(ModifierKeys::SHIFT));
    }

    #[tokio::test]
    async fn test_event_serialization() {
        let event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: ModifierKeys::CTRL,
            timestamp: Duration::from_millis(100),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: InputEvent = serde_json::from_str(&json).unwrap();

        match deserialized {
            InputEvent::KeyPressed { key, modifiers, .. } => {
                assert_eq!(key, KeyCode::A);
                assert_eq!(modifiers, ModifierKeys::CTRL);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_input_binding_display() {
        let binding = InputBinding::key_with_modifiers(KeyCode::S, ModifierKeys::CTRL);
        let display = binding.display_string();

        assert!(display.contains("Ctrl"));
        assert!(display.contains("S"));
    }

    #[tokio::test]
    async fn test_update_loop() {
        let mut manager = InputManager::new().await.unwrap();

        for _ in 0..10 {
            let result = manager.update(Duration::from_millis(16)).await;
            assert!(result.is_ok());
        }
    }
}
