//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Contain-module factory wrappers.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

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
    let typed_data =
        expect_contain_data::<OpenContainModuleData>(module_data.as_ref(), "OpenContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = OpenContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create OpenContain for object {}: {}",
            owner_id, err
        );
        OpenContain::new(Weak::new(), &OpenContainModuleData::default())
            .expect("OpenContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("OpenContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<TransportContainModuleData>(module_data.as_ref(), "TransportContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        TransportContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create TransportContain for object {}: {}",
                owner_id, err
            );
            TransportContain::new(Weak::new(), &TransportContainModuleData::default())
                .expect("TransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("TransportContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<GarrisonContainModuleData>(module_data.as_ref(), "GarrisonContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        GarrisonContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create GarrisonContain for object {}: {}",
                owner_id, err
            );
            GarrisonContain::new(Weak::new(), &GarrisonContainModuleData::default())
                .expect("GarrisonContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("GarrisonContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<TunnelContainModuleData>(module_data.as_ref(), "TunnelContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = TunnelContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create TunnelContain for object {}: {}",
            owner_id, err
        );
        TunnelContain::new(Weak::new(), &TunnelContainModuleData::default())
            .expect("TunnelContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("TunnelContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<OverlordContainModuleData>(module_data.as_ref(), "OverlordContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        OverlordContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create OverlordContain for object {}: {}",
                owner_id, err
            );
            OverlordContain::new(Weak::new(), &OverlordContainModuleData::default())
                .expect("OverlordContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("OverlordContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<HelixContainModuleData>(module_data.as_ref(), "HelixContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = HelixContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create HelixContain for object {}: {}",
            owner_id, err
        );
        HelixContain::new(Weak::new(), &HelixContainModuleData::default())
            .expect("HelixContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("HelixContain", thing, module_data, contain)
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
    let typed_data = expect_contain_data::<RailedTransportContainModuleData>(
        module_data.as_ref(),
        "RailedTransportContain",
    );
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = RailedTransportContain::new(make_owner_weak(owner_id), typed_data)
        .unwrap_or_else(|err| {
            warn!(
                "Failed to create RailedTransportContain for object {}: {}",
                owner_id, err
            );
            RailedTransportContain::new(Weak::new(), &RailedTransportContainModuleData::default())
                .expect("RailedTransportContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("RailedTransportContain", thing, module_data, contain)
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
    let typed_data = expect_contain_data::<RiderChangeContainModuleData>(
        module_data.as_ref(),
        "RiderChangeContain",
    );
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        RiderChangeContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create RiderChangeContain for object {}: {}",
                owner_id, err
            );
            RiderChangeContain::new(Weak::new(), &RiderChangeContainModuleData::default())
                .expect("RiderChangeContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("RiderChangeContain", thing, module_data, contain)
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
    let typed_data = expect_contain_data::<InternetHackContainModuleData>(
        module_data.as_ref(),
        "InternetHackContain",
    );
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        InternetHackContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create InternetHackContain for object {}: {}",
                owner_id, err
            );
            InternetHackContain::new(Weak::new(), &InternetHackContainModuleData::default())
                .expect("InternetHackContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("InternetHackContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<HealContainModuleData>(module_data.as_ref(), "HealContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain = HealContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
        warn!(
            "Failed to create HealContain for object {}: {}",
            owner_id, err
        );
        HealContain::new(Weak::new(), &HealContainModuleData::default())
            .expect("HealContain default construction failed")
    });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("HealContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<CaveContainModuleData>(module_data.as_ref(), "CaveContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let cave_system = crate::system::cave_system::TheCaveSystem();
    let contain = CaveContain::new(make_owner_weak(owner_id), typed_data, Some(cave_system.clone()))
        .unwrap_or_else(|err| {
            warn!(
                "Failed to create CaveContain for object {}: {}",
                owner_id, err
            );
            CaveContain::new(
                Weak::new(),
                &CaveContainModuleData::default(),
                Some(cave_system),
            )
                .expect("CaveContain default construction failed")
        });
    let contain: Arc<Mutex<CaveContain>> = Arc::new(Mutex::new(contain));
    let module_name_key = NameKeyGenerator::name_to_key("CaveContain");
    Box::new(CaveContainBindingModule::new(
        module_name_key,
        module_data,
        contain,
        owner_id,
    ))
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
    let typed_data =
        expect_contain_data::<ParachuteContainModuleData>(module_data.as_ref(), "ParachuteContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        ParachuteContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create ParachuteContain for object {}: {}",
                owner_id, err
            );
            ParachuteContain::new(Weak::new(), &ParachuteContainModuleData::default())
                .expect("ParachuteContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("ParachuteContain", thing, module_data, contain)
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
    let typed_data =
        expect_contain_data::<MobNexusContainModuleData>(module_data.as_ref(), "MobNexusContain");
    let (owner_id, _) = resolve_owner_info(&thing);
    let contain =
        MobNexusContain::new(make_owner_weak(owner_id), typed_data).unwrap_or_else(|err| {
            warn!(
                "Failed to create MobNexusContain for object {}: {}",
                owner_id, err
            );
            MobNexusContain::new(Weak::new(), &MobNexusContainModuleData::default())
                .expect("MobNexusContain default construction failed")
        });
    let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(contain));
    make_contain_binding_module("MobNexusContain", thing, module_data, contain)
}
