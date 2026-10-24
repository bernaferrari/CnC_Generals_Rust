//! Executor unit tests.

use super::*;
use crate::common::LocomotorSetType;
use crate::modules::AIUpdateInterface;
use crate::object::drawable::DrawableExt;
use crate::object_manager::ObjectCreationFlags;
use crate::scripting::engine::{
    ScriptActionHandler, ScriptEngine, SequentialScript, get_script_engine,
    initialize_script_engine,
};

fn live_host_named_object(
    name: &str,
    id: u32,
    alive: bool,
) -> crate::scripting::HostScriptQueryObject {
    crate::scripting::HostScriptQueryObject {
        id,
        name: name.into(),
        team: 1,
        x: 2.0,
        z: 2.0,
        alive,
        effectively_dead: !alive,
        health: if alive { 100.0 } else { 0.0 },
        initial_health: 100.0,
        owner_player: "PlyrAmerica".into(),
        template_name: "AmericaInfantryRanger".into(),
        has_contain: true,
        contain_count: 0,
        contain_max: 5,
        last_damage_source_id: 9,
        last_damage_template: "AmericaTankCrusader".into(),
        last_damage_player: "PlyrChina".into(),
        discovered_by: vec!["PlyrAmerica".into()],
        waypoint_labels: vec!["HeroPath".into()],
        ..Default::default()
    }
}

fn live_host_area_unit(
    id: u32,
    owner: &str,
    template_name: &str,
    x: f32,
    z: f32,
    kind_names: &[&str],
) -> crate::scripting::HostScriptQueryObject {
    crate::scripting::HostScriptQueryObject {
        id,
        name: format!("Unit{id}"),
        team: 1,
        x,
        z,
        alive: true,
        effectively_dead: false,
        health: 100.0,
        initial_health: 100.0,
        owner_player: owner.into(),
        template_name: template_name.into(),
        kind_names: kind_names.iter().map(|n| (*n).to_string()).collect(),
        ..Default::default()
    }
}

fn eval_player_unit_type_in_area(
    comparison: i32,
    count: i32,
    type_name: &str,
    area: &str,
) -> Condition {
    let mut condition = Condition::new(ConditionType::PlayerHasComparisonUnitTypeInTriggerArea);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, comparison))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, count))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            type_name.into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            area.into(),
        ))
        .unwrap();
    condition
}

fn eval_player_unit_kind_in_area(comparison: i32, count: i32, kind: i32, area: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::PlayerHasComparisonUnitKindInTriggerArea);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, comparison))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, count))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::KindOfParam, kind))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            area.into(),
        ))
        .unwrap();
    condition
}

fn install_live_hold_zone_census(objects: Vec<crate::scripting::HostScriptQueryObject>) {
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    crate::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(crate::polygon_trigger::PolygonTrigger::new(
            2108,
            crate::common::AsciiString::from("HoldZone"),
            vec![
                crate::common::ICoord3D::new(0, 0, 0),
                crate::common::ICoord3D::new(20, 0, 0),
                crate::common::ICoord3D::new(20, 20, 0),
                crate::common::ICoord3D::new(0, 20, 0),
            ],
        ));
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        objects,
        areas: [("HoldZone".into(), (0.0, 0.0, 20.0, 20.0))]
            .into_iter()
            .collect(),
        ..Default::default()
    });
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod actions_and_team_commands;
mod conditions_and_live_queries;

mod borrowed_engine_tests;

mod public_dispatch_callback_tests;

mod team_guard_driver_tests;

mod borrowed_presentation_owner;

