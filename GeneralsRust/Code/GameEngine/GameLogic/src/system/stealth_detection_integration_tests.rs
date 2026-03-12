//! Integration tests for Stealth and Detection systems
//!
//! Tests the interaction between:
//! - StealthManager: Manages stealth state for objects
//! - DetectionManager: Manages detection capabilities
//! - ShroudManager: Visibility system that integrates stealth/detection

#[cfg(test)]
mod stealth_detection_integration_tests {
    use crate::common::ObjectID;
    use crate::system::detection_manager::{
        DetectionManager, DetectionModifier, DetectionStrength,
    };
    use crate::system::stealth_manager::{StealthManager, StealthStatus, StealthStrength};

    /// Test basic stealth visibility: object invisible vs detection strength
    #[test]
    fn test_stealth_vs_detection_basic() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        // Setup: Create stealthed object
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();

        // Setup: Create detector unit
        detection.register_object(2).unwrap();
        detection
            .set_detection_strength(2, DetectionStrength::standard_detector())
            .unwrap();

        // Stealth: 60.0, Detection: 60.0
        // Result: Detection should NOT detect (needs > stealth)
        let can_detect = detection
            .can_detect_stealth(2, 60.0, DetectionModifier::default())
            .unwrap();
        assert!(!can_detect, "Equal strength should not detect");

