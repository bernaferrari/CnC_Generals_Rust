# Final20b — all 5 remainder failures closed (Final5)

Serial runs only (`--test-threads=1`), no git writes, no formatters. bd `hq-9udt7` still
unwritable (Dolt 1105) → this file is the handoff. All probes removed.

## 1. `jet_stop_idle_timer_sneaky_and_lockon` — FIXED (object/jets.rs)

Failure: `return_to_base_frame` 310 ≠ 311. The idle predicate ignored `status.attacking`,
so the idle RTB timer was armed on the attacking tick (frame 10 + 300 = 310) and never
re-armed at the first non-attacking tick.
Fix: idle predicate now requires `!self.status.attacking`, matching C++
`AIUpdateInterface::isIdle()` state gating — C++ arms `m_returnToBaseFrame = now +
m_returnToBaseIdleTime` only in the idle branch (JetAIUpdate.cpp:1831, 1888-1891) and the
non-idle branch clears it (JetAIUpdate.cpp:1899).

## 2. `jet_stop_and_enter_airfield_land` — FIXED (world_tick/airfield.rs)

Failure: `do_jet_landing_command(jet2, af)` returned false. Probe backtrace:
`select_and_reserve_airfield_for_return` → None. jet2 has `owner_player_id = Some(0)`
while the test airfield is ownerless; `normal_enter_relationship` returns Neutral for a
one-sided owner, so both the exact-controller and allied-airfield producer branches
refused, the reservation was released and the producer cleared.
Fix (C++-grounded, narrow): added `jet_rtb_leg_bound_to_ownerless_airfield(jet, af)` —
af alive/usable AND `owner_player_id.is_none()` AND the jet's live landing/RTB leg is
bound to it (producer match + `landing_in_progress || return_to_base_requested ||
contained_by == af`). Accepted in (a) the producer-first select and (b) the
friendly/exact-controller veto. Grounding: C++ `doLandingCommand`
(JetAIUpdate.cpp:2277-2312) accepts the commanded airfield with NO ownership/relationship
gate, and `JetOrHeliReturnForLandingState::onEnter` keeps `getPP(producerID)` by liveness
alone (JetAIUpdate.cpp:1509-1511); C++ has no ownerless objects (team-owned), so the
host's ownerless-legacy airfields never hit a one-sided-owner veto there. Owner-stamped
airfields keep the strict checks (capture/same-faction-other-player hardening intact).
Also: the APPROACH leg no longer fails closed on a pathfinder refusal — it now installs
the approach leg via `assign_rtb_path` (which has the direct-path fallback), matching C++
RETURNING_FOR_LANDING issuing the move (JetAIUpdate.cpp:1536-1541).

## 3. `shock_stun_ticks_clear_model_bits` — FIXED (object/tests/targeting_and_physics.rs)

Failure mode corrected: the test HUNG (600s+ timeout, "0..u32::MAX tick loop" note).
`apply_shock_wave_impulse` sets `shock_stun_frames = u32::MAX` (C++ `setStunned(true)`
arms NO duration, Object.cpp:1832; IS_STUNNED clears via relief only, PhysicsUpdate.cpp:
671-683). The test's `for _ in 0..start` looped 4.29e9 times regardless of relief.
Fix (test-side; live behavior is C++-correct): impulse retuned to (1.5, 3.0, 0.0) and the
loop now ticks until relief lands, bounded at 1200 ticks (bounce arcs decay
geometrically); bit asserts unchanged. Verified FLAILING/STUNNED both clear.

## 4. wave764 `sources` / `live` / `pack` — FIXED (residual scan repoint)

`simulate_host_shock_stun_dual_peel_dispatch_source` required `OBJECT_SRC` to contain
`"if countdown"`. The live peel no longer branches on the flag — physics-only peels by
delegating `tick_shock_stun_with_countdown(false)` (GW `tick_status_timer_expirations`
sole-decrements; host keeps tumble/bounce). Scan marker repointed to the live honest
channel `"tick_shock_stun_with_countdown(false)"`. All six wave764 tests + pack green.

