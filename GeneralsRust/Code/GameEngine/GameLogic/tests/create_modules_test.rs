//! Integration tests for Create modules

#[cfg(test)]
mod create_module_tests {
    use game_engine::common::thing::module::Thing as ThingTrait;
    use gamelogic::object::create::*;
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestThing;

    impl ThingTrait for TestThing {}

    fn test_thing() -> Arc<dyn ThingTrait> {
        Arc::new(TestThing)
    }

    #[test]
    fn test_preorder_create_module_exists() {
        let module = PreorderCreate::new(test_thing());
        assert!(
            module.should_do_on_build_complete(),
            "fresh PreorderCreate must still run on_build_complete"
        );
        module.on_create();
    }

    #[test]
    fn test_special_power_create_module_exists() {
        let module = SpecialPowerCreate::new(test_thing());
        assert!(module.should_do_on_build_complete());
        module.on_create();
    }

    #[test]
    fn test_veterancy_gain_create_module_exists() {
        let data = Arc::new(VeterancyGainCreateData::default());
        assert_eq!(
            data.starting_level,
            gamelogic::common::VeterancyLevel::Regular
        );
        let module = VeterancyGainCreate::new(test_thing(), data);
        module.on_create();
    }

    #[test]
    fn test_grant_upgrade_create_module_exists() {
        let data = Arc::new(GrantUpgradeCreateData::default());
        assert!(data.upgrade_name.is_empty());
        let module = GrantUpgradeCreate::new(test_thing(), data);
        assert!(module.should_do_on_build_complete());
        module.on_create();
    }

    #[test]
    fn test_supply_warehouse_create_module_exists() {
        let module = SupplyWarehouseCreate::new(test_thing());
        assert!(module.should_do_on_build_complete());
        module.on_create();
    }

    #[test]
    fn test_supply_center_create_module_exists() {
        let module = SupplyCenterCreate::new(test_thing());
        assert!(module.should_do_on_build_complete());
        module.on_create();
    }

    #[test]
    fn test_lock_weapon_create_module_exists() {
        let data = Arc::new(LockWeaponCreateData::default());
        assert_eq!(
            data.slot_to_lock,
            gamelogic::weapon::WeaponSlotType::Primary
        );
        let module = LockWeaponCreate::new(test_thing(), data);
        assert!(module.should_do_on_build_complete());
        module.on_create();
    }

    #[test]
    fn test_eva_announce_client_create_module_exists() {
        let data = Arc::new(EvaAnnounceClientCreateData::default());
        assert!(data.announce_event.is_none());
        assert!(!data.enemy_only);
        assert!(!data.ally_only);
        assert!(!data.owner_only);
        let module = EvaAnnounceClientCreate::new(test_thing(), data);
        module.on_create();
    }

    #[test]
    fn test_all_modules_have_default_trait() {
        let data_preorder = PreorderCreateData::default();
        let data_sp = SpecialPowerCreateData::default();
        let data_vet = VeterancyGainCreateData::default();
        let data_grant = GrantUpgradeCreateData::default();
        let data_wh = SupplyWarehouseCreateData::default();
        let data_sc = SupplyCenterCreateData::default();
        let data_lock = LockWeaponCreateData::default();
        let data_eva = EvaAnnounceClientCreateData::default();

        let preorder = PreorderCreate::new(test_thing());
        let special = SpecialPowerCreate::new(test_thing());
        let veterancy = VeterancyGainCreate::new(test_thing(), Arc::new(data_vet));
        let grant = GrantUpgradeCreate::new(test_thing(), Arc::new(data_grant));
        let warehouse = SupplyWarehouseCreate::new(test_thing());
        let center = SupplyCenterCreate::new(test_thing());
        let lock = LockWeaponCreate::new(test_thing(), Arc::new(data_lock));
        let eva = EvaAnnounceClientCreate::new(test_thing(), Arc::new(data_eva));

        assert!(preorder.should_do_on_build_complete());
        assert!(special.should_do_on_build_complete());
        assert!(grant.should_do_on_build_complete());
        assert!(warehouse.should_do_on_build_complete());
        assert!(center.should_do_on_build_complete());
        assert!(lock.should_do_on_build_complete());
        preorder.on_create();
        special.on_create();
        veterancy.on_create();
        grant.on_create();
        warehouse.on_create();
        center.on_create();
        lock.on_create();
        eva.on_create();
        let _ = (data_preorder, data_sp, data_wh, data_sc);
    }

    #[test]
    fn test_create_module_interface_trait_exists() {
        fn takes_create_interface<T: CreateModuleInterface>(module: &T) {
            assert!(module.should_do_on_build_complete());
            module.on_create();
        }
        takes_create_interface(&PreorderCreate::new(test_thing()));
        takes_create_interface(&SpecialPowerCreate::new(test_thing()));
        takes_create_interface(&SupplyWarehouseCreate::new(test_thing()));
        takes_create_interface(&SupplyCenterCreate::new(test_thing()));
    }
}
