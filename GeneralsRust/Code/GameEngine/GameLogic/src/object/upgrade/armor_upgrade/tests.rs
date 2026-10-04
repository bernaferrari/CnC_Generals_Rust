//! Actual installed ArmorUpgrade contracts, ArmorUpgrade.cpp:51–80,86–129,
//! UpgradeModule.cpp:105–146,191–230. Upgrade factory execution is real;
//! the fixture body is ActiveBody from new_test_from_template, not evidence
//! that the separate authored BodyBinding installation is instance-isolated.

use super::*;
use crate::common::{Coord3D, FXListId, FXListManagerInterface, ObjectStatusMaskType, ThingId};
use crate::helpers::TheThingFactory;
use crate::object::ModuleEntry;
use crate::object::Object;
use crate::object::drawable::DrawableExt;
use crate::object::drawable::{Drawable, DrawableType};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::io::Cursor;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

const ID: ObjectID = 0x7A3D_0001;
const TRIGGER: &str = "Upgrade_OwnedArmorTrigger";
const SECOND: &str = "Upgrade_OwnedArmorSecond";
const CONFLICT: &str = "Upgrade_OwnedArmorConflict";
const UNRELATED: &str = "Upgrade_OwnedArmorUnrelated";
const CHEM: &str = "Upgrade_AmericaChemicalSuits";

fn mask(name: &str) -> UpgradeMaskType {
    UpgradeMaskType::from_bits_retain(crate::upgrade::upgrade_mask_for_name(name).to_bits())
}

struct Installed {
    object: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
    drawable: Arc<RwLock<Drawable>>,
}

impl Installed {
    fn new(name: &str, activation: &str, fields: &str) -> Self {
        Self::with_body(name, activation, fields, true)
    }

    fn with_body(name: &str, activation: &str, fields: &str, has_body: bool) -> Self {
        assert!(ensure_thing_factory_exists());
        // Every upgrade named by these authored fixtures has a real catalog
        // definition/mask. Do not rely on the separate unknown-name allocator.
        crate::upgrade::center::with_upgrade_center_mut(|center| {
            for name in [TRIGGER, SECOND, CONFLICT, UNRELATED, CHEM] {
                center.new_upgrade(AsciiString::from(name));
            }
        });
        if game_engine::common::thing::module_factory::get_module_factory()
            .unwrap()
            .is_none()
        {
            game_engine::common::thing::module_factory::init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let loaded = get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
            "Object {name}\n KindOf = INERT\n ShadowSizeX = 18\n ShadowSizeY = 23\n ShadowOffsetX = 4\n ShadowOffsetY = -7\n Behavior = ArmorUpgrade OwnedArmor\n TriggeredBy = {activation}\n {fields}\n End\nEnd\n"
        ));
        assert_eq!(loaded, 1);
        let template =
            TheThingFactory::find_template(name).expect("actual authored ArmorUpgrade data");
        let object = Arc::new(RwLock::new(if has_body {
            Object::new_test_from_template(ID, 100.0, template.clone())
        } else {
            Object::new_raw(template.clone(), ID, ObjectStatusMaskType::NONE, None)
        }));
        Object::init_modules_for(&object, template.as_ref()).unwrap();
        let entry = object
            .read()
            .unwrap()
            .find_module_by_name("ArmorUpgrade")
            .expect("actual canonical installed ArmorUpgrade entry");
        entry.with_module(|module| {
            assert!(
                module.as_any().is::<ArmorUpgrade>(),
                "never use the dormant upgrade/modules/armor representation"
            );
        });
        // CPP Object::updateUpgradeModules requires a controlling player.
        // The isolated child owns this roster; no unrelated world is reset.
        let has_player = crate::player::player_list()
            .read()
            .unwrap()
            .get_player(0)
            .is_some();
        if !has_player {
            crate::player::player_list()
                .write()
                .unwrap()
                .add_player(Arc::new(RwLock::new(crate::player::Player::new(0))));
        }
        let team = Arc::new(RwLock::new(crate::team::Team::new(
            format!("{name}Team").into(),
            ID + 5,
        )));
        team.write().unwrap().set_controlling_player_id(Some(0));
        object.write().unwrap().set_team(Some(team)).unwrap();
        let drawable = Arc::new(RwLock::new(Drawable::new(
            ID + 1,
            ID,
            name.to_owned(),
            DrawableType::Static,
        )));
        object.write().unwrap().set_drawable(Some(drawable.clone()));
        Self {
            object,
            entry,
            drawable,
        }
    }

    fn applied(&self) -> bool {
        self.entry.with_module(|module| {
            module
                .as_any()
                .downcast_ref::<ArmorUpgrade>()
                .unwrap()
                .applied
        })
    }

    fn apply(&self, name: &str) {
        let upgrade = crate::upgrade::center::with_upgrade_center_mut(|center| {
            center.new_upgrade(AsciiString::from(name))
        });
        self.object.write().unwrap().give_upgrade(&upgrade);
    }

    fn armored(&self) -> bool {
        let body = self
            .object
            .read()
            .unwrap()
            .get_body_module()
            .expect("real ActiveBody fixture");
        let result = body
            .lock()
            .unwrap()
            .test_armor_set_flag(ArmorSetType::PlayerUpgrade);
        result
    }

    fn decal(&self) -> TerrainDecalType {
        self.drawable.read().unwrap().get_terrain_decal()
    }

    fn bytes(&self, crc: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.entry.with_module(|module| {
            let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
            if crc {
                module.crc(&mut save).unwrap();
            } else {
                module.xfer(&mut save).unwrap();
            }
        });
        bytes
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    matches!(
        crate::test_process::run_bounded(
            name.strip_prefix("gamelogic::").unwrap_or(name),
            "GENERALS_ARMOR_OWNED_CHILD"
        ),
        crate::test_process::TestProcess::Child
    )
}
#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

