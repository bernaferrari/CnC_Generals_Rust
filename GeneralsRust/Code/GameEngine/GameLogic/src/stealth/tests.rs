//! Comprehensive integration tests for the stealth detection system

#[cfg(test)]
mod integration_tests {
    use crate::common::*;
    use crate::object::{registry::OBJECT_REGISTRY, Object};
    use crate::stealth::*;
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_complete_stealth_workflow() {
        // Create a stealth system
        let system = StealthSystem::new();
        let visibility_manager = system.get_visibility_manager();

        // Register objects
        let stealth_unit_id = 1;
        let detector_unit_id = 2;
        let player_id = 100;

        visibility_manager.register_object(stealth_unit_id, true);
        visibility_manager.register_object(detector_unit_id, true);

        // Unit enters stealth
        visibility_manager.hide_from_all(stealth_unit_id);
        assert!(!visibility_manager.is_visible(stealth_unit_id, player_id));

        // Detector detects the unit
        visibility_manager.add_detector(stealth_unit_id, player_id, detector_unit_id, 0);
        assert!(visibility_manager.is_visible(stealth_unit_id, player_id));

        // Detector leaves range
        visibility_manager.remove_detector(stealth_unit_id, player_id, detector_unit_id);
        assert!(!visibility_manager.is_visible(stealth_unit_id, player_id));

        // Unit exits stealth
        visibility_manager.show_to_all(stealth_unit_id, 100);
        assert!(visibility_manager.is_visible(stealth_unit_id, player_id));
    }

    #[test]
    fn test_multi_detector_tracking() {
        let visibility_manager = Arc::new(VisibilityManager::new());
        let target_id = 1;
        let player_id = 100;
        let detector1 = 10;
        let detector2 = 11;

        visibility_manager.register_object(target_id, false);

        // First detector
        visibility_manager.add_detector(target_id, player_id, detector1, 0);
        assert!(visibility_manager.is_visible(target_id, player_id));

        // Second detector
        visibility_manager.add_detector(target_id, player_id, detector2, 0);
        assert!(visibility_manager.is_visible(target_id, player_id));

        // Remove first detector - still visible due to second
        visibility_manager.remove_detector(target_id, player_id, detector1);
        assert!(visibility_manager.is_visible(target_id, player_id));

        // Remove second detector - now hidden
        visibility_manager.remove_detector(target_id, player_id, detector2);
        assert!(!visibility_manager.is_visible(target_id, player_id));
    }

    #[test]
    fn test_stealth_state_machine() {
        let mut state_manager = StealthStateManager::new(1, 30);

        // Initial state
        assert_eq!(
            state_manager.get_visibility_state(),
            VisibilityState::Visible
        );
        assert!(!state_manager.is_stealthed());

        // Enable and try to stealth
        state_manager.set_stealth_enabled(true);
        assert!(state_manager.try_enable_stealth(0));
        assert_eq!(
            state_manager.get_visibility_state(),
            VisibilityState::Stealthing
        );

        // Wait for transition
        state_manager.update(30);
        assert_eq!(
            state_manager.get_visibility_state(),
            VisibilityState::Hidden
        );
        assert!(state_manager.is_stealthed());

        // Add detector
        state_manager.add_detector(2, 30);
        assert_eq!(
            state_manager.get_visibility_state(),
            VisibilityState::Detected
        );
        assert!(state_manager.is_detected());

        // Remove detector
        state_manager.remove_detector(2);
        assert_eq!(
            state_manager.get_visibility_state(),
            VisibilityState::Hidden
        );
        assert!(!state_manager.is_detected());
    }

    #[test]
    fn test_stealth_break_conditions() {
        let mut state_manager = StealthStateManager::new(1, 30);
        state_manager.set_stealth_enabled(true);
        state_manager.set_forbidden_conditions(true, true, true);

        // Stealth
        assert!(state_manager.try_enable_stealth(0));
        state_manager.update(30);
        assert!(state_manager.is_stealthed());

        // Attack breaks stealth
        state_manager.on_attack(40);
        assert_eq!(
            state_manager.get_visibility_state(),
            VisibilityState::Unstealthing
        );

        // Wait for transition
        state_manager.update(55);
        assert_eq!(
            state_manager.get_visibility_state(),
            VisibilityState::Visible
        );

        // Can't re-stealth immediately (cooldown)
        assert!(!state_manager.try_enable_stealth(60));
        assert!(!state_manager.can_stealth(60));

        // After cooldown
        for frame in 61..100 {
            state_manager.update(frame);
        }
        assert!(state_manager.can_stealth(100));
    }

