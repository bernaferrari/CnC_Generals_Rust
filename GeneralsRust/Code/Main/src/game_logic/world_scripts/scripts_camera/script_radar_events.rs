//! ScriptActions adds one event to the receiving simulation's radar.

use super::HostScriptExecutionDriver;
use crate::game_logic::host_radar::{RADAR_EVENT_SECONDS_TO_LIVE, host_world_to_radar_coord};
use crate::game_logic::{KindOf, ObjectId};
use game_engine::common::system::radar::{Coord3D, RadarEventType};
use generals_sim::GameLogicResult;
use generals_sim::scripting::radar::{RadarEventTarget, team_event_position};

impl HostScriptExecutionDriver<'_> {
    pub(super) fn apply_radar_event(
        &mut self,
        target: RadarEventTarget<'_>,
        kind: RadarEventType,
        this_object: Option<u32>,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> GameLogicResult<()> {
        let position = match target {
            RadarEventTarget::Position(position) => {
                Some(Coord3D::new(position.x, position.y, position.z))
            }
            RadarEventTarget::Object(name) => self
                .world
                .owned_named_script_object(name, this_object.map(ObjectId))
                .and_then(|id| self.world.host_object(id))
                .map(|object| host_world_to_radar_coord(object.get_position())),
            RadarEventTarget::Team(name) => {
                let Some((_, members)) =
                    self.world
                        .resolve_owned_script_team(name, calling_team, condition_team)?
                else {
                    return Ok(());
                };
                team_event_position(members.into_iter().map(|id| {
                    let object = self.world.host_object(ObjectId(id))?;
                    let counts_as_unit = !object.status.effectively_dead
                        && !object.status.destroyed
                        && ![KindOf::Structure, KindOf::Projectile, KindOf::Mine]
                            .into_iter()
                            .any(|kind| object.is_kind_of(kind));
                    Some((object.get_position(), counts_as_unit))
                }))
                .map(host_world_to_radar_coord)
            }
        };
        if let Some(position) = position {
            self.world
                .radar_mut()
                .create_event(&position, kind, RADAR_EVENT_SECONDS_TO_LIVE);
        }
        Ok(())
    }
}
