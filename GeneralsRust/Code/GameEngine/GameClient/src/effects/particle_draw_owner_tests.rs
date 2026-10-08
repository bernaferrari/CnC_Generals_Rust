//! Real factory-installed draw callback and particle manager regressions.
//! Current animated render-object pose is verified separately.
use super::*;
use gamelogic::drawable::Drawable as _;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_MARKER: &str = "GENERALS_PARTICLE_DRAW_OWNER_FACTORY_CHILD";
const TEST_NAME: &str = "factory_admitted_microwave_draw_creates_real_attached_particle_rows";

/// This callback takes the child route before any one-shot managers/bridges are
/// installed. Exact-filter output is checked so a stale test path cannot pass.
fn run_as_bounded_test_child() -> bool {
    if std::env::var_os(CHILD_MARKER).is_some() {
        return true;
    }

    let crate_prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    let test_name = format!("{}::{TEST_NAME}", module_path!());
    let test_name = test_name
        .strip_prefix(crate_prefix)
        .expect("module path starts with this test crate");
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .arg("--ignored")
        .env(CHILD_MARKER, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bounded child test");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let stdout_thread = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).expect("read child stdout");
        bytes
    });
    let stderr_thread = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).expect("read child stderr");
        bytes
    });

    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll child") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stdout = String::from_utf8_lossy(&stdout_thread.join().unwrap()).into_owned();
            let stderr = String::from_utf8_lossy(&stderr_thread.join().unwrap()).into_owned();
            panic!("child timed out; stdout={stdout}; stderr={stderr}");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = String::from_utf8_lossy(&stdout_thread.join().unwrap()).into_owned();
    let stderr = String::from_utf8_lossy(&stderr_thread.join().unwrap()).into_owned();
    assert!(
        status.success(),
        "child failed; stdout={stdout}; stderr={stderr}"
    );
    assert!(
        stdout.contains("1 passed; 0 failed"),
        "exact child test did not report one passing test; stdout={stdout}; stderr={stderr}"
    );
    false
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("GameClient manifest is nested under repository root")
        .to_path_buf()
}

