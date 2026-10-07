//! Supply dock actions must use the exact docker and the AI loan already held
//! by ProcessDock. C++ passes Object*, rather than re-resolving its numeric ID.
use super::*;
use crate::modules::{AIUpdateInterface, DockUpdateInterface, SupplyTruckAIInterface};
use crate::object::production::{
    SupplyCenterDockUpdate, SupplyCenterDockUpdateData, SupplyWarehouseDockUpdate,
    SupplyWarehouseDockUpdateData, SupplyWarehouseDockUpdateModule,
};
use crate::supply_system::{SupplyTruckAIUpdate, SupplyTruckAIUpdateData};
use game_engine::common::thing::module::ModuleInterfaceType;
use std::sync::Mutex;

#[derive(Debug)]
struct TruckAI(SupplyTruckAIUpdate);
impl TruckAI {
    fn new(id: ObjectID, capacity: i32) -> Self {
        Self(SupplyTruckAIUpdate::new(
            SupplyTruckAIUpdateData {
                max_boxes: capacity,
                ..Default::default()
            },
            id,
            0,
        ))
    }
}
impl AIUpdateInterface for TruckAI {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn get_supply_truck_ai_interface(&self) -> Option<&dyn SupplyTruckAIInterface> {
        Some(&self.0)
    }
    fn get_supply_truck_ai_interface_mut(&mut self) -> Option<&mut dyn SupplyTruckAIInterface> {
        Some(&mut self.0)
    }
}

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_SUPPLY_DOCK_LOAN_CHILD"
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
fn object(id: ObjectID) -> Arc<RwLock<Object>> {
    Arc::new(RwLock::new(Object::new_test(id, 100.0)))
}
fn admit(owner: &Arc<RwLock<Object>>) {
    let id = owner.read().unwrap().get_id();
    crate::object::registry::OBJECT_REGISTRY.register_object(id, owner);
}
fn install_ai(owner: &Arc<RwLock<Object>>, capacity: i32) -> Arc<Mutex<TruckAI>> {
    let ai = Arc::new(Mutex::new(TruckAI::new(
        owner.read().unwrap().get_id(),
        capacity,
    )));
    let handle: Arc<Mutex<dyn AIUpdateInterface>> = ai.clone();
    owner.write().unwrap().set_ai_update_interface(Some(handle));
    ai
}
fn warehouse(owner: &Arc<RwLock<Object>>, boxes: i32) -> SupplyWarehouseDockUpdate {
    let data = SupplyWarehouseDockUpdateData {
        starting_boxes: boxes,
        delete_when_empty: false,
        ..Default::default()
    };
    SupplyWarehouseDockUpdate::new(data, owner.read().unwrap().get_id(), &Coord3D::ZERO)
}

