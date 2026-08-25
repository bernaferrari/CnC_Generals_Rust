//! Shared fixture trace for the portable C++/Rust component differential.
//!
//! This producer intentionally does not claim to run either full game loop.
//! Object, player, and command records come from the shared fixture; the RNG
//! state is produced by Rust's port of `RandomValue.cpp` and is the component
//! under differential test.

use anyhow::{Context, Result, bail};
use generals_main::deterministic_trace::{
    FrameTrace, TraceCommand, TraceObject, TracePlayer, calculate_frame_crc_with_diagnostics,
};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf};

#[derive(Debug, Deserialize)]
struct FixtureScenario {
    schema: String,
    scenario: String,
    final_frame: u32,
    rng_base_seed: u32,
    rng_draws_per_frame: u32,
    objects: Vec<TraceObject>,
    players: Vec<TracePlayer>,
    commands: Vec<ScheduledCommand>,
}

#[derive(Debug, Clone, Deserialize)]
struct ScheduledCommand {
    frame: u32,
    player_id: u32,
    command_id: u32,
    command: String,
    selected_units: Vec<generals_main::game_logic::ObjectId>,
}

#[derive(Debug, Serialize)]
struct Authority {
    rng: &'static str,
    objects: &'static str,
    players: &'static str,
    commands: &'static str,
}

#[derive(Debug, Serialize)]
struct TraceDump {
    schema: &'static str,
    scenario: String,
    final_frame: u32,
    producer: &'static str,
    authority: Authority,
    frames: Vec<FrameTrace>,
}

fn main() -> Result<()> {
    let scenario_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: deterministic_fixture_trace <scenario.json>")?;
    if env::args_os().nth(2).is_some() {
        bail!("usage: deterministic_fixture_trace <scenario.json>");
    }

    let source = fs::read_to_string(&scenario_path)
        .with_context(|| format!("reading {}", scenario_path.display()))?;
    let scenario: FixtureScenario = serde_json::from_str(&source)
        .with_context(|| format!("parsing {}", scenario_path.display()))?;
    if scenario.schema != "generals.trace.scenario.v1" {
        bail!("unsupported scenario schema '{}'", scenario.schema);
    }

    let dump = produce_trace(scenario);
    println!("{}", serde_json::to_string_pretty(&dump)?);
    Ok(())
}

fn produce_trace(mut scenario: FixtureScenario) -> TraceDump {
    use game_engine::common::random_value::{
        get_game_logic_random_seed_state, get_game_logic_random_value, init_game_logic_random,
    };

    scenario.objects.sort_by_key(|object| object.id);
    scenario.players.sort_by_key(|player| player.id);
    init_game_logic_random(scenario.rng_base_seed);

    let mut frames = Vec::with_capacity(scenario.final_frame as usize);
    for frame in 1..=scenario.final_frame {
        for _ in 0..scenario.rng_draws_per_frame {
            let _ = get_game_logic_random_value(0, i32::MAX);
        }
        let rng_seed = get_game_logic_random_seed_state();
        let mut commands: Vec<TraceCommand> = scenario
            .commands
            .iter()
            .filter(|command| command.frame == frame)
            .map(|command| TraceCommand {
                player_id: command.player_id,
                command_id: command.command_id,
                command: command.command.clone(),
                selected_units: command.selected_units.clone(),
            })
            .collect();
        commands.sort_by_key(|command| (command.command_id, command.player_id));

        let crc = calculate_frame_crc_with_diagnostics(
            frame,
            &rng_seed,
            &commands,
            &scenario.objects,
            &scenario.players,
            &[],
            &[],
            None,
        );
        frames.push(FrameTrace {
            frame,
            rng_seed,
            commands,
            objects: scenario.objects.clone(),
            players: scenario.players.clone(),
            events: Vec::new(),
            xfer_bytes: Vec::new(),
            victory_state: None,
            crc,
        });
    }

    TraceDump {
        schema: "generals.frame_trace.v2",
        scenario: scenario.scenario,
        final_frame: scenario.final_frame,
        producer: "generalsrust-randomvalue-port",
        authority: Authority {
            rng: "GeneralsRust/Code/GameEngine/Common/src/common/random_value.rs",
            objects: "fixture-only (portable component differential)",
            players: "fixture-only (portable component differential)",
            commands: "fixture-only (portable component differential)",
        },
        frames,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn randomvalue_port_matches_original_cpp_first_draw_state() {
        let scenario = FixtureScenario {
            schema: "generals.trace.scenario.v1".into(),
            scenario: "unit".into(),
            final_frame: 1,
            rng_base_seed: 0x1234_5678,
            rng_draws_per_frame: 1,
            objects: Vec::new(),
            players: Vec::new(),
            commands: Vec::new(),
        };
        let dump = produce_trace(scenario);

        assert_eq!(
            dump.frames[0].rng_seed,
            [
                3_268_696_253,
                3_195_204_591,
                604_862_093,
                1_270_104_806,
                847_062_993,
                2_182_320_605,
            ]
        );
    }
}
