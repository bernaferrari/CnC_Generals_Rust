//! Actual installed repair dock, held owner AI, queue progression and Xfer.
use super::*;
#[path = "dock_owner_access_tests.rs"]
mod access;
use crate::modules::AIUpdateInterface;
use crate::object::production::dock_update::{
    RepairDockUpdate, RepairDockUpdateData, RepairDockUpdateModule,
};
use access::DockTestAccess;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module::ModuleInterfaceType;
use std::io::Cursor;
use std::sync::Mutex;

#[derive(Debug, Default)]
struct DockAI {
    distance: f32,
    targets: Vec<Coord3D>,
    endings: usize,
    dock_delay: u32,
}
impl AIUpdateInterface for DockAI {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        self.distance > 0.0
    }
    fn is_idle(&self) -> bool {
        self.distance == 0.0
    }
    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        self.targets.push(*target);
        Ok(())
    }
    fn get_locomotor_distance_to_goal(&self) -> f32 {
        self.distance
    }
    fn get_supply_truck_ai_interface(&self) -> Option<&dyn SupplyTruckAIInterface> {
        Some(self)
    }
    fn friend_ending_move(&mut self) {
        self.endings += 1;
    }
}

impl SupplyTruckAIInterface for DockAI {
    fn get_upgraded_supply_boost(&self) -> u32 {
        0
    }
    fn get_action_delay_for_dock(
        &self,
        _id: ObjectID,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.dock_delay)
    }
}

struct Fixture {
    objects: Vec<Arc<RwLock<Object>>>,
    ai: Arc<Mutex<DockAI>>,
}
impl Fixture {
    fn new() -> Self {
        let objects: Vec<_> = [0x7A4E0001, 0x7A4E0002, 0x7A4E0003]
            .into_iter()
            .map(|id| Arc::new(RwLock::new(Object::new_test(id, 100.0))))
            .collect();
        for object in &objects {
            let id = object.read().unwrap().get_id();
            crate::object::registry::OBJECT_REGISTRY.register_object(id, object);
        }
        let mut data = RepairDockUpdateData::default();
        data.base.number_approach_positions_data = 3;
        let data = Arc::new(data);
        let dock = RepairDockUpdate::new(
            (*data).clone(),
            objects[0].read().unwrap().get_id(),
            &Coord3D::ZERO,
        );
        let module =
            RepairDockUpdateModule::new(dock, &AsciiString::from("RepairDockUpdate"), data.clone());
        objects[0].write().unwrap().install_module_for_test(
            "RepairDockUpdate",
            Box::new(module),
            data,
            ModuleInterfaceType::UPDATE,
        );
        let ai = Arc::new(Mutex::new(DockAI {
            distance: 10.0,
            ..Default::default()
        }));
        let interface: Arc<Mutex<dyn AIUpdateInterface>> = ai.clone();
        objects[2]
            .write()
            .unwrap()
            .set_ai_update_interface(Some(interface));
        Self { objects, ai }
    }
    fn id(&self, index: usize) -> ObjectID {
        self.objects[index].read().unwrap().get_id()
    }
    fn dock<R>(&self, f: impl FnOnce(&mut dyn DockUpdateInterface) -> R) -> R {
        with_dock(&self.objects[0], f).unwrap()
    }
    fn update_dock(&self) {
        let entries = self.objects[0].read().unwrap().behavior_modules();
        entries[0].with_module(|module| {
            let dock = module
                .as_any_mut()
                .downcast_mut::<RepairDockUpdateModule>()
                .unwrap();
            crate::modules::BehaviorModuleInterface::update(dock.behavior_mut()).unwrap();
        });
    }
    fn occupy_front(&self) {
        self.dock(|dock| {
            let mut pos = Coord3D::ZERO;
            let mut slot = -1;
            assert!(
                dock.reserve_approach_position(self.id(1), &mut pos, &mut slot)
                    .unwrap()
            );
            assert_eq!(slot, 0);
        });
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for object in &self.objects {
            crate::object::registry::OBJECT_REGISTRY
                .unregister_object(object.read().unwrap().get_id());
        }
    }
}

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_AI_DOCK_OWNER_CHILD"
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
fn save(machine: &mut AIDockMachine) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    machine.xfer(&mut XferSave::new(&mut bytes, 1)).unwrap();
    bytes.into_inner()
}

#[test]
fn held_ai_approach_wait_advance_uses_current_slot_and_halt_cancels() {
    if !child(concat!(
        module_path!(),
        "::held_ai_approach_wait_advance_uses_current_slot_and_halt_cancels"
    )) {
        return;
    }
    let fixture = Fixture::new();
    fixture.occupy_front();
    let mut machine = AIDockMachine::new(fixture.objects[2].clone()).unwrap();
    let mut ai = fixture.ai.lock().unwrap();
    assert_eq!(
        machine.start_with_ai(fixture.id(0), &mut *ai),
        StateReturnType::Continue
    );
    assert_eq!(machine.test_slot(), 1);
    assert_eq!(machine.test_state(), Some(AIDockState::Approach.into()));
    ai.distance = 0.0;
    assert_eq!(machine.update_with_ai(&mut *ai), StateReturnType::Continue);
    assert_eq!(
        machine.test_state(),
        Some(AIDockState::WaitForClearance.into())
    );
    fixture.dock(|dock| dock.cancel_dock(fixture.id(1)).unwrap());
    assert_eq!(machine.update_with_ai(&mut *ai), StateReturnType::Continue);
    assert_eq!(
        machine.test_slot(),
        0,
        "Advance must mutate the same slot tested by Wait, rather than a temporary zero"
    );
    assert_eq!(
        machine.test_state(),
        Some(AIDockState::AdvancePosition.into())
    );
    assert_eq!(machine.update_with_ai(&mut *ai), StateReturnType::Continue);
    assert_eq!(
        machine.test_state(),
        Some(AIDockState::WaitForClearance.into())
    );
    assert_eq!(ai.targets.len(), 2);
    assert_eq!(ai.endings, 2);
    machine.halt().unwrap();
    assert!(machine.test_locked());
    assert_eq!(machine.test_state(), None);
    let mut position = Coord3D::ZERO;
    let mut slot = -1;
    fixture.dock(|dock| {
        assert!(
            dock.reserve_approach_position(fixture.id(1), &mut position, &mut slot)
                .unwrap()
        )
    });
    assert_eq!(slot, 0, "halt releases the actual dock reservation");
}

