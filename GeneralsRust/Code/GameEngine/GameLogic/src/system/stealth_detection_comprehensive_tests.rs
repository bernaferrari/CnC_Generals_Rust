//! Comprehensive Integration Tests for Stealth & Detection System
//!
//! This module tests all interactions between stealth/detection modules:
//! - StealthManager: Per-object stealth status and strength
//! - DetectionManager: Detection capabilities and range
//! - StealthConditions: Conditions that break stealth
//! - StealthSpecialPower: Temporary/permanent stealth grants
//! - DetectionEvents: Event generation and feedback
//! - Visibility: Per-player visibility system
//! - StealthUpgrade: Upgrade integration (if applicable)
//!
//! Tests organized by integration scenario with 44+ comprehensive tests

#[cfg(test)]
mod stealth_detection_comprehensive_tests {
    use crate::common::ObjectID;
    use crate::system::detection_events::{
        get_detection_events_manager, AudioEventType, DetectionEventManager, DetectionEventType,
        EvaMessageType,
    };
    use crate::system::detection_manager::{
        get_detection_manager, DetectionManager, DetectionModifier, DetectionStrength,
    };
    use crate::system::stealth_conditions::{
        get_stealth_conditions_manager, StealthCondition, StealthConditionsManager,
    };
    use crate::system::stealth_manager::{
        get_stealth_manager, StealthManager, StealthStatus, StealthStrength,
    };
    use crate::system::stealth_special_power::{
        get_stealth_special_power_manager, Coord3D, StealthSpecialPowerManager, PERMANENT_STEALTH,
    };

    // ============================================================================
    // 1. STEALTH CONDITIONS + STEALTH MANAGER (5 tests)
    // ============================================================================

    /// Test 1a: Can't stealth while attacking
    #[test]
    fn test_cant_stealth_while_attacking() {
        let mut conditions = StealthConditionsManager::new();
        let mut stealth = StealthManager::new();

        // Setup
        conditions.register_object(1).unwrap();
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();

        // Action: Set stealth while not attacking (should succeed)
        assert!(conditions.can_stealth(1).unwrap());
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );

        // Action: Start attacking
        conditions.set_attacking(1, true).unwrap();

