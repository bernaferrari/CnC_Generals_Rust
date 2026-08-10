//! Contain-module adapters and helix/mob/garrison/transport factories.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::*;
use super::helpers::*;

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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data = contain_adapter_data::<OpenContainModuleData>("OpenContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = OpenContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        OpenContain::new(Weak::new(), &OpenContainModuleData::default())
            .expect("OpenContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<TransportContainModuleData>("TransportContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = TransportContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        TransportContain::new(Weak::new(), &TransportContainModuleData::default())
            .expect("TransportContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<GarrisonContainModuleData>("GarrisonContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = GarrisonContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        GarrisonContain::new(Weak::new(), &GarrisonContainModuleData::default())
            .expect("GarrisonContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<TunnelContainModuleData>("TunnelContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = TunnelContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        TunnelContain::new(Weak::new(), &TunnelContainModuleData::default())
            .expect("TunnelContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<OverlordContainModuleData>("OverlordContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = OverlordContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        OverlordContain::new(Weak::new(), &OverlordContainModuleData::default())
            .expect("OverlordContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data = contain_adapter_data::<HelixContainModuleData>("HelixContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = HelixContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        HelixContain::new(Weak::new(), &HelixContainModuleData::default())
            .expect("HelixContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("HelixContain", thing, module_data, contain)
}

pub(super) fn railed_transport_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data = contain_adapter_data::<RailedTransportContainModuleData>(
        "RailedTransportContain",
        &module_data,
    );
    let owner_id = resolve_owner_id(&thing);
    let contain =
        RailedTransportContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
            RailedTransportContain::new(Weak::new(), &RailedTransportContainModuleData::default())
                .expect("RailedTransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RailedTransportContain", thing, module_data, contain)
}

pub(super) fn rider_change_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<RiderChangeContainModuleData>("RiderChangeContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain =
        RiderChangeContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
            RiderChangeContain::new(Weak::new(), &RiderChangeContainModuleData::default())
                .expect("RiderChangeContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("RiderChangeContain", thing, module_data, contain)
}

pub(super) fn internet_hack_contain_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<InternetHackContainModuleData>("InternetHackContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain =
        InternetHackContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
            InternetHackContain::new(Weak::new(), &InternetHackContainModuleData::default())
                .expect("InternetHackContain default construction failed")
        });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data = contain_adapter_data::<HealContainModuleData>("HealContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = HealContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        HealContain::new(Weak::new(), &HealContainModuleData::default())
            .expect("HealContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data = contain_adapter_data::<CaveContainModuleData>("CaveContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let cave_system = crate::system::cave_system::TheCaveSystem();
    let contain = CaveContain::new(
        owner_weak(owner_id),
        contain_data,
        Some(cave_system.clone()),
    )
    .unwrap_or_else(|_| {
        CaveContain::new(
            Weak::new(),
            &CaveContainModuleData::default(),
            Some(cave_system),
        )
        .expect("CaveContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<ParachuteContainModuleData>("ParachuteContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = ParachuteContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        ParachuteContain::new(Weak::new(), &ParachuteContainModuleData::default())
            .expect("ParachuteContain default construction failed")
    });
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let contain_data =
        contain_adapter_data::<MobNexusContainModuleData>("MobNexusContain", &module_data);
    let owner_id = resolve_owner_id(&thing);
    let contain = MobNexusContain::new(owner_weak(owner_id), contain_data).unwrap_or_else(|_| {
        MobNexusContain::new(Weak::new(), &MobNexusContainModuleData::default())
            .expect("MobNexusContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    build_contain_module("MobNexusContain", thing, module_data, contain)
}

pub(super) fn contain_adapter_data<'a, T>(module_name: &str, module_data: &'a Arc<dyn ModuleData>) -> &'a T
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    module_data
        .as_ref()
        .downcast_ref::<ContainModuleDataAdapter<T>>()
        .unwrap_or_else(|| panic!("{module_name} module data adapter expected"))
        .contain_data()
}
