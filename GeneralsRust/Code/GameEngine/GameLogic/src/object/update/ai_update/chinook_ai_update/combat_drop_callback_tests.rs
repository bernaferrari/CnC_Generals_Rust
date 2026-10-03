//! Post-Xfer rope-state regressions with factory-admitted owners and passengers.
//! Callback witnesses exercise the synchronous interface boundary; actual
//! TransportContain door delegation and UnitAI Rappel entry have separate debts.

use super::{
    ChinookAIUpdate, ChinookAIUpdateData, ChinookCombatDropState, ChinookFlightStatus, RopeInfo,
    INVALID_DRAWABLE_ID,
};
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Coord3D, DisabledType, Matrix3D, ObjectID, ObjectStatusMaskType, Real};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{AIUpdateInterface, ContainModuleInterface, ExitDoorType};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::object::Object;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::borrow::Cow;
use std::sync::{Arc, Mutex, RwLock};

type Error = Box<dyn std::error::Error + Send + Sync>;

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_CHINOOK_COMBAT_CALLBACK_CHILD",
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

struct Fixture {
    // The exact child owns these admissions, its immutable authored catalog,
    // and the retained factory Units until process exit. No parent registry is
    // cleared or reset. Rope state below models an already entered/restored
    // combat drop; it makes no claim about renderer bones or rope creation.
    factory: ObjectFactory,
    owner: Arc<RwLock<Object>>,
    ai: ChinookAIUpdate,
}

