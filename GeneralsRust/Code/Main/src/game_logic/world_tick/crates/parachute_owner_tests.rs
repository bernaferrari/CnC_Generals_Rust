use super::*;
struct ActualMissingBones;
impl crate::assets::pristine_bones::PristineBoneAssets for ActualMissingBones {
    fn pristine_bone_transforms(
        &self,
        _: &NativeSlaveSource,
        _: &str,
        _: i32,
        _: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        Ok(Vec::new())
    }
}
fn fixture() -> (GameLogic, ObjectId) {
    let source = r#"
Object ArbitraryAirRider
 KindOf = INFANTRY
 Body = ActiveBody ModuleTag_Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = HijackerUpdate ModuleTag_H
  ParachuteName = AuthoredAirContainer
 End
End
Object AuthoredAirContainer
 KindOf = PARACHUTE
 Body = ActiveBody ModuleTag_Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = ParachuteContain ModuleTag_C
  AllowInsideKindOf = INFANTRY
  ParachuteOpenDist = 100
 End
End
"#;
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "actual-air-contain.ini".into(),
        source.to_owned(),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    world.add_player(Player::new(
        1,
        Team::GLA,
        "Receiving air container owner",
        true,
    ));
    let rider = world
        .create_object_with_script_effects_checked(
            "ArbitraryAirRider",
            Team::GLA,
            Vec3::new(3., 4., 5.),
            None,
            &mut ScriptPresentationEffects {
                pristine_bones: Some(&ActualMissingBones),
                ..Default::default()
            },
        )
        .expect("actual receiving fixture admission")
        .expect("authored rider exists");
    (world, rider)
}

#[test]
fn source_air_admission_uses_exact_saved_height_and_slot_zero_infantry() {
    let (mut world, rider) = fixture();
    assert_eq!(world.objects.get(&rider).unwrap().transport_slot_count(), 0);
    let position = Vec3::new(23., 12., 31.);
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(rider, position, &mut effects)
        .unwrap();
    let passenger = world.objects.get(&rider).unwrap();
    let container = world.objects.get(&passenger.contained_by.unwrap()).unwrap();
    assert_eq!(container.template_name, "AuthoredAirContainer");
    assert_eq!(container.get_position(), position);
    assert_eq!(passenger.get_position(), position - Vec3::Y);
    assert!(!passenger.status.masked);
    assert!(!passenger.drawable_hidden);
    assert!(passenger.is_held_disabled());
    assert!(passenger.is_parachuting());
    assert_eq!(container.occupants, vec![rider]);
    assert_eq!(
        container.max_transport, 0,
        "Parachute does not fabricate generic transport capacity"
    );
    assert!(
        !container.supports_normal_enter()
            || container.normal_enter_admission() == ContainAdmission::InfantryOnly
    );
}
#[test]
fn missing_air_content_is_checked_without_fabricated_template_or_rider_teleport() {
    let (mut world, rider) = fixture();
    world.templates.remove("AuthoredAirContainer");
    let before = world.objects.get(&rider).unwrap().get_position();
    assert!(
        world
            .put_hijacker_in_airborne_parachute_with_effects(
                rider,
                Vec3::new(99., 2., 30.),
                &mut Default::default()
            )
            .is_err()
    );
    assert!(!world.templates.contains_key("AmericaParachute"));
    assert_eq!(world.objects.get(&rider).unwrap().get_position(), before);
    assert!(world.objects.get(&rider).unwrap().contained_by.is_none());
}
#[test]
fn actual_air_admission_is_independent_of_same_id_peer_drop() {
    let (mut world, rider) = fixture();
    let (peer, peer_rider) = fixture();
    assert_eq!(rider, peer_rider);
    drop(peer);
    let position = Vec3::new(14., 7., 16.);
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(rider, position, &mut effects)
        .unwrap();
    assert_eq!(
        world.objects.get(&rider).unwrap().get_position(),
        position - Vec3::Y
    );
    assert_eq!(world.car_bomb.airborne_parachute_puts, 1);
}

