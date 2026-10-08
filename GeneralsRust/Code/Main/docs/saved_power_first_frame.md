# Saved power before the first ordinary frame

## Observable failure and correction

A completed producer with authored energy +5 and a consumer with -8 settle to
production 5, consumption 8 and available power -3. At the accepted baseline
`4d95e76c5bf49fe1c16383d1a22a89649425e4d6`, a fresh public-file reader instead
started at 0/0/0. Its first ordinary `GameLogic::update` ran a scheduled
`PlayerHasPower` condition before the late economy pass. The condition incorrectly
set a one-shot flag to true. The flag stayed true after the counters converged,
while the unreloaded writer's flag remained false.

Successful snapshot restoration now rebuilds derived player power after every
object/status tail and before returning to an observer. It shares the ordinary
owner resolution and power fold. The ordinary tick keeps its coupled-aware
eligibility and its exact raw-assignment, expiry, mask and economy-log order.
The restore boundary reads the receiving object's transferred local health,
lifetime and construction fields, applies the existing sabotage mask, and does
not age timers, publish economy events, replay bonuses or advance simulation.
No save version, schema, new state mirror or RNG behavior changes.

## Original source contract

`GeneralsMD/Code/GameEngine/Source/Common/RTS/Energy.cpp:232–264` intentionally
omits production and consumption for current Energy Xfer versions because
building loading reconstructs them. The owner and sabotage deadline are saved.
`Player.cpp:1028–1038` skips objects under construction when adding membership;
`GameLogic/Object/Object.cpp:3770–3781` excludes disabled producers while retaining
consumers. Object team restoration follows the saved status transfer.
`Common/System/SaveGame/GameState.cpp:294–298,658–688` establishes player/object
loading and post-processing before control returns.

These are original-source comparisons, not execution of the C++ engine or proof
of C++ save-byte compatibility. The disabled, incomplete, damaged and sabotage
controls additionally preserve the inspected current Rust tick rules; they do
not establish every original power-system arithmetic rule.

## Receiving-object access scope

Existing `host_object_mut` is a runtime read-through view: it first overlays a
published shadow by numeric ObjectId. A diagnostic real-file load with an
unrelated same-ID shadow proved that this could replace saved HP 1000 with 0
and mark the receiving object destroyed. Updating only derived player counters
would conceal an already-corrupted object.

The correction uses the existing direct receiving map borrow at twelve
universal/default transfer sites across nine snapshot files: instance guards,
lifecycle envelopes, rods/transport-exit/turret/stealth-grant/weapon-leech resets,
OXOB reset/application, death-start and retained-death lifetime rows, and final
garrison mirrors. Ordinary mutable object access is unchanged.

One additional conditional per-object AI-order lookup is corrected in
`ai_team_persist::apply_payload`. The ordinary writer settling frame schedules
`next_mood_check_time = 3`, so both otherwise idle structures legitimately have
TMAI rows. Complete runtime overlay tracing identified exactly two remaining
mapped HP writes, both from that loop. Bounded inspection of the actual saved
TMAI v10 payload independently confirmed this sole non-default capture trigger.
The loop's field assignments and receiving path queue are unchanged.

This is thirteen selected access sites across ten snapshot files. Fifty-one
other mutable restore sites remain outside the migration, including optional
payload paths that can still import foreign fields. Explicit eligibility queries
in other restored behaviors and existing railroad, Common, script, terrain,
radar and camera dependencies remain. The correction does not establish general
load atomicity or whole-world restore isolation. Normal staged CnC loading ends
its coupled frame scope before restore and resets/synchronizes the shadow when
committing the restored world. Generic public restore callers must install an
external shadow themselves before coupling it to the receiving world. The old
dirty-ID path may retain IDs across pauses, but its contradictory coupling checks
do not publish them later; this existing dormant path is not repaired here.

## Real-file evidence

`tests/saved_power_first_frame.rs` uses fresh writer and reader processes, public
`SaveFileManager::save_game`/`load_game`, and ordinary updates. The unchanged final
fixture SHA-256 is
`749c002279ab8a74ffbf1fc632414286ad7ad751ba4acc81308a03436e656f10`.
The corrected-admission fixture was run on unchanged baseline production before
the production correction. No reader copies writer observations or repairs state.

