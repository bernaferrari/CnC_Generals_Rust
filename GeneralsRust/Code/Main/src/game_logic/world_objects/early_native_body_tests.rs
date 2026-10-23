//! Actual receiving Body operation -> synchronous typed native-view delivery.
use super::*;
use crate::game_logic::{NativeBodyVisualChange, NativeSlaveSource, NativeViewEffects, Player};
use crate::presentation_frame::camera::build::SlaveCameraSample;
use gamelogic::common::types::BodyDamageType;
#[derive(Default)]
struct View(
    Vec<(
        crate::assets::LiveDrawPlaybackIdentity,
        BodyDamageType,
        bool,
        game_client::system::TimeOfDay,
    )>,
);
impl NativeViewEffects for View {
    fn local_player_id(&self) -> u32 {
        0
    }
    fn sample_slave(&mut self, _: &NativeSlaveSource, _: &str) -> SlaveCameraSample {
        unreachable!("not a camera operation")
    }
    fn retire_physical_object_drawable(
        &mut self,
        _: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        Err("this probe does not retire native Drawables".into())
    }
    fn host_epoch(&self) -> u64 {
        1
    }
    fn admit_factory_drawable(
        &mut self,
        _: crate::game_logic::NativeFactoryDrawableSource,
        _: game_client::core::game_client::NativeDrawableBirthContext,
        _: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        Err("factory is outside this callback probe".into())
    }
    fn body_visual_changed(
        &mut self,
        change: &NativeBodyVisualChange,
        _: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        self.0.push((
            change.identity.clone(),
            change.state,
            change.loading_map,
            change.time_of_day.expect("probe world has valid TOD"),
        ));
        Ok(true)
    }
}
fn world(tod: game_engine::common::ini::TimeOfDay) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "early-native-body.ini".into(),
        "Object ReceivingEarlyBody\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
    )]).unwrap()).unwrap();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world.set_receiving_time_of_day(tod);
    let id = world
        .create_object_for_player("ReceivingEarlyBody", 0, glam::Vec3::new(8.0, 3.0, 7.0))
        .unwrap();
    (world, id)
}
fn damage(world: &mut GameLogic, id: ObjectId, amount: f32, view: &mut View) {
    let mut effects = crate::game_logic::ScriptPresentationEffects {
        view: Some(view),
        ..Default::default()
    };
    if amount < 0.0 {
        world
            .apply_owned_healing_with_effects(id, -amount, None, &mut effects)
            .unwrap();
    } else {
        world
            .apply_owned_damage_with_effects(
                id,
                amount,
                None,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
                &mut effects,
            )
            .unwrap();
    }
}
#[test]
fn actual_damage_and_healing_deliver_native_state_once_to_exact_receiving_world() {
    use game_client::system::TimeOfDay as Native;
    use game_engine::common::ini::TimeOfDay as Source;
    let (mut owner, id) = world(Source::Night);
    let (mut peer, peer_id) = world(Source::Morning);
    assert_eq!(id, peer_id);
    let mut owner_view = View::default();
    let mut peer_view = View::default();
    damage(&mut owner, id, 40.0, &mut owner_view);
    assert_eq!(owner_view.0.len(), 1);
    assert_eq!(
        (owner_view.0[0].1, owner_view.0[0].2, owner_view.0[0].3),
        (BodyDamageType::Damaged, false, Native::Night)
    );
    assert_eq!(owner_view.0[0].0.object_id(), id.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
    damage(&mut owner, id, 1.0, &mut owner_view);
    assert_eq!(owner_view.0.len(), 1, "same state has no native transition");
    damage(&mut owner, id, -41.0, &mut owner_view);
    assert_eq!(owner.host_object(id).unwrap().health.current, 100.0);
    assert_eq!(owner_view.0.len(), 2);
    assert_eq!(owner_view.0[1].1, BodyDamageType::Pristine);
    damage(&mut peer, peer_id, 70.0, &mut peer_view);
    assert_eq!(peer_view.0.len(), 1);
    assert_eq!(peer_view.0[0].1, BodyDamageType::ReallyDamaged);
    assert_eq!(peer_view.0[0].3, Native::Morning);
    assert_ne!(owner_view.0[0].0, peer_view.0[0].0);
}
#[test]
fn actual_construction_suppresses_native_transition_and_map_gate_is_receiving() {
    use game_engine::common::ini::TimeOfDay;
    let (mut owner, id) = world(TimeOfDay::Night);
    let mut view = View::default();
    owner
        .host_object_mut(id)
        .unwrap()
        .set_status_under_construction(true);
    damage(&mut owner, id, 40.0, &mut view);
    assert!(view.0.is_empty());
    assert_eq!(owner.host_object(id).unwrap().health.current, 60.0);
    owner
        .host_object_mut(id)
        .unwrap()
        .set_status_under_construction(false);
    owner.begin_map_admission_attempt().unwrap();
    damage(&mut owner, id, 30.0, &mut view);
    assert_eq!(view.0.len(), 1);
    assert!(view.0[0].2);
}
