# Generals particle rules and kernels

Renderer-independent particle primitives and explicit-input computation extracted
from the production GameClient implementation. The only dependency is `glam`.
This crate contains no GPU/platform types, global state, thread-local state,
locks, reference-counted state, engine lookups, or unsafe code.

## Behavioral authority

- `GeneralsMD/Code/GameEngine/Include/GameClient/ParticleSys.h`: enum ordinals,
  eight keyframes, volume depth constants, emission configurations.
- `GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp`:
  `Particle::update` translation order, `doWindMotion` positional force,
  `computePointOnUnitSphere` and hemispherical emission draw order.
- `GeneralsMD/Code/GameEngine/Source/Common/RandomValue.cpp`: empty ranges
  consume no random draws; unsupported client distributions return zero in release.

The production GameClient reexports these definitions and delegates translation,
wind, and unit-direction sampling here. The caller owns particle state, emitter
position, and RNG. Kernel calls do not choose frame timing or manage entities.
Cube normalization deliberately preserves the original direction distribution.

The current adapter represents its supported uniform range with numeric code 0;
this is its existing representation, not a claim that it serializes the C++
CONSTANT/UNIFORM enum verbatim. The extraction preserves that representation.

## Remaining integration boundaries

GameClient retains `ParticleSystemInfo`, templates, particle list ordering,
Snapshot/Xfer implementations, INI loading, the client-RNG adapter, attachments,
LOD and runtime manager. Those responsibilities have not been duplicated here.
The global manager and RNG remain migration dependencies. Main's simplified host
particles and `ww3d-particles` GPU emitters remain distinct implementations; this
extraction does not establish their behavioral equivalence or complete rendering.
Existing runtime update-order/lifetime work is hq-kvtl7; instance/Xfer ownership is
hq-lvejf. Reproduced pre-existing production particle test failures are hq-l8scp.

## Verification

From `GeneralsRust`:

```sh
cargo test --locked -p generals_particles
cargo test --locked -p game-client-rust --features internal --lib effects:: -- --test-threads=1
cargo test --locked -p game-client-rust --test fx_client_rng_determinism -- --test-threads=1
```

The core tests cover enum values, defaults, original random call order, explicit
source isolation, translation order, and wind distance cutoffs. Existing production
particle and FX tests cover the adapter and its serialization/update behavior.
Original-executable traces and complete retail particle rendering remain separate
requirements.