#[test]
#[ignore = "requires licensed local AmericaVehicle.ini and ParticleSystem.ini; run explicitly with --ignored"]
fn factory_admitted_microwave_draw_creates_real_attached_particle_rows() {
    if !run_as_bounded_test_child() {
        return;
    }

    // All manager, template, registry, and module-factory state lives in this
    // subprocess. Do not install a fake particle manager or synthetic template.
    let root = repo_root();
    let vehicle_ini =
        root.join("GeneralsRust/windows_game/extracted_big_files_v2/INI/Object/AmericaVehicle.ini");
    let particle_ini =
        root.join("GeneralsRust/windows_game/extracted_big_files_v2/INI/ParticleSystem.ini");
    assert!(
        vehicle_ini.is_file(),
        "licensed local vehicle INI required for this integration fixture: {}",
        vehicle_ini.display()
    );
    assert!(
        particle_ini.is_file(),
        "licensed local particle INI required for this integration fixture: {}",
        particle_ini.display()
    );

    if game_engine::common::thing::thing_factory::get_thing_factory()
        .expect("ThingFactory lock")
        .is_none()
    {
        game_engine::common::thing::thing_factory::init_thing_factory()
            .expect("initialize actual ThingFactory");
    }
    if game_engine::common::thing::module_factory::get_module_factory()
        .expect("ModuleFactory lock")
        .is_none()
    {
        game_engine::common::thing::module_factory::init_module_factory()
            .expect("initialize actual ModuleFactory");
    }
    gamelogic::contain_module_overrides::ensure_module_overrides_installed()
        .expect("register real GameLogic draw/module implementations");

    let vehicle_source = std::fs::read_to_string(&vehicle_ini).expect("read retail vehicle INI");
    {
        let mut things = game_engine::common::thing::thing_factory::get_thing_factory()
            .expect("ThingFactory lock");
        let count = things
            .as_mut()
            .expect("ThingFactory initialized")
            .load_ini_text(&vehicle_source);
        assert!(count > 0, "retail object INI parsed actual definitions");
        assert!(
            things
                .as_ref()
                .unwrap()
                .find_template("AmericaTankMicrowave", false)
                .is_some(),
            "retail AmericaTankMicrowave template was loaded"
        );
    }

    let mut particle_manager = ParticleSystemManager::new();
    let parsed = crate::effects::particle_ini_loader::ParticleSystemINIParser::default()
        .load_particle_system_definitions(&particle_ini, &mut particle_manager)
        .expect("parse local retail ParticleSystem.ini");
    assert!(parsed > 0, "retail particle templates parsed");
    for name in ["MicrowaveLenzflare", "MicrowaveRotisserie"] {
        assert!(
            particle_manager.find_template(name).is_some(),
            "required retail particle template {name} exists"
        );
    }
    *PARTICLE_SYSTEM_MANAGER
        .write()
        .expect("install child-local actual particle manager") = Some(particle_manager);
    register_particle_system_manager_bridge();

    let mut object_factory = gamelogic::object::object_factory::ObjectFactory::new();
    // The first real factory object intentionally has no Drawable, so the
    // target object's Drawable ID is not assumed to equal its Object ID.
    // This makes the C++ attachToDrawable identity assertion meaningful.
    let _preceding_object = object_factory
        .create_object(
            "AmericaTankMicrowave",
            gamelogic::common::Coord3D::ZERO,
            None,
            gamelogic::object::object_factory::ObjectCreationFlags::NO_DRAWABLE,
        )
        .expect("create preceding retail object without a Drawable");
    let object_id = object_factory
        .create_object(
            "AmericaTankMicrowave",
            gamelogic::common::Coord3D::ZERO,
            None,
            gamelogic::object::object_factory::ObjectCreationFlags::empty(),
        )
        .expect("create actual retail drawable object");
    let target = object_factory
        .get_object(object_id)
        .expect("factory retains object")
        .get_base_object()
        .expect("factory object has base Object");
    let drawable = target
        .read()
        .expect("owner Object read")
        .get_drawable()
        .expect("factory installed Drawable");
    let drawable_id = drawable.read().expect("Drawable read").get_drawable_id();
    assert_ne!(
        drawable_id, object_id,
        "fixture distinguishes Drawable/Object IDs"
    );

    let entries = drawable.read().expect("Drawable read").modules();
    let tank = entries
        .iter()
        .find(|entry| entry.name().as_str() == "W3DTankDraw")
        .expect("real factory installed authored W3DTankDraw wrapper");
    assert!(
        tank.with_module(|module| { module.as_any().is::<gamelogic::object::draw::W3DTankDraw>() })
    );
    tank.with_module_data(|module_data| {
        let data = module_data
            .as_any()
            .downcast_ref::<gamelogic::object::draw::W3DTankDrawModuleData>()
            .expect("factory wrapper has authored W3DTankDraw module data");
        let normal = data
            .base
            .condition_states
            .iter()
            .find(|state| {
                state.model_name.as_str() == "avthundrblt"
                    && state.conditions_yes.iter().any(|flags| flags.is_empty())
            })
            .expect("retail normal AVTHUNDRBLT condition state");
        let rows: Vec<_> = normal
            .particle_sys_bones
            .iter()
            .map(|row| (row.bone_name.as_str(), row.particle_system.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("projectorglow09", "MicrowaveLenzflare"),
                ("none", "MicrowaveRotisserie"),
            ],
            "factory data preserves C++ bone normalization, template identity, and authored row order"
        );
        let manager_guard = get_particle_system_manager().expect("particle manager read");
        let manager = manager_guard.as_ref().expect("real manager installed");
        for (_, name) in rows {
            assert!(manager.find_template(name).is_some(), "authored row resolves real template {name}");
        }
    });

    let initial_ids = {
        let guard = get_particle_system_manager().expect("particle manager read");
        guard
            .as_ref()
            .expect("real manager installed")
            .active_system_order
            .clone()
    };

    // Production-shaped boundary: hold the owning Drawable write guard and
    // enter the normal installed DRAW-module dispatch. A reentrant owner lookup
    // deadlocks this child; a passed borrowed owner must complete.
    eprintln!("PARTICLE_DRAW_OWNER_STAGE before ordinary Drawable::draw");
    {
        let mut draw = drawable.write().expect("owning Drawable write guard");
        draw.draw(None);
    }

    let (first_ids, first_systems) = {
        let manager_guard = get_particle_system_manager().expect("particle manager read");
        let manager = manager_guard.as_ref().expect("real manager installed");
        let ids = manager.active_system_order.clone();
        assert_eq!(
            &ids[..initial_ids.len()],
            initial_ids.as_slice(),
            "draw preserves existing factory-created tread emitter order"
        );
        let summaries = ids
            .iter()
            .skip(initial_ids.len())
            .map(|id| {
                let system = manager.find_particle_system(*id).expect("created system");
                (
                    system.template().name().to_owned(),
                    system.attached_drawable_id(),
                    system.attached_object(),
                    system.is_saveable(),
                    system.position(),
                )
            })
            .collect::<Vec<_>>();
        (ids, summaries)
    };
    assert_eq!(
        first_systems
            .iter()
            .map(|system| system.0.as_str())
            .collect::<Vec<_>>(),
        ["MicrowaveLenzflare", "MicrowaveRotisserie"]
    );
    for (_, attached_drawable, attached_object, saveable, _) in &first_systems {
        assert_eq!(
            *attached_drawable,
            crate::core::DrawableId(drawable_id),
            "CPP attachToDrawable records this exact Drawable ID"
        );
        assert_eq!(
            *attached_object, None,
            "drawable attachment does not also become an Object attachment"
        );
        assert!(!saveable, "model-owned systems are recreated on load");
    }
    // No live W3D HTree is available in this test harness. The NONE row is
    // nevertheless a real index-zero case and C++ initializes its origin.
    assert_eq!(first_systems[1].4, glam::Vec3::ZERO);

    // A second ordinary callback must not recalculate/recreate the state rows.
    {
        let mut draw = drawable.write().expect("owning Drawable write guard");
        draw.draw(None);
    }
    let second_ids = {
        let manager_guard = get_particle_system_manager().expect("particle manager read");
        manager_guard
            .as_ref()
            .expect("real manager installed")
            .active_system_order
            .clone()
    };
    assert_eq!(second_ids, first_ids);

    particle_owner_controls(&mut object_factory, object_id, &drawable);
}

