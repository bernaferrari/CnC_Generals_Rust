use super::{Player, PlayerType, player_list};
use crate::commands::command_processor::{
    AIManager, GameObject, ObjectManager, PlayerManager, PlayerResources, ResourceCost,
};
use crate::common::{CommandSourceType, Coord3D, CoordOrigin, INVALID_ID, Int, ObjectID};
use crate::helpers::TheThingFactory;
use crate::modules::AIUpdateInterfaceExt;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory, get_object_factory};
use log::{trace, warn};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Object manager bridge implementing the command processor trait.
pub struct ObjectManagerBridge;

struct BridgedObject {
    id: ObjectID,
}

impl GameObject for BridgedObject {
    fn get_id(&self) -> ObjectID {
        self.id
    }

    fn get_position(&self) -> Coord3D {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.id, |obj| *obj.get_position())
            .unwrap_or_else(Coord3D::origin)
    }

    fn get_owner(&self) -> Int {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.id, |obj| obj.get_controlling_player_id().map(|id| id as Int))
            .flatten()
            .unwrap_or(-1)
    }

    fn is_alive(&self) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(self.id, |obj| !obj.is_destroyed())
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
        crate::object::registry::OBJECT_REGISTRY
            .with_object(id, |_| ())
            .map(|_| Arc::new(BridgedObject { id }) as Arc<dyn GameObject>)
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
        with_player(player_id, |player| PlayerResources {
            supplies: player.get_money().get_money(),
            power_available: player.get_energy().production(),
            power_used: player.get_energy().consumption(),
        })
    }

    fn modify_player_resources(&mut self, player_id: Int, supplies: Int, power: Int) {
        let _ = with_player_mut(player_id, |player| {
            player.get_money_mut().add_money(supplies);
            if power > 0 {
                player.add_power_production(power);
            } else if power < 0 {
                player.add_power_consumption(-power);
            }
        });
    }

    fn can_player_afford(&self, player_id: Int, cost: &ResourceCost) -> bool {
        with_player(player_id, |player| player.get_money().can_afford(cost.supplies)).unwrap_or(false)
    }
}

/// AI manager bridge.
pub struct AIManagerBridge {
    object_factory: Arc<RwLock<ObjectFactory>>,
    basic_ai: HashMap<ObjectID, BasicAiController>,
}

impl AIManagerBridge {
    pub fn new() -> Self {
        Self {
            object_factory: get_object_factory(),
            basic_ai: HashMap::new(),
        }
    }

    fn ensure_basic_controller(&mut self, object_id: ObjectID) -> &mut BasicAiController {
        self.basic_ai
            .entry(object_id)
            .or_insert_with(|| BasicAiController::new(object_id))
    }

    fn live_ids(&self, ids: &[ObjectID]) -> Vec<ObjectID> {
        ids.iter()
            .copied()
            .filter(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(*id, |_| ())
                    .is_some()
            })
            .collect()
    }
}

