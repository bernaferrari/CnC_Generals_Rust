## Residual Host Playability — InGameUI Floating Text Freeze + MoneyPickUp Anim2D Presentation (2026-07-13)
**Closed (host-testable presentation residual not covered by laser SegLine / ControlBar dual-tick):**
1. **PresentationFrame floating text freeze residual** (`presentation_frame`):
   - Freeze host residual registries into snapshot-owned `PresentationFloatingText`:
     oil derrick / black market AutoDeposit, HackInternet, CashBounty, MoneyCrate.
   - Retail timeout residual: `DEFAULT_FLOATING_TEXT_TIMEOUT` = **10** frames
     (`LOGICFRAMES_PER_SECOND/3`); move-up **1.0**, vanish rate **0.1**.
   - Stable sort by spawn frame / source id / kind; included in `presentation_hash`.
   - Host inject helpers for dual-tick freeze tests (`push_residual_*_for_presentation`).
   - Fail-closed: not full DisplayString GPU / Unicode GameText localization.
2. **MoneyPickUp Anim2D world-anim freeze residual** (`presentation_frame`):
   - Freeze `HostMoneyPickUpAnim` → `PresentationWorldAnim` (template MoneyPickUp,
     display **4.0**s, ZRise **15**, fades **Yes**).
   - Honesty: `world_anim_presentation_ok` (empty honest; non-empty template check).
   - Fail-closed: not full Anim2DCollection GPU / WORLD_ANIM_FADE_ON_EXPIRE draw.
3. **CPU floating-text layout pack residual** (`graphics/floating_text_layout`):
   - Interleaved layout samples: pos + lift_y + color + alpha + amount + age/timeout.
   - Move-up / vanish residual matches C++ `updateFloatingText` / `drawFloatingText`.
   - Honesty: `cpu_pack_ok` / `has_geometry` / `gpu_upload_ready` (ready mark only).
   - Shell smoke: empty host pack + synthetic cash geometry pack residual.
4. Tests (not log-only):
   - `presentation_frame_freezes_floating_text_and_world_anim`
   - `graphics::floating_text_layout::*` (empty / synthetic / vanish)
   - shell_smoke residual flags `floating_text_layout_ok` + `world_anim_presentation_ok`
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full DisplayString GPU raster / font atlas submit for floating cash
- Full Unicode GameText localization of `GUI:AddCash`
- Full Anim2DCollection GPU / world-anim draw path
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition CONTINUOUS_FIRE_*
- Network floating-text / world-anim residual replication (network deferred)

## Residual Host Playability — PUC DamagePulseRemnant + Spectre CONTINUOUS_FIRE ModelCondition (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 16 swath/VoiceRapidFire):**
1. **Particle Uplink DamagePulseRemnant trail residual** (`special_power_strikes` /
   `ParticleUplinkCannonUpdate`):
   - Retail `DamagePulseRemnantObjectName` = `ParticleUplinkCannonTrailRemnant`
     with `ParticleUplinkCannonBeamTrailRemnantWeapon` Primary **15** / r**10**,
     DelayBetweenShots **250** ms → **7** frames, DeletionUpdate lifetime **4000** ms
     → **120** frames.
   - Host residual: each completed beam pulse spawns a `HostParticleRemnantField`
     at the pulse SwathOfDeath epicenter; ticks residual damage (ALLIES ENEMIES
     NEUTRALS — all living except source) until lifetime expires.
   - Honesty: `remnant_fields_spawned_total` / `remnant_damage_applications_total`
     / `honesty_beam_remnant_ok` / `honesty_beam_remnant_damage_ok`.
   - Snapshot/Xfer: remnant fields + totals appended after beam residual.
   - Fail-closed: not full ThingFactory Object / ImmortalBody / outer-node lasers /
     manual beam driving / laser width grow GPU.
2. **Spectre MODELCONDITION_CONTINUOUS_FIRE residual** (`special_power_strikes` /
   FiringTracker):
   - Retail `FiringTracker::speedUp` sets MODELCONDITION_CONTINUOUS_FIRE_MEAN /
     FAST; `coolDown` sets CONTINUOUS_FIRE_SLOW and clears MEAN/FAST.
   - Host residual honesty counters on orbit field:
     `model_condition_mean_sets` / `model_condition_fast_sets` /
     `model_condition_slow_sets` (incremented on fire-level transitions + coast).
   - Honesty: `honesty_model_condition_continuous_fire_ok` /
     `honesty_model_condition_slow_ok`.
   - Fail-closed: not full drawable W3D model-condition anim / SpectreHowitzerShell
     projectile Object.
3. Tests (not log-only):
   - `particle_uplink_damage_pulse_remnant_residual_honesty`
   - `spectre_model_condition_continuous_fire_residual_honesty`
   - updated `spectre_continuous_fire_rof_residual_honesty` (model-condition)
   - updated `spectre_continuous_fire_coast_cooldown_residual` (SLOW residual)
   - all `special_power_strikes::` (**31**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / full W3D CONTINUOUS_FIRE anim draw
- Full PUC outer-node lasers / manual beam driving / laser width grow matrix
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Network combat/power residual replication (network deferred)

## Residual Host Playability — Particle Uplink SwathOfDeath + Spectre VoiceRapidFire (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 16 coast/SupW/structure):**
1. **Particle Uplink SwathOfDeath residual** (`special_power_strikes` /
   `ParticleUplinkCannonUpdate`):
   - Retail SwathOfDeathDistance **200** / Amplitude **50**; DamageRadiusScalar
     **3.4** honesty constant.
   - Host residual: each damage pulse epicenter walks S-curve around click point
     (`particle_swath_offset` / `particle_swath_epicenter`); first pulse at
     `x = -distance/2`, mid pulse near click, lateral sine peaks ~amplitude.
   - Fractional nextFactor pulse schedule residual:
     `particle_next_pulse_frame = spawn + floor(pulses_made/total * duration)`.
   - Honesty: `swath_applications` / `max_swath_offset` / `honesty_beam_swath_ok`.
   - Host-testable: click-epicenter unit misses first pulse; mid swath hits; offset
     honesty > 50.
   - Fail-closed: not full building→target rotation matrix / manual driving /
     outer-node lasers / remnant trail Objects / laser width grow GPU.
2. **Spectre VoiceRapidFire residual** (`special_power_strikes` / FiringTracker):
   - Retail `FiringTracker::speedUp` plays PerUnitSound `"VoiceRapidFire"` when
     entering CONTINUOUS_FIRE_FAST.
   - Host residual: `rapid_fire_voice_cues` increments on gattling/howitzer MEAN→FAST
     transition; honesty `honesty_voice_rapid_fire_ok` + audio name residual
     `SpectreGunshipVoiceRapidFire`.
   - Fail-closed: not full audio bank stream / model-condition CONTINUOUS_FIRE_* anim.
3. Tests (not log-only):
   - `particle_uplink_swath_of_death_residual_honesty`
   - updated `particle_cannon_params_match_retail_continuous_beam`
   - updated `particle_cannon_impact_spawns_beam_and_ticks_damage`
   - updated `spectre_continuous_fire_rof_residual_honesty` (VoiceRapidFire)
   - all `special_power_strikes::` (**29**)
   - golden_skirmish_gate --frames 8 → `playable_claim=true`
   - shell_smoke_gate → `playable_claim=false` / `shell_host_playable_ok=true`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition CONTINUOUS_FIRE_*
- Full PUC outer-node lasers / manual beam driving / remnant trail Objects
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Network combat/power residual replication (network deferred)

## Residual Host Playability — Spectre Coast + SupW FuelAir + Structure Scatter (2026-07-13)
**Closed (host-testable combat/power/economy residual not covered by wave 15 anthrax/ROF):**
1. **Spectre ContinuousFireCoast residual** (`special_power_strikes` / FiringTracker):
   - Retail ContinuousFireCoast **2000** ms → **60** frames for both
     `SpectreGattlingGun` and `SpectreHowitzerGun`.
   - On each residual shot: `coast_until = frame + interval + 60`.
   - After idle past coast: coolDown zeros consecutive + fire level (base ROF).
   - Host tracks `gattling_coast_until_frame` / `howitzer_coast_until_frame` +
     `*_coast_applications` honesty; post-coast restarts at base interval.
   - VoiceRapidFire residual closed wave 16b (enter FAST honesty cue).
   - Fail-closed: not full model-condition CONTINUOUS_FIRE_* anim / howitzer shell
     projectile Object.
2. **SupW FuelAir 900/r70 matrix residual** (`host_aurora_bomb`):
   - `HostAuroraBombKind::FuelAirSupW` = retail `SupW_FuelBombDetonationWeapon`
     Primary **900** / r**70** (AirF keeps **1000**/r**100**).
   - Template classifier: `SupW_*` / `TestAuroraFuelAirSupW` → FuelAirSupW;
     `AirF_*` → FuelAir; standard Aurora → Standard.
   - Same gas delay + DaisyCutterFlame secondary **5**/r**100**; r80 is flame-only
     under SupW (outside primary 70) but still in AirF primary (matrix differs).
   - Fail-closed: not full SlowDeath multi-stage / tree burn / OCL gas Object.
3. **AutoDeposit structure geometry scatter residual** (economy / floating cash):
   - C++ KINDOF_STRUCTURE: `±0.3 * major/minor radius` on floating cash text pos.
   - Host residual `structure_floating_text_scatter` (deterministic golden-ratio
     phase); applied on oil derrick deposit/capture and black market deposit paths.
   - Honesty: `geometry_scatter_applications` / `honesty_geometry_scatter_ok`.
   - Fail-closed: not full GeometryInfo matrix / GameClientRandomValue stream /
     InGameUI GPU draw.
4. Tests (not log-only):
   - `spectre_continuous_fire_coast_cooldown_residual`
   - `supw_fuel_bomb_900_r70_matrix_residual_honesty`
   - `structure_geometry_scatter_residual`
   - all `special_power_strikes::` (28 → 29 with swath residual)
   - aurora / oil / black market host unit + integration residual paths green

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition CONTINUOUS_FIRE_*
- Full GameLogicRandomValueReal / GameClientRandomValue RNG streams
- Full InGameUI::addFloatingText GPU draw / Unicode GameText
- Network combat/power/economy residual replication (network deferred)

## Residual Host Playability — Scud Anthrax Upgrade + Spectre ContinuousFire ROF (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 13 dual-weapon / per-missile poison):**
1. **ScudStorm anthrax-upgrade residual** (`special_power_strikes` / `ScudStormAnthraxTier`):
   - Base `ScudStormDamageWeapon`: Primary **500** / Secondary **150** + LargePoison **15**.
   - Anthrax Beta (`Upgrade_GLAAnthraxBeta` / `ScudStormDamageWeaponUpgraded`):
     Secondary **200** + LargePoisonFieldWeaponUpgraded **25**/tick (radius **140**).
   - Chem Gamma (`Chem_Upgrade_GLAAnthraxGamma` / `Chem_ScudStormDamageWeaponGamma`):
     Primary **550** / Secondary **200** + poison **25**.
   - Queue path selects tier from player unlocked sciences/upgrades; strike stores
     `scud_anthrax_tier`; per-missile poison uses tier damage residual.
   - Host-testable: secondary ring hit = 200 under Beta; poison field 25/tick.
   - Fail-closed: not full ScudStormMissile projectile Object / PreAttack animation /
     Chem particle bone matrix.
2. **Spectre ContinuousFire ROF residual** (`special_power_strikes`):
   - Gattling ContinuousFireOne/**1** / Two/**2**: base interval **3** frames → MEAN
     ROF **200%** → **1** frame → FAST ROF **300%** → **1** frame.
   - Howitzer ContinuousFireOne/**1** / Two/**2** on HowitzerFiringRate base **9**:
     MEAN ROF **150%** → **6** frames; FAST ROF **200%** → **4** frames.
   - Host tracks `gattling_consecutive` / `howitzer_consecutive` + peak fire levels;
     honesty gates for MEAN reached.
   - ContinuousFireCoast residual closed wave 16 (was fail-closed here).
3. **Unit-host splash last_damage_source residual** (related combat residual):
   - Many unit host splash / fire paths now call `take_damage_from(..., Some(source))`
     so CashBounty killer residual prefers BodyModule last_damage_source.
   - Fail-closed: not full DestructionEvent killer ObjectId matrix on every path.
4. Tests (not log-only):
   - `scud_storm_anthrax_upgrade_secondary_and_poison_residual`
   - `spectre_continuous_fire_rof_residual_honesty`
   - all `special_power_strikes::` (27+)
   - `scud_storm_host_path_queues_and_completes`
   - `spectre_gunship_host_path_queues_orbit_damage_over_time`

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile Object / PreAttack animation / Chem FX bones
- Full SpectreHowitzerShell projectile Object / model-condition continuous-fire flags
- Full GameLogicRandomValueReal RNG stream
- Network Scud anthrax / Spectre ROF replication (network deferred)

## Residual Host Playability — Spectre Gattling/Howitzer Dual Residual + Scud Per-Missile Poison (2026-07-13)
**Closed (host-testable combat/power residual not covered by Scud multi-missile / Spectre OrbitTime / Aurora wave):**
1. **Spectre dual-weapon orbit residual** (`special_power_strikes`):
   - Howitzer (`SpectreHowitzerGun`): PrimaryDamage **80** in PrimaryDamageRadius
     **25** around reticle + deterministic `RandomOffsetForHowitzer` residual (**±20**).
   - Gattling (`SpectreGattlingGun`): PrimaryDamage **90** to nearest living enemy in
     AttackAreaRadius **200**, DelayBetweenShots **100** ms → **3** frames.
   - HowitzerFiringRate residual remains **9** frames; both streams exclude source
     + same-team friendlies.
   - Host-testable: first insertion tick = 80+90 at reticle; gattling-only at +3;
     far units outside orbit untouched; honesty_gattling_ok.
   - Fail-closed: not full projectile path / gunner Object (continuous-fire residual closed wave 15).
2. **ScudStorm per-missile LargePoisonField residual**:
   - Each multi-missile wave spawns `OCL_PoisonFieldLarge` residual at wave
     epicenters (retail FireOCL each detonation), not only on final wave.
   - Host-testable: poison fields grow across waves; up to ClipSize **9** fields.
3. Tests (not log-only):
   - `spectre_gattling_and_howitzer_residual_honesty`
   - updated `spectre_gunship_impact_spawns_orbit_and_ticks_damage`
   - updated `scud_storm_multi_missile_scatter_and_poison_residual`
   - updated `spectre_gunship_host_path_queues_orbit_damage_over_time`

**Still residual (fail-closed, not claimed):**
- Full SpectreHowitzerShell projectile / FiringTracker coast cool-down
- Full ScudStormMissile Object / PreAttack animation
- Full GameLogicRandomValueReal RNG stream
- Network Spectre dual-weapon / Scud poison replication (network deferred)

## Residual Host Playability — ScudStorm Multi-Missile + Spectre OrbitTime + Aurora ALLIES/Flame (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 11):**
1. **ScudStorm ClipSize-9 ScatterTarget multi-missile residual** (`special_power_strikes`):
   - Retail `ScudStormWeapon` ClipSize **9** + ScatterTarget table × ScatterTargetScalar
     **120**; PreAttackDelay **3000** ms → **90** frames; DelayBetweenShots Min/Max
     **100**/1000 ms → **3**/30 frames deterministic residual stagger.
   - Per-missile blast: `ScudStormDamageWeapon` Primary **500**/r**50** + Secondary
     **150**/r**200** (two-ring residual).
   - Each missile impact also spawns residual LargePoisonField toxin ticks
     (Primary **15** / r**140** / 500 ms / Lifetime **45** s).
   - Host-testable: first missile at PreAttack frame; all 9 applied by last delay;
     poison honesty gate.
   - Fail-closed: not full ScudStormMissile projectile Object / PreAttack animation /
     anthrax-upgraded poison weapon matrix / GameLogicRandomValueReal stream.
2. **SpectreGunship science-tier OrbitTime residual**:
   - Retail SCIENCE_SpectreGunship1/2/3 OrbitTime **10s / 15s / 20s** → **300 / 450 / 600**
     frames; `SpectreGunshipScienceTier` + `highest_from_sciences`.
   - Strike stores `spectre_tier`; orbit field duration uses tier residual.
   - Queue selects highest unlocked Spectre science on source team (default L2).
   - Fail-closed: not full SpectreGunshipUpdate gattling-strafe / howitzer projectile.
3. **Aurora bomb RadiusDamageAffects ALLIES residual** (`host_aurora_bomb`):
   - Retail `AuroraBombWeapon` / `AirF_AuroraBombDetonationWeapon` list
     `ALLIES ENEMIES NEUTRALS` (Standard also `NOT_SIMILAR`).
   - Host residual hits living non-self units of any team; source aircraft Object
     still excluded.
   - Host-testable: friend at epicenter takes primary blast; source never hit.
   - Fail-closed: not full NOT_SIMILAR / Relationship matrix.
4. **FuelAir DaisyCutterFlameWeapon secondary residual**:
   - Retail AirF_AuroraBombGas / SupW_AuroraFuelAirGas SlowDeath MIDPOINT
     `DaisyCutterFlameWeapon` PrimaryDamage **5.0** / PrimaryDamageRadius **100**.
   - Applied additively on FuelAir impact when horizontal distance ≤ flame radius.
   - Honesty constants: `AURORA_FUEL_AIR_FLAME_DAMAGE` / `AURORA_FUEL_AIR_FLAME_RADIUS`
     / `AURORA_FUEL_AIR_FLAME_AUDIO`; `HostAuroraBombKind::spawns_daisy_cutter_flame()`.
   - Host-testable: FuelAir epicenter = primary + 5; Standard Aurora has no flame.
   - Fail-closed: not full SlowDeath MIDPOINT timing / tree burn state / FX GPU;
     SupW 900/r70 collapse still uses AirF 1000/r100 host numbers.
5. **last_damage_source residual on superweapon / Aurora / continuous field paths**:
   - `update_aurora_bombs` and `update_special_power_strikes` (primary blast +
     radiation / toxin / Spectre orbit / Particle beam ticks) now call
     `take_damage_from(..., Some(source_object))` so CashBounty killer residual
     prefers BodyModule last_damage_source over nearest same-team fallback.
   - Fail-closed: not full DestructionEvent killer ObjectId on every residual
     kill path (unit host splash residual paths still open).
6. Tests (not log-only):
   - `scud_storm_multi_missile_scatter_and_poison_residual`
   - `spectre_orbit_time_science_tier_residual`
   - updated ScudStorm host-path integration (ClipSize 9 + poison honesty)
   - `allies_residual_and_legal_target_honesty`
   - `fuel_air_flame_and_allies_residual_honesty`
   - updated `queue_and_complete_delayed_dive_plan`
   - updated `aurora_bomb_host_path_queues_and_applies_delayed_area_damage`
   - snapshot xfer carries `spectre_tier` + toxin field damage/radius/tick params

**Still residual (fail-closed, not claimed):**
- Full ScudStormMissile projectile / PreAttack / anthrax-upgraded LargePoison
- Full Spectre gattling-strafe / howitzer projectile / edge-spawn flight
- Full SlowDeath multi-stage gas object / tree burn state machine
- Full SupW_FuelBombDetonationWeapon 900/r70 vs AirF 1000/r100 matrix
- Full AuroraBombLocomotor / MissileAIUpdate dive path
- Full last_damage_source on every unit-host residual splash path
- Network ScudStorm / Spectre / Aurora / superweapon blast replication (network deferred)

## Residual Host Playability — MOABFlame Secondary + RadiusDamageAffects ALLIES (2026-07-13)
**Closed (host-testable combat/power residual not covered by wave 10):**
1. **MOABFlameWeapon secondary residual** (`special_power_strikes`):
   - Retail `MOABFlameWeapon` PrimaryDamage **5.0** / PrimaryDamageRadius **100**
     (MOABGas SlowDeath MIDPOINT — tree-ignite flame).
   - Applied additively on `DaisyCutter` and `CruiseMissile` primary impact when
     horizontal distance ≤ flame radius.
   - Honesty constants: `MOAB_FLAME_DAMAGE` / `MOAB_FLAME_RADIUS` / `MOAB_FLAME_AUDIO`.
   - Host-testable: epicenter damage = primary + 5; outer ally beyond flame gets
     falloff primary only.
   - Fail-closed: not full SlowDeath MIDPOINT timing / tree burn state / FX GPU.
2. **RadiusDamageAffects ALLIES residual** for retail blast kinds:
   - ArtilleryBarrage / CarpetBomb / DaisyCutter / CruiseMissile / NuclearMissile /
     AnthraxBomb / A10Strike / ScudStorm primary blasts now hit same-team units
     (retail `RadiusDamageAffects = ALLIES ENEMIES NEUTRALS`).
   - Source launcher Object still excluded.
   - Spectre orbit / Particle Uplink continuous fields keep their own filters.
   - Host-testable: friendly at epicenter takes damage; A10 hits ally + enemy.
3. Tests (not log-only):
   - `moab_flame_and_allies_residual_honesty`
   - updated `cruise_missile_*` / `queue_and_complete_daisy_cutter_*`
   - updated anthrax/nuclear impact ally hits
   - `friendly_fire_allies_residual_and_source_excluded`
   - updated GameLogic host-path integration (Daisy/Carpet/Artillery/Cruise)

**Still residual (fail-closed, not claimed):**
- Full SlowDeath multi-stage wave / MOABGas object / tree burn state machine
- Full ChinaArtilleryCannon / AmericaJetB52 DeliverPayload transport Objects
- Full GameLogicRandomValueReal RNG stream
- Network multi-strike / ally-blast / flame replication (network deferred)

## Residual Host Playability — STEALTHED Float Gate + IC Scatter + last_damage_source Killer (2026-07-13)
**Closed (host-testable STEALTHED floating-text local display gate + Internet Center scatter + CashBounty last_damage_source killer residual):**
1. **STEALTHED local-player floating cash display gate residual**:
   - C++ AutoDepositUpdate / HackInternetAIUpdate:
     `if STEALTHED && !isLocallyControlled && !DETECTED → displayMoney = FALSE`.
   - Wired for oil derrick / black market AutoDeposit and hacker cash pings.
   - Hacker also gates on containedBy Internet Center STEALTHED residual.
   - Cash still deposits; only floating text presentation is gated.
   - Host-testable: gate matrix unit tests; suppressed honesty counters.
   - Fail-closed: not full InGameUI GPU draw / Unicode GameText.
2. **Internet Center floating-text geometry scatter residual** (`host_hacker_income`):
   - C++ ±0.3 major/minor radius GameClientRandomValue when depositing inside IC.
   - Host residual: deterministic scatter; honesty `ic_scatter_applications`.
   - Fail-closed: not full GeometryInfo major/minor matrix / client RNG stream.
3. **CashBounty last_damage_source killer residual** (`host_cash_bounty`):
   - Prefer victim BodyModule `last_damage_source` for killer ObjectId + float pos.
   - Main combat fire path uses `take_damage_from(dmg, Some(attacker_id))`.
   - Fallback nearest living same-team residual when source unset.
   - Host-testable: bounty float `killer_id == attacker`; last_damage_source honesty.
   - Fail-closed: not full DestructionEvent killer ObjectId on every residual kill path.
4. Tests (not log-only):
   - `stealthed_local_display_gate_residual` (oil)
   - `stealthed_local_display_gate_and_ic_scatter_residual` (hacker)
   - `last_damage_source_killer_residual_honesty` (cash bounty)
   - updated `cash_bounty_increases_cash_on_enemy_kill` (last_damage_source killer)

**Still residual (fail-closed, not claimed):**
- Full InGameUI::addFloatingText GPU draw / Unicode GameText localization
- Full DestructionEvent killer ObjectId matrix on every non-combat residual kill path
- Network floating-text / last_damage_source replication (network deferred)

## Residual Host Playability — Artillery WeaponErrorRadius/DelayDelivery + Carpet DropDelay Stagger (2026-07-13)
**Closed (host-testable combat/power multi-strike residual):**
1. **ArtilleryBarrage WeaponErrorRadius residual** (`special_power_strikes`):
   - C++ `DeliverPayloadNugget`: formationIndex **0** spot-on; others
     `GameLogicRandomValueReal(0, WeaponErrorRadius=100)` + random angle.
   - Host residual: deterministic `weapon_error_radius_offset` (golden-ratio phase).
   - Replaces fixed ring placement; shells stay inside **100** error radius.
   - Host-testable: lead shell at click; non-lead scatter ≤ error radius.
2. **ArtilleryBarrage per-shell DelayDelivery stagger residual**:
   - Retail `DelayDeliveryMax` = 3000 ms → **90** frames; C++ disables each
     ChinaArtilleryCannon transport until `Random(0, max)`.
   - Host residual: base approach **90** + per-shell `delay_delivery_frames`
     (lead **0**; others deterministic in `[0, 90]`).
   - Multi-wave impact: shells apply when due; strike completes on last shell.
   - Host-testable: first wave at base delay; complete at last shell frame.
3. **CarpetBomb DropDelay stagger residual**:
   - Retail OCL `DropDelay` = 300 ms → **9** frames between bombs.
   - Bomb `i` impacts at approach **90** + `i × 9`; multi-wave complete after
     last bomb (index 14).
   - Jump past several frames applies all overdue bombs in one wave (save/load).
   - Host-testable: first bomb only at frame 90; center/outer later; complete last.
4. Tests (not log-only):
   - `weapon_error_radius_and_delay_delivery_residual_honesty`
   - updated `artillery_barrage_params_match_retail_multi_shell`
   - updated `artillery_barrage_delayed_multi_shell_scatter_damage`
   - updated `carpet_bomb_params_match_retail_multi_strike` (DropDelay **9**)
   - updated `carpet_bomb_delayed_line_multi_strike_damage` (stagger waves)
   - updated GameLogic carpet/artillery host-path integration tests

**Still residual (fail-closed, not claimed):**
- Full ChinaArtilleryCannon / AmericaJetB52 DeliverPayload transport Objects
- Full GameLogicRandomValueReal RNG stream (host deterministic residual)
- Full MOABFlameWeapon secondary / NeutronMissile loft projectile
- Network multi-strike stagger replication (network deferred)

## Residual Host Playability — Hacker/CashBounty Floating Text + Artillery FormationSize Tiers (2026-07-13)
**Closed (host-testable HackInternet + CashBounty floating cash text + ArtilleryBarrage science-tier FormationSize residual):**
1. **HackInternet floating cash text residual** (`host_hacker_income`):
   - Host `+$N` presentation at hacker pos + Z offset **20** (C++ `pos.z += 20`).
   - Green RGBA **(0,255,0,255)** (retail `GameMakeColor(0,255,0,255)`).
   - GameText key honesty `GUI:AddCash`.
   - Recorded on every residual cash ping (field + Internet Center).
   - Host-testable: floating text on deposit; amount/color/key constants.
   - Fail-closed: not full InGameUI GPU draw / STEALTHED local-player display gate /
     Internet Center geometry scatter random.
2. **CashBounty floating cash text residual** (`host_cash_bounty`):
   - Host `+$N` at killer residual pos + Z offset **10** (C++ killer `pos.z += 10`).
   - Yellow RGBA **(255,255,0,255)** (retail `GameMakeColor(255,255,0,255)`).
   - Killer pos residual: nearest living unit on killer team (destruction event carries team only).
   - Host-testable: floating text on bounty award; yellow color / Z+10 honesty.
   - Fail-closed: not full killer ObjectId on destruction event / InGameUI GPU.
3. **ArtilleryBarrage science-tier FormationSize residual** (`special_power_strikes`):
   - Retail OCL FormationSize **12 / 24 / 36** for SCIENCE_ArtilleryBarrage1/2/3.
   - `ArtilleryBarrageScienceTier` + `artillery_barrage_points_for_tier`.
   - Queue path selects highest unlocked SCIENCE_ArtilleryBarrage* on source team.
   - Strike stores `artillery_tier`; impact uses tier shell count for multi-shell scatter.
   - Host-testable: L1=12 / L2=24 / L3=36 points; science name matrix; highest-wins.
   - Fail-closed: not full random WeaponErrorRadius draw / per-shell DelayDelivery stagger /
     ChinaArtilleryCannon DeliverPayload transport Object.
4. Tests (not log-only):
   - `floating_text_residual_green_z20` (hacker)
   - `floating_text_residual_yellow_z10` (cash bounty)
   - updated `artillery_barrage_params_match_retail_multi_shell` (tier FormationSize)
   - host unit tests + existing integration residual paths still green

**Still residual (fail-closed, not claimed):**
- Full InGameUI::addFloatingText GPU draw / STEALTHED local display gate (hacker)
- Full killer ObjectId on DestructionEvent (cash bounty uses nearest same-team residual)
- Full ArtilleryBarrage random WeaponErrorRadius / OCL DeliverPayload transport path
- Network floating-text / artillery-tier replication (network deferred)

## Residual Host Playability — AutoDeposit Floating Text + Oil SupplyLines Boost (2026-07-13)
**Closed (host-testable AutoDepositUpdate floating cash text + TechOilDerrick UpgradedBoost residual):**
1. **AutoDeposit floating cash text residual** (`host_oil_derrick` / `host_black_market`):
   - Host `+$N` presentation at building pos + Z offset **10** (C++ `pos.z += 10`).
   - Player color RGB OR alpha **230** (C++ `GameMakeColor(0,0,0,230)`).
   - GameText key honesty `GUI:AddCash`.
   - Recorded on oil derrick deposit / capture bonus and black market deposit.
   - Host-testable: floating text on deposit; amount/color/key constants.
   - Fail-closed: not full InGameUI GPU draw / STEALTHED local-player display gate /
     Unicode GameText localization.
2. **TechOilDerrick UpgradedBoost residual** (`host_oil_derrick`):
   - Retail `Upgrade_AmericaSupplyLines` Boost **+20** on DepositAmount **200** → **220**.
   - Host-testable: base 200 without upgrade; 220 with SupplyLines; boost honesty counter.
   - Fail-closed: not full upgrade-mux edge matrix beyond SupplyLines name residual.
3. Tests (not log-only):
   - host_oil_derrick floating text + SupplyLines boost unit tests
   - host_black_market floating text unit tests
   - updated oil derrick / black market GameLogic residual integration tests

**Still residual (fail-closed, not claimed):**
- Full InGameUI::addFloatingText GPU draw / STEALTHED local display gate
- Full AutoDeposit upgrade-mux beyond SupplyLines residual
- Network AutoDeposit floating-text replication (network deferred)

## Residual Host Playability — calcMinTurnRadius + MaxAttempts Re-Approach + Off-Map Recover (2026-07-13)
**Closed (host-testable DeliverPayloadAIUpdate calcMinTurnRadius + ConsiderNewApproach + HeadOffMap/RecoverFromOffMap residual):**
1. **calcMinTurnRadius residual** (`host_deliver_payload`):
   - C++ `maxSpeed / maxTurnRate` (both per logic frame).
   - B52Locomotor: Speed **125**/sec, TurnRate **25**°/sec → radius ≈ **286.48**
     (`5 × 180/π`); zero turn rate → sentinel **999999**.
   - ConsiderNewApproach min re-approach dist = radius × **DIST_FUDGE 2.2**.
   - RecoverFromOffMap delay frames = `ceil(radius / maxSpeed)` ≈ **69**.
   - Host-testable: formula constants; B52 radius / reapproach / recover frames.
   - Fail-closed: not full locomotor damage-state matrix / pathfinder turn arcs.
2. **MaxAttempts ConsiderNewApproach residual** (`HostCargoPlaneFlight`):
   - Leave DeliveryDistance band mid-stagger (items incomplete) → re-approach.
   - Re-approach waypoint = position + heading × minReApproachDist.
   - `reapproach_count` increments; when **> MaxAttempts (4)** → HeadOffMap give-up.
   - Delivery phases fly-through on residual heading (not home-to-target).
   - Host-testable: band exit → ConsideringNewApproach; 4 re-approaches then Departing.
   - Fail-closed: not full AI_MOVE_TO pathfinder / setAllowInvalidPosition locomotor.
3. **isOffMap / HeadOffMap / RecoverFromOffMap residual**:
   - isOffMap residual: XZ outside residual map extent (no Z).
   - HeadOffMap: fly straight on heading until off-map → Complete; `accepting_commands=false`.
   - RecoverFromOffMap: hide + turn-radius frame delay → closest-edge re-entry → Approaching.
   - Host-testable: depart completes off-map; recover unhides at PreferredHeight edge.
   - Fail-closed: not full Partition unRegister / drawable GPU hide / TerrainLogic border.
4. Tests (not log-only):
   - `calc_min_turn_radius_b52_residual`
   - `consider_new_approach_max_attempts_residual`
   - `head_off_map_and_recover_from_off_map_residual`
   - `is_off_map_and_reapproach_point_residual`
   - updated CreateAtEdge flight residual (HeadOffMap depart still host-testable)

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / full pathfinder locomotor matrix
  (calcMinTurnRadius + MaxAttempts re-approach + HeadOffMap/Recover residual closed
  2026-07-13)
- Full DropVariance RNG stream for non-carpet OCLs (carpet residual closed;
  supply OCL has none)
- Full AmericaCrateParachute container Object / W3D pristine bone extract GPU
- Full CollideModule partition / Anim2DCollection GPU / InGameUI floating text draw
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D pristine bone extract for cargo plane doors (DOOR_1 condition residual closed)
- Full ControlBar OCL timer UI / SabotageSupplyDropzone timer-reset (retail saboteur
  module commented in base INI)
- Network DeliverPayload / MoneyCrate / CreateAtEdge / re-approach replication
  (network deferred)

## Residual Host Playability — CreateAtEdge Flight + Crate Bone Attach + Money Floating Text (2026-07-13)
**Closed (host-testable CreateAtEdge cargo-plane flight + AmericaCrateParachute bones + floating cash text residual):**
1. **CreateAtEdge AmericaJetCargoPlane flight residual** (`host_deliver_payload`):
   - Edge spawn residual via closest-edge on residual map extent (XZ) at
     PreferredHeight **100** (StartAtPreferredHeight **Yes**).
   - B52Locomotor Speed **125**/sec → **~4.167**/frame approach toward target.
   - `isCloseEnoughToTarget` residual: DeliveryDistance **410** (+ PreOpen when inbound).
   - Flight phases: EdgeSpawn → Approaching → InDeliveryBand → DoorOpening →
     Delivering → Departing/Complete.
   - Door residual: DoorDelay → MODELCONDITION `DOOR_1_OPENING` / `AVCargoPln_A2`.
   - Honesty: model `AVCargoPln`, ExitBone `WeaponA01`, ExitPitchRate **30**,
     StartAtMaxSpeed **Yes**, MaxAttempts **4**.
   - Host-testable: edge spawn Y=100; approach into band; door open; complete departs.
   - Fail-closed: not full aircraft Object / pathfinder re-approach / calcMinTurnRadius
     / off-map recover / W3D door GPU.
2. **AmericaCrateParachute bone attach residual** (`host_deliver_payload`):
   - PARA_COG / PARA_ATTCH pristine bone residual (GeometryHeight **10** layout).
   - Crate hang height-fallback: SupplyDropZoneCrate GeometryHeight **12** (no PARA_MAN).
   - Open-chute sway residual about PARA_COG (presentation; logic hang unswayed).
   - Built each open-chute residual tick for cargo crates.
   - Host-testable: COG above ATTCH; hang below origin; open sway non-zero.
   - Fail-closed: not full W3D pristine bone extract / container Object GPU.
3. **Money floating cash text residual** (`host_money_crate`):
   - Host `+$N` presentation at crate pos + Z offset **20** (green RGBA 0,255,0,255).
   - GameText key honesty `GUI:AddCash` (caption not fully localized).
   - Recorded on unit MoneyCrateCollide residual collect (with MoneyPickUp Anim2D).
   - Host-testable: floating text on pickup; amount/color/key constants.
   - Fail-closed: not full InGameUI draw / Unicode GameText / EVA voice events.
4. Tests (not log-only):
   - `create_at_edge_closest_edge_and_preferred_height`
   - `is_close_enough_delivery_band_inbound_preopen`
   - `cargo_plane_flight_create_at_edge_approach_and_door`
   - `crate_parachute_bone_attach_residual`
   - updated `supply_drop_zone_residual_credits_cash_on_interval` (CreateAtEdge + bones)
   - updated `money_crate_collide_unit_pickup_residual` (floating text)
   - updated `money_crate_above_terrain_and_forbidden_kindof_residual` (bones + float)
   - host_money_crate floating text unit tests

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / full pathfinder locomotor matrix
  (edge spawn + approach band + door residual closed 2026-07-13; calcMinTurnRadius +
  MaxAttempts re-approach + HeadOffMap/Recover residual closed 2026-07-13 — see
  calcMinTurnRadius + MaxAttempts Re-Approach + Off-Map Recover section)
- Full DropVariance RNG stream for non-carpet OCLs (carpet residual closed;
  supply OCL has none)
- Full AmericaCrateParachute container Object / W3D pristine bone extract GPU
  (PARA_COG/PARA_ATTCH host residual closed 2026-07-13)
- Full CollideModule partition / Anim2DCollection GPU / InGameUI floating text draw
  (MoneyPickUp + floating cash presentation residual closed 2026-07-13)
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D pristine bone extract for cargo plane doors (DOOR_1 condition residual closed)
- Network DeliverPayload / MoneyCrate / CreateAtEdge flight replication
  (network deferred)

## Residual Host Playability — Crate Parachute Fall + MoneyPickUp Anim2D + Carpet DropVariance (2026-07-13)
**Closed (host-testable AmericaCrateParachute fall-physics + MoneyPickUp Anim2D + CarpetBomb DropVariance residual):**
1. **AmericaCrateParachute cargo fall-physics residual** (`host_deliver_payload`):
   - Spawn at B52 PreferredHeight **100** + DropOffset Y:-5 → host Y **95**.
   - Freefall until fallen `ParachuteOpenDist` **12.5**, then open chute (slower sink).
   - Low-altitude open fudge residual: start − ground ≥ **2×** OpenDist.
   - `ParachuteDirectly = Yes` residual honesty; open audio `ParachuteOpen`.
   - Host-testable: elevated spawn + parachuting; OpenDist open; land clears chute;
     unit MoneyCrateCollide blocked while airborne; BuildingPickup still legal.
   - Fail-closed: not full PutInContainer AmericaCrateParachute Object / W3D bones /
     CrateParachuteLocomotor force matrix / PreferredHeight aircraft Object.
2. **MoneyCrateCollide ForbiddenKindOf + above-terrain + MoneyPickUp residual**:
   - ForbiddenKindOf **PROJECTILE** residual (+ parachuting pickers rejected).
   - Above-terrain residual: unit path blocked while crate airborne (C++
     `isAboveTerrain` except BuildingPickup).
   - ExecuteAnimation residual: `MoneyPickUp` Anim2D presentation descriptor
     (display **4.0**s, ZRise **15**, fades **Yes**) — presentation state, not GPU.
   - Host-testable: airborne unit reject honesty; MoneyPickUp on collect; PROJECTILE
     cannot pick up.
   - Fail-closed: not full CollideModule partition pairs / Anim2DCollection GPU /
     InGameUI world-anim draw / EVA floating text / science gate matrix.
3. **CarpetBomb DropVariance residual** (`special_power_strikes`):
   - Retail OCL DropVariance X:**30** Y:**40** Z:**0** (C++ X/Y → host X/Z).
   - Deterministic host scatter residual in ±variance (not GameLogicRandomValueReal).
   - Applied per bomb epicenter on line formation residual.
   - Host-testable: scatter bounds; Z lateral non-zero; damage still hits variance
     points; zero variance identity (supply OCL has none).
   - Fail-closed: not full AmericaJetB52 CreateAtEdge / RNG stream / per-bomb
     DropDelay stagger flight path.
4. Tests (not log-only):
   - `crate_parachute_open_dist_and_sink_residual`
   - `money_crate_above_terrain_and_forbidden_kindof_residual`
   - updated `supply_drop_zone_residual_credits_cash_on_interval` (parachute land)
   - updated `money_crate_collide_unit_pickup_residual` (MoneyPickUp anim)
   - updated `queue_and_stagger_supply_drop_cargo` (elevated spawn Y)
   - `carpet_bomb_drop_variance_residual_bounds`
   - updated `carpet_bomb_params_match_retail_multi_strike` / delayed damage
   - host_money_crate MoneyPickUp unit tests

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / pathfinder re-approach /
  calcMinTurnRadius / off-map recover (edge spawn + DeliveryDistance band + door
  residual closed 2026-07-13 — see CreateAtEdge Flight + Crate Bone Attach section)
- Full DropVariance RNG stream for non-carpet OCLs (carpet residual closed;
  supply OCL has none)
- Full AmericaCrateParachute container Object / W3D pristine bone extract GPU
  (OpenDist freefall/open/land + PARA_COG/PARA_ATTCH host residual closed 2026-07-13
  — see CreateAtEdge Flight + Crate Bone Attach section)
- Full CollideModule partition / Anim2DCollection GPU / InGameUI floating text draw
  (MoneyPickUp + floating cash + ForbiddenKindOf + above-terrain residual closed
  2026-07-13 — see CreateAtEdge Flight + Crate Bone Attach section)
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D door GPU for cargo plane (DOOR_1 condition residual closed 2026-07-13)
- Network DeliverPayload / MoneyCrate / CreateAtEdge / carpet DropVariance
  replication (network deferred)

## Residual Host Playability — DropDelay Stagger + MoneyCrateCollide + Campaign.ini Table (2026-07-13)
**Closed (host-testable DeliverPayload DropDelay stagger + MoneyCrateCollide + Campaign.ini residual):**
1. **DeliverPayload DropDelay per-item stagger residual** (`host_deliver_payload`):
   - Retail OCL `DropDelay = 350` ms → **11** frames between items.
   - AmericaJetCargoPlane `DoorDelay = 500` ms → **15** frames before first item.
   - First item frame = activate + approach (**90**) + door (**15**); item *i* = first + *i*×11.
   - Host spawns **one** payload item per due frame (C++ DeliveringState exit tick).
   - **DropOffset** residual X:0 Y:0 Z:-5 applied (host Y-up → Y=-5).
   - Honesty constants: MaxAttempts **4**, PreOpenDistance **0** (supply OCL),
     DeliveryDistance **410**, Paradrop PreOpenDistance **300**.
   - Host-testable: no crates before first item; 1 crate on first due; full 6 after stagger;
     bulk BuildingPickup cash on **final** item only.
   - Fail-closed: not full CreateAtEdge cargo-plane Object / flight locomotor /
     DropVariance (supply OCL has none) / VisiblePayload bones / parachute fall physics
     (AmericaCrateParachute fall residual closed 2026-07-13 — see Crate Parachute Fall
     + MoneyPickUp Anim2D + Carpet DropVariance section).
2. **MoneyCrateCollide unit + BuildingPickup residual** (`host_money_crate`):
   - MoneyProvided **250**, SupplyLines boost **+25**, BuildingPickup **Yes**.
   - Unit residual: non-structure non-neutral within radius **20** credits cash + destroys crate.
   - BuildingPickup residual radius **80** (structure collect path).
   - Supply Drop Zone bulk BuildingPickup residual still credits $1500/$1650 on mission
     complete and marks crates paid (no unit double-credit).
   - Host-testable: unit pickup cash + CrateMoney audio; SupplyLines boost residual.
   - Fail-closed: not full CollideModule partition pairs / Anim2D MoneyPickUp / EVA text
     (MoneyPickUp presentation + ForbiddenKindOf + above-terrain residual closed
     2026-07-13 — see Crate Parachute Fall + MoneyPickUp Anim2D section).
3. **Campaign.ini residual mission table** (Main `CampaignManager`):
   - USA campaign MD_USA01…MD_USA05 residual chain + required-mission links.
   - CHALLENGE_0 residual map chain (GC_Chem…GC_ChinaBoss) + `usa_gen_01` alias.
   - Honesty: `honesty_campaign_ini_table_ok()`.
   - Fail-closed: not full Campaign.ini INI parse / GameClient manager parity /
     end-to-end cinematic score-screen flow.
4. Tests (not log-only):
   - `queue_and_stagger_supply_drop_cargo` / `drop_delay_stagger_item_frames`
   - updated `supply_drop_zone_residual_credits_cash_on_interval` (stagger timing)
   - updated `deliver_payload_cargo_residual_constants_and_skip` (DoorDelay/MaxAttempts)
   - `money_crate_collide_unit_pickup_residual`
   - `campaign_ini_residual_mission_table`
   - host_money_crate unit tests

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / DeliverPayloadAIUpdate flight
  state machine / approach geometry (constants + DoorDelay/DropDelay stagger closed)
- Full DropVariance random scatter (supply OCL has none; carpet residual closed
  2026-07-13 — see Crate Parachute Fall + MoneyPickUp Anim2D + Carpet DropVariance)
- Full AmericaCrateParachute container fall-physics for cargo
  (host residual closed 2026-07-13 — see Crate Parachute Fall section)
- Full CollideModule partition / Anim2D / ForbiddenKindOf matrix beyond residual gates
  (MoneyPickUp + PROJECTILE + above-terrain host residual closed 2026-07-13)
- Full Campaign.ini parse into Main manager (seeded residual table closed 2026-07-13)
- Full W3D pristine bone extract for cargo plane doors
- Network DeliverPayload / MoneyCrate / campaign replication (network deferred)

## Residual Host Playability — DeliverPayload Cargo Plane Delayed Spawn (2026-07-13)
**Closed (host-testable DeliverPayload cargo residual — delayed payload spawn):**
1. **DeliverPayload cargo plane residual** (`host_deliver_payload`):
   - Models retail OCL `DeliverPayload` cargo missions without full aircraft
     edge-spawn / locomotor flight / door animation.
   - Approach delay residual **90** frames @ 30 FPS, then spawn payload units
     at target in line formation.
   - Retail OCL honesty constants: Transport `AmericaJetCargoPlane`,
     PutInContainer `AmericaCrateParachute` / `AmericaParachute`,
     DropDelay **350** ms → **11** frames (stagger fail-closed; constant retained),
     DeliveryDistance **410**, Payload count **6** for supply crates.
   - Host-testable: queue → inbound; approach delay → spawn; transport name honesty.
   - Fail-closed: not full CreateAtEdge aircraft Object / DeliverPayloadAIUpdate
     state machine / DropDelay per-item stagger / parachute fall physics.
2. **America Supply Drop Zone cargo path residual**:
   - OCL interval (**3600** frames) queues cargo DeliverPayload flight (not
     immediate cash).
   - After approach delay: spawn **6** residual crates near zone + BuildingPickup
     residual cash **$1500** (SupplyLines **$1650**).
   - Template guard: crate / cargo / parachute names do **not** match zone
     structure residual (false-positive fix).
   - Honesty: `honesty_supply_drop_zone_flight_ok`,
     `honesty_supply_drop_cargo_deliver_payload_ok`,
     `honesty_deliver_payload_cargo_ok`.
   - Host-testable: no cash/crates during approach; spawn + cash on drop frame.
   - Fail-closed: not full MoneyCrateCollide unit path / ControlBar OCL timer UI.
3. **America Paradrop DeliverPayload cargo bookkeeping residual**:
   - `queue_paradrop` also records AmericaParadrop cargo mission residual
     (AmericaJetCargoPlane honesty). Infantry spawn remains host_paradrop.
   - Host-testable: cargo mission completes with paradrop drop frame.
4. Tests (not log-only):
   - `supply_drop_zone_residual_credits_cash_on_interval` (cargo delay + crates)
   - `deliver_payload_cargo_residual_constants_and_skip`
   - `host_deliver_payload` unit tests (queue/spawn/constants)
   - updated supply drop zone template false-positive guard
   - `america_paradrop_host_path_queues_and_spawns_infantry` (cargo bookkeeping)

**Still residual (fail-closed, not claimed):**
- Full CreateAtEdge AmericaJetCargoPlane Object / DeliverPayloadAIUpdate flight
  state machine (DropDelay stagger + DoorDelay + MaxAttempts/PreOpenDistance/DropOffset
  constants residual closed 2026-07-13 — see DropDelay Stagger + MoneyCrateCollide section)
- Full DropVariance / VisiblePayload bone matrix (supply OCL has no DropVariance)
- Full AmericaCrateParachute / AmericaParachute container fall-physics for cargo
  (PARA_COG host residual already closed for eject path)
- Full CollideModule partition / Anim2D MoneyPickUp (MoneyCrateCollide unit residual
  closed 2026-07-13 — see DropDelay Stagger + MoneyCrateCollide section)
- Full W3D pristine bone extract for cargo plane doors
- Network DeliverPayload / cargo replication (network deferred)

## Residual Host Playability — CamoNet Sub-Object + Physical Soldier Attach + Vision Mood + SegLine (2026-07-13)
**Closed (host-testable CamoNetting sub-object + stinger soldier attach + Partition vision mood + SegLine UV residual):**
1. **CamoNetting sub-object net mesh residual** (presentation state, not GPU):
   - Host residual mesh name honesty `CamoNet` (`CAMO_NETTING_SUB_OBJECT_MESH_NAME`).
   - Upgrade_GLACamoNetting apply → `camo_net_sub_object_shown` + presentation
     descriptor (`HostCamoNetSubObject`: shown / opacity / heat_vision_pass /
     StealthLook).
   - Enemy-undetected Invisible StealthLook → observer-hidden residual mesh;
     detected residual → heat-vision second-pass + observer-visible.
   - Host-testable: upgrade shows mesh; Invisible hides; detected heat-vision pass.
   - Fail-closed: not full W3D SubObjectsUpgrade / mesh GPU / second-pass shader.
2. **Physical SpawnBehavior soldier attach residual** (GLAStingerSite):
   - SpawnPoint bone Z-rotation facing residual (outward `atan2` ring layout).
   - `orderSlavesToAttackTarget` / `orderSlavesToGoIdle` residual on roster.
   - Live residual fire path orders alive slaves at target ObjectId.
   - `HostHiveSlaveAttach` presentation: world XZ + facing + AI order + template
     `GLAInfantryStingerSoldier`.
   - Host-testable: facings on init; fire → order; attach presentation positions.
   - Fail-closed: not full GLAInfantryStingerSoldier Object / full AI module /
     W3D model bone attach GPU.
3. **Partition / AI vision mood range residual** (Strategy Center Bombardment):
   - VisionRange **400** residual filter on mood-target acquire/clear
     (`strategy_center_mood_vision_range`; S&D × **2.0** host-testable).
   - Acquire requires vision residual **and** StrategyCenterGun range band.
   - Host-testable: vision constants; out-of-vision illegal; gun band still gated.
   - Fail-closed: not full PartitionManager filter stack / pathfinder mood matrix.
4. **SegLineRenderer UV / polyline residual** (Patriot BinaryDataStream math path):
   - `HostSegLineRendererState`: texture `EXBinaryStream32.tga`, tile factor,
     UV scroll offset, InnerColor, width, polyline of segment endpoints.
   - Built from Line3D residual segments (Segments+1 points).
   - Host-testable: polyline length; texture/tile/UV/color honesty.
   - Fail-closed: not full WGPU SegLineRenderer texture upload / GPU draw.
5. Tests (not log-only):
   - `stinger_physical_soldier_attach_facing_order_residual`
   - updated `stinger_get_closest_slave_physical_roster_residual` (facing/order/attach)
   - updated `stinger_site_residual_dual_fire_and_ap_rockets` (order on fire)
   - updated `camo_netting_structure_attack_and_damage_reveal_residual` (sub-object)
   - updated `camo_netting_structure_stealth_delay_matrix` (sub-object matrix)
   - updated `patriot_laser_arc_segment_honesty` (SegLine polyline)
   - updated `battle_plan_constants_match_retail_residual` (vision mood)
   - updated `strategy_center_turret_mood_target_residual` (vision residual)

**Still residual (fail-closed, not claimed):**
- Full PartitionManager filter stack / pathfinder mood matrix
  (vision range + weapon band host residual closed 2026-07-13)
- Full W3DLaserDraw WGPU SegLineRenderer texture upload for Patriot assist beams
  (CPU UV polyline host residual closed 2026-07-13)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full W3D pristine bone extract / DeliverPayload cargo plane path
- Full physical SpawnBehavior soldier Object / full AI / W3D model GPU attach
  (facing + order + attach presentation host residual closed 2026-07-13)
- Full CamoNetting W3D sub-object mesh GPU / heat-vision shader pass
  (host sub-object presentation residual closed 2026-07-13)
- Network camo-subobject / slave-order / vision-mood / SegLine replication
  (network deferred)

## Residual Host Playability — TurretAI Sleep/Passive + PARA_COG Bones + Laser Tile/Line3D (2026-07-13)
**Closed (host-testable mood matrix + parachute bones + laser texture residual):**
1. **TurretAI mood matrix Sleep/Passive residual** (Strategy Center Bombardment):
   - C++ `AttitudeType` / `getMoodMatrixActionAdjustment(MM_Action_Idle)` residual:
     - **Sleep** → MAA_Affect_Range_IgnoreAll (no idle mood-target acquire)
     - **Passive** → MAA_Affect_Range_WaitForAttack (only `last_damage_source` retaliate)
     - **Normal/Alert/Aggressive** → free idle mood-target residual
   - Object stores `ai_attitude` + `last_damage_source` (BodyModule residual via
     `take_damage_from`).
   - Turret bone pitch/yaw drawable residual: host exposes TurretAI angles for
     presentation consumers (`HostTurretBoneDrawable`).
   - Host-testable: Sleep blocks acquire; Passive free-acquire blocked; Passive +
     last damage retaliates + aims FirePitch; bone drawable pitch honesty.
   - Fail-closed: not full PartitionManager filter stack / W3D Turret bones
     (vision mood range host residual closed 2026-07-13 — see CamoNet Sub-Object
     + Physical Soldier Attach + Vision Mood + SegLine section).
2. **AmericaParachute bone PARA_COG / PARA_ATTCH / PARA_MAN residual**:
   - Host pristine bone offsets from GeometryHeight **10** layout (no W3D extract).
   - `updateOffsetsFromBones` residual → rider attach + rider/para sway pivots.
   - `calcSwayMtx` residual: pitch/roll about pivot → rider presentation displace
     + chute COG sway delta (logic position stays attach offset; sway is drawable).
   - Host-testable: COG above ATTCH; rider attach below origin; non-zero open sway;
     closed chute delta zero.
   - Fail-closed: not full W3D pristine bone extract / DeliverPayload cargo path.
3. **W3DLaserDraw texture / Line3D residual** (PatriotBinaryDataStream math path):
   - Texture `EXBinaryStream32.tga`, Tile **Yes**, TilingScalar **0.25**,
     InnerColor green A**180**.
   - tileFactor = length/width×aspect×scalar (retail width **4**, aspect **1** →
     length 100 → **6.25**).
   - Ground-skim residual: Z = max(z, ground+**2**).
   - Host builds **20** `HostLaserLine3DSegment` descriptors (width/tile/scroll/points).
   - Host-testable: tile factor math; skim pad; 20-segment list mid-arc elevated.
   - Fail-closed: not full WGPU SegLineRenderer / texture upload.
4. **VisionObjectName residual** (document only):
   - `strategy_center_vision_object_spawn_enabled_in_retail() == false`
     (C++ `//createVisionObject();` disabled). No spawn claim.
5. Tests (not log-only):
   - `strategy_center_turret_mood_matrix_sleep_passive_residual`
   - updated `battle_plan_constants_match_retail_residual` (Sleep/Passive/bone/vision)
   - updated `air_eject_parachute_gates` (PARA_COG bone matrix)
   - updated `patriot_laser_arc_segment_honesty` (tile/Line3D/skim)

**Still residual (fail-closed, not claimed):**
- Full PartitionManager filter stack / pathfinder mood matrix / W3D Turret bone GPU
  (Sleep/Passive + bone drawable + vision mood host residual closed 2026-07-13)
- Full W3DLaserDraw WGPU SegLineRenderer / texture upload for Patriot assist beams
  (tile factor + Line3D + ground skim + SegLine UV polyline host residual closed
  2026-07-13)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++
  — honesty residual closed 2026-07-13; no spawn claim)
- Full W3D pristine bone extract / DeliverPayload cargo plane path
  (PARA_COG host offsets + calcSwayMtx residual closed 2026-07-13)
- Full physical SpawnBehavior soldier Object / full AI / W3D model GPU attach
  (getClosestSlave + facing/order/attach presentation host residual closed
  2026-07-13)
- Full CamoNetting W3D sub-object mesh GPU / heat-vision shader pass
  (StealthLook + host sub-object presentation residual closed 2026-07-13)
- Network mood-matrix / PARA_COG / laser-tile replication (network deferred)

## Residual Host Playability — getClosestSlave + W3DLaser Arc + TurretAI Mood-Target + Camo StealthLook (2026-07-13)
**Closed (host-testable physical hive slaves + laser arc + mood-target + heat-vision residual):**
1. **Physical SpawnBehavior slave roster + getClosestSlave residual** (GLAStingerSite):
   - SpawnNumber **3** residual slots at SpawnPoint bone offsets (radius **12**, 120° ring).
   - Per-slave HP residual (MaxHealth **100**); `getClosestSlave` picks nearest alive
     slot to shooter world XZ for HiveStructureBody propagate via **host API**
     (`apply_host_hive_damage_from`) — not live skirmish `Object::take_damage` combat.
   - Kill marks slot dead; SpawnReplaceDelay respawn revives first dead slot.
   - Host-testable: closest-slot damage; independent HP; kill + respawn schedule.
   - Fail-closed: not full GLAInfantryStingerSoldier Object / AI / W3D bone attach;
     live combat still structure HP until damage-class routing is wired.
2. **W3DLaserDraw arc segment residual** (PatriotBinaryDataStream):
   - Cosine arc sample residual: mid peak = ArcHeight **30**, endpoints **0**.
   - Segments **20** residual segment endpoints host-sampled (C++ doDrawModule).
   - Host-testable: mid Z = base + ArcHeight; segment 0 start near base Z.
   - Fail-closed: not full texture / Line3D GPU draw / ground-height skim
     (tile/Line3D descriptor + ground skim host residual closed 2026-07-13 — see
     TurretAI Sleep/Passive + PARA_COG + Laser Tile section).
3. **TurretAI idle mood-target residual** (Strategy Center Bombardment):
   - `friend_checkForIdleMoodTarget` residual: idle gun acquires enemy in
     StrategyCenterGun range band (min **100** / max **400**), aims FirePitch **45**,
     flags `m_targetWasSetByIdleMood`.
   - Mood target leaving range / dying clears target so IDLESCAN can resume.
   - Host-testable: acquire + aim; out-of-range clear honesty.
   - Fail-closed: not full mood matrix Sleep/Passive / bone pitch drawable
     (Sleep/Passive + bone drawable host residual closed 2026-07-13 — see
     TurretAI Sleep/Passive + PARA_COG + Laser Tile section).
4. **CamoNetting StealthLook / heat-vision residual**:
   - Host of Drawable::setStealthLook: None / VisibleFriendly /
     VisibleFriendlyDetected / VisibleDetected / Invisible.
   - Detected residual → heat-vision second material pass opacity **1.0**.
   - Host-testable: detect cloaked structure → VISIBLE_DETECTED + opacity 1.
   - Fail-closed: not full W3D second material pass GPU / mine heat-vision hack.
5. **AlwaysHeal busy-interrupt residual**: documented as **dead code in retail C++**
   (early-return before AlwaysHeal branch). Host keeps AlwaysHeal **0.25** honesty
   constant and idle-only scan (matches unreachable retail path). Closed as
   fail-closed parity — not a missing live path.
6. Tests (not log-only):
   - `stinger_get_closest_slave_physical_roster_residual`
   - `stinger_get_closest_slave_roster_honesty` (module)
   - `patriot_laser_arc_segment_honesty` + updated binary laser residual (arc sample)
   - `strategy_center_turret_mood_target_residual`
   - updated `camo_netting_structure_attack_and_damage_reveal_residual` (StealthLook)
   - AlwaysHeal dead-path honesty in `auto_find_healing_gates`

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood matrix Sleep/Passive / bone pitch drawable matrix
  (host residual closed 2026-07-13 — see TurretAI Sleep/Passive + PARA_COG section)
- Full W3DLaserDraw texture / Line3D GPU draw for Patriot assist beams
  (tile/Line3D host residual closed 2026-07-13 — see TurretAI Sleep/Passive section)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute bone PARA_COG / DeliverPayload residual
  (PARA_COG host residual closed 2026-07-13 — see TurretAI Sleep/Passive section)
- Full physical SpawnBehavior soldier Object / AI / W3D model attach
  (getClosestSlave + per-slave HP/position host residual closed 2026-07-13)
- Full CamoNetting sub-object net mesh visual / W3D heat-vision GPU pass
  (StealthLook + second-pass opacity host residual closed 2026-07-13)
- Network mood-target / closest-slave / laser-arc / heat-vision replication
  (network deferred)

## Residual Host Playability — Parachute Pitch/Roll Sway + LaserUpdate Endpoint Track (2026-07-13)
**Closed (host-testable AmericaParachute pitch/roll sway + Patriot LaserUpdate residual):**
1. **AmericaParachute pitch/roll sway residual** (ParachuteContain + ParachuteLocomotor):
   - PitchRateMax / RollRateMax **60** deg/s → **π/90** rad/frame seed band.
   - Deterministic host seed: pitch **+½ max**, roll **−½ max** on chute open.
   - Open-chute spring/damper residual each frame:
     stiffness **0.02**, damping **0.01** (ParachuteLocomotor).
   - LowAltitudeDamping **0.2** when height ≤ **20** (ALTITUDE_DAMP_START).
   - Freefall residual does not sway; land clears pitch/roll state.
   - Host-testable: freefall zero; open → non-zero pitch/roll; land clear.
   - Fail-closed: not full bone PARA_COG / rider sway / DeliverPayload matrix.
2. **LaserUpdate endpoint track + W3DLaserDraw param residual**
   (PatriotBinaryDataStream assist beams):
   - Each frame refreshes start/end from live `from_id` / `to_id` positions
     (C++ `LaserUpdate::updateStartPos` / `updateEndPos` without bone).
   - Dead/missing target freezes last end residual.
   - W3DLaserDraw honesty: NumBeams **1**, ScrollRate **-0.25**, ArcHeight **30**,
     InnerBeamWidth **4**, Segments **20**; host advances scroll residual/frame.
   - Host-testable: endpoint follows moved victim; scroll advances; lifetime still 18.
   - Fail-closed: not full texture / arc segment GPU draw.
3. Tests (not log-only):
   - `eject_pilot_parachute_pitch_roll_sway_residual`
   - updated `patriot_assist_binary_data_stream_laser_residual`
     (endpoint track + ScrollRate + draw params)
   - module unit tests in `host_usa_pilot` / `host_base_defense`
     (sway matrix / LowAltitudeDamping / track + freeze / scroll)

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood-target / bone pitch matrix
- Full W3DLaserDraw texture / arc GPU draw for Patriot assist beams
  (endpoint track + draw-param honesty host residual closed 2026-07-13)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute bone PARA_COG / DeliverPayload residual
  (pitch/roll spring-damper host residual closed 2026-07-13)
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / W3D heat-vision drawable matrix
- Network parachute-sway / laser-endpoint replication (network deferred)

## Residual Host Playability — TurretAI HoldTurret/Idle-Recenter + CamoNetting FriendlyOpacity (2026-07-13)
**Closed (host-testable Strategy Center TurretAI HoldTurret + idle-recenter + CamoNetting FriendlyOpacity residual):**
1. **TurretAI HoldTurret + idle-recenter residual** (Strategy Center Turret):
   - After idle-scan complete: HOLD for RecenterTime default
     **2×LOGICFRAMES_PER_SECOND → 60** frames (Strategy Center does not override).
   - Angles freeze at scan desired during Hold; busy cancels hold.
   - Hold complete → RECENTER residual steps pitch/yaw to NaturalTurretAngle
     **-90** / NaturalTurretPitch **45** at Turn/PitchRate **2** deg/frame.
   - Idle-recenter complete schedules next idle-scan (C++ IDLESCAN → HOLD →
     RECENTER → IDLE residual chain).
   - Host-testable: hold freeze; elapse → recenter; natural + reschedule; busy cancel.
   - Fail-closed: not full TurretAI mood-target / bone pitch matrix.
2. **CamoNetting FriendlyOpacity residual** (StealthUpdate on CamoNetting structures):
   - FriendlyOpacityMin **50%** / Max **100%** residual on cloaked / revealed.
   - Upgrade apply + re-cloak set min; attack/damage reveal sets max.
   - Cloaked pulse residual: `min + (max-min)×(0.5+0.5·sin(phase))`,
     phase rate **0.2** (C++ setEffectiveOpacity sin path mapped to min..max).
   - Host-testable: cloak min; damage reveal max; re-cloak + pulse in range.
   - Fail-closed: not full W3D heat-vision / sub-object camo net visual matrix.
3. Tests (not log-only):
   - `strategy_center_turret_hold_and_idle_recenter_residual`
   - updated `strategy_center_turret_idle_scan_residual` (Hold after complete)
   - updated `camo_netting_structure_attack_and_damage_reveal_residual`
     (FriendlyOpacity min/max/pulse)
   - module unit tests in `host_strategy_center` / `host_upgrades`
     (RecenterTime / Hold elapsed / FriendlyOpacity / pulse matrix)

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood-target / bone pitch matrix
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / W3D heat-vision drawable matrix
- Network HoldTurret / camo-opacity replication (network deferred)

## Residual Host Playability — TurretAI Idle-Scan + CamoNetting USING_ABILITY/OrderIdle (2026-07-13)
**Closed (host-testable Strategy Center TurretAI idle-scan + CamoNetting USING_ABILITY / OrderIdleEnemies residual):**
1. **TurretAI idle-scan residual** (Strategy Center AIUpdateInterface Turret):
   - MinIdleScanInterval **500**ms → **15** frames, MaxIdleScanInterval
     **1000**ms → **30** frames.
   - MinIdleScanAngle **0**, MaxIdleScanAngle **60** deg off NaturalTurretAngle.
   - Bombardment ACTIVE schedules first scan after Min interval; idle gun
     rotates toward NaturalTurretAngle ± deterministic mid offset (**±30**),
     pitch holds NaturalTurretPitch **45**.
   - Complete enters HoldTurret residual (see HoldTurret/Idle-Recenter section).
   - Busy (attacking / target / recenter / fire) cancels mid-scan residual.
   - Host-testable: schedule; step off natural; complete → hold; busy cancel.
   - Fail-closed: not full TurretAI mood-target / bone pitch matrix.
2. **CamoNetting USING_ABILITY + OrderIdleEnemies residual**:
   - StealthForbiddenConditions residual now includes **USING_ABILITY**
     (`OBJECT_STATUS_IS_USING_ABILITY` / SpecialAbility AI residual).
   - OrderIdleEnemiesToAttackMeUponReveal residual: on CamoNetting structure
     reveal, idle enemy units within their vision range wake and AttemptToTarget
     (host residual: set target + Attacking).
   - Host-testable: using_ability uncloaks; idle enemy orders on reveal.
   - Fail-closed: not full sub-object camo net visual (FriendlyOpacity host residual closed 2026-07-13).
3. Tests (not log-only):
   - `strategy_center_turret_idle_scan_residual`
   - updated `camo_netting_structure_attack_and_damage_reveal_residual`
     (USING_ABILITY + OrderIdle)
   - module unit tests in `host_strategy_center` / `host_upgrades`
     (idle-scan matrix / USING_ABILITY / OrderIdle range gates)

**Still residual (fail-closed, not claimed):**
- Full TurretAI mood-target / bone pitch matrix
  (HoldTurret + idle-recenter host residual closed 2026-07-13 — see HoldTurret section)
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / W3D heat-vision drawable matrix
  (FriendlyOpacity host residual closed 2026-07-13 — see HoldTurret + FriendlyOpacity section)
- Network idle-scan / camo-order-idle replication (network deferred)

## Residual Host Playability — Turret Natural Pitch/Yaw + Parachute FreeFallDamage (2026-07-13)
**Closed (host-testable Strategy Center turret natural-position angles + AmericaParachute FreeFallDamage/fudge residual):**
1. **Turret natural-position pitch/yaw residual** (Strategy Center AIUpdateInterface Turret):
   - NaturalTurretAngle **-90**, NaturalTurretPitch **45**, FirePitch **45**.
   - TurretTurnRate / TurretPitchRate **60** deg/s → **2** deg/frame @ 30 FPS.
   - StrategyCenterGun fire residual aims pitch/yaw at target (atan2 + FirePitch).
   - `isTurretInNaturalPosition` residual: natural angles + idle/busy coast gate.
   - Recenter residual steps angles toward natural; frame count is angle-based
     (60° → 30 frames) with busy-coast fallback of **30** frames.
   - Host-testable: fire leaves natural; recenter restores NaturalTurretAngle/Pitch;
     pack deferred for non-natural; pack immediate when natural.
   - Fail-closed: not full TurretAI idle-scan state machine / bone pitch matrix.
2. **AmericaParachute FreeFallDamage + low-altitude open fudge residual**:
   - FreeFallDamagePercent default **0.5** → 50% max-health residual when chute
     is destroyed mid-air while significantly above terrain
     (`destroy_eject_parachute_midair` residual).
   - Low-altitude open fudge: if startZ − ground < **2×OpenDist**, fudge start
     height to `ground + 2×OpenDist` so chute can still open (C++ ParachuteContain).
   - Host-testable: fudge rewrites start height; FreeFallDamage closes chute + HP;
     ground path rejects FreeFallDamage.
   - Fail-closed: not full sway / pitch-roll / DeliverPayload bone matrix.
3. Tests (not log-only):
   - `eject_pilot_parachute_open_fudge_and_free_fall_damage_residual`
   - updated `strategy_center_bombardment_turret_fire_residual` (aim angles)
   - updated `strategy_center_delayed_set_battle_plan_and_turret_recenter_residual`
     (angle step + restore)
   - module unit tests in `host_strategy_center` / `host_usa_pilot`
     (angle matrix / fudge / FreeFallDamage gates)

**Still residual (fail-closed, not claimed):**
- Full TurretAI idle-scan Min/MaxIdleScanAngle state machine / bone matrix
  (host residual closed 2026-07-13 — see TurretAI Idle-Scan + CamoNetting section;
  full HoldTurret / mood-target / bone pitch still open)
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting USING_ABILITY / OrderIdleEnemies / opacity drawable matrix
  (USING_ABILITY + OrderIdle host residual closed 2026-07-13 — see TurretAI Idle-Scan
  + CamoNetting section; opacity / sub-object visual still open)
- Network turret-angle / FreeFallDamage replication (network deferred)

## Residual Host Playability — BinaryDataStream Laser + DetectionRate Sleep (2026-07-13)
**Closed (host-testable Patriot assist laser feedback + StealthDetector DetectionRate residual):**
1. **BinaryDataStream laser residual** (`AssistedTargetingUpdate::makeFeedbackLaser`):
   - On assist accept, residual spawns two beams with template
     `PatriotBinaryDataStream`.
   - `LaserFromAssisted`: requestor → assistant.
   - `LaserToTarget`: assistant → victim.
   - DeletionUpdate Min/MaxLifetime **600**ms → **18** frames residual lifetime.
   - Host-testable: pair endpoints; lifetime expiry; honesty counters.
   - Fail-closed: not full W3DLaserDraw / LaserUpdate client drawable
     (arc / scroll / texture / NumBeams).
2. **StealthDetectorUpdate DetectionRate residual** (Strategy Center ModuleTag_16):
   - DetectionRate **500**ms → **15** frames sleep between scans.
   - `setSDEnabled(true)` residual: first scan immediate (`next_scan_frame = 0`).
   - After scan: `next_detection_scan_frame = frame + 15`.
   - `markAsDetected(updateRate + 1)` residual hold **16** frames.
   - Host-testable: mid-rate no re-scan; re-scan at +15; hold honesty.
   - Fail-closed: VisionObjectName spawn residual still not claimed
     (createVisionObject disabled in retail C++); non-SC detectors keep
     continuous legacy scan (rate_frames=0).
3. Tests (not log-only):
   - `patriot_assist_binary_data_stream_laser_residual`
   - `strategy_center_stealth_detector_detection_rate_residual`
   - updated `patriot_assisted_targeting_request_assist_range_residual`
     (laser honesty)
   - updated `strategy_center_stealth_detector_enable_residual` (rate fields)
   - module unit tests in `host_base_defense` / `host_strategy_center`
     (laser matrix / DetectionRate gates)

**Still residual (fail-closed, not claimed):**
- Full W3DLaserDraw / LaserUpdate client drawable for Patriot assist beams
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / opacity drawable matrix
- Network laser / DetectionRate replication (network deferred)

## Residual Host Playability — Stinger HiveStructureBody + CamoNetting Structure Reveal (2026-07-13)
**Closed (host-testable HiveStructureBody/SpawnBehavior + CamoNetting structure StealthUpdate residual):**
1. **HiveStructureBody + SpawnBehavior residual** (`GLAStingerSite` ModuleTag_04/06):
   - SpawnNumber **3** residual soldiers (MaxHealth **100** each).
   - Propagate residual (SMALL_ARMS / SNIPER / POISON / RADIATION / SURRENDER /
     MICROWAVE class) damages active slave, not structure HP.
   - Swallow residual (SNIPER / POISON / SURRENDER) eats damage when **0** slaves.
   - HitStructure residual always damages the building body.
   - SPAWNS_ARE_THE_WEAPONS residual: site cannot fire with **0** soldiers.
   - SpawnReplaceDelay **30000**ms → **900** frames residual respawn.
   - Host-testable: propagate preserves structure HP; kill slave; swallow at 0;
     HitStructure damages; 0-soldier no-fire; respawn restores capacity.
   - Fail-closed: not full physical slave objects / bone attach / getClosestSlave.
2. **CamoNetting structure attack/damage reveal residual** (`StealthUpdate` +
   `Upgrade_GLACamoNetting` on Tunnel Network / Stinger / Slth buildings):
   - StealthForbiddenConditions residual: **ATTACKING** + **TAKING_DAMAGE**.
   - StealthDelay **2500**ms → **75** frames re-cloak residual.
   - Host-testable: damage uncloaks; attacking uncloaks; idle after delay re-cloaks.
   - Fail-closed: not full USING_ABILITY / opacity / OrderIdleEnemiesToAttackMe /
     sub-object camo net visual.
3. Tests (not log-only):
   - `stinger_hive_structure_body_and_spawn_respawn_residual`
   - `camo_netting_structure_attack_and_damage_reveal_residual`
   - updated `stinger_site_residual_dual_fire_and_ap_rockets` (SpawnNumber honesty)
   - module unit tests in `host_base_defense` / `host_upgrades` (hive matrix /
     StealthDelay residual honesty)

**Still residual (fail-closed, not claimed):**
- Full BinaryDataStream laser drawable feedback for Patriot assist beams
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full physical SpawnBehavior slave objects / HiveStructureBody getClosestSlave matrix
- Full CamoNetting sub-object net visual / opacity drawable matrix
- Network hive / camo-structure-stealth replication (network deferred)

## Residual Host Playability — Patriot AssistedTargeting + DemoTrap Mode (2026-07-13)
**Closed (host-testable AssistedTargetingUpdate + DemoTrapUpdate weapon-slot mode residual):**
1. **AssistedTargetingUpdate residual** (AmericaPatriotBattery ModuleTag_07):
   - PRIMARY/AA fire with `RequestAssistRange` **200** issues assist request residual.
   - Same-team equivalent Patriots (stock / Lazr / SupW family) within range that are
     free to assist accept a clip of **AssistingClipSize = 4** assist-weapon shots
     (`PatriotMissileAssistWeapon` dmg **25** / range **450**; Lazr dmg **35**;
     SupW dmg **25** + EMPPatriotEffectSpheroid residual).
   - Clip fires on DelayBetweenShots **250**ms → **8** frames residual cadence.
   - Host-testable: near assistant accepts + damages; far (range > 200) rejects;
     Lazr↔stock non-equivalent rejects; AssistingClipSize honesty.
   - Fail-closed: not full BinaryDataStream laser drawable feedback
     (`LaserFromAssisted` / `LaserToTarget` residual audio cue only).
2. **DemoTrapUpdate weapon-slot mode residual**:
   - DefaultProximityMode **Yes** → starts Proximity (SECONDARY residual).
   - ManualModeWeaponSlot (TERTIARY residual) disables proximity scan.
   - DetonationWeaponSlot (PRIMARY residual) → manual detonate residual.
   - Host-testable: manual mode enemy-in-range does not detonate; switch to
     Proximity detonates; Detonate mode manual-detonates.
   - Fail-closed: not full PreAttack scoop animation / weapon-lock UI matrix.
3. Tests (not log-only):
   - `patriot_assisted_targeting_request_assist_range_residual`
   - `lazr_patriot_assist_equivalent_family_residual`
   - `demo_trap_weapon_slot_mode_residual`
   - module unit tests in `host_base_defense` / `host_mines` (assist matrix /
     mode residual honesty)

**Still residual (fail-closed, not claimed):**
- Full BinaryDataStream laser drawable feedback for Patriot assist beams
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full SpawnBehavior / HiveStructureBody / Stinger soldier death matrix
  (host residual closed 2026-07-13 — see Stinger HiveStructureBody + CamoNetting section)
- Full CamoNetting structure attack reveal matrix
  (host residual closed 2026-07-13 — see Stinger HiveStructureBody + CamoNetting section)
- Network assist / demo-trap-mode replication (network deferred)

## Residual Host Playability — Delayed setBattlePlan ACTIVE + Turret Recenter (2026-07-13)
**Closed (host-testable BattlePlanUpdate delayed ACTIVE setBattlePlan + Bombardment recenter residual):**
1. **Delayed ACTIVE-after-unpack setBattlePlan residual** (`BattlePlanUpdate::setStatus`):
   - Plan select only starts door residual (UNPACKING OPENING); army buffs,
     building bonuses, StealthDetector enable, StrategyCenterGun equip apply
     **only** when door reaches ACTIVE / WAITING_TO_CLOSE (`setBattlePlan(plan)`).
   - Plan switch → PACKING / CLOSING → `setBattlePlan(NONE)` clears army/building
     residual + BattlePlanChangeParalyze (150 frames) on legal members.
   - Pack complete (TransitionIdleTime **0**) → unpack new plan → ACTIVE apply.
   - Host-testable: mid-unpack no army buffs; ACTIVE grants; pack clears before
     new ACTIVE; turret/stealth only while ACTIVE.
   - Fail-closed: not full AI turret pitch/yaw angle matrix / VisionObjectName.
2. **Bombardment turret recenter residual** (`recenterTurret` / `isTurretInNaturalPosition`):
   - Leaving Bombardment while gun residual is non-natural (attacking / has target /
     fired within **30** frames) delays pack by **TurretRecenterFrames = 30**.
   - During recenter door stays ACTIVE (buffs still applied); then PACKING clears.
   - Natural idle gun packs immediately.
   - Fail-closed: not full pitch scan / natural-position angle matrix.
3. Tests (not log-only):
   - `strategy_center_delayed_set_battle_plan_and_turret_recenter_residual`
   - updated door / buff / paralyze / turret / stealth residual tests
   - module unit tests in `host_strategy_center` (BecameActive / BeganPacking /
     recenter matrix / natural gate)

**Still residual (fail-closed, not claimed):**
- Full AI turret pitch/yaw natural-position angle matrix (recenter frame gate closed)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network delayed-battle-plan / recenter replication (network deferred)

## Residual Host Playability — StealthDetector Enable + BattlePlan Door Animation (2026-07-13)
**Closed (host-testable Strategy Center StealthDetectorUpdate + pack/unpack door residual):**
1. **StealthDetectorUpdate residual** (`AmericaStrategyCenter` ModuleTag_16):
   - DetectionRange **500**, DetectionRate **500**ms → **15** frames,
     InitiallyDisabled **Yes**.
   - SearchAndDestroy → `setSDEnabled(true)` residual: `is_detector` + range **500**.
   - Leaving S&D → `setSDEnabled(false)` residual: clear detector + range.
   - Host-testable: stealthed enemy at dist 400 detected under S&D; at 600 not;
     before S&D / after leave not detected.
   - Fail-closed: VisionObjectName createVisionObject disabled in retail C++
     (ShroudRevealToAllRange path); not full DetectionRate sleep phasing.
2. **Pack/unpack door model-condition residual** (`BattlePlanUpdate`):
   - AnimationTime **7000**ms → **210** frames; TransitionIdleTime **0**.
   - DOOR_1 Bombardment / DOOR_2 HoldTheLine / DOOR_3 SearchAndDestroy.
   - Select → OPENING residual; after 210 frames → WAITING_TO_CLOSE (ACTIVE).
   - Plan switch → CLOSING residual → IDLE → new OPENING (TransitionIdleTime 0).
   - Pack/unpack audio residual queued.
   - Delayed setBattlePlan ordering closed 2026-07-13 (see Delayed setBattlePlan
     ACTIVE + Turret Recenter section).
3. Tests (not log-only):
   - `strategy_center_stealth_detector_enable_residual`
   - `strategy_center_battle_plan_door_animation_residual`
   - module unit tests in `host_strategy_center` (door matrix / stealth honesty /
     AnimationTime / DetectionRange constants)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate delayed ACTIVE-after-unpack setBattlePlan ordering
  (host residual closed 2026-07-13 — see Delayed setBattlePlan ACTIVE + Turret Recenter)
- Full Bombardment turret natural-position recenter / pitch scan matrix
  (recenter host residual closed 2026-07-13 — see Delayed setBattlePlan ACTIVE +
  Turret Recenter; full pitch/yaw angle matrix still open)
- Full VisionObjectName spawn residual (createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network stealth-detector / door-animation replication (network deferred)

## Residual Host Playability — Same-Player PartitionFilter + Parachute OpenDist (2026-07-13)
**Closed (host-testable PilotFindVehicle PartitionFilterPlayer + AmericaParachute OpenDist residual):**
1. **PartitionFilterPlayer residual** (`PilotFindVehicleUpdate::scanClosestTarget`):
   - C++ `PartitionFilterPlayer(me->getControllingPlayer(), true)` residual.
   - Host killpilot captures `unmanned_owner_team` then sets Neutral.
   - AI auto-scan accepts same-team or Neutral with matching owner; rejects
     foreign-owner Neutral unmanned (China sniped hull for USA pilot).
   - Player Enter recrew path is not gated (AI scan residual only).
   - Fail-closed: not full same-map PartitionFilterSameMapStatus.
2. **AmericaParachute OpenDist residual** (`ParachuteContain` / OpenDist **100**):
   - Air-ejected pilot freefalls faster (`40` u/frame) until fallen ≥ **100**.
   - Then opens chute residual (slower `20` u/frame sink + `ParachuteOpen` audio).
   - Lands and clears parachuting residual as before.
   - Fail-closed: not full sway / pitch-roll / DeliverPayload matrix.
3. Tests (not log-only):
   - `pilot_find_vehicle_same_player_partition_filter_residual`
   - `eject_pilot_parachute_open_dist_residual`
   - updated `eject_pilot_air_ocl_parachute_residual` (OpenDist honesty)
   - module unit tests in `host_usa_pilot` (same-player / OpenDist gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
  (host residual closed 2026-07-13 — see StealthDetector Enable + BattlePlan Door Animation;
  delayed setBattlePlan ordering closed 2026-07-13 — see Delayed setBattlePlan ACTIVE +
  Turret Recenter)
- Full Bombardment turret natural-position recenter / pitch scan matrix
  (recenter host residual closed 2026-07-13 — see Delayed setBattlePlan ACTIVE +
  Turret Recenter; full pitch/yaw still open)
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
  (StealthDetector enable host residual closed 2026-07-13 — see StealthDetector + Door section;
  VisionObjectName still open / createVisionObject disabled in retail C++)
- Full AmericaParachute sway / pitch-roll / DeliverPayload residual
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network same-player / parachute-open replication (network deferred)

## Residual Host Playability — Eject DieMux + PilotFindVehicle CollideModule (2026-07-13)
**Closed (host-testable EjectPilotDie DeathTypes/ExemptStatus + CollideModule residual):**
1. **DieMux residual** (`EjectPilotDie` / `DieMuxData`):
   - `DeathTypes = ALL -CRUSHED -SPLATTED`: Crushed / Splatted residual deaths
     do **not** eject (`HostDeathType` on `ObjectStatus`).
   - `ExemptStatus = HIJACKED`: hijacked vehicles do **not** eject.
   - Normal combat residual death still ejects (Veteran+ gates preserved).
   - Fail-closed: not full DeathType enum / multi-bit status matrix.
2. **PilotFindVehicle CollideModule residual** (`VeterancyCrateCollide`):
   - wouldLikeToCollideWith host gates: RequiredKindOf VEHICLE / ForbiddenKindOf
     DOZER, not significantly above terrain, not airborne locomotor, trainable,
     canGainExpForLevel(pilot levels) — Heroic vehicle rejects Veteran pilot.
   - Nearest valid Rookie unmanned still Enter → recrew residual.
   - Fail-closed: same-player PartitionFilter residual closed 2026-07-13
     (see Same-Player PartitionFilter + Parachute OpenDist section).
3. Tests (not log-only):
   - `eject_pilot_die_mux_death_types_and_hijacked_residual`
   - `pilot_find_vehicle_collide_module_would_like_residual`
   - module unit tests in `host_usa_pilot` (death types / collide gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
  (createVisionObject disabled in retail C++ — ShroudRevealToAllRange path)
- Full AmericaParachute container OpenClose / DeliverPayload fall-physics matrix
  (OpenDist host residual closed 2026-07-13 — see Same-Player + OpenDist section)
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Full same-player PartitionFilter for PilotFindVehicle (host Neutral unmanned path)
  (host residual closed 2026-07-13 — see Same-Player PartitionFilter + Parachute OpenDist)
- Network die-mux / collide-module replication (network deferred)

## Residual Host Playability — Air Parachute Eject + USA Infantry AutoFindHealing (2026-07-13)
**Closed (host-testable EjectPilotDie air OCL + non-pilot AutoFindHealing residual):**
1. **Air OCL parachute residual** (`EjectPilotDie` / `isSignificantlyAboveTerrain`):
   - When dying eligible vehicle height > **576** (`-(3*3)*Gravity`, Gravity=**-64**)
     or `airborne_target`, residual uses OCL_EjectPilotViaParachute path.
   - Pilot spawns elevated + `OBJECT_STATUS_PARACHUTING` residual; linear sink
     **20** u/frame until ground; InvulnerableTime **60** frames still applies.
   - Ground death (y≈0, not airborne) stays OCL_EjectPilotOnGround residual.
   - Fail-closed: not full AmericaParachute container OpenClose / fall physics.
2. **USA infantry AutoFindHealing residual** (beyond pilot-only):
   - Templates: Pilot / Ranger / MissileDefender / Pathfinder / ColonelBurton
     (+ general variants) residual AutoFindHealingUpdate module.
   - Same AI-only idle gates: ScanRate **30** frames / ScanRange **300** /
     NeverHeal **0.85** → SeekingHealing at HealPad.
   - Fail-closed: AlwaysHeal busy path still not claimed (C++ early-return unreachable);
     China/GLA infantry not claimed.
3. Tests (not log-only):
   - `eject_pilot_air_ocl_parachute_residual`
   - `usa_infantry_auto_find_healing_hospital_path_residual`
   - module unit tests in `host_usa_pilot` (air OCL gates / infantry template / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
  (createVisionObject disabled in retail C++ — ShroudRevealToAllRange path)
- Full AmericaParachute container OpenClose / DeliverPayload fall-physics matrix
- Full PilotFindVehicleUpdate CollideModule wouldLikeToCollideWith matrix
  (host residual closed 2026-07-13 — see Eject DieMux + PilotFindVehicle CollideModule)
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt path (dead code in retail C++)
- Network air-eject / infantry-auto-heal replication (network deferred)

## Residual Host Playability — Pilot Base-Center + AutoFindHealing (2026-07-13)
**Closed (host-testable PilotFindVehicle base-center fallback + AutoFindHealingUpdate residual):**
1. **Base-center fallback residual** (`PilotFindVehicleUpdate::m_didMoveToBase`):
   - When AI idle pilot scan finds **no** recrewable vehicle, residual issues one
     Move toward team CommandCenter (`getAiBaseCenter` host residual).
   - Latches `pilot_did_move_to_base` (no repeat); clears on successful vehicle Enter.
   - Fail-closed: CommandCenter-only (not any-structure fallback); not full CollideModule.
2. **AutoFindHealingUpdate residual** (`AmericaInfantryPilot` ModuleTag_06):
   - AI-only idle injured pilot auto-scan (C++ human early-return; host `is_local` gate).
   - ScanRate **1000**ms → **30** frames; ScanRange **300**; NeverHeal **0.85**.
   - Nearest HealPad residual → SeekingHealing → existing HealPad HP ticks.
   - Fail-closed: AlwaysHeal busy-interrupt path not claimed; pilot-template residual only
     (non-pilot USA infantry residual closed 2026-07-13 — see Air Parachute Eject +
     USA Infantry AutoFindHealing section).
3. Tests (not log-only):
   - `pilot_find_vehicle_base_center_fallback_residual`
   - `pilot_auto_find_healing_hospital_path_residual`
   - module unit tests in `host_usa_pilot` (base-center / auto-heal gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
  (host residual closed 2026-07-13 — see Air Parachute Eject + USA Infantry AutoFindHealing)
- Full PilotFindVehicleUpdate CollideModule wouldLikeToCollideWith matrix
- Full AutoFindHealingUpdate AlwaysHeal busy-interrupt / non-pilot infantry matrix
  (non-pilot host residual closed 2026-07-13 — see Air Parachute Eject section)
- Network base-center / auto-heal replication (network deferred)

## Residual Host Playability — Eject VeterancyLevels Gate + PilotFindVehicle (2026-07-13)
**Closed (host-testable EjectPilotDie VeterancyLevels + PilotFindVehicleUpdate residual):**
1. **VeterancyLevels residual** (`EjectPilotDie` / DieMux `VeterancyLevels = ALL -REGULAR`):
   - Rookie / LEVEL_REGULAR eligible vehicles do **not** eject pilots on death.
   - Veteran / Elite / Heroic residual still ejects `AmericaInfantryPilot`.
   - Honesty records REGULAR-gate blocks vs successful ejects.
   - Fail-closed: not full DeathTypes / ExemptStatus DieMux matrix.
2. **PilotFindVehicleUpdate residual** (`AmericaInfantryPilot` ModuleTag_07):
   - AI-only idle pilot auto-scan (C++ human → sleep forever; host `is_local` gate).
   - ScanRate **1000**ms → **30** frames; ScanRange **300**; MinHealth **0.5**.
   - Nearest recrewable unmanned vehicle meeting MinHealth → Enter → recrew path.
   - Fail-closed: not full base-center fallback / VeterancyCrate same-team exp matrix
     (base-center host residual closed 2026-07-13 — see Pilot Base-Center + AutoFindHealing).
3. Tests (not log-only):
   - `eject_pilot_veterancy_levels_all_minus_regular_residual`
   - `pilot_find_vehicle_ai_auto_scan_min_health_residual`
   - module unit tests in `host_usa_pilot` (vet gate / find-vehicle gates / honesty)

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
- Full PilotFindVehicleUpdate base-center fallback / CollideModule matrix
  (base-center host residual closed 2026-07-13 — see Pilot Base-Center + AutoFindHealing;
  CollideModule still open)
- Full AutoFindHealingUpdate hospital path residual
  (host residual closed 2026-07-13 — see Pilot Base-Center + AutoFindHealing)
- Network eject-vet-gate / pilot-find-vehicle replication (network deferred)

## Residual Host Playability — Bombardment Turret + Eject InvulnerableTime (2026-07-13)
**Closed (host-testable StrategyCenterGun Bombardment turret + OCL InvulnerableTime residual):**
1. **Bombardment turret residual** (`BattlePlanUpdate::enableTurret` / StrategyCenterGun):
   - Strategy Center spawns with turret residual **disabled** (strip kind-based weapon).
   - Bombardment plan equips PRIMARY `StrategyCenterGun`: PrimaryDamage **200** /
     radius **25**, range **400**, min **100**, Delay **7000**ms → **210** frames.
   - Auto-fire residual while Bombardment active (nearest in-band enemy + splash).
   - Leaving Bombardment clears weapon residual.
   - Fail-closed: not full natural-position recenter / pack-unpack door matrix.
2. **InvulnerableTime residual** (`OCL_EjectPilotOnGround` InvulnerableTime = **2000**ms):
   - Ejected pilot residual is damage-immune for **60** frames (`goInvulnerable` host).
   - Enemies skip auto-fire on invulnerable pilots (ALLIES relationship residual).
   - Timer expiry restores normal damage.
   - Fail-closed: not full UNDETECTED_DEFECTOR FX flash matrix.
3. Tests (not log-only):
   - `strategy_center_bombardment_turret_fire_residual`
   - `eject_pilot_invulnerable_time_residual`
   - module unit tests in `host_strategy_center` / `host_usa_pilot`

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret natural-position recenter / pitch scan matrix
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
- Full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Full EjectPilotDie VeterancyLevels ALL -REGULAR gate residual
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Network battle-plan turret / eject-invuln replication (network deferred)

## Residual Host Playability — BattlePlan Paralyze + EjectPilotDie (2026-07-13)
**Closed (host-testable BattlePlanChangeParalyzeTime + USA EjectPilotDie residual):**
1. **BattlePlanChangeParalyze residual** (`BattlePlanUpdate` / Strategy Center):
   - First plan select: army residual bonuses only (no paralyze — matches C++
     PLANSTATUS_NONE only on pack-for-change).
   - Plan **switch**: legal army members receive DISABLED_PARALYZED for
     **5000 ms → 150 frames** (`BattlePlanChangeParalyzeTime`).
   - Paralyzed units cannot move/attack until timer expires (`tick_disabled_paralyzed`).
   - Fail-closed: not full pack/unpack door animation / turret recenter matrix.
2. **EjectPilotDie residual** (`EjectPilotDie` / OCL_EjectPilotOnGround):
   - Eligible USA ground vehicles (Humvee / Tomahawk / Crusader / Paladin /
     Avenger / Microwave + general variants) spawn `AmericaInfantryPilot` on death.
   - Pilot residual starts **VETERAN**; can recrew via existing pilot residual path.
   - Fail-closed: unmanned vehicles do **not** eject; aircraft air-parachute OCL
     not claimed (ground residual only).
3. Tests (not log-only):
   - `strategy_center_battle_plan_paralyze_residual_on_plan_change`
   - `eject_pilot_die_spawns_pilot_on_vehicle_death_residual`
   - module unit tests in `host_strategy_center` / `host_usa_pilot`

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full Bombardment turret enable residual (host residual closed 2026-07-13 —
  see Bombardment Turret + Eject InvulnerableTime section; natural-position recenter still open)
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Full EjectPilotDie air OCL parachute / isSignificantlyAboveTerrain matrix
- Full InvulnerableTime post-eject residual (host residual closed 2026-07-13 —
  see Bombardment Turret + Eject InvulnerableTime section)
- Full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Full EjectPilotDie VeterancyLevels ALL -REGULAR gate residual
  (host residual closed 2026-07-13 — see Eject VeterancyLevels Gate + PilotFindVehicle section)
- Network battle-plan paralyze / eject-pilot replication (network deferred)

## Residual Host Playability — Demo PlusFire SUICIDED + CommandSetUpgrade (2026-07-13)
**Closed (host-testable Demo_SuicideDynamitePackPlusFire intentional suicide + CommandSetUpgrade residual):**
1. **PlusFire SUICIDED residual** (`Demo_SuicideDynamitePackPlusFire`):
   - Intentional TertiarySuicide (`Demo_Command_TertiarySuicide` / `CommandType::DemoTertiarySuicide`)
     on SuicideBomb-tagged non-terrorist Demo units applies Primary **500**/r**18** +
     Secondary **300**/r**50**.
   - Self is consumed; process_destroy_list skips `Demo_DestroyedWeapon` double-fire
     (`demo_suicided_detonating` residual flag).
   - Fail-closed: Terrorist SUICIDED path stays `Demo_SuicideDynamitePack` (700).
2. **CommandSetUpgrade residual** (`CommandSetUpgrade` TriggeredBy SuicideBomb):
   - QueueUpgrade → complete sets `command_set_override` to retail
     `Demo_*CommandSetUpgrade` (e.g. `Demo_GLAInfantryRebelCommandSetUpgrade`).
   - TertiarySuicide gated by upgrade tag + command_set_override residual.
   - Post-research spawns inherit tag + command_set_override residual.
3. Tests (not log-only):
   - `demo_tertiary_suicide_plus_fire_command_set_residual`
   - module unit tests: tertiary gate / command set names / PlusFire rings

**Still residual (fail-closed, not claimed):**
- Full PrerequisiteSciences rank tree / control-bar science visibility matrix
- Full IsTrainable exclusive ExperienceTracker module matrix
- Full SlowDeath SUICIDED fling / OCL poison particle bone matrix
- Full control-bar CommandSet slot UI matrix (host residual is override + gate)
- Network unit-training / suicide-bomb replication (network deferred)

## Residual Host Playability — SCIENCE Unit Training + Demo SuicideBomb (2026-07-13)
**Closed (host-testable VeterancyGainCreate science spawn + Demo_Upgrade_SuicideBomb death residual):**
1. **SCIENCE unit-training residual** (`VeterancyGainCreate`):
   - `SCIENCE_RedGuardTraining` → Red Guard spawn **VETERAN** (+20% HP residual)
   - `SCIENCE_BattlemasterTraining` → Battlemaster spawn **ELITE** (+30% HP residual)
   - `SCIENCE_ArtilleryTraining` → Inferno / Nuke Cannon spawn **VETERAN**
   - `SCIENCE_TechnicalTraining` → Technical spawn **VETERAN**
   - `Infa_SCIENCE_RedGuardTraining` → MiniGunner spawn **ELITE**
   - `unlock_team_science` / production `create_object` grant honesty
2. **Demo SuicideBomb residual** (`Demo_Upgrade_SuicideBomb`):
   - QueueUpgrade → complete tags eligible Demo units/structures
   - Death residual fires `Demo_DestroyedWeapon`: Primary **50**/r**60** +
     Secondary **10**/r**70**
   - New Demo spawns inherit tag after research
   - Fail-closed: Terrorist SUICIDED path stays `Demo_SuicideDynamitePack` (700)
3. Tests (not log-only):
   - `unit_training_science_veterancy_grant_residual`
   - `demo_suicide_bomb_structure_death_residual`
   - module unit tests in `host_unit_training` / `host_demo_suicide_bomb`

**Still residual (fail-closed, not claimed):**
- Full PrerequisiteSciences rank tree / control-bar science visibility matrix
- Full IsTrainable exclusive ExperienceTracker module matrix
- Full SUICIDED → `Demo_SuicideDynamitePackPlusFire` (500 primary) non-terrorist path
  (host residual closed 2026-07-13 — see Demo PlusFire SUICIDED + CommandSetUpgrade section)
- Full CommandSetUpgrade residual for suicide-bomb command sets
  (host residual closed 2026-07-13 — see Demo PlusFire SUICIDED + CommandSetUpgrade section)
- Network unit-training / suicide-bomb replication (network deferred)

## Residual Host Playability — SCIENCE_StealthFighter Gate + Chem/Demo Death Weapons (2026-07-13)
**Closed (host-testable Stealth Fighter science production gate + Chem/Demo death-weapon residual):**
1. **SCIENCE_StealthFighter production gate residual**:
   - `enqueue_production` denies science-gated Stealth Fighter without unlock
     (`AmericaJetStealthFighter` / SupW_/Lazr_/USA_ — **not** `AirF_*`).
   - `unlock_team_science` / `PurchaseScience` records unlock honesty.
   - Successful enqueue + production spawn record residual honesty.
   - Honesty: `honesty_stealth_fighter_science_*` / host path.
2. **Chem / Demo Terrorist death-weapon residual**:
   - Standard: SuicideDynamitePack **500**/r**18** + **300**/r**50**
   - Chem Beta baseline: same rings + MediumPoisonFieldUpgraded residual
   - Chem Gamma (`Chem_Upgrade_GLAAnthraxGamma` tag): Primary **600** + gamma poison field
   - Demo (`Demo_*`): Primary **700** (no poison)
3. **Chem / Demo DemoTrap death-weapon residual**:
   - Standard: DemoTrapDetonationWeapon **600**/r**25** (legacy falloff residual)
   - Chem Beta/Gamma: Primary **250**/r**25** + Secondary **100**/r**50** + poison field
   - Demo: Primary **700**/r**25** + Secondary **500**/r**50** (no poison)
4. Tests (not log-only):
   - `stealth_fighter_science_production_gate_residual`
   - `chem_terrorist_gamma_and_demo_death_weapon_residual`
   - `chem_demo_trap_gamma_and_demo_he_residual`
   - module unit tests in `host_stealth_fighter` / `host_terrorist` / `host_mines`

**Still residual (fail-closed, not claimed):**
- Full PrerequisiteSciences rank tree / control-bar science visibility matrix
- Full SlowDeath SUICIDED fling / OCL poison particle bone matrix
- Full Demo_SuicideDynamitePackPlusFire SUICIDED path for non-terrorist units
  (host residual closed 2026-07-13 — see Demo PlusFire SUICIDED + CommandSetUpgrade section;
  Demo_DestroyedWeapon normal-death path closed earlier same day)
- Full DemoTrapUpdate weapon-slot mode / PreAttack scoop animation
- Network science-gate / death-weapon replication (network deferred)

## Residual Host Playability — Chem Anthrax Gamma + GLA CamoNetting (2026-07-13)
**Closed (host-testable Anthrax Gamma toxin combat + CamoNetting structure stealth residual):**
1. **Anthrax Gamma residual** (`Chem_Upgrade_GLAAnthraxGamma` / `Upgrade_GLAAnthraxGamma`):
   - QueueUpgrade → complete tags toxin combat units (Toxin Tractor / SCUD / Bomb Truck).
   - Toxin Tractor stream residual: Chem baseline **12.5** (Beta); Gamma → **20.5**
     (`Chem_ToxinTruckGunGamma`).
   - Contaminate spray + death/SCUD MediumPoisonField DoT: Gamma/Beta → **2.5**/tick
     (base remains **2.0**).
   - Chem SCUD residual: primary is anthrax warhead (slot 0 toxin path + poison field).
   - Honesty: `honesty_gamma_ok` / stream / spray / host_path AnthraxGamma.
2. **CamoNetting residual** (`Upgrade_GLACamoNetting`):
   - QueueUpgrade → complete grants STEALTHED + `innate_stealth` to eligible structures:
     Stealth General buildings (`Slth_*` CC/Barracks/ArmsDealer/etc.),
     `GLATunnelNetwork`, `GLAStingerSite` (+ general variants).
   - Fail-closed: Rebel infantry does **not** receive CamoNetting (Camouflage residual
     remains separate).
   - Honesty: CamoNetting complete / host_path units_affected.
3. Tests (not log-only):
   - `anthrax_gamma_residual_toxin_stream_and_field`
   - `camo_netting_upgrade_stealths_gla_structures`
   - module unit tests in `host_toxin_tractor` / `host_scud_launcher` / `host_upgrades`

**Still residual (fail-closed, not claimed):**
- Full gamma particle bones / PlusOne-Two anthrax salvage weapon-set matrix
- Full CamoNetting sub-object net visual / structure attack reveal matrix
- Full Chem_DemoTrap / Terrorist suicide gamma death-weapon matrix (host residual closed 2026-07-13 — see SCIENCE_StealthFighter Gate + Chem/Demo Death Weapons section)
- Network anthrax / camo-netting replication (network deferred)

## Residual Host Playability — SupW EMP Patriot + GLA Saboteur (2026-07-13)
**Closed (host-testable SupW Patriot EMP dual-slot + Saboteur type-specific sabotage residual):**
1. **Superweapon General EMP Patriot residual** (`SupW_AmericaPatriotBattery` / TestSupWPatriot):
   - PRIMARY `SupW_PatriotMissileWeapon` residual: PrimaryDamage **15** / range **275**,
     ClipReload residual cadence (60 frames).
   - AA residual `SupW_PatriotMissileWeaponAir`: PrimaryDamage **30** / range **400**.
   - Impact residual seeds EMPPatriotEffectSpheroid: DISABLED_EMP **10000** ms
     (**300** frames) / EffectRadius **10** (DoesNotAffectMyOwnBuildings residual).
   - Dual-slot base-defense auto-fire residual path (ground + AA + EMP).
2. **GLA Saboteur residual** (`GLAInfantrySaboteur` / Chem_/Demo_/Slth_):
   - Walk to enemy structure → type-specific Sabotage*CrateCollide residual:
     - **Power plant**: player `power_sabotaged_till_frame` **900** frames brownout
     - **Supply center**: steal **1000** cash residual
     - **Military factory** (barracks/warfactory/airfield): DISABLED_HACKED **900** frames
     - **Superweapon / Strategy / Command**: special-power recharge reset residual
     - **Internet Center**: DISABLED_HACKED **450** frames
     - **Fake building**: kill structure residual
   - Saboteur consumed on success (mobile-crate residual).
   - Fail-closed: non-saboteur cannot issue; non-matching structures cancel residual.
3. Tests (not log-only):
   - `supw_patriot_emp_residual_dual_slot_and_disable`
   - `sabotage_command_applies_only_after_unit_reaches_target` (power residual)
   - `saboteur_military_factory_residual_disables_production`
   - `sabotage_command_rejects_non_saboteur_units`
   - module unit tests in `host_base_defense` (SupW) / `host_saboteur`

**Still residual (fail-closed, not claimed):**
- Full SupW AssistedTargetingModule assist clip / RequestAssistRange matrix
- Full EMPPatriotEffectSpheroid drawable scale/tint / EMPSparks particle volume
- Full BuildingPickup CrateCollide goal-object / EVA floating-text matrix
- Full internet-center spy-vision / contained-hacker disable iterate
- Network SupW EMP / saboteur replication (network deferred)

## Residual Host Playability — Laser General Tanks/Patriot + Tunnel Network Gun (2026-07-13)
**Closed (host-testable Lazr tank lasers / Lazr Patriot dual-slot + TunnelNetworkGun residual):**
1. **Laser General tank residual** (`Lazr_AmericaTankCrusader` / `Lazr_AmericaTankPaladin`):
   - PRIMARY `Lazr_CrusaderTankGun` residual: PrimaryDamage **80** / radius **5**,
     range **150**, Delay **2000**ms (60 frames). Instant laser residual.
   - PRIMARY `Lazr_PaladinTankGun` residual: PrimaryDamage **70** / radius **3**,
     range **150**, Delay **1000**ms (30 frames). Instant laser residual.
   - Stock Crusader/Paladin residual still uses shell guns (**60**/2000ms).
   - Composite Armor residual still applies to both chassis.
2. **Laser General Patriot residual** (`Lazr_AmericaPatriotBattery` / Lazr_Patriot*):
   - PRIMARY `Lazr_PatriotMissileWeapon` residual: PrimaryDamage **40** / r**3**,
     range **225** (vs stock **30**).
   - AA residual `Lazr_PatriotMissileWeaponAir` (retail TERTIARY collapsed to secondary):
     PrimaryDamage **35** / r**3**, range **350** (vs stock **25**).
   - Dual-slot base-defense auto-fire residual path (ground + AA).
3. **Tunnel Network gun residual** (`GLATunnelNetwork` / Chem_/Demo_/Slth_ / sneak):
   - PRIMARY `TunnelNetworkGun` residual: PrimaryDamage **15** / range **175** /
     Delay **250**ms (8 frames). Ground residual only.
   - Auto-acquire residual via base-defense residual fire path.
   - Enter/exit residual remains closed (prior slice).

**Tests:**
- `lazr_tank_residual_laser_guns`
- `lazr_patriot_residual_laser_dual_slot`
- `tunnel_network_gun_residual_auto_fires`
- host unit tests for laser tank / laser patriot / tunnel gun stats

**Still residual (fail-closed, not claimed):**
- Full LaserName / LaserBoneName drawable beam matrix for Lazr tanks / Patriot
- Full Lazr Patriot AssistedTargetingModule SECONDARY assist clip / RequestAssistRange
- Full SneakAttack TunnelNetworkGunDUMMY zero-damage matrix (real gun residual)
- Full GuardTunnelNetwork AI / CaveSystem / heal matrix
- Network laser-general / tunnel-gun replication (network deferred)

## Residual Host Playability — China MiG + USA Fire Base (2026-07-13)
**Closed (host-testable MiG napalm/BlackNapalm/Nuke + Fire Base howitzer residual):**
1. **China MiG residual** (`ChinaJetMIG` / `China_MiG` / Tank_/Infa_/Boss_ + Nuke_):
   - PRIMARY `NapalmMissileWeapon` residual: Primary **75**/r**5** + Secondary **40**/r**30**,
     range **320**, min **80**, Delay **300**ms (9 frames). ClipSize **2** honesty
     (RETURN_TO_BASE rearm matrix fail-closed). AA + ground residual.
   - Impact residual seeds FireFieldSmall DoT (Inferno fire-zone registry residual).
   - BlackNapalm PLAYER_UPGRADE residual (`Upgrade_ChinaBlackNapalm`):
     Secondary **50** + FireFieldUpgradedSmall residual.
   - Nuke General (`Nuke_ChinaJetMIG`): base `Nuke_MiGMissileWeapon` Primary **100**
     + SmallRadiationField residual; Tactical Nuke MiG PLAYER_UPGRADE residual
     (`Upgrade_ChinaTacticalNukeMig`) → `Nuke_NukeMissileWeapon` Primary **150**/r**50**
     + Secondary **50**/r**60** + radiation residual.
   - Honesty: `honesty_mig_ok` / black_napalm / tactical_nuke / fires / fields.
2. **Fire Base residual** (`AmericaFireBase` / AirF_/SupW_/Lazr_):
   - PRIMARY `FireBaseHowitzerGun` residual: PrimaryDamage **75** / radius **10**,
     range **275**, min **50**, Delay **2000**ms (60 frames). Ground residual only.
   - Fire residual: intended + PrimaryDamageRadius **10** splash.
   - Honesty: `honesty_fire_base_ok` / fires / units_hit.
3. Tests (not log-only):
   - `mig_residual_napalm_and_black_napalm`
   - `mig_nuke_residual_tactical_nuke`
   - `fire_base_residual_howitzer`
   - module unit tests in `host_mig.rs` / `host_fire_base.rs`

**Still residual (fail-closed, not claimed):**
- Full JetAIUpdate RETURN_TO_BASE / ClipReload airfield rearm matrix for MiG
- Full HistoricBonus FirestormSmallCreationWeapon multi-missile matrix
- Full MediumRadiationField for Nuke_NukeMissileWeapon residual (SmallRadiation reused)
- Full SPAWNS_ARE_THE_WEAPONS / garrison HiveStructureBody matrix for Fire Base
- Full Turret pitch / ScaleWeaponSpeed lob / ScatterRadiusVsInfantry matrix
- Network MiG / Fire Base replication (network deferred)

## Residual Host Playability — USA Strategy Center Battle Plans (2026-07-13)
**Closed (host-testable Bombardment / HoldTheLine / SearchAndDestroy residual):**
1. **Strategy Center residual** (`AmericaStrategyCenter` / *StrategyCenter):
   - Select plan via residual `SpecialAbilityChangeBattlePlans` (OPTION_ONE/TWO/THREE):
     - **Bombardment**: army WeaponBonus DAMAGE **120%** on legal members
     - **HoldTheLine**: army armor damage scalar **0.9** + center max-health **×2**
     - **SearchAndDestroy**: army RANGE **120%** + sight **1.2**; center stealth detect residual
   - ValidMember residual: INFANTRY | CAN_ATTACK | VEHICLE
   - InvalidMember residual: DOZER | STRUCTURE | AIRCRAFT | DRONE
   - Honesty: `honesty_battle_plan_ok` / select / buffs
2. Tests (not log-only):
   - `strategy_center_battle_plan_residual_applies_unit_bonuses`
   - module unit tests in `host_strategy_center.rs`

**Still residual (fail-closed, not claimed):**
- Full BattlePlanUpdate pack/unpack door model-condition / 7s animation matrix
- Full 5000ms army BattlePlanChangeParalyzeTime on plan change
  (host residual closed 2026-07-13 — see BattlePlan Paralyze + EjectPilotDie section)
- Full Bombardment turret enable / recenter natural-position residual path
- Full StealthDetectorUpdate module enable stack / VisionObjectName spawn residual
- Network battle-plan replication (network deferred)

## Residual Host Playability — Helix Minigun + Inferno BlackNapalm (2026-07-13)
**Closed (host-testable Helix PRIMARY minigun + Inferno BlackNapalm fire-field residual):**
1. **Helix minigun residual** (`ChinaVehicleHelix` / Nuke_/Infa_/Tank_ / TestHelix):
   - PRIMARY `HelixMinigunWeapon` residual: PrimaryDamage **6** / radius **0**
     (intended-only), range **115**, Delay **100**ms (3 frames).
   - AntiAirborneInfantry residual honesty (`can_target_air`); AntiAirborneVehicle = No.
   - Minigun retained with portable gattling/propaganda/bunker addons (retail keeps
     HelixMinigun always — gattling addon remains separate residual path).
   - Honesty: `honesty_helix_minigun_ok` / fires / units_hit.
2. **Inferno BlackNapalm residual** (`Upgrade_ChinaBlackNapalm` on Inferno Cannon):
   - WeaponSet PLAYER_UPGRADE residual → FireFieldUpgradedSmall on shell impact.
   - Upgraded fire DoT: **7.5** / r**30** / tick **250**ms / lifetime **2500**ms
     (`SmallFireFieldWeaponUpgraded`; base FireFieldSmall remains **5**).
   - Shell impact PrimaryDamage **30** / r**15** / range **300** unchanged.
   - Honesty: `honesty_inferno_black_napalm_ok` / upgrades / upgraded zones;
     existing `honesty_inferno_cannon_ok` still green.
3. Tests (not log-only):
   - `helix_minigun_residual_intended_only`
   - `inferno_black_napalm_upgraded_fire_field_residual`
   - module unit tests in `host_helix_minigun.rs` / `host_inferno_cannon.rs`

**Still residual (fail-closed, not claimed):**
- Full ChinookAIUpdate rotor wash / AutoAcquire idle / COMANCHE_VULCAN Stinger-site matrix
- Full Helix portable gattling dual-stream simultaneous fire with minigun matrix
- Full HistoricBonus FirestormSmallCreationWeapon multi-shell Inferno matrix
- Full InfernoTankShell DumbProjectileBehavior bezier lob / upgraded particle bones
- Network Helix minigun / Inferno BlackNapalm replication (network deferred)

## Residual Host Playability — USA Stealth Fighter Combat + Comanche Cannon/AT (2026-07-13)
**Closed (host-testable Stealth Fighter missiles + Comanche 20mm/anti-tank residual):**
1. **Stealth Fighter residual** (`AmericaJetStealthFighter` / USA_ / SupW_/Lazr_ + AirF_):
   - PRIMARY `StealthJetMissileWeapon` residual: PrimaryDamage **100** / radius **5**,
     range **220**, min **60**, Delay **200**ms (6 frames). ClipSize **2** honesty
     (RETURN_TO_BASE rearm matrix fail-closed). Ground residual only (no AA).
   - Fire residual: intended + PrimaryDamageRadius **5** splash.
   - Bunker Buster PLAYER_UPGRADE residual remains in host_bunker_buster and is
     applied from the residual fire path for structure targets (garrison kill + bunker mult).
   - Honesty: `honesty_stealth_fighter_ok` / fires / units_hit.
2. **Comanche residual** (`AmericaVehicleComanche` / USA_ / AirF_/SupW_/Lazr_):
   - PRIMARY `Comanche20mmCannonWeapon` residual: PrimaryDamage **6** / intended-only,
     range **200**, Delay **100**ms (3 frames). AntiAirborneInfantry residual honesty.
   - SECONDARY `ComancheAntiTankMissileWeapon` residual at spawn:
     Primary **50**/r**5** + Secondary **30**/r**25**, range **200**, Delay **500**ms
     (15 frames), ClipSize **4** honesty.
   - Rocket pods PLAYER_UPGRADE residual still replaces secondary (retail TERTIARY
     collapse — existing host_comanche_rocket_pods path).
   - Honesty: `honesty_comanche_ok` / cannon / antitank / rocket pods.
3. Tests (not log-only):
   - `stealth_fighter_residual_missiles_and_splash`
   - `comanche_residual_cannon_and_antitank`
   - module unit tests in `host_stealth_fighter.rs` / `host_comanche_rocket_pods.rs`
   - existing `bunker_buster_residual_kills_garrison_and_damages_bunker` still green

**Still residual (fail-closed, not claimed):**
- Full SCIENCE_StealthFighter production enqueue gate residual (host residual closed 2026-07-13 — see SCIENCE_StealthFighter Gate section; full rank tree still open)
- Full JetAIUpdate RETURN_TO_BASE / ClipReload airfield rearm matrix
- Full WeaponSet PRIMARY/SECONDARY/TERTIARY chooser (host collapses tertiary rocket pods into secondary)
- Full ScatterTarget / 20-rocket volley spacing / JetAIUpdate turret matrix
- Network stealth-fighter / comanche / rocket-pods replication (network deferred)

## Residual Host Playability — USA Raptor + Battle Drone (2026-07-13)
**Closed (host-testable Raptor missiles/Laser Missiles + Battle Drone attach/gun/repair residual):**
1. **Raptor residual** (`AmericaJetRaptor` / USA_ / SupW_/Lazr_ + King Raptor AirF_):
   - PRIMARY `RaptorJetMissileWeapon` residual: PrimaryDamage **100** / radius **5**,
     range **320**, min **100**, Delay **150**ms (5 frames). ClipSize **4** honesty
     (RETURN_TO_BASE rearm matrix fail-closed). AA + ground residual.
   - King Raptor (`AirF_AmericaJetRaptor`): `AirF_RaptorJetMissileWeapon` residual
     Primary **125** / range **350** / Delay **75**ms (3 frames) / ClipSize **6**.
     PDL residual remains in host_point_defense (not re-opened).
   - Laser Missiles PLAYER_UPGRADE residual (`Upgrade_AmericaLaserMissiles`):
     standard DAMAGE **125%** → **125**; King Raptor DAMAGE **112%** → **140**.
   - Fire residual: intended + PrimaryDamageRadius **5** splash.
   - Honesty: `honesty_raptor_ok` / laser_missiles / fires / units_hit.
2. **Battle Drone residual** (`AmericaVehicleBattleDrone` / SupW_/AirF_/Lazr_):
   - Attach residual via `residual_attach_slave_drone(Battle)` on Humvee/compatible
     masters → tags `Upgrade_AmericaBattleDrone`.
   - PRIMARY `BattleDroneMachineGun` residual: PrimaryDamage **1** / range **110** /
     Delay **100**ms (3 frames). Intended-only fire residual.
   - Master repair residual: when master HP < **60%**, heal **10** HP/s
     (`RepairRatePerSecond` residual) within repair band.
   - Honesty: `honesty_battle_drone_ok` / attach / fire / repair.
3. Tests (not log-only):
   - `raptor_residual_missiles_and_laser_missiles`
   - `battle_drone_residual_attach_fire_and_repair`
   - module unit tests in `host_raptor.rs` / `host_slave_drones.rs` Battle path

**Still residual (fail-closed, not claimed):**
- Full JetAIUpdate RETURN_TO_BASE / ClipReload 8000ms airfield rearm matrix
- Full ScatterRadiusVsInfantry / projectile exhaust / Countermeasures flare matrix
- Full SlavedUpdate arm pack/unpack weld FX / RepairMinAltitude matrix
- Full ObjectCreationUpgrade ConflictsWith Battle/Scout/Hellfire exclusive matrix
- Network laser-missiles / battle-drone replication (network deferred)

## Residual Host Playability — China Nuclear Tanks + GLA Rebel BoobyTrap (2026-07-13)
**Closed (host-testable Nuclear Tanks death/speed/radiation + Rebel BoobyTrap residual):**
1. **Nuclear Tanks residual** (`Upgrade_ChinaNuclearTanks` on Battlemaster / Overlord / Emperor):
   - Locomotor residual speed: Battlemaster **25 → 35**, Overlord/Emperor **20 → 30**.
   - On death: dual-radius `NuclearTankDeathWeapon` residual
     Primary **25**/r**25** + Secondary **10**/r**75** (Nuke_ general: **110**/r**80** +
     **70**/r**100**).
   - Spawns residual `OCL_RadiationFieldSmall` / `SmallRadiationFieldWeapon`:
     **5** dmg / r**15** / tick **750**ms / lifetime **2500**ms.
   - Honesty: `honesty_nuclear_tanks_ok` / upgrade / death / radiation.
2. **Rebel BoobyTrap residual** (`Upgrade_GLAInfantryRebelBoobyTrapAttack` +
   `SpecialAbilityBoobyTrap` on `GLAInfantryRebel` / variants):
   - Plant residual: walk to enemy structure within StartAbilityRange **5** →
     mark `OBJECT_STATUS_BOOBY_TRAPPED` (host residual).
   - Reload residual: **7500** ms (**225** frames).
   - Detonate residual on enemy capture-trigger / structure death:
     Primary **200** / (r**5**+geometry) + Secondary **50** / (r**15**+geometry).
   - Allies of planter do not trigger detonation (C++ checkAndDetonateBoobyTrap).
   - Honesty: `honesty_booby_trap_ok` / plant / detonate / upgrade.
3. Tests (not log-only):
   - `nuclear_tanks_residual_speed_death_and_radiation`
   - `rebel_booby_trap_plant_and_capture_detonate_residual`
   - module unit tests in `host_nuclear_tanks.rs` / `host_booby_trap.rs`

**Still residual (fail-closed, not claimed):**
- Full FireWeaponWhenDeadBehavior exclusive module / RequiresAllTriggers matrix
- Full LocomotorSetUpgrade Nuclear*Locomotor pitch-roll visual matrix
- Full SpecialObject BoobyTrap StickyBombUpdate bone attach / stealth matrix
- Full MaxSpecialObjects=100 / UniqueSpecialObjectTargets list matrix
- Network nuclear-tanks / booby-trap replication (network deferred)

## Residual Host Playability — China Overlord Main Gun + GLA Jarmen Kell Sniper (2026-07-13)
**Closed (host-testable Overlord dual-radius/Uranium + Jarmen Kell sniper/AP residual):**
1. **Overlord main gun residual** (`ChinaTankOverlord` / Nuke_/Infa_/Tank_ + Emperor):
   - PRIMARY `OverlordTankGun` residual: PrimaryDamage **80** / radius **5** +
     SecondaryDamage **20** / radius **10**, range **175**, ClipReload **2000**ms
     (60 frames). ClipSize **2** honesty (dual-volley cadence fail-closed).
   - Dual-radius splash residual on fire (intended + primary/secondary rings).
   - Uranium Shells PLAYER_UPGRADE residual (`Upgrade_ChinaUraniumShells`):
     WeaponBonus DAMAGE **125%** → Primary **100** / Secondary **25**.
   - Portable gattling addon exclusive fire path unchanged (still deals weapon.damage
     + passenger gattling when addon installed).
   - Honesty: `honesty_overlord_gun_ok` / uranium / fires / units_hit.
2. **Jarmen Kell sniper residual** (`GLAInfantryJarmenKell` / Chem_/Demo_/Slth_/GC_*):
   - PRIMARY `GLAJarmenKellRifle` residual: PrimaryDamage **180** / range **225** /
     Delay **1000**ms (30 frames). Intended-only SNIPER residual (radius **0**).
   - AP Bullets PLAYER_UPGRADE residual (`Upgrade_GLAAPBullets`): DAMAGE **125%** → **225**.
   - Vehicle pilot-snipe special already closed via host_hero_abilities (not re-opened).
   - Honesty: `honesty_jarmen_kell_ok` / ap / fires / units_hit.
3. Tests (not log-only):
   - `overlord_gun_residual_dual_radius_and_uranium`
   - `jarmen_kell_residual_sniper_and_ap_bullets`
   - module unit tests in `host_overlord_gun.rs` / `host_jarmen_kell.rs`

**Still residual (fail-closed, not claimed):**
- Full ClipSize=2 DelayBetweenShots 300ms dual-volley cadence for Overlord
- Full ScatterRadiusVsInfantry / projectile shell lob / W3D turret matrix
- Full Nuclear Tanks death weapon residual (host residual closed 2026-07-13 — see Nuclear Tanks + BoobyTrap section; full exclusive module still open)
- Full SECONDARY AutoChooseSources=NONE pilot-sniper WeaponSet chooser matrix
- Full StealthUpdate / Camouflage / Science prereq residual matrix for Kell
- Network uranium / sniper / AP Bullets replication (network deferred)

## Residual Host Playability — GLA Scorpion + USA Tomahawk (2026-07-13)
**Closed (host-testable Scorpion gun/salvage/rocket + Tomahawk dual-radius residual):**
1. **Scorpion residual** (`GLATankScorpion` / `GLA_ScorpionTank` / Chem_/Demo_/Slth_):
   - PRIMARY `ScorpionTankGun` residual: PrimaryDamage **20** / radius **5** /
     range **150** / Delay **1000**ms (30 frames).
   - Salvage residual (CRATEUPGRADE): gun damage **20** → **25** (`ScorpionTankGunPlusOne`;
     PlusTwo keeps PlusOne gun — no further primary bonus).
   - `Upgrade_GLAScorpionRocket` PLAYER_UPGRADE residual equips SECONDARY
     `ScorpionMissileWeapon`: Primary **100**/r**5** + Secondary **80**/r**25**,
     min **40**, ClipReload **15000**ms (450 frames).
   - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`): missile rings × **1.25**.
   - Honesty: `honesty_scorpion_ok` / rocket / missile / fires.
2. **Tomahawk residual** (`AmericaVehicleTomahawk` / `USA_Tomahawk` / SupW_):
   - PRIMARY `TomahawkMissileWeapon` residual: Primary **150**/r**10** +
     Secondary **50**/r**25**, range **350**, min **100**, ClipReload **7000**ms
     (210 frames).
   - Dual-radius splash residual on fire (intended + primary/secondary rings).
   - Honesty: `honesty_tomahawk_ok` / fires / units_hit.
3. Tests (not log-only):
   - `scorpion_residual_gun_salvage_and_rocket`
   - `tomahawk_residual_dual_radius_missile`
   - module unit tests in `host_scorpion.rs` / `host_tomahawk.rs`

**Still residual (fail-closed, not claimed):**
- Full SalvageCrate W3D turret / missile-rack subobject swap matrix
- Full ClipSize=2 DelayBetweenShots 200ms dual-volley cadence for rocket+tier2
- Full TomahawkMissile projectile lob / CapableOfFollowingWaypoints path
- Full PreAttackDelay PER_SHOT anim / hide-show missile bone matrix
- Network scorpion rocket / tomahawk replication (network deferred)

## Residual Host Playability — USA Pilot + GLA WorkerShoes (2026-07-13)
**Closed (host-testable Pilot recrew + WorkerShoes residual):**
1. **USA Pilot residual** (`AmericaInfantryPilot` / AirF_ / CINE_ / TestPilot):
   - Enter unmanned ground vehicle (DISABLED_UNMANNED residual from snipe/neutron) →
     recrew: clear unmanned, transfer team to pilot team, merge pilot veterancy
     (retail `VeterancyCrateCollide` IsPilot + AddsOwnerVeterancy), consume pilot.
   - Pilots residual-start at VETERAN (`VeterancyGainCreate StartingLevel`).
   - ForbiddenKindOf residual: Dozer / worker vehicles not recrewed.
   - Honesty: `honesty_pilot_recrew_ok` / veterancy_transfer / `honesty_pilot_ok`.
2. **GLA Worker residual** (`GLAInfantryWorker` / Chem_/Slth_/GC_* / GLA_Worker):
   - `Upgrade_GLAWorkerShoes` PLAYER_UPGRADE residual:
     - Locomotor residual Speed **25** → **30** (FastHuman → WorkerShoesLocomotor).
     - WorkerAIUpdate UpgradedSupplyBoost **8** cash per supply drop-off.
   - Construction / repair / mine-clear already residual (not re-opened).
   - Honesty: `honesty_worker_shoes_apply_ok` / boost / `honesty_worker_ok`.
3. Tests (not log-only):
   - `pilot_recrew_unmanned_vehicle_after_enter_reach`
   - `pilot_recrew_rejects_manned_vehicle`
   - `worker_shoes_upgrade_speed_and_supply_boost_residual`
   - module unit tests in `host_usa_pilot.rs` / `host_gla_worker.rs`

**Still residual (fail-closed, not claimed):**
- Full EjectPilotDie air/ground OCL parachute spawn matrix
  (host ground residual closed 2026-07-13 — see BattlePlan Paralyze + EjectPilotDie section;
  air parachute / invuln timer still open)
- Full PilotFindVehicleUpdate AI auto-scan / MinHealth enter matrix
- Full AutoFindHealingUpdate hospital path residual
- Full WorkerAIUpdate BoredTime/Range auto-task matrix
- Full SupplyWarehouseActionDelay / SupplyCenterActionDelay timing matrix
- Full fake-building CommandSetUpgrade residual for Worker
- Network pilot recrew / WorkerShoes replication (network deferred)

## Residual Host Playability — USA Ranger + China Hacker DisableBuilding (2026-07-13)
**Closed (host-testable Ranger rifle/FlashBang splash + Hacker DisableBuilding residual):**
1. **USA Ranger residual polish** (`AmericaInfantryRanger` / USA_ / AirF_ / Lazr_ / SupW_ / GoldenRanger):
   - PRIMARY `RangerAdvancedCombatRifle` residual: PrimaryDamage **5** / range **100** /
     Delay **100**ms (3 frames). ClipSize **3** honesty (volley matrix fail-closed).
   - SECONDARY `RangerFlashBangGrenadeWeapon` residual (when equipped / FlashBang upgrade):
     PrimaryDamage **35** / radius **10** + SecondaryDamage **10** / radius **40**,
     range **175**, min **20**, ClipReload **2000**ms (60 frames).
   - Dual-radius splash residual on secondary fire (intended + primary/secondary rings).
   - PreferredAgainst residual (existing slot chooser): secondary preferred vs infantry /
     structures when damage 35 > 5. FlashBang upgrade equip path still via host_upgrades.
   - Honesty: `honesty_ranger_ok` / flashbang / rifle_fires / units_hit.
2. **China Hacker DisableBuilding residual combat polish** (`ChinaInfantryHacker` /
   Tank_/Nuke_ / TestHacker):
   - Special ability residual `SpecialAbilityHackerDisableBuilding`:
     walk to enemy structure within StartAbilityRange **150** → apply
     DISABLED_HACKED for EffectDuration **2000**ms (**60** frames).
   - Disabled structures count as `is_disabled()` (production stop residual).
   - Internet cash residual remains in host_hacker_income (not re-opened).
   - Honesty: `honesty_hacker_disable_building_ok` / disable count.
3. Tests (not log-only):
   - `ranger_residual_rifle_and_flashbang_splash`
   - `hacker_disable_building_command_disables_after_reach`
   - module unit tests in `host_ranger.rs` / `host_hacker_disable.rs`

**Still residual (fail-closed, not claimed):**
- Full SURRENDER DamageType infantry-surrender AI / garrison clear matrix
- Full ClipSize=3 in-clip DelayBetweenShots + ClipReload 700ms volley matrix
- Full FlashBang ScatterRadius projectile lob / PreAttackDelay anim lock
- Full Hacker unpack/pack/prep/PersistentPrepTime continuous refresh stream
- Full BinaryDataStream special object / DisableFX particle interleave
- Network flashbang / disable-building replication (network deferred)

## Residual Host Playability — China MiniGunner + Colonel Burton Combat (2026-07-13)
**Closed (host-testable MiniGunner dual gun/ramp/chain/horde + Burton sniper/knife residual):**
1. **MiniGunner residual** (`Infa_ChinaInfantryMiniGunner` / ChinaInfantryMiniGunner / variants):
   - PRIMARY `Infa_MiniGunnerGun` residual: PrimaryDamage **10** / range **125** /
     Delay **500**ms (15 frames) + SECONDARY `Infa_MiniGunnerGunAir` (10 / 350 / AA).
   - Continuous-fire ramp residual (`FiringTracker` ContinuousFireOne=**6** / Two=**12** /
     Coast=**1000**ms): Base **15** → MEAN **7** (200% RoF) → FAST **5** (300% RoF).
   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): DAMAGE **125%** → **12.5**.
   - Horde residual (China infantry HordeUpdate Radius **30** Count **5**): ROF **150%**
     stacks with continuous fire; Nationalism **125%** while in horde.
   - Honesty: `honesty_minigunner_ok` / ramp / aa / horde / nationalism / ground_fires.
2. **Colonel Burton combat residual** (`AmericaInfantryColonelBurton` / SupW_/CINE_):
   - PRIMARY `ColonelBurtonSniperRifleWeapon` residual: PrimaryDamage **40** / range **125** /
     Delay **100**ms (3 frames). ClipSize **3** honesty (volley matrix fail-closed).
   - Knife residual (`ColonelBurtonKnifeWeapon`): close-range infantry within **3** →
     MELEE one-shot PrimaryDamage **10000** (vehicles still take sniper damage).
   - Timed/remote demo charges already closed via host_mines / hero abilities (not re-opened).
   - Honesty: `honesty_burton_ok` / knife / sniper_fires.
3. Tests (not log-only):
   - `minigunner_residual_gun_ramp_aa_horde_and_chain_guns`
   - `colonel_burton_residual_sniper_and_knife`
   - module unit tests in `host_minigunner.rs` / `host_colonel_burton.rs`

**Still residual (fail-closed, not claimed):**
- Full FiringTracker model-condition CONTINUOUS_FIRE_* animation matrix
- Full MiniGunner bayonet tertiary / CaptureBuilding special residual
- Full Burton ClipSize=3 in-clip DelayBetweenShots + ClipReload 500ms volley matrix
- Full knife PreAttackDelay 833ms / PER_ATTACK anim lock matrix
- Full StealthUpdate / ChemicalSuits / AdvancedTraining residual matrix
- Network continuous-fire / sniper / knife replication (network deferred)

## Residual Host Playability — GLA Rebel + RPG Trooper (2026-07-13)
**Closed (host-testable GLA Rebel gun/AP Bullets + RPG Trooper rocket/AP Rockets residual):**
1. **Rebel residual** (`GLAInfantryRebel` / Chem_/Demo_/Slth_/GC_* / GLA_Soldier / TestRebel):
   - PRIMARY `GLARebelMachineGun` residual: PrimaryDamage **5** / range **100** /
     Delay **100**ms (3 frames). ClipSize **3** honesty (volley matrix fail-closed).
   - AP Bullets PLAYER_UPGRADE residual (`Upgrade_GLAAPBullets`): DAMAGE **125%** → **6.25**.
   - Camouflage residual already closed via host_upgrades (not re-opened).
   - Honesty: `honesty_rebel_ok` / ap / fires.
2. **RPG Trooper residual** (`GLAInfantryTunnelDefender` / variants / GLA_RPG / TestRPGTrooper):
   - PRIMARY `TunnelDefenderRocketWeapon` residual: PrimaryDamage **40** /
     radius **5** / range **175** / min **5** / Delay **1000**ms / AA+ground.
   - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
   - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`): DAMAGE **125%** → **50**.
   - Honesty: `honesty_rpg_trooper_ok` / ap / fires / units_hit.
3. Tests (not log-only):
   - `rebel_residual_gun_and_ap_bullets`
   - `rpg_trooper_residual_rocket_and_ap_rockets`
   - module unit tests in `host_gla_rebel.rs` / `host_rpg_trooper.rs`

**Still residual (fail-closed, not claimed):**
- Full ClipSize=3 in-clip DelayBetweenShots + ClipReload 700ms volley matrix
- CaptureBuilding special ability residual for Rebel (BoobyTrap host residual closed 2026-07-13)
- Full ScatterRadiusVsInfantry / projectile exhaust FX matrix for RPG Trooper
- Network AP / fire replication (network deferred)

## Residual Host Playability — China Red Guard + Tank Hunter (2026-07-13)
**Closed (host-testable China Red Guard gun/horde/nationalism/bayonet + Tank Hunter RPG/TNT residual):**
1. **Red Guard residual** (`ChinaInfantryRedguard` / China_RedGuard / Tank_/Nuke_):
   - PRIMARY `RedguardMachineGun` residual: PrimaryDamage **15** / range **100** /
     Delay **1000**ms (30 frames).
   - Horde residual (`HordeUpdate` KindOf INFANTRY, AlliesOnly, ExactMatch=No,
     Radius **30**, Count **5**): RATE_OF_FIRE **150%** → delay floor(30/1.5)=**20** frames.
   - Nationalism residual (`Upgrade_Nationalism` while in horde): additional ROF **125%**
     (stacks) → floor(30/1.875)=**16** frames.
   - Bayonet residual (`RedguardBayonet` stats): close-range infantry within **2** →
     MELEE one-shot PrimaryDamage **10000** (retail ZH WeaponSet is PRIMARY-only;
     residual from weapon def + PREATTACK_C/FIRING_C / CINE TERTIARY).
   - Honesty: `honesty_red_guard_ok` / horde / nationalism / bayonet / fires.
2. **Tank Hunter residual** (`ChinaInfantryTankHunter` / China_TankHunter / variants):
   - PRIMARY `ChinaInfantryTankHunterMissileLauncher` residual: PrimaryDamage **40** /
     radius **5** / range **175** / min **5** / Delay **1000**ms / AA+ground.
   - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
   - Horde + Nationalism residual (same China infantry HordeUpdate params as Red Guard).
   - TNT special residual (`SpecialAbilityTankHunterTNTAttack` / TNTStickyBomb):
     plant timed demo charge (TNTDetonationWeapon 500/10 + 150/50) with ReloadTime
     **7500**ms (225 frames) residual cooldown + StartAbilityRange **5**.
   - Honesty: `honesty_tank_hunter_ok` / tnt / horde / nationalism / fires.
3. Tests (not log-only):
   - `red_guard_residual_gun_horde_nationalism_and_bayonet`
   - `tank_hunter_residual_rpg_horde_and_tnt`
   - module unit tests in `host_red_guard.rs` / `host_tank_hunter.rs`

**Still residual (fail-closed, not claimed):**
- Full HordeUpdate RubOffRadius honorary-member / terrain-decal flag matrix
- Full Fanaticism infantry-general nationalism branch
- Full WeaponSet tertiary auto-choose / pre-attack anim lock for bayonet
- Full SpecialAbilityUpdate flee-after / MaxSpecialObjects=8 list / attach bones
- Full ScatterRadiusVsInfantry / projectile exhaust FX matrix
- SCIENCE_RedGuardTraining VETERAN spawn residual (host residual closed 2026-07-13 — see SCIENCE Unit Training + Demo SuicideBomb section)
- Network horde / TNT / RPG replication (network deferred)

## Residual Host Playability — Stinger Site + Patriot AA Polish (2026-07-13)
**Closed (host-testable GLA Stinger Site SPAWNS_ARE_THE_WEAPONS residual + USA Patriot AA polish):**
1. **Stinger Site residual** (`GLA_StingerSite` / Chem_/Demo_/Slth_ / GC_* variants):
   - Retail SPAWNS_ARE_THE_WEAPONS abstraction: structure fires soldier weapons
     (SpawnNumber=**3** honesty residual — not full SpawnBehavior / HiveStructureBody).
   - PRIMARY `StingerMissileWeapon` (20 / 225 / ClipReload **2000**ms → 60 frames).
   - SECONDARY `StingerMissileWeaponAir` (30 / 400 / AA only).
   - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`): damage × **1.25**.
   - Auto-acquire residual (base-defense dual-slot path) chooses air/ground weapon.
   - Honesty: `honesty_stinger_site_ok` / aa / ground_fires / ap_rockets_upgrades.
2. **Patriot residual polish** (`USA_Patriot` / AmericaPatriotBattery / Lazr_…):
   - PRIMARY `PatriotMissileWeapon` (30 / 225) already residual; now also
     SECONDARY `PatriotMissileWeaponAir` (25 / 350 / AA).
   - Dual-slot auto-acquire residual (same base-defense path as Gattling/Stinger).
   - Honesty: `honesty_patriot_ok` / aa / ground_fires (plus existing base-defense fires).
3. Tests (not log-only):
   - `stinger_site_residual_dual_fire_and_ap_rockets`
   - `patriot_residual_aa_secondary_auto_fires`
   - `base_defense_residual_patriot_auto_fires_without_attack_object` (updated)
   - module unit tests in `host_base_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full SpawnBehavior / HiveStructureBody / 3 Stinger soldiers / CamoNetting stealth
- Full AssistedTargetingModule Patriot assist clips / RequestAssistRange=200
- Full Patriot ClipSize=4 in-clip DelayBetweenShots 250ms volley matrix
- Full PointDefenseLaser for structure / anti-ballistic full matrix
- Network base-defense replication (network deferred)

## Residual Host Playability — China Battlemaster (2026-07-13)
**Closed (host-testable China Battlemaster main gun + Uranium + horde/nationalism residual):**
1. **Battlemaster residual** (`ChinaTankBattleMaster` / `China_BattlemasterTank` / Tank_/Nuke_):
   - PRIMARY `BattleMasterTankGun` residual: PrimaryDamage **60** / radius **5** /
     AttackRange **150** / Delay **2000**ms (60 frames) / WeaponSpeed 400.
   - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
   - Uranium Shells PLAYER_UPGRADE residual (`Upgrade_ChinaUraniumShells`):
     WeaponBonus DAMAGE **125%** → PrimaryDamage **75**.
   - Horde residual (`HordeUpdate` ExactMatch allies Radius **75** Count **5**):
     WeaponBonus HORDE RATE_OF_FIRE **150%** → delay floor(60/1.5)=**40** frames.
   - Nationalism residual (`Upgrade_Nationalism` while in horde):
     additional RATE_OF_FIRE **125%** (stacks) → delay floor(60/1.875)=**32** frames.
   - Honesty: `honesty_battlemaster_ok` / uranium / horde / nationalism / fires.
2. Tests (not log-only):
   - `battlemaster_residual_gun_uranium_and_horde_nationalism`
   - module unit tests in `host_battlemaster.rs`

**Still residual (fail-closed, not claimed):**
- Full HordeUpdate RubOffRadius honorary-member / terrain-decal flag matrix
- Full Fanaticism infantry-general nationalism branch
- Full Nuclear Tanks exclusive FireWeaponWhenDead / NuclearBattleMasterLocomotor visual matrix (host residual closed 2026-07-13)
- SCIENCE_BattlemasterTraining ELITE spawn residual (host residual closed 2026-07-13 — see SCIENCE Unit Training + Demo SuicideBomb section)
- Network uranium / horde replication (network deferred)

## Residual Host Playability — Helix NapalmBomb + Bomb Truck HE/Bio (2026-07-13)
**Closed (host-testable Helix NapalmBomb special power + Bomb Truck detonation residual):**
1. **Helix NapalmBomb residual** (`SpecialAbilityHelixNapalmBomb` /
   `SPECIAL_HELIX_NAPALM_BOMB` on `ChinaVehicleHelix` / `TestHelix`):
   - Requires `Upgrade_HelixNapalmBomb` residual unlock (TestHelix always unlocked).
   - Instant blast residual: PrimaryDamage **75** / radius **5** + Secondary **40** / **30**
     (`NapalmBombWeapon` / BlackNapalm same blast numbers).
   - Spawns residual FirestormSmall DoT: DamageAmount **100** (BlackNapalm **150**) /
     tick **500**ms / lifetime **6000**ms / radius **90**.
   - Reload residual: **10000** ms (300 frames).
   - Honesty: `honesty_helix_napalm_drop_ok` / blast / firestorm / `honesty_helix_napalm_ok`.
2. **Bomb Truck HE/Bio detonation residual** (`GLAVehicleBombTruck` FireWeaponWhenDead):
   - Default death: Primary **1000**/radius **40** + Secondary **100**/radius **65**.
   - HE upgrade (`Upgrade_GLABombTruckHighExplosiveBomb`): **2000**/50 + **200**/85.
   - Bio upgrade (`Upgrade_GLABombTruckBioBomb`): + MediumPoisonField DoT
     (2 / 80 / 30s / 500ms); Bio+Anthrax → 2.5 upgraded poison.
   - HE+Bio combos supported (HE blast + bio poison residual).
   - Honesty: `honesty_bomb_truck_detonate_ok` / he / bio / path.
3. Tests (not log-only):
   - `helix_napalm_bomb_special_power_residual_blast_and_firestorm`
   - `helix_napalm_bomb_requires_upgrade_on_production_helix`
   - `bomb_truck_default_detonation_residual_damages_nearby`
   - `bomb_truck_he_and_bio_detonation_residual`
   - module unit tests in `host_helix_napalm.rs` / `host_bomb_truck_detonate.rs`

**Still residual (fail-closed, not claimed):**
- Full SpecialObject NapalmBomb HeightDie fall / UnpackTime charge matrix
- Full FirestormDynamicGeometryInfoUpdate expand/reverse radius animation
- Full FireWeaponWhenDead exclusive RequiresAllTriggers / SubObjectsUpgrade Bombload visuals
- Full Anthrax Gamma / Demo_ red FX / WeaponBonus PLAYER_UPGRADE 125% HE matrix
- Network Helix napalm / bomb-truck detonation replication (network deferred)

## Residual Host Playability — Overlord/Helix Addons + Nuke Cannon Primary (2026-07-13)
**Closed (host-testable China Overlord/Helix/Emperor addons + Nuke Cannon primary residual):**
1. **Overlord gattling / propaganda tower addons residual**
   (`ChinaTankOverlord` / general variants; Helix / Emperor hosts):
   - `Upgrade_ChinaOverlordGattlingCannon` / Helix equivalent installs residual
     portable gattling: SECONDARY AA `GattlingBuildingGunAir` (5 / 400 / 250ms)
     + passenger ground residual `GattlingBuildingGun` (10) on PRIMARY fires.
   - `Upgrade_ChinaOverlordPropagandaTower` / Helix equivalent enables propaganda
     pulse on the host (Radius **150**, heal **1%**/s base, **2%**/s upgraded).
   - ConflictsWith residual: exclusive gattling / propaganda / bunker install
     (Emperor keeps innate propaganda when gattling is installed).
   - Honesty: `honesty_overlord_gattling_ok` / propaganda install + heal.
2. **Emperor tank residual** (`Tank_ChinaTankEmperor`):
   - Innate `PropagandaTowerBehavior` residual on spawn (AffectsSelf heal rates 1%/2%).
   - Optional gattling upgrade residual (same portable path).
3. **Helix residual** (`ChinaVehicleHelix`):
   - `HelixContain` Slots=**5** transport residual on spawn.
   - Same gattling / propaganda / bunker addon residual install path.
   - **Helix NapalmBomb special ability closed 2026-07-13** (see section above).
4. **Nuke Cannon primary residual** (`ChinaVehicleNukeCannon`):
   - PRIMARY `NukeCannonGun` area residual: PrimaryDamage **400** / radius **50**
     + SecondaryDamage **20** / radius **60**, range **350**, delay **300** frames.
   - Impact spawns residual `MediumRadiationField` DoT (15 / 50 / 750ms ticks / 30s).
   - Neutron secondary remains existing host residual (unchanged).
   - Honesty: `honesty_nuke_cannon_primary_ok` / radiation / host_path.
5. Tests (not log-only):
   - `overlord_gattling_addon_residual_install_and_fire`
   - `overlord_propaganda_addon_residual_heals_allies`
   - `emperor_innate_propaganda_and_helix_transport_residual`
   - `nuke_cannon_primary_residual_area_and_radiation`
   - module unit tests in `host_overlord_addons.rs` / `host_nuke_cannon.rs`
6. golden_skirmish lib tests: PASS (full vertical slice + retail map + synthetic).

**Still residual (fail-closed, not claimed):**
- Full OCL portable-structure passenger object + DamageModule share matrix
- Full W3DOverlord*Draw / W3DDependencyModelDraw bone attach / CONTINUOUS_FIRE_* anim
- Full NukeCannon DeployStyleAIUpdate unpack / projectile lob / ScatterRadiusVsInfantry
- Network addon / radiation replication (network deferred)

## Residual Host Playability — China Troop Crawler (2026-07-13)
**Closed (host-testable China Troop Crawler transport + detector + assault deploy residual):**
1. **Troop Crawler residual** (`ChinaVehicleTroopCrawler` / Tank_/Nuke_ variants):
   - `TransportContain` Slots=**8**, AllowInsideKindOf=INFANTRY (no passenger fire-from-inside).
   - `InitialPayload` residual: Redguard × **8** docked on spawn.
   - `StealthDetectorUpdate` residual: DetectionRange unset → VisionRange = **175**.
   - `AssaultTransportAIUpdate` + `TroopCrawlerAssault` DEPLOY residual:
     fire in range unloads docked infantry and orders them to attack the designated target.
   - Honesty: `honesty_troop_crawler_ok` / load_unload / assault_deploy / detect / initial_payload.
2. Tests (not log-only):
   - `troop_crawler_residual_capacity_detect_and_payload`
   - `troop_crawler_residual_detect_stealth_in_range`
   - `troop_crawler_residual_transport_load_unload`
   - `troop_crawler_residual_assault_deploy_unloads_and_attacks`
   - `troop_crawler_residual_rejects_vehicle_enter`
   - module unit tests in `host_troop_crawler.rs`

**Still residual (fail-closed, not claimed):**
- Full multi-exit-path ExitStart01-nn / ExitDelay 250ms stagger
- Full HealthRegen%PerSec / DamagePercentToUnits / MembersGetHealedAtLifeRatio retrieve matrix
- Full IR detector FX / IRParticleSys bones
- Network transport / deploy replication (network deferred)

## Residual Host Playability — China Dragon Tank + Gattling Tank (2026-07-13)
**Closed (host-testable China vehicle flame + continuous-fire ramp residual):**
1. **Dragon Tank residual** (`ChinaTankDragon` / general variants):
   - PRIMARY `DragonTankFlameWeapon` residual: PrimaryDamage **10** / radius **5**,
     SecondaryDamage **1** / radius **10**, AttackRange **75**, Delay **40**ms (2 frames).
   - Flame residual: intended + primary-radius units take full primary; secondary ring takes secondary dmg.
   - BlackNapalm PLAYER_UPGRADE residual (`Upgrade_ChinaBlackNapalm`): dmg **12.5** / sec **1.25**.
   - FireWall / Firestorm secondary remains `host_firewall` special-power residual (not re-opened).
   - Honesty: `honesty_dragon_tank_ok` / `honesty_dragon_tank_black_napalm_ok` / fires / units_hit.
2. **Gattling Tank residual** (`ChinaTankGattling` / vehicle variants):
   - PRIMARY `GattlingTankGun` (15 / 150 / 400ms) + SECONDARY `GattlingTankGunAir` (12 / 350 / AA).
   - Continuous-fire ramp residual (`FiringTracker` ContinuousFireOne=2 / Two=6 / Coast=1000ms):
     - Base delay **12** frames → MEAN **6** (200% RoF) → FAST **4** (300% RoF).
   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**.
   - Honesty: `honesty_gattling_tank_ok` / ramp / aa / ground_fires / ramp_fast / chain_gun_upgrades.
3. Tests (not log-only):
   - `dragon_tank_residual_flame_and_black_napalm`
   - `gattling_tank_residual_ramp_fire_rate_and_aa`
   - module unit tests in `host_dragon_tank.rs` / `host_gattling_tank.rs`

**Still residual (fail-closed, not claimed):**
- Full flamethrower ProjectileStream / garrison-clear AllowAttackGarrisonedBldgs matrix
- Full FiringTracker model-condition CONTINUOUS_FIRE_* animation / VoiceRapidFire full matrix
- Listening Outpost residual (closed 2026-07-13 — detect+transport)
- Overlord/Helix gattling payload residual (closed 2026-07-13 — portable addon path)
- Building Gattling continuous-fire ramp residual closed 2026-07-13 (see section below)
- Network flame / continuous-fire replication (network deferred)

## Residual Host Playability — China Gattling Cannon Structure Ramp (2026-07-13)
**Closed (host-testable structure gattling continuous-fire + AA + Chain Guns residual):**
1. **Gattling Cannon residual** (`ChinaGattlingCannon` / China_ / Nuke_ / Tank_ / Infa_):
   - PRIMARY `GattlingBuildingGun` (10 / 225 / 250ms → 8 frames).
   - SECONDARY `GattlingBuildingGunAir` (5 / 400 / AA only).
   - Continuous-fire ramp residual (`FiringTracker` ContinuousFireOne=**1** / Two=**5** /
     Coast=**2000**ms → 60 frames):
     - Base delay **8** frames → MEAN **4** (200% RoF) → FAST **2** (300% RoF).
   - Chain Guns PLAYER_UPGRADE residual (`Upgrade_ChinaChainGuns`): damage × **1.25**.
   - Auto-acquire residual (base-defense path) uses air/ground slot chooser.
   - Honesty: `honesty_gattling_building_ok` / ramp / aa / ground_fires / ramp_fast.
2. Tests (not log-only):
   - `gattling_building_residual_ramp_fire_rate_and_aa`
   - `base_defense_residual_gattling_auto_fires_without_attack_object` (updated)
   - module unit tests in `host_base_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full CONTINUOUS_FIRE_* model-condition animation / turret pitch / VoiceRapidFire full matrix
- Full AssistedTargetingModule Patriot assist clips / PointDefenseLaser for structure
  (Patriot AA dual-slot polish closed 2026-07-13 — see Stinger Site + Patriot section)
- Network structure continuous-fire / chain-gun replication (network deferred)

## Residual Host Playability — GLA Marauder + Combat Cycle (2026-07-13)
**Closed (host-testable GLA Marauder salvage fire-rate + Combat Cycle rider weapon residual):**
1. **Marauder residual** (`GLATankMarauder` / Chem_/Demo_/Slth_ / `GLA_MarauderTank`):
   - Salvage fire-rate tiers (`WEAPON_SALVAGER` / CRATEUPGRADE), **same PrimaryDamage 60**:
     - Tier 0: `MarauderTankGun` — Delay **2000**ms (60 frames), speed 300
     - Tier 1: `MarauderTankGunUpgradeOne` — Delay **1500**ms (45 frames), speed 400
     - Tier 2: `MarauderTankGunUpgradeTwo` — Delay **750**ms (23 frames), speed 500
   - Range **170**, PrimaryDamageRadius **5** splash residual on fire.
   - Honesty: `honesty_marauder_ok` / `honesty_marauder_weapon_upgrade_ok` / fires.
2. **Combat Cycle residual** (`GLAVehicleCombatBike` / Rocket / Terrorist variants):
   - `RiderChangeContain` Slots=**1**, infantry only; bike fires (passengers do not).
   - WeaponSet residual: Conditions=None → PRIMARY **NONE**; InitialPayload Rebel →
     `GLARebelBikerMachineGun` (8 / 150 / 100ms).
   - Rider weapon switch residual:
     - Rebel → `GLARebelBikerMachineGun`
     - TunnelDefender → `TunnelDefenderBikerRocketWeapon` (40 / 175 / AA / min 5)
     - Jarmen Kell → `GLABikerKellSniperRifle` (180 / 225)
     - Terrorist → `SuicideBikeBomb` area residual (700/20 + 100/50) + self-destroy
     - Worker / Hijacker / Saboteur → no combat weapon residual
   - Enter residual refreshes rider weapon from occupant template.
   - Honesty: `honesty_combat_cycle_ok` / rider_switch / fire / loads / suicides.
3. Tests (not log-only):
   - `marauder_residual_salvage_fire_rate_tiers`
   - `combat_cycle_residual_rider_weapon_switch`
   - module unit tests in `host_marauder.rs` / `host_combat_cycle.rs`

**Still residual (fail-closed, not claimed):**
- Full SalvageCrate W3D turret subobject swap (Turret / TurretUp01 / TurretUp02)
- Full Marauder ClipSize=2 / ClipReloadTime=100 dual-shot cadence matrix
- Full RiderChangeContain STATUS_RIDER* death OCL / ScuttleDelay TOPPLED matrix
- Full UseRiderStealth / Jarmen Kell secondary pilot-sniper AutoChoose matrix
- Network salvage / rider replication (network deferred)

## Residual Host Playability — GLA Technical + Toxin Tractor (2026-07-13)
**Closed (host-testable GLA Technical transport/salvage + Toxin Tractor poison residual):**
1. **Technical residual** (`GLAVehicleTechnical` / chassis reskins / Chem_/Demo_/Slth_):
   - `TransportContain` Slots=**5**, infantry only (passengers “garrison” the truck bed).
   - Passengers do **not** fire (`PassengersAllowedToFire` unset in retail).
   - Salvage weapon upgrade residual (WEAPON_SALVAGER / CRATEUPGRADE):
     - Tier 0: `TechnicalMachineGunWeapon` (10 dmg / 150 range / 200ms)
     - Tier 1: `TechnicalCannonWeapon` (45 dmg / 150 range / 1000ms / radius 25 splash)
     - Tier 2: `TechnicalRPGWeapon` (50 dmg / 150 range / min 5 / 1000ms / radius 5)
   - Honesty: `honesty_technical_ok` / weapon_upgrades / loads / unloads / fires.
2. **Toxin Tractor residual** (`GLAVehicleToxinTruck`):
   - PRIMARY `ToxinTruckGun` poison stream (10 dmg / radius 10 / range 100).
   - SECONDARY `ToxinTruckSprayer` contaminate residual: SecondaryDamage **2** /
     radius **75** + spawn MediumPoisonField DoT (2 / 80 / 30s / 500ms ticks).
   - Death residual: SmallPoisonField (2 / 12 / 10s) via ToxinShellWeapon path.
   - Honesty: `honesty_toxin_tractor_stream_ok` / `honesty_toxin_tractor_spray_ok` /
     `honesty_toxin_tractor_death_field_ok` / `honesty_toxin_tractor_ok`.
3. Tests (not log-only):
   - `technical_residual_transport_and_salvage_weapon`
   - `toxin_tractor_residual_stream_spray_and_death_field`
   - module unit tests in `host_technical.rs` / `host_toxin_tractor.rs`

**Still residual (fail-closed, not claimed):**
- Full SalvageCrate W3D gunner/turret subobject swap matrix (Technical/Toxin)
- Full FireOCLAfterWeaponCooldown MinShots=4 continuous-coast timer for spray
- Full stream projectile drawing / spigot bone / chassis reskin visual matrix
- Network salvage / toxin replication (network deferred)

## Residual Host Playability — GLA Rocket Buggy / Quad Cannon / SCUD (2026-07-13)
**Closed (host-testable GLA combat vehicle residuals):**
1. **Rocket Buggy residual** (`GLAVehicleRocketBuggy` / Chem_/Demo_/Slth_):
   - Seeds `BuggyRocketWeapon` (range **300**, min **50**, dmg **20**, clip **6**).
   - Fire residual: intended target PrimaryDamage; units in SecondaryDamageRadius
     **10** take SecondaryDamage **5** splash.
   - `ScatterRadiusVsInfantry` residual (deterministic miss vs infantry).
   - Honesty: `honesty_rocket_buggy_ok` / fires / units_hit / scatter_misses.
2. **Quad Cannon residual** (`GLAVehicleQuadCannon`):
   - PRIMARY `QuadCannonGun` ground (range **150**, dmg **10**, no air).
   - SECONDARY `QuadCannonGunAir` AA (range **350**, dmg **5**, AntiGround=No).
   - Chooser residual: airborne → secondary; ground → primary.
   - Multi-barrel salvage tier residual (`apply_quad_cannon_barrel_tier` 0/1/2
     fire-rate + weapon-name residual).
   - Honesty: ground_fires / aa_fires / barrel_upgrades.
3. **SCUD launcher residual** (`GLAVehicleScudLauncher`):
   - PRIMARY explosive area (300/50 + 50/100), range **350**, min **200**.
   - SECONDARY toxin area (200/30 + 25/60) + **MediumPoisonField** DoT
     (2 dmg / radius 80 / 30s / 500ms ticks).
   - PreferredAgainst residual: secondary vs infantry (toxin).
   - Honesty: `honesty_scud_area_ok` / `honesty_scud_toxin_ok` /
     `honesty_scud_launcher_ok`.
4. Tests (not log-only):
   - `rocket_buggy_residual_long_range_splash`
   - `quad_cannon_residual_anti_air_and_multi_barrel`
   - `scud_launcher_residual_area_and_toxin`
   - module unit tests in `host_rocket_buggy.rs` / `host_quad_cannon.rs` /
     `host_scud_launcher.rs`

**Still residual (fail-closed, not claimed):**
- Full projectile lob / MissileCallsOnDie / PreAttack PER_SHOT animation matrix
- Full SalvageCrate W3D turret subobject swap / AP rocket mult matrix
- Full Anthrax Beta upgraded poison particle / gamma field matrix
- Network weapon / toxin replication (network deferred)

## Residual Host Playability — Bomb Truck Disguise + GLA Camouflage (2026-07-12)
**Closed (host-testable disguise + Camouflage stealth upgrade):**
1. **Bomb Truck disguise residual** (`SpecialAbilityDisguiseAsVehicle` /
   `GLAVehicleBombTruck` StealthUpdate DisguisesAsTeam):
   - `CommandType::DisguiseAsVehicle` on bomb-truck residual casters
     (`*BombTruck*` / `TestBombTruck`) targeting any living ground vehicle
     (ally/enemy/neutral) except bomb trucks / trains / aircraft.
   - Instant complete residual (retail `StartAbilityRange = 1e6`).
   - Sets `OBJECT_STATUS_DISGUISED` + `STEALTHED`, stores disguise template + team.
   - Enemies auto-target via **apparent team** residual (disguised-as-USA skips USA
     auto-target; China still targets as enemy). Not pure-stealth hide
     (`is_effectively_stealthed` excludes DISGUISED — C++ `!DISGUISED` gate).
   - Reveal residual: while attacking, if 2D distance to victim ≤ **100**
     (`RevealDistanceFromTarget`) → clear disguise + stealth + honesty reveal.
   - Honesty: `honesty_bomb_truck_disguise_ok` / `honesty_bomb_truck_reveal_ok` /
     `honesty_bomb_truck_disguise_path_ok`.
2. **GLA Camouflage residual** (`Upgrade_GLACamouflage` / Rebel `StealthUpgrade`):
   - `HostUpgradeKind::Camouflage` QueueUpgrade → complete grants residual
     STEALTHED + `innate_stealth` to Rebel infantry templates
     (`GLAInfantryRebel` / Chem_/Demo_ variants / `TestRebel`).
   - Attack breaks stealth (`stealth_breaks_on_attack`); idle re-cloak residual
     in `update_stealth_and_detection`.
   - **Workers fail-closed**: `GLAInfantryWorker` has no StealthUpgrade for
     Camouflage in retail INI — residual correctly skips workers.
   - Honesty: host_upgrades `honesty_host_path_ok(Camouflage)` with units_affected.
3. Tests (not log-only):
   - `bomb_truck_disguise_residual_applies_and_hides_from_disguise_team`
   - `bomb_truck_disguise_residual_reveals_near_attack_target`
   - `bomb_truck_disguise_residual_rejects_non_bomb_truck_caster`
   - `camouflage_upgrade_queue_complete_stealths_rebel`
   - `camouflage_residual_attack_breaks_and_idle_recloaks`
   - module unit tests in `host_bomb_truck_disguise.rs` / `host_upgrades` camouflage helpers

**Still residual (fail-closed, not claimed):**
- Full StealthUpdate disguise transition opacity / half-point model swap / FX
- Full drawable indicator-color night/day matrix for disguised players
- Full 2500ms StealthDelay re-cloak timer / FriendlyOpacity pulse for Camouflage
- Bomb truck FireWeaponWhenDead HE/Bio residual closed 2026-07-13 (see Helix Napalm + Bomb Truck HE/Bio section)
- Network disguise / camouflage replication (network deferred)

## Residual Host Playability — GLA Tunnel Network Enter/Exit (2026-07-12)
**Closed (host-testable TunnelContain shared pool + cross-tunnel exit):**
1. **Tunnel Network residual** (`GLATunnelNetwork` / general variants / SneakAttack tunnel):
   - `TunnelContain` shared passenger pool per team (`GameData.ini MaxTunnelCapacity = 10`).
   - Enter any allied tunnel network structure (all ground units; aircraft residual-skip).
   - Exit / Evacuate on **any** allied tunnel dumps the shared pool at that tunnel
     (enter A → evacuate B places unit at B — key residual path).
   - Honesty: `honesty_tunnel_network_enter_exit_ok` /
     `honesty_tunnel_network_cross_exit_ok` / `honesty_tunnel_network_ok`.
2. Tests (not log-only):
   - `tunnel_network_residual_flags_and_capacity_installed`
   - `tunnel_network_residual_enter_sets_garrisoned_and_shared_pool`
   - `tunnel_network_residual_cross_exit_enter_a_evacuate_b`
   - `tunnel_network_residual_shared_capacity_full_rejects_enter`
   - `tunnel_network_residual_rejects_aircraft`
   - module unit tests in `host_tunnel_network.rs`

**Still residual (fail-closed, not claimed):**
- Full GuardTunnelNetwork / AITNGuard nemesis AI path
- Full TimeForFullHeal / healObjects tick while contained
- Full CaveSystem multi-index / last-tunnel cave-in destroy matrix
- Full ExitStart bone / multi-door exit interface
- TunnelNetworkGun residual closed 2026-07-13 — see Laser General + Tunnel Network Gun section
- Network tunnel-network replication (network deferred)

## Residual Host Playability — Pathfinder Stealth Detect + Scout/Hellfire Drones (2026-07-12)
**Closed (host-testable detect + stealth + drone attach/auto-fire):**
1. **Pathfinder residual** (`AmericaInfantryPathfinder`):
   - Spawns as stealth detector residual (DetectionRange unset → VisionRange **200**).
   - Innate stealth; `stealth_breaks_on_attack = false` (stays stealthed while sniping).
   - `stealth_breaks_on_move = true`: uncloaks while Moving/AttackMoving; re-cloaks when stopped
     (StealthDelay = 0 residual).
   - PRIMARY `USAPathfinderSniperRifle` (100 dmg / 300 range / 2000 ms).
   - Honesty: `honesty_pathfinder_detect_ok` / `honesty_pathfinder_sniper_ok`.
2. **Scout Drone residual** (`AmericaVehicleScoutDrone` / attach from Humvee):
   - Spawns as detector residual (VisionRange **150**); no primary weapon.
   - `residual_attach_slave_drone(master, Scout)` tags master with `Upgrade_AmericaScoutDrone`.
   - Honesty: `honesty_scout_drone_attach_ok` / `honesty_scout_drone_detect_ok`.
3. **Hellfire Drone residual** (`AmericaVehicleHellfireDrone` / attach from Humvee):
   - PRIMARY `HellfireMissileWeapon` (40 dmg / 150 range / ~3s cycle).
   - AutoAcquireEnemiesWhenIdle residual auto-fire (same idle-gate pattern as Sentry).
   - `residual_attach_slave_drone(master, Hellfire)` tags master with `Upgrade_AmericaHellfireDrone`.
   - Honesty: `honesty_hellfire_drone_attach_ok` / `honesty_hellfire_drone_auto_fire_ok`.
4. Tests (not log-only):
   - `pathfinder_residual_detect_stealth_and_sniper`
   - `scout_and_hellfire_drone_residual_attach_detect_and_fire`
   - `slave_drone_residual_rejects_non_master_attach`
   - module unit tests in `host_pathfinder.rs` / `host_slave_drones.rs`

**Closed (host-testable Combat Chinook passenger fire residual):**
1. **Combat Chinook residual** (`AirF_AmericaVehicleChinook`):
   - TransportContain Slots=**8**, `PassengersAllowedToFire=Yes`,
     `ArmedRidersUpgradeMyWeaponSet=Yes` (ListeningOutpostUpgradedDummyWeapon bind).
   - AllowInsideKindOf residual: infantry + vehicle (rejects aircraft).
   - Docked riders residual-fire from chinook origin (Battle Bus pattern).
   - PointDefenseLaser residual name matrix includes AirF Combat Chinook
     (ScanRange 250 / AttackRange 65 / Delay ~8 frames).
   - Honesty: `honesty_combat_chinook_load_unload_ok` /
     `honesty_combat_chinook_passenger_fire_ok` /
     `honesty_combat_chinook_weapon_set_upgrade_ok`.
   - Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
2. Tests:
   - `combat_chinook_residual_capacity_and_flags_installed`
   - `combat_chinook_residual_enter_sets_docked_and_upgrades_weapon_set`
   - `combat_chinook_residual_load_two_unload_both_free`
   - `combat_chinook_residual_passenger_fire_damages_nearby_enemy`
   - `combat_chinook_residual_capacity_full_rejects_enter`
   - `combat_chinook_residual_allows_vehicle_enter`
   - module unit tests in `host_combat_chinook.rs`

**Still residual (fail-closed, not claimed):**
- Full SlavedUpdate guard/scout wander ranges / master layer lock
- Full ObjectCreationUpgrade ConflictsWith / ProductionUpdate door UI per-vehicle
- Full Pathfinder SCIENCE_Pathfinder prereq gate / FriendlyOpacity pulse / IR detector FX
- Full ChinookAIUpdate ropes / supply warehouse boxes / rappel / combat drop clear
- Humvee TOW / FlashBang remain partial via existing host upgrades

## Residual Host Playability — USA Crusader / Paladin / Avenger / Humvee (2026-07-13)
**Closed (host-testable tank guns, FAERIE_FIRE paint, Composite Armor, Humvee polish):**
1. **Crusader residual** (`AmericaTankCrusader` / `USA_Crusader`):
   - PRIMARY `CrusaderTankGun` (60 dmg / 150 range / 2000 ms → 60 frames).
   - Upgrade_AmericaCompositeArmor MaxHealthUpgrade residual **+100** HP
     (ADD_CURRENT_HEALTH_TOO) on Crusader / Paladin.
   - Honesty: CompositeArmor host upgrade kind + unit max_health observability.
2. **Paladin residual** (point defense already partial; this pass):
   - PRIMARY `PaladinTankGun` (same gun residual as Crusader).
   - PDL secondary infantry intercept fail-closed to **Paladin only**
     (Avenger / King Raptor / Combat Chinook: missiles only).
   - Shares Composite Armor residual with Crusader.
3. **Avenger residual** (`AmericaTankAvenger` / Target Designator):
   - PRIMARY paints `OBJECT_STATUS_FAERIE_FIRE` (status residual; no HP damage).
   - Duration 200 ms → 6 frames residual (retail continuous refresh).
   - Allies shooting painted targets get TARGET_FAERIE_FIRE **150%** ROF residual.
   - SECONDARY air laser residual (dual-turret collapse): 10 dmg / 300 range / AA only.
   - PDL intercept remains via `host_point_defense`.
   - Honesty: `honesty_avenger_paint_ok` / `honesty_avenger_air_laser_ok` /
     `honesty_avenger_rof_ok` / `honesty_avenger_ok`.
4. **Humvee residual polish** (`AmericaVehicleHumvee`):
   - TransportContain Slots=**5**, PassengersAllowedToFire=Yes residual install.
   - TOW secondary residual gains air tertiary capability after research
     (range 320 / can_target_air; damage boost **50** vs aircraft residual).
5. Tests (not log-only):
   - `avenger_residual_designator_paint_and_rof`
   - `avenger_residual_air_laser_damages_aircraft`
   - `crusader_residual_tank_gun_and_composite_armor`
   - `humvee_residual_transport_and_air_tow`
   - module unit tests in `host_avenger.rs` / `host_usa_tanks.rs` / `host_humvee.rs`

**Still residual (fail-closed, not claimed):**
- Full portable AmericaTankAvengerLaserTurret OverlordContain dual-stream matrix
- Full StatusDamageHelper Xfer / multi-status exclusivity
- Full ArmorSet PLAYER_UPGRADE UpgradedTankArmor matrix
- Full SCIENCE_PaladinTank prereq gate / ProductionUpdate door UI
- Full WeaponSet PLAYER_UPGRADE Humvee turret visual swap
- Network FAERIE_FIRE / composite / TOW air replication (network deferred)

- Network detector / drone replication (network deferred)

# GeneralsRust Playability State (2026-04-02)

## Residual Host Playability — Microwave Tank + King Raptor Laser (2026-07-12)
**Closed (host-testable Microwave disable/clear + King Raptor PDL):**
1. **Microwave Tank residual** (`MicrowaveTankBuildingDisabler` / `BuildingClearer`):
   - AmericaTankMicrowave / *Microwave* residual sources, while **attacking** an
     enemy/neutral structure within AttackRange residual **200**, apply
     `DISABLED_SUBDUED` (`disabled_subdued` → `is_disabled()` so production
     / powered functions stop while cooked).
   - Clears when microwave stops targeting / leaves range / dies.
   - Garrison clear residual (KILL_GARRISONED / PrimaryDamage **1** occupant per
     shot, AttackRange **125**) via existing combat path +
     `host_bunker_buster` clearer residual (no structure HP damage).
   - Ally structures residual-skip (fail-closed vs retail RadiusDamageAffects ALLIES).
   - Honesty: `honesty_microwave_disable_ok` / `honesty_microwave_ok` +
     `honesty_kill_garrisoned_ok` for clear path.
2. **King Raptor residual laser** (`PointDefenseLaserUpdate` dual modules):
   - AirF_AmericaJetRaptor / TestKingRaptor residual carriers added to
     `host_point_defense` PDL path (regular AmericaJetRaptor fail-closed skip).
   - Dual lasers residual collapse: AirF_RaptorPointDefenseLaser +
     AirF_PointDefenseLaser → fire range **65**, delay **4** frames
     (250ms each dual-stream collapse), damage **100**, scan **200**.
   - Intercepts enemy missiles / projectiles in fire range (same primary
     residual as Paladin/Avenger).
3. Tests (not log-only):
   - `microwave_tank_residual_disables_enemy_structure`
   - `microwave_tank_residual_skips_non_microwave`
   - `kill_garrisoned_residual_microwave_clears_occupants`
   - `king_raptor_residual_laser_intercepts_missile`
   - `king_raptor_residual_skips_regular_raptor`
   - unit tests in `host_microwave.rs` / `host_point_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full subdual damage accumulate / SubdualDamageHelper heal drain matrix
- Full MicrowaveDisableStream laser attach / FireWeaponUpdate emitter
  infantry MICROWAVE field (MicrowaveTankEmitterWeapon)
- Vehicle disabler (retail WeaponSet has MicrowaveTankVehicleDisabler commented out)
- Full dual independent PointDefenseLaserUpdate scan-rate / PredictTargetVelocity
- Network microwave / King Raptor PDL replication (network deferred)

## Residual Host Playability — Particle Uplink Continuous Beam + Cleanup Area (2026-07-12)
**Closed (host-testable ParticleCannon multi-pulse beam + Ambulance CleanupArea):**
1. **Particle Uplink continuous beam residual** (`ParticleUplinkCannonUpdate` host path):
   - `DoSpecialPower(ParticleCannon)` still queues charge residual (impact_frame
     delay **120** frames).
   - On complete: **no one-shot blast** (was residual 3000 flat); spawns
     `HostParticleBeamField` at target.
   - Beam residual: TotalFiringTime **3500 ms → 105 frames**, TotalDamagePulses
     **40**, DamagePerSecond **400** → **35 dmg/pulse**, pulse interval **3**
     frames, radius **50** (fail-closed vs DamageRadiusScalar grow matrix /
     swath sine / manual beam driving).
   - First pulse on beam-start frame; subsequent pulses until duration/pulse cap.
   - Honesty: `honesty_beam_ok` / `honesty_beam_damage_ok` /
     `honesty_host_path_ok(ParticleCannon)` requires beam field spawn.
2. **Cleanup Area residual** (`CleanupAreaPower` / AmbulanceCleanHazardWeapon):
   - `DoSpecialPower(CleanupArea)` from ambulance/medic/dozer/worker clears
     residual **toxin + radiation fields** and **enemy/neutral mines** in radius
     **50** (PrimaryDamageRadius residual) when caster is within
     MaxMoveDistanceFromLocation **300**.
   - Mine clear is safe disarm (no splash) via existing `clear_mine_internal`.
   - Honesty: `honesty_cleanup_area_activate_ok` / `honesty_cleanup_area_clear_ok` /
     `honesty_cleanup_area_ok`.
3. Tests (not log-only):
   - `particle_cannon_params_match_retail_continuous_beam`
   - `particle_cannon_impact_spawns_beam_and_ticks_damage`
   - `particle_cannon_host_path_queues_and_completes` (multi-pulse E2E)
   - `cleanup_area_residual_clears_hazards_and_mines`
   - `cleanup_area_does_not_queue_superweapon_strike`
   - unit tests in `host_cleanup_area.rs`

**Still residual (fail-closed, not claimed):**
- Full ParticleUplinkCannonUpdate outer-node lasers / WidthGrowTime / SwathOfDeath
  sine path / ManualDrivingSpeed / DamagePulseRemnant trail objects
- Full CleanupHazardUpdate scan/shot/clip / CleanupStreamProjectile path
- Full HazardousMaterialArmor CLEANUP_HAZARD KindOf object stack / rubble pathfind
- Network Particle Uplink / CleanupArea replication (network deferred)

## Residual Host Playability — Oil Derrick + Hacker Internet Center Cash (2026-07-12)
**Closed (host-testable TechOilDerrick AutoDeposit + China Hacker/Internet Center cash):**
1. **Oil Derrick residual income** (`TechOilDerrick` AutoDepositUpdate residual):
   - Captured (non-neutral) constructed derrick deposits **$200** every **360** logic
     frames (retail DepositAmount=200, DepositTiming=12000 ms @ 30 FPS).
   - **InitialCaptureBonus $1000** once when a derrick first becomes non-neutral owned
     (Player::gainObject → awardInitialCaptureBonus residual).
   - Neutral / under-construction residual-skip.
   - Honesty: `honesty_oil_derrick_deposit_ok` / `honesty_oil_derrick_capture_bonus_ok`
     / `honesty_oil_derrick_ok`.
2. **Hacker / Internet Center residual cash** (`HackInternetAIUpdate` residual):
   - Field: `start_hacker_internet_hack` starts residual hacking → **$5** every **60**
     frames (RegularCashAmount / CashUpdateDelay 2000 ms).
   - Internet Center: hackers `contained_by` FSInternetCenter auto-start and use
     CashUpdateDelayFast → **$5** every **54** frames (1800 ms).
   - Veterancy residual: Regular/Veteran/Elite/Heroic = 5/6/8/10; XpPerCashUpdate +1.
   - DISABLED_HACKED residual-skip (no deposit while disabled).
   - Honesty: `honesty_hacker_income_ok` / `honesty_hacker_internet_center_ok`.
3. Tests (not log-only):
   - `oil_derrick_residual_deposits_cash_on_interval`
   - `oil_derrick_residual_skips_under_construction`
   - `hacker_internet_center_residual_deposits_cash`
   - `hacker_field_residual_deposits_cash_on_interval`
   - `hacker_residual_rejects_non_hacker`
   - unit tests in `host_oil_derrick.rs` / `host_hacker_income.rs`

**Still residual (fail-closed, not claimed):**
- Full InGameUI AutoDeposit floating text GPU / STEALTHED gate (host floating text + SupplyLines +20 residual closed 2026-07-13 — see AutoDeposit Floating Cash + Oil SupplyLines Boost section)
- Full HackInternet unpack/pack state machine / variation factor / model conditions
- Full InternetHackContain passenger order matrix / microwave resume path
- Supply Drop Zone residual (OCL crate plane every 120s — not this slice)
- Network oil-derrick / hacker cash replication (network deferred)

## Residual Host Playability — Remote Demo Charge + Black Market Cash (2026-07-12)
**Closed (host-testable plant/detonate remote C4 + GLA Black Market AutoDeposit):**
1. **Remote Demo Charge residual** (Colonel Burton `SPECIAL_REMOTE_CHARGES`):
   - `PlantRemoteDemoCharge { target_id }` walks to structure/vehicle → plants sticky
     `HostMineKind::RemoteDemoCharge` (no auto-timer; TNTDetonationWeapon residual
     damage **500** / radius **50**).
   - `DetonateRemoteDemoCharges` detonates all remote charges with matching
     `producer_id` (C++ no-target SPECIAL_REMOTE_CHARGES path).
   - Timed charges remain separate (`PlantTimedDemoCharge` + LifetimeUpdate residual).
   - Honesty: `honesty_plant_remote_demo_charge_ok` /
     `honesty_remote_demo_charge_detonate_ok` (+ hero registry counters).
2. **GLA Black Market residual cash** (`AutoDepositUpdate` ModuleTag_05 residual):
   - Constructed `FSBlackMarket` / `*BlackMarket*` deposits **$20** every **60** logic
     frames (retail DepositAmount=20, DepositTiming=2000 ms @ 30 FPS).
   - Fake markets (`*Fake*`) residual-skip (ActualMoney=No).
   - Neutral / under-construction residual-skip.
   - Honesty: `honesty_black_market_deposit_ok` / `honesty_black_market_ok`.
3. Tests (not log-only):
   - `plant_and_detonate_remote_demo_charge_residual`
   - `black_market_residual_deposits_cash_on_interval`
   - `black_market_residual_cash_increases_over_frames`
   - `black_market_residual_skips_under_construction`
   - `black_market_residual_skips_fake_market`
   - unit tests in `host_mines.rs` / `host_black_market.rs` / `host_hero_abilities.rs`

**Still residual (fail-closed, not claimed):**
- Full StickyBombUpdate attach bones / max-charge special-object list / packing anim
- Full InGameUI AutoDeposit floating text GPU (host floating text residual closed 2026-07-13 — see AutoDeposit Floating Cash section)
- Supply Drop Zone residual (OCL crate plane — still open)
- Fuel Air Bomb is already covered by DaisyCutter host strike path (not reopened)
- Network remote-charge / black-market replication (network deferred)

## Residual Host Playability — Artillery Barrage (2026-07-12)
**Closed (host-testable Artillery Barrage → delayed multi-shell scatter damage):**
1. **Artillery Barrage residual** (`SUPERWEAPON_ArtilleryBarrage1` host path):
   - `DoSpecialPower(Artillery)` queues a delayed strike (retail DelayDeliveryMax
     3000 ms → 90 frames @ 30 FPS).
   - On impact: multi-shell scatter area damage — Level1 FormationSize **12** shells
     (center + deterministic ring inside WeaponErrorRadius **100**).
   - Per-shell residual: `ArtilleryBarrageDamageWeapon` PrimaryDamage **105** /
     PrimaryDamageRadius **50**.
   - Each living enemy takes max damage from any shell epicenter (not single-point only).
   - Friendlies residual-skip (fail-closed vs retail RadiusDamageAffects ALLIES).
2. Honesty counters (`special_power_strikes.rs` HostSuperweaponKind::ArtilleryBarrage):
   - queue / complete / host_path honesty flags (same strike residual path as CarpetBomb)
3. Tests (not log-only):
   - `artillery_barrage_host_path_queues_and_applies_delayed_multi_shell_damage`
   - `artillery_barrage_params_match_retail_multi_shell`
   - `artillery_barrage_delayed_multi_shell_scatter_damage`

**Still residual (fail-closed, not claimed):**
- Full ChinaArtilleryCannon OCL DeliverPayload transport / shell projectile path
- Random WeaponErrorRadius scatter draw + per-shell DelayDelivery stagger
- Science tier FormationSize 24/36 upgrade matrix
- Friendly-fire (retail hits ALLIES NEUTRALS ENEMIES)
- Network Artillery Barrage replication (network deferred)

## Residual Host Playability — Emergency Repair (2026-07-12)
**Closed (host-testable Emergency Repair → SingleBurst ally vehicle heal):**
1. **Emergency Repair residual** (`SuperweaponEmergencyRepair` host path):
   - `DoSpecialPower(EmergencyRepair)` at a world location heals damaged **same-team VEHICLE**
     units in residual radius **100** (retail RadiusCursorRadius /
     RepairVehiclesInArea_InvisibleMarker AutoHealBehavior Radius).
   - HealingAmount residual **100 / 200 / 300** by Level1/2/3 (default Level1 fail-closed).
   - KindOf VEHICLE only; infantry / enemies / full-HP / out-of-radius residual-skip.
2. Honesty counters (`host_emergency_repair.rs` + GameLogic):
   - `activation_count` / `heal_count` / `heal_amount_total`
   - `honesty_emergency_repair_activate_ok` / `honesty_emergency_repair_heal_ok` /
     `honesty_emergency_repair_ok`
3. Tests (not log-only):
   - `emergency_repair_residual_heals_damaged_ally_vehicles`
   - `emergency_repair_does_not_queue_superweapon_strike`
   - unit tests in `host_emergency_repair.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL RepairVehicles invisible marker / RepairCloud particle path
- Full science tier upgrade matrix (Level2/3 selection from player sciences)
- Full ally relationship filter (uses same-team residual)
- Network Emergency Repair replication (network deferred)

## Residual Host Playability — GPS Scrambler (2026-07-12)
**Closed (host-testable GPS Scrambler → GrantStealth ally vehicles/infantry):**
1. **GPS Scrambler residual** (`SuperweaponGPSScrambler` host path):
   - `DoSpecialPower(GpsScrambler)` at a world location grants **STEALTHED** to
     same-team **VEHICLE|INFANTRY** in residual FinalRadius **100**
     (retail GrantStealthBehavior receiveGrant → CAN_STEALTH + STEALTHED).
   - Stealthed-and-undetected units are not enemy-targetable / not visible to enemies
     (existing host stealth gates). Attack still breaks stealth
     (STEALTH_NOT_WHILE_ATTACKING residual).
   - Skips bomb-truck disguise residual by name (C++ canDisguise skip).
   - Note: older module comments claiming "disables enemy radar" are incorrect for
     ZH retail — this is GrantStealth on allies, not radar jam.
2. Honesty counters (`host_gps_scrambler.rs` + GameLogic):
   - `activation_count` / `grant_count`
   - `honesty_gps_scrambler_activate_ok` / `honesty_gps_scrambler_grant_ok` /
     `honesty_gps_scrambler_ok`
3. Tests (not log-only):
   - `gps_scrambler_residual_grants_stealth_to_ally_units`
   - `gps_scrambler_does_not_queue_superweapon_strike`
   - unit tests in `host_gps_scrambler.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL GPSScrambler_InvisibleMarker grow-from-StartRadius pulse scan
- Full StealthUpdate module framesGranted / opacity drawable / flashAsSelected
- Full ally relationship filter (uses same-team residual)
- Network GPS Scrambler replication (network deferred)

## Residual Host Playability — Leaflet Drop (2026-07-12)
**Closed (host-testable USA Leaflet Drop → delayed enemy infantry/vehicle disable):**
1. **Leaflet Drop residual** (`LeafletDropBehavior` host path):
   - `DoSpecialPower(LeafletDrop)` queues a delayed mission (retail Delay=2500 ms → 75 frames).
   - On impact: enemy **INFANTRY|VEHICLE** in AffectRadius residual **110** receive
     `DISABLED_EMP` for DisabledDuration residual **20000 ms → 600 frames**.
   - Allies / Neutral / structures residual-skip (C++ relationship ENEMIES + kind filter).
   - Reuses host `disabled_emp` tick path (cannot move/attack until expiry).
2. Honesty counters (`host_leaflet_drop.rs` + GameLogic):
   - `activation_count` / `disable_count`
   - `honesty_leaflet_drop_activate_ok` / `honesty_leaflet_drop_disable_ok` /
     `honesty_leaflet_drop_ok`
3. Tests (not log-only):
   - `leaflet_drop_residual_disables_enemy_infantry_and_vehicles`
   - unit tests in `host_leaflet_drop.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL AmericaJetB52 / LeafletContainer drawable / LeafletFX particle path
- Full EarlyLeafletDrop science shortcut timer matrix
- Full multiplayer SharedSyncedTimer / academy classification
- Network leaflet replication (network deferred)

## Residual Host Playability — GLA Sneak Attack (2026-07-12)
**Closed (host-testable Sneak Attack → delayed tunnel spawn + shockwave):**
1. **Sneak Attack residual** (`SuperweaponSneakAttack` host path):
   - `DoSpecialPower(SneakAttack)` queues mission with Lifetime residual **5000 ms → 150 frames**
     (retail GLASneakAttackTunnelNetworkStart LifetimeUpdate).
   - On spawn: creates tunnel structure (`GLASneakAttackTunnelNetwork` if loaded, else
     residual `TestSneakTunnel`) for casting team at target location.
   - Residual shockwave pulse at spawn (SneakAttackShockwaveWeaponBig: **50 dmg / radius 50**).
2. Honesty counters (`host_sneak_attack.rs` + GameLogic):
   - `activation_count` / `tunnel_spawn_count` / `shockwave_hit_count`
   - `honesty_sneak_attack_activate_ok` / `honesty_sneak_attack_tunnel_ok` /
     `honesty_sneak_attack_shockwave_ok` / `honesty_sneak_attack_ok`
3. Tests (not log-only):
   - `sneak_attack_residual_spawns_tunnel_and_shockwave`
   - unit tests in `host_sneak_attack.rs`

**Still residual (fail-closed, not claimed):**
- Full OCL Start model animation / crack-dust particle stack
- Full multi-shockwave FireWeaponUpdate timing (10ms / 1000ms / 2500ms)
- Full GuardTunnelNetwork AI path (enter/exit residual closed above)
- SharedSyncedTimer / multiplayer academy classification
- Network sneak-attack replication (network deferred)

## Residual Host Playability — PointDefenseLaser Intercept (2026-07-12)
**Closed (host-testable Paladin/Avenger auto-destroy nearby enemy missiles):**
1. **PointDefenseLaser residual** (`PointDefenseLaserUpdate` host path):
   - `GameLogic::update_point_defense_intercept` each frame (from `update_simulation`).
   - Carriers: name residual `paladin` / `avenger` (incl. general variants / TestPaladin).
   - Primary targets: `KindOf::Projectile` or missile name residual
     (`missile` / `rocket` / `scud` / `tomahawk` / `TestMissile`); skips
     MissileDefender / Patriot Battery / Stinger Site false positives.
   - Secondary residual: enemy infantry in fire range (Paladin SecondaryTargetTypes).
   - Stats residual: Paladin range 65 / delay 30 frames / dmg 100;
     Avenger range 100 / delay 15 frames / dmg 100.
2. Honesty counters:
   - `point_defense_residual_intercepts`
   - `honesty_point_defense_intercept_ok`
3. Tests (not log-only):
   - `point_defense_laser_residual_intercepts_missile`
   - `point_defense_laser_residual_skips_non_carrier`
   - unit tests in `host_point_defense.rs`

**Still residual (fail-closed, not claimed):**
- Full PointDefenseLaserUpdate velocity prediction / scan-rate matrix
- Full KindOf BALLISTIC_MISSILE SMALL_MISSILE mask parse path
- Full TERTIARY WeaponStore allocateNewWeapon + laser beam drawable FX
- Network PDL replication (network deferred)

## Residual Host Playability — Neutron Shells (2026-07-12)
**Closed (host-testable Upgrade_ChinaNeutronShells → Nuke Cannon secondary blast):**
1. **Neutron shell residual** (`Upgrade_ChinaNeutronShells` + `NeutronBlastBehavior`):
   - QueueUpgrade complete equips Nuke Cannon SECONDARY `NukeCannonNeutronWeapon`
     (range 350, dmg 1 seed; blast does the work).
   - Combat secondary fire (`active_weapon_slot == 1` or only-ready secondary)
     applies residual blast radius 70 at impact:
     - Infantry killed
     - Vehicles unmanned + Neutral (HP preserved; combat-bike residual killed)
   - Primary NukeCannonGun still uses normal HP damage (no blast).
2. Honesty counters:
   - `neutron_shell_residual_blasts` / `infantry_kills` / `vehicles_unmanned`
   - `honesty_neutron_shell_ok`
3. PreferredAgainst AutoChoose residual expansion:
   - Secondary preferred vs infantry when secondary damage > primary (FlashBang)
   - Secondary preferred vs vehicles when secondary damage > primary (TOW)
   - Structures path unchanged (secondary damage ≥ primary)
4. Tests (not log-only):
   - `neutron_shell_residual_upgrade_and_blast`
   - `neutron_shell_residual_primary_does_not_blast`
   - `update_combat_prefers_secondary_damage_vs_infantry`
   - unit tests in `host_neutron_shell.rs` / `host_upgrades` classification

**Still residual (fail-closed, not claimed):**
- Full DumbProjectileBehavior bezier flight / min-range deploy matrix
- Full AffectAirborne / ally Relationship / contained-passenger kill matrix
- Full WeaponSet command-button toggle UI parity beyond `active_weapon_slot`
- Full INI PreferredAgainst / AutoChoose tables beyond damage+kind residual
- Network neutron replication (network deferred)

## Residual Host Playability — Cash Bounty on Kill (2026-07-12)
**Closed (host-testable kill with cash_bounty_percent → killer cash increases):**
1. **GLA SCIENCE_CashBounty residual** (`Player::doBountyForKill` + CashBountyPower):
   - Player holds `cash_bounty_percent` (default 0; tiers 5% / 10% / 20%).
   - `SCIENCE_CashBounty1/2/3` unlock (or `set_player_cash_bounty`) raises percent.
   - On enemy kill (`record_destruction` after combat): awards
     `ceil(victim_build_cost * cash_bounty_percent)` to killer supplies.
   - Skips under-construction, same-team, Neutral, and zero-percent cases.
2. Honesty (`host_cash_bounty.rs` + GameLogic):
   - `cash_bounty.bounty_kills` / `bounty_earned_total` / `max_bounty_percent`
   - `honesty_cash_bounty_ok` / `honesty_cash_bounty_award_ok` / `cash_bounty_earned_total`
3. Tests (not log-only):
   - `cash_bounty_increases_cash_on_enemy_kill`
   - `cash_bounty_zero_percent_does_not_award`
   - `cash_bounty_science_unlock_sets_percent`
   - unit tests in `host_cash_bounty.rs`
4. Engine parity: GameLogic `Player::do_bounty_for_kill` also `score_keeper.add_money_earned`.

**Still residual (fail-closed, not claimed):**
- Full CashBountyPower palace module + RequiredScience gate matrix
- Floating text / InGameUI AddCash over killer
- calcCostToBuild faction handicap matrix (uses template build_cost)
- Network bounty replication (network deferred)

## Residual Host Playability — Propaganda / Speaker Tower (2026-07-12)
**Closed (host-testable damaged ally near tower → HP recovers + ENTHUSIASTIC buff):**
1. **China Speaker Tower / PropagandaTower residual** (`PropagandaTowerBehavior`):
   - `GameLogic::update_propaganda_tower_pulse` each frame (from `update_simulation`).
   - Heals damaged **same-team non-structure** units within residual radius 150 at
     2% max-health/sec (4% with `Upgrade_ChinaSubliminalMessaging`).
   - Sets `weapon_bonus_enthusiastic` (base) / `weapon_bonus_subliminal` (upgrade);
     clears flags when unit leaves radius or tower is gone.
   - Name residual: `speakertower` / `propagandatower` / `listeningoutpost` /
     `tankemperor` / ends with `emperor`; excludes `propagandacenter`.
2. Honesty counters (`host_propaganda.rs` + GameLogic):
   - `propaganda_residual_heals` / `propaganda_residual_buffs`
   - `honesty_propaganda_heal_ok` / `honesty_propaganda_buff_ok` / `honesty_propaganda_ok`
3. Tests (not log-only):
   - `propaganda_tower_residual_recovers_hp_and_sets_enthusiastic`
   - `propaganda_tower_residual_out_of_range_then_in_range`
   - `propaganda_tower_residual_skips_enemy_units`
   - `propaganda_tower_residual_subliminal_upgrade_buff_and_faster_heal`
   - `propaganda_tower_name_residual_helix_propaganda_heals`
   - unit tests in `host_propaganda.rs`

**Still residual (fail-closed, not claimed):**
- Full sole-benefactor exclusivity / multi-tower reject matrix
- Full ally relationship filter (uses same-team residual)
- Full double-contain / stealthed FX suppress / POWERED underpower gate
- Full WeaponBonusConditionFlags ROF multiplier application in combat residual
- Full PulseFX / world-anim propaganda pulse
- Network propaganda replication (network deferred)

## Residual Host Playability — Ambulance / Infantry Heal (2026-07-12)
**Closed (host-testable damage → nearby ambulance / HealPad → infantry HP recovers):**
1. **USA Ambulance AutoHeal** (`AmericaVehicleMedic` residual):
   - `GameLogic::update_ambulance_auto_heal` each frame (from `update_simulation`).
   - Heals damaged **same-team infantry** within residual radius 100 at 4 HP/sec
     (C++ AutoHealBehavior ModuleTag_22: HealingAmount=4 / Delay=1000ms / KindOf=INFANTRY).
   - Name residual: template contains `ambulance` / `vehiclemedic` / ends with `medic`.
2. **HealPad command path** (`CommandType::GetHealed` → `AIState::SeekingHealing`):
   - Existing support-state dock heal now records `heal_residual_heal_pad_heals` honesty.
3. Honesty counters (`host_heal.rs` + GameLogic):
   - `heal_residual_ambulance_heals` / `heal_residual_heal_pad_heals`
   - `honesty_ambulance_heal_ok` / `honesty_heal_pad_ok` / `honesty_heal_ok`
4. Tests (not log-only):
   - `ambulance_auto_heal_residual_recovers_infantry_hp`
   - `ambulance_auto_heal_residual_out_of_range_then_in_range`
   - `ambulance_auto_heal_residual_skips_enemy_infantry`
   - `heal_pad_seeking_healing_residual_recovers_infantry_hp`
   - unit tests in `host_heal.rs`

**Still residual (fail-closed, not claimed):**
- Full sole-benefactor exclusivity / multi-ambulance reject
- Vehicle AutoHeal ModuleTag_23 (VEHICLE, ForbiddenKindOf=AIRCRAFT, SkipSelf)
- TransportContain HealthRegen%PerSec while embarked
- Particle / world-anim heal pulse FX
- Network heal replication (network deferred)

## Residual Host Playability — Structure / Vehicle Repair (2026-07-12)
**Closed (host-testable damage → Repair / GetRepaired → HP recovers):**
1. **Dozer structure repair** (`CommandType::Repair`):
   - `CommandExecutor::execute_repair` accepts dozer/worker on damaged ally/neutral structure
     (not under construction) → `AIState::Repairing` + destination.
   - `GameLogic::update_support_states` Repairing branch: approach within interact range (14),
     then heal structure HP over time (`HOST_REPAIR_RATE_HP_PER_SEC` flat residual).
   - `stop_moving` preserves Repairing on arrival; `update_combat` skips fire/chase while Repairing.
   - Covers WarFactory-as-structure (dozer repairs a damaged WarFactory).
2. **Vehicle pad / WarFactory repair** (`CommandType::GetRepaired`):
   - Damaged vehicles accept RepairPad **or WarFactory** (China `RepairDockUpdate` residual).
   - Aircraft accept Airfield.
   - `AIState::SeekingRepair` → approach → self-heal over time at same residual rate.
3. Honesty counters (`host_repair.rs` helpers + GameLogic):
   - `repair_residual_structure_commands` / `repair_residual_structure_heals`
   - `repair_residual_vehicle_heals`
   - `honesty_structure_repair_ok` / `honesty_vehicle_repair_ok` / `honesty_repair_ok`
4. Tests (not log-only):
   - `dozer_structure_repair_residual_recovers_hp_over_time`
   - `dozer_structure_repair_residual_walk_into_range_recovers_hp`
   - `war_factory_vehicle_repair_residual_recovers_hp`
   - existing `repairing_state_heals_target_in_range` / repair command suite
   - unit tests in `host_repair.rs`

**Still residual (fail-closed, not claimed):**
- Full C++ `RepairHealthPercentPerSecond` INI matrix / sole-benefactor healing reject
- Full `RepairDockUpdate` `TimeForFullHeal` dock bones / drone heal
- Bridge scaffolding gate during repair
- Multi-dozer task queue / `privateRepair` accept-same-bridge matrix
- Network repair replication (network deferred)

## Residual Host Playability — Mine / Demo Trap / Demo Charge (2026-07-12)
**Closed (host-testable place → enemy trigger damage / timed detonation):**
1. Host residual on Main `GameLogic` + `Object.mine_data` (`host_mines.rs`):
   - **Land mine** place (`place_land_mine` / ClusterMines special power ring)
   - **Demo trap** place (`place_demo_trap`, GLADemoTrap proximity residual)
   - **Timed demo charge** place (`place_timed_demo_charge`, TNTStickyBomb residual)
   - **Remote demo charge** place (`place_remote_demo_charge` / PlantRemoteDemoCharge)
     + **DetonateRemoteDemoCharges** command (no auto-timer; producer-scoped)
2. `update_mines_and_demo_traps` each frame:
   - proximity: enemy (not ally) in trigger range → area damage + destroy mine/trap
   - timed: absolute frame deadline → detonation
   - manual: `manual_detonate_mine` for demo-trap / remote-charge command residual
   - **dozer/worker clear**: Worker/Dozer within clear range (5) of enemy/neutral mine
     → `clear_mine_internal` (DAMAGE_DISARM residual) destroys mine with no splash;
     clearers never proximity-detonate; idle clearers approach within scan range (100)
3. `SpecialPowerType::ClusterMines` via `DoSpecialPower` places residual mine ring
   (not full OCL ClusterMinesBomb / GenerateMinefieldBehavior density).
4. Honesty counters: places / proximity / timed / manual detonations / clears;
   `honesty_mine_place_trigger_ok` / `honesty_timed_demo_charge_ok` / `honesty_mine_clear_ok`
   / `honesty_plant_remote_demo_charge_ok` / `honesty_remote_demo_charge_detonate_ok`.
5. Tests (not log-only):
   - `mine_residual_place_enemy_triggers_damage`
   - `mine_residual_ally_does_not_trigger_land_mine`
   - `demo_trap_residual_proximity_detonates_on_enemy`
   - `timed_demo_charge_residual_detonates_after_delay`
   - `cluster_mines_special_power_places_mines`
   - `demo_trap_manual_detonate_residual`
   - `plant_and_detonate_remote_demo_charge_residual`
   - `dozer_mine_clear_residual_disarms_enemy_mine_safely`
   - `dozer_mine_clear_residual_approaches_then_clears`
   - `dozer_mine_clear_residual_skips_ally_mine`
   - `dozer_mine_clear_residual_infantry_still_triggers`
   - unit tests in `host_mines.rs`

**Still residual (fail-closed, not claimed):**
- Full C++ MinefieldBehavior virtual-mine regen / scoot / multi-slot immunity
- Full DemoTrapUpdate weapon-slot mode matrix / PreAttack scoop animation
- Full WEAPONSET_MINE_CLEARING_DETAIL / Weapon AntiMine targeting matrix
- Full StickyBombUpdate attach bones / geometry-based secondary splash / max-charge list
- Full OCL ClusterMinesBomb aircraft delivery path
- Full StickyBombUpdate bone attach for BoobyTrap (host plant/detonate residual closed 2026-07-13)
- Network mine replication (network deferred)

## Residual Host Playability — W3D Mesh Asset Resolve (2026-07-12)
- Closed highest-value mesh residual after PresentationFrame unit identity:
  - `assets/mesh_asset_resolve.rs`: `model_key` / `get_model_name` → canonical W3D key
  - USA_Ranger / airanger → `airanger_s` (shipped `AIRanger_S.W3D`)
  - Load real mesh when AssetManager or filesystem extract/sample present
  - Placeholder cube + `MeshResolveHonesty` when missing (opt-in draw via debug cubes)
  - PresentationFrame freezes aliased model_key for unit mesh pass
- Fail-closed: not full W3D material / animation / GPU retail parity.
- Tests: non-empty USA_Ranger key; placeholder honesty; load when assets present else skip.

## Scope
- Non-network parity only (project policy).
- Multiplayer/network behavior remains deferred until non-network systems are complete.

## Current Assessment
- The Rust port is materially further along than the older snapshot in this file, but it is still not fully playable end-to-end.
- Recent parity work has closed several high-value slices in rendering, audio, terrain, and gameplay flow.
- Core build health is currently good for the main gameplay crates:
  - `cargo check -q -p gamelogic`
  - `cargo check -q -p game_engine`
  - `cargo check -q -p game-client-rust --features internal`
- The file parity tracker remains at 100% for existence/mapping, so the remaining work is now behavior parity, not file coverage.

## Residual Host Playability — Save/Load Secondary Weapon Xfer (2026-07-12)
**Closed (host-testable object secondary survives snapshot + file save/load):**
1. Gap: `SnapshotBuilder::snapshot_object` only stored `object.weapon` in
   `ObjectSnapshot.weapons[0]`; `restore_object` set primary only →
   `Object.secondary_weapon` always `None` after load (FlashBang/TOW/combat dual-slot
   desync). `active_weapon_slot` was also dropped.
2. Fix (fail-closed residual layout, not full C++ WeaponSet Xfer table):
   - Capture: `weapons[0]=primary`, `weapons[1]=secondary` when present
   - Secondary-only: zero-damage primary pad so secondary restores at index 1
   - `ObjectStatusSnapshot.active_weapon_slot` Xfer + capture/restore
3. Tests:
   - `snapshot_restore_preserves_secondary_weapon_and_active_slot`
   - `snapshot_restore_preserves_secondary_only_weapon_slot`
   - `snapshot_weapon_layout_helpers_round_trip`
   - `save_file_roundtrip_preserves_secondary_weapon` (SaveFileManager path)

**Still residual (fail-closed, not claimed):**
- Full C++ per-module WeaponSet / SpecialPowerModule / particle Xfer tables
- Network save/load (network deferred)

## Residual Host Playability — Special Power Strike Save/Load Xfer (2026-07-12)
**Closed (host-testable mid-flight strike survives snapshot + file save/load):**
1. Gap: `HostSpecialPowerStrikeRegistry` and `CombatParticleRegistry` lived only on
   live `GameLogic` — `WorldSnapshot` never captured pending strikes, so save
   mid-flight dropped the queue and impact never fired after load.
2. Fix (fail-closed residual, not full retail OCL / SpecialPowerModule Xfer):
   - `WorldSnapshot.special_power_strikes` stores `next_id` + all strike records
     (queued / completed / cancelled) including absolute `impact_frame`
   - `WorldSnapshot.combat_particles` stores host particle system entries
     (template + pose + spawn frame; not full W3D GPU state)
   - `SnapshotBuilder` capture/restore + Xfer markers after `GlobalAIState`
   - Registry `restore_from_snapshot` rebinds allocator + entries
3. Tests:
   - `special_power_daisy_cutter_mid_flight_save_load_still_impacts`
   - `special_power_a10_mid_flight_save_load_still_impacts`
   - `save_file_roundtrip_preserves_pending_special_power_strike`
   - registry unit restore tests in `special_power_strikes.rs` / `combat_particles.rs`

**Still residual (fail-closed, not claimed):**
- Full retail OCL aircraft / beam / multiplayer superweapon Xfer tables
- Client `ParticleSystemManager` GPU rebind after load (host registry only)
- Full C++ per-module SpecialPowerModule / particle Xfer tables
- Network save/load (network deferred)

## Residual Host Playability — Host Upgrade Research Save/Load Xfer (2026-07-12)
**Closed (host-testable mid-flight upgrade research survives snapshot + file save/load):**
1. Gap: `HostUpgradeRegistry` lived only on live `GameLogic` — `WorldSnapshot` never
   captured pending research records/honesty. Player `queued_upgrades` already survived
   via `PlayerSnapshot.research_queue`, but host residual queue honesty + entry
   bookkeeping (source object, queue_frame, pending ids) was dropped mid-flight.
2. Fix (fail-closed residual, not full retail Upgrade.ini BuildTime / ProductionUpdate):
   - `WorldSnapshot.host_upgrades` stores `next_id` + all research records
     (queued / completed / cancelled) including `queue_frame` / `complete_frame`
   - `SnapshotBuilder` capture/restore + Xfer marker after `CombatParticles`
   - `HostUpgradeRegistry::restore_from_snapshot` rebinds allocator, entries, and
     `pending_index` so mid-research completes on the next update with unlocks
3. Tests:
   - `host_upgrade_capture_mid_flight_save_load_completes_unlock`
   - `save_file_roundtrip_preserves_pending_host_upgrade`
   - `restore_from_snapshot_keeps_pending_queue` in `host_upgrades.rs`

**Still residual (fail-closed, not claimed):**
- Full retail Upgrade.ini BuildTime (30s) research timers / ProductionUpdate door UI
- Full science tree purchase / prerequisite graph / academy stats
- Full WeaponSetUpgrade / CommandSetUpgrade module matrices for every unit
- Object-type upgrades (`UPGRADE_TYPE_OBJECT`) vs player-type split beyond residual tags
- Network upgrade replication (network deferred)
- Economy effect of SupplyLines beyond tag observability

## Residual Host Playability — Upgrade Queue/Complete Host Path (2026-07-12)
**Closed (host-testable QueueUpgrade → complete → observable unlock):**
1. Host `HostUpgradeRegistry` on Main `GameLogic` records queue/complete for residual
   kinds: CaptureBuilding, FlashBangGrenade, TowMissile, SupplyLines (+ Other flag-only).
2. `CommandExecutor::execute_queue_upgrade` still deducts cost once per team and inserts
   into `Player.queued_upgrades`; now also records host residual queue honesty.
3. `GameLogic::update_player_upgrades` completes research into `unlocked_sciences` and
   applies observable unlocks:
   - **Capture**: player unlock flag gates `CaptureBuilding`; infantry receive upgrade tags
   - **FlashBang**: equips Ranger `secondary_weapon` (store `RangerFlashBangGrenadeWeapon`)
     + upgrade tag on team rangers missing secondary
   - **TOW**: equips Humvee secondary + tag
   - **SupplyLines**: tags supply centers
4. Local-player complete queues `EVA_UpgradeComplete` audio residual.
5. Honesty flags (registry API; do **not** claim full science/ProductionUpdate parity):
   - `honesty_queue_ok(kind)` / `honesty_complete_ok(kind)` / `honesty_host_path_ok(kind)`
   - `honesty_capture_unlock_ok` / `honesty_flashbang_equipped_ok`
6. Save/load: `WorldSnapshot.host_upgrades` persists in-flight research (see section above).
7. Tests (not log-only):
   - `capture_building_upgrade_queue_complete_unlocks_capture_ability`
   - `flashbang_upgrade_queue_complete_equips_ranger_secondary`
   - `supply_lines_upgrade_queue_complete_tags_supply_center`
   - `host_upgrade_capture_mid_flight_save_load_completes_unlock`
   - `save_file_roundtrip_preserves_pending_host_upgrade`
   - module unit tests in `host_upgrades.rs`
   - existing `queued_upgrade_completes_during_simulation_update` still holds

**Still residual (fail-closed, not claimed):**
- Full retail Upgrade.ini BuildTime (30s) research timers / ProductionUpdate door UI
- Full science tree purchase / prerequisite graph / academy stats
- Full WeaponSetUpgrade / CommandSetUpgrade module matrices for every unit
- Object-type upgrades (`UPGRADE_TYPE_OBJECT`) vs player-type split beyond residual tags
- Network upgrade replication (network deferred)
- Economy effect of SupplyLines beyond tag observability

## Residual Host Playability — CaptureBuilding Action (2026-07-12)
**Closed (host-testable unlock → CaptureBuilding → ownership transfer):**
1. Gate: `team_has_completed_capture_upgrade` / hero path; infantry without Capture
   research cannot enter `AIState::Capturing` when a player exists for the team.
2. Command: `CommandType::CaptureBuilding` → `CommandExecutor::execute_capture_building`
   sets target + destination + `AIState::Capturing` (does **not** flip ownership).
3. Complete: `GameLogic::update_support_states` Capturing branch — when in range of a
   live enemy structure (not under construction), cancels old production, `set_team`
   to captor team, heals to max, radar "Building captured" residual, captor → Idle.
4. Walk residual: out-of-range Capturing keeps destination; `Object::stop_moving` no
   longer clobbers interaction AI states on arrival (only `Moving`/`AttackMoving` → Idle).
5. Combat isolation: `update_combat` skips fire/chase while in Capturing (and other
   interaction states). Capture sets `target` without being an attack order — without
   this, default-weapon chase rewrote Capturing → Attacking mid-walk.
6. Instant residual when in range (fail-closed vs C++ SpecialAbilityUpdate prep timer /
   capture progress bar / defect flash).
7. Tests (not log-only):
   - `capture_building_upgrade_queue_complete_unlocks_capture_ability` (includes ownership)
   - `capture_building_walk_into_range_transfers_ownership_after_upgrade`
   - `capturing_state_transfers_building_when_in_range`
   - `infantry_capture_requires_completed_capture_upgrade_when_player_exists`
   - breadth scenario capture asserts team transfer after `update`

**Still residual (fail-closed, not claimed):**
- Full C++ capture progress bar / SpecialAbilityUpdate packing / prep duration
- Object defection flash / undetected-defector helper timing
- Network capture replication (network deferred)
- Full ActionManager canCaptureBuilding edge matrix (stealthed, garrison, etc.)

## Residual Host Playability — Black Lotus Specials (2026-07-13)
**Closed (host-testable Lotus Capture / StealCash / DisableVehicle residual):**
1. Template gate: `is_black_lotus_template` (ChinaInfantryBlackLotus / TestBlackLotus / general variants).
2. **CaptureBuilding** without infantry Capture research (hero + Lotus residual); StartAbilityRange **150**;
   reuses Capturing ownership-transfer residual + honesty `building_captures`.
3. **StealCashHack**: only Lotus; cash-generator targets only (supply / black market / drop zone /
   TestBuilding residual); amount **1000**; range **150**; honesty `cash_steals` / `cash_stolen_total`.
4. **DisableVehicleHack**: only Lotus; enemy manned ground vehicle → DISABLED_HACKED for **900** frames;
   range **150**; honesty `vehicle_disables`.
5. Fail-closed: non-Lotus units cannot issue StealCash / DisableVehicle; non-cash structures reject steal.
6. Tests (not log-only):
   - `black_lotus_capture_building_without_upgrade`
   - `steal_cash_hack_command_transfers_cash_after_reach`
   - `steal_cash_hack_rejects_non_lotus_and_non_cash_targets`
   - `disable_vehicle_hack_command_disables_after_reach`
   - `disable_vehicle_hack_rejects_non_lotus`
   - host_hero_abilities module unit tests

**Still residual (fail-closed, not claimed):**
- Full SpecialAbilityUpdate Unpack/Pack/Prep timers + laser FX interleave
- CashHack science upgrade money matrix (2000 / 4000)
- One-at-a-time Lotus special busy matrix
- Network special replication (network deferred)

## Residual Host Playability — Campaign SinglePlayer Path (2026-07-12)
**Closed (host-testable campaign residual):**
1. `golden_campaign` / `golden_campaign_gate` — SinglePlayer start, CampaignManager
   start/complete, mission `victory_rule` override (`nounits` via
   `victory_rules_for_map`), logic frames advance, mission script counter ticks
   without panic.
2. Real campaign map **script decode + install**: MD_USA01 `load_map` /
   `initialize_scripts` → 291 scripts decoded; dense lists installed under budget
   + heavy-utility skip (`mission_scripts_installed_count` honesty).
3. Sample mission seeds use retail map identities (`MD_USA01`, `GC_ChemGeneral`)
   instead of placeholder `usa_mission_01`.
4. Wired into `release_candidate` (`campaign_runtime_ok`) and `behavior_gate`.
5. **Full retail map load hang fixed (2026-07-12 residual):**
   - Root cause: `CALL_SUBROUTINE` held `ScriptEngine` global `RwLock` write while
     nested flag/timer/subroutine paths re-acquired the same lock (MD_USA01
     `SUB-Generate Random Number [1-2]` → deadlock). TLS active-engine re-entry
     (`with_script_engine_mut` / `with_script_engine_ref`) + dense-script budget
     + decorative AI skip on large worlds.
   - Object spawn was already fine (~0.6s for 2846 placements / ~2429 spawned).
6. **Default retail campaign map load (2026-07-13 residual):**
   - Default residual prefers MD_*/GC_* `load_map` when assets resolve
     (`retail_campaign_map_loaded=true`).
   - Opt-out: `GEN_CAMPAIGN_HOST_SAFE=1` or `GEN_CAMPAIGN_FULL_LOAD=0` → Lone Eagle.
   - `GEN_CAMPAIGN_FULL_LOAD=1` remains explicit force (legacy).
7. **Mission objectives residual (2026-07-13):**
   - Path-stem match (`map_name_matches_mission` / `find_mission_for_map`) so
     `.../MD_USA01.map` resolves CampaignManager mission metadata.
   - Residual primary objectives seeded; honesty:
     `objectives_loaded`, `objective_count`, `objectives_from_campaign`.
8. Honesty flags:
   - `campaign_playable_claim=true` — SP path advances with scripts/victory (not full
     retail mission playthrough)
   - `retail_campaign_map_loaded` — true by default when retail MD_*/GC_* load
     succeeds; false under host-safe opt-out or missing assets
   - `mission_scripts_installed_count` / `objectives_from_campaign` — install +
     objective residual honesty (not full cinematic / score-screen claim)

**Still residual (fail-closed, not claimed):**
- Dense campaign script evaluation is budgeted (24/frame when ≥48 scripts), not
  full same-frame C++ parity for all 291 scripts
- End-to-end mission objective completion / cinematic / score-screen campaign flow
- Full Campaign.ini INI parse into Main `CampaignManager` (seeded USA MD_USA01–05 +
  CHALLENGE_0 residual table closed 2026-07-13 — see DropDelay Stagger + MoneyCrateCollide
  + Campaign.ini Table section; GameClient manager already loads INI)
- Live retail mission playthrough with all script actions / EVA / camera chains

## Residual Host Playability — Special Power Superweapon Host Path (2026-07-12)
**Closed (host-testable DoSpecialPower → queue → impact complete path):**
1. Host `HostSpecialPowerStrikeRegistry` on Main `GameLogic` queues real strikes for
   DaisyCutter / FuelAirBomb, A10 (`Airstrike`), ScudStorm, and ParticleCannon.
2. `CommandExecutor::execute_special_power` still consumes charge + `AIState::SpecialAbility`,
   then enqueues residual strikes with retail-ish impact delay frames (90 / 60 / 150 / 120).
3. `GameLogic::update_special_power_strikes` (logic update phase) applies two-stage area
   damage to host objects, skips friendlies, marks kills for destroy list, and records
   completion stats (`objects_hit`, `total_damage_applied`).
4. Activation + impact queue `AudioEventRequest`s; impact also registers a
   `DeathExplosion` combat particle residual entry.
5. Honesty flags (registry API; do **not** claim full retail superweapon parity):
   - `honesty_queue_ok(kind)` — strike pending after command
   - `honesty_complete_ok(kind)` / `honesty_host_path_ok(kind)` — impact resolved
6. Tests (not log-only):
   - `daisy_cutter_host_path_queues_and_completes_area_damage`
   - `a10_strike_host_path_queues_and_completes`
   - `scud_storm_host_path_queues_and_completes`
   - `particle_cannon_host_path_queues_and_completes`
   - `radar_scan_does_not_queue_superweapon_strike`
   - module unit tests in `special_power_strikes.rs` (map, falloff, friendly exclusion)

**Still residual (fail-closed, not claimed):**
- Full retail OCL aircraft spawn / flight / bomber AI for DaisyCutter and A10
- Multi-missile SCUD barrage timing; Particle Uplink full uplink sequence /
  outer-node lasers / swath driving (continuous beam residual closed above)
- SharedSyncedTimer / science / public timer UI / EVA superweapon ready lines on host path
- Weapon.ini / SpecialPower.ini damage tables beyond residual constants
- Network superweapon replication (network deferred)
- Non-superweapon special abilities beyond existing PendingSpecialAbility (hijack/etc.)
- *(Pending-strike save/load residual closed — see Special Power Strike Save/Load Xfer)*

## Residual Host Playability — Combat Particle Feedback (2026-07-12)
**Closed (host-testable kill/fire → particle registry observe path):**
1. Host `CombatParticleRegistry` on Main `GameLogic` registers real particle-system
   entries (stable id + template + position) on weapon fire and combat death.
2. Death path (`process_destroy_list`) spawns `MediumExplosion` + `SmokePlume`; fire path
   spawns `MuzzleFlash` + optional `BulletImpact`.
3. With `game_client`, entries mirror into `ParticleSystemManager` via combat presets
   (`create_preset_system_xyz`) so client registry is non-empty after kill/fire.
4. `PresentationFrame` freezes `particle_systems` and emits
   `ParticleSystemSpawned` / `ObjectDestroyed` events for client/HUD observation.
5. Tests (not log-only):
   - `combat_kill_spawns_particle_system_registry_entries`
   - `combat_fire_without_kill_still_spawns_muzzle_particle`
   - `presentation_frame_observes_combat_kill_particle_systems`
   - `create_preset_system_at_registers_combat_death_and_muzzle_entries`

**Still residual (fail-closed, not claimed):**
- Full W3D GPU particle render/compute parity (Main GPU ParticleSystemManager path)
- Full ParticleSystems.ini / FXList.ini retail coverage for every combat FX
- Bone-attached / slave systems / LOD culling for combat residual path
- Client GPU rebind of mirrored systems after save/load (host registry is restored)
- Network particle replication (network deferred)
- *(Host registry systems now survive WorldSnapshot — see Special Power Strike Save/Load Xfer)*

## Residual Host Playability — Combat March Honesty (2026-07-12)
- Map-world golden skirmish prefers pure `assign_unit_path` / Move march into weapon range,
  then `AttackObject`. Narrow `set_position` range pull remains only after per-focus stall.
- Honesty flags (do **not** gate `playable_claim` when map victory still works):
  - `combat_no_teleport_ok` — damage/kills without any combat `set_position` pull
  - `combat_realistic_speed_ok` — march speed ≤ retail BasicHumanLocomotor (20 u/s)
  - `combat_store_damage_ok` — no slice damage floor; WeaponStore/template damage (ranger ~5)

### Pathfinding / pure-march closure (2026-07-12)
**Root causes fixed:**
1. `GameLogic::update_movement` now **persists velocity** (was re-accelerating from 0 every
   frame → units crawled ~0.5 u/s). Also uses horizontal XZ distance for waypoints so
   terrain height does not false-stall path advance.
2. `assign_unit_path` kicks full speed toward goal; rejects absurd A* detours for direct march;
   path waypoints lerp Y start→goal.
3. Path grid: flatter slope mask (MAX_SLOPE 4.0) + auto-clear if >35% blocked; A* closed set;
   nearest-open goal when building footprints block the cell.
4. Golden fight: stable focus (no HashMap thrash), structure-prefer targeting, distance-scaled
   march budgets for 3.5k maps, AI paused during clear; longer windows + more rangers replace
   the old 80 u/s / damage-floor-40 assists.
5. GLA/faction structures marked `Attackable` so combat targeting is consistent.

**Lone Eagle gate (target / validated when green):**
`playable_claim=true`, `retail_prod=true`, `retail_gather=true`, **`combat_no_teleport_ok=true`**,
`combat_realistic_speed_ok=true`, `combat_store_damage_ok=true`, `victory=true`, `map_combat=true`.

**Residual (honest, non-blocking):**
- Host `locomotor_bootstrap` binds SET_NORMAL speeds at create_object (BasicHumanLocomotor ~20 u/s
  for USA_Ranger / GLA infantry; Humvee 60; tanks 30; Redguard 25) from Locomotor.ini on disk or
  seed. Golden slice lift is residual only when catalog bind is missing (still ≤ 20 →
  `combat_realistic_speed_ok`).
- Fail-closed: not full multi-locomotor set / surface-type matrix / SET_PANIC upgrades —
  single primary SET_NORMAL name → max_speed/accel/turn only.
- Reintroduce SLICE_MARCH_SPEED > 20 or SLICE_DAMAGE_FLOOR > 0 only if pure-march budgets fail;
  that would clear the matching honesty flag without flipping claim off.
- Full Weapon.ini / multi-locomotor parity and SAGE passability remain open for real-time play.
- `set_position` stall fallback code remains for pathological maps; currently unused on Lone Eagle
  when pure march succeeds.
- Network still deferred.

## Recent Validated Closures (2026-04-02)
- Terrain bridge runtime parity:
  - `hq-p58f` closed.
  - `TerrainLogic::addBridgeToLogic()` / `addLandmarkBridgeToLogic()` now register bridges with the pathfinder and assign the returned bridge layer.
  - `deleteBridge()` / `deleteBridgeAt()` now disable bridge state in the pathfinder and destroy the bridge object when present.
  - Validation: `cargo check -q -p gamelogic` plus targeted bridge tests passed.
- W3D renderer batch submission parity:
  - `hq-xo41` closed.
  - `W3DRenderer` now binds real mesh/material snapshots, vertex/index buffers, and indexed/non-indexed draws instead of the placeholder fullscreen triangle path.
  - Validation: `cargo check -q -p game_engine_device`, `cargo check -q -p game_engine`, and `cargo check -q -p game-client-rust --features internal` passed.
- Audio view-resolution parity:
  - `hq-dur9` closed.
  - `AudioViewResolver` is already wired through GameClient init and GameLogic resolver registration, and the new regression test proves it is consumed by `GameAudio::update()`.
  - Validation: `cargo test -q -p game_engine --features internal --test audio_view_resolver_tests -- --exact --test-threads=1` passed.
- Audio suspend/resume parity:
  - `hq-dxxk` closed.
  - Script suspend/resume now pauses and resumes active handles instead of only toggling audio enable state.
- Host combat audio residual (fire/death request path):
  - Weapon fire queues `AudioEventRequest("WeaponFire")`; death queues `UnitDie`/`BuildingDie`.
  - FXList SoundFX falls back to gameplay dispatch when the client audio hook is absent.
  - Validation: `cargo test -q -p generals_main --lib combat_fire_queues` and `combat_kill_queues` passed; FXList fallback: `cargo test -q -p game-client-rust --features internal --lib sound_fx_nugget`
- Shell/flow parity:
  - `hq-dwel`, `hq-c8a0`, `hq-b53m`, and `hq-afm` are closed.
  - That means map-select legacy keying, underlying-options visibility, shell scheme ordering, and command translator context routing are all improved on the gameplay-flow side.
- Terrain parity slices already closed earlier in this session remain validated:
  - dynamic water dedupe and authoritative z updates
  - wall-hit source parity
  - identity-stable water-handle mapping follow-up work

## Current Subsystem Status
- Rendering: improved, but not complete. Real mesh/material submission is now in place, yet the deeper W3D/material/state parity backlog still exists.
- Terrain: improved materially. Bridge runtime and water-related slices are closer to C++ behavior, but the broader terrain visuals/water/snow work remains open.
- Audio: improved materially. Core resolver, suspend/resume, and host combat fire/death audio requests are in place; broader Miles device/handle and per-weapon INI FireSound residual remain open.
- UI/flow: better than before, especially shell and map-select behavior, but bootstrap/menu/game-start parity still has open work.
- AI: still a major blocker. The system remains far from the breadth of the C++ implementation.
- Drawable: still a major blocker. Icon management, model conditions, shadows, and animation-state parity are incomplete.
- Particles: still a major blocker. The system remains heavily placeholder-based.
- Save/load: improved in multiple slices, but still not a clean full parity story for all gameplay objects.
- Memory: still a structural concern because the Rust allocation model intentionally diverges from the C++ pool allocator behavior.

## What Is Still Blocking Playability
1. `hq-eo4` and related startup/bootstrap flow work:
   - Common startup still needs to consistently drive the real GameClient bootstrap path under the default game flow.
2. `hq-81ok` AI:
   - Skirmish AI is still the largest single gameplay blocker.
3. `hq-fos8` Drawable:
   - Missing visual state, animation, shadow, and icon parity still prevents full visual playability.
4. `hq-gq7n` Particles:
   - Explosions, smoke, fire, and other feedback effects are still incomplete.
5. `hq-zhvn` GameMemory:
   - The memory model still diverges from the C++ pool allocator semantics.
6. `hq-7zxm` Audio:
   - Core audio is improving, but the broader system still has placeholder and routing gaps.
7. `hq-aqxu` Terrain visuals:
   - Terrain rendering, water animation, snow, deformation, and pathfinding integration remain incomplete.

## Recommended Next Order
1. Finish bootstrap/startup parity (`hq-eo4`) so the real client path is the default.
2. Push AI parity (`hq-81ok`) enough to make skirmish gameplay viable.
3. Continue Drawable parity (`hq-fos8`) because it gates visible gameplay feedback.
4. Fill the particle system gaps (`hq-gq7n`) after Drawable is stable.
5. Reduce the memory-model divergence (`hq-zhvn`) to lower risk across save/load and runtime behavior.
6. Continue terrain/audio edge cases in `hq-aqxu` and `hq-7zxm`.

## Earlier Validated Closures
- Shared W3D gadget text/clip parity improved (2026-03-12):
  - `GameClient/src/gui/w3d_gadget_draw.rs` now matches more of the original shared gadget behavior instead of carrying Rust-specific text state across draws.
  - `W3DGadgetStaticText*` now applies hotkey highlighting when `WIN_STATUS_HOTKEY_TEXT` is set, clips to the full window rect during draw, and clears the clip state afterward.
  - text-entry and list-box text paths now clear their temporary `DisplayString` clip regions after each draw instead of leaking clipping into later controls.
  - checkbox image draw now honors `image_offset.y`, matching the C++ checkbox image path.
  - image-backed radio buttons now use the C++-style left/capped/tiled-center strip behavior instead of drawing only a single image across the whole control.
  - image-backed list-box hilite bars now clip the repeated center/tail pieces instead of stretching the tail segment into the remaining gap.
- Main-menu button drop-shadow text parity improved (2026-03-12):
  - `GameClient/src/gui/w3d_gadget_draw.rs::w3d_main_menu_button_drop_shadow_draw(...)` no longer routes through the generic button-text helper.
  - Rust now uses a dedicated main-menu button text path with the C++-style centered label layout and visible drop-shadow offset used by `W3DMainMenuButtonDropShadowDraw`.
  - This matters for the runtime-overridden shell buttons installed by `W3DMainMenuInit`, where generic Rust text rendering was still flatter than the original menu presentation.
- Main-menu random text draw parity improved (2026-03-12):
  - `GameClient/src/gui/w3d_gadget_draw.rs::w3d_main_menu_random_text_draw(...)` now uses the dedicated C++-style path instead of the generic static-text helper.
  - The Rust draw now left-aligns at the window origin, vertically centers the text, and applies the same clipped disabled-text rendering behavior as the original `W3DMainMenuRandomTextDraw`.
  - This matters for main-menu runtime labels in data sets that still include the random-text windows C++ configures during `W3DMainMenuInit`.
- Main-menu runtime draw override parity improved (2026-03-12):
  - `GameClient/src/gui/window_manager.rs` now applies the extra `W3DMainMenuInit` runtime draw-callback overrides that C++ installs after loading `MainMenu.wnd`.
  - This restores `W3DMainMenuButtonDropShadowDraw` on the main shell buttons and `W3DMainMenuRandomTextDraw` on the optional random-text labels when present.
  - The `.wnd` file alone leaves those windows on generic `[None]` draw callbacks, so without the runtime override Rust was missing part of the authored main-menu presentation even when assets and transitions were otherwise correct.
- Local filesystem lookup parity improved (2026-03-11):
  - `Common/src/common/system/local_file_system.rs` now resolves file paths case-insensitively across direct paths and configured search roots.
  - directory enumeration through the same backend now also resolves search directories case-insensitively before scanning them.
  - This better matches the effective C++ asset behavior on shipped data, where many extracted files are lowercased on disk while gameplay/UI code still requests mixed-case legacy names.
  - The fix is broader than shell art and should reduce repeated missing-asset drift across UI, INI, and other local filesystem-backed lookups.
- Shell startup asset lookup parity improved (2026-03-11):
  - `GameClient/src/display/image.rs` now resolves local fallback texture paths case-insensitively before opening them.
  - This closes a real repo/platform gap where extracted assets are often lowercased on disk, while mapped-image filenames preserve original mixed case from the C++ data set.
  - That matters directly for shell art such as `MainMenuRuleruserinterface.tga` and other extracted texture lookups on case-sensitive filesystems.
- Main-menu ruler visibility parity improved (2026-03-11):
  - `GameClient/src/gui/shell/main_menu.rs::sync_cpp_startup_visibility(...)` no longer re-hides `MainMenu.wnd:MainMenuRuler` every update based on `not_shown`.
  - The C++ code decides ruler visibility during `MainMenuInit()` and does not override it in the startup visibility sync.
  - This removes one more Rust-only suppression of the intro/main-menu composition, especially after returning to the main menu within the same process.
- Shell generic image draw parity re-closed (2026-03-11):
  - `GameClient/src/gui/game_window.rs::default_draw_callback(...)` now renders `WIN_STATUS_IMAGE` windows with neutral color instead of multiplying the mapped image by the `.wnd` draw color.
  - This is the exact C++ `W3DGameWinDefaultDraw` behavior for generic image-backed windows and matters directly for `MainMenu.wnd:MainMenuParent`, `MainMenu.wnd:MainMenuRuler`, and `MainMenu.wnd:Logo`.
  - The prior Rust tint path could still darken valid menu art into blue/black even when texture resolution succeeded.
- Shell default image-window parity improved:
  - `GameClient/src/gui/game_window.rs::default_draw_callback(...)` now mirrors C++ `W3DGameWinDefaultDraw` semantics for `WIN_STATUS_IMAGE` windows.
  - Image-backed windows now draw only the mapped image with neutral color in the default path; Rust no longer fills the configured color first, multiplies the image by that color, and adds a border on top.
  - This specifically removes a real shell/menu drift affecting `MainMenu.wnd:MainMenuParent`, `MainMenu.wnd:Logo`, and `MainMenu.wnd:MainMenuRuler`, where valid art could still appear darkened or blackened because Rust treated `COLOR` as image tint in the image path.
- Shell legacy window draw parity improved:
  - `GameClient/src/gui/window_manager.rs` now assigns the correct default draw path for windows with empty / `[None]` draw callbacks.
  - Generic `USER` windows no longer fall through to push-button draw semantics merely because they carry image draw data.
  - Fresh rebuilt shell runs now show:
    - `GeneralsLogo` hydrating and GPU-loading from `SCSmShellUserInterface512_001.tga`
    - `MainMenuRuler` hydrating and GPU-loading from `MainMenuRuleruserinterface.tga`
  - This directly closes a live shell/menu regression where those windows existed in the runtime tree but were not painting through the same generic window path as C++.
- Shell callback image lookup parity improved:
  - `GameClient/src/gui/game_window_global.rs::win_find_image(...)` now lazily hydrates mapped images from the common collection before callback-driven lookup.
  - This aligns callback-driven shell draw paths more closely with C++ and removes another dependency on fragile startup bulk-sync timing.
- Shell backdrop asset status is now proven, not guessed:
  - direct BIG-TOC inspection confirms the currently mounted asset set contains:
    - `TitleScreenuserinterface.tga`
    - `MainMenuRuleruserinterface.tga`
    - `SCShellUserInterface512_001.tga`
    - `SCSmShellUserInterface512_001.tga`
  - and does **not** contain `MainMenuBackdropuserinterface.tga`
  - so the current `MainMenuBackdrop -> TitleScreenuserinterface.tga` compatibility fallback is covering a real asset absence in this repo data set rather than a broken Rust archive lookup.
- Startup event-loop parity improved:
  - `Main/src/main.rs`, `.../win_main.rs`, and `.../cnc_game_engine.rs` now create the real `winit` window inside the active event loop/resume path instead of before the handler exists.
  - Fresh rebuilt shell runs no longer emit the early macOS `winit` startup errors `tried to run event handler, but no handler was set`.
- Shell ambient/scorch marker parity improved:
  - `Main/src/game_logic/game_logic.rs` now applies the original C++ illegal map-template skip list before template/fallback synthesis.
  - Fresh rebuilt shell runs no longer emit `Skipping unsupported decorative map object template 'Amb_*'` or `'Scorch'`.
- Animated-sound metadata parity improved:
  - `Common/src/common/game_engine.rs` now resolves `w3danimsound.ini` through the mounted virtual file system first and only falls back to direct path/archive probing when necessary.
  - `ww3d-animation/src/animated_sound.rs` now matches the original C++ default name semantics (`w3danimsound.ini`) instead of hardcoding `Data/INI/...`.
  - animated sound metadata can now initialize directly from mounted INI bytes instead of writing a temporary compatibility file just to read it back.
- Shell callback/object-definition parity improved:
  - `Main/src/assets/ww3d_asset_manager.rs` now includes stock top-level `Data/INI/Crate.ini` in WW3D object-definition discovery, so shell/runtime crate objects like `2FreeCrusadersCrate` resolve from real stock INI definitions instead of synthetic fallback templates.
  - `GameClient/src/gui/window_manager.rs` now wires `W3DShellMenuSchemeDraw` and `W3DClockDraw`.
  - `GameClient/src/gui/w3d_gadget_draw.rs` now implements both draw callbacks instead of falling back to generic/default draw.
  - `GameClient/src/gui/shell/base.rs` shell menu scheme draw now renders its configured lines/images through the live window manager instead of being a trace-only stub.
- Shell startup/menu presentation parity improved:
  - `GameClient/src/gui/game_window.rs` and `.../w3d_gadget_draw.rs` no longer draw a fallback solid color when a mapped image exists but its GPU texture upload fails.
  - This removes a real Rust-only behavior drift that could black-fill `MainMenu.wnd:MainMenuParent` when `MainMenuBackdrop` failed to materialize.
- Shell startup prewarm no longer blocks first menu frame:
  - `Main/src/cnc_game_engine.rs` now queues nearby shell-scene models for incremental post-startup prewarm instead of blocking startup on synchronous shell-scene model loads.
  - Fresh bounded startup runs now reach the legacy shell overlay before shell-scene prewarm work resumes.
- Common/BIG shell asset diagnostics improved:
  - `Common/src/common/system/big_file_system.rs` now exposes mounted virtual paths for parity/debug tests.
  - `Common/tests/mapped_image_parity_tests.rs` now verifies mounted shell image candidates and mapped-image metadata against real repo assets.
- Shell startup legacy-runtime deadlock resolved:
  - `Main/src/game_logic/game_logic.rs` now releases `PlayerTemplateStore` read-locks before `player.init(...)`, allowing lazy template hydration without lock inversion.
- Player template parity/availability improved:
  - `Common/src/common/ini/mod.rs` now supports robust `PlayerTemplate.ini` discovery across base, extracted, and mod roots.
  - `GameLogic/src/player.rs` now lazily ensures template-store population during runtime hydration.
  - Shell startup no longer emits `PlayerTemplate '...' not found in store` warnings in the active validated run.
- Script/object shell parity improved:
  - `GameLogic/src/scripting/executor.rs` team/named follow-waypoint actions now resolve path starts by closest waypoint-to-position (C++ shape) rather than strict name lookup.
  - `Main/src/game_logic/script_loader.rs` + `.../game_logic.rs` now mirror parsed named object placements into legacy runtime tracking for script resolution.
- Factory/bootstrap parity improved:
  - `GameLogic/src/helpers.rs` now retries `ThingFactory` init only when uninitialized, eliminating repeated heavy re-inits on normal misses.
  - Common ThingFactory now loads object declarations from base/extracted/mod INI roots.
  - ModuleFactory install paths now perform on-demand initialization retries for behavior/draw/client-update modules.
- Current shell startup reachability (validated):
  - `Fast legacy runtime sync complete ... elapsed=0.02s`
  - `Mission script runtime registered 82 WW3D scripts`
  - `shell_initialized active=true screens=1`
  - `menu_overlay_paint_jobs=5`
  - `MainMenu.wnd:MainMenuParent` and child shell windows are present in the legacy runtime overlay queue.

## Current Highest-Value Todo
- `GameClient/src/display/image.rs` + shell mapped-image path
  - close the remaining `MainMenuBackdrop` materialization gap. The mapped image exists and resolves to `MainMenuBackdropuserinterface.tga`, but the actual GPU texture payload is still missing in active mounted content and needs the same mounted-file/BIG-backed behavior the original engine expects.
- `GameEngineDevice/src/w3d/w3d_c_api.rs`
  - finish the remaining true multi-texture / texture-dependent fixed-function combiner parity beyond the current constrained fallback evaluator, now that lighting-disable and material-source state are wired through the active material/shader path.
- `GameEngineDevice/src/w3d/performance_optimizer.rs`
  - close the remaining exact heuristic threshold/tuning parity against the original C++ runtime policy now that optimizer batching preserves renderer-specialized priority/material state instead of collapsing it.
- `GameEngineDevice/src/w3d/renderer.rs`
  - close the remaining deeper render-pass/material-state specialization parity beyond the current default/unlit path and optimizer queue-state preservation.
- `GameClient/src/video_player.rs` + device-side movie backend
  - wire a real active stream provider/decoder backend behind the common player hook.
- `Common/src/common/audio/*.rs` + `GameLogic/src/helpers.rs`
  - close broader audio routing side-effect parity and malformed/streaming media edge behavior.

## What Is Good
- Core gameplay/runtime crates compile:
  - `cargo check -p game_engine`
  - `cargo check -p gamelogic`
  - `cargo check -p game-client-rust`
  - `cargo check -p game_engine_device`
- Video-device parity lane is build-clean:
  - `cargo check -p game_engine_device --features video`
- W3D-device parity lane is build-clean:
  - `cargo check -p game_engine_device --features w3d`
- W3D texture mip generation parity improved:
  - `Code/GameEngine/GameEngineDevice/src/w3d/texture_manager.rs` now generates/uploads real mip chains via iterative RGBA downsample instead of leaving mip levels as a simplified placeholder path.
- Shadow renderer caster accounting parity improved:
  - `ww3d-renderer-3d/rendering/shadow_system/shadow_renderer.rs` now uses registered `ShadowCasterSubmission` lists as the primary per-pass caster accounting source.
  - `shadow_caster_count_hint` is now fallback-only when no runtime submissions are registered.
  - Added per-light submission integration path (`render_shadows_with_submissions(...)`) so directional/point/spot passes can consume explicit per-light caster sets directly.
  - Added persistent per-light submission registry in `ShadowRenderer`; default `render_shadows(...)` now consumes per-light registered submissions without requiring external map passing each frame.
- Renderer frame-graph shadow submission bridge improved:
  - `ww3d-renderer-3d/src/lib.rs` now queues eligible meshes into the frame-graph shadow-caster queue and materializes runtime `ShadowCasterSubmission` records each frame from prepared shadow-caster meshes.
  - Added runtime accessors (`shadow_caster_submissions`, `take_pending_shadow_caster_submissions`) for direct handoff into shadow passes.
- WGPU main-renderer shadow handoff improved:
  - `ww3d-renderer-3d/rendering/wgpu_main_renderer.rs` now auto-drains renderer-emitted shadow submissions during `end_frame(...)` (engine and legacy paths) instead of requiring external polling-only glue.
  - Per-frame submission state is now exposed directly from `WgpuMainRenderer` through `shadow_caster_submissions()` and `shadow_caster_count_hint()`.
- Main forward-pass transparent routing improved:
  - `Main/src/graphics/render_pipeline.rs` now classifies render items by material blend/opacity into `ForwardOpaque` vs `ForwardTransparent` instead of hard-forcing all items to opaque.
  - Transparent items are now sorted back-to-front and included in `ForwardPass::render(...)` queue submission (previously only opaque items were submitted).
- Main effects shadow callback integration improved:
  - `Main/src/effects/lighting_system.rs` now provides `render_shadow_maps_with_context(...)` (per-light/per-layer callback context) and upgrades `render_shadow_maps(...)` to `FnMut` callback flow.
  - `Main/src/effects/integration.rs` now exposes `render_with_shadow_scene(...)`, so the active effects pipeline can accept real shadow-caster draw callbacks instead of embedding a fixed no-op-only callback shape.
  - `Main/src/effects/integration.rs` now also exposes `render_with_shadow_scene_context(...)` for direct per-light/per-layer callback wiring at effects call-sites.
  - Shadow-layer assignment in dynamic lighting now uses deterministic light-id ordering instead of direct `HashMap` iteration order, improving frame-to-frame stability for multi-light shadow array usage.
  - Shadow-pass callback now owns pipeline/bind-state setup (internal pre-bind removed), enabling callback-driven scene draws to use correct mesh-compatible render state.
- Main render-item ordering determinism improved:
  - `Main/src/graphics/render_pipeline.rs` now sorts object-id iteration during item collection and includes explicit comparator tie-breakers (`object_id`, `model_name`, `mesh_index`) after pass/material/distance keys.
  - This removes remaining equal-key ordering drift from hash-map insertion order.
- GameLogic test-build lane is unblocked again:
  - `GameLogic/src/common/types.rs` test module now imports `CoordOrigin`, restoring `Coord3D::origin()` resolution in lib-test compilation.
  - `cargo test -p gamelogic --no-run` now completes successfully.
- Dynamic-lighting shader shadow path improved:
  - `Code/Main/src/shaders/dynamic_lighting.wgsl` now performs cascade-aware depth-array shadow sampling with PCF in `sample_shadow_map(...)` instead of fixed full-light placeholder behavior.
  - Directional-light path now uses sampled shadow factor, reducing obvious sun-shadow parity drift.
- Shadow map CPU query parity improved:
  - `ww3d-renderer-3d/rendering/shadow_system/shadow_map.rs::is_point_in_shadow(...)` no longer returns a fixed constant.
  - CPU fallback now applies bias-aware/filter-aware visibility approximation, reducing hardcoded shadow-value drift.
- W3D optimizer runtime policy lane is now active instead of inert:
  - `Code/GameEngine/GameEngineDevice/src/w3d/performance_optimizer.rs` now records frame-history/memory telemetry and applies periodic auto-quality adjustment (FPS + memory pressure) to LOD bias, culling distance, and batching aggressiveness.
  - LOD runtime metrics (`average_lod_level`, `lod_transitions`) are now populated from active camera/object distances.
  - Instancing/batching now preserves transparent-vs-opaque separation (`RenderBatch.transparent`) instead of forcing opaque output.
- W3D renderer transparent queue parity improved:
  - `Code/GameEngine/GameEngineDevice/src/w3d/renderer.rs` now routes transparent materials into `transparent_queue` (instead of always `opaque_queue`) and submits both queues each frame.
  - Transparent queue sorting now uses back-to-front camera-distance ordering with per-batch distance computed from mesh bounds and active camera.
- W3D renderer object-transform submission parity improved:
  - `Code/GameEngine/GameEngineDevice/src/w3d/w3d_device.rs::render_scene(...)` now forwards per-object world transforms into renderer submission instead of effectively forcing identity transforms.
  - `Code/GameEngine/GameEngineDevice/src/w3d/renderer.rs` now uses model-matrix inverse-transpose normal matrices in submitted instance data for this path.
  - Direct scene-render submissions now also honor `RenderObject.visible` and explicit `RenderObject.transparent` override state.
- W3D init entrypoint parity is improved:
  - `GameEngineDevice::init_w3d_device(...)` / `init_video_device(...)` now call device `.init()`,
  - `W3DDevice::init_with_window(&Window)` now configures a real window-bound surface.
- Runtime audio device enumeration now uses driver-backed capabilities:
  - `GameEngineDevice::audio::DeviceManager::enumerate_devices()` no longer returns mock fixed entries.
- macOS platform CPU usage reporting parity improved:
  - `GameEngineDevice/src/platform/macos_device.rs::get_cpu_usage()` now reports live process CPU usage (via `ps`) instead of fixed zero, mapped to engine metric range.
- Miles audio capability reporting now derives from runtime backend data:
  - `MilesAudioDevice` capability snapshots are now mapped from active Kira backend capabilities in `audio` builds.
- Basic audio playback lifecycle parity improved:
  - `Common/src/common/basic_audio_manager.rs` now tracks non-looping sound lifetime by elapsed playback time vs estimated duration (WAV parsed duration when available + bitrate fallback), replacing prior handle-id retirement behavior.
  - Master-volume updates now recompute from source base volumes, removing compounding volume drift across repeated master-volume changes.
- Radius-decal icon visibility/throb parity improved:
  - `GameLogic/src/common/types.rs` `RadiusDecal::update()` now follows C++ icon UI semantics by forcing decal opacity to zero when draw-icon UI is disabled.
  - Throb interpolation now normalizes min/max opacity ordering and treats zero throb-time as stable max-opacity output.
- Duration parsing parity improved in active decal/camera-delivery lanes:
  - `NeutronMissileUpdate`, `SpectreGunshipUpdate`, `DynamicShroudClearingRangeUpdate`, `WeaponBonusUpdate`, `StructureToppleUpdate`, `StructureCollapseUpdate`, and `HordeUpdate` now use canonical `INI::parse_duration_unsigned_int(...)` for frame-duration fields instead of ad-hoc conversion logic.
- Additional C++-mapped duration parser parity closures landed in active gameplay modules:
  - `CountermeasuresBehavior`, `BattleBusSlowDeathBehavior`, `W3DLaserDraw`, `BeaconClientUpdate`, and `SpecialPowerTemplate` duration fields now use canonical `INI::parse_duration_unsigned_int(...)`.
  - `WaveGuideUpdate::WaveDelay` now uses canonical `INI::parse_duration_real(...)`.
  - This removes remaining ad-hoc `ms/s` parsing drift in these paths and aligns with C++ `parseDurationUnsignedInt` / `parseDurationReal` field mappings.
- Additional duration parser parity closures landed across behavior/update/contain paths:
  - `AutoDepositUpdate`, `AutoFindHealingUpdate`, `EnemyNearUpdate`, `CommandButtonHuntUpdate`, and `DumbProjectileBehavior` duration fields now use canonical `INI::parse_duration_unsigned_int(...)`.
  - `RebuildHoleBehavior` and shared contain duration helper paths now use canonical `INI::parse_duration_real(...)` (with truncation-faithful frame-counter handoff for respawn wait semantics).
  - This removes another set of legacy ad-hoc conversions and aligns these modules with C++ `parseDurationUnsignedInt` / `parseDurationReal` mapping behavior.
- Additional legacy behavior/contain duration parser closures landed:
  - `SlowDeathBehavior`, `SpawnBehavior`, and `OpenContain::DoorOpenTime` now use canonical `INI::parse_duration_unsigned_int(...)` in place of manual digit-only conversion paths.
  - This removes suffix-handling drift in these lanes and aligns with C++ `parseDurationUnsignedInt` field mappings.
- Additional duration-real parser closures landed in AI/dock lanes:
  - `WorkerAIUpdate::BoredTime`, `DozerAIUpdate::BoredTime`, and `RepairDockUpdate::TimeForFullHeal` now use canonical `INI::parse_duration_real(...)`.
  - This removes real-duration suffix drift in these paths and aligns with C++ `parseDurationReal` mappings.
- Main gameplay shell/template parity improved:
  - `isInShellGame()` now reflects real in-game state,
  - team template availability now avoids obvious cross-faction leakage for faction-tagged template names.
- Main gameplay fallback AI parity improved:
  - `process_ai_behavior(...)` fallback branches now use `AIDecisionSystem` enemy scan/decision flow (attack/retreat/retarget) instead of explicit placeholder/random behavior.
  - Patrol fallback destination changes now use deterministic frame/object-id keyed movement for better replay consistency.
- Game initialization script parity improved:
  - `Code/GameEngine/GameLogic/src/system/game_initialization.rs` now synchronizes `SidesList` script lists into `ScriptEngine` during startup, so side scripts loaded from map/side data are actually registered before runtime script updates.
  - Startup script extraction from world dict now supports multiple key variants and list-form values instead of forcing a synthetic default script filename.
- Startup sequence script behavior parity improved:
  - `Code/GameEngine/GameLogic/src/system/game_start.rs::run_startup_scripts(...)` now activates matching runtime scripts through `ScriptEngine` instead of no-op success.
- Single-instance runtime behavior parity improved:
  - `create_generals_mutex()` now retains the acquired `SingleInstanceGuard` for process lifetime via a global guard slot, instead of dropping it immediately after acquisition.
  - Unix process liveness checks now treat `kill(pid, 0)` permission-denied (`EPERM`) as alive, reducing stale-lock false cleanup when a running process cannot be signaled.
- FOW/shroud visibility query parity improved:
  - `GameLogic::get_visible_objects(...)` and `get_visual_object_info(...)` now use shroud visible/explored object sets (when runtime shroud state is active) instead of effectively treating all non-stealthed objects as visible.
- Player relationship initialization parity improved:
  - `GameLogic/src/system/player_init.rs::PlayerList::init_team_alliances()` now applies template allies/enemies directives in map-init paths (instead of forcing enemy defaults), and preserves observer neutrality.
  - `GameLogic/src/system/player_init.rs::init_relationships_from_allies_enemies()` now also enforces observer-neutral relationships for direct initialization callers.
- Command targeting parity improved:
  - `InputCommandProcessor::find_object_at_position(...)` now uses context-aware click picking (selected-unit command context vs raw selection context), prioritizing enemy attackable targets during issued commands.
- Input projection parity improved:
  - `InputCommandProcessor` now updates screen-to-world mapping from runtime viewport resize events and current world bounds instead of fixed `800x600` assumptions.
- Locomotor terrain-height parity improved:
  - `GameLogic/src/locomotor/core.rs::Locomotor::get_terrain_height(...)` now samples runtime terrain layer height for non-air locomotors instead of preserving stale current Z.
- Thrust locomotor helper parity improved:
  - `GameLogic/src/locomotor_impl.rs::calc_direction_to_apply_thrust(...)` now uses gravity-adjusted quadratic thrust-direction solving with C++-style fallback behavior.
  - `GameLogic/src/locomotor_impl.rs::move_towards_position_thrust(...)` now applies `max_thrust_angle` via constrained vector rotation instead of using the previous simplified direct-thrust branch.
- GrantStealth behavior parity improved:
  - `GameLogic/src/object/behavior/grant_stealth_behavior.rs` now performs C++-style final scan shutdown by destroying the grantor object at max radius instead of periodically rescanning forever.
  - `RadiusParticleSystemName` is now parsed and applied with proper create/destroy lifecycle handling.
  - Stealth grant visual feedback now flashes recipient drawables as selected instead of skipping this lane.
  - Grant scans now honor ally relationships (`Friend/Ally/Allies`) and include contain rider forwarding via `friend_get_rider`.
- QueueProductionExitUpdate parity improved:
  - `GameLogic/src/object/behavior/queue_production_exit_behavior.rs` now performs C++-style airborne creation checks and applies terrain snap when `AllowAirborneCreation = No`.
  - Door-exit path now propagates owner layer, registers spawned units with AI pathfinder, and issues `ai_follow_exit_production_path(...)` with natural-rally sequencing (including doubled-natural-rally anti-stacking fallback).
  - Airborne producer exits now apply inherited motive force from producer velocity, reducing spawn drift for in-air production.
  - Budding exit now supports C++ no-host fallback to producer position/orientation (instead of failing), copies host layer when host exists, and issues post-spawn `ai_move_to_position(...)`.
  - Queue exit update now keeps `UPDATE_SLEEP_NONE` semantics and save/load now includes queue runtime state (`current_delay`, rally point state, burst count, creation clear distance).
- SupplyCenterProductionExitUpdate parity improved:
  - `GameLogic/src/object/behavior/supply_center_production_exit_behavior.rs` now uses `ai_follow_exit_production_path(...)` for spawned truck exit handoff (matching C++ path-command behavior instead of movement-target-only assignment).
  - Supply truck force-wanting activation (`SupplyTruckAIInterface::set_force_wanting_state(true)`) is now aligned with this exit-path handoff lane.
  - `GrantTemporaryStealth` runtime gating now matches C++ conditions (owner stealthed + target temporary-grant or no `CAN_STEALTH` capability).
  - `GrantTemporaryStealth` INI now parses duration values (`parse_duration_unsigned_int`) rather than raw integers.
  - Save/load now carries supply-center rally-point runtime state through behavior snapshot/xfer (`m_rallyPoint`, `m_rallyPointExists`).
- SpawnPointProductionExitUpdate parity improved:
  - `GameLogic/src/object/behavior/spawn_point_production_exit_behavior.rs` now registers spawned units with the AI pathfinder map after spawn-point placement, matching C++ spawn activation behavior.
  - Door reservation now performs C++-style lazy spawn-bone initialization and occupier revalidation before availability checks.
  - Save/load now preserves spawn-point occupier state (`m_spawnPointOccupier[MAX_SPAWN_POINTS]`) through behavior snapshot/xfer instead of losing occupancy ownership across restore.
  - `SpawnPointBoneName` parse path now ignores `=` tokens in field token streams, improving parity with C++ INI field parsing behavior.
- DefaultProductionExitUpdate save/load parity improved:
  - `GameLogic/src/object/behavior/default_production_exit_behavior.rs` now snapshots/restores runtime rally state (`m_rallyPoint`, `m_rallyPointExists`) at behavior level, matching C++ xfer coverage.
  - Module snapshot paths now route through behavior state, eliminating the prior module-data-only save/load drift for this exit behavior.
  - `use_spawn_rally_point()` is now exposed directly from module data to support downstream parity hooks.
- Terrain water shoreline parity improved:
  - `GameClient/src/terrain/water.rs` now generates shoreline blend geometry for rectangle/circle/polygon/path/spline water segments instead of leaving `shore_geometry` empty.
  - Shore bands now include inner/outer ring vertices and triangle strips with outer alpha fade, enabling blend-friendly terrain-water transition rendering.
  - Water flow updates now also propagate per-vertex flow vectors into generated shoreline geometry.
- Minimap terrain/FOW parity improved:
  - `Main/src/graphics/minimap_renderer.rs` now supports composed minimap textures (terrain base + FOW darkening) instead of FOW-only grayscale output when terrain data is available.
  - Minimap mapping now clamps and guards against degenerate spans for world<->minimap conversions, improving click/projection stability at map bounds and during resize.
  - Minimap refresh now forces immediate upload after base/world/screen updates, reducing stale minimap frames after map/world changes.
- Minimap visibility/readability parity improved:
  - `Main/src/game_logic/game_logic.rs::update_ui_state(...)` now applies shroud-aware minimap visibility filtering for object dots (instead of unconditional all-object dots).
  - Minimap now keeps explored structures for continuity while requiring live visibility for non-structure opponent dots.
  - `Main/src/graphics/minimap_renderer.rs` now softens FOW transition edges to reduce hard Hidden/Explored/Visible banding artifacts.
- Minimap terrain-base generation parity improved:
  - `Main/src/graphics/render_pipeline.rs` now generates minimap terrain base from live terrain-height sampling and injects it into minimap rendering on initialization/world-bounds changes.
  - Base pass now includes elevation gradient shading, light embossing, and low-elevation water tinting for more faithful minimap readability.
- Minimap static-road overlay parity improved:
  - `GameClient/src/terrain/roads.rs` now exports minimap road samples with road-type tint metadata (`RoadMinimapSample::tint_rgb`).
  - `Main/src/graphics/render_pipeline.rs` now draws road overlays in the minimap terrain base pass using road-type-aware tints (instead of one hardcoded road color) and width-scaled blend/radius.
- Radar terrain-texture parity improved:
  - `Common/src/common/system/radar.rs::refresh_terrain()` now uses sampled terrain cell data (height + water flags) to generate radar terrain texture shading instead of a single flat terrain color.
  - Radar terrain texture now includes elevation-driven tinting, local slope shading, and water-cell blue/depth tint modulation when full radar-resolution terrain samples are available.
- Radar coordinate mapping parity improved:
  - `Common/src/common/system/radar.rs` world<->radar conversion now respects non-zero map origins (`map_extent.lo`) instead of assuming origin at `(0,0)`.
  - `radar_to_world(...)` now returns sampled terrain-cell height when available, reducing average-height Z drift in radar-to-world jumps.
- Terrain roads normal parity improved:
  - `GameClient/src/terrain/roads.rs` now supports terrain-sampled normal projection (`apply_terrain_normals`) for road surface/edge/marking geometry.
  - `GameClient/src/terrain/terrain_visual.rs` now applies live heightmap normals to roads during update, replacing tangent-only normal drift.
- Stealth/FOW parity improved:
  - shroud-driven visibility snapshots now apply `can_see_object_with_stealth(...)` filtering for currently visible objects.
- Drag-selection parity improved:
  - drag world bounds are now propagated through `MouseCommandContext`, and area selection now uses live object transform positions (`get_position`) instead of stale cached `position` fields.
- Main engine cursor mapping parity improved:
  - `CnCGameEngine::update_mouse_world_position()` now maps viewport coordinates into current world bounds (map-size aware), replacing fixed-span projection constants.
- Main texture decode parity improved:
  - `Code/Main/src/assets/textures.rs` now performs real DXT3/DXT5 decompression (BC2/BC3 alpha+color block decode) instead of solid-color placeholder output.
  - DXT block decode output now writes in row-major destination order (fixed block-append ordering drift).
- Replay command stream parity improved:
  - `Code/Main/src/save_load/replay.rs` now records `CommandType::Sell` as explicit `ReplayEventType::SellCommand` instead of aliasing it to `BuildCommand`.
  - Command-event classification now maps additional lanes with explicit replay types:
    - selection commands -> `SelectCommand`,
    - upgrade/economy commands (`PurchaseScience`/`QueueUpgrade`/`CancelUpgrade`) -> `Upgrade`,
    - dozer/construct variants -> `BuildCommand`.
  - Playback command deserialization now accepts replay `SelectCommand` and `Upgrade` event lanes.
- Economy command parity improved:
  - `Code/Main/src/command_executor.rs` no longer uses string-length placeholder pricing for science/upgrade commands.
  - Queue/cancel upgrade commands now use deterministic cost resolution and player-side queued-upgrade tracking, preventing duplicate queue and repeated cancel-refund exploits.
- Save/load snapshot parity improved for production and upgrade state:
  - `Code/Main/src/save_load/snapshot.rs` now snapshots/restores object module data for:
    - building production queue entries/progress/rally point,
    - object applied-upgrade tags.
  - Player snapshots now include deterministic, richer parity data:
    - computed unit population usage,
    - per-team build queue capture from building production queues,
    - non-empty tech-tree capture for owned units/buildings plus unlocked/queued upgrade markers.
- Save/load weather/path cache parity improved:
  - `Code/Main/src/game_logic/game_logic.rs` now carries runtime weather state (`current_weather`, `intensity`, `duration_remaining`, `next_change_time`) with reset/default lifecycle parity.
  - `Code/Main/src/save_load/snapshot.rs` now snapshots/restores real weather state instead of default/no-op behavior.
  - `PathfindingCacheSnapshot` is now populated from active movement paths and used to rehydrate missing movement-path payloads during restore.
- Sync model-load parity improved:
  - `Code/Main/src/assets/manager.rs::load_w3d_model(...)` now executes the real W3D loader path (with timeout + runtime-aware blocking) instead of unconditional fallback-mesh return on cache miss.
- Quoted-printable and map-cache text parity improved:
  - `Code/GameEngine/Common/src/common/system/quoted_printable.rs` now uses C++-aligned UTF-16LE byte semantics for unicode encode/decode paths (instead of approximate UTF-8 behavior).
  - `Code/GameEngine/Common/src/common/ini/ini_map_cache.rs` now routes map-cache quoted-printable decoding through shared Common helpers and no longer emits map-cache insert debug spam.
- Webpage URL INI/runtime parity improved:
  - `Code/GameEngine/Common/src/common/ini/ini_webpage_url.rs` now resolves registry language from runtime language mapping (+ env override path) instead of fixed `English`.
  - `file://` conversion now follows C++ path semantics using encoded cwd and `\\Data\\<language>\\...` formatting.
  - URL parse/open logging moved off stdout (`println!`) to debug logging.
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now actively routes `WebpageURL` blocks, and `ini_webpage_url` now consumes block properties from the core INI stream (`...from_ini`) before URL registration/conversion.
- Object INI block coverage improved in Common parser:
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now includes active block handlers for `Object` and `ObjectReskin`.
  - `Code/GameEngine/Common/src/common/ini/ini_object.rs` now bridges those handlers through the runtime global `ThingFactory` and applies source-existence-aware reskin validation.
- Terrain bridge INI transition parsing parity improved:
  - `Code/GameEngine/Common/src/common/ini/ini_road.rs` now parses `TransitionToOCL` / `TransitionToFX` values with C++-style `Transition`/`ToState`/`EffectNum` semantics and writes transition entries into damage/repair FX/OCL arrays.
  - Bridge `RadarColor` parsing now accepts legacy component format (`R:100 G:114 B:245`) instead of only compact colon triples.
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now actively routes `Road` and `Bridge` blocks into the terrain-road parser, so these definitions are consumed through the primary INI block pipeline.
- Core INI top-level coverage improved in Common parser:
  - `Code/GameEngine/Common/src/common/ini/ini.rs` now actively routes and registers additional major gameplay/content blocks: `FXList`, `Locomotor`, `ParticleSystem`, `SpecialPower`, `Terrain`, `Upgrade`, `Video`, `WaterSet`, `WaterTransparency`, `Weapon`, and `CrateData`.
  - Existing parser lanes in command/audio/control/crate modules now read definition names from block value tokens instead of incorrectly using the block keyword.
  - Remaining stock top-level tokens are now also covered with fallback/nested-safe parsing (`Armor`, `ObjectCreationList`, shell/button/UI map tokens, `WindowTransition`, etc.), with external parsers still overriding when registered.
- ObjectCreationList runtime loading parity improved:
  - `Code/GameEngine/GameLogic/src/object_creation_list/store.rs` now includes a default-path OCL loader/parser for `ObjectCreationList.ini` and ingests all stock nugget headers used by Zero Hour data:
    - `CreateObject`
    - `CreateDebris`
    - `DeliverPayload`
    - `FireWeapon`
    - `Attack`
    - `ApplyRandomForce`
  - Nested `DeliveryDecal ... End` parsing is now handled correctly inside `DeliverPayload`/`Attack` nuggets (inner `End` no longer closes the parent nugget).
  - Parser field coverage now includes the stock advanced lanes used in `ObjectCreationList.ini` (delivery payload data/decal/weapon-slot fields, fire-weapon template field, attack shot/slot/decal fields, random-force fields, and expanded generic lifetime/fade/formation/container/fx/lod/shadow fields).
  - Added regression coverage against real stock data:
    - `test_parse_stock_object_creation_list_file_when_present`
    - `test_parse_advanced_nuggets_and_nested_delivery_decal`
  - OCL load semantics now support incremental layering:
    - per-name OCL replacement no longer clears the full OCL store,
    - previously loaded unrelated OCL definitions are preserved across subsequent loads.
  - `DeliverPayload` runtime behavior is now closer to C++:
    - `create_owner=false` uses the primary object as transport (instead of skipping),
    - payload population only executes when `DeliverPayloadAIUpdate` exists,
    - preferred-height reposition is applied after `deliver_payload(...)` call,
    - delayed delivery uses default disable lane parity (`DisabledDefault`).
  - `Code/GameEngine/GameLogic/src/helpers.rs` now ensures default OCL definitions are loaded before returning OCL lookups, reducing empty placeholder OCL fallback at runtime.
- Bridge damage/repair resource resolution parity improved:
  - `Code/GameEngine/GameLogic/src/object/behavior/bridge_behavior.rs` no longer auto-creates placeholder FX/OCL resources when names are unresolved.
  - Missing bridge transition resources now remain null/skip at runtime (with debug logging), matching C++ null-reference behavior more closely.
- OCL reference parsing parity improved across gameplay modules:
  - Multiple GameLogic parsers no longer auto-create placeholder OCLs for unresolved names during INI parse.
  - Unresolved references now remain `None` and are skipped by runtime execution paths, reducing synthetic side effects from phantom empty OCL entries.
  - Updated modules include OCLUpdate, TransitionDamageFX, ObjectCreationUpgrade, FireOCLAfterWeaponCooldownUpdate, BattleBusSlowDeathBehavior, and BoneFXUpdate.
  - `TheObjectCreationListStore` helper semantics were also tightened:
    - `find/lookup/ensure` now resolve only existing OCLs,
    - implicit empty-list fallback creation was removed from helper lookup paths.
- FX reference handling parity improved across gameplay modules:
  - Added strict FX lookup path (`TheFXListStore::lookup_fx_list`) and made `find_fx_list` lookup-only (no implicit creation); `ensure_fx_list` is now explicit-create only.
  - Removed parse/runtime fallback creation paths that synthesized missing FX entries.
  - Unresolved FX names now remain null and are skipped naturally instead of auto-materializing placeholder FX lists.
  - Updated modules include TransitionDamageFX, BoneFXUpdate, BattleBusSlowDeathBehavior, DumbProjectileBehavior, W3DTreeDraw, ParticleUplinkCannonUpdate, and Unit rappel kill FX.
- Delayed weapon damage processing parity improved:
  - In the active weapon runtime path (`GameLogic/src/weapon/mod.rs`), queued delayed damage now executes real damage logic at processing time (temporary weapon + detonation fire path), instead of no-op completion.
  - Added coverage for delayed-damage enqueue path from template references.
  - Projectileless travel-time weapons in active runtime `Weapon::private_fire_weapon(...)` now use delayed-damage scheduling (or immediate damage for sub-frame delay) instead of failing on missing projectile templates when `projectile_name` is empty.
  - Delayed detonation execution now preserves queued schedule-time `WeaponBonus` snapshots instead of recomputing bonuses from empty execution-time flag inputs.
  - Parallel `weapon/weapon_template.rs` delayed scheduler marker is now replaced with active-store scheduling bridge + immediate fallback on scheduling failure, reducing cross-stack delayed-damage drift.
  - Parallel `weapon/weapon_template.rs` laser branch now creates runtime laser objects (`create_laser_object`) for hit/miss lanes instead of no-op comment paths.
  - Parallel `weapon/weapon_template.rs` now applies real `WEAPON_KILLS_SELF` self-damage with correct mask-bit semantics and huge-damage kill behavior.
  - Parallel `weapon/weapon_template.rs` area-damage source-self filtering now also uses the correct `KILLS_SELF` (SUICIDE) bit, matching non-area lane semantics.
  - Parallel `weapon/weapon_template.rs::deal_damage_internal(...)` now activates historic bonus tracking/triggering at runtime, including chained bonus-weapon fire when configured thresholds are met.
  - Parallel `weapon/weapon_template.rs::deal_damage_internal(...)` now resolves live victim object position for damage centering (fallback to provided target only if lookup fails), reducing moving-target detonation drift.
  - Parallel `weapon/weapon_template.rs` infantry-target scatter path now resolves target `KindOf::Infantry` at runtime and applies `infantry_inaccuracy_dist` to scatter radius (instead of leaving that branch inert).
  - Parallel `weapon/weapon_template.rs` scatter destination now samples terrain layer height (`TheTerrainLogic::get_layer_height`) after X/Y scatter offsets instead of keeping stale victim Z.
  - `weapon/weapon.rs` scatter-target firing path now samples terrain ground height (`TheTerrainLogic::get_ground_height`) after X/Y scatter offsets instead of preserving stale Z.
  - Parallel `weapon/weapon_template.rs` fire path now routes through source `Drawable::handle_weapon_fire_fx(...)`, enabling module-side barrel recoil/muzzle-fire handling instead of leaving this lane skipped.
  - Added coverage:
    - `test_projectileless_weapon_queues_delayed_damage`
    - `test_projectileless_weapon_skips_queue_when_damage_disabled`
- Tree-draw topple animation parity improved:
  - `GameLogic/src/object/draw/w3d_tree_draw.rs` now applies topple rotation into the draw transform path instead of skipping the rotation lane.
  - Topple axis now resolves deterministically from push direction (perpendicular axis in X/Y plane, X-axis fallback).
- Debris draw landed-transition parity improved:
  - `GameLogic/src/object/draw/w3d_debris_draw.rs` now transitions debris from flying to final state based on owner-object terrain contact (`is_above_terrain`) with minimum-frame gating, instead of relying only on external final-transition calls.
  - `GameLogic/src/object/drawable.rs` now binds owner IDs into debris draw modules to support that runtime terrain-state query path.
- Model-draw sub-object visibility state parity improved:
  - `GameLogic/src/object/draw/w3d_model_draw.rs` now treats sub-object hide/show updates case-insensitively and canonicalizes/deduplicates queued sub-object visibility entries before clearing dirty state.
- Full device feature matrix compiles:
  - `cargo check -p game_engine_device --all-features`
- Main package is now build-clean including auxiliary/dev bins:
  - `cargo check -p generals_main --all-features`
- Main playable executable remains build-clean:
  - `cargo check -p generals_main --all-features --bin generals`
- Hard runtime stubs in active non-test source remain at zero:
  - `todo!`: 0
  - `unimplemented!`: 0
  - panic-not-implemented patterns: 0

## What Is Bad / Not Perfect
- Marker debt remains (`TODO|FIXME|placeholder|stub`) in non-test engine/client/gameplay code:
  - Common: 49
  - GameLogic: 16
  - GameClient: 37
  - GameEngineDevice: 9
  - Total: 111
- ObjectCreationList parse coverage now matches stock `ObjectCreationList.ini` structure and active fields; remaining OCL parity risk is now primarily in deeper runtime execution behavior (nugget side-effects/timing edge-cases), not parser block coverage.

## Not Yet Fully Implemented (High Impact)
- `GameEngineDevice/src/w3d/performance_optimizer.rs`
  - Compile-clean and now honors runtime optimizer settings (LOD/culling/instancing/batching + cull distance) with fuller stats accounting.
  - GPU culling now executes real compute-shader dispatch + visibility readback filtering, with resilient CPU fallback for unavailable GPU resources/readback failures.
  - Optimizer telemetry now records real GPU cull and batching stage timings (`gpu_cull_time_ms`, `batch_time_ms`).
  - Optimizer frame-history/memory/quality-control fields are now actively consumed at runtime (periodic telemetry capture + auto-quality decision lane), instead of remaining dormant declarations.
  - LOD selection now applies global LOD bias and avoids implicit default `mesh_lodN` rewrites when no explicit LOD mapping exists (reduces mesh-miss behavior).
  - Instancing now computes inverse-transpose normal matrices for per-instance lighting parity.
  - Instancing/batch grouping now preserves transparent-vs-opaque separation so transparent ordering is not lost during optimization.
  - Remaining gap is exact threshold/tuning parity against C++ policy for all heuristic branches (especially dynamic-resolution integration details and cross-map content tuning).
- `GameEngineDevice/src/w3d/renderer.rs`
  - Transparent materials are now emitted through a dedicated transparent queue with back-to-front sorting.
  - Per-object transform propagation is now wired in the scene render path (no identity-transform fallback for all objects).
  - Remaining gap is deeper render-pass specialization parity (full deferred/forward+ phase behavior and material-state side effects), not transparent queue/transform omission.
- `GameEngineDevice/src/w3d/w3d_c_api.rs`
  - Compile-clean; texture resolution/decoding, state round-trip behavior, transform-to-scene camera sync, and immediate draw submission are improved.
  - Legacy scene lifecycle entry points are now exported (`W3DDevice_BeginScene` / `W3DDevice_EndScene`).
  - Scene lifecycle calls are now stateful (`BeginScene`/`EndScene` reject invalid order) with resilient implicit begin/end bridging for legacy caller drift.
  - Added additional legacy compatibility entry points:
    - `W3DDevice_DrawPrimitiveUP(...)` (primitive-count + strided immediate-mode draw),
    - `W3DDevice_DrawIndexedPrimitiveUP(...)` (indexed immediate-mode draw with 16/32-bit index formats),
    - `W3DDevice_DrawPrimitive(...)` (staged non-indexed primitive submission with start-vertex semantics),
    - `W3DDevice_SetStreamSource(...)` / `W3DDevice_SetStreamSourceUP(...)` / `W3DDevice_SetStreamSourceEx(...)` / `W3DDevice_SetIndices(...)` (staged stream/index geometry compatibility, including explicit stream byte offset state),
    - `W3DDevice_GetStreamSource(...)` / `W3DDevice_GetStreamSourceEx(...)` / `W3DDevice_GetIndices(...)` (staged geometry state readback),
    - `W3DDevice_DrawIndexedPrimitiveLegacy(...)` (DX8-style staged indexed draw args: `min_vertex_index/start_index/primitive_count`),
    - `W3DDevice_SetFVF(...)` / `W3DDevice_GetFVF(...)` and `W3DDevice_SetVertexShader(...)` / `W3DDevice_GetVertexShader(...)` + `W3DDevice_SetPixelShader(...)` / `W3DDevice_GetPixelShader(...)` (fixed-function/shader-state round-trip compatibility),
    - `W3DDevice_SetVertexDeclaration(...)` / `W3DDevice_GetVertexDeclaration(...)` (vertex declaration state compatibility),
    - `W3DDevice_SetTexture(...)` / `W3DDevice_GetTexture(...)` (stage texture bind/readback path),
    - `W3DDevice_SetViewport(...)` / `W3DDevice_GetViewport(...)` (viewport state round-trip),
    - `W3DDevice_SetTextureStageState(...)` / `W3DDevice_GetTextureStageState(...)` (texture-stage state round-trip),
    - `W3DDevice_SetMaterial(...)` / `W3DDevice_GetMaterial(...)` and `W3DDevice_SetLight(...)` / `W3DDevice_GetLight(...)`,
    - `W3DDevice_LightEnable(...)` / `W3DDevice_SetLightEnable(...)` / `W3DDevice_GetLightEnable(...)`.
  - `DrawIndexedPrimitive(...)` now supports staged geometry fallback when direct pointers are absent, including staged base-vertex index behavior.
  - Staged stream decode/readback paths now apply per-stream byte offsets and staged vertex counts consistently; indexed staged decode no longer rejects valid non-`W3D_VERTEX` FVF strides.
  - `DrawPrimitiveUP(...)` + staged stream decode now support common fixed-function non-`W3D_VERTEX` payloads through FVF-aware decoding (including transformed-lit 32-byte vertices).
  - Stage-0 texture-coordinate routing now affects staged/declaration decode:
    - `D3DTSS_TEXCOORDINDEX` now selects declaration `TEXCOORD[n]` usage for UV extraction,
    - staged multi-stream UV overlay now tries the stage-requested stream first before generic fallback.
  - Stage-0 texture transform is now applied to submitted UVs in immediate/staged draw paths using:
    - `D3DTSS_TEXTURETRANSFORMFLAGS` (`D3DTTFF_COUNT1..COUNT4`),
    - projected divide behavior (`D3DTTFF_PROJECTED`),
    - current `W3DTS_TEXTURE0` transform matrix state.
  - Stage-0 texcoord generation now supports camera-space sources:
    - `D3DTSS_TCI_CAMERASPACEPOSITION`,
    - `D3DTSS_TCI_CAMERASPACENORMAL`,
    - `D3DTSS_TCI_CAMERASPACEREFLECTIONVECTOR`,
    - `D3DTSS_TCI_SPHEREMAP`,
    - with source vectors derived from current world/view transform state before texture-matrix application.
  - Camera-space texcoord generation now also applies when `D3DTSS_TEXTURETRANSFORMFLAGS` is disabled (not only in transformed stages), improving fixed-function TCI parity.
  - Texture-stage state queries now return D3D-style defaults for unset keys (`COLOROP`/`ALPHAOP`, args, `TEXCOORDINDEX`, transform flags), improving compatibility with legacy caller assumptions.
  - Draw-time texture/UV/material stage routing now follows the first enabled bound texture stage (stage 0 preferred when enabled) instead of hardcoding stage 0.
  - Draw-time stage routing is now texture-sampling aware:
    - stage selection now skips enabled stages that do not actually sample texture args (for example `SELECTARG2 CURRENT`),
    - includes explicit `D3DTOP_SELECTARG2` argument-usage semantics in stage input analysis.
    - includes triadic texture-op arg0 coverage (`COLORARG0`/`ALPHAARG0`, `D3DTOP_LERP`/`D3DTOP_MULTIPLYADD`) so stage selection recognizes texture sampling from arg0 in multi-argument ops.
    - extended fixed-function op families now participate in texture-usage detection (`MODULATE2X/4X`, signed/smooth/add/subtract variants, blend-alpha variants, premodulate, modulate+add variants, bump-env variants), reducing multi-stage combiner under-detection drift.
    - `D3DTA_*` source detection now respects the selector mask, so `TFACTOR`-only combiner stages no longer masquerade as texture-sampling stages while `COMPLEMENT` / `ALPHAREPLICATE` modifiers on real texture args still work.
    - simple `SELECTARG*` / `MODULATE*` active stages now propagate `D3DRS_TEXTUREFACTOR` into the fallback material tint, including default white texture-factor state and `ALPHAREPLICATE` / `COMPLEMENT` handling.
    - simple enabled no-texture stages can now also contribute `TEXTUREFACTOR` tint to fallback materials instead of always collapsing to an untinted base material.
    - simple additive `TEXTUREFACTOR` alpha stages where both args are `TFACTOR`-derived now also contribute fallback alpha tint, covering common `TFACTOR + (1-TFACTOR)` mask setups used by legacy terrain/shader-manager paths.
    - supported fixed-function fallback stages now propagate `CURRENT` across simple stage chains, so later `MODULATE(TEXTURE, CURRENT)` style stages preserve earlier tint contributions instead of resetting to a stage-local approximation.
    - supported fallback chains now also handle narrow `BLENDCURRENTALPHA` cases, preserving simple `CURRENT`-alpha blends used by legacy terrain/cloud composition paths.
    - modified neutral `TEXTURE` / `DIFFUSE` args and narrow `BLENDFACTORALPHA` cases now participate in the same fallback chain evaluator, improving diffuse-forced white/black terms and texture-factor alpha blends.
    - triadic arg0 combiner cases now also participate in the fallback chain evaluator (`MULTIPLYADD`, `LERP`), covering the W3D shader-manager grayscale path’s `COLORARG0` usage.
    - narrow `DOTPRODUCT3` chains now also participate in that evaluator, preserving the shader-manager grayscale path where `MULTIPLYADD` feeds `CURRENT` into a following dot-product stage.
    - narrow arithmetic combiner chains (`ADDSIGNED`, `ADDSIGNED2X`, `SUBTRACT`, `ADDSMOOTH`) now also participate, reducing stage-selection/material-approximation drift for legacy `CURRENT`-carrying fixed-function passes.
    - neutral-source blend-alpha chains (`BLENDDIFFUSEALPHA`, `BLENDTEXTUREALPHA`, `BLENDTEXTUREALPHAPM`) now also participate in the same approximation lane, preventing those legacy stages from being dropped when the fallback is already operating in its neutral texture/diffuse domain.
    - stage-local `MODULATE*ADD*` combiner chains (`MODULATEALPHA_ADDCOLOR`, `MODULATECOLOR_ADDALPHA`, `MODULATEINVALPHA_ADDCOLOR`, `MODULATEINVCOLOR_ADDALPHA`) now also participate in that evaluator, reducing another class of active-stage/material-resolution drift in legacy `CURRENT`-carrying fixed-function passes.
    - `PREMODULATE` now acts as an explicit pass-through stage in that same evaluator, preserving previously established tint/alpha across legacy premodulate chains instead of treating the stage as unsupported.
    - `BUMPENVMAP` and `BUMPENVMAPLUMINANCE` now also act as pass-through stages in that evaluator, preserving previously established tint/alpha across legacy bump-env coordinate-manipulation stages.
    - `BLENDCURRENTALPHA` is now supported in the alpha lane as well as the color lane, reducing another alpha-only fallback drop case in legacy multi-stage chains.
    - unknown/unsupported op values are now treated as non-sampling in this lane, reducing false-positive stage picks under invalid legacy state payloads.
    - stage selection now prioritizes color-sampling stages over alpha-only sampling stages, reducing legacy multi-stage cases where stage 0 alpha-mask usage incorrectly overrode later color-texture stages.
    - alpha-only texture stage selection remains as fallback when no enabled stage samples texture in color ops.
  - Immediate draw material binding now resolves active-stage texture + current material into effective cached bound materials, improving legacy `SetTexture`/`SetMaterial` call-order parity.
  - Declaration-stream decode now supports additional legacy element types (`FLOAT1`, `USHORT4N`, `UDEC3`, `DEC3N`) across UV/position/normal reconstruction, reducing multi-stream/declaration fidelity drift.
  - Immediate/staged C-API transient draws now honor `W3DRS_ALPHABLENDENABLE` for transparency routing (in addition to material transparency), improving fixed-function render-state parity for legacy draw paths.
  - Fixed-function lighting/material-source state is now tracked in the active bridge:
    - `W3DRS_LIGHTING`, `W3DRS_AMBIENT`, `W3DRS_COLORVERTEX`, `W3DRS_LOCALVIEWER`, `W3DRS_NORMALIZENORMALS`,
    - `W3DRS_DIFFUSEMATERIALSOURCE`, `W3DRS_SPECULARMATERIALSOURCE`, `W3DRS_AMBIENTMATERIALSOURCE`, `W3DRS_EMISSIVEMATERIALSOURCE`.
    - Bound-material cache keys now include those states, ambient render-state color now feeds fallback material approximation, and specular/emissive behavior no longer ignores material-source toggles.
    - No-texture unlit fixed-function materials now remain visible when lighting is disabled instead of collapsing toward black.
  - Fixed-function unlit textured parity is now wired through the active render path:
    - fallback material generation now marks `LIGHTING = FALSE` materials as explicit unlit variants instead of only approximating them through emissive/specular edits,
    - queued renderer material params now derive from actual `MaterialProperties` instead of hardcoded defaults,
    - the default WGSL shader now bypasses dynamic PBR lighting for those unlit fixed-function materials instead of relighting textured prelit passes.
  - Optimizer/instancing paths no longer collapse renderer-specialized material state back to generic defaults:
    - transient `RenderObject`s now carry material-derived batch params/priority into the optimized path,
    - `performance_optimizer.rs` now preserves those params/priority for instanced and non-instanced batches instead of forcing dummy defaults,
    - optimizer batch keys now preserve transparency lane and specialized priority so opaque/transparent or specialized-pass batches do not merge back together incorrectly.
  - Multi-texture fallback approximation is now slightly safer in the active bridge:
    - when a genuine multi-texture chain is detected and a base material already exists, Rust no longer fabricates a false single-stage texture override for the bound-material fallback.
  - Remaining gap is now the broader legacy C-API compatibility surface:
    - full texture-dependent multi-stage combiner evaluation across genuinely multi-texture stage chains,
    - deeper render-pass/material-state specialization outside the current default shader path,
    - fuller material/light semantics where exact legacy behavior depends on richer per-vertex/per-pass state than the current bounded fallback material approximation carries.
- `GameClient/src/core/subsystems.rs`
  - `movie_play_radar` now routes through `TheInGameUI` and in-game window playback (via `WindowVideoManager`) instead of fullscreen display playback, matching C++ action routing.
  - In-game UI movie playback manager is now updated each frame through subsystem lifecycle.
  - Script `is_video_complete` now tracks both fullscreen and in-game movie playback activity before resolving completion waits.
  - Window-video startup now uses real stream-open success/failure (no synthetic fallback stream), improving C++ parity for failed movie opens.
  - Rust movie startup/stop control flow now matches the C++ path more closely:
    - failed or unavailable fullscreen/radar movie starts no longer auto-complete script waits,
    - stop/reset paths no longer synthesize video completion events,
    - pending script waits are only created when playback actually starts.
  - Fullscreen and radar movie wait state is now tracked separately:
    - fullscreen waits alone control the `intro movie playing` hack path,
    - radar/in-game waits no longer perturb intro movie state,
    - client reset/shutdown now clears residual movie/media wait state.
  - Same-name movie restarts now preserve fullscreen vs radar wait-lane separation:
    - fullscreen restarts clear only stale fullscreen pending state,
    - radar restarts clear only stale radar pending state,
    - same-name retries in one lane no longer erase the other lane's pending completion tracking.
  - Global video-player lifecycle is now wired through client init/reset/shutdown:
    - the singleton can be re-created after shutdown,
    - reset now closes global video streams,
    - client teardown now shuts the movie singleton down explicitly.
  - `TheVideoPlayer` subsystem path now behaves more like C++:
    - legacy `Video.ini` metadata is loaded into the Rust GameClient movie registry,
    - subsystem `init/reset/update` now delegate to the real global video-player singleton instead of inert wrapper state,
    - the client update loop now services the video player after the window manager, matching the original runtime ordering more closely.
  - Common movie open/load resolution now also follows the original C++ backend split more closely:
    - movie names are resolved through `Video.ini` to localized/shared `.bik` asset paths,
    - `GlobalData::mod_dir` and extracted Windows asset-tree search participate in that resolution,
    - stream creation is now delegated through a backend-provider hook rather than being permanently hardcoded to `None` in the common player layer.
  - Active movie-stream ownership now also follows the original shape more closely:
    - provider-created streams are tracked by the common player layer,
    - `TheVideoPlayer` update/reset now services and closes those tracked streams centrally,
    - this removes another Rust-only gap where backend-created movie streams would previously bypass common player lifecycle ownership.
  - Provider-validity lifecycle now also follows the original shape more closely:
    - registering or clearing a backend stream provider now notifies the live player immediately,
    - singleton recreation after shutdown restores provider-validity state,
    - shutdown now deinitializes the player and closes tracked provider-backed streams before dropping it.
  - Startup logo/sizzle sequencing now follows the original client more closely:
    - active `GameClient::update()` now drives the EA logo -> `after_intro` -> optional sizzle startup path instead of skipping it,
    - low-res movie preference now also selects the `640` startup variants on that boot path.
  - Common startup-movie defaults/runtime sync now follow the original shape more closely:
    - `play_sizzle` now defaults to enabled again instead of being silently off by default in active runtime data,
    - runtime-global-data application now carries `play_sizzle`, `after_intro`, and `allow_exit_out_of_movies` across the Common handoff instead of dropping them.
  - Common engine init now also follows the original startup transition more closely:
    - if intro playback is already disabled before boot finishes, Rust now promotes `after_intro` during engine init instead of relying only on command-line paths to do so.
  - Direct startup file boot now follows the original engine path more closely:
    - the legacy `-file <path>` switch is now parsed into runtime `initial_file`,
    - `.map` startup targets now disable shell-map/intro startup, stage `pending_file`, enqueue `NewGame`, and reseed logic random with `0`,
    - `.rep` startup targets now route through recorder playback startup instead of being ignored,
    - replay startup now explicitly initializes the recorder singleton before playback bootstrap instead of depending on prior runtime side effects.
  - Movie registry/asset resolution now follows the original path more closely:
    - direct movie names/filenames now still resolve even when no `Video.ini` registry entry exists,
    - direct `.bik` asset paths are accepted as-is,
    - `.bik` filename inputs now resolve by file name or stem,
    - mod-local `Data/INI/Default/Video.ini` and `Data/INI/Video.ini` overlays are now included in movie registry discovery.
  - Active client startup frame behavior now also follows the original boot path more closely:
    - while `play_intro` / `after_intro` is active, Rust now short-circuits the normal frame update stack and only services startup movie display work,
    - boot-path display servicing now follows `DRAW()` then `UPDATE()` ordering for that startup-only path.
  - Score-screen campaign completion now follows the original cinematic flow more closely:
    - end-of-campaign single-player now stages `Campaign::get_final_victory_movie()` instead of finalizing immediately,
    - score-screen finalization now waits for fullscreen movie playback to complete,
    - failed movie startup falls back to immediate finalization instead of leaving the score screen in a pending state,
    - low-detail preference state now suppresses the final-victory movie path instead of always attempting fullscreen playback.
  - Global/per-window pause+resume now follows C++ `WindowVideoManager` state semantics directly (`Pause`/`Play` state assignment), with hidden transitions handled by the runtime update loop.
  - Stop-path parity improved: `stop_movie` / `stop_all_movies` now set `Stop` state without removing entries (matching C++), while `stop_and_remove_movie` remains the explicit resource-release/removal lane.
  - `WindowVideoManager::update()` now matches C++ global-toggle and hidden-state transition semantics:
    - update short-circuits when `pause_all_movies` or `stop_all_movies` is active,
    - only `Play -> Hidden` and `Hidden -> Play` state transitions are applied from window visibility changes.
  - Subsystem lifecycle parity improved: `InGameUISubsystem::init/reset` now clear transient in-game UI state (queued UI events, placement anchors/templates, command/special-power pendings, and mode flags), reducing stale state carry-over between sessions.
  - Remaining gap is primarily rare edge-case timing nuance across mixed cinematic/UI transition paths.
- `Common/src/common/ini/ini_mapped_image.rs`
  - `ImageCollection::load` now follows C++ directory-flow (`UserData` overrides + `TextureSize_<N>` + `HandCreated`) and parse behavior is aligned for `Coords`/`Status`.
  - `parse_mapped_image_definition(...)` now parses in place against existing mapped-image entries (create/add-then-parse behavior) instead of clone-and-replace semantics.
  - Existing entries with `raw_texture_data` no longer hard-fail during reparse; path now warns and proceeds, matching C++ debug-assert-only behavior in release builds.
  - `parse_image_coords(...)` now accepts both colon and space-separated subtoken forms (`Left:10` and `Left 10`), matching C++ subtoken parsing tolerance.
- `GameClient/src/display/image.rs`
  - Client mapped-image metadata now syncs from Common at startup and lazily loads textures on first GPU upload from:
    - engine `FileSystem` (including BIG archive backend),
    - direct OS file paths as fallback.
  - Runtime now auto-ensures Local/BIG backends + search paths + FileSystem init on first mapped-image resource read, reducing startup ordering sensitivity.
- `Common/src/common/audio/audio_event_rts.rs`
  - Core C++ filename/play-info/localization behavior is now ported and owner/player resolver hooks are wired from `GameLogic` helper bridge.
  - C++ control/type bit masks and `AudioPriority` ordering are now corrected.
  - Async playback path now uses backend playback hook dispatch + runtime playback polling/cancel-stop semantics instead of fixed-duration simulation when backend is present.
  - Cached event-info lookup now enforces C++ validity gating (`audio_name` must match current `event_name`) so stale cache entries are ignored.
  - Remaining parity debt is in broader audio routing side-effects.
- `Common/src/common/audio/game_audio.rs` + `GameLogic/src/helpers.rs`
  - C++-style `shouldPlayLocally` player/allies/enemies gating is now active in runtime add-event path via resolver bridge.
  - Observer look-at fallback for dead local player is now wired via GameClient control-bar observer target hooks.
  - `add_audio_event` now returns C++-style sentinel handles (`NoSound`/`Error`/`NotForLocal`/`Muted`) rather than flattening failure paths to `0`.
  - `remove_audio_event` now honors stop-music sentinel handles and avoids sending stop requests for non-playing failure sentinels.
  - `is_on`/`set_on`/`set_volume`/`get_volume` now use C++ bitmask semantics, including combined affects and `SystemSetting` behavior.
  - Runtime now instantiates a music manager by default, avoiding false-positive music-play success on a missing manager.
  - C++ `getAudioLengthMS` flow is now implemented for script timing (`attack + main + decay` generated filenames), removing always-zero duration behavior.
  - Duration probing now covers WAV/MP3/OGG for script timing waits.
  - Remaining gap is rare/streaming codec edge behavior and exact legacy decode-time parity under malformed media.
- `GameLogic/src/action_manager.rs` + `GameLogic/src/commands/command_processor.rs` + `GameLogic/src/ai/ai_player.rs`
  - Playable map-extent parity improved by switching C++ `getExtent(...)`-equivalent checks to `get_maximum_pathfind_extent()` in action validation, beacon clamping, and AI superweapon fallback targeting.
- `GameLogic/src/object/unit.rs`
  - Player move-goal clipping now uses playable extent (`get_maximum_pathfind_extent`) instead of border-inclusive bounds, reducing edge-command drift.
- `GameLogic/src/terrain.rs`
  - `find_closest_edge_point` tie-break ordering now matches C++ (`top/right/bottom/left` precedence), reducing edge-selection drift in tie cases.
  - Bridge area checks now use actual bridge quad containment (not only bridge AABB), reducing false bridge-layer hits near rotated-bridge bounds.
  - `TerrainLogic::load_map(...)` now performs real map-file resolution + parse via `MapLoader`, and correctly fails on missing/invalid maps instead of unconditional success.
- `Common/src/common/audio/game_sounds.rs`
  - Sound admission path now applies limit/voice/channel gates through both concrete and trait-call paths.
  - Active-sound lifecycle cleanup now updates counters from backend playback completion.
  - AudioManager lifecycle/listener/sample configuration now flows into SoundManager trait path.
  - `ST_SHROUDED` visibility cull now uses live shroud-state resolver from GameLogic.
- `Common/src/common/audio/game_speech.rs`
  - `say_name` now resolves against loaded speech definitions instead of fabricating placeholder speech objects.
  - Speaker pause gating bug in update path is fixed.
- `Code/Main/src/game_logic/game_logic.rs` + `Code/Main/src/assets/models.rs` + `Code/Main/src/assets/manager.rs`
  - Map-object template synthesis now attempts real WW3D object-definition mapping for all unresolved template names (including decorative map objects) before fallback gating.
  - Decorative templates now perform archive-open model availability checks before insertion; unresolved decorative objects are skipped early instead of spawning fallback-triangle entities.
  - W3D model loading now probes broader mixed-case archive path/extension variants, and INI parser parity now captures `ModelName` + `Draw` keys used by Nature/Civilian map props.
  - Audio-only ambient objects (`SoundAmbient` + no model) are filtered from visual template synthesis.
  - Current quickstart/multi-map smokes in this lane report zero fallback-mesh warnings; remaining parity gap is visual completeness for decorative objects currently skipped as non-loadable instead of exact legacy remaps.
- `Code/Main/src/assets/textures.rs`
  - DDS compressed decode parity in `Main` is improved:
    - DXT3 explicit-alpha decode is implemented,
    - DXT5 interpolated-alpha decode is implemented,
    - DXT decode now writes row-major output pixels across multi-block textures.
  - Added challenge-map texture alias coverage for residual missing IDs:
    - `PMBarbwire2`, `UBStingerS02`, `UVTechWeap`, `UVToxinTrk`, `CBMogWell01`, `UBUndTunn01`, `UBUndTunnD`.
  - 9-map quickstart sweep now reports zero missing-texture warnings.

## Missing For Fully Playable (Single-Player)
- Close the high-impact non-network parity gaps above (W3D optimizer/C-API and remaining INI/audio/movie edge behavior).
- Complete runtime parity validation passes (campaign/skirmish/save-load/scripts/object visuals) against C++ reference behavior.
- Decide whether to fully port the legacy `game_engine_device` W3D feature lane or retire/gate it if superseded by the active WW3D stack.
- Improve decorative map-prop fidelity by resolving currently skipped non-loadable model IDs via stability-safe remaps (avoiding WW3D frame-state regressions observed with aggressive alias sets).

## Validation Run In This Pass
- `cargo test -q -p game_engine --test mapped_image_parity_tests -- --nocapture` (pass, `2 passed`)
- `cargo test -q -p game_engine --test audio_event_parity_tests -- --nocapture` (pass, `2 passed`)
- `cargo check -q -p game_engine` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `cargo test -q -p game_engine --test audio_event_parity_tests -- --nocapture` (pass, `2 passed`)
- `cargo check -q -p game_engine` (pass)
- `cargo check -q -p generals_main --all-features --bin generals` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `cargo check -q -p game_engine` (pass)
- `cargo check -q -p generals_main --all-features --bin generals` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d resolve_active_texture_stage_ -- --nocapture` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d op_uses_texture_arg_ -- --nocapture` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p game_engine_device --features w3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic topple_axis_ -- --nocapture` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic topple_rotation_matrix_absent_when_not_toppling -- --nocapture` (pass, `1 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p gamelogic` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p gamelogic` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic test_effective_scatter_radius_ -- --nocapture` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d resolve_active_texture_stage_ -- --nocapture` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d op_uses_texture_arg_respects_selectarg2_current -- --nocapture` (pass, `1 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p game_engine_device --features w3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p gamelogic test_effective_scatter_radius_ -- --nocapture` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p gamelogic` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p game_engine_device --features w3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d quality_direction_` (pass, `4 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p game_engine_device --features w3d average_frame_time_uses_history_entries` (pass, `1 passed`)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features --bin generals` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `cargo check -p game_engine_device --all-features --message-format short` (pass)
- `cargo check -p game-client-rust --message-format short` (pass)
- `cargo check -p gamelogic --message-format short` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main decode_dxt` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main replay::tests::` (pass, `5 passed`)
- `cargo test -q -p generals_main snapshot_restore_preserves_building_production_modules_and_object_upgrades` (pass)
- `cargo test -q -p generals_main snapshot_player_state_captures_population_build_queue_and_research` (pass)
- `cargo test -q -p generals_main snapshot_restore_preserves_weather_state` (pass)
- `cargo test -q -p generals_main snapshot_restore_rehydrates_paths_from_pathfinding_cache` (pass)
- `cargo test -q -p generals_main save_load::snapshot::tests::` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main queue_upgrade_deducts_once_per_team_and_prevents_duplicate_queue` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main cancel_upgrade_refunds_only_when_upgrade_is_queued` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main process_ai_behavior_` (pass, `3 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main single_instance::tests::test_create_generals_mutex_retains_guard` (pass)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main visibility_filter_` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main find_object_` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main mouse_position` (pass, `2 passed`)
- `RUSTFLAGS='-Awarnings' cargo test -q -p generals_main drag_selection_prefers_world_drag_bounds_when_provided` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUSTFLAGS='-Awarnings' RUST_LOG=warn timeout 35s cargo run -q -p generals_main --bin generals -- -quickstart` (alive until timeout; `Using fallback mesh: 0`; `No texture found for: 0`; no frame begin/end failure logs)
- 9-map 20s quickstart sweep (`defcon6`, `tournamenta`, `tournamentb`, `gc_nukegeneral`, `gc_superweaponsgeneral`, `gc_chemgeneral`, `forgottenforestzh`, `hostile dawn`, `homeland alliance`):
  - `Using fallback mesh`: 0
  - `No texture found for:`: 0
  - `begin_frame failed` / `engine frame already active`: 0
  - `end_frame failed`: 0

## 2026-03-02 Stability-Safe Decorative Alias Progress

### What Landed
- Added conservative non-vegetation decorative model aliases in:
  - `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
  - `GeneralsRust/Code/Main/src/assets/manager.rs`
- Closed map-prop remaps for:
  - `PMBoulders`, `PMlclusters`, `PMmcluster`, `pmcluster`, `PMRocks02/03/05/06/07`
  - `PMTrshPp03`, `PMTrshPl02`, `PMCrates`, `PMPUMP`
  - `CBSandBW2`, `CBSandBW4c`, `CVTRUCK`, `CBNShack`

### Stability Result
- Quickstart debug smoke remains stable after this subset:
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Important Guardrail
- A broader alias attempt that included `PT*` vegetation (`PTBush*`, `PTPine*`, `PTSpruce*`, `PTXPine05`) reproduced WW3D frame-state regression.
- Those vegetation aliases were rolled back.
- Remaining `due unavailable model` debug logs are now concentrated on that vegetation-only set.

### Current Residual Decorative Gaps (Debug Only)
- `Bush02 -> PTBush02`
- `Bush03 -> PTBush03`
- `Bush08 -> PTBush08`
- `Bush11 -> PTBush11`
- `TreePine -> PTPine01`
- `TreePine3 -> PTPine02`
- `TreeSpruce2 -> PTSpruce01_hi`
- `TreeSpruce05 -> PTXPine05`

## 2026-03-02 PT Vegetation Safe-Coverage Update

### What Changed
- Added runtime PT vegetation alias mode handling in:
  - `GeneralsRust/Code/Main/src/assets/manager.rs`
  - `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
- Default mode is now `trees_pines` (used when `GENERALS_PT_VEGETATION_ALIAS_MODE` is unset).

### Why
- `trees_pines` gives higher stable visual coverage than prior `bushes` default.
- It remains frame-stable in long quickstart smokes, while larger mixed modes regress WW3D frame state.

### Stability Matrix (80s Quickstart)
- Stable: `tree_pine1`, `tree_pine2`, `tree_spruce2`, `tree_spruce05`, `trees_pines`, `trees_spruces`.
- Unstable: `trees_three`, `all_fir`, `all_birch`, `all_oak`, `all_palm`, `all_maple`, `bushes_pines`, `bushes_spruces`.

### Current Effect
- `due unavailable model` (debug quickstart) improved to `176` with default `trees_pines`.
- Residual unresolved decorative set:
  - `Bush02 -> PTBush02`
  - `Bush03 -> PTBush03`
  - `Bush08 -> PTBush08`
  - `Bush11 -> PTBush11`
  - `TreeSpruce2 -> PTSpruce01_hi`
  - `TreeSpruce05 -> PTXPine05`

### Validation
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- `RUST_LOG=warn timeout 80s cargo run -q -p generals_main --bin generals -- -quickstart` (stable; no frame begin/end failures)

## 2026-03-02 WW3D Frame-State Recovery Hardening

### Runtime Fixes
- Hardened frame lifecycle in WW3D paths:
  - `ww3d-engine begin_render`: `frame_active` now toggles only on successful frame acquisition.
  - `ww3d-engine end_render`: `frame_active` now clears on all end-frame paths.
  - `WgpuMainRenderer end_frame`: always attempts engine-frame unwind even if frame rendering/callback stages error.

### Result
- Removed the persistent `begin_frame failed: engine frame already active` poison loop after a render failure.
- In aggressive stress mode (`GENERALS_PT_VEGETATION_ALIAS_MODE=all_fir`), failures now remain bounded to `end_frame` (no begin-frame lockout storm).

### Default Playability Status (unchanged stable mode)
- Default (`trees_pines`) quickstart remains stable:
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

## 2026-03-02 WW3D Uniform Arena Growth + PT Default Promotion

### What Changed
- `GeneralsRust/Code/Libraries/Source/WWVegas/WW3D2/crates/ww3d-renderer-3d/src/rendering/frame_uniform_arena.rs`
  - Frame uniform allocator moved from fixed single-buffer capacity to dynamic multi-page growth.
  - This closes render-frame failures under high mesh density (`frame uniform arena exhausted`).
- `GeneralsRust/Code/Libraries/Source/WWVegas/WW3D2/crates/ww3d-renderer-3d/src/core/error.rs`
  - Error conversion now preserves concrete renderer failure context instead of collapsing many paths into `Unknown`.
- `GeneralsRust/Code/Main/src/assets/manager.rs`
- `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
  - Default `GENERALS_PT_VEGETATION_ALIAS_MODE` is now `all_fir` (no env var needed).

### Stability Result
- Stress lane (`60s`, `GENERALS_PT_VEGETATION_ALIAS_MODE=all_fir`, `RUST_LOG=warn`):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `frame uniform arena exhausted`: `0`
- Default lane (`35s`, `RUST_LOG=debug`):
  - `due unavailable model`: `0`
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Previously Unstable PT Modes (Now Stable)
Validated at `25s` each (`RUST_LOG=warn`):
- `trees_three`, `all_fir`, `all_birch`, `all_oak`, `all_palm`, `all_maple`, `bushes_pines`, `bushes_spruces`
- All above now show:
  - `begin_frame failed=0`
  - `end_frame failed=0`

Coverage note from this sweep:
- `all_fir/all_birch/all_oak/all_palm/all_maple` each report `due unavailable model=0`.

### Validation
- `RUSTFLAGS='-Awarnings' cargo check -q -p ww3d-renderer-3d` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)
- 9-map quickstart sweep (`20s` each):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Build Cache
- `GeneralsRust/target`: `15G`.
- No cleanup run this pass (below heavy-threshold concern).

## 2026-03-02 Map Bounds Metadata Parity Update

### What Changed
- `GeneralsRust/Code/Main/src/game_logic/script_loader.rs`
  - `parse_world_bounds(...)` now falls back to `HeightMapData` dimensions when waypoint bounds are missing/degenerate.
  - Bounds are derived from playable terrain dimensions with `MAP_XY_FACTOR` scaling and returned at metadata parse time.

### Why It Matters
- Several map setup paths consume `MapMetadata.world_min/world_max` before later runtime terrain fallback.
- Early playable bounds prevent transient fallback extents and remove noisy degenerate-bounds startup behavior.

### Validation
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- Quickstart smoke (`40s`, warn logs):
  - `reported degenerate bounds`: `0`
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`
- 9-map 20s sweep:
  - aggregate `degenerate bounds`: `0`
  - frame/fallback/texture failure counters remain `0`.

## 2026-03-02 W3D C-API UV-Set Selection Parity Update

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_c_api.rs`
  - FVF decode now selects UV set based on active draw stage `D3DTSS_TEXCOORDINDEX` when `TEXCOUNT > 1`.
  - Applied to:
    - `DrawPrimitiveUP` / `DrawIndexedPrimitiveUP` immediate decode,
    - staged stream decode helpers before declaration/overlay pass.

### Why It Matters
- Some legacy callers use multi-UV FVF streams and select non-zero UV channels through stage state.
- Previous behavior always consumed UV set 0 in FVF decode, causing stage-state UV routing drift.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- quickstart smoke remains clean (`begin/end_frame` failures `0`, fallback mesh `0`, missing texture `0`, degenerate-bounds warns `0`).

## 2026-03-02 W3D C-API FVF Texcoord-Dimension Parity Update

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_c_api.rs`
  - FVF decode now reads per-set texcoord dimensions from FVF texture-format bits (`D3DFVF_TEXTUREFORMAT*`) and advances decode offsets accordingly.
  - Combined with active-stage `TEXCOORDINDEX` routing, this now handles multi-set FVF streams with mixed texcoord dimensions more faithfully.

### Why It Matters
- Legacy fixed-function callers can use non-float2 texcoord sets.
- Hardcoded float2 decode can desynchronize vertex parsing and feed wrong UVs/states to draw submission.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- quickstart smoke remains clean:
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`
  - `reported degenerate bounds`: `0`

### Test Caveat
- `cargo test -p game_engine_device --features w3d` currently fails on an unrelated pre-existing parse error in `video/cpp_bindings.rs`; this blocks executing targeted new unit tests until that independent test issue is corrected.

## 2026-03-02 W3D C-API Declaration Stream Fidelity Update

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_c_api.rs`
  - Staged draw decode now executes a declaration-first path when a vertex declaration is active.
  - This closes a parity gap where stream 0 was implicitly treated as the base source even when declaration semantics place position on another stream.
  - New behavior reconstructs `W3D_VERTEX` from declaration usages:
    - required: `POSITION`/`POSITIONT`,
    - optional overlays: `NORMAL`, `COLOR0`, `TEXCOORD[n]` (active-stage usage index, fallback `TEXCOORD0`).
  - Added position decode support for common declaration encodings (`FLOAT2/3/4`, `SHORT2/4`, normalized short/ushort forms).

### Test-Lane Fix
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/video/cpp_bindings.rs`
  - Fixed pre-existing test parser issue in enum-cast comparison assertion.
  - This removes the previously reported unrelated blocker for targeted `game_engine_device` w3d tests.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo test -q -p game_engine_device --features w3d w3d_c_api::tests::declaration_stream_decode_uses_nonzero_position_stream` (pass)
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass, `0 unresolved blocker events`)
- quickstart smoke (`25s`, warn logs):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

### Cache Check
- `GeneralsRust/target`: `19G`.
- No cleanup run this pass (below heavy cache threshold).

## 2026-03-02 GameEngineDevice Robustness Follow-Up

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/examples/video_device_demo.rs`
- `GeneralsRust/Code/GameEngine/GameEngineDevice/examples/w3d_device_demo.rs`
  - Fixed example-build compile failures seen in full crate test runs by using `tracing_subscriber::fmt::init()` and correcting missing imports.
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/video/video_device.rs`
  - `Drop` path no longer uses runtime-blocking calls that panic under current-thread Tokio test contexts.
  - Teardown now performs non-blocking lock attempt on `initialized` and clears internal handle maps directly.

### Validation
- `cargo check -q -p game_engine_device --features w3d` (pass)
- `cargo test -q -p game_engine_device --features w3d w3d_c_api::tests::declaration_stream_decode_uses_nonzero_position_stream` (pass)
- `cargo test -q -p game_engine_device --features w3d` (still failing, but now on environment/assumption-heavy integration tests, not compile blockers):
  - macOS main-thread `winit` event loop requirement,
  - current-thread runtime assumption mismatch in `VideoDevice::clone`,
  - strict `w3d` init assertion mismatch on current environment.
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)
- quickstart smoke (`25s`, warn logs):
  - `begin_frame failed`: `0`
  - `end_frame failed`: `0`
  - `Using fallback mesh`: `0`
  - `No texture found for:`: `0`

## 2026-03-02 Device Init/Clone Parity Stabilization

### What Changed
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/lib.rs`
  - `init_video_device(...)` now calls `VideoDevice::init()` before storing/returning the device.
  - `init_w3d_device(...)` now calls `W3DDevice::init()` before storing/returning the device.
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/w3d/w3d_device.rs`
  - Added `init()` as the explicit headless/default initialization entrypoint.
  - `init_with_window(...)` now forwards to `init()` (window-independent parity path remains intact).
- `GeneralsRust/Code/GameEngine/GameEngineDevice/src/video/video_device.rs`
  - Replaced clone-by-recreation with structural clone of shared state (`Arc`/locks).
  - Prevents clone-time initialization reset and avoids platform-specific display re-enumeration side effects.

### Validation
- `cargo test -q -p game_engine_device --features w3d --test integration_tests`
  - `19 passed; 0 failed`.
- `RUSTFLAGS='-Awarnings' cargo check -q -p generals_main --all-features` (pass)
- `RUSTFLAGS='-Awarnings' cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)
- quickstart smoke (`25s`, warn logs):
  - `begin_frame_failed=0`
  - `end_frame_failed=0`
  - `fallback_mesh=0`
  - `missing_texture=0`
  - `degenerate_bounds=0`

### Current Read
- Device initialization status parity for integration tests is now closed.
- Immediate playability signals remain clean in this environment after the fix set.

## 2026-03-02 Mission Script Weather Visibility (`doWeather`) Parity

### What Changed
- `GeneralsRust/Code/Main/src/game_logic/mission_scripts.rs`
  - Added mission-script weather visibility queueing (`push_weather_visible` / `drain_weather_visibility_updates`).
  - Implemented `MissionScriptActionHandler::set_weather_visible` to forward `Show Weather` script actions into runtime hooks.
  - Added regression test `handler_forwards_weather_visibility_requests`.
- `GeneralsRust/Code/Main/src/game_logic/game_logic.rs`
  - Added runtime weather visibility state (`RuntimeWeatherState.visible`, default `true`).
  - Added `GameLogic::set_weather_visible(...)`.
  - Drains weather visibility requests in script evaluation and applies last-value-wins behavior per frame.
  - Added regression test `weather_visibility_script_requests_apply_last_value`.
- `GeneralsRust/Code/Main/src/save_load/snapshot.rs`
  - Extended `WeatherSnapshot` with `visible` and defaulting (`weather_visible_default`) for serialization compatibility.
  - Snapshot capture/restore now round-trips weather visibility.
  - Updated `snapshot_restore_preserves_weather_state` to assert visibility persistence.

### Validation
- `cargo test -q -p generals_main handler_forwards_weather_visibility_requests` (pass)
- `cargo test -q -p generals_main weather_visibility_script_requests_apply_last_value` (pass)
- `cargo test -q -p generals_main snapshot_restore_preserves_weather_state` (pass)
- `cargo test -q -p generals_main save_load::snapshot::tests::` (pass)
- `cargo check -q -p generals_main --all-features` (pass)
- `cargo run -q -p generals_main --bin playability_audit` (pass; `0 unresolved blocker events`)

## 2026-03-02 Renderer Runtime Parity Update (HLOD + Shadow)

### What Improved
- `ww3d-renderer-3d/rendering/hlod_system.rs`
  - `optimize_tree` is now active runtime logic (no longer a no-op).
  - `generate_impostors` now generates concrete far-distance billboard impostor meshes.
- `ww3d-renderer-3d/rendering/shadow_system/shadow_map.rs`
  - Shadow caster rendering now has a typed submission API that executes real draw calls (`draw`/`draw_indexed`) instead of inert placeholder flow.
- `ww3d-renderer-3d/rendering/shadow_system/shadow_renderer.rs`
  - Shadow map allocation now creates/stores real GPU map resources per light.
  - Shadow caster statistics are now driven by deterministic pass accounting (`shadow_caster_count_hint`) instead of hardcoded fake increments.

### Why This Matters For Playability
- HLOD and shadow lanes now have concrete runtime behavior in key places that previously masked parity gaps behind placeholders.
- Rendering telemetry from these systems is now grounded in actual submissions/resources, reducing false confidence from synthetic counters.

### Current Remaining Gaps (Still Not Final)
- `shadow_map::is_point_in_shadow` still uses CPU approximation (now bias/filter aware) rather than direct sampled depth-map comparison outside shader execution.
- Shadow submissions are now auto-captured in `WgpuMainRenderer`, but full light-space shadow pass raster integration still needs deeper end-to-end hookup so captured submissions directly drive concrete per-light map rendering.
- Broader full-game parity (UI/mission/campaign scripting edge cases, some C-API breadth, and subsystem integration depth) still requires continued batch closure.

### Validation
- `cargo test -q -p ww3d-renderer-3d hlod_system::tests:: -- --nocapture` -> PASS
- `cargo test -q -p ww3d-renderer-3d shadow_system::shadow_map::tests:: -- --nocapture` -> PASS
- `cargo test -q -p ww3d-renderer-3d shadow_system::shadow_renderer::tests:: -- --nocapture` -> PASS

## 2026-03-02 Thumbnail Cache Runtime Fidelity Update

### What Improved
- Thumbnail cache files now use explicit encoded payload modes (`RAW0` / `RLE1`) with deterministic decode validation.
- Runtime load checks now validate decompressed pixel payload against expected RGBA thumbnail size (`width * height * 4`) rather than compressed byte count.

### Why It Matters
- Prevents silent acceptance of malformed compressed thumbnail payloads.
- Enables actual thumbnail cache size reduction behavior while keeping deterministic round-trip correctness.

### Validation
- `cargo test -q -p ww3d-renderer-3d texture_system::texturethumbnail::tests:: -- --nocapture` -> PASS

## 2026-03-02 Scripting Runtime Parity Update

### What Is Good
- GameLogic script runtime now handles C++ sequential wait patterns more faithfully:
  - wait/retry script actions recheck on the next frame,
  - framecount actions wait before advancing to the next sequential instruction.
- Team-iterated one-shot script behavior now matches C++ for false-action paths (no one-shot deactivation there).
- `TEAM_GUARD_FOR_FRAMECOUNT` dispatch now mirrors C++ behavior.

### What Was Bad (Now Fixed)
- Sequential `Pending(1)` behavior effectively retriggered every two frames.
- Framecount pending actions replayed the same instruction instead of delaying next-instruction progress.
- Team-iterated false-action one-shot handling incorrectly deactivated scripts.

### Still Missing / Not Yet Perfect
- Full mission/campaign parity still needs continued closure across remaining scripting edge cases and subsystem integration paths.
- Multiplayer/network scope remains deferred until all non-network parity targets are closed.

### 2026-03-02 Script Install Parity Improvement
- Script runtime initialization now matches C++ behavior when script lists are installed:
  - delayed-evaluation scripts start with randomized initial evaluation offset,
  - condition team inference is precomputed from team-typed condition parameters.
- This reduces mission-script burst/evaluation cadence drift and improves per-team condition targeting consistency.

### 2026-03-02 Subroutine Execution Parity Improvement
- `CALL_SUBROUTINE` now executes against live scripts/groups, preserving one-shot deactivation and script runtime state across repeated calls.
- Subroutine group names are now resolved before script-name fallback, aligning runtime behavior with C++ mission scripting.

### 2026-03-02 Victory Elimination Parity Improvement
- `system/victory_conditions.rs` no longer has a hardcoded elimination placeholder.
- Elimination checks now honor C++ multiplayer defeat semantics (`NOUNITS`, `NOBUILDINGS`, or both) through explicit flags.
- Legacy auto-elimination now also consumes live runtime player census (`has_any_units`, `has_any_buildings_counts_for_victory`) when available.
- Added targeted regression tests for:
  - default combined flag behavior,
  - single-flag elimination behavior,
  - no-flag behavior,
  - census-driven player auto-elimination.

### 2026-03-02 Remaining Limitation (This Lane)
- Full end-to-end elimination parity in this lane still depends on consistent caller usage of the system-layer elimination path in all relevant game flows.
- Safety fallback remains intentionally non-destructive for entries missing runtime census.

### 2026-03-02 Startup Minimap Parity Improvement
- Startup minimap generation is no longer a no-op in `system/game_start.rs`.
- `MinimapGenerator::generate_from_heightmap` now emits deterministic terrain minimap pixels from loaded map height data with:
  - elevation normalization,
  - source-to-minimap resampling,
  - fixed-light terrain shading.
- `GameStartSequence::generate_minimap` now forwards source map dimensions to the generator.

### 2026-03-02 Remaining Limitation (Startup/UI Lane)
- Minimap terrain layer currently covers height-derived base shading only.
- Full parity overlays still need additional closure for richer static map-feature layers beyond roads (for example bridge/special-map annotation depth) and finer dynamic reveal detail fidelity.

### 2026-03-02 Terrain Roads Parity Improvement
- `GameClient/src/terrain/roads.rs` no longer forces road normals to world-up.
- Road geometry now computes tangent-based right/normal frames for:
  - road surface mesh,
  - edge strips,
  - marking strips.
- This improves sloped-road lighting parity and avoids degenerate-normal artifacts on steep segments.

### 2026-03-02 Remaining Limitation (Terrain Lane)
- Additional terrain-texture parity and higher-fidelity water shading (foam/shoreline material response depth) remain open.