    #[test]
    fn test_detection_levels() {
        assert_eq!(DetectionLevel::None.get_range(), 0.0);
        assert_eq!(DetectionLevel::Basic.get_range(), 100.0);
        assert_eq!(DetectionLevel::Advanced.get_range(), 200.0);
        assert_eq!(DetectionLevel::Superior.get_range(), 300.0);
        assert!(DetectionLevel::Total.get_range() > 1000.0);
    }

    #[test]
    fn test_stealth_difficulty() {
        let easy = StealthDifficulty::Easy;
        let hard = StealthDifficulty::VeryHard;

        assert!(easy.get_detection_modifier() > hard.get_detection_modifier());
        assert_eq!(easy.get_detection_modifier(), 1.0);
        assert_eq!(hard.get_detection_modifier(), 0.25);
    }

    #[test]
    fn test_visual_effects() {
        let vfx = StealthVisualEffects::new();

        // Test different scenarios
        let visible_opacity = vfx.calculate_opacity(false, false, false, false, 0.0);
        assert_eq!(visible_opacity, 1.0);

        let friendly_stealth = vfx.calculate_opacity(true, false, true, false, 0.0);
        assert_eq!(friendly_stealth, 0.5);

        let enemy_hidden = vfx.calculate_opacity(true, false, false, false, 0.0);
        assert!(enemy_hidden < 0.2);

        let enemy_detected = vfx.calculate_opacity(true, true, false, false, 0.0);
        assert!(enemy_detected > 0.2 && enemy_detected < 0.7);
    }

    #[test]
    fn test_team_visibility() {
        let visibility_manager = Arc::new(VisibilityManager::new());
        let object_id = 1;
        let team_players = vec![10, 11, 12];

        visibility_manager.register_object(object_id, false);
        visibility_manager.set_visible_to_team(object_id, &team_players, 0);

        for &player_id in &team_players {
            assert!(visibility_manager.is_visible(object_id, player_id));
        }

        assert!(!visibility_manager.is_visible(object_id, 99));
    }

    #[test]
    fn test_per_player_isolation() {
        let visibility_manager = Arc::new(VisibilityManager::new());
        let object_id = 1;
        let player1 = 10;
        let player2 = 20;

        visibility_manager.register_object(object_id, false);

        // Show to player 1 only
        visibility_manager.set_visible(object_id, player1, true, 0);

        assert!(visibility_manager.is_visible(object_id, player1));
        assert!(!visibility_manager.is_visible(object_id, player2));

        // Show to player 2 as well
        visibility_manager.set_visible(object_id, player2, true, 0);

        assert!(visibility_manager.is_visible(object_id, player1));
        assert!(visibility_manager.is_visible(object_id, player2));
    }

    #[test]
    fn test_stealth_opacity_transitions() {
        let mut state_manager = StealthStateManager::new(1, 30);

        // Visible
        assert_eq!(state_manager.get_opacity(), 1.0);

        // Stealthing
        state_manager.force_set_visibility_state_for_testing(VisibilityState::Stealthing);
        assert_eq!(state_manager.get_opacity(), 0.5);

        // Hidden
        state_manager.force_set_visibility_state_for_testing(VisibilityState::Hidden);
        assert_eq!(state_manager.get_opacity(), 0.2);

        // Detected
        state_manager.force_set_visibility_state_for_testing(VisibilityState::Detected);
        assert_eq!(state_manager.get_opacity(), 0.6);

        // Unstealthing
        state_manager.force_set_visibility_state_for_testing(VisibilityState::Unstealthing);
        assert_eq!(state_manager.get_opacity(), 0.7);
    }

    #[test]
    fn test_detector_scan_timing() {
        let mut data = StealthDetectorUpdateModuleData::default();
        data.set_detection_range(200.0);
        data.set_scan_interval_frames(10);
        let data = Arc::new(data);

        let mut detector = StealthDetectorController::new(data, 1);
        assert!(detector.is_active());

        // Set cooldown
        detector.set_scan_cooldown_frames_for_testing(10);

        // Scan should decrement cooldown but not perform detection
        detector.scan_for_stealth(0);
        assert_eq!(detector.scan_cooldown_frames(), 9);

        // Keep scanning until cooldown expires
        for _ in 0..9 {
            detector.scan_for_stealth(0);
        }

        assert_eq!(detector.scan_cooldown_frames(), 0);
    }

    #[test]
    fn test_stealth_upgrade_types() {
        assert_eq!(StealthUpgradeType::GrantStealth as u32, 0);
        assert_eq!(StealthUpgradeType::ImproveConcealment as u32, 1);
        assert_eq!(StealthUpgradeType::ImproveDetection as u32, 2);
    }

    #[test]
    fn test_break_conditions_default() {
        let conditions = StealthBreakConditions::default();
        assert!(!conditions.while_moving);
        assert!(conditions.while_attacking);
        assert!(!conditions.while_damaged);
        assert!(!conditions.requires_power);
        assert!(!conditions.requires_not_garrisoned);
    }
}
