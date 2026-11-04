//! CPP RiderChangeContain.cpp:182-238 onContaining: an owned-path rider
//! boards the bike and the matching RiderInfo is applied to the owner.
use super::*;
use crate::modules::ai_world::AiWorld;
use crate::common::ObjectStatusMaskType;
use crate::object::contain::rider_change_contain::{RiderChangeContainModuleData, RiderInfo};
use crate::weapon::WeaponSetType;

fn owned_bike_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeOwnedRiderBike".into());
    base.add_kind_of(KindOf::Vehicle);
    let mut data = RiderChangeContainModuleData::default();
    data.base.slot_capacity = 1;
    data.base.base.contain_max = 1;
    data.base.base.number_of_exit_paths = 0;
    data.base.base.allow_inside_kind_of = KindOf::Infantry.cpp_mask();
    data.riders[0] = RiderInfo {
        template_name: "NativeEnterInfantry".into(),
        model_condition_flag: crate::common::ModelConditionFlags::RIDER1,
        weapon_set_flag: WeaponSetType::WeaponRider1,
        object_status: ObjectStatusMaskType::RIDER1,
        ..Default::default()
    };
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "RiderChangeContain".into(),
            module_tag: "NativeOwnedBikeContain".into(),
            data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }],
    })
}

#[test]
fn rider_boards_rider_change_bike_on_owned_path_and_applies_rider_info() {
    if game_engine::common::common_services::try_common_services().is_none() {
        game_engine::common::common_services::install_common_services(Arc::new(
            game_engine::common::common_services::CommonServices::new(),
        ));
    }
    if crate::system::engine_stores::try_active_services().is_none() {
        crate::system::engine_stores::install_active(crate::system::engine_stores::new_for_world());
    }
    let mut game = NativeMatch::with_container(50.0, owned_bike_template());
    let rider_id = game.source.read().unwrap().get_id();
    let bike_id = game.target.read().unwrap().get_id();
    game.operate(117, |world| world.enter(rider_id, bike_id)).unwrap();
    let owner = game.target.read().unwrap();
    let rider = game.source.read().unwrap();
    assert_eq!(
        owner.get_contain().unwrap().lock().unwrap().get_contained_objects().as_ref(),
        &[rider.get_id()]
    );
    assert!(owner.get_status_bits().test(crate::common::ObjectStatusTypes::Rider1));
    assert!(owner.test_weapon_set_flag(WeaponSetType::WeaponRider1));
}
