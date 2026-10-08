# Authoritative empty AI roster restoration

## Contract and bounded correction

Accepted Rust WorldSnapshot versions 23 and 24 explicitly carry `ai_players`,
including an empty vector. Restoration previously always constructed skirmish
controllers and then skipped applying empty rows. This invented a controller
for a nonlocal player, changed its saved build permission, and retained stale
controllers in an already-populated destination.

The correction skips global skirmish setup when the saved roster is empty and
always invokes the existing receiving-instance roster replacement API. The
nonempty ordering is unchanged. No schema, RNG, player-type redesign, or new
public API is introduced.

C++ authority is `Common/RTS/Player.cpp`: `setPlayerType` creates controllers
only for computer players; `xfer` transfers and checks AI presence at lines
4178–4189 and directly transfers `canBuildUnits`/`canBuildBase` at 4293–4296.
The original map/player setup establishes the presence contract. These source
comparisons are not original-engine execution or C++ byte-compatibility proof.

## Strict real-file evidence

`tests/ai_roster_save_restore.rs` uses separate fresh writer and reader
processes, public `SaveFileManager::save_game`/`load_game`, and ordinary
`GameLogic::update`. Readers consume identical pre-admitted catalogs and save
bytes only; they never read writer observations or repair loaded state.

Six cases cover:

- Empty roster into a fresh destination
- Empty roster into a destination containing active Brutal controller 3,
  whose owner is absent from the save
- Three same-faction players with only owner 2 registered as Hard/inactive
  and an explicitly relocated base anchor

Each runs against actual current v24 writer output and an explicitly synthetic
v23-envelope compatibility file. The latter changes only the world version,
retaining every positional-body byte and the sibling Players v4 chunk. It is
not historical writer output. A separate fresh process publicly decodes the
genuine checked-in outer-v23 `alliance_only.sav` and verifies its empty AI
vector, two players, and two objects.

The exact strict fixture source SHA-256 is
`a21c44a697b13ce668ad28c8df728a08e25e74cd9cd9818e7b912c2a08be489f`.
On unchanged production at `89bfc2aa38f9b6aad00714fce8fd8264c5e73ad1`,
four absence cases failed and both nonempty controls passed full immediate
and next-frame comparison. Fresh readers had one phantom active controller;
populated readers had that controller plus stale owner 3. Saved false build
permission became true, and nonlocal auto-engagement became unpaused.

The same unchanged fixture passes all six cases with the correction. Exact
recorded player/object identity, owner, faction, alive state, build flags,
resources, AI rows/configuration/activity, and pause behavior match before
and after one ordinary frame. Empty destinations have zero controllers;
populated destinations lose the stale controller. The nonempty control keeps
exactly inactive owner 2. It uses ordinary true build permissions and does
not prove that saved false permissions in a nonempty roster are preserved.

Skipping setup without applying empty rows would leave the stale controller.
Always setting up and then clearing would still overwrite saved false build
flags. Both omitted-change failures are detected by the strict assertions;
partial candidate binaries were not separately built or run.

## Admission and the preserved power gap

All inputs are admitted before objects: an explicitly completed empty weapon
catalog, positive in-memory armor parser admission, the exact first locomotor
file candidate plus fixed production seeds, source particle presets, faction
support definitions, and the scenario templates. No retail asset download,
AssetManager, native ThingFactory, or broad fallback discovery is used.

The first fixture version left anchor energy implicit. Generic Rust structure
fallback chose CommandCenter consumption 3, so ordinary source updates set
`power_available=-3`. Snapshot capture stored `player.resources.power=0`, and
restore used that value for immediate available power; the next frame rebuilt
the expected -3. Both inactive-owner cases exposed this separate immediate
power gap while their AI contracts passed.

That original source, executable, files and journals remain preserved. The
dedicated `baseline/replay-original-power-gap.py` helper replays the original
inactive-owner case from the retained executable and keeps its strict immediate
comparison; the helper was prepared but not executed during this slice.

For the isolated AI witness, all three anchor templates now explicitly author
`energy_production=Some(0)` before admission in both processes, with catalog and
object assertions. C++ `ThingTemplate.cpp` defaults EnergyProduction to zero;
the current C++ Energy Xfer expects building loading to reconstruct production
and consumption. Every original resource comparison is retained. This fixture
choice does not fix or dismiss the separately observed Rust power gap.

## Adjacent validation and diagnostics

The existing alliance suite passes **13 tests**, with **1 ignored** supplied-
file probe. That probe separately passes for each of the two genuine checked-in
v23 alliance fixtures. The historically named same-faction known-gap test is
already enabled and passes within the normal suite. Its prior ignored/red
README description was stale and is corrected explicitly. The historical
Players-v1–v4 test now checks its genuine outer-v23 empty-roster precondition
and deliberately preserves serialized false build flags.

Alliance fixtures now admit the public empty WeaponStore and source particle
presets before their unchanged inline-weapon world construction, including
the supplied-file reader. The guarantee covers the inspected fixture inputs;
arbitrary supplied saves can have additional catalog requirements.

The unchanged Strategy continuation target passes all **5 cases and 814 paired
observations**, with no persisted/gameplay difference. Reader AI count remains
zero throughout. Automatic acquired and mid-aim cases now have full recorded
state equality. Automatic cooldown and both explicit cases still expose their
body-machine and/or raw-barrel differences; no whole-state equality is claimed
for those cases.

All compilation used the default public Main graph, offline/locked, one job,
unchanged profile, and `RUST_MIN_STACK=16777216`. Compiler JSON and readback-
verified compressed binaries are retained. The three-target candidate compile
and the subsequent alliance-only fixture compile passed. LOC/unsafe ratchets,
edition-2024 rustfmt, and whitespace checks pass without expanding allowances.

## Limits

This is successful-file membership and continuation evidence. Failed loads
remain nontransactional; later restore errors can follow earlier mutations.
The manager replacement acts on the receiving instance, but the surrounding
pipeline retains independent ambient state, so no whole-world isolation claim
follows. The internal direct-snapshot AI unit test was source-reviewed but not
rerun on a separate library-test graph. No broader nonempty-roster repair,
full AI behavior proof, complete gameplay parity, application launch, network
work, or RNG workaround is included.