impl AIManager for AIManagerBridge {
    fn issue_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let targets = self.live_ids(objects);
        if targets.is_empty() {
            trace!("AIManagerBridge::issue_move_order: no controllable objects supplied");
            return false;
        }
        let mut any_success = false;
        for object_id in targets {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
            if let Some(ai) = ai_handle {
                ai.ai_move_to_position(&destination, false, CommandSourceType::FromPlayer);
                any_success = true;
                continue;
            }
            if self.ensure_basic_controller(object_id).move_to(&destination) {
                any_success = true;
            }
        }
        any_success
    }

    fn issue_waypoint_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let targets = self.live_ids(objects);
        if targets.is_empty() {
            trace!("AIManagerBridge::issue_waypoint_order: no controllable objects supplied");
            return false;
        }
        let mut any_success = false;
        for object_id in targets {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
            if let Some(ai) = ai_handle {
                ai.ai_move_to_position(&destination, true, CommandSourceType::FromPlayer);
                any_success = true;
                continue;
            }
            if self.ensure_basic_controller(object_id).move_to(&destination) {
                any_success = true;
            }
        }
        any_success
    }

    fn issue_attack_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let targets = self.live_ids(objects);
        if targets.is_empty() {
            trace!("AIManagerBridge::issue_attack_move_order: no controllable objects supplied");
            return false;
        }
        let mut any_success = false;
        for object_id in targets {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
            if let Some(ai) = ai_handle {
                ai.ai_attack_move_to_position(&destination, -1, CommandSourceType::FromPlayer);
                any_success = true;
                continue;
            }
            if self.ensure_basic_controller(object_id).move_to(&destination) {
                any_success = true;
            }
        }
        any_success
    }

    fn issue_attack_order(&mut self, attackers: &[ObjectID], target: ObjectID) -> bool {
        let Some(target_position) = crate::object::registry::OBJECT_REGISTRY
            .with_object(target, |obj| *obj.get_position())
        else {
            warn!(
                "AIManagerBridge::issue_attack_order: target object {} not found",
                target
            );
            return false;
        };
        let attacker_entries = self.live_ids(attackers);
        if attacker_entries.is_empty() {
            trace!("AIManagerBridge::issue_attack_order: no valid attackers supplied");
            return false;
        }
        let mut any_success = false;
        for object_id in attacker_entries {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
            if let Some(ai) = ai_handle {
                ai.ai_attack_position(&target_position, -1, CommandSourceType::FromAi);
                any_success = true;
                continue;
            }
            if self
                .ensure_basic_controller(object_id)
                .attack_position(&target_position)
            {
                any_success = true;
            }
        }
        any_success
    }

    fn issue_build_order(&mut self, builder: ObjectID, template: &str, position: Coord3D) -> bool {
        use game_engine::common::system::build_assistant;

        let Some(thing_template) = TheThingFactory::find_template(template) else {
            warn!(
                "AIManagerBridge::issue_build_order: template '{}' not found",
                template
            );
            return false;
        };

        let Some((builder_snapshot, owning_player, owning_player_index)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(builder, |builder_guard| {
                if builder_guard.is_effectively_dead() {
                    return None;
                }
                let player_index = builder_guard.get_controlling_player_id().unwrap_or(0);
                Some((
                    build_assistant::Object {
                        id: builder_guard.get_id(),
                        position: build_assistant::Coord3D {
                            x: builder_guard.get_position().x,
                            y: builder_guard.get_position().y,
                            z: builder_guard.get_position().z,
                        },
                        orientation: builder_guard.get_orientation(),
                        command_set: None,
                    },
                    build_assistant::Player { player_index },
                    player_index,
                ))
            })
            .flatten()
        else {
            trace!(
                "AIManagerBridge::issue_build_order: builder {} not found or dead",
                builder
            );
            return false;
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

        let _ = with_player_mut(owning_player_index as Int, |player| {
            player
                .get_money_mut()
                .add_money(-thing_template.get_build_cost());
        });

        true
    }

    fn issue_stop_order(&mut self, objects: &[ObjectID]) -> bool {
        let targets = self.live_ids(objects);
        if targets.is_empty() {
            trace!("AIManagerBridge::issue_stop_order: no controllable objects supplied");
            return false;
        }
        let mut any_success = false;
        for object_id in targets {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
            if let Some(ai) = ai_handle {
                if let Ok(mut ai_guard) = ai.lock() {
                    if ai_guard.ai_idle().is_ok() {
                        any_success = true;
                        continue;
                    }
                }
            }
            if self.ensure_basic_controller(object_id).idle() {
                any_success = true;
            }
        }
        any_success
    }

    fn issue_targeted_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        command: crate::ai::AiCommandType,
    ) -> bool {
        let targets = self.live_ids(objects);
        if targets.is_empty() {
            trace!("AIManagerBridge::issue_targeted_order: no controllable objects supplied");
            return false;
        }
        let target_pos = crate::object::registry::OBJECT_REGISTRY
            .with_object(target, |obj| *obj.get_position());
        let mut any_success = false;
        for object_id in targets {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
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
                }
            }
            if let Some(pos) = target_pos {
                if self.ensure_basic_controller(object_id).move_to(&pos) {
                    any_success = true;
                }
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
        let targets = self.live_ids(objects);
        if targets.is_empty() {
            trace!("AIManagerBridge::issue_guard_position_order: no controllable objects supplied");
            return false;
        }
        let mut any_success = false;
        for object_id in targets {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
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
            if self.ensure_basic_controller(object_id).move_to(&position) {
                any_success = true;
            }
        }
        any_success
    }

    fn issue_guard_object_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        guard_mode: crate::ai::GuardMode,
    ) -> bool {
        let targets = self.live_ids(objects);
        if targets.is_empty() {
            trace!("AIManagerBridge::issue_guard_object_order: no controllable objects supplied");
            return false;
        }
        let target_position = crate::object::registry::OBJECT_REGISTRY
            .with_object(target, |obj| *obj.get_position());
        let mut any_success = false;
        for object_id in targets {
            let ai_handle = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai())
                .flatten();
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
                if self.ensure_basic_controller(object_id).move_to(&pos) {
                    any_success = true;
                }
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
    object_id: ObjectID,
    state: BasicAiState,
}

