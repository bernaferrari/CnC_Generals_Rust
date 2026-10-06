# Braking defaults, cap and unit boundaries

This is a bounded representation/getter/binding correction, not a movement
simulation or complete Locomotor port. The original authority is:

- `Locomotor.cpp:41,270,430`: BIGNUM is 99999; omitted template braking is
  already in distance/frame²; authored braking uses `parseAccelerationReal`
- `Locomotor.cpp:636,820–828`: the runtime cap independently starts at BIGNUM,
  and `getBraking` uses an ordered greater-than comparison
- `Locomotor.cpp:668,691,722–749`: copy and assignment retain the independent
  cap; Xfer versions 1 and 2 serialize it, with v2 adding the donut timer
- `Locomotor.h:286`: the original permits a separately changed cap
- `INI.cpp:1577–1582,1718–1723` and `GameCommon.h:37–69`: `Real` scanning and
  original per-second-squared to per-frame-squared conversion arithmetic

`generate_braking.py` pins the full original files using SHA-256, extracts the
original scan, callback and getter bodies, and compiles them against the original
conversion header and minimal original base typedefs. Constructor stubs use only
the exact original braking assignments. This host C++17 oracle does not build the
original Windows game. Fast math and contraction are disabled. The host column
is explicitly adapter arithmetic: effective frame² braking multiplied by 30,
then 30 again, to match Main's existing sec² representation.

The fixture has 14 omitted/authored cases and eight IEEE comparison controls.
Finite authored cases include positive/negative zero, underflow, ordinary values,
values straddling the cap and very large positive/negative values. NaN and infinity
comparison controls are direct getter inputs, not claims about INI grammar.

## Production changes

Common now retains the original omitted 99999 frame² value, so the GameLogic
bridge can copy braking directly without confusing authored zero with omission.
GameLogic creates an independent 99999 frame² cap and preserves the original
ordered comparison, including template NaN and signed zero.

Main applies that initial cap before converting effective braking to sec².
Its two Object constructors, missing-field serde default and omitted-braking
upgrade fallback use the same sec² default. The original nominal 99999 × 900
is stored as binary32 bits `4caba8e0`; exact rounding is part of the fixture.
Authored zero and negative values are no longer replaced with the default.

Explicit serialized host scalars retain their literal values. The value 99999
cannot identify whether an old save meant the previous default or an explicit
host value, so there is no magnitude-based migration. No serialization layout
or version changes.

## Executable evidence

From `GeneralsRust`, with the task environment active:

```sh
python3 Code/Main/tests/oracles/generate_braking.py --output /tmp/braking-oracle
cmp /tmp/braking-oracle/braking_original.txt Code/Main/tests/fixtures/braking_original.txt
cargo test --offline --locked -j1 -p generals_main --test braking_representation -- --test-threads=1
cargo test --offline --locked -j1 -p generals_main --test experience_save_load -- --test-threads=1
cargo test --offline --locked -j1 -p game_engine --test locomotor_numeric_parity -- --test-threads=1
cargo check --offline --locked -j1 -p game_engine --lib
```

The ten-test braking target exercises both public Common loaders with distinct
fresh definitions, the real GameLogic bridge/constructor/getter and Main public
resolver/application path. Each of five matrix tests checks 28 cases against
frozen original-derived bits. Other tests cover runtime-cap clone and v1/v2 Xfer,
Object clone, both constructors, missing-field serde and six literal historical
serde values. Special cap inputs are authored Xfer snapshots using the original
field order; they enter through the existing public load API. No private fields
are repaired after loading. Unchanged constructor wander draws are incidental;
no seeding, RNG assertions, world publication or tick is used by this target.

At base `45d325041c64d90ade343f778f3827e768b49e07`, the final regression source
runs ten tests: one pass and nine failures. The explicit saved-scalar serde
control passes; every intended correction has a failing public-boundary test.
The baseline binary and exact test source are copied and independently hashed
before production edits. The fixed run passes 10/10 with the identical regression
and fixture bytes: all 140 matrix comparisons and eight ordered-cap controls
pass. Clone snapshot equality additionally preserves the existing Rust clone
contract; it does not establish parity for unrelated C++ clone residuals. An
earlier draft compiler type error is recorded separately and is not behavioral
evidence.

Fresh related checks pass: `experience_save_load` 6/6 on the fixed Main library,
`locomotor_numeric_parity` 16/16 (including all 3,186 authored bit comparisons),
and the existing default-dev Common library check. Bounded rustfmt, whitespace,
LOC and unsafe-ratchet checks pass with no allowance changes. Source-hash and
fixture negative controls reject a changed original BIGNUM and a zero omitted
fixture value, respectively.

## Limits

Main approach-time subtraction, force clamps and zero/negative movement handling
have independent preexisting defects. They are deliberately outside this cut;
passing scalar readouts is not proof of movement parity. Other Locomotor defaults,
INI override behavior, token grammar and RNG behavior remain outside the claim.
The separate helicopter MaxBraking parser/application and missing runtime setter
are also outside this default-cap correction.

The actual Main OXOB save path is source-traced only here: it captures host
`Object.braking`, rebinds locomotor data on restore, then restores the explicit
saved scalar. Its field is effective host braking, not a raw C++ runtime cap.
Direct Object serde controls do not claim to execute OXOB. The apparent
`entity_lifecycle_motion_state.rs` locomotor residual is disconnected from the
live module inventory, so the public lifecycle envelope is not credited with
braking persistence. The existing full-Main unit default assertion is corrected
for units but that broad cfg(test) graph is not run by the integration target.