impl Fixture {
    fn new() -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object CombatCallbackChinook\n KindOf = AIRCRAFT\n Behavior = ChinookAIUpdate CombatFlight\n NumRopes = 2\n RappelSpeed = 30\n PerRopeDelayMin = 1000\n PerRopeDelayMax = 1000\n End\n Behavior = TransportContain CombatCargo\n Slots = 8\n End\nEnd\nObject CombatCallbackRider\n KindOf = INFANTRY CAN_RAPPEL\nEnd\n"
        ), 2);
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                "CombatCallbackChinook",
                Coord3D::new(20.0, 30.0, 50.0),
                None,
                ObjectCreationFlags::NO_DRAWABLE,
            )
            .unwrap();
        let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &owner,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        assert!(
            owner.read().unwrap().get_contain().is_some(),
            "authored TransportContain"
        );
        let template = TheThingFactory::find_template("CombatCallbackChinook").unwrap();
        let module = template
            .as_ref()
            .get_behavior_module_info()
            .iter()
            .find(|entry| entry.name.as_str() == "ChinookAIUpdate")
            .unwrap();
        let data = module
            .data
            .as_ref()
            .downcast_ref::<super::ChinookAIUpdateModuleData>()
            .unwrap();
        let mut ai = ChinookAIUpdate::new(ChinookAIUpdateData::from_module(data), id, 0);
        assert_eq!(ai.data.per_rope_delay_min, 30);
        assert_eq!(ai.data.per_rope_delay_max, 30);
        ai.flight_status = ChinookFlightStatus::DoingCombatDrop;
        ai.combat_drop_started = true;
        ai.combat_drop_pos = Coord3D::new(80.0, 90.0, 0.0);
        owner.write().unwrap().set_disabled(DisabledType::Held);
        Self { factory, owner, ai }
    }

    fn rider(&mut self, z: Real) -> Arc<RwLock<Object>> {
        let id = self
            .factory
            .create_object(
                "CombatCallbackRider",
                Coord3D::new(20.0, 30.0, z),
                None,
                ObjectCreationFlags::NO_DRAWABLE | ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let object = self
            .factory
            .get_object(id)
            .unwrap()
            .get_base_object()
            .unwrap();
        assert!(Arc::ptr_eq(
            &object,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        object
    }

    fn ropes(&mut self, ids: Vec<ObjectID>, deadline: u32) {
        self.ai.combat_drop_state = Some(ChinookCombatDropState {
            ropes: vec![RopeInfo {
                rope_drawable: None,
                rope_drawable_id: INVALID_DRAWABLE_ID,
                drop_start_mtx: Matrix3D::from_translation(glam::Vec3::new(21.0, 32.0, 50.0)),
                rope_speed: 0.0,
                rope_len: 50.0,
                rope_len_max: 50.0,
                next_drop_time: deadline,
                rappeller_ids: ids,
            }],
        });
    }
}

#[derive(Debug, PartialEq)]
enum Event {
    Reserve(ObjectID),
    Exit(ObjectID),
    Speed(ObjectID, Real),
    Rappel(ObjectID),
    Idle(ObjectID),
}
type Events = Arc<Mutex<Vec<Event>>>;

#[derive(Debug)]
struct ExitWitness {
    owner: ObjectID,
    riders: Vec<ObjectID>,
    mutate_owner: bool,
    events: Events,
}

impl ContainModuleInterface for ExitWitness {
    fn can_contain(&self, _: ObjectID) -> bool {
        true
    }
    fn contain_object(&mut self, id: ObjectID) -> Result<(), String> {
        self.riders.push(id);
        Ok(())
    }
    fn release_object(&mut self, id: ObjectID) -> Result<(), String> {
        self.riders.retain(|item| *item != id);
        Ok(())
    }
    fn get_contained_objects(&self) -> Cow<'_, [ObjectID]> {
        Cow::Borrowed(&self.riders)
    }
    fn get_contained_count(&self) -> usize {
        self.riders.len()
    }
    fn get_max_capacity(&self) -> usize {
        8
    }
    fn reserve_door_for_exit(
        &mut self,
        owner: Option<&Object>,
        rider: Option<&Object>,
    ) -> ExitDoorType {
        assert_eq!(owner.unwrap().get_id(), self.owner);
        let id = rider.unwrap().get_id();
        assert!(self.riders.contains(&id));
        self.events.lock().unwrap().push(Event::Reserve(id));
        ExitDoorType::Primary
    }
    fn exit_object_via_door(&mut self, id: ObjectID, door: ExitDoorType) -> Result<(), Error> {
        assert_eq!(door, ExitDoorType::Primary);
        if self.mutate_owner {
            // A synchronous contain callback may mutate its own Object. This
            // is intentionally blocking: an illegal outer owner read must
            // fail through the bounded child, never silently skip the action.
            TheGameLogic::find_object_by_id(self.owner)
                .unwrap()
                .write()
                .unwrap()
                .set_status(ObjectStatusMaskType::MASKED, true);
        }
        TheGameLogic::find_object_by_id(id)
            .unwrap()
            .write()
            .unwrap()
            .set_contained_by(None)
            .unwrap();
        self.release_object(id)?;
        self.events.lock().unwrap().push(Event::Exit(id));
        Ok(())
    }
}

#[derive(Debug)]
struct AiWitness {
    owner: ObjectID,
    carrier: ObjectID,
    speed: Real,
    events: Events,
}
impl AIUpdateInterface for AiWitness {
    fn update(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn get_desired_speed(&self) -> Real {
        self.speed
    }
    fn set_desired_speed(&mut self, speed: Real) {
        TheGameLogic::find_object_by_id(self.owner)
            .unwrap()
            .write()
            .unwrap()
            .set_status(ObjectStatusMaskType::MASKED, true);
        self.speed = speed;
        self.events
            .lock()
            .unwrap()
            .push(Event::Speed(self.owner, speed));
    }
    fn execute_command(&mut self, params: &AiCommandParams) -> Result<(), Error> {
        assert_eq!(params.cmd_source, CommandSourceType::FromAi);
        if params.cmd == AiCommandType::Idle {
            return self.ai_idle();
        }
        assert_eq!(params.cmd, AiCommandType::RappelInto);
        assert_eq!(params.cmd_source, CommandSourceType::FromAi);
        assert_eq!(params.pos, Coord3D::new(80.0, 90.0, 0.0));
        let object = TheGameLogic::find_object_by_id(self.owner).unwrap();
        let mut owner = object.write().unwrap();
        assert_eq!(
            *owner.get_position(),
            Coord3D::new(21.0, 32.0, 50.0),
            "exit precedes transform precedes command"
        );
        owner.set_status(ObjectStatusMaskType::MASKED, false);
        self.events.lock().unwrap().push(Event::Rappel(self.owner));
        Ok(())
    }
    fn ai_idle(&mut self) -> Result<(), Error> {
        TheGameLogic::find_object_by_id(self.carrier)
            .unwrap()
            .write()
            .unwrap()
            .set_status(ObjectStatusMaskType::MASKED, true);
        TheGameLogic::find_object_by_id(self.owner)
            .unwrap()
            .write()
            .unwrap()
            .set_status(ObjectStatusMaskType::MASKED, false);
        self.events.lock().unwrap().push(Event::Idle(self.owner));
        Ok(())
    }
}

fn attach_witnesses(
    fixture: &mut Fixture,
    rider: &Arc<RwLock<Object>>,
    mutate_owner: bool,
) -> Events {
    let events = Arc::new(Mutex::new(Vec::new()));
    let carrier = fixture.owner.read().unwrap().get_id();
    let id = rider.read().unwrap().get_id();
    let contain = Arc::new(Mutex::new(ExitWitness {
        owner: carrier,
        riders: vec![id],
        mutate_owner,
        events: events.clone(),
    }));
    fixture.owner.write().unwrap().set_contain(Some(contain));
    let ai = Arc::new(Mutex::new(AiWitness {
        owner: id,
        carrier,
        speed: 0.0,
        events: events.clone(),
    }));
    rider.write().unwrap().set_ai_update_interface(Some(ai));
    rider
        .write()
        .unwrap()
        .set_contained_by(Some(carrier))
        .unwrap();
    events
}

#[test]
fn live_airborne_rappellers_remain_until_dead_grounded_or_missing() {
    if !child(concat!(
        module_path!(),
        "::live_airborne_rappellers_remain_until_dead_grounded_or_missing"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut fixture = Fixture::new();
    let airborne = fixture.rider(40.0);
    let dead = fixture.rider(40.0);
    let landed = fixture.rider(0.0);
    dead.write().unwrap().set_effectively_dead(true);
    assert!(airborne.read().unwrap().is_above_terrain());
    assert!(!landed.read().unwrap().is_above_terrain());
    let airborne_id = airborne.read().unwrap().get_id();
    let ids = vec![
        airborne_id,
        dead.read().unwrap().get_id(),
        landed.read().unwrap().get_id(),
        0x7fff_fffe,
    ];
    fixture.ropes(ids, 1000);
    let _frame = crate::system::game_logic::enter_update_frame(10);
    assert!(
        !fixture.ai.update_combat_drop(),
        "live airborne passenger keeps the drop active"
    );
    let rope = &fixture.ai.combat_drop_state.as_ref().unwrap().ropes[0];
    assert_eq!(rope.rappeller_ids, vec![airborne_id]);
    assert_eq!(rope.next_drop_time, 1000);
    airborne
        .write()
        .unwrap()
        .set_position(&Coord3D::new(20.0, 30.0, 0.0))
        .unwrap();
    assert!(fixture.ai.update_combat_drop());
    assert!(fixture.ai.combat_drop_state.is_none());
}

#[test]
fn door_exit_callback_can_mutate_exact_carrier_before_rappel() {
    if !child(concat!(
        module_path!(),
        "::door_exit_callback_can_mutate_exact_carrier_before_rappel"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut fixture = Fixture::new();
    let rider = fixture.rider(0.0);
    let id = rider.read().unwrap().get_id();
    let events = attach_witnesses(&mut fixture, &rider, true);
    fixture.ropes(Vec::new(), 10);
    let _frame = crate::system::game_logic::enter_update_frame(10);
    assert!(!fixture.ai.update_combat_drop());
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            Event::Reserve(id),
            Event::Exit(id),
            Event::Speed(id, fixture.ai.data.rappel_speed),
            Event::Rappel(id)
        ]
    );
    assert!(fixture
        .owner
        .read()
        .unwrap()
        .test_status(crate::common::ObjectStatusTypes::Masked));
    assert_eq!(rider.read().unwrap().get_contained_by(), None);
    assert_eq!(
        fixture.ai.combat_drop_state.as_ref().unwrap().ropes[0].rappeller_ids,
        vec![id]
    );
    assert_eq!(
        fixture.ai.combat_drop_state.as_ref().unwrap().ropes[0].next_drop_time,
        40
    );
}

#[test]
fn rappel_callbacks_can_mutate_exact_passenger_without_outer_read() {
    if !child(concat!(
        module_path!(),
        "::rappel_callbacks_can_mutate_exact_passenger_without_outer_read"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut fixture = Fixture::new();
    let rider = fixture.rider(0.0);
    let id = rider.read().unwrap().get_id();
    let events = attach_witnesses(&mut fixture, &rider, false);
    fixture.ropes(Vec::new(), 10);
    let _frame = crate::system::game_logic::enter_update_frame(10);
    assert!(!fixture.ai.update_combat_drop());
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            Event::Reserve(id),
            Event::Exit(id),
            Event::Speed(id, fixture.ai.data.rappel_speed),
            Event::Rappel(id)
        ]
    );
}

#[test]
fn dead_carrier_finish_releases_owner_and_passenger_before_idle() {
    if !child(concat!(
        module_path!(),
        "::dead_carrier_finish_releases_owner_and_passenger_before_idle"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut fixture = Fixture::new();
    let rider = fixture.rider(40.0);
    let id = rider.read().unwrap().get_id();
    let events = attach_witnesses(&mut fixture, &rider, false);
    // Persisted rope members have already left the transport. The carrier's
    // real death flag is what the ordinary updater supplies to onExit.
    let contain = fixture.owner.read().unwrap().get_contain().unwrap();
    contain.lock().unwrap().release_object(id).unwrap();
    rider.write().unwrap().set_contained_by(None).unwrap();
    fixture.owner.write().unwrap().set_effectively_dead(true);
    assert_eq!(rider.read().unwrap().get_contained_by(), None);
    assert!(rider.read().unwrap().is_above_terrain());
    assert!(fixture.owner.read().unwrap().is_effectively_dead());
    fixture.ropes(vec![id], 1000);
    fixture.ai.finish_combat_drop(true);
    assert_eq!(*events.lock().unwrap(), vec![Event::Idle(id)]);
    assert!(!fixture
        .owner
        .read()
        .unwrap()
        .is_disabled_by_type(DisabledType::Held));
    assert_eq!(fixture.ai.flight_status, ChinookFlightStatus::Flying);
    assert!(!fixture.ai.combat_drop_started);
    assert!(fixture.ai.combat_drop_state.is_none());
}
