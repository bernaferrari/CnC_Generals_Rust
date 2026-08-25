# GeneralsMD C++ frame-trace producer

This is a deliberately narrow portability harness for the deterministic frame
trace work. It compiles the original `Common/crc.cpp` and
`Common/RandomValue.cpp` implementation units through
`original_random_adapter.cpp`; the adapter only exposes the original
GameLogic RNG state. It does not reimplement or claim to reproduce the full
Windows game loop.

The shared input is
`parity_scenarios/smoke_attack.v1.json`. Its schema is
`generals.trace.scenario.v1`, and its output uses the canonical
`generals.frame_trace.v2` frame fields. The producer explicitly labels object,
player, and command records as fixture-only because the original engine's
Windows/device dependencies are not available in this portable build.

Build and run from this directory:

```sh
make test
make SCENARIO=/absolute/path/to/scenario.v1.json
./bin/generalsmd_frame_trace /absolute/path/to/scenario.v1.json
```

The canonical frame CRC uses the same little-endian IEEE CRC-32 framing as the
Rust trace schema. It is a comparison envelope, not the engine's separate
`XferCRC` implementation. `rng_seed` in each emitted frame is the exact six
word state from the original `RandomValue.cpp` after the configured number of
logic draws. The fixture's `rng_base_seed` is passed to the original
`InitGameLogicRandom`; the declared `rng_seed` array documents the scenario
input but is not used as a replacement PRNG state.

This harness is therefore C++-authoritative for the RNG component only. A
future Windows build can replace the fixture-only state source with a direct
`GameLogic::update()` adapter without changing the canonical schema.

CI also runs the Rust `deterministic_fixture_trace` producer over this same
fixture and compares every canonical frame with `deterministic_trace_compare`.
That differential is authoritative for RNG state and CRC framing only; the
fixture-only fields are comparison context, not gameplay-parity evidence.
