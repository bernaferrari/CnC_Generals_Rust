# Main approach-braking consumer correction

## Scope and executable boundary

This patch corrects `Object::apply_cpp_approach_brake`, its shared scalar `calc_slow_down_dist`, and the existing Main movement-pass call guard. The tests call the existing public Object method. They do not construct, admit to, or tick a world. Ordinary Object construction and the existing loader → resolver → binding application are used without scalar repairs. Pre-call appearance flags, factor and initialized timer are explicit phase inputs.

The corrected consumer executes the original approach formulas on frame values reconstructed from its public host values. This is exact comparison to the stated host adapter, not recovery of all original authored frame states. Original and reconstructed values both remain in the fixture; their disagreement is not a test tolerance or hidden adjustment.

The Main path is the default production movement authority. `GameWorldAuthority::DEFAULT_OFF` has `movement: false`. GameWorld takes the integration role only when explicitly opted in, with shadow enabled and a coupled tick active. `GameLogic::update_movement` otherwise calls `update_movement_locomotor_pass`. The duplicate `Object::update_movement` has only identified test callers and was not changed.

## Corrected behavior

- Host speed `/30` and bound braking `/30/30` reconstruct the consumer's frame inputs, preserving the original arithmetic order
- Tread and wheel goal decrements remove one frame of braking (or half a frame); wheel slowdown adds one frame of time and travel
- NoSlowDown blocks new approach latches and generic min-speed choices, while inherited braking, local clearing and wheel timers continue
- Braking/min-speed floors and positive-path division guards are removed where absent from the original; negative/zero values and ordered comparisons retain their source meaning
- Tread/wheel local clearing preserves the original double `2.0` literal; only arithmetically decremented frame goals are converted back by `*30`. Copy branches preserve the actual host desired, actual or minimum scalar
- Wheel refresh uses float frame conversion, float addition of `2.5f*30`, then unsigned conversion in the original-defined range
- The existing caller gates the appearance helper with its existing `allow_2d_motive` decision. Earlier far-clear → path raise → wing clear ordering remains intact

## Oracle contract

`generate_approach_braking.py` pins original INI.cpp, GameCommon.h, BaseType.h, Locomotor.cpp, Locomotor.h, GameLogic.h and AIPathfind.h. It extracts the original numeric parsers, braking constructor assignments/getter, type declarations, cell constants, `sqr`, shared slowdown helper and named tread/wheel/legs/climber/other/wing-clear spans. The exact extracted approach blocks are emitted separately. Branch tags are appended after executed original goal assignments; no tag is inferred by comparing resulting numbers with inputs.

This is a native host arithmetic oracle with minimal inert state/token/clock context. It is not an original Windows game execution. Compiler flags are `-std=c++17 -O0 -ffp-contract=off -fno-fast-math`; static assertions pin binary32 Real, 32-bit UnsignedInt and the native math.h float overload of `fabs`. Windows compiler/FP-environment behavior is not established. Inputs normally remain finite; the explicitly named double-local-clear phase uses positive infinity for path to distinguish double from float multiplication. NaN outputs created by IEEE division are checked by classification, all other scalar outputs by exact bits.

`raw_*` columns contain original frame-space inputs and results. `reconstructed_*` columns contain the exact `/30` and `/30/30` consumer inputs and original expressions evaluated on them. The final `host_goal` is the separately labeled adapter output. Choice tags are 0=untouched desired, 1=full decrement, 2=actual copy, 3=min copy, 4=half decrement. Other/Hover/Wings `slow` is observer-computed for diagnostic columns even when NoSlowDown prevented the original block from computing it; this extra diagnostic does not contribute to returned goal/state.

The fixture has 186 appearance rows and nine direct shared-helper rows. Forty-three appearance rows run through both real Common loaders and Main resolution/application, giving 229 public approach calls plus nine shared-helper calls per complete target. Families separately check full/half/hold, positive new latches, NoSlowDown ordering, inherited-state preservation, signed zero/negative/tiny inputs, zero and direct negative paths, strict cell/slow/half/clear/factor boundaries, defined float timer refresh, equality and no-refresh wrap comparisons. Negative path and negative Wings minimum are direct phase inputs; no whole-move/template reachability is claimed for them.

## Visible representation limits