type LogicDrawableHandle = std::sync::Arc<std::sync::RwLock<gamelogic::object::drawable::Drawable>>;

struct AdmittedDrawable {
    object_id: u32,
    drawable: LogicDrawableHandle,
}

fn admit_microwave(
    factory: &mut gamelogic::object::object_factory::ObjectFactory,
) -> AdmittedDrawable {
    let object_id = factory
        .create_object(
            "AmericaTankMicrowave",
            gamelogic::common::Coord3D::ZERO,
            None,
            gamelogic::object::object_factory::ObjectCreationFlags::empty(),
        )
        .unwrap();
    let object = factory
        .get_object(object_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let drawable = object.read().unwrap().get_drawable().unwrap();
    AdmittedDrawable {
        object_id,
        drawable,
    }
}

fn system_ids() -> Vec<ParticleSystemId> {
    let guard = get_particle_system_manager().unwrap();
    guard.as_ref().unwrap().active_system_order.clone()
}

fn ordinary_draw(drawable: &LogicDrawableHandle) {
    drawable.write().unwrap().draw(None);
}

fn assert_created_rows(drawable: &LogicDrawableHandle, stopped: bool) -> Vec<ParticleSystemId> {
    let before = system_ids();
    ordinary_draw(drawable);
    let after = system_ids();
    assert_eq!(&after[..before.len()], before.as_slice());
    let created = after[before.len()..].to_vec();
    assert_eq!(
        created.len(),
        2,
        "ordinary draw creates exactly its two authored rows"
    );
    let drawable_id = drawable.read().unwrap().get_drawable_id();
    let guard = get_particle_system_manager().unwrap();
    let manager = guard.as_ref().unwrap();
    for (index, id) in created.iter().enumerate() {
        let system = manager.find_particle_system(*id).unwrap();
        assert_eq!(
            system.template().name(),
            ["MicrowaveLenzflare", "MicrowaveRotisserie"][index]
        );
        assert_eq!(
            system.attached_drawable_id(),
            crate::core::DrawableId(drawable_id)
        );
        assert_eq!(system.attached_object(), None);
        assert!(!system.is_saveable());
        assert_eq!(system.is_stopped(), stopped);
    }
    assert_eq!(
        manager.find_particle_system(created[1]).unwrap().position(),
        glam::Vec3::ZERO
    );
    created
}

fn assert_rows_stopped(ids: &[ParticleSystemId], stopped: bool) {
    let guard = get_particle_system_manager().unwrap();
    let manager = guard.as_ref().unwrap();
    for id in ids {
        assert_eq!(
            manager.find_particle_system(*id).unwrap().is_stopped(),
            stopped
        );
    }
}

fn set_model_shroud(drawable: &LogicDrawableHandle, shrouded: bool) {
    use gamelogic::object::draw::DrawModule;
    let entry = drawable
        .read()
        .unwrap()
        .modules()
        .into_iter()
        .find(|entry| entry.name().as_str() == "W3DTankDraw")
        .unwrap();
    entry.with_module(|module| {
        let tank = (module as &mut dyn std::any::Any)
            .downcast_mut::<gamelogic::object::draw::W3DTankDraw>()
            .unwrap();
        tank.set_fully_obscured_by_shroud(shrouded);
    });
}

fn particle_owner_controls(
    factory_a: &mut gamelogic::object::object_factory::ObjectFactory,
    original_object_id: u32,
    drawable_a: &LogicDrawableHandle,
) {
    use gamelogic::common::ModelConditionFlags;
    use gamelogic::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    // The authored damaged/rubble condition has the same two particle rows.
    const DAMAGED_STATE: ModelConditionFlags =
        ModelConditionFlags::REALLYDAMAGED.union(ModelConditionFlags::RUBBLE);
    // CPP DrawableStatus bit; GameLogic's internal draw constant is private.
    const NO_STATE_PARTICLES: u32 = 0x0000_0008;

    let mut factory_b = ObjectFactory::new();
    factory_b
        .create_object(
            "AmericaTankMicrowave",
            gamelogic::common::Coord3D::ZERO,
            None,
            ObjectCreationFlags::NO_DRAWABLE,
        )
        .unwrap();
    let b = admit_microwave(&mut factory_b);
    assert_eq!(
        b.object_id, original_object_id,
        "independent factories reuse ObjectID"
    );
    assert_ne!(
        b.drawable.read().unwrap().get_drawable_id(),
        drawable_a.read().unwrap().get_drawable_id()
    );
    // ID-based registry lookup now names B. Interleave both driving Drawables.
    assert_created_rows(&b.drawable, false);
    let after_b = system_ids();
    ordinary_draw(drawable_a);
    assert_eq!(
        system_ids(),
        after_b,
        "A does not replay already consumed recalc"
    );
    drawable_a
        .write()
        .unwrap()
        .set_model_condition_state(DAMAGED_STATE);
    assert_created_rows(drawable_a, false);
    let after_a = system_ids();
    ordinary_draw(&b.drawable);
    assert_eq!(system_ids(), after_a, "B retains its original systems");

    let no_state = admit_microwave(factory_a);
    no_state
        .drawable
        .write()
        .unwrap()
        .set_drawable_status(NO_STATE_PARTICLES);
    let before = system_ids();
    ordinary_draw(&no_state.drawable);
    assert_eq!(system_ids(), before);
    no_state
        .drawable
        .write()
        .unwrap()
        .clear_drawable_status(NO_STATE_PARTICLES);
    ordinary_draw(&no_state.drawable);
    assert_eq!(
        system_ids(),
        before,
        "clearing status does not replay consumed recalc"
    );
    no_state
        .drawable
        .write()
        .unwrap()
        .set_model_condition_state(DAMAGED_STATE);
    assert_created_rows(&no_state.drawable, false);

    let hidden = admit_microwave(factory_a);
    hidden
        .drawable
        .write()
        .unwrap()
        .set_drawable_hidden(true)
        .unwrap();
    let before = system_ids();
    ordinary_draw(&hidden.drawable);
    assert_eq!(
        system_ids(),
        before,
        "explicit hide suppresses ordinary draw"
    );
    hidden
        .drawable
        .write()
        .unwrap()
        .set_drawable_hidden(false)
        .unwrap();
    let rows = assert_created_rows(&hidden.drawable, false);
    hidden
        .drawable
        .write()
        .unwrap()
        .set_drawable_hidden(true)
        .unwrap();
    assert_rows_stopped(&rows, true);
    hidden
        .drawable
        .write()
        .unwrap()
        .set_drawable_hidden(false)
        .unwrap();
    assert_rows_stopped(&rows, false);

    let invisible = admit_microwave(factory_a);
    invisible.drawable.write().unwrap().set_visible(false);
    assert_created_rows(&invisible.drawable, false);

    let shrouded = admit_microwave(factory_a);
    set_model_shroud(&shrouded.drawable, true);
    let rows = assert_created_rows(&shrouded.drawable, true);
    set_model_shroud(&shrouded.drawable, false);
    assert_rows_stopped(&rows, false);
}
