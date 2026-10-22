//! Ordinary headless execution of the same physical callback boundary. Real
//! native resource disposal is covered separately by the WGPU receiver tests.
use super::*;
use crate::game_logic::{NativeObjectRetirement, NativeViewEffects, ScriptPresentationEffects};

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "physical-retirement-owner.ini".into(),
            "Object PhysicalOwner\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = DestroyDie DeleteTag\nEnd\nEnd\n".into(),
        )]).unwrap(),
    ).unwrap();
    world.add_player(Player::new(1, Team::USA, "PhysicalOwner", true));
    let id = world
        .create_object("PhysicalOwner", Team::USA, Vec3::new(3., 4., 5.))
        .unwrap();
    (world, id)
}

struct Observe {
    id: ObjectId,
    team_name: String,
    team_id: Option<u32>,
    calls: usize,
    reject: bool,
}
impl Observe {
    fn new(world: &GameLogic, id: ObjectId, reject: bool) -> Self {
        let object = &world.objects[&id];
        Self {
            id,
            team_name: object.team_instance_name.clone(),
            team_id: object.team_instance_id,
            calls: 0,
            reject,
        }
    }
}
impl NativeViewEffects for Observe {
    fn local_player_id(&self) -> u32 {
        1
    }
    fn host_epoch(&self) -> u64 {
        1
    }
    fn sample_slave(
        &mut self,
        _: &crate::game_logic::NativeSlaveSource,
        _: &str,
    ) -> crate::presentation_frame::camera::build::SlaveCameraSample {
        panic!("physical removal does not sample a camera")
    }
    fn admit_factory_drawable(
        &mut self,
        _: crate::game_logic::NativeFactoryDrawableSource,
        _: game_client::core::game_client::NativeDrawableBirthContext,
        _: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        panic!("this headless probe is attached only after creation")
    }
    fn retire_physical_object_drawable(
        &mut self,
        source: &NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        self.calls += 1;
        assert_eq!(source.identity.object_id(), self.id.0);
        assert_eq!(source.object.id, self.id);
        assert!(
            !source.live_objects.contains_key(&self.id),
            "destructor lookup must be absent"
        );
        assert!(
            !source.live_objects.ordered_ids().contains(&self.id),
            "destructor visitation must be absent"
        );
        assert_eq!(source.object.team, Team::USA);
        assert_eq!(source.object.team_instance_id, self.team_id);
        assert_eq!(source.object.team_instance_name, self.team_name);
        if self.reject {
            Err("receiving retirement rejected".into())
        } else {
            Ok(())
        }
    }
}

fn queue(world: &mut GameLogic, id: ObjectId, death: bool) {
    if death {
        world
            .apply_owned_damage(
                id,
                1000.,
                None,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                None,
                &crate::game_logic::object::DamageHitContext::default(),
            )
            .unwrap();
    } else {
        world.destroy_object(id);
    }
    assert!(
        world.objects.contains_key(&id),
        "onDelete remains an installed phase"
    );
    assert!(world.objects.ordered_ids().contains(&id));
    assert!(world.has_pending_destroy_work());
}

#[test]
fn physical_callback_loses_lookup_but_keeps_owned_identity_for_both_destroy_routes() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "physical_callback_loses_lookup_but_keeps_owned_identity_for_both_destroy_routes",
        || {
            for death in [false, true] {
                let (mut owner, id) = world();
                let (peer, peer_id) = world();
                assert_eq!(id, peer_id);
                let mut probe = Observe::new(&owner, id, false);
                queue(&mut owner, id, death);
                owner
                    .process_destroy_list_with_effects_checked(&mut ScriptPresentationEffects {
                        view: Some(&mut probe),
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!(probe.calls, 1);
                assert!(owner.objects.get(&id).is_none());
                assert!(!owner.objects.ordered_ids().contains(&id));
                assert!(owner.objects.failed_retirement_object().is_none());
                assert!(!owner.has_pending_object_removals());
                assert_eq!(peer.objects[&peer_id].health.current, 100.);
                assert!(!peer.objects[&peer_id].status.destroyed);
            }
        },
    );
}