#[test]
fn source_parachute_admission_checks_real_roster_and_slot_exception() {
    let (mut world, rider) = fixture();
    let chute = world
        .create_object("AuthoredAirContainer", Team::GLA, Vec3::ZERO)
        .unwrap();
    assert!(world.parachute_can_contain(chute, rider));
    let mut vehicle = ThingTemplate::new("NotTransportable");
    vehicle.add_kind_of(KindOf::Vehicle);
    let id = ObjectId(901);
    world
        .objects
        .insert(id, Object::new(vehicle, id, Team::GLA));
    assert!(!world.parachute_can_contain(chute, id));
    world.objects.get_mut(&chute).unwrap().occupants.push(id);
    assert!(!world.parachute_can_contain(chute, rider));
}

#[test]
fn actual_source_parachute_update_has_no_manual_sink_or_fabricated_landing_override() {
    let (mut world, rider) = fixture();
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(30., 400., 40.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    assert!(!world.objects.get(&chute).unwrap().has_ai_update_interface());
    assert!(
        world
            .tick_parachute_contain_owned(chute, &mut effects)
            .unwrap()
    );
    assert_eq!(world.objects.get(&chute).unwrap().get_position().y, 400.);
    assert!(!world.objects.get(&chute).unwrap().status.parachute_open);
    assert!(world.objects.get(&chute).unwrap().drawable_hidden);
    assert!(world.set_owned_object_position_with_effects(
        chute,
        Vec3::new(30., 100., 40.),
        world.frame,
        &mut effects
    ));
    assert!(
        world
            .tick_parachute_contain_owned(chute, &mut effects)
            .unwrap()
    );
    let object = world.objects.get(&chute).unwrap();
    assert!(object.status.parachute_open);
    assert!(!object.drawable_hidden);
    assert_eq!(
        object.get_position().y,
        100.,
        "module update must not perform its own Euler step"
    );
    assert!(
        object.parachute_landing_override().is_none(),
        "opening without AI does not issue a move or invent an override"
    );
    assert!(!object.status.no_collisions);
    assert!(!world.objects.get(&rider).unwrap().status.no_collisions);
    assert!(
        world
            .tick_parachute_contain_owned(rider, &mut effects)
            .unwrap(),
        "held passenger must not run generic synthetic sink"
    );
}
#[test]
fn actual_parachute_update_peer_drop_does_not_change_start_height_or_open_state() {
    let (mut world, rider) = fixture();
    let (mut peer, peer_rider) = fixture();
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(20., 400., 20.),
            &mut effects,
        )
        .unwrap();
    peer.put_hijacker_in_airborne_parachute_with_effects(
        peer_rider,
        Vec3::new(20., 12., 20.),
        &mut effects,
    )
    .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    let peer_chute = peer.objects.get(&peer_rider).unwrap().contained_by.unwrap();
    assert_eq!(chute, peer_chute);
    peer.tick_parachute_contain_owned(peer_chute, &mut effects)
        .unwrap();
    assert!(peer.objects.get(&peer_chute).unwrap().status.parachute_open);
    drop(peer);
    world
        .tick_parachute_contain_owned(chute, &mut effects)
        .unwrap();
    assert_eq!(
        world
            .objects
            .get(&chute)
            .unwrap()
            .status
            .parachute_start_height,
        400.
    );
    assert!(!world.objects.get(&chute).unwrap().status.parachute_open);
}