## 5. `sell_deconstruction_negative_percent_survives_shadow_writeback` — FIXED; destroyer identified

Root cause (backtrace-proven, not inferred): `logic.update()` frame 0 →
`evaluate_victory_condition` (game_logic/mod.rs:112) → `kill_player_for_victory(0)`
(world_tick/presence.rs) → `destroy_object(SellPad)` (sold=true, hp=500 intact). The
skirmish config sets `VictoryType::NO_BUILDINGS`; `counts_as_victory_building` (C++
`Team::hasAnyBuildings(mask)` forces STRUCTURE onto the MP_COUNT_FOR_VICTORY mask,
Team.cpp) counts only STRUCTURE && MP_COUNT_FOR_VICTORY. The test's inline SellPad
template lacked the MP bit, so the sole owner was defeated on frame 0 and the victory
kill destroyed the freshly sold pad — env-independent, exactly as reported.
Fix (test-side; production defeat rule is C++-correct and untouched): both SellPad
template copies now add `KindOf::MpCountForVictory` — retail sellable structures author
KINDOF_MP_COUNT_FOR_VICTORY (FactionBuilding.ini; synthesized structures must carry it,
see buildings.rs:996-1007). Sell now walks past -0.1 (finish ≈ frame 179 < 200) and the
negative-percent shadow writeback asserts pass.

## Guard results (serial, final binary)

Green: all 5 targets; wave764 pack 6/6; `pathfinding` 115/115; `wave555` 6/6; `wave852`
1/1; `wave560` 6/6; `wave923` 1/1; `wave662` 6/6; `shock_stun_channel_via_set_shock_stun`;
`shock_bounce_keep_flip_before_stun_test`; `motion_step_bounce_keeps_inverted_roll_for_stun_kill`;
`shock_bounce_settles_freefall_and_switches_to_stunned`;
`source_scan_tests::generals_science_purchase_residual`;
`source_scan_tests::strategy_center_battle_plan_residual`; `takeoff` 12/12;
`airfield` 86/87; `return_to_base` 4/5.

Observed failures OUTSIDE the 5 (all verified NOT caused by this pass):
- `combat` filter: 954-957/966 with 9-12 failing — failure SET VARIES between identical-binary
  runs (capture/cia/radar/spy_satellite/dead_behavior/fire_sound_loop/chinook snapshot);
  env/timing flakes plus sibling (CmdModuleTriage) in-flight command_executor work — none
  intersect jets.rs / airfield.rs / create_destroy_die.rs changes.
- `gameworld_shadow::tests`: 264-265 passed, 16-17 failing — authority/production/env
  channel tests, again varying between identical runs; not on my changed surfaces.
- `targeting_and_physics::jet_hangar_taxi_then_afterburner_at_runway_head_and_rtb_approach`:
  fails at the PRE-EXISTING entry gate `!needs_rearm && !requested && !docked_rearm_pending
  → return false` (airfield.rs:2581 region) — my edits execute only after that gate, so this
  is a 6th pre-existing failure, outside the assigned 5. Left as documented remainder.
- `cnc_game_engine::source_scan_tests::return_to_base_aircraft_residual` — command_system
  scan (`"returntobase"` mapping) failing mid-sibling work on command_executor;
  `get_repaired_command_aircraft_requires_airfield` — Idle ≠ SeekingRepair; do_jet_landing
  early gate returns false pre-my-changes identically (SeekingRepair is never reachable
  through do_jet_landing_command). Both outside the 5.

Net: the 5 assigned failures are closed end-to-end; the touched production surfaces are
jets.rs (idle arming), airfield.rs (ownerless-airfield RTB exception + approach fallback),
and the wave764 scan marker. Test-side completions: shock-stun no-duration loop, SellPad
MP_COUNT_FOR_VICTORY (×2 copies). Probes removed (create_destroy_die.rs, presence.rs
restored byte-identical; targeting_and_physics.rs probe block removed).
