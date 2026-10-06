# Locomotor conversion arithmetic

This records the earlier Common arithmetic slice. The later bounded braking
default/cap/unit correction and its public Main/GameLogic checks are documented
in `Code/Main/tests/oracles/braking_representation.md`; descriptions below of
the former zero fallback and unchanged braking defaults are historical.

This slice changes only the nine fields that Common already stores in simulation
units. The original authority is `Locomotor.cpp:417–430`, the `Real` members in
`Locomotor.h`, `INI.cpp:574–580,1709–1723`, and `GameCommon.h:37–75`.

| Authored fields | Original callback | Stored units |
| --- | --- | --- |
| Speed, SpeedDamaged | INI::parseVelocityReal | distance/frame |
| TurnRate, TurnRateDamaged | INI::parseAngularVelocityReal | radians/frame |
| Acceleration, AccelerationDamaged, Lift, LiftDamaged, Braking | INI::parseAccelerationReal | distance/frame² |

`generate_locomotor_numeric.py` pins all five original files, verifies each field's
callback and `Real` member, then invokes `generate_ini_numeric.py`. That extractor
executes the original INI callback bodies, original `scanReal`, and complete
original `GameCommon.h`, with the original BaseType typedefs and PI macro. `Real`
is `float` (`BaseType.h:106`) and PI is `3.14159265359f` (`BaseType.h:68`).

The extractor's 177-token output must match the already frozen shared fixture
byte for byte. The additional field manifest maps those original outputs to all
nine locomotor fields (1,593 results). No expected bits are derived from the Rust
implementation. Compiler flags disable fast math and floating-point contraction.
This is a host C++17 callback oracle, not execution of the original Windows game,
its complete loader, or retail assets. Complete finite decimal inputs bound the
token grammar; the single-token supplier is a harness stub.

From `GeneralsRust`:

```sh
python3 Code/GameEngine/Common/tests/oracles/generate_locomotor_numeric.py --output /tmp/locomotor-numeric-oracle
cargo test --offline --locked -j 1 -p game_engine --test locomotor_numeric_parity -- --test-threads=1
cargo check --offline --locked -j 1 -p game_engine --lib
```

Both public paths (`INI::parse_current_file` through `parse_locomotor_block`, and
`load_locomotors_from_str`) invoke `parse_locomotor_template_definition`. The
integration target executes all 177 inputs against each changed field through
both paths: 3,186 bitwise comparisons. Inputs include signed zero, subnormals,
adjacent exponent boundaries, ordinary authored values, and large finite values
that expose the old angular conversion's intermediate overflow. Equal healthy
and damaged inputs allow negative cases to retain the existing validation rule;
an additional distinct-value case catches incorrect field routing.

Further tests retain omitted defaults, damaged fallback, explicit positive and
negative zero, negative-underflow-to-zero behavior, raw fields, duplicate fields,
replacement templates, and existing invalid-token handling. Six independent
arithmetic mutants check that the frozen corpus detects division and incorrect
factor ordering. These tests do not claim that existing default representation
or token grammar agrees with C++ in every respect.

## Production reachability and unit boundaries

GameLogic's `locomotor/ini_bridge.rs::from_common_ini_template` copies the nine
already-converted Common values, with its existing zero-Braking fallback. Main's
`game_logic/locomotor_bootstrap.rs::host_stats_from_template` and
`host_locomotor_binding_from_template` restore those simulation units to host
per-second units by multiplication by 30 (or 30 twice). They therefore consume
the changed bits; those branches are unmodified and are statically traced here.

Common still stores MinSpeed, MinTurnSpeed, SpeedLimitZ, Extra2DFriction,
MaxThrustAngle, AccelerationPitchLimit, DecelerationPitchLimit, BounceAmount,
FrontWheelTurnAngle and SlideIntoPlaceTime as raw authored numbers. GameLogic's
bridge converts them, while Main also consumes several raw values directly.
Changing these Common representations would double-convert downstream values.
The raw-field test explicitly preserves this existing contract, including real
fields whose names sound angular but whose original callback is `parseReal`.

The patch leaves omitted/default/explicit-zero distinctions, braking caps, force
clamps, algorithms, serialization, callbacks and RNG behavior untouched. Bridge
angular-velocity/duration arithmetic, Main raw-field arithmetic, and the known
default/zero representation gaps remain separate obligations. No Main or
GameLogic runtime parity is claimed by the Common-only executable tests.

## Frozen before/fixed witness

At source base `0648815395be377317276a50bc1d3eaa68ebff16`, the exact public test
source reports 5 passes and 11 failures. The nine field tests report 2,326 bit
mismatches out of 3,186 comparisons; the distinct-field and replacement tests
also fail. The five unchanged-contract/mutation controls pass. The baseline
binary and test source are independently copied and hashed before rebuilding;
the fixed run uses the same test and original fixture bytes.
The fixed target passes 16/16 with all 3,186 comparisons exact. The existing
`ini_numeric_parity` and `weapon_numeric_parity` targets pass 10/10 and 1/1, and
the default-dev Common library check passes. Relevant format, LOC, unsafe and
whitespace gates pass with no baseline allowance changes.

Existing internal locomotor unit tests are not activated by this target. Their
loose conversion tolerances are not used as evidence. No retail assets, broad
internal feature graph, RNG fixtures, or paused workspace are used.
