//! Actual parsed assets -> Common Object INI -> authored ModuleFactory -> the
//! exact cached body. Numeric owner lookup never supplies the definition.

use super::*;
use crate::common::{Coord3D, ObjectStatusMaskType};
use crate::contain_module_overrides::with_active_body_binding_for_test as with_active;
use crate::damage::{DamageInfo, DamageInfoInput, DamageType};
use crate::helpers::TheThingFactory;
use crate::object::body::body_module::ArmorSetType;
use crate::object::body::body_module::BodyModuleInterface;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::object::{ModuleEntry, Object};
use game_engine::common::ini::INI;
use game_engine::common::ini::INILoadType;
use game_engine::common::ini::ini_damage_fx::{DamageType as FxDamageType, get_damage_fx_store};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module::{
    BaseModuleData, Module, ModuleData, ModuleInterfaceType, ModuleType, Thing,
};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::any::Any;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::RwLock;

const ID: ObjectID = 0x7B3D_0081;
const PROBE: &str = "AuthoredArmorInstallationProbe";

#[track_caller]
fn assert_damage_close(actual: f32, expected: f32) {
    // Authored percentages are parsed as f32 * 0.01 before damage scaling.
    assert!(
        (actual - expected).abs() <= 1.0e-4,
        "damage mismatch: expected {expected}, got {actual}"
    );
}
const KINDS: [&str; 6] = [
    "ActiveBody",
    "StructureBody",
    "HighlanderBody",
    "ImmortalBody",
    "HiveStructureBody",
    "UndeadBody",
];

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_AUTHORED_ARMOR_CHILD",
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

