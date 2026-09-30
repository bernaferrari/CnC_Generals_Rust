use super::super::*;
use crate::game_logic::object::unit_ai_runtime::UnitAiRuntime;

impl GameLogic {
    pub(crate) fn unit_ai_runtime(&self, id: ObjectId) -> Option<&UnitAiRuntime> {
        self.objects.get(&id).map(|object| &object.unit_ai_runtime)
    }

    pub(crate) fn unit_ai_runtime_mut(&mut self, id: ObjectId) -> Option<&mut UnitAiRuntime> {
        self.objects
            .get_mut(&id)
            .map(|object| &mut object.unit_ai_runtime)
    }

    pub(crate) fn clear_unit_hunt_scan(&mut self, id: ObjectId) {
        if let Some(runtime) = self.unit_ai_runtime_mut(id) {
            runtime.clear_hunt();
        }
    }
}
