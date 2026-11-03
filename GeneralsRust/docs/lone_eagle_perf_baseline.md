# Lone Eagle performance baseline

Captured by `lone_eagle_bench` on the Mac playable worktree. Times are wall-clock over observed logic/present deltas after InGame settle.

| mode | ticks | frames | ms/tick | ms/frame | avg present_ms | collect | terrain | forward | radar_pixels | terrain_chunks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| windowed | 120 | 120 | 577.027 | 577.027 | 144.409 | 18.356 | 33.288 | 76.540 | 16384 | 743 |
| headless | 120 | 30 | 275.645 | 1102.582 | 173.931 | 19.957 | 34.191 | 102.416 | 16384 | 730 |

## Hot spots

Present breakdown comes from `RenderPipeline::execute` residuals (`present_collect_ms` / `present_terrain_ms` / `present_forward_ms`). Larger of the three is the leading GPU/CPU present cost.
- **windowed**: top present stages [("forward", 76.53979827117288), ("terrain", 33.28766583173703), ("collect", 18.355507868789662)]; sample=/Users/bernardoferrari/Downloads/.grok-worktrees/playable/GeneralsRust/Code/Main/../../docs/lone_eagle_bench_windowed_sample.txt
- **headless**: top present stages [("forward", 102.41564239017548), ("terrain", 34.19132168751335), ("collect", 19.956527521580824)]

## Notes

- Facility census cache: `facility_cache_gate` / `build_facility_template_names_cache_is_per_world_and_invalidates` (hq-wq7g6y).
- Radar terrain texture gated via `radar_pixels>0` (full 128×128 = 16384 non-black samples). Confirmed by `present_frame_gate` PASS.
- Terrain source tiles hydrate through TerrainZH.big after heightmap hint install.
- Leading present cost is **forward** (~77 ms windowed / ~102 ms headless debug). No further micro-opt applied this pass beyond the facility census cache; needs `samply`/`cargo flamegraph` for the next clear win.
- `present_frame_gate` PASS detail: terrain=462 models=39 ui=131 selection=1 radar_pixels=16384 present_ms≈189 (sit avg).

