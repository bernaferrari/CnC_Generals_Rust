//! C++ AI.h:814-819 command forwarding, not the private move-away pathfinder.

use super::{AIUpdateInterface, AIUpdateInterfaceExt};
use crate::ai::{AiCommandParams, AiCommandType};
use crate::common::{CommandSourceType, Coord3D, ObjectID, ObjectStatusMaskType};
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::system::game_logic::GameLogic;
use std::sync::{Arc, Mutex, RwLock};

type RecordedCommand = (AiCommandType, Option<ObjectID>, CommandSourceType, bool);

#[derive(Debug)]
struct ReceivingAi {
    commands: Arc<Mutex<Vec<RecordedCommand>>>,
    allowed: bool,
}

impl AIUpdateInterface for ReceivingAi {
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

    fn is_allowed_to_move_away_from_unit(&self) -> bool {
        self.allowed
    }

    fn execute_command(
        &mut self,
        params: &AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // The receiver sees every command and applies its own restriction,
        // just as UnitAIUpdate's private MoveAwayFromUnit handler does.
        self.commands.lock().unwrap().push((
            params.cmd,
            params.obj,
            params.cmd_source,
            self.is_allowed_to_move_away_from_unit(),
        ));
        Ok(())
    }
}

struct OtherWorldUnit {
    world: GameLogic,
    object: Arc<RwLock<Object>>,
    id: ObjectID,
}

impl OtherWorldUnit {
    fn using_ability(id: ObjectID) -> Self {
        assert!(OBJECT_REGISTRY.get_object(id).is_none());
        let mut object = Object::new_test(id, 100.0);
        object.set_status(ObjectStatusMaskType::IS_USING_ABILITY, true);
        let object = Arc::new(RwLock::new(object));
        let mut fixture = Self {
            world: GameLogic::new(),
            object,
            id,
        };
        fixture
            .world
            .register_object(fixture.object.clone())
            .expect("other world's actual unit admission");
        fixture
    }
}

impl Drop for OtherWorldUnit {
    fn drop(&mut self) {
        // Keep the object pinned; no controller/object guard spans callbacks.
        let identity_matches = self
            .world
            .find_object_by_id(self.id)
            .is_some_and(|object| Arc::ptr_eq(&object, &self.object));
        assert!(identity_matches, "other world's identity must be retained");
        self.world.destroy_object(self.id);
        self.world
            .process_destroy_list()
            .expect("exact other-world unit retirement");
        assert!(self.world.find_object_by_id(self.id).is_none());
        assert!(OBJECT_REGISTRY.get_object(self.id).is_none());
    }
}

fn receiver(
    allowed: bool,
) -> (
    Arc<Mutex<dyn AIUpdateInterface>>,
    Arc<Mutex<Vec<RecordedCommand>>>,
) {
    let commands = Arc::new(Mutex::new(Vec::new()));
    let ai = Arc::new(Mutex::new(ReceivingAi {
        commands: commands.clone(),
        allowed,
    }));
    (ai, commands)
}

#[test]
fn move_away_forwards_to_receiving_policy_with_original_source() {
    let (ai, commands) = receiver(false);
    ai.ai_move_away_from_unit(0xA1_2A_0001, CommandSourceType::FromScript);
    let observed = commands.lock().unwrap().clone();
    assert_eq!(
        observed,
        vec![(
            AiCommandType::MoveAwayFromUnit,
            Some(0xA1_2A_0001),
            CommandSourceType::FromScript,
            false,
        )],
        "the private receiver decides whether the forwarded command is allowed"
    );
}

#[test]
fn other_world_using_ability_cannot_veto_move_away_dispatch() {
    let _serial = crate::test_sync::lock();
    let other = OtherWorldUnit::using_ability(0xA1_2A_0002);
    let (ai, commands) = receiver(true);
    ai.ai_move_away_from_unit(other.id, CommandSourceType::FromAi);
    let observed = commands.lock().unwrap().clone();
    assert_eq!(
        observed,
        vec![(
            AiCommandType::MoveAwayFromUnit,
            Some(other.id),
            CommandSourceType::FromAi,
            true,
        )],
        "C++ forwards the other unit without rediscovering an ambient owner"
    );
}
