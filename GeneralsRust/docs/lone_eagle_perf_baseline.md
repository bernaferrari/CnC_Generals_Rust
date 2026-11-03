# Lone Eagle perf baseline (Mac arm64, debug unless noted)

Updated: 2026-11-02 America/Sao_Paulo

## Before / after (windowed 1280x720, runtime host)

| Build | ms/tick | ms/frame (present_ms) | Notes |
| --- | ---: | ---: | --- |
| debug before | ~ (bench wall) | **~273–577** | Status samples during InGame; `present_forward_ms` ≈ 193–511 |
| debug after screenshot fix | *pending quiet-machine rerun* | *pending* | Capture interval 0 during bench; non-blocking PNG worker |
| release / release-with-debug | *not rerun* | *not rerun* | Disk pressure; deferred until debug is honest |

Prior `lone_eagle_bench` numbers that quoted ~577 ms/frame were **dominated by diagnostic PNG readback**, not by W3D draw.

## Profile evidence (`/usr/bin/sample`, InGame, before fix)

Main-thread Call graph under `ForwardPass::render` (~3241 samples):

| Samples | Share of ForwardPass | Symbol / path | C++ equivalent |
| ---: | ---: | --- | --- |
| ~2896 | **~89%** | `Engine::end_render` → `process_pending_screenshots` → `flush_pending_screenshot_readbacks` → `write_screenshot_png` | **None** — C++ W3D does not PNG-encode the backbuffer every present |
| ~35 | ~1% | `native_terrain_pass::record` / chunk visibility | `W3DTerrain` draw; C++ keeps GPU buffers, no per-frame mesh rebuild in steady state |
| (rest) | | wgpu encode / draws | DX8/W3D submit |

Root cause: runtime-host `gpui_frame` requested a capture every **40 ms** in interactive states, and `flush_pending_screenshot_readbacks` called `device.poll(wait_indefinitely)` then **synchronously** PNG-encoded 1280×720 on the present thread. That cost was attributed to `present_forward_ms`.

## Fixes landed on `origin/main`

1. **`Unseal: ww3d-engine — perf: non-blocking screenshot finalize`** (`b46793d15`)
   - Hot-path flush only `try_recv` + non-blocking `Poll`
   - RGBA convert + PNG encode on a `ww3d-screenshot` worker
   - Diagnostic host interactive interval **40 → 500 ms**
   - `lone_eagle_bench` uses `-gpui_capture_interval_ms=0`, one `request_capture` after the timed window
2. **`perf: Arc facility-name cache + O(objects) player census`** (`b67530f45`)
   - `build_facility_template_names` returns `Arc<HashSet<_>>` (no HashSet clone per access)
   - `owned_host_player_census` single pass over objects (was O(players × objects))
3. **`fidelity: retail terrain TGA candidates + on-screen PNG gates`** (`1d4f1d719`)
   - Always try `Art/Terrain/{class}.tga` (base `Terrain.big`)
   - Warn if `Terrain.big` missing
   - `present_frame_gate` samples on-screen minimap + terrain-view variance

## Next hotspots (not yet fixed)

Observed on a Loading-phase sample after the screenshot fix (load avg ~33, disk contention from other worktrees):

| Rank | Symptom | Likely path | C++ avoidance |
| ---: | --- | --- | --- |
| 1 | Boot / first present still multi-second | `image::codecs::tga::TgaDecoder` during terrain hydrate | Decode once, cache tile bitmaps (`TerrainTextureClass`) |
| 2 | ShellMap legacy sync ~100 s | `world_players` fast legacy runtime sync | Cached side/script tables; no full re-parse every boot under load |
| 3 | INFO spam | `ThingFactory loaded 1 object templates` × thousands | C++ is quiet; gate behind debug flag |
| 4 | Audio always-on | `kira` / `rodio` reverb/mixer | Disable/mix cheaper when `-noaudio` / headless |
| 5 | KindOf HashMap scans | `live_kind_of_cpp_mask` / host kind facts | Bitmasks / cached kind sets like C++ `KINDOF` |
| 6 | Draw-pose HashMap churn | `installed_draw_pose` / `NativeDrawOwnerId` | Stable handles; C++ drawables are pointer-stable |
| 7 | `GameWorldShadow::refresh_mapped_from_host` | per-frame observation rebuild | Dirty flags / partition notifications |
| 8 | `collect_extra_blend_tile_positions` | blend tile gather | Cache until heightmap dirty |
| 9 | Status PNG gates themselves | smoke harness | Already off during bench measure |
| 10 | First-frame pipeline/shader warm | wgpu/Metal | Accept once; do not count as steady-state |

## How to re-measure (quiet machine)

```bash
cd GeneralsRust
export LONE_EAGLE_BENCH_TICKS=120 LONE_EAGLE_BENCH_FRAMES=120 LONE_EAGLE_BENCH_TIMEOUT_SECS=900
./target/debug/lone_eagle_bench
# optional: /usr/bin/sample <generals-pid> 20 -file /tmp/generals_sample.txt
# release later: cargo build -p generals_main --bin generals --bin lone_eagle_bench --release -j 4
```

Confirm `write_screenshot_png` is **absent** from InGame Call graph after the fix.

## Hashes

```
git log origin/main -3 --oneline
1d4f1d719 fidelity: retail terrain TGA candidates + on-screen PNG gates
b67530f45 perf: Arc facility-name cache + O(objects) player census
b46793d15 Unseal: ww3d-engine — perf: non-blocking screenshot finalize
```