#[test]
fn failed_owned_retirement_is_not_live_or_retried_and_cannot_advance_save_create_or_reset() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "failed_owned_retirement_is_not_live_or_retried_and_cannot_advance_save_create_or_reset",
        || {
            for death in [false, true] {
                let (mut owner, id) = world();
                let mut probe = Observe::new(&owner, id, true);
                queue(&mut owner, id, death);
                let mut effects = ScriptPresentationEffects {
                    view: Some(&mut probe),
                    ..Default::default()
                };
                let first = owner
                    .process_destroy_list_with_effects_checked(&mut effects)
                    .unwrap_err()
                    .to_string();
                assert!(first.contains("receiving retirement rejected"));
                assert!(owner.objects.get(&id).is_none());
                assert!(!owner.objects.ordered_ids().contains(&id));
                let object = owner.objects.failed_retirement_object().unwrap();
                assert_eq!(object.id, id);
                assert_eq!(object.team, Team::USA);
                let retained = std::ptr::from_ref(object);
                assert_eq!(
                    owner
                        .process_destroy_list_with_effects_checked(&mut effects)
                        .unwrap_err()
                        .to_string(),
                    first
                );
                drop(effects);
                assert_eq!(probe.calls, 1, "failed callbacks never replay");
                assert_eq!(
                    std::ptr::from_ref(owner.objects.failed_retirement_object().unwrap()),
                    retained
                );
                let before_generation = owner.next_visual_object_generation;
                let before_pending =
                    bincode_legacy::serialize(&owner.pending_object_removal_snapshot()).unwrap();
                let template = owner
                    .objects
                    .failed_retirement_object()
                    .unwrap()
                    .thing()
                    .template
                    .clone();
                let admission = owner
                    .try_add_object(Object::new(template, id, Team::USA))
                    .unwrap_err()
                    .to_string();
                assert_eq!(
                    admission, first,
                    "admission must retain the terminal native error"
                );
                assert!(owner.objects.get(&id).is_none());
                assert_eq!(owner.next_visual_object_generation, before_generation);
                assert_eq!(
                    bincode_legacy::serialize(&owner.pending_object_removal_snapshot()).unwrap(),
                    before_pending
                );
                assert_eq!(
                    probe.calls, 1,
                    "rejected new lifetime cannot retry native destruction"
                );
                assert_eq!(
                    std::ptr::from_ref(owner.objects.failed_retirement_object().unwrap()),
                    retained
                );
                let old_frame = owner.frame;
                let old_time = owner.sim_time_seconds;
                let old_carry = owner.accumulated_time;
                let mut before = 0;
                let mut after = 0;
                assert!(matches!(
                    owner.begin_native_logic_tick(
                        10.,
                        None,
                        Some(4),
                        Default::default(),
                        |_| before += 1,
                        |_| after += 1,
                    ),
                    crate::game_logic::game_logic::NativeTickProgress::Failed(_)
                ));
                assert_eq!((before, after), (0, 0));
                assert_eq!(
                    (owner.frame, owner.sim_time_seconds, owner.accumulated_time),
                    (old_frame, old_time, old_carry)
                );
                assert!(
                    owner
                        .create_object_with_script_effects_checked(
                            "PhysicalOwner",
                            Team::USA,
                            Vec3::ZERO,
                            None,
                            &mut Default::default(),
                        )
                        .is_err()
                );
                assert!(
                    crate::save_load::snapshot::SnapshotBuilder::new()
                        .create_world_snapshot(&owner)
                        .is_err()
                );
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner.reset()))
                        .is_err()
                );
                assert_eq!(
                    (owner.frame, owner.sim_time_seconds, owner.accumulated_time),
                    (old_frame, old_time, old_carry)
                );
                assert_eq!(
                    std::ptr::from_ref(owner.objects.failed_retirement_object().unwrap()),
                    retained
                );
                owner.objects.clear();
                assert!(
                    owner.object_retirement_error().is_some(),
                    "roster clear cannot acknowledge failed native cleanup"
                );
                assert_eq!(
                    std::ptr::from_ref(owner.objects.failed_retirement_object().unwrap()),
                    retained
                );
            }
        },
    );
}

