//! Comprehensive AI Module Tests
//!
//! Tests for all AI update modules including:
//! - Unit behavior modules
//! - Strategic AI systems
//! - Integration tests

#[cfg(test)]
mod ai_module_tests {
    use super::super::*;
    use crate::common::{ObjectID, Coord3D};

    // Test AIUpdateContext creation and basic functionality
    #[test]
    fn test_ai_update_context() {
        let context = AIUpdateContext::new(123, 100);

        assert_eq!(context.object_id, 123);
        assert_eq!(context.current_frame, 100);
        assert_eq!(context.health_percentage, 1.0);
        assert!(!context.is_moving);
        assert!(!context.is_attacking);
        assert!(context.current_target.is_none());
    }

    // Test base AI module functionality
    #[test]
    fn test_base_ai_module() {
        let mut module = ai_update_base::AIUpdateModule::new(
            AIModuleType::Base,
            AIModulePriority::Normal
        );

        assert_eq!(module.get_module_type(), AIModuleType::Base);
        assert_eq!(module.get_priority(), AIModulePriority::Normal);
        assert_eq!(module.get_state(), AIModuleState::Idle);
        assert!(module.is_enabled());

        module.set_enabled(false);
        assert!(!module.is_enabled());
    }

    // Test AIModuleManager
    #[test]
    fn test_ai_module_manager() {
        let mut manager = ai_update_base::AIModuleManager::new();

        let module1 = Box::new(ai_update_base::AIUpdateModule::new(
            AIModuleType::Dozer,
            AIModulePriority::High
        ));

        let module2 = Box::new(ai_update_base::AIUpdateModule::new(
            AIModuleType::Wander,
            AIModulePriority::Low
        ));

        manager.add_module(module1);
        manager.add_module(module2);

        let context = AIUpdateContext::new(1, 0);
        assert!(manager.init_all(&context).is_ok());
    }

    // Test DozerAI behavior
    #[test]
    fn test_dozer_ai_building() {
        let mut dozer = DozerAIUpdate::new();

        // Test initial state
        assert_eq!(dozer.get_module_type(), AIModuleType::Dozer);

        // Set build target
        dozer.set_build_target("PowerPlant".to_string(), [100.0, 100.0, 0.0]);

        let mut context = AIUpdateContext::new(1, 0);
        context.position = [95.0, 95.0, 0.0];

        assert!(dozer.update(&mut context).is_ok());
    }

    #[test]
    fn test_dozer_ai_repair() {
        let mut dozer = DozerAIUpdate::new();

        dozer.add_repair_target(123);
        dozer.add_repair_target(456);

        let context = AIUpdateContext::new(1, 0);
        assert!(dozer.init(&context).is_ok());
    }

    // Test SupplyTruckAI basic wiring (supply system implementation)
    #[test]
    fn test_supply_truck_ai() {
        use crate::modules::SupplyTruckAIInterface;

        let data = SupplyTruckAIUpdateData::default();
        let truck = SupplyTruckAIUpdate::new(data, 1, 0);

        assert_eq!(truck.get_number_boxes(), 0);
        assert!(!truck.is_forced_into_wanting_state());
        assert!(!truck.is_forced_into_busy_state());
    }

    // Test TransportAI behavior
    #[test]
    fn test_transport_ai() {
        let mut transport = TransportAIUpdate::new();

        transport.set_transport_mission([100.0, 100.0, 0.0], [200.0, 200.0, 0.0]);

        assert!(!transport.is_full());
        assert!(transport.is_empty());

        let mut context = AIUpdateContext::new(1, 0);
        assert!(transport.update(&mut context).is_ok());
    }

    // Test JetAI behavior
    #[test]
    fn test_jet_ai_fuel_management() {
        let mut jet = JetAIUpdate::new();

        jet.set_airfield(100);
        jet.set_attack_target(200);

        assert!(!jet.needs_refuel());
        assert!(!jet.needs_rearm());

        let mut context = AIUpdateContext::new(1, 0);
        context.delta_time = 1.0;

        // Consume fuel
        for _ in 0..100 {
            let _ = jet.update(&mut context);
        }

        assert!(jet.needs_refuel());
    }