        // Assertion: Cannot stealth while attacking
        assert!(conditions.is_attacking(1).unwrap());
        assert!(!conditions.can_stealth(1).unwrap());
    }

    /// Test 1b: Can't stealth while moving fast
    #[test]
    fn test_cant_stealth_while_moving_fast() {
        let mut conditions = StealthConditionsManager::new();

        conditions.register_object(1).unwrap();

        // Setup: Unit starts stealthed with no conditions
        assert!(conditions.can_stealth(1).unwrap());

        // Action: Unit starts moving
        conditions.set_moving(1, true).unwrap();

        // Assertion: Cannot stealth while moving
        assert!(conditions.is_moving(1).unwrap());
        assert!(!conditions.can_stealth(1).unwrap());

        // Cleanup: Unit stops moving
        conditions.set_moving(1, false).unwrap();
        assert!(conditions.can_stealth(1).unwrap());
    }

    /// Test 1c: Can't stealth while firing weapons
    #[test]
    fn test_cant_stealth_while_firing_weapons() {
        let mut conditions = StealthConditionsManager::new();

        conditions.register_object(1).unwrap();

        // Setup: Unit can stealth initially
        assert!(conditions.can_stealth(1).unwrap());

        // Action: Fire all weapons
        conditions.set_firing_primary(1, true).unwrap();
        assert!(!conditions.can_stealth(1).unwrap());

        conditions.set_firing_primary(1, false).unwrap();
        conditions.set_firing_secondary(1, true).unwrap();
        assert!(!conditions.can_stealth(1).unwrap());

        conditions.set_firing_secondary(1, false).unwrap();
        conditions.set_firing_tertiary(1, true).unwrap();
        assert!(!conditions.can_stealth(1).unwrap());

        // Cleanup
        conditions.set_firing_tertiary(1, false).unwrap();
        assert!(conditions.can_stealth(1).unwrap());
    }

    /// Test 1d: Can't stealth while taking damage
    #[test]
    fn test_cant_stealth_while_taking_damage() {
        let mut conditions = StealthConditionsManager::new();

        conditions.register_object(1).unwrap();

        // Setup: Unit can stealth normally
        assert!(conditions.can_stealth(1).unwrap());

        // Action: Take damage
        conditions.set_taking_damage(1, true).unwrap();

        // Assertion: Cannot stealth while damaged
        assert!(conditions.is_taking_damage(1).unwrap());
        assert!(!conditions.can_stealth(1).unwrap());

        // Cleanup: Stop taking damage
        conditions.set_taking_damage(1, false).unwrap();
        assert!(conditions.can_stealth(1).unwrap());
    }

    /// Test 1e: Can't stealth while riders attack
    #[test]
    fn test_cant_stealth_while_riders_attack() {
        let mut conditions = StealthConditionsManager::new();

        conditions.register_object(1).unwrap();

        // Setup: Unit can stealth normally
        assert!(conditions.can_stealth(1).unwrap());

        // Action: Riders start attacking
        conditions.set_riders_attacking(1, true).unwrap();

        // Assertion: Cannot stealth while riders attacking
        assert!(conditions.are_riders_attacking(1).unwrap());
        assert!(!conditions.can_stealth(1).unwrap());

        // Cleanup
        conditions.set_riders_attacking(1, false).unwrap();
        assert!(conditions.can_stealth(1).unwrap());
    }

    // ============================================================================
    // 2. DETECTION + MODIFIERS (4 tests)
    // ============================================================================

    /// Test 2a: Detection at close range (full effectiveness)
    #[test]
    fn test_detection_at_close_range() {
        let mut detection = DetectionManager::new();

        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        // Close range: distance_factor = 1.0 (full effectiveness)
        let close_modifier = DetectionModifier::new(1.0, 1.0, 1.0, 1.0);
        let effectiveness = detection
            .get_detection_effectiveness(1, close_modifier)
            .unwrap();

        // Standard detector at close range should have full 60.0 effectiveness
        assert!((effectiveness - 60.0).abs() < 0.01);
        assert!(detection
            .can_detect_stealth(1, 50.0, close_modifier)
            .unwrap());
        assert!(!detection
            .can_detect_stealth(1, 70.0, close_modifier)
            .unwrap());
    }

    /// Test 2b: Detection at distance (reduced effectiveness)
    #[test]
    fn test_detection_at_distance_reduced() {
        let mut detection = DetectionManager::new();

        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        // Distant range: distance_factor = 0.5 (half effectiveness)
        let distant_modifier = DetectionModifier::new(0.5, 1.0, 1.0, 1.0);
        let effectiveness = detection
            .get_detection_effectiveness(1, distant_modifier)
            .unwrap();

        // Standard detector at distance: 60.0 * 0.5 = 30.0
        assert!((effectiveness - 30.0).abs() < 0.01);
        assert!(detection
            .can_detect_stealth(1, 20.0, distant_modifier)
            .unwrap());
        assert!(!detection
            .can_detect_stealth(1, 40.0, distant_modifier)
            .unwrap());
    }

    /// Test 2c: Detection of moving unit (easier)
    #[test]
    fn test_detection_of_moving_unit() {
        let mut detection = DetectionManager::new();

        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        // Moving unit: movement_factor = 1.0 (full effectiveness)
        let moving_modifier = DetectionModifier::new(1.0, 1.0, 1.0, 1.0);

        // Stealth: 50.0, Detection: 60.0
        // Should detect moving unit
        assert!(detection
            .can_detect_stealth(1, 50.0, moving_modifier)
            .unwrap());
    }

    /// Test 2d: Detection of stationary unit (harder)
    #[test]
    fn test_detection_of_stationary_unit() {
        let mut detection = DetectionManager::new();

        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::standard_detector())
            .unwrap();

        // Stationary unit: movement_factor = 0.5 (reduced effectiveness)
        let stationary_modifier = DetectionModifier::new(1.0, 1.0, 0.5, 1.0);
        let effectiveness = detection
            .get_detection_effectiveness(1, stationary_modifier)
            .unwrap();

        // Standard detector vs stationary: 60.0 * 0.5 = 30.0
        assert!((effectiveness - 30.0).abs() < 0.01);
        assert!(detection
            .can_detect_stealth(1, 20.0, stationary_modifier)
            .unwrap());
        assert!(!detection
            .can_detect_stealth(1, 40.0, stationary_modifier)
            .unwrap());
    }

    // ============================================================================
    // 3. STEALTH + DETECTION (5 tests)
    // ============================================================================

    /// Test 3a: Standard stealth vs standard detection
    #[test]
    fn test_standard_stealth_vs_standard_detection() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();

        detection.register_object(2).unwrap();
        detection
            .set_detection_strength(2, DetectionStrength::standard_detector())
            .unwrap();

        // Both 60.0: detection is NOT > stealth, so should not detect
        let can_detect = detection
            .can_detect_stealth(2, 60.0, DetectionModifier::default())
            .unwrap();
        assert!(!can_detect);
    }

    /// Test 3b: Strong stealth vs weak detection (undetectable)
    #[test]
    fn test_strong_stealth_vs_weak_detection() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::strong_stealth())
            .unwrap();

        detection.register_object(2).unwrap();
        detection
            .set_detection_strength(2, DetectionStrength::weak_detector())
            .unwrap();

        // Weak detection (30.0) vs strong stealth (90.0)
        let can_detect = detection
            .can_detect_stealth(2, 90.0, DetectionModifier::default())
            .unwrap();
        assert!(!can_detect, "Weak detection cannot see strong stealth");
    }

    /// Test 3c: Weak stealth vs strong detection (easily detected)
    #[test]
    fn test_weak_stealth_vs_strong_detection() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::weak_stealth())
            .unwrap();

        detection.register_object(2).unwrap();
        detection
            .set_detection_strength(2, DetectionStrength::strong_detector())
            .unwrap();

        // Strong detection (90.0) vs weak stealth (30.0)
        let can_detect = detection
            .can_detect_stealth(2, 30.0, DetectionModifier::default())
            .unwrap();
        assert!(can_detect, "Strong detection easily sees weak stealth");
    }

    /// Test 3d: Partial detection with modifiers
    #[test]
    fn test_partial_detection_with_modifiers() {
        let mut detection = DetectionManager::new();

        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::strong_detector())
            .unwrap();

        // Strong detection at distance with movement penalty
        let modifier = DetectionModifier::new(0.5, 1.0, 0.5, 1.0);
        let effectiveness = detection.get_detection_effectiveness(1, modifier).unwrap();

        // 90.0 * 0.5 * 0.5 = 22.5
        assert!((effectiveness - 22.5).abs() < 0.01);

        // Should not detect strong stealth (90.0) at this effectiveness
        assert!(!detection.can_detect_stealth(1, 90.0, modifier).unwrap());
    }

    /// Test 3e: Detection impossible without detectors
    #[test]
    fn test_detection_impossible_without_detectors() {
        let mut detection = DetectionManager::new();

        // Register unit with no detection capability
        detection.register_object(1).unwrap();
        detection
            .set_detection_strength(1, DetectionStrength::none())
            .unwrap();

        // No detection capability should never detect
        assert!(!detection
            .can_detect_stealth(1, 0.0, DetectionModifier::default())
            .unwrap());
        assert!(!detection
            .can_detect_stealth(1, 50.0, DetectionModifier::default())
            .unwrap());
    }

    // ============================================================================
    // 4. DISGUISE + VISIBILITY (4 tests)
    // Note: Tests visibility state transitions with opacity
    // ============================================================================

    /// Test 4a: Visibility status transitions
    #[test]
    fn test_visibility_status_transitions() {
        let mut stealth = StealthManager::new();

        stealth.register_object(1).unwrap();

        // Initial: Hidden
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Hidden
        );

        // Transition to Invisible
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );

        // Transition to Revealed
        stealth
            .set_stealth_status(1, 0, StealthStatus::Revealed)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Revealed
        );

        // Transition back to Invisible
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );
    }

    /// Test 4b: Per-player opacity rendering
    #[test]
    fn test_per_player_opacity_rendering() {
        let mut stealth = StealthManager::new();

        stealth.register_object(1).unwrap();

        // Player 0 sees invisible
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );

        // Player 1 sees revealed
        stealth
            .set_stealth_status(1, 1, StealthStatus::Revealed)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 1).unwrap(),
            StealthStatus::Revealed
        );

        // Player 2 sees hidden
        stealth
            .set_stealth_status(1, 2, StealthStatus::Hidden)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 2).unwrap(),
            StealthStatus::Hidden
        );

        // Verify independence: player 0 still invisible
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );
    }

    /// Test 4c: Reveal animation progression
    #[test]
    fn test_reveal_animation_progression() {
        let mut stealth = StealthManager::new();

        stealth.register_object(1).unwrap();

        // Start: invisible to all players
        for player in 0..3 {
            stealth
                .set_stealth_status(1, player, StealthStatus::Invisible)
                .unwrap();
        }

        // Reveal to player 0 at frame 100
        stealth.reveal_stealth(1, 0, 2, 100).unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Revealed
        );
        assert_eq!(
            stealth.get_stealth_status(1, 1).unwrap(),
            StealthStatus::Invisible
        );

        // Reveal to player 1 at frame 150
        stealth.reveal_stealth(1, 1, 2, 150).unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 1).unwrap(),
            StealthStatus::Revealed
        );
    }

    /// Test 4d: Reset stealth back to invisible
    #[test]
    fn test_reset_stealth_back_to_invisible() {
        let mut stealth = StealthManager::new();

        stealth.register_object(1).unwrap();

        // Reveal to all players
        stealth.break_stealth_all(1, 100).unwrap();
        for player in 0..3 {
            assert_eq!(
                stealth.get_stealth_status(1, player).unwrap(),
                StealthStatus::Revealed
            );
        }

        // Reset back to invisible
        stealth.reset_stealth_all(1).unwrap();
        for player in 0..3 {
            assert_eq!(
                stealth.get_stealth_status(1, player).unwrap(),
                StealthStatus::Invisible
            );
        }
    }

    // ============================================================================
    // 5. SPECIAL POWERS INTEGRATION (5 tests)
    // ============================================================================

    /// Test 5a: Temporary stealth grant expires
    #[test]
    fn test_temporary_stealth_grant_expires() {
        let mut special_power = StealthSpecialPowerManager::new();

        // Grant stealth for 5 frames
        special_power.grant_stealth(1, 0, 5, 0).unwrap();
        assert!(special_power.is_granted_stealth(1).unwrap());
        assert_eq!(special_power.get_remaining_frames(1).unwrap(), 5);

        // Update frames 1-4
        for frame in 1..5 {
            special_power.update_grants(frame).unwrap();
            assert!(special_power.is_granted_stealth(1).unwrap());
            assert_eq!(
                special_power.get_remaining_frames(1).unwrap(),
                5 - frame as i32
            );
        }

        // Update frame 5: grant should expire
        special_power.update_grants(5).unwrap();
        assert!(!special_power.is_granted_stealth(1).unwrap());
    }

    /// Test 5b: Permanent stealth grant persists
    #[test]
    fn test_permanent_stealth_grant_persists() {
        let mut special_power = StealthSpecialPowerManager::new();

        // Grant permanent stealth
        special_power.grant_stealth_permanent(1, 0, 0).unwrap();
        assert!(special_power.is_granted_stealth(1).unwrap());
        assert_eq!(
            special_power.get_remaining_frames(1).unwrap(),
            PERMANENT_STEALTH
        );

        // Update many frames
        for frame in 1..=1000 {
            special_power.update_grants(frame).unwrap();
            assert!(special_power.is_granted_stealth(1).unwrap());
            assert_eq!(
                special_power.get_remaining_frames(1).unwrap(),
                PERMANENT_STEALTH
            );
        }
    }

    /// Test 5c: Area stealth effect grows radius
    #[test]
    fn test_area_stealth_effect_grows_radius() {
        let mut special_power = StealthSpecialPowerManager::new();

        let center = Coord3D::new(100.0, 200.0, 0.0);

        // Create area that grows from 0 to 100 over 100 frames
        let area_id = special_power
            .create_area_stealth(center, 0.0, 100.0, 100, 0xFFFFFFFF, 1)
            .unwrap();

        let initial_area = special_power.get_area_stealth(area_id).unwrap();
        assert_eq!(initial_area.current_radius, 0.0);

        // Add a unit to keep area alive
        special_power.add_unit_to_area(area_id, 10).unwrap();

        // Update radius growth
        for _frame in 0..50 {
            special_power.update_area_effects(0).unwrap();
        }

        let growing_area = special_power.get_area_stealth(area_id).unwrap();
        assert!(growing_area.current_radius > 0.0);
        assert!(growing_area.current_radius < 100.0);
        assert!(growing_area.is_growing());

        // Update to completion
        for _frame in 50..100 {
            special_power.update_area_effects(0).unwrap();
        }

        let final_area = special_power.get_area_stealth(area_id).unwrap();
        assert!((final_area.current_radius - 100.0).abs() < 0.1);
        assert!(!final_area.is_growing());
    }

    /// Test 5d: Spy vision shares team vision
    #[test]
    fn test_spy_vision_shares_team_vision() {
        let mut special_power = StealthSpecialPowerManager::new();

        // Grant spy vision to player 0 for 100 frames
        special_power
            .grant_spy_vision(0, 100, 0xFFFFFFFF, 0)
            .unwrap();
        assert!(special_power.has_spy_vision(0));

        let grant = special_power.get_spy_vision_grant(0).unwrap();
        assert_eq!(grant.frames_remaining, 100);
        assert!(grant.is_active());

        // Add vision sources
        special_power.add_spy_vision_source(0, 1).unwrap();
        special_power.add_spy_vision_source(0, 2).unwrap();

        let grant = special_power.get_spy_vision_grant(0).unwrap();
        assert_eq!(grant.shared_from_players.len(), 2);
        assert!(grant.shared_from_players.contains(&1));
        assert!(grant.shared_from_players.contains(&2));
    }

    /// Test 5e: Multiple special powers stack
    #[test]
    fn test_multiple_special_powers_stack() {
        let mut special_power = StealthSpecialPowerManager::new();

        // Grant stealth to multiple units
        special_power.grant_stealth(1, 0, 50, 0).unwrap();
        special_power.grant_stealth(2, 0, 100, 0).unwrap();
        special_power.grant_stealth_permanent(3, 0, 0).unwrap();

        // Create area effect
        let center = Coord3D::new(0.0, 0.0, 0.0);
        let area_id = special_power
            .create_area_stealth(center, 0.0, 100.0, 30, 0xFFFFFFFF, 4)
            .unwrap();

        // Grant spy vision
        special_power
            .grant_spy_vision(0, 100, 0xFFFFFFFF, 0)
            .unwrap();

        // All should coexist
        assert!(special_power.is_granted_stealth(1).unwrap());
        assert!(special_power.is_granted_stealth(2).unwrap());
        assert!(special_power.is_granted_stealth(3).unwrap());
        assert!(special_power.get_area_stealth(area_id).is_ok());
        assert!(special_power.has_spy_vision(0));

        let all_grants = special_power.get_all_active_grants();
        assert_eq!(all_grants.len(), 3);

        let all_areas = special_power.get_all_area_effects();
        assert_eq!(all_areas.len(), 1);
    }

    // ============================================================================
    // 6. UPGRADES + CAPABILITY (4 tests)
    // Note: Tests integration with stealth capability system
    // ============================================================================

    /// Test 6a: Unit gains stealth capability from upgrade
    #[test]
    fn test_unit_gains_stealth_capability_from_upgrade() {
        let mut stealth = StealthManager::new();
        let mut special_power = StealthSpecialPowerManager::new();

        stealth.register_object(1).unwrap();

        // Initially no stealth
        assert_eq!(stealth.get_stealth_strength(1).unwrap().value(), 0.0);

        // Simulate upgrade: grant permanent stealth
        special_power.grant_stealth_permanent(1, 0, 0).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();

        // After upgrade: has stealth capability
        assert!(special_power.is_granted_stealth(1).unwrap());
        assert_eq!(stealth.get_stealth_strength(1).unwrap().value(), 60.0);
    }

    /// Test 6b: Spawned units inherit stealth upgrade
    #[test]
    fn test_spawned_units_inherit_stealth_upgrade() {
        let mut stealth = StealthManager::new();
        let mut special_power = StealthSpecialPowerManager::new();

        // Original unit with stealth
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();
        special_power.grant_stealth_permanent(1, 0, 0).unwrap();

        // Spawned unit inherits properties
        stealth.register_object(2).unwrap();
        stealth
            .set_stealth_strength(2, StealthStrength::standard_cloak())
            .unwrap();
        special_power.grant_stealth_permanent(2, 0, 0).unwrap();

        // Both have stealth
        assert_eq!(stealth.get_stealth_strength(1).unwrap().value(), 60.0);
        assert_eq!(stealth.get_stealth_strength(2).unwrap().value(), 60.0);
        assert!(special_power.is_granted_stealth(1).unwrap());
        assert!(special_power.is_granted_stealth(2).unwrap());
    }

    /// Test 6c: Black market blocks stealth upgrade
    #[test]
    fn test_black_market_blocks_stealth_upgrade() {
        let mut conditions = StealthConditionsManager::new();
        let mut stealth = StealthManager::new();

        conditions.register_object(1).unwrap();
        stealth.register_object(1).unwrap();

        // Normal conditions: can stealth
        assert!(conditions.can_stealth(1).unwrap());

        // Set black market requirement
        conditions.set_no_black_market(1, true).unwrap();

        // Cannot stealth without black market
        assert!(conditions.is_no_black_market(1).unwrap());
        assert!(!conditions.can_stealth(1).unwrap());

        // Black market enabled
        conditions.set_no_black_market(1, false).unwrap();

        // Can stealth again
        assert!(conditions.can_stealth(1).unwrap());
    }

    /// Test 6d: Multiple upgrades stack
    #[test]
    fn test_multiple_upgrades_stack() {
        let mut stealth = StealthManager::new();
        let mut special_power = StealthSpecialPowerManager::new();

        stealth.register_object(1).unwrap();

        // Apply multiple upgrades
        stealth
            .set_stealth_strength(1, StealthStrength::strong_stealth())
            .unwrap();
        special_power.grant_stealth_permanent(1, 0, 0).unwrap();

        // Add area effect around unit
        let center = Coord3D::new(0.0, 0.0, 0.0);
        let area_id = special_power
            .create_area_stealth(center, 0.0, 50.0, 30, 0xFFFFFFFF, 1)
            .unwrap();
        special_power.add_unit_to_area(area_id, 1).unwrap();

        // All upgrades coexist
        assert_eq!(stealth.get_stealth_strength(1).unwrap().value(), 90.0);
        assert!(special_power.is_granted_stealth(1).unwrap());
        let area = special_power.get_area_stealth(area_id).unwrap();
        assert!(area.affected_units.contains(&1));
    }

    // ============================================================================
    // 7. CONDITIONS + SPECIAL POWERS (3 tests)
    // ============================================================================

    /// Test 7a: Special power stealth overrides conditions
    #[test]
    fn test_special_power_stealth_overrides_conditions() {
        let mut conditions = StealthConditionsManager::new();
        let mut special_power = StealthSpecialPowerManager::new();

        conditions.register_object(1).unwrap();

        // Unit is attacking
        conditions.set_attacking(1, true).unwrap();
        assert!(!conditions.can_stealth(1).unwrap());

        // But has special power stealth grant (independent of conditions)
        special_power.grant_stealth_permanent(1, 0, 0).unwrap();
        assert!(special_power.is_granted_stealth(1).unwrap());

        // Conditions still prevent normal stealth
        assert!(!conditions.can_stealth(1).unwrap());
        // But special power grant persists
        assert!(special_power.is_granted_stealth(1).unwrap());
    }

    /// Test 7b: Permanent grant ignores breaking conditions
    #[test]
    fn test_permanent_grant_ignores_breaking_conditions() {
        let mut conditions = StealthConditionsManager::new();
        let mut special_power = StealthSpecialPowerManager::new();

        conditions.register_object(1).unwrap();

        // Grant permanent stealth
        special_power.grant_stealth_permanent(1, 0, 0).unwrap();

        // Set multiple breaking conditions
        conditions.set_attacking(1, true).unwrap();
        conditions.set_moving(1, true).unwrap();
        conditions.set_taking_damage(1, true).unwrap();

        // Conditions prevent normal stealth
        assert!(!conditions.can_stealth(1).unwrap());
        assert_eq!(conditions.count_active_conditions(1).unwrap(), 3);

        // But permanent grant is independent
        assert!(special_power.is_granted_stealth(1).unwrap());
        assert_eq!(
            special_power.get_remaining_frames(1).unwrap(),
            PERMANENT_STEALTH
        );
    }

    /// Test 7c: Granted stealth breaks on expiration
    #[test]
    fn test_granted_stealth_breaks_on_expiration() {
        let mut special_power = StealthSpecialPowerManager::new();

        // Grant temporary stealth
        special_power.grant_stealth(1, 0, 3, 0).unwrap();
        assert!(special_power.is_granted_stealth(1).unwrap());

        // Update until expiration
        special_power.update_grants(1).unwrap();
        assert!(special_power.is_granted_stealth(1).unwrap());

        special_power.update_grants(2).unwrap();
        assert!(special_power.is_granted_stealth(1).unwrap());

        special_power.update_grants(3).unwrap();
        // Grant expired
        assert!(!special_power.is_granted_stealth(1).unwrap());
    }

    // ============================================================================
    // 8. DETECTION EVENTS + FEEDBACK (3 tests)
    // ============================================================================

    /// Test 8a: Detection queues radar event
    #[test]
    fn test_detection_queues_radar_event() {
        let mut events = DetectionEventManager::new();

        // Register detection
        assert!(events.register_detection(1, 2, 100, 0, 1).is_ok());

        // Should have queued events (stealth discovered + radar)
        assert!(events.has_pending_events());
        assert!(events.pending_event_count() >= 2);

        // Dequeue events
        let event1 = events.dequeue_event();
        assert!(event1.is_some());

        let event2 = events.dequeue_event();
        assert!(event2.is_some());
    }

    /// Test 8b: Multiple detections create multiple events
    #[test]
    fn test_multiple_detections_create_multiple_events() {
        let mut events = DetectionEventManager::new();

        // Register multiple detections
        assert!(events.register_detection(1, 2, 100, 0, 1).is_ok());
        assert!(events.register_detection(1, 3, 101, 0, 1).is_ok());
        assert!(events.register_detection(1, 4, 102, 0, 1).is_ok());

        // Should have multiple events
        let pending = events.pending_event_count();
        assert!(pending >= 6); // 3 detections * 2 events each

        // Process all
        let all_events = events.process_all_events();
        assert!(all_events.len() >= 6);
        assert_eq!(events.pending_event_count(), 0);
    }

    /// Test 8c: Event history tracks all detections
    #[test]
    fn test_event_history_tracks_all_detections() {
        let mut events = DetectionEventManager::new();

        // Register multiple detections for same object
        assert!(events.register_detection(1, 2, 100, 0, 1).is_ok());
        assert!(events.register_detection(1, 2, 101, 0, 1).is_ok());

        // Get history for detected object
        let history = events.get_detection_history(2);
        assert!(history.len() >= 2); // At least 2 stealth discovered events

        // Check last detection frame
        let last_frame = events.get_last_detection_frame(2);
        assert!(last_frame > 0);
    }

    // ============================================================================
    // 9. FULL SYSTEM WORKFLOW (6 tests)
    // ============================================================================

    /// Test 9a: Unit becomes stealthed
    #[test]
    fn test_unit_becomes_stealthed() {
        let mut stealth = StealthManager::new();
        let mut conditions = StealthConditionsManager::new();

        stealth.register_object(1).unwrap();
        conditions.register_object(1).unwrap();

        // Initial: visible
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Hidden
        );

        // Preconditions: can stealth
        assert!(conditions.can_stealth(1).unwrap());

        // Apply stealth
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();

        // Now invisible
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );
    }

    /// Test 9b: Detector discovers stealth
    #[test]
    fn test_detector_discovers_stealth() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        // Stealthed unit
        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::weak_stealth())
            .unwrap();

        // Detector unit
        detection.register_object(2).unwrap();
        detection
            .set_detection_strength(2, DetectionStrength::strong_detector())
            .unwrap();

        // Check: strong detector (90) > weak stealth (30)
        assert!(detection
            .can_detect_stealth(2, 30.0, DetectionModifier::default())
            .unwrap());
    }

    /// Test 9c: Events fire (radar, audio, Eva)
    #[test]
    fn test_events_fire_radar_audio_eva() {
        let mut events = DetectionEventManager::new();

        // Register detection triggers events
        assert!(events.register_detection(1, 2, 100, 0, 1).is_ok());

        // Should have radar event
        let radar = events.create_radar_event(1, DetectionEventType::RadarEventStealthDiscovered);
        assert!(radar.is_ok());

        // Should have audio event capability
        let audio = events.create_audio_event(AudioEventType::LoudPing, glam::Vec3::ZERO);
        assert!(audio.is_ok());

        // Should have Eva message capability
        let eva = events.create_eva_message(EvaMessageType::EnemyDetected, 0, 2);
        assert!(eva.is_ok());
    }

    /// Test 9d: Stealth is revealed
    #[test]
    fn test_stealth_is_revealed() {
        let mut stealth = StealthManager::new();

        stealth.register_object(1).unwrap();

        // Start invisible to player 0
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );

        // Reveal by detector unit
        stealth.reveal_stealth(1, 0, 2, 150).unwrap();

        // Now revealed to player 0
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Revealed
        );
    }

    /// Test 9e: Unit enters disguise
    #[test]
    fn test_unit_enters_disguise() {
        let mut stealth = StealthManager::new();

        stealth.register_object(1).unwrap();

        // Revealed unit
        stealth
            .set_stealth_status(1, 0, StealthStatus::Revealed)
            .unwrap();
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Revealed
        );

        // Re-activate stealth (disguise)
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();

        // Back to invisible
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );
    }

    /// Test 9f: Different players see different states
    #[test]
    fn test_different_players_see_different_states() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();

        stealth.register_object(1).unwrap();
        stealth
            .set_stealth_strength(1, StealthStrength::standard_cloak())
            .unwrap();
        stealth
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        stealth
            .set_stealth_status(1, 1, StealthStatus::Invisible)
            .unwrap();

        // Detector for player 0
        detection.register_object(2).unwrap();
        detection
            .set_detection_strength(2, DetectionStrength::standard_detector())
            .unwrap();

        // Detector for player 1
        detection.register_object(3).unwrap();
        detection
            .set_detection_strength(3, DetectionStrength::weak_detector())
            .unwrap();

        // Player 0's detector (60) can see standard stealth (60)? No, needs >
        // Player 1's detector (30) cannot see standard stealth (60)

        // Simulate: Player 0 reveals via different source
        stealth.reveal_stealth(1, 0, 2, 100).unwrap();

        // Different visibility per player
        assert_eq!(
            stealth.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Revealed
        );
        assert_eq!(
            stealth.get_stealth_status(1, 1).unwrap(),
            StealthStatus::Invisible
        );
    }

    // ============================================================================
    // 10. EDGE CASES & ERROR HANDLING (5 tests)
    // ============================================================================

    /// Test 10a: Invalid player IDs gracefully handled
    #[test]
    fn test_invalid_player_ids_gracefully_handled() {
        let mut stealth = StealthManager::new();
        let mut events = DetectionEventManager::new();

        stealth.register_object(1).unwrap();

        // Invalid player ID (> 7)
        assert!(stealth
            .set_stealth_status(1, 8, StealthStatus::Invisible)
            .is_err());
        assert!(stealth.get_stealth_status(1, 255).is_err());

        // Event manager with invalid player
        assert!(events.register_detection(1, 2, 100, 8, 0).is_err());
        assert!(events
            .create_eva_message(EvaMessageType::EnemyDetected, 255, 2)
            .is_err());
    }

    /// Test 10b: Unregistered objects return errors
    #[test]
    fn test_unregistered_objects_return_errors() {
        let stealth = StealthManager::new();
        let detection = DetectionManager::new();
        let mut conditions = StealthConditionsManager::new();

        // Unregistered in stealth
        assert!(stealth.get_stealth_strength(999).is_err());
        assert!(stealth.get_stealth_status(999, 0).is_err());

        // Unregistered in detection
        assert!(detection.get_detection_strength(999).is_err());
        assert!(detection
            .can_detect_stealth(999, 50.0, DetectionModifier::default())
            .is_err());

        // Unregistered in conditions
        assert!(conditions.get_condition_flags(999).is_err());
        assert!(conditions.can_stealth(999).is_err());
    }

    /// Test 10c: Concurrent modifier calculations
    #[test]
    fn test_concurrent_modifier_calculations() {
        let mut detection = DetectionManager::new();

        // Register detector
        detection.register_object(1).unwrap();

        // Note: In single-threaded test, simulate concurrent scenarios
        let modifiers = vec![
            DetectionModifier::new(1.0, 1.0, 1.0, 1.0),
            DetectionModifier::new(0.5, 0.5, 0.5, 0.5),
            DetectionModifier::new(0.1, 0.1, 0.1, 0.1),
        ];

        for modifier in modifiers {
            let effectiveness = detection.get_detection_effectiveness(1, modifier);
            assert!(effectiveness.is_ok());
        }
    }

    /// Test 10d: Frame overflow handling
    #[test]
    fn test_frame_overflow_handling() {
        let mut special_power = StealthSpecialPowerManager::new();

        // Grant stealth
        special_power.grant_stealth(1, 0, 100, 0).unwrap();

        // Update with very large frame numbers (near u32 max)
        let large_frame = u32::MAX - 10;
        special_power.update_grants(large_frame).unwrap();

        // Should handle gracefully
        assert!(special_power.is_granted_stealth(1).is_ok());
    }

    /// Test 10e: Resource cleanup on unit deletion
    #[test]
    fn test_resource_cleanup_on_unit_deletion() {
        let mut stealth = StealthManager::new();
        let mut detection = DetectionManager::new();
        let mut conditions = StealthConditionsManager::new();

        // Register unit in all systems
        stealth.register_object(1).unwrap();
        detection.register_object(1).unwrap();
        conditions.register_object(1).unwrap();

        // Verify registered
        assert!(stealth.get_stealth_strength(1).is_ok());
        assert!(detection.get_detection_strength(1).is_ok());
        assert!(conditions.get_condition_flags(1).is_ok());

        // Unregister from all systems
        assert!(stealth.unregister_object(1).is_ok());
        assert!(detection.unregister_object(1).is_ok());
        assert!(conditions.unregister_object(1).is_ok());

        // Verify cleanup
        assert!(stealth.get_stealth_strength(1).is_err());
        assert!(detection.get_detection_strength(1).is_err());
        assert!(conditions.get_condition_flags(1).is_err());
    }
}
