use anyhow::{bail, Context, Result};
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::deterministic_trace::{run_trace_scenario, FrameTrace, TraceScenario};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, Weapon};
use glam::Vec3;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

const DEFAULT_FINAL_FRAME: u32 = 6;
const SMOKE_SEED: [u32; 6] = [
    0x12345678, 0x9abcdef0, 0x13579bdf, 0x2468ace0, 0xfedcba98, 0x76543210,
];

#[derive(Debug)]
struct Args {
    scenario: String,
    final_frame: u32,
    output: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct TraceDump {
    schema: &'static str,
    scenario: String,
    final_frame: u32,
    frames: Vec<FrameTrace>,
}

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1))?;
    let dump = run_named_scenario(&args.scenario, args.final_frame)?;
    let json = serde_json::to_string_pretty(&dump)?;

    if let Some(output) = args.output {
        fs::write(&output, json).with_context(|| format!("writing {}", output.display()))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

fn run_named_scenario(scenario: &str, final_frame: u32) -> Result<TraceDump> {
    match scenario {
        "smoke_attack" => {
            let (mut game_logic, attacker, target) = smoke_attack_game_logic();
            let scenario = TraceScenario::new(SMOKE_SEED, final_frame).with_commands(
                1,
                vec![command(
                    1,
                    CommandType::AttackObject { target_id: target },
                    vec![attacker],
                )],
            );
            let frames = run_trace_scenario(&mut game_logic, &scenario);
            Ok(TraceDump {
                schema: "generalsrust.frame_trace.v1",
                scenario: "smoke_attack".to_string(),
                final_frame,
                frames,
            })
        }
        other => bail!("unknown scenario '{other}', expected 'smoke_attack'"),
    }
}

impl Args {
    fn parse<I>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut scenario = "smoke_attack".to_string();
        let mut final_frame = DEFAULT_FINAL_FRAME;
        let mut output = None;
        let mut iter = args.into_iter();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--scenario" => {
                    scenario = iter.next().context("--scenario requires a scenario name")?;
                }
                "--frames" => {
                    let value = iter.next().context("--frames requires a frame count")?;
                    final_frame = value
                        .parse()
                        .with_context(|| format!("invalid --frames value '{value}'"))?;
                }
                "--output" => {
                    output = Some(PathBuf::from(
                        iter.next().context("--output requires a file path")?,
                    ));
                }
                "--help" | "-h" => {
                    println!(
                        "Usage: deterministic_trace_dump [--scenario smoke_attack] [--frames N] [--output path]"
                    );
                    std::process::exit(0);
                }
                other => bail!("unknown argument '{other}'"),
            }
        }

        Ok(Self {
            scenario,
            final_frame,
            output,
        })
    }
}

fn smoke_attack_game_logic() -> (GameLogic, ObjectId, ObjectId) {
    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    game_logic.add_player(Player::new(1, Team::GLA, "GLA", false));
    game_logic.templates.insert(
        "TraceHumvee".to_string(),
        test_template("TraceHumvee", 360.0),
    );
    game_logic.templates.insert(
        "TraceTechnical".to_string(),
        test_template("TraceTechnical", 240.0),
    );

    let attacker = game_logic
        .create_object("TraceHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("trace attacker template is registered");
    let target = game_logic
        .create_object("TraceTechnical", Team::GLA, Vec3::new(35.0, 0.0, 0.0))
        .expect("trace target template is registered");

    let weapon = Some(Weapon {
        damage: 25.0,
        range: 100.0,
        reload_time: 0.0,
        projectile_speed: 0.0,
        ..Weapon::default()
    });
    game_logic
        .get_objects_mut()
        .get_mut(&attacker)
        .expect("trace attacker was just inserted")
        .weapon = weapon;

    (game_logic, attacker, target)
}

fn test_template(name: &str, max_health: f32) -> ThingTemplate {
    let mut template = ThingTemplate::new(name);
    template
        .set_health(max_health)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Vehicle);
    template
}

fn command(
    command_id: u32,
    command_type: CommandType,
    selected_units: Vec<ObjectId>,
) -> GameCommand {
    GameCommand {
        command_type,
        player_id: 0,
        command_id,
        timestamp: UNIX_EPOCH + Duration::from_secs(command_id as u64),
        selected_units,
        modifier_keys: ModifierKeys::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_attack_dump_is_stable_json() {
        let dump = run_named_scenario("smoke_attack", 3).expect("scenario should run");
        let json = serde_json::to_string(&dump).expect("dump should serialize");

        assert!(json.contains("\"schema\":\"generalsrust.frame_trace.v1\""));
        assert_eq!(dump.frames.len(), 3);
        assert_eq!(dump.frames[0].commands[0].command_id, 1);
        assert_ne!(dump.frames[0].crc, dump.frames[2].crc);
    }

    #[test]
    fn unknown_scenario_is_rejected() {
        let err = run_named_scenario("missing", 1).expect_err("unknown scenario should fail");
        assert!(err.to_string().contains("unknown scenario"));
    }
}