impl BasicAiController {
    fn new(object_id: ObjectID) -> Self {
        Self {
            object_id,
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
        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(self.object_id, |object| object.set_position(destination).is_ok())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod basic_ai_ownership_tests {
    use super::*;
    use crate::common::Coord3D;
    use crate::object::Object;
    use crate::object::registry::OBJECT_REGISTRY;

    fn bridge() -> AIManagerBridge {
        AIManagerBridge {
            object_factory: Arc::new(RwLock::new(ObjectFactory::new())),
            basic_ai: HashMap::new(),
        }
    }

    fn register(id: ObjectID) {
        OBJECT_REGISTRY.register_object(id, Object::new_test(id, 100.0));
    }

    #[test]
    fn basic_controller_is_reused_for_the_same_bridge_id() {
        register(0xA1_001);
        let mut bridge = bridge();
        assert!(
            bridge
                .ensure_basic_controller(0xA1_001)
                .move_to(&Coord3D::new(10.0, 20.0, 0.0))
        );
        assert!(
            bridge
                .ensure_basic_controller(0xA1_001)
                .attack_position(&Coord3D::new(30.0, 40.0, 0.0))
        );
        let controller = bridge.basic_ai.get(&0xA1_001).expect("controller reused");
        assert!(matches!(&controller.state, BasicAiState::Attacking(_)));
        let pos = OBJECT_REGISTRY
            .with_object(0xA1_001, |object| *object.get_position())
            .expect("object");
        assert_eq!(pos, Coord3D::new(30.0, 40.0, 0.0));
    }

    #[test]
    fn distinct_bridges_own_independent_same_id_controllers() {
        register(0xA1_002);
        let mut first = bridge();
        let mut second = bridge();
        assert!(
            first
                .ensure_basic_controller(0xA1_002)
                .move_to(&Coord3D::new(5.0, 0.0, 0.0))
        );
        assert!(
            second
                .ensure_basic_controller(0xA1_002)
                .move_to(&Coord3D::new(0.0, 7.0, 0.0))
        );
        assert_eq!(first.basic_ai.len(), 1);
        assert_eq!(second.basic_ai.len(), 1);
        assert!(matches!(
            &first.basic_ai[&0xA1_002].state,
            BasicAiState::MovingTo(_)
        ));
        assert!(matches!(
            &second.basic_ai[&0xA1_002].state,
            BasicAiState::MovingTo(_)
        ));
    }
}
