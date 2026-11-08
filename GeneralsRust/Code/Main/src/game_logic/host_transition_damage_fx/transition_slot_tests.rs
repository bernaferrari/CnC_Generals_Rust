use super::*;

#[test]
fn actual_definition_parser_retains_sparse_slots_without_asset_registry() {
    let module = crate::assets::ini_parser::BehaviorModuleDefinition {
        class_name: "TransitionDamageFX".into(),
        module_tag: None,
        attributes: [
            (
                "DamagedFXList3".into(),
                "Loc: X:3 Y:4 Z:5 FXList:FX_Third".into(),
            ),
            ("DamagedOCL1".into(), "OCL:OCL_First".into()),
            ("DamagedParticleSystem12".into(), "PSys:Smoke_Last".into()),
        ]
        .into_iter()
        .collect(),
    };
    let mut data = HostTransitionDamageFxData::vehicle_residual();
    overlay_authored_transition_module(&mut data, &module);
    assert_eq!(data.fx_lists_for_state[1], vec!["", "", "FX_Third"]);
    assert_eq!(data.fx_locs_for_state[1][2].loc, [3., 4., 5.]);
    assert_eq!(data.ocl_for_state[1], vec!["OCL_First"]);
    assert_eq!(data.particles_for_state[1].len(), 12);
    assert!(data.particles_for_state[1][0].name.is_empty());
    assert_eq!(data.particles_for_state[1][11].name, "Smoke_Last");
    let event = on_body_damage_state_change(
        &data,
        HostBodyDamageType::Pristine,
        HostBodyDamageType::Damaged,
    )
    .unwrap();
    assert!(event.fx_name.is_none());
    assert_eq!(event.extra_fx_names, vec!["", "FX_Third"]);
}
#[test]
fn actual_damage_transition_fires_owned_ocl_at_the_attacker_in_nugget_order() {
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::object::DamageHitContext;
    use glam::Vec3;

    fn world(selected: &str) -> (GameLogic, ObjectId, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
        world.add_player(Player::new(1, Team::USA, "Victim", true));
        world.add_player(Player::new(2, Team::China, "Attacker", false));
        world.admit_object_creation_list_sources([format!(r#"
ObjectCreationList TransitionRetaliation
  CreateObject
    ObjectNames = BeforeRetaliation
    Count = 1
    Offset = X:30 Y:0 Z:0
  End
  FireWeapon
    Weapon = {selected}
  End
  CreateObject
    ObjectNames = AfterRetaliation
    Count = 1
    Offset = X:30 Y:0 Z:0
  End
End
"#)]).unwrap();
        let weapons = [("TransitionRetaliationWeapon", 10.0), ("OtherRetaliationWeapon", 20.0)]
            .into_iter().map(|(name, damage)| {
                let mut weapon = gamelogic::weapon::WeaponTemplate::new(name.into());
                weapon.primary_damage = damage;
                weapon.primary_damage_radius = 5.0;
                weapon.damage_type = gamelogic::damage::DamageType::Unresistable;
                (name.to_ascii_lowercase(), std::sync::Arc::new(weapon))
            }).collect();
        world.weapon_definitions = Some(weapons);
        for name in ["TransitionVictim", "TransitionAttacker", "BeforeRetaliation", "AfterRetaliation"] {
            let mut template = ThingTemplate::new(name);
            template.set_health(100.0);
            template.add_kind_of(KindOf::Attackable).add_kind_of(KindOf::Vehicle);
            world.templates.insert(name.into(), template);
        }
        let victim = world.create_object("TransitionVictim", Team::USA, Vec3::ZERO).unwrap();
        let attacker = world.create_object("TransitionAttacker", Team::China, Vec3::new(30.0, 0.0, 0.0)).unwrap();
        let mut transition = HostTransitionDamageFxData::default();
        transition.enabled = true;
        transition.ocl_for_state[1].push("TransitionRetaliation".into());
        world.objects.get_mut(&victim).unwrap().transition_damage_fx = Some(transition);
        (world, victim, attacker)
    }
    fn hit(world: &mut GameLogic, victim: ObjectId, attacker: ObjectId, amount: f32) {
        world.apply_owned_damage(
            victim, amount, Some(attacker), DamageType::Unresistable,
            HostDeathType::Normal, None, &DamageHitContext::default(),
        ).unwrap();
    }
    fn health(world: &GameLogic, name: &str) -> f32 {
        world.objects.registered_values().find(|object| object.template_name == name).unwrap().health.current
    }

    let (mut first, victim, attacker) = world("TransitionRetaliationWeapon");
    let (mut second, peer_victim, peer_attacker) = world("OtherRetaliationWeapon");
    assert_eq!((victim, attacker), (peer_victim, peer_attacker));
    hit(&mut first, victim, attacker, 1.0);
    hit(&mut second, victim, attacker, 1.0);
    assert_eq!(first.objects[&attacker].health.current, 100.0);
    assert_eq!(second.objects[&attacker].health.current, 100.0);
    hit(&mut first, victim, attacker, 39.0);
    hit(&mut second, victim, attacker, 39.0);
    assert_eq!(first.objects[&attacker].health.current, 90.0);
    assert_eq!(second.objects[&attacker].health.current, 80.0);
    assert_eq!(first.objects[&attacker].last_damage_source, Some(victim));
    assert_eq!(second.objects[&attacker].last_damage_source, Some(victim));
    assert_eq!(health(&first, "BeforeRetaliation"), 90.0);
    assert_eq!(health(&second, "BeforeRetaliation"), 80.0);
    assert_eq!(health(&first, "AfterRetaliation"), 100.0);
    assert_eq!(health(&second, "AfterRetaliation"), 100.0);
    hit(&mut first, victim, attacker, 1.0);
    hit(&mut second, victim, attacker, 1.0);
    assert_eq!(first.objects[&attacker].health.current, 90.0);
    assert_eq!(second.objects[&attacker].health.current, 80.0);
}

#[test]
fn failed_retirement_retains_receipt_in_its_authored_slot() {
    let mut ids = [10, 0, 20];
    let mut attempts = Vec::new();
    retire_old_transition_ids(&mut ids, |id| {
        attempts.push(id);
        id != 20
    });
    assert_eq!(attempts, vec![10, 20]);
    assert_eq!(ids, [0, 0, 20]);
    retire_old_transition_ids(&mut ids, |_| true);
    assert_eq!(ids, [0, 0, 0]);
}

#[derive(serde::Deserialize)]
struct CppCase {
    name: String,
    arguments: Vec<u32>,
    trace: Vec<String>,
}

#[test]
fn original_cpp_callback_matches_slot_selection_cleanup_and_effect_order() {
    // Expected data is produced only by cpp_transition_slots_oracle.py's verbatim
    // original callback. This checks selection/order and successful cleanup, not
    // native allocation, bone sampling or full-game equivalence.
    let cases: Vec<CppCase> =
        serde_json::from_str(include_str!("cpp_transition_slots.json")).unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let a = &case.arguments;
        let mut data = HostTransitionDamageFxData::vehicle_residual();
        let state = a[1] as usize;
        for (bits, names, prefix) in [
            (a[2], &mut data.fx_lists_for_state[state], "FX"),
            (a[3], &mut data.ocl_for_state[state], "OCL"),
        ] {
            names.resize(12, String::new());
            for slot in 0..12 {
                if bits & (1 << slot) != 0 {
                    names[slot] = format!("{prefix}_{slot}");
                }
            }
        }
        data.particles_for_state[state].resize(12, HostTransitionParticle::default());
        for slot in 0..12 {
            if a[4] & (1 << slot) != 0 {
                data.particles_for_state[state][slot].name = format!("PS_{slot}");
            }
        }
        data.damage_fx_types = if a[6] != 0 { u64::MAX } else { 0 };
        data.damage_ocl_types = if a[7] != 0 { u64::MAX } else { 0 };
        data.damage_particle_types = if a[8] != 0 { u64::MAX } else { 0 };
        let old = HostBodyDamageType::from_ordinal(a[0] as u8);
        let new = HostBodyDamageType::from_ordinal(a[1] as u8);
        let event = on_body_damage_state_change_for_damage(
            &data,
            old,
            new,
            (a[9] != 0).then_some(gamelogic::damage::DamageType::Flame),
        )
        .unwrap();
        let mut ids = [0; 12];
        for slot in 0..12 {
            if a[5] & (1 << slot) != 0 {
                ids[slot] = 100 + slot as u32;
            }
        }
        let mut actual = Vec::new();
        assert_eq!(event.clear_old_state, Some(old.ordinal()));
        retire_old_transition_ids(&mut ids, |id| {
            actual.push(format!("destroy:{id}"));
            true
        });
        for_each_transition_slot(&event, |effect| {
            actual.push(match effect {
                TransitionSlotEffect::Fx { slot, .. } => format!("fx:{slot}"),
                TransitionSlotEffect::Ocl { slot, .. } => format!("ocl:{slot}"),
                TransitionSlotEffect::Particle { slot, .. } => format!("create:{slot}"),
            })
        });
        actual.push(format!(
            "old_ids:{}",
            ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        ));
        let expected: Vec<_> = case
            .trace
            .into_iter()
            .filter(|line| {
                ["destroy:", "fx:", "ocl:", "create:", "old_ids:"]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
            })
            .collect();
        assert_eq!(actual, expected, "original C++ case {}", case.name);
    }
}

#[cfg(feature = "game_client")]
#[test]
fn native_transition_world_location_uses_canonical_yaw_and_full_affine_transform() {
    use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
    let mut object = Object::new(
        ThingTemplate::new("AffineTransitionOwner"),
        ObjectId(9939),
        Team::USA,
    );
    let mut location = HostTransitionLoc {
        loc: [1., 0., 0.],
        ..Default::default()
    };
    object.set_transform_matrix(glam::Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2));
    let yaw_world = host_world(&object, Some(&location));
    assert!(
        (yaw_world - glam::Vec3::NEG_Z).length() < 1.0e-6,
        "positive Main Y rotation sends local X toward negative world Z"
    );

    let transform = glam::Mat4::from_scale_rotation_translation(
        glam::Vec3::new(2., 3., 4.),
        glam::Quat::from_euler(glam::EulerRot::YXZ, 0.7, 0.4, -0.3),
        glam::Vec3::new(10., 20., 30.),
    );
    object.set_transform_matrix(transform);
    // INI/source XYZ is Z-up: local [2,3,4] becomes Main [2,4,3].
    location.loc = [2., 3., 4.];
    let expected = transform.transform_point3(glam::Vec3::new(2., 4., 3.));
    assert!(
        (host_world(&object, Some(&location)) - expected).length() < 1.0e-5,
        "FX and OCL share the actual owner's full affine conversion"
    );
    assert_eq!(host_world(&object, None), transform.w_axis.truncate());
}

#[test]
fn state_reentry_cannot_overwrite_a_failed_retirement_receipt() {
    let mut ids = vec![0, 77];
    let mut attempts = Vec::new();
    assert!(!prepare_transition_slot(&mut ids, 1, |id| {
        attempts.push(id);
        false
    }));
    assert_eq!(ids, vec![0, 77]);
    assert_eq!(attempts, vec![77]);
    assert!(prepare_transition_slot(&mut ids, 1, |id| id == 77));
    assert_eq!(ids, vec![0, 0]);
    assert!(prepare_transition_slot(&mut ids, 3, |_| panic!(
        "empty slot"
    )));
    assert_eq!(ids, vec![0, 0, 0, 0]);
}
