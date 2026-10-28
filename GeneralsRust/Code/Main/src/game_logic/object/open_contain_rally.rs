use super::Object;
use glam::Vec3;

impl Object {
    /// C++ `ExitInterface::getRallyPoint` through the actual receiving
    /// interface selected by `Object::getObjectExitInterface`.
    pub(crate) fn rally_point_for_exit(&self) -> Option<Vec3> {
        match self.thing().template.rally_point_exit_owner() {
            // ParkingPlaceBehavior has its own rally state in C++; keep the
            // existing building-owned value until that behavior is migrated.
            crate::game_logic::RallyPointExitOwner::Production
            | crate::game_logic::RallyPointExitOwner::Parking => self
                .building_data
                .as_ref()
                .and_then(|building| building.rally_point),
            crate::game_logic::RallyPointExitOwner::OpenContain => self.open_contain_rally_point,
            crate::game_logic::RallyPointExitOwner::OtherBehavior
            | crate::game_logic::RallyPointExitOwner::None => None,
        }
    }

    /// C++ `ExitInterface::setRallyPoint`; the source behavior wins over the
    /// OpenContain fallback, exactly as the authored interface scan does.
    pub(crate) fn set_rally_point_for_exit(&mut self, point: Vec3) {
        match self.thing().template.rally_point_exit_owner() {
            crate::game_logic::RallyPointExitOwner::Production
            | crate::game_logic::RallyPointExitOwner::Parking => {
                if let Some(building) = self.building_data.as_mut() {
                    building.rally_point = Some(point);
                }
            }
            crate::game_logic::RallyPointExitOwner::OpenContain => {
                self.open_contain_rally_point = Some(point);
            }
            crate::game_logic::RallyPointExitOwner::OtherBehavior
            | crate::game_logic::RallyPointExitOwner::None => {}
        }
    }
}
