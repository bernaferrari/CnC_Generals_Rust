//! Executor unit tests.

use super::*;
use crate::common::LocomotorSetType;
use crate::modules::AIUpdateInterface;
use crate::object::drawable::DrawableExt;
use crate::object_manager::ObjectCreationFlags;
use crate::scripting::engine::{
    ScriptActionHandler, ScriptEngine, SequentialScript, initialize_script_engine,
    with_script_engine_mut,
};
use std::sync::Mutex;

#[derive(Debug)]
struct RecordingAi {
    commands: Arc<
        Mutex<
            Vec<(
                AiCommandType,
                Option<ObjectID>,
                Option<String>,
                i32,
                CommandSourceType,
            )>,
        >,
    >,
    locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
}

struct ReentrantWorldActionHandler {
    calls: Arc<Mutex<Vec<String>>>,
}

impl ReentrantWorldActionHandler {
    fn record(&self, call: impl Into<String>) {
        self.calls.lock().unwrap().push(call.into());
        let reentered = with_script_engine_mut(|engine| {
            engine.increment_counter("WorldHandlerImmediateReentry", 1)
        });
        assert!(
            matches!(reentered, Some(Ok(()))),
            "a host/UI callback must be able to re-enter the active ScriptEngine immediately"
        );
    }
}

impl ScriptActionHandler for ReentrantWorldActionHandler {
    fn display_text(&self, text: &str) -> GameLogicResult<()> {
        self.record(format!("text:{text}"));
        Ok(())
    }

    fn movie_play_fullscreen(&self, filename: &str) -> GameLogicResult<()> {
        self.record(format!("movie:{filename}"));
        Ok(())
    }

    fn music_set_track(&self, name: &str, fade_out: bool, fade_in: bool) -> GameLogicResult<()> {
        self.record(format!("music:{name}:{fade_out}:{fade_in}"));
        Ok(())
    }

    fn zoom_camera(
        &self,
        zoom: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.record(format!(
            "zoom:{zoom}:{seconds}:{ease_in_seconds}:{ease_out_seconds}"
        ));
        Ok(())
    }

    fn set_radar_forced(&self, forced: bool) -> GameLogicResult<()> {
        self.record(format!("radar:{forced}"));
        Ok(())
    }

    fn add_named_timer(&self, name: &str, text: &str, countdown: bool) -> GameLogicResult<()> {
        self.record(format!("timer:{name}:{text}:{countdown}"));
        Ok(())
    }

    fn freeze_time(&self) -> GameLogicResult<()> {
        self.record("freeze");
        Ok(())
    }

    fn unfreeze_time(&self) -> GameLogicResult<()> {
        self.record("unfreeze");
        Ok(())
    }

    fn set_visual_speed_multiplier(&self, multiplier: i32) -> GameLogicResult<()> {
        self.record(format!("speed:{multiplier}"));
        Ok(())
    }

    fn set_weather_visible(&self, visible: bool) -> GameLogicResult<()> {
        self.record(format!("weather:{visible}"));
        Ok(())
    }

    fn hide_object_superweapon_display_by_script(
        &self,
        object_id: ObjectID,
    ) -> GameLogicResult<()> {
        self.record(format!("hide-superweapon:{object_id}"));
        Ok(())
    }

    fn show_object_superweapon_display_by_script(
        &self,
        object_id: ObjectID,
    ) -> GameLogicResult<()> {
        self.record(format!("show-superweapon:{object_id}"));
        Ok(())
    }
}

impl AIUpdateInterface for RecordingAi {
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

    fn choose_locomotor_set(
        &mut self,
        set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.locomotors.lock().unwrap().push(set);
        Ok(())
    }

    fn execute_command(
        &mut self,
        command: &AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commands.lock().unwrap().push((
            command.cmd,
            command.obj,
            command.team.clone(),
            command.int_value,
            command.cmd_source,
        ));
        Ok(())
    }
}

#[derive(Debug)]
struct RecruitableRecordingAi {
    commands: Arc<
        Mutex<
            Vec<(
                AiCommandType,
                Option<ObjectID>,
                Option<String>,
                i32,
                CommandSourceType,
            )>,
        >,
    >,
    recruitable: Arc<Mutex<Vec<bool>>>,
}

impl AIUpdateInterface for RecruitableRecordingAi {
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

    fn execute_command(
        &mut self,
        command: &AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commands.lock().unwrap().push((
            command.cmd,
            command.obj,
            command.team.clone(),
            command.int_value,
            command.cmd_source,
        ));
        Ok(())
    }

    fn set_is_recruitable(&mut self, recruitable: bool) {
        self.recruitable.lock().unwrap().push(recruitable);
    }
}

/// Records CommandButtonHuntUpdate::setCommandButton calls without running the full hunt update.
struct RecordingHuntModule {
    recorded: Arc<Mutex<Vec<String>>>,
    data: Arc<crate::object::update::command_button_hunt_update::CommandButtonHuntUpdateModuleData>,
}

impl game_engine::common::system::Snapshotable for RecordingHuntModule {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl game_engine::common::thing::module::Module for RecordingHuntModule {
    fn get_module_data(&self) -> &dyn game_engine::common::thing::module::ModuleData {
        self.data.as_ref()
    }

    fn get_command_button_hunt_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::CommandButtonHuntControlInterface>
    {
        Some(self)
    }
}

impl game_engine::common::thing::module::CommandButtonHuntControlInterface for RecordingHuntModule {
    fn set_command_button(&mut self, button_name: String) {
        self.recorded.lock().unwrap().push(button_name);
    }
}

#[derive(Debug)]
struct RecordingMoveAi {
    commands: Arc<Mutex<Vec<(AiCommandType, Coord3D, CommandSourceType)>>>,
    locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
    cleared: Arc<Mutex<u32>>,
    attitudes: Arc<Mutex<Vec<crate::modules::AIAttitudeType>>>,
}

impl AIUpdateInterface for RecordingMoveAi {
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

    fn choose_locomotor_set(
        &mut self,
        set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.locomotors.lock().unwrap().push(set);
        Ok(())
    }

    fn execute_command(
        &mut self,
        command: &AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commands
            .lock()
            .unwrap()
            .push((command.cmd, command.pos, command.cmd_source));
        Ok(())
    }

    fn clear_waypoint_queue(&mut self) {
        *self.cleared.lock().unwrap() += 1;
    }

    fn set_attitude(
        &mut self,
        attitude: crate::modules::AIAttitudeType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.attitudes.lock().unwrap().push(attitude);
        Ok(())
    }
}

fn install_recording_named_unit(
    unit_id: ObjectID,
    name: &str,
    group_id: Option<u32>,
) -> (
    Arc<Mutex<Vec<(AiCommandType, Coord3D, CommandSourceType)>>>,
    Arc<Mutex<Vec<LocomotorSetType>>>,
    Arc<Mutex<u32>>,
    Arc<Mutex<Vec<crate::modules::AIAttitudeType>>>,
) {
    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let cleared = Arc::new(Mutex::new(0u32));
    let attitudes = Arc::new(Mutex::new(Vec::new()));
    let unit = crate::object_manager::GameObjectInstance::new(
        unit_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("named unit");
    {
        let base_arc = unit.base();
        let mut base = base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingMoveAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
            cleared: Arc::clone(&cleared),
            attitudes: Arc::clone(&attitudes),
        }))));
        if let Some(gid) = group_id {
            base.enter_group(&crate::ai::AIGroup::new(gid));
        }
    }
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(unit, Coord3D::new(4.0, 5.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object(name.to_string(), unit_id)
        .unwrap();
    (commands, locomotors, cleared, attitudes)
}

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