#[test]
fn actual_parachute_remove_preserves_cached_offset_and_detaches_after_source_callbacks() {
    let (mut world, rider) = fixture();
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(30., 12., 40.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    world
        .objects
        .get_mut(&chute)
        .unwrap()
        .parachute_attachments
        .as_mut()
        .unwrap()
        .rider_attach_offset = Vec3::new(2., -3., 4.);
    world.objects.get_mut(&chute).unwrap().pathfind_layer = 1;
    world.frame = 77;
    world
        .remove_direct_contained_child_with_effects(chute, rider, &mut effects)
        .unwrap();
    let object = world.objects.get(&rider).unwrap();
    assert_eq!(
        object.get_position(),
        Vec3::new(32., 9., 44.),
        "onRemoving uses saved rider offsets after literal roster erasure"
    );
    assert!(object.contained_by.is_none());
    assert!(!object.is_held_disabled());
    assert!(!object.is_parachuting());
    assert!(!object.status.masked);
    assert!(
        !object.allow_to_fall,
        "physics fields do not imply an authored PhysicsBehavior"
    );
    assert_eq!(object.pathfind_layer, 1);
    assert!(world.objects.get(&chute).unwrap().occupants.is_empty());
    assert!(world.objects.get(&chute).unwrap().status.no_collisions);
    assert!(
        world
            .objects
            .get(&chute)
            .unwrap()
            .parachute_attachments
            .as_ref()
            .unwrap()
            .need_rider_bones
    );
}
#[test]
fn actual_parachute_remove_uses_same_owner_after_same_id_peer_drop() {
    let (mut world, rider) = fixture();
    let (peer, peer_rider) = fixture();
    assert_eq!(rider, peer_rider);
    drop(peer);
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(25., 14., 35.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    world
        .remove_direct_contained_child_with_effects(chute, rider, &mut effects)
        .unwrap();
    assert_eq!(
        world.objects.get(&rider).unwrap().get_position(),
        Vec3::new(25., 13., 35.)
    );
    assert!(world.objects.get(&rider).unwrap().contained_by.is_none());
}

#[test]
fn actual_native_ground_collide_removes_rider_then_kills_real_chute_body() {
    let (mut world, rider) = fixture();
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(30., 12., 40.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    assert!(world.set_owned_object_position_with_effects(
        chute,
        Vec3::new(30., 0., 40.),
        world.frame,
        &mut effects
    ));
    world
        .dispatch_physics_ground_collide_with_effects(chute, &mut effects)
        .unwrap();
    let object = world.objects.get(&rider).unwrap();
    assert_eq!(object.get_position().y, 0.);
    assert!(object.contained_by.is_none());
    assert!(!object.is_parachuting());
    assert!(!world.objects.get(&chute).unwrap().is_alive());
    assert!(world.objects.get(&chute).unwrap().occupants.is_empty());
}
#[test]
fn actual_body_parachute_death_uses_incoming_source_after_removal() {
    let (mut world, rider) = fixture();
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(30., 400., 40.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    let attacker = world
        .create_object("ArbitraryAirRider", Team::USA, Vec3::new(10., 0., 10.))
        .unwrap();
    let stale = world
        .create_object("ArbitraryAirRider", Team::USA, Vec3::new(12., 0., 10.))
        .unwrap();
    world.objects.get_mut(&chute).unwrap().last_damage_source = Some(stale);
    let before = world.objects.get(&rider).unwrap().health.current;
    let expected = world.objects.get(&rider).unwrap().health.maximum * 0.5;
    let context = DamageHitContext::new(
        world.objects.get(&attacker),
        None,
        combat::DamageType::Unresistable,
    );
    world
        .apply_owned_damage_with_effects(
            chute,
            100000.,
            Some(attacker),
            combat::DamageType::Unresistable,
            host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
            &mut effects,
        )
        .unwrap();
    let object = world.objects.get(&rider).unwrap();
    assert!(object.contained_by.is_none());
    assert_eq!(object.health.current, before - expected);
    assert_eq!(object.last_damage_source, Some(attacker));
    assert_ne!(object.last_damage_source, Some(stale));
    assert!(!object.is_held_disabled());
}
#[test]
fn missing_accepted_parachute_death_input_is_checked_not_reconstructed_from_history() {
    let (mut world, rider) = fixture();
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(30., 400., 40.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    assert!(
        world
            .parachute_on_die_owned(chute, None, &mut effects)
            .is_err()
    );
    assert_eq!(world.objects.get(&rider).unwrap().contained_by, Some(chute));
}

#[test]
fn actual_parachute_missing_optional_set_retains_current_locomotor_without_failure() {
    let (mut world, rider) = fixture();
    world
        .templates
        .get_mut("AuthoredAirContainer")
        .unwrap()
        .set_authored_ai_update_interface(Some(true));
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(30., 400., 40.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    let object = world.objects.get(&chute).unwrap();
    assert!(object.has_ai_update_interface());
    assert_eq!(
        object
            .thing()
            .template
            .authored_locomotor_sets
            .as_ref()
            .unwrap()
            .len(),
        0
    );
    let before = (
        object.cur_locomotor_name.clone(),
        object.selected_locomotor_definition,
    );
    world
        .tick_parachute_contain_owned(chute, &mut effects)
        .unwrap();
    let object = world.objects.get(&chute).unwrap();
    assert_eq!(
        (
            object.cur_locomotor_name.clone(),
            object.selected_locomotor_definition
        ),
        before
    );
    assert!(!object.status.parachute_open);
}

#[test]
fn actual_physics_ground_callback_kills_parachute_before_same_actor_fall_damage() {
    let source = r#"
Object ActualFallingRider
 KindOf = INFANTRY
 Body = ActiveBody RiderBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = HijackerUpdate ModuleTag_H
  ParachuteName = ActualFallingChute
 End
End
Object ActualFallingChute
 KindOf = PARACHUTE
 Body = ActiveBody ModuleTag_B
  MaxHealth = 1
  InitialHealth = 1
 End
 Behavior = PhysicsBehavior ModuleTag_P
  Mass = 1
 End
 Behavior = ParachuteContain ModuleTag_C
  AllowInsideKindOf = INFANTRY
 End
End
"#;
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "ground-callback-source-phase.ini".into(),
        source.to_owned(),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    world.add_player(Player::new(1, Team::GLA, "Ground callback owner", true));
    let mut effects = ScriptPresentationEffects {
        pristine_bones: Some(&ActualMissingBones),
        ..Default::default()
    };
    let rider = world
        .create_object_with_script_effects_checked(
            "ActualFallingRider",
            Team::GLA,
            Vec3::new(3., 4., 5.),
            None,
            &mut effects,
        )
        .expect("actual receiving factory admission")
        .expect("authored rider exists");
    world
        .put_hijacker_in_airborne_parachute_with_effects(
            rider,
            Vec3::new(30., 1., 40.),
            &mut effects,
        )
        .unwrap();
    let chute = world.objects.get(&rider).unwrap().contained_by.unwrap();
    {
        let object = world.objects.get_mut(&chute).unwrap();
        object.health.current = 1.;
        object.health.maximum = 1.;
        object.was_airborne_last_frame = true;
        object.immune_to_falling_damage = false;
        object.landing_splat_done = false;
        object.movement.velocity = Vec3::new(0., -100., 0.);
        object.allow_to_fall = true;
        object.min_fall_speed_for_damage = 0.;
        object.fall_height_damage_factor = 100.;
    }
    world
        .drive_owned_contact_with_effects(
            chute,
            crate::game_logic::object::MotionContact::Start(0.),
            Object::step_motion_contact_with_effects,
            &mut effects,
        )
        .unwrap();
    assert_eq!(
        world.objects.get(&chute).unwrap().status.death_type,
        host_usa_pilot::HostDeathType::Normal,
        "onCollide kills the actual chute before falling damage can choose SPLATTED"
    );
    assert!(world.objects.get(&rider).unwrap().contained_by.is_none());
    assert!(world.objects.get(&chute).unwrap().occupants.is_empty());
    assert!(!world.objects.get(&chute).unwrap().pending_ground_collide);
}