#[test]
fn colliding_admission_preserves_pending_old_lifetime_until_one_native_retirement() {
    for death in [false, true] {
        let (mut owner, id) = world();
        let (peer, peer_id) = world();
        assert_eq!(id, peer_id);
        let mut probe = Observe::new(&owner, id, false);
        queue(&mut owner, id, death);
        let old = owner.objects[&id].clone();
        let old_generation = old.visual_object_generation;
        let old_order = owner.objects.ordered_ids().to_vec();
        let pending = owner.pending_object_removal_snapshot();
        let error = owner
            .try_admit_host_object(Object::new(old.thing().template.clone(), id, Team::China))
            .unwrap_err();
        assert!(error.to_string().contains("awaiting physical retirement"));
        assert_eq!(owner.objects[&id].visual_object_generation, old_generation);
        assert_eq!(owner.objects[&id].kill_score_phase, old.kill_score_phase);
        assert_eq!(
            owner.objects[&id].kill_experience_awarded,
            old.kill_experience_awarded
        );
        assert_eq!(
            bincode_legacy::serialize(&owner.objects[&id].native_die_runtime).unwrap(),
            bincode_legacy::serialize(&old.native_die_runtime).unwrap()
        );
        assert_eq!(owner.objects.ordered_ids(), old_order);
        assert_eq!(owner.pending_object_removal_snapshot().len(), pending.len());
        assert_eq!(probe.calls, 0);
        owner
            .process_destroy_list_with_effects_checked(&mut ScriptPresentationEffects {
                view: Some(&mut probe),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(probe.calls, 1);
        assert!(owner.objects.get(&id).is_none());
        owner
            .try_admit_host_object(Object::new(old.thing().template.clone(), id, Team::China))
            .unwrap();
        assert_ne!(owner.objects[&id].visual_object_generation, old_generation);
        assert_eq!(
            owner.objects[&id].kill_score_phase,
            crate::game_logic::kill_credit::KillScorePhase::Ready
        );
        assert!(!owner.objects[&id].kill_experience_awarded);
        owner.process_destroy_list();
        assert!(owner.objects.get(&id).is_some());
        assert!(!peer.objects[&peer_id].status.destroyed);
    }
}

#[test]
fn bulk_clear_requires_receiving_owner_before_mutation_and_retires_once() {
    let (mut owner, id) = world();
    let (peer, peer_id) = world();
    owner.require_native_drawable_owner();
    let generation = owner.objects[&id].visual_object_generation;
    let next_id = owner.next_object_id_for_snapshot();
    let epoch = owner.visual_world_epoch;
    assert!(
        owner
            .clear_all_objects_with_effects_checked(&mut Default::default())
            .is_err()
    );
    assert!(!owner.objects[&id].status.destroyed);
    assert_eq!(owner.objects[&id].visual_object_generation, generation);
    assert_eq!(owner.next_object_id_for_snapshot(), next_id);
    assert_eq!(owner.visual_world_epoch, epoch);
    assert!(!owner.has_pending_destroy_work());
    let mut probe = Observe::new(&owner, id, false);
    owner
        .clear_all_objects_with_effects_checked(&mut ScriptPresentationEffects {
            view: Some(&mut probe),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(probe.calls, 1);
    assert!(owner.objects.is_empty());
    assert!(!owner.has_pending_destroy_work());
    assert_eq!(owner.next_object_id_for_snapshot(), ObjectId(1));
    assert_ne!(owner.visual_world_epoch, epoch);
    assert!(!peer.objects[&peer_id].status.destroyed);
    owner
        .clear_all_objects_with_effects_checked(&mut ScriptPresentationEffects {
            view: Some(&mut probe),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(probe.calls, 1);
}

#[test]
fn bulk_clear_failed_native_destructor_never_resets_or_retries_old_identity() {
    let (mut owner, id) = world();
    owner.require_native_drawable_owner();
    let next_id = owner.next_object_id_for_snapshot();
    let epoch = owner.visual_world_epoch;
    let mut probe = Observe::new(&owner, id, true);
    assert!(
        owner
            .clear_all_objects_with_effects_checked(&mut ScriptPresentationEffects {
                view: Some(&mut probe),
                ..Default::default()
            })
            .is_err()
    );
    assert_eq!(probe.calls, 1);
    assert_eq!(owner.objects.failed_retirement_object().unwrap().id, id);
    assert_eq!(owner.next_object_id_for_snapshot(), next_id);
    assert_eq!(owner.visual_world_epoch, epoch);
    assert!(
        owner
            .clear_all_objects_with_effects_checked(&mut ScriptPresentationEffects {
                view: Some(&mut probe),
                ..Default::default()
            })
            .is_err()
    );
    assert_eq!(probe.calls, 1);
}

#[test]
fn bulk_retirement_preserves_prior_queue_then_registration_and_containment_order() {
    struct OrderedProbe {
        calls: Vec<(ObjectId, Vec<ObjectId>)>,
        parent: ObjectId,
        child: ObjectId,
    }
    impl NativeViewEffects for OrderedProbe {
        fn local_player_id(&self) -> u32 {
            1
        }
        fn host_epoch(&self) -> u64 {
            1
        }
        fn sample_slave(
            &mut self,
            _: &crate::game_logic::NativeSlaveSource,
            _: &str,
        ) -> crate::presentation_frame::camera::build::SlaveCameraSample {
            panic!("no camera sample during deletion")
        }
        fn admit_factory_drawable(
            &mut self,
            _: crate::game_logic::NativeFactoryDrawableSource,
            _: game_client::core::game_client::NativeDrawableBirthContext,
            _: &mut game_engine::common::thing::module_factory::ModuleFactory,
        ) -> Result<(), String> {
            panic!("no births during deletion")
        }
        fn retire_physical_object_drawable(
            &mut self,
            source: &NativeObjectRetirement<'_>,
        ) -> Result<(), String> {
            let id = source.object.id;
            assert!(!source.live_objects.contains_key(&id));
            assert!(!source.live_objects.ordered_ids().contains(&id));
            assert_eq!(source.object.team, Team::USA);
            if id == self.parent {
                assert!(
                    source.live_objects.contains_key(&self.child),
                    "child remains installed until its own queued physical turn"
                );
                assert!(source.live_objects[&self.child].status.destroyed);
                assert_eq!(source.live_objects[&self.child].contained_by, None);
                assert!(source.object.contained_units().is_empty());
            }
            self.calls
                .push((id, source.live_objects.ordered_ids().to_vec()));
            Ok(())
        }
    }
    let (mut owner, queued) = world();
    let (peer, peer_id) = world();
    let child = owner
        .create_object("PhysicalOwner", Team::USA, Vec3::ZERO)
        .unwrap();
    let parent = owner
        .create_object("PhysicalOwner", Team::USA, Vec3::ZERO)
        .unwrap();
    let last = owner
        .create_object("PhysicalOwner", Team::USA, Vec3::ZERO)
        .unwrap();
    // Canonical membership fixture: eligibility/INI capacity is covered by
    // containment suites; this exercises ordinary installed onDelete routing.
    owner
        .objects
        .get_mut(&parent)
        .unwrap()
        .template_mut()
        .contain_module
        .kind = crate::game_logic::thing::ContainModuleKind::Transport;
    owner
        .objects
        .get_mut(&parent)
        .unwrap()
        .occupants
        .push(child);
    owner
        .objects
        .get_mut(&child)
        .unwrap()
        .set_contained_by(Some(parent));
    assert_eq!(owner.objects.ordered_ids(), &[last, parent, child, queued]);
    owner.destroy_object(queued);
    let mut probe = OrderedProbe {
        calls: Vec::new(),
        parent,
        child,
    };
    owner
        .destroy_all_objects_immediate_with_effects_checked(&mut ScriptPresentationEffects {
            view: Some(&mut probe),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        probe.calls,
        vec![
            (queued, vec![last, parent, child]),
            (last, vec![parent, child]),
            (parent, vec![child]),
            (child, vec![]),
        ]
    );
    assert!(owner.objects.is_empty());
    assert!(owner.pending_object_removal_snapshot().is_empty());
    assert_eq!(peer_id, queued);
    assert!(!peer.objects[&peer_id].status.destroyed);
}

// Terrain admission may create GenericBridge once. Ordinary footprint refresh
// is observation/classification, never another factory admission.
fn admitted_bridge_retirement_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "bridge-retirement-owner.ini".into(),
            "Object GenericBridge\nKindOf = STRUCTURE BRIDGE IMMOBILE\nBody = ActiveBody BodyTag\nMaxHealth = 300\nInitialHealth = 300\nEnd\nEnd\n".into(),
        )]).unwrap(),
    ).unwrap();
    world.pathfinding_system.grid =
        crate::game_logic::pathfinding::PathfindingGrid::new(200.0, 200.0, 10.0);
    world.terrain = Some(crate::game_logic::terrain::TerrainData::flat(
        Vec3::ZERO,
        Vec3::new(200.0, 0.0, 200.0),
    ));
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 20;
    map.height = 20;
    map.heightmap = vec![0; 400];
    map.bridges.push(gamelogic::system::map_loader::BridgeData {
        polygon: [(10.0, 30.0), (10.0, 10.0), (30.0, 10.0), (30.0, 30.0)]
            .into_iter()
            .map(|(x, y)| gamelogic::system::map_loader::Coord2D::new(x, y))
            .collect(),
        height: 35.0,
        from: gamelogic::common::Coord3D::new(10.0, 20.0, 35.0),
        to: gamelogic::common::Coord3D::new(30.0, 20.0, 35.0),
        width: 20.0,
        template_name: "RetirementDeck".into(),
    });
    let ground = world.world_services.terrain().clone();
    world
        .admit_map_bridge_geometry(&mut ground.write().unwrap(), map)
        .unwrap();
    assert!(
        world.objects.is_empty(),
        "geometry reserves a slot before one factory birth"
    );
    world
        .seed_pathfinding_from_terrain_with_effects_checked(&mut Default::default())
        .unwrap();
    assert_eq!(world.objects.len(), 1);
    let id = *world.objects.keys().next().unwrap();
    assert_eq!(world.objects[&id].template_name, "GenericBridge");
    assert_ne!(world.objects[&id].visual_object_generation, 0);
    let next_id = world.next_object_id_for_snapshot();
    world.sync_structure_path_blocks();
    assert_eq!(
        world.objects.len(),
        1,
        "ordinary refresh reuses the originally admitted span"
    );
    assert_eq!(world.next_object_id_for_snapshot(), next_id);
    (world, id)
}

#[test]
fn bridge_actor_deletion_refresh_never_resurrects_or_allocates_a_span() {
    let (mut owner, id) = admitted_bridge_retirement_world();
    let (peer, peer_id) = admitted_bridge_retirement_world();
    assert_eq!(id, peer_id);
    let peer_token = peer.objects[&peer_id].visual_draw_identity_token.clone();
    let peer_generation = peer.objects[&peer_id].visual_object_generation;
    let peer_next = peer.next_object_id_for_snapshot();
    let next = owner.next_object_id_for_snapshot();
    let original_slots = owner.pathfinding_system.grid.admitted_bridge_states();
    assert_eq!(original_slots.len(), 1);
    assert_eq!(original_slots[0].1, id.0);
    owner.destroy_object(id);
    owner
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    assert!(
        owner.objects.is_empty(),
        "physical deletion cannot create a replacement GenericBridge"
    );
    assert!(!owner.has_pending_object_removals());
    assert_eq!(owner.next_object_id_for_snapshot(), next);
    for _ in 0..2 {
        owner.sync_structure_path_blocks();
        assert!(owner.objects.is_empty());
        assert_eq!(owner.next_object_id_for_snapshot(), next);
        let slots = owner.pathfinding_system.grid.admitted_bridge_states();
        assert_eq!(slots.len(), original_slots.len());
        assert_eq!(
            slots[0].0, original_slots[0].0,
            "reserved layer identity does not change"
        );
        assert_eq!(
            slots[0].1, id.0,
            "a missing original actor is not rebound to a fresh ID"
        );
        assert!(slots[0].2, "missing span is classified destroyed");
    }
    assert!(!peer.objects[&peer_id].status.destroyed);
    assert_eq!(
        peer.objects[&peer_id].visual_object_generation,
        peer_generation
    );
    assert!(std::sync::Arc::ptr_eq(
        &peer.objects[&peer_id].visual_draw_identity_token,
        &peer_token
    ));
    assert_eq!(peer.next_object_id_for_snapshot(), peer_next);
}

#[test]
fn admitted_bridge_bulk_retirement_finishes_in_one_pass_without_factory_reentry() {
    let (mut owner, id) = admitted_bridge_retirement_world();
    let (peer, peer_id) = admitted_bridge_retirement_world();
    assert_eq!(id, peer_id);
    let next = owner.next_object_id_for_snapshot();
    let peer_next = peer.next_object_id_for_snapshot();
    owner
        .destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
        .unwrap();
    assert!(owner.objects.is_empty());
    assert!(owner.objects.ordered_ids().is_empty());
    assert!(!owner.has_pending_object_removals());
    assert_eq!(
        owner.next_object_id_for_snapshot(),
        next,
        "one-pass bulk disposal never admits replacement bridge lifetimes"
    );
    owner.sync_structure_path_blocks();
    assert!(owner.objects.is_empty());
    assert_eq!(owner.next_object_id_for_snapshot(), next);
    assert!(!peer.objects[&peer_id].status.destroyed);
    assert_eq!(peer.next_object_id_for_snapshot(), peer_next);
}
