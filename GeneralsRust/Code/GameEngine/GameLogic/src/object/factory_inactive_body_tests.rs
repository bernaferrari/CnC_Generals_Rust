//! InactiveBody.cpp:22-26 owns its constructor effect; 155-176 owns Xfer.
//! Exercise actual installed modules without publishing a registry or world.
use super::*;
use crate::common::{DefaultThingTemplate, TemplateModuleInfo, ThingTemplate};
use crate::object::body::body_module::BodyModuleData;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module::ModuleInterfaceType;
use std::io::Cursor;

struct InactiveTemplate {
    base: DefaultThingTemplate,
    modules: Vec<TemplateModuleInfo>,
}

impl std::fmt::Debug for InactiveTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InactiveTemplate").finish_non_exhaustive()
    }
}

impl ThingTemplate for InactiveTemplate {
    fn get_name(&self) -> &AsciiString {
        self.base.get_name()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.base.is_kind_of(kind)
    }
    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.base.get_template_geometry_info()
    }
    fn calc_vision_range(&self) -> f32 {
        self.base.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.base.calc_shroud_clearing_range()
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &self.modules
    }
}

fn prepare(factory: &mut ObjectFactory) -> Arc<RwLock<Object>> {
    let mut identity_owner = crate::system::game_logic::GameLogic::new();
    let template = InactiveTemplate {
        base: DefaultThingTemplate::new("OwnedInactiveBody".into()),
        modules: vec![TemplateModuleInfo {
            name: "InactiveBody".into(),
            module_tag: "OwnedInactiveBodyTag".into(),
            data: Arc::new(BodyModuleData::default()),
            interface_mask: ModuleInterfaceType::BODY,
        }],
    };
    let id = factory
        .prepare_object(
            &mut identity_owner,
            Arc::new(template),
            Coord3D::new(50.0, 50.0, 0.0),
            None,
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    factory.get_object(id).unwrap().get_base_object().unwrap()
}

fn body_entry(owner: &Arc<RwLock<Object>>) -> Arc<ModuleEntry> {
    owner
        .read()
        .unwrap()
        .modules
        .iter()
        .find(|entry| entry.name().as_str() == "InactiveBody")
        .unwrap()
        .clone()
}

#[test]
fn factory_inactive_body_marks_only_exact_unpublished_owner() {
    let mut peer_factory = ObjectFactory::new();
    let peer = prepare(&mut peer_factory);
    let mut held_peer = peer.write().unwrap();
    assert!(held_peer.is_effectively_dead());
    held_peer.set_effectively_dead(false);
    let mut factory = ObjectFactory::new();
    let owner = prepare(&mut factory);
    let owner = owner.read().unwrap();
    assert_eq!(owner.get_id(), held_peer.get_id());
    assert!(owner.is_effectively_dead());
    assert!(!held_peer.is_effectively_dead());
    assert!(owner.modules_ready);
    let body = owner.get_body_module().unwrap();
    let body = body.lock().unwrap();
    assert_eq!(body.get_health(), 0.0);
    assert!(body.is_indestructible());
}

#[test]
fn installed_inactive_body_xfer_preserves_cpp_version_chain_and_peer() {
    let mut source_factory = ObjectFactory::new();
    let source = prepare(&mut source_factory);
    let source_entry = body_entry(&source);
    let mut bytes = Vec::new();
    source_entry.with_module(|module| {
        module
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    });
    // InactiveBody, BodyModule, BehaviorModule, ObjectModule, Module versions,
    // followed by BodyModule's original f32 damage scalar. No adapter payload.
    let mut expected = vec![1; 5];
    expected.extend_from_slice(&1.0_f32.to_le_bytes());
    assert_eq!(bytes, expected);
    let sentinel = 0x7654_3210_u32;
    bytes.extend_from_slice(&sentinel.to_le_bytes());

    let mut destination_factory = ObjectFactory::new();
    let destination = prepare(&mut destination_factory);
    let destination_entry = body_entry(&destination);
    let mut held_source = source.write().unwrap();
    held_source.set_effectively_dead(false);
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    destination_entry.with_module(|module| {
        module.xfer(&mut load).unwrap();
        module.load_post_process().unwrap();
    });
    assert_eq!(load.bytes_read(), expected.len() as u64);
    let mut next = 0;
    load.xfer_unsigned_int(&mut next).unwrap();
    assert_eq!(next, sentinel);
    assert!(destination.read().unwrap().is_effectively_dead());
    assert!(!held_source.is_effectively_dead());
}
