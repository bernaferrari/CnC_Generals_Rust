# Final20c — command/ai remainders closed (CmdAiRemainder)

Serial runs only (`--test-threads=1`), no git writes, no formatters, no probes left behind.

## 1. `script_skirmish_command_button_most_valuable_does_button_not_force_attack` — FIXED (polluter identified + cleanup)

Serial-only, cross-module. Repro: `cargo test -p generals_main --lib command_button -- --test-threads=1`
(30 pass / 1 fail, 100%). Bisection (multi-filter libtest):
- target + `hunt` filters (production_and_hunt, crates, host_mods_combat, wave384) → target OK.
- target + `combat_targeting` (command_executor::validate ×5) → OK.
- target + `unit_specific_sound` → FAIL. Polluter:
  `cnc_game_engine::ui_commands::tests::command_button_unit_specific_sound_resolves_from_ini`.

Root cause: that test calls `initialize_control_bar()` and inserts
`Command_BlackLotusHackBuilding` into the process-global INI control bar, never cleaning up.
The victim resolves its button through
`GameLogic::leftover_skirmish_command_button_exists` (script_team_actions.rs:634-653):
with a non-empty global bar where `find_command_button_resolved("Command_Stop")` is None, the
`!bar.get_button_names().is_empty() → return false` branch treats the polluted bar as
authoritative and the name-derived fallback `command_type_from_button_name` is never reached.
The skirmish script then never executes Command_Stop → attacker stays Moving → assert
`AIState::Idle` fails. In isolation the global bar is empty, so the fallback fires and the test
passes. C++ ground: the control-bar CommandButton list is process-global (ControlBar::init /
CommandSet maintenance); a host mutation test must restore the shared store.

Fix:
- `Code/GameEngine/Common/src/common/ini/ini_command_button.rs` — new
  `ControlBar::remove_command_button(&mut self, name) -> bool` (removes from
  `command_buttons` + `button_order`).
- `Code/Main/src/cnc_game_engine/ui_commands.rs` — the polluter now removes its inserted
  button after asserting, restoring the global bar.

Verified: `command_button` filter 31/31 serial.

## 2. `ai::cpp_parity_tests` pad-layout 2F (`compute_center_and_radius_pads_geom_point_four`, `process_building_queue_skips_automatic_layout_pads`) — NOT REPRODUCIBLE; GREEN

Both tests pass serially under every scoped repro:
- `cargo test -p generals_main --lib -- ai::cpp_parity_tests --test-threads=1` → 73/73.
- `cargo test -p generals_main --lib -- ai:: --test-threads=1` → 76/76.
No working-tree delta exists under `Code/Main/src/ai/` (git diff HEAD), and the fixture math
the documentation blamed (`leftover_compute_center_and_radius_of_base` geom*0.4 hypot) matches
its asserts. Conclusion: the documented "fixture-math errors since LasersFix" remainder is
stale — repaired by changes already in the tree (Final5/LasersFix-era work), not by this pass.
Closed as green with evidence; no edit made.

## 3. `jet_hangar_taxi_then_afterburner_at_runway_head_and_rtb_approach` — FIXED (airfield.rs + test fixture)

Two stacked causes, both closed:

(a) Production entry gate (`world_tick/airfield.rs`, `try_return_to_base_rearm`): the gate
`!needs_rearm && !requested && !docked_rearm_pending → return false` rejected the test's
taxi-phase jet (no weapon ⇒ no rearm need, no request). Added `rtb_leg_in_progress`
(`jet_ai.landing_in_progress || jet_ai.rtb_landing_phase != 0`) as a fourth pass condition.
C++ ground: JetAIUpdate drives RETURNING_FOR_LANDING → LANDING → DOING_LANDING/TAXI as
sequential states once entered (JetAIUpdate.cpp:1509-1541, 2277-2312);
`isOutOfSpecialReloadAmmo` gates entering the return, not continuing an open leg.

(b) Test fixture (targeting_and_physics.rs):
- The objects are ownerless legacy objects; without an alive USA player,
  `normal_enter_relationship`'s `(None, None)` branch requires
  `unique_player_id_for_team` and falls to Neutral, so no airfield candidate passed
  `is_friendly_airfield`. Seeded `Player::new(0, Team::USA, ...)` in the test.
- The distant inbound jet was called through `try_return_to_base_rearm` without the RTB
  request stamp. C++ order is command-then-tick (`doLandingCommand` stamps the request,
  JetAIUpdate.cpp:2277-2312); the test now sets `return_to_base_requested = true` before the
  rearm call (its unnamed auto-reload empty clip alone is not an
  `isOutOfSpecialReloadAmmo` trigger).

Verified: test green; `airfield`/`return_to_base`/`jet` guard filter green except the two
documented pre-existing remainders below.

## 4. (Found during guards) wave519 presentation shock/power/jet pack — FIXED (scan repoint)

`host_live_presentation_shock_power_jet_residual_wave519::presentation_shock_power_jet_sources`
(+ composite + live) failed: the stamp scan accepted only two stale stamp phrases, while the
live honest stamp in `presentation_frame/unit_render.rs:1058` reads
`"Wave 519: exploded flail/bounce residual bits."`. Repointed the scan to also accept the live
phrase (same pattern as Final20b item 4). wave519 6/6 serial.

## Guard results (serial, final binary, combined run: 286 passed / 2 failed)

Green: `command_button` 31/31; `ai::` 76/76 (pad-layout included); all `jet*`/`airfield`/
`return_to_base` targets incl. `jet_hangar_taxi_then_afterburner_at_runway_head_and_rtb_approach`;
wave519 6/6; `superweapons_and_scripts::` module 43/43.

Remaining failures — BOTH pre-existing and documented (Final20b "outside the 5" list), in the
network_and_scripts repair cluster now owned by sibling NetScriptsTriage; untouched by this pass:
- `cnc_game_engine::source_scan_tests::return_to_base_aircraft_residual` (command_system scan).
- `game_logic::...::get_repaired_command_aircraft_requires_airfield` (Idle ≠ SeekingRepair;
  unreachable through do_jet_landing_command identically before this pass).

Full-suite serial run attempted (`--lib -- --test-threads=1`): hangs at pre-existing
`game_logic::game_logic::tests::fast_chunky_sync_fail_opens_when_legacy_globals_are_busy`
(killed at the 1h clamp, no test-result summary; module unrelated to every surface touched
here) — same hang class as Final20b item 3's unbounded stun loop.

Changed surfaces: `world_tick/airfield.rs` (RTB gate), `object/tests/targeting_and_physics.rs`
(two fixture seeds), `cnc_game_engine/ui_commands.rs` + `Common/.../ini_command_button.rs`
(global-bar test cleanup + remove API), wave519 residual scan phrase. No probes left.

---

# Final20c — phase3/scatter/jets remainder (SmallRemainderTriage)

Serial runs only (`--test-threads=1`), no git writes, no formatters, no probes left.
Appended below CmdAiRemainder's report (their items 1-2 also independently re-verified
green from this session: `command_button` filter 31/31 within my 266-test gate;
`ai::cpp_parity_tests` 73/73).

## 1. phase3_produce 4F → 14/14 GREEN

Two production tests shared a dozer-arrival root cause; two were fixture-contract errors.

(a) Production fix — `world_tick/production.rs` `update_construction`: the dozer pre-scan
required `arrived` (`!status.moving && path fully consumed`) on top of the 70wu dock window.
`resume_construction` installs a live 2-node approach leg (`path_approach_with_state`), and a
direct `update_construction` call never consumes it, so `nearby_dozers` was always 0 —
`exclusive_dozer_does_not_stack_build_rate` got percent 0, `dozer_dock_plays_under_
construction_loop_and_stops_on_complete` never started the UnderConstruction loop, and the
HP-gain test saw a still scaffold. C++ oracle: `DozerActionPickActionPosState::update`
(DozerAIUpdate.cpp:318-335) — arrival SUCCESS is purely
`dist(dozer, goalPos) <= max(MIN_ACTION_TOLERANCE=70, boundingSphere + SLOP=15)`; the
distance IS the arrival test (a live approach leg inside the window still builds; a dozer
stopped outside never does), and `DOZER_DO_BUILD_AT_DOCK` (cpp:499-507) then gates progress
to that ACTION dock. Fix: dropped the `arrived` tuple field; the dock-distance test is the
arrival gate. Fixes all three tests (progress + sound + HP channel).

