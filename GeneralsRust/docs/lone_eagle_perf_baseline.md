# Lone Eagle performance baseline

Captured during windowed `present_frame_gate` sit-through on Lone Eagle (America/Sao_Paulo local).

| mode | samples | ms/tick (approx) | avg present_ms | collect | terrain | forward | radar_pixels | terrain_chunks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| windowed present | 21 | 612.245 | 156.450 | 21.917 | 34.234 | 84.239 | 16384 | 462 |

## Hot spots

Present breakdown from `RenderPipeline::execute` residuals. Leading stage: **forward**.

## Notes

- Facility census cache: `facility_cache_gate` / `build_facility_template_names_cache_is_per_world_and_invalidates` (hq-wq7g6y).
- Radar terrain texture gated via `radar_pixels>0` (full 128×128 = 16384 non-black samples).
- Terrain source tiles hydrate through TerrainZH.big after heightmap hint install.
- Headless N-tick bench harness: `lone_eagle_bench` (requires Menu-settled Object admission before start).
