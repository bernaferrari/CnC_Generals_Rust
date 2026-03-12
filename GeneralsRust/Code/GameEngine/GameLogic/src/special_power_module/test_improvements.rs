//! Comprehensive tests for Special Power System improvements

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::common::*;
    use crate::effects::ObjectCreationList;
    use crate::helpers::TheObjectCreationListStore;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use std::sync::{Arc, RwLock};

    fn register_test_owner(owner_id: ObjectID) -> Arc<RwLock<Object>> {
        let owner = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
        OBJECT_REGISTRY.register_object(owner_id, &owner);
        owner
    }

    #[test]
    fn test_player_science_system() {
        // Initialize systems
        initialize_player_science();

        let manager = get_player_science_manager().expect("Manager should be initialized");
        let mut mgr = manager.write().unwrap();

        // Register player
        mgr.register_player(1);

        // Test science unlocking
        mgr.add_science(1, "SCIENCE_A10_STRIKE".into());
        mgr.add_science(1, "SCIENCE_SUPERWEAPON".into());

        let player = mgr.get_player(1).expect("Player should exist");
        assert!(player.has_science("SCIENCE_A10_STRIKE"));
        assert!(player.has_science("SCIENCE_SUPERWEAPON"));
        assert!(!player.has_science("SCIENCE_MISSING"));
    }

    #[test]
    fn test_player_money_system() {
        // Initialize systems
        initialize_player_money();

        let manager = get_player_money_manager().expect("Manager should be initialized");
        let mut mgr = manager.write().unwrap();

        // Register player with starting money
        mgr.register_player(1, 10000);

        // Test money operations
        assert!(mgr.can_afford(1, 5000));
        assert!(!mgr.can_afford(1, 15000));

        // Spend money
        assert!(mgr.spend_money(1, 3000, 0));
        assert_eq!(mgr.get_money(1), 7000);

        // Try to spend more than available
        assert!(!mgr.spend_money(1, 10000, 0));
        assert_eq!(mgr.get_money(1), 7000); // Should remain unchanged

        // Add money
        mgr.add_money(1, 5000, 0);
        assert_eq!(mgr.get_money(1), 12000);
    }

    #[test]
    fn test_player_rank_progression() {
        // Initialize systems
        initialize_player_science();

        let manager = get_player_science_manager().unwrap();
        let mut mgr = manager.write().unwrap();

        mgr.register_player(1);

        // Test rank progression through experience
        mgr.add_experience(1, 500);
        assert_eq!(mgr.get_player(1).unwrap().get_rank(), PlayerRank::Recruit);

        mgr.add_experience(1, 500);
        assert_eq!(mgr.get_player(1).unwrap().get_rank(), PlayerRank::Veteran);

        mgr.add_experience(1, 4000);
        assert_eq!(mgr.get_player(1).unwrap().get_rank(), PlayerRank::Elite);

        mgr.add_experience(1, 10000);
        assert_eq!(mgr.get_player(1).unwrap().get_rank(), PlayerRank::Heroic);
    }

    #[test]
    fn test_area_damage_falloff() {
        use area_damage::{AreaDamageConfig, DamageFalloff};

        // Test linear falloff
        let config = AreaDamageConfig {
            max_damage: 1000.0,
            radius: 100.0,
            min_damage: 0.0,
            falloff: DamageFalloff::Linear,
            damage_type: DamageTypeFlags::EXPLOSION,
            affects_friendlies: false,
            affects_buildings: true,
            affects_terrain: false,
        };

        assert_eq!(config.calculate_damage_at_distance(0.0), 1000.0);
        assert!((config.calculate_damage_at_distance(50.0) - 500.0).abs() < 0.1);
        assert_eq!(config.calculate_damage_at_distance(100.0), 0.0);

        // Test two-stage falloff
        let config2 = AreaDamageConfig {
            max_damage: 2000.0,
            radius: 200.0,
            min_damage: 0.0,
            falloff: DamageFalloff::TwoStage {
                inner_radius: 100.0,
            },
            damage_type: DamageTypeFlags::EXPLOSION,
            affects_friendlies: true,
            affects_buildings: true,
            affects_terrain: true,
        };

        assert_eq!(config2.calculate_damage_at_distance(50.0), 2000.0);
        assert_eq!(config2.calculate_damage_at_distance(100.0), 2000.0);
        assert!((config2.calculate_damage_at_distance(150.0) - 1000.0).abs() < 0.1);
    }

    #[test]
    fn test_special_power_cooldown_integration() {
        let data = SpecialPowerModuleData::new("TestPower".into(), SpecialPowerKind::OCL);
        let mut power = SpecialPowerModule::new(data);

        // Test initial state
        assert!(power.is_ready());
        assert!(!power.is_on_cooldown());
        assert_eq!(power.get_cooldown_progress(), 1.0);

        // Activate power
        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 50.0);
        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success());

        // Should be on cooldown now
        assert!(!power.is_ready());
        assert!(power.is_on_cooldown());

        // Update cooldown
        power.update(15.0);
        assert!(power.is_on_cooldown());

        power.update(15.0);
        assert!(power.is_ready());
    }

    #[test]
    fn test_fire_weapon_power_creation() {
        use fire_weapon_power::{FireWeaponPower, FireWeaponPowerData};

        let data = FireWeaponPowerData::new("ParticleCannon".into());
        let mut power = FireWeaponPower::new(data);

        power.set_owner(1);

        assert_eq!(power.get_data().power_kind, SpecialPowerKind::FireWeapon);
        assert!(power.get_data().is_superweapon());
        assert!(power.is_ready());
    }

    #[test]
    fn test_a10_strike_power() {
        use a10_strike_power::{A10StrikePower, A10StrikePowerData};

        TheObjectCreationListStore::register_object_creation_list(
            "SUPERWEAPON_A10ThunderboltMissileStrike1",
            ObjectCreationList::new(),
        );

        let data = A10StrikePowerData::new("A10Strike".into());
        let mut power = A10StrikePower::new(data);
        let owner_id = 9301;
        let _owner = register_test_owner(owner_id);
        power.set_owner_object_id(owner_id);

        assert_eq!(power.get_name(), "A10Strike");
        assert!(power.is_ready());

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 100.0);
        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success(), "activation failed: {:?}", result);

        assert!(power.is_on_cooldown());
        assert!(power.is_strike_active());
        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_nuclear_missile_power() {
        use nuclear_missile_power::{NuclearMissilePower, NuclearMissilePowerData};

        TheObjectCreationListStore::register_object_creation_list(
            "SUPERWEAPON_NeutronMissile",
            ObjectCreationList::new(),
        );

        let data = NuclearMissilePowerData::new("NuclearMissile".into());
        let owner_id = 9302;
        let _owner = register_test_owner(owner_id);
        let mut power = NuclearMissilePower::new(data, owner_id);

        assert_eq!(power.get_name(), "NuclearMissile");
        assert!(power.is_ready());
        assert!(power.get_data().is_superweapon());

        let targeting = TargetingInfo::new(Coord3D::new(500.0, 0.0, 500.0), 0.0, 250.0);
        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success(), "activation failed: {:?}", result);

        assert!(power.is_on_cooldown());
        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_money_transfer_cash_hack() {
        initialize_player_money();

        let manager = get_player_money_manager().unwrap();
        let mut mgr = manager.write().unwrap();

        // Register two players
        mgr.register_player(1, 10000);
        mgr.register_player(2, 5000);

        // Transfer money (simulate cash hack)
        assert!(mgr.transfer_money(1, 2, 3000, 0));

        assert_eq!(mgr.get_money(1), 7000);
        assert_eq!(mgr.get_money(2), 8000);
    }

    #[test]
    fn test_targeting_validation() {
        use targeting::{TargetValidation, TargetValidator, TargetingInfo};

        let targeting = TargetingInfo {
            position: Coord3D::new(100.0, 0.0, 100.0),
            target_object: None,
            range: 500.0,
            radius: 50.0,
            requires_los: false,
            min_range: 0.0,
            max_altitude: 1000.0,
            flags: SpecialPowerFlags::empty(),
        };

        let source = Coord3D::new(200.0, 0.0, 200.0);

        // Valid target
        let validation = TargetValidator::validate_target(&targeting, &source, None);
        assert!(validation.is_valid());

        // Out of range
        let far_source = Coord3D::new(700.0, 0.0, 700.0);
        let validation = TargetValidator::validate_target(&targeting, &far_source, None);
        assert_eq!(validation, TargetValidation::OutOfRange);
    }

    #[test]
    fn test_integration_context_creation() {
        use integration::SpecialPowerIntegrationContext;

        let context = SpecialPowerIntegrationContext::new();

        assert!(context.object_manager.is_none());
        assert!(context.ai_update.is_none());
        assert!(context.player.is_none());
    }
}
