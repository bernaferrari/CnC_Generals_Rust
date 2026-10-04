//! Driving-frame evidence through actual parsed Object admission, delivery
//! creation and the ordinary Main tick. CPP RadiusDecal.cpp:172-190.
use super::*;
use crate::game_logic::host_radius_decal_update::{
    DELIVERY_DECAL_OPACITY_MAX, DELIVERY_DECAL_OPACITY_MIN, DELIVERY_DECAL_THROB_FRAMES,
    SCUD_STORM_DECAL_TEXTURE,
};

#[cfg(not(target_arch = "wasm32"))]
fn isolated(test: &str, run: impl FnOnce()) {
    use std::io::Read;
    use std::process::Stdio;
    const CHILD: &str = "GENERALS_RADIUS_DECAL_CLOCK_CHILD";
    let module = module_path!().split_once("::").expect("crate prefix").1;
    let exact = format!("{module}::{test}");
    if std::env::var(CHILD).ok().as_deref() == Some(exact.as_str()) {
        run();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
        .env(CHILD, &exact)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real decal clock witness");
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    let mut timeout = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timeout = true;
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let output = readers.map(|r| String::from_utf8_lossy(&r.join().unwrap()).into_owned());
    assert!(!timeout, "real decal witness deadline: {exact}: {output:?}");
    assert!(
        status.success(),
        "real decal witness failed: {exact}: {output:?}"
    );
    assert!(
        output[0].contains("running 1 test") && output[0].contains("1 passed; 0 failed"),
        "exact child must execute one real test: {output:?}"
    );
}
#[cfg(target_arch = "wasm32")]
fn isolated(_: &str, run: impl FnOnce()) {
    run();
}

struct RestoreInputs {
    native_frame: u64,
    draw_icon_ui: bool,
    host_frame: u32,
    authority: GameWorldAuthority,
    store: Option<gamelogic::weapon::WeaponStore>,
}
impl RestoreInputs {
    fn install(native_frame: u64) -> Self {
        let native = gamelogic::system::game_logic::get_game_logic();
        let (previous_frame, previous_draw_icon_ui) = {
            let logic = native.lock().unwrap();
            (logic.get_current_frame(), logic.get_draw_icon_ui())
        };
        let saved = Self {
            native_frame: previous_frame,
            draw_icon_ui: previous_draw_icon_ui,
            host_frame: crate::game_logic::host_historic_bonus::logic_frame(),
            authority: current_gameworld_authority(),
            store: gamelogic::weapon::with_weapon_store_mut(std::mem::take).ok(),
        };
        {
            let mut logic = native.lock().unwrap();
            logic.set_current_frame(native_frame);
            logic.set_draw_icon_ui(true);
        }
        gameworld_authority::publish_gameworld_authority(GameWorldAuthority::DEFAULT_OFF);
        gamelogic::initialize_weapon_store().expect("real authored weapon catalog");
        assert_eq!(
            crate::assets::ini_template_loader::register_weapons_from_ini_text(
                r#"
Weapon RadiusClockOwnerGun
  PrimaryDamage = 1
  AttackRange = 200
  DamageType = SMALL_ARMS
  ProjectileObject = NONE
  WeaponSpeed = 30000000
  DelayBetweenShots = 100000
  ClipSize = 8
  AntiGround = Yes
End
"#
            ),
            1
        );
        saved
    }
}
impl Drop for RestoreInputs {
    fn drop(&mut self) {
        if let Some(previous) = self.store.take() {
            gamelogic::initialize_weapon_store().expect("restore catalog presence");
            gamelogic::weapon::with_weapon_store_mut(|store| *store = previous).unwrap();
        } else {
            gamelogic::weapon::shutdown_weapon_store().unwrap();
        }
        let native = gamelogic::system::game_logic::get_game_logic();
        let mut logic = native.lock().unwrap();
        logic.set_current_frame(self.native_frame);
        logic.set_draw_icon_ui(self.draw_icon_ui);
        drop(logic);
        gameworld_authority::publish_gameworld_authority(self.authority);
        crate::game_logic::host_historic_bonus::set_logic_frame(self.host_frame);
    }
}

fn world(frame: u32) -> (GameLogic, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object GLAScudStormClockOwnerProbe
  KindOf = STRUCTURE SELECTABLE CAN_ATTACK
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
  Behavior = RadiusDecalUpdate ModuleTag_DeliveryDecal
  End
  WeaponSet
    Conditions = None
    Weapon = PRIMARY RadiusClockOwnerGun
  End
End
"#,
            "radius-clock-owner.ini",
        )
        .expect("actual Object INI");
    let definition = parser
        .get_definition("GLAScudStormClockOwnerProbe")
        .unwrap();
    assert!(
        definition
            .behavior_modules
            .iter()
            .any(|m| m.class_name == "RadiusDecalUpdate")
    );
    let template = GameLogic::build_template_from_object_definition(
        "GLAScudStormClockOwnerProbe",
        definition,
        None,
    );
    assert_eq!(
        template.primary_weapon_name.as_deref(),
        Some("RadiusClockOwnerGun")
    );
    let mut world = GameLogic::new();
    world.set_current_frame(u64::from(frame));
    world.templates.insert(template.name.clone(), template);
    let source = world
        .create_object("GLAScudStormClockOwnerProbe", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut target_rule = ThingTemplate::new("RadiusClockOwnerTarget");
    target_rule
        .set_health(100.0)
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable);
    target_rule.set_primary_weapon_none();
    world
        .templates
        .insert(target_rule.name.clone(), target_rule);
    let target = world
        .create_object(
            "RadiusClockOwnerTarget",
            Team::GLA,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .unwrap();
    assert!(
        world
            .host_object(source)
            .unwrap()
            .radius_decal_update
            .is_some()
    );
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .damage,
        1.0
    );
    world.host_object_mut(source).unwrap().attack_target(target);
    assert!(world.host_object(source).unwrap().status.attacking);
    assert!(world.create_delivery_radius_decal(source, Vec3::new(40.0, 0.0, 0.0)));
    let decal = &world
        .host_object(source)
        .unwrap()
        .radius_decal_update
        .as_ref()
        .unwrap()
        .delivery_decal;
    assert_eq!(decal.birth_frame, frame);
    assert_eq!(
        decal.template.as_ref().unwrap().texture,
        SCUD_STORM_DECAL_TEXTURE
    );
    (world, source)
}
fn expected(frame: u32) -> f32 {
    let theta = 2.0 * std::f32::consts::PI * ((frame % DELIVERY_DECAL_THROB_FRAMES) as f32)
        / DELIVERY_DECAL_THROB_FRAMES as f32;
    DELIVERY_DECAL_OPACITY_MIN
        + 0.5 * (theta.sin() + 1.0) * (DELIVERY_DECAL_OPACITY_MAX - DELIVERY_DECAL_OPACITY_MIN)
}
fn opacity(world: &GameLogic, id: ObjectId) -> f32 {
    let state = world
        .host_object(id)
        .unwrap()
        .radius_decal_update
        .as_ref()
        .unwrap();
    assert!(
        state.awake && !state.delivery_decal.is_empty(),
        "actual live attacking decal"
    );
    state.delivery_decal.opacity
}
fn ordinary(world: &mut GameLogic, before: u32) {
    assert_eq!(world.frame, before);
    world.tick_logic_frame(LOGIC_FRAME_TIMESTEP, None, Some(1));
    assert_eq!(
        world.frame,
        before + 1,
        "exact one ordinary simulation frame"
    );
}
#[test]
fn radius_decal_two_owned_worlds_keep_their_phase_after_foreign_native_clock() {
    isolated(
        "radius_decal_two_owned_worlds_keep_their_phase_after_foreign_native_clock",
        || {
            let _restore = RestoreInputs::install(4);
            let (mut first, first_id) = world(2);
            let (mut second, second_id) = world(8);
            assert_eq!(first_id, second_id, "same external ID in two actual owners");
            assert_eq!(gamelogic::helpers::TheGameLogic::get_frame(), 4);
            ordinary(&mut first, 2);
            assert!(
                (opacity(&first, first_id) - expected(2)).abs() < 1e-6,
                "first uses its own frame2"
            );
            let first_opacity = opacity(&first, first_id);
            ordinary(&mut second, 8);
            assert!(
                (opacity(&second, second_id) - expected(8)).abs() < 1e-6,
                "second uses its own frame8"
            );
            assert_eq!(
                opacity(&first, first_id),
                first_opacity,
                "foreign tick does not mutate first decal"
            );
            ordinary(&mut first, 3);
            assert!((opacity(&first, first_id) - expected(3)).abs() < 1e-6);
        },
    );
}
#[test]
fn radius_decal_same_owner_clock_control_and_real_stop_kill() {
    isolated(
        "radius_decal_same_owner_clock_control_and_real_stop_kill",
        || {
            let _restore = RestoreInputs::install(2);
            let (mut world, id) = world(2);
            ordinary(&mut world, 2);
            assert!((opacity(&world, id) - expected(2)).abs() < 1e-6);
            world.host_object_mut(id).unwrap().stop();
            ordinary(&mut world, 3);
            let rd = world
                .host_object(id)
                .unwrap()
                .radius_decal_update
                .as_ref()
                .unwrap();
            assert!(!rd.awake && rd.delivery_decal.is_empty());
            assert!(!rd.kill_when_no_longer_attacking);
            assert_eq!(world.radius_decal_update_reg.attack_kills, 1);
        },
    );
}
#[test]
fn radius_decal_zero_owned_frame_is_not_replaced_by_nonzero_native_clock() {
    isolated(
        "radius_decal_zero_owned_frame_is_not_replaced_by_nonzero_native_clock",
        || {
            let _restore = RestoreInputs::install(4);
            let (mut world, id) = world(0);
            ordinary(&mut world, 0);
            assert!(
                (opacity(&world, id) - expected(0)).abs() < 1e-6,
                "frame0 is legitimate own phase"
            );
        },
    );
}