    // Test TurretAI behavior
    #[test]
    fn test_turret_ai_scanning() {
        let mut turret = TurretAIUpdate::new();

        turret.set_scan_radius(300.0);
        turret.set_powered(true);

        let context = AIUpdateContext::new(1, 0);
        assert!(turret.init(&context).is_ok());

        let mut update_context = AIUpdateContext::new(1, 0);
        assert!(turret.update(&mut update_context).is_ok());
    }

    #[test]
    fn test_turret_ai_power_state() {
        let mut turret = TurretAIUpdate::new();

        turret.set_powered(false);
        assert!(!turret.should_update(&AIUpdateContext::new(1, 0)));

        turret.set_powered(true);
        assert!(turret.should_update(&AIUpdateContext::new(1, 0)));
    }

    // Test WanderAI behavior
    #[test]
    fn test_wander_ai() {
        let mut wander = WanderAIUpdate::new();

        wander.set_home_position([100.0, 100.0, 0.0]);
        wander.set_wander_radius(200.0);

        let context = AIUpdateContext::new(1, 0);
        assert!(wander.init(&context).is_ok());
    }

    // Test DeployStyleAI behavior
    #[test]
    fn test_deploy_style_ai() {
        let mut deploy = DeployStyleAIUpdate::new();

        assert!(deploy.is_packed());
        assert!(!deploy.is_deployed());

        deploy.deploy();

        let mut context = AIUpdateContext::new(1, 0);
        context.delta_time = 1.0;

        // Simulate deployment time
        for _ in 0..5 {
            let _ = deploy.update(&mut context);
        }

        assert!(deploy.is_deployed());
    }

    #[test]
    fn test_deploy_style_auto_behavior() {
        let mut deploy = DeployStyleAIUpdate::new();

        deploy.set_deploy_mode(deploy_style_ai::DeployMode::Auto);

        let mut context = AIUpdateContext::new(1, 0);
        context.is_moving = false;
        context.delta_time = 1.0;

        assert!(deploy.update(&mut context).is_ok());
    }

    // Test Target Prioritization System
    #[test]
    fn test_target_prioritization_scoring() {
        let mut system = TargetPrioritization::new();

        let target = target_prioritization::PrioritizationTarget {
            object_id: 1,
            position: [100.0, 100.0, 0.0],
            unit_type: "CommandCenter".to_string(),
            health_percentage: 0.8,
            is_attacking: false,
            distance_to_attacker: 50.0,
        };

        let score = system.evaluate_target(&target);

        assert!(score.total_score > 0.0);
        assert!(score.value_score > 0.0);
        assert!(score.distance_score > 0.0);
    }

    #[test]
    fn test_target_prioritization_selection() {
        let mut system = TargetPrioritization::new();

        let target1 = target_prioritization::PrioritizationTarget {
            object_id: 1,
            position: [100.0, 100.0, 0.0],
            unit_type: "Infantry".to_string(),
            health_percentage: 1.0,
            is_attacking: false,
            distance_to_attacker: 50.0,
        };

        let target2 = target_prioritization::PrioritizationTarget {
            object_id: 2,
            position: [150.0, 150.0, 0.0],
            unit_type: "CommandCenter".to_string(),
            health_percentage: 0.8,
            is_attacking: false,
            distance_to_attacker: 100.0,
        };

        let targets = vec![target1, target2];
        let best = system.select_best_target(&targets);

        // Command center should be prioritized over infantry
        assert_eq!(best, Some(2));
    }

    #[test]
    fn test_target_prioritization_top_n() {
        let mut system = TargetPrioritization::new();

        let targets: Vec<_> = (0..10).map(|i| {
            target_prioritization::PrioritizationTarget {
                object_id: i,
                position: [100.0, 100.0, 0.0],
                unit_type: "Infantry".to_string(),
                health_percentage: (i as f32) / 10.0,
                is_attacking: i % 2 == 0,
                distance_to_attacker: i as f32 * 10.0,
            }
        }).collect();

        let top_3 = system.get_top_n_targets(&targets, 3);
        assert_eq!(top_3.len(), 3);
    }