#[test]
fn actual_installed_same_id_owners_interleave_and_reset_without_undo() {
    if !child(concat!(
        module_path!(),
        "::actual_installed_same_id_owners_interleave_and_reset_without_undo"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedArmorA", CHEM, "");
    let b = Installed::new("OwnedArmorB", TRIGGER, "");
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
    assert!(!a.applied() && !b.applied() && !a.armored() && !b.armored());
    assert_ne!(a.decal(), TerrainDecalType::ChemSuit);
    assert!(!Arc::ptr_eq(&a.entry, &b.entry));
    a.apply(CHEM);
    assert!(a.applied() && a.armored());
    assert_eq!(a.decal(), TerrainDecalType::ChemSuit);
    assert!(!b.applied() && !b.armored());
    assert_ne!(b.decal(), TerrainDecalType::ChemSuit);
    b.apply(TRIGGER);
    assert!(b.applied() && b.armored());
    a.object
        .write()
        .unwrap()
        .remove_upgrade_mask(mask(UNRELATED));
    assert!(a.applied(), "unrelated reset retains execution");
    a.object.write().unwrap().remove_upgrade_mask(mask(CHEM));
    assert!(!a.applied() && b.applied());
    assert!(a.armored(), "C++ reset has no inverse armor implementation");
    assert_eq!(a.decal(), TerrainDecalType::ChemSuit);
    a.apply(CHEM);
    assert!(a.applied() && b.applied());
    drop(a);
    assert!(
        b.applied() && b.armored(),
        "retiring one same-ID module cannot reset another"
    );
}

#[test]
fn installed_chemical_suits_without_a_body_still_apply_decal_and_execution() {
    if !child(concat!(
        module_path!(),
        "::installed_chemical_suits_without_a_body_still_apply_decal_and_execution"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    // CPP ArmorUpgrade.cpp:70–80 guards only the body flag. The Chemical
    // Suits branch and UpgradeMux execution follow even without a body.
    let owner = Installed::with_body("OwnedArmorChemicalNoBody", CHEM, "", false);
    assert!(owner.object.read().unwrap().get_body_module().is_none());
    assert!(!owner.applied());
    assert_ne!(owner.decal(), TerrainDecalType::ChemSuit);
    owner.apply(CHEM);
    assert!(owner.object.read().unwrap().get_body_module().is_none());
    assert!(owner.applied(), "no body does not cancel the executed flag");
    assert_eq!(owner.decal(), TerrainDecalType::ChemSuit);
    owner
        .object
        .write()
        .unwrap()
        .remove_upgrade_mask(mask(CHEM));
    assert!(!owner.applied());
    assert_eq!(
        owner.decal(),
        TerrainDecalType::ChemSuit,
        "C++ reset does not undo the decal"
    );
    owner.apply(CHEM);
    assert!(owner.applied());
}

struct RegisteredOwner(Arc<RwLock<Object>>);
impl Drop for RegisteredOwner {
    fn drop(&mut self) {
        let registry = &crate::object::registry::OBJECT_REGISTRY;
        if registry
            .get_object(ID)
            .is_some_and(|actual| Arc::ptr_eq(&actual, &self.0))
        {
            registry.unregister_object(ID);
        }
    }
}

#[test]
fn registered_owner_apply_never_relocks_its_current_object_guard() {
    if !child(concat!(
        module_path!(),
        "::registered_owner_apply_never_relocks_its_current_object_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let owner = Installed::new("OwnedArmorRegistered", TRIGGER, "");
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &owner.object);
    let registration = RegisteredOwner(owner.object.clone());
    owner.apply(TRIGGER);
    assert!(owner.applied() && owner.armored());
    drop(registration);
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(ID)
            .is_none()
    );
}

#[derive(Debug)]
struct Effects {
    entry: Arc<ModuleEntry>,
    calls: AtomicUsize,
}
impl FXListManagerInterface for Effects {
    fn do_fx_pos(&self, _: FXListId, _: &Coord3D, _: Option<&glam::Mat4>) {
        panic!("must use actual driving owner");
    }
    fn do_fx_obj(&self, _: FXListId, _: ThingId) {
        panic!("must preserve owner borrow");
    }
    fn do_fx_for_object(&self, _: FXListId, owner: &Object) {
        assert_eq!(
            owner.get_template().get_name().as_str(),
            "OwnedArmorEffects"
        );
        assert!(
            owner
                .modules
                .iter()
                .any(|entry| Arc::ptr_eq(entry, &self.entry))
        );
        assert!(
            self.entry.module.try_lock().is_ok(),
            "FX runs without the installed module guard"
        );
        assert!(
            owner.completed_upgrades().intersects(mask(TRIGGER)),
            "FX precedes RemovesUpgrades"
        );
        let body = owner.get_body_module().unwrap();
        assert!(
            !body
                .lock()
                .unwrap()
                .test_armor_set_flag(ArmorSetType::PlayerUpgrade),
            "FX precedes armor implementation"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn self_removal_and_fx_preserve_cpp_order_outside_the_entry_guard() {
    if !child(concat!(
        module_path!(),
        "::self_removal_and_fx_preserve_cpp_order_outside_the_entry_guard"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let owner = Installed::new(
        "OwnedArmorEffects",
        TRIGGER,
        &format!("RemovesUpgrades = {TRIGGER}\n FXListUpgrade = FX_OwnedArmor"),
    );
    let effects = Arc::new(Effects {
        entry: owner.entry.clone(),
        calls: AtomicUsize::new(0),
    });
    assert!(crate::helpers::register_fx_list_manager(effects.clone()));
    owner.apply(TRIGGER);
    assert!(owner.applied() && owner.armored());
    assert!(
        !owner
            .object
            .read()
            .unwrap()
            .completed_upgrades()
            .intersects(mask(TRIGGER))
    );
    assert_eq!(effects.calls.load(Ordering::SeqCst), 1);
    owner.apply(TRIGGER);
    assert_eq!(
        effects.calls.load(Ordering::SeqCst),
        1,
        "executed suppresses repeat FX"
    );
}

#[test]
fn actual_authored_all_triggers_and_conflicts_gate_execution() {
    if !child(concat!(
        module_path!(),
        "::actual_authored_all_triggers_and_conflicts_gate_execution"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let owner = Installed::new(
        "OwnedArmorRequireAll",
        &format!("{TRIGGER} {SECOND}"),
        &format!("RequiresAllTriggers = Yes\n ConflictsWith = {CONFLICT}"),
    );
    // Explicit masks exercise the actual installed dispatch, preserving
    // UpgradeMux's complete-key contract. Object::give_upgrade currently has
    // a separate tracked aggregate-mask bug (CPP Object.cpp2410–2436/4474).
    owner
        .object
        .write()
        .unwrap()
        .apply_upgrade_modules(mask(TRIGGER));
    assert!(!owner.applied() && !owner.armored());
    let complete = mask(TRIGGER) | mask(SECOND);
    owner
        .object
        .write()
        .unwrap()
        .apply_upgrade_modules(complete | mask(CONFLICT));
    assert!(!owner.applied() && !owner.armored());
    owner
        .object
        .write()
        .unwrap()
        .apply_upgrade_modules(complete);
    assert!(
        owner.applied() && owner.armored(),
        "both triggers and no conflict admit the module"
    );
}

#[test]
fn crc_and_xfer_match_cpp_and_do_not_replay_armor_after_load() {
    if !child(concat!(
        module_path!(),
        "::crc_and_xfer_match_cpp_and_do_not_replay_armor_after_load"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let a = Installed::new("OwnedArmorSave", TRIGGER, "");
    let b = Installed::new("OwnedArmorLoad", TRIGGER, "");
    assert_eq!(a.bytes(true), vec![1, 0]);
    assert_eq!(a.bytes(false), vec![1, 1, 1, 1, 1, 1, 0]);
    a.apply(TRIGGER);
    assert_eq!(a.bytes(true), vec![1, 1]);
    let bytes = a.bytes(false);
    assert_eq!(bytes, vec![1, 1, 1, 1, 1, 1, 1]);
    b.entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap();
        module.load_post_process().unwrap();
    });
    assert!(
        b.applied() && !b.armored(),
        "post-load only restores execution, not implementation side effects"
    );
    b.object.write().unwrap().remove_upgrade_mask(mask(TRIGGER));
    assert!(!b.applied() && !b.armored());
    b.apply(TRIGGER);
    assert!(b.applied() && b.armored());
}

#[test]
fn chem_suit_decal_uses_module_triggered_by() {
    let mut chem = ArmorUpgradeModuleData::default();
    chem.upgrade_mux_data
        .activation_upgrade_names
        .push(AsciiString::from(CHEM));
    assert!(chem.upgrade_mux_data.is_triggered_by(CHEM));
    let other = ArmorUpgradeModuleData::default();
    assert!(!other.upgrade_mux_data.is_triggered_by(CHEM));
    assert!(!mux_can_upgrade(
        &other.upgrade_mux_data,
        false,
        UpgradeMaskType::from_bits_retain(1)
    ));
}

/// Real installed Rust W3D hook boundary; the client deliberately reads the
/// same Drawable/entry and changes shadow state before descriptor preparation.
/// The current ID-based bridge's release is idempotent on absent resources.
/// This verifies Rust hook order and guard release, not a C++ first-allocation
/// release callback: CPP2709 only releases when m_terrainDecal already exists.
struct ReentrantDecals {
    drawable: Arc<RwLock<Drawable>>,
    first: crate::object::drawable::DrawableModuleHandle,
    events: std::sync::Mutex<Vec<(bool, Option<crate::object::draw::TerrainDecalDesc>)>>,
}
impl crate::object::draw::TerrainDecalClient for ReentrantDecals {
    fn release(&self, id: ObjectID) {
        assert_eq!(id, ID);
        self.drawable.write().unwrap().set_instance_scale(2.0);
        self.first.with_module(|module| {
            let model = module
                .as_any_mut()
                .downcast_mut::<crate::object::draw::W3DModelDraw>()
                .unwrap();
            crate::object::draw::draw_module::DrawModule::set_shadows_enabled(model, false);
        });
        self.events.lock().unwrap().push((false, None));
    }
    fn set_decal(&self, desc: &crate::object::draw::TerrainDecalDesc) {
        assert_eq!(self.drawable.read().unwrap().get_instance_scale(), 2.0);
        self.first.with_module(|module| {
            assert!(module.as_any().is::<crate::object::draw::W3DModelDraw>())
        });
        self.events.lock().unwrap().push((true, Some(desc.clone())));
    }
    fn set_size(&self, _: ObjectID, _: f32, _: f32) {}
    fn set_opacity(&self, _: ObjectID, _: f32) {}
    fn set_pose(&self, _: ObjectID, _: Coord3D, _: f32) {}
    fn set_shrouded(&self, _: ObjectID, _: bool) {}
    fn set_shadow_enabled(&self, _: ObjectID, _: bool) {}
}

#[test]
fn chemical_suits_real_w3d_callback_releases_guards_and_precedes_description() {
    if !child(concat!(
        module_path!(),
        "::chemical_suits_real_w3d_callback_releases_guards_and_precedes_description"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    let owner = Installed::new("OwnedArmorRealDecal", CHEM, "");
    owner
        .drawable
        .write()
        .unwrap()
        .set_transform(glam::Mat4::from_rotation_translation(
            glam::Quat::from_rotation_z(0.5),
            glam::Vec3::new(11.0, 22.0, 33.0),
        ));
    let data = crate::object::draw::W3DModelDrawModuleData::new();
    let mut model = crate::object::draw::W3DModelDraw::new(data.clone());
    model.bind_owner_id(ID);
    crate::object::draw::draw_module::DrawModule::set_shadows_enabled(&mut model, true);
    let first = owner.drawable.write().unwrap().add_module(
        game_engine::common::thing::module::ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        "PrimaryChemSuitDraw".into(),
        Arc::new(data),
        Box::new(model),
    );
    let data = crate::object::draw::W3DModelDrawModuleData::new();
    let mut second = crate::object::draw::W3DModelDraw::new(data.clone());
    second.bind_owner_id(ID);
    let second = owner.drawable.write().unwrap().add_module(
        game_engine::common::thing::module::ModuleInterfaceType::DRAW,
        "W3DModelDraw".into(),
        "SecondaryMustNotStack".into(),
        Arc::new(data),
        Box::new(second),
    );
    let client = Arc::new(ReentrantDecals {
        drawable: owner.drawable.clone(),
        first,
        events: std::sync::Mutex::new(Vec::new()),
    });
    crate::object::draw::register_terrain_decal_client(client.clone());
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &owner.object);
    let registration = RegisteredOwner(owner.object.clone());
    owner.apply(CHEM);
    assert!(owner.applied() && owner.armored());
    let events = client.events.lock().unwrap();
    assert_eq!(
        events.len(),
        2,
        "only the first DRAW uses the existing Rust release/set hook boundary"
    );
    assert!(!events[0].0 && events[1].0);
    let desc = events[1].1.as_ref().unwrap();
    assert_eq!(desc.object_id, ID);
    assert_eq!(desc.texture_name, "EXChemSuit");
    assert_eq!(
        (desc.size_x, desc.size_y, desc.offset_x, desc.offset_y),
        (18.0, 23.0, 4.0, -7.0)
    );
    assert_eq!(
        (desc.position.x, desc.position.y, desc.position.z),
        (11.0, 22.0, 33.0)
    );
    assert!((desc.angle - 0.5).abs() < 1.0e-6);

    assert!(
        !desc.shadow_enabled,
        "Rust description observes the release hook's live model change"
    );
    assert!(!desc.is_unit_blob);
    drop(events);
    second.with_module(|module| assert!(module.as_any().is::<crate::object::draw::W3DModelDraw>()));
    owner.apply(CHEM);
    assert_eq!(
        client.events.lock().unwrap().len(),
        2,
        "executed repeated grant has no visual effects"
    );
    drop(registration);
    // The recorder owns exactly the fixture Drawable; process lifetime ends
    // before any unrelated test can reuse this one-shot hook.
}
