use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use game_engine::common::ini::INI;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BaseModuleData, CreateInterface, Module, ModuleData, ModuleType, NameKeyType,
    Object as ModuleObjectTrait, Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::{
    apply_module_overrides_to_existing_templates, register_module_override,
};
use log::warn;

use crate::common::{ObjectID, TheGameLogic, INVALID_ID};
use crate::modules::ContainModuleInterface;
use crate::object::contain::{
    CaveContain, CaveContainModuleData, GarrisonContain, GarrisonContainModuleData, HealContain,
    HealContainModuleData, HelixContain, HelixContainModuleData, InternetHackContain,
    InternetHackContainModuleData, MobNexusContain, MobNexusContainModuleData, OpenContain,
    OpenContainModuleData, OverlordContain, OverlordContainModuleData, ParachuteContain,
    ParachuteContainModuleData, RailedTransportContain, RailedTransportContainModuleData,
    RiderChangeContain, RiderChangeContainModuleData, TransportContain, TransportContainModuleData,
    TunnelContain, TunnelContainModuleData,
};

fn resolve_owner_id(thing: &Arc<dyn ModuleThing>) -> ObjectID {
    thing
        .as_object()
        .map(ModuleObjectTrait::get_object_id)
        .unwrap_or(INVALID_ID)
}

fn owner_weak(owner_id: ObjectID) -> Weak<RwLock<crate::object::Object>> {
    TheGameLogic::find_object_by_id(owner_id)
        .map(|arc| Arc::downgrade(&arc))
        .unwrap_or_else(Weak::new)
}