    // Test Threat Assessment System
    #[test]
    fn test_threat_assessment_basic() {
        let mut system = ThreatAssessmentSystem::new();
        system.set_base_position([0.0, 0.0, 0.0]);

        let threat = threat_assessment::ThreatInfo {
            threat_id: 123,
            threat_type: threat_assessment::ThreatType::Military,
            threat_level: threat_assessment::ThreatLevel::Moderate,
            position: [100.0, 100.0, 0.0],
            severity: 0.7,
            detection_frame: 0,
            last_update_frame: 0,
            estimated_strength: 50.0,
            distance_to_base: 141.42,
        };

        system.add_threat(threat);

        assert_eq!(system.active_threats.len(), 1);
        assert!(matches!(
            system.get_overall_threat_level(),
            threat_assessment::ThreatLevel::High | threat_assessment::ThreatLevel::Moderate
        ));
    }

    #[test]
    fn test_threat_assessment_removal() {
        let mut system = ThreatAssessmentSystem::new();

        let threat = threat_assessment::ThreatInfo {
            threat_id: 123,
            threat_type: threat_assessment::ThreatType::Military,
            threat_level: threat_assessment::ThreatLevel::Low,
            position: [100.0, 100.0, 0.0],
            severity: 0.3,
            detection_frame: 0,
            last_update_frame: 0,
            estimated_strength: 10.0,
            distance_to_base: 200.0,
        };

        system.add_threat(threat);
        assert_eq!(system.active_threats.len(), 1);

        system.remove_threat(123);
        assert_eq!(system.active_threats.len(), 0);
    }

    #[test]
    fn test_threat_assessment_response() {
        let mut system = ThreatAssessmentSystem::new();

        let critical_threat = threat_assessment::ThreatInfo {
            threat_id: 999,
            threat_type: threat_assessment::ThreatType::Strategic,
            threat_level: threat_assessment::ThreatLevel::Critical,
            position: [10.0, 10.0, 0.0],
            severity: 0.95,
            detection_frame: 0,
            last_update_frame: 0,
            estimated_strength: 100.0,
            distance_to_base: 15.0,
        };

        system.add_threat(critical_threat);

        let response = system.get_recommended_response();
        assert_eq!(response, threat_assessment::ThreatResponse::Emergency);
    }

    // Test Build Order Optimizer
    #[test]
    fn test_build_order_creation() {
        let mut order = build_order::BuildOrder::new(
            "Barracks".to_string(),
            build_order::BuildPriority::High
        );

        order.max_count = Some(2);

        assert!(!order.is_complete());

        order.current_count = 2;
        assert!(order.is_complete());
    }

    #[test]
    fn test_build_order_prerequisites() {
        let mut order = build_order::BuildOrder::new(
            "WarFactory".to_string(),
            build_order::BuildPriority::Normal
        );

        order.prerequisites.push("Barracks".to_string());

        let mut buildings = std::collections::HashMap::new();
        assert!(!order.can_build(&buildings));

        buildings.insert("Barracks".to_string(), 1);
        assert!(order.can_build(&buildings));
    }

    #[test]
    fn test_build_optimizer_strategy() {
        let mut optimizer = BuildOrderOptimizer::new();

        optimizer.set_strategy_template("rush");
        assert!(optimizer.get_build_queue_size() > 0);

        optimizer.build_queue.clear();

        optimizer.set_strategy_template("economic");
        assert!(optimizer.get_build_queue_size() > 0);
    }

    #[test]
    fn test_build_optimizer_resources() {
        let mut optimizer = BuildOrderOptimizer::new();

        optimizer.update_resources(5000);

        let mut order = build_order::BuildOrder::new(
            "PowerPlant".to_string(),
            build_order::BuildPriority::High
        );
        order.cost = 800;

        optimizer.add_build_order(order);

        let next = optimizer.get_next_build();
        assert!(next.is_some());
    }

