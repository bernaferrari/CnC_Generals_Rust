//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Body-module factory wrappers (Inactive/Active/Structure/Highlander/Immortal/Hive/Undead).
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

#[derive(Debug, Clone)]
struct InactiveBodyModuleData {
    base: BodyModuleData,
}

impl Default for InactiveBodyModuleData {
    fn default() -> Self {
        Self {
            base: BodyModuleData::default(),
        }
    }
}

crate::impl_legacy_module_data_via_base!(InactiveBodyModuleData, base);

impl Snapshotable for InactiveBodyModuleData {
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

#[derive(Debug)]
struct InactiveBodyModule {
    module_name_key: NameKeyType,
    data: Arc<InactiveBodyModuleData>,
    body: Arc<Mutex<InactiveBody>>,
    owner_id: ObjectID,
}

impl InactiveBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<InactiveBodyModuleData>,
        owner_id: ObjectID,
    ) -> Self {
        let body = Arc::new(Mutex::new(InactiveBody::new_with_owner(
            data.base.clone(),
            owner_id,
        )));
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for InactiveBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for InactiveBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(body) = self.body.lock() {
            body.crc(xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut body) = self.body.lock() {
            body.xfer(xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(mut body) = self.body.lock() {
            body.load_post_process()
        } else {
            Ok(())
        }
    }
}

fn inactive_body_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(InactiveBodyModuleData::default())
}

fn inactive_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .downcast_ref::<InactiveBodyModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("InactiveBodyModuleData expected, using default fallback");
            InactiveBodyModuleData::default()
        });
    let module_data_arc = Arc::new(typed_data);
    let module_name_key = NameKeyGenerator::name_to_key("InactiveBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let module = InactiveBodyModule::new(module_name_key, module_data_arc, owner_id);

    Box::new(module)
}

#[derive(Debug)]
struct ActiveBodyModule {
    module_name_key: NameKeyType,
    data: Arc<ActiveBodyModuleData>,
    body: Arc<Mutex<ActiveBody>>,
    owner_id: ObjectID,
}

impl ActiveBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ActiveBodyModuleData>,
        body: Arc<Mutex<ActiveBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for ActiveBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for ActiveBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self.body.lock().map_err(|_| "ActiveBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ActiveBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ActiveBody lock poisoned")?;
        body.load_post_process()
    }
}

fn active_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ActiveBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ActiveBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn active_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<ActiveBodyModuleData>("ActiveBody", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("ActiveBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(ActiveBody::new_with_owner(
        data_arc.as_ref().clone(),
        owner_id,
    )));
    Box::new(ActiveBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct StructureBodyModule {
    module_name_key: NameKeyType,
    data: Arc<StructureBodyModuleData>,
    body: Arc<Mutex<StructureBody>>,
    owner_id: ObjectID,
}

impl StructureBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<StructureBodyModuleData>,
        body: Arc<Mutex<StructureBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for StructureBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for StructureBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self
            .body
            .lock()
            .map_err(|_| "StructureBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "StructureBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "StructureBody lock poisoned")?;
        body.load_post_process()
    }
}

fn structure_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = StructureBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StructureBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn structure_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<StructureBodyModuleData>("StructureBody", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("StructureBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(StructureBody::new(
        data_arc.as_ref().clone(),
        owner_id,
    )));
    Box::new(StructureBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct HighlanderBodyModule {
    module_name_key: NameKeyType,
    data: Arc<ActiveBodyModuleData>,
    body: Arc<Mutex<HighlanderBody>>,
    owner_id: ObjectID,
}

impl HighlanderBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ActiveBodyModuleData>,
        body: Arc<Mutex<HighlanderBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for HighlanderBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for HighlanderBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self
            .body
            .lock()
            .map_err(|_| "HighlanderBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HighlanderBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HighlanderBody lock poisoned")?;
        body.load_post_process()
    }
}

fn highlander_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    active_body_module_data_factory(ini)
}

fn highlander_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<ActiveBodyModuleData>("HighlanderBody", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("HighlanderBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(HighlanderBody::new(
        data_arc.as_ref().clone(),
        owner_id,
    )));
    Box::new(HighlanderBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct ImmortalBodyModule {
    module_name_key: NameKeyType,
    data: Arc<ActiveBodyModuleData>,
    body: Arc<Mutex<ImmortalBody>>,
    owner_id: ObjectID,
}

impl ImmortalBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<ActiveBodyModuleData>,
        body: Arc<Mutex<ImmortalBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for ImmortalBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for ImmortalBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self.body.lock().map_err(|_| "ImmortalBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ImmortalBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "ImmortalBody lock poisoned")?;
        body.load_post_process()
    }
}

fn immortal_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    active_body_module_data_factory(ini)
}

fn immortal_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<ActiveBodyModuleData>("ImmortalBody", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("ImmortalBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(ImmortalBody::new(
        data_arc.as_ref().clone(),
        owner_id,
    )));
    Box::new(ImmortalBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct HiveStructureBodyModule {
    module_name_key: NameKeyType,
    data: Arc<HiveStructureBodyModuleData>,
    body: Arc<Mutex<HiveStructureBody>>,
    owner_id: ObjectID,
}

impl HiveStructureBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<HiveStructureBodyModuleData>,
        body: Arc<Mutex<HiveStructureBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for HiveStructureBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for HiveStructureBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self
            .body
            .lock()
            .map_err(|_| "HiveStructureBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HiveStructureBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self
            .body
            .lock()
            .map_err(|_| "HiveStructureBody lock poisoned")?;
        body.load_post_process()
    }
}

fn hive_structure_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HiveStructureBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HiveStructureBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn hive_structure_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<HiveStructureBodyModuleData>("HiveStructureBody", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("HiveStructureBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(HiveStructureBody::new(
        data_arc.as_ref().clone(),
        owner_id,
    )));
    Box::new(HiveStructureBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

#[derive(Debug)]
struct UndeadBodyModule {
    module_name_key: NameKeyType,
    data: Arc<UndeadBodyModuleData>,
    body: Arc<Mutex<UndeadBody>>,
    owner_id: ObjectID,
}

impl UndeadBodyModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<UndeadBodyModuleData>,
        body: Arc<Mutex<UndeadBody>>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            body,
            owner_id,
        }
    }
}

impl Module for UndeadBodyModule {
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
        attach_body_to_object(self.owner_id, Arc::clone(&self.body));
    }
}

impl Snapshotable for UndeadBodyModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let body = self.body.lock().map_err(|_| "UndeadBody lock poisoned")?;
        body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "UndeadBody lock poisoned")?;
        body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut body = self.body.lock().map_err(|_| "UndeadBody lock poisoned")?;
        body.load_post_process()
    }
}

fn undead_body_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = UndeadBodyModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse UndeadBody module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn undead_body_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<UndeadBodyModuleData>("UndeadBody", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("UndeadBody");
    let (owner_id, _) = resolve_owner_info(&thing);
    let body = Arc::new(Mutex::new(UndeadBody::new(
        data_arc.as_ref().clone(),
        owner_id,
    )));
    Box::new(UndeadBodyModule::new(
        module_name_key,
        data_arc,
        body,
        owner_id,
    ))
}