fn attach_contain_to_object(object_id: ObjectID, contain: Arc<Mutex<dyn ContainModuleInterface>>) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_contain(Some(contain));
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainModuleDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
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

    pub fn contain_data(&self) -> &T {
        &self.contain
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> Snapshotable
    for ContainModuleDataAdapter<T>
{
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
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
    pub fn from_module_data(module_data: &'a dyn ModuleData) -> Option<Self> {
        // Prefer direct concrete module data first, then adapter-backed module data.
        if let Some(data) = module_data.as_any().downcast_ref::<OpenContainModuleData>() {
            return Some(Self::Open(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<TransportContainModuleData>()
        {
            return Some(Self::Transport(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<GarrisonContainModuleData>()
        {
            return Some(Self::Garrison(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<TunnelContainModuleData>()
        {
            return Some(Self::Tunnel(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<OverlordContainModuleData>()
        {
            return Some(Self::Overlord(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<HelixContainModuleData>()
        {
            return Some(Self::Helix(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<RailedTransportContainModuleData>()
        {
            return Some(Self::RailedTransport(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<RiderChangeContainModuleData>()
        {
            return Some(Self::RiderChange(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<InternetHackContainModuleData>()
        {
            return Some(Self::InternetHack(data));
        }
        if let Some(data) = module_data.as_any().downcast_ref::<HealContainModuleData>() {
            return Some(Self::Heal(data));
        }
        if let Some(data) = module_data.as_any().downcast_ref::<CaveContainModuleData>() {
            return Some(Self::Cave(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<ParachuteContainModuleData>()
        {
            return Some(Self::Parachute(data));
        }
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<MobNexusContainModuleData>()
        {
            return Some(Self::MobNexus(data));
        }

        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<OpenContainModuleData>>()
        {
            return Some(Self::Open(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<TransportContainModuleData>>()
        {
            return Some(Self::Transport(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<GarrisonContainModuleData>>()
        {
            return Some(Self::Garrison(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<TunnelContainModuleData>>()
        {
            return Some(Self::Tunnel(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<OverlordContainModuleData>>()
        {
            return Some(Self::Overlord(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<HelixContainModuleData>>()
        {
            return Some(Self::Helix(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<RailedTransportContainModuleData>>()
        {
            return Some(Self::RailedTransport(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<RiderChangeContainModuleData>>()
        {
            return Some(Self::RiderChange(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<InternetHackContainModuleData>>()
        {
            return Some(Self::InternetHack(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<HealContainModuleData>>()
        {
            return Some(Self::Heal(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<CaveContainModuleData>>()
        {
            return Some(Self::Cave(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<ParachuteContainModuleData>>()
        {
            return Some(Self::Parachute(adapter.contain_data()));
        }
        if let Some(adapter) = module_data
            .as_any()
            .downcast_ref::<ContainModuleDataAdapter<MobNexusContainModuleData>>()
        {
            return Some(Self::MobNexus(adapter.contain_data()));
        }

        None
    }
}

#[derive(Debug)]
struct ContainBindingModule {
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn build_contain_module(
    module_name: &str,
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key(module_name);
    let owner_id = resolve_owner_id(&thing);
    Box::new(ContainBindingModule::new(
        module_name_key,
        module_data,
        contain,
        owner_id,
    ))
}

fn open_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn open_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<OpenContainModuleData>>()
        .expect("OpenContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain =
        OpenContain::new(owner_weak(owner_id), typed_data.contain_data()).unwrap_or_else(|_| {
            OpenContain::new(Weak::new(), &OpenContainModuleData::default())
                .expect("OpenContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("OpenContain", thing, module_data, contain)
}

fn transport_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn transport_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<TransportContainModuleData>>()
        .expect("TransportContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = TransportContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            TransportContain::new(Weak::new(), &TransportContainModuleData::default())
                .expect("TransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("TransportContain", thing, module_data, contain)
}

fn garrison_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn garrison_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<GarrisonContainModuleData>>()
        .expect("GarrisonContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = GarrisonContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            GarrisonContain::new(Weak::new(), &GarrisonContainModuleData::default())
                .expect("GarrisonContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("GarrisonContain", thing, module_data, contain)
}

fn tunnel_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn tunnel_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<TunnelContainModuleData>>()
        .expect("TunnelContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = TunnelContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            TunnelContain::new(Weak::new(), &TunnelContainModuleData::default())
                .expect("TunnelContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("TunnelContain", thing, module_data, contain)
}

fn overlord_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn overlord_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<OverlordContainModuleData>>()
        .expect("OverlordContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = OverlordContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            OverlordContain::new(Weak::new(), &OverlordContainModuleData::default())
                .expect("OverlordContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("OverlordContain", thing, module_data, contain)
}

fn helix_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn helix_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<HelixContainModuleData>>()
        .expect("HelixContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = HelixContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            HelixContain::new(Weak::new(), &HelixContainModuleData::default())
                .expect("HelixContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("HelixContain", thing, module_data, contain)
}

fn railed_transport_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn railed_transport_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<RailedTransportContainModuleData>>()
        .expect("RailedTransportContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = RailedTransportContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            RailedTransportContain::new(Weak::new(), &RailedTransportContainModuleData::default())
                .expect("RailedTransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RailedTransportContain", thing, module_data, contain)
}

fn rider_change_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn rider_change_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<RiderChangeContainModuleData>>()
        .expect("RiderChangeContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = RiderChangeContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            RiderChangeContain::new(Weak::new(), &RiderChangeContainModuleData::default())
                .expect("RiderChangeContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RiderChangeContain", thing, module_data, contain)
}

fn internet_hack_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn internet_hack_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<InternetHackContainModuleData>>()
        .expect("InternetHackContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = InternetHackContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            InternetHackContain::new(Weak::new(), &InternetHackContainModuleData::default())
                .expect("InternetHackContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("InternetHackContain", thing, module_data, contain)
}

fn heal_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn heal_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<HealContainModuleData>>()
        .expect("HealContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain =
        HealContain::new(owner_weak(owner_id), typed_data.contain_data()).unwrap_or_else(|_| {
            HealContain::new(Weak::new(), &HealContainModuleData::default())
                .expect("HealContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("HealContain", thing, module_data, contain)
}

fn cave_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn cave_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<CaveContainModuleData>>()
        .expect("CaveContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = CaveContain::new(owner_weak(owner_id), typed_data.contain_data(), None)
        .unwrap_or_else(|_| {
            CaveContain::new(Weak::new(), &CaveContainModuleData::default(), None)
                .expect("CaveContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("CaveContain", thing, module_data, contain)
}

fn parachute_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn parachute_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<ParachuteContainModuleData>>()
        .expect("ParachuteContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = ParachuteContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            ParachuteContain::new(Weak::new(), &ParachuteContainModuleData::default())
                .expect("ParachuteContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("ParachuteContain", thing, module_data, contain)
}

fn mob_nexus_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

fn mob_nexus_contain_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .as_any()
        .downcast_ref::<ContainModuleDataAdapter<MobNexusContainModuleData>>()
        .expect("MobNexusContain module data adapter expected");
    let owner_id = resolve_owner_id(&thing);
    let contain = MobNexusContain::new(owner_weak(owner_id), typed_data.contain_data())
        .unwrap_or_else(|_| {
            MobNexusContain::new(Weak::new(), &MobNexusContainModuleData::default())
                .expect("MobNexusContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("MobNexusContain", thing, module_data, contain)
}

fn install_contain_overrides() -> Result<(), String> {
    register_module_override(
        "OpenContain",
        ModuleType::Behavior,
        open_contain_module_factory,
        open_contain_module_data_factory,
    )?;
    register_module_override(
        "TransportContain",
        ModuleType::Behavior,
        transport_contain_module_factory,
        transport_contain_module_data_factory,
    )?;
    register_module_override(
        "GarrisonContain",
        ModuleType::Behavior,
        garrison_contain_module_factory,
        garrison_contain_module_data_factory,
    )?;
    register_module_override(
        "TunnelContain",
        ModuleType::Behavior,
        tunnel_contain_module_factory,
        tunnel_contain_module_data_factory,
    )?;
    register_module_override(
        "OverlordContain",
        ModuleType::Behavior,
        overlord_contain_module_factory,
        overlord_contain_module_data_factory,
    )?;
    register_module_override(
        "HelixContain",
        ModuleType::Behavior,
        helix_contain_module_factory,
        helix_contain_module_data_factory,
    )?;
    register_module_override(
        "ParachuteContain",
        ModuleType::Behavior,
        parachute_contain_module_factory,
        parachute_contain_module_data_factory,
    )?;
    register_module_override(
        "MobNexusContain",
        ModuleType::Behavior,
        mob_nexus_contain_module_factory,
        mob_nexus_contain_module_data_factory,
    )?;
    register_module_override(
        "RailedTransportContain",
        ModuleType::Behavior,
        railed_transport_contain_module_factory,
        railed_transport_contain_module_data_factory,
    )?;
    register_module_override(
        "RiderChangeContain",
        ModuleType::Behavior,
        rider_change_contain_module_factory,
        rider_change_contain_module_data_factory,
    )?;
    register_module_override(
        "InternetHackContain",
        ModuleType::Behavior,
        internet_hack_contain_module_factory,
        internet_hack_contain_module_data_factory,
    )?;
    register_module_override(
        "HealContain",
        ModuleType::Behavior,
        heal_contain_module_factory,
        heal_contain_module_data_factory,
    )?;
    register_module_override(
        "CaveContain",
        ModuleType::Behavior,
        cave_contain_module_factory,
        cave_contain_module_data_factory,
    )?;
    Ok(())
}

static CONTAIN_OVERRIDES_READY: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_module_overrides_installed() -> Result<(), String> {
    CONTAIN_OVERRIDES_READY
        .get_or_init(|| {
            install_contain_overrides()?;
            apply_module_overrides_to_existing_templates()?;
            Ok(())
        })
        .clone()
}