#[test]
fn dock_snapshot_round_trip_retains_slot_state_and_continues_without_ai_relock() {
    if !child(concat!(
        module_path!(),
        "::dock_snapshot_round_trip_retains_slot_state_and_continues_without_ai_relock"
    )) {
        return;
    }
    let fixture = Fixture::new();
    fixture.occupy_front();
    let mut machine = AIDockMachine::new(fixture.objects[2].clone()).unwrap();
    let mut ai = fixture.ai.lock().unwrap();
    assert_eq!(
        machine.start_with_ai(fixture.id(0), &mut *ai),
        StateReturnType::Continue
    );
    ai.distance = 0.0;
    machine.update_with_ai(&mut *ai);
    let bytes = save(&mut machine);
    assert_eq!(
        &bytes[bytes.len() - 4..],
        &1i32.to_le_bytes(),
        "C++ Xfer appends the approach int after StateMachine"
    );
    let mut restored = AIDockMachine::new(fixture.objects[2].clone()).unwrap();
    restored
        .xfer(&mut XferLoad::new(Cursor::new(bytes.clone()), 1))
        .unwrap();
    restored.load_post_process().unwrap();
    assert_eq!(restored.test_slot(), 1);
    assert_eq!(
        restored.test_state(),
        Some(AIDockState::WaitForClearance.into())
    );
    assert_eq!(save(&mut restored), bytes);
    let mut crc = Cursor::new(Vec::new());
    restored.crc(&mut XferSave::new(&mut crc, 1)).unwrap();
    assert!(
        crc.into_inner().is_empty(),
        "StateMachine CRC omits approach position"
    );
    fixture.dock(|dock| dock.cancel_dock(fixture.id(1)).unwrap());
    assert_eq!(restored.update_with_ai(&mut *ai), StateReturnType::Continue);
    assert_eq!(restored.test_slot(), 0);
    restored.halt().unwrap();
}

#[test]
fn equal_id_machine_contexts_keep_independent_queue_values_and_constructor_is_inert() {
    let owner = Arc::new(RwLock::new(Object::new_test(0x7A4E0101, 100.0)));
    let mut first = AIDockMachine::new(owner.clone()).unwrap();
    let mut second = AIDockMachine::new(owner).unwrap();
    first.test_set_slot(4);
    second.test_set_slot(9);
    let bytes = save(&mut first);
    second
        .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
        .unwrap();
    first.test_set_slot(7);
    assert_eq!(second.test_slot(), 4);
    assert_eq!(first.test_slot(), 7);
    assert_eq!(second.test_owner(), first.test_owner());
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(0x7A4E0101)
            .is_none()
    );
}

#[test]
fn held_ai_complete_dock_obeys_action_delay_lock_phases_and_exit_cleanup() {
    if !child(concat!(
        module_path!(),
        "::held_ai_complete_dock_obeys_action_delay_lock_phases_and_exit_cleanup"
    )) {
        return;
    }
    let fixture = Fixture::new();
    let mut machine = AIDockMachine::new(fixture.objects[2].clone()).unwrap();
    let mut ai = fixture.ai.lock().unwrap();
    ai.dock_delay = 5;
    let _frame = crate::system::game_logic::enter_update_frame(100);
    assert_eq!(
        machine.start_with_ai(fixture.id(0), &mut *ai),
        StateReturnType::Continue
    );
    ai.distance = 0.0;
    for expected in [
        AIDockState::WaitForClearance,
        AIDockState::MoveToEntry,
        AIDockState::MoveToDock,
        AIDockState::ProcessDock,
    ] {
        fixture.update_dock();
        assert_eq!(machine.update_with_ai(&mut *ai), StateReturnType::Continue);
        assert_eq!(machine.test_state(), Some(expected.into()));
        assert_eq!(machine.test_locked(), expected == AIDockState::MoveToDock);
    }
    assert_eq!(machine.test_slot(), -1);
    assert_eq!(ai.endings, 3);
    {
        let _frame = crate::system::game_logic::enter_update_frame(104);
        assert_eq!(machine.update_with_ai(&mut *ai), StateReturnType::Continue);
        assert_eq!(machine.test_state(), Some(AIDockState::ProcessDock.into()));
        assert_eq!(
            ai.targets.len(),
            3,
            "no exit movement before action deadline"
        );
    }
    {
        let _frame = crate::system::game_logic::enter_update_frame(105);
        assert_eq!(machine.update_with_ai(&mut *ai), StateReturnType::Continue);
        assert_eq!(machine.test_state(), Some(AIDockState::MoveToExit.into()));
        assert!(!machine.test_locked());
        assert_eq!(ai.targets.len(), 4);
        assert_eq!(machine.update_with_ai(&mut *ai), StateReturnType::Success);
        assert_eq!(machine.test_state(), None);
        assert!(!machine.test_locked());
        assert_eq!(
            ai.endings, 5,
            "terminal rally cleanup retains the borrowed AI"
        );
    }
    machine.halt().unwrap();
}
