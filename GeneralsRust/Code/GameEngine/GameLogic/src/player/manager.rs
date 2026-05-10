use super::{player_list, Player, PlayerType};
use crate::commands::command_processor::{
    AIManager, GameObject, ObjectManager, PlayerManager, PlayerResources, ResourceCost,
};
use crate::common::{CommandSourceType, Coord3D, CoordOrigin, Int, ObjectID, INVALID_ID};
use crate::helpers::TheThingFactory;
use crate::modules::AIUpdateInterfaceExt;
use crate::object::object_factory::{get_object_factory, ObjectCreationFlags, ObjectFactory};
use log::{trace, warn};
use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex, RwLock};

/// Object manager bridge implementing the command processor trait.
pub struct ObjectManagerBridge;

struct BridgedObject {
    base: Arc<RwLock<crate::object::Object>>,
}

impl GameObject for BridgedObject {
    fn get_id(&self) -> ObjectID {
        self.base
            .read()
            .map(|obj| obj.get_id())
            .unwrap_or(INVALID_ID)
    }

    fn get_position(&self) -> Coord3D {
        self.base
            .read()
            .map(|obj| *obj.get_position())
            .unwrap_or_else(|_| Coord3D::origin())
    }

    fn get_owner(&self) -> Int {
        if let Ok(obj_guard) = self.base.read() {
            if let Some(team_arc) = obj_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    if let Some(id) = team_guard.get_controlling_player_id() {
                        return id as Int;
                    }
                }
            }
        }

        -1
    }

    fn is_alive(&self) -> bool {
        self.base
            .read()
            .map(|obj| !obj.is_destroyed())
            .unwrap_or(false)
    }

    fn can_be_controlled_by(&self, player_id: Int) -> bool {
        let owner = self.get_owner();
        owner == -1 || owner == player_id
    }
}

// Duplicate BasicAiController definition removed to match the canonical implementation above.

impl ObjectManager for ObjectManagerBridge {
    fn get_object(
        &self,
        id: ObjectID,
    ) -> Option<Arc<dyn crate::commands::command_processor::GameObject>> {
        let factory = get_object_factory();
        let objects = factory.read().ok()?;
        let instance = objects.get_object(id)?;
        let base = instance.get_base_object();
        Some(Arc::new(BridgedObject { base }) as Arc<dyn GameObject>)
    }

    fn get_objects_in_region(&self, _region: &crate::common::IRegion2D) -> Vec<ObjectID> {
        Vec::new()
    }

    fn create_object(
        &mut self,
        template: &str,
        position: Coord3D,
        _player_id: Int,
    ) -> Option<ObjectID> {
        let factory_handle = get_object_factory();
        let mut factory = factory_handle.write().ok()?;
        factory
            .create_object(template, position, None, ObjectCreationFlags::FROM_TEMPLATE)
            .ok()
    }

    fn destroy_object(&mut self, id: ObjectID) -> bool {
        let factory_handle = get_object_factory();
        let result = if let Ok(mut factory) = factory_handle.write() {
            factory.destroy_object(id).is_ok()
        } else {
            false
        };
        result
    }
}

/// Player manager bridge.
pub struct PlayerManagerBridge;

impl PlayerManager for PlayerManagerBridge {
    fn get_player_resources(&self, player_id: Int) -> Option<PlayerResources> {
        let player_arc = {
            let list = player_list().read().ok()?;
            list.get_player(player_id).cloned()
        }?;
        let player = player_arc.read().ok()?;
        Some(PlayerResources {
            supplies: player.get_money().get_money(),
            power_available: player.get_energy().production(),
            power_used: player.get_energy().consumption(),
        })
    }

    fn modify_player_resources(&mut self, player_id: Int, supplies: Int, power: Int) {
        let player_arc = match player_list().read() {
            Ok(list) => list.get_player(player_id).cloned(),
            Err(_) => None,
        };
        if let Some(player_arc) = player_arc {
            if let Ok(mut player) = player_arc.write() {
                player.get_money_mut().add_money(supplies);
                if power > 0 {
                    player.add_power_production(power);
                } else if power < 0 {
                    player.add_power_consumption(-power);
                }
            }
        }
    }

