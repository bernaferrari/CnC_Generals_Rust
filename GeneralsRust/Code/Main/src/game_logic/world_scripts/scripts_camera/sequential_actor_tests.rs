//! C++ ScriptEngine.cpp:7917–8010 checks live AI idle after each action.
//! This authors a core action chain on the actual Main runtime, not a decoded
//! retail map or a synthetic HostScriptQueryObject/AI callback witness.

use super::*;
use gamelogic::scripting::core::{
    Parameter, ParameterType, Script, ScriptAction, ScriptActionType,
};
use gamelogic::scripting::engine::{ScriptEngine, SequentialScript, get_script_engine};
use gamelogic::system::map_loader::{MapData, MapWaypoint};
use gamelogic::terrain::TerrainLogic;

struct TerrainRestore(Option<TerrainLogic>);

impl Drop for TerrainRestore {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            *gamelogic::terrain::get_terrain_logic().write().unwrap() = previous;
        }
    }
}

fn set_flag(name: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::SetFlag);
    action
        .add_parameter(Parameter::with_string(ParameterType::Flag, name.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    Box::new(action)
}

fn flag(name: &str) -> bool {
    gamelogic::scripting::engine::with_script_engine_ref(|engine| {
        engine.get_flag(name).is_some_and(|flag| flag.value)
    })
    .unwrap()
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn isolated(module: &str, test: &str, run: impl FnOnce()) {
    // Legacy script engine, tracker, and terrain are process-owned. Bound a
    // fresh exact-test child without clearing another test's registry/world.
    const CHILD: &str = "GENERALS_MAIN_SEQUENTIAL_ACTOR_CHILD";
    let exact = format!("{}::{test}", module.split_once("::").unwrap().1);
    if std::env::var(CHILD).ok().as_deref() != Some(exact.as_str()) {
        use std::io::Read;
        use std::process::Stdio;
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
            .env(CHILD, &exact)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pipes: [Box<dyn Read + Send>; 2] = [
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        let readers = pipes.map(|mut pipe| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).unwrap();
                bytes
            })
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break (status, false);
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                break (child.wait().unwrap(), true);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let output =
            readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
        assert!(!timed_out, "Main actor child timed out: {output:?}");
        assert!(
            status.success(),
            "Main actor child failed: {status}: {output:?}"
        );
        assert!(
            output[0].contains("1 passed; 0 failed"),
            "exact child ran no test: {output:?}"
        );
        return;
    }

    run();
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn normal_runtime_sequential_actor_move_pauses_and_idle_resumes_same_tick() {
    isolated(
        module_path!(),
        "normal_runtime_sequential_actor_move_pauses_and_idle_resumes_same_tick",
        || {
            let mut map = MapData::new();
            map.width = 64;
            map.height = 64;
            map.heightmap = vec![0; 64 * 64];
            map.boundaries = vec![gamelogic::common::ICoord2D::new(64, 64)];
            map.waypoints.push(MapWaypoint {
                id: 1,
                name: "SeqGoal".into(),
                location: gamelogic::common::Coord3D::new(60.0, 0.0, 0.0),
                path_label1: String::new(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            });
            let mut terrain = TerrainLogic::new();
            terrain.load_map_data(map);
            let _terrain = TerrainRestore(Some(std::mem::replace(
                &mut *gamelogic::terrain::get_terrain_logic().write().unwrap(),
                terrain,
            )));

            let mut world = GameLogic::new();
            world.add_player(Player::new(0, Team::USA, "SeqPlayer", true));
            let mut template = ThingTemplate::new("SequentialInfantry");
            template.add_kind_of(KindOf::Infantry).set_health(80.0);
            world
                .templates
                .insert("SequentialInfantry".into(), template);
            let actor = world
                .create_object_for_player("SequentialInfantry", 0, glam::Vec3::ZERO)
                .unwrap();
            world.host_object_mut(actor).unwrap().name = "SeqHero".into();
            assert_eq!(world.host_object(actor).unwrap().owner_player_id, Some(0));
            assert_eq!(world.host_object(actor).unwrap().ai_state, AIState::Idle);
            assert!(world.unit_can_move(actor));

            let mut movement = ScriptAction::new(ScriptActionType::MoveNamedUnitTo);
            // C++ doNamedMoveToWaypoint: Unit parameter 0, Waypoint parameter 1.
            movement
                .add_parameter(Parameter::with_string(
                    ParameterType::Unit,
                    "SeqHero".into(),
                ))
                .unwrap();
            movement
                .add_parameter(Parameter::with_string(
                    ParameterType::Waypoint,
                    "SeqGoal".into(),
                ))
                .unwrap();
            movement.next_action = Some(set_flag("SeqAfter"));
            let mut before = set_flag("SeqBefore");
            before.next_action = Some(Box::new(movement));
            let mut script = Script::new();
            script.script_name = "MainSequentialActor".into();
            script.action = Some(before);
            let mut sequence = SequentialScript::new();
            sequence.object_id = actor.0;
            sequence.script_to_execute_sequentially = Some(Box::new(script));

            // Same actual engine + handler pair installed by initialize_scripts_with_map_data.
            let mut engine = ScriptEngine::new().unwrap();
            engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(
                world.mission_scripts.clone(),
            ))));
            engine.append_sequential_script(sequence);
            *get_script_engine().write().unwrap() = Some(engine);
            world.scripts_loaded = true;

            world.evaluate_and_execute_scripts(0.0);
            assert!(flag("SeqBefore"));
            assert_eq!(
                world.host_object(actor).unwrap().ai_state,
                AIState::Moving,
                "idle SET_FLAG must advance into the actual MOVE in this script tick"
            );
            assert_eq!(
                world.host_object(actor).unwrap().path_goal_position,
                Some(glam::Vec3::new(60.0, 0.0, 0.0))
            );
            assert!(
                !flag("SeqAfter"),
                "MOVE must pause the chain using live post-action idle"
            );
            assert_eq!(
                get_script_engine()
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .snapshot_sequential_scripts()
                    .len(),
                1
            );

            world.evaluate_and_execute_scripts(0.0);
            assert!(!flag("SeqAfter"), "moving actor stays paused");
            assert!(world.unit_command_stop(actor));
            assert_eq!(world.host_object(actor).unwrap().ai_state, AIState::Idle);
            let frame = world.frame;
            world.evaluate_and_execute_scripts(0.0);
            assert_eq!(world.frame, frame);
            assert!(
                flag("SeqAfter"),
                "actual idle resumes SET_FLAG in the same tick"
            );
            assert_eq!(
                get_script_engine()
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .snapshot_sequential_scripts()
                    .len(),
                0,
                "idle final SET_FLAG must finish the sequence in the same tick"
            );
        },
    );
}
