//! Actual compatibility trait query, CPP GarrisonContain.cpp529-557.
use super::tests::{
    authored_capacity_one, cleanup_objects, infantry, isolated_garrison_case, owned_object,
    reset_players,
};
use super::*;
use crate::common::ThingTemplate;

struct AuthoredBuildingTemplate {
    base: crate::common::DefaultThingTemplate,
    modules: Vec<crate::common::TemplateModuleInfo>,
}
impl std::fmt::Debug for AuthoredBuildingTemplate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthoredBuildingTemplate")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}
impl crate::common::ThingTemplate for AuthoredBuildingTemplate {
    fn get_name(&self) -> &crate::common::AsciiString {
        self.base.get_name()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.base.is_kind_of(kind)
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        self.base.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> f32 {
        self.base.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.base.calc_shroud_clearing_range()
    }
    fn get_behavior_module_info(&self) -> &[crate::common::TemplateModuleInfo] {
        &self.modules
    }
}

pub(super) fn building_template(name: &str) -> Arc<dyn crate::common::ThingTemplate> {
    let mut base = crate::common::DefaultThingTemplate::new(name.into());
    base.add_kind_of(KindOf::Structure);
    let mut data = crate::object::body::active_body::ActiveBodyModuleData::default();
    data.max_health = 100.0;
    data.initial_health = 100.0;
    Arc::new(AuthoredBuildingTemplate {
        base,
        modules: vec![crate::common::TemplateModuleInfo {
            name: "ActiveBody".into(),
            module_tag: "GarrisonBuildingBody".into(),
            data: Arc::new(data),
            interface_mask: game_engine::common::thing::module::ModuleInterfaceType::BODY,
        }],
    })
}

#[test]
fn trait_query_returns_real_eligibility_and_capacity_without_changing_roster() {
    if !isolated_garrison_case(concat!(
        module_path!(),
        "::trait_query_returns_real_eligibility_and_capacity_without_changing_roster"
    )) {
        return;
    }
    reset_players();
    let owner = owned_object("QueryGarrison", 96101, 0);
    let passenger = infantry("QueryPassenger", 96102, 0);
    let waiting = infantry("QueryWaiting", 96103, 0);
    let default = GarrisonContain::new(
        Arc::downgrade(&owner),
        &GarrisonContainModuleData::default(),
    )
    .unwrap();
    assert!(
        !ContainModuleInterface::can_contain(&default, 96102),
        "CPP strict capacity does not treat unknown -1 as unlimited for Garrison"
    );
    let mut contain =
        GarrisonContain::new(Arc::downgrade(&owner), &authored_capacity_one(false)).unwrap();
    assert!(ContainModuleInterface::can_contain(&contain, 96102));
    assert!(ContainModuleInterface::can_contain(&contain, 96103));
    assert!(!ContainModuleInterface::can_contain(&contain, INVALID_ID));
    assert!(!ContainModuleInterface::can_contain(&contain, 96199));
    {
        let _held_owner = owner.write().unwrap();
        assert!(
            !ContainModuleInterface::can_contain(&contain, 96101),
            "self rejection precedes registry/owner borrowing"
        );
    }
    assert_eq!(ContainModuleInterface::get_contained_count(&contain), 0);
    assert!(ContainModuleInterface::get_contained_objects(&contain).is_empty());
    for object in [&passenger, &waiting] {
        let object = object.read().unwrap();
        assert!(object.get_contained_by().is_none());
        assert!(!object.is_disabled_by_type(DisabledType::Held));
    }
    ContainModuleInterface::contain_object(&mut contain, 96102).unwrap();
    assert!(
        !ContainModuleInterface::can_contain(&contain, 96103),
        "actual authored one-slot garrison is full"
    );
    assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);
    assert_eq!(
        ContainModuleInterface::get_contained_objects(&contain).as_ref(),
        &[96102]
    );
    assert!(waiting.read().unwrap().get_contained_by().is_none());
    ContainModuleInterface::release_object(&mut contain, 96102).unwrap();
    assert!(ContainModuleInterface::can_contain(&contain, 96103));
    assert_eq!(ContainModuleInterface::get_contained_count(&contain), 0);
    cleanup_objects(&[96101, 96102, 96103]);
}