    fn can_player_afford(&self, player_id: Int, cost: &ResourceCost) -> bool {
        let player_arc = match player_list().read() {
            Ok(list) => list.get_player(player_id).cloned(),
            Err(_) => None,
        };
        if let Some(player_arc) = player_arc {
            if let Ok(player) = player_arc.read() {
                return player.get_money().can_afford(cost.supplies);
            }
        }
        false
    }
}

/// AI manager bridge.
pub struct AIManagerBridge {
    object_factory: Arc<RwLock<ObjectFactory>>,
    basic_ai: HashMap<ObjectID, Arc<Mutex<BasicAiController>>>,
}

impl AIManagerBridge {
    pub fn new() -> Self {
        Self {
            object_factory: get_object_factory(),
            basic_ai: HashMap::new(),
        }
    }

    fn ensure_basic_controller(
        &mut self,
        object_id: ObjectID,
        object: Arc<RwLock<crate::object::Object>>,
    ) -> Arc<Mutex<BasicAiController>> {
        match self.basic_ai.entry(object_id) {
            Entry::Occupied(entry) => Arc::clone(entry.get()),
            Entry::Vacant(entry) => {
                let controller = Arc::new(Mutex::new(BasicAiController::new(object)));
                entry.insert(Arc::clone(&controller));
                controller
            }
        }
    }
}

