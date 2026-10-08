use super::*;
use crate::object::draw::w3d_supply_draw::{W3DSupplyDraw, W3DSupplyDrawModuleData};

#[test]
fn installed_supply_draw_forwards_the_driving_drawable() {
    const CHILD: &str = "GENERALS_SUPPLY_DRAW_OWNER_REENTRY_CHILD";
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let module = module_path!().strip_prefix(prefix).unwrap();
    let test_name = format!("{module}::installed_supply_draw_forwards_the_driving_drawable");
    if let crate::test_process::TestProcess::ParentVerified =
        crate::test_process::run_bounded(&test_name, CHILD)
    {
        return;
    }
    let _serial = crate::test_sync::lock();
    const ID: ObjectID = 0xA77A_10F3;
    let mut data = W3DSupplyDrawModuleData::new();
    data.base.attach_to_drawable_bone = AsciiString::from("attach");
    let mut supply = W3DSupplyDraw::new(data.clone());
    supply.bind_owner_id(ID);
    let mut drawable = Drawable::new(ID, ID, "SupplyRenderOwner".into(), DrawableType::Static);
    let entry = drawable.add_module(
        ModuleInterfaceType::DRAW,
        "W3DSupplyDraw".into(),
        "SupplyRenderOwner".into(),
        Arc::new(data),
        Box::new(supply),
    );
    let drawable = Arc::new(RwLock::new(drawable));
    let mut owner = crate::object::Object::new_test(ID, 100.0);
    owner.set_drawable(Some(Arc::clone(&drawable)));
    let owner = Arc::new(RwLock::new(owner));
    drawable.write().unwrap().bind_object_ref(&owner);
    OBJECT_REGISTRY.register_object(ID, &owner);
    let _registered = Registered([ID]);

    eprintln!("render child: before actual installed W3DSupplyDraw under Drawable writer");
    crate::drawable::Drawable::draw(&mut *drawable.write().unwrap(), None);
    entry.with_module(|module| {
        let supply = (module as &mut dyn std::any::Any)
            .downcast_mut::<W3DSupplyDraw>()
            .unwrap();
        assert_eq!(supply.total_bones(), -1);
    });
}
