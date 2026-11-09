//! Contain-module adapters and helix/mob/garrison/transport factories.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::helpers::*;
use super::*;

#[derive(Debug, Clone)]
pub struct ContainModuleDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
    base: BaseModuleData,
    contain: T,
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> ContainModuleDataAdapter<T> {
    pub(crate) fn new(contain: T) -> Self {
        Self {
            base: BaseModuleData::new(),
            contain,
        }
    }

    pub fn contain_data(&self) -> &T {
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

/// Closed set of contain module data variants used by this port.
///
/// C++ has a finite list of contain module classes; representing that list as an enum keeps
/// call sites typed and avoids scattered `as_any().downcast_*` logic.
pub enum ContainModuleDataKind<'a> {
    Open(&'a OpenContainModuleData),
    Transport(&'a TransportContainModuleData),
    Garrison(&'a GarrisonContainModuleData),
    Tunnel(&'a TunnelContainModuleData),
    Overlord(&'a OverlordContainModuleData),
    Helix(&'a HelixContainModuleData),
    RailedTransport(&'a RailedTransportContainModuleData),
    RiderChange(&'a RiderChangeContainModuleData),
    InternetHack(&'a InternetHackContainModuleData),
    Heal(&'a HealContainModuleData),
    Cave(&'a CaveContainModuleData),
    Parachute(&'a ParachuteContainModuleData),
    MobNexus(&'a MobNexusContainModuleData),
}

impl<'a> ContainModuleDataKind<'a> {
    /// The original inherited OpenContain data, borrowed from one admitted module.
    pub fn open_data(&self) -> &'a OpenContainModuleData {
        match self {
            Self::Open(data) => data,
            Self::Transport(data) => &data.base,
            Self::Garrison(data) => &data.base,
            Self::Tunnel(data) => &data.base,
            Self::Overlord(data) => &data.base.base,
            Self::Helix(data) => &data.base.base,
            Self::RailedTransport(data) => &data.base.base,
            Self::RiderChange(data) => &data.base.base,
            Self::InternetHack(data) => &data.base.base,
            Self::Heal(data) => &data.base,
            Self::Cave(data) => &data.base,
            Self::Parachute(data) => &data.base,
            Self::MobNexus(data) => &data.base,
        }
    }

    pub fn from_module_data(module_data: &'a dyn ModuleData) -> Option<Self> {
        macro_rules! try_kind {
            ($data_ty:ty, $variant:ident) => {{
                let any = module_data.as_any();
                if let Some(data) = any.downcast_ref::<$data_ty>() {
                    return Some(Self::$variant(data));
                }
                if let Some(adapter) = any.downcast_ref::<ContainModuleDataAdapter<$data_ty>>() {
                    return Some(Self::$variant(adapter.contain_data()));
                }
            }};
        }

        try_kind!(OpenContainModuleData, Open);
        try_kind!(TransportContainModuleData, Transport);
        try_kind!(GarrisonContainModuleData, Garrison);
        try_kind!(TunnelContainModuleData, Tunnel);
        try_kind!(OverlordContainModuleData, Overlord);
        try_kind!(HelixContainModuleData, Helix);
        try_kind!(RailedTransportContainModuleData, RailedTransport);
        try_kind!(RiderChangeContainModuleData, RiderChange);
        try_kind!(InternetHackContainModuleData, InternetHack);
        try_kind!(HealContainModuleData, Heal);
        try_kind!(CaveContainModuleData, Cave);
        try_kind!(ParachuteContainModuleData, Parachute);
        try_kind!(MobNexusContainModuleData, MobNexus);

        None
    }
}

pub(crate) fn open_contain_data_view(
    handle: ModuleDataHandle,
) -> game_engine::common::thing::module::ModuleDataView<OpenContainModuleData> {
    game_engine::common::thing::module::ModuleDataView::new(handle, |data| {
        ContainModuleDataKind::from_module_data(data)
            .expect("OpenContain-derived module data expected")
            .open_data()
    })
}

pub(crate) fn transport_contain_data_view(
    handle: ModuleDataHandle,
) -> game_engine::common::thing::module::ModuleDataView<TransportContainModuleData> {
    game_engine::common::thing::module::ModuleDataView::new(handle, |data| {
        match ContainModuleDataKind::from_module_data(data)
            .expect("TransportContain-derived module data expected")
        {
            ContainModuleDataKind::Transport(data) => data,
            ContainModuleDataKind::Overlord(data) => &data.base,
            ContainModuleDataKind::Helix(data) => &data.base,
            ContainModuleDataKind::RailedTransport(data) => &data.base,
            ContainModuleDataKind::RiderChange(data) => &data.base,
            ContainModuleDataKind::InternetHack(data) => &data.base,
            _ => panic!("TransportContain-derived module data expected"),
        }
    })
}

#[derive(Debug)]
struct ContainBindingModule {
    module_name_key: NameKeyType,
    module_data: ModuleDataHandle,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
    owner_id: ObjectID,
}

impl ContainBindingModule {
    fn new(
        module_name_key: NameKeyType,
        module_data: ModuleDataHandle,
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

/// The Object's containment interface and the authored behavior must refer to
/// the same runtime. Installation borrows the actual module rather than
/// rediscovering its owner through a process-wide ObjectId lookup.
pub(crate) fn contain_handle_for_module(
    module: &dyn Module,
) -> Option<Arc<Mutex<dyn ContainModuleInterface>>> {
    module
        .as_any()
        .downcast_ref::<ContainBindingModule>()
        .map(|binding| Arc::clone(&binding.contain))
}

impl Module for ContainBindingModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.module_data)
    }

    fn on_object_created(&mut self) {
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_owner_created() {
                warn!(
                    "Contain module on_owner_created failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }

    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        // C++ CaveContain is a CreateModuleInterface; other contains are not.
        if self.module_name_key == NameKeyGenerator::name_to_key("CaveContain") {
            Some(self)
        } else {
            None
        }
    }

    fn on_delete(&mut self) {
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_delete() {
                warn!(
                    "Contain module on_delete failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }
}

impl CreateInterface for ContainBindingModule {
    fn on_create(&self) {
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_create() {
                warn!(
                    "CaveContain on_create failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }

    fn on_build_complete(&self) {
        if let Ok(mut contain_guard) = self.contain.lock() {
            if let Err(err) = contain_guard.on_build_complete() {
                warn!(
                    "CaveContain on_build_complete failed for object {}: {}",
                    self.owner_id, err
                );
            }
        }
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.contain
            .lock()
            .map(|guard| guard.should_do_on_build_complete())
            .unwrap_or(false)
    }
}

impl Snapshotable for ContainBindingModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(contain) = self.contain.lock() {
            contain.snapshot_crc(xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut contain) = self.contain.lock() {
            contain.snapshot_xfer(xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(mut contain) = self.contain.lock() {
            contain.snapshot_load_post_process()
        } else {
            Ok(())
        }
    }
}

pub(super) fn build_contain_module(
    module_name: &str,
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key(module_name);
    let owner_id = resolve_owner_id(&thing);
    // Every container assembled in a receiving world resolves that world's
    // objects (C++ TheGameLogic of the owning engine), not the published one.
    if let Some(world) = crate::object::supplied_module_world(thing.as_ref()) {
        match contain.lock() {
            Ok(mut contain) => contain.bind_owning_world(world.world()),
            Err(poisoned) => poisoned.into_inner().bind_owning_world(world.world()),
        }
    }
    Box::new(ContainBindingModule::new(
        module_name_key,
        module_data,
        contain,
        owner_id,
    ))
}

pub(super) fn open_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OpenContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OpenContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn open_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<OpenContainModuleData>("OpenContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match OpenContain::new_for_owner(owner_id, module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("OpenContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("OpenContain", thing, module_data, contain)
}

pub(super) fn transport_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TransportContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TransportContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn transport_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<TransportContainModuleData>("TransportContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match TransportContain::new_for_owner(owner_id, module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("TransportContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("TransportContain", thing, module_data, contain)
}

pub(super) fn garrison_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GarrisonContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GarrisonContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn garrison_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<GarrisonContainModuleData>("GarrisonContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match GarrisonContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("GarrisonContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("GarrisonContain", thing, module_data, contain)
}

pub(super) fn tunnel_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = TunnelContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse TunnelContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn tunnel_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<TunnelContainModuleData>("TunnelContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match TunnelContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("TunnelContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("TunnelContain", thing, module_data, contain)
}

pub(super) fn overlord_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OverlordContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OverlordContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn overlord_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<OverlordContainModuleData>("OverlordContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match OverlordContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("OverlordContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("OverlordContain", thing, module_data, contain)
}

pub(super) fn helix_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HelixContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HelixContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn helix_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<HelixContainModuleData>("HelixContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match HelixContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("HelixContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("HelixContain", thing, module_data, contain)
}

pub(super) fn railed_transport_contain_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = RailedTransportContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RailedTransportContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn railed_transport_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<RailedTransportContainModuleData>("RailedTransportContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match RailedTransportContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("RailedTransportContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RailedTransportContain", thing, module_data, contain)
}

pub(super) fn rider_change_contain_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = RiderChangeContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse RiderChangeContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn rider_change_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<RiderChangeContainModuleData>("RiderChangeContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match RiderChangeContain::new_for_owner(owner_id, module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("RiderChangeContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RiderChangeContain", thing, module_data, contain)
}

pub(super) fn internet_hack_contain_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = InternetHackContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse InternetHackContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn internet_hack_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<InternetHackContainModuleData>("InternetHackContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match InternetHackContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("InternetHackContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("InternetHackContain", thing, module_data, contain)
}

pub(super) fn heal_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HealContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HealContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn heal_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<HealContainModuleData>("HealContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match HealContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("HealContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("HealContain", thing, module_data, contain)
}

pub(super) fn cave_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CaveContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CaveContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn cave_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<CaveContainModuleData>("CaveContain", &module_data);
    let owner = match crate::object::supplied_module_owner(thing.as_ref()) {
        Ok(owner) => owner,
        Err(error) => return failed_initialization_module("CaveContain", module_data, error),
    };
    // C++ CaveContain resolves TheCaveSystem of the GameLogic that owns this
    // entrance (CaveContain.cpp:45-312). Without the receiving world's cave
    // system the module fails initialization instead of borrowing whichever
    // world happens to be published.
    let Some(world) = crate::object::supplied_module_world(thing.as_ref()) else {
        return failed_initialization_module(
            "CaveContain",
            module_data,
            "CaveContain requires its receiving world's CaveSystem".to_string(),
        );
    };
    let contain = match CaveContain::new(Arc::downgrade(&owner), module_data.clone(), world) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("CaveContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("CaveContain", thing, module_data, contain)
}

pub(super) fn parachute_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ParachuteContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ParachuteContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn parachute_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<ParachuteContainModuleData>("ParachuteContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match ParachuteContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("ParachuteContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("ParachuteContain", thing, module_data, contain)
}

pub(super) fn mob_nexus_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MobNexusContainModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MobNexusContain module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(ContainModuleDataAdapter::new(data))
}

pub(super) fn mob_nexus_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    require_contain_data::<MobNexusContainModuleData>("MobNexusContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = match MobNexusContain::new(owner_weak(owner_id), module_data.clone()) {
        Ok(contain) => contain,
        Err(error) => return failed_initialization_module("MobNexusContain", module_data, error.to_string()),
    };
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("MobNexusContain", thing, module_data, contain)
}

fn require_contain_data<T>(module_name: &str, module_data: &ModuleDataHandle)
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    assert!(
        module_data.is::<ContainModuleDataAdapter<T>>(),
        "{module_name} module data adapter expected"
    );
}