impl AIManager for AIManagerBridge {
    fn issue_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let targets: Vec<(ObjectID, Arc<RwLock<crate::object::Object>>)> = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_move_order: failed to lock object factory");
                return false;
            };
            objects
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect()
        };

        if targets.is_empty() {
            trace!("AIManagerBridge::issue_move_order: no controllable objects supplied");
            return false;
        }

        let mut any_success = false;

        for (object_id, base) in targets {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_move_order: object {} poisoned read lock",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                ai.ai_move_to_position(&destination, false, CommandSourceType::FromPlayer);
                any_success = true;
                continue;
            }

            let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
            match controller.lock() {
                Ok(mut controller) => {
                    if controller.move_to(&destination) {
                        any_success = true;
                    }
                }
                Err(_) => warn!(
                    "AIManagerBridge::issue_move_order: failed to lock basic controller for {}",
                    object_id
                ),
            };
        }

        any_success
    }

    fn issue_waypoint_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let targets: Vec<(ObjectID, Arc<RwLock<crate::object::Object>>)> = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_waypoint_order: failed to lock object factory");
                return false;
            };
            objects
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect()
        };

        if targets.is_empty() {
            trace!("AIManagerBridge::issue_waypoint_order: no controllable objects supplied");
            return false;
        }

        let mut any_success = false;

        for (object_id, base) in targets {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_waypoint_order: object {} poisoned read lock",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                ai.ai_move_to_position(&destination, true, CommandSourceType::FromPlayer);
                any_success = true;
                continue;
            }

            let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
            match controller.lock() {
                Ok(mut controller) => {
                    if controller.move_to(&destination) {
                        any_success = true;
                    }
                }
                Err(_) => warn!(
                    "AIManagerBridge::issue_waypoint_order: failed to lock basic controller for {}",
                    object_id
                ),
            };
        }

        any_success
    }

    fn issue_attack_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let targets: Vec<(ObjectID, Arc<RwLock<crate::object::Object>>)> = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_attack_move_order: failed to lock object factory");
                return false;
            };
            objects
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect()
        };

        if targets.is_empty() {
            trace!("AIManagerBridge::issue_attack_move_order: no controllable objects supplied");
            return false;
        }

        let mut any_success = false;

        for (object_id, base) in targets {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_attack_move_order: object {} poisoned read lock",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                ai.ai_attack_move_to_position(&destination, -1, CommandSourceType::FromPlayer);
                any_success = true;
                continue;
            }

            let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
            match controller.lock() {
                Ok(mut controller) => {
                    if controller.move_to(&destination) {
                        any_success = true;
                    }
                }
                Err(_) => warn!(
                    "AIManagerBridge::issue_attack_move_order: failed to lock basic controller for {}",
                    object_id
                ),
            };
        }

        any_success
    }

    fn issue_attack_order(&mut self, attackers: &[ObjectID], target: ObjectID) -> bool {
        let (attacker_entries, target_base) = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_attack_order: failed to lock object factory");
                return false;
            };

            let attackers = attackers
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect::<Vec<_>>();

            let target = factory
                .get_object(target)
                .map(|instance| instance.get_base_object());

            (attackers, target)
        };

        let Some(target_base) = target_base else {
            warn!(
                "AIManagerBridge::issue_attack_order: target object {} not found",
                target
            );
            return false;
        };

        let target_position = match target_base.read() {
            Ok(guard) => *guard.get_position(),
            Err(_) => {
                warn!(
                    "AIManagerBridge::issue_attack_order: target object {} lock poisoned",
                    target
                );
                return false;
            }
        };

        if attacker_entries.is_empty() {
            trace!("AIManagerBridge::issue_attack_order: no valid attackers supplied");
            return false;
        }

        let mut any_success = false;

        for (object_id, base) in attacker_entries {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_attack_order: attacker {} lock poisoned",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                // Legacy call did not surface errors; assume success when callable.
                ai.ai_attack_position(&target_position, -1, CommandSourceType::FromAi);
                any_success = true;
                continue;
            }

            let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
            match controller.lock() {
                Ok(mut controller) => {
                    if controller.attack_position(&target_position) {
                        any_success = true;
                    }
                }
                Err(_) => warn!(
                    "AIManagerBridge::issue_attack_order: failed to lock basic controller for {}",
                    object_id
                ),
            };
        }

        any_success
    }

    fn issue_build_order(&mut self, builder: ObjectID, template: &str, position: Coord3D) -> bool {
        use game_engine::common::system::build_assistant;

        let builder_base = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_build_order: failed to lock object factory");
                return false;
            };

            let Some(instance) = factory.get_object(builder) else {
                trace!(
                    "AIManagerBridge::issue_build_order: builder {} not found",
                    builder
                );
                return false;
            };

            instance.get_base_object()
        };

        let Some(thing_template) = TheThingFactory::find_template(template) else {
            warn!(
                "AIManagerBridge::issue_build_order: template '{}' not found",
                template
            );
            return false;
        };

        let (builder_snapshot, owning_player, owning_player_arc) = {
            let Ok(builder_guard) = builder_base.read() else {
                warn!(
                    "AIManagerBridge::issue_build_order: builder {} lock poisoned",
                    builder
                );
                return false;
            };

            if builder_guard.is_effectively_dead() {
                trace!(
                    "AIManagerBridge::issue_build_order: builder {} is dead/effectively dead",
                    builder
                );
                return false;
            }

            let player_index = builder_guard
                .get_controlling_player()
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|player_guard| player_guard.get_player_index() as u32)
                })
                .unwrap_or(0);

            (
                build_assistant::Object {
                    id: builder_guard.get_id(),
                    position: build_assistant::Coord3D {
                        x: builder_guard.get_position().x,
                        y: builder_guard.get_position().y,
                        z: builder_guard.get_position().z,
                    },
                    orientation: builder_guard.get_orientation(),
                },
                build_assistant::Player { player_index },
                builder_guard.get_controlling_player(),
            )
        };

        let mut assistant_template =
            build_assistant::ThingTemplate::new(thing_template.get_name().as_str());
        let template_geometry = thing_template.get_template_geometry_info();
        assistant_template.geometry_info.major_radius =
            template_geometry.get_major_radius().max(1.0);
        assistant_template.geometry_info.minor_radius =
            template_geometry.get_minor_radius().max(1.0);
        assistant_template.geometry_info.height =
            template_geometry.get_max_height_above_position().max(1.0);

        let Some(assistant) = build_assistant::get_build_assistant() else {
            warn!("AIManagerBridge::issue_build_order: build assistant unavailable");
            return false;
        };

        let built = assistant.build_object_now(
            Some(&builder_snapshot),
            &assistant_template,
            &build_assistant::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            },
            0.0,
            &owning_player,
        );

        if built.is_none() {
            return false;
        }

        if let Some(player_arc) = owning_player_arc {
            if let Ok(mut player_guard) = player_arc.write() {
                player_guard
                    .get_money_mut()
                    .add_money(-thing_template.get_build_cost());
            }
        }

        true
    }

    fn issue_stop_order(&mut self, objects: &[ObjectID]) -> bool {
        let targets: Vec<(ObjectID, Arc<RwLock<crate::object::Object>>)> = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_stop_order: failed to lock object factory");
                return false;
            };
            objects
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect()
        };

        if targets.is_empty() {
            trace!("AIManagerBridge::issue_stop_order: no controllable objects supplied");
            return false;
        }

        let mut any_success = false;

        for (object_id, base) in targets {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_stop_order: object {} lock poisoned",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                if let Ok(mut ai_guard) = ai.lock() {
                    if ai_guard.ai_idle().is_ok() {
                        any_success = true;
                        continue;
                    } else {
                        trace!(
                            "AIManagerBridge::issue_stop_order: AI module rejected stop for {}",
                            object_id
                        );
                    }
                }
            }

            let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
            match controller.lock() {
                Ok(mut controller) => {
                    if controller.idle() {
                        any_success = true;
                    }
                }
                Err(_) => warn!(
                    "AIManagerBridge::issue_stop_order: failed to lock basic controller for {}",
                    object_id
                ),
            };
        }

        any_success
    }

    fn issue_targeted_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        command: crate::ai::AiCommandType,
    ) -> bool {
        let (targets, target_pos) = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_targeted_order: failed to lock object factory");
                return false;
            };

            let targets = objects
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect::<Vec<_>>();

            let target_pos = factory.get_object(target).and_then(|instance| {
                instance
                    .get_base_object()
                    .read()
                    .ok()
                    .map(|g| *g.get_position())
            });

            (targets, target_pos)
        };

        if targets.is_empty() {
            trace!("AIManagerBridge::issue_targeted_order: no controllable objects supplied");
            return false;
        }

        let mut any_success = false;
        for (object_id, base) in targets {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_targeted_order: object {} lock poisoned",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                if let Ok(mut ai_guard) = ai.lock() {
                    let mut params = crate::ai::AiCommandParams::new(
                        command,
                        crate::ai::CommandSourceType::FromPlayer,
                    );
                    params.obj = Some(target);
                    if ai_guard.execute_command(&params).is_ok() {
                        any_success = true;
                        continue;
                    }

                    trace!(
                        "AIManagerBridge::issue_targeted_order: AI module rejected {:?} for {}",
                        command,
                        object_id
                    );
                }
            }

            if let Some(pos) = target_pos {
                let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
                match controller.lock() {
                    Ok(mut controller) => {
                        if controller.move_to(&pos) {
                            any_success = true;
                        }
                    }
                    Err(_) => warn!(
                        "AIManagerBridge::issue_targeted_order: failed to lock basic controller for {}",
                        object_id
                    ),
                };
            }
        }

        any_success
    }

    fn issue_guard_position_order(
        &mut self,
        objects: &[ObjectID],
        position: Coord3D,
        guard_mode: crate::ai::GuardMode,
    ) -> bool {
        let targets: Vec<(ObjectID, Arc<RwLock<crate::object::Object>>)> = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_guard_position_order: failed to lock object factory");
                return false;
            };
            objects
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect()
        };

        if targets.is_empty() {
            trace!("AIManagerBridge::issue_guard_position_order: no controllable objects supplied");
            return false;
        }

        let mut any_success = false;
        for (object_id, base) in targets {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_guard_position_order: object {} lock poisoned",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                if let Ok(mut ai_guard) = ai.lock() {
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::GuardPosition,
                        CommandSourceType::FromPlayer,
                    );
                    params.pos = position;
                    params.int_value = guard_mode.as_i32();
                    if ai_guard.execute_command(&params).is_ok() {
                        any_success = true;
                        continue;
                    }
                }
            }

            let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
            match controller.lock() {
                Ok(mut controller) => {
                    if controller.move_to(&position) {
                        any_success = true;
                    }
                }
                Err(_) => warn!(
                    "AIManagerBridge::issue_guard_position_order: failed to lock basic controller for {}",
                    object_id
                ),
            };
        }

        any_success
    }

    fn issue_guard_object_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        guard_mode: crate::ai::GuardMode,
    ) -> bool {
        let (targets, target_position) = {
            let Ok(factory) = self.object_factory.read() else {
                warn!("AIManagerBridge::issue_guard_object_order: failed to lock object factory");
                return false;
            };

            let targets = objects
                .iter()
                .filter_map(|object_id| {
                    factory
                        .get_object(*object_id)
                        .map(|instance| (*object_id, instance.get_base_object()))
                })
                .collect::<Vec<_>>();

            let target_position = factory.get_object(target).and_then(|instance| {
                instance
                    .get_base_object()
                    .read()
                    .ok()
                    .map(|g| *g.get_position())
            });

            (targets, target_position)
        };

        if targets.is_empty() {
            trace!("AIManagerBridge::issue_guard_object_order: no controllable objects supplied");
            return false;
        }

        let mut any_success = false;
        for (object_id, base) in targets {
            let ai_handle = match base.read() {
                Ok(obj_guard) => obj_guard.get_ai(),
                Err(_) => {
                    warn!(
                        "AIManagerBridge::issue_guard_object_order: object {} lock poisoned",
                        object_id
                    );
                    None
                }
            };

            if let Some(ai) = ai_handle {
                if let Ok(mut ai_guard) = ai.lock() {
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::GuardObject,
                        CommandSourceType::FromPlayer,
                    );
                    params.obj = Some(target);
                    params.int_value = guard_mode.as_i32();
                    if ai_guard.execute_command(&params).is_ok() {
                        any_success = true;
                        continue;
                    }
                }
            }

            if let Some(pos) = target_position {
                let controller = self.ensure_basic_controller(object_id, Arc::clone(&base));
                match controller.lock() {
                    Ok(mut controller) => {
                        if controller.move_to(&pos) {
                            any_success = true;
                        }
                    }
                    Err(_) => warn!(
                        "AIManagerBridge::issue_guard_object_order: failed to lock basic controller for {}",
                        object_id
                    ),
                };
            }
        }

        any_success
    }
}

#[derive(Debug, Clone)]
enum BasicAiState {
    Idle,
    MovingTo(Coord3D),
    Attacking(Coord3D),
}

impl Default for BasicAiState {
    fn default() -> Self {
        BasicAiState::Idle
    }
}

struct BasicAiController {
    object: Arc<RwLock<crate::object::Object>>,
    state: BasicAiState,
}

impl BasicAiController {
    fn new(object: Arc<RwLock<crate::object::Object>>) -> Self {
        Self {
            object,
            state: BasicAiState::Idle,
        }
    }

    fn move_to(&mut self, destination: &Coord3D) -> bool {
        self.state = BasicAiState::MovingTo(destination.clone());
        self.apply_position(destination)
    }

    fn attack_position(&mut self, destination: &Coord3D) -> bool {
        self.state = BasicAiState::Attacking(destination.clone());
        self.apply_position(destination)
    }

    fn idle(&mut self) -> bool {
        self.state = BasicAiState::Idle;
        true
    }

    fn apply_position(&mut self, destination: &Coord3D) -> bool {
        match self.object.write() {
            Ok(mut object) => object.set_position(destination).is_ok(),
            Err(_) => {
                warn!("BasicAiController: failed to acquire object lock");
                false
            }
        }
    }
}
