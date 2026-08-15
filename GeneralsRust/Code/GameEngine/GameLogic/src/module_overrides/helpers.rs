//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Shared helpers and contain binding adapters.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn resolve_owner_info(thing: &Arc<dyn ModuleThing>) -> (ObjectID, Coord3D) {
    let owner_id = thing
        .as_object()
        .map(ModuleObjectTrait::get_object_id)
        .unwrap_or(INVALID_ID);

    let owner_pos = TheGameLogic::find_object_by_id(owner_id)
        .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
        .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

    (owner_id, owner_pos)
}

fn cloned_module_data<TData>(module_name: &str, module_data: &Arc<dyn ModuleData>) -> Arc<TData>
where
    TData: ModuleData + Clone + 'static,
{
    Arc::new(
        module_data
            .downcast_ref::<TData>()
            .unwrap_or_else(|| panic!("{module_name} module data type expected"))
            .clone(),
    )
}

fn resolve_drawable_id(thing: &Arc<dyn ModuleThing>) -> u32 {
    thing
        .as_drawable()
        .map(ModuleDrawableTrait::get_drawable_id)
        .unwrap_or(INVALID_ID)
}

fn module_data_proc_or(
    module_name: &str,
    module_type: ModuleType,
    fallback: NewModuleDataProc,
) -> NewModuleDataProc {
    if let Ok(factory_guard) = get_module_factory() {
        if let Some(factory) = factory_guard.as_ref() {
            if let Some(template) = factory.find_module_template(module_name, module_type) {
                if let Some(create_data_proc) = template.create_data_proc {
                    return create_data_proc;
                }
            }
        }
    }
    fallback
}

fn attach_body_to_object(object_id: ObjectID, body: Arc<Mutex<dyn BodyModuleInterface>>) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_body_module(Some(body));
        }
    }
}

fn attach_contain_to_object(object_id: ObjectID, contain: Arc<Mutex<dyn ContainModuleInterface>>) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_contain(Some(contain));
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ContainModuleDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
    base: BaseModuleData,
    contain: T,
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> ContainModuleDataAdapter<T> {
    fn new(contain: T) -> Self {
        Self {
            base: BaseModuleData::new(),
            contain,
        }
    }

    pub(crate) fn contain_data(&self) -> &T {
        &self.contain
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> Snapshotable
    for ContainModuleDataAdapter<T>
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> ModuleData
    for ContainModuleDataAdapter<T>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> crate::common::types::ModuleData
    for ContainModuleDataAdapter<T>
{
}

fn contain_data_ref<T: Clone + Send + Sync + std::fmt::Debug + 'static>(
    module_data: &dyn ModuleData,
) -> Option<T> {
    module_data
        .downcast_ref::<ContainModuleDataAdapter<T>>()
        .map(ContainModuleDataAdapter::contain_data)
        .cloned()
}

fn expect_contain_data<T: Clone + Send + Sync + std::fmt::Debug + Default + 'static>(
    module_data: &dyn ModuleData,
    module_name: &str,
) -> T {
    contain_data_ref::<T>(module_data).unwrap_or_else(|| {
        warn!("{module_name} module data adapter missing; using default data");
        T::default()
    })
}

#[derive(Debug)]
pub(crate) struct ContainBindingModule {
    module_name_key: NameKeyType,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
    owner_id: ObjectID,
}

impl ContainBindingModule {
    fn new(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        contain: Arc<Mutex<dyn ContainModuleInterface>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            module_data,
            contain,
            owner_id,
        }
    }
}

impl Module for ContainBindingModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        attach_contain_to_object(self.owner_id, Arc::clone(&self.contain));
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_owner_created() {
                warn!(
                    "Contain module on_owner_created failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }
}

impl Snapshotable for ContainBindingModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(contain) = self.contain.lock() {
            contain.crc(xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut contain) = self.contain.lock() {
            contain.xfer(xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(mut contain) = self.contain.lock() {
            contain.load_post_process()
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
struct CaveContainBindingModule {
    module_name_key: NameKeyType,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<CaveContain>>,
    owner_id: ObjectID,
}

impl CaveContainBindingModule {
    fn new(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        contain: Arc<Mutex<CaveContain>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            module_data,
            contain,
            owner_id,
        }
    }
}

impl CreateInterface for CaveContainBindingModule {
    fn on_create(&self) {
        let Some(cave_data) = contain_data_ref::<CaveContainModuleData>(self.module_data.as_ref())
        else {
            return;
        };
        if let Ok(mut contain_guard) = self.contain.lock() {
            let _ = contain_guard.on_create(cave_data);
        }
    }

    fn on_build_complete(&self) {
        if let Ok(mut contain_guard) = self.contain.lock() {
            let _ = contain_guard.on_build_complete();
        }
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.contain
            .lock()
            .map(|guard| guard.should_do_on_build_complete())
            .unwrap_or(false)
    }
}

impl Module for CaveContainBindingModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::clone(&self.contain);
        attach_contain_to_object(self.owner_id, contain);
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_owner_created() {
                warn!(
                    "Cave contain on_owner_created failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }

    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(self)
    }
}

impl Snapshotable for CaveContainBindingModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(contain) = self.contain.lock() {
            contain.crc(xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut contain) = self.contain.lock() {
            contain.xfer(xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(mut contain) = self.contain.lock() {
            contain.load_post_process()
        } else {
            Ok(())
        }
    }
}

fn make_contain_binding_module(
    module_name: &str,
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key(module_name);
    let (owner_id, _) = resolve_owner_info(&thing);
    Box::new(ContainBindingModule::new(
        module_name_key,
        module_data,
        contain,
        owner_id,
    ))
}

fn make_owner_weak(owner_id: ObjectID) -> Weak<RwLock<crate::object::Object>> {
    TheGameLogic::find_object_by_id(owner_id)
        .map(|arc| Arc::downgrade(&arc))
        .unwrap_or_else(Weak::new)
}