(b) `exclusive_dozer_does_not_stack_build_rate` got 0.05 (= 0.1 × 0.5): the
`ensure_test_player_for_team` fixture is a 0/0 energy grid, and `compute_player_power_factors`
(C++ Energy.cpp:51-65: consumption==0 → ratio=production=0) feeds
`ThingTemplate::calcTimeToBuild` (ThingTemplate.cpp:1541-1558: ratio<1 → buildTime/=
penaltyRate, floor m_MinLowEnergyProductionSpeed=0.5) — 0/0 grid ⇒ 0.5× production speed is
C++-correct. The exclusivity contract asserts the 1× rate, so the test now satisfies the
grid (`power_produced = 10` → ratio ≥ 1 → factor 1.0). Test-side; production untouched.

(c) `completed_production_preserves_factory_identity_exit_facing_and_rally` expected the RAW
rally at `movement.target_position`. C++ oracle: `DefaultProductionExitUpdate::spawn`
(DefaultProductionExitUpdate.cpp:88-94) pushes the ADJUSTED custom rally —
`Pathfinder::adjustDestination` (AIPathfind.cpp:5331) spiral → `checkForAdjust`
(cpp:5177-5219) → `adjustCoordToCell` (cpp:8936-8948) snaps to the pathfind cell; the unit
never targets the raw coordinate. Host `adjust_factory_exit_destination` (production.rs) is
faithful (observed (44.5,−35.5) is a cell-center snap 3.54wu from the rally). Test now
asserts the destination is within one PATHFIND_CELL (10wu) of the rally and that
`path.last() == target_position`. Test-side.

## 2. scatter_and_chain 1F → 46/46 GREEN

`flashbang_scatter_misses_intended_residual` failed at "splash still hits". The test
expected a vehicle to LOSE HP in flashbang splash. Retail oracle: Weapon.ini:2535-2556
`RangerFlashBangGrenadeWeapon` is DamageType SURRENDER (35/10 + 10/40, ScatterRadius 4), and
retail Armor.ini gives every vehicle armor set SURRENDER 0% (TankArmor line 139 et al.;
"Capture type weapons are effective only against infantry"); infantry armors are 100%. C++
`ActiveBody::attemptDamage` applies the armored amount (the surrender swap-out is compiled
out, ActiveBody.cpp:509-527 `ALLOW_SURRENDER` disabled) — so a tank takes ZERO flashbang HP
damage by design. Host armor tables (`TankArmor`/`ProjectileArmor` Surrender=0.0) are
retail-correct. Test corrected: splash ring still ACQUIRES the vehicle (`hits > 0`) but
`hp_after == hp_before` is now the asserted contract, with the Armor.ini citation.

## 3. production_and_mobs jet family 4 — RE-VERIFIED GREEN (no edit)

`empty_jet_circles_last_airfield_instead_of_bleeding_in_place`,
`jet_airfield_rearm_waits_clip_reload_frames`,
`jet_out_of_ammo_paths_to_distant_airfield_then_rearms`,
`overlord_gattling_addon_residual_install_and_fire`: all pass serially — JetDockFix3
confirmed. `production_and_mobs` filter: 110/110.

## 4. ai::cpp_parity_tests — RE-VERIFIED GREEN (no edit)

73/73 serial, including both documented pad-layout failures
(`compute_center_and_radius_pads_geom_point_four`,
`process_building_queue_skips_automatic_layout_pads`). Matches CmdAiRemainder item 2: stale
remainder documentation, repaired by earlier in-tree work.

## Serial-lottery note (transient, resolved)

My `update_construction` rework briefly broke the build mid-edit (`exclusive_builder` E0425)
and blocked sibling compiles ~10 min; restored in the same session (PilotsUnitsTriage's
special_power.rs `PROBE tnt` window caused one more cross-block; theirs to remove).

## Guard results (serial)

- Gate (my surfaces + shared filters `phase3_produce scatter_and_chain production_and_mobs
  ai::cpp_parity_tests command_button`): 266 passed / 0 failed.
- Adjacent dozer/construction sweep (`resume_construction dozer construction queue_exit
  under_construction wave819`): 276 passed / 16 failed with my fix — BASELINE-PROVEN
  pre-existing: with HEAD's production.rs the same sweep fails 19 (the same 16 plus my 3
  then-failing phase3 targets). The 16: 2× cnc_game_engine source_scan
  construction/resume-hotkey residuals, 3-4× wave819 dozer bored dual peel, 3×
  gameworld_shadow economy_construction, network_and_scripts
  dozer_line_assigns_each_worker_a_segment (NetScriptsTriage cluster), 4× presentation_frame
  construction/dual_tick/render_overlay, graphics render_pipeline frozen_fow, ui hud
  construction_panel cameos — sibling clusters + the final20b-documented env/channel flake
  class. Not touched by this pass. (Full-suite serial additionally hangs at the
  CmdAiRemainder-documented `fast_chunky_sync_fail_opens_when_legacy_globals_are_busy`.)

Changed surfaces: `world_tick/production.rs` (dozer arrival gate), `world_tests/phase3_produce.rs`
(powered grid fixture + cell-adjusted rally assert), `world_tests/scatter_and_chain.rs`
(flashbang-vs-vehicle retail armor contract). No probes left.

## PilotsUnitsTriage — pilots_and_movement 16F→2F, unit_residuals triaged (Final20c)

Serial runs only (`--test-threads=1`), no git writes. All probes removed
(special_power.rs byte-identical to HEAD, verified via git diff).

### pilots_and_movement: 60/76 → 74/76 serial (14 of 16 fixed)

Fixed (all test-side except where noted; production behavior verified
C++-grounded, untouched):
1. `worker_shoes_upgrade_speed_and_supply_boost_residual` — TWO stacked causes:
   (a) established research BuildTime class: retail Upgrade_GLAWorkerShoes
   BuildTime 10s now resolves on the producer queue; single `update()` assumed
   the no-INI fallback → tick `retail_research_frames()` per-frame.
   (b) fixture `GLASupplyStash` template lacked `dock_kind = SupplyCenter` →
   `number_approach_positions_for_dock` returns 0 → live dock approach-queue
   fail-closed (`is_clear_to_approach` on an empty slot vector = Blocked) →
   `try_claim_dock` refused → drop-off never credited. Stamp dock_kind on the
   fixture (retail GLASupplyStash authors DockKind SUPPLY_CENTER).
2. `hacker_move_command_stops_hacking` — ensure_test_player_for_team now seeds
   100k supplies; assert cash delta instead of absolute 0.
3. `tank_hunter_tnt_does_not_consume_cooldown_at_click` +
   `burton_and_tnt_plant_reject_bridges` — production now arms parsed
   SpecialPowerModules with authored ReloadTime at creation (C++
   SpecialPowerModule.cpp:86-94 ctor parity), so the click hit a
   spawn-armed (not-ready) power and never queued. Tests force the module
   ready pre-click, preserving the true contract (click must not consume /
   restart the charge; consume_at_prep still fires at preparation).
4. steal_cash_hack x3, `disable_vehicle_hack_command_disables_after_reach`,
   `leftover_lotus_prep_aborts_when_target_stealthed`,
   `leftover_sa_trigger_queues_ini_trigger_sound` — leftover SA channel never
   got past Unpacking: `leftover_sa_consume_prep_charge` (typed charge consume,
   C++ SpecialPower.cpp:308 fail-closed, no any-unit fallback) refused on the
   bare fixture with NO authored modules. Fix: `ensure_test_black_lotus_template`
   now authors the three retail modules (steal/disable/capture, ReloadTime 0 →
   spawn-ready, deterministic). Plus the established facing-budget class:
   C++ NeedToFace precedes Unpack (6730ms steal / 2000ms disable) + Prep, and
   the facing slice consumes a variable number of turn-rate ticks — fixed
   big-dt budgets replaced with bounded completion loops (frame-advancing).
