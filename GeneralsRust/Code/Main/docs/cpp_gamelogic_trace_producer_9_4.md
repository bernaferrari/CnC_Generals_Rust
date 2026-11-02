# C++ GameLogic trace producer for hq-x1j4u.9.4

9.4 needs one versioned retail skirmish scenario produced by the **original
C++ GameLogic update loop** and by this Rust host, then compared with
`generals.frame_trace.v2`. Fixture-only or Rust-vs-Rust dumps are not a C++
differential.

## What already exists

| Piece | Role | Limit |
| --- | --- | --- |
| `GeneralsMD/Code/ParityHarness/` | Portable C++ producer over `parity_scenarios/smoke_attack.v1.json` | Compiles original `Common/crc.cpp` + `Common/RandomValue.cpp` via `original_random_adapter.cpp`. Does **not** call `GameLogic::update()`. Object/player/command records are fixture-only. |
| Rust `deterministic_fixture_trace` | Matching fixture producer | Same schema; also not a full game loop. |
| `deterministic_trace_compare` / `run_cpp_rust_differential.py` | Frame CRC + RNG-state compare | Grades component-only (`rng_seed`, CRC framing). `full_game_loop` stays false until a real C++ tick producer lands. |

## Smallest C++ adapter that would unblock 9.4

Keep the existing `generals.trace.scenario.v1` → `generals.frame_trace.v2`
envelope. Replace ParityHarness’s fixture state source with a thin Windows
adapter that:

1. Boots the retail GeneralsMD / ZH Windows binary path far enough to own a
   live `GameLogic` (same asset roots the Rust host uses for Lone Eagle).
2. Loads one versioned skirmish scenario (map path, factions, starting cash,
   seeded RNG, optional queued commands).
3. For each logic frame: call `GameLogic::update()`, then emit one
   `generals.frame_trace.v2` frame with live objects, players, commands, and
   the original RNG six-word state (already exposed by
   `original_random_adapter.cpp`).
4. Stops at a declared frame count or end condition; does not require the
   full WW3D present path.

Do **not** invent a second oracle format. Do **not** label a Mac-only
Rust↔Rust or fixture↔fixture pass as the C++ differential.

## Files to touch (C++ side)

- `GeneralsMD/Code/ParityHarness/main.cpp` — swap fixture fill for live-tick
  emission; keep CLI `SCENARIO=` / `generalsmd_frame_trace`.
- `GeneralsMD/Code/ParityHarness/original_random_adapter.cpp` — retain as the
  RNG-state readback; extend only if live `GameLogic` needs an extra hook.
- New small adapter unit (suggested name
  `gamelogic_update_adapter.cpp` / `.h`) that:
  - constructs / borrows the Windows `GameLogic` singleton-or-session the
    retail EXE already uses,
  - loads the scenario map,
  - runs `GameLogic::update()` N times,
  - serializes object/player/command snapshots into the existing v2 writer.
- Link against the Windows GameLogic / GameEngine libs the retail build
  already produces (not the portable harness subset).

## Windows dependencies the portable Mac harness cannot satisfy

The ParityHarness README is explicit: original engine Windows/device
dependencies are unavailable in the portable build. A live tick producer
needs at least:

- Win32 process / CRT host for the GeneralsMD EXE or a console driver that
  links the same GameLogic static/import libs.
- Retail data trees (`Data/*.big`, MapsZH / Lone Eagle) resolved the same way
  as the shipping game.
- Whatever subsystem init `GameLogic::init` / map load currently pulls in on
  Windows (file system, memory pools, message stream, player template INI).
  Audio/GPU present can stay stubbed if map load + `update()` do not require
  them; if they do, stub at the device boundary rather than rewriting
  GameLogic.

## Rust side (unchanged contract)

- Keep consuming `generals.frame_trace.v2` from both producers.
- Report the first divergent frame / state / event / RNG draw / Xfer byte.
- `port_dashboard.py` may flip `full_game_loop` only after the C++ producer
  above exists and the compare is green on a versioned retail scenario.

Until that Windows adapter lands, hq-x1j4u.9.4 stays **BLOCKED**. No fake
Rust-vs-Rust gate.