For authored Braking=0.045, the parsed original frame value is `3851b71a`; the accepted host binding produces `3d3851ee`, and reconstructing it yields `3851b71b`. At the recorded strict boundary, original slowdown `47ea5ffd` becomes reconstructed `47ea5ffc`. Original full-decrement goal `403fff2e` becomes reconstructed half-decrement goal `403fff97`, and factor `3f800002` becomes `3f800000`. Both outputs stay in the fixture and explicit tests require that disagreement to remain visible.

The omitted default BIGNUM cap also loses information: original frame braking `47c34f80` → host `4caba8e0` → reconstructed `47c34f81`. Minimum-speed parsing uses the original velocity callback for raw frame columns while the existing Common/Main binding retains authored host scalars. These differences are not repaired by overwriting bound fields. Exact original-state recovery requires a separate representation decision.

## Timer and caller limits

The retained UINT_MAX first-approach sentinel differs from original construction/startMove/maintain-time initialization and collides with a legitimate UINT_MAX deadline. This lifecycle issue is excluded from the numeric comparisons. All timer oracle rows begin with a nonsentinel initialized timer. No-refresh comparisons at UINT_MAX-1, UINT_MAX and zero execute only unsigned comparison, not an out-of-range refresh.

At frame 16777216, original float refresh yields 16777292 rather than exact integer addition's 16777291. Defined-range arithmetic is tested near this boundary and near the upper unsigned range. Every native refresh is range-checked before the original float→unsigned cast. Undefined C++ out-of-range conversions are never executed. Rust's saturating cast outside that range is a host fallback, not a parity claim.

The caller guard is source-reviewed and native-compiled, not executable caller evidence. It retains the fresh terrain height, world-resolved carrier deck drop and ambient-gravity-derived caller decision. It does not move that gate into an Object cached-height predicate or change its earlier sample placement. No test-only driver/API was added and no world tick was invoked.

Force integration, pre-appearance turn/reverse/terrain decisions, the entry far-clear positive-braking guard, path raising, object-status versus locomotor-flag distinction, thrust's own movement algorithm, timer lifecycle and full save/load movement behavior remain outside this cut. Removing the shared helper's unsupported floor affects its other consumers numerically, including thrust and the duplicate Object mover; their integration remains unverified.

## Validation

The frozen integration target, fixture and generator were all finalized before changing production. The independently retained baseline executable at commit `f63f13d815b6686061987bd57816f53031236836` passes the explicit representation-loss test and fails all ten behavior groups (1/11). The unchanged target and fixture pass all 11 groups after the correction. Each group aggregates output/state differences so an early case does not hide later controls. Both loader paths match their exact pre-call scalar/binding columns even on the baseline.

The corrected ordinary Main test graph compiled in 4m12s. Existing braking-representation tests pass 10/10. Rustfmt on the four changed Rust files, whitespace, LOC and unsafe ratchets pass with zero violations. Existing compiler warnings remain. XP save/load is unaffected and was not rerun; its prior checkpoint results do not count as new evidence. There was no separate Main metadata, full Main cfg(test), release, Clippy, Miri, alternate graph, world tick or unrelated subsystem test.

Reproduce from `GeneralsRust`, after loading the task environment:

```sh
python3 Code/Main/tests/oracles/generate_approach_braking.py --output /tmp/approach-oracle --verify Code/Main/tests/fixtures/approach_original.txt --self-test
cargo test --offline --locked -j1 -p generals_main --test approach_braking --no-run
RUST_MIN_STACK=16777216 /path/to/retained/approach-fixed --test-threads=1
RUST_MIN_STACK=16777216 cargo test --offline --locked -j1 -p generals_main --test braking_representation -- --test-threads=1
rustfmt --edition 2024 --check --config skip_children=true Code/Main/src/game_logic/object/physics_motion.rs Code/Main/src/game_logic/object/mod.rs Code/Main/src/game_logic/world_tick/movement_support.rs Code/Main/tests/approach_braking.rs
python3 Code/Main/scripts/check_rust_loc.py --json
python3 Code/Main/scripts/check_unsafe_contracts.py --json
git diff --check
```

To reproduce the baseline without altering production, apply only the frozen regression source/fixture/generator to the preceding accepted commit and build the same target. The original executable and native oracle programs are retained locally with recorded hashes; delivery packages contain source and logs, not binaries.