    // Test Tech Progression Manager
    #[test]
    fn test_tech_progression_basic() {
        let mut manager = TechProgressionManager::new();

        let node = tech_progression::TechNode::new(
            "BasicTech".to_string(),
            tech_progression::TechTier::Tier1
        );

        manager.add_tech_node(node);
        manager.update_resources(5000);

        assert!(manager.can_research("BasicTech"));
    }

    #[test]
    fn test_tech_progression_prerequisites() {
        let mut manager = TechProgressionManager::new();

        let mut node1 = tech_progression::TechNode::new(
            "BasicTech".to_string(),
            tech_progression::TechTier::Tier1
        );
        node1.cost = 1000;

        let mut node2 = tech_progression::TechNode::new(
            "AdvancedTech".to_string(),
            tech_progression::TechTier::Tier2
        );
        node2.prerequisites.push("BasicTech".to_string());
        node2.cost = 2000;

        manager.add_tech_node(node1);
        manager.add_tech_node(node2);
        manager.update_resources(5000);

        assert!(manager.can_research("BasicTech"));
        assert!(!manager.can_research("AdvancedTech"));

        manager.completed_research.insert("BasicTech".to_string());
        assert!(manager.can_research("AdvancedTech"));
    }

    #[test]
    fn test_tech_progression_research() {
        let mut manager = TechProgressionManager::default();

        manager.update_resources(5000);

        let available = manager.get_available_research();
        assert!(!available.is_empty());

        if let Some(tech) = available.first() {
            assert!(manager.start_research(tech.clone()).is_ok());
        }
    }

    #[test]
    fn test_tech_progression_strategy() {
        let mut manager = TechProgressionManager::default();

        manager.set_strategy(tech_progression::TechStrategy::Rush);
        assert_eq!(manager.strategy, tech_progression::TechStrategy::Rush);

        manager.set_strategy(tech_progression::TechStrategy::Economic);
        assert_eq!(manager.strategy, tech_progression::TechStrategy::Economic);
    }

    // Integration tests
    #[test]
    fn test_dozer_with_build_optimizer() {
        let mut dozer = DozerAIUpdate::new();
        let mut optimizer = BuildOrderOptimizer::new();

        optimizer.set_strategy_template("balanced");
        optimizer.update_resources(5000);

        if let Some(order) = optimizer.get_next_build() {
            dozer.set_build_target(order.building_type, [100.0, 100.0, 0.0]);
        }

        let mut context = AIUpdateContext::new(1, 0);
        assert!(dozer.update(&mut context).is_ok());
    }

    #[test]
    fn test_turret_with_target_prioritization() {
        let mut turret = TurretAIUpdate::new();
        let mut prioritization = TargetPrioritization::new();

        let targets: Vec<_> = (0..5).map(|i| {
            target_prioritization::PrioritizationTarget {
                object_id: i,
                position: [100.0 + i as f32 * 10.0, 100.0, 0.0],
                unit_type: "Infantry".to_string(),
                health_percentage: 1.0,
                is_attacking: true,
                distance_to_attacker: 50.0 + i as f32 * 10.0,
            }
        }).collect();

        let best_target = prioritization.select_best_target(&targets);
        assert!(best_target.is_some());

        let context = AIUpdateContext::new(1, 0);
        assert!(turret.init(&context).is_ok());
    }

    #[test]
    fn test_jet_with_threat_assessment() {
        let mut jet = JetAIUpdate::new();
        let mut threat_system = ThreatAssessmentSystem::new();

        jet.set_airfield(100);

        let threat = threat_assessment::ThreatInfo {
            threat_id: 200,
            threat_type: threat_assessment::ThreatType::Military,
            threat_level: threat_assessment::ThreatLevel::High,
            position: [300.0, 300.0, 0.0],
            severity: 0.8,
            detection_frame: 0,
            last_update_frame: 0,
            estimated_strength: 75.0,
            distance_to_base: 100.0,
        };

        threat_system.add_threat(threat);

        if let Some(priority_threat) = threat_system.get_highest_priority_threat() {
            jet.set_attack_target(priority_threat.threat_id);
        }

        let mut context = AIUpdateContext::new(1, 0);
        assert!(jet.update(&mut context).is_ok());
    }
}