struct Assets(PathBuf);
impl Assets {
    fn load() -> Self {
        let path = std::env::temp_dir().join(format!(
            "generals-authored-armor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&path).unwrap();
        let mut armor = String::new();
        let mut fx = String::new();
        for (side, percentages) in [("A", [25, 10, 15, 5]), ("B", [80, 60, 70, 30])] {
            for (kind, percent) in ["Base", "Upgrade", "Hero", "Combined"]
                .into_iter()
                .zip(percentages)
            {
                armor.push_str(&format!(
                    "Armor AuthoredArm{side}_{kind}\n Armor = DEFAULT {percent}%\nEnd\n"
                ));
                fx.push_str(&format!(
                    "DamageFX AuthoredFx{side}_{kind}\n ThrottleTime = DEFAULT 500\nEnd\n"
                ));
            }
        }
        let fx_path = path.join("DamageFX.ini");
        fs::write(&fx_path, fx).unwrap();
        // GameEngine.cpp:444-445: parse DamageFX, then Armor, before Object INI.
        INI::new().load(&fx_path, INILoadType::Overwrite).unwrap();
        let armor_path = path.join("Armor.ini");
        fs::write(&armor_path, armor).unwrap();
        assert_eq!(
            crate::object::armor::load_armor_templates_from_path(&armor_path).unwrap(),
            8
        );
        for side in ["A", "B"] {
            for kind in ["Base", "Upgrade", "Hero", "Combined"] {
                assert_eq!(
                    get_damage_fx_store()
                        .unwrap()
                        .find_damage_fx(&format!("AuthoredFx{side}_{kind}"))
                        .unwrap()
                        .get_damage_fx_throttle_time(FxDamageType::SmallArms, None),
                    15
                );
            }
        }
        assert!(
            ensure_thing_factory_exists(),
            "real empty factory, no bulk retail shell loader"
        );
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        get_module_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .add_module_internal(
                Some(probe_factory),
                Some(probe_data_factory),
                ModuleType::Behavior,
                PROBE,
                ModuleInterfaceType::NONE,
            );
        Self(path)
    }
}
impl Drop for Assets {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn input() -> DamageInfoInput {
    DamageInfoInput {
        damage_type: DamageType::SmallArms,
        amount: 100.0,
        ..Default::default()
    }
}

// Test-only, explicitly consumed owner loan. It supplies no body/template state
// and never admits an Object or Unit into a registry.
struct ProbeData {
    base: BaseModuleData,
    loan: Mutex<Option<(Arc<RwLock<Object>>, f32)>>,
}
impl std::fmt::Debug for ProbeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProbeData").finish_non_exhaustive()
    }
}
impl ModuleData for ProbeData {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: u32) {
        self.base.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> u32 {
        self.base.get_module_tag_name_key()
    }
}
impl Snapshotable for ProbeData {
    fn crc(&self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
struct Probe {
    data: Arc<dyn ModuleData>,
    observed: bool,
}
impl Module for Probe {
    fn get_module_name_key(&self) -> u32 {
        NameKeyGenerator::name_to_key(PROBE)
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
    fn on_object_created(&mut self) {
        let (owner, expected) = self
            .data
            .as_ref()
            .as_any()
            .downcast_ref::<ProbeData>()
            .unwrap()
            .loan
            .lock()
            .unwrap()
            .take()
            .unwrap();
        let owner = owner
            .try_read()
            .expect("exact owner loan released for callbacks");
        assert!(!owner.modules_ready);
        let cached = owner.get_body().expect("cached before callback");
        assert_damage_close(
            cached.lock().unwrap().estimate_damage(&input()).unwrap(),
            expected,
        );
        self.observed = true;
    }
}
impl Snapshotable for Probe {
    fn crc(&self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
fn probe_factory(_: Arc<dyn Thing>, data: Arc<dyn ModuleData>) -> Box<dyn Module> {
    let (owner, expected) = {
        let loan = data
            .as_ref()
            .as_any()
            .downcast_ref::<ProbeData>()
            .unwrap()
            .loan
            .lock()
            .unwrap();
        let (owner, expected) = loan.as_ref().unwrap();
        (owner.clone(), *expected)
    };
    let owner = owner
        .try_read()
        .expect("body installation released exact owner");
    let cached = owner
        .get_body()
        .expect("C++ caches before next authored constructor");
    // Authored armor is resolved at the C++ constructor point.
    assert_damage_close(
        cached.lock().unwrap().estimate_damage(&input()).unwrap(),
        expected,
    );
    Box::new(Probe {
        data,
        observed: false,
    })
}
fn probe_data_factory(_: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(ProbeData {
        base: BaseModuleData::new(),
        loan: Mutex::new(None),
    })
}

fn template(
    name: &str,
    kind: &str,
    side: &str,
    probe: bool,
) -> Arc<dyn crate::common::ThingTemplate> {
    let mut sets = String::new();
    // A one-bit request prefers its exact one-bit set over this earlier
    // two-bit set; a two-bit request chooses the combined set.
    for (conditions, label) in [
        ("NONE", "Base"),
        ("HERO PLAYER_UPGRADE", "Combined"),
        ("PLAYER_UPGRADE", "Upgrade"),
        ("HERO", "Hero"),
    ] {
        sets.push_str(&format!(
            " ArmorSet\n Conditions = {conditions}\n Armor = AuthoredArm{side}_{label}\n DamageFX = AuthoredFx{side}_{label}\n End\n"
        ));
    }
    sets.push_str(
        " ArmorSet\n Conditions = CRATE_UPGRADE_ONE\n Armor = None\n DamageFX = None\n End\n",
    );
    let callback = if probe {
        format!(" Behavior = {PROBE} ArmorProbe\n End\n")
    } else {
        String::new()
    };
    let kindof = if matches!(kind, "StructureBody" | "HiveStructureBody") {
        "STRUCTURE"
    } else {
        "INERT"
    };
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&format!(
        "Object {name}\n KindOf = {kindof}\n{sets} Body = {kind} AuthoredBody\n MaxHealth = 500\n InitialHealth = 500\n End\n{callback}End\n"
    )), 1, "actual Common authored Object definition");
    TheThingFactory::find_template(name).unwrap()
}

struct Installed {
    owner: Arc<RwLock<Object>>,
    entry: Arc<ModuleEntry>,
    cached: Arc<Mutex<dyn BodyModuleInterface>>,
}
impl Installed {
    fn new(name: &str, kind: &str, side: &str, id: ObjectID, probe: bool) -> Self {
        let template = template(name, kind, side, probe);
        let owner = Arc::new(RwLock::new(Object::new_raw(
            template.clone(),
            id,
            ObjectStatusMaskType::NONE,
            None,
        )));
        assert!(
            owner.read().unwrap().get_body().is_none(),
            "raw construction is inert"
        );
        if probe {
            let data = template
                .get_behavior_module_info()
                .iter()
                .find(|entry| entry.name.as_str() == PROBE)
                .unwrap();
            *data
                .data
                .as_ref()
                .as_any()
                .downcast_ref::<ProbeData>()
                .unwrap()
                .loan
                .lock()
                .unwrap() = Some((owner.clone(), if side == "A" { 25.0 } else { 80.0 }));
        }
        Object::init_modules_for(&owner, template.as_ref()).unwrap();
        Self::from_owner(owner, kind)
    }
    fn from_owner(owner: Arc<RwLock<Object>>, kind: &str) -> Self {
        let (entry, cached) = {
            let object = owner.read().unwrap();
            (
                object.find_module_by_name(kind).unwrap(),
                object.get_body().unwrap(),
            )
        };
        Self {
            owner,
            entry,
            cached,
        }
    }
    fn names(&self) -> (Option<AsciiString>, Option<AsciiString>) {
        let definition = self.owner.read().unwrap().get_template().clone();
        self.entry.with_module(|module| {
            with_active(module, &self.cached, |active| {
                assert!(
                    active.is_bound_to_template_for_test(&definition),
                    "the body retains the exact immutable Object definition handle"
                );
                (
                    active.current_armor_template_name(),
                    active.current_damage_fx_name(),
                )
            })
        })
    }
    fn hit(&self, expected: f32) {
        let mut info = DamageInfo::default();
        info.input = input();
        let mut body = self.cached.lock().unwrap();
        assert_damage_close(body.estimate_damage(&info.input).unwrap(), expected);
        let before = body.get_health();
        body.attempt_damage(&mut info).unwrap();
        assert_damage_close(info.output.actual_damage_dealt, expected);
        assert_damage_close(body.get_health(), before - expected);
    }
    fn expect_names(&self, side: &str, label: &str) {
        assert_eq!(
            self.names(),
            (
                Some(AsciiString::from(
                    format!("AuthoredArm{side}_{label}").as_str()
                )),
                Some(AsciiString::from(
                    format!("AuthoredFx{side}_{label}").as_str()
                )),
            )
        );
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
    fn save_cache(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.cached
            .lock()
            .unwrap()
            .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap();
        bytes
    }
}

#[test]
fn authored_same_id_body_kinds_bind_before_next_constructor_and_scale_default_and_upgrade() {
    if !child(concat!(
        module_path!(),
        "::authored_same_id_body_kinds_bind_before_next_constructor_and_scale_default_and_upgrade"
    )) {
        return;
    }
    let _assets = Assets::load();
    for kind in KINDS {
        let a = Installed::new(&format!("ArmorA_{kind}"), kind, "A", ID, true);
        let b = Installed::new(&format!("ArmorB_{kind}"), kind, "B", ID, true);
        assert!(!Arc::ptr_eq(&a.cached, &b.cached));
        for (body, side, base, upgrade) in [(&a, "A", 25.0, 10.0), (&b, "B", 80.0, 60.0)] {
            body.owner
                .read()
                .unwrap()
                .find_module_by_name(PROBE)
                .unwrap()
                .with_module(|module| {
                    assert!(module.as_any().downcast_ref::<Probe>().unwrap().observed)
                });
            body.expect_names(side, "Base");
            body.hit(base);
            body.cached
                .lock()
                .unwrap()
                .set_armor_set_flag(ArmorSetType::PlayerUpgrade)
                .unwrap();
            body.hit(upgrade);
            body.expect_names(side, "Upgrade");
        }
        assert!(
            crate::object::registry::OBJECT_REGISTRY
                .get_object(ID)
                .is_none()
        );
    }
}

#[test]
fn authored_sparse_flags_choose_exact_then_combined_and_none_clears_previous_armor() {
    if !child(concat!(
        module_path!(),
        "::authored_sparse_flags_choose_exact_then_combined_and_none_clears_previous_armor"
    )) {
        return;
    }
    let _assets = Assets::load();
    let body = Installed::new("ArmorSparseA", "ActiveBody", "A", ID, false);
    body.cached
        .lock()
        .unwrap()
        .set_armor_set_flag(ArmorSetType::PlayerUpgrade)
        .unwrap();
    body.hit(10.0);
    body.expect_names("A", "Upgrade");
    body.cached
        .lock()
        .unwrap()
        .set_armor_set_flag(ArmorSetType::Hero)
        .unwrap();
    body.hit(5.0);
    body.expect_names("A", "Combined");
    body.cached
        .lock()
        .unwrap()
        .clear_armor_set_flag(ArmorSetType::PlayerUpgrade)
        .unwrap();
    body.hit(15.0);
    body.expect_names("A", "Hero");
    body.cached
        .lock()
        .unwrap()
        .clear_armor_set_flag(ArmorSetType::Hero)
        .unwrap();
    body.cached
        .lock()
        .unwrap()
        .set_armor_set_flag(ArmorSetType::Veteran)
        .unwrap();
    body.hit(25.0);
    body.expect_names("A", "Base");
    body.cached
        .lock()
        .unwrap()
        .clear_armor_set_flag(ArmorSetType::Veteran)
        .unwrap();
    body.cached
        .lock()
        .unwrap()
        .set_armor_set_flag(ArmorSetType::CrateUpgradeOne)
        .unwrap();
    body.hit(100.0);
    assert_eq!(
        body.names(),
        (None, None),
        "selected None set clears armor and FX"
    );
    body.cached
        .lock()
        .unwrap()
        .clear_armor_set_flag(ArmorSetType::CrateUpgradeOne)
        .unwrap();
    body.hit(25.0);
    body.expect_names("A", "Base");
}

#[test]
fn authored_module_restore_keeps_receiving_definition_and_the_same_cached_runtime() {
    if !child(concat!(
        module_path!(),
        "::authored_module_restore_keeps_receiving_definition_and_the_same_cached_runtime"
    )) {
        return;
    }
    let _assets = Assets::load();
    for kind in KINDS {
        let source = Installed::new(&format!("ArmorSource_{kind}"), kind, "A", ID, false);
        let receiving = Installed::new(&format!("ArmorReceiving_{kind}"), kind, "B", ID, false);
        {
            let mut body = source.cached.lock().unwrap();
            body.set_armor_set_flag(ArmorSetType::Hero).unwrap();
            body.set_armor_set_flag(ArmorSetType::PlayerUpgrade)
                .unwrap();
        }
        source.hit(5.0);
        let saved = source.save();
        assert_eq!(
            source.save_cache(),
            saved,
            "canonical derived Snapshot dispatch: {kind}"
        );
        receiving.entry.with_module(|module| {
            module
                .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
                .unwrap();
            module.load_post_process().unwrap();
        });
        assert_eq!(receiving.save_cache(), saved);
        assert_eq!(receiving.save(), saved);
        assert!(Arc::ptr_eq(
            &receiving.cached,
            &receiving.owner.read().unwrap().get_body().unwrap()
        ));
        receiving.hit(30.0);
        receiving.expect_names("B", "Combined");
        assert_damage_close(
            source
                .cached
                .lock()
                .unwrap()
                .estimate_damage(&input())
                .unwrap(),
            5.0,
        );
    }
}

#[test]
fn admitted_factory_and_unadmitted_same_id_bind_their_actual_templates() {
    if !child(concat!(
        module_path!(),
        "::admitted_factory_and_unadmitted_same_id_bind_their_actual_templates"
    )) {
        return;
    }
    let _assets = Assets::load();
    let _definition = template("ArmorPublishedA", "ActiveBody", "A", false);
    let mut factory = ObjectFactory::new();
    let id = factory
        .create_object(
            "ArmorPublishedA",
            Coord3D::default(),
            None,
            ObjectCreationFlags::NO_DRAWABLE | ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let published = Installed::from_owner(
        factory.get_object(id).unwrap().get_base_object().unwrap(),
        "ActiveBody",
    );
    let unadmitted = Installed::new("ArmorUnadmittedB", "ActiveBody", "B", id, false);
    assert!(!Arc::ptr_eq(&published.cached, &unadmitted.cached));
    assert_damage_close(
        published
            .cached
            .lock()
            .unwrap()
            .estimate_damage(&input())
            .unwrap(),
        25.0,
    );
    assert_damage_close(
        unadmitted
            .cached
            .lock()
            .unwrap()
            .estimate_damage(&input())
            .unwrap(),
        80.0,
    );
    published.expect_names("A", "Base");
    unadmitted.expect_names("B", "Base");
    assert_eq!(
        published.cached.lock().unwrap().get_health(),
        500.0,
        "constructing a same-ID owner cannot mutate the published body"
    );
}

#[test]
fn authored_missing_named_armor_errors_before_cache_or_callbacks() {
    if !child(concat!(
        module_path!(),
        "::authored_missing_named_armor_errors_before_cache_or_callbacks"
    )) {
        return;
    }
    let _assets = Assets::load();
    let definition = template("ArmorMissingNamed", "ActiveBody", "Missing", false);
    let owner = Arc::new(RwLock::new(Object::new_raw(
        definition.clone(),
        ID,
        ObjectStatusMaskType::NONE,
        None,
    )));
    let error = Object::init_modules_for(&owner, definition.as_ref()).unwrap_err();
    assert!(
        error.to_string().contains("AuthoredArmMissing_Base"),
        "retain the actual missing reference: {error}"
    );
    assert!(owner.read().unwrap().get_body().is_none());
    assert!(!owner.read().unwrap().modules_ready);
}
