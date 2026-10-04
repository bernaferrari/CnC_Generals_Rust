use super::*;
use crate::common::thing::module::BaseModuleData;

#[derive(Debug)]
struct Owner;
impl Thing for Owner {}

struct MarkerModule {
    data: Arc<dyn ModuleData>,
    marker: u32,
}

impl Module for MarkerModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.marker
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for MarkerModule {
    fn crc(&self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn create_module(_: Arc<dyn Thing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    Box::new(MarkerModule { data, marker: 1 })
}

fn create_other_module(_: Arc<dyn Thing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    Box::new(MarkerModule { data, marker: 2 })
}

fn create_data(_: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(BaseModuleData::new())
}

fn descriptor(name: &str, mask: ModuleInterfaceType) -> TemplateModuleDescriptor {
    TemplateModuleDescriptor {
        name: name.into(),
        module_tag: "ModuleTag_Ownership".into(),
        interface_mask: mask,
        inheritable: false,
        overrideable_by_like_kind: false,
        copied_from_default: false,
    }
}

#[test]
fn moving_factory_between_name_key_namespaces_preserves_module_identity() {
    let factory = std::thread::spawn(|| {
        NameKeyGenerator::init();
        for index in 0..100 {
            NameKeyGenerator::name_to_key(&format!("Unrelated{index}"));
        }
        let mut factory = ModuleFactory::new();
        let mut descriptors = TemplateModuleDescriptorSet::default();
        descriptors
            .behavior
            .push(descriptor("OwnershipModule", ModuleInterfaceType::DAMAGE));
        descriptors
            .draw
            .push(descriptor("OwnershipModule", ModuleInterfaceType::DRAW));
        factory.register_descriptor_set(&descriptors);
        factory.add_module_internal(
            Some(create_module),
            Some(create_data),
            ModuleType::Behavior,
            "OwnershipModule",
            ModuleInterfaceType::DAMAGE,
        );
        factory
    })
    .join()
    .unwrap();

    std::thread::spawn(move || {
        NameKeyGenerator::init();
        // This thread's numeric key 1 names a different string from the creating thread.
        NameKeyGenerator::name_to_key("ReceivingThread");
        let mut factory = factory;
        assert_eq!(
            factory.find_module_interface_mask("OwnershipModule", ModuleType::Behavior),
            ModuleInterfaceType::DAMAGE
        );
        assert_eq!(
            factory.find_module_interface_mask("OwnershipModule", ModuleType::Draw),
            ModuleInterfaceType::DRAW
        );
        assert_eq!(
            factory.find_module_interface_mask("OwnershipModule", ModuleType::ClientUpdate),
            ModuleInterfaceType::NONE
        );
        assert_eq!(
            factory.find_module_interface_mask("ownershipmodule", ModuleType::Behavior),
            ModuleInterfaceType::NONE
        );
        assert_eq!(
            factory.find_module_interface_mask("NotRegistered", ModuleType::Behavior),
            ModuleInterfaceType::NONE
        );
        assert_eq!(
            factory
                .descriptor_for(ModuleType::Behavior, "OwnershipModule")
                .unwrap()
                .name
                .as_str(),
            "OwnershipModule"
        );
        let tag_key = NameKeyGenerator::name_to_key("ModuleTag_Ownership");
        let data = factory
            .new_module_data_from_ini(
                None,
                "OwnershipModule",
                ModuleType::Behavior,
                "ModuleTag_Ownership",
            )
            .unwrap();
        assert_eq!(data.get_module_tag_name_key(), tag_key);
        let module = factory
            .new_module(
                Arc::new(Owner),
                "OwnershipModule",
                Arc::clone(&data),
                ModuleType::Behavior,
            )
            .unwrap();
        assert!(
            std::ptr::eq(module.get_module_data(), data.as_ref()),
            "factory creation must keep the supplied data instance"
        );
    })
    .join()
    .unwrap();
}

#[test]
fn constructing_factory_does_not_consume_ambient_name_keys() {
    std::thread::spawn(|| {
        NameKeyGenerator::init();
        assert_eq!(NameKeyGenerator::name_to_key("Before"), 1);
        let factory = ModuleFactory::new();
        assert_eq!(NameKeyGenerator::name_to_key("After"), 2);
        assert_eq!(
            factory.find_module_interface_mask("ActiveBody", ModuleType::Behavior),
            ModuleInterfaceType::BODY
        );
        assert_eq!(NameKeyGenerator::name_to_key("AfterLookup"), 3);
    })
    .join()
    .unwrap();
}

fn created_marker(factory: &ModuleFactory) -> u32 {
    factory
        .new_module(
            Arc::new(Owner),
            "OwnedBehavior",
            Arc::new(BaseModuleData::new()),
            ModuleType::Behavior,
        )
        .unwrap()
        .get_module_name_key()
}

#[test]
fn owned_overrides_keep_factory_precedence_and_isolation() {
    std::thread::spawn(|| {
        let mut first = ModuleFactory::new();
        let mut second = ModuleFactory::new();
        first
            .register_override(
                "OwnedBehavior",
                ModuleType::Behavior,
                create_module,
                create_data,
            )
            .unwrap();
        second
            .register_override(
                "OwnedBehavior",
                ModuleType::Behavior,
                create_other_module,
                create_data,
            )
            .unwrap();
        let mut descriptors = TemplateModuleDescriptorSet::default();
        descriptors
            .behavior
            .push(descriptor("OwnedBehavior", ModuleInterfaceType::DAMAGE));
        for factory in [&mut first, &mut second] {
            factory.register_descriptor_set(&descriptors);
            // Explicit overrides retain precedence over a later template constructor.
            factory.add_module_internal(
                Some(create_other_module),
                Some(create_data),
                ModuleType::Behavior,
                "OwnedBehavior",
                ModuleInterfaceType::DAMAGE,
            );
        }
        assert_eq!(created_marker(&first), 1);
        assert_eq!(created_marker(&second), 2);
        // Ambient startup changes cannot select constructors in either owned factory.
        register_module_override(
            "OwnedBehavior",
            ModuleType::Behavior,
            create_other_module,
            create_data,
        )
        .unwrap();
        NameKeyGenerator::init();
        first.init().unwrap();
        first.reset().unwrap();
        assert_eq!(created_marker(&first), 1);
        assert_eq!(created_marker(&second), 2);
        // A late override updates only the supplied factory, preserving its mask/order.
        second
            .register_override(
                "OwnedBehavior",
                ModuleType::Behavior,
                create_module,
                create_data,
            )
            .unwrap();
        assert_eq!(created_marker(&second), 1);
        assert_eq!(
            second.find_module_interface_mask("OwnedBehavior", ModuleType::Behavior),
            ModuleInterfaceType::DAMAGE
        );
        assert_eq!(
            second
                .descriptors_for_type(ModuleType::Behavior)
                .last()
                .unwrap()
                .name
                .as_str(),
            "OwnedBehavior"
        );
        assert!(
            second
                .register_override("", ModuleType::Behavior, create_module, create_data)
                .is_err()
        );
        clear_module_overrides_for_test();
    })
    .join()
    .unwrap();
}

#[test]
fn construction_leaves_pending_startup_and_overrides_for_explicit_import() {
    std::thread::spawn(|| {
        clear_module_overrides_for_test();
        clear_pending_descriptors_for_test();
        register_module_override(
            "OwnedBehavior",
            ModuleType::Behavior,
            create_module,
            create_data,
        )
        .unwrap();
        enqueue_pending_descriptor(
            ModuleType::Behavior,
            &descriptor("OwnedBehavior", ModuleInterfaceType::DAMAGE),
        );
        let mut factory = ModuleFactory::new();
        assert!(
            factory
                .descriptor_for(ModuleType::Behavior, "OwnedBehavior")
                .is_none()
        );
        assert_eq!(PENDING_DESCRIPTORS.with_borrow(Vec::len), 1);
        apply_registered_module_overrides(&mut factory).unwrap();
        factory.absorb_pending_descriptors();
        assert_eq!(created_marker(&factory), 1);
        assert_eq!(PENDING_DESCRIPTORS.with_borrow(Vec::len), 0);
        // The imported choices belong to the factory after the startup table is cleared.
        clear_module_overrides_for_test();
        factory.add_module_internal(
            None,
            None,
            ModuleType::Behavior,
            "OwnedBehavior",
            ModuleInterfaceType::DAMAGE,
        );
        assert_eq!(created_marker(&factory), 1);
    })
    .join()
    .unwrap();
}

// C++ ModuleFactory.cpp:705-707 leaves registered ModuleData untouched during
// loadPostProcess, including when the factory is the data's only owner.
#[derive(Debug)]
struct PostLoadErrorData {
    tag_key: NameKeyType,
    payload: u32,
}

impl ModuleData for PostLoadErrorData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.tag_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.tag_key
    }
}