5. `leftover_burton_flips_180_after_unpack` — same facing phase; total-delta
   from spawn is timeline-dependent, so the test now measures the flip
   residual itself across the Unpacking boundary (exactly PI at unpack end).
6. `angry_mob_spawns_member_objects_on_nexus` — production follows C++
   SpawnBehavior.cpp:221-243 replacement-times drain (first INITIAL=5 due at
   sync, rest on exit-delay dues); the assert demanded all 10 immediately.
   Test now asserts the initial fill, advances through the replacement window
   to SpawnNumber, then asserts the no-over-spawn steady state.
7. `plant_timed_demo_charge_command_plants_after_reach` +
   `plant_and_detonate_remote_demo_charge_residual` — facing-budget class;
   bounded completion loops to the plant residual.

### pilots_and_movement remainder (2, precisely)

1. `black_lotus_capture_building_without_upgrade` — advanced to the LAST
   assert (ownership transfer). Fixed along the way: fixture capture_power =
   BlackLotus (command fail-closed without it) + local-player shroud-visibility
   gate (fixture has no map FOW → drop is_local). Remainder: the leftover
   CAPTURE channel never completes even with an 80s bounded drive — the
   capture prep/completion residual (likely owned by the
   defector/capture-FX channel, not the lotus SA channel) needs a dedicated
   trace of `leftover_start_sa_preparation`'s capture arm and the
   BlackLotusCaptureBuilding completion path. Next probe: eprintln in
   `leftover_start_sa_preparation` for LeftoverSaKind::CaptureBuilding.
2. `flashbang_grenade_bezier_flight_and_blast` — flight/spawn/honesty asserts
   pass; the impact runs (`elapsed >= frames`) and calls
   `apply_ranger_residual_at(pos, source, intended, true)` but the enemy HP
   never drops. Suspects (in order): `ranger_flashbang_scatter_misses(seed,
   hit_r)` miss branch silently eating the damage (seed = f(source, intended,
   frame) — possibly deterministic-miss for this fixture), or a legality gate
   inside apply_ranger_residual_at. Next probe: print
   `flashbang_scatter_misses` counter + the gates at streams_and_rpg.rs:436.

### unit_residuals: 39/52 (unchanged) — triaged to classes, not fixed (timebox)

- hacker_disable_building x2 → SAME facing + authored-module class as the
  fixed pilots cluster; the helpers.rs BlackLotus module authoring likely
  unblocks their prep phase; need their budgets converted to bounded loops.
- ranger/rpg_trooper/missile_defender/gattling/minigunner/dragon_tank/
  avenger weapon ramp/residual cluster → weapon_bootstrap seed table vs
  retail INI class (established): fixtures author bootstrap weapon tables
  while production now resolves retail INI; each needs its expected constants
  reconciled to the loaded INI values.
- troop_crawler_transport_load_unload + detect_stealth → transport/contain
  (dock claim + open-contain) — COORDINATED with NetScriptsTriage via hub;
  they own dock/evac/repair; deferred to avoid same-file collisions.
- wave_guide x2 → dam Die template + water-velocity grid queue, untraced.

### Guards (serial): pilots_and_movement 74/76; unit_residuals 39/52 (no
regression from my helpers.rs change: 39 == baseline 39). Touched production:
NONE (probes removed; update.rs and special_power.rs verified byte-identical
to HEAD via git diff). Files changed: pilot_and_crate_movement.rs,
hacker_and_special_movement.rs, world_tests/helpers.rs (fixture fidelity only).

# Final20c — network_and_scripts triage (82/105 → 96-98/105 serial)

Serial runs only (`--test-threads=1`), no git writes, no formatters. All probes removed.
Started 23 failing; **14 closed**, 9 remain (7 stable + 2 flake-class seen crossing runs:
`host_named_unit_found_with_empty_object_registry`, `live_host_polygon_inside_and_enter_without_object_registry`
— both pass/fail between identical trees, the documented cross-test global-state class).

## Root-cause classes confirmed this pass (production verified against C++ oracle in every case)

1. **Frame-0 sole-benefactor refusal (8 tests fixed).** C++ `Object::attemptHealingFromSoleBenefactor`
   (Object.cpp:1905) refuses healers while `now <= m_soleHealingBenefactorExpirationFrame` — on a virgin
   frame-0 fixture `0 > 0` is false, so the first in-range heal is refused, and the dozer repair tick then
   cancels to Idle (`update.rs:576 !healed && !target_full`). Fixtures must stamp the live clock
   (`game_logic.frame = 1`), mirroring mood.rs/teams.rs precedent: repair_command_allows_repairing_neutral_structures,
   dozer_structure_repair_residual_recovers_hp_over_time, war_factory_vehicle_repair_residual_recovers_hp,
   repairing_state_heals_target_in_range, ambulance_auto_heal_residual_recovers_infantry_hp,
   ambulance_auto_heal_residual_out_of_range_then_in_range. **Production is oracle-faithful — do not "fix" the `>`.**

2. **KindOf / authored-module fixture stamps (4 fixed).**
   - war_factory test: C++ `canGetRepairedAt` (ActionManager.cpp:159-163) allows ground vehicles at
     KINDOF_REPAIR_PAD only (FS_WARFACTORY does not authorize) → fixture adds `KindOf::RepairPad`.
   - get_repaired_command_aircraft: C++ requires `isAboveTerrain` (ActionManager.cpp:164-171) → fixture
     stamps `status.airborne_target = true`. Accept-side now goes through JetAIUpdate doLandingCommand
     (JetAIUpdate.cpp:2277) which does NOT set SeekingRepair or the generic order target — test asserts
     "accepted, not Idle" (C++-faithful).
   - entering_state_docks: bare TestTank authors no Transport module; C++ dock authority revalidates the
     ContainModule at arrival → fixture now uses create_test_transport (STILL FAILING, see below).
   - hijack ×2: executor + pending-drain both gate on the hijacker basename (C++
     ConvertToHijackedVehicleCrateCollide residual) → fixtures author a "TestHijacker" infantry template.

3. **Map-bounds scan filter (1 fixed).** process_ai_behavior_hunt: C++ PartitionFilterSameMapStatus —
   the far enemy at x=2000 was off the default 512² map; moved in-bounds (240,0,0), still >200-bubble.

4. **One-phase-per-tick capture channel (3 fixed).** C++ SpecialAbilityUpdate advances one phase per
   update call; tests ticked 2 phases worth in 2 calls. Added the third tick (unpack create → unpack
   drain → preparation drain → trigger): capture_trigger_awards_ranger_award_xp,
   capture_does_not_heal_building_to_full, infantry_capture_prep_stamps_raising_flag (RAISING_FLAG is
   stamped at startPreparation, special_abilities.rs:2304, not at Unpacking).

5. **C++-faithful behavior flips (3 fixed, test-side).**
   - evacuate_command: exit door walk (OpenContain::exitObjectViaDoor → aiFollowPath,
     NumberOfExitPaths defaults to 1, OpenContain.cpp:54) ⇒ exiting rider is Moving, not Idle.
   - script_radar_event: C++ doRadarCreateEvent (ScriptActions.cpp:2842) is `TheRadar->createEvent` ONLY —
     no "Under attack" UI text; test now asserts the radar system's last-event location.
   - host_ai_rebind: `rebuild_count` is PRESERVED across rebind (map load ≠ combat loss) — pinned by
     ai::cpp_parity_tests rebind_after_world_reset_keeps_difficulty_active_and_remaining_rebuilds.
     Stale `== 0` assertion replaced by preserve-contract + realistic partial-budget seed.

6. **Production fix (1): seeding.rs audio-only bypass.** `should_skip_map_object_template` ran before the
   SoundAmbient-only seed path, so `Ambient*`-prefixed audio-only map objects could never become live
   templates — defeating seeding.rs's own documented purpose (Drawable startAmbientSound). Fix: audio-only
   definitions (no model + SoundAmbient) bypass the skip list in the SEED path only;
   `should_spawn_fallback` keeps the full list. asset_template_catalogue_seed now green.

