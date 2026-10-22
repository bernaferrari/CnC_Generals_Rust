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
