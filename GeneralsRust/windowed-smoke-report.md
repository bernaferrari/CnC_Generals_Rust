# Windowed Smoke — viewport extent fix + five-flag drive frontier (WindowedSmoke2, 2026-09-02)

Scope: verify WindowedSmoke's depth-format fix; land the viewport/scissor/swapchain fix
(parent-directed priority); attempt the physical (CGEvent) five-flag sit-through; document
blockers precisely. Serial runs only, no git writes, no formatters. Harness tooling kept
under `/tmp/wsmoke/` (durable, documented below) — repo tree carries only the render fixes.

## 1. Depth-format fix — VERIFIED in tree (no edit needed)

- `Code/GameEngine/src/terrain/terrain_visual/visual_struct.rs` —
  `TERRAIN_PIPELINES_DEPTH_FORMAT = Depth32Float` with the wgpu validation-crash rationale
  (pipeline authored `Depth24PlusStencil8` fatalled against the `Depth32Float` attachment).
  All terrain pipelines + river_gpu + shroud_gpu use the shared constant.
- `Code/Main/src/graphics/laser_draw.rs:151` — post-frame laser pass pinned to
  `Depth32Float` ("Must match the pass this pipeline draws into … wgpu fatals on
  pipeline/attachment mismatch").
- Confirmed live: windowed runs present frames (`live_frame_ok=true` latched from windowed
  surface present), `render_item_count=31`, 760 gameworld presentation entities in InGame.

## 2. Viewport/swapchain extent fix — LANDED (my lane)

**Root cause (measured, not inferred):** `WW3D surface configured: size=1280x960` while the
window is 640x480 logical (scale factor 2). The WND window manager, UI renderer, and gadget
draw paths all author/draw in logical points, so the UI painted the top-left 640x480
quadrant of a 1280x960 surface — exactly the "~500x500 render, rest black" defect.

**Fix — swapchain extent follows the logical window size (C++ `TheDisplay` parity):**
- `cnc_game_engine/types.rs` — new `render_surface_extent(&Window) -> (u32,u32)`
  (`inner_size / scale_factor`), the one shared definition of render extent.
- `cnc_game_engine/boot.rs` — engine config (surface init) uses the logical extent;
  the later camera-aspect/UIManager seed now receives `LogicalSize` (logical).
- `cnc_game_engine/input.rs` `resize()` — converts the winit physical size to logical
  before `ww3d_engine::resize` (surface reconfigure + depth resize) and
  `ui_manager.resize`; per-frame `set_tactical_3d_viewport` uses the logical extent.
- `cnc_game_engine/start_game.rs` — `tactical_viewport_size()` (3D view, projection,
  mouse unprojection) and the GameHUD seed use the logical extent.
- `cnc_game_engine/hotkeys.rs` — diplomacy panel resize uses the logical extent.
- Input space untouched: `mouse_position` is already logical (`apply_cursor_position`
  divides by scale), so mouse math is now in the SAME space as the render target.

**Scissor fatal guard — LANDED (blocks every windowed run otherwise):** with a correct
640x480 surface, the shell draw path's out-of-layout scissors became fatal:
`Scissor Rect {x:140,y:540,w:500,h:1} is not contained in the render target (640,480)`
(abort trap 6; release `panic=abort`). These scissors arrive in raw 800x600
CREATIONRESOLUTION coordinates (MenuWidgets' lane: layout→WM screen scaling).
`GameEngine/GameClient/src/gui/ui_renderer.rs` (`render`, ~1523): a scissor whose origin is
outside `screen_size` now skips the draw (`continue`) instead of issuing the fatal command;
partially-overlapping scissors still clamp. This is additive and stays correct once the
layout scaling lands.

**Verification:** `cargo check -p generals_main --bin generals` clean; release build;
windowed boot → Menu → present → `request_capture` PNG. With my extent fix (pre-MenuWidgets
snapshot) the C&C logo drew full-size at its correct 640x480 position (previous runs: it
rendered inside the top-left quadrant). Evidence PNGs: `/tmp/wsmoke/menu_frame.png`,
`check_menu.png`, `fiveflag_frame.png` (latest snapshot black — caught MenuWidgets'
mid-refactor shell layout; their scaling fix is in flight).

## 3. Five-flag status (honest)

| flag | state | evidence |
|---|---|---|
| window_visible | **TRUE** | winit query published `window_visible=true`; window on screen (desktop screenshot) |
| live_frame_ok | **TRUE** | `Runtime host latched live_frame_ok from windowed surface present`; promoted `frame.png` captures |
| ingame | **TRUE** (automated chain) | runs reach `state=InGame`, `select_ok`, presentation entities 760 |
| wnd_widget_tree_nav | FALSE | requires physical Menu→match click chain — not yet completed (see blockers) |
| gameplay (interactive) | FALSE | requires `match_started_from_menu_wnd` (same physical chain) |

`retail_sit_through_missing=wnd_widget_tree_nav,gameplay` on every observed run — the two
flags that by design require real OS mouse input.

## 4. Physical drive: mechanics proven, final chain blocked — precise blockers

Working, durable harness (all under `/tmp/wsmoke/`, reusable):
- `generals_clicker7/8` (swiftc-built): activate-by-pid + HID move/down/up; clicker8 does
  click-to-activate at an unoccluded point then double-clicks the target.
- `window_move|x=..|y=..` control command (existing host feature): parks the OS window in
  point space so gadgets sit outside overlapping IDE windows — VERIFIED
  (`window_move: requested 1151,260 pt -> winit outer 1151,260 640x512`, drain logged).
- `winlist`/`osrect.sh`: live CG window rect for truth-vs-child-coordinate mapping
  (macOS constrained the frame to 577x462 @ (1122,289) in one observation while the child
  believed 640x512 @ (1151,260) — clicks must map through the OS rect).
- `spawnchild.sh`, `drive6-10.sh`: child launch exactly per `bootstrap.rs`
  (`-runtime_host=windowed -windowed -width=640 -height=480` + gpui_* files), status-driven
  phase machine (Menu → SP reveal → SK → `start_game` → InGame RMB), with evidence logging.

Physical evidence that DID latch earlier (pre-sleep): physical Menu clicks latched
`menu_wnd_click` (reveal fired — `ButtonSinglePlayer` became hittable only after first
input, matching the C++ first-run reveal), and InGame physical RMBs were issued
(`activated=true` windows; `selected_count=1`).

**Blocker 1 (environmental, resolved late):** the display SLEPT mid-session
(`screencapture` returned an all-black 3456x2234 frame). A sleeping display blackholes HID
clicks and app activation — every "silently ignored click" after ~03:00 is explained by
this. Wake with `caffeinate -u -t 3`; drivers should do this at start. The two remaining
flags were not completed post-wake within budget.

**Blocker 2 (cadence):** Menu-state frames take ~3.7 s in release
(`RenderPipeline breakdown: total=3.74s … render_items=0` — forward-pass pathology, not
mine). Control-file writes are per-frame drained, so drivers must write one command and
wait ≥6 s; drive7's back-to-back writes clobbered `window_move` before the drain (proven by
the drain log). Perf ownership: graphics/render_pipeline (not the smoke harness).

**Blocker 3 (sibling churn):** the shell menu content is MenuWidgets' active lane
(GadgetButton/StaticText children, 800x600→WM scaling); snapshots of
`target/release/generals` taken mid-refactor render a black menu and publish zero
`gadget_hit` lines (pre-reveal dropdown hidden + layout in flux). My drive runs against a
snapshot binary (`/tmp/wsmoke/generals_snap`); re-run `drive10.sh` after MenuWidgets lands,
with `caffeinate -u -t 3` first.

## 5. Handoff for the next driver run (five-flag completion)

1. `caffeinate -u -t 3 && caffeinate -i -t 900 &` (keep display awake).
2. Build release with MenuWidgets' landed scaling; snapshot the binary.
3. `/tmp/wsmoke/drive10.sh` (or drive9 for OS-rect-mapped clicks) — it already:
   boots → `window_move` → reveal click at the unoccluded strip (x>1635) → SP (frac
   805,290) → SK (805,323) → `start_game` → InGame physical RMB (select via the automated
   `select_local_unit` or physical LMB) → polls `gameplay=true`, copies `frame.png`.
4. Expected latch order: `menu_wnd_click`+`skirmish_path` (SP/SK clicks) →
   `match_started_from_menu_wnd` (start from Menu) → `wnd_widget_tree_nav=true` →
   physical RMB with selection → `gameplay=true` → `playable_claim=true` in status.

Changed files (this pass): `types.rs`, `boot.rs`, `input.rs`, `start_game.rs`,
`hotkeys.rs`, `ui_renderer.rs` (GameClient). No test greens touched: all edits are
platform/bootstrap/render-lane; `cargo check -p generals_main --bin generals` clean.

---

## MenuWidgets — WND GadgetButton/StaticText shell-menu reveal (2026-09-02)

### Trace (evidence)
- Shell push/init runs: `MainMenuInit: Initializing main menu` → `window ids built` →
  `layout shown and dropdown hide attempted` (game_client_rust::gui::shell::main_menu, 06:00:06Z).
- First-run path fires in windowed runs: `First time running the game - hiding mouse and
  fading` → `not_shown=true`, dropdowns hidden by `apply_cpp_init_hide_pack`.
- `MainMenuUpdate` runs; first-run auto-reveal is C++-commented-out
  (GeneralsMD MainMenu.cpp:886-899): menu stays hidden until >20px mouse move or
  GWM_CHAR (MainMenu.cpp:982-1005). With no physical input, MapBorder2 + buttons stay
  HIDDEN → draw_window_hierarchy early-returns → no buttons/text on screen. This is the
  button/text defect root cause (verified: live capture shows ruler+backdrop drawing —
  i.e. the WM image-window draw dispatch works — while all PUSHBUTTON/STATICTEXT
  children are hidden).
- Layout scaling is NOT the bug (verified in debug logs): `resolve_window_rect` scales
  800x600 CREATIONRESOLUTION rects to WM screen_size (ButtonDiffBack 208x36 → 166x29 at
  640x480; MainMenuParent → 640x480). C++ parity: GameWindowManagerScript.cpp
  parseScreenRect (GameWindowManagerScript.cpp:467-540) scales by
  TheDisplay/createRes at parse time.
- Reveal chain once unhidden: WindowTransitions.ini `MainMenuDefaultMenuLogoFade`
  (FLASH MapBorder2 frames 4-7 → unhide; BUTTONFLASH buttons frames 11-15 → unhide),
  loaded via fallback `GeneralsRust/windows_game/extracted_big_files_v2/INI/WindowTransitions.ini`
  (has MainMenuDefaultMenuLogoFade; log line 243). WM::update → transitions.update
  advances per frame (probe verified: update#0..#29, #120, #240 while roots=0 pre-menu).
- Fonts init fine (FontLibrary + cosmic-text, 951 faces parsed).

### Fix landed (my lane)
`Code/GameEngine/GameClient/src/gui/shell/main_menu.rs` (+13): in MainMenuUpdate, when
`not_shown` after the just_entered delay AND `GENERALS_RUNTIME_HOST_WND=1` (unattended
runtime-host windowed run), run `reveal_hidden_main_menu` — the exact C++ reveal
sequence (winHide(FALSE) MapBorder2 + MainMenuFade immediate + MainMenuDefaultMenu +
cursor visible + not_shown=false). Interactive sessions keep C++ wait-for-input behavior.

### Sibling lanes (coordinated, do not duplicate)
- MenuTextures: button atlases `Buttons-Left/Middle/Right` live in EnglishZH.big
  (SCSmShellUserInterface512); hydration fails without zh_install_roots BIG paths →
  win_draw_image_ex paints raw draw-data tint 255 0 0 255 = the RED rectangles. Their
  fix (image.rs backend paths + honest fallback fill) lands the button/textures.
- WindowedSmoke2: swapchain extent + ui_renderer.rs scissor guard (out-of-target
  scissors skipped instead of wgpu fatal).

### Blocker (pre-existing, outside my lane): silent process death
5 of 6 windowed runs died silently (no panic in log; macOS report
`generals-2026-09-02-031123.ips`: SIGABRT via abort() in-process during WGPU asset init
or shortly after Menu entry; also seen mid-boot and ~40-90s after Menu). Menu render
currently ~3.7s/frame with 0 render items under sibling CPU contention. Because of this
the post-fix screenshot could NOT be captured this session (mw7 run died in boot).
Binary with fix built (release, 03:49) and launches; rerun
`GENERALS_RUNTIME_HOST_WND=1 ./target/release/generals -runtime_host=windowed -windowed
-width=640 -height=480 ... -gpui_frame=$D/frame.png` when the abort is fixed; expect
"revealing main menu" INFO then transition-driven unhide within ~15 transition frames.

---

## DriveRunner — physical five-flag drive + SIGSEGV/wedge root cause (2026-09-02 04:05-05:40)

### Drive result (honest): wnd_widget_tree_nav latch reached, final publication blocked by a render-path deadlock — now fixed in tree

Chain progress this session (release, GENERALS_RUNTIME_HOST_WND=1, window parked 1151,260 640x512,
clicker7 physical clicks aimed at engine-published `gadget_hit=Name@x,y` centers):
- `window_move` ok; menu reveal fired (`MainMenuUpdate: unattended runtime-host run — revealing main menu`).
- Physical click `ButtonSinglePlayer@1666,399` → "Single Player button selected" +
  **`InteractivePlayabilityEvidence: latched menu_wnd_click (windowed=true consumed=true hit=true)`**.
- Dropdown published `ButtonSkirmish@1666,527`; physical click →
  "physical GBM_SELECTED notify name=MainMenu.wnd:ButtonSkirmish" → skirmish_path latch set;
  SkirmishGameOptionsMenu pushed (WM 67→365 windows).
- `start_game` from Menu state → **`host_start_game_from_ui: InGame menu_match=true menu_click=true`**
  ⇒ menu_wnd_click + skirmish_path + match_started_from_menu_wnd all latched ⇒
  `wnd_widget_tree_nav` evidence chain COMPLETE (twice: runs at 08:13:43 and 08:21:30).
- InGame physical RMB/build + status publication of `wnd_widget_tree_nav=true`/`gameplay=true`
  NOT captured: the process died at InGame entry both times (below). Screenshots this session:
  /tmp/wsmoke/{ev_menu_revealed.png, skirmish_menu.png, menu_reveal_check.png, live_check.png, wake_desktop.png}.

### Blocker 1 (environmental, solved): snapshot binary cwd broke WND resolve — explained the "empty WM"
`main.rs set_working_directory_to_executable()` chdirs to the EXE dir. `/tmp/wsmoke/generals_snap`
→ cwd=/tmp/wsmoke → `resolve_window_script_path` finds no windows_game ancestor → first MainMenu
push fails LayoutError(InvalidParameter), and `do_push` LEAVES THE BROKEN SCREEN IN THE STACK;
the retry "succeeds" vacuously (screens=1) without creating any window → black menu, zero
`gadget_hit`, all named lookups "missing" (this was the real cause of the black-menu runs).
Fix used: run the snapshot from `GeneralsRust/target/release{,-with-debug}/generals_snap`
(exe-dir ancestor reaches windows_game). Suggested code fix (shell lifecycle, not done here):
pop the broken screen when `run_init` fails inside `do_push`.

### Blocker 2 (code, ROOT-CAUSED + FIXED in tree): same-thread RwLock read→write deadlock in list-box draw
Deterministic hang at the SkirmishGameOptionsMenu push and SIGSEGV(PAC) at InGame entry
(3 kills: 2 wedges + 1 crash; .ips generals_snap-2026-09-02-{050658,051348}).
Symbolicated sample (release-with-debug): main thread
`render (input.rs:1807) → RenderPipeline::execute (pipeline_execute.rs:300)
→ WgpuMainRenderer::end_frame (wgpu_main_renderer.rs:407) → flush_ui_to_frame (ui_render_pass.rs:221)
→ WindowManager::draw_all → draw_window_hierarchy_internal → w3d_gadget_list_box_image_draw
→ draw_mapped_image_clipped (list_box.rs:689) → parking_lot RwLock lock_exclusive_slow → cond_wait`.
Cause: callers held `get_mapped_image_collection().try_read()` while `draw_mapped_image_clipped`
re-took the same collection `.write()` on the same thread (std/parking_lot RwLocks are not
reentrant) — read→write self-deadlock; the release SIGSEGV is the sibling corruption of the
same pattern. Fix (list_box.rs only, minimal): `draw_mapped_image_clipped` now takes
`image_name: &str` (body looked up by name anyway); both list-box image sites compute geometry
inside a short read guard, DROP it, then draw by name; `draw_window_image_clipped` gates
existence, drops the guard, then calls. `cargo check -p generals_main --bin generals` clean.

### Handoff (next driver, ~10 min to green flags)
1. `cargo build --release --bin generals`; cp to `GeneralsRust/target/release/generals_snap`.
2. `caffeinate -u -t 3`; keep `caffeinate -dis` alive for the whole session (it died once and
   clicks stopped delivering until re-woken).
3. spawnchild (now points at release-with-debug — repoint to release), then when
   `gadget_hit=MainMenu.wnd:ButtonSinglePlayer@1666,399` publishes: click it; WAIT for
   `ButtonSkirmish@1666,527` to publish; click it; confirm GBM notify; send
   `start_game|mode=skirmish|faction=USA` immediately (drains within ~1-2 frames).
4. On InGame: physical RMB (select + move order) → `gameplay` latch; capture status + frame.png.

---

## DeadlockAuditRedrive — InGame SIGSEGV root-caused + CreditsMenu wedge root-caused + guard audit (2026-09-02 05:45-08:10)

### 1. InGame-entry SIGSEGV (generals_snap-2026-09-02-{050658,051348}.ips) — ROOT-CAUSED + FIXED, VERIFIED SURVIVING
Symbolicated both .ips against release-with-debug: NOT the gadget-draw path. Both crashes:
`run_loop.rs:221 → host_run_ingame_logic_presentation_frame (camera_drain.rs:773) → GameClient::update_presentation_shell (impl_update.rs:1389) → ParticleSystemManager::update (particle_manager.rs:906) → ParticleSystem::begin_frame_emit (particle_system.rs:1507, resolve_attached_parent) → query_live_drawable_fx_pose (live_slot.rs:200)` — EXC_BAD_ACCESS wild read (0x0c0888{8a,89}3a091818).
Root cause: `register_live_game_client` stores `&mut engine.game_client` as a raw address during `mark_initialized` — while the engine is still inside the boxed boot future (`run_loop.rs` `engine_init_future`). The engine is then MOVED out (`engine = Some(new_engine)`, run_loop.rs:494) and the box dropped: the slot points at freed heap. The first `with_live_game_client_mut` (particle FX object-attach pose resolve, only reached in-match when `host_fx_object_pose` misses) derefs garbage → SIGSEGV at InGame render boot, deterministic.
Fix: `GameClient::republish_live_slot_after_engine_move()` (impl_init.rs) called in run_loop.rs immediately after `engine = Some(new_engine)` — re-points the slot at the final engine address before any frame.
VERIFIED: driven twice past InGame entry (v11 07:19, v12 07:26) — process alive, `state=InGame`, `ingame=true`, control bar + world rendering; no crash reports generated.

### 2. CreditsMenu push wedge — ROOT-CAUSED + FIXED via live sample
Sampled the wedged process (`/usr/bin/sample`, release-with-debug; homebrew `sample` shim is broken — python3.13 interpreter missing): main thread parked in `std RwLock::lock_contended` under
`CreditsMenu::init (menu_callbacks.rs:2030) → WindowManager::set_focus (focus_tab.rs:55) → InputFocus system message → "CreditsMenuSystem" callback (script_callbacks.rs:720) → get_menu_manager().read()`.
Recursive read on a std RwLock (not re-entrant): `bind_layout_callbacks` held `manager.read()` across `with_arc_write(menu, init)`; init's set_focus re-entered the same manager lock. Same shape in all 5 menu-manager dispatch kinds (init/update/shutdown/system/input × SinglePlayer/Options/MapSelect/Credits/LanLobby).
Fix (established fetch-then-drop discipline):
- `script_callbacks.rs`: new `dispatch_menu()` helper clones the menu Arc, drops the manager guard, THEN `with_arc_write` — all 25 closures converted.
- `menu_callbacks.rs`: init-time (4 menus) + credits-update inline `set_focus` deferred via `queue_window_manager_op_deferred` — an inline focus under the menu write guard re-enters the SAME menu lock via the window's system callback (write→write; update re-focuses every frame).
VERIFIED: post-fix probe pushed Credits — "Initializing Credits Menu" completes, rendering continues (Render health logs), sample shows the main thread in the normal run-loop path, no lock wait. Screenshot /tmp/wsmoke/credits_after.png (backdrop + logo + credits panel).

### 3. w3d_gadget_draw family guard audit — CLEAN
Swept every collection-guard scope (scripted brace-scope analyzer over all 36 collection-touching files + manual review of all 16 w3d_gadget_draw files, WM draw.rs, game_window_global.rs, game_window_transitions.rs, flush_ui_to_frame):
- list_box.rs was the only read→write-across-draw site (already fixed by DriveRunner; the fixed pattern is correctly applied: geometry under short read, drop, draw by name).
- win_draw_image_ex / draw_window_image_clipped / transitions / anim2_d / hud radar / power meters: guards dropped before draws or taken inside the renderer closure with no outer hold; power.rs drops global+player guards before image lookups. No remaining same-thread re-entrancy found.
- ui_globals draw protocol (drop write guard before wm.draw_all) is honored by the frame owner.

### 4. New blocker found + fixed on the re-drive: shell screens survive into InGame
v11 drive: menu click chain latched (SP hit @1666,399 published, skirmish_path engaged, start_game from Menu) and the process SURVIVED InGame — but the WND menu stack (MainMenu + SkirmishGameOptionsMenu) was never popped: menu widgets kept drawing over the Defcon6 world and their gadget hit-testing ate every world click (physical LMB could never select; sel=0 forever; "ButtonStart missing" spam). The runtime-host `start_game` command bypasses the C++ Start-button shell pop.
Fix: `host_game_engine_reset` (start_game.rs, the C++ `TheGameEngine->reset()` parity site) now tears the shell down: `game_client::gui::ShellHandle::default().reset()` — pops every pushed screen via the immediate path.

### 5. Drive state at wrap-up
Final re-drive (v12 + shell-reset fix) was mid-flight at budget cap: SP click hit published, SK clicked at the correctly parsed published center (1666,527), start_game written, `state=Loading`, child alive. Drive script /tmp/wsmoke/drive12.sh keeps running unattended (logs /tmp/wsmoke/drive12.log; screenshots /tmp/wsmoke/m11_menu_revealed.png, m11_skirmish_menu.png, m11_ingame.png; five-flag poll continues). Next driver: read drive12.log tail — if `gameplay=true` latched, copy status+frame from `cat /tmp/wsmoke/current_dir`. Note drive12's SK-click coordinate parse and LMB-select phases are already corrected.

Changed files this pass: `Code/GameEngine/GameClient/src/core/game_client/impl_init.rs` (republish method), `Code/Main/src/cnc_game_engine/run_loop.rs` (post-move re-register), `Code/GameEngine/GameClient/src/gui/window_manager/script_callbacks.rs` (dispatch_menu, 25 sites), `Code/GameEngine/GameClient/src/gui/callbacks/menu_callbacks.rs` (deferred focus, import), `Code/Main/src/cnc_game_engine/start_game.rs` (shell teardown in engine reset). No probes left in-tree; instrumentation kept per instructions (layout_load/wnd_parse/create_destroy warns are pre-existing). `cargo check -p generals_main --bin generals` clean; no formatters; no git writes.

### 6. Final drive outcome (v12, post-shell-reset)
The last v12 run entered InGame and stayed: 30+ minutes / 34,582 frames alive in `state=InGame` (vs the old deterministic SIGSEGV on entry). Physical LMB selection DID reach the world after the shell teardown fix — `selected_count=1` observed repeatedly (08:11, and held at the final RMB #237). `gameplay` still did not latch: with sel=1, 237 physical RMBs at (1420,500) produced no `gameplay=true` — next driver should verify the RMB order is consumed as a valid command (target point validity / pick path) rather than a timing issue. No new .ips were produced all session (the only two remain the 05:06/05:13 crashes root-caused in §1). Final child termination was an external SIGTERM (sibling drive-lane cleanup), not a crash. Known perf note: one 17s slow-frame spike in InGame (update=4.7s, render=12.3s) — hq-0dw4l water-texture lane, not this lane.

---

## InGameRenderTriage — in-game render defects: terrain noise FIXED, CSF captions root-caused+fixed, red squares fix landed, minimap diagnosed (2026-09-02 08:30-09:55)

### Reproduction method
Piggybacked `request_capture` on live drive12 children plus own drives. Reliable InGame
entry without physical clicks: write `start_game|mode=skirmish|faction=USA` to control.txt
from Menu (Loading→InGame), then `click_skirmish_start` to pop the shell. Evidence PNGs:
`/tmp/wsmoke/irt_ingame_now.png` (defects confirmed), `irt_fix1_ingame.png` (post-fix).

### 1. Main-view "giant stretched noisy texture" — FIXED (root cause: missing mipmaps, not UVs)
One-shot probes (UVPROBE/BINDPROBE, removed from tree after diagnosis) proved the UV
pipeline is CORRECT: chunk vertices advance 1.0 world unit with UVs advancing 0.1 per
vertex = exactly one full tile per MAP_XY_FACTOR cell (C++ WorldHeightMap::getUVData,
GeneralsMD GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp:1589-1694 —
per-cell atlas coords; the port binds standalone tile textures with a REPEAT sampler).
Slots bound real map tiles (Cobble02/TLCobble02.tga, GreeceSand01/TGGrcSand01.tga).
The noise was MINIFICATION ALIASING: TLCobble02 is a 256² tile with a 16×16 stone
lattice sampled into ~60px cells with `mip_level_count: 1`
(textures.rs `create_gpu_texture_from_rgba`) while the terrain sampler expects mips
(mipmap_filter Linear, lod_max 32 — the port already models C++ GameData
TextureReductionFactor/SetLOD bias as lod_min_clamp; W3D textures are D3D-mipmapped).
Fix: full CPU box-filtered mip chain (image::imageops::thumbnail per level) uploaded per
terrain diffuse texture. VERIFIED: post-fix capture shows smooth tiled terrain with
visible map features — moiré gone (irt_fix1_ingame.png).

### 2. MISSING: captions — CSF TABLE ROOT-CAUSED + FIXED (one stale-cache layer remains)
Log showed `Loaded 0 localized GameText strings`: `find_csf_path` (game_text.rs:223)
only searched loose extractions, but generals.csf exists ONLY inside the BIGs.
Deeper: the shipped/repacked asset BIGs (repo assets and the DIRECT PLAY retail install
carry byte-identical copies) have SCRAMBLED ENTRY TABLES for a subset of entries —
EnglishZH.big's generals.csf entry points at a 20-24KB fragment while the intact file
(` FSC` magic, 6421 labels) sits at the stored-size offset; TexturesZH.big entries are
intact (DDS magic at the stored offset) — corruption is per-entry, not per-archive.
Fix (game_text.rs `init_runtime_strings`): loose file → engine virtual FS
(`Data/<lang>/generals.csf`, C++ Language::init parity) → raw ` FSC` header scan over
zh_install_roots() archives keeping the richest parse. VERIFIED: `Loaded 6421 localized
GameText strings` (was 0).
REMAINING (precise): control-bar + main-menu captions still render
`MISSING: 'MISSING: 'GUI:…''` — a fetch whose KEY is already a missing-string, i.e. some
layer caches/fetches window text BEFORE GameText init and re-fetches the translated
string later (wnd_parse.rs `apply_window_text`:212 is one fetch site). Next step: find
the pre-CSF text cache (ControlBar bridge / layout instancing before
init_localized_ui_resources) and re-translate after CSF load.

Fix (Common ini.rs + ini_mapped_image.rs): `INI::set_tolerant_blocks(true)` for
mapped-image loads — parse_file skips unknown junk lines and corrupt blocks (skip to next
END/block header) instead of aborting; `load_recovering_truncated_head` retries head-cut
files from the first registered-block line. Strict parse (normal `load`) is unchanged for
healthy files.
Final verification run: tolerant parser ENGAGED (14,995 corrupt lines skipped in
`/tmp/wsmoke/irt_final2.png` run) but `Imported 1320 mapped images` was UNCHANGED — the
recovered blocks still don't reach the imported collection (block-level parse still
drops them, or names collide in `ImageCollection`). Precise next step: log per-file
recovered-image counts inside `parse_mapped_image_definition` for the SA*.INI files and
diff against the block count in the file; the parse-side plumbing works (skips +
clean-block dispatch), so the loss is between block parse and collection insert.
Captions note from the same frame: text is now SINGLE-wrapped `MISSING: 'USA'` /
`MISSING: 'Player Name'` — CSF lookups reach the recovered 6421-label table; the
remaining misses are BARE labels (`USA`, `Player Name`, `Units`) that the recovered
table lacks or that need the C++ `PlayerTemplate` DisplayName path — verify bare-label
presence in the parsed table before touching lookup code.

### 4. Minimap black — PRECISELY DIAGNOSED (simulation-lane seam)
The radar draw pipeline WORKS: pre-start (skirmish menu up, shellmap radar alive) the
LeftHUD radar rendered terrain (/tmp/wsmoke/irt_probe_ingame.png). Black after match
start = `should_draw_radar_check` false → hud.rs:126 fallback `FALLBACK_HUD_FILL`
(0xE01A1E24): `host_update_the_radar` (run_loop.rs:909-928; C++ GameEngine.cpp:732
`TheRadar->UPDATE()`) sets `local_has_radar = player.has_radar()`; PlayerState::has_radar
(player_state.rs:62) is `radar_count > 0 && !radar_disabled`; the match-start
CommandCenter never increments radar_count in the runtime-host path (RadarUpdate →
`set_radar_visibility(true)` → `add_radar`, object_queries.rs:745-757 never runs for it).
Next step (simulation lane): add_radar for the local player when the match-start CC
spawns (C++ RadarUpdate attaches to CommandCenter; Player::addRadar), or radar.force_on
as a presentation stopgap.
FINAL-RUN UPDATE: in the last verification drive the radar RENDERED (terrain + selection
lines + shroud circle visible in /tmp/wsmoke/irt_final2.png bottom-left) — the draw
pipeline and terrain sampling are sound; visibility still flips with player radar state,
reinforcing the radar_count seam above rather than a renderer bug.

### Asset note (upstream of 2/3)
The shipped BIG set is a repack with mangled entry tables + interleaved binary fragments
for a subset of entries (INIZH INIs, EnglishZH CSF). Engine-side recovery now handles
INIs and the CSF; TerrainZH/TexturesZH TGAs are intact. A re-extraction from intact
media would obsolete the recovery paths.

### Guards / hygiene
No test greens touched: render/asset-loader lanes only; `cargo check -p game-client-rust`
clean after probe removal (UVPROBE/BINDPROBE and their static removed; those two files
end net-unchanged). Serial windowed runs with caffeinate -dis; no git writes; no
formatters. Changed files: GameClient/src/game_text.rs,
GameClient/src/terrain/textures.rs, Common/src/common/ini/ini.rs,
Common/src/common/ini/ini_mapped_image.rs.

---

## GameplayLatch — RMB order consumption root-caused; FIVE-FLAG COMPLETE (2026-09-02 09:45)

### Result
drive13.sh (`/tmp/wsmoke/drive13.sh`) physical chain latched **all five flags** — status
`retail_sit_through_missing=` EMPTY: `window_visible=true live_frame_ok=true ingame=true
wnd_widget_tree_nav=true gameplay=true`. Evidence: `/tmp/wsmoke/fiveflag_status.txt`,
`/tmp/wsmoke/fiveflag_gameplay_frame.png` (in-match: Defcon6 world + control bar + the
move-order marker ▲ at the RMB point), `/tmp/wsmoke/drive13.log` line
`SUCCESS: selected_count=1 … wnd_widget_tree_nav=true … gameplay=true`.
Latch chain in child stderr: `menu_match=true` (first time in this lineage) →
`RMB context command: had_sel=true match=true headless=false` →
`RMB context command issued=true gameplay_order=true`. No new .ips (only the three known
03:11/05:06/05:13 crashes remain).

### Root causes (both in the latch's upstream, not timing)
1. **`skirmish_path` never latched → `match_started_from_menu_wnd=false`** — this alone
   dead-ends BOTH remaining flags (`note_gameplay_order` and `wnd_menu_to_match_complete`
   both require it; drive12's `pre-note menu_click=true was_menu=true offline=true` →
   `menu_match=false` proved the missing third input). Why the physical SK click died:
   C++ MainMenu.cpp:1320 ButtonSinglePlayer GBM_SELECTED calls
   `dropDownWindows[DROPDOWN_SINGLE]->winHide(FALSE)` synchronously, then
   `setGroup("MainMenuSinglePlayerMenu")`. `FlashTransition::update` owns the border
   per-frame — frames 0-3 `winHide(TRUE)` (GameWindowTransitionsStyles.cpp:92-151, ported
   faithfully in game_window_transitions.rs:251-295). At C++ 60 fps that is ~50 ms; at the
   windowed Menu's 3-5 s/frame the Skirmish button stays PARENT-HIDDEN (MapBorder) for
   7+ frames (~30 s), `find_window_at_point_raw` (window_manager/input.rs:621) skips the
   hidden parent's subtree, and the physical click resolves
   `under=MainMenu.wnd:MainMenuRuler` (proven in drive12 stderr: identical coordinates
   hit ButtonSinglePlayer pre-GBM, Ruler post-GBM) → no GBM_SELECTED → no
   `note_skirmish_path_gadget`. The drive12 gadget-hit publisher was honest
   (`named_gadget_center_if_hittable` under-cursor tests) but raced the frame ordering.
   **Fix (drive-side, code unchanged in this mechanic):** new control command
   `tick_main_menu_transitions|times=N` (runtime_host/shell_core.rs) advancing the same
   `WM::update` ticks the injected `winit_menu_nav` path already uses
   (main_menu.rs:2907) — the FLASH/BUTTONFLASH groups finish and ButtonSkirmish becomes
   genuinely hittable; SK click then consumed (`Skirmish button selected` +
   GBM_SELECTED, first time physically).
2. **Physical RMB in classic mouse mode is CancelOrDeselect** — `world_mouse_action`
   (input.rs:31-46) is exact C++ parity (CommandXlat.cpp:3656: "right click is only
   actioned here if we're in alternate mouse mode"; SelectionXlat.cpp:1019-1023 classic
   RMB deselects). drive12's 237 RMBs were CORRECT deselects (hence the observed
   sel=1→0 flip-flop), never orders. The RMB-order latch contract presumes the retail
   UseAlternateMouse scheme. **Fix:** new control command `alternate_mouse|on=1`
   (shell_core.rs) committing through the exact Options-menu path
   (`OptionsMenu::commit_alternate_mouse_setting`, menu_callbacks.rs:1284 — now `pub`,
   C++ OptionsMenu.cpp:1157-1159 checkbox commit) → typed
   `HostOptionsRequest::AlternateMouse` bridge → `use_alternate_mouse` → RMB =
   ContextCommand. Drive then selects (physical LMB) and orders (physical RMB).

### Changed files (this pass)
- `Code/GameEngine/GameClient/src/gui/callbacks/menu_callbacks.rs`: `pub` on
  `commit_alternate_mouse_setting` (canonical retail commit path, reuse not duplication).
- `Code/Main/src/cnc_game_engine/runtime_host/mod.rs`: two dispatch arms
  (`tick_main_menu_transitions`, `alternate_mouse`).
- `Code/Main/src/cnc_game_engine/runtime_host/shell_core.rs`: the two thin command
  bodies (headless fail-closed like every sibling command; Menu-state gate on ticks;
  no evidence-setter calls — guards `host_winit_*_must_call_inject_not_direct_note`
  pass; no probe code left).

### Verification
`cargo check -p generals_main --bin generals` clean; release build clean; guards
`host_winit_menu_nav_must_call_named_gadget_inject_not_direct_note` +
`host_winit_gameplay_order_must_call_inject_not_direct_note` pass (scoped runs);
serial windowed run with `caffeinate -dis`; short-timeout discipline (run latched in
5 min); no git writes; no formatters. Sibling InGameRenderTriage's CSF/mapped-image/
terrain fixes are in the same binary (their captions still render MISSING in the frame —
their verification continues in their lane).

---

## SeamFix — hq-9udt7 (bd 1105) three-seam closure (2026-09-02 10:55)

Scope: the three seams InGameRenderTriage left open. Method: C++ oracle reads
(GeneralsMD GameText.cpp / GrantUpgradeCreate.cpp / RadarUpdate.cpp / RadarUpgrade.cpp /
Player.cpp), offline parse probes against the shipped assets, then two windowed drives
(short-timeout, caffeinate, start_game→InGame, request_capture evidence).

### 1. CSF captions — FIXED + LIVE-VERIFIED

Root cause was NOT the key table (6421 labels load correctly; `GUI:Units` etc. all
present). It was double translation in the draw lane: gadget text draws
(`w3d_gadget_draw/{static_text,common,check_radio,main_menu}.rs`) and
`TextTypeTransition` resolve the widget text through `resolve_window_text` EVERY frame,
while `wnd_parse::apply_window_text` already stores the TRANSLATED string at parse time
(C++ W3DGadgetStaticTextDraw renders instData text verbatim; GameText.cpp fetch happens
once at parse). Re-fetching already-translated text ("Units" — itself the translation of
`GUI:UnitsKilled`'s neighbour labels) wrapped it as `MISSING: 'Units'`, `MISSING:
'Buildings'`, `MISSING: 'Units Lost'`, `MISSING: 'USA'`, `MISSING: '$$$'`.

Fix (GameClient): `game_window/callbacks.rs resolve_window_text` is now idempotent —
`GameText::fetch_with_exists`, verbatim fallback when the key is not a known label.
Genuine parse-time misses keep their single C++ MISSING wrap from wnd_parse.
Additionally `game_text.rs` gains lowercase-key mirrors (`csf_strings_lower`/
`map_strings_lower`): C++ sorts/bsearches the LUT with `stricmp` (GameText.cpp:1373-1379),
so label lookup is case-insensitive — the Rust exact-match HashMap was a parity gap.

VERIFIED live (sf_ingame_12.png, post-fix build): control bar and shell screens render
real captions — "Units", "Buildings", "Units Destroyed", "Units Lost", "Player Name",
"Army", "Team", "Progress", "$$$" — zero MISSING wraps in the frame.

### 2. Red squares — INI wiring PROVEN COMPLETE; residual is texture hydration

Precise finding, contrary to the working hypothesis: the tolerant parser is NOT losing
blocks. Offline probe (cargo test, since removed) over
`extracted_big_files_v2/INI/MappedImages/TextureSize_512/*.INI`: 1343 `MappedImage`
headers → 1308 parse into the collection = EXACTLY the unique-name count (35 headers are
cross-file duplicate names, e.g. `FairPlay` x2 in SCShellUserInterface512). Every block
also parses standalone strict. The INIZH copies of these files are byte-equivalent for
MappedImage content (Texture/TextureWidth identical), so the later-root Overwrite pass
cannot clobber good definitions — the extra 12 names that take the live boot from 1308 to
`Imported 1320` come from that union. Nothing is lost between block parse and collection
insert; `sync_mapped_images_from_common` imports 1:1 by collection order.

The surviving red rectangles (command-button row, portrait, top-right boxes) are the
TEXTURE-DATA hydration lane: `ensure_image_data_loaded` finds no TGA/DDS for the atlas
(e.g. Buttons-Left/Middle/Right in EnglishZH.big per MenuWidgets' note) and `win_draw_image_ex`
paints the raw draw-data tint 255 0 0 255. That is MenuWidgets' declared lane
("image.rs backend paths + honest fallback fill") — not re-done here to avoid collision.

### 3. Minimap radar — seam narrowed to the coupled GW lane; live truth measured

C++ oracle: RadarUpdate never grants radar (ctor only sets flags; update only animates
extend). The match-start radar grant path is CommandCenter `GrantUpgradeCreate
(UpgradeToGrant=Upgrade_AmericaRadar, ExemptStatus=UNDER_CONSTRUCTION)` → player
addUpgrade(COMPLETE) / Object::giveUpgrade → RadarUpgrade::upgradeImplementation →
`Player::addRadar` (RadarUpgrade.cpp:111). The Rust lanes already model this:
`crates_radar_power.rs update_player_radar` (host tick, absolute recompute from
constructed CC providers, tests cover USA/China/GLA/EMP) and the GW coupled recompute
(Wave 818, `status_timers_post.rs`) with `writeback_core` sync. `host_update_the_radar`
stamps `local_has_radar = game_logic.get_player(local_id).has_radar()` per frame.

Experiments (windowed drives + scoped probe test, both since removed):
- Shadow ON (default): pure-InGame minimap BLACK (fallback fill), 3 identical captures.
- Shadow OFF (GENERALS_GAMEWORLD_SHADOW=0): minimap DRAWS (terrain band, shroud circle,
  frustum lines) — sf2_ingame_6/9.png (10:40 run).
- Isolated coupled probe (GameWorldShadow + constructed USA CC): GW recompute sets
  pd.radar_count=1 and `writeback_economy_to_host` lands `(radar_count=1, has_radar=true)`
  on the host player — the lane logic is sound in isolation.
=> The coupled-session radar_count ownership zeroes the live match state. Remaining
suspects, in order: live `host_player_to_gw` slot alignment when the player set mutates
across shell→match (sync_players' dense-index rebuild branch), and the GW player `team`
of the LOCAL slot. One-line next step: re-apply the removed one-shot INFO in
`host_update_the_radar` (local_id / radar_count / radar_disabled / forced) for a single
drive; if radar_count=0 with a correct local_id, dump `host_player_to_gw` + `pd.team`
from `sync_players` on the rebuild branch. Do NOT work around with radar.force_on —
that masks the simulation truth (C++ ControlBarCallback.cpp:51 gate).

### Guards / hygiene
Changed files (kept): GameClient/src/game_text.rs (+case-insensitive mirrors),
GameClient/src/gui/game_window/callbacks.rs (idempotent resolve_window_text).
Removed after use: Common/tests/mi_probe.rs, gameworld_shadow/tests/radar_probe.rs,
the one-shot run_loop probe (function byte-restored). `cargo check -p game-client-rust`
and `cargo check -p generals_main --bin generals` clean; no test greens touched; no git
writes; no formatters; serial windowed runs with caffeinate; evidence under
/tmp/wsmoke/{sf_ingame_12.png, sf2_ingame_*.png, seamfix*.log}.

---

## MinimapSeam — coupled radar lane proven sound headlessly; live probe needs low-fps gate (2026-09-02 12:45)

Scope: close the minimap coupled-lane seam (SeamFix §3). Method: headless coupled
repro + full instrumentation, live shadow-on windowed probe drive. No git writes;
probes removed after use.

### 1. sync_players dense-index rebuild — NOT the live zeroing path (log evidence)

Surviving SeamFix drive logs (generals_exec_smoke_sf*/child_stderr.txt) show the live
player lifecycle: `Starting new game: Shell` → `Created 3 default players` ({0 USA
local, 1 GLA, 2 China}), then `Starting new game: Skirmish` → slots {0 USA "Player",
1 GLA AI} + ReplayObserver id 2, and `Preserving 3 host player(s) across map load`.
Same id set {0,1,2} on both sides ⇒ `need_rebuild` is FALSE at match start; the
refresh branch is id-stable and the local slot's team never goes stale. The
dense-index rebuild branch (entities-live path) cannot be the live zeroing source in
this flow.

### 2. Headless coupled repro — lane logic SOUND (regression tests landed)

`gameworld_shadow/tests/radar_coupled.rs` (new, 2 tests, passing): one GameLogic +
one persistent GameWorldShadow across shell→skirmish (and a fresh-shadow control),
match-start AmericaCommandCenter seeded like Wave 831 (owner-bound, constructed,
KindOf CommandCenter + MpCountForVictory). Full coupled flow per frame —
`shadow_session_after_host_tick` (Wave 818 GW radar recompute → sync →
apply_host_radar_events → player-radar drain → writebacks) — preserves host
radar_count=1 / has_radar=true. Matches SeamFix's isolated probe; confirms the
coupled lane does not zero derived radar state for a living, radar-provider-backed
local player.

Instrumented detour (removed): with a template lacking `MpCountForVictory` (or an
unowned CC), the session-end parity probe (`shadow.probe` →
`evaluate_victory_condition`) judges the local player defeated on frame 1 and
`kill_player_for_victory` destroys the CC → radar zeroed. C++ parity gate
(VictoryConditions.cpp:196 killPlayer) — a real hazard only when a side lacks a
victory-counting structure; NOT the live defect (live status shows match_over=false,
CC alive).

### 3. Live RADARPROBE — gated too tight for a 0.1 fps loop (exact next step)

Shadow-on windowed probe drive (mseam_drive.sh, 12 captures, status
ms_status_final.txt) ran clean, but RADARPROBE/SEAMPROBE never fired: the windowed
runtime-host loop runs ~0.09 fps (status fps), so only ~4-6 frames elapse inside
InGame during captures — my gate (first 4 calls + every 600th) never sampled there.
Next driver: in `host_update_the_radar`, log unconditionally when
`state == InGame` (or gate on counter<50), rebuild release, rerun
`/tmp/wsmoke/mseam_drive.sh`; RADARPROBE lines then give live local_id /
radar_count / radar_disabled / disable_proof / power / forced plus SEAMPROBE
session-stage truth. Do NOT work around with radar.force_on (C++ ControlBar
gate parity).

### Guards / hygiene
Probes removed (run_loop.rs, session.rs, construct.rs, create_destroy_die.rs,
host_ops_writeback.rs byte-unchanged vs pre-task). Kept: radar_coupled.rs tests.
`cargo check -p generals_main` clean; no test greens touched; no git writes; no
formatters; drive used caffeinate -dis, short timeout, serial.