7. **dozer_line (partial).** Owner stamps (create_object_for_player — exact-controller gate in
   place_line_build_segment) + Moving-approach expectation (C++ line build places scaffolds then dozers
   walk; AI_CONSTRUCT only on arrival) + segment-ownership dest asserts now pass; LAST failing assert is
   `created_structures == 2` (:1796) — second line tile is refused by build-location legality. Next step:
   probe `is_location_legal_to_build_for_builder` for the second tile at spacing
   (TestBuilding geometry default vs tile spacing 20wu at (10,10)→(30,10)).

## REMAINDER (7 stable) — precise diagnosis for the next session

- **hacker_disable_prep_stamps_firing_a** (host_queries:2512, None vs Some(Preparing)): channel never
  starts. Players are owner-stamped; executor gates traced OK by reading (metadata present, relation
  Enemies via team fallback, capturable target). Needs a runtime probe of
  `can_unit_hacker_disable_building` + `unit_command_begin_hacker_disable_building` + the pending drain
  (update.rs:2192 region) — suspect `consume_special_power_charge_for`/shroud gate in the drain, or the
  Approaching→Preparing transition refusing (target `is_effectively_stealthed`/structure check in
  resolve). SHARED with PilotsUnitsTriage (their unit_residuals hacker_disable_building ×2 + disable_vehicle
  hang on the same channel) — protocol agreed: whoever finds the root cause posts it on hub.
- **infantry_capture_requires_completed_capture_upgrade_when_player_exists** (host_queries:1980, Idle vs
  Capturing): upgrade applied + clock advanced 15.1s, but second CaptureBuilding still refused. Suspect
  `pause_special_power_countdown(..., true)` never unpauses via `apply_upgrade_to_object` — C++ unpause is
  upgrade-completion-driven. Probe `is_hacker...`-style readiness for RangerCaptureBuilding
  (`consume_special_power_charge_for`) after the upgrade.
- **propaganda_tower_residual_subliminal_upgrade_buff_and_faster_heal** (scripts:981): scan/enthusiastic
  now land (frame bump added); heal amount after 30×1/30s ticks < before+2.5. Pulse-economy trace needed:
  how many heal pulses fire in 1s with subliminal 4%/s (expect ~one 3.2HP pulse).
- **entering_state_docks_unit_into_transport_when_close** (host_queries:1308): TestTransport container
  now authored but rider still not registered. Probe `can_unit_enter_normal_target` /
  `normal_enter_available_capacity_for` (slots=Some(1) vs max_transport=2 mismatch?) and the dock-range
  gate in update.rs Entering branch (unit at 2wu).
- **retail_pilot_metadata_drives_starting_veteran_and_same_owner_recrew** (scripts:2347, Entering vs
  Idle): recrew-phase Enter order into the vehicle stays Entering — same dock/contain arrival family as
  entering_state_docks; retail metadata parse itself is fine (earlier expects pass).
- **host_ai_rebind_after_world_wipe** (host_queries:846): with realistic partial budget the soup assert
  still fails — AI places nothing in 20 `logic.update()` ticks after rebind (templates reinstall ✓, cash ✓,
  active ✓). Trace ai_manager tick inside logic.update + first-build gate (next_building_time, layout
  anchor when no structures exist post-wipe).
- **dozer_line** scaffold-count tail (above).

## Verified green after fixes (serial)
- 14 originally-failing tests fixed: the 8 frame-stamp repair/heal tests, war_factory, get_repaired
  aircraft, both hijack tests, hunt map-wide, radar event, 3 capture-channel tests, evacuate, rebind
  (contract realigned; soup tail still open), asset_template_catalogue (production seeding fix).
- Suite total moved 82 → 96-98 passing across runs (2 cross-test flake-class tests oscillate).

## Sibling coordination
- CmdAiRemainder: airfield.rs entry-gate + ui_commands/remove_command_button + wave519 — no overlap with
  my repair/capture edits. get_repaired_command_aircraft was confirmed mine; now fixed.
- PilotsUnitsTriage: hacker disable channel shared — protocol: first finder posts fix; my probe notes above.
- SmallRemainderTriage: fixed transient E0425 in world_tick/production.rs mid-run; noted dozer_line as mine.
- PilotsUnitsTriage also fixed a transient in command_executor/special_power.rs (tnt probes).

---


Serial runs only (`--test-threads=1`), no git writes, no formatters. All probes removed
(command_executor/abilities.rs and support_states/update.rs verified byte-identical to their
pre-probe session snapshots after removal).

## Closed this round (4, each verified green serially in the rebuilt binary)

