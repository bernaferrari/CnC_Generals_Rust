# Shared authored FX rules

`generals_fx` owns the nugget schema, pure nugget parsing, authored random ranges,
and an owned exact-name catalog. It has no engine, rendering, audio, RNG, locking,
or unsafe dependency. Names and catalog keys can use the engine's `AsciiString` or plain `String`.
Gameplay bindings retain the immutable authored name buffer rather than copying
a new string or adding an Arc around each named reference.

C++ authorities are `GameClient/FXList.cpp` (constructors, field tables and ordered
nuggets), `Common/INI/INI.cpp` (units, colors and range grammar), and
`Common/RandomValue.cpp` (distribution names). Field defaults include
`GenericTracer`, normalized white, particle delay -1 and bone orientation true.

Common supplies the INI unit-conversion adapter and canonical parsed definitions.
Headless loading and GameClient loading call the same nugget parser. GameClient
compiles those definitions into its existing renderer/audio implementations.
Effects stay in authored order, including repetitions. Loading an override
replaces an earlier list. Parsing preserves ranges without drawing random values;
the client runtime samples them when executing the original effect operations.

An owned `FxCatalog` does not publish state. Common's startup catalog and the
client's derived execution cache still have global adapters. Runtime object,
particle, audio and view services also retain their existing lifetime boundaries.
Moving those services into explicit instance execution contexts is tracked in
Bead hq-kheus; extracting this crate does not establish complete FX instance
isolation. Unsupported non-uniform particle distributions retain C++ release's
zero-result behavior in the existing client adapter.

Verification commands (from GeneralsRust):

```sh
cargo test --locked -p generals_fx
cargo test --locked -p game_engine --test fx_list_ini -- --test-threads=1
cargo test --locked -p game-client-rust --features internal --lib fx_list::tests -- --test-threads=1
cargo test --locked -p game-client-rust --test fx_client_rng_determinism -- --test-threads=1
```
