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

const RADIUS_WEAPON_INI: &str = r#"
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
"#;

struct RestoreInputs {
    native_frame: u64,
    draw_icon_ui: bool,
    host_frame: u32,
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
        };

        {
            let mut logic = native.lock().unwrap();
            logic.set_current_frame(native_frame);
            logic.set_draw_icon_ui(true);
        }

        saved
    }
}
impl Drop for RestoreInputs {
    fn drop(&mut self) {
        let native = gamelogic::system::game_logic::get_game_logic();
        let mut logic = native.lock().unwrap();
        logic.set_current_frame(self.native_frame);
        logic.set_draw_icon_ui(self.draw_icon_ui);
        drop(logic);

        crate::game_logic::host_historic_bonus::set_logic_frame(self.host_frame);
    }
}

fn world(frame: u32) -> (GameLogic, ObjectId) {
    let source_text = r#"
Object GLAScudStormClockOwnerProbe
  KindOf = STRUCTURE SELECTABLE CAN_ATTACK
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = RadiusDecalUpdate ModuleTag_DeliveryDecal
  End
  WeaponSet
    Conditions = None
    Weapon = PRIMARY RadiusClockOwnerGun
  End
End
Object RadiusClockOwnerTarget
  KindOf = STRUCTURE
  Body = ActiveBody TargetBody
    MaxHealth = 100
    InitialHealth = 100
  End
End
"#;
    let mut world = GameLogic::new();
    world.set_current_frame(u64::from(frame));
    // One actual parser admission owns both Object/Client module projections.
    // The target also belongs to this roster; no unpaired authored actor enters.
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "radius-clock-owner.ini".to_owned(),
                source_text.to_owned(),
            )])
            .expect("actual paired Object INI"),
        )
        .expect("receiving Object admission");
    let definition = world
        .receiving_object_definitions
        .as_ref()
        .unwrap()
        .get("GLAScudStormClockOwnerProbe")
        .unwrap();
    assert!(
        definition
            .behavior_modules
            .iter()
            .any(|module| module.class_name == "RadiusDecalUpdate")
    );
    assert_eq!(
        world.templates["GLAScudStormClockOwnerProbe"]
            .primary_weapon_name
            .as_deref(),
        Some("RadiusClockOwnerGun")
    );
    assert!(
        world.templates["RadiusClockOwnerTarget"]
            .primary_weapon
            .is_none()
    );
    // This world owns the real weapon rows used by can_attack and firing;
    // no process-global weapon catalog is installed by the fixture.
    let mut weapons = std::collections::HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut weapons,
            RADIUS_WEAPON_INI,
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        1,
    );
    world
        .admit_weapon_content_snapshot(weapons.into_iter().collect(), &[])
        .expect("actual receiving radius weapon");
    let source = world
        .create_object("GLAScudStormClockOwnerProbe", Team::USA, Vec3::ZERO)
        .unwrap();
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
    // ActiveBody's C++ default InitialHealth is zero; this scenario explicitly
    // authors a live, completed attacker rather than relying on a fixture fallback.
    assert_eq!(world.host_object(source).unwrap().health.current, 100.0);
    assert!(world.host_object(source).unwrap().can_attack());
    {
        let (object, attack_events) = world.host_object_and_attack_events_mut(source).unwrap();
        object.attack_target(
            target,
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    };
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
            world
                .host_object_mut(id)
                .unwrap()
                .stop(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
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

#[test]
fn actual_decal_and_horde_updates_read_only_the_driving_icon_policy() {
    isolated(
        "actual_decal_and_horde_updates_read_only_the_driving_icon_policy",
        || {
            use crate::game_logic::host_radius_decal_update::{
                HostRadiusDecalTemplate, HostRadiusDecalUpdateData,
            };
            use crate::game_logic::{HordeGrantCounter, HostResidualMutationOp};
            fn owner() -> (GameLogic, ObjectId) {
                let mut world = GameLogic::new();
                let mut template = ThingTemplate::new("OwnedIconPolicyProbe");
                template.set_health(100.0).add_kind_of(KindOf::Vehicle);
                world.templates.insert(template.name.clone(), template);
                let id = world
                    .create_object("OwnedIconPolicyProbe", Team::China, Vec3::ZERO)
                    .unwrap();
                let mut data = HostRadiusDecalUpdateData::default();
                data.create_radius_decal(
                    HostRadiusDecalTemplate::scud_storm(),
                    200.0,
                    Vec3::ZERO,
                    0,
                );
                world.host_object_mut(id).unwrap().radius_decal_update = Some(data);
                (world, id)
            }
            let (mut a, aid) = owner();
            let (mut b, bid) = owner();
            assert_eq!(aid, bid);
            let foreign_policy = gamelogic::helpers::TheGameLogic::get_draw_icon_ui();
            a.set_draw_icon_ui(!foreign_policy);
            b.set_draw_icon_ui(foreign_policy);
            for (world, id, show) in [
                (&mut a, aid, !foreign_policy),
                (&mut b, bid, foreign_policy),
            ] {
                world.frame = 3;
                world.update_radius_decal_update();
                let opacity = world
                    .host_object(id)
                    .unwrap()
                    .radius_decal_update
                    .as_ref()
                    .unwrap()
                    .delivery_decal
                    .opacity;
                assert_eq!(
                    opacity > 0.0,
                    show,
                    "RadiusDecal reads its driving flag, not the standalone Core flag"
                );
                world.apply_host_residual_mutation_op(
                    HostResidualMutationOp::SetWeaponBonusHorde {
                        id,
                        now_horde: true,
                        was_horde: false,
                        grant: HordeGrantCounter::None,
                    },
                );
                let decal = world.host_object(id).unwrap().get_terrain_decal_type();
                assert_eq!(
                    decal != crate::game_logic::host_battlemaster::TERRAIN_DECAL_NONE,
                    show,
                    "Horde decal mutation uses the receiving owner"
                );
            }
            let retained = b
                .host_object(bid)
                .unwrap()
                .radius_decal_update
                .as_ref()
                .unwrap()
                .delivery_decal
                .opacity;
            a.set_draw_icon_ui(foreign_policy);
            a.update_radius_decal_update();
            assert_eq!(
                b.host_object(bid)
                    .unwrap()
                    .radius_decal_update
                    .as_ref()
                    .unwrap()
                    .delivery_decal
                    .opacity,
                retained
            );
            assert_eq!(
                gamelogic::helpers::TheGameLogic::get_draw_icon_ui(),
                foreign_policy
            );
        },
    );
}
