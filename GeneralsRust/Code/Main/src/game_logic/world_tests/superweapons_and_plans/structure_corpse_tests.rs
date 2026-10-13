//! Dead Body admission is independent of its retained structure Update module.
//! C++ ActiveBody.cpp:641-649,1190-1226; structure updates finish without deletion.
use super::*;

fn lethal_owner(collapse: bool) -> (GameLogic, ObjectId, Option<ObjectId>) {
    let mut world = GameLogic::new();
    if !collapse {
        assert_eq!(
            crate::assets::ini_template_loader::register_weapons_from_ini_text(
                r#"
Weapon ToppledStructureWeapon
  PrimaryDamage = 99999
  PrimaryDamageRadius = 20
  DamageType = EXPLOSION
  DeathType = EXPLODED
End
"#
            ),
            1
        );
    }
    ensure_test_player_for_team(&mut world, Team::USA);
    let mut template = ThingTemplate::new("AuthoredLethalStructure");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Immobile)
        .set_health(100.0);
    template.authored_structure_topple = !collapse;
    if !collapse {
        let mut definition =
            crate::game_logic::host_structure_topple::definition::StructureToppleDefinition::default();
        definition.fx.weapon = "ToppledStructureWeapon".into();
        definition.fx.fx = "FX_DefaultStructureCrushing".into();
        template.structure_topple_definition = Some(Ok(std::sync::Arc::new(definition)));
    }
    template.authored_structure_collapse = collapse;
    template.energy_production = Some(10);
    template.geometry_info.authored = true;
    template.geometry_info.height = 50.0;
    template.geometry_info.major_radius = 20.0;
    template.geometry_info.minor_radius = 20.0;
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("AuthoredLethalStructure", Team::USA, Vec3::ZERO)
        .unwrap();
    // This control does not infer death from a template name or set HP directly.
    world.update_player_resources(0.0);
    assert_eq!(world.get_player(0).unwrap().power_produced, 10);
    let victim = if collapse {
        None
    } else {
        let mut infantry = ThingTemplate::new("StructureSweepInfantry");
        infantry
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0);
        world.templates.insert(infantry.name.clone(), infantry);
        Some(
            world
                .create_object(
                    "StructureSweepInfantry",
                    Team::GLA,
                    Vec3::new(12.0, 0.0, 0.0),
                )
                .unwrap(),
        )
    };
    // apply_host_damage executes the canonical body damage path and returns its
    // lethal transition; the normal caller then dispatches the one onDie boundary.
    assert_eq!(world.apply_host_damage(id, 200.0), (true, false));
    assert_eq!(world.host_object(id).unwrap().health.current, 0.0);
    assert!(world.host_object(id).unwrap().status.destroyed);
    world.mark_object_for_destruction(id, None);
    assert_corpse(&world, id);
    world.update_player_resources(0.0);
    assert_eq!(world.get_player(0).unwrap().power_produced, 0);
    (world, id, victim)
}

fn assert_corpse(world: &GameLogic, id: ObjectId) {
    let object = world
        .host_object(id)
        .expect("dead body remains in its owner store");
    assert_eq!(
        object.health.current, 0.0,
        "structure admission must not heal the body"
    );
    assert!(
        object.status.destroyed,
        "body death marker is independent of physical removal"
    );
    assert!(object.status.on_die_started);
    assert!(!object.is_alive());
    assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
}

fn finished(world: &GameLogic, id: ObjectId, collapse: bool) -> bool {
    let object = world.host_object(id).unwrap();
    if collapse {
        object.structure_collapse_data.as_ref().unwrap().state
            == crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
    } else {
        object.structure_topple_data.as_ref().unwrap().state
            == crate::game_logic::host_structure_topple::HostStructureToppleState::Done
    }
}

fn exercise_retained_body(coupled: bool) {
    for collapse in [false, true] {
        let (mut world, id, victim) = lethal_owner(collapse);
        let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
        let mut completed = false;
        for _ in 0..800 {
            if coupled {
                world.update();
            } else {
                world.update();
            }
            if coupled {
                shadow.sync_from_host(&world);
                let _ =
                    crate::gameworld_shadow::run_owned_host_boundary(Some(&mut shadow), &mut world);
            }
            assert_corpse(&world, id);
            assert!(world.get_next_mood_target(id, true, true, false).is_none());
            assert_eq!(world.get_player(0).unwrap().power_produced, 0);
            completed = finished(&world, id, collapse);
            if completed {
                break;
            }
        }
        assert!(
            completed,
            "dead body must not suppress the independent Update module"
        );
        let object = world.host_object(id).unwrap();
        if collapse {
            let data = object.structure_collapse_data.as_ref().unwrap();
            assert!(data.initial_played && data.start_burst_played && data.final_played);
            assert!(
                data.pending_phase_fx.is_empty(),
                "phase output is consumed by the live driver"
            );
            assert!(object.presentation_collapse_height_offset() < -1.0);
        } else {
            let data = object.structure_topple_data.as_ref().unwrap();
            assert!(data.start_fx_played && data.done_fx_played);
            assert!(
                data.pending_fx.is_empty(),
                "topple output is consumed by the live driver"
            );
            assert!(
                data.last_crushed_location > 0.0,
                "dead structure still advances the crush sweep"
            );
            let victim = victim.unwrap();
            assert!(
                world
                    .host_object(victim)
                    .is_none_or(|body| !body.is_alive()),
                "real infantry must receive the retained corpse's crush sweep"
            );
        }
        world.mark_object_for_destruction(id, Some(Team::GLA));
        assert_corpse(&world, id);
        world.destroy_object(id);
        world.destroy_object(id);
        assert_eq!(
            world
                .objects_to_destroy
                .iter()
                .filter(|event| event.id == id)
                .count(),
            1
        );
        world.process_destroy_list();
        assert!(world.host_object(id).is_none());
    }
}

#[test]
fn lethal_structure_body_stays_dead_while_host_update_completes() {
    crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at(
        module_path!(),
        "lethal_structure_body_stays_dead_while_host_update_completes",
        || {
            exercise_retained_body(false);
        },
    );
}

#[test]
fn lethal_structure_body_stays_dead_through_coupled_update_and_explicit_delete() {
    crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at(
        module_path!(),
        "lethal_structure_body_stays_dead_through_coupled_update_and_explicit_delete",
        || {
            exercise_retained_body(true);
        },
    );
}