All catalogs are admitted before objects: an explicitly completed empty weapon
catalog, positive armor parser admission, the exact first locomotor input plus
fixed bootstrap seeds, source particle presets, faction support definitions and
authored scenario templates. No retail asset discovery or application launch is
used. The original independent implicit-power gap remains preserved in its
separate AI-roster evidence; it was not removed by this fixture.

Ten cases cover deficit, surplus, zero, disabled producer, disabled consumer,
incomplete producer, damaged producer, future sabotage, foreign dead producer
and foreign incomplete producer. The writer settles one ordinary frame before
admitting two zero-delay one-shot scripts. Readers admit identical definitions
before loading. One script latches `PlayerHasPower`; the other unconditionally
sets `first_frame_seen`. Flags, actual engine actives, identity, exact object HP
and derived totals are recorded immediately and after two ordinary updates.

The candidate passes all ten cases and 60 writer/reader observation rows
(30 matched pairs). An independent
replay of the retained candidate executable also passes and matches the recorded
observations, inputs and script timing. The two foreign controls prove the generic
getters really see conflicting foreign health/construction while receiving local
state and the grid are correct. The unchanged foreign-state comparison covers
mapped count, mapped entity IDs, HP, under-construction and construction percent.
It does not compare the entire foreign world.

The damaged producer naturally regenerates from 500 to 500.29998779296875 during
the ordinary settling frame. The admission check requires alive and damaged;
strict writer/reader comparison retains exact HP. The earlier incorrect HP 500
assumption, diagnostic journals and correction are preserved separately.

Script definitions are preadmitted and initially active; Main's
`loaded_script_lists` is empty in this fixture. This proves ordinary scheduled
first-frame observation, not general script-definition or script-active save
persistence. Sabotage stays at a future deadline 100, preserving the existing
host nonzero mask and deferring its known equality difference from C++.

## Independent negative controls

Both mutations retain the exact fixture and are removed from the final source.

- Restoring the final garrison lookup's old mutable view causes only the foreign
  dead case to fail. Load returns success but receiving HP becomes 0 and destroyed
  becomes true. The other nine case pairs match. This detects the ownership error.
- Changing only the restore grid's predicate back to generic coupled getters keeps
  receiving HP 1000 and lifetime correct, but both foreign cases load grid 0/5/-5.
  The unconditional first-frame script runs while `PlayerHasPower` fires one frame
  late. All 60 observations are retained and the other eight case pairs match.
  This independently detects the wrong eligibility source.

Exact source manifests, single-site patches, compiler outputs, runtime journals,
full diagnostic stacks and hash-verified compressed executables are retained.
Both negative controls failed for these specific assertions, not merely a nonzero
exit. All fourteen candidate Rust/fixture source pins were verified after removal.

## Build and scope limits

Run Cargo from `GeneralsRust` so its `.cargo/config.toml` is discovered. Accepted
builds use the default public Main graph, offline/locked mode, one job and
`RUST_MIN_STACK=16777216`, without profile or Rust-flag changes. A small external
cloud wrapper and its verification JSON preserve that working-directory rule;
they are reproduction tooling outside the repository patch.

An initial wrong-working-directory dependency build was interrupted with exit 130
and is retained as an unaccepted setup attempt. Fixture type corrections and
both temporary tracing variants are separately identified; tracing is absent
from the final production source.

The final default-graph build completed all four public test targets. Serial
execution passed the ten-case power driver, six-case AI-roster driver, five-case
Strategy driver and thirteen alliance tests. The rebuilt power executable is
byte-identical to the independently reviewed candidate. Each fresh-process
driver has one intentionally ignored child entry point, which the parent driver
executes explicitly; alliance retains one ignored external-input probe in its
normal suite. That probe additionally passed in two separate explicit invocations
for the inspected genuine v23 alliance-only and explicit-map fixtures.

Strategy matched its persisted/gameplay contract over 814 paired frames. Its
previously documented body/runtime observation differences remain in the
automatic-cooldown, explicit-mid-aim and explicit-pending cases; these are not
claimed fixed. Generated provenance/review-queue drift, LOC and unsafe gates and
`git diff --check` pass. Rustfmt reports
preexisting formatting drift in two touched files; read-only formatting of HEAD and the
final files confirms those edits are identical, with no new formatting drift.

No full application, network, broad AI, RNG or whole-engine parity claim is made.
