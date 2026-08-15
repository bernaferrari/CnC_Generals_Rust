//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Collide-module factory wrappers (FireWeapon/Squish/ShroudCrate).
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

#[derive(Clone)]
struct SharedCollideModule<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> SharedCollideModule<T> {
    fn new(inner: Arc<Mutex<T>>) -> Self {
        Self { inner }
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, T>, CollisionError> {
        self.inner.lock().map_err(|_| {
            CollisionError::InvalidObject("SharedCollideModule lock poisoned".to_string())
        })
    }
}

impl<T> CollideModuleTrait for SharedCollideModule<T>
where
    T: CollideModuleTrait + Send + Sync + 'static,
{
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        loc: &Coord3D,
        normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        let mut inner = self.lock_inner()?;
        inner.on_collide(other, loc, normal)
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.lock_inner()
            .map(|inner| inner.would_like_to_collide_with(other))
            .unwrap_or(false)
    }

    fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_hijacked_vehicle_crate_collide())
            .unwrap_or(false)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_sabotage_building_crate_collide())
            .unwrap_or(false)
    }

    fn is_car_bomb_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_car_bomb_crate_collide())
            .unwrap_or(false)
    }

    fn is_railroad(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_railroad())
            .unwrap_or(false)
    }

    fn is_salvage_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_salvage_crate_collide())
            .unwrap_or(false)
    }
}

#[derive(Debug)]
struct FireWeaponCollideModule {
    module_name_key: NameKeyType,
    data: Arc<FireWeaponCollideModuleData>,
    collide: Arc<Mutex<FireWeaponCollide>>,
    object_id: ObjectID,
}

impl FireWeaponCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<FireWeaponCollideModuleData>,
        collide: FireWeaponCollide,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }

    fn register_collide_module(&self) -> Result<(), CollisionError> {
        COLLISION_MANAGER.register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        )
    }
}

impl Module for FireWeaponCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        if let Err(err) = self.register_collide_module() {
            warn!(
                "Failed to register FireWeaponCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl Snapshotable for FireWeaponCollideModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.load_post_process()
    }
}

fn fire_weapon_collide_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireWeaponCollideModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn fire_weapon_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_data_arc =
        cloned_module_data::<FireWeaponCollideModuleData>("FireWeaponCollide", &module_data);
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let collide = FireWeaponCollide::new(object_id, module_data_arc.clone())
        .expect("FireWeaponCollide::new should not fail during module construction");
    let module_name_key = NameKeyGenerator::name_to_key("FireWeaponCollide");
    Box::new(FireWeaponCollideModule::new(
        module_name_key,
        module_data_arc,
        collide,
        object_id,
    ))
}

#[derive(Debug)]
struct SquishCollideModule {
    module_name_key: NameKeyType,
    data: Arc<SquishCollideModuleData>,
    collide: Arc<Mutex<SquishCollide>>,
    object_id: ObjectID,
}

impl SquishCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<SquishCollideModuleData>,
        collide: SquishCollide,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }
}

impl Module for SquishCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn get_interface_mask(&self) -> ModuleInterfaceType {
        ModuleInterfaceType::COLLIDE
    }

    fn on_object_created(&mut self) {
        if let Err(err) = COLLISION_MANAGER.register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        ) {
            warn!(
                "Failed to register SquishCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl Snapshotable for SquishCollideModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(collide) = self.collide.lock() {
            collide.crc(xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut collide) = self.collide.lock() {
            collide.xfer(xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(mut collide) = self.collide.lock() {
            collide.load_post_process()
        } else {
            Ok(())
        }
    }
}

fn squish_collide_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(SquishCollideModuleData::default())
}

fn squish_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_data_arc =
        cloned_module_data::<SquishCollideModuleData>("SquishCollide", &module_data);
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let collide = SquishCollide::new(object_id, Arc::clone(&module_data_arc));
    let module_name_key = NameKeyGenerator::name_to_key("SquishCollide");
    Box::new(SquishCollideModule::new(
        module_name_key,
        module_data_arc,
        collide,
        object_id,
    ))
}

#[derive(Debug)]
struct ShroudCrateCollideModule {
    module_name_key: NameKeyType,
    data: Arc<ShroudCrateCollideModuleData>,
    collide: Arc<Mutex<ShroudCrateCollide>>,
    object_id: ObjectID,
}

impl ShroudCrateCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ShroudCrateCollideModuleData>,
        collide: ShroudCrateCollide,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }
}

impl Module for ShroudCrateCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn get_interface_mask(&self) -> ModuleInterfaceType {
        ModuleInterfaceType::COLLIDE
    }

    fn on_object_created(&mut self) {
        if let Err(err) = COLLISION_MANAGER.register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        ) {
            warn!(
                "Failed to register ShroudCrateCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl Snapshotable for ShroudCrateCollideModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let collide = self
            .collide
            .lock()
            .map_err(|_| "ShroudCrateCollide lock poisoned".to_string())?;
        collide.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "ShroudCrateCollide lock poisoned".to_string())?;
        collide.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "ShroudCrateCollide lock poisoned".to_string())?;
        collide.load_post_process()
    }
}

fn shroud_crate_collide_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ShroudCrateCollideModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ShroudCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn shroud_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let config = module_data
        .get_shroud_crate_collide_config()
        .expect("ShroudCrateCollideModuleData expected");
    let module_data_arc = Arc::new(ShroudCrateCollideModuleData::from_config(
        config,
        module_data.get_module_tag_name_key(),
    ));
    let object_id = thing
        .as_object()
        .map(|object| object.get_object_id())
        .unwrap_or_default();

    let collide = ShroudCrateCollide::new(object_id, module_data_arc.crate_data());
    let module_name_key = NameKeyGenerator::name_to_key("ShroudCrateCollide");
    Box::new(ShroudCrateCollideModule::new(
        module_name_key,
        module_data_arc,
        collide,
        object_id,
    ))
}