/// Locally owned Core objects and polygon used by player-area fixtures.
/// Never installs an object registry, trigger snapshot, player slot or test lock.
struct OwnedAreaCensus {
    objects: Vec<(String, crate::object::Object)>,
    trigger: crate::polygon_trigger::PolygonTrigger,
}
impl OwnedAreaCensus {
    fn new(objects: Vec<(String, crate::object::Object)>) -> Self {
        Self {
            objects,
            trigger: crate::polygon_trigger::PolygonTrigger::new(
                901,
                "HoldZone".into(),
                vec![
                    crate::common::ICoord3D::new(0, 0, 0),
                    crate::common::ICoord3D::new(20, 0, 0),
                    crate::common::ICoord3D::new(20, 20, 0),
                    crate::common::ICoord3D::new(0, 20, 0),
                ],
            ),
        }
    }
}
impl crate::scripting::engine::ScriptExecutionDriver for OwnedAreaCensus {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn object_count_changed_frame(&self) -> crate::scripting::engine::ScriptOwnerQuery<u32> {
        crate::scripting::engine::ScriptOwnerQuery::Present(0)
    }
    fn player_area_value(
        &self,
        player: &str,
        area: &str,
        _: Option<&str>,
        query: crate::scripting::engine::ScriptPlayerAreaQuery<'_>,
    ) -> crate::scripting::engine::ScriptOwnerQuery<(i32, bool)> {
        use crate::scripting::engine::{ScriptOwnerQuery, ScriptPlayerAreaQuery};
        if area != "HoldZone" || !self.objects.iter().any(|(owner, _)| owner == player) {
            return ScriptOwnerQuery::Missing;
        }
        let mut value = 0i32;
        for (_, object) in self.objects.iter().filter(|(owner, _)| owner == player) {
            let position = object.get_position();
            if !self
                .trigger
                .point_in_trigger_int(&crate::common::ICoord3D::new(
                    position.x as i32,
                    position.y as i32,
                    position.z as i32,
                ))
            {
                continue;
            }
            let inert = object.is_kind_of(crate::common::KindOf::Inert);
            let dead = object.is_effectively_dead();
            let contribution = match &query {
                ScriptPlayerAreaQuery::Templates(names) => {
                    if (!dead && !inert || object.is_kind_of(crate::common::KindOf::Crate))
                        && names.iter().any(|name| name == object.get_template_name())
                    {
                        1
                    } else {
                        0
                    }
                }
                ScriptPlayerAreaQuery::Kind(kind) => {
                    if !dead && !inert && object.is_kind_of(*kind) {
                        1
                    } else {
                        0
                    }
                }
                ScriptPlayerAreaQuery::Units => {
                    if !dead && !inert && !object.is_kind_of(crate::common::KindOf::Projectile) {
                        1
                    } else {
                        0
                    }
                }
                ScriptPlayerAreaQuery::Value => {
                    if !dead && !inert {
                        object.get_template().get_build_cost()
                    } else {
                        0
                    }
                }
            };
            value = value.wrapping_add(contribution);
        }
        ScriptOwnerQuery::Present((value, false))
    }
}
fn owned_area_unit(
    id: u32,
    owner: &str,
    name: &str,
    x: f32,
    y: f32,
    kinds: &[&str],
) -> (String, crate::object::Object) {
    let mut template = crate::common::DefaultThingTemplate::new(name.into());
    for name in kinds {
        if let Some(kind) = parse_kind_of(name) {
            template.add_kind_of(kind);
        }
    }
    let mut object = crate::object::Object::new_raw(
        std::sync::Arc::new(template),
        id,
        crate::common::ObjectStatusMaskType::none(),
        None,
    );
    let position = crate::common::Coord3D::new(x, y, 0.0);
    let mut matrix = object.get_transform_matrix();
    matrix.w_axis = position.extend(1.0);
    object
        .set_transform_matrix_with_services(
            &matrix,
            &crate::terrain::TerrainLogic::new(),
            &mut crate::object::collide::collision_system::CollisionSystem::new(),
            0,
            &mut |_| {},
            &mut |_| {},
        )
        .expect("owned census canonical pose admission");
    assert_eq!(*object.get_position(), position);
    (owner.into(), object)
}

mod borrowed_selection_owner;

mod borrowed_order_owner;

#[derive(Default)]
struct OwnedAiActions {
    effects: Vec<(String, Vec<String>)>,
}
impl ScriptExecutionDriver for OwnedAiActions {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn ai_player(
        &mut self,
        request: crate::scripting::engine::ScriptAiPlayerRequest<'_>,
    ) -> Option<GameLogicResult<()>> {
        use crate::scripting::engine::ScriptAiPlayerRequest::*;
        let effect = match request {
            BuildSupplyCenter {
                player,
                building,
                cash,
            } => (
                "supply".into(),
                vec![player.into(), building.into(), cash.to_string()],
            ),
            BuildUpgrade { player, upgrade } => {
                ("upgrade".into(), vec![player.into(), upgrade.into()])
            }
            BuildNearest {
                player,
                building,
                team,
                ..
            } => (
                "nearest".into(),
                vec![player.into(), building.into(), team.into()],
            ),
            BaseDefense {
                player,
                structure,
                flank,
            } => (
                "defense".into(),
                vec![
                    player.into(),
                    structure.unwrap_or("").into(),
                    flank.to_string(),
                ],
            ),
            SetTeamDelay { player, seconds } => {
                ("delay".into(), vec![player.into(), seconds.to_string()])
            }
            _ => return None,
        };
        self.effects.push(effect);
        Some(Ok(()))
    }
    fn skirmish_approach_path(
        &mut self,
        team: &str,
        path: &str,
        as_team: bool,
        follow: bool,
        _: Option<u32>,
        _: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        self.effects.push((
            "approach".into(),
            vec![
                team.into(),
                path.into(),
                as_team.to_string(),
                follow.to_string(),
            ],
        ));
        Some(Ok(()))
    }
}

mod video_owner_tests;