1. **dozer_line_assigns_each_worker_a_segment — FIXED (production + fixture).**
   Two stacked causes, both C++-grounded:
   (a) Production: `execute_dozer_line` (command_executor/construct.rs) placed scaffolds but
   never assigned the selected workers segment walks — the old dest asserts passed only by
   accident of the construction *scoot* overwriting worker targets
   (`move_objects_for_construction` shoved both dozers when placing scaffold 1, and the shove
   destination accidentally satisfied dozer A's ownership assert). Fix: `place_line_build_segment`
   now returns `Result<ObjectId, CommandResult>` and `execute_dozer_line`, after placing all
   scaffolds, assigns each selected worker (`can_construct` + exact controller) one segment:
   order target → `dozer_new_task_build` → `worker_exit_supply_for_dozer_task` → walk leg via
   `find_good_build_or_repair_position` (DozerAIUpdate.cpp:1855-1894 source-seeded
   findPositionAround) + `path_to_goal_with_state_ignoring(worker, approach, AIState::Moving,
   Some(building))`. C++ oracle: buildObjectLineNow (BuildAssistant.cpp:430-454) places every
   scaffold first (legality checked per-tile inside buildTiledLocations :1173-1181, NO re-check
   at placement — buildObjectNow :309-424 only shoves movables); dozers then WALK (AI_MOVE ⇒
   Moving) and flip to AI_CONSTRUCT only on arrival (DOZER_DO_BUILD_AT_DOCK cpp:499-507).
   (b) Fixture: TestBuilding authored no Geometry. C++ line tiling is geometry-driven
   (`objectSize = majorRadius*2`, BuildAssistant.cpp:441); retail structure templates always
   author Geometry rows. The host's unauthored-template place-radius residual (20wu) made two
   20wu-spaced tiles overlap in every placement gate (place_r 20 + neighbour r 20 > 20). The
   test now authors the retail-default 10wu cylinder footprint so tiles sit at majorRadius*2 =
   20wu, exactly touching (dist 20 is NOT < 10+10, C++ isLocationClearOfObjects).

2. **hacker_disable_prep_stamps_firing_a — FIXED (production + test).** Root cause found for
   the shared HDB channel (posted to Main per the agreed protocol; unblocks the unit_residuals
   hacker_disable ×2 class): `hacker_disable_building_channel_has_enemy_relation`
   (special_abilities.rs:2404) lacked the C++ default-hostility fallback that the click gate
   `can_unit_hacker_disable_building` (object_queries.rs:2399-2405) applies. With two
   owner-stamped players and no diplomacy rows, the click passed (`can=true`) but the running
   channel read strict player NEUTRAL → packed itself on the first tick. Fix: same
   Neutral→Enemies non-neutral-different-team fallback added (C++ Object::getRelationship,
   Object.cpp:1548-1568 — two living players on different non-neutral teams are ENEMIES).
   Test-side: the channel then honestly sits Approaching for the 180° need-to-face turn
   (C++ SpecialAbilityUpdate orders NeedToFace ahead of startPreparation; the established
   facing-slice class) — replaced the single `update_ai` tick with a bounded 600-frame
   completion loop.

3. **infantry_capture_requires_completed_capture_upgrade_when_player_exists — FIXED (fixture).**
   Two fixture-fidelity gaps: (a) TestInfantry authored `capture_power = Ranger` but not the
   capture upgrade trigger, so the C++ UnpauseSpecialPowerUpgrade residual in
   `apply_upgrade_to_object` (object_queries.rs — exact `TriggeredBy` match) never unpaused the
   paused RangerCaptureBuilding power. Retail oracle: AmericaInfantry.ini:165156-165159
   `UnpauseSpecialPowerUpgrade / SpecialPowerTemplate = SpecialAbilityRangerCaptureBuilding /
   TriggeredBy = Upgrade_InfantryCaptureBuilding`. Fixture now authors
   `capture_upgrade_trigger = Some("Upgrade_InfantryCaptureBuilding")` (helpers.rs).
   (b) Player 0 was `is_local`, so the C++ local-human shroud authority
   (ActionManager.cpp:76-102) failed closed on the FOW-less fixture before the upgrade contract
   under test — dropped is_local (same class as the pilots BlackLotus capture fixture).


4. **entering_state_docks_unit_into_transport_when_close — FIXED (fixture).**
   VERIFIED GREEN in the final serial suite. Fixture corrected on two counts (both C++
   TransportContain.cpp:105,183-186 slot arithmetic): capacity 4 (the TestTank rider authors
   TransportSlotCount 3 — a 2-slot transport refuses it) and a registered USA player so the
   exact-controller gate (`normal_enter_controller_matches`; the ownerless same-team branch
   needs `unique_player_id_for_team`) can match. Probe evidence: with capacity 4 alone the
   arrival gate still returned false purely for missing player registration.

## Remaining (documented; not reached this round — timebox)

- network_and_scripts: `propaganda_tower_residual_subliminal_upgrade_buff_and_faster_heal`
  (heal-pulse economy trace), `host_ai_rebind_after_world_wipe` (ai_manager first-build gate
  post-wipe: next_building_time / layout anchor), `retail_pilot_metadata...` (Entering vs Idle —
  same dock/contain arrival family as entering_state_docks; likely closes with the same
  registered-player controller provenance applied to its recrew-phase Enter), plus the 2
  documented cross-test flake-class tests (both passed in this session's baseline run).
- pilots ×2, unit_residuals ×13, construction-area ×16: unchanged from the triage reports;
  the HDB channel production fix above is the shared root cause for the hacker_disable ×2
  class and should be applied/verified there next.

Changed surfaces: `command_executor/construct.rs` (line-build worker assignment),
`world_objects/support_states/special_abilities.rs` (channel relation fallback),
`world_tests/network_and_scripts/{scripts_and_capture,host_queries_and_visibility}.rs` +
`world_tests/helpers.rs` (fixture fidelity: geometry authoring, capture upgrade trigger,
transport capacity/player registration, bounded facing loop).

---

# Final20c — unit_residuals closure (UnitResFix)

Serial runs only (`--test-threads=1`), no git writes, no formatters, probes removed
(grep PROBE clean over all touched files).

## Result: unit_residuals 39/52 → 52/52 GREEN serially

(The 2 hacker_disable ×2 were already green at baseline from WorldContinuation's
special_abilities.rs relation fallback + the pilots' bounded-facing-loop conversion;
my baseline measured 41/52 — the 2 detect/capacity crawler tests closed below plus
the 2 hacker ones were the difference from the recorded 39.)

### 1. weapon cluster ×5 (ranger/missile_defender/rpg/dragon/gattling/minigunner) — test-side

Established seed-vs-retail-INI class: fixtures asserted the bootstrap seed constants
minus an invented "−2.5 range residual", while production resolves the loaded retail
INI. All expected values rebased on Weapon.ini (extracted_big_files_v2):

- ranger (:43-62): RangerAdvancedCombatRifle AttackRange 100.0 (Weapon.ini:129508);
  flashbang AttackRange 175 / MinimumAttackRange 20 (Weapon.ini:131314-131315).
  Additionally the rifle-vs-vehicle damage assert was corrected to the retail armored
  amount 5 × 25% = 1.25 (Armor TankArmor SMALL_ARMS 25%, Object/FactionUnit.ini:441;
  ActiveBody::attemptDamage applies the armored amount) — previously masked because
  the range assert failed first.
- missile_defender (:892,:898,:1007): MissileDefenderMissileWeapon range 175
  (:129551) and laser-guided range 300 (:129576), both via the host constants
  MISSILE_DEFENDER_PRIMARY_RANGE/LASER_RANGE which already match retail.
- rpg (:631,:635): TunnelDefenderRocketWeapon AttackRange 175 (:130798),
  MinimumAttackRange 5 (:130800).
- dragon (:144): DragonTankFlameWeapon AttackRange 75 (:131684) via DRAGON_RANGE.
- gattling (:318): GattlingTankGun AttackRange 150 (:129891).
- minigunner (:1746): Infa_MiniGunnerGun AttackRange 125 (:135005).

### 2. troop_crawler ×2 (test-side + one small production honesty channel)

- detect_stealth: the crawler's StealthDetectorUpdate scan was simply not due at
  frame 1 (probe evidence: DetectionRate 27, next-scan ≈ frame 6 at creation).
  C++ scans on its authored cadence, not on the first tick. Test now bounded
  frame-advances until detection (≤60 frames) and drives the fail-closed far-enemy
  check through a full extra cadence (frames 60-120) so it can no longer pass
  merely because no scan ran.
- transport_load_unload: riders exit through the TransportContain exit-door stream
  (C++ TransportContain.cpp exit-busy + AIExitState poll; established in Final28b).
  Test now drains `pending_stream_exit` via drain_pending_transport_exits_for_test
  after both evacuate commands before asserting, and accepts the C++-faithful
  Moving state of an exiting rider (OpenContain::exitObjectViaDoor → aiFollowPath).
  Production addition: registries.rs evac now records the Troop Crawler
  hull-specific unload honesty (`record_troop_crawler_residual_unload`,
  container_is_troop_crawler flag) mirroring the existing Combat Chinook / Battle
  Bus hull-specific branches — a crawler evac is not a generic transport unload;
  without it the test's unload honesty counter could never advance.

### 3. wave_guide ×2 — PRODUCTION FIX (special_power_strikes.rs dam-die fallback)

Root cause for both: with no authored WaveGuide1 terrain path, the dam-die bind
fallback set `final_destination = Some((0,0))` — the wave's own spawn point — so on
the first moving tick `reached_destination` fired, the guide was marked done and
destroyed before ANY motion, flood damage, or addWaterVelocity push. C++ ground:
WaveGuideUpdate.cpp:795-822 compares the wave against the LAST WaveGuide1 waypoint;
with no path there is no destination to reach. Fix: the MissingWaypoint/None arm
now leaves final_destination unset (wave keeps moving along its authored facing);
InvalidPath still marks done. Both tests (flood-damage kill at range, and
addWaterVelocity impulses carrying WAVE_WATER_VELOCITY + PreferredHeight 40) are
green with zero test-side changes.

### 4. Production fix #2 for the cluster — weapons.rs chooseBest estimate

`estimated_slot_damage_vs` now estimates from the weapon STORE template's
PrimaryDamage (C++ WeaponSet.cpp:847-851 estimates the template, not a host-stamped
Object copy). Retail STATUS paint weapons (AvengerTargetDesignator, PrimaryDamage
200 = status duration) are stamped onto objects with damage 0 by the honest
no-HP-damage paint policy; that zeroed copy was feeding chooseBest's zero-damage
elimination and silently eliminating the designator slot, so the Avenger could
never fire/paint against ground targets (probe evidence: slot0 damage 0.0,
select_combat_weapon_slot None). The avenger designator paint/ROF residual test
now passes; "designator deals no HP damage" contract is unchanged (the paint
branch applies status only).

## Guards (serial)

- unit_residuals 52/52.
- Guard sweep `avenger transport gattling minigunner`: 272/274. The 2 failures
  (`presentation_frame::apply_honesty::{empty_transport_exit_slot_starts_disabled,
  transport_exit_slots_bind_occupant_portraits}`) are BASELINE-PROVEN pre-existing:
  with my three production files stashed to HEAD they fail identically (empty
  occupant-portrait binding, the documented final20b presentation_frame
  env/channel class; sibling-owned).

Changed surfaces: `world_tests/unit_residuals/{infantry_and_transport,
projectiles_and_vehicle_residuals}.rs` (fixture/contract rebases),
`world_scripts/special_power_strikes.rs` (pathless dam-die destination fallback),
`object/weapons.rs` (template-damage chooseBest estimate),
`world_combat/registries.rs` (crawler hull-specific unload record). No probes left.

---

# Final20c — network_and_scripts 3 + pilots 2 + construction-area 16 closure (NetPilotsFinish)

Serial runs only (`--test-threads=1`), no git writes, no formatters. All probes removed
(including a stray `PROBE steal loop` eprintln left in pilots_and_movement/
hacker_and_special_movement.rs by an earlier session — removed, loop logic restored).

## 1. network_and_scripts 3 → 3/3 GREEN

1. **propaganda_tower_residual_subliminal_upgrade_buff_and_faster_heal** — FIXED (fixture).
   C++ PropagandaTowerBehavior::effectLogic:275 reads
   `getControllingPlayer()->hasUpgradeComplete(m_upgradeRequired)` — the subliminal upgrade
   lives on the tower's controlling PLAYER, never on the tower object (production
   `update_propaganda_tower_pulse` reads owner→player completed_upgrades, gps_and_fields.rs:1045-1050,
   oracle-faithful). The old fixture ownerlessly tagged the tower object. Fix: registered China
   player (ensure_test_player_for_team), `completed_upgrades.insert(Upgrade_ChinaSubliminalMessaging)`,
   tower created via `create_object_for_player`. 4%/s heal lands (3.2 HP/s > base 1.6).

2. **host_ai_rebind_after_world_wipe_keeps_players_cash_and_rebuilds** — FIXED (fixture, C++-grounded).
   Trace: after `objects.clear()` the soup can never start — ai/economy.rs `queue_dozer`
   (C++ AIPlayer.cpp:3128-3171 queueDozer→findFactory) needs a live CommandCenter and
   `process_building_queue` needs a dozer; a fully wiped world deadlocks both. A real map load
   restores the map ObjectLists (faction CC + starting worker), so the fixture now restores
   GLA_CommandCenter + GLAInfantryWorker at the AI layout anchor (rebind_contract asserts run
   BEFORE the restore; relocate_base skips on empty world so rebuild_count==1 survives).
   `next_building_time`/timers were NOT the blocker (rebind already zeroes them).

3. **retail_pilot_metadata_drives_starting_veteran_and_same_owner_recrew** — FIXED (test contract
   realigned to the C++ oracle). Real first failure was :2381 (foreign-tank Enter accepted, expected
   Idle). C++ ground: `canEnterObject` "Special case for unmanned vehicles. Any infantry unit can
   take over any unmanned vehicle!" — KINDOF_INFANTRY + DISABLED_UNMANNED + !REJECT_UNMANNED → TRUE
   with NO controller check (ActionManager.cpp:549-557); the same-controller requirement is the
   PILOT flavor only (VeterancyCrateCollide.cpp:74-79
   `other->getControllingPlayer() == getObject()->getControllingPlayer()`). The host implements
   exactly this split (`can_execute_pilot_recrew` vs `can_execute_infantry_unmanned_recrew`);
   the test now asserts both query gates for the foreign and name-only blocks (no order installed,
   so the later same-owner recrew flow is untouched) and the end-to-end recrew passes.

## 2. pilots_and_movement 2 → 2/2 GREEN

4. **black_lotus_capture_building_without_upgrade** — FIXED (fixture). The capture machine
   (support_states/update.rs:1548-1563) aborts Capturing on the first in-range tick when the
   template authors NO capture timings ("partial/unsupported parse must not invent a
   zero-duration capture ability"). Retail ModuleTag_09 (ChinaInfantry.ini:164817-164829):
   StartAbilityRange 150 / UnpackTime 6730 / PreparationTime 6000 / PackTime 2800 — fixture now
   authors all four. Also: the out-of-range tick's stale approach leg is settled with
   `stop_moving()` before the teleport (C++ SpecialAbilityUpdate.cpp:209-220 aborts unpack/prep
   while moving), and the bounded loop now drives through PackTime (startPacking keeps
   AIState::Capturing until pack ends, update.rs:1812-1836) before the "captor leaves Capturing"
   assert.

5. **flashbang_grenade_bezier_flight_and_blast** — FIXED (test contract realigned to retail armor).
   The blast target was a TestTank: retail RangerFlashBangGrenadeWeapon is DamageType SURRENDER
   (Weapon.ini:2535-2556) and TankArmor authors SURRENDER 0% (Armor.ini:139) — a vehicle takes
   ZERO flashbang HP by design (ActiveBody.cpp:509-527 applies the armored amount); infantry
   armors are 100%. Same class SmallRemainderTriage already corrected in scatter_and_chain.
   The test now keeps the tank (asserts HP unchanged, armor citation) and adds an enemy infantry
   splash victim that must lose HP.

## 3. construction-area 16 → 16/16 GREEN (narrow sweep 292 passed / 0 failed serial)

`dozer_line_assigns_each_worker_a_segment` was already green from WorldContinuation's
execute_dozer_line rewire. The remaining 15, all baseline-proven pre-existing:

- **wave819 dozer bored dual peel ×4** (live/pack/source_markers/sources) — scan repoint: the
  GL-side stamp `gl.contains("Wave 819")` is dead (the bored peel lives unstamped in
  world_scripts/rebuild_dozer.rs:1466/1513, which IS in the harness concat). Repointed both
  checks to the live token `DOZER_BORED_TIME_FRAMES` (rebuild_dozer.rs:1515) — same class as
  the final20c wave519 repoint.
- **cnc_game_engine source_scan ×2** —
  `resume_construction_hotkey_residual`: the pf assert demanded `Command_ResumeConstruction` in
  the strip — retail has NO resume button: populateUnderConstruction
  (ControlBarUnderConstruction.cpp:57-66) shows Command_CancelConstruction ONLY; resume is the
  ACTIONTYPE_RESUME_CONSTRUCTION cursor/click surface (InGameUI.h:318) dispatched as
  MSG_RESUME_CONSTRUCTION (GameLogicDispatch.cpp:1135-1147), pinned engine-side (Alt+E arm) and
  by the executor MSG residual. Repointed to Cancel-only parity.
  `construction_cameo_hotkey_priority_residual`: hud.rs declares `pub enum ConstructionTab` with
  bare variants — `ConstructionTab::Aircraft` as a qualified path only exists in the engine cycle
  array (selection.rs). Widened: hud must declare enum+Aircraft variant; ENGINE_SRC must carry the
  qualified Aircraft path.
- **ui hud construction_panel cameos** — `construction_panel_does_not_invent_faction_cameos` was
  self-matching: `include_str!("hud.rs")` includes the test module's own assertion literals
  ("fn add_infantry_units" etc.). Now scans the production slice (split at `#[cfg(test)]`).
- **presentation_frame construction/dual_tick/render_overlay ×4 + graphics frozen_fow** (ScanFix +
  PresFix agents, each verified green solo + module serial, zero new failures):
  - dual_tick_registry ×2 + frozen_fow: stale source-scan literals repointed to live tokens
    (`presentation_ocl_timer_seconds` fallback / `sync_structure_context_from_presentation` clamp
    in control_bar impl; world_tick/production.rs sole-tick tokens; live ghost-aware frozen-FOW
    guard `if !is_ghost && item.fow_visibility.visibility_alpha <= 0.01` forward_render.rs:682).
  - under_construction_keeps_cancel_when_script_disabled: fixture gap — `GameLogic::new()`
    registers NO players, so the selection write no-oped and the ownerless object failed
    `is_owned_by_local` (legacy fallback needs a live local team) → empty strip before the UC
    branch. Registered player 0 + `create_object_for_player` owner stamp (production
    oracle-faithful; C++ populateUnderConstruction keeps Cancel).
  - presentation_freezes_dozer_construct_can_make_cameos: prereq gate — the static residual table
    (host_production_buildable_command_residual.rs:1355-1359) authors
    AmericaPowerPlant ← AmericaCommandCenter when neither leftover factory nor live template has
    prereqs; retail FactionBuilding.ini authors NO Prerequisites line, so a satisfied scan is
    oracle-equivalent to CANMAKE_OK. Fixture adds a constructed player-owned CC + owner-stamped
    dozer.
- **gameworld_shadow economy_construction ×3** (ShadowFix agent, fixture-only, production
  oracle-faithful): the session's trailing `shadow.probe()` runs
  `GameLogic::evaluate_victory_condition()` which — per C++ VictoryConditions.cpp:87-95/168-196 —
  marks army-less playable players defeated on frame 0-1 and `kill_player_for_victory` zeroes
  supplies exactly like C++ Player::killPlayer (`m_money.withdraw(m_money.countMoney())`,
  Player.cpp; SP-computer resurrect does not apply to skirmish). Fixtures now register an
  MpCountForVictory keep-alive structure (sell_heal.rs precedent) so assertions observe the
  economy materialization. exit_delay test additionally arms the opt-in
  GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY=1 gate it asserts on (default off per
  tick/authority.rs:234-242).

## Guards (serial)

- Narrow construction sweep (`resume_construction dozer construction queue_exit
  under_construction wave819`): **292 passed / 0 failed** (was 277/15).
- network_and_scripts + pilots_and_movement: **181/181** (was 176/5).
- capture_and_containment + transport_tests: **64/64** (helpers.rs-carryover guard intact).
- Combined gate (network_and_scripts pilots_and_movement capture_and_containment
  transport_tests wave819 source_scan_tests): my named targets all green; the extra
  source_scan_tests failures are the pre-existing out-of-scope set present in the identical
  pre-edit wide sweep (windowed smoke env-class + sibling-concurrent surfaces, not part of this
  ticket's 16).
- No production code changed by this pass except... none: all production edits this round are
  scan/test realignments; production verified oracle-faithful at every diagnosed gate (C++
  citations above). Changed surfaces: world_tests/{network_and_scripts,pilots_and_movement} test
  files, game_logic/residuals/host_live_host_dozer_bored_dual_peel_residual_wave819.rs (scan),
  cnc_game_engine/source_scan_tests.rs (2 scans), ui/hud.rs (self-scan slice),
  presentation_frame/tests/{apply_honesty,render_overlay,dual_tick_registry}.rs,
  graphics/render_pipeline/tests.rs, gameworld_shadow/tests/economy_construction.rs.
- Coordination: UnitResFix owns unit_residuals only (no overlap); PresFix/ScanFix/ShadowFix
  subagents file-scoped as listed; stray probe in hacker_and_special_movement.rs:672 removed.

---

# Final20c — gameworld_shadow closure (ShadowClose)

Serial runs only (`--test-threads=1`), no git writes, no formatters. All probes removed
(create_destroy_die.rs and host_ops_writeback.rs restored; test-side probe blocks removed).

## Re-enumeration: live failure set changed from the ticket's 15

The documented 15 (economy_construction x3, fire_damage x3, presentation x3, sell_heal x5)
were already closed by siblings (final20c ShadowFix/NetPilotsFinish items above). The live
serial set was 14 DIFFERENT failures (stable across runs, fail in narrow isolation too):

authority_writeback x6, continue_attack x4, entity_modules x1, sell_heal scan x1,
sync_ids x2.

## Closed this pass (10, each fix C++-cited)

1. `experience_authority_defers_host_xp_until_writeback` + 2.
   `host_experience_log_drives_set_experience_channel` — fixture stamps
   `is_trainable = true`. C++ `ThingTemplate` ctor defaults `m_isTrainable = FALSE`
   (ThingTemplate.cpp:994); retail player-built infantry author
   `IsTrainable = Yes` (AmericaInfantry.ini:163029). `gain_experience` fails closed on
   untrainable (update.rs:818-822, C++ addExperiencePoints) — honest; fixtures now
   retail-faithful.
3. `host_update_movement_skips_when_gameworld_movement_authority` — backtrace-proven
   root cause: the session's trailing `probe()` (apply_host_damage.rs:498 →
   session.rs:2590) runs `evaluate_victory_condition` (game_logic/mod.rs:112) →
   `kill_player_for_victory` (presence.rs:249) — the skirmish NO_BUILDINGS rule defeats
   the structure-less USA player on frame 0-1 (C++ VictoryConditions.cpp:87-95/168-196)
   and destroys the unit mid-test. Fixture seeds the established MpCountForVictory
   keep-alive structure (sell_heal.rs / economy_construction.rs precedent).
   Production oracle-faithful.
4. `production_authority_defaults_on` → rewritten as
   `production_authority_defaults_off_host_sole_writer` (+ authority_env_lock). The
   default-on premise diverged: tick/authority.rs:234-242 pins "Opt-in ... Production
   default off", and the wave177 residual pins it twice
   (honesty_production_authority_default_on_source requires `false` in the gate body;
   simulate_gameworld_production_authority_honesty asserts enabled()==false after
   ensure_gate_damage_authority).
5. `attack_target_writeback_updates_host` — arms the opt-in
   GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY=1 under authority_env_lock: the writeback IS
   the AI-attack last-writer channel, gated at writeback_core.rs:295 (production
   default off = host sole writer, matching the C++ logic/client split). Same pattern
   as the passing continue_attack decision-authority tests.
6. `entity_modules_default_on_installs_live_instances` →
   `entity_modules_armed_installs_live_instances` — arms
   GENERALS_GAMEWORLD_ENTITY_MODULES=1 via AuthorityEnvGuard. Wave153 residual pins the
   contract: "preview ENTITY_MODULES is default off"
   (host_gameworld_authority_residual_wave153.rs:48); authority.rs:268-275.
7. `stale_engine_id_does_not_skip_host_movement` — same victory-kill class as (3)
   (update_with_dt evaluates victory); keep-alive added at (300,0,300) (far: a nearby
   structure's deferred clearance-shove displaced and stopped the unit) plus
   `movement.acceleration = 240` (C++ Locomotor::getMaxAcceleration; retail locomotors
   author Acceleration — the host integrate velocity-ramps by acceleration*dt, so a
   0-accel fixture never builds velocity; physics.rs effective_acceleration).
8. `command_attack_range_snap_movement_authority_source` (sell_heal) — scan repoint
   (established pattern): the Final20b jet rework extracted the RTB approach legs into
   `assign_rtb_path` (JetAIUpdate.cpp:1536-1541, RETURNING_FOR_LANDING issues the
   move), so `assign_unit_path` left `try_return_to_base_rearm`'s brace-matched body.
   Scan accepts `assign_rtb_path` and now explicitly pins that the helper routes
   through movement-authority-gated `assign_unit_path` — honesty preserved.
9. `completed_production_waits_for_open_door_before_entity_first_spawn` — producer
   renamed to retail "AmericaBarracks" (RETAIL_PRODUCTION_DOOR_INI:514 — 1 door, 0/0/0
   frames). The invented "DoorGateBarracks" resolved 0 doors, so
   `production_door_allows_spawn(0, phase)` (host_production_buildable_command_residual.rs:648)
   always allowed spawn and the C++ door-cycle gate (ProductionUpdate spawn waits for
   WAITING_OPEN) never engaged.
10. `same_faction_slots_keep_owner_authority_through_shadow_and_presentation` —
    selection premise realigned to the C++ oracle: GameLogic::selectObject
    (GameLogic.cpp:2595-2641) filters by playerMask ONLY — no owner predicate — and the
    host `select_objects` documents exactly that (host_ops_writeback.rs:9-17). Test now
    asserts selection accepts both; the authority boundary is the COMMAND path
    (`command_move` exact-controller filter, host_ops_writeback.rs:232-236), which the
    remaining asserts still pin (stale selection must not move the opponent's unit).

## Measured state (final full serial run of the module)

`cargo test -p generals_main --lib -- gameworld_shadow:: --test-threads=1`:
**281 passed / 4 failed** (was 271/14). Progress +10 closed.

## Documented remainder (4, precise)

1. `continue_attack::missile_defender_laser_guided_decision_authority` — decision log
   still empty after the bounded `update_support_states_for_test` drive. The engage
   (and log) lives in `update_leftover_laser_guided_channels`
   (missile_defenders.rs:368-382 → engage_target_decision_aware:946, gated on
   `gameworld_ai_decision_authority_live()`). Next probe: dump the LeftoverSaChannel
   phase/remaining after each tick and `weapon_slot(1)` at the engage gate — suspect
   the channel never reaches zero remaining (timings) or `weapon_slot(1)` is None
   (slot indexing vs `secondary_weapon` field).
2. `continue_attack::mood_auto_acquire_logs_decision_under_authority` — fixture now
   authors `auto_acquire_idle_bits |= AUTO_ACQUIRE_IDLE` (mood.rs:453 gate) and
   `vision_range = 150` (mood.rs:518-521 gate) but still no log. Next probe: which of
   mood_allows_attack / cannot_possibly_attack_object / choose_best_weapon_for_target
   (attack.rs:196-207) refuses — suspect choose_best_weapon needs authored
   damage/range consistency or the team-common-target gate at mood.rs:490-507.
3. `continue_attack::movement_authority_integrates_host_when_shadow_disabled` — still
   dist=0 with mapless compute-now path + accel 240 + max_speed 60 (single full tick of
   update_movement). The sibling `stale_engine_id` test (same integrate) marches with
   the identical stamps, so the remaining delta is its `assign_unit_path` fixture:
   probe `apply_computed_unit_path`'s returned path/waiting_for_path and
   `effective_max_speed()` (group/battle-plan scalars may zero it on a fresh object).
4. `sync_ids::writeback_production_and_rally_to_host` (:1021 "writeback must touch
   building") — NEW collateral, first seen after the (9) rename: the door test now
   completes through the real retail door path and leaves process-global
   production/door log state that the next producer test observes
   (`host_production_log` pending guard at writeback_production.rs:43-47 skips its
   writeback). Fix direction: drain `host_production_log`/door logs at the door test's
   end (the logs are thread-local process-global; established clear-at-boundary
   pattern), or clear at the rally test's start.

Guards: not re-run module-wide beyond gameworld_shadow (budget); production code
touched by this pass: NONE (all fixes test-side except zero production edits — probes
removed, verified). Changed files: gameworld_shadow/tests/{authority_writeback,
continue_attack,entity_modules,sell_heal,sync_ids}.rs.

# Final20c — gameworld_shadow remainder closure (ShadowFinal4)

Serial runs only (`--test-threads=1`), no git writes, no formatters. Probes removed.

**Result: `cargo test -p generals_main --lib -- gameworld_shadow:: --test-threads=1` = 285 passed / 0 failed.** All 4 documented remainder failures closed.

## Closed this pass (4)

1. `continue_attack::missile_defender_laser_guided_decision_authority` — **production
   bug, fixed** (game_logic/world_objects/support_states/update.rs, `AIState::SpecialAbility`
   arm). The no-pending-ability cleanup called `obj.set_target(None)` unconditionally;
   `Object::set_target(None)` (object/orders.rs:43) force-Idles the unit, and the
   frame-802 new-order gate (`ai_state != SpecialAbility` + no Attacking-persist) then
   aborted the live LaserGuided channel one tick after activation — before prep ever
   reached triggerAbilityEffect. Empirically proven with a channel-only drive
   (`update_leftover_laser_guided_channels` alone: engage at t30, decision log + weapon
   lock + persist channel all correct) vs the full support drive (channel dead at tick 2,
   lock fingerprint `set_weapon_lock(0, LockedTemporarily)` from
   `leftover_kill_special_objects`→`leftover_reset_laser_primary`). The command path
   (command_executor/special_power.rs:643-648) inserts NO pending ability for
   MissileDefenderLaserGuided, so every real prep was killed the same way — the entire
   prep/engage/persist machinery was unreachable via gameplay. Fix: the generic cleanup
   now skips units with a live leftover channel (C++: a leftover channel IS an active
   SpecialAbilityUpdate module — GameLogic.cpp:3677-3718 UpdateModules; prep/onExit owned
   by the module itself, SpecialAbilityUpdate.cpp:1276-1293; the sibling Attacking-case
   persist guard at update.rs:79-108 already acknowledged this for the post-engage phase).
2. `continue_attack::mood_auto_acquire_logs_decision_under_authority` — fixture
   realignment, three retail preconditions: (a) `set_ai_state(Idle)` — mood auto-acquire
   is an Idle-state scan (mood.rs:601 eligible gate; the
   `try_mood_auto_acquire_enters_attack` world-test precedent sets it explicitly);
   (b) `last_fire_time: -10.0` — `AIAttackState::chooseWeapon`
   (choose_best_weapon_for_target → select_combat_weapon_slot slot-ready check) only
   considers past-reload slots, and a frame-0 fixture with last_fire_time 0 / reload 1
   has none; (c) shroud `Clear` stamp for the owning local player — human idle
   auto-acquire adds the UNFOGGED qualifier (AIUpdate.cpp:4608-4619
   PartitionFilterFreeOfFog residual, mood.rs:547-549 + 244-260) and no vision pass runs
   in a raw fixture.
3. `continue_attack::movement_authority_integrates_host_when_shadow_disabled` —
   fixture rewritten onto the proven sibling recipe
   (`stale_engine_id_does_not_skip_host_movement`): MpCountForVictory keep-alive (the
   NO_BUILDINGS victory rule kills structure-less players on frame 0-1,
   VictoryConditions.cpp:87-95/168-196), hand-authored locomotor path
   (twin-precedent), `update_with_dt` drive. Diagnosis trail: `assign_unit_path` +
   first-tick `update_movement_for_test` in a fresh MAPLESS GameLogic freezes the unit
   permanently (velocity zeroed, idx advanced to 1, pos unchanged; probe-isolated — the
   identical assign marches once any earlier path-carrying tick has run). Mechanism: the
   computePointOnPath lead fallback (AIPathfind.cpp:910-950) returns the current position
   while the mapless grid cannot validate the far-lead line, so direction==0 zeroes
   velocity at movement.rs:1005 and the Locomotor.cpp:1393-1430 approach brake holds
   goalSpeed at 0 on distAlongPath 0. Mapless is a startup/test affordance; production
   maps load terrain and the queue defers one frame (AI.cpp:332-339), so this is a
   test-mode artifact, not a production bug.
4. `sync_ids::writeback_production_and_rally_to_host` — the real mechanism was NOT the
   pending-log leak: the test never armed `GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY=1`,
   and `writeback_production_to_host` returns 0 without it (writeback_production.rs:11).
   It had silently passed on the OLD door test's env leak (a failing test panics past its
   restore block at sync_ids.rs:1192-1197, leaving =1 behind); once the AmericaBarracks
   rename fixed the door test, its clean restore starved this test. Fix: arm the env
   under `authority_env_lock()` with restore (attack_target_writeback pattern), clear
   `host_production_log` at the start, and drain it at the door test's end
   (clear-at-boundary). C++ premise: ProductionUpdate makes TheGameLogic the sole
   production writer, so the GW writeback channel is opt-in (tick/authority.rs:236).

## Production code touched

`game_logic/world_objects/support_states/update.rs` only (SpecialAbility no-pending
cleanup now no-ops for live leftover channels; see #1). Adjacent suites re-run serially:
hacker_and_special_movement 44/44, missile_defender/laser filters 126/126
(world_save::world_tests::landmark_bridge_and_new_map_tests::
`load_map_data_sites_call_leftover_new_map` fails — PRE-EXISTING, unrelated: it source-
scans world_save.rs for ≥2 `terrain.new_map(false)` sites and world_save.rs currently
contains 0; outside this ticket's scope).

### Follow-up (WorldSaveScan, bd hq-9udt7): pre-existing failure closed

`load_map_data_sites_call_leftover_new_map` — root cause confirmed as the world_save
split: root world_save.rs is a 32-line facade, and the guarded sites moved into members.
Both `terrain.new_map(false)` sites (C++ GameLogic.cpp:1629
`TheTerrainLogic->newMap(loadingSaveGame)`: map load + fast legacy runtime sync) live in
`world_save/world_players.rs`; the landmark-bridge seam (`register_spawned_landmark_bridges`
definition, `add_landmark_bridge_from_geometry`) lives in `world_subsystems.rs` with the
call site in `world_load.rs`. Test fix only (no production change): the scan now
include_str!s the live members with per-member production-prefix clipping before
`#[cfg(test)]` (guard-strengthened: also pins the world_load.rs call site). Verified
serially: targeted test green; full `game_logic::world_save::world_tests` 8/8.

## Measured state

gameworld_shadow serial: **285/285** (was 281/4). All probes removed; test files
touched: continue_attack.rs, sync_ids.rs.
