# Final20 remainder close-out — Final19 report

Serial runs only (`--test-threads=1`). No git writes. bd update hq-9udt7 hit Dolt 1105 → this file.

## Cluster A (1/1 green)
- `targeting_and_physics::shock_bounce_settles_freefall_and_switches_to_stunned` **OK**.
- Root causes (physics_motion.rs):
  1. Stun relief (`maybe_clear_shock_stun_relief`) cleared IS_STUNNED in the same tick as
     first ground contact and mid-fall when a fast body stepped over the 0.64wu
     3-frame band (Thing.cpp:308-311) — the FLAILING→STUNNED flip (PhysicsUpdate.cpp:671-682)
     was never observable. Fix: relief defers while `shock_grounded_once && !vel_settled &&
     (already_on_ground || was_significantly_airborne)`; terrain clause additionally requires
     `|vy| < STUN_RELIEF_EPSILON`.
  2. The `shock_stun_frames == 0` settle branch never refreshed model bits — stale
     STUNNED/FREEFALL bits stuck after relief. Fix: `refresh_model_condition_bits()` added
     there, and the end-of-tick refresh (lost in an earlier edit of the file) restored.
  3. Test scenario: the 40vy arc carried the hull off-map/upside-down so the first bounce
     killed it via `test_stunned_unit_for_destruction` (PhysicsUpdate.cpp:505-517) before any
     grounded stun frame. Impulse retuned to `(0.5, 6.0, 0.0)` (verified by seed sweep).

## Cluster B (10/11 green) — gameworld_shadow::tests source-scan honesty
Fixed by repointing/widening stale scans to the live split:
- fire_damage x3: already green (scan file set was complete).
- presentation.rs: `command_move_attack_host_object_id_source` (window 2000→6000),
  `residual_acquire_query_source` (ai.rs→ai/teams.rs; engine pick→`host_find_object_at_position`;
  money-crate/mines scans→full brace-matched bodies; battle-drone entry removed —
  C++ BattleDroneAIUpdate doRepairLogic heals only its slaver, no acquire scan exists),
  `worker_unfinished_construction_presentation_source` (lookAt→`host_player_look_at` accepted;
  wrapper→`host_resume_selected_construction`).
- sell_heal.rs: `angry_mob_pdl_damage_source_authority_source` (live attribution APIs:
  `Some(plan.mob_id)`, `Some(plan.source_object),`, `take_radiation_field_tick(hit.damage, …)`),
  `create_object_spawn_pose_movement_authority_source` (tunnel capture honest channel =
  decision authority + cave-in destroy; C++ TunnelTracker::destroyObject destroys garrison
  in place — no reposition to log),
  `map_ground_support_pose_movement_authority_source` (`fn update_support_states(` exact match
  skips the `…_for_test` helper),
  `suicide_consume_destroy_damage_authority_source` (scan retargeted to
  `host_apply_production_spawn_ready_completions`, Wave 679 spawn-ready drain).

### REMAINDER (1): `sell_deconstruction_negative_percent_survives_shadow_writeback`
Behavioral failure, not a scan: a sold structure ("SellPad", percent 0.999, hp 500 intact)
gets `status.destroyed = true` during the FIRST `logic.update()` frame (host path, non-sole,
with or without GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY), so the percent never walks past
-0.1 and `host_object(oid)` is gone ("still selling" expect). The destroyer was not
identified (update_sell_list cannot finish before the 45-frame scaffold gate); needs an owner
to trace which update subsystem destroys freshly-sold structures.

## Cluster C (7/7 green)
- `cnc_game_engine::source_scan_tests::{generals_science_purchase_residual,
  strategy_center_battle_plan_residual}`: PF asserts retargeted to live split
  (`is_strategy_center_template`, `local_science_purchase_points`+`local_has_science`).
- wave555 (4) + wave852 (1): `engine_scan_src` harness now folds in
  `control_bar_bridge.rs` (live PurchaseScience/battle-plan executor);
  wave555 dispatch scan retargeted to `host_apply_control_bar_direct`;
  wave852 `player_can_purchase_science(player_id, name)` now reachable.
- Bonus (same honesty class): peeled the 3 remaining `self.game_logic.get_frame()`
  dual-reads in `cnc_game_engine/audio.rs` (2) and `input.rs` (1) to
  `presentation_or_boot_logic_frame()`; wave560 (4) and wave923 (window 900→2000) green.

## Pre-existing failures verified NOT caused by this pass (baseline reproduce with my
physics edits reverted): `jet_stop_and_enter_airfield_land`,
`jet_stop_idle_timer_sneaky_and_lockon` (do_jet_landing_command false / 310≠311),
`shock_stun_ticks_clear_model_bits` (FLAILING bit persists; note its 0..u32::MAX tick loop),
wave764 live/pack/sources (`OBJECT_SRC` lacks the `if countdown` marker).

## Verified green after changes (serial)
- cluster A test, cluster B 10/11, cluster C 7/7, wave555+wave852+wave560+wave923+wave662 packs,
  `shock_stun_channel_via_set_shock_stun`, targeting_and_physics shock subset,
  `shock_bounce_keep_flip_before_stun_test`, `motion_step_bounce_keeps_inverted_roll_for_stun_kill`.