impl Snapshotable for PostLoadErrorData {
    fn crc(&self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.payload = 99;
        Err("ModuleFactory must not visit this data hook".into())
    }
}

fn create_post_load_error_data(_: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(PostLoadErrorData {
        tag_key: 0,
        payload: 17,
    })
}

fn post_load_error_factory() -> ModuleFactory {
    let mut factory = ModuleFactory::new();
    factory.add_module_internal(
        Some(create_module),
        Some(create_post_load_error_data),
        ModuleType::Behavior,
        "PostLoadErrorProbe",
        ModuleInterfaceType::DAMAGE,
    );
    factory
}

fn new_post_load_error_data(factory: &mut ModuleFactory) -> Arc<dyn ModuleData> {
    factory
        .new_module_data_from_ini(
            None,
            "PostLoadErrorProbe",
            ModuleType::Behavior,
            "ModuleTag_PostLoadErrorProbe",
        )
        .expect("actual registered create_data_proc constructs the data")
}

#[test]
fn factory_post_load_is_inert_when_registered_data_has_one_owner() {
    let mut factory = post_load_error_factory();
    let data = new_post_load_error_data(&mut factory);
    let tag_key = data.get_module_tag_name_key();
    assert_ne!(tag_key, 0);
    assert_eq!(Arc::strong_count(&data), 2);
    // Store identity only: a retained Weak would also prevent Arc::get_mut
    // from entering the erroneous OLD unique-owner branch.
    let identity = Arc::as_ptr(&data);
    drop(data);
    assert_eq!(Arc::strong_count(&factory.module_data_list[0]), 1);
    assert_eq!(Arc::weak_count(&factory.module_data_list[0]), 0);

    assert_eq!(factory.load_post_process(), Ok(()));

    let data = &factory.module_data_list[0];
    assert!(std::ptr::eq(identity, Arc::as_ptr(data)));
    assert_eq!(data.get_module_tag_name_key(), tag_key);
    assert_eq!(
        data.as_ref()
            .as_any()
            .downcast_ref::<PostLoadErrorData>()
            .unwrap()
            .payload,
        17,
        "the registered data's error-producing hook must not run"
    );
}

#[test]
fn factory_post_load_preserves_aliased_registered_data_identity_and_payload() {
    let mut factory = post_load_error_factory();
    let data = new_post_load_error_data(&mut factory);
    let alias = Arc::clone(&data);
    let tag_key = data.get_module_tag_name_key();
    assert_eq!(Arc::strong_count(&data), 3);

    assert_eq!(factory.load_post_process(), Ok(()));

    assert_eq!(Arc::strong_count(&data), 3);
    assert!(Arc::ptr_eq(&data, &alias));
    assert!(Arc::ptr_eq(&alias, &factory.module_data_list[0]));
    assert_eq!(alias.get_module_tag_name_key(), tag_key);
    assert_eq!(
        alias
            .as_ref()
            .as_any()
            .downcast_ref::<PostLoadErrorData>()
            .unwrap()
            .payload,
        17
    );
    drop(alias);
    drop(data);
    assert_eq!(Arc::strong_count(&factory.module_data_list[0]), 1);
}
