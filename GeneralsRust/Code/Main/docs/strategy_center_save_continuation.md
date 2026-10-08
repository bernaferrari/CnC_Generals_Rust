# Strategy Center fresh-process save continuation

Verified on 2026-10-07 on production base
`8979cf354c802510cc7c606acc32cab2e3c8b14e`. Only the public integration
test and this document changed; no production or save-schema fix was needed.

The tested fixture is `tests/strategy_center_save_continuation.rs`, SHA-256
`4280654a8a83f61708f43022d3ef28ff0d193f3e1d14af852bbf11400d1f0702`.
Its default-feature test executable has SHA-256
`8175b411825756e0b71249b8ca8189a0a6eefc6d3b92f3f84054a2b1e110f281`.

## Production boundary and fixed inputs

Each case launches a fresh writer/reference process and then a separate fresh
reader. The writer activates Bombardment publicly, waits through its ordinary
210-frame door transition, and creates the ground target. Explicit controls
issue the public AttackObject command before the first mood tick. The writer
uses `SaveFileManager::save_game` to create a real `.sav`, then continues
uninterrupted. The reader calls `load_game`, observes the immediate checkpoint,
and advances only through `GameLogic::update`. It never reads the reference
trace or repairs objects, goals, plans, readiness, ownership, or controller state.
Fresh processes prevent the existing ObjectId-keyed turret TLS cache from
silently carrying writer state into the reader.

Both processes preload identical bounded catalogs before any player/object
admission or load:

- Three scenario templates: AmericaStrategyCenter at the origin, a stationary
  weaponless ground vehicle at `(150, 0, 0)`, and a weaponless enemy anchor at
  `(2000, 0, 0)`. Two actual Player records have alliances 31 and 45
- The twelve existing GLA support definitions required by the reader's
  `restore_global_ai_state` route, through `ensure_ai_faction_templates(GLA)`.
  These are definitions only; exactly three live objects remain throughout
- An intentionally empty weapon catalog admitted through the public API.
  Scenario weapon slots remain unnamed; activation installs the existing
  Strategy residual gun with damage 200, minimum/range 100/400, seven-second
  reload, speed 150, zero pre-attack delay, and unlimited ammunition
- One unreferenced Armor definition through the public parser
- One exact authored Locomotor.ini at the loader's first candidate inside
  the isolated case directory. BasicHumanLocomotor and TechnicalLocomotor
  fields match their existing source seeds. Exact bytes and public parse count
  two are checked before bootstrap; the successful first candidate stops later
  probes, and remaining fixed seeds are admitted in memory
- Existing MuzzleFlash and BulletImpact particle presets through the public
  in-memory manager API, with zero systems created during catalog admission.
  The retail particle initializer is never called

All fifteen ThingTemplate configurations, locomotor definitions and particle
preset definitions are compared between processes. AssetManager and native
ThingFactory remain absent. This establishes a bounded authored-file route;
it is not a zero-file-read or whole-application bootstrap claim.

## Results

The driver passed all five source-derived scenarios, covering ten fresh child
processes and 814 paired observations, including five immediate checkpoints.
There is one normal Rust driver test and one ignored child entry point selected
explicitly by that driver; the five scenarios are not five distinct Rust tests.
Every subsequent frame is compared exactly, with float bits retained for pose
and weapon timing. There is no equality tolerance.

An independent serial replay of the same byte-verified executable also passed
all ten child processes and 814 observations per side. Its complete writer and
reader traces exactly reproduce the first corrected run, including all state
differences below. This is Rust continuation verification informed by C++ source,
not a full-engine original C++ execution comparison.

| Case | Checkpoint next frame | Final next frame | Paired observations |
| --- | ---: | ---: | ---: |
| Automatic acquisition | 212 | 480 | 269 |
| Automatic mid-Aim | 233 | 480 | 248 |
| Automatic cooldown | 257 | 480 | 224 |
| Explicit mid-Aim | 233 | 280 | 48 |
| Explicit pending shot | 256 | 280 | 25 |

All cases match persisted gameplay inputs, goals, plan/door registry, turret
phase/pose, readiness inputs, canonical barrel cursors, accepted discharge
markers, pending records, roster, and HP on every observed frame. The reader
creates no extra objects.