#[test]
fn held_ai_warehouse_action_uses_exact_equal_id_docker() {
    if !child(concat!(
        module_path!(),
        "::held_ai_warehouse_action_uses_exact_equal_id_docker"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let depot = object(0x7A5A1001);
    admit(&depot);
    let actual = object(0x7A5A1002);
    let shadow = object(0x7A5A1002);
    admit(&shadow);
    let actual_ai = install_ai(&actual, 3);
    let shadow_ai = install_ai(&shadow, 0);
    let mut loan = actual_ai.lock().unwrap();
    assert!(
        actual_ai.try_lock().is_err(),
        "actual installed AI is already held"
    );
    let mut dock = warehouse(&depot, 2);
    assert!(dock.action_with_ai(&actual, None, &mut *loan).unwrap());
    assert_eq!(loan.0.get_number_boxes(), 1);
    assert_eq!(dock.get_boxes_stored(), 1);
    assert_eq!(shadow_ai.lock().unwrap().0.get_number_boxes(), 0);
    assert!(Arc::ptr_eq(
        &crate::object::registry::OBJECT_REGISTRY
            .get_object(0x7A5A1002)
            .unwrap(),
        &shadow
    ));
}

#[test]
fn rejected_warehouse_box_restores_stock_on_driving_ai() {
    if !child(concat!(
        module_path!(),
        "::rejected_warehouse_box_restores_stock_on_driving_ai"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let depot = object(0x7A5A1101);
    admit(&depot);
    let actual = object(0x7A5A1102);
    let actual_ai = install_ai(&actual, 0);
    let shadow = object(0x7A5A1102);
    admit(&shadow);
    let shadow_ai = install_ai(&shadow, 3);
    let mut loan = actual_ai.lock().unwrap();
    let mut dock = warehouse(&depot, 2);
    assert!(!dock.action_with_ai(&actual, None, &mut *loan).unwrap());
    assert_eq!(
        dock.get_boxes_stored(),
        2,
        "C++ restores stock if gainOneBox rejects it"
    );
    assert_eq!(loan.0.get_number_boxes(), 0);
    assert_eq!(shadow_ai.lock().unwrap().0.get_number_boxes(), 0);
}

#[test]
fn native_process_dock_forwards_held_truck_loan_to_installed_warehouse() {
    if !child(concat!(
        module_path!(),
        "::native_process_dock_forwards_held_truck_loan_to_installed_warehouse"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let depot = object(0x7A5A1201);
    admit(&depot);
    let actual = object(0x7A5A1202);
    let actual_ai = install_ai(&actual, 3);
    let shadow = object(0x7A5A1202);
    admit(&shadow);
    let shadow_ai = install_ai(&shadow, 0);
    let data = Arc::new(SupplyWarehouseDockUpdateData {
        starting_boxes: 2,
        delete_when_empty: false,
        ..Default::default()
    });
    let module = SupplyWarehouseDockUpdateModule::new(
        warehouse(&depot, 2),
        &"SupplyWarehouseDockUpdate".into(),
        data.clone(),
    );
    depot.write().unwrap().install_module_for_test(
        "SupplyWarehouseDockUpdate",
        Box::new(module),
        data,
        ModuleInterfaceType::UPDATE,
    );
    let mut machine = AIDockMachine::new(actual.clone()).unwrap();
    machine
        .state_machine
        .set_goal_object_by_id(Some(0x7A5A1201));
    let mut loan = actual_ai.lock().unwrap();
    assert_eq!(
        machine.state_machine.set_current_state_with_ai_and_owner(
            AIDockState::ProcessDock.into(),
            &mut *loan,
            &mut machine.context
        ),
        StateReturnType::Continue
    );
    assert_eq!(
        machine.update_with_ai(&mut *loan),
        StateReturnType::Continue
    );
    assert_eq!(
        machine.state_machine.get_current_state_id(),
        Some(AIDockState::ProcessDock.into())
    );
    assert_eq!(
        loan.0.get_number_boxes(),
        1,
        "the actual ProcessDock callback must consume its supplied loan"
    );
    assert_eq!(shadow_ai.lock().unwrap().0.get_number_boxes(), 0);
    assert_eq!(
        with_dock(&depot, |dock| dock.supply_warehouse_boxes_stored()),
        Some(Some(1))
    );
}

#[test]
fn held_ai_center_action_unloads_driving_truck_and_scores_deposit() {
    if !child(concat!(
        module_path!(),
        "::held_ai_center_action_unloads_driving_truck_and_scores_deposit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let center = object(0x7A5A1301);
    admit(&center);
    let actual = object(0x7A5A1302);
    let actual_ai = install_ai(&actual, 3);
    let shadow = object(0x7A5A1302);
    admit(&shadow);
    let shadow_ai = install_ai(&shadow, 0);
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    crate::player::player_list()
        .write()
        .unwrap()
        .add_player(player.clone());
    let mut team = crate::team::Team::new("supply-dock-owner".into(), 0x7A5A1303);
    team.set_controlling_player_id(Some(0));
    center
        .write()
        .unwrap()
        .set_team(Some(Arc::new(RwLock::new(team))))
        .unwrap();
    center
        .write()
        .unwrap()
        .set_status(ObjectStatusMaskType::STEALTHED, true);
    let value = player.read().unwrap().get_supply_box_value();
    assert!(value > 0);
    let initial_money = player.read().unwrap().get_money().get_money();
    let initial_score = player
        .read()
        .unwrap()
        .get_score_keeper()
        .get_total_money_earned();
    let mut loan = actual_ai.lock().unwrap();
    assert!(loan.0.gain_one_box(4));
    assert!(loan.0.gain_one_box(3));
    let mut dock = SupplyCenterDockUpdate::new(
        SupplyCenterDockUpdateData::default(),
        0x7A5A1301,
        &Coord3D::ZERO,
    );
    assert!(
        !dock.action_with_ai(&actual, None, &mut *loan).unwrap(),
        "C++ center action completes after one delivery"
    );
    assert_eq!(loan.0.get_number_boxes(), 0);
    assert_eq!(shadow_ai.lock().unwrap().0.get_number_boxes(), 0);
    assert_eq!(
        player.read().unwrap().get_money().get_money(),
        initial_money + (2 * value) as i32
    );
    assert_eq!(
        player
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_money_earned(),
        initial_score + (2 * value) as i32
    );
}

#[test]
fn registered_center_action_drops_object_guard_before_stock_callback() {
    if !child(concat!(
        module_path!(),
        "::registered_center_action_drops_object_guard_before_stock_callback"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let center = object(0x7A5A1401);
    admit(&center);
    let actual = object(0x7A5A1402);
    let actual_ai = install_ai(&actual, 3);
    admit(&actual);
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    crate::player::player_list()
        .write()
        .unwrap()
        .add_player(player.clone());
    let mut team = crate::team::Team::new("supply-dock-owner".into(), 0x7A5A1403);
    team.set_controlling_player_id(Some(0));
    center
        .write()
        .unwrap()
        .set_team(Some(Arc::new(RwLock::new(team))))
        .unwrap();
    center
        .write()
        .unwrap()
        .set_status(ObjectStatusMaskType::STEALTHED, true);
    let value = player.read().unwrap().get_supply_box_value();
    assert!(value > 0);
    let initial_money = player.read().unwrap().get_money().get_money();
    let initial_score = player
        .read()
        .unwrap()
        .get_score_keeper()
        .get_total_money_earned();
    let mut loan = actual_ai.lock().unwrap();
    assert!(loan.0.gain_one_box(4));
    assert!(loan.0.gain_one_box(3));
    let mut dock = SupplyCenterDockUpdate::new(
        SupplyCenterDockUpdateData::default(),
        0x7A5A1401,
        &Coord3D::ZERO,
    );
    assert!(
        !dock.action_with_ai(&actual, None, &mut *loan).unwrap(),
        "C++ center action completes after one delivery"
    );
    assert_eq!(loan.0.get_number_boxes(), 0);
    assert_eq!(
        player.read().unwrap().get_money().get_money(),
        initial_money + (2 * value) as i32
    );
    assert_eq!(
        player
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_money_earned(),
        initial_score + (2 * value) as i32
    );
}

#[test]
fn standalone_registered_warehouse_releases_object_before_stock_callback() {
    if !child(concat!(
        module_path!(),
        "::standalone_registered_warehouse_releases_object_before_stock_callback"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let depot = object(0x7A5A1501);
    admit(&depot);
    let docker = object(0x7A5A1502);
    admit(&docker);
    let ai = install_ai(&docker, 3);
    let mut dock = warehouse(&depot, 2);
    assert!(dock.action(0x7A5A1502, None).unwrap());
    assert_eq!(ai.lock().unwrap().0.get_number_boxes(), 1);
    assert_eq!(dock.get_boxes_stored(), 1);
}