        // Stealth: 60.0, Detection: 70.0
        // Result: Detection SHOULD detect (70 > 60)
        let can_detect = detection
            .can_detect_stealth(2, 55.0, DetectionModifier::default())
            .unwrap();
        assert!(
            can_detect,
            "Superior detection should detect inferior stealth"
        );
    }

    /// Test stealth strength levels vs detection
    #[test]
    fn test_stealth_levels_vs_detection() {
        let stealth_levels = vec![
            (StealthStrength::none(), "none", 0.0),
            (StealthStrength::weak_stealth(), "weak", 30.0),
            (StealthStrength::standard_cloak(), "standard", 60.0),
            (StealthStrength::strong_stealth(), "strong", 90.0),
        ];

        let detection_levels = vec![
            (DetectionStrength::none(), "none", 0.0),
            (DetectionStrength::weak_detector(), "weak", 30.0),
            (DetectionStrength::standard_detector(), "standard", 60.0),
            (DetectionStrength::strong_detector(), "strong", 90.0),
        ];

        let mut detection = DetectionManager::new();
        detection.register_object(1).unwrap();

        for (det_strength, det_name, det_value) in &detection_levels {
            detection.set_detection_strength(1, *det_strength).unwrap();

            for (_, stealth_name, stealth_value) in &stealth_levels {
                let can_detect = detection
                    .can_detect_stealth(1, *stealth_value, DetectionModifier::default())
                    .unwrap();

                if det_value > stealth_value {
                    assert!(
                        can_detect,
                        "Detection {} ({:.1}) should detect stealth {} ({:.1})",
                        det_name, det_value, stealth_name, stealth_value
                    );
                } else {
                    assert!(
                        !can_detect,
                        "Detection {} ({:.1}) should NOT detect stealth {} ({:.1})",
                        det_name, det_value, stealth_name, stealth_value
                    );
                }
            }
        }
    }

    /// Test detection modifiers affect effectiveness
    #[test]
    fn test_detection_modifiers_reduce_effectiveness() {
        let mut detection = DetectionManager::new();
        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        // Base: 60.0 detection vs 50.0 stealth (should detect)
        assert!(detection
            .can_detect_stealth(1, 50.0, DetectionModifier::default())
            .unwrap());

        // With distance modifier 0.7: 60 * 0.7 = 42 vs 50 (should NOT detect)
        let distance_modifier = DetectionModifier {
            distance_factor: 0.7,
            ..Default::default()
        };
        assert!(!detection
            .can_detect_stealth(1, 50.0, distance_modifier)
            .unwrap());

        // With multiple modifiers: 60 * 0.5 * 0.5 = 15 vs 50 (should NOT detect)
        let weak_modifier = DetectionModifier {
            distance_factor: 0.5,
            unit_type_factor: 0.5,
            movement_factor: 1.0,
            special_factor: 1.0,
        };
        assert!(!detection
            .can_detect_stealth(1, 50.0, weak_modifier)
            .unwrap());
    }

    /// Test per-player stealth visibility: different players see different states
    #[test]
    fn test_per_player_stealth_visibility() {
        let mut stealth = StealthManager::new();
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();

        // Player 0: sees invisible (standard cloak active)
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert!(stealth.is_invisible_to_player(1, 0).unwrap());

        // Player 1: sees revealed (already detected)
        stealth
            .set_stealth_status(1, 1, StealthStatus::Revealed)
            .unwrap();
        assert!(!stealth.is_invisible_to_player(1, 1).unwrap());

        // Player 2: sees not stealthed
        stealth
            .set_stealth_status(1, 2, StealthStatus::Hidden)
            .unwrap();
        assert!(!stealth.is_invisible_to_player(1, 2).unwrap());

        // Verify independence
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );
        assert_eq!(
            stealth.get_stealth_status(1, 1).unwrap(),
            StealthStatus::Revealed
        );
        assert_eq!(
            stealth.get_stealth_status(1, 2).unwrap(),
            StealthStatus::Hidden
        );
    }

    /// Test stealth reveal mechanic
    #[test]
    fn test_stealth_reveal_mechanic() {
        let mut stealth = StealthManager::new();
        stealth.register_object(1).unwrap(); // Stealthed unit
        stealth.register_object(2).unwrap(); // Detector unit

        // Set unit 1 as stealthed
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert!(stealth.is_invisible_to_player(1, 0).unwrap());

        // Unit 2 reveals stealth at frame 100
        stealth.reveal_stealth(1, 0, 2, 100).unwrap();

        // Now player 0 sees revealed status
        assert!(!stealth.is_invisible_to_player(1, 0).unwrap());
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Revealed
        );
    }

    /// Test break stealth for all players
    #[test]
    fn test_break_stealth_for_all_players() {
        let mut stealth = StealthManager::new();
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();

        // Set invisible to all players
        for player in 0..8 {
            stealth
                .set_stealth_status(1, player, StealthStatus::Invisible)
                .unwrap();
        }

        // Break stealth for all
        stealth.break_stealth_all(1, 50).unwrap();

        // All players should see revealed
        for player in 0..8 {
            assert_eq!(
                stealth.get_stealth_status(1, player).unwrap(),
                StealthStatus::Revealed,
                "Player {} should see revealed after break_stealth_all",
                player
            );
        }
    }

    /// Test reset stealth functionality
    #[test]
    fn test_reset_stealth_to_invisible() {
        let mut stealth = StealthManager::new();
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();

        // Start as revealed
        for player in 0..4 {
            stealth
                .set_stealth_status(1, player, StealthStatus::Revealed)
                .unwrap();
        }

        // Reset to invisible
        stealth.reset_stealth_all(1).unwrap();

        // All should be invisible
        for player in 0..4 {
            assert!(stealth.is_invisible_to_player(1, player).unwrap());
        }
    }

    /// Test detection effectiveness calculation with modifiers
    #[test]
    fn test_detection_effectiveness_calculation() {
        let mut detection = DetectionManager::new();
        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::new(100.0))
            .unwrap();

        // Base effectiveness: 100.0
        let base = detection
            .get_detection_effectiveness(1, DetectionModifier::default())
            .unwrap();
        assert_eq!(base, 100.0);

        // With 0.5 distance factor: 100.0 * 0.5 = 50.0
        let distance_mod = DetectionModifier {
            distance_factor: 0.5,
            ..Default::default()
        };
        let modified = detection
            .get_detection_effectiveness(1, distance_mod)
            .unwrap();
        assert_eq!(modified, 50.0);

        // With all 0.5 factors: 100.0 * 0.5^3 = 12.5
        let weak_mod = DetectionModifier::new(0.5, 0.5, 0.5, 1.0);
        let weak_effectiveness = detection.get_detection_effectiveness(1, weak_mod).unwrap();
        assert_eq!(weak_effectiveness, 12.5);
    }

    /// Test detector discovery: get_detectors returns only objects with detection
    #[test]
    fn test_get_detectors_only_returns_detectors() {
        let mut detection = DetectionManager::new();

        // Register objects
        for i in 1..=10 {
            detection.register_object(i).unwrap();
        }

        // Set detection only on some
        detection
            .set_detection_strength(2, DetectionStrength::standard_detector())
            .unwrap();
        detection
            .set_detection_strength(5, DetectionStrength::weak_detector())
            .unwrap();
        detection
            .set_detection_strength(9, DetectionStrength::strong_detector())
            .unwrap();

        let detectors = detection.get_detectors();
        assert_eq!(detectors.len(), 3);
        assert!(detectors.contains(&2));
        assert!(detectors.contains(&5));
        assert!(detectors.contains(&9));
    }

    /// Test stealth strength clamping
    #[test]
    fn test_stealth_strength_clamping() {
        let negative = StealthStrength::new(-50.0);
        assert_eq!(negative.value(), 0.0, "Negative stealth should clamp to 0");

        let too_high = StealthStrength::new(200.0);
        assert_eq!(too_high.value(), 100.0, "High stealth should clamp to 100");

        let valid = StealthStrength::new(75.0);
        assert_eq!(valid.value(), 75.0, "Valid stealth should pass through");
    }

    /// Test detection strength clamping
    #[test]
    fn test_detection_strength_clamping() {
        let negative = DetectionStrength::new(-30.0);
        assert_eq!(
            negative.value(),
            0.0,
            "Negative detection should clamp to 0"
        );

        let too_high = DetectionStrength::new(150.0);
        assert_eq!(
            too_high.value(),
            100.0,
            "High detection should clamp to 100"
        );

        let valid = DetectionStrength::new(45.0);
        assert_eq!(valid.value(), 45.0, "Valid detection should pass through");
    }

    /// Test registration and unregistration workflow
    #[test]
    fn test_stealth_detection_registration_workflow() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        // Register objects in stealth system
        assert!(stealth.register_object(1).is_ok());
        assert!(stealth.register_object(2).is_ok());
        assert!(
            stealth.register_object(1).is_err(),
            "Should not register twice"
        );

        // Register objects in detection system
        assert!(detection.register_object(1).is_ok());
        assert!(detection.register_object(3).is_ok());
        assert!(
            detection.register_object(1).is_err(),
            "Should not register twice"
        );

        // Set properties
        assert!(stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .is_ok());
        assert!(detection
            .set_detection_strength(3, DetectionStrength::strong_detector())
            .is_ok());

        // Unregister
        assert!(stealth.unregister_object(1).is_ok());
        assert!(detection.unregister_object(3).is_ok());

        // Should not find unregistered objects
        assert!(stealth.get_stealth_strength(1).is_err());
        assert!(detection.get_detection_strength(3).is_err());
    }

    /// Test invalid player ID handling
    #[test]
    fn test_invalid_player_id_handling() {
        let mut stealth = StealthManager::new();
        stealth.register_object(1).unwrap();

        // Valid player IDs (0-7)
        for player in 0..8 {
            assert!(stealth
                .set_stealth_status(1, player, StealthStatus::Invisible)
                .is_ok());
        }

        // Invalid player IDs
        assert!(stealth
            .set_stealth_status(1, 8, StealthStatus::Invisible)
            .is_err());
        assert!(stealth.get_stealth_status(1, 255).is_err());
        assert!(stealth.is_invisible_to_player(1, 9).is_err());
    }

    /// Test frame tracking for debugging
    #[test]
    fn test_update_frame_tracking() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        assert_eq!(stealth.get_last_update_frame(), 0);
        assert_eq!(detection.get_last_update_frame(), 0);

        stealth.set_update_frame(50);
        detection.set_update_frame(75);

        assert_eq!(stealth.get_last_update_frame(), 50);
        assert_eq!(detection.get_last_update_frame(), 75);
    }

    /// Test practical scenario: GLA stealth unit vs standard detection
    #[test]
    fn test_scenario_gla_stealth_vs_standard_detection() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        // GLA Ranger (stealthed infantry)
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::strong_stealth())
            .unwrap(); // 90.0

        // Standard infantry detector
        detection.register_object(2).unwrap();
        detection
            .set_detection_strength(2, DetectionStrength::standard_detector())
            .unwrap(); // 60.0

        // Standard infantry CANNOT detect GLA stealth (60 < 90)
        assert!(!detection
            .can_detect_stealth(2, 90.0, DetectionModifier::default())
            .unwrap());

        // Upgraded detector (with vision upgrade tech)
        detection.register_object(3).unwrap();
        detection
            .set_detection_strength(3, DetectionStrength::strong_detector())
            .unwrap(); // 90.0

        // Strong detector CAN detect GLA stealth (90 > 90 is false, but 90 >= 90 needs reconsideration)
        // Actually: 90 is NOT > 90, so detection fails. Need 91+ to detect 90
        assert!(!detection
            .can_detect_stealth(3, 90.0, DetectionModifier::default())
            .unwrap());

        // But with a slight detection boost (10+ bonus)
        detection.register_object(4).unwrap();
        detection
            .set_detection_strength(4, DetectionStrength::new(95.0))
            .unwrap(); // 95.0

        // Now can detect GLA stealth
        assert!(detection
            .can_detect_stealth(4, 90.0, DetectionModifier::default())
            .unwrap());
    }

    /// Test practical scenario: Stealth unit visibility progression
    #[test]
    fn test_scenario_stealth_unit_visibility_progression() {
        let mut stealth = StealthManager::new();
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();

        // Initial state: invisible to all players
        for player in 0..8 {
            stealth
                .set_stealth_status(1, player, StealthStatus::Invisible)
                .unwrap();
        }

        // Player 0 detects the stealth unit
        stealth.reveal_stealth(1, 0, 2, 50).unwrap();
        assert!(!stealth.is_invisible_to_player(1, 0).unwrap());

        // Player 1 also detects it moments later
        stealth.reveal_stealth(1, 1, 3, 52).unwrap();
        assert!(!stealth.is_invisible_to_player(1, 1).unwrap());

        // Players 2-7 still cannot see it
        for player in 2..8 {
            assert!(stealth.is_invisible_to_player(1, player).unwrap());
        }

        // Stealthed unit reactivates stealth somehow
        stealth.reset_stealth_all(1).unwrap();

        // Now invisible to all again
        for player in 0..8 {
            assert!(stealth.is_invisible_to_player(1, player).unwrap());
        }
    }
}
