//! CPP GarrisonContain529–557 through the actual authored factory interface.
use super::*;
use crate::modules::ContainModuleInterface;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

const OWNER_ID: u32 = 0x7B3D_0091;
struct Installed {
    owner: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
}
fn definitions() {
    static DEFINITIONS: OnceLock<()> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        if get_thing_factory().unwrap().is_none(){init_thing_factory().unwrap();}
        if get_module_factory().unwrap().is_none(){init_module_factory().unwrap();}
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let mut text=String::new();
        for (name,kind,health,max,neutral) in [
            ("GarrisonAdmissionHealthy","INERT",100,2,"Yes"),
            ("GarrisonAdmissionFull","INERT",100,0,"Yes"),
            ("GarrisonAdmissionNegativeMax","INERT",100,-1,"Yes"),
            ("GarrisonAdmissionNoNeutral","INERT",100,2,"No"),
            ("GarrisonAdmissionZeroHealth","INERT",0,2,"Yes"),
            ("GarrisonAdmissionUntilDestroyed","INERT GARRISONABLE_UNTIL_DESTROYED",100,2,"Yes"),
        ] {
            text.push_str(&format!("Object {name}\n KindOf = {kind}\n Body = ActiveBody AuthoredBody\n MaxHealth = 100\n InitialHealth = {health}\n End\n Behavior = GarrisonContain AuthoredGarrison\n ContainMax = {max}\n AllowInsideKindOf = INFANTRY\n AllowNeutralInside = {neutral}\n End\nEnd\n"));
        }
        text.push_str("Object GarrisonAdmissionInfantry\n KindOf = INFANTRY\nEnd\nObject GarrisonAdmissionNoGarrison\n KindOf = INFANTRY NO_GARRISON\nEnd\nObject GarrisonAdmissionWrongKind\n KindOf = VEHICLE\nEnd\n");
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&text),9);
    });
}
fn raw(name: &str, id: u32) -> Object {
    definitions();
    Object::new_raw(
        crate::helpers::TheThingFactory::find_template(name).unwrap(),
        id,
        ObjectStatusMaskType::NONE,
        None,
    )
}
impl Installed {
    fn new(name: &str) -> Self {
        definitions();
        let template = crate::helpers::TheThingFactory::find_template(name).unwrap();
        let owner = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            OWNER_ID,
            ObjectStatusMaskType::NONE,
            None,
        )));
        assert!(owner.read().unwrap().get_contain().is_none());
        Object::init_modules_for(&owner, template.as_ref()).unwrap();
        let (entry, contain) = {
            let owner = owner.read().unwrap();
            (
                owner.find_module_by_name("GarrisonContain").unwrap(),
                owner.get_contain().unwrap(),
            )
        };
        Self {
            owner,
            entry,
            contain,
        }
    }
    fn admit(&self, passenger: &Object, capacity: bool) -> bool {
        let owner = self.owner.write().unwrap();
        self.contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(passenger, &owner, capacity)
    }
    fn save(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap()
        });
        bytes
    }
    fn damaged(&self) {
        // Restore a valid damaged-body continuation through the installed
        // ActiveBody Xfer. This isolates admission from ambient GlobalData
        // thresholds while retaining the actual factory body implementation.
        let body_entry = self
            .owner
            .read()
            .unwrap()
            .find_module_by_name("ActiveBody")
            .unwrap();
        let mut bytes = Vec::new();
        body_entry.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap();
        });
        // ActiveBody -> BodyModule -> BehaviorModule -> ObjectModule -> Module
        // versions, damage scalar, five hull values, then BodyDamageType.
        assert_eq!(&bytes[..5], &[1; 5]);
        bytes[9..13].copy_from_slice(&25.0_f32.to_le_bytes());
        bytes[17..21].copy_from_slice(&25.0_f32.to_le_bytes());
        bytes[29..33].copy_from_slice(&2_u32.to_le_bytes()); // BODY_REALLYDAMAGED
        body_entry.with_module(|module| {
            module
                .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
                .unwrap();
        });
    }
}

#[test]
fn installed_garrison_admission_uses_held_owner_and_cpp_body_kind_relation_capacity_order() {
    let healthy = Installed::new("GarrisonAdmissionHealthy");
    let peer = Installed::new("GarrisonAdmissionZeroHealth");
    assert_eq!(
        healthy.owner.read().unwrap().get_id(),
        peer.owner.read().unwrap().get_id()
    );
    assert!(!Arc::ptr_eq(&healthy.contain, &peer.contain));
    let infantry = raw("GarrisonAdmissionInfantry", OWNER_ID + 1);
    let no_garrison = raw("GarrisonAdmissionNoGarrison", OWNER_ID + 1);
    let wrong_kind = raw("GarrisonAdmissionWrongKind", OWNER_ID + 1);
    let saved = healthy.save();
    assert!(healthy.admit(&infantry, true));
    assert!(healthy.admit(&infantry, false));
    assert!(!healthy.admit(&wrong_kind, false));
    assert!(!healthy.admit(&no_garrison, false));
    assert!(!peer.admit(&infantry, false));
    assert_eq!(
        healthy.save(),
        saved,
        "query does not mutate the installed runtime"
    );
    let full = Installed::new("GarrisonAdmissionFull");
    assert!(!full.admit(&infantry, true));
    assert!(full.admit(&infantry, false));
    let negative = Installed::new("GarrisonAdmissionNegativeMax");
    assert!(
        !negative.admit(&infantry, true),
        "CPP uses count < max without an unlimited sentinel"
    );
    assert!(negative.admit(&infantry, false));
    let no_neutral = Installed::new("GarrisonAdmissionNoNeutral");
    assert!(
        !no_neutral.admit(&infantry, false),
        "Open neutral relationship rule precedes derived gates"
    );
    healthy.damaged();
    assert!(!healthy.admit(&infantry, false));
    let until = Installed::new("GarrisonAdmissionUntilDestroyed");
    until.damaged();
    assert!(until.admit(&infantry, false));
    assert!(
        !until.admit(&no_garrison, false),
        "NoGarrison still applies to damaged-until-destroyed containers"
    );
    // Xfer reaches the same attached interface; read-only admission stays stable.
    full.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    });
    assert_eq!(full.save(), saved);
    assert!(
        !full.admit(&infantry, true),
        "Xfer does not replace authored capacity"
    );
    assert!(full.admit(&infantry, false));
    assert!(crate::object::registry::OBJECT_REGISTRY
        .get_object(OWNER_ID)
        .is_none());
}