Automatic acquisition preserves yaw -90 degrees at next frame 212, then takes
exactly one two-degree step to -88 on processed frame 212. Automatic discharge
and HP loss occur at processed frames 256 and 466 (next frames 257 and 467),
with target HP `1000 -> 800 -> 600`. The cooldown gun is still unready for frame
465 and ready for frame 466 using the production `frame * f32(1/30)` clock;
the saved last-fire float bits and Aim/Fire alternation match throughout.

Explicit discharge occurs at processed frame 255, leaving HP 1000 and one
projectileless delayed record at next frame 256. Both processes apply exactly
one HP change on processed frame 256, reaching HP 800 without another accepted
discharge. No materialized projectile is present in that checkpoint.

## Observed state differences retained

Exact observed-state equality is false in all five cases. These differences
remain in the traces and summaries rather than being repaired or hidden:

- `restore_global_ai_state` registers player 1 as an active AI with a difficulty
  while the writer has no AI controller. This differs at every observed frame
  but causes no roster, position, discharge, or HP difference in this window
- The explicit aiming flag is true in the writer and false in the reader
  throughout both explicit windows. After the pending-shot load, fire-intent
  count and last-fire diagnostic fields also remain different
- Cooldown and explicit-pending `weapon_fire_status` reconstruct as ReadyToFire
  rather than BetweenFiringShots at the immediate checkpoint; the next ordinary
  update reconciles that derived status without an extra discharge
- The raw backing barrel index is 1 in the writer and 0 in the reader after a
  post-shot load. `restore_weapon_barrel_runtime_for_slot` intentionally stages
  the saved raw cursor until draw topology is available. The authoritative
  `weapon_barrel_cursor_for_snapshot`, configured cadence/count and actual
  fired barrel match. The raw cooldown difference ends at the second discharge

These observations do not establish exact snapshot parity. They also do not
establish a gameplay defect merely because a diagnostic field reconstructs
differently. No gameplay fix was inferred from them.

## Failure lineage and verification limits

The first unchanged fixture compiled, but its readers failed setup admission:
four gained the twelve restore-required GLA definitions, and cooldown failed
on an unregistered MuzzleFlash template. That run also lacked confinement of
the restore-triggered locomotor lookup. Its source, executable identity, saves,
inputs, logs, and partial journals are retained. It is not a gameplay regression
baseline. Catalog corrections were made symmetrically before admission/load.

The first corrected-source compile exposed a JSON macro syntax error. Adding
parentheses around the controller-array expression was the sole subsequent
source change. That failed source and compiler output are retained separately.

Compilation used Main default features (`default`, `game_client`), offline and
locked, one job, unchanged test profile, and `RUST_MIN_STACK=16777216`. The public
target was built with `--no-run` before serial direct execution. Process status,
stdout/stderr, full inputs, saves and per-frame journals are retained; rows are
flushed before invariant checks, including on failure.

The Rust LOC and unsafe ratchets, generated provenance and review-queue drift
checks, edition-2024 rustfmt for the new test, and `git diff --check` pass.
Existing repository-wide ratchet allowances are unchanged.

This does not establish C++ byte compatibility, original turret phase Xfer
equivalence, materialized projectile-flight persistence, finite clips,
nonzero wind-up, activation-phase continuation, transactional load, broad AI
continuation, retail assets, or same-process world isolation. No RNG algorithm
or ownership work, network change, application launch, or broad test graph was
included.

## 2026-10-07 empty-roster correction follow-up

The results above remain the dated pre-correction baseline. The later bounded
AI-roster restore change was tested with this exact unchanged Strategy witness
and the same authored inputs in fresh writer/reader processes. All five cases
and 814 paired observations still match persisted/gameplay expectations.

The reader-only AI controller is now absent throughout every trace, matching
the writer's explicitly empty saved roster. Automatic acquired and mid-aim
cases now report full equality across all recorded state fields. Automatic
cooldown and the two explicit cases still record the body-machine and/or raw
barrel-cursor differences described above; those observations remain visible
and are not relabeled as whole-state equality.

The retained executable for this follow-up has SHA-256
`e49bed7cf70ae1aa228c68ded56c1d935b0d6c8db4e00339e9d1ceaf0b49ba89`.
The exact default-Main driver passed in both the implementation run and an
independent retained-binary replay. See [AI roster restore evidence](ai_roster_save_restore.md)
for the unchanged-production failing membership witness, both accepted schema
controls, bounded repair, adjacent gates, and remaining limits. The full
original diagnostic history above has not been rewritten.
